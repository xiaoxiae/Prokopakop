use std::fmt::{Display, Formatter, Result};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Instant;

use fxhash::FxHashMap;

use crate::game::board::{BoardMove, BoardMoveExt, Game};
use crate::game::evaluate::{CHECKMATE_SCORE, QUEEN_VALUE, get_piece_value};
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

    /// Add a killer move at the given ply
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

    pub fn clear(&mut self) {
        for killers in &mut self.killers {
            killers[0] = BoardMove::empty();
            killers[1] = BoardMove::empty();
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

    // Initialize killer moves table
    let mut killer_moves = KillerMoves::new(256);

    for depth in 1..=limits.max_depth.unwrap_or(256) {
        stats.current_depth.store(depth as u64, Ordering::Relaxed);

        // Clear killer moves for each iteration to avoid move ordering pollution
        // between iterations (optional - you might want to keep them)
        killer_moves.clear();

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
            &mut killer_moves,
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
    killer_moves: &mut KillerMoves,
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

    // Order moves with TT, PV, and killer moves
    let pv_move = previous_pv.get(0).copied();
    order_moves_with_heuristics(
        game,
        &mut moves[0..move_count],
        tt_move,
        pv_move,
        killer_moves.get_killers(ply),
    );

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
            killer_moves,
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
            // Update killer moves if this is a quiet move
            // (captures use MVV-LVA ordering already)
            if !game.is_capture(*board_move) {
                killer_moves.add_killer(ply, *board_move);
            }
            break;
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

fn calculate_delta_margin(game: &Game, board_move: &BoardMove) -> f32 {
    let mut max_gain = 0.0;

    // Add value of captured piece
    if let Some((victim_piece, _victim_color)) = game.pieces[board_move.get_to() as usize] {
        max_gain += if victim_piece == Piece::King {
            10000.0 // Very high value for capturing a king
        } else {
            get_piece_value(victim_piece)
        };
    }

    if let Some((attacker_piece, _attacker_color)) = game.pieces[board_move.get_from() as usize] {
        if attacker_piece == Piece::Pawn {
            let to_rank = board_move.get_to() / 8;
            if to_rank == 0 || to_rank == 7 {
                max_gain += QUEEN_VALUE - get_piece_value(Piece::Pawn);
            }
        }
    }

    max_gain
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

    // Delta pruning margin - add a small safety buffer
    const DELTA_MARGIN: f32 = 50.0; // About half a pawn

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

    // Filter to only captures (and optionally checks) with delta pruning
    let mut capture_moves = Vec::new();
    for i in 0..move_count {
        let board_move = moves[i];

        if game.is_capture(board_move) || game.is_check(board_move) {
            // Apply delta pruning for captures only (not for checks)
            if game.is_capture(board_move) {
                let max_gain = calculate_delta_margin(game, &board_move);

                // Delta pruning: if even the best possible outcome can't improve alpha,
                // skip this move
                if stand_pat + max_gain + DELTA_MARGIN < alpha {
                    continue; // Prune this move
                }
            }

            capture_moves.push(board_move);
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

fn order_moves_with_heuristics(
    game: &Game,
    moves: &mut [BoardMove],
    tt_move: Option<BoardMove>,
    pv_move: Option<BoardMove>,
    killer_moves: [BoardMove; 2],
) {
    // Assign scores to each move for sorting
    let mut move_scores: Vec<(BoardMove, i32)> = moves
        .iter()
        .map(|&mv| {
            let score;

            // PV
            if Some(mv) == pv_move {
                score = 1_000_000;
            }
            // TT
            else if Some(mv) == tt_move {
                score = 900_000;
            }
            // Good captures (MVV-LVA)
            else if game.is_capture(mv) {
                score = 800_000 + mvv_lva_score(game, &mv);
            }
            // Killer moves
            else if mv == killer_moves[0] {
                score = 700_000;
            } else if mv == killer_moves[1] {
                score = 600_000;
            }
            // Other quiet moves are random
            else {
                score = 0;
            }

            (mv, score)
        })
        .collect();

    // Sort by score (highest first)
    move_scores.sort_unstable_by(|a, b| b.1.cmp(&a.1));

    // Copy sorted moves back
    for (i, (mv, _)) in move_scores.iter().enumerate() {
        moves[i] = *mv;
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
