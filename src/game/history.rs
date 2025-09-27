use crate::game::board::{BoardMove, BoardMoveExt};

#[derive(Debug, Clone)]
pub struct HistoryTable {
    // History scores indexed by [from_square][to_square]
    scores: [[i32; 64]; 64],
    max_score: i32,
}

impl HistoryTable {
    pub fn new() -> Self {
        Self {
            scores: [[0; 64]; 64],
            max_score: 8192, // Threshold for scaling
        }
    }

    pub fn add_history(&mut self, board_move: BoardMove, depth: usize) {
        let from = board_move.get_from() as usize;
        let to = board_move.get_to() as usize;

        // Bonus is proportional to depth squared (more weight for deeper cutoffs)
        let bonus = (depth * depth) as i32;

        self.scores[from][to] += bonus;

        // Check if we need to scale down all scores to prevent overflow
        if self.scores[from][to] > self.max_score {
            self.age_history();
        }
    }

    pub fn add_history_penalty(&mut self, board_move: BoardMove, depth: usize) {
        let from = board_move.get_from() as usize;
        let to = board_move.get_to() as usize;

        // Smaller penalty to not over-penalize moves
        let penalty = ((depth * depth) / 2) as i32;

        self.scores[from][to] = (self.scores[from][to] - penalty).max(-self.max_score);
    }

    pub fn get_history_score(&self, board_move: &BoardMove) -> i32 {
        let from = board_move.get_from() as usize;
        let to = board_move.get_to() as usize;
        self.scores[from][to]
    }

    fn age_history(&mut self) {
        for from in 0..64 {
            for to in 0..64 {
                self.scores[from][to] /= 2;
            }
        }
    }
}
