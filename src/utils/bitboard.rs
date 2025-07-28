use crate::game::{Color, Piece};
use rand::RngCore;
use std::fs::File;
use std::io::{Result, Write};
use std::path::Path;
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

type PieceBitboards = [Bitboard; 64];
type PawnBitboards = [PieceBitboards; Color::COUNT];
type ValidMoveBitboards = [[PieceBitboards; Piece::COUNT]; Color::COUNT];

const fn create_bitboard_for_piece(
    x: usize,
    y: usize,
    deltas: &[[i8; 2]],
    infinite: bool,
    exclude_last: bool,
    blockers: Bitboard,
) -> Bitboard {
    let mut bitboard = 0;

    let mut i = 0;
    while i < deltas.len() {
        let dx = deltas[i][0];
        let dy = deltas[i][1];

        let mut nx = x as i8;
        let mut ny = y as i8;

        loop {
            if blockers & position_to_bitmask(nx as u32, ny as u32) != 0 {
                break;
            }

            nx += dx;
            ny += dy;

            if !is_position_valid(nx as isize, ny as isize) {
                if exclude_last {
                    bitboard &= !position_to_bitmask((nx - dx) as u32, (ny - dy) as u32);
                }

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

const fn get_piece_deltas(piece: &Piece, color_value: usize) -> &'static [[i8; 2]] {
    match piece {
        Piece::Pawn => match color_value {
            0 => &[[0, -1]],
            1 => &[[0, 1]],
            _ => unreachable!(),
        },
        Piece::Knight => &[
            [1, 2],
            [2, 1],
            [-1, 2],
            [-2, 1],
            [1, -2],
            [2, -1],
            [-1, -2],
            [-2, -1],
        ],
        Piece::Bishop => &[[1, 1], [1, -1], [-1, 1], [-1, -1]],
        Piece::Rook => &[[1, 0], [0, 1], [-1, 0], [0, -1]],
        Piece::Queen => &[
            [1, 0],
            [0, 1],
            [-1, 0],
            [0, -1],
            [1, 1],
            [1, -1],
            [-1, 1],
            [-1, -1],
        ],
        Piece::King => &[
            [1, 0],
            [0, 1],
            [-1, 0],
            [0, -1],
            [1, 1],
            [1, -1],
            [-1, 1],
            [-1, -1],
        ],
    }
}

const fn get_is_infinite(piece: &Piece) -> bool {
    match piece {
        Piece::Pawn => false,
        Piece::Knight => false,
        Piece::Bishop => true,
        Piece::Rook => true,
        Piece::Queen => true,
        Piece::King => false,
    }
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
                        Some(piece_type) => {
                            let deltas = get_piece_deltas(&piece_type, color);
                            let infinite = get_is_infinite(&piece_type);

                            bitboards[color][piece][x + y * 8] |=
                                create_bitboard_for_piece(x, y, deltas, infinite, false, 0);
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

                bitboards[color][x + y * 8] |=
                    create_bitboard_for_piece(x, y, &deltas, false, false, 0);

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

                bitboards[color][x + y * 8] |=
                    create_bitboard_for_piece(x, y, &deltas, false, false, 0);

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

pub const ROOK_BLOCKER_BITBOARD: PieceBitboards =
    calculate_blocker_bitboards(get_piece_deltas(&Piece::Rook, 0));
pub const BISHOP_BLOCKER_BITBOARD: PieceBitboards =
    calculate_blocker_bitboards(get_piece_deltas(&Piece::Bishop, 0));

pub struct MagicBitboardEntry {
    pub magic: u64,
    pub shift: u8,
    pub entries: Vec<Bitboard>,
}

pub type MagicBitboards = Vec<MagicBitboardEntry>;

const fn calculate_blocker_bitboards(deltas: &[[i8; 2]]) -> PieceBitboards {
    let mut bitboards: PieceBitboards = [0; 64];

    let mut x = 0;
    while x < 8 {
        let mut y = 0;

        while y < 8 {
            bitboards[x + y * 8] = create_bitboard_for_piece(x, y, &deltas, true, true, 0);

            y += 1;
        }

        x += 1;
    }

    bitboards
}

pub fn calculate_magic_bitboard(x: usize, y: usize, piece: &Piece) -> MagicBitboardEntry {
    let possible_blockers_bitboard = match piece {
        Piece::Bishop => BISHOP_BLOCKER_BITBOARD[x + y * 8],
        Piece::Rook => ROOK_BLOCKER_BITBOARD[x + y * 8],
        _ => unreachable!(),
    };

    let blocker_count = possible_blockers_bitboard.count_ones();
    let key_count = 2usize.pow(blocker_count);
    let mut keys = Vec::with_capacity(key_count);

    // compute all possible blocker values
    for mut index in 0..key_count {
        let mut blockers = possible_blockers_bitboard;
        let mut bitboard = Bitboard::default();
        let mut zeros = 0;

        // spread the index value over the blockers bitboard
        while index != 0 {
            let current_zeros = blockers.trailing_zeros();
            zeros += current_zeros;
            blockers = (blockers >> current_zeros) & !1;

            bitboard |= ((index & 1) << zeros) as Bitboard;
            index >>= 1;
        }

        // for that particular blocker arrangement, calculate the valid moves
        let deltas = get_piece_deltas(&piece, 0); // color doesn't matter
        let valid_moves = create_bitboard_for_piece(x, y, deltas, true, false, bitboard);

        keys.push((bitboard, valid_moves));
    }

    let mut rng = rand::rng();
    let magic_bitmap_size = blocker_count;
    loop {
        // this is apparently the way to do it, since we need a relatively small number of bits
        // https://www.chessprogramming.org/Looking_for_Magics
        let magic: Bitboard = rng.next_u64() & rng.next_u64() & rng.next_u64();

        if magic == 0 {
            continue;
        }

        let mut hash_table = vec![None; 2usize.pow(magic_bitmap_size)];
        let mut collision = false;

        for (blockers, moves) in &keys {
            let hash = ((blockers.wrapping_mul(magic)) >> (64 - magic_bitmap_size)) as usize;

            if let Some(existing_moves) = hash_table[hash] {
                if existing_moves != *moves {
                    collision = true;
                    break;
                }
            } else {
                hash_table[hash] = Some(*moves);
            }
        }

        if !collision {
            return MagicBitboardEntry {
                magic,
                shift: 64 - magic_bitmap_size as u8,
                entries: hash_table
                    .into_iter()
                    .map(|moves| moves.unwrap_or(0))
                    .collect(),
            };
        }
    }
}

pub fn serialize_magic_bitboards_to_file_flat<P: AsRef<Path>>(
    magic_bitboards: &MagicBitboards,
    output_path: P,
) -> Result<()> {
    let mut file = File::create(output_path)?;

    // Calculate total entries and build combined magic table
    let mut all_entries: Vec<u64>  = Vec::new();
    let mut magic_table = Vec::new();
    let mut current_offset = 0;

    for entry in magic_bitboards {
        magic_table.push((entry.magic, current_offset, entry.shift));
        all_entries.extend(&entry.entries);
        current_offset += entry.entries.len();
    }

    writeln!(file, "// This file is auto-generated. Do not edit manually.")?;
    writeln!(file, "use crate::Bitboard;")?;
    writeln!(file)?;

    // combined data for accessing magic table (magic_number, start_offset, shift)
    writeln!(file, "pub const MAGIC_TABLE: [(u64, usize, u8); 128] = [")?;
    for (i, &(magic, offset, shift)) in magic_table.iter().enumerate() {
        write!(file, "    ({:#018x}, {}, {})", magic, offset, shift)?;
        if i < magic_table.len() - 1 {
            write!(file, ",")?;
        }
        writeln!(file)?;
    }
    writeln!(file, "];")?;
    writeln!(file)?;

    writeln!(file, "pub const MAGIC_ENTRIES: [Bitboard; {}] = [", all_entries.len())?;
    for (i, entry) in all_entries.iter().enumerate() {
        write!(file, "    {:#018x}", entry)?;
        if i < all_entries.len() - 1 {
            write!(file, ",")?;
        }
        writeln!(file)?;
    }
    writeln!(file, "];")?;
    writeln!(file)?;

    Ok(())
}