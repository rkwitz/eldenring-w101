"""Round 26 (W1): the Commons gate's fog picture, made here (no game or Wizard101 art), stdlib only.

A white mist in the fog-gate manner: alpha from a few octaves of smooth value noise with soft vertical streaks,
fading out at the left, right and top edges, fullest near the bottom; colour near-white. Written as a DXT5 DDS
(texture with alpha) for tpf.build_tpf. Deterministic (fixed seed), so every build makes the same picture.

Usage: python fog_texture.py out.dds [out.png]   (the PNG needs Pillow; for looking at it)
"""
import math
import random
import struct
import sys

SIZE = 256
COLOUR = (236, 238, 242)
# Round 27 (A6, Ryan: "too transparent"; rounds 26: 0.30 .. 0.80 with 12 % side and 25 % top fades): nearly opaque
ALPHA_LO, ALPHA_HI = 0.88, 1.0  # mist density range before the edge fade
SIDE_FADE, TOP_FADE = 0.04, 0.08


def _noise(size, cells, rng):
    grid = [[rng.random() for _ in range(cells + 1)] for _ in range(cells + 1)]
    for row in grid:
        row[cells] = row[0]  # tiles left-right
    smooth = lambda t: t * t * (3 - 2 * t)  # noqa: E731
    out = []
    for y in range(size):
        gy = y * cells / size
        y0, ty = int(gy), smooth(gy - int(gy))
        line = []
        for x in range(size):
            gx = x * cells / size
            x0, tx = int(gx), smooth(gx - int(gx))
            a = grid[y0][x0] + (grid[y0][x0 + 1] - grid[y0][x0]) * tx
            b = grid[y0 + 1][x0] + (grid[y0 + 1][x0 + 1] - grid[y0 + 1][x0]) * tx
            line.append(a + (b - a) * ty)
        out.append(line)
    return out


def alpha_map(size=SIZE, seed=26):
    rng = random.Random(seed)
    octaves = [(_noise(size, c, rng), w) for c, w in ((4, 0.5), (8, 0.25), (16, 0.15), (32, 0.10))]
    streak = _noise(size, 24, rng)
    out = []
    for y in range(size):
        v = y / (size - 1)  # 0 top .. 1 bottom
        row = []
        for x in range(size):
            u = x / (size - 1)
            n = sum(o[y][x] * w for o, w in octaves)
            n = 0.75 * n + 0.25 * streak[(y // 6) % size][x]  # stretched rows: soft vertical streaks
            a = ALPHA_LO + (ALPHA_HI - ALPHA_LO) * min(1.0, max(0.0, (n - 0.3) / 0.4))
            edge = min(1.0, u / SIDE_FADE, (1 - u) / SIDE_FADE) * min(1.0, v / TOP_FADE)
            row.append(a * edge * (0.95 + 0.05 * v))
        out.append(row)
    return out


def dxt5(alpha, colour=COLOUR):
    """DXT5 DDS (one mip) of one colour with per-texel `alpha` (rows of 0..1)."""
    size = len(alpha)
    c565 = (colour[0] >> 3) << 11 | (colour[1] >> 2) << 5 | (colour[2] >> 3)
    blocks = bytearray()
    for by in range(0, size, 4):
        for bx in range(0, size, 4):
            texels = [round(alpha[by + j][bx + i] * 255) for j in range(4) for i in range(4)]
            a0, a1 = max(texels), min(texels)
            if a0 == a1:
                idx = [0] * 16
            else:  # 8-value mode (a0 > a1): index 0 = a0, 1 = a1, 2..7 between
                ramp = [a0, a1] + [((7 - k) * a0 + k * a1) // 7 for k in range(1, 7)]
                idx = [min(range(8), key=lambda k: abs(ramp[k] - t)) for t in texels]
            bits = sum(k << (3 * n) for n, k in enumerate(idx))
            blocks += struct.pack("<BB", a0, a1) + bits.to_bytes(6, "little")
            blocks += struct.pack("<HHI", c565, c565, 0)
    pixel_format = struct.pack("<II4s5I", 32, 0x4, b"DXT5", 0, 0, 0, 0, 0)
    header = struct.pack("<4s7I44x", b"DDS ", 124, 0x81007, size, size, size * size, 0, 1) + pixel_format
    header += struct.pack("<5I", 0x1000, 0, 0, 0, 0)
    return header + bytes(blocks)


def fog_dds():
    return dxt5(alpha_map())


if __name__ == "__main__":
    a = alpha_map()
    open(sys.argv[1], "wb").write(dxt5(a))
    if len(sys.argv) > 2:
        from PIL import Image
        im = Image.new("RGBA", (SIZE, SIZE))
        im.putdata([COLOUR + (round(v * 255),) for row in a for v in row])
        bg = Image.new("RGBA", im.size, (40, 60, 40, 255))
        Image.alpha_composite(bg, im).save(sys.argv[2])
