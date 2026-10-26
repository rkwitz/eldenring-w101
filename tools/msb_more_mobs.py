"""More mobs on every Unicorn Way patrol route: PER_ROUTE enemies walk each W101 mob path instead of one (Ryan,
2026-10-25: "about 5 per path").

For every mob msb_add_mobs.py placed (an enemy whose entity id is in FIRST_MOBS), PER_ROUTE - 1 copies of it are added:
same model, Think / NpcParam and PatrolInfo route (WalkRouteIndex), standing on the route's patrol points spread
evenly along it (point round(j * n / PER_ROUTE) for copy j, facing the next point), so they spread out over the path
and stand on the street surface the points were snapped to. The copies are named <model>_90NN (NN after the highest
used for that model, so msb_add_mobs --rows still finds them) and get entity ids from FIRST_NEW_ENTITY up, in the order
of FIRST_MOBS, PER_ROUTE - 1 per mob (printed, for w101_quests.MOBS: the quests' kill counters).

Each copy also walks its own PatrolInfo route (2026-10-26, jellyfish clumped on the shared one: every member of a
route heads for its first point): a copy of the original's route event with the same patrol point regions, the list
turned so it starts at the copy's own point (no new regions; new events at the end of the PatrolInfo block, so no
WalkRouteIndex moves). spread_routes does this for copies already placed, too.

Insertion keeps the map valid as msb_add_mobs.insert_enemy does (end of the enemy block, every later part index
remapped). Running it on a map that already has the copies does nothing (copies are found by entity id).

Usage: python msb_more_mobs.py map.msb.dcx out.msb.dcx
"""

import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_ghost import WALK_ROUTE, check_grouped, count_users, q, u32  # noqa: E402
from msb_add_mobs import ENEMY, insert_enemy  # noqa: E402
from msb_floor import _list, _set_name, entry_name, read_msb, write_msb  # noqa: E402

PER_ROUTE = 5
# w101_quests.MOBS order: lost_soul c4180_9000..9003, skeletal_pirate c3500_9000..9003, dark_fairy c3450_9000..9001
FIRST_MOBS = list(range(1042360860, 1042360870))
FIRST_NEW_ENTITY = 1042360870  # ..909 (40 copies); msb_add_boss uses 850-853, the grace helpers 950-951
PATROL_EVENT = 20


def route_points(lists, route):
    """Positions of the patrol point regions of PatrolInfo event number `route` (type index)."""
    events = [e for e in _list(lists, "EVENT_PARAM_ST").entries if u32(e, 0x0C) == PATROL_EVENT]
    regions = _list(lists, "POINT_PARAM_ST").entries
    td = q(events[route], 0x20)
    return [struct.unpack_from("<3f", regions[i], 0x14) for i in struct.unpack_from("<64h", events[route], td + 16)
            if i >= 0]


def facing(a, b):
    return math.degrees(math.atan2(b[0] - a[0], b[2] - a[2]))


def more_mobs(raw, per_route=PER_ROUTE):
    header, lists = read_msb(raw)
    check_grouped(lists)
    parts = _list(lists, "PARTS_PARAM_ST").entries

    def entity(e):
        return u32(e, q(e, 0x60))

    used = {entity(e) for e in parts}
    names = {entry_name(e) for e in parts}
    report = []
    next_entity = FIRST_NEW_ENTITY
    for mob_entity in FIRST_MOBS:
        mob = next(e for e in parts if u32(e, 0x0C) == ENEMY and entity(e) == mob_entity)
        model = entry_name(mob)[:5]
        route = struct.unpack_from("<h", mob, q(mob, 0x68) + WALK_ROUTE)[0]
        points = route_points(lists, route)
        ids = []
        for j in range(1, per_route):
            new_entity, next_entity = next_entity, next_entity + 1
            ids.append(new_entity)
            if new_entity in used:
                continue  # placed by an earlier run
            number = next(n for n in range(9000, 10000) if f"{model}_{n:04d}" not in names)
            name = f"{model}_{number:04d}"
            names.add(name)
            k = round(j * len(points) / per_route) % len(points)
            here, ahead = points[k], points[(k + 1) % len(points)]
            e = bytearray(mob)
            _set_name(e, name)
            type_index = sum(1 for x in parts if u32(x, 0x0C) == ENEMY)
            struct.pack_into("<ii", e, 0x08, number, ENEMY)
            struct.pack_into("<i", e, 0x10, type_index)
            struct.pack_into("<3f", e, 0x20, *here)
            struct.pack_into("<3f", e, 0x2C, 0.0, facing(here, ahead), 0.0)
            struct.pack_into("<I", e, q(e, 0x60), new_entity)
            insert_enemy(lists, bytes(e))
            parts = _list(lists, "PARTS_PARAM_ST").entries
            report.append(f"{name}: entity {new_entity}, copy of {entry_name(mob)} ({mob_entity}) on route {route}, "
                          f"point {k} of {len(points)} at {tuple(round(c, 1) for c in here)}")
        report.append(f"{entry_name(mob)} ({mob_entity}) route {route}: + {ids}")
    report += spread_routes(lists)
    count_users(lists)
    check_grouped(lists)
    _check_type_indices(_list(lists, "PARTS_PARAM_ST").entries)
    return write_msb(header, lists), report


def spread_routes(lists):
    """Give every copy (entity FIRST_NEW_ENTITY..) still on its original's route a route of its own, starting at the
    patrol point it stands on. Returns report lines."""
    parts = _list(lists, "PARTS_PARAM_ST").entries
    events = _list(lists, "EVENT_PARAM_ST").entries
    copies = range(FIRST_NEW_ENTITY, FIRST_NEW_ENTITY + len(FIRST_MOBS) * (PER_ROUTE - 1))
    originals = {}
    for e in parts:
        if u32(e, 0x0C) == ENEMY and u32(e, q(e, 0x60)) in FIRST_MOBS:
            originals[u32(e, q(e, 0x60))] = struct.unpack_from("<h", e, q(e, 0x68) + WALK_ROUTE)[0]
    shared = set(originals.values())
    patrols = [k for k, e in enumerate(events) if u32(e, 0x0C) == PATROL_EVENT]
    template = max((events[k] for k in patrols), key=lambda e: len(entry_name(e)))
    report = []
    for k, e in enumerate(parts):
        if u32(e, 0x0C) != ENEMY or u32(e, q(e, 0x60)) not in copies:
            continue
        td = q(e, 0x68)
        route = struct.unpack_from("<h", e, td + WALK_ROUTE)[0]
        if route not in shared:
            continue  # has its own already
        source = events[patrols[route]]
        regions = [i for i in struct.unpack_from("<64h", source, q(source, 0x20) + 16) if i >= 0]
        here = struct.unpack_from("<3f", e, 0x20)
        points = _list(lists, "POINT_PARAM_ST").entries
        start = min(range(len(regions)), key=lambda n: math.dist(here, struct.unpack_from("<3f", points[regions[n]], 0x14)))
        turned = regions[start:] + regions[:start]
        new = bytearray(template)
        _set_name(new, f"walk_{entry_name(e)}")
        new_index = sum(1 for x in events if u32(x, 0x0C) == PATROL_EVENT)
        struct.pack_into("<iii", new, 0x08, 1 + max(u32(x, 0x08) for x in events), PATROL_EVENT, new_index)
        ntd = q(new, 0x20)
        new[ntd] = source[q(source, 0x20)]  # patrol type of the original
        struct.pack_into("<64h", new, ntd + 16, *(turned + [-1] * (64 - len(turned))))
        at = 1 + max(i for i, x in enumerate(events) if u32(x, 0x0C) == PATROL_EVENT)
        events.insert(at, bytes(new))
        patrols = [i for i, x in enumerate(events) if u32(x, 0x0C) == PATROL_EVENT]
        e = bytearray(e)
        struct.pack_into("<h", e, td + WALK_ROUTE, new_index)
        parts[k] = bytes(e)
        report.append(f"{entry_name(e)}: own route {new_index} (route {route} from point {start})")
    return report


def _check_type_indices(parts):
    seen = {}
    for e in parts:
        kind = u32(e, 0x0C)
        assert u32(e, 0x10) == seen.get(kind, 0), f"{entry_name(e)}: type index {u32(e, 0x10)}"
        seen[kind] = seen.get(kind, 0) + 1


def main():
    from msb_add_garden import run

    def place(raw):
        out, report = more_mobs(raw)
        print("\n".join(report))
        return out
    run(place)


if __name__ == "__main__":
    main()
