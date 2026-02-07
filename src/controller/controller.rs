use crate::engine::nnue::load_nnue_from_file;
use crate::engine::search::history::History;
use crate::engine::search::limits::{SearchLimits, SearchParams};
use crate::engine::search::results::SearchResult;
use crate::engine::search::searcher::Search;
use crate::engine::table::TranspositionTable;
use crate::game::board::{BoardMove, BoardMoveExt, Game};
use std::path::Path;

use fxhash::FxHashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Instant;

// I literally have a text file of jokes that I gathered over the years
// Now there is a chance that somebody actually reads some of them
//
// Pull requests are very welcome :)
const JOKES: &[&str] = &[
    "How do you think the unthinkable? With an itheberg.",
    "Say what you want about deaf people.",
    "A man is washing the car with his son. The son asks, 'Dad, can't you just use a sponge?'",
    "Two goldfish are in a tank. One looks to the other and says, 'You man the guns while I drive.'",
    "Apparently, someone in London gets stabbed every 52 seconds. Poor bastard.",
    "Want to hear a word I just made up? Plagiarism.",
    "There is no 'i' in denial.",
    "What's the difference between a bad golfer and a bad skydiver? One goes WHACK 'Damn.' and the other goes 'Damn.' WHACK",
    "My grandfather has the heart of a lion and a lifetime ban at the zoo.",
    "As I handed my dad his 50th birthday card, he looked at me and said: one would have been enough.",
];

pub struct GameController {
    pub game: Game,
    pub perft_hash: bool,
    pub hash_table_size: usize,
    pub move_overhead: u64,
    pub threads: u64,
    pub history: History,
    initialized: bool,
    search_thread: Option<JoinHandle<SearchResult>>,
    stop_flag: Arc<AtomicBool>,
    ponder_flag: Arc<AtomicBool>,
    search_start: Arc<Mutex<Instant>>,
    tt: Arc<Mutex<TranspositionTable>>,
    used_jokes: Vec<bool>,
    last_search_result: Option<SearchResult>,
}

#[derive(Debug)]
pub enum MoveResultType {
    Success,         // successful move
    InvalidNotation, // wrong algebraic notation
    InvalidMove,     // invalid move
}

type PerftTable = FxHashMap<u64, usize>;

impl GameController {
    pub fn new() -> Self {
        Self {
            game: Game::new(None),
            perft_hash: true,
            hash_table_size: 128,
            move_overhead: 10,
            threads: 1,
            history: History::new(),
            initialized: false,
            search_thread: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
            ponder_flag: Arc::new(AtomicBool::new(false)),
            search_start: Arc::new(Mutex::new(Instant::now())),
            tt: Arc::new(Mutex::new(TranspositionTable::new(128))),
            used_jokes: vec![false; JOKES.len()],
            last_search_result: None,
        }
    }

    pub fn reset_board(&mut self) {
        self.game = Game::new(None);
        self.history = History::new();
        self.history.push_position(self.game.zobrist_key);
    }

    pub fn set_board_from_fen(&mut self, fen: &str) {
        self.game = Game::new(Some(fen));
        self.history = History::new();
        self.history.push_position(self.game.zobrist_key);
    }

    pub fn reset_transposition_table(&mut self) {
        if let Ok(mut tt) = self.tt.lock() {
            tt.clear();
        }
    }

    pub fn initialize(&mut self) {
        self.initialized = true;

        self.reset_board();
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn set_option(&mut self, name: &str, value: &str) {
        match name.to_lowercase().as_str() {
            "perfthash" => match value.to_lowercase().as_str() {
                "true" => self.perft_hash = true,
                "false" => self.perft_hash = false,
                _ => eprintln!(
                    "Invalid value for PerftHash option: {}. Expected 'true' or 'false'",
                    value
                ),
            },
            "move overhead" => match value.parse::<u64>() {
                Ok(overhead) => {
                    if overhead <= 5000 {
                        self.move_overhead = overhead;
                    } else {
                        eprintln!(
                            "Invalid value for Move Overhead option: {}. Expected value between 0 and 5000",
                            value
                        );
                    }
                }
                Err(_) => {
                    eprintln!(
                        "Invalid value for Move Overhead option: {}. Expected numeric value",
                        value
                    );
                }
            },
            "hash" => match value.parse::<usize>() {
                Ok(val) => {
                    if val <= 33554432 {
                        self.hash_table_size = val;
                        self.tt = Arc::new(Mutex::new(TranspositionTable::new(val)));
                    } else {
                        eprintln!(
                            "Invalid value for Hash option: {}. Expected value between 1 and 33554432",
                            value
                        );
                    }
                }
                Err(_) => {
                    eprintln!(
                        "Invalid value for Hash option: {}. Expected numeric value",
                        value
                    );
                }
            },
            "threads" => match value.parse::<u64>() {
                Ok(threads) => {
                    if threads <= 1024 {
                        self.threads = threads;
                    } else {
                        eprintln!(
                            "Invalid value for Threads option: {}. Expected value between 1 and 1024",
                            value
                        );
                    }
                }
                Err(_) => {
                    eprintln!(
                        "Invalid value for Threads option: {}. Expected numeric value",
                        value
                    );
                }
            },
            "nnue" => load_nnue_from_file(Path::new(value)),
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
                    self.history.push_position(self.game.zobrist_key);

                    MoveResultType::Success
                } else {
                    MoveResultType::InvalidMove
                }
            }
            _ => MoveResultType::InvalidNotation,
        }
    }

    pub fn perft(&mut self, depth: usize) -> Vec<(BoardMove, usize)> {
        self.perft_with_hashing(depth, self.perft_hash)
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

    pub fn search(&mut self, params: Vec<String>, uci_info: bool) {
        // Stop + reset any existing search
        self.stop_search();
        self.stop_flag.store(false, Ordering::Relaxed);

        let search_params = SearchParams::parse(params);

        // Set ponder flag if this is a ponder search
        let is_ponder = search_params.ponder;
        self.ponder_flag.store(is_ponder, Ordering::Relaxed);

        // Reset search start time
        if let Ok(mut start) = self.search_start.lock() {
            *start = Instant::now();
        }

        let mut game_clone = self.game.clone();
        let mut history_clone = self.history.clone();
        let stop_flag = Arc::clone(&self.stop_flag);
        let ponder_flag = Arc::clone(&self.ponder_flag);
        let search_start = Arc::clone(&self.search_start);
        let move_overhead = self.move_overhead;
        // Clone the shared transposition table reference
        let tt = Arc::clone(&self.tt);

        let handle = thread::spawn(move || {
            let limits = SearchLimits {
                max_depth: search_params.depth,
                max_nodes: search_params.nodes,
                max_time_ms: search_params.calculate_move_time(game_clone.side, move_overhead),
                exact: search_params.movetime.is_some(),
                moves: search_params.searchmoves,
                infinite: search_params.infinite,
            };

            let result = {
                if let Ok(mut tt_guard) = tt.lock() {
                    let mut search = Search::new(
                        &mut game_clone,
                        limits,
                        stop_flag,
                        &mut *tt_guard,
                        &mut history_clone,
                        uci_info,
                        search_start,
                        ponder_flag,
                    );
                    search.run()
                } else {
                    unreachable!();
                }
            };

            // Output the best move in UCI format
            if uci_info {
                if result.pv.len() >= 2 {
                    println!(
                        "bestmove {} ponder {}",
                        result.best_move.unparse(),
                        result.pv[1].unparse()
                    );
                } else {
                    println!("bestmove {}", result.best_move.unparse());
                }

                if let Ok(mut tt_guard) = tt.lock() {
                    let pruned = tt_guard.prune_old_entries();
                    println!("info string Pruned {} old TT entries", pruned);
                }
            }

            result
        });

        self.search_thread = Some(handle);
    }

    pub fn stop_search(&mut self) -> Option<SearchResult> {
        // Signal the search to stop (used for UCI "stop" command)
        self.stop_flag.store(true, Ordering::Relaxed);

        if let Some(handle) = self.search_thread.take() {
            if let Ok(result) = handle.join() {
                self.last_search_result = Some(result.clone());
                return Some(result);
            }
        }
        None
    }

    pub fn ponderhit(&mut self) {
        // Reset the search timer so time management starts fresh from now
        if let Ok(mut start) = self.search_start.lock() {
            *start = Instant::now();
        }
        // Clear ponder flag so the search thread starts respecting time limits
        self.ponder_flag.store(false, Ordering::Relaxed);
    }

    pub fn wait_for_search(&mut self) -> Option<SearchResult> {
        // Wait for search to complete naturally (don't interrupt)
        // Used for training data generation where we want full evaluations
        if let Some(handle) = self.search_thread.take() {
            if let Ok(result) = handle.join() {
                self.last_search_result = Some(result.clone());
                return Some(result);
            }
        }
        None
    }

    pub fn print_uci_options(&self) {
        println!("option name Hash type spin default 128 min 1 max 33554432");
        println!("option name Move Overhead type spin default 10 min 0 max 5000");
        println!("option name Threads type spin default 1 min 1 max 1024");
        println!("option name PerftHash type check default true");
        println!("option name NNUE type string default <none>");
    }

    pub fn print_evaluation(&self) {
        let nnue_score = self.game.evaluate();
        println!("{:.2}", nnue_score);
    }

    pub fn tell_joke(&mut self) {
        let available_indices: Vec<usize> = self
            .used_jokes
            .iter()
            .enumerate()
            .filter(|(_, used)| !*used)
            .map(|(i, _)| i)
            .collect();

        // If no jokes are available, segfault C:
        if available_indices.is_empty() {
            unsafe {
                let null_ptr: *mut u8 = std::ptr::null_mut();
                *null_ptr = 42;
            }
        }

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let random_seed = hasher.finish();

        let selected_index = available_indices[random_seed as usize % available_indices.len()];

        self.used_jokes[selected_index] = true;

        println!("{}", JOKES[selected_index]);
    }
}
