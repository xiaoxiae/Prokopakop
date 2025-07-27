use crate::game::{BoardMove, BoardSquare, Color, Game, MoveResultType, Piece};

pub enum Mode {
    UCI,
    Player,
}

pub struct GameController {
    pub game: Option<Game>,
    pub mode: Option<Mode>,
}

impl GameController {
    pub fn new() -> Self {
        Self {
            game: None,
            mode: None,
        }
    }
    
    pub fn new_game(&mut self, fen: Option<&str>) {
        self.game = Some(Game::new(fen));
    }
    
    pub fn initialize(&mut self, mode: Mode) {
        self.mode = Some(mode);
        self.new_game(None);
    }

    pub fn print(&self) {
        const RESET: &str = "\x1b[0m";
        const LIGHT_SQUARE_BG: &str = "\x1b[48;5;214m"; // Orange background
        const DARK_SQUARE_BG: &str = "\x1b[48;5;130m"; // Brown background
        const WHITE_PIECE: &str = "\x1b[1;97m"; // Bright white for white pieces
        const BLACK_PIECE: &str = "\x1b[1;30m"; // Black for black pieces
        
        let game = self.game.as_ref().unwrap();

        for y in (0..8).rev() {
            let mut line = String::new();
            for x in 0..8 {
                let is_light_square = (x + y) % 2 == 0;
                let bg_color = if is_light_square {
                    LIGHT_SQUARE_BG
                } else {
                    DARK_SQUARE_BG
                };
                line.push_str(bg_color);

                match game.pieces[y as usize][x as usize] {
                    Some((piece, color)) => {
                        let piece_color = match color {
                            Color::White => WHITE_PIECE,
                            Color::Black => BLACK_PIECE,
                        };
                        line.push_str(&format!("{} {} {}", piece_color, piece.to_emoji(), RESET));
                    }
                    None => line.push_str("   "),
                }

                line.push_str(RESET);
            }
            log::info!("{}", line);
        }
    }

    pub fn try_move_piece(&mut self, long_algebraic_notation: String) -> MoveResultType {
        let from = long_algebraic_notation.get(0..2);
        let to = long_algebraic_notation.get(2..4);

        let game = self.game.as_mut().unwrap();

        let promotion = long_algebraic_notation
            .get(4..5)
            .and_then(|promotion| promotion.chars().next())
            .and_then(|char| Piece::from_char(char));

        match (
            from.and_then(BoardSquare::parse),
            to.and_then(BoardSquare::parse),
        ) {
            (Some(from), Some(to)) => game.try_move_piece(BoardMove {
                from,
                to,
                promotion,
            }),
            (_, _) => MoveResultType::InvalidNotation,
        }
    }

    pub fn get_valid_moves(&self, depth: usize) -> Vec<(BoardMove, usize)> {
        vec![]
    }
}
