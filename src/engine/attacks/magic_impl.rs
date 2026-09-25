use super::*;

pub static ROOK_MAGICS: [u64; 64] = [
    0x0980008011400020,
    0x8340004410002000,
    0x0880200090008268,
    0x0080080080100004,
    0x8100110004020800,
    0x0300010004000822,
    0x08801A0029000080,
    0x8100050001204882,
    0x0844800081400320,
    0x0804402010004000,
    0x0108802003100480,
    0x0004808008001000,
    0x0003001801001014,
    0x0002000200041008,
    0x0004008108042210,
    0x0105000100009042,
    0x0400808000400021,
    0xC100404010002000,
    0x0060008010002088,
    0x0400808008001000,
    0x4440808008000400,
    0x1002008004000280,
    0x40024400300D1248,
    0x0010020000408104,
    0x0101008200204200,
    0x8020002040005000,
    0x4100100080802000,
    0x4008006A80100280,
    0x1020080080040080,
    0x0004010040020040,
    0x0018A12400080290,
    0x6140004200008104,
    0x4000400020800090,
    0x2020002080804000,
    0x0000408202002010,
    0x0080100501000820,
    0x0000800400800800,
    0x000A200408014010,
    0x0100800200800100,
    0xA00800570200008C,
    0x008000406000C010,
    0x1040100028002000,
    0x0048200100110040,
    0x0068490210030020,
    0x1009080005010010,
    0x2142000804010100,
    0x1001080110840002,
    0x1801004400820001,
    0x010440208D020200,
    0x0000400020008080,
    0x0200200080100280,
    0x0000100020090100,
    0x0204008008020480,
    0x8104010040020040,
    0x78000201B0080400,
    0x0040800051002880,
    0x0050108001002041,
    0x208A801100614003,
    0x0006002042089082,
    0x0011090004201001,
    0x1002001004200802,
    0x0005000208040001,
    0x0002002701AC0822,
    0x000010250184004A,
];
pub static BISHOP_MAGICS: [u64; 64] = [
    0xC0A0012206040EA0,
    0x8010228200420001,
    0x0110008220400400,
    0x02445C0080106000,
    0x0044042004008100,
    0x0880900420408C05,
    0x0201080110080002,
    0x0000108094202000,
    0x0000042002040108,
    0x0000623024110042,
    0x0086100094811002,
    0x0000044502002080,
    0x0100460211400040,
    0x0008109004200004,
    0x0202320084844000,
    0x8040042421041009,
    0x201010C05102008C,
    0x1020888208024080,
    0x0108000C80290200,
    0x8048000420425203,
    0x0005000090402000,
    0x2080400201104100,
    0x8820420111101000,
    0x4AC0302208821802,
    0x000440001002A840,
    0x2002200010041080,
    0x1012080201004400,
    0x8440040002410120,
    0x1090820084010400,
    0x2084852012021000,
    0x12040062C1011003,
    0x02008205E1090080,
    0x088C102808042080,
    0x0802102200904280,
    0x8020209002080020,
    0x2200080800060A00,
    0x20C0004010010100,
    0x0802004100821003,
    0x0008024400008080,
    0x0000840102008090,
    0x0030A40420244007,
    0x0A19084210011282,
    0x0004082090019806,
    0x6108004208020080,
    0x0081200410110100,
    0x1040810701010208,
    0x0282047832012080,
    0x0010020099000020,
    0x000E010422400840,
    0x10204208B0089090,
    0x081004440C048000,
    0x88C0180084040001,
    0x3100020803040080,
    0x890070A041210C00,
    0x0020200101010A09,
    0x0004100240410400,
    0x0006004402080200,
    0x0801062484042000,
    0x00010002D7441004,
    0x0810080000208800,
    0x0000020808030411,
    0x1450001020014440,
    0x004060081081A288,
    0x0044011404108A00,
];

const fn magic_index(occ: u64, mask: u64, magic: u64, shift: u32) -> usize {
    (((occ & mask).wrapping_mul(magic)) >> shift) as usize
}

const fn shift_for(mask: u64) -> u32 {
    64 - mask.count_ones()
}

const fn build_magic_table<const N: usize>(
    masks: &[u64; 64],
    magics: &[u64; 64],
    offsets: &[u32; 64],
    dirs: &[(i32, i32); 4],
) -> [u64; N] {
    let mut table = [0u64; N];
    let mut sq = 0;
    while sq < 64 {
        let mask = masks[sq];
        let magic = magics[sq];
        let shift = shift_for(mask);
        let n = 1usize << mask.count_ones();
        let mut i = 0;
        while i < n {
            let occ = pdep(i as u64, mask);
            let idx = magic_index(occ, mask, magic, shift);
            table[offsets[sq] as usize + idx] = slow_attacks_const(sq, occ, dirs);
            i += 1;
        }
        sq += 1;
    }
    table
}

static ROOK_MAGIC_TABLE: [u64; ROOK_SIZE] =
    build_magic_table(&ROOK_MASKS, &ROOK_MAGICS, &ROOK_OFFSETS, &ROOK_DIRS);
static BISHOP_MAGIC_TABLE: [u64; BISHOP_SIZE] =
    build_magic_table(&BISHOP_MASKS, &BISHOP_MAGICS, &BISHOP_OFFSETS, &BISHOP_DIRS);

const fn compute_shifts(masks: &[u64; 64]) -> [u32; 64] {
    let mut shifts = [0u32; 64];
    let mut sq = 0;
    while sq < 64 {
        shifts[sq] = shift_for(masks[sq]);
        sq += 1;
    }
    shifts
}

static ROOK_SHIFTS: [u32; 64] = compute_shifts(&ROOK_MASKS);
static BISHOP_SHIFTS: [u32; 64] = compute_shifts(&BISHOP_MASKS);

#[inline(always)]
pub fn rook_attacks(sq: usize, occ: u64) -> u64 {
    unsafe {
        let mask = *ROOK_MASKS.get_unchecked(sq);
        let magic = *ROOK_MAGICS.get_unchecked(sq);
        let shift = *ROOK_SHIFTS.get_unchecked(sq);
        let idx = magic_index(occ, mask, magic, shift);
        *ROOK_MAGIC_TABLE.get_unchecked(*ROOK_OFFSETS.get_unchecked(sq) as usize + idx)
    }
}

#[inline(always)]
pub fn bishop_attacks(sq: usize, occ: u64) -> u64 {
    unsafe {
        let mask = *BISHOP_MASKS.get_unchecked(sq);
        let magic = *BISHOP_MAGICS.get_unchecked(sq);
        let shift = *BISHOP_SHIFTS.get_unchecked(sq);
        let idx = magic_index(occ, mask, magic, shift);
        *BISHOP_MAGIC_TABLE.get_unchecked(*BISHOP_OFFSETS.get_unchecked(sq) as usize + idx)
    }
}

#[inline(always)]
pub fn queen_attacks(sq: usize, occ: u64) -> u64 {
    rook_attacks(sq, occ) | bishop_attacks(sq, occ)
}

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
    fn magic_sliders_match_slow() {
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
