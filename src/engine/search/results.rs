use super::limits::SearchLimits;
use crate::game::board::{BoardMove, BoardMoveExt};
use std::fmt::{Display, Formatter, Result};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub best_move: BoardMove,
    pub evaluation: f32,
    pub pv: Vec<BoardMove>, // Principal variation
}

impl Display for SearchResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{} ({})", self.best_move.unparse(), self.evaluation)
    }
}

impl SearchResult {
    pub fn leaf(evaluation: f32) -> Self {
        Self {
            best_move: BoardMove::empty(),
            evaluation,
            pv: Vec::new(),
        }
    }

    pub fn with_pv(best_move: BoardMove, evaluation: f32, mut pv: Vec<BoardMove>) -> Self {
        let mut new_pv = vec![best_move];
        new_pv.append(&mut pv);
        Self {
            best_move,
            evaluation,
            pv: new_pv,
        }
    }

    pub fn interrupted() -> Self {
        Self {
            best_move: BoardMove::empty(),
            evaluation: f32::NAN,
            pv: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.evaluation.is_nan()
    }
}

pub struct SearchStats {
    pub nodes: u64,
    pub search_start: Arc<Mutex<Instant>>,
    pub ponder_flag: Arc<AtomicBool>,
    pub current_depth: u64,
}

impl SearchStats {
    pub fn new(search_start: Arc<Mutex<Instant>>, ponder_flag: Arc<AtomicBool>) -> Self {
        Self {
            nodes: 0,
            search_start,
            ponder_flag,
            current_depth: 0,
        }
    }

    pub fn increment_nodes(&mut self) {
        self.nodes += 1;
    }

    pub fn get_elapsed_ms(&self) -> u64 {
        if let Ok(start) = self.search_start.lock() {
            start.elapsed().as_millis() as u64
        } else {
            0
        }
    }

    pub fn get_nps(&self) -> u64 {
        if let Ok(start) = self.search_start.lock() {
            let elapsed_secs = start.elapsed().as_secs_f64();
            if elapsed_secs > 0.0 {
                (self.nodes as f64 / elapsed_secs) as u64
            } else {
                0
            }
        } else {
            0
        }
    }

    fn is_pondering(&self) -> bool {
        self.ponder_flag.load(Ordering::Relaxed)
    }

    pub fn should_stop(&self, limits: &SearchLimits, stop_flag: &Arc<AtomicBool>) -> bool {
        // Check external stop flag
        if stop_flag.load(Ordering::Relaxed) {
            return true;
        }

        // While pondering, don't stop due to time/node limits
        if self.is_pondering() {
            return false;
        }

        if limits.infinite {
            return false;
        }

        let node_count = self.nodes;

        // Check node limit
        if let Some(max_nodes) = limits.max_nodes {
            if node_count >= max_nodes {
                return true;
            }
        }

        // Check time limit
        if let Some(max_time_ms) = limits.max_time_ms {
            if self.get_elapsed_ms() >= max_time_ms {
                return true;
            }
        }

        false
    }

    pub fn has_time_for_iteration(&self, limits: &SearchLimits, last_iteration_ms: u64) -> bool {
        // While pondering, always continue
        if self.is_pondering() {
            return true;
        }

        if limits.infinite {
            return true;
        }

        if let Some(max_time_ms) = limits.max_time_ms {
            let elapsed = self.get_elapsed_ms();
            let remaining = max_time_ms.saturating_sub(elapsed);

            // Deeper searches take longer, we'll use 2.5 for now
            let estimated_next_iteration_ms = (last_iteration_ms as f64 * 2.5) as u64;

            return remaining >= estimated_next_iteration_ms;
        }

        true
    }
}
