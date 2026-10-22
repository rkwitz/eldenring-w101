"""Put Elden Ring stand-ins for Unicorn Way's mobs in m60_42_36_00 (The First Step): one enemy per Wizard101 mob spawn
group, standing on the first node of that group's path and patrolling the whole path as a loop.

Ryan's picks (2026-10-22, "subject to change later"); Rattlebones (Giant Skeleton Torso) goes in his boss room later:
  Lost Soul (Ghost-Blue, 4 groups)      -> Spirit Jellyfish   c4180  Think 41800000 / NpcParam 41800010  (unverified)
  Skeletal Pirate (4 groups)            -> Skeleton, curved swords c3500  35004000 / 35004010        (unverified;
                                           same chr as the vanilla Limgrave skeletons 35000000/35000010 in m60_42_35)
  Dark Sprite (2 groups)                -> Winged Misbegotten c3450  34501000 / 34501012             (unverified)
"Unverified" = rows picked by Smithbox row names, not seen on a vanilla enemy and not read from regulation.bin.

Positions: W101 path nodes through the street part's current transform, snapped to the street surface of the OBJ
(see msb_add_ghost.to_world). Each enemy gets its own PatrolInfo event (patrol type 0, as vanilla c4300_9020's) with
one patrol point region per node.

Keeping the map valid (see msb_add_ghost.py for why the first ghost crashed):
  - every list stays grouped by type; new models go in name order among the chr models, new patrol points at the end
    of the patrol-point block, new PatrolInfo events at the end of their block, new enemies at the end of the enemy
    block (per-type index = enemy count);
  - inserting enemies shifts the global index of every later part, so every field that holds a part index is
    remapped. The fields come from Smithbox's MSBE meta (MapRef="Part-...") and were checked empirically by diffing
    vanilla m60_42_36_00 against Ryan's Smithbox-saved copy (which renumbered them all): event common PartIndex,
    Treasure td+8, ObjAct td+4, SignPool td+0, Enemy/DummyEnemy CollisionPartIndex td+0x1C, Asset/DummyAsset
    PartIndices td+0x30..0x4C and UnkT54PartIndex td+0x54, Region ActivationPartIndex (entity data +0) and
    GroupDefeatReward (region type 37) PartIndices td+0x14..0x24. Anything else the map holds is refused.
  - An existing ghost from msb_add_ghost.py (<model>_9000 on "Path Ghost 01") is reused for the Ghosts 01 group in
    place, keeping its route.

Usage: python msb_add_mobs.py map.msb.dcx pathing.json street.obj template_map.msb out.msb.dcx [--one-each]
  template_map.msb: vanilla m60_43_36_00.msb (enemy part and c4300 model templates)
  --one-each: only the first group of each mob (fallback file)
"""

import json
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_ghost import (ENEMIES, PART_ENEMY, SUFFIX, STREET, TEMPLATE_PART, THINK, WALK_ROUTE, add_model,  # noqa: E402
                           add_route, check_grouped, count_users, facing, find, ghost_slot, load, q, retarget,
                           street_surface, to_world, u32)
from msb_floor import _list, _set_name, entry_name, read_msb, write_msb  # noqa: E402

MOBS = {  # W101 mob template -> (model, Think, NpcParam)
    "Ghost-Blue-L01": ("c4180", 41800000, 41800010),
    "Skeleton-Pirate-L01": ("c3500", 35004000, 35004010),
    "Sprite-Dark-L01": ("c3450", 34501000, 34501012),
}
GHOST_PATH = "Path Ghost 01"  # the route msb_add_ghost.py gave the ghost
FIRST_REGION_ID = 9100

ASSET, DUMMY_ASSET, ENEMY, DUMMY_ENEMY = 13, 9, 2, 10
KNOWN_PART_TYPES = {0, 2, 4, 5, 9, 10, 13}  # map piece, enemy, player, collision, dummy asset/enemy, asset
KNOWN_EVENTS = {4: [8], 7: [4], 20: [], 23: [0]}  # event type -> part index offsets in its type data
GROUP_DEFEAT_REWARD = 37


def remap_parts(lists, shift):
    """Apply `shift` (old part index -> new) to every part index field of the map."""
    def fix(e, offsets):
        for o in offsets:
            v = u32(e, o)
            if v >= 0:
                struct.pack_into("<i", e, o, shift(v))

    for plist, fields in (("PARTS_PARAM_ST", _part_fields), ("EVENT_PARAM_ST", _event_fields),
                          ("POINT_PARAM_ST", _region_fields)):
        entries = _list(lists, plist).entries
        for k, e in enumerate(entries):
            e = bytearray(e)
            fix(e, fields(e))
            entries[k] = bytes(e)


def _part_fields(e):
    kind, td = u32(e, 0x0C), q(e, 0x68)
    assert kind in KNOWN_PART_TYPES, f"part type {kind} may hold part indices; not handled"
    if kind in (ENEMY, DUMMY_ENEMY):
        return [td + 0x1C]
    if kind in (ASSET, DUMMY_ASSET):
        return [td + o for o in range(0x30, 0x50, 4)] + [td + 0x54]
    return []


def _event_fields(e):
    kind, td = u32(e, 0x0C), q(e, 0x20)
    assert kind in KNOWN_EVENTS, f"event type {kind} may hold part indices; not handled"
    return [q(e, 0x18)] + [td + o for o in KNOWN_EVENTS[kind]]


def _region_fields(e):
    fields = [q(e, 0x50)]  # entity data: ActivationPartIndex first
    if u32(e, 0x08) == GROUP_DEFEAT_REWARD:
        fields += [q(e, 0x58) + o for o in range(0x14, 0x28, 4)]
    return fields


def insert_enemy(lists, entry):
    """Append enemy `entry` at the end of the enemy block, remapping all later part indices. Returns its index."""
    parts = _list(lists, "PARTS_PARAM_ST").entries
    at = 1 + max(i for i, e in enumerate(parts) if u32(e, 0x0C) == ENEMY)
    remap_parts(lists, lambda i: i + 1 if i >= at else i)
    parts.insert(at, entry)
    return at


def mob_groups(pathing):
    """[(template name, spawn group name, path name)] of the mob spawn groups, in file order."""
    paths = {p["id"]: p["name"] for p in pathing["paths"]}
    templates = {int(k): Path(v["file"]).stem for k, v in pathing["templates"].items()}
    out = []
    for group in pathing["spawn_groups"]:
        entry = group["entries"][0]
        if templates.get(entry["template"]) in MOBS:
            out.append((templates[entry["template"]], group["name"], paths[entry["path_id"]]))
    return out


def path_nodes(pathing, name):
    path = next(p for p in pathing["paths"] if p["name"] == name)
    return [pathing["nodes"][str(n)]["pos"] for n in path["nodes"]]


def add_mobs(raw, template_raw, pathing, tris, one_each=False):
    header, lists = read_msb(raw)
    _, template_lists = read_msb(template_raw)
    check_grouped(lists)
    parts = _list(lists, "PARTS_PARAM_ST")
    street = parts.entries[find(parts, STREET)]
    template = _list(template_lists, "PARTS_PARAM_ST")
    template_enemy = template.entries[find(template, TEMPLATE_PART)]
    names = {entry_name(e) for e in parts.entries}
    region_ids = {u32(e, 0x2C) for e in _list(lists, "POINT_PARAM_ST").entries}
    next_region = max([FIRST_REGION_ID - 1] + [i for i in region_ids if i >= FIRST_REGION_ID]) + 1
    seen, report = set(), []
    groups = sorted(mob_groups(pathing), key=lambda g: g[2] != GHOST_PATH)  # the ghost's group first
    slot = ghost_slot(lists)
    if slot is not None:  # reuse the ghost and its route for the group walking its path
        mob = next(m for m, _, path in groups if path == GHOST_PATH)
        enemy = next(k for k, v in ENEMIES.items() if v == MOBS[mob])
        retarget(lists, slot, template_lists, enemy)
        names.add(MOBS[mob][0] + SUFFIX)

    for mob, group, path in groups:
        if one_each and mob in seen:
            continue
        seen.add(mob)
        model_name, think, npc = MOBS[mob]
        if slot is not None and path == GHOST_PATH:
            report.append((group, path, model_name + SUFFIX, "reused the ghost and its route"))
            continue
        number = next(n for n in range(int(SUFFIX[1:]), 10000) if f"{model_name}_{n:04d}" not in names)
        part = f"{model_name}_{number:04d}"
        names.add(part)
        placed = [to_world(p, street, tris) for p in path_nodes(pathing, path)]
        points = [p for p, _ in placed]
        assert all(s is not None for _, s in placed), f"{path}: a node is off the street mesh"
        model = add_model(lists, template_lists, model_name)
        route = add_route(lists, points, part, f"walk_{part}", next_region)
        next_region += len(points)
        e = bytearray(template_enemy)
        _set_name(e, part)
        type_index = sum(1 for x in parts.entries if u32(x, 0x0C) == ENEMY)
        struct.pack_into("<iiii", e, 0x08, number, ENEMY, type_index, model)
        struct.pack_into("<3f", e, 0x20, *points[0])
        struct.pack_into("<3f", e, 0x2C, 0.0, facing(points[0], points[1]), 0.0)
        struct.pack_into("<i", e, q(e, 0x60), 0)  # no entity id
        td = q(e, 0x68)
        struct.pack_into("<ii", e, td + THINK, think, npc)
        struct.pack_into("<h", e, td + WALK_ROUTE, route)
        insert_enemy(lists, bytes(e))
        report.append((group, path, part, f"{len(points)} points, first at {tuple(round(c, 1) for c in points[0])}"))

    count_users(lists)
    check_grouped(lists)
    return write_msb(header, lists), report


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    src, pathing, obj, template, dst = args
    data = Path(src).read_bytes()
    assert data[:4] == b"DCX\0" and data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed map"
    raw = zlib.decompress(data[0x4C:])
    assert write_msb(*read_msb(raw)) == raw, "map does not round-trip"
    out, report = add_mobs(raw, load(template), json.loads(Path(pathing).read_text()), street_surface(obj),
                           "--one-each" in sys.argv)
    packed = zlib.compress(out, 9)
    head = bytearray(data[:0x4C])
    struct.pack_into(">II", head, 0x1C, len(out), len(packed))
    Path(dst).write_bytes(bytes(head) + packed)
    for line in report:
        print(*line, sep=" | ")


if __name__ == "__main__":
    main()
