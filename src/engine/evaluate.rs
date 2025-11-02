use crate::game::board::Game;
use crate::game::pieces::Piece;

pub const CHECKMATE_SCORE: f32 = 32767.0;

// Base piece values
pub const PAWN_VALUE: f32 = 100.0;
pub const KNIGHT_VALUE: f32 = 320.0;
pub const BISHOP_VALUE: f32 = 330.0;
pub const ROOK_VALUE: f32 = 500.0;
pub const QUEEN_VALUE: f32 = 900.0;

pub fn get_piece_value(piece: Piece) -> f32 {
    match piece {
        Piece::Pawn => PAWN_VALUE,
        Piece::Knight => KNIGHT_VALUE,
        Piece::Bishop => BISHOP_VALUE,
        Piece::Rook => ROOK_VALUE,
        Piece::Queen => QUEEN_VALUE,
        Piece::King => 0.0,
    }
}

pub fn get_see_piece_value(piece: Piece) -> f32 {
    match piece {
        Piece::Pawn => PAWN_VALUE,
        Piece::Knight => KNIGHT_VALUE,
        Piece::Bishop => BISHOP_VALUE,
        Piece::Rook => ROOK_VALUE,
        Piece::Queen => QUEEN_VALUE,
        Piece::King => CHECKMATE_SCORE,
    }
}

pub fn calculate_game_phase(game: &Game) -> f32 {
    const STARTING_MATERIAL: f32 =
        2.0 * QUEEN_VALUE + 4.0 * ROOK_VALUE + 4.0 * BISHOP_VALUE + 4.0 * KNIGHT_VALUE;

    let material = game.piece_bitboards[Piece::Pawn as usize].count_ones() as f32 * PAWN_VALUE
        + game.piece_bitboards[Piece::Knight as usize].count_ones() as f32 * KNIGHT_VALUE
        + game.piece_bitboards[Piece::Bishop as usize].count_ones() as f32 * BISHOP_VALUE
        + game.piece_bitboards[Piece::Rook as usize].count_ones() as f32 * ROOK_VALUE
        + game.piece_bitboards[Piece::Queen as usize].count_ones() as f32 * QUEEN_VALUE;

    let material_ratio = material / STARTING_MATERIAL;

    let phase = 1.0 - material_ratio;
    phase.clamp(0.0, 1.0)
}
