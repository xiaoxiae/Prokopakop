mod controller;
mod game;
mod test;
mod utils;

pub use crate::controller::*;
use crate::game::{MoveResultType, Piece};
pub use crate::utils::*;
use clap::{Arg, Command};

fn main() {
    env_logger::init();

    // Parse command line arguments
    let matches = Command::new("Prokopakop")
        .version("1.0")
        .about("Na Prokopání.")
        .arg(
            Arg::new("fen")
                .long("fen")
                .value_name("FEN")
                .help("Sets initial position from FEN string")
                .num_args(1),
        )
        .arg(
            Arg::new("perft")
                .long("perft")
                .value_name("DEPTH")
                .help("Run perft to specified depth")
                .num_args(1),
        )
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

    // Handle FEN position
    if let Some(fen) = matches.get_one::<String>("fen") {
        controller.initialize(ControllerMode::Play);
        controller.new_game(Some(fen.as_str()));
    }

    // Handle perft
    if let Some(depth_str) = matches.get_one::<String>("perft") {
        if let Ok(depth) = depth_str.parse::<usize>() {
            // Initialize if not already done
            if controller.mode.is_none() {
                controller.initialize(ControllerMode::Play);
                controller.new_game(None);
            }

            let moves = controller.get_valid_moves(depth);
            let mut total = 0;
            for (m, c) in &moves {
                println!("{}: {}", m.unparse(), c);
                total += c;
            }
            println!("\nNodes: {}", total);
            return;
        }
    }

    // Interactive mode
    loop {
        let input = receive();

        // Common commands
        match (&input, controller.mode.is_some()) {
            (Some(GUICommand::Quit), _) => break,
            (Some(GUICommand::Position(fen)), true) => {
                match fen {
                    None => controller.new_game(None),
                    Some(fen) => controller.new_game(Some(fen.as_str())),
                }

                if let Some(ControllerMode::Play) = controller.mode {
                    controller.print();
                }
            }
            _ => {}
        }

        match controller.mode {
            // UCI-only commands
            Some(ControllerMode::UCI) => match input {
                Some(GUICommand::IsReady) => respond(BotCommand::ReadyOk),
                Some(GUICommand::ValidMoves(depth_string)) => {
                    let moves = controller.get_valid_moves(depth_string.parse::<usize>().unwrap());

                    let mut total = 0;
                    for (m, c) in &moves {
                        println!("{}: {}", m.unparse(), c);
                        total += c;
                    }

                    println!("\nNodes: {}", total);
                }
                _ => {}
            },
            // Player-only commands
            Some(ControllerMode::Play) => match input {
                Some(GUICommand::Move(notation)) => {
                    let result = controller.try_move_piece(notation);

                    match result {
                        MoveResultType::Success => controller.print(),
                        _ => log::info!("{:?}", result),
                    };
                }
                Some(GUICommand::Unmove) => {
                    let result = controller.try_unmove_piece();

                    match result {
                        MoveResultType::Success => controller.print(),
                        _ => log::info!("{:?}", result),
                    };
                }
                Some(GUICommand::Status) => controller.print(),
                Some(GUICommand::Fen) => controller.print_fen(),
                Some(GUICommand::ValidMoves(square_or_depth_string)) => {
                    // If parsing as move works, it has to be depth
                    match BoardSquare::parse(square_or_depth_string.as_str()) {
                        Some(square) => {
                            let moves = controller.get_valid_moves(0);

                            let moves_to = moves
                                .iter()
                                .map(|(m, _)| m)
                                .filter(|m| m.from == square)
                                .map(|m| &m.to)
                                .collect::<Vec<_>>();

                            controller.print_with_moves(moves_to);
                        }
                        None => {
                            let moves = controller.get_valid_moves(
                                square_or_depth_string.parse::<usize>().unwrap_or(1),
                            );

                            let mut total = 0;
                            for (m, c) in &moves {
                                println!("{}: {}", m.unparse(), c);
                                total += c;
                            }

                            println!("\nNodes: {}", total);
                        }
                    }
                }
                _ => {}
            },
            None => match input {
                Some(GUICommand::UCI) => {
                    let name = "Prokopakop";
                    let author = "Tomíno Komíno";

                    respond(BotCommand::Identify(name.to_string(), author.to_string()));
                    respond(BotCommand::UCIOk);

                    controller.initialize(ControllerMode::UCI)
                }
                Some(GUICommand::Play) => {
                    respond(BotCommand::PlayOk);

                    controller.initialize(ControllerMode::Play);
                    controller.print()
                }
                _ => {}
            },
        }
    }
}

fn generate_magic_bitboards() {
    let mut magic_bitboards: MagicBitboards = Default::default();

    // Rook magic numbers
    for y in 0..8 {
        for x in 0..8 {
            let index = x + y * 8;
            let result = calculate_magic_bitboard(x, y, &Piece::Rook);

            log::debug!(
                "Rook ({}/{}): {:064b}, {}",
                index + 1,
                64,
                result.magic,
                64 - result.shift
            );

            magic_bitboards.push(result);
        }
    }

    // Bishop magic numbers
    for y in 0..8 {
        for x in 0..8 {
            let index = x + y * 8;
            let result = calculate_magic_bitboard(x, y, &Piece::Bishop);

            log::debug!(
                "Bishop ({}/{}): {:064b}, {}",
                index + 1,
                64,
                result.magic,
                64 - result.shift
            );

            magic_bitboards.push(result);
        }
    }

    log::debug!("Magic bitboards generated!");

    // We're bootstrapping babyyyyy
    serialize_magic_bitboards_to_file_flat(&magic_bitboards, concat!("src/utils/magic.rs"))
        .expect("TODO: panic message");
}
