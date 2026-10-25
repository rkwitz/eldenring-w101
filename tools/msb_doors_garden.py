"""Garden to road level and door arrivals turned round, in m60_42_36_00 (Ryan's test, 2026-10-25).

1. msb_add_garden.place_garden with its new GARDEN_ORIGIN: the garden asset moves from 50 m under the street door to
   road level north-west of the street (see msb_add_garden.GARDEN_ORIGIN), with its window and door regions.
   What other scripts placed in the garden frame moves with it here, by the garden part's move (same rotation):
   msb_add_quests' Lady Oriel statue AEG099_010_9003 and goal sphere W101_Q_hedge_maze. The statue also goes down
   onto the sunken centre floor (garden FLVER y -0.7; w101_quests had 1.68, Oriel floats 4 m up in Wizard101, and the
   winged statue hung in the air). msb_add_boss.py (Seraph) and msb_add_indoor.py (garden weather box) follow the
   garden part when they run again, so run them after this.
2. Every door arrival region (W101_*_in / _out of msb_add_room, msb_add_interiors and msb_add_garden) gets its
   tables' facing again; the tables were turned round by 180 degrees: in game the player arrived facing the door
   they had come through.

Usage: python msb_doors_garden.py map.msb.dcx out.msb.dcx
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import msb_add_interiors  # noqa: E402
import msb_add_room  # noqa: E402
from msb_add_garden import GARDEN, STREET, place_garden, put_regions, run  # noqa: E402
from msb_floor import _list, entry_name, find, read_msb, transform, write_msb  # noqa: E402
from msb_add_room import to_world  # noqa: E402

CARRIED_PARTS = ["AEG099_010_9003"]  # msb_add_quests: Lady Oriel
CARRIED_REGIONS = ["W101_Q_hedge_maze"]  # msb_add_quests: her goal sphere
ORIEL_FLOOR = -0.7  # garden FLVER frame: the sunken centre under her


def _local(origin, ry, world):
    """Inverse of to_world."""
    import math
    c, s = math.cos(math.radians(ry)), math.sin(math.radians(ry))
    dx, dy, dz = (w - o for w, o in zip(world, origin))
    return (c * dx - s * dz, dy, s * dx + c * dz)


def doors_garden(raw):
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    old_origin, old_rotation = transform(parts.entries[find(parts, GARDEN)])
    raw = place_garden(write_msb(header, lists))
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    origin, rotation = transform(parts.entries[find(parts, GARDEN)])
    assert rotation == old_rotation, "the garden turned"
    ry = rotation[1]

    def carry(e, at, floor=None):
        e = bytearray(e)
        local = _local(old_origin, ry, struct.unpack_from("<3f", e, at))
        if floor is not None:
            local = (local[0], floor, local[2])
        struct.pack_into("<3f", e, at, *to_world(origin, ry, local))
        return bytes(e), local

    for name in CARRIED_PARTS:
        k = find(parts, name)
        parts.entries[k], local = carry(parts.entries[k], 0x20, ORIEL_FLOOR)
        print(f"{name}: garden frame {tuple(round(c, 3) for c in local)}")
    regions = _list(lists, "POINT_PARAM_ST").entries
    for k, e in enumerate(regions):
        if entry_name(e) in CARRIED_REGIONS:
            regions[k], local = carry(e, 0x14)
            print(f"{entry_name(e)}: garden frame {tuple(round(c, 3) for c in local)}")

    street = transform(parts.entries[find(parts, STREET)])
    frames = {"street": (street[0], street[1][1]), "room": None, "garden": (origin, ry)}
    for name in ("AEG030_519_9000", "AEG030_519_9001", "AEG030_557_9000"):
        p, r = transform(parts.entries[find(parts, name)])
        frames[name] = (p, r[1])
    frames["room"] = frames["AEG030_519_9000"]
    arrivals = [r for r in msb_add_room.REGIONS + msb_add_interiors.REGIONS
                if r[0].endswith("_in") or r[0].endswith("_out")]
    put_regions(lists, arrivals, frames)
    return write_msb(header, lists)


if __name__ == "__main__":
    run(doors_garden)
