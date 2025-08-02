use crate::game::{BoardMove, BoardSquare, Color, Game, MoveResultType};

pub enum ControllerMode {
    UCI,
    Play,
}

pub struct GameController {
    pub game: Game,
    pub mode: Option<ControllerMode>,
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

    pub fn initialize(&mut self, mode: ControllerMode) {
        self.mode = Some(mode);
        self.new_game(None);
    }

    pub fn print(&self, possible_moves: Option<Vec<&BoardSquare>>) {
        const RESET: &str = "\x1b[0m";
        const LIGHT_SQUARE_BG: &str = "\x1b[48;5;172m";
        const DARK_SQUARE_BG: &str = "\x1b[48;5;130m";
        const WHITE_PIECE: &str = "\x1b[1;97m";
        const BLACK_PIECE: &str = "\x1b[1;30m";
        const MOVE_HIGHLIGHT: &str = "\x1b[1;34m";

        // Convert possible moves to a HashSet for O(1) lookup
        let move_squares: std::collections::HashSet<(usize, usize)> = possible_moves
            .map(|moves| {
                moves
                    .iter()
                    .map(|square| (square.x as usize, square.y as usize))
                    .collect()
            })
            .unwrap_or_default();

        for y in (0..8).rev() {
            let mut line = String::new();
            for x in 0..8 {
                let is_light_square = (x + y) % 2 == 1;
                let bg_color = if is_light_square {
                    LIGHT_SQUARE_BG
                } else {
                    DARK_SQUARE_BG
                };
                line.push_str(bg_color);

                match self.game.pieces[x + y * 8] {
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
            println!("{}", line);
        }
    }

    pub fn try_move_piece(&mut self, long_algebraic_notation: String) -> MoveResultType {
        match BoardMove::parse(long_algebraic_notation.as_str()) {
            Some(board_move) => {
                let valid_moves = self.game.get_current_position_moves();

                if valid_moves.contains(&board_move) {
                    self.game.make_move(board_move);
                    MoveResultType::Success
                } else {
                    MoveResultType::InvalidMove
                }
            }
            None => MoveResultType::InvalidNotation,
        }
    }

    pub fn try_unmove_piece(&mut self) -> MoveResultType {
        match self.game.history.len() {
            0 => MoveResultType::NoHistory,
            _ => {
                self.game.unmake_move();
                MoveResultType::Success
            }
        }
    }

    pub fn get_valid_moves(&mut self, depth: usize) -> Vec<(BoardMove, usize)> {
        let mut all_moves = vec![];

        // Get all valid moves for the current position
        let current_moves = self.game.get_current_position_moves();

        for board_move in current_moves {
            let move_count = self.dfs_count_moves(board_move.clone(), depth);
            all_moves.push((board_move, move_count));
        }

        all_moves
    }

    fn dfs_count_moves(&mut self, initial_move: BoardMove, depth: usize) -> usize {
        if depth == 0 {
            return 1;
        }

        self.game.make_move(initial_move);

        let mut total_count = 0;

        let current_moves = self.game.get_current_position_moves();

        if depth == 1 {
            total_count = current_moves.len();
        } else {
            // Recursive case: explore each move further
            for board_move in current_moves {
                total_count += self.dfs_count_moves(board_move, depth - 1);
            }
        }

        self.game.unmake_move();

        total_count
    }
}
