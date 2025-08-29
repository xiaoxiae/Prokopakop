use crate::game::board::{BoardMove, BoardMoveExt, Game, MoveResultType};
use crate::game::pieces::Color;
use crate::utils::square::{BoardSquare, BoardSquareExt};
use fxhash::FxHashMap;

pub enum ControllerMode {
    UCI,
    Play,
}

pub struct GameController {
    pub game: Game,
    pub use_hash: bool,
    pub mode: Option<ControllerMode>,
}

type PerftTable = FxHashMap<u64, usize>;

impl GameController {
    pub fn new() -> Self {
        Self {
            game: Game::new(None),
            mode: None,
            use_hash: true, // Default to true
        }
    }

    pub fn new_game(&mut self) {
        self.game = Game::new(None);
    }

    pub fn new_game_from_fen(&mut self, fen: &str) {
        self.game = Game::new(Some(fen));
    }

    pub fn initialize(&mut self, mode: ControllerMode) {
        self.mode = Some(mode);
        self.new_game();
    }

    pub fn set_option(&mut self, name: &str, value: &str) {
        match name.to_lowercase().as_str() {
            "hash" => match value.to_lowercase().as_str() {
                "true" => self.use_hash = true,
                "false" => self.use_hash = false,
                _ => eprintln!(
                    "Invalid value for Hash option: {}. Expected 'true' or 'false'",
                    value
                ),
            },
            _ => {
                eprintln!("Unknown option: {}", name);
            }
        }
    }

    pub fn print_with_moves(&self, possible_moves: Vec<BoardSquare>) {
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
        match BoardMove::parse(long_algebraic_notation.as_str())
            .or(long_algebraic_notation.parse::<BoardMove>().ok())
        {
            Some(board_move) => {
                let (move_count, valid_moves) = self.game.get_moves();

                // Check if the move is in the valid moves array
                if valid_moves[0..move_count].contains(&board_move) {
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

    pub fn perft(&mut self, depth: usize) -> Vec<(BoardMove, usize)> {
        self.perft_with_hashing(depth, self.use_hash)
    }

    fn perft_with_hashing(&mut self, depth: usize, hashing: bool) -> Vec<(BoardMove, usize)> {
        let mut table: PerftTable = FxHashMap::default();
        let mut move_breakdown = vec![];

        // Get all valid moves for the current position
        let (move_count, valid_moves) = self.game.get_moves();

        for i in 0..move_count {
            let board_move = valid_moves[i];
            let move_count = if hashing {
                self.dfs_count_moves_with_hashing(board_move, depth, &mut table)
            } else {
                self.dfs_count_moves_no_hashing(board_move, depth)
            };
            move_breakdown.push((board_move, move_count));
        }

        move_breakdown
    }

    fn dfs_count_moves_with_hashing(
        &mut self,
        initial_move: BoardMove,
        depth: usize,
        table: &mut PerftTable,
    ) -> usize {
        if depth <= 1 {
            return 1;
        }

        self.game.make_move(initial_move);

        if let Some(count) = table.get(&(self.game.zobrist_key ^ depth as u64)) {
            self.game.unmake_move();
            return *count;
        }

        let mut total_count = 0;

        let (current_move_count, current_moves) = self.game.get_moves();

        // Bulk counting
        if depth == 2 {
            total_count = current_move_count;
        } else {
            for i in 0..current_move_count {
                let board_move = current_moves[i];
                total_count += self.dfs_count_moves_with_hashing(board_move, depth - 1, table);
            }
        }

        table.insert(self.game.zobrist_key ^ depth as u64, total_count);

        self.game.unmake_move();

        total_count
    }

    fn dfs_count_moves_no_hashing(&mut self, initial_move: BoardMove, depth: usize) -> usize {
        if depth <= 1 {
            return 1;
        }

        self.game.make_move(initial_move);

        let mut total_count = 0;

        let (current_move_count, current_moves) = self.game.get_moves();

        // Bulk counting
        if depth == 2 {
            total_count = current_move_count;
        } else {
            for i in 0..current_move_count {
                let board_move = current_moves[i];
                total_count += self.dfs_count_moves_no_hashing(board_move, depth - 1);
            }
        }

        self.game.unmake_move();

        total_count
    }
}
