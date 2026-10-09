//! PGN with engine evals in the comments -> `fen | cp | result` training lines.
//!
//! Two comment styles are recognized (detected per comment):
//!   * CCRL/cutechess:  {+0.34/24 11s}    -> score for the position BEFORE the move, mover's point of view
//!   * Lichess:         { [%eval 0.17] }  -> score for the position AFTER the move, White's point of view
//! Output scores are always from White's point of view; results are 1.0 / 0.5 / 0.0 (White's point of view).
//!
//! Streams the input one game at a time (constant memory), std-only, no dependencies.

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};
use std::str::FromStr;
use std::time::Instant;

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR";
const KNIGHT: [(i32, i32); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];
const KING: [(i32, i32); 8] = [
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];
const ROOK: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

// castling rights bits
const C_WK: u8 = 1; // white kingside
const C_WQ: u8 = 2; // white queenside
const C_BK: u8 = 4; // black kingside
const C_BQ: u8 = 8; // black queenside

fn on(f: i32, r: i32) -> bool {
    (0..8).contains(&f) && (0..8).contains(&r)
}

const MAX_ACTIVE: usize = 32;
const PAD: u16 = 768;

// Binary format:
//   32 bytes header
//   N × 131-byte records
//
// Record:
//   [0..64)   = us[32] + them[32], little-endian u16
//   [64..66)  = score_cp, little-endian i16
//   [66]      = result * 2: 0, 1, or 2
const MAGIC: &[u8; 8] = b"NNUEBIN\0";

const VERSION: u32 = 2;
const HEADER_SIZE: u64 = 32;
const RECORD_SIZE: u32 = 32;

const V2_OCCUPANCY_OFFSET: usize = 0;
const V2_PIECES_OFFSET: usize = 8;
const V2_SCORE_OFFSET: usize = 24;
const V2_RESULT_OFFSET: usize = 26;
const V2_FLAGS_OFFSET: usize = 27;

const FLAG_WHITE_TO_MOVE: u8 = 1;

#[derive(Clone, Copy)]
struct Feature {
    us: [u16; MAX_ACTIVE],
    them: [u16; MAX_ACTIVE],
}

// ---------------------------------------------------------------------------
// Board (just enough chess to replay SAN games). Square 0 = a1, 63 = h8.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Pos {
    b: [u8; 64], // b'.' = empty, otherwise the FEN piece letter
    white: bool,
    castle: u8,
    ep: Option<u8>,
}

struct Mv {
    frm: usize,
    to: usize,
    promo: Option<u8>,
    castle: bool,
}

impl Pos {
    fn start() -> Pos {
        let mut b = [b'.'; 64];
        for (i, row) in START.split('/').enumerate() {
            let r = 7 - i;
            let mut f = 0usize;
            for ch in row.bytes() {
                if ch.is_ascii_digit() {
                    f += (ch - b'0') as usize;
                } else {
                    b[r * 8 + f] = ch;
                    f += 1;
                }
            }
        }
        Pos {
            b,
            white: true,
            castle: C_WK | C_WQ | C_BK | C_BQ,
            ep: None,
        }
    }

    fn attacked(&self, sq: usize, by_white: bool) -> bool {
        let f = (sq & 7) as i32;
        let r = (sq >> 3) as i32;
        let (p, n, bi, ro, q, k) = if by_white {
            (b'P', b'N', b'B', b'R', b'Q', b'K')
        } else {
            (b'p', b'n', b'b', b'r', b'q', b'k')
        };
        let pr = if by_white { r - 1 } else { r + 1 };
        for df in [-1, 1] {
            if on(f + df, pr) && self.b[(pr * 8 + f + df) as usize] == p {
                return true;
            }
        }
        for (df, dr) in KNIGHT {
            if on(f + df, r + dr) && self.b[((r + dr) * 8 + f + df) as usize] == n {
                return true;
            }
        }
        for (df, dr) in KING {
            if on(f + df, r + dr) && self.b[((r + dr) * 8 + f + df) as usize] == k {
                return true;
            }
        }
        for (dirs, a1, a2) in [(&BISHOP[..], bi, q), (&ROOK[..], ro, q)] {
            for &(df, dr) in dirs {
                let mut x = f + df;
                let mut y = r + dr;
                while on(x, y) {
                    let c = self.b[(y * 8 + x) as usize];
                    if c != b'.' {
                        if c == a1 || c == a2 {
                            return true;
                        }
                        break;
                    }
                    x += df;
                    y += dr;
                }
            }
        }
        false
    }

    fn king_sq(&self, white: bool) -> Option<usize> {
        let king = if white { b'K' } else { b'k' };
        self.b.iter().position(|&c| c == king)
    }

    /// Is the side to move in check?
    fn in_check(&self) -> bool {
        match self.king_sq(self.white) {
            Some(k) => self.attacked(k, !self.white),
            None => true,
        }
    }

    /// Geometry for non-pawn pieces (`piece` is the uppercase letter).
    fn reaches(&self, frm: usize, to: usize, piece: u8) -> bool {
        let f = (frm & 7) as i32;
        let r = (frm >> 3) as i32;
        let df = (to & 7) as i32 - f;
        let dr = (to >> 3) as i32 - r;
        if df == 0 && dr == 0 {
            return false;
        }
        match piece {
            b'N' => return KNIGHT.contains(&(df, dr)),
            b'K' => return df.abs().max(dr.abs()) == 1,
            _ => {}
        }
        let straight = df == 0 || dr == 0;
        let diagonal = df.abs() == dr.abs();
        if (piece == b'R' && !straight) || (piece == b'B' && !diagonal) {
            return false;
        }
        if piece == b'Q' && !(straight || diagonal) {
            return false;
        }
        let sf = df.signum();
        let sr = dr.signum();
        let tf = f + df;
        let tr = r + dr;
        let mut x = f + sf;
        let mut y = r + sr;
        while (x, y) != (tf, tr) {
            if self.b[(y * 8 + x) as usize] != b'.' {
                return false;
            }
            x += sf;
            y += sr;
        }
        true
    }

    fn pawn_reaches(&self, frm: usize, to: usize, capture: bool) -> bool {
        let f = (frm & 7) as i32;
        let r = (frm >> 3) as i32;
        let tf = (to & 7) as i32;
        let tr = (to >> 3) as i32;
        let d = if self.white { 1 } else { -1 };
        let target = self.b[to];
        if capture {
            let enemy = target != b'.' && (target.is_ascii_uppercase() != self.white);
            return (tf - f).abs() == 1 && tr == r + d && (enemy || Some(to as u8) == self.ep);
        }
        if tf != f || target != b'.' {
            return false;
        }
        if tr == r + d {
            return true;
        }
        let start = if self.white { 1 } else { 6 };
        r == start && tr == r + 2 * d && self.b[((r + d) * 8 + f) as usize] == b'.'
    }
}

/// Apply a move; returns the new position and whether it captured something.
fn make(pos: &Pos, frm: usize, to: usize, promo: Option<u8>, castle: bool) -> (Pos, bool) {
    let mut p = *pos;
    let white = pos.white;
    let piece = p.b[frm];
    let mut is_capture = p.b[to] != b'.';
    p.b[frm] = b'.';
    p.ep = None;
    if castle {
        let r = frm >> 3;
        if to > frm {
            p.b[r * 8 + 5] = p.b[r * 8 + 7];
            p.b[r * 8 + 7] = b'.';
        } else {
            p.b[r * 8 + 3] = p.b[r * 8];
            p.b[r * 8] = b'.';
        }
    }
    if piece == b'P' || piece == b'p' {
        if Some(to as u8) == pos.ep && !is_capture && (frm & 7) != (to & 7) {
            let cap_sq = if white { to - 8 } else { to + 8 };
            p.b[cap_sq] = b'.';
            is_capture = true;
        }
        if frm.abs_diff(to) == 16 {
            p.ep = Some(((frm + to) / 2) as u8);
        }
    }
    p.b[to] = match promo {
        Some(pc) => {
            if white {
                pc.to_ascii_uppercase()
            } else {
                pc.to_ascii_lowercase()
            }
        }
        None => piece,
    };
    if piece == b'K' {
        p.castle &= !(C_WK | C_WQ);
    } else if piece == b'k' {
        p.castle &= !(C_BK | C_BQ);
    }
    for s in [frm, to] {
        match s {
            0 => p.castle &= !C_WQ,
            7 => p.castle &= !C_WK,
            56 => p.castle &= !C_BQ,
            63 => p.castle &= !C_BK,
            _ => {}
        }
    }
    p.white = !white;
    (p, is_capture)
}

/// Resolve a SAN string against the position.
fn find_move(pos: &Pos, san: &[u8]) -> Result<Mv, String> {
    let mut s = san;
    while let Some(&last) = s.last() {
        if matches!(last, b'+' | b'#' | b'!' | b'?') {
            s = &s[..s.len() - 1];
        } else {
            break;
        }
    }
    let white = pos.white;
    if s == b"O-O" || s == b"0-0" || s == b"O-O-O" || s == b"0-0-0" {
        let r = if white { 0 } else { 7 };
        let kside = s == b"O-O" || s == b"0-0";
        return Ok(Mv {
            frm: r * 8 + 4,
            to: r * 8 + if kside { 6 } else { 2 },
            promo: None,
            castle: true,
        });
    }
    let shown = || String::from_utf8_lossy(san).into_owned();

    // [piece][file][rank][x]<dest>[=promo]
    let mut end = s.len();
    let mut promo = None;
    if end >= 1 && b"NBRQnbrq".contains(&s[end - 1]) {
        promo = Some(s[end - 1]);
        end -= 1;
        if end >= 1 && s[end - 1] == b'=' {
            end -= 1;
        }
    }
    if end < 2 {
        return Err(format!("cannot parse SAN {:?}", shown()));
    }
    let dfile = s[end - 2];
    let drank = s[end - 1];
    if !(b'a'..=b'h').contains(&dfile) || !(b'1'..=b'8').contains(&drank) {
        return Err(format!("cannot parse SAN {:?}", shown()));
    }
    let dest = ((drank - b'1') as usize) * 8 + (dfile - b'a') as usize;
    let mut pre = &s[..end - 2];
    let mut piece = b'P';
    if let Some(&c0) = pre.first() {
        if b"NBRQK".contains(&c0) {
            piece = c0;
            pre = &pre[1..];
        }
    }
    let mut capture = false;
    if pre.last() == Some(&b'x') {
        capture = true;
        pre = &pre[..pre.len() - 1];
    }
    let mut fchar: Option<u8> = None;
    let mut rchar: Option<u8> = None;
    let mut k = 0;
    if k < pre.len() && (b'a'..=b'h').contains(&pre[k]) {
        fchar = Some(pre[k] - b'a');
        k += 1;
    }
    if k < pre.len() && (b'1'..=b'8').contains(&pre[k]) {
        rchar = Some(pre[k] - b'1');
        k += 1;
    }
    if k != pre.len() {
        return Err(format!("cannot parse SAN {:?}", shown()));
    }

    let target = pos.b[dest];
    if target != b'.' && (target.is_ascii_uppercase() == white) {
        return Err(format!("{:?}: destination holds own piece", shown()));
    }
    let want = if white {
        piece
    } else {
        piece.to_ascii_lowercase()
    };
    let mut found = 0usize;
    let mut count = 0;
    for sq in 0..64usize {
        if pos.b[sq] != want {
            continue;
        }
        if let Some(fc) = fchar {
            if (sq & 7) as u8 != fc {
                continue;
            }
        }
        if let Some(rc) = rchar {
            if (sq >> 3) as u8 != rc {
                continue;
            }
        }
        let ok = if piece == b'P' {
            pos.pawn_reaches(sq, dest, capture)
        } else {
            pos.reaches(sq, dest, piece)
        };
        if !ok {
            continue;
        }
        let (nxt, _) = make(pos, sq, dest, promo, false);
        let ks = nxt
            .king_sq(white)
            .ok_or_else(|| "king missing".to_string())?;
        if nxt.attacked(ks, !white) {
            continue;
        }
        found = sq;
        count += 1;
    }
    if count != 1 {
        return Err(format!("{:?}: {} matching moves", shown(), count));
    }
    Ok(Mv {
        frm: found,
        to: dest,
        promo,
        castle: false,
    })
}

fn write_fen(pos: &Pos, out: &mut Vec<u8>) {
    for r in (0..8usize).rev() {
        let mut empty = 0u8;
        for f in 0..8usize {
            let c = pos.b[r * 8 + f];
            if c == b'.' {
                empty += 1;
            } else {
                if empty > 0 {
                    out.push(b'0' + empty);
                    empty = 0;
                }
                out.push(c);
            }
        }
        if empty > 0 {
            out.push(b'0' + empty);
        }
        if r > 0 {
            out.push(b'/');
        }
    }
    out.push(b' ');
    out.push(if pos.white { b'w' } else { b'b' });
    out.push(b' ');
    if pos.castle == 0 {
        out.push(b'-');
    } else {
        if pos.castle & C_WK != 0 {
            out.push(b'K');
        }
        if pos.castle & C_WQ != 0 {
            out.push(b'Q');
        }
        if pos.castle & C_BK != 0 {
            out.push(b'k');
        }
        if pos.castle & C_BQ != 0 {
            out.push(b'q');
        }
    }
    out.push(b' ');
    let mut wrote_ep = false;
    if let Some(e) = pos.ep {
        let f = (e & 7) as i32;
        let r = (e >> 3) as i32;
        let pr = if pos.white { r - 1 } else { r + 1 };
        let pawn = if pos.white { b'P' } else { b'p' };
        let can = [-1, 1]
            .iter()
            .any(|&df| on(f + df, pr) && pos.b[(pr * 8 + f + df) as usize] == pawn);
        if can {
            out.push(b'a' + f as u8);
            out.push(b'1' + r as u8);
            wrote_ep = true;
        }
    }
    if !wrote_ep {
        out.push(b'-');
    }
}

// ---------------------------------------------------------------------------
// Comment parsing
// ---------------------------------------------------------------------------

/// Try to match `[+-](digits[.digits] | M digits)/digits` at `c[i]` (which is '+' or '-').
/// None = no match here; Some(None) = mate score; Some(Some((cp, depth))) = numeric score
/// (cp from the mover's point of view).
fn match_score(c: &[u8], i: usize) -> Option<Option<(i32, i32)>> {
    let n = c.len();
    let neg = c[i] == b'-';
    let mut j = i + 1;
    let mate = j < n && c[j] == b'M';
    if mate {
        j += 1;
    }
    let ds = j;
    while j < n && c[j].is_ascii_digit() {
        j += 1;
    }
    if j == ds {
        return None;
    }
    if !mate && j < n && c[j] == b'.' {
        let mut k = j + 1;
        while k < n && c[k].is_ascii_digit() {
            k += 1;
        }
        if k > j + 1 {
            j = k;
        }
    }
    let num_end = j;
    if j >= n || c[j] != b'/' {
        return None;
    }
    j += 1;
    let ps = j;
    while j < n && c[j].is_ascii_digit() {
        j += 1;
    }
    if j == ps {
        return None;
    }
    if mate {
        return Some(None);
    }
    let v: f64 = std::str::from_utf8(&c[i + 1..num_end]).ok()?.parse().ok()?;
    let cp = (v * 100.0).round_ties_even() as i32;
    let depth: i32 = std::str::from_utf8(&c[ps..j])
        .ok()?
        .parse()
        .unwrap_or(i32::MAX);
    Some(Some((if neg { -cp } else { cp }, depth)))
}

/// CCRL style comment -> (cp, depth) from the mover's point of view. Mate scores give None.
fn parse_comment(c: &[u8]) -> Option<(i32, i32)> {
    for i in 0..c.len() {
        if c[i] == b'+' || c[i] == b'-' {
            if let Some(res) = match_score(c, i) {
                return res;
            }
        }
    }
    None
}

/// Lichess `[%eval 0.17]` -> centipawns (White's point of view). Mate evals give None.
fn parse_eval(c: &[u8]) -> Option<i32> {
    let key = b"[%eval";
    let at = c.windows(key.len()).position(|w| w == key)?;
    let n = c.len();
    let mut j = at + key.len();
    let ws = j;
    while j < n && c[j].is_ascii_whitespace() {
        j += 1;
    }
    if j == ws || j >= n || c[j] == b'#' {
        return None;
    }
    let s = j;
    if c[j] == b'-' {
        j += 1;
    }
    let ds = j;
    while j < n && c[j].is_ascii_digit() {
        j += 1;
    }
    if j == ds {
        return None;
    }
    if j < n && c[j] == b'.' {
        let mut k = j + 1;
        while k < n && c[k].is_ascii_digit() {
            k += 1;
        }
        if k > j + 1 {
            j = k;
        }
    }
    if j >= n || c[j] != b']' {
        return None;
    }
    let v: f64 = std::str::from_utf8(&c[s..j]).ok()?.parse().ok()?;
    Some((v * 100.0).round_ties_even() as i32)
}

fn comment_has_score(c: &[u8]) -> bool {
    if c.windows(6).any(|w| w == b"[%eval") {
        return true;
    }
    for i in 0..c.len() {
        if (c[i] == b'+' || c[i] == b'-') && matches!(match_score(c, i), Some(Some(_))) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// PGN reading
// ---------------------------------------------------------------------------

/// Split movetext into (san, comment) pairs. Returns true if any comment carries a score.
fn tokenize<'a>(mt: &'a [u8], moves: &mut Vec<(&'a [u8], &'a [u8])>) -> bool {
    let n = mt.len();
    let mut i = 0;
    let mut has_score = false;
    while i < n {
        let c = mt[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        match c {
            b'{' => {
                let start = i + 1;
                let mut j = start;
                while j < n && mt[j] != b'}' {
                    j += 1;
                }
                let comment = &mt[start..j];
                if !has_score && comment_has_score(comment) {
                    has_score = true;
                }
                if let Some(last) = moves.last_mut() {
                    last.1 = comment;
                }
                i = j + 1;
            }
            b'}' => i += 1,
            b';' => {
                while i < n && mt[i] != b'\n' {
                    i += 1;
                }
            }
            b'$' => {
                i += 1;
                while i < n && mt[i].is_ascii_digit() {
                    i += 1;
                }
            }
            _ => {
                let start = i;
                while i < n && !mt[i].is_ascii_whitespace() && !matches!(mt[i], b'{' | b'}' | b';')
                {
                    i += 1;
                }
                let tok = &mt[start..i];
                if tok == b"1-0" || tok == b"0-1" || tok == b"1/2-1/2" || tok == b"*" {
                    break;
                }
                // strip a leading move number like "12." or "12..."
                let d = tok.iter().take_while(|c| c.is_ascii_digit()).count();
                let san = if d > 0 && tok.get(d) == Some(&b'.') {
                    let dots = tok[d..].iter().take_while(|&&c| c == b'.').count();
                    &tok[d + dots..]
                } else {
                    tok
                };
                if !san.is_empty() {
                    moves.push((san, &mt[0..0]));
                }
            }
        }
    }
    has_score
}

fn parse_header_line(line: &[u8]) -> Option<(&[u8], &[u8])> {
    if line.first() != Some(&b'[') {
        return None;
    }
    let sp = line.iter().position(|&c| c == b' ')?;
    let q1 = line.iter().position(|&c| c == b'"')?;
    let q2 = line.iter().rposition(|&c| c == b'"')?;
    if q2 <= q1 || sp == 0 {
        return None;
    }
    Some((&line[1..sp], &line[q1 + 1..q2]))
}

fn parse_int(v: &[u8]) -> Option<i32> {
    std::str::from_utf8(v).ok()?.trim().parse().ok()
}

struct Header {
    result: Option<&'static str>,
    white_elo: Option<i32>,
    black_elo: Option<i32>,
    normal: bool,
}

enum Outcome {
    Skip,
    Filtered,
    Play(&'static str),
}

impl Header {
    fn new() -> Header {
        Header {
            result: None,
            white_elo: None,
            black_elo: None,
            normal: true,
        }
    }

    fn set(&mut self, k: &[u8], v: &[u8]) {
        match k {
            b"Result" => {
                self.result = match v {
                    b"1-0" => Some("1.0"),
                    b"0-1" => Some("0.0"),
                    b"1/2-1/2" => Some("0.5"),
                    _ => None,
                }
            }
            b"WhiteElo" => self.white_elo = parse_int(v),
            b"BlackElo" => self.black_elo = parse_int(v),
            b"Termination" => self.normal = v == b"Normal",
            _ => {}
        }
    }

    fn outcome(&self, o: &Opts) -> Outcome {
        let res = match self.result {
            Some(r) => r,
            None => return Outcome::Skip,
        };
        if o.min_elo > 0 {
            match (self.white_elo, self.black_elo) {
                (Some(w), Some(b)) if w.min(b) >= o.min_elo => {}
                _ => return Outcome::Filtered,
            }
        }
        if o.normal_only && !self.normal {
            return Outcome::Filtered;
        }
        Outcome::Play(res)
    }
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

trait PositionSink {
    fn write_position(&mut self, pos: &Pos, cp: i32, result: &str) -> io::Result<()>;

    fn finish(&mut self) -> io::Result<()>;
}

#[derive(Clone, Copy)]
struct Opts {
    min_depth: i32,
    max_cp: i32,
    keep_noisy: bool,
    dedupe: bool,
    min_elo: i32,
    normal_only: bool,
    sample: f64,
    max_positions: u64,
    seed: u64,
}

#[derive(Default, Debug)]
struct Stats {
    scanned: u64,
    games: u64,
    filtered_games: u64,
    no_eval_games: u64,
    bad_games: u64,
    no_score: u64,
    shallow: u64,
    huge: u64,
    noisy: u64,
    dup: u64,
    sampled_out: u64,
    written: u64,
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (v >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn fnv_position(pos: &Pos) -> u64 {
    let features = pos_features(pos);

    let mut h = 0xcbf29ce484222325u64;

    for &x in features.us.iter().chain(features.them.iter()) {
        for byte in x.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }

    h
}

struct Ctx<S: PositionSink> {
    sink: S,
    opts: Opts,
    stats: Stats,
    seen: HashSet<u64>,
    rng: Rng,
}

impl<S: PositionSink> Ctx<S> {
    /// Process one finished game. Returns Ok(true) when --max-positions has been reached.
    fn finish_game(&mut self, outcome: &Outcome, movetext: &[u8]) -> io::Result<bool> {
        self.stats.scanned += 1;
        let result = match outcome {
            Outcome::Skip => return Ok(false),
            Outcome::Filtered => {
                self.stats.filtered_games += 1;
                return Ok(false);
            }
            Outcome::Play(r) => *r,
        };
        let mut moves: Vec<(&[u8], &[u8])> = Vec::new();
        if !tokenize(movetext, &mut moves) {
            // no scores anywhere in this game: skip without replaying it
            self.stats.no_eval_games += 1;
            return Ok(false);
        }
        self.stats.games += 1;
        let mut pos = Pos::start();
        for &(san, comment) in &moves {
            let mv = match find_move(&pos, san) {
                Ok(m) => m,
                Err(e) => {
                    self.stats.bad_games += 1;
                    if self.stats.bad_games <= 20 {
                        eprintln!(
                            "game {}: {}; remaining moves skipped",
                            self.stats.scanned, e
                        );
                    }
                    break;
                }
            };
            let (nxt, is_capture) = make(&pos, mv.frm, mv.to, mv.promo, mv.castle);
            let (cp_white, depth, use_next) = match parse_comment(comment) {
                // engine comment: score for the position BEFORE the move, mover's point of view
                Some((cp, d)) => (Some(if pos.white { cp } else { -cp }), Some(d), false),
                // Lichess comment: eval is for the position AFTER the move, White's point of view
                None => (parse_eval(comment), None, true),
            };
            let cp = match cp_white {
                Some(v) => v,
                None => {
                    self.stats.no_score += 1;
                    pos = nxt;
                    continue;
                }
            };
            let target = if use_next { &nxt } else { &pos };
            if depth.map_or(false, |d| d < self.opts.min_depth) {
                self.stats.shallow += 1;
            } else if cp.abs() > self.opts.max_cp {
                self.stats.huge += 1;
            } else if !self.opts.keep_noisy
                && (is_capture || mv.promo.is_some() || target.in_check())
            {
                self.stats.noisy += 1;
            } else if self.opts.sample < 1.0 && self.rng.next_f64() >= self.opts.sample {
                self.stats.sampled_out += 1;
            } else {
                let key = if self.opts.dedupe {
                    fnv_position(target)
                } else {
                    0
                };

                if self.opts.dedupe && !self.seen.insert(key) {
                    self.stats.dup += 1;
                } else {
                    self.sink.write_position(target, cp, result)?;
                    self.stats.written += 1;

                    if self.opts.max_positions > 0 && self.stats.written >= self.opts.max_positions
                    {
                        return Ok(true);
                    }
                }
            }
            pos = nxt;
        }
        Ok(false)
    }
}

fn pos_features(pos: &Pos) -> Feature {
    let mut us = [PAD; MAX_ACTIVE];
    let mut them = [PAD; MAX_ACTIVE];

    let mut n = 0;

    for sq in 0..64 {
        let piece = pos.b[sq];

        if piece == b'.' {
            continue;
        }

        debug_assert!(n < MAX_ACTIVE);

        let pt = match piece.to_ascii_lowercase() {
            b'p' => 0u16,
            b'n' => 1,
            b'b' => 2,
            b'r' => 3,
            b'q' => 4,
            b'k' => 5,
            _ => unreachable!(),
        };

        let is_white = piece.is_ascii_uppercase();
        let own = is_white == pos.white;

        let us_sq = if pos.white { sq } else { sq ^ 56 };
        let them_sq = if pos.white { sq ^ 56 } else { sq };

        us[n] = (if own { 0 } else { 384 }) + pt * 64 + us_sq as u16;

        them[n] = (if own { 384 } else { 0 }) + pt * 64 + them_sq as u16;

        n += 1;
    }

    us[..n].sort_unstable();
    them[..n].sort_unstable();

    Feature { us, them }
}

fn piece_type(piece: u8) -> u8 {
    match piece.to_ascii_lowercase() {
        b'p' => 0,
        b'n' => 1,
        b'b' => 2,
        b'r' => 3,
        b'q' => 4,
        b'k' => 5,
        _ => unreachable!(),
    }
}

/// Encode a board position into the compact v2 32-byte representation.
///
/// Piece codes:
///   0..5  = side-to-move's pawn..king
///   6..11 = opponent's pawn..king
fn encode_v2(
    pos: &Pos,
    score_white: i32,
    result_white: u8,
    out: &mut [u8; RECORD_SIZE as usize],
) -> io::Result<()> {
    debug_assert_eq!(out.len(), 32);

    out.fill(0);

    if !(0..=2).contains(&result_white) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid result code",
        ));
    }

    let score_stm = if pos.white { score_white } else { -score_white };

    let result_stm = if pos.white {
        result_white
    } else {
        match result_white {
            0 => 2,
            1 => 1,
            2 => 0,
            _ => unreachable!(),
        }
    };

    let score = i16::try_from(score_stm).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("score {} does not fit in i16", score_stm),
        )
    })?;

    // 0..7: occupancy
    let mut occupancy = 0u64;

    // 8..23: 32 packed nibbles.
    let mut piece_codes = [0u8; MAX_ACTIVE];

    let mut n = 0usize;

    for sq in 0..64usize {
        let piece = pos.b[sq];

        if piece == b'.' {
            continue;
        }

        if n >= MAX_ACTIVE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "position contains more than 32 pieces",
            ));
        }

        occupancy |= 1u64 << sq;

        let is_white = piece.is_ascii_uppercase();
        let own = is_white == pos.white;

        let pt = piece_type(piece);

        piece_codes[n] = if own { pt } else { pt + 6 };

        n += 1;
    }

    out[V2_OCCUPANCY_OFFSET..V2_OCCUPANCY_OFFSET + 8].copy_from_slice(&occupancy.to_le_bytes());

    for i in 0..n {
        let code = piece_codes[i];

        if i & 1 == 0 {
            out[V2_PIECES_OFFSET + i / 2] |= code;
        } else {
            out[V2_PIECES_OFFSET + i / 2] |= code << 4;
        }
    }

    out[V2_SCORE_OFFSET..V2_SCORE_OFFSET + 2].copy_from_slice(&score.to_le_bytes());

    out[V2_RESULT_OFFSET] = result_stm;

    out[V2_FLAGS_OFFSET] = if pos.white { FLAG_WHITE_TO_MOVE } else { 0 };

    // 28..31 remain zero.
    Ok(())
}

#[derive(Debug)]
enum ParseError {
    BadFen,
    BadNumber,
    BadLine,
}

fn pos_from_fen(fen: &[u8]) -> Result<Pos, ParseError> {
    let mut fields = fen.split(|&c| c.is_ascii_whitespace());

    let placement = fields.next().ok_or(ParseError::BadFen)?;
    let side = fields.next().ok_or(ParseError::BadFen)?;

    let white = match side {
        b"w" => true,
        b"b" => false,
        _ => return Err(ParseError::BadFen),
    };

    let mut b = [b'.'; 64];

    let mut rank = 7usize;
    let mut file = 0usize;

    for &c in placement {
        match c {
            b'/' => {
                if file != 8 || rank == 0 {
                    return Err(ParseError::BadFen);
                }

                rank -= 1;
                file = 0;
            }

            b'1'..=b'8' => {
                file += (c - b'0') as usize;

                if file > 8 {
                    return Err(ParseError::BadFen);
                }
            }

            b'p' | b'n' | b'b' | b'r' | b'q' | b'k' | b'P' | b'N' | b'B' | b'R' | b'Q' | b'K' => {
                if file >= 8 {
                    return Err(ParseError::BadFen);
                }

                b[rank * 8 + file] = c;
                file += 1;
            }

            _ => return Err(ParseError::BadFen),
        }
    }

    if rank != 0 || file != 8 {
        return Err(ParseError::BadFen);
    }

    Ok(Pos {
        b,
        white,
        // These aren't needed by v2 encoding.
        castle: 0,
        ep: None,
    })
}

fn parse_number<T>(s: &[u8]) -> Result<T, ParseError>
where
    T: std::str::FromStr,
{
    std::str::from_utf8(s)
        .map_err(|_| ParseError::BadNumber)?
        .trim()
        .parse::<T>()
        .map_err(|_| ParseError::BadNumber)
}

struct BinWriter<W: Write + Seek> {
    out: W,
    record: [u8; RECORD_SIZE as usize],
    written: u64,
}

impl<W: Write + Seek> BinWriter<W> {
    fn new(mut out: W) -> io::Result<Self> {
        let mut header = [0u8; HEADER_SIZE as usize];

        header[0..8].copy_from_slice(MAGIC);
        header[8..12].copy_from_slice(&VERSION.to_le_bytes());
        header[12..16].copy_from_slice(&RECORD_SIZE.to_le_bytes());

        // Count is filled in by finish().
        header[16..24].copy_from_slice(&0u64.to_le_bytes());

        out.write_all(&header)?;

        Ok(Self {
            out,
            record: [0; RECORD_SIZE as usize],
            written: 0,
        })
    }

    fn write(&mut self, pos: &Pos, score_white: i32, result_white: u8) -> io::Result<()> {
        encode_v2(pos, score_white, result_white, &mut self.record)?;

        self.out.write_all(&self.record)?;
        self.written += 1;

        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        self.out.seek(SeekFrom::Start(16))?;
        self.out.write_all(&self.written.to_le_bytes())?;
        self.out.flush()
    }
}

impl<W: Write + Seek> PositionSink for BinWriter<W> {
    fn write_position(&mut self, pos: &Pos, cp: i32, result: &str) -> io::Result<()> {
        let result_code = match result {
            "0.0" => 0,
            "0.5" => 1,
            "1.0" => 2,
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid result")),
        };

        self.write(pos, cp, result_code)
    }

    fn finish(&mut self) -> io::Result<()> {
        self.finish()
    }
}

struct FenWriter<W: Write> {
    out: W,
    fenbuf: Vec<u8>,
}

impl<W: Write> FenWriter<W> {
    fn new(out: W) -> Self {
        Self {
            out,
            fenbuf: Vec::with_capacity(128),
        }
    }
}

impl<W: Write> PositionSink for FenWriter<W> {
    fn write_position(&mut self, pos: &Pos, cp: i32, result: &str) -> io::Result<()> {
        self.fenbuf.clear();
        write_fen(pos, &mut self.fenbuf);

        self.out.write_all(&self.fenbuf)?;
        writeln!(self.out, " | {} | {}", cp, result)?;

        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

fn run_pgn_to_fen(input: &str, output: &str, opts: Opts) -> io::Result<()> {
    let file = File::open(input)?;
    let total = file.metadata()?.len().max(1);
    let mut rd = BufReader::with_capacity(1 << 22, file);
    let out = BufWriter::with_capacity(1 << 23, File::create(output)?);
    let mut ctx = Ctx {
        sink: FenWriter::new(out),
        opts,
        stats: Stats::default(),
        seen: HashSet::new(),
        rng: Rng::new(opts.seed),
    };

    let start = Instant::now();
    let mut line: Vec<u8> = Vec::with_capacity(4096);
    let mut movetext: Vec<u8> = Vec::with_capacity(16384);
    let mut hdr = Header::new();
    let mut outcome = Outcome::Skip;
    let mut started = false;
    let mut in_header = true;
    let mut stopped = false;
    let mut bytes: u64 = 0;

    loop {
        line.clear();
        let n = rd.read_until(b'\n', &mut line)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if line.starts_with(b"[Event ") {
            if started {
                if ctx.finish_game(&outcome, &movetext)? {
                    stopped = true;
                    break;
                }
                if ctx.stats.scanned % 200_000 == 0 {
                    let secs = start.elapsed().as_secs_f64().max(1e-9);
                    let frac = (bytes as f64 / total as f64).max(1e-9);
                    eprintln!(
                        "{:.1}% | {} games scanned | {} positions | {:.0} MB/s | ETA {:.0} min",
                        frac * 100.0,
                        ctx.stats.scanned,
                        ctx.stats.written,
                        bytes as f64 / 1e6 / secs,
                        secs * (1.0 - frac) / frac / 60.0
                    );
                }
            }
            hdr = Header::new();
            movetext.clear();
            in_header = true;
            started = true;
            outcome = Outcome::Skip;
        }
        if !started {
            continue;
        }
        if in_header && line.first() == Some(&b'[') {
            if let Some((k, v)) = parse_header_line(&line) {
                hdr.set(k, v);
            }
        } else {
            if in_header {
                in_header = false;
                outcome = hdr.outcome(&ctx.opts);
            }
            // only copy the movetext of games that passed the header filters
            if matches!(outcome, Outcome::Play(_)) {
                movetext.extend_from_slice(&line);
            }
        }
    }
    if started && !stopped {
        ctx.finish_game(&outcome, &movetext)?;
    }
    ctx.sink.finish()?;
    eprintln!("{:?}", ctx.stats);
    Ok(())
}

fn run_pgn_to_bin(input: &str, output: &str, opts: Opts) -> io::Result<()> {
    let file = File::open(input)?;
    let total = file.metadata()?.len().max(1);
    let mut rd = BufReader::with_capacity(1 << 22, file);
    let out = BufWriter::with_capacity(1 << 23, File::create(output)?);
    let mut ctx = Ctx {
        sink: BinWriter::new(out)?,
        opts,
        stats: Stats::default(),
        seen: HashSet::new(),
        rng: Rng::new(opts.seed),
    };

    let start = Instant::now();
    let mut line: Vec<u8> = Vec::with_capacity(4096);
    let mut movetext: Vec<u8> = Vec::with_capacity(16384);
    let mut hdr = Header::new();
    let mut outcome = Outcome::Skip;
    let mut started = false;
    let mut in_header = true;
    let mut stopped = false;
    let mut bytes: u64 = 0;

    loop {
        line.clear();
        let n = rd.read_until(b'\n', &mut line)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if line.starts_with(b"[Event ") {
            if started {
                if ctx.finish_game(&outcome, &movetext)? {
                    stopped = true;
                    break;
                }
                if ctx.stats.scanned % 200_000 == 0 {
                    let secs = start.elapsed().as_secs_f64().max(1e-9);
                    let frac = (bytes as f64 / total as f64).max(1e-9);
                    eprintln!(
                        "{:.1}% | {} games scanned | {} positions | {:.0} MB/s | ETA {:.0} min",
                        frac * 100.0,
                        ctx.stats.scanned,
                        ctx.stats.written,
                        bytes as f64 / 1e6 / secs,
                        secs * (1.0 - frac) / frac / 60.0
                    );
                }
            }
            hdr = Header::new();
            movetext.clear();
            in_header = true;
            started = true;
            outcome = Outcome::Skip;
        }
        if !started {
            continue;
        }
        if in_header && line.first() == Some(&b'[') {
            if let Some((k, v)) = parse_header_line(&line) {
                hdr.set(k, v);
            }
        } else {
            if in_header {
                in_header = false;
                outcome = hdr.outcome(&ctx.opts);
            }
            // only copy the movetext of games that passed the header filters
            if matches!(outcome, Outcome::Play(_)) {
                movetext.extend_from_slice(&line);
            }
        }
    }
    if started && !stopped {
        ctx.finish_game(&outcome, &movetext)?;
    }
    ctx.sink.finish()?;
    eprintln!("{:?}", ctx.stats);
    Ok(())
}

fn run_fen_to_bin(input: &str, output: &str) -> io::Result<()> {
    let file = File::open(input)?;
    let total = file.metadata()?.len().max(1);

    let mut rd = BufReader::with_capacity(1 << 22, file);
    let out = BufWriter::with_capacity(1 << 23, File::create(output)?);

    let mut writer = BinWriter::new(out)?;

    let start = Instant::now();
    let mut line: Vec<u8> = Vec::with_capacity(256);
    let mut bytes: u64 = 0;
    let mut lines: u64 = 0;
    let mut bad: u64 = 0;
    let mut last_report: u64 = 0;

    loop {
        line.clear();

        let n = rd.read_until(b'\n', &mut line)?;

        if n == 0 {
            break;
        }

        bytes += n as u64;
        lines += 1;

        let parsed = (|| -> Result<(), ParseError> {
            let mut parts = line.split(|&c| c == b'|');

            let fen = parts.next().ok_or(ParseError::BadLine)?;
            let cp = parts.next().ok_or(ParseError::BadLine)?;
            let result = parts.next().ok_or(ParseError::BadLine)?;

            let pos = pos_from_fen(fen)?;
            let score_white: i32 = parse_number(cp)?;

            let result_value: f32 = parse_number(result)?;

            let result_white = if result_value == 0.0 {
                0
            } else if result_value == 0.5 {
                1
            } else if result_value == 1.0 {
                2
            } else {
                return Err(ParseError::BadNumber);
            };

            writer
                .write(&pos, score_white, result_white)
                .map_err(|_| ParseError::BadLine)?;

            Ok(())
        })();

        if parsed.is_err() {
            bad += 1;
        }

        if lines / 200_000 > last_report {
            last_report = lines / 200_000;

            let secs = start.elapsed().as_secs_f64().max(1e-9);
            let frac = (bytes as f64 / total as f64).max(1e-9);

            eprintln!(
                "{:.1}% | {} lines | {} positions | {:.0} MB/s | ETA {:.0} min",
                frac * 100.0,
                lines,
                writer.written,
                bytes as f64 / 1e6 / secs,
                secs * (1.0 - frac) / frac / 60.0
            );
        }
    }

    writer.finish()?;

    eprintln!(
        "finished | {} lines | {} positions | {} bad lines",
        lines, writer.written, bad
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

fn usage() {
    eprintln!(
        "usage:\n\
         converter ptf <input.pgn> <output.txt> [options]\n\
         converter ptb <input.pgn> <out.bin>    [options]\n\
         converter ftb <input.txt> <out.bin>    [options]\n\
         \n\
         --min-depth N        skip engine moves searched shallower than N (default 10; book moves)\n\
         --max-cp N           skip scores larger than N centipawns (default 2000)\n\
         --keep-noisy         keep captures, promotions and positions in check\n\
         --keep-duplicates    don't remove duplicate positions (needed for huge runs: dedupe keeps a hash per position in RAM)\n\
         --min-elo N          skip games where either player is rated below N\n\
         --normal-only        skip games not ended normally (time forfeits, abandoned)\n\
         --sample P           keep each position with probability P (e.g. 0.5)\n\
         --max-positions N    stop after writing N positions (0 = no limit; underscores allowed: 5_000_000)\n\
         --seed N             seed for --sample (default 1)"
    );
}

fn parse_flag<T: FromStr>(args: &[String], i: &mut usize) -> T {
    let flag = args[*i].clone();
    *i += 1;
    match args
        .get(*i)
        .and_then(|v| v.replace('_', "").parse::<T>().ok())
    {
        Some(v) => v,
        None => {
            eprintln!("{flag} needs a valid value");
            std::process::exit(2)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut paths: Vec<String> = Vec::new();
    let mut o = Opts {
        min_depth: 10,
        max_cp: 2000,
        keep_noisy: false,
        dedupe: true,
        min_elo: 0,
        normal_only: false,
        sample: 1.0,
        max_positions: 0,
        seed: 1,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--min-depth" => o.min_depth = parse_flag(&args, &mut i),
            "--max-cp" => o.max_cp = parse_flag(&args, &mut i),
            "--min-elo" => o.min_elo = parse_flag(&args, &mut i),
            "--sample" => o.sample = parse_flag(&args, &mut i),
            "--max-positions" => o.max_positions = parse_flag(&args, &mut i),
            "--seed" => o.seed = parse_flag(&args, &mut i),
            "--keep-noisy" => o.keep_noisy = true,
            "--keep-duplicates" => o.dedupe = false,
            "--normal-only" => o.normal_only = true,
            "-h" | "--help" => {
                usage();
                return;
            }
            s if s.starts_with("--") => {
                eprintln!("unknown option {s}");
                usage();
                std::process::exit(2);
            }
            _ => paths.push(args[i].clone()),
        }
        i += 1;
    }
    if paths.len() != 3 || !(o.sample > 0.0 && o.sample <= 1.0) {
        usage();
        std::process::exit(2);
    }
    match &paths[0][..] {
        "ptf" => {
            if let Err(e) = run_pgn_to_fen(&paths[1], &paths[2], o) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        "ptb" => {
            if let Err(e) = run_pgn_to_bin(&paths[1], &paths[2], o) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        "ftb" => {
            if let Err(e) = run_fen_to_bin(&paths[1], &paths[2]) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        _ => {
            usage();
            std::process::exit(2);
        }
    }
}
