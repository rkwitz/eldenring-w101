"""Hide Elden Ring's own HP, FP and stamina bars and rune counter, which the hud_orbs W101 globes replace.

Usage: python er_hud_hide_bars.py path\\to\\ELDEN RING\\Game\\menu path\\to\\mod\\menu

Reads the unpacked HUD movies (UXM) and writes copies for the Mod Engine mod folder; the game's files are not
changed, so deleting the copies undoes it. The hidden placements get a colour transform with alpha 0, so the game's
script still updates them but they are drawn invisibly:
- 01_000_fe.gfx, PlayerHUD > Gauge: HP, MP (FP), HPRegain (the recoverable-HP sliver) and SP (stamina). Enemy and
  ally tags also have placements named HP, so only the sprite that places SP is touched.
- 01_001_fe_soul.gfx: the root's Soul clip (rune counter).
"""
import struct
import sys
import zlib
from pathlib import Path

# movie: (a name that marks the right sprite or root timeline, names to hide there, how many placements to expect)
TARGETS = {
    "01_000_fe.gfx": (b"SP", {b"HP", b"MP", b"HPRegain", b"SP"}, 4),
    "01_001_fe_soul.gfx": (b"Soul", {b"Soul"}, 2),  # root Soul and the Soul inside it
}


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


def placed_name(body):
    """Instance name of a PlaceObject2 body, or None."""
    flags = body[0]
    pos = 3
    if flags & 0x02:
        pos += 2
    if flags & 0x04:
        pos = matrix_end(body, pos)
    if flags & 0x08:
        pos = cxform_end(body, pos)
    if flags & 0x10:
        pos += 2
    return body[pos:body.index(b"\0", pos)] if flags & 0x20 else None


def hide(body, names):
    """A PlaceObject2 body with its colour transform replaced by INVISIBLE, if its name is in `names`."""
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
    if name not in names:
        return None
    return bytes([flags | 0x08]) + body[1:cx_start] + INVISIBLE + body[rest:]


def tag_bytes(code, body):
    return struct.pack("<HI", code << 6 | 0x3F, len(body)) + body


def hide_in(tag_list, data, marker, names, hidden):
    """Bytes of the tags with matching placements hidden, if `marker` is placed among them, else None."""
    placed = {placed_name(data[p + h:p + h + l]) for c, p, h, l in tag_list if c == 26}
    if marker not in placed:
        return None
    out = bytearray()
    for code, pos, header, length in tag_list:
        changed = hide(data[pos + header:pos + header + length], names) if code == 26 else None
        if changed:
            hidden.append(placed_name(changed).decode())
        out += tag_bytes(code, changed) if changed else data[pos:pos + header + length]
    return bytes(out)


def patch(source, target, marker, names, expected):
    data = source.read_bytes()
    if data[:3] in (b"CFX", b"CWS"):
        data = data[:8] + zlib.decompress(data[8:])
    assert data[:3] in (b"GFX", b"FWS"), f"{source} is not a GFX/SWF movie"
    rect_bits = data[8] >> 3
    start = 8 + (5 + 4 * rect_bits + 7) // 8 + 4
    hidden = []
    top = list(tags(data, start, len(data)))
    # Sprites first, each on its own; then the root timeline (top-level placements) as a whole.
    pieces = []
    for code, pos, header, length in top:
        tag = data[pos:pos + header + length]
        if code == 39:  # DefineSprite
            body = data[pos + header:pos + header + length]
            inner = hide_in(list(tags(body, 4, len(body))), body, marker, names, hidden)
            if inner is not None:
                tag = tag_bytes(39, body[:4] + inner)
        pieces.append((code, pos, header, length, tag))
    root = [(c, p, h, l) for c, p, h, l, _ in pieces if c != 39]
    rooted = hide_in(root, data, marker, names, hidden)
    out = bytearray(data[:start])
    if rooted is None:
        for *_, tag in pieces:
            out += tag
    else:
        # hide_in rebuilt the non-sprite tags in order; splice them back between the sprites.
        rebuilt = iter(list(tags(rooted, 0, len(rooted))))
        for code, *_, tag in pieces:
            if code == 39:
                out += tag
            else:
                c, p, h, l = next(rebuilt)
                out += rooted[p:p + h + l]
    if len(hidden) != expected:
        sys.exit(f"{source.name}: expected to hide {expected} placements, hid {hidden}")
    struct.pack_into("<I", out, 4, len(out))
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(bytes(out))
    print(f"{target}: hid {', '.join(hidden)} ({len(out)} bytes)")


def main():
    game_menu, mod_menu = Path(sys.argv[1]), Path(sys.argv[2])
    for movie, (marker, names, expected) in TARGETS.items():
        patch(game_menu / movie, mod_menu / movie, marker, names, expected)


if __name__ == "__main__":
    main()
