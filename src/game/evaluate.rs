use crate::game::board::Game;
use crate::game::pieces::Piece;

pub const CHECKMATE_SCORE: f32 = 100_000.0;

pub fn get_piece_value(piece: Piece) -> f32 {
    match piece {
        Piece::Rook => 500.0,
        Piece::Bishop => 300.0,
        Piece::Queen => 900.0,
        Piece::Knight => 300.0,
        Piece::Pawn => 100.0,
        Piece::King => 0.0,
    }
}

pub fn evaluate_material(game: &Game) -> f32 {
    let mut material = 0.0;

    // Sum up material for both sides
    for square_option in &game.pieces {
        if let Some((piece, color)) = square_option {
            material += get_piece_value(*piece) * *color;
        }
    }

    material as f32
}
