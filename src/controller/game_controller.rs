use crate::game::board::{BoardMove, BoardMoveExt, Game};
use crate::game::pieces::Color;
use crate::game::search::{SearchLimits, iterative_deepening};

use fxhash::FxHashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};

pub struct GameController {
    pub game: Game,
    pub use_hash: bool,
    initialized: bool,
    search_thread: Option<JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
}

#[derive(Debug)]
pub enum MoveResultType {
    Success,         // successful move
    InvalidNotation, // wrong algebraic notation
    InvalidMove,     // invalid move
}

#[derive(Debug, Clone)]
pub struct SearchParams {
    pub depth: Option<usize>,        // search to depth x
    pub movetime: Option<u64>,       // search exactly x milliseconds
    pub wtime: Option<u64>,          // white has x milliseconds left on clock
    pub btime: Option<u64>,          // black has x milliseconds left on clock
    pub winc: Option<u64>,           // white increment per move in milliseconds
    pub binc: Option<u64>,           // black increment per move in milliseconds
    pub movestogo: Option<usize>,    // there are x moves to the next time control
    pub nodes: Option<u64>,          // search x nodes only
    pub infinite: bool,              // search until "stop" command
    pub searchmoves: Vec<BoardMove>, // restrict search to these moves only
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            depth: None,
            movetime: None,
            wtime: None,
            btime: None,
            winc: None,
            binc: None,
            movestogo: None,
            nodes: None,
            infinite: false,
            searchmoves: Vec::new(),
        }
    }
}

impl SearchParams {
    pub fn parse(params: Vec<String>) -> Self {
        let mut search_params = SearchParams::default();
        let mut iter = params.iter();

        while let Some(param) = iter.next() {
            match param.as_str() {
                "depth" => {
                    if let Some(value) = iter.next() {
                        search_params.depth = value.parse().ok();
                    }
                }
                "movetime" => {
                    if let Some(value) = iter.next() {
                        search_params.movetime = value.parse().ok();
                    }
                }
                "wtime" => {
                    if let Some(value) = iter.next() {
                        search_params.wtime = value.parse().ok();
                    }
                }
                "btime" => {
                    if let Some(value) = iter.next() {
                        search_params.btime = value.parse().ok();
                    }
                }
                "winc" => {
                    if let Some(value) = iter.next() {
                        search_params.winc = value.parse().ok();
                    }
                }
                "binc" => {
                    if let Some(value) = iter.next() {
                        search_params.binc = value.parse().ok();
                    }
                }
                "movestogo" => {
                    if let Some(value) = iter.next() {
                        search_params.movestogo = value.parse().ok();
                    }
                }
                "nodes" => {
                    if let Some(value) = iter.next() {
                        search_params.nodes = value.parse().ok();
                    }
                }
                "infinite" => {
                    search_params.infinite = true;
                }
                "searchmoves" => {
                    // Collect all remaining moves
                    while let Some(move_str) = iter.next() {
                        // Check if this is another parameter (not a move)
                        if [
                            "depth",
                            "movetime",
                            "wtime",
                            "btime",
                            "winc",
                            "binc",
                            "movestogo",
                            "nodes",
                            "infinite",
                        ]
                        .contains(&move_str.as_str())
                        {
                            // Put it back by breaking and letting the outer loop handle it
                            // Note: This is a simplified approach. In production, you might want
                            // to handle this differently
                            break;
                        }

                        if let Some(board_move) = BoardMove::parse(move_str) {
                            search_params.searchmoves.push(board_move);
                        }
                    }
                }
                _ => {
                    // Unknown parameter, skip
                    eprintln!("Unknown go parameter: {}", param);
                }
            }
        }

        search_params
    }

    pub fn calculate_move_time(&self, color: Color) -> Option<u64> {
        // If movetime is specified, use that
        if let Some(movetime) = self.movetime {
            return Some(movetime);
        }

        // If infinite search, no time limit
        if self.infinite {
            return None;
        }

        // Get the time and increment for the current side
        let (time_left, increment) = match color {
            Color::White => (self.wtime, self.winc.unwrap_or(0)),
            Color::Black => (self.btime, self.binc.unwrap_or(0)),
        };

        // If we have time control information
        if let Some(time) = time_left {
            let moves_remaining = self.movestogo.unwrap_or(30) as u64; // Assume 30 moves if not specified

            // Keep 50 ms buffer
            let available_time = time.saturating_sub(50);

            // Spend most of increment
            let base_time = available_time / moves_remaining.max(1);
            let allocated_time = base_time + (increment * 8 / 10);

            // Min 10ms for move
            Some(allocated_time.max(10))
        } else {
            None
        }
    }
}

type PerftTable = FxHashMap<u64, usize>;

impl GameController {
    pub fn new() -> Self {
        Self {
            game: Game::new(None),
            use_hash: true,
            initialized: false,
            search_thread: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn new_game(&mut self) {
        self.game = Game::new(None);
    }

    pub fn new_game_from_fen(&mut self, fen: &str) {
        self.game = Game::new(Some(fen));
    }

    pub fn initialize(&mut self) {
        self.initialized = true;
        self.new_game();
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
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

    pub fn try_move_piece(&mut self, long_algebraic_notation: &str) -> MoveResultType {
        match BoardMove::parse(long_algebraic_notation) {
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

    pub fn search(&mut self, params: Vec<String>) {
        // Stop + reset any existing search
        self.stop_search();
        self.stop_flag.store(false, Ordering::Relaxed);

        let search_params = SearchParams::parse(params);

        let mut game_clone = self.game.clone();
        let stop_flag = Arc::clone(&self.stop_flag);

        let handle = thread::spawn(move || {
            let limits = SearchLimits {
                max_depth: search_params.depth,
                max_nodes: search_params.nodes,
                max_time_ms: search_params.calculate_move_time(game_clone.side),
                moves: search_params.searchmoves,
                infinite: search_params.infinite,
            };

            let result = iterative_deepening(&mut game_clone, limits, stop_flag);

            // Output the best move in UCI format
            println!("bestmove {}", result.best_move.unparse());
        });

        self.search_thread = Some(handle);
    }

    pub fn stop_search(&mut self) {
        // Signal the search to stop
        self.stop_flag.store(true, Ordering::Relaxed);

        // Wait for the thread to finish
        if let Some(handle) = self.search_thread.take() {
            // Give the thread a moment to finish gracefully
            let _ = handle.join();
        }
    }
}
