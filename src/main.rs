mod controller;
mod game;
mod test;
mod utils;

use clap::{Arg, Command};
use controller::game_controller::{GameController, MoveResultType};
use game::board::{BoardMoveExt, Game};
use game::opening_book::OpeningBook;
use game::pgn::parse_pgn_file;
use utils::bitboard::generate_magic_bitboards;
use utils::cli::GUICommand;

fn main() {
    env_logger::init();

    // Parse command line arguments
    let matches = Command::new("Prokopakop")
        .version("1.0")
        .about("UCI Chess Engine")
        .arg(
            Arg::new("magic")
                .long("magic")
                .help("Generate magic bitboards")
                .num_args(0),
        )
        .arg(
            Arg::new("build-book")
                .long("build-book")
                .help("Build opening book from PGN files")
                .value_name("PGN_FILES")
                .num_args(1..)
                .requires("output"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .help("Output file for opening book")
                .value_name("FILE")
                .num_args(1),
        )
        .arg(
            Arg::new("max-positions")
                .long("max-positions")
                .help("Maximum number of positions to keep")
                .value_name("COUNT")
                .num_args(1)
                .default_value("65536"),
        )
        .get_matches();

    // Handle magic flag
    if matches.get_flag("magic") {
        generate_magic_bitboards();
        return;
    }

    // Handle build-book command
    if let Some(pgn_files) = matches.get_many::<String>("build-book") {
        let output_file = matches.get_one::<String>("output").unwrap();

        let max_positions = matches
            .get_one::<String>("max-positions")
            .unwrap()
            .parse::<usize>()
            .unwrap();

        build_opening_book(pgn_files.collect(), output_file, max_positions);
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
            GUICommand::Search(params) => controller.search(params),
            GUICommand::Perft(depth_string) => {
                let moves = controller.perft(depth_string.parse::<usize>().unwrap());

                let mut total = 0;
                for (m, c) in &moves {
                    println!("{}: {}", m.unparse(), c);
                    total += c;
                }

                println!("\nNodes: {}", total);
            }
            GUICommand::Stop => controller.stop_search(),
            GUICommand::Eval => controller.print_detailed_evaluation(),
            GUICommand::Invalid(command) => eprintln!("Invalid command: {}", command),
        }
    }
}

fn build_opening_book(pgn_files: Vec<&String>, output_file: &str, max_positions: usize) {
    println!(
        "Building opening book from {} PGN file(s)...",
        pgn_files.len()
    );

    let mut book = OpeningBook::new();
    let mut total_games = 0;
    let mut successful_games = 0;

    for pgn_file in pgn_files {
        println!("Processing {}...", pgn_file);

        match parse_pgn_file(pgn_file) {
            Ok(games) => {
                println!("Found {} games in {}", games.len(), pgn_file);

                for pgn_game in games {
                    if pgn_game.average_elo.is_none() {
                        continue;
                    }

                    total_games += 1;

                    // Create a new game and play through the moves
                    let mut game = Game::new(None);
                    let positions = game.record_position_sequence(&pgn_game.moves);

                    // Only record positions from the first 20 moves (opening phase)
                    let opening_positions: Vec<_> = positions.into_iter().take(20).collect();

                    if !opening_positions.is_empty() {
                        book.add_game(
                            opening_positions,
                            pgn_game.result,
                            pgn_game.average_elo.unwrap(),
                        );
                        successful_games += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("Error parsing {}: {:?}", pgn_file, e);
            }
        }
    }

    println!(
        "Processed {} games total, {} successful",
        total_games, successful_games
    );
    println!("Raw book size: {} positions", book.position_count());

    if book.position_count() > max_positions {
        println!("Pruning to maximum {} positions...", max_positions);
        book.prune_by_size(max_positions);
        println!("Final book size: {} positions", book.position_count());
    }

    // Save to file
    match book.save_to_file(output_file) {
        Ok(()) => {
            println!("Opening book saved to {}", output_file);
            println!("Final statistics:");
            println!("  Positions: {}", book.position_count());
            println!("  Total games referenced: {}", book.total_games());
        }
        Err(e) => {
            eprintln!("Error saving opening book: {}", e);
        }
    }
}
