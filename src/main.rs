mod controller;
mod game;
mod test;
mod utils;

use clap::{Arg, Command};
use controller::game_controller::{GameController, MoveResultType};
use game::board::BoardMoveExt;
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
        .get_matches();

    // Handle magic flag
    if matches.get_flag("magic") {
        generate_magic_bitboards();
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
            GUICommand::FenPosition(fen) => controller.new_game_from_fen(fen.as_str()),
            GUICommand::MovePosition(moves) => {
                controller.new_game();

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
            GUICommand::Invalid(command) => eprintln!("Invalid command: {}", command),
        }
    }
}
