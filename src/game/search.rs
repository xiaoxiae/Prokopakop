use std::fmt::{Display, Formatter, Result};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Instant;

use crate::game::board::{BoardMove, BoardMoveExt, Game};
use crate::game::evaluate::{CHECKMATE_SCORE, get_piece_value};
use crate::game::pieces::{ColoredPiece, Piece};

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

    // This will always be close because of PLY format
    if score.abs() > CHECKMATE_SCORE - 1000.0 {
        let moves_to_mate = (CHECKMATE_SCORE - score.abs()) as i32;

        if score > 0.0 {
            info.push_str(&format!(" score mate {}", moves_to_mate));
        } else {
            info.push_str(&format!(" score mate -{}", moves_to_mate));
        }
    } else {
        info.push_str(&format!(" score cp {}", (score * 100.0) as i32));
    }

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
    let mut previous_pv: Vec<BoardMove> = Vec::new();

    for depth in 1..=limits.max_depth.unwrap_or(256) {
        stats.current_depth.store(depth as u64, Ordering::Relaxed);

        let result = alpha_beta(
            game,
            depth,
            1,
            -f32::INFINITY,
            f32::INFINITY,
            &previous_pv,
            &stop_flag,
            &stats,
            &limits,
        );

        if !stats.should_stop(&limits, &stop_flag) {
            print_uci_info(depth, result.evaluation, &result.pv, &stats);
            best_result = result.clone();
            previous_pv = result.pv;
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
    previous_pv: &[BoardMove],
    stop_flag: &Arc<AtomicBool>,
    stats: &Arc<SearchStats>,
    limits: &SearchLimits,
) -> SearchResult {
    stats.increment_nodes();
    if stats.should_stop(&limits, &stop_flag) {
        return SearchResult::leaf(game.evaluate() * game.side);
    }

    // TODO: quiescence search
    if depth == 0 {
        return SearchResult::leaf(game.evaluate() * game.side);
    }

    let (move_count, mut moves) = game.get_moves();

    if move_count == 0 {
        if game.is_king_in_check(game.side) {
            return SearchResult::leaf(-CHECKMATE_SCORE + ply as f32);
        } else {
            return SearchResult::leaf(0.0);
        }
    }

    // Order moves with PV first
    order_moves_with_pv(game, &mut moves[0..move_count], previous_pv.get(0).copied());

    let mut best_move = BoardMove::empty();
    let mut best_value = -f32::INFINITY;
    let mut best_pv = Vec::new();

    for board_move in moves[0..move_count].iter() {
        game.make_move(*board_move);

        // Pass the PV for the next ply
        let next_pv = if !previous_pv.is_empty() && *board_move == previous_pv[0] {
            &previous_pv[1..]
        } else {
            &[]
        };

        let result = alpha_beta(
            game,
            depth - 1,
            ply + 1,
            -beta,
            -alpha,
            next_pv,
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

/// Order moves with PV move first, then MVV-LVA for captures
fn order_moves_with_pv(game: &Game, moves: &mut [BoardMove], pv_move: Option<BoardMove>) {
    // First, sort by MVV-LVA
    moves.sort_unstable_by(|a, b| {
        let score_a = mvv_lva_score(game, a);
        let score_b = mvv_lva_score(game, b);
        score_b.cmp(&score_a)
    });

    // If we have a PV move, find it and move it to the front
    if let Some(pv) = pv_move {
        if let Some(pv_index) = moves.iter().position(|&m| m == pv) {
            // Move the PV move to the front by rotating
            moves[0..=pv_index].rotate_right(1);
        }
    }
}

/// https://www.chessprogramming.org/MVV-LVA
fn mvv_lva_score(game: &Game, board_move: &BoardMove) -> i32 {
    if let Some((victim_piece, _victim_color)) = game.pieces[board_move.get_to() as usize] {
        // Get the attacking piece
        if let Some((attacker_piece, _attacker_color)) = game.pieces[board_move.get_from() as usize]
        {
            // Get values from the evaluate module
            let victim_value = get_piece_value(victim_piece);
            let attacker_value = get_piece_value(attacker_piece);

            // Special case for king captures (since KING_VALUE is 0 in evaluate.rs)
            let victim_score = if victim_piece == Piece::King {
                10000.0 // Very high value for capturing a king
            } else {
                victim_value
            };

            // MVV-LVA score: victim value * 100 - attacker value
            // Convert to i32 for sorting
            return (victim_score * 100.0 - attacker_value) as i32;
        }
    }

    // Non-captures get a negative score to be sorted after captures
    -1
}
