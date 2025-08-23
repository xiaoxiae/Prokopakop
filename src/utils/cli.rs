use std::io;

pub(crate) enum GUICommand {
    // UCI
    UCI,
    IsReady,

    // Regular chess bot
    Play,         // start play
    Status,       // status of the board
    Move(String), // perform a move
    Unmove,       // undo a move
    Fen,          // produce a fen string for the current position

    // Shared
    Position(Option<String>),  // `position` for both
    ValidMoves(String),        // `go perf <depth>` for UCI, `moves` for player,
    SetOption(String, String), // setoption name <name> value <value>
    Quit,                      // quit the program

    // Miscellaneous
    Magic, // look for magic numbers
}

pub(crate) enum BotCommand {
    Identify(String, String),
    UCIOk,
    ReadyOk,

    // Player
    PlayOk,
}

impl GUICommand {
    pub(crate) fn receive() -> Option<GUICommand> {
        let mut input = String::new();

        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        let parts = input.as_str().trim().split_whitespace().collect::<Vec<_>>();

        match parts.as_slice() {
            ["uci"] => Some(GUICommand::UCI),
            ["isready"] => Some(GUICommand::IsReady),
            ["ucinewgame"] => Some(GUICommand::Position(None)),
            ["position", "startpos"] => Some(GUICommand::Position(None)),
            ["position", "startpos", ..] => unimplemented!(),
            ["position", "fen", fen @ ..] if !fen.is_empty() => {
                Some(GUICommand::Position(Some(fen.join(" "))))
            }
            ["setoption", "name", name, "value", value] => {
                Some(GUICommand::SetOption(name.to_string(), value.to_string()))
            }
            ["moves"] => Some(GUICommand::ValidMoves("1".to_string())),
            ["go", "perft", depth] | ["moves", depth] => {
                Some(GUICommand::ValidMoves(depth.to_string()))
            }
            ["play"] => Some(GUICommand::Play),
            ["quit"] => Some(GUICommand::Quit),
            ["magic"] => Some(GUICommand::Magic),
            ["move", notation] => Some(GUICommand::Move(notation.to_string())),
            ["unmove"] => Some(GUICommand::Unmove),
            ["status"] => Some(GUICommand::Status),
            ["fen"] => Some(GUICommand::Fen),
            _ => None,
        }
    }

    pub(crate) fn respond(command: BotCommand) {
        match command {
            BotCommand::Identify(name, author) => {
                println!("identify name {}", name);
                println!("identify author {}", author);
            }
            BotCommand::ReadyOk => println!("readyok"),
            BotCommand::UCIOk => println!("uciok"),

            BotCommand::PlayOk => println!("playok"),
        }
    }
}
