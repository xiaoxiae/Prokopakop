use crate::BitboardExt;
use crate::game::{BoardMove, BoardSquare, Color, Game, MoveResultType, Piece};

pub enum Mode {
    UCI,
    Player,
}

pub struct GameController {
    pub game: Game,
    pub mode: Option<Mode>,
}

impl GameController {
    pub fn new() -> Self {
        Self {
            // TODO: a bit wasteful
            game: Game::new(None),
            mode: None,
        }
    }

    pub fn new_game(&mut self, fen: Option<&str>) {
        self.game = Game::new(fen);
    }

    pub fn initialize(&mut self, mode: Mode) {
        self.mode = Some(mode);
        self.new_game(None);
    }

    pub fn print(&self, possible_moves: Option<Vec<&BoardSquare>>) {
        const RESET: &str = "\x1b[0m";
        const LIGHT_SQUARE_BG: &str = "\x1b[48;5;214m"; // Orange background
        const DARK_SQUARE_BG: &str = "\x1b[48;5;130m"; // Brown background
        const WHITE_PIECE: &str = "\x1b[1;97m"; // Bright white for white pieces
        const BLACK_PIECE: &str = "\x1b[1;30m"; // Black for black pieces
        const MOVE_HIGHLIGHT: &str = "\x1b[1;34m"; // Blue color for move highlights

        // Convert possible moves to a HashSet for O(1) lookup
        let move_squares: std::collections::HashSet<(usize, usize)> = possible_moves
            .map(|moves| {
                moves.iter().map(|square| (square.x as usize, square.y as usize)).collect()
            })
            .unwrap_or_default();

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

                match self.game.pieces[y][x] {
                    Some((piece, color)) => {
                        let piece_color = match color {
                            Color::White => WHITE_PIECE,
                            Color::Black => BLACK_PIECE,
                        };
                        line.push_str(&format!("{} {} {}", piece_color, piece.to_emoji(), RESET));
                    }
                    None => {
                        // Check if this square is a possible move
                        if move_squares.contains(&(x, y)) {
                            line.push_str(&format!("{} ● {}", MOVE_HIGHLIGHT, RESET));
                        } else {
                            line.push_str("   ");
                        }
                    }
                }

                line.push_str(RESET);
            }
            log::info!("{}", line);
        }
    }

    pub fn try_move_piece(&mut self, long_algebraic_notation: String) -> MoveResultType {
        match BoardMove::parse(long_algebraic_notation.as_str()) {
            Some(board_move) => self.game.try_make_move(board_move),
            None => MoveResultType::InvalidNotation,
        }
    }

    pub fn get_valid_moves(&self, depth: usize) -> Vec<(BoardMove, usize)> {
        let mut moves = vec![];

        for x in 0..8 {
            for y in 0..8 {

                let valid_bitmap = self.game.get_valid_move_bitboard(&BoardSquare { x, y });

                for x2 in 0..8 {
                    for y2 in 0..8 {
                        let to = BoardSquare { x: x2, y: y2 };

                        // TODO: pawn promotions
                        if valid_bitmap & to.to_mask() != 0 {
                            moves.push(
                                (
                                    BoardMove {
                                        from: BoardSquare { x, y },
                                        to,
                                        promotion: None
                                    },
                                    1
                                )
                            )
                        }
                    }
                }
            }
        }

        moves
    }
}
