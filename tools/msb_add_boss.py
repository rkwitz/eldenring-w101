"""Put Rattlebones in his tower room (asset aeg030_519, placed by msb_add_room.py) in m60_42_36_00: Ryan's pick
Giant Skeleton Torso (chr c4960, Think 49600000 / NpcParam 49600000, rows chosen by Smithbox row names, not read from
regulation.bin) standing on the room's duel sigil and facing the door, plus the region the boss bar event
(emevd.py --rattlebones-boss) watches.

  c4960_9000              enemy, entity BOSS_ENTITY, at W101 (0, -45) = the room's 8-player sigil (room FLVER
                          (0, 0, -0.45)), facing the arrival point W101_Rattlebones_in; no patrol route (the room has
                          no navmesh: it turns and attacks but won't walk)
  W101_Rattlebones_arena  sphere r ARENA_RADIUS over the whole room; inside it the "Rattlebones" health bar shows
  W101_Rattlebones_sigil  small sphere on the sigil, facing the door: the boss event (emevd.boss_bar) holds him with
                          gravity off until the player walks in, then warps him onto it and turns gravity back on

In the first test (2026-10-23) the bar showed but Rattlebones was nowhere: he spawns with the map while the player is
on the street, likely before the room's asset collision 60 m down is there for him, and fell through the floor (still
alive, so the bar shows). The sigil point and the event fix that.

The enemy is a copy of vanilla patroller c4300_9020 from m60_43_36_00 (as msb_add_mobs.py), inserted at the end of the
enemy block with every later part index remapped; the c4960 model is added in name order among the character models.
Running it again moves the existing entries instead.

Usage: python msb_add_boss.py map.msb.dcx template_map.msb out.msb.dcx
  template_map.msb: vanilla m60_43_36_00.msb (uncompressed, or DFLT DCX)
"""

import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import entity_ids, put_regions  # noqa: E402
from msb_add_ghost import (TEMPLATE_PART, THINK, WALK_ROUTE, add_model, check_grouped, count_users, load,  # noqa: E402
                           q, u32)
from msb_add_mobs import ENEMY, insert_enemy  # noqa: E402
from msb_add_room import ROOM, to_world  # noqa: E402
from msb_floor import _list, _set_name, entry_name, find, read_msb, transform, write_msb  # noqa: E402

BOSS, MODEL, THINK_ID, NPC_ID = "c4960_9000", "c4960", 49600000, 49600000
BOSS_ENTITY = 1042360850
SIGIL = (0.0, 0.0, -0.45)  # room FLVER frame: W101 (0, -45), floor height
FACING = 180.0  # room frame: towards -z, the door and W101_Rattlebones_in
ARENA_RADIUS = 13.0
SIGIL_ENTITY = 1042362809
REGIONS = [("W101_Rattlebones_arena", "room", (0.0, 4.0, 0.0), 0.0, ARENA_RADIUS, 1042362808, 9208),
           ("W101_Rattlebones_sigil", "room", (SIGIL[0], 0.1, SIGIL[2]), FACING, 0.5, SIGIL_ENTITY, 9209)]


def place_boss(raw, template_raw):
    header, lists = read_msb(raw)
    _, template_lists = read_msb(template_raw)
    check_grouped(lists)
    parts = _list(lists, "PARTS_PARAM_ST")
    room_position, room_rotation = transform(parts.entries[find(parts, ROOM)])
    ry = room_rotation[1]
    position = to_world(room_position, ry, SIGIL)
    rotation = (ry + FACING + 180.0) % 360.0 - 180.0

    names = [entry_name(e) for e in parts.entries]
    if BOSS in names:
        k = names.index(BOSS)
        e = bytearray(parts.entries[k])
    else:
        assert BOSS_ENTITY not in entity_ids(lists), f"entity id {BOSS_ENTITY} is taken"
        model = add_model(lists, template_lists, MODEL)
        template = _list(template_lists, "PARTS_PARAM_ST")
        e = bytearray(template.entries[find(template, TEMPLATE_PART)])
        _set_name(e, BOSS)
        type_index = sum(1 for x in parts.entries if u32(x, 0x0C) == ENEMY)
        struct.pack_into("<iiii", e, 0x08, int(BOSS[-4:]), ENEMY, type_index, model)
        struct.pack_into("<I", e, q(e, 0x60), BOSS_ENTITY)
        td = q(e, 0x68)
        struct.pack_into("<ii", e, td + THINK, THINK_ID, NPC_ID)
        struct.pack_into("<h", e, td + WALK_ROUTE, -1)
        k = None
    struct.pack_into("<3f", e, 0x20, *position)
    struct.pack_into("<3f", e, 0x2C, 0.0, rotation, 0.0)
    if k is None:
        insert_enemy(lists, bytes(e))
    else:
        parts.entries[k] = bytes(e)
    count_users(lists)
    print(f"{BOSS}: entity {BOSS_ENTITY}, Think {THINK_ID}, NpcParam {NPC_ID}, at {tuple(round(c, 3) for c in position)}, "
          f"rotation y {rotation:.1f}")
    put_regions(lists, REGIONS, {"room": (room_position, ry)})
    check_grouped(lists)
    return write_msb(header, lists)


def main():
    src, template, dst = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
    data = src.read_bytes()
    assert data[:4] == b"DCX\0" and data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed map"
    raw = zlib.decompress(data[0x4C:])
    assert write_msb(*read_msb(raw)) == raw, "map does not round-trip"
    out = place_boss(raw, load(template))
    packed = zlib.compress(out, 9)
    head = bytearray(data[:0x4C])
    struct.pack_into(">II", head, 0x1C, len(out), len(packed))
    dst.write_bytes(bytes(head) + packed)


if __name__ == "__main__":
    main()
