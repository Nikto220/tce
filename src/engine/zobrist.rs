use std::sync::OnceLock;

pub struct Zobrist {
    pub piece: [[u64; 64]; 12],
    pub side: u64,
    pub castling: [u64; 16],
    pub en_passant: [u64; 8],
}

static ZOBRIST: OnceLock<Zobrist> = OnceLock::new();

impl Zobrist {
    pub fn get() -> &'static Self {
        ZOBRIST.get_or_init(|| {
            let mut seed = 0x1234_5678_9ABC_DEF0u64;

            let mut next = || {
                seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = seed;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            };

            Self {
                piece: std::array::from_fn(|_| std::array::from_fn(|_| next())),
                side: next(),
                castling: std::array::from_fn(|_| next()),
                en_passant: std::array::from_fn(|_| next()),
            }
        })
    }
}
