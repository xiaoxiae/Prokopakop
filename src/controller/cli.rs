use std::io;

pub enum PositionStart {
    StartPos,
    Fen(String),
}

pub enum GUICommand {
    UCI,
    IsReady,
    UciNewGame, // ucinewgame
    Position {
        // position (startpos | fen <fen/epd>) [moves <move> ...]
        start: PositionStart,
        moves: Vec<String>,
    },
    SetOption(String, String), // setoption name <name> value <value>
    Perft(String),             // go perft <depth>
    Search(Vec<String>),       // go (with params)
    PonderHit,                 // ponderhit
    Stop,                      // stop
    Quit,                      // quit the program
    Eval,                      // eval - print evaluation
    Joke,                      // joke - tell a random joke

    Invalid(String), // placeholder for invalid commands so we can pattern match
}

impl GUICommand {
    pub fn receive() -> GUICommand {
        let mut input = String::new();

        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                // EOF reached (Ctrl+D on Unix)
                return GUICommand::Quit;
            }
            Ok(_) => {
                // Input received successfully
            }
            Err(_) => {
                panic!("Failed to read line");
            }
        }

        let parts = input.as_str().trim().split_whitespace().collect::<Vec<_>>();

        match parts.as_slice() {
            ["uci"] => GUICommand::UCI,
            ["isready"] => GUICommand::IsReady,
            ["ucinewgame"] => GUICommand::UciNewGame,
            ["position", rest @ ..] if !rest.is_empty() => Self::parse_position(rest),
            ["setoption", "name", name_and_rest @ ..] if !name_and_rest.is_empty() => {
                Self::parse_setoption(name_and_rest)
            }
            ["go", "perft", depth] => GUICommand::Perft(depth.to_string()),
            ["go", params @ ..] => {
                GUICommand::Search(params.iter().map(|p| p.to_string()).collect())
            }
            ["ponderhit"] => GUICommand::PonderHit,
            ["stop"] => GUICommand::Stop,
            ["quit"] => GUICommand::Quit,
            ["eval"] => GUICommand::Eval,
            ["joke"] => GUICommand::Joke,
            _ => GUICommand::Invalid(input),
        }
    }

    fn parse_position(parts: &[&str]) -> GUICommand {
        let (head, moves) = match parts.iter().position(|&part| part == "moves") {
            Some(i) => (&parts[..i], &parts[i + 1..]),
            None => (parts, &[] as &[&str]),
        };

        let start = match head {
            ["startpos", ..] => PositionStart::StartPos,
            ["fen", fen @ ..] if !fen.is_empty() => PositionStart::Fen(fen.join(" ")),
            [] => return GUICommand::Invalid("position with no starting position".to_string()),
            fen => PositionStart::Fen(fen.join(" ")),
        };

        GUICommand::Position {
            start,
            moves: moves.iter().map(|m| m.to_string()).collect(),
        }
    }

    fn parse_setoption(parts: &[&str]) -> GUICommand {
        // Find the "value" keyword to split name and value
        if let Some(value_pos) = parts.iter().position(|&part| part == "value") {
            // Everything before "value" is the option name
            let name = parts[..value_pos].join(" ");
            // Everything after "value" is the option value
            let value = parts[value_pos + 1..].join(" ");

            if !name.is_empty() && !value.is_empty() {
                GUICommand::SetOption(name, value)
            } else {
                GUICommand::Invalid(format!("setoption name {} value {}", name, value))
            }
        } else {
            // No "value" keyword found - this could be a button-type option
            let name = parts.join(" ");
            if !name.is_empty() {
                GUICommand::SetOption(name, String::new()) // Empty value for button options
            } else {
                GUICommand::Invalid("setoption with empty name".to_string())
            }
        }
    }
}
