"""Place the other enterable boss interiors of Unicorn Way under the street in m60_42_36_00, empty for now, each with
door warps both ways like Rattlebones' room (msb_add_room.py; events: emevd.py --interiors):

  Lady Blackhope's tower (W101 zone WC_Unicorn_T1, "Street1 Tower1"): the same room nif as Rattlebones'
      (WC_Generic_Gauntlet_NoDoor), so a second part of asset aeg030_519 (blender_build_room.py), AEG030_519_9001.
      Street door "Activator Volume (2)" W101 (-10512, 1917, 29) r 1.9, street exit "Target location (WC-UW T1 Exit)"
      (-10515, 2162), room arrival (-8.5, -886.1) facing the room; the room's door is Rattlebones' (same mesh).
  Fear of Fire's house (W101 zone WC_Unicorn_H4, the Alchemy clubhouse): asset aeg030_557 (blender_build_interiors.py,
      from SP_Z00I01_ClubHouse.nif), AEG030_557_9000. Street door "TeleportVol_Teleport location-ThiefHouse" W101
      (-12720, 14932, 12) r 1.01, street exit "From_AlchemyClubhouse" (-12709, 14723) facing away from the door,
      interior arrival "Start" (20.2, 840.5) facing into the house, interior door volume (-21.9, 1101.5) r 1.0.

The other doors of the street are NPC houses (H1-H3) or the arena, not boss rooms. Each interior sits DEPTH m under
its street door (the garden showed that a 200 m warp fails while the room's 60 m one works), turned like the street:
the tower room's centre right under the door as for Rattlebones, the house with its own door under the street's. The
garden (50 m down, west of x -249), Rattlebones' room (x -178..-152, z 179..205), the T1 room (x -118..-92,
z 6..32) and the house (x -146..-108, z 74..152, street frame) don't overlap.

Room/house door regions sit 0.6 m above the floor with r 1.2 (the player's position is at the feet, so a sphere
1 m up with r 1 would only catch someone standing exactly under its centre); street door spheres are as Rattlebones'
(1 m up, r 1.5), checked against the street collision for reachability.

Usage: python msb_add_interiors.py map.msb.dcx out.msb.dcx   (DFLT DCX as Smithbox saves; rerunning moves the entries)
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import STREET, put_asset, put_regions, run  # noqa: E402
from msb_floor import _list, find, read_msb, transform, write_msb  # noqa: E402

DEPTH = 60.0  # m under the street's road surface
T1_DOOR = (-105.12, 0.32, 19.17)  # street FLVER frame: the W101 activator, on the street floor
H4_DOOR = (-127.2, 0.29, 149.32)
H4_INSIDE_DOOR = (-0.219, 0.0, 11.0)  # house FLVER frame: its door volume, on the floor
# (part name, model, part id, origin in the street frame)
INTERIORS = [
    ("AEG030_519_9001", "AEG030_519", 9201, (T1_DOOR[0], -DEPTH, T1_DOOR[2])),
    ("AEG030_557_9000", "AEG030_557", 9202, (H4_DOOR[0] - H4_INSIDE_DOOR[0], -DEPTH, H4_DOOR[2] - H4_INSIDE_DOOR[2])),
]
# (name, frame, FLVER position, facing, radius, entity id, region id at 0x2C); facing 0 = towards the frame's -z in game,
# 180 = towards +z (see msb_add_room.REGIONS)
REGIONS = [
    ("W101_Blackhope_door", "street", (-105.12, 1.0, 19.17), 0.0, 1.5, 1042362816, 9210),
    ("W101_Blackhope_in", "AEG030_519_9001", (-0.085, 0.05, -8.86), 180.0, 0.5, 1042362817, 9211),
    ("W101_Blackhope_exit", "AEG030_519_9001", (0.0, 0.6, -12.6), 0.0, 1.2, 1042362818, 9212),
    ("W101_Blackhope_out", "street", (-105.15, 0.38, 21.62), 180.0, 0.5, 1042362819, 9213),
    ("W101_FearOfFire_door", "street", (-127.2, 1.0, 149.32), 0.0, 1.5, 1042362820, 9214),
    ("W101_FearOfFire_in", "AEG030_557_9000", (0.202, 0.05, 8.405), 0.0, 0.5, 1042362821, 9215),
    ("W101_FearOfFire_exit", "AEG030_557_9000", (-0.219, 0.6, 11.0), 0.0, 1.2, 1042362822, 9216),
    ("W101_FearOfFire_out", "street", (-127.09, 0.35, 146.8), 0.0, 0.5, 1042362823, 9217),
]


def place_interiors(raw):
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    street_position, street_rotation = transform(parts.entries[find(parts, STREET)])
    assert street_rotation[0] == 0 and street_rotation[2] == 0, "street is tilted"
    ry = street_rotation[1]
    frames = {"street": (street_position, ry)}
    for name, model, part_id, origin in INTERIORS:
        frames[name] = (put_asset(lists, name, model, street_position, ry, origin, part_id=part_id), ry)
    put_regions(lists, REGIONS, frames)
    return write_msb(header, lists)


if __name__ == "__main__":
    run(place_interiors)
