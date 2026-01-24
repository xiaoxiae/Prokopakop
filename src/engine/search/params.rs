/// Tunable search parameters for SPSA optimization.
///
/// These are compile-time constants. The SPSA tuner modifies this file
/// directly and recompiles the engine for each iteration.
///
/// Format: NAME, current_value, min, max, description

// Futility pruning margins (centipawns)
pub const FUTILITY_MARGIN_1: f32 = 200.0; // min: 100, max: 400
pub const FUTILITY_MARGIN_2: f32 = 400.0; // min: 200, max: 600
pub const FUTILITY_MARGIN_3: f32 = 600.0; // min: 300, max: 900

// Reverse futility pruning margins (centipawns)
pub const REVERSE_FUTILITY_MARGIN_1: f32 = 150.0; // min: 75, max: 300
pub const REVERSE_FUTILITY_MARGIN_2: f32 = 300.0; // min: 150, max: 500
pub const REVERSE_FUTILITY_MARGIN_3: f32 = 500.0; // min: 250, max: 750

// Razoring margins (centipawns)
pub const RAZORING_MARGIN_1: f32 = 300.0; // min: 150, max: 500
pub const RAZORING_MARGIN_2: f32 = 450.0; // min: 250, max: 700
pub const RAZORING_MARGIN_3: f32 = 600.0; // min: 350, max: 900

// Null move pruning
pub const NULL_MOVE_REDUCTION: usize = 2; // min: 1, max: 4
pub const NULL_MOVE_DEPTH_THRESHOLD: usize = 6; // min: 4, max: 8
pub const NULL_MOVE_MIN_DEPTH: usize = 3; // min: 2, max: 5

// Late move reduction
pub const LMR_DIVISOR: f32 = 2.0; // min: 1.0, max: 4.0
pub const LMR_MIN_DEPTH: usize = 3; // min: 2, max: 5
pub const LMR_MOVE_INDEX: usize = 3; // min: 2, max: 6

// Extended futility
pub const EXT_FUTILITY_MULTIPLIER: f32 = 1.5; // min: 1.0, max: 2.5

// Delta pruning (quiescence) - centipawns
pub const DELTA_PRUNING_MARGIN: f32 = 50.0; // min: 25, max: 100

// Aspiration windows
pub const ASPIRATION_INITIAL: f32 = 50.0; // min: 25, max: 100
pub const ASPIRATION_MIN: f32 = 25.0; // min: 10, max: 50
pub const ASPIRATION_EXPAND: f32 = 2.5; // min: 1.5, max: 4.0

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
