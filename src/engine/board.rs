use super::zobrist::*;

use super::attacks::*;

pub const A1: u8 = 0;
pub const B1: u8 = 1;
pub const C1: u8 = 2;
pub const D1: u8 = 3;
pub const E1: u8 = 4;
pub const F1: u8 = 5;
pub const G1: u8 = 6;
pub const H1: u8 = 7;

pub const A8: u8 = 56;
pub const B8: u8 = 57;
pub const C8: u8 = 58;
pub const D8: u8 = 59;
pub const E8: u8 = 60;
pub const F8: u8 = 61;
pub const G8: u8 = 62;
pub const H8: u8 = 63;

pub const WP: u8 = 0;
pub const WN: u8 = 1;
pub const WB: u8 = 2;
pub const WR: u8 = 3;
pub const WQ: u8 = 4;
pub const WK: u8 = 5;

pub const BP: u8 = 6;
pub const BN: u8 = 7;
pub const BB: u8 = 8;
pub const BR: u8 = 9;
pub const BQ: u8 = 10;
pub const BK: u8 = 11;

pub const NN: u8 = 12;

pub const WK_CASTLE: u8 = 1 << 0;
pub const WQ_CASTLE: u8 = 1 << 1;
pub const BK_CASTLE: u8 = 1 << 2;
pub const BQ_CASTLE: u8 = 1 << 3;

const NOT_A_FILE: u64 = 0xfefe_fefe_fefe_fefe;
const NOT_H_FILE: u64 = 0x7f7f_7f7f_7f7f_7f7f;

const RANK_2: u64 = 0x0000_0000_0000_ff00;
const RANK_7: u64 = 0x00ff_0000_0000_0000;
const RANK_8: u64 = 0xff00_0000_0000_0000;
const RANK_1: u64 = 0x0000_0000_0000_00ff;
const RANK_3: u64 = 0x0000_0000_00FF_0000;
const RANK_6: u64 = 0x0000_FF00_0000_0000;

const MAX_MOVES: usize = 256;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Board {
    pieces: [u64; 12],

    occupancy: u64,
    white_occ: u64,
    black_occ: u64,

    piece_on: [u8; 64],

    turn: bool,
    castling: u8,
    en_passant: u8,

    hash: u64,
}

struct CheckInfo {
    king_sq: u8,

    // 0 = not in check
    // 1 bit = single check
    // 2+ bits = double check
    checkers: u64,

    checker_count: usize,

    // Squares that can resolve a single check.
    evasion_mask: u64,

    // Own pieces that are absolutely pinned.
    pinned: u64,

    // Legal movement line for each pinned piece.
    pin_rays: [u64; 64],
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(transparent)]
pub struct Move(u32);

impl Move {
    pub const NONE: Self = Move(0);

    const FROM_SHIFT: u32 = 26;
    const   TO_SHIFT: u32 = 20;
    const MVTP_SHIFT: u32 = 17;
    const PROM_SHIFT: u32 = 13;

    const FROM_MASK: u32 = 0b111111 << Self::FROM_SHIFT;
    const   TO_MASK: u32 = 0b111111 <<   Self::TO_SHIFT;
    const MVTP_MASK: u32 = 0b111    << Self::MVTP_SHIFT;
    const PROM_MASK: u32 = 0b1111   << Self::PROM_SHIFT;

    pub const MVTP_QUIET: u8 = 0;
    pub const MVTP_CAPTURE: u8 = 1;
    pub const MVTP_DOUBLE_PAWN_PUSH: u8 = 2;
    pub const MVTP_EN_PASSANT: u8 = 3;
    pub const MVTP_CASTLE_KING_SIDE: u8 = 4;
    pub const MVTP_CASTLE_QUEEN_SIDE: u8 = 5;
    pub const MVTP_PROMOTION: u8 = 6;
    pub const MVTP_PROMOTION_CAPTURE: u8 = 7;

    #[inline(always)]
    pub const fn new(from: u8, to: u8, move_type: u8, promotion: u8) -> Self {
        Self(((from as u32) << Self::FROM_SHIFT) | ((to as u32) << Self::TO_SHIFT) | ((move_type as u32) << Self::MVTP_SHIFT) | ((promotion as u32) << Self::PROM_SHIFT))
    }

    #[inline(always)]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[inline(always)]
    pub const fn to_raw(self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub const fn from(&self) -> u8 {
        ((self.0 & Self::FROM_MASK) >> Self::FROM_SHIFT) as u8
    }

    #[inline(always)]
    pub const fn to(&self) -> u8 {
        ((self.0 & Self::TO_MASK) >> Self::TO_SHIFT) as u8
    }

    #[inline(always)]
    pub const fn move_type(&self) -> u8 {
        ((self.0 & Self::MVTP_MASK) >> Self::MVTP_SHIFT) as u8
    }

    #[inline(always)]
    pub const fn promotion(&self) -> u8 {
        ((self.0 & Self::PROM_MASK) >> Self::PROM_SHIFT) as u8
    }
}

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            format!(
                "{}{}{}",
                str_from_square(self.from()),
                str_from_square(self.to()),
                if self.move_type() == Move::MVTP_PROMOTION
                    || self.move_type() == Move::MVTP_PROMOTION_CAPTURE {
                    match self.promotion() {
                        BN | WN => "n",
                        BB | WB => "b",
                        BR | WR => "r",
                        BQ | WQ => "q",
                        _ => unreachable!(),
                    }
                } else {
                    ""
                }
            )
            .as_str(),
        )?;

        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Undo {
    pub captured_piece: u8,
    pub captured_square: u8,

    pub moved_piece: u8,

    pub castling: u8,
    pub en_passant: u8,

    pub mv: Move,

    pub hash: u64,
}

pub struct MoveArray {
    moves: [Move; MAX_MOVES],
    len: usize,
}

impl MoveArray {
    pub fn new() -> Self {
        Self {
            moves: [Move::NONE; MAX_MOVES],
            len: 0,
        }
    }

    pub fn add_element(&mut self, mv: Move) {
        if self.len >= MAX_MOVES {
            panic!("not enough capacity");
        }

        self.moves[self.len] = mv;

        self.len += 1;
    }

    pub fn empty(&mut self) {
        self.len = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn swap(&mut self, i1: usize, i2: usize) {
        let temp = self.moves[i1];
        self.moves[i1] = self.moves[i2];
        self.moves[i2] = temp;
    }

    pub fn order_pv_move(&mut self, pv_move: Option<Move>, idx_to_swap: usize) {
        if idx_to_swap >= self.len {
            return;
        }

        let Some(pv_move) = pv_move else {
            return;
        };

        for i in 0..self.len() {
            if self.moves[i] == pv_move {
                self.swap(idx_to_swap, i);
                return;
            }
        }
    }
}

impl<'a> IntoIterator for &'a MoveArray {
    type Item = &'a Move;
    type IntoIter = core::slice::Iter<'a, Move>;

    fn into_iter(self) -> Self::IntoIter {
        self.moves[..self.len].iter()
    }
}

impl Board {
    pub fn new() -> Self {
        let pieces = [
            0x0000_0000_0000_FF00u64, // WP
            0x0000_0000_0000_0042u64, // WN
            0x0000_0000_0000_0024u64, // WB
            0x0000_0000_0000_0081u64, // WR
            0x0000_0000_0000_0008u64, // WQ
            0x0000_0000_0000_0010u64, // WK
            0x00FF_0000_0000_0000u64, // BP
            0x4200_0000_0000_0000u64, // BN
            0x2400_0000_0000_0000u64, // BB
            0x8100_0000_0000_0000u64, // BR
            0x0800_0000_0000_0000u64, // BQ
            0x1000_0000_0000_0000u64, // BK
        ];

        let mut piece_on = [0u8; 64];

        for piece in 0..12 {
            let mut bb = pieces[piece];

            while bb != 0 {
                let square = bb.trailing_zeros() as usize;
                bb &= bb - 1;

                piece_on[square] = (piece + 1) as u8;
            }
        }

        let white_occ = pieces[0] | pieces[1] | pieces[2] | pieces[3] | pieces[4] | pieces[5];
        let black_occ = pieces[6] | pieces[7] | pieces[8] | pieces[9] | pieces[10] | pieces[11];
        let mut instance = Self {
            pieces,
            white_occ,
            black_occ,
            occupancy: white_occ | black_occ,
            piece_on,
            turn: true,
            castling: 0b00001111,
            en_passant: u8::MAX,
            hash: 0,
        };
        instance.hash = instance.recompute_hash();
        instance
    }

    #[inline]
    pub fn zobrist_hash(&self) -> u64 {
        self.hash
    }

    #[inline]
    fn ep_file(en_passant: u8) -> u8 {
        en_passant % 8
    }

    #[inline]
    fn set_castling_hash(&mut self, new_castling: u8) {
        let z = Zobrist::get();

        self.hash ^= z.castling[(self.castling & 0x0F) as usize];
        self.castling = new_castling;
        self.hash ^= z.castling[(self.castling & 0x0F) as usize];
    }

    #[inline]
    fn set_en_passant_hash(&mut self, new_ep: u8) {
        let z = Zobrist::get();

        let file = Self::ep_file(self.en_passant);
        if file != u8::MAX {
            self.hash ^= z.en_passant[file as usize];
        }

        self.en_passant = new_ep;

        let file = Self::ep_file(self.en_passant);
        if file != u8::MAX {
            self.hash ^= z.en_passant[file as usize];
        }
    }

    fn recompute_hash(&self) -> u64 {
        let z = Zobrist::get();
        let mut hash = z.castling[(self.castling & 0x0F) as usize];

        if self.turn {
            hash ^= z.side;
        }

        let file = Self::ep_file(self.en_passant);
        if file != u8::MAX {
            hash ^= z.en_passant[file as usize];
        }

        for piece in 0..12 {
            let mut bb = self.pieces[piece];

            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                bb &= bb - 1;
                hash ^= z.piece[piece][sq];
            }
        }

        hash
    }

    pub fn position_startpos(&mut self) {
        *self = Self::new();
    }

    pub fn get_board(&self) -> [u64; 12] {
        self.pieces
    }

    pub fn get_turn(&self) -> bool {
        self.turn
    }

    pub fn position_startpos_moves(&mut self, moves: &[&str]) {
        self.position_startpos();

        for move_str in moves {
            let mv = match self.parse_move(move_str) {
                Some(mv) => mv,
                None => {
                    println!("invalid move {}", move_str);
                    continue;
                }
            };

            self.make_move(mv);
        }
    }

    pub fn make_move(&mut self, mv: Move) -> Undo {
        let mv_from = mv.from();
        let mv_to = mv.to();

        let piece = self.piece_at(mv_from);
        assert_ne!(piece, NN);
        //println!("move: {}", mv);

        let mut undo = Undo {
            mv,
            moved_piece: piece,
            captured_piece: NN,
            captured_square: NN,
            castling: self.castling,
            en_passant: self.en_passant,
            hash: self.hash,
        };

        //println!("{:#?}", mv);

        // --------------------------------------------------
        // 1. BICIE
        // --------------------------------------------------

        match mv.move_type() {
            Move::MVTP_PROMOTION | Move::MVTP_PROMOTION_CAPTURE => {
                if let piece = self.piece_at(mv_to) && piece != NN {
                    self.remove_piece(piece, mv_to);

                    undo.captured_piece = piece;
                    undo.captured_square = mv_to;
                }
            }

            Move::MVTP_EN_PASSANT => {
                let captured_square = if self.turn { mv_to - 8 } else { mv_to + 8 };

                let captured_piece = if self.turn { BP } else { WP };

                self.remove_piece(captured_piece, captured_square);

                undo.captured_piece = captured_piece;
                undo.captured_square = captured_square;
            }

            Move::MVTP_CAPTURE => {
                if let captured_piece = self.piece_at(mv_to) && captured_piece != NN {
                    self.remove_piece(captured_piece, mv_to);

                    undo.captured_piece = captured_piece;
                    undo.captured_square = mv_to;
                }
            }

            _ => {}
        }

        // --------------------------------------------------
        // 2. NORMALNY RUCH / PROMOCJA
        // --------------------------------------------------

        match mv.move_type() {
            Move::MVTP_PROMOTION | Move::MVTP_PROMOTION_CAPTURE => {
                // usuwamy pionka
                self.remove_piece(piece, mv_from);

                // dodajemy promowaną figurę
                let promoted_piece = mv.promotion();

                self.add_piece(promoted_piece, mv_to);
            }

            Move::MVTP_CASTLE_KING_SIDE => {
                if self.turn {
                    // biały: e1 -> g1
                    self.move_piece(WK, E1, G1);

                    // wieża: h1 -> f1
                    self.move_piece(WR, H1, F1);
                } else {
                    // czarny: e8 -> g8
                    self.move_piece(BK, E8, G8);

                    // wieża: h8 -> f8
                    self.move_piece(BR, H8, F8);
                }
            }

            Move::MVTP_CASTLE_QUEEN_SIDE => {
                if self.turn {
                    // biały: e1 -> c1
                    self.move_piece(WK, E1, C1);

                    // wieża: a1 -> d1
                    self.move_piece(WR, A1, D1);
                } else {
                    // czarny: e8 -> c8
                    self.move_piece(BK, E8, C8);

                    // wieża: a8 -> d8
                    self.move_piece(BR, A8, D8);
                }
            }

            _ => {
                self.move_piece(piece, mv_from, mv_to);
            }
        }

        // --------------------------------------------------
        // 3. ROSZADA
        // --------------------------------------------------

        self.update_castling_rights(piece, mv_from);

        if undo.captured_piece != NN && undo.captured_square != u8::MAX {
            self.update_castling_rights_capture(undo.captured_square, undo.captured_square);
        }

        // --------------------------------------------------
        // 4. EN PASSANT
        // --------------------------------------------------

        self.set_en_passant_hash(u8::MAX);

        if matches!(mv.move_type(), Move::MVTP_DOUBLE_PAWN_PUSH) {
            let ep = if self.turn { mv_from + 8 } else { mv_from - 8 };

            self.set_en_passant_hash(ep);
        }

        self.turn = !self.turn;
        self.hash ^= Zobrist::get().side;

        undo
    }

    pub fn unmake_move(&mut self, undo: Undo) {
        let mv = undo.mv;
        let mv_type = mv.move_type();
        let mv_from = mv.from();
        let mv_to = mv.to();
        // Najpierw wracamy do strony, która wykonała ruch
        self.turn = !self.turn;

        let piece = match mv_type {
            Move::MVTP_PROMOTION | Move::MVTP_PROMOTION_CAPTURE => {
                // Promocja:
                // usuwamy promowaną figurę
                let promoted_piece = mv.promotion();

                self.remove_piece(promoted_piece, mv_to);

                // i przywracamy pionka
                if self.turn { WP } else { BP }
            }

            _ => {
                let p = self.piece_at(mv_to);
                assert_ne!(p, NN);
                p
            },
        };

        // --------------------------------------------------
        // ROSZADA
        // --------------------------------------------------

        match mv.move_type() {
            Move::MVTP_CASTLE_KING_SIDE => {
                if self.turn {
                    self.move_piece(WK, G1, E1);
                    self.move_piece(WR, F1, H1);
                } else {
                    self.move_piece(BK, G8, E8);
                    self.move_piece(BR, F8, H8);
                }
            }

            Move::MVTP_CASTLE_QUEEN_SIDE => {
                if self.turn {
                    self.move_piece(WK, C1, E1);
                    self.move_piece(WR, D1, A1);
                } else {
                    self.move_piece(BK, C8, E8);
                    self.move_piece(BR, D8, A8);
                }
            }

            Move::MVTP_PROMOTION | Move::MVTP_PROMOTION_CAPTURE => {
                let pawn = if self.turn { WP } else { BP };

                self.add_piece(pawn, mv_from);
            }

            _ => {
                self.move_piece(piece, mv_to, mv_from);
            }
        }

        // --------------------------------------------------
        // PRZYWRÓCENIE ZBITEJ FIGURY
        // --------------------------------------------------

        if undo.captured_piece != NN && undo.captured_square != u8::MAX {
            self.add_piece(undo.captured_piece, undo.captured_square);
        }

        // --------------------------------------------------
        // PRZYWRÓCENIE STANU
        // --------------------------------------------------

        self.castling = undo.castling;
        self.en_passant = undo.en_passant;
        self.hash = undo.hash;

        #[cfg(debug_assertions)]
        debug_assert_eq!(
            self.hash,
            self.recompute_hash(),
            "Zobrist hash mismatch after unmake_move"
        );
    }

    #[inline(always)]
    fn piece_at(&self, square: u8) -> u8 {
        let p = self.piece_on[square as usize];

        if p == 0 { NN } else { p - 1 }
    }

    fn generate_pawn_moves(&mut self, mv_arr: &mut MoveArray, check_info: &CheckInfo) {
        let (pawns, enemy, pinned) = if self.turn {
            (self.pieces[WP as usize], self.black_occ, check_info.pinned)
        } else {
            (self.pieces[BP as usize], self.white_occ, check_info.pinned)
        };

        let empty = !self.occupancy;

        if self.turn {
            self.generate_white_pawns(pawns, enemy, empty, pinned, check_info, mv_arr);
        } else {
            self.generate_black_pawns(pawns, enemy, empty, pinned, check_info, mv_arr);
        }
    }

    fn generate_white_pawns(
        &mut self,
        pawns: u64,
        enemy: u64,
        empty: u64,
        pinned: u64,
        check_info: &CheckInfo,
        mv_arr: &mut MoveArray,
    ) {
        // --------------------------------------------------
        // SINGLE PUSH
        // --------------------------------------------------

        let single = (pawns << 8) & empty;

        let promotion_pushes = single & RANK_8;
        let quiet_pushes = single & !RANK_8;

        // Normal pawn pushes
        let mut targets = quiet_pushes;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to - 8;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            mv_arr.add_element(Move::new(
                from,
                to,
                Move::MVTP_QUIET,
                NN,
            ));
        }

        // Promotions
        let mut targets = promotion_pushes;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to - 8;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            self.add_white_promotions(from, to, false, mv_arr);
        }

        // --------------------------------------------------
        // DOUBLE PUSH
        // --------------------------------------------------

        let double = ((pawns & RANK_2) << 8 & empty) << 8 & empty;

        let mut targets = double;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to - 16;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            mv_arr.add_element(Move::new(
                from,
                to,
                Move::MVTP_DOUBLE_PAWN_PUSH,
                NN,
            ));
        }

        // --------------------------------------------------
        // CAPTURE LEFT
        // --------------------------------------------------

        let capture_left = (pawns & NOT_A_FILE) << 7 & enemy;

        let promotion_captures = capture_left & RANK_8;
        let normal_captures = capture_left & !RANK_8;

        let mut targets = normal_captures;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to - 7;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            mv_arr.add_element(Move::new(
                from,
                to,
                Move::MVTP_CAPTURE,
                NN,
            ));
        }

        let mut targets = promotion_captures;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to - 7;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            self.add_white_promotions(from, to, true, mv_arr);
        }

        // --------------------------------------------------
        // CAPTURE RIGHT
        // --------------------------------------------------

        let capture_right = (pawns & NOT_H_FILE) << 9 & enemy;

        let promotion_captures = capture_right & RANK_8;
        let normal_captures = capture_right & !RANK_8;

        let mut targets = normal_captures;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to - 9;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            mv_arr.add_element(Move::new(
                from,
                to,
                Move::MVTP_CAPTURE,
                NN,
            ));
        }

        let mut targets = promotion_captures;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to - 9;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            self.add_white_promotions(from, to, true, mv_arr);
        }

        // --------------------------------------------------
        // EN PASSANT
        // --------------------------------------------------

        if self.en_passant != u8::MAX {
            let attackers = WHITE_PAWN_ATTACKERS[self.en_passant as usize] & pawns;

            let mut attackers = attackers;

            while attackers != 0 {
                let from = attackers.trailing_zeros() as u8;
                attackers &= attackers - 1;

                if !self.en_passant_allowed(from, self.en_passant, true, check_info) {
                    continue;
                }

                mv_arr.add_element(Move::new(
                    from,
                    self.en_passant,
                    Move::MVTP_EN_PASSANT,
                    NN,
                ));
            }
        }
    }

    #[inline(always)]
    fn add_white_promotions(&self, from: u8, to: u8, capture: bool, mv_arr: &mut MoveArray) {
        let move_type = if capture {
            Move::MVTP_PROMOTION_CAPTURE
        } else {
            Move::MVTP_PROMOTION
        };

        mv_arr.add_element(Move::new(
            from,
            to,
            move_type,
            WQ,
        ));

        mv_arr.add_element(Move::new(
            from,
            to,
            move_type,
            WR,
        ));

        mv_arr.add_element(Move::new(
            from,
            to,
            move_type,
            WB,
        ));

        mv_arr.add_element(Move::new(
            from,
            to,
            move_type,
            WN,
        ));
    }

    fn generate_black_pawns(
        &mut self,
        pawns: u64,
        enemy: u64,
        empty: u64,
        pinned: u64,
        check_info: &CheckInfo,
        mv_arr: &mut MoveArray,
    ) {
        // --------------------------------------------------
        // SINGLE PUSH
        // --------------------------------------------------

        let single = (pawns >> 8) & empty;

        let promotion_pushes = single & RANK_1;
        let quiet_pushes = single & !RANK_1;

        let mut targets = quiet_pushes;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to + 8;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            mv_arr.add_element(Move::new(
                from,
                to,
                Move::MVTP_QUIET,
                NN,
            ));
        }

        // Promotions
        let mut targets = promotion_pushes;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to + 8;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            self.add_black_promotions(from, to, false, mv_arr);
        }

        // --------------------------------------------------
        // DOUBLE PUSH
        // --------------------------------------------------

        let double = ((pawns & RANK_7) >> 8 & empty) >> 8 & empty;

        let mut targets = double;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to + 16;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            mv_arr.add_element(Move::new(
                from,
                to,
                Move::MVTP_DOUBLE_PAWN_PUSH,
                NN,
            ));
        }

        // --------------------------------------------------
        // CAPTURE LEFT
        // --------------------------------------------------

        let capture_left = (pawns & NOT_H_FILE) >> 7 & enemy;

        let promotion_captures = capture_left & RANK_1;
        let normal_captures = capture_left & !RANK_1;

        let mut targets = normal_captures;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to + 7;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            mv_arr.add_element(Move::new(
                from,
                to,
                Move::MVTP_CAPTURE,
                NN,
            ));
        }

        let mut targets = promotion_captures;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to + 7;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            self.add_black_promotions(from, to, true, mv_arr);
        }

        // --------------------------------------------------
        // CAPTURE RIGHT
        // --------------------------------------------------

        let capture_right = (pawns & NOT_A_FILE) >> 9 & enemy;

        let promotion_captures = capture_right & RANK_1;
        let normal_captures = capture_right & !RANK_1;

        let mut targets = normal_captures;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to + 9;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            mv_arr.add_element(Move::new(
                from,
                to,
                Move::MVTP_CAPTURE,
                NN,
            ));
        }

        let mut targets = promotion_captures;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let from = to + 9;

            if !pawn_move_allowed(from, to, pinned, check_info) {
                continue;
            }

            self.add_black_promotions(from, to, true, mv_arr);
        }

        // --------------------------------------------------
        // EN PASSANT
        // --------------------------------------------------

        if self.en_passant != u8::MAX {
            let attackers = BLACK_PAWN_ATTACKERS[self.en_passant as usize] & pawns;

            let mut attackers = attackers;

            while attackers != 0 {
                let from = attackers.trailing_zeros() as u8;
                attackers &= attackers - 1;

                if !self.en_passant_allowed(from, self.en_passant, false, check_info) {
                    continue;
                }

                mv_arr.add_element(Move::new(
                    from,
                    self.en_passant,
                    Move::MVTP_EN_PASSANT,
                    NN,
                ));
            }
        }
    }

    #[inline(always)]
    fn add_black_promotions(&self, from: u8, to: u8, capture: bool, mv_arr: &mut MoveArray) {
        let move_type = if capture {
            Move::MVTP_PROMOTION_CAPTURE
        } else {
            Move::MVTP_PROMOTION
        };

        mv_arr.add_element(Move::new(
            from,
            to,
            move_type,
            BQ,
        ));

        mv_arr.add_element(Move::new(
            from,
            to,
            move_type,
            BR,
        ));

        mv_arr.add_element(Move::new(
            from,
            to,
            move_type,
            BB,
        ));

        mv_arr.add_element(Move::new(
            from,
            to,
            move_type,
            BN,
        ));
    }

    fn en_passant_allowed(
        &mut self,
        from: u8,
        to: u8,
        white: bool,
        check_info: &CheckInfo,
    ) -> bool {
        let from_bb = 1u64 << from;
        let to_bb = 1u64 << to;

        // Double check: EP cannot be legal because only the king
        // may move in double check.
        if check_info.checker_count >= 2 {
            return false;
        }

        // A pinned pawn may not move off its pin ray.
        if check_info.pinned & from_bb != 0 && (check_info.pin_rays[from as usize] & to_bb) == 0 {
            return false;
        }

        // If currently in check, EP can only work if it resolves the check.
        //
        // EP is special because the captured pawn disappears from
        // a different square, so evasion_mask alone is insufficient.
        let undo = self.make_move(Move::new(
            from,
            to,
            Move::MVTP_EN_PASSANT,
            NN,
        ));

        let legal = !self.is_in_check(white);

        self.unmake_move(undo);

        legal
    }

    fn generate_knight_moves(&self, info: &CheckInfo, mv_arr: &mut MoveArray) {
        let own = if self.turn {
            self.white_occ
        } else {
            self.black_occ
        };

        let enemy = if self.turn {
            self.black_occ
        } else {
            self.white_occ
        };

        let mut pieces = self.pieces[if self.turn { WN } else { BN } as usize];

        while pieces != 0 {
            let from = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;

            // Knights cannot legally move if absolutely pinned.
            if info.pinned & (1u64 << from) != 0 {
                continue;
            }

            let mut targets = KNIGHT_ATTACKERS[from as usize] & !own;

            // If in check, knight must capture checker.
            // Knights cannot block a sliding check.
            if info.checkers != 0 {
                targets &= info.evasion_mask;
            }

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let move_type = if enemy & (1u64 << to) != 0 {
                    Move::MVTP_CAPTURE
                } else {
                    Move::MVTP_QUIET
                };

                mv_arr.add_element(Move::new(
                    from,
                    to,
                    move_type,
                    NN,
                ));
            }
        }
    }

    fn generate_bishop_moves(&self, info: &CheckInfo, mv_arr: &mut MoveArray) {
        let own = if self.turn {
            self.white_occ
        } else {
            self.black_occ
        };

        let enemy = if self.turn {
            self.black_occ
        } else {
            self.white_occ
        };

        let mut pieces = self.pieces[if self.turn { WB } else { BB } as usize];

        while pieces != 0 {
            let from = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;

            let from_bb = 1u64 << from;

            let mut targets = bishop_attacks(from as usize, self.occupancy) & !own;

            if info.pinned & from_bb != 0 {
                targets &= info.pin_rays[from as usize];
            }

            if info.checkers != 0 {
                targets &= info.evasion_mask;
            }

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let move_type = if enemy & (1u64 << to) != 0 {
                    Move::MVTP_CAPTURE
                } else {
                    Move::MVTP_QUIET
                };

                mv_arr.add_element(Move::new(
                    from,
                    to,
                    move_type,
                    NN,
                ));
            }
        }
    }

    fn generate_rook_moves(&self, info: &CheckInfo, mv_arr: &mut MoveArray) {
        let own = if self.turn {
            self.white_occ
        } else {
            self.black_occ
        };

        let enemy = if self.turn {
            self.black_occ
        } else {
            self.white_occ
        };

        let mut pieces = self.pieces[if self.turn { WR } else { BR } as usize];

        while pieces != 0 {
            let from = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;

            let from_bb = 1u64 << from;

            let mut targets = rook_attacks(from as usize, self.occupancy) & !own;

            if info.pinned & from_bb != 0 {
                targets &= info.pin_rays[from as usize];
            }

            if info.checkers != 0 {
                targets &= info.evasion_mask;
            }

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let move_type = if enemy & (1u64 << to) != 0 {
                    Move::MVTP_CAPTURE
                } else {
                    Move::MVTP_QUIET
                };

                mv_arr.add_element(Move::new(
                    from,
                    to,
                    move_type,
                    NN,
                ));
            }
        }
    }

    fn generate_queen_moves(&self, info: &CheckInfo, mv_arr: &mut MoveArray) {
        let own = if self.turn {
            self.white_occ
        } else {
            self.black_occ
        };

        let enemy = if self.turn {
            self.black_occ
        } else {
            self.white_occ
        };

        let mut pieces = self.pieces[if self.turn { WQ } else { BQ } as usize];

        while pieces != 0 {
            let from = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;

            let from_bb = 1u64 << from;

            let mut targets = queen_attacks(from as usize, self.occupancy) & !own;

            if info.pinned & from_bb != 0 {
                targets &= info.pin_rays[from as usize];
            }

            if info.checkers != 0 {
                targets &= info.evasion_mask;
            }

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let move_type = if enemy & (1u64 << to) != 0 {
                    Move::MVTP_CAPTURE
                } else {
                    Move::MVTP_QUIET
                };

                mv_arr.add_element(Move::new(
                    from,
                    to,
                    move_type,
                    NN,
                ));
            }
        }
    }

    pub fn generate_moves(&mut self, mv_arr: &mut MoveArray) {
        let info = self.get_check_info();

        self.generate_king_moves(&info, mv_arr);

        if info.checker_count >= 2 {
            return;
        }

        self.generate_pawn_moves(mv_arr, &info);
        self.generate_knight_moves(&info, mv_arr);
        self.generate_bishop_moves(&info, mv_arr);
        self.generate_rook_moves(&info, mv_arr);
        self.generate_queen_moves(&info, mv_arr);

        if info.checker_count == 0 {
            self.generate_castling(mv_arr);
        }
    }

    #[inline(always)]
    pub fn is_in_check(&self, white: bool) -> bool {
        let king = if white { WK } else { BK };
        let king_bb = self.pieces[king as usize];

        if king_bb == 0 {
            return true;
        }

        let king_square = king_bb.trailing_zeros() as u8;

        self.is_square_attacked(king_square, !white, self.occupancy)
    }

    #[inline(always)]
    fn is_square_attacked(&self, square: u8, by_white: bool, occupancy: u64) -> bool {
        let pawn = if by_white {
            self.pieces[WP as usize]
        } else {
            self.pieces[BP as usize]
        };

        let knight = if by_white {
            self.pieces[WN as usize]
        } else {
            self.pieces[BN as usize]
        };

        let bishop = if by_white {
            self.pieces[WB as usize]
        } else {
            self.pieces[BB as usize]
        };

        let rook = if by_white {
            self.pieces[WR as usize]
        } else {
            self.pieces[BR as usize]
        };

        let queen = if by_white {
            self.pieces[WQ as usize]
        } else {
            self.pieces[BQ as usize]
        };

        let king = if by_white {
            self.pieces[WK as usize]
        } else {
            self.pieces[BK as usize]
        };

        let pawn_attackers = if by_white {
            WHITE_PAWN_ATTACKERS[square as usize]
        } else {
            BLACK_PAWN_ATTACKERS[square as usize]
        };

        if pawn_attackers & pawn != 0 {
            return true;
        }

        if KNIGHT_ATTACKERS[square as usize] & knight != 0 {
            return true;
        }

        if KING_ATTACKERS[square as usize] & king != 0 {
            return true;
        }

        if bishop_attacks(square as usize, occupancy) & (bishop | queen) != 0 {
            return true;
        }

        if rook_attacks(square as usize, occupancy) & (rook | queen) != 0 {
            return true;
        }

        false
    }

    fn checkers(&self, king_sq: u8) -> u64 {
        let enemy_pawns = if self.turn {
            self.pieces[BP as usize]
        } else {
            self.pieces[WP as usize]
        };

        let enemy_knights = if self.turn {
            self.pieces[BN as usize]
        } else {
            self.pieces[WN as usize]
        };

        let enemy_bishops = if self.turn {
            self.pieces[BB as usize]
        } else {
            self.pieces[WB as usize]
        };

        let enemy_rooks = if self.turn {
            self.pieces[BR as usize]
        } else {
            self.pieces[WR as usize]
        };

        let enemy_queens = if self.turn {
            self.pieces[BQ as usize]
        } else {
            self.pieces[WQ as usize]
        };

        let enemy_king = if self.turn {
            self.pieces[BK as usize]
        } else {
            self.pieces[WK as usize]
        };

        let pawn_attackers = if self.turn {
            BLACK_PAWN_ATTACKERS[king_sq as usize]
        } else {
            WHITE_PAWN_ATTACKERS[king_sq as usize]
        };

        let mut result = 0u64;

        result |= pawn_attackers & enemy_pawns;

        result |= KNIGHT_ATTACKERS[king_sq as usize] & enemy_knights;

        result |= KING_ATTACKERS[king_sq as usize] & enemy_king;

        result |= bishop_attacks(king_sq as usize, self.occupancy) & (enemy_bishops | enemy_queens);

        result |= rook_attacks(king_sq as usize, self.occupancy) & (enemy_rooks | enemy_queens);

        result
    }

    fn calculate_pins(&self, king_sq: u8) -> (u64, [u64; 64]) {
        let mut pinned = 0u64;
        let mut pin_rays = [!0u64; 64];

        let king_file = (king_sq & 7) as i32;
        let king_rank = (king_sq >> 3) as i32;

        let own = if self.turn {
            self.white_occ
        } else {
            self.black_occ
        };

        let enemy_rooks = if self.turn {
            self.pieces[BR as usize] | self.pieces[BQ as usize]
        } else {
            self.pieces[WR as usize] | self.pieces[WQ as usize]
        };

        let enemy_bishops = if self.turn {
            self.pieces[BB as usize] | self.pieces[BQ as usize]
        } else {
            self.pieces[WB as usize] | self.pieces[WQ as usize]
        };

        const DIRECTIONS: [(i32, i32); 8] = [
            (1, 0),   // east
            (-1, 0),  // west
            (0, 1),   // north
            (0, -1),  // south
            (1, 1),   // NE
            (-1, 1),  // NW
            (1, -1),  // SE
            (-1, -1), // SW
        ];

        for (df, dr) in DIRECTIONS {
            let diagonal = df != 0 && dr != 0;

            let mut file = king_file + df;
            let mut rank = king_rank + dr;

            let mut first_own: Option<u8> = None;

            while (0..8).contains(&file) && (0..8).contains(&rank) {
                let sq = (rank * 8 + file) as u8;
                let bb = 1u64 << sq;

                if self.occupancy & bb != 0 {
                    if first_own.is_none() {
                        if own & bb != 0 {
                            first_own = Some(sq);
                        } else {
                            // Enemy piece is immediately next to king.
                            break;
                        }
                    } else {
                        // We already found exactly one own piece.
                        let slider = if diagonal {
                            enemy_bishops & bb != 0
                        } else {
                            enemy_rooks & bb != 0
                        };

                        if slider {
                            let pinned_sq = first_own.unwrap();

                            pinned |= 1u64 << pinned_sq;

                            // The pinned piece can only move along this ray.
                            pin_rays[pinned_sq as usize] = between(king_sq, sq) | (1u64 << sq);
                        }

                        break;
                    }
                }

                file += df;
                rank += dr;
            }
        }

        (pinned, pin_rays)
    }

    fn get_check_info(&self) -> CheckInfo {
        let king_piece = if self.turn { WK } else { BK };

        let king_sq = self.pieces[king_piece as usize].trailing_zeros() as u8;

        let checkers = self.checkers(king_sq);

        let (pinned, pin_rays) = self.calculate_pins(king_sq);

        let checker_count = checkers.count_ones();

        let evasion_mask = match checker_count {
            0 => !0u64,

            // Double check:
            // only king moves are legal.
            _ if checker_count >= 2 => 0,

            1 => {
                let checker_sq = checkers.trailing_zeros() as u8;

                let checker_piece = self.piece_at(checker_sq);

                let slider = matches!(checker_piece, BB | WB | BQ | WQ | BR | WR);

                if slider {
                    between(king_sq, checker_sq) | (1u64 << checker_sq)
                } else {
                    1u64 << checker_sq
                }
            }

            _ => unreachable!(),
        };

        CheckInfo {
            king_sq,
            checkers,
            checker_count: checkers.count_ones() as usize,
            evasion_mask,
            pinned,
            pin_rays,
        }
    }

    fn generate_castling(&self, mv_arr: &mut MoveArray) {
        if self.turn {
            // White kingside.
            if self.castling & WK_CASTLE != 0
                && self.pieces[WK as usize] & (1u64 << E1) != 0
                && self.pieces[WR as usize] & (1u64 << H1) != 0
                && self.occupancy & ((1u64 << F1) | (1u64 << G1)) == 0
                && !self.is_square_attacked(E1, false, self.occupancy)
                && !self.is_square_attacked(F1, false, self.occupancy)
                && !self.is_square_attacked(G1, false, self.occupancy)
            {
                mv_arr.add_element(Move::new(
                    E1,
                    G1,
                    Move::MVTP_CASTLE_KING_SIDE,
                    NN,
                ));
            }

            // White queenside.
            if self.castling & WQ_CASTLE != 0
                && self.pieces[WK as usize] & (1u64 << E1) != 0
                && self.pieces[WR as usize] & (1u64 << A1) != 0
                && self.occupancy & ((1u64 << B1) | (1u64 << C1) | (1u64 << D1)) == 0
                && !self.is_square_attacked(E1, false, self.occupancy)
                && !self.is_square_attacked(D1, false, self.occupancy)
                && !self.is_square_attacked(C1, false, self.occupancy)
            {
                mv_arr.add_element(Move::new(
                    E1,
                    C1,
                    Move::MVTP_CASTLE_QUEEN_SIDE,
                    NN,
                ));
            }
        } else {
            if self.castling & BK_CASTLE != 0
                && self.pieces[BK as usize] & (1u64 << E8) != 0
                && self.pieces[BR as usize] & (1u64 << H8) != 0
                && self.occupancy & ((1u64 << F8) | (1u64 << G8)) == 0
                && !self.is_square_attacked(E8, true, self.occupancy)
                && !self.is_square_attacked(F8, true, self.occupancy)
                && !self.is_square_attacked(G8, true, self.occupancy)
            {
                mv_arr.add_element(Move::new(
                    E8,
                    G8,
                    Move::MVTP_CASTLE_KING_SIDE,
                    NN,
                ));
            }

            if self.castling & BQ_CASTLE != 0
                && self.pieces[BK as usize] & (1u64 << E8) != 0
                && self.pieces[BR as usize] & (1u64 << A8) != 0
                && self.occupancy & ((1u64 << B8) | (1u64 << C8) | (1u64 << D8)) == 0
                && !self.is_square_attacked(E8, true, self.occupancy)
                && !self.is_square_attacked(D8, true, self.occupancy)
                && !self.is_square_attacked(C8, true, self.occupancy)
            {
                mv_arr.add_element(Move::new(
                    E8,
                    C8,
                    Move::MVTP_CASTLE_QUEEN_SIDE,
                    NN,
                ));
            }
        }
    }

    fn generate_king_moves(&self, info: &CheckInfo, mv_arr: &mut MoveArray) {
        let own = if self.turn {
            self.white_occ
        } else {
            self.black_occ
        };

        let enemy = if self.turn {
            self.black_occ
        } else {
            self.white_occ
        };

        let from = info.king_sq;

        let mut targets = KING_ATTACKERS[from as usize] & !own;

        while targets != 0 {
            let to = targets.trailing_zeros() as u8;
            targets &= targets - 1;

            let to_bb = 1u64 << to;
            let from_bb = 1u64 << from;

            // If we're capturing, remove the captured enemy
            // piece from occupancy.
            let occupancy_after = (self.occupancy & !from_bb) | to_bb;

            if self.is_square_attacked(to, !self.turn, occupancy_after) {
                continue;
            }

            mv_arr.add_element(Move::new(
                from,
                to,
                if enemy & to_bb != 0 {
                    Move::MVTP_CAPTURE
                } else {
                    Move::MVTP_QUIET
                },
                NN,
            ));
        }
    }

    #[inline(always)]
    fn add_piece(&mut self, piece: u8, square: u8) {
        let bb = 1u64 << square;

        self.pieces[piece as usize] |= bb;
        self.piece_on[square as usize] = (piece + 1) as u8;

        if piece < 6 {
            self.white_occ |= bb;
        } else {
            self.black_occ |= bb;
        }

        self.occupancy |= bb;

        self.hash ^= Zobrist::get().piece[piece as usize][square as usize];
    }

    #[inline(always)]
    fn remove_piece(&mut self, piece: u8, square: u8) {
        let bb = 1u64 << square;

        self.pieces[piece as usize] &= !bb;
        self.piece_on[square as usize] = 0;

        if piece < 6 {
            self.white_occ &= !bb;
        } else {
            self.black_occ &= !bb;
        }

        self.occupancy &= !bb;

        self.hash ^= Zobrist::get().piece[piece as usize][square as usize];
    }

    #[inline(always)]
    fn move_piece(&mut self, piece: u8, from: u8, to: u8) {
        let from_bb = 1u64 << from;
        let to_bb = 1u64 << to;
        let mask = from_bb | to_bb;

        self.pieces[piece as usize] ^= mask;

        self.piece_on[from as usize] = 0;
        self.piece_on[to as usize] = (piece + 1) as u8;

        if piece < 6 {
            self.white_occ ^= mask;
        } else {
            self.black_occ ^= mask;
        }

        self.occupancy ^= mask;

        let z = Zobrist::get();
        self.hash ^= z.piece[piece as usize][from as usize];
        self.hash ^= z.piece[piece as usize][to as usize];
    }

    #[inline(always)]
    fn update_castling_rights(&mut self, piece: u8, from: u8) {
        let mut rights = self.castling;

        match piece {
            WK => rights &= !(WK_CASTLE | WQ_CASTLE),
            BK => rights &= !(BK_CASTLE | BQ_CASTLE),

            WR if from == H1 => rights &= !WK_CASTLE,
            WR if from == A1 => rights &= !WQ_CASTLE,
            BR if from == H8 => rights &= !BK_CASTLE,
            BR if from == A8 => rights &= !BQ_CASTLE,

            _ => {}
        }

        self.set_castling_hash(rights);
    }

    #[inline(always)]
    fn update_castling_rights_capture(&mut self, piece: u8, square: u8) {
        let mut rights = self.castling;

        match piece {
            WR if square == H1 => rights &= !WK_CASTLE,
            WR if square == A1 => rights &= !WQ_CASTLE,
            BR if square == H8 => rights &= !BK_CASTLE,
            BR if square == A8 => rights &= !BQ_CASTLE,

            _ => {}
        }

        self.set_castling_hash(rights);
    }

    pub fn parse_move(&self, s: &str) -> Option<Move> {
        let bytes = s.as_bytes();

        if bytes.len() < 4 || bytes.len() > 5 {
            return None;
        }

        let from = square_from_str(&s[0..2])?;
        let to = square_from_str(&s[2..4])?;

        let piece = self.piece_at(from);

        // Czy ruch jest biciem?
        let capture = self.piece_at(to) != NN;

        // Roszada
        if piece == 5 && self.turn {
            if from == E1 && to == G1 && self.castling & WK_CASTLE != 0 {
                return Some(Move::new(
                    from,
                    to,
                    Move::MVTP_CASTLE_KING_SIDE,
                    NN,
                ));
            }

            if from == E1 && to == C1 && self.castling & WQ_CASTLE != 0 {
                return Some(Move::new(
                    from,
                    to,
                    Move::MVTP_CASTLE_QUEEN_SIDE,
                    NN,
                ));
            }
        }

        if piece == 11 && !self.turn {
            if from == E8 && to == G8 && self.castling & BK_CASTLE != 0 {
                return Some(Move::new(
                    from,
                    to,
                    Move::MVTP_CASTLE_KING_SIDE,
                    NN,
                ));
            }

            if from == E8 && to == C8 && self.castling & BQ_CASTLE != 0 {
                return Some(Move::new(
                    from,
                    to,
                    Move::MVTP_CASTLE_QUEEN_SIDE,
                    NN,
                ));
            }
        }

        // En passant
        if self.en_passant == to && (piece == 0 || piece == 6) {
            return Some(Move::new(
                from,
                to,
                Move::MVTP_EN_PASSANT,
                NN,
            ));
        }

        // Promocja
        if bytes.len() == 5 {
            let promotion = match bytes[4] {
                b'q' => {
                    if self.turn {
                        4
                    } else {
                        10
                    }
                }
                b'r' => {
                    if self.turn {
                        3
                    } else {
                        9
                    }
                }
                b'b' => {
                    if self.turn {
                        2
                    } else {
                        8
                    }
                }
                b'n' => {
                    if self.turn {
                        1
                    } else {
                        7
                    }
                }
                _ => return None,
            };

            return Some(Move::new(
                from,
                to,
                if capture {
                    Move::MVTP_PROMOTION_CAPTURE
                } else {
                    Move::MVTP_PROMOTION
                },
                promotion,
            ));
        }

        // Podwójny ruch pionem
        if piece == WP && from / 8 == 1 && to == from + 16 {
            return Some(Move::new(
                from,
                to,
                Move::MVTP_DOUBLE_PAWN_PUSH,
                NN,
            ));
        }

        if piece == BP && from / 8 == 6 && to == from - 16 {
            return Some(Move::new(
                from,
                to,
                Move::MVTP_DOUBLE_PAWN_PUSH,
                NN,
            ));
        }

        // Zwykłe bicie
        if capture {
            return Some(Move::new(
                from,
                to,
                Move::MVTP_CAPTURE,
                NN,
            ));
        }

        // Zwykły ruch
        Some(Move::new(
            from,
            to,
            Move::MVTP_QUIET,
            NN,
        ))
    }

    pub fn position_fen(&mut self, fen: &[&str]) {
        if fen.len() != 6 && fen.len() != 4 {
            println!("info string Invalid FEN!");
            return;
        }

        let board_fen = fen[0];
        let side_to_move = fen[1];
        let castling_fen = fen[2];
        let en_passant_fen = fen[3];

        let mut pieces = [0u64; 12];
        let mut piece_on = [0u8; 64];

        let mut rank: i32 = 7;
        let mut file: i32 = 0;

        for c in board_fen.chars() {
            match c {
                '/' => {
                    if file != 8 {
                        println!("info string Invalid FEN: rank is not 8 squares");
                        return;
                    }

                    rank -= 1;
                    file = 0;
                }

                '1'..='8' => {
                    let empty = c.to_digit(10).unwrap() as i32;

                    file += empty;

                    if file > 8 {
                        println!("info string Invalid FEN: too many squares in rank");
                        return;
                    }
                }

                'P' | 'N' | 'B' | 'R' | 'Q' | 'K' | 'p' | 'n' | 'b' | 'r' | 'q' | 'k' => {
                    if rank < 0 || file >= 8 {
                        println!("info string Invalid FEN board");
                        return;
                    }

                    let square = (rank * 8 + file) as u8;
                    let bit = 1u64 << square;

                    match c {
                        'P' => {
                            pieces[WP as usize] |= bit;
                            piece_on[square as usize] = 1u8;
                        }
                        'N' => {
                            pieces[WN as usize] |= bit;
                            piece_on[square as usize] = 2u8;
                        }
                        'B' => {
                            pieces[WB as usize] |= bit;
                            piece_on[square as usize] = 3u8;
                        }
                        'R' => {
                            pieces[WR as usize] |= bit;
                            piece_on[square as usize] = 4u8;
                        }
                        'Q' => {
                            pieces[WQ as usize] |= bit;
                            piece_on[square as usize] = 5u8;
                        }
                        'K' => {
                            pieces[WK as usize] |= bit;
                            piece_on[square as usize] = 6u8;
                        }

                        'p' => {
                            pieces[BP as usize] |= bit;
                            piece_on[square as usize] = 7u8;
                        }
                        'n' => {
                            pieces[BN as usize] |= bit;
                            piece_on[square as usize] = 8u8;
                        }
                        'b' => {
                            pieces[BB as usize] |= bit;
                            piece_on[square as usize] = 9u8;
                        }
                        'r' => {
                            pieces[BR as usize] |= bit;
                            piece_on[square as usize] = 10u8;
                        }
                        'q' => {
                            pieces[BQ as usize] |= bit;
                            piece_on[square as usize] = 11u8;
                        }
                        'k' => {
                            pieces[BK as usize] |= bit;
                            piece_on[square as usize] = 12u8;
                        }

                        _ => unreachable!(),
                    }

                    file += 1;
                }

                _ => {
                    println!("info string Invalid FEN piece: {}", c);
                    return;
                }
            }
        }

        if rank != 0 || file != 8 {
            println!("info string Invalid FEN board");
            return;
        }

        // --------------------------------------------------
        // SIDE TO MOVE
        // --------------------------------------------------

        let turn = match side_to_move {
            "w" => true,
            "b" => false,
            _ => {
                println!("info string Invalid FEN side to move");
                return;
            }
        };

        // --------------------------------------------------
        // CASTLING
        // --------------------------------------------------

        let mut castling = 0u8;

        if castling_fen.contains('K') {
            castling |= WK_CASTLE;
        }

        if castling_fen.contains('Q') {
            castling |= WQ_CASTLE;
        }

        if castling_fen.contains('k') {
            castling |= BK_CASTLE;
        }

        if castling_fen.contains('q') {
            castling |= BQ_CASTLE;
        }

        // --------------------------------------------------
        // EN PASSANT
        // --------------------------------------------------

        let en_passant = if en_passant_fen == "-" {
            u8::MAX
        } else {
            square_from_str(en_passant_fen).unwrap_or(u8::MAX)
        };

        // --------------------------------------------------
        // OCCUPANCY
        // --------------------------------------------------

        let white_occ = pieces[WP as usize] | pieces[WN as usize] | pieces[WB as usize] | pieces[WR as usize] | pieces[WQ as usize] | pieces[WK as usize];

        let black_occ = pieces[BP as usize] | pieces[BN as usize] | pieces[BB as usize] | pieces[BR as usize] | pieces[BQ as usize] | pieces[BK as usize];

        let occupancy = white_occ | black_occ;

        // --------------------------------------------------
        // SET BOARD
        // --------------------------------------------------

        *self = Self {
            pieces,
            occupancy,
            white_occ,
            black_occ,
            piece_on,
            turn,
            castling,
            en_passant,
            hash: 0,
        };

        self.recompute_hash();
    }
}

fn square_from_str(s: &str) -> Option<u8> {
    let b = s.as_bytes();

    if b.len() != 2 || !(b'a'..=b'h').contains(&b[0]) || !(b'1'..=b'8').contains(&b[1]) {
        return None;
    }

    let file = b[0] - b'a';
    let rank = b[1] - b'1';

    Some(rank * 8 + file)
}

pub fn str_from_square(sq: u8) -> String {
    let mut ret = String::new();

    let file = sq % 8;
    let rank = sq / 8;

    ret.push(match file {
        0 => 'a',
        1 => 'b',
        2 => 'c',
        3 => 'd',
        4 => 'e',
        5 => 'f',
        6 => 'g',
        7 => 'h',
        _ => unreachable!(),
    });

    ret.push(match rank {
        0 => '1',
        1 => '2',
        2 => '3',
        3 => '4',
        4 => '5',
        5 => '6',
        6 => '7',
        7 => '8',
        _ => unreachable!(),
    });

    ret
}

#[inline]
fn between(a: u8, b: u8) -> u64 {
    let af = (a & 7) as i32;
    let ar = (a >> 3) as i32;

    let bf = (b & 7) as i32;
    let br = (b >> 3) as i32;

    let df = bf - af;
    let dr = br - ar;

    let step_f = df.signum();
    let step_r = dr.signum();

    // Not aligned.
    if df != 0 && dr != 0 && df.abs() != dr.abs() {
        return 0;
    }

    let mut f = af + step_f;
    let mut r = ar + step_r;

    let mut result = 0u64;

    while f != bf || r != br {
        result |= 1u64 << (r * 8 + f);

        f += step_f;
        r += step_r;
    }

    // Remove destination/checker square.
    result & !(1u64 << b)
}

#[inline(always)]
fn pawn_move_allowed(from: u8, to: u8, pinned: u64, check_info: &CheckInfo) -> bool {
    let from_bb = 1u64 << from;
    let to_bb = 1u64 << to;

    // Double check: only king moves are legal.
    if check_info.checker_count >= 2 {
        return false;
    }

    // If we're in check, the pawn's destination must resolve it.
    if check_info.checkers != 0 && (check_info.evasion_mask & to_bb) == 0 {
        return false;
    }

    // If absolutely pinned, destination must lie on the pin ray.
    if pinned & from_bb != 0 && (check_info.pin_rays[from as usize] & to_bb) == 0 {
        return false;
    }

    true
}
