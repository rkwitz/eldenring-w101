"""Audit (and clean) every Elden Ring part that Unicorn Way can meet: the street, the rooms under it and the garden.

Ryan, 2026-10-25: runes as soon as the map loads, "a random ER statue on Unicorn Way, more skulls, a random armor stand on
the road". Two causes are looked for in every map given (the project's m60 tiles of all three sizes, and vanilla tiles
next to them):

1. Vanilla enemies with nothing under them. footprint_clear.py sank every collision part (h......) of the 256 m tiles
   the street touches, so the terrain those tiles' enemies stand on is gone: they fall and die when the tile loads, and
   the player gets their runes. Every Enemy part (type 2) of a 256 m tile whose collision parts are all sunk is
   deleted, except ours (OUR_ENTITIES) and c1000, the invisible helpers of graces and vanilla tile events (no runes,
   no model; The First Step's grace has one on the street). Enemies standing in a zone (below) are deleted too,
   wherever their tile is.
2. Leftover set dressing in a zone: assets, dummy assets and dummy enemies (corpses, skulls, statues, armour stands,
   item-glow markers, treasure corpses) above the sink depth whose XZ position is over the street footprint
   (footprint_clear.Footprint of the street OBJ, MARGIN 6 m), or within GARDEN_RADIUS of the garden part and less than
   GARDEN_BELOW under its floor, or within ROOM_RADIUS of a room part. They are moved SINK m down, as footprint_clear
   does (no index changes). Our own parts (footprint_clear.KEEP) stay.

Deleting parts keeps the map valid the way msb_add_mobs.py inserts them: every part-index field is remapped
(msb_add_mobs.remap_parts; a field pointing at a deleted part becomes -1) and the enemy type indices are renumbered.
Treasure, retry point and mount events whose parts are all deleted are removed with them (no event index is held
elsewhere: enemies' WalkRouteIndex counts PatrolInfo events only). PlatoonInfo groups and GroupDefeatReward regions
(flask refills) just lose the deleted members.

Usage: python msb_audit.py home.msb.dcx street.obj out_dir map.msb.dcx... [--apply]
  home: m60_42_36_00 (street, garden and room transforms). Prints one line per finding; with --apply writes every
  changed map to out_dir as DFLT DCX (KRAK inputs are read through dcx_krak.py).
"""

import math
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import dcx_krak  # noqa: E402
from footprint_clear import KEEP, SINK, Footprint, parse_obj, street_transform, to_home  # noqa: E402
from msb_add_mobs import ENEMY, remap_parts  # noqa: E402
from msb_add_ghost import check_grouped, count_users, q, u32  # noqa: E402
from msb_floor import _list, entry_name, read_msb, write_msb  # noqa: E402

COLLISION, ASSET, DUMMY_ASSET, DUMMY_ENEMY = 5, 13, 9, 10
TREASURE, MOUNT, RETRY_POINT = 4, 21, 24  # event types
GARDEN, ROOMS = "AEG030_555_9000", ("AEG030_519_9000", "AEG030_519_9001", "AEG030_557_9000")
GARDEN_RADIUS, GARDEN_BELOW = 70.0, 20.0  # the maze is ~80 m across; vanilla ground under it is 70 m lower
ROOM_RADIUS = 35.0
OUR_ENTITIES = range(1042360850, 1042360950)  # msb_add_boss (850-853) and msb_add_mobs (860 and up)
HELPERS = ("c1000",)  # invisible event characters: no model, no runes
KIND = {2: "enemy", 9: "dummy asset", 10: "dummy enemy", 13: "asset"}


def load(path):
    return dcx_krak.decompress(Path(path).read_bytes())


def zones(home_raw, obj):
    """Callable (map name, x, y, z) -> zone name or None."""
    position, rotation = street_transform(home_raw)
    street = Footprint(*parse_obj(obj), position, rotation)
    _, lists = read_msb(home_raw)
    parts = {entry_name(e): struct.unpack_from("<3f", e, 0x20) for e in _list(lists, "PARTS_PARAM_ST").entries}
    garden = parts[GARDEN]
    rooms = [parts[r] for r in ROOMS if r in parts]

    def zone(name, x, y, z):
        hx, hz = to_home(name, x, z)
        if street.covers(hx, hz):
            return "street"
        if math.hypot(hx - garden[0], hz - garden[2]) < GARDEN_RADIUS and y > garden[1] - GARDEN_BELOW:
            return "garden"
        if any(math.hypot(hx - r[0], hz - r[2]) < ROOM_RADIUS for r in rooms):
            return "room"
        return None
    return zone


def ours(e):
    name = entry_name(e)
    if any(k in name for k in KEEP):
        return True
    return u32(e, 0x0C) == ENEMY and u32(e, q(e, 0x60)) in OUR_ENTITIES


def audit(name, raw, zone, apply=False):
    """(new map bytes or None, [finding lines]) for map `name`."""
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST").entries
    models = [entry_name(m) for m in _list(lists, "MODEL_PARAM_ST").entries]
    collisions = [e for e in parts if u32(e, 0x0C) == COLLISION]
    ground_gone = name.endswith("_00") and bool(collisions) and all(
        struct.unpack_from("<f", e, 0x24)[0] < -SINK / 2 for e in collisions)
    lines, delete, sink = [], set(), []
    for k, e in enumerate(parts):
        kind = u32(e, 0x0C)
        x, y, z = struct.unpack_from("<3f", e, 0x20)
        if kind not in KIND or y < -SINK / 2 or ours(e):
            continue
        model = models[u32(e, 0x14)]
        where = zone(name, x, y, z)
        label = f"{name}: {KIND[kind]} {entry_name(e)} ({model}) at ({x:.1f}, {y:.1f}, {z:.1f})"
        if kind == ENEMY:
            if model in HELPERS:
                continue
            if ground_gone or where:
                td = q(e, 0x68)
                rows = struct.unpack_from("<ii", e, td + 0x08)
                why = f"in the {where}" if where else "falls: its tile's collision is sunk"
                lines.append(f"{label}, entity {u32(e, q(e, 0x60))}, Think/NpcParam {rows[0]}/{rows[1]}: deleted ({why})")
                delete.add(k)
        elif where:
            lines.append(f"{label}: sunk {SINK:.0f} m (over the {where})")
            sink.append(k)
    if not apply or not (delete or sink):
        return None, lines
    for k in sink:
        e = bytearray(parts[k])
        x, y, z = struct.unpack_from("<3f", e, 0x20)
        struct.pack_into("<3f", e, 0x20, x, y - SINK, z)
        parts[k] = bytes(e)
    if delete:
        events = _list(lists, "EVENT_PARAM_ST").entries
        drop = [i for i, ev in enumerate(events) if _orphaned(ev, delete)]
        for i in reversed(drop):
            lines.append(f"{name}: event {entry_name(events[i])} (type {u32(events[i], 0x0C)}) removed with its parts")
            del events[i]
        _renumber(events, 0x0C)
        gone = sorted(delete)
        remap_parts(lists, lambda i: -1 if i in delete else i - sum(1 for g in gone if g < i))
        for k in reversed(gone):
            del parts[k]
        _renumber(parts, 0x0C)
        count_users(lists)
    check_grouped(lists)
    return write_msb(header, lists), lines


def _orphaned(ev, delete):
    """True for an event whose whole point is a deleted part: a treasure on it, a retry point at it, a mount pair."""
    kind, td = u32(ev, 0x0C), q(ev, 0x20)
    fields = {TREASURE: [8], RETRY_POINT: [0], MOUNT: [0, 4]}.get(kind)
    return bool(fields) and all(u32(ev, td + o) in delete for o in fields)


def _renumber(entries, type_at):
    """Per-type indices (the int after the type) counted again in list order."""
    seen = {}
    for k, e in enumerate(entries):
        kind = u32(e, type_at)
        if u32(e, type_at + 4) != seen.get(kind, 0):
            e = bytearray(e)
            struct.pack_into("<i", e, type_at + 4, seen.get(kind, 0))
            entries[k] = bytes(e)
        seen[kind] = seen.get(kind, 0) + 1


def dflt(raw, template):
    """DFLT DCX of `raw` with the header of DFLT file `template`."""
    packed = zlib.compress(raw, 9)
    head = bytearray(template[:0x4C])
    struct.pack_into(">II", head, 0x1C, len(raw), len(packed))
    return bytes(head) + packed


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    home, obj, out, *maps = args
    apply = "--apply" in sys.argv
    template = Path(home).read_bytes()
    assert template[0x28:0x2C] == b"DFLT", "home map must be DFLT"
    zone = zones(load(home), obj)
    for path in maps:
        name = Path(path).name.split(".")[0]
        raw = load(path)
        assert write_msb(*read_msb(raw)) == raw, f"{name} does not round-trip"
        new, lines = audit(name, raw, zone, apply)
        print("\n".join(lines) or f"{name}: nothing found")
        if new is not None:
            Path(out).mkdir(parents=True, exist_ok=True)
            (Path(out) / f"{name}.msb.dcx").write_bytes(dflt(new, template))


if __name__ == "__main__":
    main()
