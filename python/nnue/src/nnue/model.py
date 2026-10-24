import torch
import torch.nn as nn

from .constants import H, PAD


class NNUE(nn.Module):
    def __init__(self):
        super().__init__()
        self.ft = nn.EmbeddingBag(PAD + 1, H, mode="sum", padding_idx=PAD)
        self.ft_bias = nn.Parameter(torch.zeros(H))
        self.l1 = nn.Linear(2 * H, 32)
        self.l2 = nn.Linear(32, 32)
        self.out = nn.Linear(32, 1)
        with torch.no_grad():
            self.ft.weight.normal_(0, 0.05)
            self.ft.weight[PAD].zero_()

    def forward(self, idx):  # (B, 2, MAX_ACTIVE), int64
        # idx: (B, 2, MAX_ACTIVE)

        us = self.ft(idx[:, 0]) + self.ft_bias
        them = self.ft(idx[:, 1]) + self.ft_bias

        x = torch.cat([us, them], dim=1).clamp(0, 1)
        x = self.l1(x).clamp(0, 1)
        x = self.l2(x).clamp(0, 1)

        return self.out(x).squeeze(1)
