use strum::EnumCount;

use crate::game::board::{BoardMove, BoardMoveExt, Game};
use crate::game::pieces::{Color, Piece};
use crate::utils::bitboard::{Bitboard, BitboardExt};
use crate::utils::square::BoardSquareExt;

// Do not increase this! We're using it to count moves to checkmate
// by incrementing via PLY and subtract later. Incrementing this too much
// will make it loose precision and the PLY info.
pub const CHECKMATE_SCORE: f32 = 100_000.0;

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

// Pawn structure bonuses/penalties
pub const PASSED_PAWN_BONUS: [f32; 8] = [
    0.0,   // Rank 1 (no pawn should be here)
    5.0,   // Rank 2
    10.0,  // Rank 3
    20.0,  // Rank 4
    35.0,  // Rank 5
    60.0,  // Rank 6
    100.0, // Rank 7
    0.0,   // Rank 8 (promoted)
];

pub const DOUBLED_PAWN_PENALTY: f32 = -10.0;

pub const MOBILITY_MULTIPLIER: f32 = 2.0;
pub const PAWN_MOBILITY_WEIGHT: f32 = 0.5;
pub const KNIGHT_MOBILITY_WEIGHT: f32 = 4.0;
pub const BISHOP_MOBILITY_WEIGHT: f32 = 3.0;
pub const ROOK_MOBILITY_WEIGHT: f32 = 2.0;
pub const QUEEN_MOBILITY_WEIGHT: f32 = 1.0;

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

// Calculate game phase (0.0 = early game, 1.0 = late game) based on material
pub fn calculate_game_phase(total_material: f32) -> f32 {
    let max_material =
        2.0 * QUEEN_VALUE + 4.0 * ROOK_VALUE + 4.0 * BISHOP_VALUE + 4.0 * KNIGHT_VALUE;

    let phase = 1.0 - (total_material / max_material).min(1.0);
    phase.max(0.0)
}

pub fn evaluate_material(game: &Game) -> (f32, f32) {
    let mut white = 0.0;
    let mut black = 0.0;

    // Use bitboard operations instead of for-loop
    for piece in 0..Piece::COUNT {
        let piece_type = Piece::from_repr(piece).unwrap();
        let piece_value = get_piece_value(piece_type);

        // Count white pieces of this type
        let white_pieces =
            game.piece_bitboards[piece] & game.color_bitboards[Color::White as usize];
        white += white_pieces.count_ones() as f32 * piece_value;

        // Count black pieces of this type
        let black_pieces =
            game.piece_bitboards[piece] & game.color_bitboards[Color::Black as usize];
        black += black_pieces.count_ones() as f32 * piece_value;
    }

    (white, black)
}

fn is_passed_pawn(pawn_square: u8, color: Color, enemy_pawns: Bitboard) -> bool {
    let file = pawn_square.get_x();
    let rank = pawn_square.get_y();

    const FILE_A: u64 = 0x0101010101010101;
    const FILE_H: u64 = 0x8080808080808080;

    let center_file = FILE_A << file;

    let left_file = (center_file >> 1) & !FILE_H;
    let right_file = (center_file << 1) & !FILE_A;

    let files_mask = center_file | left_file | right_file;

    let rank_mask = match color {
        Color::White => 0xFFFFFFFFFFFFFFFFu64 << ((rank + 1) * 8),
        Color::Black => {
            if rank > 0 {
                (1u64 << (rank * 8)) - 1
            } else {
                0
            }
        }
    };

    let passed_mask = files_mask & rank_mask;

    (passed_mask & enemy_pawns) == 0
}

fn count_doubled_pawns(pawns: Bitboard) -> u32 {
    let mut doubled = 0;

    for file in 0..8 {
        let file_mask = 0x0101010101010101u64 << file;
        let pawns_on_file = (pawns & file_mask).count_ones();
        if pawns_on_file > 1 {
            doubled += pawns_on_file - 1; // Count extra pawns as doubled
        }
    }

    doubled
}

pub fn evaluate_pawn_structure(game: &Game, game_phase: f32) -> f32 {
    let mut eval = 0.0;

    let white_pawns =
        game.piece_bitboards[Piece::Pawn as usize] & game.color_bitboards[Color::White as usize];
    let black_pawns =
        game.piece_bitboards[Piece::Pawn as usize] & game.color_bitboards[Color::Black as usize];

    for square in white_pawns.iter_positions() {
        if is_passed_pawn(square, Color::White, black_pawns) {
            let rank = (square / 8) as usize;
            let bonus = PASSED_PAWN_BONUS[rank] * (1.0 + game_phase * 0.5);
            eval += bonus;
        }
    }

    for square in black_pawns.iter_positions() {
        if is_passed_pawn(square, Color::Black, white_pawns) {
            let rank = 7 - (square / 8) as usize;
            let bonus = PASSED_PAWN_BONUS[rank] * (1.0 + game_phase * 0.5);
            eval -= bonus;
        }
    }

    // Evaluate doubled pawns
    let white_doubled = count_doubled_pawns(white_pawns);
    let black_doubled = count_doubled_pawns(black_pawns);

    eval += white_doubled as f32 * DOUBLED_PAWN_PENALTY;
    eval -= black_doubled as f32 * DOUBLED_PAWN_PENALTY;

    eval
}

pub fn evaluate_positional(game: &Game, game_phase: f32) -> f32 {
    let mut positional = 0.0;

    for piece in 0..Piece::COUNT {
        let piece_type = Piece::from_repr(piece).unwrap();

        let white_pieces =
            game.piece_bitboards[piece] & game.color_bitboards[Color::White as usize];
        for square in white_pieces.iter_positions() {
            let position_value =
                get_position_value(piece_type, square as usize, Color::White, game_phase);
            positional += position_value;
        }

        let black_pieces =
            game.piece_bitboards[piece] & game.color_bitboards[Color::Black as usize];
        for square in black_pieces.iter_positions() {
            let position_value =
                get_position_value(piece_type, square as usize, Color::Black, game_phase);
            positional -= position_value; // Subtract for black pieces
        }
    }

    // Pawn structure evaluation
    positional += evaluate_pawn_structure(game, game_phase);

    positional
}

pub fn evaluate_mobility(game: &Game, game_phase: f32) -> f32 {
    let mobility_score;

    let (white_move_count, white_moves) = game.get_side_moves(Color::White);
    let (black_move_count, black_moves) = game.get_side_moves(Color::Black);

    let white_weighted = calculate_weighted_mobility(&game, &white_moves[..white_move_count]);
    let black_weighted = calculate_weighted_mobility(&game, &black_moves[..black_move_count]);

    mobility_score = (white_weighted - black_weighted) * (1.0 + game_phase * 0.5);

    mobility_score
}

fn calculate_weighted_mobility(game: &Game, moves: &[BoardMove]) -> f32 {
    let mut weighted_mobility = 0.0;

    for mv in moves {
        let piece = game.pieces[mv.get_from() as usize].unwrap().0;

        weighted_mobility += match piece {
            Piece::Pawn => PAWN_MOBILITY_WEIGHT,
            Piece::Knight => KNIGHT_MOBILITY_WEIGHT,
            Piece::Bishop => BISHOP_MOBILITY_WEIGHT,
            Piece::Rook => ROOK_MOBILITY_WEIGHT,
            Piece::Queen => QUEEN_MOBILITY_WEIGHT,
            Piece::King => 0.0,
        };
    }

    weighted_mobility
}
