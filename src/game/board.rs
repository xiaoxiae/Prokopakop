use super::pieces::{Color, Piece};
use crate::game::ColoredPiece;
use crate::utils::Bitboard;
use crate::{
    BISHOP_BLOCKER_BITBOARD, BitboardExt, MAGIC_ENTRIES, MAGIC_TABLE, PAWN_ATTACK_MOVE_BITBOARD,
    PAWN_FIRST_MOVE_BITBOARD, ROOK_BLOCKER_BITBOARD, VALID_MOVE_BITBOARDS, position_to_bitmask,
};
use std::cmp::PartialEq;
use strum::{EnumCount, IntoEnumIterator};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]

// TODO: should store the index instead of x/y
pub struct BoardSquare {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Copy, Clone)]
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
        format!(
            "{}{}",
            (self.x as u8 + b'a' as u8) as char,
            (self.y as u8 + b'1' as u8) as char
        )
    }

    pub fn to_mask(&self) -> Bitboard {
        Bitboard::position_to_bitmask(self.x, self.y)
    }

    pub fn to_index(&self) -> usize {
        self.x as usize + self.y as usize * 8
    }

    pub fn from_index(index: u64) -> BoardSquare {
        BoardSquare {
            x: (index % 8) as u32,
            y: (index / 8) as u32,
        }
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

        // Can't promote to a king or pawn
        if promotion.is_some_and(|p| p == Piece::King || p == Piece::Pawn) {
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

pub struct ValidMovesIterator {
    bitboard: Bitboard,
    square: BoardSquare,

    is_promoting: bool,
    current_promotion: usize,
}

impl Iterator for ValidMovesIterator {
    type Item = BoardMove;

    fn next(&mut self) -> Option<Self::Item> {
        // out of moves!
        if self.bitboard == 0 {
            return None;
        }

        let to_index = self.bitboard.trailing_zeros() as u64;
        let to_square = BoardSquare::from_index(to_index);

        if self.is_promoting {
            // the first 4 are promoting pieces; if we get to 5, advance to next bit
            let result;
            if self.current_promotion == 5 {
                self.current_promotion = 0;
                self.bitboard &= self.bitboard - 1;

                result = Some(BoardMove {
                    from: self.square,
                    to: to_square,
                    promotion: Piece::from_repr(self.current_promotion),
                });
            } else {
                result = Some(BoardMove {
                    from: self.square,
                    to: to_square,
                    promotion: Piece::from_repr(self.current_promotion),
                });

                self.current_promotion += 1;
            }

            result
        } else {
            self.bitboard &= self.bitboard - 1;

            Some(BoardMove {
                from: self.square.clone(),
                to: to_square,
                promotion: None,
            })
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.bitboard.count_ones() as usize;
        (count, Some(count))
    }
}

pub type PieceBoard = [Option<ColoredPiece>; 64];

#[derive(Debug)]
pub enum MoveResultType {
    Success,         // successful move
    InvalidNotation, // wrong algebraic notation

    WrongSource,      // invalid source (no piece / wrong color piece)
    WrongDestination, // invalid destination (occupied square, that piece can't move there)

    NoHistory, // can't undo -- no history
}

#[derive(Debug)]
pub struct Game {
    pub side: Color,

    pub pieces: PieceBoard,

    pub castling_flags: u8, // 0x0000KQkq, where kq/KQ is one if black/white king and queen
    pub en_passant_bitmap: Bitboard, // if a piece just moved for the first time, 1 will be over the square

    pub color_bitboards: [Bitboard; 2],
    pub piece_bitboards: [Bitboard; Piece::COUNT],

    pub halfmoves: usize,
    pub halfmoves_since_capture: usize,

    // store the move, which piece was there, and en-passant + castling flags
    // you might think: can't we calculate the flags? NO!
    // - castling flag is needed, since there might be some fucky movement on the last rank back and forth
    pub history: Vec<(BoardMove, Option<ColoredPiece>, u8)>,
}

impl Game {
    pub fn new(fen: Option<&str>) -> Game {
        let fen_game = fen.unwrap_or("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");

        let mut parts = fen_game.split_whitespace();

        let mut color_bitboards: [Bitboard; 2] = [Bitboard::default(); 2];
        let mut piece_bitboards: [Bitboard; Piece::COUNT] = [Bitboard::default(); Piece::COUNT];

        let mut pieces = [None; 64];

        let mut y = 0u32;
        for rank in parts.next().unwrap().split('/') {
            let mut x = 0u32;

            for char in rank.chars() {
                // Numbers encode empty spaces
                if let Some(c) = char.to_digit(10) {
                    x += c;
                    continue;
                }

                let square = BoardSquare { x, y: 8 - y - 1 };

                let bitmap = square.to_mask();
                let index = square.to_index();

                let color = if char.is_ascii_uppercase() {
                    Color::White
                } else {
                    Color::Black
                };

                color_bitboards[color as usize] |= bitmap;

                match Piece::from_char(char.to_ascii_lowercase()) {
                    Some(piece) => {
                        piece_bitboards[piece as usize] |= bitmap;
                        pieces[index] = Some((piece, color));
                    }
                    _ => {}
                }

                x += 1;
            }

            y += 1;
        }

        let side = match parts.next() {
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

        let halfmoves_since_capture = parts.next().unwrap_or("0").parse::<usize>().unwrap();
        let fullmoves = parts.next().unwrap_or("0").parse::<usize>().unwrap();

        // Debug bitmap prints
        color_bitboards[Color::White as usize].print(Some("White Bitboard"), None);
        color_bitboards[Color::Black as usize].print(Some("Black Bitboard"), None);

        for piece in Piece::iter() {
            piece_bitboards[piece as usize]
                .print(Some(format!("{:?} Piece Bitboard", piece).as_str()), None);
        }

        let mut halfmoves = fullmoves * 2;
        if side == Color::Black {
            halfmoves += 1;
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
            side,
            pieces,
            castling_flags,
            en_passant_bitmap,
            piece_bitboards,
            halfmoves_since_capture,
            halfmoves,
            history: vec![],
        }
    }

    fn unset_piece(&mut self, square: &BoardSquare) {
        let index = square.to_index();

        if let Some((piece, color)) = self.pieces[index] {
            let mask = square.to_mask();

            // nuke piece/color
            self.piece_bitboards[piece as usize] &= !mask;
            self.color_bitboards[color as usize] &= !mask;
            self.pieces[index] = None;
        }
    }

    fn set_piece(&mut self, square: &BoardSquare, colored_piece: ColoredPiece) {
        let index = square.to_index();

        self.unset_piece(square);

        let mask = square.to_mask();

        // then do the actual placing
        self.piece_bitboards[colored_piece.0 as usize] |= mask;
        self.color_bitboards[colored_piece.1 as usize] |= mask;
        self.pieces[index] = Some(colored_piece);
    }

    pub fn unmake_move(&mut self) {
        let (board_move, captured_piece, castling_flags_copy) = self.history.pop().unwrap();

        // move the piece back
        let colored_piece @ (piece, color) = self.pieces[board_move.to.to_index()].unwrap();

        self.unset_piece(&board_move.to);
        self.set_piece(&board_move.from, colored_piece);

        // place the captured piece back
        if let Some(c_colored_piece) = captured_piece {
            self.set_piece(&board_move.to, c_colored_piece);
        }

        // restore bitmaps
        self.castling_flags = castling_flags_copy;

        // uncastle
        if piece == Piece::King && board_move.from.x.abs_diff(board_move.to.x) == 2 {
            self.set_piece(
                &BoardSquare {
                    // bit hack: the to X position is either 2 (0b10) or 6 (0b110),
                    // so >> gives us a flag whether it's the first or last file
                    x: (board_move.to.x >> 2) * 7,
                    y: board_move.from.y,
                },
                (Piece::Rook, color),
            );

            self.unset_piece(&BoardSquare {
                x: (board_move.from.x + board_move.to.x) / 2,
                y: board_move.from.y,
            });
        }

        // if pawn moves in a cross manner and doesn't capture piece, en-passant happened
        if piece == Piece::Pawn && captured_piece.is_none() && board_move.to.x != board_move.from.x
        {
            let captured_pawn_square = BoardSquare {
                x: board_move.to.x,
                y: board_move.from.y,
            };

            self.set_piece(&captured_pawn_square, (Piece::Pawn, !color));
            self.en_passant_bitmap = board_move.to.to_mask();
        } else {
            self.en_passant_bitmap = 0;
        }

        self.side = !self.side;
        self.halfmoves -= 1;

        // TODO: half-moves since last capture
    }

    pub fn make_move(&mut self, board_move: BoardMove) {
        let from_index = board_move.from.to_index();

        let (mut piece, color) = self.pieces[from_index].expect("No piece at source square");

        let to_index = board_move.to.to_index();

        let captured_piece = self.pieces[to_index];

        self.unset_piece(&board_move.from);

        // if pawn reaches the last rank, promote!
        if piece == Piece::Pawn && (board_move.to.y == 7 || board_move.to.y == 0) {
            piece = board_move
                .promotion
                .expect("Pawn move to last rank must contain promotion information.");
        }

        self.set_piece(&board_move.to, (piece, color));

        // if we moved to the en-passant bit, with a pawn, take
        if piece == Piece::Pawn && self.en_passant_bitmap & board_move.to.to_mask() != 0 {
            self.unset_piece(&BoardSquare {
                x: board_move.to.x,
                y: board_move.from.y,
            })
        }

        let castling_flags_copy = self.castling_flags;

        // set en-passant bit if pawn en-passanted
        if piece == Piece::Pawn
            && (board_move.from.y == 1 || board_move.from.y == 6)
            && (board_move.to.y == 3 || board_move.to.y == 4)
        {
            self.en_passant_bitmap =
                position_to_bitmask(board_move.from.x, (board_move.from.y + board_move.to.y) / 2);
        } else {
            self.en_passant_bitmap = 0;
        }

        self.history
            .push((board_move, captured_piece, castling_flags_copy));

        self.side = !self.side;
        self.halfmoves += 1;

        // rook moves & castling
        if piece == Piece::Rook {
            let i = match (color, board_move.from.x) {
                (Color::Black, 0) => 1,
                (Color::Black, 7) => 2,
                (Color::White, 0) => 4,
                (Color::White, 7) => 8,
                _ => 0,
            };

            self.castling_flags &= !i;
        }

        // king moves
        if piece == Piece::King {
            // castling (move by 2)
            if board_move.from.x.abs_diff(board_move.to.x) == 2 {
                self.unset_piece(&BoardSquare {
                    // bit hack: the to X position is either 2 (0b10) or 6 (0b110),
                    // so >> gives us a flag whether it's the first or last file
                    x: (board_move.to.x >> 2) * 7,
                    y: board_move.from.y,
                });

                self.set_piece(
                    &BoardSquare {
                        x: (board_move.from.x + board_move.to.x) / 2,
                        y: board_move.from.y,
                    },
                    (Piece::Rook, color),
                );
            }

            // either way no more castling for this side
            self.castling_flags &= !(0b11 << (2 * color as usize));
        }

        // TODO: half-moves since last capture
    }

    pub fn get_occlusion(&self, index: usize, piece: Piece) -> Bitboard {
        // first calculate the key
        let (blocker_bitboard, offset) = match piece {
            Piece::Rook => (ROOK_BLOCKER_BITBOARD[index], 0),
            Piece::Bishop => (BISHOP_BLOCKER_BITBOARD[index], 64),
            _ => unreachable!(),
        };

        let key = blocker_bitboard & (self.color_bitboards[0] | self.color_bitboards[1]);

        // then, given the magic table information...
        let (magic_number, table_offset, bit_offset) = MAGIC_TABLE[offset + index];

        // ... obtain calculate the blocker bitboard
        MAGIC_ENTRIES[table_offset + (magic_number.wrapping_mul(key) >> bit_offset) as usize]
    }

    ///
    /// Generate a bitboard that contains valid moves for a particular square,
    /// assuming the current game state.
    /// TODO: pins
    /// TODO: don't move into check (also during castling!)
    /// TODO: prevent check
    ///
    pub fn get_square_valid_move_bitboard(&self, square: &BoardSquare) -> Bitboard {
        let index = square.to_index();
        let mask = square.to_mask();

        let (piece, color) = match self.pieces[index] {
            Some(v) => v,
            None => return 0,
        };

        // Can't play as an opposing piece
        if color != self.side {
            return 0;
        }

        // Baseline valid moves bitmap
        let mut valid_moves = VALID_MOVE_BITBOARDS[self.side as usize][piece as usize][index];

        match piece {
            // Pawn stuff
            Piece::Pawn => {
                // First moves apply when 1st or 6th rank
                if square.y == 1 || square.y == 6 {
                    valid_moves |= PAWN_FIRST_MOVE_BITBOARD[self.side as usize][index];
                }

                // Attack moves towards enemy pieces (including en-passant)
                valid_moves |= PAWN_ATTACK_MOVE_BITBOARD[self.side as usize][index]
                    & (self.color_bitboards[!self.side as usize] | self.en_passant_bitmap);
            }
            // Magic bitboard stuff
            Piece::Bishop | Piece::Rook => {
                let opacity_bitmap = self.get_occlusion(index, piece);
                valid_moves &= opacity_bitmap;
            }
            Piece::Queen => {
                // For queen, it's combined
                let opacity_bitmap = self.get_occlusion(index, Piece::Rook)
                    | self.get_occlusion(index, Piece::Bishop);

                valid_moves &= opacity_bitmap;
            }
            // King stuff
            Piece::King => {
                // TODO: check occlusions of enemy attackers and create a bitmap with it
                //   - it's only necessary to look at rooks/bishops, others aren't sliding (pawns, kings, knights)
                //   - once we have this, we can just end valid moves and we're good

                // castling bits for the particular color
                let castling_bits = self.castling_flags >> (color as usize * 2) & 0b11;

                // fun optimization -- multiplying by 9 yields the correct bitboards:
                // 0b00 * 9 -> b00000
                // 0b01 * 9 -> b01001
                // 0b10 * 9 -> b10010
                // 0b11 * 9 -> b11011
                valid_moves |= (castling_bits as u64 * 9 << 2) << ((color as usize ^ 1) * 56);
            }
            _ => {}
        }

        // Can't ever capture / go through own pieces
        valid_moves &= !self.color_bitboards[color as usize];

        valid_moves
    }

    pub fn get_square_valid_moves(&self, square: &BoardSquare) -> ValidMovesIterator {
        let index = square.to_index();

        let bitboard = self.get_square_valid_move_bitboard(square);

        // pawn on second to last rank with some valid moves must promote
        let is_promoting = bitboard != 0
            && self.pieces[index].is_some_and(|(p, c)| {
                p == Piece::Pawn
                    && (square.y == 6 && c == Color::White || square.y == 1 && c == Color::Black)
            });

        ValidMovesIterator {
            square: square.clone(),
            bitboard,
            is_promoting,
            current_promotion: 0,
        }
    }
}
