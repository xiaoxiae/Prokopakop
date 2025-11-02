use clap::{Arg, Command};
use prokopakop::controller::cli::GUICommand;
use prokopakop::controller::controller::{GameController, MoveResultType};
use prokopakop::controller::training::{TrainingConfig, TrainingDataGenerator};
use prokopakop::game::bitboard::generate_magic_bitboards;
use prokopakop::game::board::BoardMoveExt;

fn main() {
    env_logger::init();

    // Parse command line arguments
    let matches = Command::new("Prokopakop")
        .version("1.0")
        .about("UCI Chess Engine, made to kop the Prokop")
        .arg(
            Arg::new("magic")
                .long("magic")
                .help("Generate magic bitboards")
                .num_args(0),
        )
        .arg(
            Arg::new("training")
                .long("training")
                .help("Generate NNUE training data through self-play")
                .num_args(0),
        )
        .arg(
            Arg::new("games")
                .short('g')
                .long("games")
                .value_name("NUM")
                .help("Number of games to play (default: 32)")
                .default_value("32"),
        )
        .arg(
            Arg::new("depth")
                .short('d')
                .long("depth")
                .value_name("DEPTH")
                .help("Fixed search depth per move (default: 8)")
                .default_value("8"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output file for training data (default: data/selfplay.txt)")
                .default_value("data/selfplay.txt"),
        )
        .arg(
            Arg::new("start-moves-min")
                .long("start-moves-min")
                .value_name("NUM")
                .help("Minimum number of random starting moves (default: 1)")
                .default_value("1"),
        )
        .arg(
            Arg::new("start-moves-max")
                .long("start-moves-max")
                .value_name("NUM")
                .help("Maximum number of random starting moves (default: 6)")
                .default_value("6"),
        )
        .get_matches();

    // Handle magic flag
    if matches.get_flag("magic") {
        generate_magic_bitboards();
        return;
    }

    // Handle training flag
    if matches.get_flag("training") {
        let num_games = matches
            .get_one::<String>("games")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap();

        let search_depth = matches
            .get_one::<String>("depth")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap();

        let output_file = matches
            .get_one::<String>("output")
            .map(|s| s.as_str())
            .unwrap();

        let start_moves_min = matches
            .get_one::<String>("start-moves-min")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap();

        let start_moves_max = matches
            .get_one::<String>("start-moves-max")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap();

        if start_moves_min > start_moves_max {
            eprintln!("Error: start-moves-min must be <= start-moves-max");
            std::process::exit(1);
        }

        eprintln!("=== NNUE Training Data Generator ===");
        eprintln!("Games: {}", num_games);
        eprintln!("Search depth: {}", search_depth);
        eprintln!("Starting moves: {} - {}", start_moves_min, start_moves_max);
        eprintln!("Output file: {}", output_file);
        eprintln!();

        let config = TrainingConfig::new(num_games, search_depth, start_moves_min, start_moves_max);
        let generator = TrainingDataGenerator::new(config);

        // Generate training data in parallel and write immediately to file
        match generator.generate_parallel_to_file(output_file) {
            Ok(total_positions) => {
                eprintln!();
                eprintln!("Training data successfully exported to: {}", output_file);
                eprintln!("Total positions collected: {}", total_positions);
            }
            Err(e) => {
                eprintln!("Error during training: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    let mut controller = GameController::new();

    // Interactive UCI mode
    loop {
        let input = GUICommand::receive();

        match input {
            GUICommand::Quit => {
                // Make sure to stop any ongoing search before quitting
                controller.stop_search();
                break;
            }
            GUICommand::UCI => {
                let name = "Prokopakop";
                let author = "Tomíno Komíno";

                println!("id name {}", name);
                println!("id author {}", author);

                controller.print_uci_options();

                println!("uciok");

                controller.initialize();
            }
            _ if !controller.is_initialized() => {
                // Ignore commands until UCI initialization
                continue;
            }
            GUICommand::FenPosition(fen) => controller.set_board_from_fen(fen.as_str()),
            GUICommand::MovePosition(moves) => {
                controller.reset_board();

                if let Some(moves_strings) = moves {
                    for notation in moves_strings {
                        match controller.try_move_piece(&notation) {
                            MoveResultType::Success => (),
                            MoveResultType::InvalidMove => {
                                eprintln!("Invalid move: {}", notation);
                            }
                            MoveResultType::InvalidNotation => {
                                eprintln!("Invalid notation format: {}", notation);
                            }
                        }
                    }
                } else {
                    controller.reset_transposition_table();
                }
            }
            GUICommand::SetOption(name, value) => {
                controller.set_option(name.as_str(), value.as_str())
            }
            GUICommand::IsReady => println!("readyok"),
            GUICommand::Search(params) => controller.search(params, true),
            GUICommand::Perft(depth_string) => {
                let moves = controller.perft(depth_string.parse::<usize>().unwrap());

                let mut total = 0;
                for (m, c) in &moves {
                    println!("{}: {}", m.unparse(), c);
                    total += c;
                }

                println!("\nNodes: {}", total);
            }
            GUICommand::Stop => {
                let _ = controller.stop_search();
            }
            GUICommand::Eval => controller.print_evaluation(),
            GUICommand::Joke => controller.tell_joke(),
            GUICommand::Invalid(command) => eprintln!("Invalid command: {}", command),
        }
    }
}
