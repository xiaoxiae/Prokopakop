mod controller;
mod game;
mod test;
mod utils;

pub use crate::controller::*;
use crate::game::MoveResultType;
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
            (Some(GUICommand::Position(fen)), true) => match fen {
                None => controller.new_game(None),
                Some(fen) => controller.new_game(Some(fen.as_str())),
            },
            (Some(GUICommand::ValidMoves(depth)), true) => {
                let moves = controller.get_valid_moves(*depth);

                for (m, c) in &moves {
                    println!("{}: {}", m.unparse(), c);
                }

                println!("\nNodes: {}", moves.len());
            },
            _ => {}
        }

        match controller.mode {
            // UCI-only commands
            Some(Mode::UCI) => match input {
                Some(GUICommand::IsReady) => respond(BotCommand::ReadyOk),

                _ => {}
            },
            // Player-only commands
            Some(Mode::Player) => match input {
                Some(GUICommand::Move(notation)) => {
                    let result = controller.try_move_piece(notation);

                    match result {
                        MoveResultType::Success => controller.print(),
                        _ => log::info!("{:?}", result),
                    };
                }

                _ => {}
            },
            None => match input {
                Some(GUICommand::UCI) => {
                    let name = "Prokopakop";
                    let author = "Tomíno Komíno";

                    respond(BotCommand::Identify(name.to_string(), author.to_string()));
                    respond(BotCommand::UCIOk);

                    controller.initialize(Mode::UCI)
                }
                Some(GUICommand::Play) => {
                    respond(BotCommand::PlayOk);

                    controller.initialize(Mode::Player);
                }
                _ => {}
            },
        }
    }
}
