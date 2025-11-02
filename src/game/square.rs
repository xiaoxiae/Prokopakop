use crate::game::bitboard::Bitboard;

pub type BoardSquare = u8;

#[allow(dead_code)]
pub trait BoardSquareExt {
    fn get_x(&self) -> u8;
    fn get_y(&self) -> u8;
    fn parse(string: &str) -> Option<BoardSquare>;
    fn unparse(&self) -> String;
    fn from_position(x: u8, y: u8) -> BoardSquare;
    fn to_mask(&self) -> Bitboard;

    const A1: BoardSquare = 0;
    const A2: BoardSquare = 8;
    const A3: BoardSquare = 16;
    const A4: BoardSquare = 24;
    const A5: BoardSquare = 32;
    const A6: BoardSquare = 40;
    const A7: BoardSquare = 48;
    const A8: BoardSquare = 56;

    const B1: BoardSquare = 1;
    const B2: BoardSquare = 9;
    const B3: BoardSquare = 17;
    const B4: BoardSquare = 25;
    const B5: BoardSquare = 33;
    const B6: BoardSquare = 41;
    const B7: BoardSquare = 49;
    const B8: BoardSquare = 57;

    const C1: BoardSquare = 2;
    const C2: BoardSquare = 10;
    const C3: BoardSquare = 18;
    const C4: BoardSquare = 26;
    const C5: BoardSquare = 34;
    const C6: BoardSquare = 42;
    const C7: BoardSquare = 50;
    const C8: BoardSquare = 58;

    const D1: BoardSquare = 3;
    const D2: BoardSquare = 11;
    const D3: BoardSquare = 19;
    const D4: BoardSquare = 27;
    const D5: BoardSquare = 35;
    const D6: BoardSquare = 43;
    const D7: BoardSquare = 51;
    const D8: BoardSquare = 59;

    const E1: BoardSquare = 4;
    const E2: BoardSquare = 12;
    const E3: BoardSquare = 20;
    const E4: BoardSquare = 28;
    const E5: BoardSquare = 36;
    const E6: BoardSquare = 44;
    const E7: BoardSquare = 52;
    const E8: BoardSquare = 60;

    const F1: BoardSquare = 5;
    const F2: BoardSquare = 13;
    const F3: BoardSquare = 21;
    const F4: BoardSquare = 29;
    const F5: BoardSquare = 37;
    const F6: BoardSquare = 45;
    const F7: BoardSquare = 53;
    const F8: BoardSquare = 61;

    const G1: BoardSquare = 6;
    const G2: BoardSquare = 14;
    const G3: BoardSquare = 22;
    const G4: BoardSquare = 30;
    const G5: BoardSquare = 38;
    const G6: BoardSquare = 46;
    const G7: BoardSquare = 54;
    const G8: BoardSquare = 62;

    const H1: BoardSquare = 7;
    const H2: BoardSquare = 15;
    const H3: BoardSquare = 23;
    const H4: BoardSquare = 31;
    const H5: BoardSquare = 39;
    const H6: BoardSquare = 47;
    const H7: BoardSquare = 55;
    const H8: BoardSquare = 63;
}

impl BoardSquareExt for u8 {
    fn get_x(&self) -> u8 {
        self % 8
    }

    fn get_y(&self) -> u8 {
        self / 8
    }

    fn parse(string: &str) -> Option<BoardSquare> {
        let mut chars = string.chars();

        match (chars.next(), chars.next()) {
            (Some(file), Some(rank)) if file.is_alphabetic() && rank.is_numeric() => Some(
                BoardSquare::from_position(file as u8 - b'a' as u8, rank as u8 - b'1' as u8),
            ),
            (_, _) => None,
        }
    }

    fn unparse(&self) -> String {
        format!(
            "{}{}",
            (self.get_x() + b'a' as u8) as char,
            (self.get_y() + b'1' as u8) as char
        )
    }

    fn from_position(x: u8, y: u8) -> BoardSquare {
        x + y * 8
    }

    fn to_mask(&self) -> Bitboard {
        1 << self
    }
}
