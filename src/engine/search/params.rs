/// Tunable search parameters for SPSA optimization.
///
/// These are compile-time constants. The SPSA tuner modifies this file
/// directly and recompiles the engine for each iteration.
///
/// Format: NAME, current_value, min, max, description

// Futility pruning margins (centipawns)
pub const FUTILITY_MARGIN_1: f32 = 51.8; // min: 25, max: 100
pub const FUTILITY_MARGIN_2: f32 = 218.9; // min: 150, max: 350
pub const FUTILITY_MARGIN_3: f32 = 647.5; // min: 450, max: 800

// Reverse futility pruning margins (centipawns)
pub const REVERSE_FUTILITY_MARGIN_1: f32 = 141.4; // min: 75, max: 225
pub const REVERSE_FUTILITY_MARGIN_2: f32 = 116.1; // min: 50, max: 200
pub const REVERSE_FUTILITY_MARGIN_3: f32 = 350.3; // min: 200, max: 520

// Razoring margins (centipawns)
pub const RAZORING_MARGIN_1: f32 = 341.8; // min: 220, max: 460
pub const RAZORING_MARGIN_2: f32 = 465.9; // min: 320, max: 600
pub const RAZORING_MARGIN_3: f32 = 790.6; // min: 550, max: 950

// Null move pruning
pub const NULL_MOVE_REDUCTION: usize = 2; // min: 1, max: 4
pub const NULL_MOVE_DEPTH_THRESHOLD: usize = 6; // min: 4, max: 8
pub const NULL_MOVE_MIN_DEPTH: usize = 3; // min: 2, max: 5

// Late move reduction
pub const LMR_DIVISOR: f32 = 1.3; // min: 0.5, max: 2.5
pub const LMR_MIN_DEPTH: usize = 3; // min: 2, max: 5
pub const LMR_MOVE_INDEX: usize = 3; // min: 2, max: 6

// Extended futility
pub const EXT_FUTILITY_MULTIPLIER: f32 = 1.4; // min: 0.7, max: 1.8

// Delta pruning (quiescence) - centipawns
pub const DELTA_PRUNING_MARGIN: f32 = 75.3; // min: 40, max: 110

// Aspiration windows
pub const ASPIRATION_INITIAL: f32 = 54.4; // min: 30, max: 85
pub const ASPIRATION_MIN: f32 = 20.8; // min: 8, max: 30
pub const ASPIRATION_EXPAND: f32 = 2.7; // min: 1.8, max: 3.6

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
