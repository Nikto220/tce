import argparse
import os
import time

import torch

from .constants import SCALE
from .fastdata import iter_batches
from .data import prefetch
from .export import export
from .model import NNUE


def save_checkpoint(path, model, opt, sched, epoch):
    tmp = path + ".tmp"  # write then rename, so an interrupted save can't corrupt the checkpoint
    torch.save(
        {
            "model": model.state_dict(),
            "opt": opt.state_dict(),
            "sched": sched.state_dict(),
            "epoch": epoch,
        },
        tmp,
    )
    os.replace(tmp, path)


def load_checkpoint(path, model, opt, sched):
    ckpt = torch.load(path, map_location="cpu")
    if "model" not in ckpt:  # old format: a bare state_dict from an earlier version
        model.load_state_dict(ckpt)
        print(f"loaded weights from {path} (old format: no optimizer state or epoch saved)")
        return 0
    model.load_state_dict(ckpt["model"])
    opt.load_state_dict(ckpt["opt"])
    sched.load_state_dict(ckpt["sched"])
    print(f"resumed from {path} at epoch {ckpt['epoch']}")
    return ckpt["epoch"]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("data")
    p.add_argument("--epochs", type=int, default=10, help="total epochs to train to (not additional)")
    p.add_argument("--batch", type=int, default=16384)
    p.add_argument("--lr", type=float, default=1e-3)
    p.add_argument("--lam", type=float, default=0.7, help="weight of score vs game result")
    p.add_argument("--out", default="nnue.bin")
    p.add_argument("--checkpoint", default="nnue.pt")
    p.add_argument("--fresh", action="store_true", help="ignore an existing checkpoint and start over")
    p.add_argument("--shuffle-size", type=int, default=1_000_000,
                   help="positions shuffled together (about 136 bytes of RAM each)")
    p.add_argument("--max-positions", type=int, default=0, help="use only the first N lines of the data file (0 = all)")
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--log-every", type=int, default=50, help="print progress every N batches (0 = off)")
    a = p.parse_args()

    device = torch.device("xpu" if torch.xpu.is_available() else "cuda" if torch.cuda.is_available() else "cpu")
    print("device:", device, flush=True)

    model = NNUE().to(device)
    opt = torch.optim.Adam(model.parameters(), lr=a.lr)
    sched = torch.optim.lr_scheduler.StepLR(opt, step_size=max(1, a.epochs // 3), gamma=0.3)

    start = 0
    if os.path.exists(a.checkpoint) and not a.fresh:
        start = load_checkpoint(a.checkpoint, model, opt, sched)
    else:
        print("no checkpoint loaded, starting from scratch", flush=True)

    if start >= a.epochs:
        print(f"checkpoint is already at epoch {start}; pass a larger --epochs to keep training")
        return

    for ep in range(start, a.epochs):
        print(f"epoch {ep + 1}/{a.epochs}: reading the first chunk of up to {a.shuffle_size:,} positions...", flush=True)
        # the file is streamed: only `shuffle_size` positions are in memory at a time
        batches = prefetch(iter_batches(a.data, a.batch, a.shuffle_size, a.seed + ep, a.max_positions), depth=16)
        total, count, t0 = 0.0, 0, time.time()
        t_win, c_win = t0, 0
        for step, (idx, score, result) in enumerate(batches, 1):
            x = torch.from_numpy(idx).to(device).long()
            score = torch.from_numpy(score).to(device)
            result = torch.from_numpy(result).to(device)
            target = a.lam * torch.sigmoid(score / SCALE) + (1 - a.lam) * result
            loss = ((torch.sigmoid(model(x)) - target) ** 2).mean()
            opt.zero_grad()
            loss.backward()
            opt.step()
            n = len(x)
            total += loss.item() * n
            count += n
            if step == 1:
                print(f"  first batch trained after {time.time() - t0:.1f}s (data loading + device warm-up)", flush=True)
                t_win, c_win = time.time(), count
            if a.log_every and step % a.log_every == 0:
                now = time.time()
                rate = (count - c_win) / max(now - t_win, 1e-9)
                t_win, c_win = now, count
                print(f"  epoch {ep + 1} step {step}  {count:,} positions  loss {total / count:.6f}  {rate:,.0f} pos/s",
                      flush=True)
        if count == 0:
            raise SystemExit(f"no valid positions found in {a.data}")
        sched.step()
        print(f"epoch {ep + 1}/{a.epochs}  loss {total / count:.6f}  ({count:,} positions)")
        save_checkpoint(a.checkpoint, model, opt, sched, ep + 1)
        export(model, a.out)


if __name__ == "__main__":
    main()

