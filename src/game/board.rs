use super::pieces::{Color, Piece};
use crate::game::evaluate::{
    PIECE_VALUES, calculate_game_phase, evaluate_bishop_pair, evaluate_king_safety,
    evaluate_material, evaluate_mobility, evaluate_positional,
};
use crate::game::pieces::ColoredPiece;
use crate::utils::bitboard::{
    BLACK_PROMOTION_ROW, Bitboard, BitboardExt, MAGIC_BLOCKER_BITBOARD, PIECE_MOVE_BITBOARDS,
    RAY_BETWEEN, WHITE_PROMOTION_ROW,
};
use crate::utils::magic::{MAGIC_ENTRIES, MAGIC_TABLE};
use crate::utils::square::{BoardSquare, BoardSquareExt};
use crate::utils::zobris::ZOBRIST_TABLE;
use strum::EnumCount;

pub(crate) type BoardMove = u16;

pub(crate) trait BoardMoveExt {
    fn empty() -> BoardMove;
    fn new(from: BoardSquare, to: BoardSquare, promotion: Option<Piece>) -> BoardMove;
    fn regular(from: BoardSquare, to: BoardSquare) -> BoardMove;
    fn promoting(from: BoardSquare, to: BoardSquare, promotion: Piece) -> BoardMove;
    fn get_from(&self) -> BoardSquare;
    fn get_to(&self) -> BoardSquare;
    fn get_promotion(&self) -> Option<Piece>;
    fn parse(string: &str) -> Option<BoardMove>;

    #[allow(dead_code)]
    fn unparse(&self) -> String;
}

impl BoardMoveExt for u16 {
    fn empty() -> BoardMove {
        0
    }

    fn new(from: BoardSquare, to: BoardSquare, promotion: Option<Piece>) -> BoardMove {
        (from as u16)
            | ((to as u16) << 6)
            | ((promotion
                .and_then(|p| Some(1 << (p as u16)))
                .unwrap_or_default())
                << 12)
    }

    fn regular(from: BoardSquare, to: BoardSquare) -> BoardMove {
        (from as u16) | ((to as u16) << 6)
    }

    fn promoting(from: BoardSquare, to: BoardSquare, promotion: Piece) -> BoardMove {
        (from as u16) | ((to as u16) << 6) | ((1 << (promotion as u16)) << 12)
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

#[derive(Debug, Clone)]
struct PinData {
    pub pinned_pieces: Bitboard,
    pub pinner_squares: [BoardSquare; 64],
}

impl PinData {
    fn new() -> Self {
        Self {
            pinned_pieces: 0,
            pinner_squares: [BoardSquare::default(); 64],
        }
    }

    pub fn add_pin(&mut self, pinned_square: BoardSquare, pinner_square: BoardSquare) {
        self.pinned_pieces |= 1 << pinned_square;
        self.pinner_squares[pinned_square as usize] = pinner_square as u8;
    }

    pub fn get_pin_mask_for_square(
        &self,
        square: BoardSquare,
        king_position: BoardSquare,
    ) -> Bitboard {
        if !self.pinned_pieces.is_set(square) {
            // all squares are allowed
            return !Bitboard::default();
        }

        // otherwise it's ray between + the piece
        let pinner_square = self.pinner_squares[square as usize];
        let ray = RAY_BETWEEN[pinner_square as usize][king_position as usize];

        ray | pinner_square.to_mask()
    }
}

type PieceBoard = [Option<ColoredPiece>; 64];

#[allow(dead_code)]
trait ConstColor {
    const COLOR: Color;
    const OPPONENT: Color;
    const COLOR_INDEX: usize;
    const OPPONENT_INDEX: usize;

    type Opponent: ConstColor;
}

struct ConstWhite;
struct ConstBlack;

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

trait ConstPiece {
    const PIECE: Piece;
    const PIECE_INDEX: usize;
}

macro_rules! impl_const_piece {
    ($struct_name:ident, $piece:expr) => {
        pub struct $struct_name;

        impl ConstPiece for $struct_name {
            const PIECE: Piece = $piece;
            const PIECE_INDEX: usize = $piece as usize;
        }
    };
}

impl_const_piece!(ConstPawn, Piece::Pawn);
impl_const_piece!(ConstKnight, Piece::Knight);
impl_const_piece!(ConstBishop, Piece::Bishop);
impl_const_piece!(ConstRook, Piece::Rook);
impl_const_piece!(ConstQueen, Piece::Queen);
impl_const_piece!(ConstKing, Piece::King);

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
    (|$T:ident| $body:block) => {{
        {
            type $T = ConstRook;
            $body
        }
        {
            type $T = ConstBishop;
            $body
        }
    }};
}

macro_rules! for_each_non_king_const_piece {
    (|$T:ident| $body:block) => {{
        {
            type $T = ConstPawn;
            $body
        }
        {
            type $T = ConstKnight;
            $body
        }
        {
            type $T = ConstBishop;
            $body
        }
        {
            type $T = ConstRook;
            $body
        }
        {
            type $T = ConstQueen;
            $body
        }
    }};
}

#[derive(Debug, Clone)]
pub struct Game {
    pub side: Color,

    pub pieces: PieceBoard,

    castling_flags: u8, // 0x0000KQkq, where kq/KQ is one if black/white king and queen
    en_passant_bitmap: Bitboard, // if a piece just moved for the first time, 1 will be over the square

    pub color_bitboards: [Bitboard; Color::COUNT],
    pub piece_bitboards: [Bitboard; Piece::COUNT],

    all_pieces: Bitboard,

    halfmoves: usize,
    halfmoves_since_capture: u8,

    // store the move, which piece was there, and en-passant + castling flags
    // the flags can NOT be calculated as an arbitrary position can have those
    // (move, captured_piece, castling_flags, en_passant_bitmap, halfmoves_since_capture)
    pub history: Vec<(BoardMove, Option<ColoredPiece>, u8, Bitboard, u8)>,

    // store the zobrist key for the current position (computed iteratively)
    pub zobrist_key: u64,

    pub non_pawn_remaining_material: f32,
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
            non_pawn_remaining_material: 0.0,
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

        game.halfmoves_since_capture = parts.next().unwrap_or("0").parse::<u8>().unwrap();

        // Fullmoves start at 1 and are incremented for white play
        let fullmoves = parts.next().unwrap_or("1").parse::<usize>().unwrap();
        game.halfmoves = (fullmoves - 1) * 2 + 1;
        if game.side == Color::Black {
            game.halfmoves += 1;
        }

        game
    }

    #[allow(dead_code)]
    pub(crate) fn get_fen(&self) -> String {
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

        self.zobrist_key ^= ZOBRIST_TABLE.pieces[color as usize][piece as usize][square as usize];

        self.non_pawn_remaining_material -= PIECE_VALUES[piece as usize + 1];
    }

    fn set_piece(&mut self, square: BoardSquare, colored_piece @ (piece, color): ColoredPiece) {
        let mask = square.to_mask();

        self.piece_bitboards[piece as usize] |= mask;
        self.color_bitboards[color as usize] |= mask;

        self.pieces[square as usize] = Some(colored_piece);

        self.all_pieces = self.color_bitboards[Color::White as usize]
            | self.color_bitboards[Color::Black as usize];

        self.zobrist_key ^= ZOBRIST_TABLE.pieces[color as usize][piece as usize][square as usize];

        self.non_pawn_remaining_material += PIECE_VALUES[piece as usize + 1];
    }

    // EWW duplication!!!
    fn set_piece_const<P: ConstPiece, C: ConstColor>(&mut self, square: BoardSquare) {
        debug_assert!(self.pieces[square as usize].is_none());

        let mask = square.to_mask();

        self.piece_bitboards[P::PIECE_INDEX] |= mask;
        self.color_bitboards[C::COLOR_INDEX] |= mask;

        self.pieces[square as usize] = Some((P::PIECE, C::COLOR));

        self.all_pieces = self.color_bitboards[Color::White as usize]
            | self.color_bitboards[Color::Black as usize];

        self.zobrist_key ^= ZOBRIST_TABLE.pieces[C::COLOR_INDEX][P::PIECE_INDEX][square as usize];

        self.non_pawn_remaining_material += PIECE_VALUES[P::PIECE_INDEX + 1];
    }

    fn update_turn(&mut self, delta: isize) {
        self.side = !self.side;
        self.halfmoves = self.halfmoves.wrapping_add_signed(delta);

        self.zobrist_key ^= ZOBRIST_TABLE.side_to_move;
    }

    fn update_castling_flags(&mut self, castling_flags: u8) {
        self.zobrist_key ^= ZOBRIST_TABLE.castling[self.castling_flags as usize];
        self.castling_flags = castling_flags;
        self.zobrist_key ^= ZOBRIST_TABLE.castling[castling_flags as usize];
    }

    fn update_en_passant_bitmap(&mut self, en_passant_bitmap: Bitboard) {
        // remove old
        let prev_idx = self.en_passant_bitmap.next_index();
        let prev_mask = u8::from(self.en_passant_bitmap != 0);
        let prev_col = (prev_idx.get_x() % 64 + 1) * prev_mask;
        self.zobrist_key ^= ZOBRIST_TABLE.en_passant[prev_col as usize];

        // update
        self.en_passant_bitmap = en_passant_bitmap;

        // add new
        let new_idx = self.en_passant_bitmap.next_index();
        let new_mask = u8::from(en_passant_bitmap != 0);
        let new_col = (new_idx.get_x() % 64 + 1) * new_mask;
        self.zobrist_key ^= ZOBRIST_TABLE.en_passant[new_col as usize];
    }

    ///
    /// Bitboards for a piece of a given color.
    ///
    fn colored_piece_bitboard_const<P: ConstPiece, C: ConstColor>(&self) -> Bitboard {
        self.piece_bitboards[P::PIECE_INDEX] & self.color_bitboards[C::COLOR_INDEX]
    }

    ///
    /// Undo a move.
    ///
    pub(crate) fn unmake_move(&mut self) {
        let (
            board_move,
            captured_piece,
            castling_flags,
            en_passant_bitmap,
            halfmoves_since_capture,
        ) = self.history.pop().unwrap();

        self.halfmoves_since_capture = halfmoves_since_capture;

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
                Some(_) => (Piece::Pawn, C::COLOR),
                _ => (P::PIECE, C::COLOR),
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

    pub(crate) fn make_null_move(&mut self) {
        self.history.push((
            BoardMove::empty(),
            None,
            self.castling_flags,
            self.en_passant_bitmap,
            self.halfmoves_since_capture,
        ));

        self.update_en_passant_bitmap(0);
        self.halfmoves_since_capture = self.halfmoves_since_capture.saturating_add(1);
        self.update_turn(0);
    }

    pub(crate) fn unmake_null_move(&mut self) {
        let (_, _, _, en_passant_bitmap, halfmoves_since_capture) = self.history.pop().unwrap();

        self.update_en_passant_bitmap(en_passant_bitmap);
        self.halfmoves_since_capture = halfmoves_since_capture;
        self.update_turn(0);
    }

    fn make_move_const<P: ConstPiece, C: ConstColor>(&mut self, board_move: BoardMove) {
        let captured_piece = self.pieces[board_move.get_to() as usize];

        let prev_halfmoves_since_capture = self.halfmoves_since_capture;

        self.history.push((
            board_move,
            captured_piece,
            self.castling_flags,
            self.en_passant_bitmap,
            prev_halfmoves_since_capture,
        ));

        // update halfmoves_since_capture by either capture or pawn move
        if captured_piece.is_some() || P::PIECE == Piece::Pawn {
            self.halfmoves_since_capture = 0;
        } else {
            self.halfmoves_since_capture = self.halfmoves_since_capture.saturating_add(1);
        }

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
            if board_move.get_to().get_y() == 7 || board_move.get_to().get_y() == 0 {
                self.set_piece(
                    board_move.get_to(),
                    (board_move.get_promotion().unwrap(), C::COLOR),
                );
            } else {
                self.set_piece_const::<ConstPawn, C>(board_move.get_to());
            }
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

    fn get_piece_attack_bitboard_const<P: ConstPiece, C: ConstColor>(
        &self,
        square: BoardSquare,
    ) -> Bitboard {
        if P::PIECE == Piece::Pawn {
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

            if P::PIECE.is_slider() {
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
    /// Generate a bitboard that contains pseudo-legal moves for a particular square,
    /// ignoring most king-safety-related rules.
    ///
    fn get_pseudo_legal_move_bitboard_const<P: ConstPiece, C: ConstColor>(
        &self,
        square: BoardSquare,
    ) -> Bitboard {
        // Get attack moves (which are also regular moves for all but pawns)
        let mut valid_moves = self.get_piece_attack_bitboard_const::<P, C>(square);

        if P::PIECE == Piece::Pawn {
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
    /// Retrieve pinning information for rook and bishop attacks.
    /// Returns structured data with pinned pieces and their valid move squares.
    ///
    fn get_pinner_bitboards_const<C: ConstColor>(&self) -> PinData {
        let king_position = self.get_king_position_const::<C>();
        let mut pin_data = PinData::new();

        for_each_simple_slider!(|P| {
            let raycast_1 = self.get_occlusion_bitmap_const::<P>(king_position, self.all_pieces);
            let raycast_2 =
                self.get_occlusion_bitmap_const::<P>(king_position, self.all_pieces & !raycast_1);

            let attacker_positions = (self.colored_piece_bitboard_const::<P, C::Opponent>()
                | self.colored_piece_bitboard_const::<ConstQueen, C::Opponent>())
                & (raycast_2 & !raycast_1);

            for attacker_position in attacker_positions.iter_positions() {
                let ray = RAY_BETWEEN[king_position as usize][attacker_position as usize];

                let pinned_piece_bitboard = ray & self.all_pieces;
                let pinned_piece_position = pinned_piece_bitboard.next_index();

                pin_data.add_pin(pinned_piece_position, attacker_position);
            }
        });

        pin_data
    }

    ///
    /// Retrieve attack information when the king is under exactly one attack.
    /// Returns the attack bitmap (ray from attacker to king) for blocking moves,
    /// plus the attacker position for capture moves.
    ///
    fn get_slider_attack_data_const<PA: ConstPiece, C: ConstColor>(
        &self,
        position: BoardSquare,
    ) -> Bitboard {
        let raycast_from_king = self.get_occlusion_bitmap_const::<PA>(position, self.all_pieces);

        // Find enemy sliders (including queens) that could attack along this ray
        let attacker_bitboard =
            self.colored_piece_bitboard_const::<PA, C::Opponent>() & raycast_from_king;

        let attacker_position = attacker_bitboard.next_index();

        RAY_BETWEEN[position as usize][attacker_position as usize] | attacker_position.to_mask()
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
    ///
    fn add_king_evade_or_capture_moves_const<C: ConstColor>(
        &self,
        king_position: BoardSquare,
        attacked_from_bitboard: Bitboard,
        moves: &mut [BoardMove; 256],
        move_count: &mut usize,
    ) {
        // we need to collect bitboards of attacking sliders, as king could otherwise
        // move "away" from them, which is technically a safe square
        let mut slider_attack_bitboard = Bitboard::default();

        let opponent_rooks =
            attacked_from_bitboard & self.colored_piece_bitboard_const::<ConstRook, C::Opponent>();
        let opponent_bishops = attacked_from_bitboard
            & self.colored_piece_bitboard_const::<ConstBishop, C::Opponent>();
        let opponent_queens =
            attacked_from_bitboard & self.colored_piece_bitboard_const::<ConstQueen, C::Opponent>();

        // Process rook attacks (including queen's rook-like attacks)
        for position in (opponent_rooks | opponent_queens).iter_positions() {
            slider_attack_bitboard |= self.get_occlusion_bitmap_const::<ConstRook>(
                position,
                self.all_pieces & !king_position.to_mask(),
            );
        }

        // Process bishop attacks (including queen's bishop-like attacks)
        for position in (opponent_bishops | opponent_queens).iter_positions() {
            slider_attack_bitboard |= self.get_occlusion_bitmap_const::<ConstBishop>(
                position,
                self.all_pieces & !king_position.to_mask(),
            );
        }

        let legal_move_bitboard = self
            .get_pseudo_legal_move_bitboard_const::<ConstKing, C>(king_position)
            & !slider_attack_bitboard;

        for target in legal_move_bitboard.iter_positions() {
            if !self.is_square_attacked_const::<C::Opponent>(target) {
                moves[*move_count] = BoardMove::regular(king_position, target);
                *move_count += 1;
            }
        }
    }

    fn check_discovered_en_passant_attack<C: ConstColor>(
        &self,
        square: BoardSquare,
        king_position: BoardSquare,
    ) -> bool {
        if square.get_y() == king_position.get_y() {
            let target = self.en_passant_bitmap.next_index();

            // En passant capture - need to check for horizontal discovered attacks
            let captured_pawn_square = BoardSquare::from_position(target.get_x(), square.get_y());

            // Simulate the board state after en passant (both pawns removed)
            let simulated_blockers = self.all_pieces
                & !square.to_mask() // Remove moving pawn
                & !captured_pawn_square.to_mask() // Remove captured pawn
                | target.to_mask(); // Add pawn at destination

            // Check if this exposes the king to a rook/queen attack along the rank
            let rook_attacks =
                self.get_occlusion_bitmap_const::<ConstRook>(king_position, simulated_blockers);

            // Check if any enemy rooks or queens can now attack the king
            let enemy_rook_queens = (self.colored_piece_bitboard_const::<ConstRook, C::Opponent>()
                | self.colored_piece_bitboard_const::<ConstQueen, C::Opponent>())
                & rook_attacks;

            if enemy_rook_queens != 0 {
                return true;
            }
        }

        false
    }

    fn add_regular_moves(
        &self,
        source: BoardSquare,
        mut target_bitboard: Bitboard,
        moves: &mut [BoardMove; 256],
        move_count: &mut usize,
    ) {
        while target_bitboard != 0 {
            let target = target_bitboard.next_index();
            moves[*move_count] = BoardMove::regular(source, target);
            *move_count += 1;
            target_bitboard &= !target.to_mask();
        }
    }

    fn add_promotion_moves(
        &self,
        source: BoardSquare,
        mut target_bitboard: Bitboard,
        moves: &mut [BoardMove; 256],
        move_count: &mut usize,
    ) {
        while target_bitboard != 0 {
            let target = target_bitboard.next_index();

            for promotion_piece in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight] {
                moves[*move_count] = BoardMove::promoting(source, target, promotion_piece);
                *move_count += 1;
            }

            target_bitboard &= !target.to_mask();
        }
    }

    fn process_zero_attack_moves_const<P: ConstPiece, C: ConstColor>(
        &self,
        pin_data: &PinData,
        king_position: BoardSquare,
        moves: &mut [BoardMove; 256],
        move_count: &mut usize,
    ) {
        let move_bitboard = self.colored_piece_bitboard_const::<P, C>();

        // for non-pawn pieces, just move
        if P::PIECE != Piece::Pawn {
            for square in move_bitboard.iter_positions() {
                let pin_mask = pin_data.get_pin_mask_for_square(square, king_position);

                let pseudo_legal_move_bitboard =
                    self.get_pseudo_legal_move_bitboard_const::<P, C>(square);

                self.add_regular_moves(
                    square,
                    pseudo_legal_move_bitboard & pin_mask,
                    moves,
                    move_count,
                );
            }

            return;
        }

        // for pawns, we have special rules because en-passant sucks
        let promotion_mask = if C::COLOR == Color::White {
            WHITE_PROMOTION_ROW
        } else {
            BLACK_PROMOTION_ROW
        };

        // TODO: this is too much repeated code, refactor because ewwwwwwwww
        for square in (move_bitboard & promotion_mask).iter_positions() {
            let pin_mask = pin_data.get_pin_mask_for_square(square, king_position);
            let pseudo_legal_move_bitboard =
                self.get_pseudo_legal_move_bitboard_const::<ConstPawn, C>(square);

            let legal_move_bitboard = pseudo_legal_move_bitboard & pin_mask;

            self.add_promotion_moves(
                square,
                legal_move_bitboard & !self.en_passant_bitmap,
                moves,
                move_count,
            );

            if (legal_move_bitboard & self.en_passant_bitmap) != 0 {
                let target = self.en_passant_bitmap.next_index();

                if !self.check_discovered_en_passant_attack::<C>(square, king_position) {
                    for promotion_piece in [Piece::Queen, Piece::Rook, Piece::Bishop, Piece::Knight]
                    {
                        moves[*move_count] = BoardMove::promoting(square, target, promotion_piece);
                        *move_count += 1;
                    }
                }
            }
        }

        for square in (move_bitboard & !promotion_mask).iter_positions() {
            let pin_mask = pin_data.get_pin_mask_for_square(square, king_position);
            let pseudo_legal_move_bitboard =
                self.get_pseudo_legal_move_bitboard_const::<ConstPawn, C>(square);

            let legal_move_bitboard = pseudo_legal_move_bitboard & pin_mask;

            self.add_regular_moves(
                square,
                legal_move_bitboard & !self.en_passant_bitmap,
                moves,
                move_count,
            );

            if (legal_move_bitboard & self.en_passant_bitmap) != 0 {
                let target = self.en_passant_bitmap.next_index();

                if !self.check_discovered_en_passant_attack::<C>(square, king_position) {
                    moves[*move_count] = BoardMove::regular(square, target);
                    *move_count += 1;
                }
            }
        }
    }

    fn process_one_attack_moves_const<P: ConstPiece, PA: ConstPiece, C: ConstColor>(
        &self,
        pin_data: &PinData,
        king_position: BoardSquare,
        attacking_position: BoardSquare,
        moves: &mut [BoardMove; 256],
        move_count: &mut usize,
    ) {
        let move_bitboard = self.colored_piece_bitboard_const::<P, C>();

        // go through all non-king pieces
        for square in move_bitboard.iter_positions() {
            // Get pin mask for this square if it's pinned
            let pin_mask = pin_data.get_pin_mask_for_square(square, king_position);

            let bitboard;

            if !PA::PIECE.is_slider() {
                // There is a special bullshit case where a pawn attacks a king and we can take it via en-passant
                if P::PIECE == Piece::Pawn
                    && PA::PIECE == Piece::Pawn
                    && (self.en_passant_bitmap
                        & self.get_piece_attack_bitboard_const::<ConstPawn, C>(square))
                        & pin_mask
                        != 0
                {
                    moves[*move_count] =
                        BoardMove::regular(square, self.en_passant_bitmap.next_index());
                    *move_count += 1;
                    continue;
                }

                bitboard = self.get_pseudo_legal_move_bitboard_const::<P, C>(square)
                    & attacking_position.to_mask()
                    & pin_mask;
            } else {
                // If it is a slider, we can either take, or block
                let attack_data = self.get_slider_attack_data_const::<PA, C>(king_position);

                bitboard = self.get_pseudo_legal_move_bitboard_const::<P, C>(square)
                    & attack_data
                    & pin_mask;
            }

            // for pawns, we have special rules because en-passant sucks
            let promotion_mask = if C::COLOR == Color::White {
                WHITE_PROMOTION_ROW
            } else {
                BLACK_PROMOTION_ROW
            };

            if P::PIECE == Piece::Pawn && (square.to_mask() & promotion_mask) != 0 {
                self.add_promotion_moves(square, bitboard, moves, move_count);
            } else {
                self.add_regular_moves(square, bitboard, moves, move_count);
            }
        }
    }

    ///
    /// Obtain a list of valid moves for the current position.
    ///
    fn get_moves_const<C: ConstColor>(&self) -> (usize, [BoardMove; 256]) {
        let mut moves = [BoardMove::default(); 256];
        let mut move_count = 0usize;

        let king_position = self.get_king_position_const::<C>();
        let king_attacks = self.get_attacked_from_const::<C::Opponent>(king_position);

        if king_attacks.count_ones() == 0 {
            // king is not under attack, so just move regularly, but not into a pin
            let pin_data = self.get_pinner_bitboards_const::<C>();

            for_each_non_king_const_piece!(|P| {
                self.process_zero_attack_moves_const::<P, C>(
                    &pin_data,
                    king_position,
                    &mut moves,
                    &mut move_count,
                );
            });

            // for king, just don't move into an attack
            let mut bitboard =
                self.get_pseudo_legal_move_bitboard_const::<ConstKing, C>(king_position);

            // we can also castle!
            bitboard |= self.get_castling_bitboard_const::<C>();

            for target in bitboard.iter_positions() {
                if !self.is_square_attacked_const::<C::Opponent>(target) {
                    moves[move_count] = BoardMove::regular(king_position, target);
                    move_count += 1;
                }
            }
        } else if king_attacks.count_ones() == 1 {
            // king is under one attack -- he can
            //  - block with an unpinned piece / take the attacker
            //  - evade
            let pin_data = self.get_pinner_bitboards_const::<C>();

            let attacking_position = king_attacks.next_index();
            let (attacking_piece, _) = self.pieces[attacking_position as usize].unwrap();

            match attacking_piece {
                Piece::Pawn => {
                    for_each_non_king_const_piece!(|P| {
                        self.process_one_attack_moves_const::<P, ConstPawn, C>(
                            &pin_data,
                            king_position,
                            attacking_position,
                            &mut moves,
                            &mut move_count,
                        );
                    });
                }
                Piece::Knight => {
                    for_each_non_king_const_piece!(|P| {
                        self.process_one_attack_moves_const::<P, ConstKnight, C>(
                            &pin_data,
                            king_position,
                            attacking_position,
                            &mut moves,
                            &mut move_count,
                        );
                    });
                }
                Piece::Bishop => {
                    for_each_non_king_const_piece!(|P| {
                        self.process_one_attack_moves_const::<P, ConstBishop, C>(
                            &pin_data,
                            king_position,
                            attacking_position,
                            &mut moves,
                            &mut move_count,
                        );
                    });
                }
                Piece::Rook => {
                    for_each_non_king_const_piece!(|P| {
                        self.process_one_attack_moves_const::<P, ConstRook, C>(
                            &pin_data,
                            king_position,
                            attacking_position,
                            &mut moves,
                            &mut move_count,
                        );
                    });
                }
                Piece::Queen => {
                    for_each_non_king_const_piece!(|P| {
                        self.process_one_attack_moves_const::<P, ConstQueen, C>(
                            &pin_data,
                            king_position,
                            attacking_position,
                            &mut moves,
                            &mut move_count,
                        );
                    });
                }
                _ => unreachable!(),
            }

            self.add_king_evade_or_capture_moves_const::<C>(
                king_position,
                king_attacks,
                &mut moves,
                &mut move_count,
            );
        } else {
            // can only evade if we have multiple attacks
            self.add_king_evade_or_capture_moves_const::<C>(
                king_position,
                king_attacks,
                &mut moves,
                &mut move_count,
            );
        }

        (move_count, moves)
    }

    pub(crate) fn get_moves(&self) -> (usize, [BoardMove; 256]) {
        self.get_side_moves(self.side)
    }

    pub(crate) fn get_side_moves(&self, side: Color) -> (usize, [BoardMove; 256]) {
        match side {
            Color::White => self.get_moves_const::<ConstWhite>(),
            Color::Black => self.get_moves_const::<ConstBlack>(),
        }
    }

    ///
    /// Returns true if the king of the given color is in check.
    ///
    pub(crate) fn is_king_in_check(&self, color: Color) -> bool {
        match color {
            Color::White => self.is_square_attacked_const::<ConstBlack>(
                self.get_king_position_const::<ConstWhite>(),
            ),
            Color::Black => self.is_square_attacked_const::<ConstWhite>(
                self.get_king_position_const::<ConstBlack>(),
            ),
        }
    }

    pub(crate) fn is_capture(&self, board_move: BoardMove) -> bool {
        // Check if there's a piece at the destination
        if self.pieces[board_move.get_to() as usize].is_some() {
            return true;
        }

        // Check for en passant capture
        if let Some((piece, _)) = self.pieces[board_move.get_from() as usize] {
            if piece == Piece::Pawn && self.en_passant_bitmap.is_set(board_move.get_to()) {
                return true;
            }
        }

        false
    }

    pub(crate) fn is_check(&mut self, board_move: BoardMove) -> bool {
        self.make_move(board_move);
        let is_check = self.is_king_in_check(!self.side);
        self.unmake_move();
        is_check
    }

    pub(crate) fn evaluate(&self) -> f32 {
        let (white_material, black_material) = evaluate_material(self);
        let game_phase = calculate_game_phase(self);

        let (white_move_count, white_moves) = self.get_side_pseudo_legal_moves(Color::White);
        let (black_move_count, black_moves) = self.get_side_pseudo_legal_moves(Color::Black);

        let white_moves_slice = &white_moves[..white_move_count];
        let black_moves_slice = &black_moves[..black_move_count];

        let material_value = white_material - black_material;
        let positional_value = evaluate_positional(self, game_phase);
        let bishop_pair_value = evaluate_bishop_pair(self, game_phase);

        let mobility_value =
            evaluate_mobility(self, game_phase, white_moves_slice, black_moves_slice);

        let king_safety =
            evaluate_king_safety(self, game_phase, white_moves_slice, black_moves_slice);

        material_value + positional_value + mobility_value + bishop_pair_value + king_safety
    }

    pub fn is_fifty_move_rule(&self) -> bool {
        self.halfmoves_since_capture >= 100
    }

    /// Play through a sequence of moves and record the zobrist hash after each move
    pub fn record_position_sequence(&mut self, moves: &[BoardMove]) -> Vec<(u64, BoardMove)> {
        let mut positions = Vec::new();

        for &board_move in moves {
            // Record the position before making the move
            let zobrist_key = self.zobrist_key;
            positions.push((zobrist_key, board_move));

            // Make the move
            self.make_move(board_move);
        }

        positions
    }

    pub(crate) fn get_side_pseudo_legal_moves(&self, color: Color) -> (usize, [BoardMove; 256]) {
        match color {
            Color::White => self.get_pseudo_legal_moves_const::<ConstWhite>(),
            Color::Black => self.get_pseudo_legal_moves_const::<ConstBlack>(),
        }
    }

    fn get_pseudo_legal_moves_const<C: ConstColor>(&self) -> (usize, [BoardMove; 256]) {
        let mut moves = [BoardMove::default(); 256];
        let mut move_count = 0usize;

        // Generate moves for each piece type
        self.add_pseudo_legal_piece_moves::<ConstPawn, C>(&mut moves, &mut move_count);
        self.add_pseudo_legal_piece_moves::<ConstKnight, C>(&mut moves, &mut move_count);
        self.add_pseudo_legal_piece_moves::<ConstBishop, C>(&mut moves, &mut move_count);
        self.add_pseudo_legal_piece_moves::<ConstRook, C>(&mut moves, &mut move_count);
        self.add_pseudo_legal_piece_moves::<ConstQueen, C>(&mut moves, &mut move_count);
        self.add_pseudo_legal_piece_moves::<ConstKing, C>(&mut moves, &mut move_count);

        (move_count, moves)
    }

    fn add_pseudo_legal_piece_moves<P: ConstPiece, C: ConstColor>(
        &self,
        moves: &mut [BoardMove; 256],
        move_count: &mut usize,
    ) {
        let piece_bitboard = self.colored_piece_bitboard_const::<P, C>();

        // Special handling for pawns due to promotions
        if P::PIECE == Piece::Pawn {
            self.add_pseudo_legal_pawn_moves::<C>(piece_bitboard, moves, move_count);
            return;
        }

        // For all other pieces, just generate regular moves
        for square in piece_bitboard.iter_positions() {
            let target_bitboard = self.get_pseudo_legal_move_bitboard_const::<P, C>(square);
            self.add_regular_moves(square, target_bitboard, moves, move_count);
        }
    }

    fn add_pseudo_legal_pawn_moves<C: ConstColor>(
        &self,
        pawn_bitboard: Bitboard,
        moves: &mut [BoardMove; 256],
        move_count: &mut usize,
    ) {
        let promotion_rank = if C::COLOR == Color::White {
            WHITE_PROMOTION_ROW
        } else {
            BLACK_PROMOTION_ROW
        };

        // Handle promoting pawns
        for square in (pawn_bitboard & promotion_rank).iter_positions() {
            let target_bitboard = self.get_pseudo_legal_move_bitboard_const::<ConstPawn, C>(square);
            self.add_promotion_moves(square, target_bitboard, moves, move_count);
        }

        // Handle non-promoting pawns
        for square in (pawn_bitboard & !promotion_rank).iter_positions() {
            let target_bitboard = self.get_pseudo_legal_move_bitboard_const::<ConstPawn, C>(square);
            self.add_regular_moves(square, target_bitboard, moves, move_count);
        }
    }

    pub(crate) fn get_king_position(&self, color: Color) -> BoardSquare {
        match color {
            Color::White => self.get_king_position_const::<ConstWhite>(),
            Color::Black => self.get_king_position_const::<ConstBlack>(),
        }
    }

    pub(crate) fn get_attacked_from(&self, square: BoardSquare, color: Color) -> Bitboard {
        match color {
            Color::White => self.get_attacked_from_const::<ConstBlack>(square),
            Color::Black => self.get_attacked_from_const::<ConstWhite>(square),
        }
    }

    pub(crate) fn get_king_attacks(&self, color: Color) -> Bitboard {
        match color {
            Color::White => self.get_attacked_from_const::<ConstBlack>(
                self.get_king_position_const::<ConstWhite>(),
            ),
            Color::Black => self.get_attacked_from_const::<ConstWhite>(
                self.get_king_position_const::<ConstBlack>(),
            ),
        }
    }

    pub(crate) fn get_king_visibility(&self, color: Color) -> Bitboard {
        return match color {
            Color::White => self.get_occlusion_bitmap_const::<ConstQueen>(
                self.get_king_position_const::<ConstWhite>(),
                self.all_pieces,
            ),
            Color::Black => self.get_occlusion_bitmap_const::<ConstQueen>(
                self.get_king_position_const::<ConstBlack>(),
                self.all_pieces,
            ),
        };
    }
}
