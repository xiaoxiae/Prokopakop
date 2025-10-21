use strum::EnumCount;

use crate::game::board::{BoardMove, BoardMoveExt, Game};
use crate::game::pieces::{Color, Piece};
use crate::utils::bitboard::{Bitboard, BitboardExt, PIECE_MOVE_BITBOARDS};
use crate::utils::square::{BoardSquare, BoardSquareExt};

// Do not increase this! We're using it to count moves to checkmate
// by incrementing via PLY and subtract later. Incrementing this too much
// will make it loose precision and the PLY info.
pub const CHECKMATE_SCORE: f32 = 32767.0;

// Base piece values
pub const PAWN_VALUE: f32 = 100.0;
pub const KNIGHT_VALUE: f32 = 320.0;
pub const BISHOP_VALUE: f32 = 330.0;
pub const ROOK_VALUE: f32 = 500.0;
pub const QUEEN_VALUE: f32 = 900.0;
pub const KING_VALUE: f32 = 0.0;

// Matches order in Pieces so we can quickly calculate game phase
pub const MINOR_MAJOR_PIECE_VALUES: [f32; Piece::COUNT + 1] = [
    0.0,
    ROOK_VALUE,
    BISHOP_VALUE,
    QUEEN_VALUE,
    KNIGHT_VALUE,
    0.0,
    0.0,
];

// Matches order in Pieces so we can quickly calculate game phase
pub const PIECE_VALUES: [f32; Piece::COUNT + 1] = [
    0.0,
    ROOK_VALUE,
    BISHOP_VALUE,
    QUEEN_VALUE,
    KNIGHT_VALUE,
    PAWN_VALUE,
    10000.0,
];

// Multipliers for the piece tables
// (since they're stored normalized)
pub const PAWN_POSITION_MULTIPLIER: f32 = 50.0;
pub const KNIGHT_POSITION_MULTIPLIER: f32 = 40.0;
pub const BISHOP_POSITION_MULTIPLIER: f32 = 20.0;
pub const ROOK_POSITION_MULTIPLIER: f32 = 30.0;
pub const QUEEN_POSITION_MULTIPLIER: f32 = 20.0;
pub const KING_EARLY_POSITION_MULTIPLIER: f32 = 40.0;
pub const KING_LATE_POSITION_MULTIPLIER: f32 = 50.0;

// Pawn structure bonuses/penalties
pub const PASSED_PAWN_BONUS: [f32; 8] = [
    0.0,   // Rank 1
    5.0,   // Rank 2
    10.0,  // Rank 3
    20.0,  // Rank 4
    35.0,  // Rank 5
    60.0,  // Rank 6
    100.0, // Rank 7
    0.0,   // Rank 8 (promoted)
];

pub const PASSED_PAWN_LATE_MULTIPLIER: f32 = 0.5;

pub const ISOLATED_PAWN_PENALTY: f32 = -15.0;
pub const DOUBLED_PAWN_PENALTY: f32 = -10.0;

pub const MOBILITY_MULTIPLIER: f32 = 0.5;
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

const BISHOP_PAIR_BASE_BONUS: f32 = 30.0;
const BISHOP_PAIR_LATE_MULTIPLIER: f32 = 0.5;

// Prefer center positions + pushes
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

// Prefer center positions
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

// Prefer mostly center positions / corners
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
    0.1,  0.0, 0.5, 0.5,  0.5,  0.5, 0.0, 0.1,
    0.1,  0.0, 0.0, 0.0,  0.0,  0.0, 0.0, 0.1,
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
    0.7, 0.7, 1.0, 0.5, 0.5, 0.6, 1.0, 0.7,
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

pub const KING_SAFETY_MULTIPLIER: f32 = 1.0;

pub const MISSING_PAWN_SHIELD_PENALTY: f32 = -20.0;
pub const MISSING_KING_FILE_PAWN_PENALTY: f32 = -10.0;

pub const OPEN_FILE_NEAR_KING_PENALTY: f32 = -25.0;
pub const SEMI_OPEN_FILE_NEAR_KING_PENALTY: f32 = -15.0;
pub const OPEN_FILE_WITH_ROOK_PENALTY: f32 = -15.0;

const PAWN_ATTACK_WEIGHT: f32 = 1.0;
const KNIGHT_ATTACK_WEIGHT: f32 = 2.0;
const BISHOP_ATTACK_WEIGHT: f32 = 2.0;
const ROOK_ATTACK_WEIGHT: f32 = 3.0;
const QUEEN_ATTACK_WEIGHT: f32 = 5.0;

const MULTI_ATTACK_WEIGHT: f32 = 20.0;
const SQUARE_ATTACK_FACTOR: f32 = 1.0 / 50.0;

fn get_positional_piece_value(piece: Piece, square: usize, color: Color, game_phase: f32) -> f32 {
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

///
/// Calculate game phase (0.0 = early game, 1.0 = late game) based on material
///
pub fn calculate_game_phase(game: &Game) -> f32 {
    const STARTING_MATERIAL: f32 =
        2.0 * QUEEN_VALUE + 4.0 * ROOK_VALUE + 4.0 * BISHOP_VALUE + 4.0 * KNIGHT_VALUE;

    let material_ratio = game.non_pawn_remaining_material / STARTING_MATERIAL;

    let phase = 1.0 - material_ratio;
    phase.clamp(0.0, 1.0)
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

    const FILE_A: Bitboard = Bitboard::FILES[0];
    const FILE_H: Bitboard = Bitboard::FILES[7];

    let center_file = FILE_A << file;

    let left_file = (center_file >> 1) & !FILE_H;
    let right_file = (center_file << 1) & !FILE_A;

    let files_mask = center_file | left_file | right_file;

    let rank_mask = match color {
        Color::White => Bitboard::ONES << ((rank + 1) * 8),
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
        let file_mask = Bitboard::FILES[file];
        let pawns_on_file = (pawns & file_mask).count_ones();
        if pawns_on_file > 1 {
            doubled += pawns_on_file - 1;
        }
    }

    doubled
}

fn is_isolated_pawn(pawn_square: u8, friendly_pawns: Bitboard) -> bool {
    let file = pawn_square.get_x();

    const FILE_A: u64 = Bitboard::FILES[0];
    const FILE_H: u64 = Bitboard::FILES[7];

    let center_file = FILE_A << file;

    let left_file = (center_file >> 1) & !FILE_H;
    let right_file = (center_file << 1) & !FILE_A;

    let adjacent_files_mask = left_file | right_file;

    (adjacent_files_mask & friendly_pawns) == 0
}

// Update the evaluate_pawn_structure function to include isolated pawns
pub fn evaluate_pawn_structure(game: &Game, game_phase: f32) -> f32 {
    let mut eval = 0.0;

    let white_pawns =
        game.piece_bitboards[Piece::Pawn as usize] & game.color_bitboards[Color::White as usize];
    let black_pawns =
        game.piece_bitboards[Piece::Pawn as usize] & game.color_bitboards[Color::Black as usize];

    for square in white_pawns.iter_positions() {
        if is_passed_pawn(square, Color::White, black_pawns) {
            let rank = square.get_y() as usize;
            let bonus = PASSED_PAWN_BONUS[rank] * (1.0 + game_phase * PASSED_PAWN_LATE_MULTIPLIER);
            eval += bonus;
        }

        if is_isolated_pawn(square, white_pawns) {
            eval += ISOLATED_PAWN_PENALTY;
        }
    }

    for square in black_pawns.iter_positions() {
        if is_passed_pawn(square, Color::Black, white_pawns) {
            let rank = 7 - square.get_y() as usize;
            let bonus = PASSED_PAWN_BONUS[rank] * (1.0 + game_phase * PASSED_PAWN_LATE_MULTIPLIER);
            eval -= bonus;
        }

        if is_isolated_pawn(square, black_pawns) {
            eval -= ISOLATED_PAWN_PENALTY;
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
                get_positional_piece_value(piece_type, square as usize, Color::White, game_phase);
            positional += position_value;
        }

        let black_pieces =
            game.piece_bitboards[piece] & game.color_bitboards[Color::Black as usize];
        for square in black_pieces.iter_positions() {
            let position_value =
                get_positional_piece_value(piece_type, square as usize, Color::Black, game_phase);
            positional -= position_value;
        }
    }

    // Pawn structure evaluation
    positional += evaluate_pawn_structure(game, game_phase);

    positional
}

pub fn evaluate_mobility(
    game: &Game,
    game_phase: f32,
    white_moves: &[BoardMove],
    black_moves: &[BoardMove],
) -> f32 {
    let mobility_score;

    let white_weighted = calculate_weighted_mobility(&game, &white_moves);
    let black_weighted = calculate_weighted_mobility(&game, &black_moves);

    mobility_score = (white_weighted - black_weighted) * (1.0 + game_phase * MOBILITY_MULTIPLIER);

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

pub fn evaluate_bishop_pair(game: &Game, game_phase: f32) -> f32 {
    let mut eval = 0.0;

    let white_bishops =
        game.piece_bitboards[Piece::Bishop as usize] & game.color_bitboards[Color::White as usize];
    let black_bishops =
        game.piece_bitboards[Piece::Bishop as usize] & game.color_bitboards[Color::Black as usize];

    let bishop_pair_bonus =
        BISHOP_PAIR_BASE_BONUS * (1.0 + game_phase * BISHOP_PAIR_LATE_MULTIPLIER);

    if white_bishops.count_ones() >= 2 {
        eval += bishop_pair_bonus;
    }

    if black_bishops.count_ones() >= 2 {
        eval -= bishop_pair_bonus;
    }

    eval
}

fn evaluate_king_pawn_shield(game: &Game, color: Color, king_square: BoardSquare) -> f32 {
    let friendly_pawns =
        game.piece_bitboards[Piece::Pawn as usize] & game.color_bitboards[color as usize];

    let file = king_square.get_x();

    let mut shield_penalty = 0.0;

    // Check pawns on king's file and adjacent files
    for f in file.saturating_sub(1)..=(file + 1).min(7) {
        let file_mask = Bitboard::FILES[f as usize];
        let has_pawn = (friendly_pawns & file_mask) != 0;

        if !has_pawn {
            // Missing pawn in shield
            shield_penalty += MISSING_PAWN_SHIELD_PENALTY;

            // Extra penalty if it's the king's own file
            if f == file {
                shield_penalty += MISSING_KING_FILE_PAWN_PENALTY;
            }
        }
    }

    shield_penalty
}

fn evaluate_open_files_near_king(game: &Game, color: Color, king_square: BoardSquare) -> f32 {
    let file = king_square.get_x();

    let all_pawns = game.piece_bitboards[Piece::Pawn as usize];
    let friendly_pawns = all_pawns & game.color_bitboards[color as usize];

    let enemy_rooks_queens = (game.piece_bitboards[Piece::Rook as usize]
        | game.piece_bitboards[Piece::Queen as usize])
        & game.color_bitboards[!color as usize];

    let mut open_file_penalty = 0.0;

    for f in file.saturating_sub(1)..=(file + 1).min(7) {
        let file_mask = Bitboard::FILES[f as usize];

        let friendly_on_file = (friendly_pawns & file_mask) != 0;
        let enemy_on_file = ((all_pawns & !friendly_pawns) & file_mask) != 0;
        let enemy_heavy_on_file = (enemy_rooks_queens & file_mask) != 0;

        if !friendly_on_file && !enemy_on_file {
            // Fully open file
            open_file_penalty += OPEN_FILE_NEAR_KING_PENALTY;

            if enemy_heavy_on_file {
                open_file_penalty += OPEN_FILE_WITH_ROOK_PENALTY;
            }
        } else if !friendly_on_file && enemy_on_file {
            // Semi-open file (for us)
            open_file_penalty += SEMI_OPEN_FILE_NEAR_KING_PENALTY;
        }
    }

    open_file_penalty
}

///
/// Return the safety score for the king of the given color.
/// The **higher** the value, the more safe it is.
///
pub fn calculate_king_safety(game: &Game, color: Color, opponent_moves: &[BoardMove]) -> f32 {
    let king_square = game.get_king_position(color);

    let mut safety = 0.0;

    safety += evaluate_king_pawn_shield(game, color, king_square);
    safety += evaluate_open_files_near_king(game, color, king_square);
    safety += evaluate_king_zone_attacks(game, color, king_square, opponent_moves);

    safety
}

fn get_king_zone(king_square: BoardSquare, color: Color) -> Bitboard {
    const FILE_A: Bitboard = Bitboard::FILES[0];
    const FILE_H: Bitboard = Bitboard::FILES[7];

    let king_bb = king_square.to_mask();

    let adjacent = PIECE_MOVE_BITBOARDS[Piece::King as usize][king_square as usize];

    let forward_1 = match color {
        Color::White => king_bb << 8,
        Color::Black => king_bb >> 8,
    };

    let forward_2 = match color {
        Color::White => king_bb << 16,
        Color::Black => king_bb >> 16,
    };

    let forward_zone = (forward_1 | forward_2)
        | ((forward_1 | forward_2) << 1) & !FILE_A
        | ((forward_1 | forward_2) >> 1) & !FILE_H;

    adjacent | forward_zone
}

struct KingAttackInfo {
    attack_weight: f32,
    attacker_count: u32,
}

fn count_king_zone_attacks(
    game: &Game,
    moves: &[BoardMove],
    king_zone: Bitboard,
) -> KingAttackInfo {
    let mut attack_weight = 0.0;
    let mut attacking_pieces = Bitboard::default();

    for mv in moves {
        let mask = mv.get_to().to_mask();

        // Check if this move attacks the king zone
        if (mask & king_zone) != 0 {
            let from_square = mv.get_from();
            let piece = game.pieces[from_square as usize].unwrap().0;

            // Track unique attackers using bitboard
            attacking_pieces |= from_square.to_mask();

            // Weight by piece type
            let weight = match piece {
                Piece::Pawn => PAWN_ATTACK_WEIGHT,
                Piece::Knight => KNIGHT_ATTACK_WEIGHT,
                Piece::Bishop => BISHOP_ATTACK_WEIGHT,
                Piece::Rook => ROOK_ATTACK_WEIGHT,
                Piece::Queen => QUEEN_ATTACK_WEIGHT,
                Piece::King => 0.0,
            };

            attack_weight += weight;
        }
    }

    KingAttackInfo {
        attack_weight,
        attacker_count: attacking_pieces.count_ones(),
    }
}

fn evaluate_king_zone_attacks(
    game: &Game,
    color: Color,
    king_square: BoardSquare,
    moves: &[BoardMove],
) -> f32 {
    let zone = get_king_zone(king_square, color);
    let attack_info = count_king_zone_attacks(game, moves, zone);

    let mut danger = attack_info.attack_weight;

    // Bonus for multiple attackers
    if attack_info.attacker_count >= 2 {
        danger += (attack_info.attacker_count - 1) as f32 * MULTI_ATTACK_WEIGHT;
    }

    // Quadratic scaling (with a small factor so it's not too crazy)
    -(danger * danger) * SQUARE_ATTACK_FACTOR
}

pub fn evaluate_king_safety(
    game: &Game,
    game_phase: f32,
    white_moves: &[BoardMove],
    black_moves: &[BoardMove],
) -> f32 {
    // Stop caring about king safety in endgame
    if game_phase > 0.7 {
        return 0.0;
    }

    // Scale by game phase (more important in middlegame)
    let phase_multiplier = (1.0 - game_phase) * KING_SAFETY_MULTIPLIER;

    let white_safety = calculate_king_safety(game, Color::White, black_moves);
    let black_safety = calculate_king_safety(game, Color::Black, white_moves);

    (white_safety - black_safety) * phase_multiplier
}
