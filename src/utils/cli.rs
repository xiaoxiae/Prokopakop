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
            ["setoption", "name", name, "value", value] => {
                GUICommand::SetOption(name.to_string(), value.to_string())
            }
            ["go", "perft", depth] => GUICommand::Perft(depth.to_string()),
            ["go", params @ ..] => {
                GUICommand::Search(params.iter().map(|p| p.to_string()).collect())
            }
            ["stop"] => GUICommand::Stop,
            ["quit"] => GUICommand::Quit,
            _ => GUICommand::Invalid(input),
        }
    }
}
