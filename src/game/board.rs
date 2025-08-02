use super::pieces::{Color, Piece};
use crate::game::ColoredPiece;
use crate::utils::Bitboard;
use crate::{
    ATTACK_BITBOARDS,
    BISHOP_BLOCKER_BITBOARD,
    BitboardExt,
    MAGIC_ENTRIES,
    MAGIC_ROOK_BLOCKER_BITBOARD,
    MAGIC_TABLE,
    // TODO: we do not need all these bitboards, especially those for static figures since they're just some + and -...
    position_to_bitmask,
};
use std::cmp::PartialEq;
use strum::EnumCount;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct BoardSquare {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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
    InvalidMove,     // invalid move

    NoHistory, // can't undo -- no history
}

#[derive(Debug)]
pub struct Game {
    pub side: Color,

    pub pieces: PieceBoard,

    pub castling_flags: u8, // 0x0000KQkq, where kq/KQ is one if black/white king and queen
    pub en_passant_bitmap: Bitboard, // if a piece just moved for the first time, 1 will be over the square

    pub color_bitboards: [Bitboard; Color::COUNT],
    pub piece_bitboards: [Bitboard; Piece::COUNT],

    pub attack_bitboards: [Bitboard; Color::COUNT],

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

        let mut game = Game {
            color_bitboards: [Bitboard::default(); Color::COUNT],
            side: Color::White,
            pieces: [None; 64],
            castling_flags: 0,
            en_passant_bitmap: 0,
            piece_bitboards: [Bitboard::default(); Piece::COUNT],
            halfmoves_since_capture: 0,
            halfmoves: 0,
            history: vec![],
            attack_bitboards: [Bitboard::default(); Color::COUNT],
        };

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

                let color = if char.is_ascii_uppercase() {
                    Color::White
                } else {
                    Color::Black
                };

                match Piece::from_char(char.to_ascii_lowercase()) {
                    Some(piece) => game.set_piece(&square, (piece, color)),
                    _ => {}
                }

                x += 1;
            }

            y += 1;
        }

        game.side = match parts.next() {
            Some("w") => Color::White,
            Some("b") => Color::Black,
            _ => panic!("Incorrect FEN format"),
        };

        for c in parts.next().unwrap().chars() {
            match c {
                'k' => game.castling_flags |= 0b00000001,
                'q' => game.castling_flags |= 0b00000010,
                'K' => game.castling_flags |= 0b00000100,
                'Q' => game.castling_flags |= 0b00001000,
                _ => {}
            }
        }

        game.en_passant_bitmap = match parts.next() {
            Some("-") => 0,
            Some(board_square_string) => match BoardSquare::parse(board_square_string) {
                Some(square) => square.to_mask(),
                _ => panic!("FEN parsing failure: incorrect En Passant target square"),
            },
            _ => panic!("FEN parsing failure: incorrect En Passant target square"),
        };

        game.halfmoves_since_capture = parts.next().unwrap_or("0").parse::<usize>().unwrap();

        let fullmoves = parts.next().unwrap_or("0").parse::<usize>().unwrap();

        game.halfmoves = fullmoves * 2;
        if game.side == Color::Black {
            game.halfmoves += 1;
        }

        game
    }

    ///
    /// Update the attack/defense bitboard for a particular colored piece.
    ///
    fn update_attack_bitboard(&mut self, color: Color) {
        let mut bitboard = Bitboard::default();

        for board_square in self.color_bitboards[color as usize].iter_set_positions() {
            bitboard |= self.get_attack_bitboard(&board_square)
        }

        self.attack_bitboards[color as usize] = bitboard;
    }

    fn unset_piece(&mut self, square: &BoardSquare) {
        let index = square.to_index();

        if let Some(colored_piece @ (piece, color)) = self.pieces[index] {
            let mask = square.to_mask();

            self.piece_bitboards[piece as usize] &= !mask;
            self.color_bitboards[color as usize] &= !mask;
            self.pieces[index] = None;

            // TODO: move out so this doesn't get recomputed all the time
            self.update_attack_bitboard(color)
        }
    }

    fn set_piece(&mut self, square: &BoardSquare, colored_piece @ (piece, color): ColoredPiece) {
        let index = square.to_index();

        self.unset_piece(square);

        let mask = square.to_mask();

        self.piece_bitboards[piece as usize] |= mask;
        self.color_bitboards[color as usize] |= mask;
        self.pieces[index] = Some(colored_piece);

        // TODO: move out so this doesn't get recomputed all the time
        self.update_attack_bitboard(color)
    }

    pub(crate) fn unmake_move(&mut self) {
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

    pub(crate) fn make_move(&mut self, board_move: BoardMove) {
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

        // set en-passant bit if pawn went two tiles
        if piece == Piece::Pawn && board_move.from.y.abs_diff(board_move.to.y) == 2 {
            self.en_passant_bitmap =
                position_to_bitmask(board_move.from.x, (board_move.from.y + board_move.to.y) / 2);
        } else {
            self.en_passant_bitmap = 0;
        }

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

        self.history
            .push((board_move, captured_piece, castling_flags_copy));

        // TODO: half-moves since last capture
    }

    fn get_occlusion(&self, index: usize, piece: Piece) -> Bitboard {
        // first calculate the key
        let (blocker_bitboard, offset) = match piece {
            Piece::Rook => (MAGIC_ROOK_BLOCKER_BITBOARD[index], 0),
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
    /// Retrieve attack moves for the piece at the particular square.
    /// Useful for preventing the king from getting narked.
    ///
    fn get_attack_bitboard(&self, square: &BoardSquare) -> Bitboard {
        let index = square.to_index();

        let (piece, color) = match self.pieces[index] {
            Some(v) => v,
            None => return 0,
        };

        // By default, just use pre-calculated attack bitboards
        let mut valid_moves = ATTACK_BITBOARDS[color as usize][piece as usize][index];

        match piece {
            // Slider stuff using magic bitmaps
            Piece::Bishop | Piece::Rook => {
                let opacity_bitmap = self.get_occlusion(index, piece);

                valid_moves &= opacity_bitmap;
            }
            Piece::Queen => {
                valid_moves = ATTACK_BITBOARDS[color as usize][piece as usize][index];

                let opacity_bitmap = self.get_occlusion(index, Piece::Rook)
                    | self.get_occlusion(index, Piece::Bishop);

                valid_moves &= opacity_bitmap;
            }
            // Otherwise just leaper stuff
            _ => {
                valid_moves = ATTACK_BITBOARDS[color as usize][piece as usize][index];
            }
        }

        valid_moves
    }

    ///
    /// Generate a bitboard that contains pseud-legal moves for a particular square,
    /// assuming the current game state and ignoring safety-related rules.
    ///
    /// TODO: pins
    /// TODO: prevent check
    ///
    fn get_pseudo_legal_move_bitboard(&self, square: &BoardSquare) -> Bitboard {
        let index = square.to_index();

        let (piece, color) = match self.pieces[index] {
            Some(v) => v,
            None => return 0,
        };

        // Obtain attack move bitboard, which are also regular moves for all but pawns
        let mut valid_moves = self.get_attack_bitboard(square);

        match piece {
            // Pawn stuff
            Piece::Pawn => {
                // Attack moves only when there is enemy or en-passant
                valid_moves &= (self.color_bitboards[!color as usize] | self.en_passant_bitmap);

                // First moves + regular moves
                match (color, square.y) {
                    (Color::White, 1) => valid_moves |= 1 << (index + 16) | 1 << (index + 8),
                    (Color::Black, 6) => valid_moves |= 1 << (index - 16) | 1 << (index - 8),
                    (Color::White, _) => valid_moves |= 1 << (index + 8),
                    (Color::Black, _) => valid_moves |= 1 << (index - 8),
                }
            }
            // King stuff
            Piece::King => {
                // get occlusion to the left/right side of king and use it to mask the castling bit
                // occlusion is also attacks
                let castling_blockers = (valid_moves
                    & (self.color_bitboards[0]
                        | self.color_bitboards[1]
                        | self.attack_bitboards[!color as usize]))
                    >> (color as usize ^ 1) * 56;

                let castling_blocker_bits =
                    (!((castling_blockers >> 4) | (castling_blockers >> 3))) & 0b11;

                // castling bits for the particular color
                let castling_bits =
                    self.castling_flags >> (color as usize * 2) & castling_blocker_bits as u8;

                // fun optimization -- multiplying by 9 yields the correct bitboards:
                // 0b00 * 9 -> b00000
                // 0b01 * 9 -> b01001
                // 0b10 * 9 -> b10010
                // 0b11 * 9 -> b11011
                valid_moves |= (castling_bits as u64 * 9 << 2) << ((color as usize ^ 1) * 56)
                    & !(self.color_bitboards[0] | self.color_bitboards[1]);

                // also never move into attacks
                valid_moves &= !self.attack_bitboards[!color as usize];
            }
            _ => {}
        }

        // Can't ever capture / go through own pieces
        valid_moves &= !self.color_bitboards[color as usize];

        valid_moves
    }

    ///
    /// Return valid moves for a particular square.
    ///
    pub fn get_square_valid_moves(&self, square: &BoardSquare) -> ValidMovesIterator {
        let index = square.to_index();

        let bitboard = self.get_pseudo_legal_move_bitboard(square);

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

    ///
    /// Obtain a list of valid moves for the current position.
    ///
    pub fn get_current_position_moves(&self) -> Vec<BoardMove> {
        self.color_bitboards[self.side as usize]
            .iter_set_positions()
            .flat_map(|p| self.get_square_valid_moves(&p))
            .collect::<Vec<_>>()
    }
}
