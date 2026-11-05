"""Boost the colours of DXT1/DXT3/DXT5 textures without decompressing them (stdlib only).

Wizard101's textures look muted under Elden Ring's lighting. A block-compressed texture stores, per 4x4 block, two
endpoint colours (RGB565) and 2-bit indices that pick each pixel's colour on the line between them. A saturation and
brightness change is linear in RGB, so applying it to the two endpoints changes every pixel the same way, up to
clamping and 565 rounding, and keeps the file's format, size and mipmaps.

DXT1 blocks whose first endpoint is not greater than the second use 3 colours plus transparent black, so after the
change the endpoints are swapped back into the order the block had, with its indices remapped to match.
"""

import struct

LUMA = (0.299, 0.587, 0.114)


def _unpack565(c):
    return ((c >> 11) & 31) / 31, ((c >> 5) & 63) / 63, (c & 31) / 31


def _pack565(r, g, b):
    q = lambda v, n: max(0, min(n, round(v * n)))
    return (q(r, 31) << 11) | (q(g, 63) << 5) | q(b, 31)


def _adjust(c, saturation, brightness, gains=(1.0, 1.0, 1.0)):
    rgb = _unpack565(c)
    luma = sum(w * v for w, v in zip(LUMA, rgb))
    return _pack565(*((luma + (v - luma) * saturation) * brightness * g for v, g in zip(rgb, gains)))


def _swap_indices(bits, three_colour):
    """Indices of a block whose two endpoints were swapped: 0<->1, and 2<->3 in 4-colour blocks."""
    out = 0
    for k in range(16):
        i = (bits >> (2 * k)) & 3
        i = {0: 1, 1: 0}.get(i, i) if three_colour else i ^ 1
        out |= i << (2 * k)
    return out


def _colour_block(data, at, saturation, brightness, dxt1, gains=(1.0, 1.0, 1.0)):
    c0, c1, bits = struct.unpack_from("<HHI", data, at)
    three_colour = dxt1 and c0 <= c1
    n0, n1 = _adjust(c0, saturation, brightness, gains), _adjust(c1, saturation, brightness, gains)
    if three_colour:
        if n0 > n1:
            n0, n1, bits = n1, n0, _swap_indices(bits, True)
    elif n0 < n1:
        n0, n1, bits = n1, n0, _swap_indices(bits, False)
    elif n0 == n1 and dxt1:
        bits = 0  # equal endpoints would read as 3-colour mode; all four colours are the same anyway
    struct.pack_into("<HHI", data, at, n0, n1, bits)


def boost(dds, saturation=1.3, brightness=1.1, gains=(1.0, 1.0, 1.0)):
    """DDS bytes with every colour block (all mipmaps) made more saturated and brighter; `gains` then scales red,
    green and blue separately (a colour grade, e.g. round 25's Wizard101 floor tint)."""
    four_cc = dds[84:88]
    gains = tuple(gains)
    if four_cc not in (b"DXT1", b"DXT3", b"DXT5") or (saturation == 1 and brightness == 1 and gains == (1.0, 1.0, 1.0)):
        return dds
    data = bytearray(dds)
    block = 8 if four_cc == b"DXT1" else 16
    colour = 0 if four_cc == b"DXT1" else 8
    for at in range(128, len(data) - block + 1, block):
        _colour_block(data, at + colour, saturation, brightness, four_cc == b"DXT1", gains)
    return bytes(data)
