"""Hide Elden Ring's own HP, FP and stamina bars and rune counter, which the hud_orbs W101 globes replace, and move
the quick-slot (equipment) panel up, above the globes.

Usage: python er_hud_hide_bars.py path\\to\\ELDEN RING\\Game\\menu path\\to\\mod\\menu

Reads the unpacked HUD movies (UXM) and writes copies for the Mod Engine mod folder; the game's files are not
changed, so deleting the copies undoes it. The hidden placements get a colour transform with alpha 0, so the game's
script still updates them but they are drawn invisibly:
- 01_000_fe.gfx, PlayerHUD > Gauge: HP, MP (FP), HPRegain (the recoverable-HP sliver) and SP (stamina). Enemy and
  ally tags also have placements named HP, so only the sprite that places SP is touched.
  Gauge's Equip is the row of buff / equipment-effect icons under the bars (top left); hidden for now (round 29 E1).
- 01_000_fe.gfx, PlayerHUD > ItemPanel > Arts > Fade > Text: the weapon skill name above the right-hand slot
  ("Blood Boon Ritual"); hidden for now (round 29 E2). The sprite is found by its MpShortage placement.
- 01_000_fe.gfx, the same Arts > Fade: the strip behind that name. Round 32 (U5, Ryan: "the bar above the item HUD
  where the skill name was is still there"): Fade's three unnamed PlaceObject3 images, depth 1
  MENU_FL_Arts_waku4 (the strip, 754x80 at half scale), depth 2 MENU_FL_Arts (the skill mark at its left end) and
  depth 7 MENU_PossessionWaku, and its MpShortage (the not-enough-FP mark drawn on the strip). The sprite is the one
  placing both MpShortage and Text (each item slot's sprite places an MpShortage too, with no Text).
- 01_000_fe.gfx, PlayerHUD > PersonalCompass: the compass bar at the top centre (round 34 CR2).
- 01_001_fe_soul.gfx: the root's Soul clip (rune counter).
The quick slots are PlayerHUD's ItemPanel and ItemPanel2 (pouch); in the 1920x1080 movie the slot cross reaches down
to about y 1045, and the globes' tops (Wizard101 y 570 of 768 at hud_orbs scale 1, bottom 723) are at y 865, so both
panels move up by ITEM_RAISE and keep Elden Ring's own x. The movie scales with the screen like the globes, so this
holds at any 16:9 resolution.
"""
import struct
import sys
import zlib
from pathlib import Path

HIDE = "hide"
ITEM_RAISE = 200.0  # movie pixels; the cross then ends at about y 845, just above the globes
# movie: [(a name, or a tuple of names all placed there, that marks the right sprite or root timeline,
#          {placement name or ("depth", n) for an unnamed one: edit}, placements to expect)];
# an edit is HIDE or (dx, dy) in movie pixels.
TARGETS = {
    "01_000_fe.gfx": [
        (b"SP", {b"HP": HIDE, b"MP": HIDE, b"HPRegain": HIDE, b"SP": HIDE, b"Equip": HIDE}, 5),
        (b"MpShortage", {b"Text": HIDE}, 1),  # Arts > Fade: the skill (Ash of War) name; Item/weapon Fades place no MpShortage+Text pair
        # Round 32 (U5): the strip under that name, its mark and the FP-shortage mark on it (Arts > Fade only).
        ((b"MpShortage", b"Text"), {("depth", 1): HIDE, ("depth", 2): HIDE, ("depth", 7): HIDE, b"MpShortage": HIDE}, 4),
        (b"ItemPanel", {b"ItemPanel": (0.0, -ITEM_RAISE), b"ItemPanel2": (0.0, -ITEM_RAISE)}, 2),
        # Round 34 (CR2, Ryan: Wizard101 has no compass): PlayerHUD's PersonalCompass (the bar of ticks and the
        # E / N letters at the top centre; its markers are inside it).
        (b"PersonalCompass", {b"PersonalCompass": HIDE}, 1),
    ],
    "01_001_fe_soul.gfx": [(b"Soul", {b"Soul": HIDE}, 2)],  # root Soul and the Soul inside it
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


def read_matrix(data, pos):
    """[scale x, scale y] or None, [rotate 0, rotate 1] or None, [translate x, y] in twips."""
    bits = Bits(data, pos)

    def signed(n):
        value = bits.read(n)
        return value - (1 << n) if n and value >> (n - 1) else value
    parts = []
    for _ in range(2):
        if bits.read(1):
            n = bits.read(5)
            parts.append([signed(n), signed(n)])
        else:
            parts.append(None)
    n = bits.read(5)
    parts.append([signed(n), signed(n)])
    return parts


def write_matrix(scale, rotate, translate):
    """MATRIX record bytes."""
    out, count = 0, 0

    def put(value, n):
        nonlocal out, count
        out = (out << n) | (value & ((1 << n) - 1))
        count += n

    def width(values):
        return max(max(v.bit_length() + 1 for v in values), 1)
    for pair in (scale, rotate):
        put(pair is not None, 1)
        if pair is not None:
            n = width(pair)
            put(n, 5)
            put(pair[0], n)
            put(pair[1], n)
    n = width(translate) if any(translate) else 0
    put(n, 5)
    if n:
        put(translate[0], n)
        put(translate[1], n)
    pad = -count % 8
    return (out << pad).to_bytes((count + pad) // 8, "big")


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


def po3_fields(body):
    """PlaceObject3 body: (flags, depth, position after the character id or None, matrix end, name or None)."""
    flags, flags2 = body[0], body[1]
    depth = struct.unpack_from("<H", body, 2)[0]
    pos = 4
    if flags2 & 0x08:  # class name
        pos = body.index(b"\0", pos) + 1
    if flags & 0x02:
        pos += 2
    matrix_at = pos
    if flags & 0x04:
        pos = matrix_end(body, pos)
    matrix_stop = pos
    if flags & 0x08:
        pos = cxform_end(body, pos)
    if flags & 0x10:
        pos += 2
    name = body[pos:body.index(b"\0", pos)] if flags & 0x20 else None
    return flags, depth, matrix_at, matrix_stop, name


def placed_name(body, code=26):
    """Instance name of a PlaceObject2 (or, `code` 70, PlaceObject3) body, or None."""
    if code == 70:
        return po3_fields(body)[4]
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


def edit3(body, edits):
    """A PlaceObject3 body hidden when `edits` hides its name or ("depth", its depth), or None."""
    flags, depth, _, matrix_stop, name = po3_fields(body)
    change = edits.get(name) if name else None
    if change is None:
        change = edits.get(("depth", depth))
    if change is None:
        return None
    if change != HIDE or flags & 0x08:
        sys.exit(f"PlaceObject3 at depth {depth}: only hiding one without a colour transform is supported")
    return bytes([flags | 0x08]) + body[1:matrix_stop] + INVISIBLE + body[matrix_stop:]


def edit(body, edits):
    """A PlaceObject2 body with its edit from `edits` applied (by instance name), or None."""
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
    change = edits.get(name)
    if change is None:
        return None
    if change == HIDE:
        return bytes([flags | 0x08]) + body[1:cx_start] + INVISIBLE + body[rest:]
    if not flags & 0x04:
        sys.exit(f"{name.decode()} has no matrix to move")
    at = 5 if flags & 0x02 else 3
    scale, rotate, (x, y) = read_matrix(body, at)
    moved = write_matrix(scale, rotate, [x + round(change[0] * 20), y + round(change[1] * 20)])
    return body[:at] + moved + body[cx_start:]


def tag_bytes(code, body):
    return struct.pack("<HI", code << 6 | 0x3F, len(body)) + body


def edit_in(tag_list, data, marker, edits, done):
    """Bytes of the tags with matching placements edited, if `marker` is placed among them, else None."""
    placed = {placed_name(data[p + h:p + h + l], c) for c, p, h, l in tag_list if c in (26, 70)}
    if not all(m in placed for m in (marker if isinstance(marker, tuple) else (marker,))):
        return None
    out = bytearray()
    for code, pos, header, length in tag_list:
        body = data[pos + header:pos + header + length]
        changed = edit(body, edits) if code == 26 else edit3(body, edits) if code == 70 else None
        if changed:
            name = placed_name(changed, code)
            done.append(name.decode() if name else f"depth {po3_fields(changed)[1]}")
        out += tag_bytes(code, changed) if changed else data[pos:pos + header + length]
    return bytes(out)


def patch(data, marker, edits, expected, movie):
    """`data` (an uncompressed movie) with `edits` applied in the sprite or root timeline that places `marker`."""
    rect_bits = data[8] >> 3
    start = 8 + (5 + 4 * rect_bits + 7) // 8 + 4
    done = []
    pieces = []
    for code, pos, header, length in tags(data, start, len(data)):
        tag = data[pos:pos + header + length]
        if code == 39:  # DefineSprite
            body = data[pos + header:pos + header + length]
            inner = edit_in(list(tags(body, 4, len(body))), body, marker, edits, done)
            if inner is not None:
                tag = tag_bytes(39, body[:4] + inner)
        pieces.append((code, pos, header, length, tag))
    root = [(c, p, h, l) for c, p, h, l, _ in pieces if c != 39]
    rooted = edit_in(root, data, marker, edits, done)
    out = bytearray(data[:start])
    rebuilt = iter(list(tags(rooted, 0, len(rooted)))) if rooted is not None else None
    for code, *_, tag in pieces:
        if code == 39 or rebuilt is None:
            out += tag
        else:
            c, p, h, l = next(rebuilt)
            out += rooted[p:p + h + l]
    if len(done) != expected:
        sys.exit(f"{movie}: expected to edit {expected} placements, edited {done}")
    struct.pack_into("<I", out, 4, len(out))
    print(f"{movie}: {', '.join(done)}")
    return bytes(out)


def main():
    game_menu, mod_menu = Path(sys.argv[1]), Path(sys.argv[2])
    for movie, rules in TARGETS.items():
        data = (game_menu / movie).read_bytes()
        if data[:3] in (b"CFX", b"CWS"):
            data = data[:8] + zlib.decompress(data[8:])
        assert data[:3] in (b"GFX", b"FWS"), f"{movie} is not a GFX/SWF movie"
        for marker, edits, expected in rules:
            data = patch(data, marker, edits, expected, movie)
        target = mod_menu / movie
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        print(f"wrote {target} ({len(data)} bytes)")


if __name__ == "__main__":
    main()
