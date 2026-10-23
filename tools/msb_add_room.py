"""Place Rattlebones' tower room (asset aeg030_519, built by blender_build_room.py) under Unicorn Way in m60_42_36_00,
with the four regions the door warp events use (emevd.py rattlebones_doors).

In Wizard101 the room is a separate zone (WC_Unicorn_T2) behind the door of the tower at the north-west end of
Unicorn Way: walking into the door's activator sphere (W101 -16493.7, 19182.1, r 1.32) loads the room at its
"Tower2 Entrance" point, and walking into the room's door (r 0.96) puts you back at "Street1 Tower2 Exit", 4 m in front
of the tower door. Here the room sits ROOM_DEPTH m under the tower, turned like the street, and the events move the
player between Other-type regions (2004[41], as the Melina cutscene does for the same map, no loading screen):

  W101_Rattlebones_door  sphere r 1.5 on the street tower's door (Door19)     -> W101_Rattlebones_in
  W101_Rattlebones_exit  sphere r 1.2 on the room's door (Door_01)            -> W101_Rattlebones_out
  W101_Rattlebones_in    room arrival, facing into the room
  W101_Rattlebones_out   street arrival in front of the tower door, facing away from it

Positions are given in the street's / room's own FLVER frame (= W101 (x, z, y) * 0.01) and carried through the parts'
transforms, so moving the street in Smithbox and running this again keeps everything lined up (existing entries are
updated in place). New regions go at the end of the region list, inside the Other-type block that ends it, so no
other region changes index; the room part goes at the end of the parts list (assets are the last part type).

Usage: python msb_add_room.py map.msb.dcx out.msb.dcx   (DFLT DCX as Smithbox saves)
"""

import math
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_floor import (ENTITY_DATA_FIELD, _list, _set_name, add_model, entry_name, find, read_msb,  # noqa: E402
                       set_transform, transform, write_msb)

STREET, STREET_MODEL = "AEG030_518_9000", "AEG030_518"
ROOM, ROOM_MODEL = "AEG030_519_9000", "AEG030_519"
ROOM_PART_ID = 9200  # part header 0x08 (unique per map)
ROOM_DEPTH = 60.0  # m under the street's road surface
TOWER_DOOR = (-164.94, 0.29, 191.82)  # street FLVER frame: the W101 door activator (Door19 on Tower40)
ROOM_ORIGIN = (TOWER_DOOR[0], -ROOM_DEPTH, TOWER_DOOR[2])  # room centre on its floor, in the street frame
OTHER = 0xFFFFFFFF
SPHERE = 2
TEMPLATE_REGION_ENTITY = 1042362980  # vanilla Other sphere (Melina cutscene player position), copied for ours
# (name, frame, FLVER position, facing: 0 = the frame's +z, 180 = its -z, radius, entity id, region id at 0x2C)
REGIONS = [
    ("W101_Rattlebones_door", "street", (-164.9, 1.0, 192.0), 0.0, 1.5, 1042362800, 9200),
    ("W101_Rattlebones_in", "room", (-0.26, 0.05, -10.3), 0.0, 0.5, 1042362801, 9201),
    ("W101_Rattlebones_exit", "room", (0.0, 0.6, -12.6), 0.0, 1.2, 1042362802, 9202),
    ("W101_Rattlebones_out", "street", (-165.03, 0.35, 187.68), 180.0, 0.5, 1042362803, 9203),
]


def to_world(frame_position, frame_rotation, local):
    """World position of `local` (FLVER coordinates of a part at `frame_position`, y rotation `frame_rotation`)."""
    c, s = math.cos(math.radians(frame_rotation)), math.sin(math.radians(frame_rotation))
    x, y, z = local
    return (frame_position[0] + x * c + z * s, frame_position[1] + y, frame_position[2] - x * s + z * c)


def place_room(raw):
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    street_position, street_rotation = transform(parts.entries[find(parts, STREET)])
    assert street_rotation[0] == 0 and street_rotation[2] == 0, "street is tilted"
    ry = street_rotation[1]
    room_position = to_world(street_position, ry, ROOM_ORIGIN)

    names = [entry_name(e) for e in parts.entries]
    if ROOM in names:
        k = names.index(ROOM)
        parts.entries[k] = set_transform(parts.entries[k], room_position, (0.0, ry, 0.0))
    else:
        model = add_model(lists, ROOM_MODEL, STREET_MODEL)
        e = bytearray(parts.entries[find(parts, STREET)])  # same draw/shadow settings as the street
        assert parts.entries[-1][0x0C:0x10] == e[0x0C:0x10], "assets are not the last part type"
        _set_name(e, ROOM)
        struct.pack_into("<i", e, 0x08, ROOM_PART_ID)
        struct.pack_into("<i", e, 0x10, sum(1 for x in parts.entries if x[0x0C:0x10] == e[0x0C:0x10]))
        struct.pack_into("<i", e, 0x14, model)
        struct.pack_into("<i", e, struct.unpack_from("<q", e, ENTITY_DATA_FIELD)[0], 0)  # no entity id
        parts.entries.append(set_transform(bytes(e), room_position, (0.0, ry, 0.0)))
        models = _list(lists, "MODEL_PARAM_ST").entries
        m = bytearray(models[model])
        struct.pack_into("<i", m, 0x18, sum(struct.unpack_from("<i", x, 0x14)[0] == model for x in parts.entries))
        models[model] = bytes(m)
    print(f"{ROOM}: position {tuple(round(c, 3) for c in room_position)}, rotation y {ry}")

    regions = _list(lists, "POINT_PARAM_ST").entries
    assert struct.unpack_from("<I", regions[-1], 0x08)[0] == OTHER, "regions don't end with the Other type"

    def entity_of(e):
        return struct.unpack_from("<I", e, struct.unpack_from("<q", e, 0x50)[0] + 4)[0]

    template = next(e for e in regions if entity_of(e) == TEMPLATE_REGION_ENTITY)
    assert struct.unpack_from("<II", template, 0x08)[0] == OTHER and struct.unpack_from("<I", template, 0x10)[0] == SPHERE
    used_ids = {struct.unpack_from("<i", e, 0x2C)[0] for e in regions if entry_name(e) not in [r[0] for r in REGIONS]}
    for name, frame, local, facing, radius, entity, region_id in REGIONS:
        assert region_id not in used_ids, f"region id {region_id} is taken"
        origin = (street_position, ry) if frame == "street" else (room_position, ry)
        position = to_world(origin[0], origin[1], local)
        rotation = (origin[1] + facing + 180.0) % 360.0 - 180.0
        existing = [k for k, e in enumerate(regions) if entry_name(e) == name]
        if existing:
            k = existing[0]
            e = bytearray(regions[k])
        else:
            k = None
            e = bytearray(template)
            _set_name(e, name)
            struct.pack_into("<i", e, 0x0C, sum(struct.unpack_from("<I", r, 0x08)[0] == OTHER for r in regions))
            struct.pack_into("<i", e, 0x2C, region_id)
            struct.pack_into("<I", e, struct.unpack_from("<q", e, 0x50)[0] + 4, entity)
        struct.pack_into("<3f", e, 0x14, *position)
        struct.pack_into("<3f", e, 0x20, 0.0, rotation, 0.0)
        struct.pack_into("<f", e, struct.unpack_from("<q", e, 0x48)[0], radius)
        if k is None:
            regions.append(bytes(e))
        else:
            regions[k] = bytes(e)
        print(f"{name}: entity {entity}, sphere r {radius} at {tuple(round(c, 3) for c in position)}, facing {rotation:.1f}")
    return write_msb(header, lists)


def main():
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    data = src.read_bytes()
    assert data[:4] == b"DCX\0" and data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed map"
    raw = zlib.decompress(data[0x4C:])
    assert write_msb(*read_msb(raw)) == raw, "map does not round-trip"
    out = place_room(raw)
    packed = zlib.compress(out, 9)
    head = bytearray(data[:0x4C])
    struct.pack_into(">II", head, 0x1C, len(out), len(packed))
    dst.write_bytes(bytes(head) + packed)


if __name__ == "__main__":
    main()
