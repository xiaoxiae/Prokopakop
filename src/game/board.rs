use super::pieces::{Color, Piece};
use crate::game::ColoredPiece;
use crate::utils::Bitboard;
use crate::zobris::ZOBRIST;
use crate::{
    BitboardExt, BoardSquare, BoardSquareExt, MAGIC_BISHOP_BLOCKER_BITBOARD, MAGIC_ENTRIES,
    MAGIC_ROOK_BLOCKER_BITBOARD, MAGIC_TABLE, PIECE_MOVE_BITBOARDS,
};
use strum::EnumCount;
use strum::IntoEnumIterator;

pub type BoardMove = u16;

pub trait BoardMoveExt {
    fn new(from: BoardSquare, to: BoardSquare, promotion: Option<Piece>) -> BoardMove;
    fn get_from(&self) -> BoardSquare;
    fn get_to(&self) -> BoardSquare;
    fn get_promotion(&self) -> Option<Piece>;
    fn parse(string: &str) -> Option<BoardMove>;
    fn unparse(&self) -> String;
}

impl BoardMoveExt for u16 {
    fn new(from: BoardSquare, to: BoardSquare, promotion: Option<Piece>) -> BoardMove {
        (from as u16)
            | ((to as u16) << 6)
            | ((promotion
                .and_then(|p| Some(1 << (p as u16)))
                .unwrap_or_default())
                << 12)
    }

    fn get_from(&self) -> BoardSquare {
        (*self as BoardSquare) & 0b111111
    }
    fn get_to(&self) -> BoardSquare {
        ((*self >> 6) as BoardSquare) & 0b111111
    }

    fn get_promotion(&self) -> Option<Piece> {
        let shifted = *self >> 12;
        if shifted == 0 {
            None
        } else {
            Piece::from_repr(shifted.trailing_zeros() as usize)
        }
    }

    fn parse(string: &str) -> Option<BoardMove> {
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
            (Some(from), Some(to)) => Some(BoardMove::new(from, to, promotion)),
            _ => None,
        }
    }

    fn unparse(&self) -> String {
        format!(
            "{}{}{}",
            self.get_from().unparse(),
            self.get_to().unparse(),
            self.get_promotion()
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

        let to_square = self.bitboard.next_index();

        if self.is_promoting {
            let result = Some(BoardMove::new(
                self.square,
                to_square,
                Piece::from_repr(self.current_promotion),
            ));

            self.current_promotion += 1;

            if self.current_promotion == 4 {
                self.bitboard &= self.bitboard - 1;
                self.current_promotion = 0;
            }

            result
        } else {
            self.bitboard &= self.bitboard - 1;

            Some(BoardMove::new(self.square.clone(), to_square, None))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.bitboard.count_ones() as usize;
        (count, Some(count))
    }
}

#[derive(Debug, Clone, Default)]
pub struct PinInfo {
    pub pinned_piece_position: BoardSquare,
    pub valid_move_squares: Bitboard,
}

#[derive(Debug, Clone, Default)]
pub struct PinData {
    pub pins: [[PinInfo; 4]; 2], // [bishop_pins, rook_pins]
    pub pin_counts: [usize; 2],  // [bishop_count, rook_count]
    pub all_pinned_pieces: Bitboard,
}

impl PinData {
    /// Get pin mask for a specific square
    pub fn get_pin_mask_for_square(&self, square: BoardSquare) -> Option<Bitboard> {
        // Check if this square has a pinned piece
        if (self.all_pinned_pieces & (1 << square)) == 0 {
            return None;
        }

        // Find the pin info for this square
        for piece_type in 0..2 {
            for i in 0..self.pin_counts[piece_type] {
                let pin_info = &self.pins[piece_type][i];
                if pin_info.pinned_piece_position == square {
                    return Some(pin_info.valid_move_squares);
                }
            }
        }

        None
    }
}

pub type PieceBoard = [Option<ColoredPiece>; 64];

const MAX_VALID_MOVES: usize = 256;

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

    pub halfmoves: usize,
    pub halfmoves_since_capture: usize,

    // store the move, which piece was there, and en-passant + castling flags
    // the flags can NOT be calculated as an arbitrary position can have those
    pub history: Vec<(BoardMove, Option<ColoredPiece>, u8, Bitboard)>,

    // store the zobrist key for the current position (computed iteratively)
    pub zobrist_key: u64,
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
            zobrist_key: 0,
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

                let square = BoardSquare::from_position(x as u8, 8 - y as u8 - 1);

                let color = if char.is_ascii_uppercase() {
                    Color::White
                } else {
                    Color::Black
                };

                match Piece::from_char(char.to_ascii_lowercase()) {
                    Some(piece) => game.set_piece(square, (piece, color)),
                    _ => {}
                }

                x += 1;
            }

            y += 1;
        }

        match parts.next() {
            Some("b") => game.update_turn(0), // to flip turn
            Some("w") => {}
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
        game.update_castling_flags(castling_flags);

        match parts.next() {
            Some("-") => {}
            Some(board_square_string) => match BoardSquare::parse(board_square_string) {
                Some(square) => game.update_en_passant_bitmap(square.to_mask()),
                _ => panic!("FEN parsing failure: incorrect En Passant target square"),
            },
            _ => panic!("FEN parsing failure: incorrect En Passant target square"),
        }

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
                let square = BoardSquare::from_position(x, rank);

                if let Some((piece, color)) = self.pieces[square as usize] {
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

        // DO NOT mess with this ordering, as FEN expects it this way
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
            fen.push_str(&self.en_passant_bitmap.next_index().unparse());
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

    fn unset_piece(&mut self, square: BoardSquare) {
        debug_assert!(self.pieces[square as usize].is_some());

        let (piece, color) = self.pieces[square as usize].unwrap();
        let mask = square.to_mask();

        self.piece_bitboards[piece as usize] &= !mask;
        self.color_bitboards[color as usize] &= !mask;
        self.pieces[square as usize] = None;

        self.zobrist_key ^= ZOBRIST.pieces[color as usize][piece as usize][square as usize];
    }

    fn set_piece(&mut self, square: BoardSquare, colored_piece @ (piece, color): ColoredPiece) {
        debug_assert!(self.pieces[square as usize].is_none());

        let mask = square.to_mask();

        self.piece_bitboards[piece as usize] |= mask;
        self.color_bitboards[color as usize] |= mask;
        self.pieces[square as usize] = Some(colored_piece);

        self.zobrist_key ^= ZOBRIST.pieces[color as usize][piece as usize][square as usize];
    }

    fn update_turn(&mut self, delta: isize) {
        self.side = !self.side;
        self.halfmoves = self.halfmoves.wrapping_add_signed(delta);

        self.zobrist_key ^= ZOBRIST.side_to_move;
    }

    fn update_castling_flags(&mut self, castling_flags: u8) {
        self.zobrist_key ^= ZOBRIST.castling[self.castling_flags as usize];
        self.castling_flags = castling_flags;
        self.zobrist_key ^= ZOBRIST.castling[castling_flags as usize];
    }

    fn update_en_passant_bitmap(&mut self, en_passant_bitmap: Bitboard) {
        // TODO: remove ifs with add/bit magic
        if self.en_passant_bitmap != 0 {
            let previous_column = self.en_passant_bitmap.next_index().get_x();
            self.zobrist_key ^= ZOBRIST.en_passant[previous_column as usize + 1];
        }

        self.en_passant_bitmap = en_passant_bitmap;

        if en_passant_bitmap != 0 {
            let new_column = en_passant_bitmap.next_index().get_x();
            self.zobrist_key ^= ZOBRIST.en_passant[new_column as usize + 1];
        }
    }

    ///
    /// Bitboards for all pieces.
    ///
    fn all_pieces_bitboard(&self) -> Bitboard {
        self.color_bitboards[0] | self.color_bitboards[1]
    }

    ///
    /// Bitboards for a piece of a given color.
    ///
    fn colored_piece_bitboard(&self, (piece, color): ColoredPiece) -> Bitboard {
        self.piece_bitboards[piece as usize] & self.color_bitboards[color as usize]
    }

    ///
    /// Undo a move.
    ///
    pub(crate) fn unmake_move(&mut self) {
        let (board_move, captured_piece, castling_flags, en_passant_bitmap) =
            self.history.pop().unwrap();

        let colored_piece @ (piece, color) = self.pieces[board_move.get_to() as usize].expect(
            "No piece at target square when unmaking a move. This should never ever happen.",
        );

        // move the piece back
        self.unset_piece(board_move.get_to());

        // if we promoted, make sure to unpromote
        self.set_piece(
            board_move.get_from(),
            match board_move.get_promotion() {
                None => colored_piece,
                Some(_) => (Piece::Pawn, color),
            },
        );

        // place the captured piece back
        if let Some(c_colored_piece) = captured_piece {
            self.set_piece(board_move.get_to(), c_colored_piece);
        }

        // restore bitmaps / flags
        self.update_castling_flags(castling_flags);
        self.update_en_passant_bitmap(en_passant_bitmap);

        // uncastle, if the king moved 2 spots; since we're indexing by rows, this should work
        if piece == Piece::King && board_move.get_from().abs_diff(board_move.get_to()) == 2 {
            self.set_piece(
                BoardSquare::from_position(
                    // bit hack: the to X position is either 2 (0b10) or 6 (0b110),
                    // so >> gives us a flag whether it's the first or last file
                    (board_move.get_to().get_x() >> 2) * 7,
                    board_move.get_from().get_y(),
                ),
                (Piece::Rook, color),
            );

            self.unset_piece((board_move.get_from() + board_move.get_to()) / 2);
        }

        // if pawn moves in a cross manner and doesn't capture piece, en-passant happened
        if piece == Piece::Pawn
            && captured_piece.is_none()
            && board_move.get_to().get_x() != board_move.get_from().get_x()
        {
            let captured_pawn_square = BoardSquare::from_position(
                board_move.get_to().get_x(),
                board_move.get_from().get_y(),
            );

            self.set_piece(captured_pawn_square, (Piece::Pawn, !color));
        }

        self.update_turn(-1);
    }

    ///
    /// Perform a board move; does NOT check the legality!
    ///
    pub(crate) fn make_move(&mut self, board_move: BoardMove) {
        let mut promoted = false;

        let (mut piece, color) = self.pieces[board_move.get_from() as usize]
            .expect("No piece at the source square while making a move.");

        let captured_piece = self.pieces[board_move.get_to() as usize];

        self.history.push((
            board_move,
            captured_piece,
            self.castling_flags,
            self.en_passant_bitmap,
        ));

        // remove captured piece
        if captured_piece.is_some() {
            self.unset_piece(board_move.get_to());

            // if we capture rooks, modify the flag of the side whose rook it was
            //
            // we kind of need to do this since FEN requires this, and even despite
            // there could be fuckery with taking rook, promoting and going back
            if let Some((Piece::Rook, c)) = captured_piece {
                let castling_flags = self.castling_flags
                    & !match (c, board_move.get_to()) {
                        (Color::Black, BoardSquare::H8) => 0b00000001,
                        (Color::Black, BoardSquare::A8) => 0b00000010,
                        (Color::White, BoardSquare::H1) => 0b00000100,
                        (Color::White, BoardSquare::A1) => 0b00001000,
                        _ => 0,
                    };

                self.update_castling_flags(castling_flags);
            }
        }

        // moves + promotions
        self.unset_piece(board_move.get_from());

        // if pawn reaches the last rank, promote!
        if piece == Piece::Pawn
            && (board_move.get_to().get_y() == 7 || board_move.get_to().get_y() == 0)
        {
            piece = board_move
                .get_promotion()
                .expect("Pawn move to last rank must contain promotion information.");

            promoted = true;
        }

        self.set_piece(board_move.get_to(), (piece, color));

        // if we moved to the en-passant bit, with a pawn, take
        if piece == Piece::Pawn && self.en_passant_bitmap.is_set(board_move.get_to()) {
            self.unset_piece(BoardSquare::from_position(
                board_move.get_to().get_x(),
                board_move.get_from().get_y(),
            ))
        }

        // set en-passant bit if pawn went two tiles (i.e. two full rows)
        self.update_en_passant_bitmap(
            if piece == Piece::Pawn && board_move.get_from().abs_diff(board_move.get_to()) == 16 {
                ((board_move.get_from() + board_move.get_to()) / 2).to_mask()
            } else {
                0
            },
        );

        // rook moves & castling
        if piece == Piece::Rook && !promoted {
            let castling_flags = self.castling_flags
                & !match (color, board_move.get_from()) {
                    (Color::Black, BoardSquare::H8) => 0b00000001,
                    (Color::Black, BoardSquare::A8) => 0b00000010,
                    (Color::White, BoardSquare::H1) => 0b00000100,
                    (Color::White, BoardSquare::A1) => 0b00001000,
                    _ => 0,
                };

            self.update_castling_flags(castling_flags);
        }

        // king moves
        if piece == Piece::King {
            // castling (move by 2)
            if board_move.get_from().abs_diff(board_move.get_to()) == 2 {
                self.unset_piece(BoardSquare::from_position(
                    // bit hack: the to X position is either 2 (0b10) or 6 (0b110),
                    // so >> gives us a flag whether it's the first or last file
                    (board_move.get_to().get_x() >> 2) * 7,
                    board_move.get_from().get_y(),
                ));

                self.set_piece(
                    (board_move.get_from() + board_move.get_to()) / 2,
                    (Piece::Rook, color),
                );
            }

            // either way no more castling for this side
            self.update_castling_flags(self.castling_flags & !(0b11 << (2 * color as usize)));
        }

        self.update_turn(1);
    }

    ///
    /// Retrieve a bitboard corresponding to what a sliding piece sees, given a square.
    /// If blockers is provided, it will be used instead (defaults to all pieces).
    ///
    fn get_occlusion_bitmap(
        &self,
        square: BoardSquare,
        piece: Piece,
        blockers: Option<Bitboard>,
    ) -> Bitboard {
        match piece {
            Piece::Queen => {
                // Queen moves like both rook and bishop, so combine their attack patterns
                let rook_attacks = self.get_occlusion_bitmap(square, Piece::Rook, blockers);
                let bishop_attacks = self.get_occlusion_bitmap(square, Piece::Bishop, blockers);
                rook_attacks | bishop_attacks
            }
            Piece::Rook | Piece::Bishop => {
                // Original logic for rook and bishop
                let (possible_blocker_positions_bitboard, offset) = match piece {
                    Piece::Rook => (MAGIC_ROOK_BLOCKER_BITBOARD[square as usize], 0),
                    Piece::Bishop => (MAGIC_BISHOP_BLOCKER_BITBOARD[square as usize], 64),
                    _ => unreachable!(),
                };

                let key = possible_blocker_positions_bitboard
                    & blockers.unwrap_or(self.all_pieces_bitboard());

                // then, given the magic table information...
                let (magic_number, table_offset, bit_offset) =
                    MAGIC_TABLE[offset + square as usize];

                // ... obtain calculate the blocker bitboard
                MAGIC_ENTRIES
                    [table_offset + (magic_number.wrapping_mul(key) >> bit_offset) as usize]
            }
            _ => unreachable!(),
        }
    }

    ///
    /// Retrieve attack moves for the piece at the particular square.
    ///
    fn get_piece_attack_bitboard(
        &self,
        square: BoardSquare,
        (piece, color): ColoredPiece,
    ) -> Bitboard {
        if piece == Piece::Pawn {
            let result = match color {
                // the & with the random number prevents 'loop around' attacks
                // where the pawn attacks on the other side of the board
                Color::White => {
                    ((1u64.wrapping_shl(square.wrapping_add(9) as u32)) & !0x0101010101010101)
                        | ((1u64.wrapping_shl(square.wrapping_add(7) as u32))
                            & !(0x0101010101010101 << 7))
                }
                Color::Black => {
                    ((1u64.wrapping_shl(square.wrapping_sub(9) as u32))
                        & !(0x0101010101010101 << 7))
                        | ((1u64.wrapping_shl(square.wrapping_sub(7) as u32)) & !0x0101010101010101)
                }
            };

            return result;
        }

        // use pre-calculated attack bitboards for other pieces
        let mut valid_moves = PIECE_MOVE_BITBOARDS[piece as usize][square as usize];

        match piece {
            // Slider stuff using magic bitmaps
            Piece::Bishop | Piece::Rook | Piece::Queen => {
                valid_moves &= self.get_occlusion_bitmap(square, piece, None);
            }
            _ => {}
        }

        valid_moves
    }

    ///
    /// Return true/false whether we can kingside/queenside castle for the particular color.
    /// The piece is king/queen for kingside/queenside castling.
    ///
    /// Note that this will return true when castling into a check, as these are checked later.
    ///
    fn can_castle(&self, piece: Piece, color: Color) -> bool {
        // return false immediately if we can't castle because of castling flags
        if match (piece, color) {
            (Piece::King, Color::Black) => 0b0001,
            (Piece::Queen, Color::Black) => 0b0010,
            (Piece::King, Color::White) => 0b0100,
            (Piece::Queen, Color::White) => 0b1000,
            _ => unreachable!(),
        } & self.castling_flags
            == 0
        {
            return false;
        }

        // we can only castle if there are no blocking pieces
        // this gives us the blockers in the castling row
        let castling_blockers = self.all_pieces_bitboard() >> (color as usize ^ 1) * 56;

        if match piece {
            Piece::Queen => 0b00001110,
            Piece::King => 0b01100000,
            _ => unreachable!(),
        } & castling_blockers
            != 0
        {
            return false;
        }

        // since we're already doing destination checks for king moves,
        // we don't need to check the final resulting position
        if match (piece, color) {
            (Piece::Queen, Color::Black) => self.is_square_attacked(BoardSquare::D8, !color),
            (Piece::King, Color::Black) => self.is_square_attacked(BoardSquare::F8, !color),
            (Piece::Queen, Color::White) => self.is_square_attacked(BoardSquare::D1, !color),
            (Piece::King, Color::White) => self.is_square_attacked(BoardSquare::F1, !color),
            _ => unreachable!(),
        } {
            return false;
        }

        true
    }

    ///
    /// Generate a bitboard that contains pseud-legal moves for a particular square,
    /// ignoring most safety-related rules and using the color of the piece as the
    /// current turn (i.e. ignoring `this.side`).
    ///
    fn get_pseudo_legal_move_bitboard(&self, square: BoardSquare) -> Bitboard {
        let colored_piece @ (piece, color) = self.pieces[square as usize].unwrap();

        // Obtain attack move bitboard, which are also regular moves for all but pawns
        let mut valid_moves = self.get_piece_attack_bitboard(square, colored_piece);

        match piece {
            // Pawn stuff
            Piece::Pawn => {
                // Attack moves only when there is enemy or en-passant
                valid_moves &= self.color_bitboards[!color as usize] | self.en_passant_bitmap;

                // regular moves (not into/through pieces)
                let forward_move = match color {
                    Color::White => 1 << (square + 8),
                    Color::Black => 1 << (square - 8),
                } & !self.all_pieces_bitboard();

                valid_moves |= forward_move;

                // if we can move forward, we can also try double forward
                if forward_move != 0 {
                    valid_moves |= match (color, square.get_y()) {
                        (Color::White, 1) => 1 << (square + 16) | 1 << (square + 8),
                        (Color::Black, 6) => 1 << (square - 16) | 1 << (square - 8),
                        _ => 0,
                    } & !self.all_pieces_bitboard()
                }
            }
            // King stuff
            Piece::King => {
                // only castle if not under attack
                if !self.is_square_attacked(
                    self.colored_piece_bitboard((Piece::King, color))
                        .next_index(),
                    !color,
                ) {
                    valid_moves |= ((self.can_castle(Piece::Queen, color) as u64) << 2
                        | (self.can_castle(Piece::King, color) as u64) << 6)
                        << ((color as usize ^ 1) * 56)
                }
            }
            _ => {}
        }

        // Can't ever capture / go through own pieces
        valid_moves &= !self.color_bitboards[color as usize];

        valid_moves
    }

    fn get_king_position(&self, color: Color) -> BoardSquare {
        self.colored_piece_bitboard((Piece::King, color))
            .next_index()
    }

    ///
    /// Return a bitboard of attackers for a particular position.
    ///
    fn get_attacked_from(&self, square: BoardSquare, color: Color) -> Bitboard {
        Piece::iter().fold(Bitboard::default(), |current, piece| {
            current
                | (self.get_piece_attack_bitboard(square, (piece, !color))
                    & self.colored_piece_bitboard((piece, color)))
        })
    }

    ///
    /// Check for the attack on a square by a particular color.
    ///
    fn is_square_attacked(&self, square: BoardSquare, color: Color) -> bool {
        // pawns are symmetric and other pieces don't care about color, so that's why we negate the color
        Piece::iter().any(|piece| {
            self.get_piece_attack_bitboard(square, (piece, !color))
                & self.colored_piece_bitboard((piece, color))
                != 0
        })
    }

    ///
    /// Return valid moves for a particular square, optionally masked by pin restrictions.
    ///
    pub fn get_square_pseudo_legal_moves(
        &self,
        square: BoardSquare,
        valid_move_mask: Option<Bitboard>,
    ) -> ValidMovesIterator {
        let mut bitboard = self.get_pseudo_legal_move_bitboard(square);

        // Apply pin mask if provided
        if let Some(mask) = valid_move_mask {
            bitboard &= mask;
        }

        // pawn on second to last rank must promote on the last rank
        let is_promoting = self.pieces[square as usize].is_some_and(|(p, c)| {
            p == Piece::Pawn
                && (square.get_y() == 6 && c == Color::White
                    || square.get_y() == 1 && c == Color::Black)
        });

        ValidMovesIterator {
            square: square.clone(),
            bitboard,
            is_promoting,
            current_promotion: 0,
        }
    }

    ///
    /// Retrieve pinning information for rook and bishop attacks.
    /// Returns structured data with pinned pieces and their valid move squares.
    ///
    pub fn get_pinner_bitboards(&self, color: Color) -> PinData {
        let king_position = self.get_king_position(color);

        let mut pin_data = PinData::default();

        // we're doing 3 calls to colored_piece_bitboard to get the ray/pieces
        //
        //  Atk        |   (3)
        //   |         |    |
        //  Pin   |    |    |
        //   |    |    |    |
        //  Kng  (1)  (2)   |
        //
        for piece in Piece::simple_sliders() {
            let piece_idx = piece as usize;

            // (1) get occlusions to get possible pinned pieces
            let raycast_1 = self.get_occlusion_bitmap(king_position, piece, None);

            // (2) subtract the calculated occlusions from blockers to get the blocked pieces
            let raycast_2 = self.get_occlusion_bitmap(
                king_position,
                piece,
                Some(self.all_pieces_bitboard() & !raycast_1),
            );

            // now we can get a bitmap of the attackers that are pinning things down
            // queen accounts for both rook and bishop so we count it both times
            let attacker_positions = (self.colored_piece_bitboard((piece, !color))
                | self.colored_piece_bitboard((Piece::Queen, !color)))
                & (raycast_2 & !raycast_1);

            // for each of these, calculate the pinned piece by casting back from the attacker
            for attacker_position in attacker_positions.iter_positions() {
                // (3) cast back from attacker to get the raycast
                let raycast_3 = self.get_occlusion_bitmap(
                    attacker_position,
                    piece,
                    Some(self.all_pieces_bitboard() & !raycast_1),
                );

                // only one pinned pieces
                debug_assert!(
                    (raycast_3 & raycast_1 & self.all_pieces_bitboard()).count_ones() == 1
                );

                let pinned_piece_position =
                    (raycast_3 & raycast_1 & self.all_pieces_bitboard()).next_index();

                let valid_positions = (raycast_2 & raycast_3) | (1 << attacker_position);

                let pin_info = PinInfo {
                    pinned_piece_position,
                    valid_move_squares: valid_positions,
                };

                // Add to all pinned pieces bitboard
                pin_data.all_pinned_pieces |= 1 << pinned_piece_position;

                // Add to appropriate array using piece value as index
                pin_data.pins[piece_idx][pin_data.pin_counts[piece_idx]] = pin_info;
                pin_data.pin_counts[piece_idx] += 1;
            }
        }

        pin_data
    }

    ///
    /// Retrieve attack information when the king is under exactly one attack.
    /// Returns the attack bitmap (ray from attacker to king) for blocking moves,
    /// plus the attacker position for capture moves.
    ///
    pub fn get_single_slider_attack_data(&self, position: BoardSquare, color: Color) -> Bitboard {
        // Check all slider pieces that could be attacking the king
        for piece in Piece::simple_sliders() {
            // (1) Raycast from king to find potential attackers
            let raycast_from_king = self.get_occlusion_bitmap(position, piece, None);

            // Find enemy sliders (including queens) that could attack along this ray
            let potential_attackers = (self.colored_piece_bitboard((piece, !color))
                | self.colored_piece_bitboard((Piece::Queen, !color)))
                & raycast_from_king;

            // Check each potential attacker
            for attacker_position in potential_attackers.iter_positions() {
                // (2) Raycast back from attacker to king with current board state
                let raycast_from_attacker = self.get_occlusion_bitmap(
                    attacker_position,
                    piece,
                    Some(self.all_pieces_bitboard()),
                );

                // Valid positions to block are the rays between the attacker and the king,
                // and the attacker position
                return (raycast_from_attacker & raycast_from_king) | attacker_position.to_mask();
            }
        }

        panic!("No slider attacks found for king");
    }

    ///
    /// Add moves that make the king evade an attack.
    /// Is not easy as it sounds, due to situations like these:
    ///
    ///    ...
    ///  R!!K!
    ///    ...
    ///
    /// When under an attack by a rook like this, it should not move back,
    /// even though the square behind the king is not under attack.
    fn add_king_evade_or_capture_moves(
        &self,
        king_position: BoardSquare,
        king_attacks: Bitboard,
        resulting_positions: &mut [BoardMove],
        count: &mut usize,
    ) {
        // we need to collect bitboards of attacking sliders, as king could otherwise
        // move "away" from them, which is technically a safe square
        let mut slider_attack_bitboard = Bitboard::default();

        for position in king_attacks.iter_positions() {
            let (piece, _) = self.pieces[position as usize].unwrap();

            if piece.is_slider() {
                slider_attack_bitboard |= self.get_occlusion_bitmap(
                    position,
                    piece,
                    Some(self.all_pieces_bitboard() & !king_position.to_mask()),
                )
            }
        }

        // now we can only move to spots that are not attacked by the sliders
        for board_move in
            self.get_square_pseudo_legal_moves(king_position, Some(!slider_attack_bitboard))
        {
            if !self.is_square_attacked(board_move.get_to(), !self.side) {
                resulting_positions[*count] = board_move;
                *count += 1;
            }
        }
    }

    ///
    /// Obtain a list of valid moves for the current position.
    ///
    pub fn get_moves(&self) -> ([BoardMove; MAX_VALID_MOVES], usize) {
        let mut resulting_positions = [BoardMove::default(); MAX_VALID_MOVES];
        let mut count = 0;

        let king_position = self.get_king_position(self.side);
        let king_attacks = self.get_attacked_from(king_position, !self.side);

        if king_attacks.count_ones() == 0 {
            // king is not under attack, so just move but not into a pin
            let pin_data = self.get_pinner_bitboards(self.side);

            // for non-king pieces, make them move
            for square in (self.color_bitboards[self.side as usize] & !king_position.to_mask())
                .iter_positions()
            {
                // Get pin mask for this square if it's pinned
                let pin_mask = pin_data.get_pin_mask_for_square(square);

                let (moving_piece, moving_color) = self.pieces[square as usize].unwrap();

                for board_move in self.get_square_pseudo_legal_moves(square, pin_mask) {
                    if moving_piece == Piece::Pawn
                        && self.en_passant_bitmap != 0
                        && board_move.get_to() == self.en_passant_bitmap.next_index()
                    {
                        // En passant capture - need to check for horizontal discovered attacks
                        let captured_pawn_square = BoardSquare::from_position(
                            board_move.get_to().get_x(),
                            square.get_y(), // Same rank as moving pawn
                        );

                        // Simulate the board state after en passant (both pawns removed)
                        let simulated_blockers = self.all_pieces_bitboard()
                            & !square.to_mask() // Remove moving pawn
                            & !captured_pawn_square.to_mask() // Remove captured pawn
                            | board_move.get_to().to_mask(); // Add pawn at destination

                        // Check if this exposes the king to a rook/queen attack along the rank
                        let rook_attacks = self.get_occlusion_bitmap(
                            king_position,
                            Piece::Rook,
                            Some(simulated_blockers),
                        );

                        // Check if any enemy rooks or queens can now attack the king
                        let enemy_rook_queens = (self
                            .colored_piece_bitboard((Piece::Rook, !moving_color))
                            | self.colored_piece_bitboard((Piece::Queen, !moving_color)))
                            & rook_attacks;

                        if enemy_rook_queens != 0 {
                            continue; // Skip this move as it would expose the king
                        }
                    }

                    resulting_positions[count] = board_move;
                    count += 1;
                }
            }

            // for king, just don't move into an attack
            for board_move in self.get_square_pseudo_legal_moves(king_position, None) {
                if !self.is_square_attacked(board_move.get_to(), !self.side) {
                    resulting_positions[count] = board_move;
                    count += 1;
                }
            }
        } else if king_attacks.count_ones() == 1 {
            // king is under one attack -- he can
            //  - block with an unpinned piece / take the attacker
            //  - evade
            let pin_data = self.get_pinner_bitboards(self.side);

            let attacking_position = king_attacks.next_index();
            let (attacking_piece, _) = self.pieces[attacking_position as usize].unwrap();

            // go through all non-king pieces
            for square in (self.color_bitboards[self.side as usize] & !king_position.to_mask())
                .iter_positions()
            {
                // Get pin mask for this square if it's pinned
                let pin_mask = pin_data.get_pin_mask_for_square(square);

                if !attacking_piece.is_slider() {
                    // If it's not a slider, all we can do is take it, so combine pins with its square
                    let (moving_piece, moving_color) = self.pieces[square as usize].unwrap();

                    // There is a special bullshit case where a pawn attacks a king and we can take it via en-passant
                    if attacking_piece == Piece::Pawn
                        && moving_piece == Piece::Pawn
                        && (self.en_passant_bitmap
                            & self.get_piece_attack_bitboard(square, (moving_piece, moving_color)))
                            & pin_mask.unwrap_or(!0)
                            != 0
                    {
                        resulting_positions[count] =
                            BoardMove::new(square, self.en_passant_bitmap.next_index(), None);
                        count += 1;
                        continue;
                    }

                    for board_move in self.get_square_pseudo_legal_moves(
                        square,
                        Some(attacking_position.to_mask() & pin_mask.unwrap_or(!0)),
                    ) {
                        resulting_positions[count] = board_move;
                        count += 1;
                    }
                } else {
                    // If it is a slider, we can either take, or block
                    let attack_data = self.get_single_slider_attack_data(king_position, self.side);

                    for board_move in self.get_square_pseudo_legal_moves(
                        square,
                        Some(attack_data & pin_mask.unwrap_or(!0)),
                    ) {
                        resulting_positions[count] = board_move;
                        count += 1;
                    }
                }
            }

            self.add_king_evade_or_capture_moves(
                king_position,
                king_attacks,
                &mut resulting_positions,
                &mut count,
            );
        } else {
            // can only evade if we have multiple attacks
            self.add_king_evade_or_capture_moves(
                king_position,
                king_attacks,
                &mut resulting_positions,
                &mut count,
            );
        }

        (resulting_positions, count)
    }
}
