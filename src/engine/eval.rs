use super::super::board::Board;

const H: usize = 128;
const QA: i32 = 255;
const QB: i32 = 64;
const SCALE: i32 = 400;

const NET_BYTES: usize = (768 * H + H + 2 * H) * 2 + 4;

pub struct Nnue {
    ft_w: [[i16; H]; 768],
    ft_b: [i16; H],
    out_w: [i16; 2 * H],
    out_b: i32,
}

impl Nnue {
    pub fn from_bytes(b: &[u8]) -> Box<Self> {
        assert_eq!(b.len(), NET_BYTES, "nnue.bin size doesn't match H");
        let mut net = Box::new(Nnue {
            ft_w: [[0; H]; 768], ft_b: [0; H], out_w: [0; 2 * H], out_b: 0,
        });
        let mut o = 0;
        let i16_at = |o: &mut usize| { let v = i16::from_le_bytes([b[*o], b[*o + 1]]); *o += 2; v };
        for r in 0..768 { for c in 0..H { net.ft_w[r][c] = i16_at(&mut o); } }
        for c in 0..H { net.ft_b[c] = i16_at(&mut o); }
        for c in 0..2 * H { net.out_w[c] = i16_at(&mut o); }
        net.out_b = i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        net
    }

    pub fn evaluate(&self, acc: &Accumulator, white_to_move: bool) -> i32 {
        let (us, them) = if white_to_move { (&acc.v[0], &acc.v[1]) } else { (&acc.v[1], &acc.v[0]) };
        let mut sum = self.out_b;
        for i in 0..H {
            sum += (us[i] as i32).clamp(0, QA) * self.out_w[i] as i32;
            sum += (them[i] as i32).clamp(0, QA) * self.out_w[H + i] as i32;
        }
        sum * SCALE / (QA * QB)
    }
}

#[inline]
fn feature(persp: usize, color: usize, pt: usize, sq: usize) -> usize {
    // persp/color: 0 = white, 1 = black
    let rel_color = color ^ persp;                       // 0 = own pieces
    let rel_sq = if persp == 0 { sq } else { sq ^ 56 };
    rel_color * 384 + pt * 64 + rel_sq
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Accumulator {
    v: [[i16; H]; 2], // [white perspective, black perspective]
}

impl Accumulator {
    pub fn new(net: &Nnue) -> Self {
        Self { v: [net.ft_b; 2] }
    }

    #[inline]
    pub fn add(&mut self, net: &Nnue, color: usize, pt: usize, sq: usize) {
        for p in 0..2 {
            let w = &net.ft_w[feature(p, color, pt, sq)];
            for i in 0..H { self.v[p][i] += w[i]; }
        }
    }

    #[inline]
    pub fn sub(&mut self, net: &Nnue, color: usize, pt: usize, sq: usize) {
        for p in 0..2 {
            let w = &net.ft_w[feature(p, color, pt, sq)];
            for i in 0..H { self.v[p][i] -= w[i]; }
        }
    }
}

pub const MATE: i32 = INF;
pub const REMIS: i32 = 0;
pub const INF: i32 = 1_000_000;
