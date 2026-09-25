pub const KNIGHT_ATTACKERS: [u64; 64] = generate_knight_attackers();
pub const WHITE_PAWN_ATTACKERS: [u64; 64] = generate_white_pawn_attackers();
pub const BLACK_PAWN_ATTACKERS: [u64; 64] = generate_black_pawn_attackers();
pub const KING_ATTACKERS: [u64; 64] = generate_king_attackers();

const KNIGHT_DIRS: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];

const KING_DIRS: [(i8, i8); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

const fn generate_knight_attackers() -> [u64; 64] {
    let mut attacks = [0u64; 64];

    let mut square = 0;

    while square < 64 {
        let file = (square % 8) as i8;
        let rank = (square / 8) as i8;

        let mut i = 0;

        while i < 8 {
            let new_file = file + KNIGHT_DIRS[i].0;
            let new_rank = rank + KNIGHT_DIRS[i].1;

            if new_file >= 0 && new_file < 8 && new_rank >= 0 && new_rank < 8 {
                let attacker = (new_rank * 8 + new_file) as u8;

                attacks[square] |= 1u64 << attacker;
            }

            i += 1;
        }

        square += 1;
    }

    attacks
}

const fn generate_white_pawn_attackers() -> [u64; 64] {
    let mut attacks = [0u64; 64];

    let mut square = 0;

    while square < 64 {
        let file = square % 8;
        let rank = square / 8;

        // Pion biały, który atakuje square,
        // musi znajdować się jeden rząd niżej.

        if rank > 0 {
            // attacker z lewej
            if file > 0 {
                let attacker = square - 9;
                attacks[square] |= 1u64 << attacker;
            }

            // attacker z prawej
            if file < 7 {
                let attacker = square - 7;
                attacks[square] |= 1u64 << attacker;
            }
        }

        square += 1;
    }

    attacks
}

const fn generate_black_pawn_attackers() -> [u64; 64] {
    let mut attacks = [0u64; 64];

    let mut square = 0;

    while square < 64 {
        let file = square % 8;
        let rank = square / 8;

        // Pion czarny, który atakuje square,
        // musi znajdować się jeden rząd wyżej.

        if rank < 7 {
            // attacker z lewej
            if file > 0 {
                let attacker = square + 7;
                attacks[square] |= 1u64 << attacker;
            }

            // attacker z prawej
            if file < 7 {
                let attacker = square + 9;
                attacks[square] |= 1u64 << attacker;
            }
        }

        square += 1;
    }

    attacks
}

const fn generate_king_attackers() -> [u64; 64] {
    let mut attacks = [0u64; 64];

    let mut square = 0;

    while square < 64 {
        let file = (square % 8) as i8;
        let rank = (square / 8) as i8;

        let mut i = 0;

        while i < 8 {
            let new_file = file + KING_DIRS[i].0;
            let new_rank = rank + KING_DIRS[i].1;

            if new_file >= 0 && new_file < 8 && new_rank >= 0 && new_rank < 8 {
                let attacker = (new_rank * 8 + new_file) as u8;

                attacks[square] |= 1u64 << attacker;
            }

            i += 1;
        }

        square += 1;
    }

    attacks
}

// ---------------------------------------------------------------------------
// Sliding pieces
// ---------------------------------------------------------------------------

pub const ROOK_DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub const BISHOP_DIRS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

/// Reference (ray-walking) implementation. Not used in the hot path;
/// kept as the oracle for tests.
pub fn slow_attacks(sq: u8, occ: u64, dirs: &[(i32, i32)]) -> u64 {
    let mut attacks = 0u64;
    for &(dr, df) in dirs {
        let (mut r, mut f) = ((sq / 8) as i32 + dr, (sq % 8) as i32 + df);
        while (0..8).contains(&r) && (0..8).contains(&f) {
            let bit = 1u64 << (r * 8 + f);
            attacks |= bit;
            if occ & bit != 0 {
                break;
            }
            r += dr;
            f += df;
        }
    }
    attacks
}

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
mod pext_impl;

#[cfg(any(test, not(all(target_arch = "x86_64", target_feature = "bmi2"))))]
mod magic_impl;

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
pub use pext_impl::{rook_attacks, bishop_attacks, queen_attacks};

#[cfg(not(all(target_arch = "x86_64", target_feature = "bmi2")))]
pub use magic_impl::{rook_attacks, bishop_attacks, queen_attacks};


/// Software PDEP: deposits the low bits of `src` into the set-bit positions of `mask`.
/// Only used at compile time to build the tables.
const fn pdep(mut src: u64, mut mask: u64) -> u64 {
    let mut out = 0u64;
    while mask != 0 {
        let lowest = mask & mask.wrapping_neg();
        if src & 1 != 0 {
            out |= lowest;
        }
        src >>= 1;
        mask &= mask - 1;
    }
    out
}

const fn slow_attacks_const(sq: usize, occ: u64, dirs: &[(i32, i32); 4]) -> u64 {
    let mut attacks = 0u64;
    let mut d = 0;
    while d < 4 {
        let (dr, df) = dirs[d];
        let mut r = (sq / 8) as i32 + dr;
        let mut f = (sq % 8) as i32 + df;
        while r >= 0 && r < 8 && f >= 0 && f < 8 {
            let bit = 1u64 << (r * 8 + f);
            attacks |= bit;
            if occ & bit != 0 {
                break;
            }
            r += dr;
            f += df;
        }
        d += 1;
    }
    attacks
}

const ROOK_SIZE: usize = 102_400;
const BISHOP_SIZE: usize = 5_248;

static ROOK_MASKS: [u64; 64] = compute_masks(true);
static ROOK_OFFSETS: [u32; 64] = compute_offsets(&compute_masks(true));

static BISHOP_MASKS: [u64; 64] = compute_masks(false);
static BISHOP_OFFSETS: [u32; 64] = compute_offsets(&compute_masks(false));

const fn compute_masks(rook: bool) -> [u64; 64] {
    let mut masks = [0u64; 64];
    let mut sq = 0;
    while sq < 64 {
        masks[sq] = if rook { rook_mask(sq) } else { bishop_mask(sq) };
        sq += 1;
    }
    masks
}

const fn compute_offsets(masks: &[u64; 64]) -> [u32; 64] {
    let mut offsets = [0u32; 64];
    let mut total = 0u32;
    let mut sq = 0;
    while sq < 64 {
        offsets[sq] = total;
        total += 1 << masks[sq].count_ones();
        sq += 1;
    }
    offsets
}

// --- Masks (relevant occupancy, edges excluded) ------------------------------

const fn rook_mask(sq: usize) -> u64 {
    let (r, f) = (sq / 8, sq % 8);
    let mut m = 0u64;

    let mut i = r + 1;
    while i < 7 {
        m |= 1u64 << (i * 8 + f);
        i += 1;
    }

    i = 1;
    while i < r {
        m |= 1u64 << (i * 8 + f);
        i += 1;
    }

    i = f + 1;
    while i < 7 {
        m |= 1u64 << (r * 8 + i);
        i += 1;
    }

    i = 1;
    while i < f {
        m |= 1u64 << (r * 8 + i);
        i += 1;
    }

    m
}

const fn bishop_mask(sq: usize) -> u64 {
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    let mut m = 0u64;

    let mut d = 0;
    while d < 4 {
        let (dr, df) = BISHOP_DIRS[d];
        let mut rr = r + dr;
        let mut ff = f + df;
        while rr >= 1 && rr <= 6 && ff >= 1 && ff <= 6 {
            m |= 1u64 << (rr * 8 + ff);
            rr += dr;
            ff += df;
        }
        d += 1;
    }

    m
}
