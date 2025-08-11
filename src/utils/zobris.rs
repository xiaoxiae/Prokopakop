use crate::game::{Color, Piece};
use strum::EnumCount;

pub struct LCG {
    state: u64,
}

impl LCG {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub const fn next_u64(mut self) -> (u64, Self) {
        // https://en.wikipedia.org/wiki/Linear_congruential_generator
        const A: u64 = 1664525;
        const C: u64 = 1013904223;

        self.state = self.state.wrapping_mul(A).wrapping_add(C);

        (self.state, self)
    }
}

pub struct ZobristKeys {
    pub pieces: [[[u64; 64]; Piece::COUNT]; Color::COUNT],
    pub castling: [u64; 16],
    pub en_passant: [u64; 8 + 1], // [0] no state
    pub side_to_move: u64,
}

impl ZobristKeys {
    pub const fn new() -> Self {
        let mut rng = LCG::new(0xbadc0ffee);

        let mut pieces = [[[0u64; 64]; Piece::COUNT]; Color::COUNT];
        let mut color = 0;
        while color < 2 {
            let mut piece = 0;
            while piece < 6 {
                let mut square_idx = 0;
                while square_idx < 64 {
                    let (value, new_rng) = rng.next_u64();
                    pieces[color][piece][square_idx] = value;
                    rng = new_rng;
                    square_idx += 1;
                }

                piece += 1;
            }

            color += 1;
        }

        let mut castling = [0u64; 16];
        let mut castle_idx = 0;
        while castle_idx < 16 {
            let (value, new_rng) = rng.next_u64();
            castling[castle_idx] = value;
            rng = new_rng;
            castle_idx += 1;
        }

        let mut en_passant = [0u64; 8 + 1];
        let mut ep_idx = 0;
        while ep_idx < 8 {
            let (value, new_rng) = rng.next_u64();
            en_passant[ep_idx + 1] = value;
            rng = new_rng;
            ep_idx += 1;
        }

        let (side_to_move, _) = rng.next_u64();

        Self {
            pieces,
            castling,
            en_passant,
            side_to_move,
        }
    }
}

pub static ZOBRIST: ZobristKeys = ZobristKeys::new();
