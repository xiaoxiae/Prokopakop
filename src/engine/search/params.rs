/// Tunable search parameters for SPSA optimization.
///
/// These are compile-time constants. The SPSA tuner modifies this file
/// directly and recompiles the engine for each iteration.
///
/// Format: NAME, current_value, min, max, description

// Futility pruning margins (centipawns)
pub const FUTILITY_MARGIN_1: f32 = 72.57344087043118; // min: 50, max: 400
pub const FUTILITY_MARGIN_2: f32 = 345.2369180807008; // min: 200, max: 600
pub const FUTILITY_MARGIN_3: f32 = 639.7607284522868; // min: 300, max: 900

// Reverse futility pruning margins (centipawns)
pub const REVERSE_FUTILITY_MARGIN_1: f32 = 175.71091054749604; // min: 75, max: 300
pub const REVERSE_FUTILITY_MARGIN_2: f32 = 193.24236405805058; // min: 100, max: 500
pub const REVERSE_FUTILITY_MARGIN_3: f32 = 368.2479878202008; // min: 200, max: 750

// Razoring margins (centipawns)
pub const RAZORING_MARGIN_1: f32 = 312.29019632294074; // min: 150, max: 500
pub const RAZORING_MARGIN_2: f32 = 456.325330192876; // min: 250, max: 700
pub const RAZORING_MARGIN_3: f32 = 744.0629221396491; // min: 350, max: 900

// Null move pruning
pub const NULL_MOVE_REDUCTION: usize = 2; // min: 1, max: 4
pub const NULL_MOVE_DEPTH_THRESHOLD: usize = 6; // min: 4, max: 8
pub const NULL_MOVE_MIN_DEPTH: usize = 3; // min: 2, max: 5

// Late move reduction
pub const LMR_DIVISOR: f32 = 1.4195619487441853; // min: 0.5, max: 4.0
pub const LMR_MIN_DEPTH: usize = 3; // min: 2, max: 5
pub const LMR_MOVE_INDEX: usize = 3; // min: 2, max: 6

// Extended futility
pub const EXT_FUTILITY_MULTIPLIER: f32 = 1.147072476298593; // min: 0.8, max: 2.5

// Delta pruning (quiescence) - centipawns
pub const DELTA_PRUNING_MARGIN: f32 = 61.35552227115877; // min: 25, max: 100

// Aspiration windows
pub const ASPIRATION_INITIAL: f32 = 47.8128964512257; // min: 25, max: 100
pub const ASPIRATION_MIN: f32 = 21.83962275724517; // min: 10, max: 50
pub const ASPIRATION_EXPAND: f32 = 2.3225785873337497; // min: 1.5, max: 4.0

/// Helper functions for depth-indexed lookups
#[inline(always)]
pub const fn futility_margin(depth: usize) -> f32 {
    match depth {
        0 => 0.0,
        1 => FUTILITY_MARGIN_1,
        2 => FUTILITY_MARGIN_2,
        _ => FUTILITY_MARGIN_3,
    }
}

#[inline(always)]
pub const fn reverse_futility_margin(depth: usize) -> f32 {
    match depth {
        0 => 0.0,
        1 => REVERSE_FUTILITY_MARGIN_1,
        2 => REVERSE_FUTILITY_MARGIN_2,
        _ => REVERSE_FUTILITY_MARGIN_3,
    }
}

#[inline(always)]
pub const fn razoring_margin(depth: usize) -> f32 {
    match depth {
        1 => RAZORING_MARGIN_1,
        2 => RAZORING_MARGIN_2,
        _ => RAZORING_MARGIN_3,
    }
}
