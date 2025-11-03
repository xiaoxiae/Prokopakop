use fxhash::FxHashMap;

#[derive(Clone, Debug)]
pub struct PositionHistory {
    positions: FxHashMap<u64, u32>,
    history: Vec<u64>, // Keep track of order for undo
}

impl PositionHistory {
    pub fn new() -> Self {
        Self {
            positions: FxHashMap::default(),
            history: Vec::with_capacity(256),
        }
    }

    pub fn push(&mut self, zobrist_key: u64) {
        self.history.push(zobrist_key);
        *self.positions.entry(zobrist_key).or_insert(0) += 1;
    }

    pub fn pop(&mut self) {
        if let Some(zobrist_key) = self.history.pop() {
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
