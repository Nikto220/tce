from .constants import MAX_ACTIVE, PAD

_PIECE = {"p": 0, "n": 1, "b": 2, "r": 3, "q": 4, "k": 5}


def parse_fen(fen: str):
    """Return ([(is_white, piece_type, square)], white_to_move). a1 = 0, h8 = 63."""
    placement, side = fen.split()[:2]
    pieces = []
    rank, file = 7, 0
    for ch in placement:
        if ch == "/":
            rank -= 1
            file = 0
        elif ch.isdigit():
            file += int(ch)
        else:
            pieces.append((ch.isupper(), _PIECE[ch.lower()], rank * 8 + file))
            file += 1
    return pieces, side == "w"


def features(fen: str):
    """Active feature indices for (side to move, other side), padded."""
    pieces, white_to_move = parse_fen(fen)
    us, them = [], []
    for is_white, pt, sq in pieces:
        own = is_white == white_to_move
        us.append((0 if own else 1) * 384 + pt * 64 + (sq if white_to_move else sq ^ 56))
        them.append((1 if own else 0) * 384 + pt * 64 + (sq ^ 56 if white_to_move else sq))
    pad = lambda l: l + [PAD] * (MAX_ACTIVE - len(l))
    return pad(us), pad(them), white_to_move
