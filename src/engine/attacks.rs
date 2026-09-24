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

// --- PEXT / PDEP -----------------------------------------------------------

/// True when the hardware `pext` instruction is compiled in.
/// Print or assert this at startup so you never benchmark the fallback by accident.
pub const USING_PEXT: bool = cfg!(all(target_arch = "x86_64", target_feature = "bmi2"));

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

/// Software PEXT: the exact inverse of `pdep`. Gathers the bits of `src`
/// selected by `mask` into the low bits of the result.
/// Correct on every platform, but much slower than the instruction.
#[allow(dead_code)] // only called on non-BMI2 builds and in tests
const fn pext_soft(src: u64, mut mask: u64) -> u64 {
    let mut out = 0u64;
    let mut bit = 1u64;
    while mask != 0 {
        let lowest = mask & mask.wrapping_neg();
        if src & lowest != 0 {
            out |= bit;
        }
        bit <<= 1;
        mask &= mask - 1;
    }
    out
}

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
#[inline(always)]
#[allow(unused_unsafe)] // newer compilers treat this intrinsic as safe when the feature is enabled
pub fn pext(src: u64, mask: u64) -> u64 {
    // SAFETY: BMI2 is guaranteed by the cfg on this function.
    unsafe { core::arch::x86_64::_pext_u64(src, mask) }
}

#[cfg(not(all(target_arch = "x86_64", target_feature = "bmi2")))]
#[inline(always)]
fn pext(src: u64, mask: u64) -> u64 {
    pext_soft(src, mask)
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

const fn build_table<const N: usize>(rook: bool) -> [u64; N] {
    let masks = compute_masks(rook);
    let offsets = compute_offsets(&masks);
    let dirs = if rook { ROOK_DIRS } else { BISHOP_DIRS };
    let mut table = [0u64; N];
    let mut sq = 0;
    while sq < 64 {
        let mask = masks[sq];
        let n = 1usize << mask.count_ones();
        let mut i = 0;
        while i < n {
            let occ = pdep(i as u64, mask);
            table[offsets[sq] as usize + i] = slow_attacks_const(sq, occ, &dirs);
            i += 1;
        }
        sq += 1;
    }
    table
}

const ROOK_SIZE: usize = 102_400;
const BISHOP_SIZE: usize = 5_248;

static ROOK_MASKS: [u64; 64] = compute_masks(true);
static ROOK_OFFSETS: [u32; 64] = compute_offsets(&compute_masks(true));
static ROOK_TABLE: [u64; ROOK_SIZE] = build_table(true);

static BISHOP_MASKS: [u64; 64] = compute_masks(false);
static BISHOP_OFFSETS: [u32; 64] = compute_offsets(&compute_masks(false));
static BISHOP_TABLE: [u64; BISHOP_SIZE] = build_table(false);

// --- Lookups -----------------------------------------------------------------

#[inline(always)]
pub fn rook_attacks(sq: usize, occ: u64) -> u64 {
    debug_assert!(sq < 64);
    unsafe {
        let idx = pext(occ, *ROOK_MASKS.get_unchecked(sq)) as usize;
        *ROOK_TABLE.get_unchecked(*ROOK_OFFSETS.get_unchecked(sq) as usize + idx)
    }
}

#[inline(always)]
pub fn bishop_attacks(sq: usize, occ: u64) -> u64 {
    debug_assert!(sq < 64);
    unsafe {
        let idx = pext(occ, *BISHOP_MASKS.get_unchecked(sq)) as usize;
        *BISHOP_TABLE.get_unchecked(*BISHOP_OFFSETS.get_unchecked(sq) as usize + idx)
    }
}

#[inline(always)]
pub fn queen_attacks(sq: usize, occ: u64) -> u64 {
    rook_attacks(sq, occ) | bishop_attacks(sq, occ)
}

// --- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn xorshift(x: &mut u64) -> u64 {
        *x ^= *x << 13;
        *x ^= *x >> 7;
        *x ^= *x << 17;
        *x
    }

    #[test]
    fn pext_soft_inverts_pdep() {
        for sq in 0..64 {
            for mask in [rook_mask(sq), bishop_mask(sq)] {
                let n = 1u64 << mask.count_ones();
                let step = (n / 64).max(1);
                let mut i = 0;
                while i < n {
                    assert_eq!(pext_soft(pdep(i, mask), mask), i);
                    i += step;
                }
            }
        }
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
    #[test]
    fn pext_instruction_matches_soft() {
        let mut s = 0x2545F4914F6CDD1Du64;
        for _ in 0..100_000 {
            let (x, m) = (xorshift(&mut s), xorshift(&mut s));
            assert_eq!(pext(x, m), pext_soft(x, m));
        }
    }

    #[test]
    fn sliders_match_slow() {
        let mut s = 0x9E3779B97F4A7C15u64;
        for _ in 0..200_000 {
            // AND of two randoms gives sparser, more realistic occupancies
            let occ = xorshift(&mut s) & xorshift(&mut s);
            for sq in 0..64usize {
                assert_eq!(
                    rook_attacks(sq, occ),
                    slow_attacks(sq as u8, occ, &ROOK_DIRS),
                    "rook sq={sq} occ={occ:#x}"
                );
                assert_eq!(
                    bishop_attacks(sq, occ),
                    slow_attacks(sq as u8, occ, &BISHOP_DIRS),
                    "bishop sq={sq} occ={occ:#x}"
                );
            }
        }
    }
}
