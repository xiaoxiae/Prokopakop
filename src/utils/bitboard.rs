use crate::game::{Color, Piece};
use rand::RngCore;
use rayon::prelude::*;
use std::fs::File;
use std::io::{Result, Write};
use std::path::Path;
use strum::EnumCount;

pub type Bitboard = u64;
pub type BoardSquare = u8;

pub trait BitboardExt {
    fn next_index(&self) -> BoardSquare;
    fn is_set(&self, index: BoardSquare) -> bool;
    fn print(&self, title: Option<&str>, position: Option<BoardSquare>);
    fn iter_positions(&self) -> BitboardIterator;
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
    fn next_index(&self) -> BoardSquare {
        self.trailing_zeros() as BoardSquare
    }

    fn is_set(&self, index: BoardSquare) -> bool {
        self & (1 << index) != 0
    }

    fn print(&self, title: Option<&str>, position: Option<BoardSquare>) {
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
                let is_marked_position =
                    position.map_or(false, |b| b.get_x() == x && b.get_y() == y);

                line.push_str(
                    match (
                        position_to_bitmask(x as u32, y as u32) & self != 0,
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

    fn iter_positions(&self) -> BitboardIterator {
        BitboardIterator { remaining: *self }
    }
}

pub trait BoardSquareExt {
    fn get_x(&self) -> u8;
    fn get_y(&self) -> u8;
    fn parse(string: &str) -> Option<BoardSquare>;
    fn unparse(&self) -> String;
    fn from_position(x: u8, y: u8) -> BoardSquare;
    fn to_mask(&self) -> Bitboard;

    // TODO: macro!?
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

pub struct BitboardIterator {
    remaining: u64,
}

impl Iterator for BitboardIterator {
    type Item = BoardSquare;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        let index = self.remaining.trailing_zeros() as u8;
        self.remaining &= self.remaining - 1; // Clear the lowest set bit

        Some(index)
    }
}

type PieceBitboards = [Bitboard; 64];
type PawnAttackBitboards = [PieceBitboards; Color::COUNT];
type ValidMoveBitboards = [PieceBitboards; Piece::COUNT];

const fn create_bitboard_for_piece(
    x: usize,
    y: usize,
    deltas: &[[i8; 2]],
    slider: bool,
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

            if !slider {
                break;
            }
        }

        i += 1;
    }

    bitboard
}

const fn get_attack_piece_deltas(piece: &Piece, color_value: usize) -> &'static [[i8; 2]] {
    match piece {
        Piece::Pawn => match color_value {
            0 => &[[-1, -1], [1, -1]],
            1 => &[[-1, 1], [1, 1]],
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

const fn get_is_slider(piece: &Piece) -> bool {
    match piece {
        Piece::Pawn => false,
        Piece::Knight => false,
        Piece::Bishop => true,
        Piece::Rook => true,
        Piece::Queen => true,
        Piece::King => false,
    }
}

const fn calculate_attack_bitboards_for_pieces() -> ValidMoveBitboards {
    let mut bitboards = [[0; 64]; Piece::COUNT];

    let mut piece = 0;
    while piece < Piece::COUNT {
        let mut x = 0;

        while x < 8 {
            let mut y = 0;

            while y < 8 {
                match Piece::from_repr(piece) {
                    Some(piece_type) => {
                        let deltas = get_attack_piece_deltas(&piece_type, 0);
                        let slider = get_is_slider(&piece_type);

                        bitboards[piece][x + y * 8] |=
                            create_bitboard_for_piece(x, y, deltas, slider, false, 0);
                    }
                    None => unreachable!(),
                }

                y += 1;
            }

            x += 1;
        }

        piece += 1;
    }

    bitboards
}

const fn calculate_pawn_attack_moves() -> PawnAttackBitboards {
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

pub const PIECE_MOVE_BITBOARDS: ValidMoveBitboards = calculate_attack_bitboards_for_pieces();
pub const PAWN_ATTACK_BITBOARDS: PawnAttackBitboards = calculate_pawn_attack_moves();

pub const MAGIC_ROOK_BLOCKER_BITBOARD: PieceBitboards =
    calculate_blocker_bitboards(get_attack_piece_deltas(&Piece::Rook, 0));
pub const MAGIC_BISHOP_BLOCKER_BITBOARD: PieceBitboards =
    calculate_blocker_bitboards(get_attack_piece_deltas(&Piece::Bishop, 0));

pub const MAGIC_BLOCKER_BITBOARD: [Bitboard; 128] = {
    let mut combined = [0u64; 128];
    let mut i = 0;

    // Copy rook bitboards (first 64 elements)
    while i < 64 {
        combined[i] = MAGIC_ROOK_BLOCKER_BITBOARD[i];
        i += 1;
    }

    // Copy bishop bitboards (next 64 elements)
    i = 0;
    while i < 64 {
        combined[64 + i] = MAGIC_BISHOP_BLOCKER_BITBOARD[i];
        i += 1;
    }

    combined
};

pub struct MagicBitboardEntry {
    pub magic: u64,
    pub shift: u8,
    pub entries: Vec<Bitboard>,
    pub max_index: usize,
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

pub fn calculate_magic_bitboard(
    x: usize,
    y: usize,
    piece: &Piece,
    target_max_index: Option<usize>,
) -> MagicBitboardEntry {
    let possible_blockers_bitboard = match piece {
        Piece::Bishop => MAGIC_BISHOP_BLOCKER_BITBOARD[x + y * 8],
        Piece::Rook => MAGIC_ROOK_BLOCKER_BITBOARD[x + y * 8],
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
        let deltas = get_attack_piece_deltas(&piece, 0); // color doesn't matter
        let valid_moves = create_bitboard_for_piece(x, y, deltas, true, false, bitboard);

        keys.push((bitboard, valid_moves));
    }

    let mut rng = rand::rng();
    let magic_bitmap_size = blocker_count;
    let max_attempts = if target_max_index.is_some() {
        1_000_000
    } else {
        100_000
    };
    let mut attempts = 0;

    loop {
        attempts += 1;

        // Give up if we're trying to find a better one and can't
        if attempts > max_attempts && target_max_index.is_some() {
            // Return a result with the current target as max_index
            return calculate_magic_bitboard(x, y, piece, None);
        }

        // this is apparently the way to do it, since we need a relatively small number of bits
        // https://www.chessprogramming.org/Looking_for_Magics
        let magic: Bitboard = rng.next_u64() & rng.next_u64() & rng.next_u64();

        if magic == 0 {
            continue;
        }

        let mut hash_table = vec![None; 2usize.pow(magic_bitmap_size)];
        let mut collision = false;
        let mut highest_index = 0;

        for (blockers, moves) in &keys {
            let hash = ((blockers.wrapping_mul(magic)) >> (64 - magic_bitmap_size)) as usize;

            // Track the highest index we actually use
            if hash > highest_index {
                highest_index = hash;
            }

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
            // If we have a target and this isn't better, keep trying
            if let Some(target) = target_max_index {
                if highest_index >= target {
                    continue;
                }
            }

            // Truncate the entries vector to only include up to the highest index
            let entries: Vec<Bitboard> = (0..=highest_index)
                .map(|i| hash_table[i].unwrap_or(0))
                .collect();

            return MagicBitboardEntry {
                magic,
                shift: 64 - magic_bitmap_size as u8,
                entries,
                max_index: highest_index,
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
    let mut all_entries: Vec<u64> = Vec::new();
    let mut magic_table = Vec::new();
    let mut current_offset = 0;

    for entry in magic_bitboards {
        magic_table.push((entry.magic, current_offset, entry.shift));
        all_entries.extend(&entry.entries);
        current_offset += entry.entries.len();
    }

    writeln!(
        file,
        "// This file is auto-generated. Do not edit manually."
    )?;
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

    writeln!(
        file,
        "pub const MAGIC_ENTRIES: [Bitboard; {}] = [",
        all_entries.len()
    )?;
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

pub fn generate_magic_bitboards() {
    let mut magic_bitboards: MagicBitboards = Vec::with_capacity(128);

    // Initialize with basic magic numbers
    log::info!("Finding initial magic bitboards...");

    // Rook magic numbers
    for y in 0..8 {
        for x in 0..8 {
            let index = x + y * 8;
            let result = calculate_magic_bitboard(x, y, &Piece::Rook, None);

            log::debug!(
                "Rook ({}/{}): {:064b}, shift={}, max_index={}",
                index + 1,
                64,
                result.magic,
                64 - result.shift,
                result.max_index
            );

            magic_bitboards.push(result);
        }
    }

    // Bishop magic numbers
    for y in 0..8 {
        for x in 0..8 {
            let index = x + y * 8;
            let result = calculate_magic_bitboard(x, y, &Piece::Bishop, None);

            log::debug!(
                "Bishop ({}/{}): {:064b}, shift={}, max_index={}",
                index + 1,
                64,
                result.magic,
                64 - result.shift,
                result.max_index
            );

            magic_bitboards.push(result);
        }
    }

    log::info!("Initial magic bitboards generated!");
    serialize_magic_bitboards_to_file_flat(&magic_bitboards, concat!("src/utils/magic.rs"))
        .expect("Failed to serialize initial magic bitboards");

    // Now run indefinitely trying to find more compact magic numbers
    log::info!("Searching for more compact magic bitboards...");

    let thread_count = rayon::current_num_threads();
    log::info!("Using {} threads for parallel search", thread_count);

    let mut iteration = 0;
    loop {
        iteration += 1;
        let mut improved = false;

        // Process all 128 positions in parallel
        let improvement_results: Vec<_> = (0..128)
            .into_par_iter()
            .map(|i| {
                let (x, y, piece) = if i < 64 {
                    let x = i % 8;
                    let y = i / 8;
                    (x, y, Piece::Rook)
                } else {
                    let idx = i - 64;
                    let x = idx % 8;
                    let y = idx / 8;
                    (x, y, Piece::Bishop)
                };

                let current_max_index = magic_bitboards[i].max_index;

                // Each thread tries to find a better magic number
                let candidates: Vec<_> = (0..thread_count)
                    .into_par_iter()
                    .map(|_| calculate_magic_bitboard(x, y, &piece, Some(current_max_index)))
                    .collect();

                // Find the best candidate among all thread results
                let best_candidate = candidates
                    .into_iter()
                    .min_by_key(|entry| entry.max_index)
                    .unwrap();

                (i, best_candidate, current_max_index)
            })
            .collect();

        // Merge results - update magic_bitboards with any improvements
        for (i, new_entry, old_max_index) in improvement_results {
            if new_entry.max_index < old_max_index {
                let (x, y, piece) = if i < 64 {
                    let x = i % 8;
                    let y = i / 8;
                    (x, y, Piece::Rook)
                } else {
                    let idx = i - 64;
                    let x = idx % 8;
                    let y = idx / 8;
                    (x, y, Piece::Bishop)
                };

                let piece_name = match piece {
                    Piece::Rook => "Rook",
                    Piece::Bishop => "Bishop",
                    _ => unreachable!(),
                };

                log::info!(
                    "Iteration {}: Improved {} at ({},{}): max_index {} -> {} (saved {} entries)",
                    iteration,
                    piece_name,
                    x,
                    y,
                    old_max_index,
                    new_entry.max_index,
                    old_max_index - new_entry.max_index
                );

                magic_bitboards[i] = new_entry;
                improved = true;
            }
        }

        // Save if we found improvements
        if improved {
            serialize_magic_bitboards_to_file_flat(&magic_bitboards, concat!("src/utils/magic.rs"))
                .expect("Failed to serialize improved magic bitboards");

            let total_entries: usize = magic_bitboards.iter().map(|e| e.max_index + 1).sum();
            log::info!(
                "Total entries after iteration {}: {}",
                iteration,
                total_entries
            );
        }

        if iteration % 10 == 0 {
            let total_entries: usize = magic_bitboards.iter().map(|e| e.max_index + 1).sum();
            log::info!(
                "Completed {} iterations. Total entries: {}",
                iteration,
                total_entries
            );
        }
    }
}
