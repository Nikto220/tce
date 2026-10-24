import numpy as np
import torch

from .constants import H, PAD, QA, QB


def _q(a, scale, dtype):
    info = np.iinfo(dtype)
    return np.clip(np.round(a * scale), info.min, info.max).astype(dtype)


def _write_i16(f, array, scale):
    f.write(_q(array, scale, np.int16).astype("<i2").tobytes())


def _write_i32(f, array, scale):
    f.write(_q(array, scale, np.int32).astype("<i4").tobytes())


def export(model, path: str):
    with torch.no_grad():
        ft_w = model.ft.weight[:PAD].cpu().numpy()
        ft_b = model.ft_bias.cpu().numpy()

        l1_w = model.l1.weight.cpu().numpy()
        l1_b = model.l1.bias.cpu().numpy()

        l2_w = model.l2.weight.cpu().numpy()
        l2_b = model.l2.bias.cpu().numpy()

        out_w = model.out.weight.cpu().numpy().reshape(-1)
        out_b = model.out.bias.cpu().numpy()

    assert ft_w.shape == (PAD, H)
    assert ft_b.shape == (H,)
    assert l1_w.shape == (32, 2 * H)
    assert l1_b.shape == (32,)
    assert l2_w.shape == (32, 32)
    assert l2_b.shape == (32,)
    assert out_w.shape == (32,)
    assert out_b.shape == (1,)

    with open(path, "wb") as f:
        # Feature transformer
        _write_i16(f, ft_w, QA)
        _write_i16(f, ft_b, QA)

        # Hidden layer 1
        _write_i16(f, l1_w, QB)
        _write_i32(f, l1_b, QA * QB)

        # Hidden layer 2
        _write_i16(f, l2_w, QB)
        _write_i32(f, l2_b, QA * QB)

        # Output layer
        _write_i16(f, out_w, QB)
        _write_i32(f, out_b, QA * QB)

    print(f"Exported complete network to {path}")

