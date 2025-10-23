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

const HIDDEN_SIZE: usize = 128;
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;

// Hyperparameter configuration struct for the optimization experiment
struct HyperparamConfig {
    experiment_name: String,
    wdl: f32,
    start_lr: f32,
    gamma: f32,
}

impl HyperparamConfig {
    fn checkpoint_dir(&self) -> String {
        format!(
            "{}/wdl_{:.2}_lr_{:.5}_gamma_{:.2}",
            self.experiment_name, self.wdl, self.start_lr, self.gamma
        )
    }
}

fn main() {
    // Experiment:
    //    let wdl_values = [0.0, 0.25, 0.5];
    //    let lr_values = [0.001, 0.0005, 0.0001];
    //    let gamma_values = [0.1, 0.5, 0.9];
    //    let superbatches = 90;
    //    let experiment_name = "experiment-1";
    //
    // Observations:
    //    LR didn't matter too much
    //    wdl of 0.5 is garbage
    //    gamma 0.1 < 0.5 < 0.9 (probably)
    //
    // --------------------------------------------------
    // Rank Name                                          Elo        +/-       nElo        +/-      Games      Score       Draw           Ptnml(0-2)
    //    1 nnue-wdl_0.25_lr_0.00010_gamma_0.90-18     112.42      30.50     116.75      29.47        534      65.6%      29.2% [25, 24, 78, 39, 101]
    //    2 nnue-wdl_0.25_lr_0.00100_gamma_0.90-24     105.97      29.01     114.76      29.47        534      64.8%      28.1% [23, 26, 75, 56, 87]
    //    3 nnue-wdl_0.00_lr_0.00050_gamma_0.50-11     104.54      27.71     118.33      29.47        534      64.6%      33.3% [17, 28, 89, 48, 85]
    //    4 nnue-wdl_0.00_lr_0.00010_gamma_0.90-9       93.60      28.41     101.94      29.41        536      63.2%      30.2% [23, 30, 81, 51, 83]
    //    5 nnue-wdl_0.00_lr_0.00100_gamma_0.10-13      91.87      27.83     102.14      29.47        534      62.9%      33.3% [25, 21, 89, 55, 77]
    //    6 nnue-wdl_0.25_lr_0.00100_gamma_0.10-22      91.17      27.80     101.39      29.47        534      62.8%      33.7% [21, 29, 90, 46, 81]
    //    7 nnue-wdl_0.00_lr_0.00050_gamma_0.90-12      91.17      26.98     104.48      29.47        534      62.8%      32.6% [16, 36, 87, 51, 77]
    //    8 nnue-wdl_0.00_lr_0.00100_gamma_0.90-15      87.01      25.83     103.70      29.47        534      62.3%      33.0% [15, 35, 88, 62, 67]
    //    9 nnue-wdl_0.25_lr_0.00050_gamma_0.90-21      86.31      29.87      88.96      29.47        534      62.2%      27.0% [32, 27, 72, 51, 85]
    //   10 nnue-wdl_0.25_lr_0.00050_gamma_0.50-20      85.62      27.44      95.94      29.47        534      62.1%      32.6% [23, 28, 87, 55, 74]
    //   11 nnue-wdl_0.25_lr_0.00100_gamma_0.50-23      82.18      27.17      92.71      29.47        534      61.6%      33.7% [23, 28, 90, 54, 72]
    //   12 nnue-wdl_0.00_lr_0.00010_gamma_0.50-8       79.43      28.57      85.04      29.47        534      61.2%      27.0% [30, 29, 72, 63, 73]
    //   13 nnue-wdl_0.25_lr_0.00010_gamma_0.10-16      79.43      28.25      86.01      29.47        534      61.2%      29.6% [30, 25, 79, 61, 72]
    //   14 nnue-wdl_0.25_lr_0.00050_gamma_0.10-19      77.38      28.92      81.69      29.47        534      61.0%      28.5% [24, 43, 76, 40, 84]
    //   15 nnue-wdl_0.25_lr_0.00010_gamma_0.50-17      72.61      29.70      74.36      29.47        534      60.3%      27.0% [34, 32, 72, 48, 81]
    //   16 nnue-wdl_0.00_lr_0.00010_gamma_0.10-7       71.25      26.86      80.56      29.47        534      60.1%      32.6% [23, 35, 87, 55, 67]
    //   17 nnue-wdl_0.00_lr_0.00050_gamma_0.10-10      65.84      28.55      69.76      29.47        534      59.4%      24.3% [29, 43, 65, 59, 71]
    //   18 nnue-wdl_0.00_lr_0.00100_gamma_0.50-14      60.46      27.70      65.77      29.47        534      58.6%      30.3% [29, 37, 81, 53, 67]
    //   19 nnue-wdl_0.50_lr_0.00100_gamma_0.50-32     -29.35      28.54     -30.52      29.47        534      45.8%      32.2% [66, 34, 86, 41, 40]
    //   20 nnue-wdl_0.50_lr_0.00050_gamma_0.90-30     -48.46      28.48     -50.92      29.47        534      43.1%      31.5% [65, 51, 84, 27, 40]
    //   21 nnue-wdl_0.50_lr_0.00050_gamma_0.10-28     -54.44      27.51     -59.41      29.47        534      42.2%      33.0% [65, 49, 88, 34, 31]
    //   22 9119428-6                                  -57.78      26.57     -65.42      29.47        534      41.8%      27.0% [59, 64, 72, 50, 22]
    //   23 nnue-wdl_0.50_lr_0.00100_gamma_0.90-33     -65.84      28.86     -69.02      29.47        534      40.6%      30.7% [74, 49, 82, 27, 35]
    //   24 nnue-wdl_0.50_lr_0.00050_gamma_0.50-29     -69.89      28.42     -74.64      29.47        534      40.1%      31.5% [75, 47, 84, 31, 30]
    //   25 nnue-wdl_0.50_lr_0.00100_gamma_0.10-31     -72.21      28.61     -76.86      29.52        532      39.8%      30.1% [75, 50, 80, 31, 30]
    //   26 c056f9b-5                                  -73.01      26.51     -83.60      29.41        536      39.6%      22.0% [57, 85, 59, 46, 21]
    //   27 nnue-wdl_0.50_lr_0.00010_gamma_0.90-27     -79.43      29.63     -82.00      29.47        534      38.8%      28.8% [84, 45, 77, 29, 32]
    //   28 nnue-wdl_0.50_lr_0.00010_gamma_0.50-26     -84.93      28.41     -91.89      29.47        534      38.0%      31.8% [83, 42, 85, 34, 23]
    //   29 nnue-wdl_0.50_lr_0.00010_gamma_0.10-25     -93.63      28.81    -100.96      29.52        532      36.8%      28.9% [83, 52, 77, 30, 24]
    //   30 5cc228e-3                                 -100.90      26.53    -118.35      29.36        538      35.9%      24.2% [68, 84, 65, 36, 16]
    //   31 910fb21-4                                 -101.00      25.75    -122.48      29.47        534      35.9%      27.0% [68, 76, 72, 41, 10]
    //   32 c144fb9-2                                 -147.19      25.67    -181.16      28.03        590      30.0%      26.1%  [99, 82, 77, 30, 7]
    //   33 8024a6e-1                                 -176.33      28.25    -206.65      27.94        594      26.6%      25.9% [127, 63, 77, 21, 9]
    //   34 e6662f0-0                                 -277.49      35.58    -325.74      27.94        594      16.8%      19.9%  [189, 35, 59, 9, 5]
    // --------------------------------------------------
    //

    let wdl_values = [0.0, 0.125, 0.25];
    let lr_values = [0.001, 0.0005, 0.0001];
    let gamma_values = [0.5, 0.9];
    let experiment_name = "experiment-2";

    let superbatches = 180;

    let mut configs = Vec::new();
    for &wdl in &wdl_values {
        for &start_lr in &lr_values {
            for &gamma in &gamma_values {
                configs.push(HyperparamConfig {
                    experiment_name: experiment_name.to_string(),
                    wdl,
                    start_lr,
                    gamma,
                });
            }
        }
    }

    println!("Starting hyperparameter optimization experiment");
    println!("Testing {} configurations", configs.len());
    println!();

    for (idx, config) in configs.iter().enumerate() {
        println!(
            "[{}/{}] Training: WDL={}, LR={}, Gamma={}",
            idx + 1,
            configs.len(),
            config.wdl,
            config.start_lr,
            config.gamma
        );

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
                SavedFormat::id("l0w").round().quantise::<i16>(QA),
                SavedFormat::id("l0b").round().quantise::<i16>(QA),
                SavedFormat::id("l1w").round().quantise::<i16>(QB),
                SavedFormat::id("l1b").round().quantise::<i16>(QA * QB),
            ])
            // map output into ranges [0, 1] to fit against our labels which
            // are in the same range
            // `target` == wdl * game_result + (1 - wdl) * sigmoid(search score in centipawns / SCALE)
            // where `wdl` is determined by `wdl_scheduler`
            .loss_fn(|output, target| output.sigmoid().squared_error(target))
            // the basic `(768 -> N)x2 -> 1` inference
            .build(|builder, stm_inputs, ntm_inputs| {
                // weights
                let l0 = builder.new_affine("l0", 768, HIDDEN_SIZE);
                let l1 = builder.new_affine("l1", 2 * HIDDEN_SIZE, 1);

                // inference
                let stm_hidden = l0.forward(stm_inputs).screlu();
                let ntm_hidden = l0.forward(ntm_inputs).screlu();
                let hidden_layer = stm_hidden.concat(ntm_hidden);
                l1.forward(hidden_layer)
            });

        let schedule = TrainingSchedule {
            net_id: "simple".to_string(),
            eval_scale: SCALE as f32,
            steps: TrainingSteps {
                batch_size: 16_384,
                batches_per_superbatch: 6104,
                start_superbatch: 1,
                end_superbatch: superbatches,
            },
            wdl_scheduler: wdl::ConstantWDL { value: config.wdl },
            lr_scheduler: lr::StepLR {
                start: config.start_lr,
                gamma: config.gamma,
                step: 30,
            },
            save_rate: 10,
        };

        let settings = LocalSettings {
            threads: 32,
            test_set: None,
            output_directory: &config.checkpoint_dir(),
            batch_queue_size: 64,
        };

        // loading directly from a `BulletFormat` file
        let data_loader = loader::DirectSequentialDataLoader::new(&["data/baseline.data"]);

        trainer.run(&schedule, &settings, &data_loader);
        println!();
    }

    println!("Hyperparameter optimization experiment completed!");
}
