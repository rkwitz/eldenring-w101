"""Add one ghost enemy to m60_42_36_00 (The First Step) walking a Wizard101 Unicorn Way ghost patrol.

The ghost is a Wandering Noble (chr c4300) using the "Stranded Soul" NpcParam row, the translucent wandering-spirit
variant. Its route is W101's "Path Ghost 01" from WC_Unicorn/pathData.xml: the path lists node ids whose positions are in
pathNodeData.bin (W101 units, Z up). Those are converted the same way as the street mesh (nif2obj: x0.01, Y up) and
then placed with the street part's transform, so the points land on the street wherever it was moved to.

Adds: model c4300 (copy of a cXXXX model), regions walk_route_c4300_9000_99_NN (copies of a patrol point region),
a PatrolInfo event looping through them, and the enemy part c4300_9000 (copy of an existing enemy) using that route.

Usage: python msb_add_ghost.py map.msb.dcx WC_Unicorn_dir out.msb.dcx
"""

import math
import re
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_floor import _list, _set_name, add_model, entry_name, find, read_msb, write_msb  # noqa: E402

STREET = "AEG030_518_9000"
PATH = "Path Ghost 01"
MODEL, MODEL_TEMPLATE = "c4300", "c4200"
PART, PART_TEMPLATE = "c4300_9000", "c4200_9002"
REGION_TEMPLATE = "walk_route_c4200_9008_19_61"
EVENT_TEMPLATE_TYPE_INDEX = 1  # a looping patrol route with points
EVENT_NAME = "ghost_walk"
THINK_ID, NPC_ID = 43000000, 43009000  # Wandering Noble AI, Wandering Noble (Stranded Soul)
REGION_ID = 9100

PART_TYPE, PATROL_EVENT, PATROL_POINT = 0x0C, 20, 32
TYPE_DATA = 0x68  # part header field: offset of the enemy type data
THINK, NPC, WALK_ROUTE = 0x08, 0x0C, 0x20  # in the enemy type data


def path_points(zone):
    """World-less W101 positions (x, y, z) of the nodes of PATH, from pathData.xml and pathNodeData.bin."""
    nodes, data, i = {}, (zone / "pathNodeData.bin").read_bytes(), 0
    while (i := data.find(bytes.fromhex("1bdc7e85"), i)) >= 0:  # position member hash, then 3 floats
        node_id = struct.unpack_from("<Q", data, data.find(bytes.fromhex("45909b7c"), i) + 4)[0]
        nodes[node_id] = struct.unpack_from("<3f", data, i + 4)
        i += 4
    paths = (zone / "pathData.xml").read_bytes()
    k = re.search(re.escape(PATH.encode()), paths).end() + 8  # name, then size and member hash
    count = paths[k] // 2
    return [nodes[struct.unpack_from("<Q", paths, k + 1 + 8 * n)[0]] for n in range(count)]


def to_world(point, street):
    """W101 point -> map position, through the street mesh conversion (FLVER = (x, z, y) * 0.01) and part transform."""
    x, y, z = (c * 0.01 for c in point)
    fx, fy, fz = x, z, y
    position, rotation = struct.unpack_from("<3f", street, 0x20), struct.unpack_from("<3f", street, 0x2C)
    a = math.radians(rotation[1])
    return (position[0] + fx * math.cos(a) + fz * math.sin(a), position[1] + fy,
            position[2] - fx * math.sin(a) + fz * math.cos(a))


def add_ghost(raw, zone):
    header, lists = read_msb(raw)
    parts, regions, events = _list(lists, "PARTS_PARAM_ST"), _list(lists, "POINT_PARAM_ST"), _list(lists, "EVENT_PARAM_ST")
    assert PART not in [entry_name(e) for e in parts.entries], f"{PART} is already in the map"
    street = parts.entries[find(parts, STREET)]
    points = [to_world(p, street) for p in path_points(zone)]

    template = regions.entries[find(regions, REGION_TEMPLATE)]
    type_index = 1 + max(struct.unpack_from("<i", e, 0x0C)[0] for e in regions.entries
                         if struct.unpack_from("<i", e, 0x08)[0] == PATROL_POINT)
    first_region = len(regions.entries)
    for n, p in enumerate(points):
        e = bytearray(template)
        _set_name(e, f"walk_route_{PART}_99_{n + 1:02d}")
        struct.pack_into("<i", e, 0x0C, type_index + n)
        struct.pack_into("<3f", e, 0x14, *p)
        struct.pack_into("<i", e, 0x2C, REGION_ID + n)
        regions.entries.append(bytes(e))

    patrols = [e for e in events.entries if struct.unpack_from("<i", e, 0x0C)[0] == PATROL_EVENT]
    route_index = len(patrols)
    e = bytearray(next(e for e in patrols if struct.unpack_from("<i", e, 0x10)[0] == EVENT_TEMPLATE_TYPE_INDEX))
    _set_name(e, EVENT_NAME)
    # 0x08 is the event's ordinal among all events (small, unique), not a free id.
    struct.pack_into("<ii", e, 0x08, 1 + max(struct.unpack_from("<i", x, 0x08)[0] for x in events.entries), PATROL_EVENT)
    struct.pack_into("<i", e, 0x10, route_index)
    type_data = struct.unpack_from("<q", e, 0x20)[0]
    e[type_data] = 0  # patrol type 0: back to the first point after the route, i.e. loop
    indices = [first_region + n for n in range(len(points))] + [-1] * (64 - len(points))
    struct.pack_into("<64h", e, type_data + 16, *indices)
    events.entries.append(bytes(e))

    model = add_model(lists, MODEL, MODEL_TEMPLATE)
    e = bytearray(parts.entries[find(parts, PART_TEMPLATE)])
    _set_name(e, PART)
    struct.pack_into("<i", e, 0x08, int(PART.split("_")[1]))
    struct.pack_into("<i", e, 0x10, sum(1 for x in parts.entries if x[0x0C:0x10] == e[0x0C:0x10]))  # index in its type
    struct.pack_into("<i", e, 0x14, model)
    struct.pack_into("<3f", e, 0x20, *points[0])
    struct.pack_into("<3f", e, 0x2C, 0.0, 0.0, 0.0)
    t = struct.unpack_from("<q", e, TYPE_DATA)[0]
    struct.pack_into("<ii", e, t + THINK, THINK_ID, NPC_ID)
    struct.pack_into("<h", e, t + WALK_ROUTE, route_index)
    parts.entries.append(bytes(e))
    return write_msb(header, lists), points


def main():
    src, zone, dst = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    data = src.read_bytes()
    assert data[:4] == b"DCX\0" and data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed map"
    out, points = add_ghost(zlib.decompress(data[0x4C:]), zone)
    packed = zlib.compress(out, 9)
    head = bytearray(data[:0x4C])
    struct.pack_into(">II", head, 0x1C, len(out), len(packed))
    dst.write_bytes(bytes(head) + packed)
    for p in points:
        print("route point", tuple(round(c, 2) for c in p))


if __name__ == "__main__":
    main()
