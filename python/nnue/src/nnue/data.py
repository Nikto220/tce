import numpy as np
import torch

from .features import features


def load_data(path: str):
    """Each line: FEN | score_cp (white POV) | result (1 / 0.5 / 0, white POV)."""
    idx, score, result = [], [], []
    with open(path) as f:
        for line in f:
            fen, cp, res = (x.strip() for x in line.split("|"))
            us, them, white = features(fen)
            idx.append([us, them])
            score.append(float(cp) if white else -float(cp))
            result.append(float(res) if white else 1 - float(res))
    return (
        torch.from_numpy(np.array(idx, dtype=np.int16)),  # int16 keeps RAM low
        torch.tensor(score, dtype=torch.float32),
        torch.tensor(result, dtype=torch.float32),
    )
