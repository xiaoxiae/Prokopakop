/*
Yoinked from https://github.com/jw1912/bullet/blob/main/examples/simple.rs
*/
use bullet_lib::{
    game::inputs,
    nn::optimiser,
    trainer::{
        save::SavedFormat,
        schedule::{TrainingSchedule, TrainingSteps, lr, wdl},
        settings::LocalSettings,
    },
    value::{ValueTrainerBuilder, loader},
};
use bulletformat::{BulletFormat, ChessBoard};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "train")]
#[command(about = "Prokopakop's NNUE trainer + utilities", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run hyperparameter optimization training
    Train {
        /// Path to experiment directory containing config.toml
        #[arg(value_name = "DIR")]
        experiment_dir: String,
    },
    /// Deduplicate FEN positions from a file
    Deduplicate {
        /// Path to input file (or stdin if not provided)
        #[arg(value_name = "FILE")]
        input_file: Option<String>,
    },
    /// Convert text FEN to binary format
    Convert {
        /// Path to input file (text FEN)
        #[arg(short, long)]
        input: PathBuf,
        /// Path to output file (binary)
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TrainingConfig {
    // Hyperparameter search ranges
    wdl_values: Vec<f32>,
    lr_values: Vec<f32>,
    gamma_values: Vec<f32>,

    // Training parameters
    lr_step: usize,
    repeats: usize,

    // Network architecture
    hidden_size: usize,

    // Evaluation and scaling
    eval_scale: i32,
    qa: i16,
    qb: i16,

    // Training steps
    batch_size: usize,
    batches_per_superbatch: usize,
    start_superbatch: usize,
    end_superbatch: usize,

    // Compute settings
    threads: usize,
    batch_queue_size: usize,

    // Data
    data_path: String,
}

impl TrainingConfig {
    fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let config: TrainingConfig = toml::from_str(&contents)?;
        Ok(config)
    }
}

// Hyperparameter configuration struct for the optimization experiment
struct HyperparamConfig {
    experiment_dir: PathBuf,
    wdl: f32,
    start_lr: f32,
    gamma: f32,
}

impl HyperparamConfig {
    fn checkpoint_dir(&self, idx: usize) -> PathBuf {
        self.experiment_dir.join(format!(
            "wdl_{:.2}_lr_{:.5}_gamma_{:.2}_{}",
            self.wdl, self.start_lr, self.gamma, idx
        ))
    }
}

fn run_train(experiment_dir_str: &str) {
    let experiment_dir = PathBuf::from(experiment_dir_str);
    if !experiment_dir.is_dir() {
        eprintln!("Error: {} is not a valid directory", experiment_dir_str);
        std::process::exit(1);
    }

    let config_path = experiment_dir.join("config.toml");
    if !config_path.exists() {
        eprintln!(
            "Error: config.toml not found in {}",
            experiment_dir.display()
        );
        std::process::exit(1);
    }

    let config = TrainingConfig::load(config_path.to_string_lossy().as_ref())
        .expect("Failed to load config file");

    let mut configs = Vec::new();
    for &wdl in &config.wdl_values {
        for &start_lr in &config.lr_values {
            for &gamma in &config.gamma_values {
                configs.push(HyperparamConfig {
                    experiment_dir: experiment_dir.clone(),
                    wdl,
                    start_lr,
                    gamma,
                });
            }
        }
    }

    println!("Starting hyperparameter optimization experiment");
    println!("Experiment directory: {}", experiment_dir.display());
    println!("Testing {} configurations", configs.len());
    println!();

    for i in 0..config.repeats {
        for (idx, hyperparam_config) in configs.iter().enumerate() {
            let checkpoint_dir = hyperparam_config.checkpoint_dir(i);

            // Skip if checkpoint directory already exists
            if checkpoint_dir.exists() {
                println!(
                    "[{}/{}] Skipping (already exists): WDL={}, LR={}, Gamma={}",
                    idx + 1,
                    configs.len(),
                    hyperparam_config.wdl,
                    hyperparam_config.start_lr,
                    hyperparam_config.gamma
                );
                continue;
            }

            println!(
                "[{}/{}] Training: WDL={}, LR={}, Gamma={}",
                idx + 1,
                configs.len(),
                hyperparam_config.wdl,
                hyperparam_config.start_lr,
                hyperparam_config.gamma
            );

            let checkpoint_dir_str = checkpoint_dir.to_string_lossy().to_string();

            let mut trainer = ValueTrainerBuilder::default()
                // makes `ntm_inputs` available below
                .dual_perspective()
                // standard optimiser used in NNUE
                // the default AdamW params include clipping to range [-1.98, 1.98]
                .optimiser(optimiser::AdamW)
                // basic piece-square chessboard inputs
                .inputs(inputs::Chess768)
                // chosen such that inference may be efficiently implemented in-engine
                .save_format(&[
                    SavedFormat::id("l0w").round().quantise::<i16>(config.qa),
                    SavedFormat::id("l0b").round().quantise::<i16>(config.qa),
                    SavedFormat::id("l1w").round().quantise::<i16>(config.qb),
                    SavedFormat::id("l1b")
                        .round()
                        .quantise::<i16>((config.qa as i32 * config.qb as i32) as i16),
                ])
                // map output into ranges [0, 1] to fit against our labels which
                // are in the same range
                // `target` == wdl * game_result + (1 - wdl) * sigmoid(search score in centipawns / eval_scale)
                // where `wdl` is determined by `wdl_scheduler`
                .loss_fn(|output, target| output.sigmoid().squared_error(target))
                // the basic `(768 -> N)x2 -> 1` inference
                .build(|builder, stm_inputs, ntm_inputs| {
                    // weights
                    let l0 = builder.new_affine("l0", 768, config.hidden_size);
                    let l1 = builder.new_affine("l1", 2 * config.hidden_size, 1);

                    // inference
                    let stm_hidden = l0.forward(stm_inputs).screlu();
                    let ntm_hidden = l0.forward(ntm_inputs).screlu();
                    let hidden_layer = stm_hidden.concat(ntm_hidden);
                    l1.forward(hidden_layer)
                });

            let schedule = TrainingSchedule {
                net_id: format!("experiment"),
                eval_scale: config.eval_scale as f32,
                steps: TrainingSteps {
                    batch_size: config.batch_size,
                    batches_per_superbatch: config.batches_per_superbatch,
                    start_superbatch: config.start_superbatch,
                    end_superbatch: config.end_superbatch,
                },
                wdl_scheduler: wdl::ConstantWDL {
                    value: hyperparam_config.wdl,
                },
                lr_scheduler: lr::StepLR {
                    start: hyperparam_config.start_lr,
                    gamma: hyperparam_config.gamma,
                    step: config.lr_step,
                },
                save_rate: 10,
            };

            let settings = LocalSettings {
                threads: config.threads,
                test_set: None,
                output_directory: &checkpoint_dir_str,
                batch_queue_size: config.batch_queue_size,
            };

            // loading directly from a `BulletFormat` file
            let data_file = experiment_dir.join(&config.data_path);
            let data_file_str = data_file.to_string_lossy().to_string();
            let data_loader = loader::DirectSequentialDataLoader::new(&[&data_file_str]);

            trainer.run(&schedule, &settings, &data_loader);
            println!();
        }
    }

    println!("Hyperparameter optimization experiment completed!");
}

fn run_deduplicate(input_file: Option<&str>) {
    let mut seen_fens = std::collections::HashSet::new();

    let reader: Box<dyn BufRead> = if let Some(file_path) = input_file {
        Box::new(io::BufReader::new(
            fs::File::open(file_path).unwrap_or_else(|e| {
                eprintln!("Error opening file '{}': {}", file_path, e);
                std::process::exit(1);
            }),
        ))
    } else {
        Box::new(io::BufReader::new(io::stdin()))
    };

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error reading line: {}", e);
                continue;
            }
        };

        let line = line.trim_end_matches('\n');
        if line.is_empty() {
            continue;
        }

        // Extract FEN (everything before the first |)
        let fen = line.split('|').next().unwrap_or("").trim();

        if !seen_fens.contains(fen) {
            seen_fens.insert(fen.to_string());
            println!("{}", line);
        }
    }
}

fn convert_text(
    inp_path: impl AsRef<Path>,
    out_path: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let timer = Instant::now();

    let file = BufReader::new(File::open(&inp_path)?);

    let mut data = Vec::new();

    let mut results = [0, 0, 0];

    let mut output = BufWriter::new(File::create(&out_path)?);

    for line in file.lines() {
        match line?.parse::<ChessBoard>() {
            Ok(pos) => {
                results[pos.result_idx()] += 1;
                data.push(pos);
            }
            Err(message) => println!("error parsing: {message}"),
        }

        if data.len() % 16384 == 0 {
            BulletFormat::write_to_bin(&mut output, &data)?;
            data.clear();
        }
    }

    BulletFormat::write_to_bin(&mut output, &data)?;

    println!(
        "Summary: {} Positions in {:.2} seconds",
        results.iter().sum::<u64>(),
        timer.elapsed().as_secs_f32()
    );
    println!(
        "Wins: {}, Draws: {}, Losses: {}",
        results[2], results[1], results[0]
    );

    Ok(())
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Train { experiment_dir } => {
            run_train(&experiment_dir);
        }
        Commands::Deduplicate { input_file } => {
            run_deduplicate(input_file.as_deref());
        }
        Commands::Convert { input, output } => {
            if let Err(e) = convert_text(&input, &output) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}
