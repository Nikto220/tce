from .fastdata import iter_batches

def prefetch(it, depth=4):
    import queue
    import threading

    q = queue.Queue(maxsize=depth)
    done = object()

    def worker():
        try:
            for item in it:
                q.put(item)
            q.put(done)
        except BaseException as e:
            q.put(e)

    threading.Thread(
        target=worker,
        daemon=True,
    ).start()

    while True:
        item = q.get()

        if item is done:
            return

        if isinstance(item, BaseException):
            raise item

        yield item

def record_to_fen(record: bytes) -> str:
    if len(record) != 32:
        raise ValueError(f"Expected 32 bytes, got {len(record)}")

    occupancy = int.from_bytes(record[0:8], "little")
    pieces_data = record[8:24]
    flags = record[27]

    white_to_move = bool(flags & 1)

    # Piece codes: 0..5 = side to move, 6..11 = opponent.
    # Piece type: 0 pawn, 1 knight, 2 bishop,
    #             3 rook, 4 queen, 5 king.
    piece_chars = "PNBRQK"

    board = [None] * 64
    n = 0
    occupied = occupancy

    while occupied:
        sq = (occupied & -occupied).bit_length() - 1
        occupied &= occupied - 1

        byte = pieces_data[n // 2]
        code = (byte & 0x0F) if n % 2 == 0 else (byte >> 4)

        if code >= 12:
            raise ValueError(f"Invalid piece code: {code}")

        own = code < 6
        pt = code % 6

        # Convert side-to-move-relative color to actual color.
        if white_to_move:
            is_white = own
        else:
            is_white = not own

        ch = piece_chars[pt]
        board[sq] = ch if is_white else ch.lower()
        n += 1

    if n > 32:
        raise ValueError("Too many occupied squares")

    # FEN ranks go from rank 8 to rank 1.
    rows = []
    for rank in range(7, -1, -1):
        row = ""
        empty = 0

        for file in range(8):
            sq = rank * 8 + file
            piece = board[sq]

            if piece is None:
                empty += 1
            else:
                if empty:
                    row += str(empty)
                    empty = 0
                row += piece

        if empty:
            row += str(empty)

        rows.append(row)

    side = "w" if white_to_move else "b"

    # Castling rights and en passant aren't stored in this record.
    return f"{'/'.join(rows)} {side} - - 0 1"
