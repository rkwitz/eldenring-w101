"""Make Unicorn Way's interiors in m60_42_36_00 behave like indoors: no rain, no Torrent; and no rune skulls anywhere
(NO_SKULLS).

Rain fell inside Rattlebones' room (Ryan, 2026-10-23): the rooms are assets 50-60 m under the street, still in the
open-world tile, so they get its weather. Elden Ring's own answer for weather inside a piece of an open map is the
WeatherOverride region (type 35): inside it the region's weather (a WeatherParam id or a WeatherLotParam id) replaces
the tile's. Weather 2000 is what vanilla uses for the inside of the Grand Lift of Dectus (m60_38_50_00 "inside the big
elevator") and under the Church of the Crucible (m61_51_46_00), so every interior and the hedge-maze garden (sunny
in Wizard101) gets a WeatherOverride box with 2000.

Torrent: HorseRideOverride regions (type 53, override 1 = riding not allowed, 2 = allowed), as vanilla puts in boss
rooms (m60_35_51_00 "lake fort boss room no-horse area"). The tower rooms and the house are no-riding throughout, so
riding in through a door (emevd.rattlebones_doors warps a mounted player too) puts the player on foot inside; the
garden is outdoors and stays rideable. The 5 m no-riding boxes round the door triggers of the first version knocked
Ryan off Torrent before he reached the door (2026-10-25); rerunning removes them (REMOVED).

Each region is a byte copy of those vanilla regions (box shape) with name, position, rotation, size, region id and
type index set; new types go in their place in the type-ordered region list (35 after the patrol points, 53 before
the Other regions) and every region index the map's events hold is remapped (msb_add_ghost.add_route's rule).
Boxes are given in a part's FLVER frame (centre of the floor footprint, size x/z, floor height); they start BELOW m
under the floor and are HEIGHT m tall. Rerunning updates the boxes in place.

Usage: python msb_add_indoor.py map.msb.dcx vanilla_dir out.msb.dcx [--door-boxes]
  vanilla_dir: uncompressed vanilla m60_38_50_00.msb, m60_35_51_00.msb and m61_51_45_00.msb
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import run  # noqa: E402
from msb_add_ghost import KNOWN_EVENTS, check_grouped, q, u32  # noqa: E402
from msb_add_room import to_world  # noqa: E402
from msb_floor import _list, _set_name, entry_name, find, read_msb, transform, write_msb  # noqa: E402

WEATHER, HORSE, TUMBLEWEED, PATROL_EVENT = 35, 53, 27, 20
TEMPLATES = {WEATHER: ("m60_38_50_00.msb", "領域 大エレベータ内 天候変化"),
             HORSE: ("m60_35_51_00.msb", "湖砦　ボス部屋内　馬禁止エリア"),
             TUMBLEWEED: ("m61_51_45_00.msb", "教会地下 タンブルウィード無効")}
# DisableTumbleweed (type 27) box over the whole of Unicorn Way, its rooms and the garden, in m60_42_36_00's own
# coordinates (x, z centre, size x, size z, bottom y, height): the glowing "rune skulls" Ryan kept seeing on the road
# (2026-10-26) are not map parts but Elden Ring's random rolling objects, spawned in the open world round the player
# like tumbleweeds (mods that drop them edit RollingObjLotParam); vanilla turns them off under the Church of the
# Crucible with this region type (m61_51_45_00 "tumbleweed disabled"). Covers footprint_clear's footprint
# (x -541..202, z -658..73) and the garden (to z 140).
NO_SKULLS = ("W101_NoSkulls", (-170.0, -250.0), 780.0, 860.0, -20.0, 450.0)
NO_SKULLS_REGION_ID = 9246
INDOOR_WEATHER = 2000
NO_RIDING = 1
BELOW, HEIGHT = 3.0, 40.0
STREET = "AEG030_518_9000"
ROOMS = [  # (key, part, footprint centre (FLVER), size x, size z, floor height)
    ("T2", "AEG030_519_9000", (0.0, 0.0), 27.0, 27.0, 0.0),
    ("T1", "AEG030_519_9001", (0.0, 0.0), 27.0, 27.0, 0.0),
    ("H4", "AEG030_557_9000", (0.0, -25.25), 38.0, 79.0, 0.0),
]
GARDEN = ("G", "AEG030_555_9000", (6.0, 0.0), 90.0, 90.0, -1.0)
# --door-boxes: tight no-riding boxes on the door triggers themselves (DOOR_BOX m square, from 1 m under the trigger
# centre, DOOR_HEIGHT m tall), for door events that wait for the player on foot (emevd.py --bosses --on-foot), in case
# a mounted player can't be warped: he rides right up to the door and is set down in it. Without the option they are
# removed (the round-4 5 m boxes knocked him off before the door).
DOOR_TRIGGERS = [("T2d", "W101_Rattlebones_door"), ("T1d", "W101_Blackhope_door"), ("H4d", "W101_FearOfFire_door"),
                 ("Gd", "W101_Garden_door"), ("Gx", "W101_Garden_exit")]
REMOVED = [f"W101_NoRide_{k}" for k, _ in DOOR_TRIGGERS]
DOOR_BOX, DOOR_HEIGHT, DOOR_REGION_ID = 3.0, 5.0, 9237  # region ids 9237-9241
FIRST_REGION_ID = 9230


def boxes():
    """[(name, region type, part, centre (x, z), size x, size z, floor)]"""
    out = [(f"W101_NoRain_{k}", WEATHER, p, c, sx, sz, f) for k, p, c, sx, sz, f in ROOMS + [GARDEN]]
    out += [(f"W101_NoRide_{k}", HORSE, p, c, sx, sz, f) for k, p, c, sx, sz, f in ROOMS]
    return out


def insert_region(lists, entry, kind):
    """Insert region `entry` of type `kind` at the end of its type block (or where the block belongs in the
    type-ordered list), remapping every region index held by the map's events. Returns its index."""
    regions, events = _list(lists, "POINT_PARAM_ST").entries, _list(lists, "EVENT_PARAM_ST").entries
    kinds = {u32(e, 0x0C) for e in events}
    assert kinds <= KNOWN_EVENTS, f"events of types {kinds - KNOWN_EVENTS} may hold region indices; not handled"
    types = [u32(e, 0x08) & 0xFFFFFFFF for e in regions]  # Other (-1) sorts last
    at = max([i + 1 for i, t in enumerate(types) if t <= kind] or [0])
    for k, e in enumerate(events):
        e = bytearray(e)
        common = q(e, 0x18)
        index = struct.unpack_from("<i", e, common + 4)[0]
        if index >= at:
            struct.pack_into("<i", e, common + 4, index + 1)
        if u32(e, 0x0C) == PATROL_EVENT:
            td = q(e, 0x20)
            old = struct.unpack_from("<64h", e, td + 16)
            struct.pack_into("<64h", e, td + 16, *[i + 1 if i >= at else i for i in old])
        events[k] = bytes(e)
    regions.insert(at, entry)
    return at


def remove_region(lists, name):
    """Drop region `name`, remapping every region index the map's events hold and renumbering its type's indices."""
    regions, events = _list(lists, "POINT_PARAM_ST").entries, _list(lists, "EVENT_PARAM_ST").entries
    at = next(k for k, e in enumerate(regions) if entry_name(e) == name)
    kind = u32(regions[at], 0x08)
    for k, e in enumerate(events):
        e = bytearray(e)
        common = q(e, 0x18)
        index = struct.unpack_from("<i", e, common + 4)[0]
        assert index != at, f"event {entry_name(e)} uses {name}"
        if index > at:
            struct.pack_into("<i", e, common + 4, index - 1)
        if u32(e, 0x0C) == PATROL_EVENT:
            td = q(e, 0x20)
            old = struct.unpack_from("<64h", e, td + 16)
            assert at not in old
            struct.pack_into("<64h", e, td + 16, *[i - 1 if i > at else i for i in old])
        events[k] = bytes(e)
    del regions[at]
    n = 0
    for k, e in enumerate(regions):
        if u32(e, 0x08) == kind:
            e = bytearray(e)
            struct.pack_into("<i", e, 0x0C, n)
            regions[k] = bytes(e)
            n += 1
    print(f"{name}: removed")


def place_indoor(raw, vanilla_dir, door_boxes=False):
    header, lists = read_msb(raw)
    for name in REMOVED:
        if any(entry_name(e) == name for e in _list(lists, "POINT_PARAM_ST").entries):
            remove_region(lists, name)
    templates = {}
    for kind, (file, name) in TEMPLATES.items():
        _, vanilla = read_msb((Path(vanilla_dir) / file).read_bytes())
        templates[kind] = next(e for e in _list(vanilla, "POINT_PARAM_ST").entries if entry_name(e) == name)
        assert u32(templates[kind], 0x08) == kind and u32(templates[kind], 0x10) == 5, "expected a box region"
    parts = _list(lists, "PARTS_PARAM_ST").entries
    regions = _list(lists, "POINT_PARAM_ST").entries
    for n, (name, kind, part, centre, size_x, size_z, floor) in enumerate(boxes()):
        position, rotation = transform(parts[find(_list(lists, "PARTS_PARAM_ST"), part)])
        ry = rotation[1]
        world = to_world(position, ry, (centre[0], floor - BELOW, centre[1]))
        existing = [k for k, e in enumerate(regions) if entry_name(e) == name]
        e = bytearray(regions[existing[0]] if existing else templates[kind])
        if not existing:
            _set_name(e, name)
            region_id = FIRST_REGION_ID + n
            assert region_id not in {struct.unpack_from("<i", r, 0x2C)[0] for r in regions}, f"region id {region_id}"
            struct.pack_into("<i", e, 0x0C, sum(u32(r, 0x08) == kind for r in regions))
            struct.pack_into("<i", e, 0x2C, region_id)
            struct.pack_into("<I", e, q(e, 0x50) + 4, 0)  # no entity id
            struct.pack_into("<i", e, q(e, 0x58), INDOOR_WEATHER if kind == WEATHER else NO_RIDING)
        struct.pack_into("<3f", e, 0x14, *world)
        struct.pack_into("<3f", e, 0x20, 0.0, ry, 0.0)
        struct.pack_into("<3f", e, q(e, 0x48), size_x, size_z, HEIGHT)
        if existing:
            regions[existing[0]] = bytes(e)
        else:
            insert_region(lists, bytes(e), kind)
        print(f"{name}: type {kind}, box {size_x} x {size_z} x {HEIGHT} from {tuple(round(c, 2) for c in world)}")
    name, (cx, cz), size_x, size_z, bottom, height = NO_SKULLS
    regions = _list(lists, "POINT_PARAM_ST").entries
    existing = [k for k, e in enumerate(regions) if entry_name(e) == name]
    e = bytearray(regions[existing[0]] if existing else templates[TUMBLEWEED])
    if not existing:
        _set_name(e, name)
        assert NO_SKULLS_REGION_ID not in {struct.unpack_from("<i", r, 0x2C)[0] for r in regions}
        struct.pack_into("<i", e, 0x0C, sum(u32(r, 0x08) == TUMBLEWEED for r in regions))
        struct.pack_into("<i", e, 0x2C, NO_SKULLS_REGION_ID)
        struct.pack_into("<I", e, q(e, 0x50) + 4, 0)
    struct.pack_into("<3f", e, 0x14, cx, bottom, cz)
    struct.pack_into("<3f", e, 0x20, 0.0, 0.0, 0.0)
    struct.pack_into("<3f", e, q(e, 0x48), size_x, size_z, height)
    if existing:
        regions[existing[0]] = bytes(e)
    else:
        insert_region(lists, bytes(e), TUMBLEWEED)
    print(f"{name}: no rolling objects in {size_x} x {size_z} x {height} m round ({cx}, {bottom}, {cz})")
    if door_boxes:
        street_ry = transform(parts[find(_list(lists, "PARTS_PARAM_ST"), STREET)])[1][1]
        for n, (key, trigger) in enumerate(DOOR_TRIGGERS):
            regions = _list(lists, "POINT_PARAM_ST").entries
            x, y, z = struct.unpack_from("<3f", regions[next(k for k, e in enumerate(regions) if entry_name(e) == trigger)], 0x14)
            e = bytearray(templates[HORSE])
            _set_name(e, f"W101_NoRide_{key}")
            assert DOOR_REGION_ID + n not in {struct.unpack_from("<i", r, 0x2C)[0] for r in regions}
            struct.pack_into("<i", e, 0x0C, sum(u32(r, 0x08) == HORSE for r in regions))
            struct.pack_into("<i", e, 0x2C, DOOR_REGION_ID + n)
            struct.pack_into("<I", e, q(e, 0x50) + 4, 0)
            struct.pack_into("<i", e, q(e, 0x58), NO_RIDING)
            struct.pack_into("<3f", e, 0x14, x, y - 1.0, z)
            struct.pack_into("<3f", e, 0x20, 0.0, street_ry, 0.0)
            struct.pack_into("<3f", e, q(e, 0x48), DOOR_BOX, DOOR_BOX, DOOR_HEIGHT)
            insert_region(lists, bytes(e), HORSE)
            print(f"W101_NoRide_{key}: {DOOR_BOX} m no-riding box on {trigger}")
    check_grouped(lists)
    return write_msb(header, lists)


if __name__ == "__main__":
    door_boxes = "--door-boxes" in sys.argv
    sys.argv = [a for a in sys.argv if a != "--door-boxes"]
    vanilla_dir = sys.argv[2]
    del sys.argv[2]
    run(lambda raw: place_indoor(raw, vanilla_dir, door_boxes))
