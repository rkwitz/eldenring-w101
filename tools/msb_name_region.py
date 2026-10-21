"""Put the "Unicorn Way" area name (PlaceName 610900) over the street in m60_42_36_00, starting at the graveyard exit door.

How Elden Ring picks the title card: outside legacy dungeons the current place name comes from the world's map-name
texture (MapNameTexParam row 11 -> PlaceName 61000 "Limgrave" around The First Step). An MSB MapNameOverride region
(type 51, shape box, type data = PlaceName id) replaces it while the player stands inside, also while the player is still
in the Stranded Graveyard (m18). The title card appears whenever that name changes, so:
  - stepping out of an override box onto Limgrave ground shows "Limgrave";
  - the box must start at the outer face of the lift-up door AEG110_064_1000, or the card shows behind it (the old
    90x90 box reached the top of the m18 elevator, 17 m behind the door).
No map event (m60_42_36_00, m18_00_00_00, common_func) shows a welcome message here; it is all map-name driven.

This edits the regions named UnicornWay_name_* in place (same entries, indices and type indices), making each a box in
the door's own frame: local z from DOOR_GAP (just outside the door) to FAR, local x from LEFT to RIGHT, bottom BELOW m
under the door, HEIGHT tall. Box regions have their origin at the bottom centre; size = (width x, depth z, height y).
door_trigger then turns UnicornWay_name_enter into a small Other-type trigger box UnicornWay_title_door (entity
1042362700) just outside the door, which event 1042362991 (emevd.title_on_door_exit) uses to show the card on walking
out: in game the override box showed the card after a warp/load but not when walking out of the door.

Usage: python msb_name_region.py in.msb.dcx out.msb.dcx   (DFLT DCX as Smithbox saves, or an uncompressed .msb)
"""
import math
import struct
import sys
import zlib

sys.path.insert(0, __file__.rsplit("/", 1)[0] if "/" in __file__ else ".")
from msb_floor import _list, entry_name, read_msb, write_msb  # noqa: E402

TEXT_ID = 610900  # PlaceName "Unicorn Way" (msg item.msgbnd, tools/er_text.py); positive like the game's own overrides
MAP_NAME_OVERRIDE = 51
BOX = 5
DOOR = "AEG110_064_1000"  # the lift-up exit door (ObjAct 1042363540); its local +z points out of the graveyard
PREFIX = "UnicornWay_name"
TRIGGER = "UnicornWay_title_door"  # 21 chars: fits the name slot of the old UnicornWay_name_enter entry
TRIGGER_ENTITY = 1042362700  # IF In/Outside Area target for event 1042362991 (emevd.title_on_door_exit)
OTHER = 0xFFFFFFFF
# Trigger box in the door's frame: just past the door, across the doorway
T_NEAR, T_FAR, T_LEFT, T_RIGHT, T_BELOW, T_HEIGHT = 0.5, 4.5, -4.0, 4.0, 1.0, 5.0
# Street ground (classic OBJ, verts within a few m of floor height) spans about x -320..160, z -11..255 in the street's
# frame, which matches the door's frame to within a degree; a 10 m margin covers the difference.
DOOR_GAP, FAR, LEFT, RIGHT, BELOW, HEIGHT = 0.15, 276.0, -330.0, 170.0, 10.0, 160.0


def _door_frame(lists, door, near, far, left, right, below, height):
    """(centre, rotation y, box size) of a box given in the door's own frame (box origin = bottom centre)."""
    parts = _list(lists, "PARTS_PARAM_ST").entries
    d = next(e for e in parts if entry_name(e) == door)
    (px, py, pz), (_, ry, _) = struct.unpack_from("<3f", d, 0x20), struct.unpack_from("<3f", d, 0x2C)
    c, s = math.cos(math.radians(ry)), math.sin(math.radians(ry))
    lx, lz = (left + right) / 2, (near + far) / 2
    return (px + lx * c + lz * s, py - below, pz - lx * s + lz * c), ry, (right - left, far - near, height)


def door_trigger(lists, door=DOOR):
    """Turn the second name region (UnicornWay_name_enter, a type-51 copy) into an Other-type box region with entity id
    TRIGGER_ENTITY just outside the door, in place (same list position; type index = number of Other regions). The
    MapNameOverride type data (8 bytes before the trailing struct) is dropped, as in vanilla Other regions."""
    regions = _list(lists, "POINT_PARAM_ST").entries
    k = next((k for k, e in enumerate(regions) if entry_name(e) in (TRIGGER, PREFIX + "_enter")), None)
    assert k is not None, "no UnicornWay_name_enter / UnicornWay_title_door region"
    e = bytearray(regions[k])
    if struct.unpack_from("<I", e, 0x08)[0] == MAP_NAME_OVERRIDE:
        to, tail = struct.unpack_from("<qq", e, 0x58)
        assert tail == to + 8
        e = e[:to] + e[tail:]
        others = sum(struct.unpack_from("<I", r, 0x08)[0] == OTHER for r in regions)
        struct.pack_into("<Ii", e, 0x08, OTHER, others)
        struct.pack_into("<qq", e, 0x58, 0, to)
        name_at, after = struct.unpack_from("<q", e, 0)[0], struct.unpack_from("<q", e, 0x30)[0]
        nm = (TRIGGER + "\0").encode("utf-16-le")
        assert name_at + len(nm) <= after
        e[name_at:after] = nm.ljust(after - name_at, b"\0")
    centre, ry, size = _door_frame(lists, door, T_NEAR, T_FAR, T_LEFT, T_RIGHT, T_BELOW, T_HEIGHT)
    struct.pack_into("<3f", e, 0x14, *centre)
    struct.pack_into("<3f", e, 0x20, 0.0, ry, 0.0)
    struct.pack_into("<3f", e, struct.unpack_from("<q", e, 0x48)[0], *size)
    struct.pack_into("<i", e, struct.unpack_from("<q", e, 0x50)[0] + 4, TRIGGER_ENTITY)
    regions[k] = bytes(e)
    print(f"{TRIGGER} (region {k}, Other #{struct.unpack_from('<i', e, 0x0C)[0]}): entity {TRIGGER_ENTITY} centre "
          f"{tuple(round(v, 2) for v in centre)} rot {ry} box {size}")


def door_box(lists, door=DOOR, text_id=TEXT_ID):
    centre, ry, size = _door_frame(lists, door, DOOR_GAP, FAR, LEFT, RIGHT, BELOW, HEIGHT)
    regions = _list(lists, "POINT_PARAM_ST").entries
    ours = [k for k, e in enumerate(regions) if entry_name(e).startswith(PREFIX)
            and struct.unpack_from("<I", e, 0x08)[0] == MAP_NAME_OVERRIDE]
    assert ours, "no UnicornWay_name regions to edit"
    for k in ours:
        e = bytearray(regions[k])
        assert struct.unpack_from("<II", e, 0x08)[0] == MAP_NAME_OVERRIDE and struct.unpack_from("<I", e, 0x10)[0] == BOX
        struct.pack_into("<3f", e, 0x14, *centre)
        struct.pack_into("<3f", e, 0x20, 0.0, ry, 0.0)
        struct.pack_into("<3f", e, struct.unpack_from("<q", e, 0x48)[0], *size)
        struct.pack_into("<i", e, struct.unpack_from("<q", e, 0x58)[0], text_id)
        regions[k] = bytes(e)
        print(f"{entry_name(e)} (region {k}): centre {tuple(round(v, 2) for v in centre)} rot {ry} box {size} "
              f"text {text_id}")
    return ours


def main():
    src, dst = sys.argv[1], sys.argv[2]
    data = open(src, "rb").read()
    dcx = data[:4] == b"DCX\0"
    raw = zlib.decompress(data[0x4C:]) if dcx else data
    header, lists = read_msb(raw)
    assert write_msb(header, lists) == raw, "map does not round-trip"
    door_box(lists)
    door_trigger(lists)
    out = write_msb(header, lists)
    if dcx:
        packed = zlib.compress(out, 9)
        head = bytearray(data[:0x4C])
        struct.pack_into(">II", head, 0x1C, len(out), len(packed))
        out = bytes(head) + packed
    open(dst, "wb").write(out)


if __name__ == "__main__":
    main()
