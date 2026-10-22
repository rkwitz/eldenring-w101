"""Put one ghost enemy in m60_42_36_00 (The First Step) walking Wizard101's Unicorn Way "Path Ghost 01" as a loop.

Why the first ghost (2026-10-20) crashed the game on load: it was the only edit ever written that broke the type
grouping of the map's lists. Every vanilla map, and every file Smithbox/SoulsFormats writes, keeps each list's entries
in contiguous blocks by type (parts: map pieces, enemies, players, collisions, dummy assets, dummy enemies, connect
collisions, assets; events: treasures, ..., patrol infos, ..., sign pools; models and regions likewise). The old script
appended the c4300 model after the asset models, the enemy part after the assets, the 6 patrol points after the
"Other" regions and the PatrolInfo event after the sign pool. Appending assets (sky, floor, footprint) never broke
the grouping because assets are the last type, which is why only the ghost crashed. The entries themselves matched
vanilla patrollers byte for byte apart from that (and a wrong model instance count, copied from c4200).

What this does instead, keeping every list grouped and every index consistent:
  - model c4300: a copy of the vanilla Limgrave c4300 model entry, inserted in name order among the character models;
    every part's model index is remapped and the character models' type indices renumbered.
  - patrol points: copies of a vanilla patrol point region, inserted at the end of the patrol-point block (type 32);
    every region index in the events (common region index, patrol point lists) is remapped. The map may only hold
    event types whose region references are known (treasure, ObjAct, patrol info, sign pool), else this refuses.
  - the PatrolInfo event: inserted at the end of the PatrolInfo block, with the next type index and event ordinal.
  - the enemy: no new part. Inserting a part would shift every later part's global index, which many fields hold
    (treasures, ObjActs, sign pools, assets, regions...). Instead an existing vanilla enemy that is buried under the
    street (REPLACE, a c6001 with no entity id, no groups, no route, not referenced by any event) becomes the ghost
    in place: same slot, same per-type index, new name/model/params/position/route. Its model's instance count drops.

Path points are W101 node positions (cm, Z up; from w101_pathing.py's JSON) through the street mesh conversion
(FLVER = (x, z, y) * 0.01) and the street part's current transform; the height is snapped to the street surface of
the OBJ under each point (highest surface not above the node + 1 m, so arches and roofs are ignored).

Navmesh: Elden Ring characters walk patrol routes by navmesh path queries. The W101 street has no navmesh, so the
ghost will most likely stand at its spawn (first point) and only turn/attack, unless the old navmesh of the tile
happens to lie close under it. See er-enemies.md for the options.

Usage: python msb_add_ghost.py map.msb.dcx pathing.json street.obj template_map.msb out.msb.dcx [--plain] [--static]
  template_map.msb: vanilla m60_43_36_00.msb (holds a Wandering Noble c4300_9020 that patrols a walk route)
  --plain:  vanilla Limgrave Wandering Noble params (Think 43005000, NpcParam 43005210) instead of the Stranded Soul
  --static: no patrol (no regions, no event); just the ghost standing at the first point
"""

import json
import math
import struct
import sys
import zlib
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from msb_floor import _list, _set_name, entry_name, find, read_msb, write_msb  # noqa: E402

STREET = "AEG030_518_9000"
PATH = "Path Ghost 01"
MODEL = "c4300"
PART = "c4300_9000"
REPLACE = "c6001_9017"  # vanilla enemy under the street (y 63.5, ~30 m below the road), becomes the ghost
TEMPLATE_PART = "c4300_9020"  # in template_map: vanilla Wandering Noble walking walk_route_c4300_9020_5
EVENT_NAME = "ghost_walk"
STRANDED_SOUL = 43005000, 43009000  # Think (Limgrave patrolling noble), NpcParam "Wandering Noble (Stranded Soul)"
PLAIN = 43005000, 43005210  # exactly vanilla c4300_9020's
REGION_ID = 9100  # region 0x2C: unique id, also the walk_route_<part>_<event>_<id> name suffix in vanilla

MODEL_CHR, PART_ENEMY, EVENT_PATROL, REGION_PATROL = 2, 2, 20, 32
KNOWN_EVENTS = {4, 7, 20, 23}  # treasure, ObjAct, PatrolInfo, SignPool: only common +4 / patrol points hold regions
THINK, WALK_ROUTE = 0x08, 0x20  # in the enemy type data (Smithbox meta: WalkRouteIndex s16 = PatrolInfo type index)


def u32(e, o):
    return struct.unpack_from("<i", e, o)[0]


def q(e, o):
    return struct.unpack_from("<q", e, o)[0]


def path_points(pathing):
    """W101 positions (cm, Z up) of PATH's nodes in walk order."""
    path = next(p for p in pathing["paths"] if p["name"] == PATH)
    return [pathing["nodes"][str(n)]["pos"] for n in path["nodes"]]


def street_surface(obj):
    """(xz triangles, heights) of the OBJ, in FLVER coordinates (OBJ z negated)."""
    v, f = [], []
    with open(obj) as fh:
        for line in fh:
            if line.startswith("v "):
                x, y, z = map(float, line.split()[1:4])
                v.append((x, y, -z))
            elif line.startswith("f "):
                ids = [int(t.split("/")[0]) - 1 for t in line.split()[1:]]
                f += [(ids[0], ids[k], ids[k + 1]) for k in range(1, len(ids) - 1)]
    return np.array(v)[np.array(f)]


def surface_height(tris, fx, fz, below):
    """Highest street surface at FLVER (fx, fz) that is not above `below`, or None."""
    a, b, c = tris[:, 0], tris[:, 1], tris[:, 2]
    d = (b[:, 2] - c[:, 2]) * (a[:, 0] - c[:, 0]) + (c[:, 0] - b[:, 0]) * (a[:, 2] - c[:, 2])
    ok = np.abs(d) > 1e-9
    d = np.where(ok, d, 1)
    l1 = ((b[:, 2] - c[:, 2]) * (fx - c[:, 0]) + (c[:, 0] - b[:, 0]) * (fz - c[:, 2])) / d
    l2 = ((c[:, 2] - a[:, 2]) * (fx - c[:, 0]) + (a[:, 0] - c[:, 0]) * (fz - c[:, 2])) / d
    l3 = 1 - l1 - l2
    inside = ok & (l1 >= 0) & (l2 >= 0) & (l3 >= 0)
    y = l1 * a[:, 1] + l2 * b[:, 1] + l3 * c[:, 1]
    y = y[inside & (y <= below)]
    return float(y.max()) if len(y) else None


def to_world(point, street, tris):
    """W101 point -> (map position, surface found?) through the street conversion and part transform."""
    fx, fy, fz = point[0] * 0.01, point[2] * 0.01, point[1] * 0.01
    surface = surface_height(tris, fx, fz, fy + 1.0) if tris is not None else None
    fy = fy if surface is None else surface
    (px, py, pz), rotation = struct.unpack_from("<3f", street, 0x20), struct.unpack_from("<3f", street, 0x2C)
    a = math.radians(rotation[1])
    return (px + fx * math.cos(a) + fz * math.sin(a), py + fy, pz - fx * math.sin(a) + fz * math.cos(a)), surface


def facing(p, target):
    """Rotation Y (degrees) that makes a character (facing local -z) look from p towards target."""
    return math.degrees(math.atan2(-(target[0] - p[0]), -(target[2] - p[2])))


def block_end(entries, type_offset, kind):
    """Index just after the last entry of type `kind` (the lists are grouped by type)."""
    last = max(i for i, e in enumerate(entries) if u32(e, type_offset) == kind)
    return last + 1


def check_grouped(lists):
    for title, offset in (("MODEL_PARAM_ST", 8), ("EVENT_PARAM_ST", 12), ("POINT_PARAM_ST", 8), ("PARTS_PARAM_ST", 12)):
        seq = []
        for e in _list(lists, title).entries:
            if not seq or seq[-1] != u32(e, offset):
                seq.append(u32(e, offset))
        assert len(seq) == len(set(seq)), f"{title} is not grouped by type: {seq}"


def add_model(lists, template_lists):
    """Index of model MODEL, inserted in name order among the character models (copy of the template map's)."""
    models, parts = _list(lists, "MODEL_PARAM_ST").entries, _list(lists, "PARTS_PARAM_ST").entries
    names = [entry_name(e) for e in models]
    if MODEL in names:
        return names.index(MODEL)
    source = next(e for e in _list(template_lists, "MODEL_PARAM_ST").entries if entry_name(e) == MODEL)
    chars = [i for i, e in enumerate(models) if u32(e, 8) == MODEL_CHR]
    at = next((i for i in chars if names[i] > MODEL), chars[-1] + 1)
    entry = bytearray(source)
    struct.pack_into("<i", entry, 0x18, 0)  # instance count, set by the caller
    models.insert(at, bytes(entry))
    for k, i in enumerate(i for i, e in enumerate(models) if u32(e, 8) == MODEL_CHR):
        e = bytearray(models[i])
        struct.pack_into("<i", e, 0x0C, k)
        models[i] = bytes(e)
    for k, e in enumerate(parts):
        if u32(e, 0x14) >= at:
            e = bytearray(e)
            struct.pack_into("<i", e, 0x14, u32(e, 0x14) + 1)
            parts[k] = bytes(e)
    return at


def add_route(lists, points):
    """Type index of a new looping PatrolInfo event through new patrol point regions at `points`."""
    regions, events = _list(lists, "POINT_PARAM_ST").entries, _list(lists, "EVENT_PARAM_ST").entries
    kinds = {u32(e, 0x0C) for e in events}
    assert kinds <= KNOWN_EVENTS, f"events of types {kinds - KNOWN_EVENTS} may hold region indices; not handled"
    template = next(e for e in regions if u32(e, 0x08) == REGION_PATROL)
    at = block_end(regions, 0x08, REGION_PATROL)
    first_type_index = sum(1 for e in regions if u32(e, 0x08) == REGION_PATROL)
    ids = {u32(e, 0x2C) for e in regions}
    assert not ids & set(range(REGION_ID, REGION_ID + len(points))), "region ids already used"

    def shift(index):
        return index + len(points) if index >= at else index

    for k, e in enumerate(events):  # remap region indices held by events
        e = bytearray(e)
        common = q(e, 0x18)
        struct.pack_into("<i", e, common + 4, shift(u32(e, common + 4)) if u32(e, common + 4) >= 0 else -1)
        if u32(e, 0x0C) == EVENT_PATROL:
            td = q(e, 0x20)
            old = struct.unpack_from("<64h", e, td + 16)
            struct.pack_into("<64h", e, td + 16, *[shift(i) if i >= 0 else -1 for i in old])
        events[k] = bytes(e)

    patrols = [e for e in events if u32(e, 0x0C) == EVENT_PATROL]
    route_index = len(patrols)
    for n, p in enumerate(points):
        e = bytearray(template)
        _set_name(e, f"walk_route_{PART}_99_{n + 1:02d}")
        struct.pack_into("<i", e, 0x0C, first_type_index + n)
        struct.pack_into("<3f", e, 0x14, *p)
        struct.pack_into("<3f", e, 0x20, 0.0, 0.0, 0.0)
        struct.pack_into("<i", e, 0x2C, REGION_ID + n)
        regions.insert(at + n, bytes(e))

    e = bytearray(next(x for x in patrols if x[q(x, 0x20)] == 1 and q(x, 0x20) + 16 + 128 <= len(x)))
    _set_name(e, EVENT_NAME)
    struct.pack_into("<iii", e, 0x08, 1 + max(u32(x, 0x08) for x in events), EVENT_PATROL, route_index)
    td = q(e, 0x20)
    e[td] = 0  # patrol type 0, as vanilla walk_route_c4300_9020_5 (loops back to the first point)
    struct.pack_into("<64h", e, td + 16, *([at + n for n in range(len(points))] + [-1] * (64 - len(points))))
    events.insert(block_end(events, 0x0C, EVENT_PATROL), bytes(e))
    return route_index


def referenced_parts(lists):
    """Global part indices held by the map's events (common part index and the type data's part index)."""
    refs = set()
    for e in _list(lists, "EVENT_PARAM_ST").entries:
        refs.add(u32(e, q(e, 0x18)))
        refs |= set(struct.unpack_from("<4i", e, q(e, 0x20)))
    return refs


def add_ghost(raw, template_raw, pathing, tris, params=STRANDED_SOUL, static=False):
    header, lists = read_msb(raw)
    _, template_lists = read_msb(template_raw)
    check_grouped(lists)
    parts = _list(lists, "PARTS_PARAM_ST").entries
    names = [entry_name(e) for e in parts]
    assert PART not in names, f"{PART} is already in the map"
    slot = names.index(REPLACE)
    old = parts[slot]
    assert u32(old, 0x0C) == PART_ENEMY and u32(old, q(old, 0x60)) == 0, f"{REPLACE} must be an enemy with no entity id"
    assert struct.unpack_from("<h", old, q(old, 0x68) + WALK_ROUTE)[0] == -1, f"{REPLACE} already has a route"
    assert slot not in referenced_parts(lists), f"an event refers to {REPLACE}"

    street = parts[find(_list(lists, "PARTS_PARAM_ST"), STREET)]
    placed = [to_world(p, street, tris) for p in path_points(pathing)]
    points = [p for p, _ in placed]

    model = add_model(lists, template_lists)
    route = -1 if static else add_route(lists, points)

    template = _list(template_lists, "PARTS_PARAM_ST")
    e = bytearray(template.entries[find(template, TEMPLATE_PART)])
    assert len(e) == len(old) and q(e, 0x68) == q(old, 0x68), "template enemy layout differs"
    _set_name(e, PART)
    struct.pack_into("<iiii", e, 0x08, int(PART.split("_")[1]), PART_ENEMY, u32(old, 0x10), model)
    struct.pack_into("<3f", e, 0x20, *points[0])
    struct.pack_into("<3f", e, 0x2C, 0.0, facing(points[0], points[1]), 0.0)
    struct.pack_into("<i", e, q(e, 0x60), 0)  # no entity id
    t = q(e, 0x68)
    struct.pack_into("<ii", e, t + THINK, *params)
    struct.pack_into("<h", e, t + WALK_ROUTE, route)
    parts[slot] = bytes(e)

    models = _list(lists, "MODEL_PARAM_ST").entries
    for i in {model, u32(old, 0x14) + (1 if u32(old, 0x14) >= model else 0)}:
        m = bytearray(models[i])
        struct.pack_into("<i", m, 0x18, sum(1 for x in parts if u32(x, 0x14) == i))
        models[i] = bytes(m)
    check_grouped(lists)
    return write_msb(header, lists), placed


def load(path):
    data = Path(path).read_bytes()
    return zlib.decompress(data[0x4C:]) if data[:4] == b"DCX\0" else data


def main():
    flags = [a for a in sys.argv[1:] if a.startswith("--")]
    src, pathing, obj, template, dst = [a for a in sys.argv[1:] if not a.startswith("--")]
    data = Path(src).read_bytes()
    assert data[:4] == b"DCX\0" and data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed map"
    raw = zlib.decompress(data[0x4C:])
    assert write_msb(*read_msb(raw)) == raw, "map does not round-trip"
    out, placed = add_ghost(raw, load(template), json.loads(Path(pathing).read_text()), street_surface(obj),
                            PLAIN if "--plain" in flags else STRANDED_SOUL, "--static" in flags)
    packed = zlib.compress(out, 9)
    head = bytearray(data[:0x4C])
    struct.pack_into(">II", head, 0x1C, len(out), len(packed))
    Path(dst).write_bytes(bytes(head) + packed)
    for p, surface in placed:
        print("route point", tuple(round(c, 2) for c in p), "street surface" if surface is not None else "no surface")


if __name__ == "__main__":
    main()
