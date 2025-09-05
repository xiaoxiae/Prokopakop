use std::fmt::{Display, Formatter, Result};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Instant;

use crate::game::board::{BoardMove, BoardMoveExt, Game};
use crate::game::evaluate::CHECKMATE_SCORE;

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
    fn leaf(evaluation: f32) -> Self {
        Self {
            best_move: BoardMove::empty(),
            evaluation,
            pv: Vec::new(),
        }
    }

    fn with_pv(best_move: BoardMove, evaluation: f32, mut pv: Vec<BoardMove>) -> Self {
        let mut new_pv = vec![best_move];
        new_pv.append(&mut pv);
        Self {
            best_move,
            evaluation,
            pv: new_pv,
        }
    }
}

/// Search limits and parameters
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SearchLimits {
    pub max_depth: Option<usize>,
    pub max_nodes: Option<u64>,
    pub max_time_ms: Option<u64>,
    pub moves: Vec<BoardMove>, // TODO: implement this!
    pub infinite: bool,
}

/// Statistics tracked during search
pub struct SearchStats {
    pub nodes: AtomicU64,
    pub start_time: Instant,
    pub current_depth: AtomicU64,
}

impl SearchStats {
    pub fn new() -> Self {
        Self {
            nodes: AtomicU64::new(0),
            start_time: Instant::now(),
            current_depth: AtomicU64::new(0),
        }
    }

    pub fn increment_nodes(&self) {
        self.nodes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    pub fn get_nps(&self) -> u64 {
        let elapsed_secs = self.start_time.elapsed().as_secs_f64();
        if elapsed_secs > 0.0 {
            (self.nodes.load(Ordering::Relaxed) as f64 / elapsed_secs) as u64
        } else {
            0
        }
    }

    pub fn should_stop(&self, limits: &SearchLimits, stop_flag: &Arc<AtomicBool>) -> bool {
        // Check external stop flag
        if stop_flag.load(Ordering::Relaxed) {
            return true;
        }

        if limits.infinite {
            return false;
        }

        let node_count = self.nodes.load(Ordering::Relaxed);

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
}

/// Formats and prints UCI info string
pub fn print_uci_info(depth: usize, score: f32, pv: &[BoardMove], stats: &SearchStats) {
    let mut info = format!("info depth {}", depth);

    // TODO: implement checkmate
    info.push_str(&format!(" score cp {}", score));

    // Add nodes
    info.push_str(&format!(" nodes {}", stats.nodes.load(Ordering::Relaxed)));

    // Add nps
    info.push_str(&format!(" nps {}", stats.get_nps()));

    // Add time
    info.push_str(&format!(" time {}", stats.get_elapsed_ms()));

    // Add principal variation
    if !pv.is_empty() {
        info.push_str(" pv");
        for mv in pv {
            info.push_str(&format!(" {}", mv.unparse()));
        }
    }

    println!("{}", info);
}

pub fn iterative_deepening(
    game: &mut Game,
    limits: SearchLimits,
    stop_flag: Arc<AtomicBool>,
) -> SearchResult {
    let stats = Arc::new(SearchStats::new());
    let mut best_result = SearchResult::leaf(0.0);

    for depth in 1..=limits.max_depth.unwrap_or(256) {
        stats.current_depth.store(depth as u64, Ordering::Relaxed);

        let result = alpha_beta(
            game,
            depth,
            0,
            -f32::INFINITY,
            f32::INFINITY,
            &stop_flag,
            &stats,
            &limits,
        );

        if !stats.should_stop(&limits, &stop_flag) {
            // TODO: this is suspect -- we should check for improvement?
            print_uci_info(depth, result.evaluation, &result.pv, &stats);
            best_result = result;
        } else {
            break;
        }
    }

    best_result
}

fn alpha_beta(
    game: &mut Game,
    depth: usize,
    ply: usize,
    mut alpha: f32,
    beta: f32,
    stop_flag: &Arc<AtomicBool>,
    stats: &Arc<SearchStats>,
    limits: &SearchLimits,
) -> SearchResult {
    stats.increment_nodes();
    if stats.should_stop(&limits, &stop_flag) {
        return SearchResult::leaf(game.evaluate());
    }

    // TODO: quiescence search
    if depth == 0 {
        return SearchResult::leaf(game.evaluate());
    }

    let (move_count, moves) = game.get_moves();

    if move_count == 0 {
        if game.is_king_in_check(!game.side) {
            return SearchResult::leaf(-CHECKMATE_SCORE + ply as f32);
        } else {
            return SearchResult::leaf(0.0);
        }
    }

    let mut best_move = BoardMove::empty();
    let mut best_value = -f32::INFINITY;
    let mut best_pv = Vec::new();

    for board_move in moves[0..move_count].into_iter() {
        game.make_move(*board_move);
        let result = alpha_beta(
            game,
            depth - 1,
            ply + 1,
            -beta,
            -alpha,
            stop_flag,
            stats,
            limits,
        );
        game.unmake_move();

        let value = -result.evaluation;

        if value > best_value {
            best_value = value;
            best_move = *board_move;
            best_pv = result.pv;
        }

        alpha = alpha.max(value);
        if alpha >= beta {
            break; // Beta cutoff
        }
    }

    SearchResult::with_pv(best_move, best_value, best_pv)
}
