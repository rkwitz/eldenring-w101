"""Place the hedge-maze garden (asset aeg030_555) and the glowing window of Rattlebones' tower (asset aeg030_529), both
built by blender_build_garden.py, in m60_42_36_00, with the four regions the garden door events use
(emevd.py --garden).

Wizard101: the double door at the west end of Unicorn Way ("Teleport location (Street1 Hedgemaze Entrance)
Activator", sphere at W101 (-29588.0, 19501.9, 161.3) r 1.63) loads zone WC_Unicorn_HedgeMaze (Life school's hedge
maze, geometry WC_Z06I02_Life_Hedge_Maze.nif) at "Target (Street1 Hedgemaze Entrance)" (600.7, -3195.2), facing into
the garden. The garden's own door ("TeleportVol_Teleport (Hedgemaze Street1 Exit)", (607.4, -3574.9) r 2.19) puts you
back on the street at "Target location (Street1 Hedgemaze Exit)" (-29200.8, 19500.0), 3.9 m east of the door, facing
away from it. Here the garden sits at road level north-west of the street (GARDEN_ORIGIN), turned like the street,
and the events move the player between Other-type regions as the Rattlebones doors do
(emevd.rattlebones_doors):

  W101_Garden_door  sphere r 2.0 at the street's double door        -> W101_Garden_in
  W101_Garden_exit  sphere r 1.5 on the garden's door               -> W101_Garden_out
  W101_Garden_in    garden arrival, facing into the garden
  W101_Garden_out   street arrival east of the door, facing away from it

The first version put the garden 200 m down (so its 192 m backdrop trees stayed under the road) and its door did
nothing in game (Ryan, 2026-10-23), while the Rattlebones room 60 m down works with the same events and region layout;
the street collision lets the player well into the trigger, so the 200 m warp is what failed (2004[41] is a "short
warp" request). The garden is now at the room's distance and blender_build_garden.py cuts its backdrop off below the
street (CLIP_TOP).

Positions are in the street's / garden's own FLVER frame (= W101 (x, z, y) * 0.01; heights snapped to the classic
OBJs' surfaces) and carried through the parts' transforms, so moving the street in Smithbox and running this again
keeps everything lined up (existing entries are updated in place).

The garden is open to the sky but sits under the terrain and the street, which would shade all of it: its part has
IsShadowDest cleared (as msb_add_sky.py does for the dome), so it is lit by the sun as if it were outdoors.

The window part (W101 node WC_Unicorn_T2_quest, see w101_quest_window.py) sits on the tower at WINDOW_CENTRE in the
street frame; it casts no shadows.

Usage: python msb_add_garden.py map.msb.dcx out.msb.dcx   (DFLT DCX as Smithbox saves)
"""

import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_room import TEMPLATE_REGION_ENTITY, to_world  # noqa: E402
from msb_floor import (ENTITY_DATA_FIELD, _list, _set_name, add_model, entry_name, find, read_msb,  # noqa: E402
                       set_transform, transform, write_msb)

STREET, STREET_MODEL = "AEG030_518_9000", "AEG030_518"
GARDEN, GARDEN_MODEL = "AEG030_555_9000", "AEG030_555"
WINDOW, WINDOW_MODEL = "AEG030_529_9000", "AEG030_529"
PART_ID = 9000  # part header 0x08
STREET_DOOR = (-295.88, 1.62, 195.02)  # street FLVER frame: the W101 hedge-maze door activator
GARDEN_DOOR = (6.074, 1.62, -35.75)  # garden FLVER frame: its exit door volume
# Street frame. Until 2026-10-25 the garden sat 50 m under the street door (GARDEN_DEPTH), where Ryan could see up
# through the underside of the street from inside it. Now it is at road level (garden floor = street floor), world
# (-340, 94, 100) in tile 41_36's half nearest ours, north-west of the street over the beach: checked against our
# street mesh and vanilla l60 terrain of tiles 40-42 x 35-37, no street geometry within 25 m of the maze, terrain under
# it and 40 m round it below 23 m (sea and beach), and no vertex of the garden asset (below CLIP_TOP) under terrain or
# in the street. Doors warp the 117 m between with emevd's fade-out warp.
GARDEN_ORIGIN = (-215.39, 0.0, 307.25)
WINDOW_CENTRE = (-164.777, 1.558, 200.745)  # street FLVER frame (w101_quest_window.py prints it)
OTHER, SPHERE = 0xFFFFFFFF, 2
SHADOW_SOURCES = (0x0A, 0x0C, 0x0D, 0x0E)  # entity data flags (msb_add_sky.py)
SHADOW_DEST = 0x11
# (name, frame, FLVER position, facing, radius, entity id, region id at 0x2C); facing 0 = towards the frame's -z in game,
# 180 = towards +z (see msb_add_room.REGIONS)
REGIONS = [
    ("W101_Garden_door", "street", (-295.3, 2.3, 195.02), 0.0, 2.0, 1042362804, 9204),
    ("W101_Garden_in", "garden", (6.007, 1.68, -31.95), 180.0, 0.5, 1042362805, 9205),
    ("W101_Garden_exit", "garden", (6.074, 2.4, -35.75), 0.0, 1.5, 1042362806, 9206),
    ("W101_Garden_out", "street", (-291.5, 1.68, 195.0), -91.1, 0.5, 1042362807, 9207),
]


def entity_ids(lists):
    """Every non-zero entity id of the map's parts and regions."""
    ids = set()
    for title in ("PARTS_PARAM_ST", "POINT_PARAM_ST"):
        offset = ENTITY_DATA_FIELD if title == "PARTS_PARAM_ST" else 0x50
        for e in _list(lists, title).entries:
            at = struct.unpack_from("<q", e, offset)[0]
            value = struct.unpack_from("<I", e, at + (0 if title == "PARTS_PARAM_ST" else 4))[0]
            if value:
                ids.add(value)
    return ids


def put_asset(lists, name, model_name, frame_position, ry, local, shadow_dest=True, part_id=PART_ID):
    """Add (or move) asset part `name` (model `model_name`, a copy of the street part, header id `part_id`) at FLVER
    point `local` of the street. Returns its world position."""
    parts = _list(lists, "PARTS_PARAM_ST")
    names = [entry_name(e) for e in parts.entries]
    position = to_world(frame_position, ry, local)
    if name in names:
        k = names.index(name)
        e = bytearray(parts.entries[k])
    else:
        k = None
        model = add_model(lists, model_name, STREET_MODEL)
        e = bytearray(parts.entries[find(parts, STREET)])
        assert parts.entries[-1][0x0C:0x10] == e[0x0C:0x10], "assets are not the last part type"
        _set_name(e, name)
        struct.pack_into("<i", e, 0x08, part_id)
        struct.pack_into("<i", e, 0x10, sum(1 for x in parts.entries if x[0x0C:0x10] == e[0x0C:0x10]))
        struct.pack_into("<i", e, 0x14, model)
        entity = struct.unpack_from("<q", e, ENTITY_DATA_FIELD)[0]
        struct.pack_into("<i", e, entity, 0)  # no entity id
        for flag in SHADOW_SOURCES + (() if shadow_dest else (SHADOW_DEST,)):
            e[entity + flag] = 0
    e = set_transform(bytes(e), position, (0.0, ry, 0.0))
    if k is None:
        parts.entries.append(e)
    else:
        parts.entries[k] = e
    models = _list(lists, "MODEL_PARAM_ST").entries
    model = struct.unpack_from("<i", e, 0x14)[0]
    m = bytearray(models[model])
    struct.pack_into("<i", m, 0x18, sum(struct.unpack_from("<i", x, 0x14)[0] == model for x in parts.entries))
    models[model] = bytes(m)
    print(f"{name}: position {tuple(round(c, 3) for c in position)}, rotation y {ry}")
    return position


def put_regions(lists, regions, frames):
    """Add (or move) Other-type sphere regions [(name, frame, local, facing, radius, entity, region id)] at the end
    of the region list (inside the Other block that ends it), copies of the vanilla Melina cutscene sphere.
    `frames` maps a frame name to (part position, y rotation)."""
    entries = _list(lists, "POINT_PARAM_ST").entries
    assert struct.unpack_from("<I", entries[-1], 0x08)[0] == OTHER, "regions don't end with the Other type"

    def entity_of(e):
        return struct.unpack_from("<I", e, struct.unpack_from("<q", e, 0x50)[0] + 4)[0]

    template = next(e for e in entries if entity_of(e) == TEMPLATE_REGION_ENTITY)
    assert struct.unpack_from("<I", template, 0x08)[0] == OTHER and struct.unpack_from("<I", template, 0x10)[0] == SPHERE
    ours = {r[0] for r in regions}
    used_ids = {struct.unpack_from("<i", e, 0x2C)[0] for e in entries if entry_name(e) not in ours}
    used_entities = entity_ids(lists) - {entity_of(e) for e in entries if entry_name(e) in ours}
    for name, frame, local, facing, radius, entity, region_id in regions:
        assert region_id not in used_ids, f"region id {region_id} is taken"
        assert entity not in used_entities, f"entity id {entity} is taken"
        origin, ry = frames[frame]
        position = to_world(origin, ry, local)
        rotation = (ry + facing + 180.0) % 360.0 - 180.0
        existing = [k for k, e in enumerate(entries) if entry_name(e) == name]
        if existing:
            k = existing[0]
            e = bytearray(entries[k])
        else:
            k = None
            e = bytearray(template)
            _set_name(e, name)
            struct.pack_into("<i", e, 0x0C, sum(struct.unpack_from("<I", r, 0x08)[0] == OTHER for r in entries))
            struct.pack_into("<i", e, 0x2C, region_id)
            struct.pack_into("<I", e, struct.unpack_from("<q", e, 0x50)[0] + 4, entity)
        struct.pack_into("<3f", e, 0x14, *position)
        struct.pack_into("<3f", e, 0x20, 0.0, rotation, 0.0)
        struct.pack_into("<f", e, struct.unpack_from("<q", e, 0x48)[0], radius)
        if k is None:
            entries.append(bytes(e))
        else:
            entries[k] = bytes(e)
        print(f"{name}: entity {entity}, sphere r {radius} at {tuple(round(c, 3) for c in position)}, facing {rotation:.1f}")


def place_garden(raw):
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    street_position, street_rotation = transform(parts.entries[find(parts, STREET)])
    assert street_rotation[0] == 0 and street_rotation[2] == 0, "street is tilted"
    ry = street_rotation[1]
    garden_position = put_asset(lists, GARDEN, GARDEN_MODEL, street_position, ry, GARDEN_ORIGIN, shadow_dest=False)
    put_asset(lists, WINDOW, WINDOW_MODEL, street_position, ry, WINDOW_CENTRE)
    put_regions(lists, REGIONS, {"street": (street_position, ry), "garden": (garden_position, ry)})
    return write_msb(header, lists)


def run(place):
    """Command line: python script.py map.msb.dcx out.msb.dcx, through `place(raw) -> raw`."""
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    data = src.read_bytes()
    assert data[:4] == b"DCX\0" and data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed map"
    raw = zlib.decompress(data[0x4C:])
    assert write_msb(*read_msb(raw)) == raw, "map does not round-trip"
    out = place(raw)
    packed = zlib.compress(out, 9)
    head = bytearray(data[:0x4C])
    struct.pack_into(">II", head, 0x1C, len(out), len(packed))
    dst.write_bytes(bytes(head) + packed)


if __name__ == "__main__":
    run(place_garden)
