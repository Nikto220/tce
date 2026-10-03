import argparse
import os

import torch

from .constants import SCALE
from .data import load_data
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
    if "model" not in ckpt:  # old format: a bare state_dict from the earlier version
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
    a = p.parse_args()

    device = torch.device("xpu" if torch.xpu.is_available() else "cpu")
    print("device:", device)

    model = NNUE().to(device)
    opt = torch.optim.Adam(model.parameters(), lr=a.lr)
    sched = torch.optim.lr_scheduler.StepLR(opt, step_size=max(1, a.epochs // 3), gamma=0.3)

    start = 0
    if os.path.exists(a.checkpoint) and not a.fresh:
        start = load_checkpoint(a.checkpoint, model, opt, sched)
    else:
        print("no checkpoint loaded, starting from scratch")

    if start >= a.epochs:
        print(f"checkpoint is already at epoch {start}; pass a larger --epochs to keep training")
        return

    idx, score, result = load_data(a.data)
    n = len(idx)

    for ep in range(start, a.epochs):
        perm = torch.randperm(n)
        total = 0.0
        for i in range(0, n, a.batch):
            b = perm[i : i + a.batch]
            x = idx[b].to(device).long()
            target = (a.lam * torch.sigmoid(score[b] / SCALE) + (1 - a.lam) * result[b]).to(device)
            loss = ((torch.sigmoid(model(x)) - target) ** 2).mean()
            opt.zero_grad()
            loss.backward()
            opt.step()
            total += loss.item() * len(b)
        sched.step()
        print(f"epoch {ep + 1}/{a.epochs}  loss {total / n:.6f}")
        save_checkpoint(a.checkpoint, model, opt, sched, ep + 1)
        export(model, a.out)


if __name__ == "__main__":
    main()
