import numpy as np
import torch

from .constants import PAD, QA, QB


def _q(a, scale, dtype):
    info = np.iinfo(dtype)
    return np.clip(np.round(a * scale), info.min, info.max).astype(dtype)


def export(model, path: str):
    with torch.no_grad():
        ft_w = model.ft.weight[:PAD].cpu().numpy()
        ft_b = model.ft_bias.cpu().numpy()
        out_w = model.out.weight.cpu().numpy().reshape(-1)
        out_b = model.out.bias.cpu().numpy()

    with open(path, "wb") as f:
        f.write(_q(ft_w, QA, np.int16).astype("<i2").tobytes())
        f.write(_q(ft_b, QA, np.int16).astype("<i2").tobytes())
        f.write(_q(out_w, QB, np.int16).astype("<i2").tobytes())
        f.write(_q(out_b, QA * QB, np.int32).astype("<i4").tobytes())
