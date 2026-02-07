use std::sync::{Arc, Mutex, atomic::AtomicBool};
use std::time::Instant;

use crate::engine::evaluate::{
    CHECKMATE_SCORE, QUEEN_VALUE, calculate_game_phase, get_piece_value,
};
use crate::engine::killer::KillerMoves;
use crate::engine::table::{NodeType, TranspositionTable};
use crate::game::board::{BoardMove, BoardMoveExt, Game};
use crate::game::pieces::{Color, Piece};

use super::history::History;
use super::limits::SearchLimits;
use super::params::{
    ASPIRATION_EXPAND, ASPIRATION_INITIAL, ASPIRATION_MIN, DELTA_PRUNING_MARGIN,
    EXT_FUTILITY_MULTIPLIER, LMR_DIVISOR, LMR_MIN_DEPTH, LMR_MOVE_INDEX, NULL_MOVE_DEPTH_THRESHOLD,
    NULL_MOVE_MIN_DEPTH, NULL_MOVE_REDUCTION, futility_margin, razoring_margin,
    reverse_futility_margin,
};
use super::results::{SearchResult, SearchStats};

/// Main search struct containing all search state
pub struct Search<'a> {
    pub game: &'a mut Game,
    pub stats: SearchStats,
    pub limits: SearchLimits,
    pub tt: &'a mut TranspositionTable,
    pub history: &'a mut History,
    pub killer_moves: KillerMoves,
    pub stop_flag: Arc<AtomicBool>,
    pub uci_info: bool,
}

impl<'a> Search<'a> {
    /// Create a new search instance
    pub fn new(
        game: &'a mut Game,
        limits: SearchLimits,
        stop_flag: Arc<AtomicBool>,
        tt: &'a mut TranspositionTable,
        history: &'a mut History,
        uci_info: bool,
        search_start: Arc<Mutex<Instant>>,
        ponder_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            game,
            stats: SearchStats::new(search_start, ponder_flag),
            limits,
            tt,
            history,
            killer_moves: KillerMoves::new(256),
            stop_flag,
            uci_info,
        }
    }

    /// Run iterative deepening search
    pub fn run(&mut self) -> SearchResult {
        let mut best_completed_result = SearchResult::leaf(0.0);
        let mut previous_pv: Vec<BoardMove> = Vec::new();
        let mut last_iteration_ms = 0u64;

        // If only one move is available, return it immediately
        let (count, moves) = self.game.get_moves();

        if count == 1 && !self.limits.exact {
            let best_move = moves[0];
            let pv = vec![best_move];
            if self.uci_info {
                self.print_uci_info(1, 0.0, &pv);
            }

            return SearchResult {
                best_move,
                evaluation: 0.0,
                pv,
            };
        }

        // Start new search generation
        self.tt.new_search();

        for depth in 1..=self.limits.max_depth.unwrap_or(256) {
            // Check if we have enough time for this iteration (skip for first few depths)
            if depth > 3 && last_iteration_ms > 0 {
                if !self
                    .stats
                    .has_time_for_iteration(&self.limits, last_iteration_ms)
                {
                    if self.uci_info {
                        println!(
                            "info string Skipping depth {} due to time constraints",
                            depth
                        );
                    }
                    break;
                }
            }

            let iteration_start = Instant::now();
            self.stats.current_depth = depth as u64;

            let result = if depth > 1 && !best_completed_result.pv.is_empty() {
                self.aspiration_search(
                    depth,
                    best_completed_result.evaluation,
                    &previous_pv,
                    best_completed_result.best_move,
                )
            } else {
                self.alpha_beta(depth, 1, -f32::INFINITY, f32::INFINITY, &previous_pv)
            };

            // Only accept the result if it's valid (not interrupted)
            if result.is_valid() && !self.stats.should_stop(&self.limits, &self.stop_flag) {
                if self.uci_info {
                    self.print_uci_info(depth, result.evaluation, &result.pv);
                }
                best_completed_result = result.clone();
                previous_pv = result.pv;
                last_iteration_ms = iteration_start.elapsed().as_millis() as u64;

                // If we found a checkmate, stop searching deeper
                if result.evaluation.abs() > CHECKMATE_SCORE - 1000.0 {
                    break;
                }
            } else {
                // Search was interrupted, don't update best_completed_result
                if self.uci_info {
                    println!("info string Search interrupted at depth {}", depth);
                }
                break;
            }
        }

        // Always return the best move from the last completed iteration
        if best_completed_result.best_move == BoardMove::empty() {
            // Emergency fallback: if we somehow have no completed iteration,
            // at least return the first legal move
            let (count, moves) = self.game.get_moves();
            if count > 0 {
                best_completed_result = SearchResult {
                    best_move: moves[0],
                    evaluation: 0.0,
                    pv: vec![moves[0]],
                };
            }
        }

        best_completed_result
    }

    /// Alpha-beta search with negamax
    fn alpha_beta(
        &mut self,
        depth: usize,
        ply: usize,
        mut alpha: f32,
        mut beta: f32,
        previous_pv: &[BoardMove],
    ) -> SearchResult {
        self.stats.increment_nodes();

        if self.stats.should_stop(&self.limits, &self.stop_flag) {
            return SearchResult::interrupted();
        }

        // Threefold repetition checks (only for low depths since this one is costly)
        let zobrist_key = self.game.zobrist_key;

        if self.game.is_fifty_move_rule() {
            return SearchResult::leaf(0.0);
        }

        if ply > 1 && ply <= 6 {
            if self.history.is_threefold_repetition(zobrist_key) {
                return SearchResult::leaf(0.0);
            }
        }

        let original_alpha = alpha;
        let is_pv_node = beta - alpha > 1.0; // PV nodes have open window
        let in_check = self.game.is_king_in_check(self.game.side);

        // Probe transposition table
        let mut tt_move = None;
        if let Some(tt_entry) = self.tt.probe(zobrist_key) {
            tt_move = Some(tt_entry.best_move);

            // Use TT value if depth is sufficient (but not in PV nodes for exact scores)
            if tt_entry.depth >= depth as u8
                && (!is_pv_node || tt_entry.node_type != NodeType::Exact)
            {
                match tt_entry.node_type {
                    NodeType::Exact => {
                        // Exact score - we can return immediately (only in non-PV nodes)
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
                    return SearchResult::with_pv(
                        tt_entry.best_move,
                        tt_entry.evaluation,
                        Vec::new(),
                    );
                }
            }
        }

        // Enter quiescence search to remove the horizon effect
        if depth == 0 {
            return self.quiescence_search(ply, alpha, beta);
        }

        let static_eval = if !in_check {
            self.game.evaluate() * self.game.side
        } else {
            -f32::INFINITY // Don't use static eval when in check
        };

        // Reverse futility pruning (static eval pruning)
        // If our position is so good that even with a margin we're above beta, we can return
        if !is_pv_node && !in_check && depth <= 3 && beta.abs() < CHECKMATE_SCORE - 1000.0 {
            let margin = reverse_futility_margin(depth);
            if static_eval - margin >= beta {
                return SearchResult::leaf(beta);
            }
        }

        // Razoring - drop into quiescence when evaluation is far below alpha at low depths
        if !is_pv_node
            && !in_check
            && depth <= 3
            && depth >= 1
            && alpha.abs() < CHECKMATE_SCORE - 1000.0
        {
            let margin = razoring_margin(depth);

            if static_eval + margin < alpha {
                // Do a quiescence search to verify the position is really bad
                let q_result = self.quiescence_search(ply, alpha, beta);

                // If quiescence confirms we're below alpha, return early
                if q_result.evaluation < alpha {
                    return q_result;
                }
            }
        }

        // Null move pruning (skip in PV nodes)
        // Don't try null move if we're way below beta
        // Also don't do this in king and pawn endgames
        if !is_pv_node
            && depth >= NULL_MOVE_MIN_DEPTH
            && !in_check
            && beta.abs() < CHECKMATE_SCORE - 1000.0
            && static_eval >= beta
            && (self.game.color_bitboards[Color::White as usize]
                | self.game.color_bitboards[Color::Black as usize])
                != (self.game.piece_bitboards[Piece::Pawn as usize]
                    | self.game.piece_bitboards[Piece::King as usize])
        {
            self.game.make_null_move();

            let r = NULL_MOVE_REDUCTION + (depth >= NULL_MOVE_DEPTH_THRESHOLD) as usize;
            let null_result = self.alpha_beta(
                depth.saturating_sub(1 + r),
                ply + 1,
                -beta,
                -beta + 1.0, // Null window
                &[],
            );

            self.game.unmake_null_move();

            if -null_result.evaluation >= beta {
                return SearchResult::leaf(beta); // Fail high
            }
        }

        // Check if futility pruning can be applied to this node
        let futility_pruning_enabled =
            !is_pv_node && !in_check && depth <= 3 && alpha.abs() < CHECKMATE_SCORE - 1000.0;

        let fut_margin = if futility_pruning_enabled {
            futility_margin(depth)
        } else {
            f32::INFINITY
        };

        let can_prune_node = futility_pruning_enabled && static_eval + fut_margin <= alpha;

        let (move_count, mut moves) = self.game.get_moves();

        if move_count == 0 {
            let eval = if in_check {
                -CHECKMATE_SCORE + ply as f32
            } else {
                0.0
            };

            self.tt.store(
                zobrist_key,
                depth as u8,
                eval,
                BoardMove::empty(),
                NodeType::Exact,
            );

            return SearchResult::leaf(eval);
        }

        let pv_move = previous_pv.get(0).copied();
        self.order_moves(
            &mut moves[0..move_count],
            tt_move,
            pv_move,
            self.killer_moves.get_killers(ply),
        );

        let mut best_move = BoardMove::empty();
        let mut best_value = -f32::INFINITY;
        let mut best_pv = Vec::new();
        let mut moves_searched = 0;
        let mut quiet_moves_searched = 0;

        for (move_index, board_move) in moves[0..move_count].iter().enumerate() {
            let is_capture = self.game.is_capture(*board_move);
            let is_promotion = board_move.get_promotion().is_some();
            let gives_check = self.game.is_check(*board_move);

            let is_quiet_move = !is_capture && !is_promotion && !gives_check;

            // Futility pruning: Skip quiet moves if position is hopeless
            if moves_searched > 0 && can_prune_node && is_quiet_move {
                continue;
            }

            // Extended futility pruning for individual moves at depth 2-3
            if futility_pruning_enabled && depth >= 2 && is_quiet_move && quiet_moves_searched >= 3
            {
                // Use a more aggressive margin for individual move pruning
                let move_fut_margin = fut_margin * EXT_FUTILITY_MULTIPLIER;
                if static_eval + move_fut_margin <= alpha {
                    quiet_moves_searched += 1;
                    continue;
                }
            }

            self.game.make_move(*board_move);

            let new_zobrist = self.game.zobrist_key;
            self.history.push_position(new_zobrist);

            // Pass the PV for the next ply
            let next_pv = if !previous_pv.is_empty() && *board_move == previous_pv[0] {
                &previous_pv[1..]
            } else {
                &[]
            };

            let mut value;

            // PVS: First move gets full window, others get null window first
            if moves_searched == 0 {
                // Search the first move with full window
                let result = self.alpha_beta(depth - 1, ply + 1, -beta, -alpha, next_pv);
                value = -result.evaluation;

                if !result.is_valid() {
                    self.history.pop_position();
                    self.game.unmake_move();
                    return SearchResult::interrupted();
                }

                if value > best_value {
                    best_value = value;
                    best_move = *board_move;
                    best_pv = result.pv;
                }
            } else {
                // Late move reduction for non-PV moves
                if move_index >= LMR_MOVE_INDEX
                    && depth >= LMR_MIN_DEPTH
                    && is_quiet_move
                    && !in_check
                    && !self.game.is_king_in_check(!self.game.side)
                {
                    // More reduction for late moves and high depths
                    let mut reduction =
                        ((depth as f32).ln() * (move_index as f32).ln() / LMR_DIVISOR) as usize;
                    reduction = reduction.clamp(1, depth - 1);

                    // Reduce less in PV nodes (when window is wider)
                    if is_pv_node {
                        reduction = reduction.saturating_sub(1).max(1);
                    }

                    // Search with reduced depth first
                    let reduced_depth = depth.saturating_sub(1 + reduction);
                    let reduced_result =
                        self.alpha_beta(reduced_depth, ply + 1, -alpha - 1.0, -alpha, next_pv);

                    if !reduced_result.is_valid() {
                        self.history.pop_position();
                        self.game.unmake_move();
                        return SearchResult::interrupted();
                    }

                    value = -reduced_result.evaluation;

                    // If the move fails low, skip it
                    if value <= alpha {
                        // Penalize this move in history since it failed low
                        self.history
                            .add_history_penalty(*board_move, !self.game.side, depth);

                        self.history.pop_position();
                        self.game.unmake_move();
                        moves_searched += 1;
                        if is_quiet_move {
                            quiet_moves_searched += 1;
                        }
                        continue;
                    }
                    // Otherwise, we need to do a full depth search
                }

                // PVS: Search with null window first
                let null_window_result =
                    self.alpha_beta(depth - 1, ply + 1, -alpha - 1.0, -alpha, next_pv);

                if !null_window_result.is_valid() {
                    self.history.pop_position();
                    self.game.unmake_move();
                    return SearchResult::interrupted();
                }

                value = -null_window_result.evaluation;

                // If the null window search fails high, re-search with full window
                if value > alpha && value < beta {
                    let full_window_result =
                        self.alpha_beta(depth - 1, ply + 1, -beta, -alpha, next_pv);
                    value = -full_window_result.evaluation;

                    if value > best_value {
                        best_value = value;
                        best_move = *board_move;
                        best_pv = full_window_result.pv;
                    }
                } else if value > best_value {
                    // Even though it didn't require re-search, update best if it's better
                    best_value = value;
                    best_move = *board_move;
                    best_pv = null_window_result.pv;
                }
            }

            self.history.pop_position();
            self.game.unmake_move();
            moves_searched += 1;
            if is_quiet_move {
                quiet_moves_searched += 1;
            }

            alpha = alpha.max(best_value);
            if alpha >= beta {
                // This move caused a beta cutoff - it's a good move!
                if !self.game.is_capture(*board_move) {
                    self.killer_moves.add_killer(ply, *board_move);
                    self.history.add_history(*board_move, self.game.side, depth);
                }
                break;
            } else if value <= original_alpha {
                // This move didn't improve alpha - penalize it
                if !self.game.is_capture(*board_move) {
                    self.history
                        .add_history_penalty(*board_move, self.game.side, depth);
                }
            }
        }

        let node_type = if best_value <= original_alpha {
            NodeType::UpperBound // No move improved alpha
        } else if best_value >= beta {
            NodeType::LowerBound // Beta cutoff
        } else {
            NodeType::Exact // Exact value
        };

        self.tt
            .store(zobrist_key, depth as u8, best_value, best_move, node_type);

        // Don't include empty PV moves
        if best_move == BoardMove::empty() {
            // If no move was selected (all pruned or failed), return leaf evaluation
            SearchResult::leaf(best_value)
        } else {
            SearchResult::with_pv(best_move, best_value, best_pv)
        }
    }

    /// Aspiration search with window narrowing
    fn aspiration_search(
        &mut self,
        depth: usize,
        previous_score: f32,
        previous_pv: &[BoardMove],
        previous_best_move: BoardMove,
    ) -> SearchResult {
        // Don't use aspiration windows for checkmate scores
        if previous_score.abs() > CHECKMATE_SCORE - 1000.0 {
            return self.alpha_beta(depth, 1, -f32::INFINITY, f32::INFINITY, previous_pv);
        }

        // Skip aspiration windows for low depths (<=4)
        if depth <= 4 {
            return self.alpha_beta(depth, 1, -f32::INFINITY, f32::INFINITY, previous_pv);
        }

        // Exponential narrowing: starting at initial and approaching min at higher depths
        let initial_window = (ASPIRATION_INITIAL
            * (ASPIRATION_MIN / ASPIRATION_INITIAL).powf((depth as f32 - 4.0) / 10.0))
        .max(ASPIRATION_MIN);

        let mut alpha = previous_score - initial_window;
        let mut beta = previous_score + initial_window;

        let mut fail_high_count = 0;
        let mut fail_low_count = 0;

        loop {
            let result = self.alpha_beta(depth, 1, alpha, beta, previous_pv);

            // If search was interrupted, return the previous best move
            if !result.is_valid() {
                return SearchResult {
                    best_move: previous_best_move,
                    evaluation: previous_score,
                    pv: previous_pv.to_vec(),
                };
            }

            // If search was interrupted and returned empty move, fall back to previous result
            if result.best_move == BoardMove::empty() && previous_best_move != BoardMove::empty() {
                return SearchResult {
                    best_move: previous_best_move,
                    evaluation: previous_score,
                    pv: previous_pv.to_vec(),
                };
            }

            // Check if we should stop before continuing
            if self.stats.should_stop(&self.limits, &self.stop_flag) {
                // Return the last valid result we have
                if result.best_move != BoardMove::empty() {
                    return result;
                } else {
                    return SearchResult {
                        best_move: previous_best_move,
                        evaluation: previous_score,
                        pv: previous_pv.to_vec(),
                    };
                }
            }

            if result.evaluation <= alpha {
                fail_low_count += 1;
                fail_high_count = 0;

                if self.uci_info {
                    println!(
                        "info string Aspiration fail low at depth {} (attempt {}), widening alpha",
                        depth, fail_low_count
                    );
                }

                if fail_low_count >= 1 {
                    if self.uci_info {
                        println!("info string Second fail low, switching to full window search");
                    }
                    let fallback_result =
                        self.alpha_beta(depth, 1, -f32::INFINITY, f32::INFINITY, previous_pv);

                    if fallback_result.best_move == BoardMove::empty()
                        && previous_best_move != BoardMove::empty()
                    {
                        return SearchResult {
                            best_move: previous_best_move,
                            evaluation: previous_score,
                            pv: previous_pv.to_vec(),
                        };
                    }

                    return fallback_result;
                }

                let delta = previous_score - alpha;
                alpha = previous_score - delta * ASPIRATION_EXPAND;
            } else if result.evaluation >= beta {
                fail_high_count += 1;
                fail_low_count = 0;

                if self.uci_info {
                    println!(
                        "info string Aspiration fail high at depth {} (attempt {}), widening beta",
                        depth, fail_high_count
                    );
                }

                if fail_high_count >= 1 {
                    if self.uci_info {
                        println!("info string Second fail high, switching to full window search");
                    }
                    let fallback_result =
                        self.alpha_beta(depth, 1, -f32::INFINITY, f32::INFINITY, previous_pv);

                    if fallback_result.best_move == BoardMove::empty()
                        && previous_best_move != BoardMove::empty()
                    {
                        return SearchResult {
                            best_move: previous_best_move,
                            evaluation: previous_score,
                            pv: previous_pv.to_vec(),
                        };
                    }

                    return fallback_result;
                }

                let delta = beta - previous_score;
                beta = previous_score + delta * ASPIRATION_EXPAND;
            } else {
                return result;
            }
        }
    }

    /// Quiescence search for tactical moves
    fn quiescence_search(&mut self, ply: usize, mut alpha: f32, beta: f32) -> SearchResult {
        self.stats.increment_nodes();

        if self.stats.should_stop(&self.limits, &self.stop_flag) {
            return SearchResult::interrupted();
        }

        // Limit quiescence search depth to prevent explosion
        const MAX_QUIESCENCE_PLY: usize = 32;
        if ply > MAX_QUIESCENCE_PLY {
            return SearchResult::leaf(self.game.evaluate() * self.game.side);
        }

        let stand_pat = self.game.evaluate() * self.game.side;

        // If we're already doing well enough to cause a beta cutoff, we can return
        if stand_pat >= beta {
            return SearchResult::leaf(beta);
        }

        // Update alpha with standing pat score
        if stand_pat > alpha {
            alpha = stand_pat;
        }

        // Get all moves
        let (move_count, moves) = self.game.get_moves();

        // If no moves available, check for checkmate or stalemate
        if move_count == 0 {
            if self.game.is_king_in_check(self.game.side) {
                return SearchResult::leaf(-CHECKMATE_SCORE + ply as f32);
            } else {
                return SearchResult::leaf(0.0);
            }
        }

        let game_phase = calculate_game_phase(self.game);

        // Filter to only captures (and optionally checks) with delta pruning
        let mut capture_moves = Vec::new();
        for i in 0..move_count {
            let board_move = moves[i];

            // SEE pruning: skip captures that lose material
            // Don't apply to checks since they might have tactical value
            if self.game.is_capture(board_move) {
                let see_value = self.game.see(board_move.get_to());
                if see_value < 0.0 {
                    continue;
                }
            }

            // Only extend checks for the first ply, since the check is super expensive
            if self.game.is_capture(board_move) || (ply <= 1 && self.game.is_check(board_move)) {
                // Apply delta pruning for captures only (not for checks)
                // Don't do this for endgames though since we might miss stuff
                if game_phase < 0.7 && self.game.is_capture(board_move) {
                    let max_gain = self.calculate_delta_margin(&board_move);

                    // Delta pruning: if even the best possible outcome can't improve alpha,
                    // skip this move; margin is tunable (default about half a pawn)
                    if stand_pat + max_gain + DELTA_PRUNING_MARGIN < alpha {
                        continue;
                    }
                }

                capture_moves.push(board_move);
            }
        }

        // If no captures/checks available, return the standing pat evaluation
        if capture_moves.is_empty() {
            return SearchResult::leaf(stand_pat);
        }

        capture_moves.sort_unstable_by(|a, b| {
            let score_a = self.mvv_lva_score(a);
            let score_b = self.mvv_lva_score(b);
            score_b.cmp(&score_a)
        });

        let mut best_value = stand_pat;
        let mut best_move = BoardMove::empty();
        let mut best_pv = Vec::new();

        for board_move in capture_moves.iter() {
            self.game.make_move(*board_move);

            let result = self.quiescence_search(ply + 1, -beta, -alpha);

            if !result.is_valid() {
                self.game.unmake_move();
                return SearchResult::interrupted();
            }

            self.game.unmake_move();

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

    /// Order moves using various heuristics
    fn order_moves(
        &self,
        moves: &mut [BoardMove],
        tt_move: Option<BoardMove>,
        pv_move: Option<BoardMove>,
        killer_moves: [BoardMove; 2],
    ) {
        moves.sort_unstable_by_key(|&mv| {
            if Some(mv) == pv_move {
                -1_000_000
            } else if Some(mv) == tt_move {
                -900_000
            } else if self.game.is_capture(mv) {
                let see = self.game.see_sign(mv.get_to());

                if see > 0 {
                    -800_000 - self.mvv_lva_score(&mv)
                } else {
                    -400_000 - self.mvv_lva_score(&mv)
                }
            } else if mv == killer_moves[0] {
                -700_000
            } else if mv == killer_moves[1] {
                -600_000
            } else {
                -500_000 - self.history.get_history_score(&mv, self.game.side)
            }
        });
    }

    /// Calculate MVV-LVA score for move ordering
    fn mvv_lva_score(&self, board_move: &BoardMove) -> i32 {
        if let Some((victim_piece, _victim_color)) = self.game.pieces[board_move.get_to() as usize]
        {
            if let Some((attacker_piece, _attacker_color)) =
                self.game.pieces[board_move.get_from() as usize]
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

        -1
    }

    /// Calculate maximum possible gain from a capture move
    fn calculate_delta_margin(&self, board_move: &BoardMove) -> f32 {
        let mut max_gain = 0.0;

        // Add value of captured piece
        if let Some((victim_piece, _victim_color)) = self.game.pieces[board_move.get_to() as usize]
        {
            max_gain += if victim_piece == Piece::King {
                10000.0
            } else {
                get_piece_value(victim_piece)
            };
        }

        if let Some((attacker_piece, _attacker_color)) =
            self.game.pieces[board_move.get_from() as usize]
        {
            if attacker_piece == Piece::Pawn {
                let to_rank = board_move.get_to() / 8;
                if to_rank == 0 || to_rank == 7 {
                    max_gain += QUEEN_VALUE - get_piece_value(Piece::Pawn);
                }
            }
        }

        max_gain
    }

    /// Print UCI info string with search statistics
    fn print_uci_info(&mut self, depth: usize, mut score: f32, pv: &[BoardMove]) {
        let mut info = format!("info depth {}", depth);

        // Convert score to white's perspective for UCI output
        score = score * self.game.side;

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
            info.push_str(&format!(" score cp {}", score as i32));
        }

        // Add nodes
        info.push_str(&format!(" nodes {}", self.stats.nodes));

        // Add nps
        info.push_str(&format!(" nps {}", self.stats.get_nps()));

        // Add time
        info.push_str(&format!(" time {}", self.stats.get_elapsed_ms()));

        // Add hashtable information
        info.push_str(&format!(" hashfull {}", self.tt.get_fullness_permille()));

        let hit_rate = self.tt.get_hit_rate_percent();
        if hit_rate > 0 {
            info.push_str(&format!(" tbhits {}", hit_rate));
        }

        // Add principal variation
        if !pv.is_empty() {
            info.push_str(" pv");
            for mv in pv {
                info.push_str(&format!(" {}", mv.unparse()));
            }
        }

        println!("{}", info);
    }
}
