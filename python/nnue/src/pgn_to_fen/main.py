"""Convert commented engine PGN files into NNUE training lines.

Each output line is:  FEN | score_cp (White's point of view) | result (1 / 0.5 / 0, White's point of view)

The score is taken from the engine comment attached to the move played from that
position, e.g. `{+0.34/24 11s}` or `{(Nc6) -0.21/42 61s}` (score/depth time).
Dependency-free: it has its own small move generator for replaying SAN moves.
"""

import argparse
import re
import sys

FILES = "abcdefgh"
START = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR"
KNIGHT = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)]
KING = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)]
ROOK = [(1, 0), (-1, 0), (0, 1), (0, -1)]
BISHOP = [(1, 1), (1, -1), (-1, 1), (-1, -1)]
RIGHTS_SQ = {0: "Q", 7: "K", 56: "q", 63: "k"}
SAN_RE = re.compile(r"^([NBRQK])?([a-h])?([1-8])?(x)?([a-h][1-8])(?:=?([NBRQnbrq]))?$")
SCORE_RE = re.compile(r"([+-])(?:(\d+(?:\.\d+)?)|M(\d+))/(\d+)")
RESULTS = {"1-0": 1.0, "0-1": 0.0, "1/2-1/2": 0.5}


def on(f, r):
    return 0 <= f < 8 and 0 <= r < 8


def sq_from_str(s):
    return (int(s[1]) - 1) * 8 + FILES.index(s[0])


class Pos:
    __slots__ = ("b", "white", "castle", "ep")

    def __init__(self):
        self.b = ["."] * 64
        for i, row in enumerate(START.split("/")):
            r, f = 7 - i, 0
            for ch in row:
                if ch.isdigit():
                    f += int(ch)
                else:
                    self.b[r * 8 + f] = ch
                    f += 1
        self.white = True
        self.castle = set("KQkq")
        self.ep = None

    def copy(self):
        p = Pos.__new__(Pos)
        p.b = self.b[:]
        p.white = self.white
        p.castle = set(self.castle)
        p.ep = self.ep
        return p

    def attacked(self, sq, by_white):
        f, r = sq & 7, sq >> 3
        b = self.b
        P, N, B, R, Q, K = "PNBRQK" if by_white else "pnbrqk"
        pr = r - 1 if by_white else r + 1
        for df in (-1, 1):
            if on(f + df, pr) and b[pr * 8 + f + df] == P:
                return True
        for df, dr in KNIGHT:
            if on(f + df, r + dr) and b[(r + dr) * 8 + f + df] == N:
                return True
        for df, dr in KING:
            if on(f + df, r + dr) and b[(r + dr) * 8 + f + df] == K:
                return True
        for dirs, pieces in ((BISHOP, (B, Q)), (ROOK, (R, Q))):
            for df, dr in dirs:
                x, y = f + df, r + dr
                while on(x, y):
                    c = b[y * 8 + x]
                    if c != ".":
                        if c in pieces:
                            return True
                        break
                    x += df
                    y += dr
        return False

    def king_sq(self, white):
        return self.b.index("K" if white else "k")

    def in_check(self, white):
        return self.attacked(self.king_sq(white), not white)

    def reaches(self, frm, to, piece):
        """Pseudo-legal geometry for non-pawn pieces (destination ownership checked by caller)."""
        f, r, tf, tr = frm & 7, frm >> 3, to & 7, to >> 3
        df, dr = tf - f, tr - r
        if piece == "N":
            return (df, dr) in KNIGHT
        if piece == "K":
            return max(abs(df), abs(dr)) == 1
        straight = df == 0 or dr == 0
        diagonal = abs(df) == abs(dr)
        if (piece == "R" and not straight) or (piece == "B" and not diagonal):
            return False
        if piece == "Q" and not (straight or diagonal):
            return False
        sf, sr = (df > 0) - (df < 0), (dr > 0) - (dr < 0)
        x, y = f + sf, r + sr
        while (x, y) != (tf, tr):
            if self.b[y * 8 + x] != ".":
                return False
            x += sf
            y += sr
        return True

    def pawn_reaches(self, frm, to, capture):
        f, r, tf, tr = frm & 7, frm >> 3, to & 7, to >> 3
        d = 1 if self.white else -1
        target = self.b[to]
        if capture:
            enemy = target != "." and (target.isupper() != self.white)
            return abs(tf - f) == 1 and tr == r + d and (enemy or to == self.ep)
        if tf != f or target != ".":
            return False
        if tr == r + d:
            return True
        start = 1 if self.white else 6
        return r == start and tr == r + 2 * d and self.b[(r + d) * 8 + f] == "."


def make(pos, frm, to, promo, castle):
    """Return (new_pos, is_capture)."""
    p = pos.copy()
    b = p.b
    white = pos.white
    piece = b[frm]
    is_capture = b[to] != "."
    b[frm] = "."
    p.ep = None
    if castle:
        r = frm >> 3
        if to > frm:
            b[r * 8 + 5], b[r * 8 + 7] = b[r * 8 + 7], "."
        else:
            b[r * 8 + 3], b[r * 8 + 0] = b[r * 8 + 0], "."
    if piece in "Pp":
        if to == pos.ep and not is_capture and (frm & 7) != (to & 7):
            b[to - 8 if white else to + 8] = "."
            is_capture = True
        if abs(to - frm) == 16:
            p.ep = (frm + to) // 2
    b[to] = (promo.upper() if white else promo.lower()) if promo else piece
    if piece == "K":
        p.castle -= {"K", "Q"}
    elif piece == "k":
        p.castle -= {"k", "q"}
    for s in (frm, to):
        if s in RIGHTS_SQ:
            p.castle.discard(RIGHTS_SQ[s])
    p.white = not white
    return p, is_capture


def find_move(pos, san):
    """Resolve a SAN string to (from, to, promo, castle)."""
    s = san.rstrip("+#!?")
    white = pos.white
    if s in ("O-O", "0-0", "O-O-O", "0-0-0"):
        r = 0 if white else 7
        kside = s.replace("0", "O") == "O-O"
        return r * 8 + 4, r * 8 + (6 if kside else 2), None, True
    m = SAN_RE.match(s)
    if not m:
        raise ValueError(f"cannot parse SAN {san!r}")
    piece = m.group(1) or "P"
    fchar, rchar, capture = m.group(2), m.group(3), m.group(4) is not None
    dest = sq_from_str(m.group(5))
    promo = m.group(6)
    want = piece if white else piece.lower()
    target = pos.b[dest]
    if target != "." and (target.isupper() == white):
        raise ValueError(f"{san!r}: destination holds own piece")
    found = []
    for sq in range(64):
        if pos.b[sq] != want:
            continue
        if fchar and (sq & 7) != FILES.index(fchar):
            continue
        if rchar and (sq >> 3) != int(rchar) - 1:
            continue
        ok = pos.pawn_reaches(sq, dest, capture) if piece == "P" else pos.reaches(sq, dest, piece)
        if not ok:
            continue
        nxt, _ = make(pos, sq, dest, promo, False)
        if nxt.attacked(nxt.king_sq(white), not white):
            continue
        found.append(sq)
    if len(found) != 1:
        raise ValueError(f"{san!r}: {len(found)} matching moves")
    return found[0], dest, promo, False


def to_fen(pos):
    rows = []
    for r in range(7, -1, -1):
        row, empty = "", 0
        for f in range(8):
            c = pos.b[r * 8 + f]
            if c == ".":
                empty += 1
            else:
                row += (str(empty) if empty else "") + c
                empty = 0
        rows.append(row + (str(empty) if empty else ""))
    castle = "".join(c for c in "KQkq" if c in pos.castle) or "-"
    ep = "-"
    if pos.ep is not None:
        f, r = pos.ep & 7, pos.ep >> 3
        pr = r - 1 if pos.white else r + 1
        pawn = "P" if pos.white else "p"
        if any(on(f + df, pr) and pos.b[pr * 8 + f + df] == pawn for df in (-1, 1)):
            ep = FILES[f] + str(r + 1)
    return f"{'/'.join(rows)} {'w' if pos.white else 'b'} {castle} {ep}"


def parse_comment(comment):
    """Return (cp from the mover's point of view, depth) or None (no score / mate score)."""
    m = SCORE_RE.search(comment)
    if not m or m.group(3) is not None:
        return None
    cp = round(float(m.group(2)) * 100)
    return (-cp if m.group(1) == "-" else cp), int(m.group(4))


def split_games(text):
    for chunk in re.split(r"\n(?=\[Event )", text):
        header, movetext, in_header = {}, [], True
        for line in chunk.split("\n"):
            if in_header and line.startswith("["):
                m = re.match(r'\[(\w+)\s+"(.*)"\]', line)
                if m:
                    header[m.group(1)] = m.group(2)
            else:
                in_header = False
                movetext.append(line)
        yield header, "\n".join(movetext)


def moves_with_comments(movetext):
    moves = []
    for tok in re.findall(r"\{[^}]*\}|;[^\n]*|\$\d+|[^\s{};]+", movetext):
        if tok.startswith("{"):
            if moves:
                moves[-1][1] = tok
        elif tok.startswith(";") or tok.startswith("$"):
            continue
        elif tok in RESULTS or tok == "*":
            break
        else:
            tok = re.sub(r"^\d+\.+", "", tok)
            if tok:
                moves.append([tok, ""])
    return moves


def convert(text, min_depth, max_cp, keep_noisy, dedupe):
    stats = dict(games=0, bad_games=0, no_score=0, shallow=0, huge=0, noisy=0, dup=0, written=0)
    seen, out = set(), []
    for header, movetext in split_games(text):
        result = RESULTS.get(header.get("Result", ""))
        if result is None:
            continue
        stats["games"] += 1
        pos = Pos()
        try:
            for san, comment in moves_with_comments(movetext):
                frm, to, promo, castle = find_move(pos, san)
                nxt, is_capture = make(pos, frm, to, promo, castle)
                parsed = parse_comment(comment)
                if parsed is None:
                    stats["no_score"] += 1
                elif parsed[1] < min_depth:
                    stats["shallow"] += 1
                elif abs(parsed[0]) > max_cp:
                    stats["huge"] += 1
                elif not keep_noisy and (is_capture or promo or pos.in_check(pos.white)):
                    stats["noisy"] += 1
                else:
                    fen = to_fen(pos)
                    if dedupe and fen in seen:
                        stats["dup"] += 1
                    else:
                        seen.add(fen)
                        cp_white = parsed[0] if pos.white else -parsed[0]
                        out.append(f"{fen} | {cp_white} | {result}")
                        stats["written"] += 1
                pos = nxt
        except ValueError as e:
            stats["bad_games"] += 1
            print(f"game {stats['games']}: {e}; remaining moves skipped", file=sys.stderr)
    return out, stats


def main():
    p = argparse.ArgumentParser(description="PGN with engine comments -> 'fen | cp | result' lines")
    p.add_argument("pgn")
    p.add_argument("out")
    p.add_argument("--min-depth", type=int, default=10, help="skip moves searched shallower than this (book moves)")
    p.add_argument("--max-cp", type=int, default=2000, help="skip scores with a larger magnitude")
    p.add_argument("--keep-noisy", action="store_true", help="keep captures, promotions and positions in check")
    p.add_argument("--keep-duplicates", action="store_true")
    a = p.parse_args()

    with open(a.pgn, encoding="utf-8", errors="replace") as f:
        text = f.read().replace("\r\n", "\n")
    lines, stats = convert(text, a.min_depth, a.max_cp, a.keep_noisy, not a.keep_duplicates)
    with open(a.out, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(stats, file=sys.stderr)


if __name__ == "__main__":
    main()
