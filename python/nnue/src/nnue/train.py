import argparse

import torch

from .constants import SCALE
from .data import load_data
from .export import export
from .model import NNUE


def main():
    p = argparse.ArgumentParser()
    p.add_argument("data")
    p.add_argument("--epochs", type=int, default=10)
    p.add_argument("--batch", type=int, default=16384)
    p.add_argument("--lr", type=float, default=1e-3)
    p.add_argument("--lam", type=float, default=0.7, help="weight of score vs game result")
    p.add_argument("--out", default="nnue.bin")
    p.add_argument("--checkpoint", default="nnue.pt")
    a = p.parse_args()

    device = torch.device("xpu" if torch.xpu.is_available() else "cpu")
    print("device:", device)

    idx, score, result = load_data(a.data)
    n = len(idx)
    model = NNUE().to(device)
    opt = torch.optim.Adam(model.parameters(), lr=a.lr)
    sched = torch.optim.lr_scheduler.StepLR(opt, step_size=max(1, a.epochs // 3), gamma=0.3)

    for ep in range(a.epochs):
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
        torch.save(model.state_dict(), a.checkpoint)
        export(model, a.out)
