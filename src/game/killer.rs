use crate::game::board::{BoardMove, BoardMoveExt};

#[derive(Debug, Clone)]
pub struct KillerMoves {
    // 2 killer moves per ply
    killers: Vec<[BoardMove; 2]>,
}

impl KillerMoves {
    pub fn new(max_ply: usize) -> Self {
        Self {
            killers: vec![[BoardMove::empty(); 2]; max_ply],
        }
    }

    pub fn add_killer(&mut self, ply: usize, board_move: BoardMove) {
        if ply >= self.killers.len() {
            return;
        }

        if self.killers[ply][0] == board_move {
            return;
        }

        self.killers[ply][1] = self.killers[ply][0];
        self.killers[ply][0] = board_move;
    }

    pub fn get_killers(&self, ply: usize) -> [BoardMove; 2] {
        if ply < self.killers.len() {
            self.killers[ply].clone()
        } else {
            [BoardMove::default(); 2]
        }
    }
}
