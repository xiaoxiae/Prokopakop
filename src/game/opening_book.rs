use crate::game::board::{BoardMove, BoardMoveExt};
use fxhash::FxHashMap;
use rand::Rng;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct BookEntry {
    pub moves: Vec<BookMove>,
    pub total_rating: u32, // Changed from total_count to total_rating
}

#[derive(Debug, Clone)]
pub struct BookMove {
    pub board_move: BoardMove,
    pub times_played: u32, // Changed from count to be more explicit
    pub rating_sum: u32,   // Sum of all ratings for this move
}

impl PartialEq for BookMove {
    fn eq(&self, other: &Self) -> bool {
        self.times_played == other.times_played
    }
}

impl Eq for BookMove {}

impl PartialOrd for BookMove {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BookMove {
    fn cmp(&self, other: &Self) -> Ordering {
        // Sort only by times played (descending)
        other.times_played.cmp(&self.times_played)
    }
}

#[derive(Clone)]
pub struct OpeningBook {
    positions: FxHashMap<u64, BookEntry>,
}

impl OpeningBook {
    pub fn new() -> Self {
        Self {
            positions: FxHashMap::default(),
        }
    }

    pub fn add_game(
        &mut self,
        positions: Vec<(u64, BoardMove)>,
        game_result: GameResult,
        average_elo: u32,
    ) {
        for (zobrist_key, board_move) in positions {
            let entry = self
                .positions
                .entry(zobrist_key)
                .or_insert_with(|| BookEntry {
                    moves: Vec::new(),
                    total_rating: 0,
                });

            entry.total_rating += average_elo;

            // Find existing move or create new one
            if let Some(book_move) = entry.moves.iter_mut().find(|m| m.board_move == board_move) {
                book_move.times_played += 1;
                book_move.rating_sum += average_elo;
            } else {
                entry.moves.push(BookMove {
                    board_move,
                    times_played: 1,
                    rating_sum: average_elo,
                });
            }

            // Keep moves sorted by times played
            entry.moves.sort_unstable();
        }
    }

    pub fn get_moves(&self, zobrist_key: u64) -> Option<&[BookMove]> {
        self.positions
            .get(&zobrist_key)
            .map(|entry| entry.moves.as_slice())
    }

    pub fn get_best_move(&self, zobrist_key: u64) -> Option<BoardMove> {
        let moves = self.get_moves(zobrist_key)?;
        if moves.is_empty() {
            return None;
        }

        let mut weights = Vec::with_capacity(moves.len());
        let mut total_weight = 0.0;

        for i in 0..moves.len() {
            // Weight based on position in sorted list (by times played)
            let weight = 0.5 * 0.1_f64.powi(i as i32);
            weights.push(weight);
            total_weight += weight;
        }

        // Normalize weights
        for weight in &mut weights {
            *weight /= total_weight;
        }

        // Generate random number and select move based on cumulative weights
        let mut rng = rand::rng();
        let random_value = rng.random::<f64>();
        let mut cumulative_weight = 0.0;

        for (i, weight) in weights.iter().enumerate() {
            cumulative_weight += weight;
            if random_value <= cumulative_weight {
                return Some(moves[i].board_move);
            }
        }

        Some(moves[0].board_move)
    }

    pub fn prune_by_size(&mut self, max_positions: usize) {
        if self.positions.len() <= max_positions {
            return;
        }

        // Create a min-heap to keep track of positions with lowest total_rating
        let mut heap = BinaryHeap::new();

        for (zobrist_key, entry) in &self.positions {
            if heap.len() < max_positions {
                heap.push((entry.total_rating, *zobrist_key));
            } else if entry.total_rating > heap.peek().unwrap().0 {
                heap.pop();
                heap.push((entry.total_rating, *zobrist_key));
            }
        }

        // Keep only the positions in the heap
        let keys_to_keep: std::collections::HashSet<u64> =
            heap.into_iter().map(|(_, key)| key).collect();
        self.positions.retain(|key, _| keys_to_keep.contains(key));
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Write header with version and position count
        writeln!(writer, "# prokopakopening book v2.0 (with ELO weighting)")?;
        writeln!(writer, "# Positions: {}", self.positions.len())?;
        writeln!(writer)?;

        // Save in format: <hash> <move>:<times_played>:<rating_sum> ...
        for (zobrist_key, entry) in &self.positions {
            write!(writer, "{:016x}", zobrist_key)?;
            for book_move in &entry.moves {
                write!(writer, " {}", book_move.board_move.unparse())?;
            }
            writeln!(writer)?;
        }

        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut book = OpeningBook::new();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse line format: <hash> <move>:<times_played>:<rating_sum> ...
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(zobrist_key) = u64::from_str_radix(parts[0], 16) {
                    let mut entry = BookEntry {
                        moves: Vec::new(),
                        total_rating: 0,
                    };

                    // Parse all moves (starting from index 1)
                    for move_data in &parts[1..] {
                        // Try new format first (move:times_played:rating_sum)
                        let move_parts: Vec<&str> = move_data.split(':').collect();

                        if let Some(board_move) = BoardMove::parse(move_data) {
                            entry.moves.push(BookMove {
                                board_move,
                                times_played: 0,
                                rating_sum: 0,
                            });
                        }
                    }

                    // Sort moves by times_played
                    entry.moves.sort_unstable();

                    if !entry.moves.is_empty() {
                        book.positions.insert(zobrist_key, entry);
                    }
                }
            }
        }

        Ok(book)
    }

    pub fn position_count(&self) -> usize {
        self.positions.len()
    }

    pub fn total_games(&self) -> u32 {
        // Sum of all times_played across all moves in all positions
        self.positions
            .values()
            .flat_map(|entry| &entry.moves)
            .map(|m| m.times_played)
            .sum()
    }

    pub fn average_rating(&self) -> f32 {
        let total_rating_sum: u32 = self
            .positions
            .values()
            .flat_map(|entry| &entry.moves)
            .map(|m| m.rating_sum)
            .sum();

        let total_games = self.total_games();

        if total_games > 0 {
            total_rating_sum as f32 / total_games as f32
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum GameResult {
    White,
    Black,
    Draw,
}
