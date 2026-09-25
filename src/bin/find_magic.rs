// src/bin/find_magics.rs

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

pub const ROOK_DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub const BISHOP_DIRS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

fn xorshift64(mut x: u64) -> u64 {
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

fn find_magic(sq: usize, mask: u64, bits: u32, dirs: &[(i32, i32); 4], seed: &mut u64) -> u64 {
    let n = 1usize << bits;
    let occs: Vec<u64> = (0..n as u64).map(|i| pdep(i, mask)).collect();
    let attacks: Vec<u64> = occs.iter().map(|&o| slow_attacks_const(sq, o, dirs)).collect();

    loop {
        *seed = xorshift64(*seed);
        let r1 = *seed;
        *seed = xorshift64(*seed);
        let r2 = *seed;
        *seed = xorshift64(*seed);
        let r3 = *seed;
        let magic = r1 & r2 & r3; // sparse candidate — fewer set bits, better spread

        // quick reject: a good magic scatters the top byte of mask*magic
        if ((mask.wrapping_mul(magic)) >> 56).count_ones() < 6 {
            continue;
        }

        let mut used = vec![None; n];
        let mut ok = true;
        for i in 0..n {
            let idx = ((occs[i].wrapping_mul(magic)) >> (64 - bits)) as usize;
            match used[idx] {
                None => used[idx] = Some(attacks[i]),
                Some(a) if a == attacks[i] => {} // constructive collision, fine
                Some(_) => { ok = false; break; }
            }
        }
        if ok {
            return magic;
        }
    }
}

fn main() {
    let mut seed = 0x2545F4914F6CDD1Du64;

    println!("pub static ROOK_MAGICS: [u64; 64] = [");
    for sq in 0..64 {
        let mask = rook_mask(sq);
        let bits = mask.count_ones();
        let magic = find_magic(sq, mask, bits, &ROOK_DIRS, &mut seed);
        println!("    0x{magic:016X},");
    }
    println!("];");

    println!("pub static BISHOP_MAGICS: [u64; 64] = [");
    for sq in 0..64 {
        let mask = bishop_mask(sq);
        let bits = mask.count_ones();
        let magic = find_magic(sq, mask, bits, &BISHOP_DIRS, &mut seed);
        println!("    0x{magic:016X},");
    }
    println!("];");
}
