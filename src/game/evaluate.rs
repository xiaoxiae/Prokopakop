use crate::game::board::Game;
use crate::game::pieces::{Color, Piece};

pub const CHECKMATE_SCORE: f32 = 100_000_000.0;

pub const PAWN_VALUE: f32 = 100.0;
pub const KNIGHT_VALUE: f32 = 320.0;
pub const BISHOP_VALUE: f32 = 330.0;
pub const ROOK_VALUE: f32 = 500.0;
pub const QUEEN_VALUE: f32 = 900.0;
pub const KING_VALUE: f32 = 0.0;

pub const PAWN_POSITION_MULTIPLIER: f32 = 50.0;
pub const KNIGHT_POSITION_MULTIPLIER: f32 = 40.0;
pub const BISHOP_POSITION_MULTIPLIER: f32 = 20.0;
pub const ROOK_POSITION_MULTIPLIER: f32 = 30.0;
pub const QUEEN_POSITION_MULTIPLIER: f32 = 20.0;
pub const KING_EARLY_POSITION_MULTIPLIER: f32 = 40.0;
pub const KING_LATE_POSITION_MULTIPLIER: f32 = 50.0;

pub fn get_piece_value(piece: Piece) -> f32 {
    match piece {
        Piece::Pawn => PAWN_VALUE,
        Piece::Knight => KNIGHT_VALUE,
        Piece::Bishop => BISHOP_VALUE,
        Piece::Rook => ROOK_VALUE,
        Piece::Queen => QUEEN_VALUE,
        Piece::King => KING_VALUE,
    }
}

#[rustfmt::skip]
const PAWN_TABLE: [f32; 64] = [
    0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0, 0.0,
    1.0,  1.0,  1.0,  1.0,  1.0,  1.0,  1.0, 1.0,
    0.2,  0.2,  0.4,  0.6,  0.6,  0.4,  0.2, 0.2,
    0.1,  0.1,  0.2,  0.5,  0.5,  0.2,  0.1, 0.1,
    0.0,  0.0,  0.0,  0.4,  0.4,  0.0,  0.0, 0.0,
    0.1, -0.1, -0.2,  0.0,  0.0, -0.2, -0.1, 0.1,
    0.1,  0.2,  0.2, -0.4, -0.4,  0.2,  0.2, 0.1,
    0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0, 0.0,
];

#[rustfmt::skip]
const KNIGHT_TABLE: [f32; 64] = [
    0.0, 0.1,  0.2,  0.2,  0.2,  0.2,  0.1,  0.0,
    0.1, 0.3,  0.5,  0.5,  0.5,  0.5,  0.3,  0.1,
    0.2, 0.5,  0.6,  0.65, 0.65, 0.6,  0.5,  0.2,
    0.2, 0.55, 0.65, 0.7,  0.7,  0.65, 0.55, 0.2,
    0.2, 0.5,  0.65, 0.7,  0.7,  0.65, 0.5,  0.2,
    0.2, 0.55, 0.6,  0.65, 0.65, 0.6,  0.55, 0.2,
    0.1, 0.3,  0.5,  0.55, 0.55, 0.5,  0.3,  0.1,
    0.0, 0.1,  0.2,  0.2,  0.2,  0.2,  0.1,  0.0,
];

#[rustfmt::skip]
const BISHOP_TABLE: [f32; 64] = [
    0.2, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.2,
    0.1, 0.3, 0.3, 0.3, 0.3, 0.3, 0.3, 0.1,
    0.1, 0.3, 0.4, 0.5, 0.5, 0.4, 0.3, 0.1,
    0.1, 0.3, 0.5, 0.6, 0.6, 0.5, 0.3, 0.1,
    0.1, 0.3, 0.5, 0.6, 0.6, 0.5, 0.3, 0.1,
    0.1, 0.4, 0.4, 0.5, 0.5, 0.4, 0.4, 0.1,
    0.1, 0.5, 0.3, 0.3, 0.3, 0.3, 0.5, 0.1,
    0.2, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.2,
];

#[rustfmt::skip]
const ROOK_TABLE: [f32; 64] = [
    0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5,
    0.5, 0.6, 0.6, 0.6, 0.6, 0.6, 0.6, 0.5,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
    0.5, 0.5, 0.5, 0.6, 0.6, 0.5, 0.5, 0.5,
];

#[rustfmt::skip]
const QUEEN_TABLE: [f32; 64] = [
    0.2,  0.1, 0.1, 0.05, 0.05, 0.1, 0.1, 0.2,
    0.1,  0.0, 0.0, 0.0,  0.0,  0.0, 0.0, 0.1,
    0.1,  0.0, 0.5, 0.5,  0.5,  0.5, 0.0, 0.1,
    0.05, 0.0, 0.5, 0.5,  0.5,  0.5, 0.0, 0.05,
    0.0,  0.0, 0.5, 0.5,  0.5,  0.5, 0.0, 0.05,
    0.1,  0.5, 0.5, 0.5,  0.5,  0.5, 0.0, 0.1,
    0.1,  0.0, 0.5, 0.0,  0.0,  0.0, 0.0, 0.1,
    0.2,  0.1, 0.1, 0.05, 0.05, 0.1, 0.1, 0.2,
];

#[rustfmt::skip]
const KING_EARLY_TABLE: [f32; 64] = [
    0.2, 0.1, 0.1, 0.0, 0.0, 0.1, 0.1, 0.2,
    0.2, 0.1, 0.1, 0.0, 0.0, 0.1, 0.1, 0.2,
    0.2, 0.1, 0.1, 0.0, 0.0, 0.1, 0.1, 0.2,
    0.2, 0.1, 0.1, 0.0, 0.0, 0.1, 0.1, 0.2,
    0.3, 0.2, 0.2, 0.1, 0.1, 0.2, 0.2, 0.3,
    0.4, 0.3, 0.3, 0.3, 0.3, 0.3, 0.3, 0.4,
    0.7, 0.7, 0.5, 0.5, 0.5, 0.5, 0.7, 0.7,
    0.7, 0.8, 0.6, 0.5, 0.5, 0.6, 0.8, 0.7, // castled positions favored
];

#[rustfmt::skip]
const KING_LATE_TABLE: [f32; 64] = [
    0.0, 0.1, 0.2, 0.3, 0.3, 0.2, 0.1, 0.0,
    0.2, 0.3, 0.4, 0.5, 0.5, 0.4, 0.3, 0.2,
    0.2, 0.4, 0.7, 0.8, 0.8, 0.7, 0.4, 0.2,
    0.2, 0.4, 0.8, 0.9, 0.9, 0.8, 0.4, 0.2,
    0.2, 0.4, 0.8, 0.9, 0.9, 0.8, 0.4, 0.2,
    0.2, 0.4, 0.7, 0.8, 0.8, 0.7, 0.4, 0.2,
    0.2, 0.2, 0.5, 0.5, 0.5, 0.5, 0.2, 0.2,
    0.0, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.0,
];

// Helper function to get position table value for a piece at a given square
fn get_position_value(piece: Piece, square: usize, color: Color, game_phase: f32) -> f32 {
    let adjusted_square = match color {
        Color::Black => square,
        Color::White => 63 - square,
    };

    let normalized_value = match piece {
        Piece::Pawn => PAWN_TABLE[adjusted_square],
        Piece::Knight => KNIGHT_TABLE[adjusted_square],
        Piece::Bishop => BISHOP_TABLE[adjusted_square],
        Piece::Rook => ROOK_TABLE[adjusted_square],
        Piece::Queen => QUEEN_TABLE[adjusted_square],
        Piece::King => {
            // Interpolate between early and late game king tables based on game phase
            let early_value = KING_EARLY_TABLE[adjusted_square];
            let late_value = KING_LATE_TABLE[adjusted_square];
            early_value * (1.0 - game_phase) + late_value * game_phase
        }
    };

    // Apply the appropriate multiplier
    let multiplier = match piece {
        Piece::Pawn => PAWN_POSITION_MULTIPLIER,
        Piece::Knight => KNIGHT_POSITION_MULTIPLIER,
        Piece::Bishop => BISHOP_POSITION_MULTIPLIER,
        Piece::Rook => ROOK_POSITION_MULTIPLIER,
        Piece::Queen => QUEEN_POSITION_MULTIPLIER,
        Piece::King => {
            // Interpolate king multiplier based on game phase
            KING_EARLY_POSITION_MULTIPLIER * (1.0 - game_phase)
                + KING_LATE_POSITION_MULTIPLIER * game_phase
        }
    };

    normalized_value * multiplier
}

// Calculate game phase (0.0 = early game, 1.0 = late game)
// Based on remaining material on the board
pub fn calculate_game_phase(total_material: f32) -> f32 {
    let max_material =
        2.0 * QUEEN_VALUE + 4.0 * ROOK_VALUE + 4.0 * BISHOP_VALUE + 4.0 * KNIGHT_VALUE;

    let phase = 1.0 - (total_material / max_material).min(1.0);
    phase.max(0.0)
}

pub fn evaluate_material(game: &Game) -> (f32, f32) {
    let mut white = 0.0;
    let mut black = 0.0;

    // Sum up material for both sides
    for square_option in &game.pieces {
        if let Some((piece, color)) = square_option {
            match color {
                Color::White => white += get_piece_value(*piece),
                Color::Black => black += get_piece_value(*piece),
            }
        }
    }

    (black, white)
}

pub fn evaluate_positional(game: &Game, game_phase: f32) -> f32 {
    let mut positional = 0.0;

    // Sum up positional values for both sides
    for (square, square_option) in game.pieces.iter().enumerate() {
        if let Some((piece, color)) = square_option {
            let position_value = get_position_value(*piece, square, *color, game_phase);
            positional += position_value * *color;
        }
    }

    positional
}
