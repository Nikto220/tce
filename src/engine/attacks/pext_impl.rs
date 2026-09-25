use super::*;

// --- PEXT / PDEP -----------------------------------------------------------

/// True when the hardware `pext` instruction is compiled in.
/// Print or assert this at startup so you never benchmark the fallback by accident.
pub const USING_PEXT: bool = cfg!(all(target_arch = "x86_64", target_feature = "bmi2"));

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

static ROOK_TABLE: [u64; ROOK_SIZE] = build_table(true);

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

