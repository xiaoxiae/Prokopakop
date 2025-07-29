mod controller;
mod game;
mod test;
mod utils;

pub use crate::controller::*;
use crate::game::{BoardSquare, MoveResultType, Piece};
pub use crate::utils::*;

fn main() {
    env_logger::init();

    // TODO: to class
    let mut controller = GameController::new();

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
                    controller.print(None);
                }
            }
            (Some(GUICommand::Magic), _) => {
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
                serialize_magic_bitboards_to_file_flat(
                    &magic_bitboards,
                    concat!("src/utils/magic.rs"),
                )
                .expect("TODO: panic message");
            }
            _ => {}
        }

        match controller.mode {
            // UCI-only commands
            Some(ControllerMode::UCI) => match input {
                Some(GUICommand::IsReady) => respond(BotCommand::ReadyOk),
                Some(GUICommand::ValidMoves(depth_string)) => {
                    let moves = controller.get_valid_moves(depth_string.parse::<usize>().unwrap());

                    for (m, c) in &moves {
                        println!("{}: {}", m.unparse(), c);
                    }

                    println!("\nNodes: {}", moves.len());
                }
                _ => {}
            },
            // Player-only commands
            Some(ControllerMode::Play) => match input {
                Some(GUICommand::Move(notation)) => {
                    let result = controller.try_move_piece(notation);

                    match result {
                        MoveResultType::Success => controller.print(None),
                        _ => log::info!("{:?}", result),
                    };
                }
                Some(GUICommand::Unmove) => {
                    let result = controller.try_unmove_piece();

                    match result {
                        MoveResultType::Success => controller.print(None),
                        _ => log::info!("{:?}", result),
                    };
                }
                Some(GUICommand::ValidMoves(square_string)) => {
                    match BoardSquare::parse(square_string.as_str()) {
                        Some(square) => {
                            let moves = controller.get_valid_moves(1);

                            controller.print(Some(
                                moves
                                    .iter()
                                    .map(|(m, _)| m)
                                    .filter(|m| m.from == square)
                                    .map(|m| &m.to)
                                    .collect::<Vec<_>>(),
                            ))
                        }
                        None => {}
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
                    controller.print(None)
                }
                _ => {}
            },
        }
    }
}
