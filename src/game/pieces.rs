use std::ops::Not;
use strum_macros::{EnumCount, EnumIter, FromRepr};

#[derive(Copy, Clone, Debug, PartialEq, EnumIter, EnumCount, FromRepr)]
pub enum Piece {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

#[derive(Copy, Clone, Debug, PartialEq, EnumIter, EnumCount, FromRepr)]
pub enum Color {
    Black = 0,
    White = 1,
}

impl Not for Color {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

impl Piece {
    pub fn from_char(c: char) -> Option<Piece> {
        match c {
            'p' => Some(Piece::Pawn),
            'n' => Some(Piece::Knight),
            'b' => Some(Piece::Bishop),
            'r' => Some(Piece::Rook),
            'q' => Some(Piece::Queen),
            'k' => Some(Piece::King),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Piece::Pawn   => 'p',
            Piece::Knight => 'n',
            Piece::Bishop => 'b',
            Piece::Rook   => 'r',
            Piece::Queen  => 'q',
            Piece::King   => 'k',
        }
    }

    pub fn to_emoji(&self) -> char {
        // We change the color via Ansi codes
        match self {
            Piece::Pawn => '♟',
            Piece::Knight => '♞',
            Piece::Bishop => '♝',
            Piece::Rook => '♜',
            Piece::Queen => '♛',
            Piece::King => '♚',
        }
    }
}
