use super::pieces::{Color, Piece};
use crate::game::ColoredPiece;
use crate::utils::Bitboard;
use crate::{
    ATTACK_BITBOARDS, BitboardExt, MAGIC_BISHOP_BLOCKER_BITBOARD, MAGIC_ENTRIES,
    MAGIC_ROOK_BLOCKER_BITBOARD, MAGIC_TABLE, PAWN_ATTACK_BITBOARDS, position_to_bitmask,
};
use std::cmp::PartialEq;
use strum::{EnumCount, IntoEnumIterator};

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
            let result = Some(BoardMove {
                from: self.square,
                to: to_square,
                promotion: Piece::from_repr(self.current_promotion),
            });

            self.current_promotion += 1;

            if self.current_promotion == 4 {
                self.bitboard &= self.bitboard - 1;
                self.current_promotion = 0;
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
    // the flags can NOT be calculated as an arbitrary position can have those
    pub history: Vec<(BoardMove, Option<ColoredPiece>, u8, Bitboard)>,
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

        // Fullmoves start at 1 and are incremented for white play
        let fullmoves = parts.next().unwrap_or("1").parse::<usize>().unwrap();
        game.halfmoves = (fullmoves - 1) * 2 + 1;
        if game.side == Color::Black {
            game.halfmoves += 1;
        }

        game
    }

    pub fn get_fen(&self) -> String {
        let mut fen = String::new();

        for y in 0..8 {
            let rank = 7 - y;
            let mut empty_count = 0;

            for x in 0..8 {
                let square = BoardSquare { x, y: rank };
                let square_index = square.to_index();

                if let Some((piece, color)) = self.pieces[square_index] {
                    // Empty square count before a piece
                    if empty_count > 0 {
                        fen.push_str(&empty_count.to_string());
                        empty_count = 0;
                    }

                    // Add the piece character
                    let piece_char = piece.to_char();
                    if color == Color::White {
                        fen.push(piece_char.to_ascii_uppercase());
                    } else {
                        fen.push(piece_char);
                    }
                } else {
                    empty_count += 1;
                }
            }

            // Remaining empty squares
            if empty_count > 0 {
                fen.push_str(&empty_count.to_string());
            }

            // No delimiter for last rank
            if y < 7 {
                fen.push('/');
            }
        }

        // Active color
        fen.push(' ');
        fen.push(if self.side == Color::White { 'w' } else { 'b' });

        // Castling
        fen.push(' ');
        let mut castling = String::new();
        if self.castling_flags & 0b00000100 != 0 {
            castling.push('K');
        }
        if self.castling_flags & 0b00001000 != 0 {
            castling.push('Q');
        }
        if self.castling_flags & 0b00000001 != 0 {
            castling.push('k');
        }
        if self.castling_flags & 0b00000010 != 0 {
            castling.push('q');
        }

        if castling.is_empty() {
            fen.push('-');
        } else {
            fen.push_str(&castling);
        }

        // En passant
        fen.push(' ');
        if self.en_passant_bitmap == 0 {
            fen.push('-');
        } else {
            let square = BoardSquare::from_index(self.en_passant_bitmap.trailing_zeros() as u64);
            fen.push_str(&square.unparse());
        }

        // Halfmove clock
        fen.push(' ');
        fen.push_str(&self.halfmoves_since_capture.to_string());

        // Fulmove from total halfmoves
        fen.push(' ');
        let fullmoves = if self.side == Color::White {
            self.halfmoves / 2 + 1
        } else {
            (self.halfmoves + 1) / 2
        };
        fen.push_str(&fullmoves.to_string());

        fen
    }

    ///
    /// Update the attack/defense bitboards for both colors.
    ///
    fn update_attack_bitboards(&mut self) {
        for color in Color::iter() {
            let mut bitboard = Bitboard::default();

            for board_square in self.color_bitboards[color as usize].iter_set_positions() {
                bitboard |= self.get_attack_bitboard(&board_square)
            }

            self.attack_bitboards[color as usize] = bitboard;
        }
    }

    fn unset_piece(&mut self, square: &BoardSquare) {
        let index = square.to_index();

        debug_assert!(self.pieces[index].is_some());

        let (piece, color) = self.pieces[index].unwrap();
        let mask = square.to_mask();

        self.piece_bitboards[piece as usize] &= !mask;
        self.color_bitboards[color as usize] &= !mask;
        self.pieces[index] = None;
    }

    fn set_piece(&mut self, square: &BoardSquare, colored_piece @ (piece, color): ColoredPiece) {
        let index = square.to_index();

        debug_assert!(self.pieces[index].is_none());

        let mask = square.to_mask();

        self.piece_bitboards[piece as usize] |= mask;
        self.color_bitboards[color as usize] |= mask;
        self.pieces[index] = Some(colored_piece);
    }

    pub(crate) fn unmake_move(&mut self) {
        let (board_move, captured_piece, castling_flags, en_passant_bitmap) =
            self.history.pop().unwrap();

        let colored_piece @ (piece, color) = self.pieces[board_move.to.to_index()].expect(
            "No piece at target square when unmaking a move. This should never ever happen.",
        );

        // move the piece back
        self.unset_piece(&board_move.to);

        // if we promoted, make sure to unpromote
        self.set_piece(&board_move.from, match board_move.promotion {
            None => colored_piece,
            Some(_) => (Piece::Pawn, color),
        });

        // place the captured piece back
        if let Some(c_colored_piece) = captured_piece {
            self.set_piece(&board_move.to, c_colored_piece);
        }

        // restore bitmaps / flags
        self.castling_flags = castling_flags;
        self.en_passant_bitmap = en_passant_bitmap;

        // uncastle, if the king moved 2 spots
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
        }

        self.side = !self.side;
        self.halfmoves -= 1;

        self.update_attack_bitboards()
    }

    pub(crate) fn make_move(&mut self, board_move: BoardMove) {
        let from_index = board_move.from.to_index();

        let castling_flags_copy = self.castling_flags;
        let en_passant_bitmap_copy = self.en_passant_bitmap;

        let mut promoted = false;

        let (mut piece, color) =
            self.pieces[from_index].expect("No piece at the source square while making a move.");

        let to_index = board_move.to.to_index();
        let captured_piece = self.pieces[to_index];

        // remove captured piece
        if captured_piece.is_some() {
            self.unset_piece(&board_move.to);

            // if we capture rooks, modify the flag of the side whose rook it was
            if let Some((Piece::Rook, c)) = captured_piece {
                let i = match (c, board_move.to.x, board_move.to.y) {
                    (Color::Black, 0, 7) => 1,
                    (Color::Black, 7, 7) => 2,
                    (Color::White, 0, 0) => 4,
                    (Color::White, 7, 0) => 8,
                    _ => 0,
                };

                self.castling_flags &= !i;
            }
        }

        // moves + promotions
        self.unset_piece(&board_move.from);

        // if pawn reaches the last rank, promote!
        if piece == Piece::Pawn && (board_move.to.y == 7 || board_move.to.y == 0) {
            piece = board_move
                .promotion
                .expect("Pawn move to last rank must contain promotion information.");

            promoted = true;
        }

        self.set_piece(&board_move.to, (piece, color));

        // if we moved to the en-passant bit, with a pawn, take
        if piece == Piece::Pawn && self.en_passant_bitmap & board_move.to.to_mask() != 0 {
            self.unset_piece(&BoardSquare {
                x: board_move.to.x,
                y: board_move.from.y,
            })
        }

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
        if piece == Piece::Rook && !promoted {
            let i = match (color, board_move.from.x, board_move.from.y) {
                (Color::Black, 0, 7) => 1,
                (Color::Black, 7, 7) => 2,
                (Color::White, 0, 0) => 4,
                (Color::White, 7, 0) => 8,
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

        self.history.push((
            board_move,
            captured_piece,
            castling_flags_copy,
            en_passant_bitmap_copy,
        ));

        self.update_attack_bitboards()
    }

    fn get_occlusion_bitmap(
        &self,
        board_index: usize,
        piece: Piece,
        blockers: Option<Bitboard>,
    ) -> Bitboard {
        // first calculate the key
        let (possible_blocker_positions_bitboard, offset) = match piece {
            Piece::Rook => (MAGIC_ROOK_BLOCKER_BITBOARD[board_index], 0),
            Piece::Bishop => (MAGIC_BISHOP_BLOCKER_BITBOARD[board_index], 64),
            _ => unreachable!(),
        };

        let key = possible_blocker_positions_bitboard
            & blockers.unwrap_or(self.color_bitboards[0] | self.color_bitboards[1]);

        // then, given the magic table information...
        let (magic_number, table_offset, bit_offset) = MAGIC_TABLE[offset + board_index];

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
        let mut valid_moves = ATTACK_BITBOARDS[piece as usize][index];

        match piece {
            // Slider stuff using magic bitmaps
            Piece::Bishop | Piece::Rook => {
                let opacity_bitmap = self.get_occlusion_bitmap(index, piece, None);

                valid_moves &= opacity_bitmap;
            }
            Piece::Queen => {
                let opacity_bitmap = self.get_occlusion_bitmap(index, Piece::Rook, None)
                    | self.get_occlusion_bitmap(index, Piece::Bishop, None);

                valid_moves &= opacity_bitmap;
            }
            Piece::Pawn => {
                valid_moves = PAWN_ATTACK_BITBOARDS[color as usize][index];
            }
            _ => {}
        }

        valid_moves
    }

    ///
    /// Generate a bitboard that contains pseud-legal moves for a particular square,
    /// ignoring most safety-related rules and using the color of the piece as the
    /// current turn (i.e. ignoring `this.side`).
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
                valid_moves &= self.color_bitboards[!color as usize] | self.en_passant_bitmap;

                // regular moves (not into/through pieces)
                let forward_move = match color {
                    Color::White => 1 << (index + 8),
                    Color::Black => 1 << (index - 8),
                } & !(self.color_bitboards[color as usize]
                    | self.color_bitboards[!color as usize]);

                valid_moves |= forward_move;

                // if we can move forward, we can also try double forward
                if forward_move != 0 {
                    valid_moves |= match (color, square.y) {
                        (Color::White, 1) => 1 << (index + 16) | 1 << (index + 8),
                        (Color::Black, 6) => 1 << (index - 16) | 1 << (index - 8),
                        _ => 0,
                    } & !(self.color_bitboards[color as usize]
                        | self.color_bitboards[!color as usize])
                }
            }
            // King stuff
            Piece::King => {
                // we can only castle if there are no blocking pieces
                let castling_blockers = (self.color_bitboards[0] | self.color_bitboards[1])
                    >> (color as usize ^ 1) * 56;

                // and further if there is no check in the castling directions
                let castling_attackers =
                    self.attack_bitboards[!color as usize] >> (color as usize ^ 1) * 56;

                // 0bQK, where KQ is queenside/kingside castling respectively
                let castling_blocker_bits = (((0b00001110 & castling_blockers) == 0) as usize
                    | ((((0b01100000 & castling_blockers) == 0) as usize) << 1))
                    & (((0b00001100 & castling_attackers) == 0) as usize
                        | ((((0b01100000 & castling_attackers) == 0) as usize) << 1));

                // if the king is not under check, we can castle
                if self.attack_bitboards[!color as usize]
                    & self.piece_bitboards[Piece::King as usize]
                    & self.color_bitboards[color as usize]
                    == 0
                {
                    // castling bits for the particular color
                    let castling_bits =
                        self.castling_flags >> (color as usize * 2) & castling_blocker_bits as u8;

                    // fun optimization -- multiplying by 9 yields almost the correct bitboards:
                    // 0b00 * 9 -> b00000
                    // 0b01 * 9 -> b01001
                    // 0b10 * 9 -> b10010
                    // 0b11 * 9 -> b11011
                    valid_moves |=
                        ((castling_bits as u64 * 9 & 0b10001) << 2) << ((color as usize ^ 1) * 56)
                }

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
    /// Retrieve bitboards with pieces that are pinning.
    ///
    pub fn get_pinner_bitboards(&self) -> [Bitboard; 2] {
        let king_index = (self.piece_bitboards[Piece::King as usize]
            & self.color_bitboards[self.side as usize])
            .trailing_zeros() as usize;

        let mut pinners = [0, 0];

        for (i, &piece) in [Piece::Bishop, Piece::Rook].iter().enumerate() {
            // first, get occlusions to get possible pinned pieces
            let possible_blockers_occlusion = self.get_occlusion_bitmap(king_index, piece, None);

            // next, subtract the calculated occlusions from blockers to get the blocked pieces
            let possible_attackers_occlusion = self.get_occlusion_bitmap(
                king_index,
                piece,
                Some(
                    (self.color_bitboards[Color::White as usize]
                        | self.color_bitboards[Color::Black as usize])
                        & !possible_blockers_occlusion,
                ),
            );

            // now we can get a bitmap of the attackers that are pinning things down
            let attackers_bitmap = (self.piece_bitboards[piece as usize]
                | self.piece_bitboards[Piece::Queen as usize])
                & self.color_bitboards[!self.side as usize]
                & (possible_attackers_occlusion & !possible_blockers_occlusion);

            pinners[i] = attackers_bitmap;
        }

        pinners
    }

    ///
    /// Obtain a list of valid moves for the current position.
    ///
    pub fn get_current_position_moves(&mut self) -> Vec<BoardMove> {
        let pseudo_safe_positions = self.color_bitboards[self.side as usize]
            .iter_set_positions()
            .flat_map(|p| self.get_square_valid_moves(&p))
            .collect::<Vec<_>>();

        pseudo_safe_positions
            .into_iter()
            .filter(|&p| {
                self.make_move(p);

                let mut is_king_safe = true;

                // the colors here are flipped!
                if (self.attack_bitboards[self.side as usize]
                    & (self.piece_bitboards[Piece::King as usize]
                        & self.color_bitboards[!self.side as usize]))
                    != 0
                {
                    is_king_safe = false;
                }

                self.unmake_move();

                is_king_safe
            })
            .collect::<Vec<_>>()
    }
}
