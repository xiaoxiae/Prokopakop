use super::pieces::{Color, Piece};
use crate::game::ColoredPiece;
use crate::utils::Bitboard;
use crate::zobris::ZOBRIST;
use crate::{
    BitboardExt, BoardSquare, BoardSquareExt, MAGIC_BLOCKER_BITBOARD, MAGIC_ENTRIES, MAGIC_TABLE,
    PIECE_MOVE_BITBOARDS,
};
use strum::EnumCount;

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

pub struct PieceMovesIterator {
    bitboard: Bitboard,
    square: BoardSquare,
}

impl PieceMovesIterator {
    pub fn new(bitboard: Bitboard, square: BoardSquare) -> Self {
        Self { bitboard, square }
    }
}

impl Iterator for PieceMovesIterator {
    type Item = BoardMove;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bitboard == 0 {
            return None;
        }

        let to_square = self.bitboard.next_index();
        self.bitboard &= self.bitboard - 1;

        Some(BoardMove::new(self.square.clone(), to_square, None))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.bitboard.count_ones() as usize;
        (count, Some(count))
    }
}

pub struct PromotingMovesIterator {
    bitboard: Bitboard,
    square: BoardSquare,
    current_promotion: usize,
}

impl PromotingMovesIterator {
    pub fn new(bitboard: Bitboard, square: BoardSquare) -> Self {
        Self {
            bitboard,
            square,
            current_promotion: 0,
        }
    }
}

impl Iterator for PromotingMovesIterator {
    type Item = BoardMove;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bitboard == 0 {
            return None;
        }

        let to_square = self.bitboard.next_index();

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
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.bitboard.count_ones() as usize * 4; // 4 promotion pieces per square
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
    pub fn get_pin_mask_for_square(&self, square: BoardSquare) -> Bitboard {
        // Check if this square has a pinned piece
        if (self.all_pinned_pieces & (1 << square)) == 0 {
            return !0;
        }

        // Find the pin info for this square
        // TODO: this shit is very slow
        for piece_type in 0..2 {
            for i in 0..self.pin_counts[piece_type] {
                let pin_info = &self.pins[piece_type][i];
                if pin_info.pinned_piece_position == square {
                    return pin_info.valid_move_squares;
                }
            }
        }

        unreachable!()
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

pub trait ConstColor {
    const COLOR: Color;
    const OPPONENT: Color;
    const COLOR_INDEX: usize;
    const OPPONENT_INDEX: usize;

    type Opponent: ConstColor;
}

pub struct ConstWhite;
pub struct ConstBlack;

impl ConstColor for ConstWhite {
    const COLOR: Color = Color::White;
    const OPPONENT: Color = Color::Black;
    const COLOR_INDEX: usize = Color::White as usize;
    const OPPONENT_INDEX: usize = Color::Black as usize;

    type Opponent = ConstBlack;
}

impl ConstColor for ConstBlack {
    const COLOR: Color = Color::Black;
    const OPPONENT: Color = Color::White;
    const COLOR_INDEX: usize = Color::Black as usize;
    const OPPONENT_INDEX: usize = Color::White as usize;

    type Opponent = ConstWhite;
}

pub trait ConstPiece {
    const PIECE: Piece;
    const PIECE_INDEX: usize;
    const IS_SLIDER: bool;
    const IS_PAWN: bool;
    const IS_KING: bool;
}

macro_rules! impl_const_piece {
    ($struct_name:ident, $piece:expr) => {
        pub struct $struct_name;

        impl ConstPiece for $struct_name {
            const PIECE: Piece = $piece;
            const PIECE_INDEX: usize = $piece as usize;
            const IS_SLIDER: bool = matches!($piece, Piece::Bishop | Piece::Rook | Piece::Queen);
            const IS_PAWN: bool = matches!($piece, Piece::Pawn);
            const IS_KING: bool = matches!($piece, Piece::King);
        }
    };
}

impl_const_piece!(ConstPawn, Piece::Pawn);
impl_const_piece!(ConstKnight, Piece::Knight);
impl_const_piece!(ConstBishop, Piece::Bishop);
impl_const_piece!(ConstRook, Piece::Rook);
impl_const_piece!(ConstQueen, Piece::Queen);
impl_const_piece!(ConstKing, Piece::King);

macro_rules! dispatch_piece {
    ($piece:expr, $func:ident, $game:expr, $($args:expr),*) => {
        match $piece {
            Piece::Pawn => $game.$func::<ConstPawn>($($args),*),
            Piece::Knight => $game.$func::<ConstKnight>($($args),*),
            Piece::Bishop => $game.$func::<ConstBishop>($($args),*),
            Piece::Rook => $game.$func::<ConstRook>($($args),*),
            Piece::Queen => $game.$func::<ConstQueen>($($args),*),
            Piece::King => $game.$func::<ConstKing>($($args),*),
        }
    };
}

macro_rules! dispatch_slider {
    ($piece:expr, $func:ident, $game:expr, $($args:expr),*) => {
        match $piece {
            Piece::Bishop => $game.$func::<ConstBishop>($($args),*),
            Piece::Rook => $game.$func::<ConstRook>($($args),*),
            Piece::Queen => $game.$func::<ConstQueen>($($args),*),
            _ => unreachable!()
        }
    };
}

macro_rules! dispatch_piece_color {
    ($piece:expr, $color:expr, $func:ident, $game:expr, $($args:expr),*) => {
        match ($piece, $color) {
            (Piece::Pawn, Color::White) => $game.$func::<ConstPawn, ConstWhite>($($args),*),
            (Piece::Pawn, Color::Black) => $game.$func::<ConstPawn, ConstBlack>($($args),*),
            (Piece::Knight, Color::White) => $game.$func::<ConstKnight, ConstWhite>($($args),*),
            (Piece::Knight, Color::Black) => $game.$func::<ConstKnight, ConstBlack>($($args),*),
            (Piece::Bishop, Color::White) => $game.$func::<ConstBishop, ConstWhite>($($args),*),
            (Piece::Bishop, Color::Black) => $game.$func::<ConstBishop, ConstBlack>($($args),*),
            (Piece::Rook, Color::White) => $game.$func::<ConstRook, ConstWhite>($($args),*),
            (Piece::Rook, Color::Black) => $game.$func::<ConstRook, ConstBlack>($($args),*),
            (Piece::Queen, Color::White) => $game.$func::<ConstQueen, ConstWhite>($($args),*),
            (Piece::Queen, Color::Black) => $game.$func::<ConstQueen, ConstBlack>($($args),*),
            (Piece::King, Color::White) => $game.$func::<ConstKing, ConstWhite>($($args),*),
            (Piece::King, Color::Black) => $game.$func::<ConstKing, ConstBlack>($($args),*),
        }
    };
}

macro_rules! for_each_simple_slider {
    // syntax: for_each_simple_slider!(|Type, PIECE| { ... })
    (|$T:ident, $piece:ident| $body:block) => {{
        {
            type $T = ConstRook;
            const $piece: Piece = Piece::Rook;
            $body
        }
        {
            type $T = ConstBishop;
            const $piece: Piece = Piece::Bishop;
            $body
        }
    }};
}

#[derive(Debug)]
pub struct Game {
    pub side: Color,

    pub pieces: PieceBoard,

    pub castling_flags: u8, // 0x0000KQkq, where kq/KQ is one if black/white king and queen
    pub en_passant_bitmap: Bitboard, // if a piece just moved for the first time, 1 will be over the square

    pub color_bitboards: [Bitboard; Color::COUNT],
    pub piece_bitboards: [Bitboard; Piece::COUNT],

    pub all_pieces: Bitboard,

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
            all_pieces: Bitboard::default(),
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

        self.all_pieces = self.color_bitboards[Color::White as usize]
            | self.color_bitboards[Color::Black as usize];

        self.zobrist_key ^= ZOBRIST.pieces[color as usize][piece as usize][square as usize];
    }

    fn set_piece(&mut self, square: BoardSquare, colored_piece @ (piece, color): ColoredPiece) {
        let mask = square.to_mask();

        self.piece_bitboards[piece as usize] |= mask;
        self.color_bitboards[color as usize] |= mask;

        self.pieces[square as usize] = Some(colored_piece);

        self.all_pieces = self.color_bitboards[Color::White as usize]
            | self.color_bitboards[Color::Black as usize];

        self.zobrist_key ^= ZOBRIST.pieces[color as usize][piece as usize][square as usize];
    }

    fn set_piece_const<P: ConstPiece, C: ConstColor>(&mut self, square: BoardSquare) {
        debug_assert!(self.pieces[square as usize].is_none());

        let mask = square.to_mask();

        self.piece_bitboards[P::PIECE_INDEX] |= mask;
        self.color_bitboards[C::COLOR_INDEX] |= mask;

        self.pieces[square as usize] = Some((P::PIECE, C::COLOR));

        self.all_pieces = self.color_bitboards[Color::White as usize]
            | self.color_bitboards[Color::Black as usize];

        self.zobrist_key ^= ZOBRIST.pieces[C::COLOR_INDEX][P::PIECE_INDEX][square as usize];
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
    /// Bitboards for a piece of a given color.
    ///
    pub fn colored_piece_bitboard_const<P: ConstPiece, C: ConstColor>(&self) -> Bitboard {
        self.piece_bitboards[P::PIECE_INDEX] & self.color_bitboards[C::COLOR_INDEX]
    }

    ///
    /// Undo a move.
    ///
    pub(crate) fn unmake_move(&mut self) {
        let (board_move, captured_piece, castling_flags, en_passant_bitmap) =
            self.history.pop().unwrap();

        let (piece, color) = self.pieces[board_move.get_to() as usize].expect(
            "No piece at target square when unmaking a move. This should never ever happen.",
        );

        dispatch_piece_color!(
            piece,
            color,
            unmake_move_const,
            self,
            board_move,
            captured_piece,
            castling_flags,
            en_passant_bitmap
        );
    }

    fn unmake_move_const<P: ConstPiece, C: ConstColor>(
        &mut self,
        board_move: BoardMove,
        captured_piece: Option<(Piece, Color)>,
        castling_flags: u8,
        en_passant_bitmap: u64,
    ) {
        // move the piece back
        self.unset_piece(board_move.get_to());

        // if we promoted, make sure to unpromote
        self.set_piece(
            board_move.get_from(),
            match board_move.get_promotion() {
                None => (P::PIECE, C::COLOR),
                Some(_) => (Piece::Pawn, C::COLOR),
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
        if P::PIECE == Piece::King && board_move.get_from().abs_diff(board_move.get_to()) == 2 {
            self.set_piece_const::<ConstRook, C>(BoardSquare::from_position(
                // bit hack: the to X position is either 2 (0b10) or 6 (0b110),
                // so >> gives us a flag whether it's the first or last file
                (board_move.get_to().get_x() >> 2) * 7,
                board_move.get_from().get_y(),
            ));

            self.unset_piece((board_move.get_from() + board_move.get_to()) / 2);
        }

        // if pawn moves in a cross manner and doesn't capture piece, en-passant happened
        if P::PIECE == Piece::Pawn
            && captured_piece.is_none()
            && board_move.get_to().get_x() != board_move.get_from().get_x()
        {
            let captured_pawn_square = BoardSquare::from_position(
                board_move.get_to().get_x(),
                board_move.get_from().get_y(),
            );

            self.set_piece_const::<ConstPawn, C::Opponent>(captured_pawn_square);
        }

        self.update_turn(-1);
    }

    ///
    /// Perform a board move; does NOT check the legality!
    ///
    pub(crate) fn make_move(&mut self, board_move: BoardMove) {
        let (piece, color) = self.pieces[board_move.get_from() as usize]
            .expect("No piece at the source square while making a move.");

        dispatch_piece_color!(piece, color, make_move_const, self, board_move);
    }

    fn make_move_const<P: ConstPiece, C: ConstColor>(&mut self, board_move: BoardMove) {
        let captured_piece = self.pieces[board_move.get_to() as usize];

        self.history.push((
            board_move,
            captured_piece,
            self.castling_flags,
            self.en_passant_bitmap,
        ));

        // remove captured piece
        if let Some((captured, captured_color)) = captured_piece {
            self.unset_piece(board_move.get_to());

            if captured == Piece::Rook {
                let castling_flags = self.castling_flags
                    & !match (captured_color, board_move.get_to()) {
                        (Color::Black, BoardSquare::H8) => 0b00000001,
                        (Color::Black, BoardSquare::A8) => 0b00000010,
                        (Color::White, BoardSquare::H1) => 0b00000100,
                        (Color::White, BoardSquare::A1) => 0b00001000,
                        _ => 0,
                    };
                self.update_castling_flags(castling_flags);
            }
        }

        // remove moving piece
        self.unset_piece(board_move.get_from());

        if P::PIECE == Piece::Pawn {
            // handle promotion
            let mut final_piece = P::PIECE;
            if P::PIECE == Piece::Pawn
                && (board_move.get_to().get_y() == 7 || board_move.get_to().get_y() == 0)
            {
                final_piece = board_move
                    .get_promotion()
                    .expect("Pawn move to last rank must contain promotion information.");
            }

            self.set_piece(board_move.get_to(), (final_piece, C::COLOR));
        } else {
            self.set_piece_const::<P, C>(board_move.get_to());
        }

        // en-passant capture
        if P::PIECE == Piece::Pawn && self.en_passant_bitmap.is_set(board_move.get_to()) {
            self.unset_piece(BoardSquare::from_position(
                board_move.get_to().get_x(),
                board_move.get_from().get_y(),
            ))
        }

        // en-passant mark
        if P::PIECE == Piece::Pawn {
            self.update_en_passant_bitmap(
                if board_move.get_from().abs_diff(board_move.get_to()) == 16 {
                    ((board_move.get_from() + board_move.get_to()) / 2).to_mask()
                } else {
                    0
                },
            );
        } else {
            self.update_en_passant_bitmap(0);
        }

        // rook → update castling rights
        if P::PIECE == Piece::Rook {
            let castling_flags = self.castling_flags
                & !match (C::COLOR, board_move.get_from()) {
                    (Color::Black, BoardSquare::H8) => 0b00000001,
                    (Color::Black, BoardSquare::A8) => 0b00000010,
                    (Color::White, BoardSquare::H1) => 0b00000100,
                    (Color::White, BoardSquare::A1) => 0b00001000,
                    _ => 0,
                };
            self.update_castling_flags(castling_flags);
        }

        // king special moves
        if P::PIECE == Piece::King {
            if board_move.get_from().abs_diff(board_move.get_to()) == 2 {
                self.unset_piece(BoardSquare::from_position(
                    (board_move.get_to().get_x() >> 2) * 7,
                    board_move.get_from().get_y(),
                ));
                self.set_piece_const::<ConstRook, C>(
                    (board_move.get_from() + board_move.get_to()) / 2,
                );
            }
            self.update_castling_flags(self.castling_flags & !(0b11 << (2 * C::COLOR_INDEX)));
        }

        self.update_turn(1);
    }

    ///
    /// Uses compile-time dispatch based on piece type for better performance.
    ///
    fn get_occlusion_bitmap_const<P: ConstPiece>(
        &self,
        square: BoardSquare,
        blockers: Bitboard,
    ) -> Bitboard {
        match P::PIECE {
            Piece::Queen => {
                // Queen moves like both rook and bishop, so combine their attack patterns
                let rook_attacks = self.get_occlusion_bitmap_const::<ConstRook>(square, blockers);
                let bishop_attacks =
                    self.get_occlusion_bitmap_const::<ConstBishop>(square, blockers);
                rook_attacks | bishop_attacks
            }
            Piece::Rook | Piece::Bishop => {
                let key = MAGIC_BLOCKER_BITBOARD[P::PIECE_INDEX * 64 + square as usize] & blockers;

                let (magic_number, table_offset, bit_offset) =
                    MAGIC_TABLE[P::PIECE_INDEX * 64 + square as usize];

                MAGIC_ENTRIES
                    [table_offset + (magic_number.wrapping_mul(key) >> bit_offset) as usize]
            }
            _ => 0,
        }
    }

    ///
    /// Uses compile-time dispatch for both piece type and color.
    ///
    fn get_piece_attack_bitboard_const<P: ConstPiece, C: ConstColor>(
        &self,
        square: BoardSquare,
    ) -> Bitboard {
        if P::IS_PAWN {
            // Compile-time pawn attack calculation based on color
            match C::COLOR {
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
            }
        } else {
            // Use pre-calculated attack bitboards for other pieces
            let mut valid_moves = PIECE_MOVE_BITBOARDS[P::PIECE_INDEX][square as usize];

            if P::IS_SLIDER {
                // Apply magic bitboard occlusion for sliding pieces
                valid_moves &= self.get_occlusion_bitmap_const::<P>(square, self.all_pieces);
            }

            valid_moves
        }
    }

    ///
    /// Returns a bitboard with valid castling squares for the given color.
    /// Note: This doesn't check if castling into check, as that's handled elsewhere.
    ///
    fn get_castling_bitboard_const<C: ConstColor>(&self) -> Bitboard {
        // Check castling flags at compile time
        let kingside_flag = if C::COLOR == Color::White {
            0b0100
        } else {
            0b0001
        };
        let queenside_flag = if C::COLOR == Color::White {
            0b1000
        } else {
            0b0010
        };

        let can_kingside = (self.castling_flags & kingside_flag) != 0;
        let can_queenside = (self.castling_flags & queenside_flag) != 0;

        if !can_kingside && !can_queenside {
            return 0;
        }

        // Get blockers in the castling row
        let castling_blockers = self.all_pieces >> (C::OPPONENT_INDEX * 56);

        let mut castling_moves = 0;

        // Check kingside castling
        if can_kingside && (castling_blockers & 0b01100000) == 0 {
            // Check if the intermediate square is attacked
            let intermediate_square = if C::COLOR == Color::White {
                BoardSquare::F1
            } else {
                BoardSquare::F8
            };

            if !self.is_square_attacked_const::<C::Opponent>(intermediate_square) {
                castling_moves |= 1 << 6; // G file
            }
        }

        // Check queenside castling
        if can_queenside && (castling_blockers & 0b00001110) == 0 {
            // Check if the intermediate square is attacked
            let intermediate_square = if C::COLOR == Color::White {
                BoardSquare::D1
            } else {
                BoardSquare::D8
            };

            if !self.is_square_attacked_const::<C::Opponent>(intermediate_square) {
                castling_moves |= 1 << 2; // C file
            }
        }

        // Shift to the correct rank
        castling_moves << (C::OPPONENT_INDEX * 56)
    }

    ///
    /// Generate a bitboard that contains pseud-legal moves for a particular square,
    /// ignoring most safety-related rules and using the color of the piece as the
    /// current turn (i.e. ignoring `this.side`).
    ///
    fn get_pseudo_legal_move_bitboard(&self, square: BoardSquare) -> Bitboard {
        let (piece, color) = self.pieces[square as usize].unwrap();

        return dispatch_piece_color!(
            piece,
            color,
            get_pseudo_legal_move_bitboard_const,
            self,
            square
        );
    }

    ///
    /// Generate a bitboard that contains pseud-legal moves for a particular square,
    /// ignoring most safety-related rules and using the color of the piece as the
    /// current turn (i.e. ignoring `this.side`).
    ///
    fn get_pseudo_legal_move_bitboard_const<P: ConstPiece, C: ConstColor>(
        &self,
        square: BoardSquare,
    ) -> Bitboard {
        // Get attack moves (which are also regular moves for all but pawns)
        let mut valid_moves = self.get_piece_attack_bitboard_const::<P, C>(square);

        if P::IS_PAWN {
            // Attack moves only when there is enemy or en-passant
            valid_moves &= self.color_bitboards[C::OPPONENT_INDEX] | self.en_passant_bitmap;

            // Regular forward moves (not into/through pieces)
            let forward_move = if C::COLOR == Color::White {
                1 << (square + 8)
            } else {
                1 << (square - 8)
            } & !self.all_pieces;

            valid_moves |= forward_move;

            // Double forward move from starting position
            if forward_move != 0 {
                let starting_rank = if C::COLOR == Color::White { 1 } else { 6 };
                if square.get_y() == starting_rank {
                    let double_forward = if C::COLOR == Color::White {
                        1 << (square + 16)
                    } else {
                        1 << (square - 16)
                    } & !self.all_pieces;

                    valid_moves |= double_forward;
                }
            }
        } else if P::IS_KING {
            // Only add castling if king is not under attack
            let king_position = self
                .colored_piece_bitboard_const::<ConstKing, C>()
                .next_index();

            if !self.is_square_attacked_const::<C::Opponent>(king_position) {
                valid_moves |= self.get_castling_bitboard_const::<C>();
            }
        }

        // Can't capture or move through own pieces
        valid_moves &= !self.color_bitboards[C::COLOR_INDEX];

        valid_moves
    }

    ///
    /// Returns the position of the king of a given color.
    ///
    fn get_king_position_const<C: ConstColor>(&self) -> BoardSquare {
        self.colored_piece_bitboard_const::<ConstKing, C>()
            .next_index()
    }

    ///
    /// Returns a bitboard of all pieces of the given color that can attack the square.
    ///
    fn get_attacked_from_const<C: ConstColor>(&self, square: BoardSquare) -> Bitboard {
        let pawn_attackers = self.get_piece_attack_bitboard_const::<ConstPawn, C::Opponent>(square)
            & self.colored_piece_bitboard_const::<ConstPawn, C>();

        let knight_attackers = self
            .get_piece_attack_bitboard_const::<ConstKnight, C::Opponent>(square)
            & self.colored_piece_bitboard_const::<ConstKnight, C>();

        let bishop_attackers = self
            .get_piece_attack_bitboard_const::<ConstBishop, C::Opponent>(square)
            & self.colored_piece_bitboard_const::<ConstBishop, C>();

        let rook_attackers = self.get_piece_attack_bitboard_const::<ConstRook, C::Opponent>(square)
            & self.colored_piece_bitboard_const::<ConstRook, C>();

        let queen_attackers = self
            .get_piece_attack_bitboard_const::<ConstQueen, C::Opponent>(square)
            & self.colored_piece_bitboard_const::<ConstQueen, C>();

        let king_attackers = self.get_piece_attack_bitboard_const::<ConstKing, C::Opponent>(square)
            & self.colored_piece_bitboard_const::<ConstKing, C>();

        pawn_attackers
            | knight_attackers
            | bishop_attackers
            | rook_attackers
            | queen_attackers
            | king_attackers
    }

    ///
    /// Check for the attack on a square by a particular color.
    ///
    fn is_square_attacked_const<C: ConstColor>(&self, square: BoardSquare) -> bool {
        self.get_attacked_from_const::<C>(square) != 0
    }

    ///
    /// Return valid moves for a particular square, masked by a bitmap.
    ///
    pub fn get_square_pseudo_legal_moves(
        &self,
        square: BoardSquare,
        valid_move_mask: Bitboard,
    ) -> ValidMovesIterator {
        let mut bitboard = self.get_pseudo_legal_move_bitboard(square);
        bitboard &= valid_move_mask;

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
    pub fn get_pinner_bitboards_const<C: ConstColor>(&self) -> PinData {
        let king_position = self.get_king_position_const::<C>();

        let mut pin_data = PinData::default();

        // we're doing 3 calls to colored_piece_bitboard to get the ray/pieces
        //
        //  Atk        |   (3)
        //   |         |    |
        //  Pin   |    |    |
        //   |    |    |    |
        //  Kng  (1)  (2)   |
        //
        for_each_simple_slider!(|P, piece| {
            // (1) get occlusions to get possible pinned pieces
            let raycast_1 = self.get_occlusion_bitmap_const::<P>(king_position, self.all_pieces);

            // (2) subtract the calculated occlusions from blockers to get the blocked pieces
            let raycast_2 =
                self.get_occlusion_bitmap_const::<P>(king_position, self.all_pieces & !raycast_1);

            // now we can get a bitmap of the attackers that are pinning things down
            // queen accounts for both rook and bishop so we count it both times
            let attacker_positions = (self.colored_piece_bitboard_const::<P, C::Opponent>()
                | self.colored_piece_bitboard_const::<ConstQueen, C::Opponent>())
                & (raycast_2 & !raycast_1);

            // for each of these, calculate the pinned piece by casting back from the attacker
            for attacker_position in attacker_positions.iter_positions() {
                // (3) cast back from attacker to get the raycast
                let raycast_3 = self.get_occlusion_bitmap_const::<P>(
                    attacker_position,
                    self.all_pieces & !raycast_1,
                );

                // only one pinned pieces
                debug_assert!((raycast_3 & raycast_1 & self.all_pieces).count_ones() == 1);

                let pinned_piece_position = (raycast_3 & raycast_1 & self.all_pieces).next_index();

                let valid_positions = (raycast_2 & raycast_3) | (1 << attacker_position);

                let pin_info = PinInfo {
                    pinned_piece_position,
                    valid_move_squares: valid_positions,
                };

                // Add to all pinned pieces bitboard
                pin_data.all_pinned_pieces |= 1 << pinned_piece_position;

                // Add to appropriate array using piece value as index
                pin_data.pins[P::PIECE_INDEX][pin_data.pin_counts[P::PIECE_INDEX]] = pin_info;
                pin_data.pin_counts[P::PIECE_INDEX] += 1;
            }
        });

        pin_data
    }

    ///
    /// Retrieve attack information when the king is under exactly one attack.
    /// Returns the attack bitmap (ray from attacker to king) for blocking moves,
    /// plus the attacker position for capture moves.
    /// Const generic version for compile-time color optimization.
    ///
    pub fn get_single_slider_attack_data_const<C: ConstColor>(
        &self,
        position: BoardSquare,
    ) -> Bitboard {
        // Check all slider pieces that could be attacking the king
        for_each_simple_slider!(|P, piece| {
            // (1) Raycast from king to find potential attackers
            let raycast_from_king = self.get_occlusion_bitmap_const::<P>(position, self.all_pieces);

            // Find enemy sliders (including queens) that could attack along this ray
            let potential_attackers = (self.colored_piece_bitboard_const::<P, C::Opponent>()
                | self.colored_piece_bitboard_const::<ConstQueen, C::Opponent>())
                & raycast_from_king;

            // Check each potential attacker
            for attacker_position in potential_attackers.iter_positions() {
                // (2) Raycast back from attacker to king with current board state
                let raycast_from_attacker =
                    self.get_occlusion_bitmap_const::<P>(attacker_position, self.all_pieces);

                // Valid positions to block are the rays between the attacker and the king,
                // and the attacker position
                return (raycast_from_attacker & raycast_from_king) | attacker_position.to_mask();
            }
        });

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
    /// Const generic version for compile-time color optimization.
    ///
    fn add_king_evade_or_capture_moves_const<C: ConstColor>(
        &self,
        king_position: BoardSquare,
        attacked_from_bitboard: Bitboard,
        moves: &mut Vec<BoardMove>,
    ) {
        // we need to collect bitboards of attacking sliders, as king could otherwise
        // move "away" from them, which is technically a safe square
        let mut slider_attack_bitboard = Bitboard::default();

        // TODO: unwrap with piece bitmaps
        for position in attacked_from_bitboard.iter_positions() {
            let (piece, _) = self.pieces[position as usize].unwrap();

            slider_attack_bitboard |= dispatch_piece!(
                piece,
                get_occlusion_bitmap_const,
                self,
                position,
                self.all_pieces & !king_position.to_mask()
            );
        }

        let legal_move_bitboard = self
            .get_pseudo_legal_move_bitboard_const::<ConstKing, C>(king_position)
            & !slider_attack_bitboard;

        for board_move in PieceMovesIterator::new(legal_move_bitboard, king_position) {
            // now we can only move to spots that are not attacked by the sliders
            if !self.is_square_attacked_const::<C::Opponent>(board_move.get_to()) {
                moves.push(board_move);
            }
        }
    }

    ///
    /// Obtain a list of valid moves for the current position.
    /// Const generic version for compile-time color optimization.
    ///
    pub fn get_moves_const<C: ConstColor>(&self) -> Vec<BoardMove> {
        let mut moves = Vec::with_capacity(64); // Most positions have < 40 moves

        let king_position = self.get_king_position_const::<C>();
        let king_attacks = self.get_attacked_from_const::<C::Opponent>(king_position);

        if king_attacks.count_ones() == 0 {
            // king is not under attack, so just move but not into a pin
            let pin_data = self.get_pinner_bitboards_const::<C>();

            // for non-king pieces, make them move
            for square in (self.color_bitboards[C::COLOR as usize] & !king_position.to_mask())
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
                        let simulated_blockers = self.all_pieces
                            & !square.to_mask() // Remove moving pawn
                            & !captured_pawn_square.to_mask() // Remove captured pawn
                            | board_move.get_to().to_mask(); // Add pawn at destination

                        // Check if this exposes the king to a rook/queen attack along the rank
                        let rook_attacks = self.get_occlusion_bitmap_const::<ConstRook>(
                            king_position,
                            simulated_blockers,
                        );

                        // Check if any enemy rooks or queens can now attack the king
                        let enemy_rook_queens = (self
                            .colored_piece_bitboard_const::<ConstRook, C::Opponent>()
                            | self.colored_piece_bitboard_const::<ConstQueen, C::Opponent>())
                            & rook_attacks;

                        if enemy_rook_queens != 0 {
                            continue; // Skip this move as it would expose the king
                        }
                    }

                    moves.push(board_move);
                }
            }

            // for king, just don't move into an attack
            for board_move in self.get_square_pseudo_legal_moves(king_position, !0) {
                if !self.is_square_attacked_const::<C::Opponent>(board_move.get_to()) {
                    moves.push(board_move);
                }
            }
        } else if king_attacks.count_ones() == 1 {
            // king is under one attack -- he can
            //  - block with an unpinned piece / take the attacker
            //  - evade
            let pin_data = self.get_pinner_bitboards_const::<C>();

            let attacking_position = king_attacks.next_index();
            let (attacking_piece, _) = self.pieces[attacking_position as usize].unwrap();

            // go through all non-king pieces
            for square in
                (self.color_bitboards[C::COLOR_INDEX] & !king_position.to_mask()).iter_positions()
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
                            & self.get_piece_attack_bitboard_const::<ConstPawn, C>(square))
                            & pin_mask
                            != 0
                    {
                        moves.push(BoardMove::new(
                            square,
                            self.en_passant_bitmap.next_index(),
                            None,
                        ));
                        continue;
                    }

                    for board_move in self.get_square_pseudo_legal_moves(
                        square,
                        attacking_position.to_mask() & pin_mask,
                    ) {
                        moves.push(board_move);
                    }
                } else {
                    // If it is a slider, we can either take, or block
                    let attack_data = self.get_single_slider_attack_data_const::<C>(king_position);

                    for board_move in
                        self.get_square_pseudo_legal_moves(square, attack_data & pin_mask)
                    {
                        moves.push(board_move);
                    }
                }
            }

            self.add_king_evade_or_capture_moves_const::<C>(
                king_position,
                king_attacks,
                &mut moves,
            );
        } else {
            // can only evade if we have multiple attacks
            self.add_king_evade_or_capture_moves_const::<C>(
                king_position,
                king_attacks,
                &mut moves,
            );
        }

        moves
    }

    pub fn get_moves(&self) -> Vec<BoardMove> {
        match self.side {
            Color::White => self.get_moves_const::<ConstWhite>(),
            Color::Black => self.get_moves_const::<ConstBlack>(),
        }
    }
}
