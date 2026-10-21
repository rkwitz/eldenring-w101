"""Hide Elden Ring's own HP and FP bars (top-left HUD) but keep the stamina bar, for the hud_orbs W101 globes.

Usage: python er_hud_hide_bars.py path\\to\\ELDEN RING\\Game\\menu\\01_000_fe.gfx path\\to\\mod\\menu\\01_000_fe.gfx

Reads the unpacked HUD movie (UXM) and writes a copy for the Mod Engine mod folder; the game's file is not changed,
so deleting the copy undoes it. In 01_000_fe.gfx, PlayerHUD > Gauge places the bars as MP, HP and HPRegain (the
recoverable-HP sliver) next to SP; this gives those three placements a colour transform with alpha 0, so they are
still updated by the game's script but drawn invisibly.
"""
import struct
import sys
import zlib
from pathlib import Path

HIDE = {b"HP", b"MP", b"HPRegain"}
KEEP = b"SP"  # the sprite that also places SP is the gauge sprite


def tags(data, pos, end):
    """(code, start, header length, body length) of each SWF tag from pos to end."""
    while pos < end:
        word = struct.unpack_from("<H", data, pos)[0]
        code, length, header = word >> 6, word & 0x3F, 2
        if length == 0x3F:
            length, header = struct.unpack_from("<I", data, pos + 2)[0], 6
        yield code, pos, header, length
        pos += header + length
        if code == 0:
            break


class Bits:
    def __init__(self, data, pos):
        self.data, self.bit = data, pos * 8

    def read(self, n):
        value = 0
        for _ in range(n):
            value = (value << 1) | (self.data[self.bit >> 3] >> (7 - (self.bit & 7))) & 1
            self.bit += 1
        return value

    def end(self):
        return (self.bit + 7) // 8


def matrix_end(data, pos):
    bits = Bits(data, pos)
    for _ in range(2):  # scale, rotate
        if bits.read(1):
            n = bits.read(5)
            bits.read(2 * n)
    n = bits.read(5)
    bits.read(2 * n)
    return bits.end()


def cxform_end(data, pos):
    bits = Bits(data, pos)
    has_add, has_mult, n = bits.read(1), bits.read(1), bits.read(4)
    bits.read(4 * n * (has_add + has_mult))
    return bits.end()


# CXFORMWITHALPHA, multiply terms only (10 bits each): red, green, blue 256 (1.0), alpha 0.
INVISIBLE = 0
for value, width in [(0, 1), (1, 1), (10, 4), (256, 10), (256, 10), (256, 10), (0, 10)]:
    INVISIBLE = (INVISIBLE << width) | value
INVISIBLE = (INVISIBLE << 2).to_bytes(6, "big")  # 46 bits, padded to 48


def hide(body):
    """A PlaceObject2 body with its colour transform replaced by INVISIBLE, if its name is in HIDE."""
    flags = body[0]
    pos = 3  # flags, depth
    if flags & 0x02:
        pos += 2
    if flags & 0x04:
        pos = matrix_end(body, pos)
    cx_start = pos
    if flags & 0x08:
        pos = cxform_end(body, pos)
    rest = pos
    if flags & 0x10:
        pos += 2
    if not flags & 0x20:
        return None
    name = body[pos:body.index(b"\0", pos)]
    if name not in HIDE:
        return None
    return bytes([flags | 0x08]) + body[1:cx_start] + INVISIBLE + body[rest:]


def tag_bytes(code, body):
    return struct.pack("<HI", code << 6 | 0x3F, len(body)) + body


def main():
    source, target = Path(sys.argv[1]), Path(sys.argv[2])
    data = source.read_bytes()
    if data[:3] in (b"CFX", b"CWS"):
        data = data[:8] + zlib.decompress(data[8:])
    assert data[:3] in (b"GFX", b"FWS"), f"{source} is not a GFX/SWF movie"
    rect_bits = data[8] >> 3
    start = 8 + (5 + 4 * rect_bits + 7) // 8 + 4
    out = bytearray(data[:start])
    hidden = []
    for code, pos, header, length in tags(data, start, len(data)):
        tag = data[pos:pos + header + length]
        if code == 39:  # DefineSprite
            body = data[pos + header:pos + header + length]
            inner = list(tags(body, 4, len(body)))
            names = [body[p + h:p + h + l] for c, p, h, l in inner if c == 26]
            if any(KEEP + b"\0" in n for n in names) and any(hide(n) for n in names):
                new = bytearray(body[:4])
                for c, p, h, l in inner:
                    piece = body[p + h:p + h + l]
                    changed = hide(piece) if c == 26 else None
                    if changed:
                        hidden.append(piece)
                    new += tag_bytes(c, changed) if changed else body[p:p + h + l]
                tag = tag_bytes(39, bytes(new))
        out += tag
    if len(hidden) != len(HIDE):
        sys.exit(f"expected to hide {len(HIDE)} bars, found {len(hidden)}; is this Elden Ring's 01_000_fe.gfx?")
    struct.pack_into("<I", out, 4, len(out))
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(bytes(out))
    print(f"hid {len(hidden)} bars, wrote {target} ({len(out)} bytes)")


if __name__ == "__main__":
    main()
