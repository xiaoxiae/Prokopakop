use crate::game::{BoardMove, Color, Game, MoveResultType};
use crate::{BoardSquare, BoardSquareExt};

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

    pub fn print_with_moves(&self, possible_moves: Vec<&BoardSquare>) {
        const RESET: &str = "\x1b[0m";
        const LIGHT_SQUARE_BG: &str = "\x1b[48;5;172m";
        const DARK_SQUARE_BG: &str = "\x1b[48;5;130m";
        const WHITE_PIECE: &str = "\x1b[1;97m";
        const BLACK_PIECE: &str = "\x1b[1;30m";
        const MOVE_HIGHLIGHT: &str = "\x1b[1;34m";
        const HEADING_BG: &str = "\x1b[48;5;240m"; // Neutral gray background

        // Print centered heading with background
        let heading_text = match self.game.side {
            Color::White => "White to move",
            Color::Black => "Black to move",
        };
        let heading_color = match self.game.side {
            Color::White => WHITE_PIECE,
            Color::Black => BLACK_PIECE,
        };

        // Board width is 8 squares * 3 chars each = 24 chars
        let board_width = 24;
        let padding = (board_width - heading_text.len()) / 2;
        let total_padding = board_width - heading_text.len();
        let right_padding = total_padding - padding;

        println!(
            "{}{}{}{}{}{}",
            HEADING_BG,
            " ".repeat(padding),
            heading_color,
            heading_text,
            " ".repeat(right_padding),
            RESET
        );

        // Convert possible moves to a HashSet for O(1) lookup
        let move_squares: std::collections::HashSet<(usize, usize)> = possible_moves
            .iter()
            .map(|square| (square.get_x() as usize, square.get_y() as usize))
            .collect();

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

    pub fn print(&self) {
        self.print_with_moves(vec![]);
    }

    pub fn print_fen(&self) {
        println!("{}", self.game.get_fen());
    }

    pub fn try_move_piece(&mut self, long_algebraic_notation: String) -> MoveResultType {
        match BoardMove::parse(long_algebraic_notation.as_str()) {
            Some(board_move) => {
                let (valid_moves, _) = self.game.get_current_position_moves();

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
        let (current_moves, count) = self.game.get_current_position_moves();

        for board_move in current_moves.into_iter().take(count) {
            let move_count = self.dfs_count_moves(board_move.clone(), depth);
            all_moves.push((board_move, move_count));
        }

        all_moves
    }

    fn dfs_count_moves(&mut self, initial_move: BoardMove, depth: usize) -> usize {
        if depth <= 1 {
            return 1;
        }

        self.game.make_move(initial_move);

        let mut total_count = 0;

        let (current_moves, count) = self.game.get_current_position_moves();

        // Bulk counting
        if depth == 1 {
            total_count = count;
        } else {
            for board_move in current_moves.into_iter().take(count) {
                total_count += self.dfs_count_moves(board_move, depth - 1);
            }
        }

        self.game.unmake_move();

        total_count
    }
}
