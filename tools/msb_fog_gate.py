"""Round 24 (M3): the Stranded Graveyard exit (the lift-up gate Ryan pulls open into Unicorn Way) is a fog gate seen
from Unicorn Way at every distance.

Ryan: "from the Unicorn Way side, looking back at it, the gate stands alone in the archway with out-of-bounds visible
behind it; up close it becomes a fog gate. While on Unicorn Way, that gate should ALWAYS be a fog gate at any
distance." The fog is m18's own asset AEG099_230_9001 (entity 18001696, no event uses it) standing in the gate,
m18 (-150.56, 77.82, 16.27) rot y 153.0: it only exists while m18 is loaded, i.e. near the gate. This puts a copy of
it in m60_42_36_00 at the same world spot (m18 -> m60_42_36 = +(147, 16, -102), WorldMapLegacyConvParam rows
140/141, er-map-edits), as a copy of the street part, so it carries the street's draw settings (PartsDrawParam row
36: drawn at any distance, no LOD fade), no entity id, casting no shadow.

Usage: python msb_fog_gate.py map.msb.dcx out.msb.dcx   (DFLT DCX as Smithbox saves; rerunning moves it in place)
"""
import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import STREET, put_asset, run  # noqa: E402
from msb_floor import _list, entry_name, find, read_msb, write_msb  # noqa: E402

FOG_MODEL = "AEG099_230"
FOG_PART = "AEG099_230_9100"
FOG_PART_ID = 9400  # part header 0x08 (unique in the map; ours use 9000-9312)
M18_POSITION, M18_ROTATION = (-150.56, 77.82, 16.27), 153.0  # m18_00_00_00 AEG099_230_9001
M18_TO_M60 = (147.0, 16.0, -102.0)


def place_fog(raw):
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    assert all(struct.unpack_from("<i", e, 0x08)[0] != FOG_PART_ID or entry_name(e) == FOG_PART for e in parts.entries)
    street = parts.entries[find(parts, STREET)]
    (px, py, pz), sy = struct.unpack_from("<3f", street, 0x20), struct.unpack_from("<3f", street, 0x2C)[1]
    world = tuple(a + b for a, b in zip(M18_POSITION, M18_TO_M60))
    # inverse of msb_add_room.to_world: the point in the street's own frame
    c, s = math.cos(math.radians(sy)), math.sin(math.radians(sy))
    dx, dz = world[0] - px, world[2] - pz
    local = (dx * c - dz * s, world[1] - py, dx * s + dz * c)
    ry = (M18_ROTATION + 180.0) % 360.0 - 180.0
    put_asset(lists, FOG_PART, FOG_MODEL, (px, py, pz), sy, local, part_id=FOG_PART_ID)
    # put_asset turns the part by the street's yaw; the fog keeps m18's own rotation
    k = find(parts, FOG_PART)
    e = bytearray(parts.entries[k])
    struct.pack_into("<3f", e, 0x2C, 0.0, ry, 0.0)
    parts.entries[k] = bytes(e)
    print(f"{FOG_PART}: world {tuple(round(v, 3) for v in world)}, rotation y {ry}")
    return write_msb(header, lists)


if __name__ == "__main__":
    run(place_fog)
