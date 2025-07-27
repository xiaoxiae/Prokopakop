use crate::game::{Color, Piece};
use strum::EnumCount;

pub type Bitboard = u64;

pub trait BitboardExt {
    fn position_to_bitmask(x: u32, y: u32) -> Self;
    fn is_set(&self, x: u32, y: u32) -> bool;
    fn print(&self, title: Option<&str>, position: Option<(u32, u32)>);
}

// used like this because we can't have a const fn as a trait,
// but we want to use it for the compile-time bitmap calculation
pub const fn position_to_bitmask(x: u32, y: u32) -> u64 {
    1u64 << x + y * 8
}

pub const fn is_position_valid(x: isize, y: isize) -> bool {
    x >= 0 && x < 8 && y >= 0 && y < 8
}

impl BitboardExt for u64 {
    fn position_to_bitmask(x: u32, y: u32) -> Self {
        position_to_bitmask(x, y)
    }

    fn is_set(&self, x: u32, y: u32) -> bool {
        self & position_to_bitmask(x, y) != 0
    }

    fn print(&self, title: Option<&str>, position: Option<(u32, u32)>) {
        if let Some(title_text) = title {
            log::debug!(
                "\x1b[97m{}{}\x1b[0m",
                " ".repeat((3 * 8 - title_text.len()) / 2),
                title_text
            );
        }

        for y in (0..8).rev() {
            let mut line = String::new();
            for x in 0..8 {
                let is_marked_position = position.map_or(false, |(px, py)| px == x && py == y);

                line.push_str(
                    match (
                        Self::position_to_bitmask(x, y) & self != 0,
                        is_marked_position,
                    ) {
                        (_, true) => "\x1b[93m ● \x1b[0m",
                        (true, false) => "\x1b[97m 1 \x1b[0m",
                        (false, false) => "\x1b[90m 0 \x1b[0m",
                    },
                );
            }
            log::debug!("{}", line);
        }

        if title.is_some() {
            log::debug!("");
        }
    }
}

type PawnBitboards = [[Bitboard; 64]; Color::COUNT];
type ValidMoveBitboards = [[[Bitboard; 64]; Piece::COUNT]; Color::COUNT];

const fn create_bitboard_for_piece(
    mut x: usize,
    mut y: usize,
    deltas: &[[isize; 2]],
    infinite: bool,
) -> Bitboard {
    let mut bitboard = 0;

    let mut i = 0;
    while i < deltas.len() {
        let dx = deltas[i][0];
        let dy = deltas[i][1];

        let mut nx = x as isize;
        let mut ny = y as isize;

        loop {
            nx += dx;
            ny += dy;

            if !is_position_valid(nx, ny) {
                break;
            }

            bitboard |= position_to_bitmask(nx as u32, ny as u32);

            if !infinite {
                break;
            }
        }

        i += 1;
    }

    bitboard
}

const fn calculate_bitboard_for_pieces() -> ValidMoveBitboards {
    let mut bitboards = [[[0; 64]; Piece::COUNT]; Color::COUNT];

    let mut color = 0;
    while color < Color::COUNT {
        let mut piece = 0;
        while piece < Piece::COUNT {
            let mut x = 0;

            while x < 8 {
                let mut y = 0;

                while y < 8 {
                    match Piece::from_repr(piece) {
                        Some(Piece::Pawn) => {
                            let deltas = match color {
                                0 => [[0, -1]],
                                1 => [[0, 1]],
                                _ => unreachable!(),
                            };

                            bitboards[color][piece][x + y * 8] |=
                                create_bitboard_for_piece(x, y, &deltas, false);
                        }
                        Some(Piece::Knight) => {
                            let deltas = [
                                [1, 2],
                                [2, 1],
                                [-1, 2],
                                [-2, 1],
                                [1, -2],
                                [2, -1],
                                [-1, -2],
                                [-2, -1],
                            ];

                            bitboards[color][piece][x + y * 8] |=
                                create_bitboard_for_piece(x, y, &deltas, false);
                        }
                        Some(Piece::Bishop) => {
                            let deltas = [[1, 1], [1, -1], [-1, 1], [-1, -1]];

                            bitboards[color][piece][x + y * 8] |=
                                create_bitboard_for_piece(x, y, &deltas, true);
                        }
                        Some(Piece::Rook) => {
                            let deltas = [[1, 0], [0, 1], [-1, 0], [0, -1]];

                            bitboards[color][piece][x + y * 8] |=
                                create_bitboard_for_piece(x, y, &deltas, true);
                        }
                        Some(Piece::Queen) => {
                            let deltas = [
                                [1, 0],
                                [0, 1],
                                [-1, 0],
                                [0, -1],
                                [1, 1],
                                [1, -1],
                                [-1, 1],
                                [-1, -1],
                            ];

                            bitboards[color][piece][x + y * 8] |=
                                create_bitboard_for_piece(x, y, &deltas, true);
                        }
                        Some(Piece::King) => {
                            let deltas = [
                                [1, 0],
                                [0, 1],
                                [-1, 0],
                                [0, -1],
                                [1, 1],
                                [1, -1],
                                [-1, 1],
                                [-1, -1],
                            ];

                            bitboards[color][piece][x + y * 8] |=
                                create_bitboard_for_piece(x, y, &deltas, false);
                        }
                        None => unreachable!(),
                    }

                    y += 1;
                }

                x += 1;
            }

            piece += 1;
        }

        color += 1;
    }

    bitboards
}

const fn calculate_pawn_attack_moves() -> PawnBitboards {
    let mut bitboards = [[0; 64]; Color::COUNT];

    let mut color = 0;
    while color < Color::COUNT {
        let mut x = 0;

        while x < 8 {
            let mut y = 0;

            while y < 8 {
                let deltas = match color {
                    0 => [[-1, -1], [1, -1]],
                    1 => [[-1, 1], [1, 1]],
                    _ => unreachable!(),
                };

                bitboards[color][x + y * 8] |= create_bitboard_for_piece(x, y, &deltas, false);

                y += 1;
            }

            x += 1;
        }
        color += 1;
    }

    bitboards
}

const fn calculate_pawn_first_moves() -> PawnBitboards {
    let mut bitboards = [[0; 64]; Color::COUNT];

    let mut color = 0;
    while color < Color::COUNT {
        let mut x = 0;

        while x < 8 {
            let mut y = 0;

            while y < 8 {
                let deltas = match color {
                    0 => [[0, -2]],
                    1 => [[0, 2]],
                    _ => unreachable!(),
                };

                bitboards[color][x + y * 8] |= create_bitboard_for_piece(x, y, &deltas, false);

                y += 1;
            }

            x += 1;
        }
        color += 1;
    }

    bitboards
}

pub const VALID_MOVE_BITBOARDS: ValidMoveBitboards = calculate_bitboard_for_pieces();

pub const PAWN_ATTACK_MOVE_BITBOARD: PawnBitboards = calculate_pawn_attack_moves();
pub const PAWN_FIRST_MOVE_BITBOARD: PawnBitboards = calculate_pawn_first_moves();
