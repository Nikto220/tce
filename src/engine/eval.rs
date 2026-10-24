//! HalfKP NNUE.
//!
//! Feature transformer: 64 king squares x 10 piece classes x 64 squares -> H, one
//! accumulator per perspective. Then clipped-ReLU dense layers 2H -> L1 -> L2 -> 1.
//!
//! The byte layout and the feature indexing MUST match
//! python/nnue/src/nnue/export.py and python/nnue/src/lib.rs (decode_v2_record).

pub const H: usize = 128;
const L1: usize = 32;
const L2: usize = 32;

const KP: usize = 10 * 64; // piece classes (no kings) * squares
const FEATURES: usize = 64 * KP; // 40960 == PAD in constants.py

const QA: i32 = 255;
const QB: i32 = 64;
const SCALE: i32 = 400;

// i16: ft_w, ft_b, l1_w, l2_w, out_w     i32: l1_b, l2_b, out_b
const NET_BYTES: usize = 2 * (FEATURES * H + H + L1 * 2 * H + L2 * L1 + L2) + 4 * (L1 + L2 + 1);

pub struct Nnue {
    ft_w: Vec<i16>, // FEATURES * H, row = feature (heap: ~10 MB)
    ft_b: [i16; H],
    l1_w: [[i16; 2 * H]; L1], // [out][in]
    l1_b: [i32; L1],
    l2_w: [[i16; L1]; L2], // [out][in]
    l2_b: [i32; L2],
    out_w: [i16; L2],
    out_b: i32,
}

struct Reader<'a> {
    b: &'a [u8],
    o: usize,
}

impl Reader<'_> {
    fn i16(&mut self) -> i16 {
        let v = i16::from_le_bytes([self.b[self.o], self.b[self.o + 1]]);
        self.o += 2;
        v
    }

    fn i32(&mut self) -> i32 {
        let v = i32::from_le_bytes([
            self.b[self.o],
            self.b[self.o + 1],
            self.b[self.o + 2],
            self.b[self.o + 3],
        ]);
        self.o += 4;
        v
    }
}

impl Nnue {
    pub fn from_bytes(b: &[u8]) -> Box<Self> {
        assert_eq!(
            b.len(),
            NET_BYTES,
            "nnue.bin size doesn't match the HalfKP layout (H/L1/L2 or old net?)"
        );

        // Read order == write order in export.py.
        let mut r = Reader { b, o: 0 };

        let ft_w: Vec<i16> = (0..FEATURES * H).map(|_| r.i16()).collect();
        let ft_b = std::array::from_fn(|_| r.i16());

        let l1_w = std::array::from_fn(|_| std::array::from_fn(|_| r.i16()));
        let l1_b = std::array::from_fn(|_| r.i32());

        let l2_w = std::array::from_fn(|_| std::array::from_fn(|_| r.i16()));
        let l2_b = std::array::from_fn(|_| r.i32());

        let out_w = std::array::from_fn(|_| r.i16());
        let out_b = r.i32();

        debug_assert_eq!(r.o, NET_BYTES);

        Box::new(Nnue {
            ft_w,
            ft_b,
            l1_w,
            l1_b,
            l2_w,
            l2_b,
            out_w,
            out_b,
        })
    }

    /// Centipawns from the side to move's point of view.
    pub fn evaluate(&self, acc: &Accumulator, white_to_move: bool) -> i32 {
        let (us, them) = if white_to_move {
            (&acc.v[0], &acc.v[1])
        } else {
            (&acc.v[1], &acc.v[0])
        };

        // Input: clipped accumulators, scale QA, order [us | them] like torch.cat in model.py.
        let mut x = [0i32; 2 * H];
        for i in 0..H {
            x[i] = (us[i] as i32).clamp(0, QA);
            x[H + i] = (them[i] as i32).clamp(0, QA);
        }

        // Layer 1: scale QA*QB -> clip -> back to scale QA.
        let mut h1 = [0i32; L1];
        for o in 0..L1 {
            let w = &self.l1_w[o];
            let mut sum = self.l1_b[o];
            for i in 0..2 * H {
                sum += x[i] * w[i] as i32;
            }
            h1[o] = sum.clamp(0, QA * QB) / QB;
        }

        // Layer 2: same.
        let mut h2 = [0i32; L2];
        for o in 0..L2 {
            let w = &self.l2_w[o];
            let mut sum = self.l2_b[o];
            for i in 0..L1 {
                sum += h1[i] * w[i] as i32;
            }
            h2[o] = sum.clamp(0, QA * QB) / QB;
        }

        // Output: scale QA*QB, the net predicts score / SCALE.
        let mut sum = self.out_b;
        for i in 0..L2 {
            sum += h2[i] * self.out_w[i] as i32;
        }
        sum * SCALE / (QA * QB)
    }
}

/// Square of each king: [white, black].
#[inline]
pub fn king_squares(pieces: &[u64; 12]) -> [usize; 2] {
    [
        pieces[5].trailing_zeros() as usize,
        pieces[11].trailing_zeros() as usize,
    ]
}

/// HalfKP feature index for `persp` (0 = white, 1 = black).
/// Everything is expressed from `persp`'s point of view: its king, its pieces as "own".
#[inline]
fn feature(persp: usize, kings: &[usize; 2], color: usize, pt: usize, sq: usize) -> usize {
    let (ksq, rsq) = if persp == 0 {
        (kings[0], sq)
    } else {
        (kings[1] ^ 56, sq ^ 56)
    };
    let class = if color == persp { pt } else { pt + 5 };
    ksq * KP + class * 64 + rsq
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Accumulator {
    v: [[i16; H]; 2], // [white perspective, black perspective]
}

impl Accumulator {
    pub fn new(net: &Nnue) -> Self {
        Self { v: [net.ft_b; 2] }
    }

    /// Add one piece to ONE perspective. `kings` are the current king squares.
    /// Kings themselves are not features in HalfKP.
    #[inline]
    pub fn add(
        &mut self,
        net: &Nnue,
        persp: usize,
        kings: &[usize; 2],
        color: u8,
        pt: u8,
        sq: usize,
    ) {
        if pt == 5 {
            return;
        }
        let f = feature(persp, kings, color as usize, pt as usize, sq);
        let w = &net.ft_w[f * H..(f + 1) * H];
        for i in 0..H {
            self.v[persp][i] += w[i];
        }
    }

    #[inline]
    pub fn sub(
        &mut self,
        net: &Nnue,
        persp: usize,
        kings: &[usize; 2],
        color: u8,
        pt: u8,
        sq: usize,
    ) {
        if pt == 5 {
            return;
        }
        let f = feature(persp, kings, color as usize, pt as usize, sq);
        let w = &net.ft_w[f * H..(f + 1) * H];
        for i in 0..H {
            self.v[persp][i] -= w[i];
        }
    }

    /// Rebuild one perspective from scratch. Needed whenever that side's king moves,
    /// because every feature index of that perspective depends on the king square.
    pub fn refresh(&mut self, net: &Nnue, persp: usize, pieces: &[u64; 12], kings: &[usize; 2]) {
        self.v[persp] = net.ft_b;
        for piece in 0..12 {
            let mut bb = pieces[piece];
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                bb &= bb - 1;
                self.add(net, persp, kings, (piece / 6) as u8, (piece % 6) as u8, sq);
            }
        }
    }
}

pub const MATE: i32 = INF;
pub const REMIS: i32 = 0;
pub const INF: i32 = 1_000_000;
