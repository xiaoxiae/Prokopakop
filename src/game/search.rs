use std::fmt::{Display, Formatter, Result};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Instant;

use fxhash::FxHashMap;

use crate::game::board::{BoardMove, BoardMoveExt, Game};
use crate::game::evaluate::{CHECKMATE_SCORE, get_piece_value};
use crate::game::opening_book::OpeningBook;
use crate::game::pieces::Piece;
use crate::game::table::{GameTranspositionExt, NodeType, TranspositionTable};

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

/// Formats and prints UCI info string
pub fn print_uci_info(depth: usize, score: f32, pv: &[BoardMove], stats: &SearchStats) {
    let mut info = format!("info depth {}", depth);

    // Check if this is a checkmate score
    if score.abs() > CHECKMATE_SCORE - 1000.0 {
        // Calculate moves to mate (converting from plies to moves)
        let plies_to_mate = (CHECKMATE_SCORE - score.abs()) as i32;

        // Convert plies to moves (round up)
        let moves_to_mate = (plies_to_mate + 1) / 2;

        // Handle the special case of already being in checkmate
        if moves_to_mate == 0 {
            info.push_str(" score mate 0");
        } else if score > 0.0 {
            // We're winning - delivering checkmate
            info.push_str(&format!(" score mate {}", moves_to_mate));
        } else {
            // We're losing - being checkmated
            info.push_str(&format!(" score mate -{}", moves_to_mate));
        }
    } else {
        // Regular centipawn score
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
    tt: &mut TranspositionTable,
    position_history: &mut PositionHistory,
    opening_book: Option<&OpeningBook>,
) -> SearchResult {
    let stats = Arc::new(SearchStats::new());
    let mut best_result = SearchResult::leaf(0.0);
    let mut previous_pv: Vec<BoardMove> = Vec::new();

    // Check opening book first
    if let Some(book) = opening_book {
        if let Some(best_move) = book.get_best_move(game.zobrist_key) {
            println!("info string Using opening book move");

            // Use a neutral evaluation since opening book moves don't have evaluations
            let pv = vec![best_move];
            print_uci_info(1, 0.0, &pv, &stats);

            return SearchResult {
                best_move,
                evaluation: 0.0,
                pv,
            };
        }
    }

    // Start new search generation
    tt.new_search();

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
            tt,
            position_history,
        );

        if !stats.should_stop(&limits, &stop_flag) {
            print_uci_info(depth, result.evaluation, &result.pv, &stats);
            best_result = result.clone();
            previous_pv = result.pv;

            // If we found a checkmate, stop searching deeper
            if result.evaluation.abs() > CHECKMATE_SCORE - 1000.0 {
                break;
            }
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
    mut beta: f32,
    previous_pv: &[BoardMove],
    stop_flag: &Arc<AtomicBool>,
    stats: &Arc<SearchStats>,
    limits: &SearchLimits,
    tt: &mut TranspositionTable,
    position_history: &mut PositionHistory,
) -> SearchResult {
    stats.increment_nodes();
    if stats.should_stop(&limits, &stop_flag) {
        return SearchResult::leaf(game.evaluate() * game.side);
    }

    // Threefold repetition checks
    let zobrist_key = game.get_zobrist_key();
    if ply > 1 && ply <= 4 {
        if position_history.is_threefold_repetition(zobrist_key) {
            return SearchResult::leaf(0.0); // Draw by repetition
        }
    }

    let original_alpha = alpha;

    // Probe transposition table
    let mut tt_move = None;
    if let Some(tt_entry) = tt.probe(zobrist_key) {
        tt_move = Some(tt_entry.best_move);

        // Use TT value if depth is sufficient
        if tt_entry.depth >= depth as u8 {
            match tt_entry.node_type {
                NodeType::Exact => {
                    // Exact score - we can return immediately
                    return SearchResult::with_pv(
                        tt_entry.best_move,
                        tt_entry.evaluation,
                        Vec::new(),
                    );
                }
                NodeType::LowerBound => {
                    // Beta cutoff occurred, evaluation is a lower bound
                    alpha = alpha.max(tt_entry.evaluation);
                }
                NodeType::UpperBound => {
                    // No move improved alpha, evaluation is an upper bound
                    beta = beta.min(tt_entry.evaluation);
                }
            }

            // Check for alpha-beta cutoff after adjusting bounds
            if alpha >= beta {
                return SearchResult::with_pv(tt_entry.best_move, tt_entry.evaluation, Vec::new());
            }
        }
    }

    // Enter quiescence search to remove the horizon effect
    if depth == 0 {
        return quiescence_search(game, ply, alpha, beta, stop_flag, stats, limits);
    }

    let (move_count, mut moves) = game.get_moves();

    if move_count == 0 {
        let eval = if game.is_king_in_check(game.side) {
            -CHECKMATE_SCORE + ply as f32
        } else {
            0.0
        };

        tt.store(
            zobrist_key,
            depth as u8,
            eval,
            BoardMove::empty(),
            NodeType::Exact,
        );

        return SearchResult::leaf(eval);
    }

    // Order moves with TT move and PV
    let pv_move = previous_pv.get(0).copied();
    order_moves_with_tt_and_pv(game, &mut moves[0..move_count], tt_move, pv_move);

    let mut best_move = BoardMove::empty();
    let mut best_value = -f32::INFINITY;
    let mut best_pv = Vec::new();

    for board_move in moves[0..move_count].iter() {
        game.make_move(*board_move);

        // Add position to history for repetition detection
        let new_zobrist = game.get_zobrist_key();
        position_history.push(new_zobrist);

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
            tt,
            position_history,
        );

        // Remove position from history after unmaking the move
        position_history.pop();
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

    // Store in transposition table
    let node_type = if best_value <= original_alpha {
        NodeType::UpperBound // No move improved alpha
    } else if best_value >= beta {
        NodeType::LowerBound // Beta cutoff
    } else {
        NodeType::Exact // Exact value
    };

    tt.store(zobrist_key, depth as u8, best_value, best_move, node_type);

    SearchResult::with_pv(best_move, best_value, best_pv)
}

fn quiescence_search(
    game: &mut Game,
    ply: usize,
    mut alpha: f32,
    beta: f32,
    stop_flag: &Arc<AtomicBool>,
    stats: &Arc<SearchStats>,
    limits: &SearchLimits,
) -> SearchResult {
    stats.increment_nodes();

    if stats.should_stop(&limits, &stop_flag) {
        return SearchResult::leaf(game.evaluate() * game.side);
    }

    // Limit quiescence search depth to prevent explosion
    const MAX_QUIESCENCE_PLY: usize = 32;
    if ply > MAX_QUIESCENCE_PLY {
        return SearchResult::leaf(game.evaluate() * game.side);
    }

    let stand_pat = game.evaluate() * game.side;

    // If we're already doing well enough to cause a beta cutoff, we can return
    if stand_pat >= beta {
        return SearchResult::leaf(beta);
    }

    // Update alpha with standing pat score
    if stand_pat > alpha {
        alpha = stand_pat;
    }

    // Get all moves
    let (move_count, moves) = game.get_moves();

    // If no moves available, check for checkmate or stalemate
    if move_count == 0 {
        if game.is_king_in_check(game.side) {
            return SearchResult::leaf(-CHECKMATE_SCORE + ply as f32);
        } else {
            return SearchResult::leaf(0.0);
        }
    }

    // Filter to only captures (and optionally checks)
    let mut capture_moves = Vec::new();
    for i in 0..move_count {
        if game.is_capture(moves[i]) || game.is_check(moves[i]) {
            capture_moves.push(moves[i]);
        }
    }

    // If no captures/checks available, return the standing pat evaluation
    if capture_moves.is_empty() {
        return SearchResult::leaf(stand_pat);
    }

    // Order captures by MVV-LVA
    capture_moves.sort_unstable_by(|a, b| {
        let score_a = mvv_lva_score(game, a);
        let score_b = mvv_lva_score(game, b);
        score_b.cmp(&score_a)
    });

    let mut best_value = stand_pat;
    let mut best_move = BoardMove::empty();
    let mut best_pv = Vec::new();

    for board_move in capture_moves.iter() {
        game.make_move(*board_move);

        let result = quiescence_search(game, ply + 1, -beta, -alpha, stop_flag, stats, limits);

        // Remove position from history
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

    // Return the best result found
    if best_move == BoardMove::empty() {
        SearchResult::leaf(best_value)
    } else {
        SearchResult::with_pv(best_move, best_value, best_pv)
    }
}

fn order_moves_with_tt_and_pv(
    game: &Game,
    moves: &mut [BoardMove],
    tt_move: Option<BoardMove>,
    pv_move: Option<BoardMove>,
) {
    // First, sort by MVV-LVA
    moves.sort_unstable_by(|a, b| {
        let score_a = mvv_lva_score(game, a);
        let score_b = mvv_lva_score(game, b);
        score_b.cmp(&score_a)
    });

    // Find indices of special moves
    let pv_index = pv_move.and_then(|pv| moves.iter().position(|&m| m == pv));
    let tt_index = tt_move.and_then(|tt| {
        if Some(tt) != pv_move {
            moves.iter().position(|&m| m == tt)
        } else {
            None
        }
    });

    // Move PV to front if it exists
    if let Some(idx) = pv_index {
        // Rotate to bring PV move to front
        moves[0..=idx].rotate_right(1);
    }

    // Move TT to second position if it exists and is different from PV
    if let Some(idx) = tt_index {
        // Adjust index if PV was moved
        let adjusted_idx = if pv_index.is_some() && idx > pv_index.unwrap() {
            idx
        } else if pv_index.is_some() && idx < pv_index.unwrap() {
            idx + 1
        } else {
            idx
        };

        // Rotate to bring TT move to position 1 (or 0 if no PV)
        let start_pos = if pv_index.is_some() { 1 } else { 0 };
        if adjusted_idx >= start_pos {
            moves[start_pos..=adjusted_idx].rotate_right(1);
        }
    }
}

fn mvv_lva_score(game: &Game, board_move: &BoardMove) -> i32 {
    if let Some((victim_piece, _victim_color)) = game.pieces[board_move.get_to() as usize] {
        if let Some((attacker_piece, _attacker_color)) = game.pieces[board_move.get_from() as usize]
        {
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
