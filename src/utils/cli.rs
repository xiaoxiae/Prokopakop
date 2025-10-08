use std::io;

pub(crate) enum GUICommand {
    UCI,
    IsReady,
    FenPosition(String),               // position fen <fen>
    MovePosition(Option<Vec<String>>), // position startpos <maybe some moves>
    SetOption(String, String),         // setoption name <name> value <value>
    Perft(String),                     // go perft <depth>
    Search(Vec<String>),               // go (with params)
    Stop,                              // stop
    Quit,                              // quit the program
    Eval,                              // eval - print detailed evaluation
    Joke,                              // joke - tell a random joke

    Invalid(String), // placeholder for invalid commands so we can pattern match
}

impl GUICommand {
    pub fn receive() -> GUICommand {
        let mut input = String::new();

        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        let parts = input.as_str().trim().split_whitespace().collect::<Vec<_>>();

        match parts.as_slice() {
            ["uci"] => GUICommand::UCI,
            ["isready"] => GUICommand::IsReady,
            ["ucinewgame"] => GUICommand::MovePosition(None),
            ["position", "startpos"] => GUICommand::MovePosition(None),
            ["position", "startpos", "moves", moves @ ..] => {
                GUICommand::MovePosition(Some(moves.iter().map(|m| m.to_string()).collect()))
            }
            ["position", "fen", fen @ ..] if !fen.is_empty() => {
                GUICommand::FenPosition(fen.join(" "))
            }
            ["setoption", "name", name_and_rest @ ..] if !name_and_rest.is_empty() => {
                Self::parse_setoption(name_and_rest)
            }
            ["go", "perft", depth] => GUICommand::Perft(depth.to_string()),
            ["go", params @ ..] => {
                GUICommand::Search(params.iter().map(|p| p.to_string()).collect())
            }
            ["stop"] => GUICommand::Stop,
            ["quit"] => GUICommand::Quit,
            ["eval"] => GUICommand::Eval,
            ["joke"] => GUICommand::Joke,
            _ => GUICommand::Invalid(input),
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
