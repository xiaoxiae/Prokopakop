use super::pieces::{Color, Piece};
use crate::utils::Bitboard;
use crate::{
    BISHOP_BLOCKER_BITBOARD, BitboardExt, MAGIC_ENTRIES, MAGIC_TABLE, PAWN_ATTACK_MOVE_BITBOARD,
    PAWN_FIRST_MOVE_BITBOARD, ROOK_BLOCKER_BITBOARD, VALID_MOVE_BITBOARDS, position_to_bitmask,
};
use std::cmp::PartialEq;
use strum::{EnumCount, IntoEnumIterator};

#[derive(Debug, Eq, PartialEq)]
pub struct BoardSquare {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug)]
pub struct BoardMove {
    pub from: BoardSquare,
    pub to: BoardSquare,
    pub promotion: Option<Piece>,
}

impl BoardSquare {
    pub fn parse(string: &str) -> Option<BoardSquare> {
        let mut chars = string.chars();

        match (chars.next(), chars.next()) {
            (Some(file), Some(rank)) if file.is_alphabetic() && rank.is_numeric() => {
                Some(BoardSquare {
                    x: file as u32 - b'a' as u32,
                    y: rank as u32 - b'1' as u32,
                })
            }
            (_, _) => None,
        }
    }

    pub fn unparse(&self) -> String {
        format!("{}{}", self.x as u8 + b'a' as u8, self.y as u8 + b'1' as u8)
    }

    pub fn to_mask(&self) -> Bitboard {
        Bitboard::position_to_bitmask(self.x, self.y)
    }

    pub fn to_index(&self) -> usize {
        self.x as usize + self.y as usize * 8
    }
}

impl BoardMove {
    pub fn parse(string: &str) -> Option<BoardMove> {
        let from = string.get(0..2);
        let to = string.get(2..4);

        let promotion = string
            .get(4..5)
            .and_then(|promotion| promotion.chars().next())
            .and_then(|char| Piece::from_char(char));

        // Can't promote to a king...
        if promotion.is_some_and(|p| p == Piece::King) {
            return None;
        }

        match (
            from.and_then(BoardSquare::parse),
            to.and_then(BoardSquare::parse),
        ) {
            (Some(from), Some(to)) => Some(BoardMove {
                from,
                to,
                promotion,
            }),
            _ => None,
        }
    }

    pub fn unparse(&self) -> String {
        format!(
            "{}{}{}",
            self.from.unparse(),
            self.to.unparse(),
            self.promotion
                .and_then(|p| Some(p.to_char().to_string()))
                .unwrap_or("".to_string())
        )
    }
}

pub type PieceBoard = [[Option<(Piece, Color)>; 8]; 8];

#[derive(Debug)]
pub enum MoveResultType {
    Success,         // successful move
    InvalidNotation, // wrong algebraic notation

    WrongSource,      // invalid source (no piece / wrong color piece)
    WrongDestination, // invalid destination (occupied square, that piece can't move there)

    MoveToCheck,
}

#[derive(Debug)]
pub struct Game {
    pub turn: Color,

    pub pieces: PieceBoard,

    pub castling_flags: u8, // 0x0000QKqk, where kq/KQ is one if black/white king and queen
    pub en_passant_bitmap: Bitboard, // if a piece just moved for the first time, 1 will be over the square

    pub color_bitboards: [Bitboard; 2],
    pub piece_bitboards: [Bitboard; Piece::COUNT],

    pub halfmove_clock: u64, // how many halfmoves have been played since the last capture or pawn advance
    pub fullmove_number: u64, // how many full moves have been played; incremented after black's move
}

impl Game {
    pub fn new(fen: Option<&str>) -> Game {
        let fen_game = fen.unwrap_or("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");

        let mut parts = fen_game.split_whitespace();

        let mut color_bitboards: [Bitboard; 2] = [Bitboard::default(); 2];
        let mut piece_bitboards: [Bitboard; Piece::COUNT] = [Bitboard::default(); Piece::COUNT];

        let mut pieces = PieceBoard::default();

        let mut y = 0u32;
        for rank in parts.next().unwrap().split('/') {
            let mut x = 0u32;

            for char in rank.chars() {
                // Numbers encode empty spaces
                if let Some(c) = char.to_digit(10) {
                    x += c;
                    continue;
                }

                let reversed_y = pieces.len() - y as usize - 1;

                let bitmap = Bitboard::position_to_bitmask(x, reversed_y as u32);

                let color = if char.is_ascii_uppercase() {
                    Color::White
                } else {
                    Color::Black
                };

                color_bitboards[color as usize] |= bitmap;

                match Piece::from_char(char.to_ascii_lowercase()) {
                    Some(piece) => {
                        piece_bitboards[piece as usize] |= bitmap;
                        pieces[reversed_y][x as usize] = Some((piece, color));
                    }
                    _ => {}
                }

                x += 1;
            }

            y += 1;
        }

        let turn = match parts.next() {
            Some("w") => Color::White,
            Some("b") => Color::Black,
            _ => panic!("Incorrect FEN format"),
        };

        let mut castling_flags = 0;
        for c in parts.next().unwrap().chars() {
            match c {
                'k' => castling_flags |= 0b00000001,
                'q' => castling_flags |= 0b00000010,
                'K' => castling_flags |= 0b00000100,
                'Q' => castling_flags |= 0b00001000,
                _ => {}
            }
        }

        let en_passant_bitmap = match parts.next() {
            Some("-") => 0,
            Some(board_square_string) => match BoardSquare::parse(board_square_string) {
                Some(square) => square.to_mask(),
                _ => panic!("FEN parsing failure: incorrect En Passant target square"),
            },
            _ => panic!("FEN parsing failure: incorrect En Passant target square"),
        };

        let halfmove_clock = parts.next().unwrap_or("0").parse().unwrap();
        let fullmove_number = parts.next().unwrap_or("0").parse().unwrap();

        // Debug bitmap prints
        color_bitboards[Color::White as usize].print(Some("White Bitboard"), None);
        color_bitboards[Color::Black as usize].print(Some("Black Bitboard"), None);

        for piece in Piece::iter() {
            piece_bitboards[piece as usize]
                .print(Some(format!("{:?} Piece Bitboard", piece).as_str()), None);
        }

        // for color in Color::iter() {
        //     for piece in Piece::iter() {
        //         for x in 0..8 {
        //             for y in 0..8 {
        //                 let bitboard =
        //                     VALID_MOVE_BITBOARDS[color as usize][piece as usize][x + y * 8];

        //                 if bitboard == 0 {
        //                     continue;
        //                 }

        //                 bitboard.print(
        //                     Some(format!("{:?} {:?} ({}, {})", color, piece, x, y).as_str()),
        //                     Some((x as u32, y as u32)),
        //                 )
        //             }
        //         }
        //     }
        // }

        Game {
            color_bitboards,
            turn,
            pieces,
            castling_flags,
            en_passant_bitmap,
            piece_bitboards,
            halfmove_clock,
            fullmove_number,
        }
    }

    pub fn unmake_move(&mut self, board_move: BoardMove) {
        unimplemented!()
    }

    pub fn make_move(&mut self, board_move: BoardMove) {
        /// TODO: promotions

        let (piece, color) = self.pieces[board_move.from.y as usize][board_move.from.x as usize]
            .expect("No piece at source square");

        // move on bitboards
        self.piece_bitboards[piece as usize] &= !board_move.from.to_mask();
        self.piece_bitboards[piece as usize] |= board_move.to.to_mask();

        self.color_bitboards[self.turn as usize] &= !board_move.from.to_mask();
        self.color_bitboards[self.turn as usize] |= board_move.to.to_mask();

        // move the piece itself
        self.pieces[board_move.to.y as usize][board_move.to.x as usize] = Some((piece, color));
        self.pieces[board_move.from.y as usize][board_move.from.x as usize] = None;

        // if we moved to the en-passant bit, do the en-passant thingy
        if self.en_passant_bitmap & board_move.to.to_mask() != 0 {
            self.pieces[board_move.from.y as usize][board_move.to.x as usize] = None;

            // TODO: other bitmaps: these should be moved to some function idk
        }

        // En-passant bit for
        if piece == Piece::Pawn
            && (board_move.from.y == 1 || board_move.from.y == 6)
            && (board_move.to.y == 3 || board_move.to.y == 4)
        {
            // TODO: maybe don't hardcode it like this?
            self.en_passant_bitmap = position_to_bitmask(
                board_move.from.x, // same as to
                (board_move.from.y + board_move.to.y) / 2,
            );
        } else {
            self.en_passant_bitmap = 0;
        }

        // TODO
        // castling_flags: u8, // 0x0000QKqk, where kq/KQ is one if black/white king and queen
        // en_passant_bitmap: u64, // if a piece just moved for the first time, 1 will be left in its place
    }

    ///
    /// Generate a bitboard that contains valid moves for a particular square,
    /// assuming the current game state.
    /// TODO: magic bitboards for occlusions
    /// TODO: pins
    /// TODO: castling
    ///
    pub fn get_valid_move_bitboard(&self, square: &BoardSquare) -> Bitboard {
        let (piece, color) = match self.pieces[square.y as usize][square.x as usize] {
            Some(v) => v,
            None => return 0,
        };

        let index = square.to_index();

        // Baseline valid moves bitmap
        let mut valid_moves = VALID_MOVE_BITBOARDS[self.turn as usize][piece as usize][index];

        // Can't capture own pieces
        valid_moves &= !self.color_bitboards[color as usize];

        // Magic bitboards for calculating blockers
        if piece == Piece::Rook || piece == Piece::Bishop {
            // first calculate the key
            let (blocker_bitboard, offset) = match piece {
                Piece::Rook => (ROOK_BLOCKER_BITBOARD[index], 0),
                Piece::Bishop => (BISHOP_BLOCKER_BITBOARD[index], 64),
                _ => unreachable!(),
            };

            let key = blocker_bitboard & (self.color_bitboards[0] | self.color_bitboards[1]);

            // then, given the magic table information
            let (magic_number, table_offset, bit_offset) = MAGIC_TABLE[offset + index];

            // we calculate the opacity bitmap
            let opacity_bitmap = MAGIC_ENTRIES
                [table_offset + (magic_number.wrapping_mul(key) >> bit_offset) as usize];

            valid_moves &= opacity_bitmap;
        }

        // If pawn
        if piece == Piece::Pawn {
            // First moves
            // TODO: maybe don't hardcode it like this?
            if square.y == 1 || square.y == 6 {
                valid_moves |= PAWN_FIRST_MOVE_BITBOARD[self.turn as usize][index];
            }

            // Attack moves towards enemy pieces (including en-passant)
            valid_moves |= PAWN_ATTACK_MOVE_BITBOARD[self.turn as usize][index]
                & (self.color_bitboards[!self.turn as usize] | self.en_passant_bitmap);
        }

        valid_moves
    }

    pub fn try_make_move(&mut self, board_move: BoardMove) -> MoveResultType {
        let from_bitmask = board_move.from.to_mask();
        let to_bitmask = board_move.to.to_mask();

        // source is empty/doesn't match the color of the piece
        match self.pieces[board_move.from.y as usize][board_move.from.x as usize] {
            Some((piece, color)) => {
                if color != self.turn {
                    return MoveResultType::WrongSource;
                }

                // Baseline valid moves bitmap
                let valid_moves = self.get_valid_move_bitboard(&board_move.from);

                // Not a valid destination square
                if valid_moves & to_bitmask == 0 {
                    return MoveResultType::WrongDestination;
                }

                self.make_move(board_move);
                self.turn = !self.turn;

                if self.turn == Color::White {
                    self.halfmove_clock += 1;
                }

                MoveResultType::Success
            }
            None => MoveResultType::WrongSource,
        }
    }
}
