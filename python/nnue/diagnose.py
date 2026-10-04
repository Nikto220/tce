"""Find out what limits training throughput. Run with the same Python you train with:

  uv run python diagnose.py disk   data.bin [skip_gib]       raw sequential read speed of the file
  uv run python diagnose.py loader data.bin [max_positions]  your fastdata loader alone (no model, no GPU)

Watch available memory (Task Manager / `free -h`) while each one runs.
"""
import sys
import time


def disk(path, skip_gib=0.0, block=128 << 20, limit=4 << 30):
    """Read the file front to back in big blocks. Use skip_gib to start past anything the OS has cached."""
    n = last_n = 0
    t0 = last_t = time.time()
    with open(path, "rb", buffering=0) as f:
        f.seek(int(skip_gib * (1 << 30)))
        while n < limit:
            b = f.read(block)
            if not b:
                break
            n += len(b)
            now = time.time()
            if now - last_t >= 1.0:
                print(f"{n / 2**30:6.2f} GiB read   {(n - last_n) / (now - last_t) / 1e6:8,.0f} MB/s", flush=True)
                last_t, last_n = now, n
    print(f"average: {n / (time.time() - t0) / 1e6:,.0f} MB/s over {n / 2**30:.1f} GiB")


def loader(path, max_positions=0):
    from nnue.fastdata import iter_batches  # adjust if your module lives elsewhere

    n = last_n = 0
    t0 = last_t = time.time()
    for _x, s, _r in iter_batches(path, 16384, 1_000_000, 0, max_positions):
        n += len(s)
        now = time.time()
        if now - last_t >= 2.0:
            print(f"{n:>14,} positions   {(n - last_n) / (now - last_t):>12,.0f} pos/s", flush=True)
            last_t, last_n = now, n
    print(f"average: {n / (time.time() - t0):,.0f} pos/s over {n:,} positions")


if __name__ == "__main__":
    if len(sys.argv) < 3 or sys.argv[1] not in ("disk", "loader"):
        sys.exit(__doc__)
    arg = float(sys.argv[3]) if len(sys.argv) > 3 else 0
    if sys.argv[1] == "disk":
        disk(sys.argv[2], arg)
    else:
        loader(sys.argv[2], int(arg))
