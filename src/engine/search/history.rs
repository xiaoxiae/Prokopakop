use fxhash::FxHashMap;

use crate::game::board::{BoardMove, BoardMoveExt};

/// Combined history tracking for move metrics and position repetitions
#[derive(Debug, Clone)]
pub struct History {
    // Move history scores indexed by [from_square][to_square]
    move_scores: [[i32; 64]; 64],
    max_score: i32,

    // Position repetition tracking
    positions: FxHashMap<u64, u32>,
    position_history: Vec<u64>, // Keep track of order for undo
}

impl History {
    pub fn new() -> Self {
        Self {
            move_scores: [[0; 64]; 64],
            max_score: 8192, // Threshold for scaling
            positions: FxHashMap::default(),
            position_history: Vec::with_capacity(256),
        }
    }

    // Move history methods
    pub fn add_history(&mut self, board_move: BoardMove, depth: usize) {
        let from = board_move.get_from() as usize;
        let to = board_move.get_to() as usize;

        // Bonus is proportional to depth squared (more weight for deeper cutoffs)
        let bonus = (depth * depth) as i32;

        self.move_scores[from][to] += bonus;

        // Check if we need to scale down all scores to prevent overflow
        if self.move_scores[from][to] > self.max_score {
            self.age_history();
        }
    }

    pub fn add_history_penalty(&mut self, board_move: BoardMove, depth: usize) {
        let from = board_move.get_from() as usize;
        let to = board_move.get_to() as usize;

        // Smaller penalty to not over-penalize moves
        let penalty = ((depth * depth) / 2) as i32;

        self.move_scores[from][to] = (self.move_scores[from][to] - penalty).max(-self.max_score);
    }

    pub fn get_history_score(&self, board_move: &BoardMove) -> i32 {
        let from = board_move.get_from() as usize;
        let to = board_move.get_to() as usize;
        self.move_scores[from][to]
    }

    fn age_history(&mut self) {
        for from in 0..64 {
            for to in 0..64 {
                self.move_scores[from][to] /= 2;
            }
        }
    }

    pub fn push_position(&mut self, zobrist_key: u64) {
        self.position_history.push(zobrist_key);
        *self.positions.entry(zobrist_key).or_insert(0) += 1;
    }

    pub fn pop_position(&mut self) {
        if let Some(zobrist_key) = self.position_history.pop() {
            if let Some(count) = self.positions.get_mut(&zobrist_key) {
                if *count > 1 {
                    *count -= 1;
                } else {
                    self.positions.remove(&zobrist_key);
                }
            }
        }
    }

    pub fn is_threefold_repetition(&self, zobrist_key: u64) -> bool {
        // Check if this position (including current) appears 3 or more times
        self.positions.get(&zobrist_key).copied().unwrap_or(0) >= 2
    }
}
