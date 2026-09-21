use super::super::attacks::*;

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

pub const WP: usize = 0;
pub const WN: usize = 1;
pub const WB: usize = 2;
pub const WR: usize = 3;
pub const WQ: usize = 4;
pub const WK: usize = 5;

pub const BP: usize = 6;
pub const BN: usize = 7;
pub const BB: usize = 8;
pub const BR: usize = 9;
pub const BQ: usize = 10;
pub const BK: usize = 11;

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

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Board {
    pieces: [u64; 12],

    occupancy: u64,
    white_occ: u64,
    black_occ: u64,

    turn: bool,
    castling: u8,
    en_passant: Option<u8>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MoveType {
    Quiet,
    Capture,
    DoublePawnPush,
    EnPassant,
    CastleKingSide,
    CastleQueenSide,
    Promotion,
    PromotionCapture,
}

#[derive(Debug, Clone, Copy)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub move_type: MoveType,
    pub promotion: Option<usize>,
}

impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("{}{}{}", str_from_square(self.from), str_from_square(self.to), if self.move_type == MoveType::Promotion || self.move_type == MoveType::PromotionCapture {match self.promotion.unwrap() {
            BN | WN => "n",
            BB | WB => "b",
            BR | WR => "r",
            BQ | WQ => "q",
            _ => unreachable!()
        }} else {""}).as_str())?;

        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Undo {
    pub captured_piece: Option<usize>,
    pub captured_square: Option<u8>,

    pub castling: u8,
    pub en_passant: Option<u8>,

    pub mv: Move,
}

impl Board {
    pub fn new() -> Self {
        let pieces = [
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
            0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000u64,
        ];
        let white_occ = pieces[0] | pieces[1] | pieces[2] | pieces[3] | pieces[4] | pieces[5];
        let black_occ = pieces[6] | pieces[7] | pieces[8] | pieces[9] | pieces[10] | pieces[11];
        Self {
            pieces,
            white_occ,
            black_occ,
            occupancy: white_occ | black_occ,
            turn: true,
            castling: 0b00000000,
            en_passant: None,
        }
    }

    pub fn position_startpos(&mut self) {
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

        let white_occ = pieces[0] | pieces[1] | pieces[2] | pieces[3] | pieces[4] | pieces[5];
        let black_occ = pieces[6] | pieces[7] | pieces[8] | pieces[9] | pieces[10] | pieces[11];
        *self = Self {
            pieces,
            white_occ,
            black_occ,
            occupancy: white_occ | black_occ,
            turn: true,
            castling: 0b00001111,
            en_passant: None,
        }
    }

    pub fn get_board(&self) -> [u64; 12] {
        self.pieces
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
        let mut undo = Undo {
            mv,
            captured_piece: None,
            captured_square: None,
            castling: self.castling,
            en_passant: self.en_passant,
        };

        let piece = self.piece_at(mv.from).expect("Brak figury na polu from");

        // --------------------------------------------------
        // 1. BICIE
        // --------------------------------------------------

        match mv.move_type {
            MoveType::Capture | MoveType::PromotionCapture => {
                if let Some(piece) = self.piece_at(mv.to) {
                    self.remove_piece(piece, mv.to);

                    undo.captured_piece = Some(piece);
                    undo.captured_square = Some(mv.to);
                }
            }

            MoveType::EnPassant => {
                let captured_square = if self.turn { mv.to - 8 } else { mv.to + 8 };

                let captured_piece = if self.turn { BP } else { WP };

                self.remove_piece(captured_piece, captured_square);

                undo.captured_piece = Some(captured_piece);
                undo.captured_square = Some(captured_square);
            }

            _ => {}
        }

        // --------------------------------------------------
        // 2. NORMALNY RUCH / PROMOCJA
        // --------------------------------------------------

        match mv.move_type {
            MoveType::Promotion | MoveType::PromotionCapture => {
                // usuwamy pionka
                self.remove_piece(piece, mv.from);

                // dodajemy promowaną figurę
                let promoted_piece = mv.promotion.expect("Brak figury promocji");

                self.add_piece(promoted_piece, mv.to);
            }

            MoveType::CastleKingSide => {
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

            MoveType::CastleQueenSide => {
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
                self.move_piece(piece, mv.from, mv.to);
            }
        }

        // --------------------------------------------------
        // 3. ROSZADA
        // --------------------------------------------------

        self.update_castling_rights(piece, mv.from);

        if let (Some(captured), Some(square)) = (undo.captured_piece, undo.captured_square) {
            self.update_castling_rights_capture(captured, square);
        }

        // --------------------------------------------------
        // 4. EN PASSANT
        // --------------------------------------------------

        self.en_passant = None;

        if matches!(mv.move_type, MoveType::DoublePawnPush) {
            self.en_passant = Some(if self.turn { mv.from + 8 } else { mv.from - 8 });
        }

        // --------------------------------------------------
        // 5. ZMIANA STRONY
        // --------------------------------------------------

        self.turn = !self.turn;

        self.update_occupancy();

        undo
    }

    pub fn unmake_move(&mut self, undo: Undo) {
        let mv = undo.mv;
        // Najpierw wracamy do strony, która wykonała ruch
        self.turn = !self.turn;

        let piece = match mv.move_type {
            MoveType::Promotion | MoveType::PromotionCapture => {
                // Promocja:
                // usuwamy promowaną figurę
                let promoted_piece = mv.promotion.expect("Brak promocji");

                self.remove_piece(promoted_piece, mv.to);

                // i przywracamy pionka
                if self.turn { WP } else { BP }
            }

            _ => self.piece_at(mv.to).expect("Nie znaleziono figury na to"),
        };

        // --------------------------------------------------
        // ROSZADA
        // --------------------------------------------------

        match mv.move_type {
            MoveType::CastleKingSide => {
                if self.turn {
                    self.move_piece(WK, G1, E1);
                    self.move_piece(WR, F1, H1);
                } else {
                    self.move_piece(BK, G8, E8);
                    self.move_piece(BR, F8, H8);
                }
            }

            MoveType::CastleQueenSide => {
                if self.turn {
                    self.move_piece(WK, C1, E1);
                    self.move_piece(WR, D1, A1);
                } else {
                    self.move_piece(BK, C8, E8);
                    self.move_piece(BR, D8, A8);
                }
            }

            MoveType::Promotion | MoveType::PromotionCapture => {
                let pawn = if self.turn { WP } else { BP };

                self.add_piece(pawn, mv.from);
            }

            _ => {
                self.move_piece(piece, mv.to, mv.from);
            }
        }

        // --------------------------------------------------
        // PRZYWRÓCENIE ZBITEJ FIGURY
        // --------------------------------------------------

        if let (Some(piece), Some(square)) = (undo.captured_piece, undo.captured_square) {
            self.add_piece(piece, square);
        }

        // --------------------------------------------------
        // PRZYWRÓCENIE STANU
        // --------------------------------------------------

        self.castling = undo.castling;
        self.en_passant = undo.en_passant;

        self.update_occupancy();
    }

    fn update_occupancy(&mut self) {
        self.white_occ = self.pieces[0]
            | self.pieces[1]
            | self.pieces[2]
            | self.pieces[3]
            | self.pieces[4]
            | self.pieces[5];

        self.black_occ = self.pieces[6]
            | self.pieces[7]
            | self.pieces[8]
            | self.pieces[9]
            | self.pieces[10]
            | self.pieces[11];

        self.occupancy = self.white_occ | self.black_occ;
    }

    fn piece_at(&self, square: u8) -> Option<usize> {
        let mask = 1u64 << square;

        for piece in 0..12 {
            if self.pieces[piece] & mask != 0 {
                return Some(piece);
            }
        }

        None
    }

    fn generate_pseudo_legal_moves(&self) -> Vec<Move> {
        let mut ret = Vec::<Move>::new();

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

        let mut pieces = self.pieces[if self.turn { WN } else { BN }]; // albo BN

        while pieces != 0 {
            let from = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;

            let targets = KNIGHT_ATTACKERS[from as usize] & !own;

            let mut targets = targets;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let move_type = if enemy & (1u64 << to) != 0 {
                    MoveType::Capture
                } else {
                    MoveType::Quiet
                };

                ret.push(Move {
                    from,
                    to,
                    move_type,
                    promotion: None,
                });
            }
        }

        let mut pieces = self.pieces[if self.turn { WK } else { BK }];

        while pieces != 0 {
            let from = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;

            let targets = KING_ATTACKERS[from as usize] & !own;

            let mut targets = targets;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let move_type = if enemy & (1u64 << to) != 0 {
                    MoveType::Capture
                } else {
                    MoveType::Quiet
                };

                ret.push(Move {
                    from,
                    to,
                    move_type,
                    promotion: None,
                });
            }
        }

        let mut pieces = self.pieces[if self.turn { WB } else { BB }];

        while pieces != 0 {
            let from = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;

            let mut targets = bishop_attacks(from, self.occupancy) & !own;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let move_type = if enemy & (1u64 << to) != 0 {
                    MoveType::Capture
                } else {
                    MoveType::Quiet
                };

                ret.push(Move {
                    from,
                    to,
                    move_type,
                    promotion: None,
                });
            }
        }

        let mut pieces = self.pieces[if self.turn { WR } else { BR }];

        while pieces != 0 {
            let from = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;

            let mut targets = rook_attacks(from, self.occupancy) & !own;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let move_type = if enemy & (1u64 << to) != 0 {
                    MoveType::Capture
                } else {
                    MoveType::Quiet
                };

                ret.push(Move {
                    from,
                    to,
                    move_type,
                    promotion: None,
                });
            }
        }

        let mut pieces = self.pieces[if self.turn { WQ } else { BQ }];

        while pieces != 0 {
            let from = pieces.trailing_zeros() as u8;
            pieces &= pieces - 1;

            let mut targets =
                (bishop_attacks(from, self.occupancy) | rook_attacks(from, self.occupancy)) & !own;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let move_type = if enemy & (1u64 << to) != 0 {
                    MoveType::Capture
                } else {
                    MoveType::Quiet
                };

                ret.push(Move {
                    from,
                    to,
                    move_type,
                    promotion: None,
                });
            }
        }

        if self.turn {
            if self.castling & WK_CASTLE != 0 && self.occupancy & ((1u64 << F1) | (1u64 << G1)) == 0
            {
                ret.push(Move {
                    from: E1,
                    to: G1,
                    move_type: MoveType::CastleKingSide,
                    promotion: None,
                });
            }

            if self.castling & WQ_CASTLE != 0
                && self.occupancy & ((1u64 << B1) | (1u64 << C1) | (1u64 << D1)) == 0
            {
                ret.push(Move {
                    from: E1,
                    to: C1,
                    move_type: MoveType::CastleQueenSide,
                    promotion: None,
                });
            }
        } else {
            if self.castling & BK_CASTLE != 0 && self.occupancy & ((1u64 << F8) | (1u64 << G8)) == 0
            {
                ret.push(Move {
                    from: E8,
                    to: G8,
                    move_type: MoveType::CastleKingSide,
                    promotion: None,
                });
            }

            if self.castling & BQ_CASTLE != 0
                && self.occupancy & ((1u64 << B8) | (1u64 << C8) | (1u64 << D8)) == 0
            {
                ret.push(Move {
                    from: E8,
                    to: C8,
                    move_type: MoveType::CastleQueenSide,
                    promotion: None,
                });
            }
        }

        if self.turn {
            let pawns = self.pieces[WP];

            // pojedynczy ruch
            let mut targets = (pawns << 8) & !self.occupancy;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let from = to - 8;

                if to >= 56 {
                    for promotion in [WQ, WR, WB, WN] {
                        ret.push(Move {
                            from,
                            to,
                            move_type: MoveType::Promotion,
                            promotion: Some(promotion),
                        });
                    }
                } else {
                    ret.push(Move {
                        from,
                        to,
                        move_type: MoveType::Quiet,
                        promotion: None,
                    });
                }
            }

            // podwójny ruch
            let mut targets = (((pawns & RANK_2) << 8) & !self.occupancy) << 8 & !self.occupancy;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                ret.push(Move {
                    from: to - 16,
                    to,
                    move_type: MoveType::DoublePawnPush,
                    promotion: None,
                });
            }

            let mut targets = ((pawns & NOT_A_FILE) << 7) & enemy;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let from = to - 7;

                if to >= 56 {
                    for promotion in [WQ, WR, WB, WN] {
                        ret.push(Move {
                            from,
                            to,
                            move_type: MoveType::PromotionCapture,
                            promotion: Some(promotion),
                        });
                    }
                } else {
                    ret.push(Move {
                        from,
                        to,
                        move_type: MoveType::Capture,
                        promotion: None,
                    });
                }
            }

            let mut targets = ((pawns & NOT_H_FILE) << 9) & enemy;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let from = to - 9;

                if to >= 56 {
                    for promotion in [WQ, WR, WB, WN] {
                        ret.push(Move {
                            from,
                            to,
                            move_type: MoveType::PromotionCapture,
                            promotion: Some(promotion),
                        });
                    }
                } else {
                    ret.push(Move {
                        from,
                        to,
                        move_type: MoveType::Capture,
                        promotion: None,
                    });
                }
            }

            if let Some(ep) = self.en_passant {
                let attackers = WHITE_PAWN_ATTACKERS[ep as usize] & self.pieces[WP];

                let mut attackers = attackers;

                while attackers != 0 {
                    let from = attackers.trailing_zeros() as u8;
                    attackers &= attackers - 1;

                    ret.push(Move {
                        from,
                        to: ep,
                        move_type: MoveType::EnPassant,
                        promotion: None,
                    });
                }
            }
        } else {
            let pawns = self.pieces[BP];

            // pojedynczy ruch
            let mut targets = (pawns >> 8) & !self.occupancy;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let from = to + 8;

                if to < 8 {
                    for promotion in [BQ, BR, BB, BN] {
                        ret.push(Move {
                            from,
                            to,
                            move_type: MoveType::Promotion,
                            promotion: Some(promotion),
                        });
                    }
                } else {
                    ret.push(Move {
                        from,
                        to,
                        move_type: MoveType::Quiet,
                        promotion: None,
                    });
                }
            }

            // podwójny ruch
            let mut targets = (((pawns & RANK_7) >> 8) & !self.occupancy) >> 8 & !self.occupancy;
            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                ret.push(Move {
                    from: to - 16,
                    to,
                    move_type: MoveType::DoublePawnPush,
                    promotion: None,
                });
            }

            let mut targets = ((pawns & NOT_A_FILE) >> 7) & enemy;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let from = to + 7;

                if to >= 56 {
                    for promotion in [WQ, WR, WB, WN] {
                        ret.push(Move {
                            from,
                            to,
                            move_type: MoveType::PromotionCapture,
                            promotion: Some(promotion),
                        });
                    }
                } else {
                    ret.push(Move {
                        from,
                        to,
                        move_type: MoveType::Capture,
                        promotion: None,
                    });
                }
            }

            let mut targets = ((pawns & NOT_H_FILE) >> 9) & enemy;

            while targets != 0 {
                let to = targets.trailing_zeros() as u8;
                targets &= targets - 1;

                let from = to + 9;

                if to >= 56 {
                    for promotion in [WQ, WR, WB, WN] {
                        ret.push(Move {
                            from,
                            to,
                            move_type: MoveType::PromotionCapture,
                            promotion: Some(promotion),
                        });
                    }
                } else {
                    ret.push(Move {
                        from,
                        to,
                        move_type: MoveType::Capture,
                        promotion: None,
                    });
                }
            }

            if let Some(ep) = self.en_passant {
                let attackers = BLACK_PAWN_ATTACKERS[ep as usize] & self.pieces[BP];

                let mut attackers = attackers;

                while attackers != 0 {
                    let from = attackers.trailing_zeros() as u8;
                    attackers &= attackers - 1;

                    ret.push(Move {
                        from,
                        to: ep,
                        move_type: MoveType::EnPassant,
                        promotion: None,
                    });
                }
            }
        }

        ret
    }

    fn check_castle(&self, undo: Undo, turn: bool) -> bool {
        if undo.mv.move_type == MoveType::CastleKingSide {
            if turn && !self.is_square_attacked(E1, false) && !self.is_square_attacked(F1, false) && !self.is_square_attacked(G1, false) {
                return true;
            }

            if !turn && !self.is_square_attacked(E8, false) && !self.is_square_attacked(F8, false) && !self.is_square_attacked(G8, false) {
                return true;
            }
        } else if undo.mv.move_type == MoveType::CastleQueenSide {
            if turn && !self.is_square_attacked(E1, false) && !self.is_square_attacked(D1, false) && !self.is_square_attacked(C1, false) && !self.is_square_attacked(B1, false) {
                return true;
            }

            if !turn && !self.is_square_attacked(E8, false) && !self.is_square_attacked(D8, false) && !self.is_square_attacked(C8, false) && !self.is_square_attacked(B1, false){
                return true;
            }
        }

        false
    }

    pub fn generate_moves(&mut self) -> Vec<Move> {
        let pseudo_legal = self.generate_pseudo_legal_moves();
        let mut legal = Vec::<Move>::new();

        for mv in pseudo_legal {
            let undo = self.make_move(mv);

            if !self.is_in_check(!self.turn) && {if undo.mv.move_type == MoveType::CastleKingSide || undo.mv.move_type == MoveType::CastleQueenSide {self.check_castle(undo, !self.turn)} else {true}} {
                legal.push(mv);
            }

            self.unmake_move(undo);
        }

        legal
    }

    pub fn is_in_check(&self, white: bool) -> bool {
        let king = if white { WK } else { BK };

        let king_bb = self.pieces[king];

        if king_bb == 0 {
            return true; // albo panic, zależnie od projektu
        }

        let king_square = king_bb.trailing_zeros() as u8;

        self.is_square_attacked(king_square, !white)
    }

    fn is_square_attacked(&self, square: u8, by_white: bool) -> bool {
        let pawns = if by_white {
            self.pieces[WP]
        } else {
            self.pieces[BP]
        };

        let knights = if by_white {
            self.pieces[WN]
        } else {
            self.pieces[BN]
        };

        let bishops = if by_white {
            self.pieces[WB]
        } else {
            self.pieces[BB]
        };

        let rooks = if by_white {
            self.pieces[WR]
        } else {
            self.pieces[BR]
        };

        let queens = if by_white {
            self.pieces[WQ]
        } else {
            self.pieces[BQ]
        };

        let king = if by_white {
            self.pieces[WK]
        } else {
            self.pieces[BK]
        };

        // piony
        let pawn_attacks = if by_white {
            WHITE_PAWN_ATTACKERS[square as usize]
        } else {
            BLACK_PAWN_ATTACKERS[square as usize]
        };

        if pawn_attacks & pawns != 0 {
            return true;
        }

        // skoczki
        if KNIGHT_ATTACKERS[square as usize] & knights != 0 {
            return true;
        }

        // króle
        if KING_ATTACKERS[square as usize] & king != 0 {
            return true;
        }

        // gońce + hetmany
        if bishop_attacks(square, self.occupancy) & (bishops | queens) != 0 {
            return true;
        }

        // wieże + hetmany
        if rook_attacks(square, self.occupancy) & (rooks | queens) != 0 {
            return true;
        }

        false
    }

    fn remove_piece(&mut self, piece: usize, square: u8) {
        self.pieces[piece] &= !(1u64 << square);
    }

    fn add_piece(&mut self, piece: usize, square: u8) {
        self.pieces[piece] |= 1u64 << square;
    }

    fn move_piece(&mut self, piece: usize, from: u8, to: u8) {
        let from_mask = 1u64 << from;
        let to_mask = 1u64 << to;

        self.pieces[piece] &= !from_mask;
        self.pieces[piece] |= to_mask;
    }

    fn update_castling_rights(&mut self, piece: usize, from: u8) {
        match piece {
            WK => self.castling &= !(WK_CASTLE | WQ_CASTLE),
            BK => self.castling &= !(BK_CASTLE | BQ_CASTLE),

            WR if from == H1 => {
                self.castling &= !WK_CASTLE;
            }

            WR if from == A1 => {
                self.castling &= !WQ_CASTLE;
            }

            BR if from == H8 => {
                self.castling &= !BK_CASTLE;
            }

            BR if from == A8 => {
                self.castling &= !BQ_CASTLE;
            }

            _ => {}
        }
    }

    fn update_castling_rights_capture(&mut self, piece: usize, square: u8) {
        match piece {
            WR if square == H1 => {
                self.castling &= !WK_CASTLE;
            }

            WR if square == A1 => {
                self.castling &= !WQ_CASTLE;
            }

            BR if square == H8 => {
                self.castling &= !BK_CASTLE;
            }

            BR if square == A8 => {
                self.castling &= !BQ_CASTLE;
            }

            _ => {}
        }
    }

    pub fn parse_move(&self, s: &str) -> Option<Move> {
        let bytes = s.as_bytes();

        if bytes.len() < 4 || bytes.len() > 5 {
            return None;
        }

        let from = square_from_str(&s[0..2])?;
        let to = square_from_str(&s[2..4])?;

        let piece = self.piece_at(from)?;

        // Czy ruch jest biciem?
        let capture = self.piece_at(to).is_some();

        // Roszada
        if piece == 5 && self.turn {
            if from == E1 && to == G1 && self.castling & WK_CASTLE != 0 {
                return Some(Move {
                    from,
                    to,
                    move_type: MoveType::CastleKingSide,
                    promotion: None,
                });
            }

            if from == E1 && to == C1 && self.castling & WQ_CASTLE != 0 {
                return Some(Move {
                    from,
                    to,
                    move_type: MoveType::CastleQueenSide,
                    promotion: None,
                });
            }
        }

        if piece == 11 && !self.turn {
            if from == E8 && to == G8 && self.castling & BK_CASTLE != 0 {
                return Some(Move {
                    from,
                    to,
                    move_type: MoveType::CastleKingSide,
                    promotion: None,
                });
            }

            if from == E8 && to == C8 && self.castling & BQ_CASTLE != 0 {
                return Some(Move {
                    from,
                    to,
                    move_type: MoveType::CastleQueenSide,
                    promotion: None,
                });
            }
        }

        // En passant
        if self.en_passant == Some(to) && (piece == 0 || piece == 6) {
            return Some(Move {
                from,
                to,
                move_type: MoveType::EnPassant,
                promotion: None,
            });
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

            return Some(Move {
                from,
                to,
                move_type: if capture {
                    MoveType::PromotionCapture
                } else {
                    MoveType::Promotion
                },
                promotion: Some(promotion),
            });
        }

        // Podwójny ruch pionem
        if piece == WP && from / 8 == 1 && to == from + 16 {
            return Some(Move {
                from,
                to,
                move_type: MoveType::DoublePawnPush,
                promotion: None,
            });
        }

        if piece == BP && from / 8 == 6 && to == from - 16 {
            return Some(Move {
                from,
                to,
                move_type: MoveType::DoublePawnPush,
                promotion: None,
            });
        }

        // Zwykłe bicie
        if capture {
            return Some(Move {
                from,
                to,
                move_type: MoveType::Capture,
                promotion: None,
            });
        }

        // Zwykły ruch
        Some(Move {
            from,
            to,
            move_type: MoveType::Quiet,
            promotion: None,
        })
    }

    pub fn position_fen(&mut self, fen: &[&str]) {
        if fen.len() != 6 {
            panic!("Invalid FEN: expected 6 fields, got {}", fen.len());
        }

        let board_fen = fen[0];
        let side_to_move = fen[1];
        let castling_fen = fen[2];
        let en_passant_fen = fen[3];

        let mut pieces = [0u64; 12];

        let mut rank: i32 = 7;
        let mut file: i32 = 0;

        for c in board_fen.chars() {
            match c {
                '/' => {
                    if file != 8 {
                        panic!("Invalid FEN: rank is not 8 squares");
                    }

                    rank -= 1;
                    file = 0;
                }

                '1'..='8' => {
                    let empty = c.to_digit(10).unwrap() as i32;

                    file += empty;

                    if file > 8 {
                        panic!("Invalid FEN: too many squares in rank");
                    }
                }

                'P' | 'N' | 'B' | 'R' | 'Q' | 'K' | 'p' | 'n' | 'b' | 'r' | 'q' | 'k' => {
                    if rank < 0 || file >= 8 {
                        panic!("Invalid FEN board");
                    }

                    let square = (rank * 8 + file) as u8;
                    let bit = 1u64 << square;

                    match c {
                        'P' => pieces[WP] |= bit,
                        'N' => pieces[WN] |= bit,
                        'B' => pieces[WB] |= bit,
                        'R' => pieces[WR] |= bit,
                        'Q' => pieces[WQ] |= bit,
                        'K' => pieces[WK] |= bit,

                        'p' => pieces[BP] |= bit,
                        'n' => pieces[BN] |= bit,
                        'b' => pieces[BB] |= bit,
                        'r' => pieces[BR] |= bit,
                        'q' => pieces[BQ] |= bit,
                        'k' => pieces[BK] |= bit,

                        _ => unreachable!(),
                    }

                    file += 1;
                }

                _ => {
                    panic!("Invalid FEN piece: {}", c);
                }
            }
        }

        if rank != 0 || file != 8 {
            panic!("Invalid FEN board");
        }

        // --------------------------------------------------
        // SIDE TO MOVE
        // --------------------------------------------------

        let turn = match side_to_move {
            "w" => true,
            "b" => false,
            _ => panic!("Invalid FEN side to move"),
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
            None
        } else {
            square_from_str(en_passant_fen)
        };

        // --------------------------------------------------
        // OCCUPANCY
        // --------------------------------------------------

        let white_occ = pieces[WP] | pieces[WN] | pieces[WB] | pieces[WR] | pieces[WQ] | pieces[WK];

        let black_occ = pieces[BP] | pieces[BN] | pieces[BB] | pieces[BR] | pieces[BQ] | pieces[BK];

        let occupancy = white_occ | black_occ;

        // --------------------------------------------------
        // SET BOARD
        // --------------------------------------------------

        *self = Self {
            pieces,
            occupancy,
            white_occ,
            black_occ,
            turn,
            castling,
            en_passant,
        };
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
        _ => unreachable!()
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
        _ => unreachable!()
    });

    ret
}
