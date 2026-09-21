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

pub fn bishop_attacks(square: u8, occupancy: u64) -> u64 {
    let file = (square % 8) as i8;
    let rank = (square / 8) as i8;

    let mut attacks = 0u64;

    // NE
    let mut f = file + 1;
    let mut r = rank + 1;

    while f < 8 && r < 8 {
        let sq = (r * 8 + f) as u8;
        let bit = 1u64 << sq;

        attacks |= bit;

        if occupancy & bit != 0 {
            break;
        }

        f += 1;
        r += 1;
    }

    // NW
    let mut f = file - 1;
    let mut r = rank + 1;

    while f >= 0 && r < 8 {
        let sq = (r * 8 + f) as u8;
        let bit = 1u64 << sq;

        attacks |= bit;

        if occupancy & bit != 0 {
            break;
        }

        f -= 1;
        r += 1;
    }

    // SE
    let mut f = file + 1;
    let mut r = rank - 1;

    while f < 8 && r >= 0 {
        let sq = (r * 8 + f) as u8;
        let bit = 1u64 << sq;

        attacks |= bit;

        if occupancy & bit != 0 {
            break;
        }

        f += 1;
        r -= 1;
    }

    // SW
    let mut f = file - 1;
    let mut r = rank - 1;

    while f >= 0 && r >= 0 {
        let sq = (r * 8 + f) as u8;
        let bit = 1u64 << sq;

        attacks |= bit;

        if occupancy & bit != 0 {
            break;
        }

        f -= 1;
        r -= 1;
    }

    attacks
}

pub fn rook_attacks(square: u8, occupancy: u64) -> u64 {
    let file = (square % 8) as i8;
    let rank = (square / 8) as i8;

    let mut attacks = 0u64;

    // N
    let mut r = rank + 1;

    while r < 8 {
        let sq = (r * 8 + file) as u8;
        let bit = 1u64 << sq;

        attacks |= bit;

        if occupancy & bit != 0 {
            break;
        }

        r += 1;
    }

    // S
    let mut r = rank - 1;

    while r >= 0 {
        let sq = (r * 8 + file) as u8;
        let bit = 1u64 << sq;

        attacks |= bit;

        if occupancy & bit != 0 {
            break;
        }

        r -= 1;
    }

    // E
    let mut f = file + 1;

    while f < 8 {
        let sq = (rank * 8 + f) as u8;
        let bit = 1u64 << sq;

        attacks |= bit;

        if occupancy & bit != 0 {
            break;
        }

        f += 1;
    }

    // W
    let mut f = file - 1;

    while f >= 0 {
        let sq = (rank * 8 + f) as u8;
        let bit = 1u64 << sq;

        attacks |= bit;

        if occupancy & bit != 0 {
            break;
        }

        f -= 1;
    }

    attacks
}
