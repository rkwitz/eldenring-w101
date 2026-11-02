"""Put Unicorn Way's bosses in their rooms in m60_42_36_00 (Ryan's picks; every boss bar shows the Wizard101 name,
see emevd.boss_bar and er_text.py --npc-names):

  W101 boss       room (asset part, placed by)                 Elden Ring stand-in        Think / NpcParam
  Rattlebones     tower T2 (AEG030_519_9000, msb_add_room)     Giant Skeleton Torso c4960 49600068 / 49600168
  Lady Blackhope  tower T1 (AEG030_519_9001, msb_add_interiors) Nox Swordstress c3300      33000000 / 33000062
  Fear of Fire    house H4 (AEG030_557_9000, msb_add_interiors) Mad Pumpkin Head c4340     43400900 / 43400910
  Seraph          hedge maze (AEG030_555_9000, msb_add_garden)  Malenia c2120              21200000 / 21200056

The rows are the ones vanilla maps place: Nox Swordstress as m12_01_00_00 places c3300_9001/9002 and 9033/9034 (entity
0 or plain 90005250 events: shown with no event at all; round 9, 2026-10-27: with the Sellia boss-duo rows 33000900 /
33000940 of m60_49_39_00 her bar showed but she was nowhere in the room, as Rattlebones with rows no map places), Mad Pumpkin Head boss row from the enemy shortlist (Smithbox names "Mad Pumpkin Head (Boss)"), Malenia's
phase-1 character c2120_9000 of m15_00_00_00 (her phase 2 is the same character, switched by her arena event when
her HP hits 1 while immortal; without that event she simply dies, and emevd.phase_one_only kills her at 1 HP in case
the row keeps her alive), and the Giant Skeleton Torsos of Mohgwyn Palace (m12_05_00_00). Rattlebones first used
49600000 / 49600000, rows no vanilla map places; in game he stayed invisible even after the boss event put him on
the sigil (2026-10-23), so he now uses the plain Mohgwyn rows (the Mountaintops ones get extra SpEffect sets and
visibility events). His model (14 x 8 x 16 m bounding box, FLVER header) fits the 23 m wide, 12 m high room.

Each boss is a copy of vanilla patroller c4300_9020 from m60_43_36_00 (as msb_add_mobs.py) with the model, rows and
entity id changed and no patrol route (the rooms have no navmesh), inserted at the end of the enemy block with every
later part index remapped; models go in name order among the character models. Two regions per boss:
  W101_<boss>_arena  sphere over the room; inside it the boss bar shows (emevd.boss_bar)
  W101_<boss>_sigil  point on the duel sigil, facing the arrival: the boss event warps the boss there when the player
                     walks in (with gravity off until then: spawning with the map, before the room's asset collision
                     is there for him, he can fall through the floor)
  W101_<boss>_room   (Rattlebones) the room above its floor, for emevd.boss_watchdog (see WATCHED)
Positions are in the room asset's FLVER frame (W101 (x, z, y) * 0.01) through the part's transform. Rerunning moves
the existing entries and updates their rows.

Usage: python msb_add_boss.py map.msb.dcx template_map.msb out.msb.dcx [--only Rattlebones]
  template_map.msb: vanilla m60_43_36_00.msb (uncompressed, or DFLT DCX)
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import entity_ids, put_regions, run  # noqa: E402
from msb_add_ghost import (TEMPLATE_PART, THINK, WALK_ROUTE, add_model, check_grouped, count_users, load,  # noqa: E402
                           q, u32)
from msb_add_mobs import ENEMY, insert_enemy  # noqa: E402
from msb_add_room import to_world  # noqa: E402
from msb_floor import _list, _set_name, entry_name, find, read_msb, transform, write_msb  # noqa: E402

RATTLEBONES_BACK = 3.05  # sigil 3.5 m behind the room centre (W101 sigil (0, -45) = FLVER z -0.45; the door is at -z)
# (key, part, Think, NpcParam, entity, room part, sigil (room FLVER frame), facing (0 = room +z), arena centre,
#  arena radius, arena entity / region id, sigil entity / region id; region ids 9220-9225 are w101_quests')
BOSSES = [
    # round 25 B3: on the far side of the duel circle (as Wizard101 puts the boss, ref 8_w101_boss_room.png), facing
    # the door, instead of the centre: the torso is 16 m long (FLVER box +-8 m round its origin) and only hit the
    # player near the walls from the centre of a 22 m room
    ("Rattlebones", "c4960_9000", 49600068, 49600168, 1042360850, "AEG030_519_9000", (0.0, 0.0, RATTLEBONES_BACK), 180.0,
     (0.0, 4.0, 0.0), 13.0, (1042362808, 9208), (1042362809, 9209)),
    ("Blackhope", "c3300_9000", 33000000, 33000062, 1042360851, "AEG030_519_9001", (0.0, 0.0, -0.45), 180.0,
     (0.0, 4.0, 0.0), 13.0, (1042362824, 9218), (1042362825, 9219)),
    # W101 sigil (27.8, -397.2) of the clubhouse; the arrival is at +z
    ("FearOfFire", "c4340_9000", 43400900, 43400910, 1042360852, "AEG030_557_9000", (0.278, 0.0, -3.972), 0.0,
     (0.0, 6.0, -25.0), 45.0, (1042362826, 9242), (1042362827, 9243)),
    # the alcove at the far (north) end of the maze's central avenue (maze floor 0.3, a 12 x 5 m bay off the 2 m
    # cross corridor at z 31-32), 32 m from Lady Oriel's statue in the sunken centre (w101_quests: (5.97, 1.68, 0.94),
    # goal sphere 14 m), facing down the avenue (-z) towards the centre the player comes from; the arena sphere
    # (14 m, reaching z 16) stays clear of Oriel's sphere (to z 15)
    ("Seraph", "c2120_9000", 21200000, 21200056, 1042360853, "AEG030_555_9000", (6.0, 0.3, 33.0), 180.0,
     (6.0, 2.0, 30.0), 14.0, (1042362828, 9244), (1042362829, 9245)),
]
SIGIL_LIFT = 0.1  # sigil region height above the floor
# Round 25 B2 (Rattlebones fell through the floor and never came back): one more sphere per watched boss, so big that
# it holds the whole room above the floor (radius 12.15 m at floor level, past the 11.2 m ring wall) but not 1.5 m
# under it: centre WATCH_RADIUS - WATCH_DEPTH above the floor. emevd.boss_watchdog puts the boss back on his sigil
# when he is alive and outside it (under the floor, or out through a wall). (key, room part, entity, region id)
WATCH_RADIUS, WATCH_DEPTH = 50.0, 1.5
WATCHED = [("Rattlebones", "AEG030_519_9000", 1042362856, 9259)]


def place_bosses(raw, template_raw, only=None):
    """`only`: keys of BOSSES to place (and of WATCHED to add), e.g. {"Rattlebones"}; the others are left as they are
    (round 23 took the Seraph out of the map: placing all would put her back)."""
    header, lists = read_msb(raw)
    _, template_lists = read_msb(template_raw)
    check_grouped(lists)
    parts = _list(lists, "PARTS_PARAM_ST")
    regions, frames = [], {}
    for key, name, think, npc, entity, room, sigil, facing, arena, radius, arena_ids, sigil_ids in BOSSES:
        if only and key not in only:
            continue
        room_position, room_rotation = transform(parts.entries[find(parts, room)])
        ry = room_rotation[1]
        frames[room] = (room_position, ry)
        position = to_world(room_position, ry, sigil)
        rotation = (ry + facing + 180.0) % 360.0 - 180.0
        names = [entry_name(e) for e in parts.entries]
        if name in names:
            k = names.index(name)
            e = bytearray(parts.entries[k])
        else:
            assert entity not in entity_ids(lists), f"entity id {entity} is taken"
            model = add_model(lists, template_lists, name[:5])
            template = _list(template_lists, "PARTS_PARAM_ST")
            e = bytearray(template.entries[find(template, TEMPLATE_PART)])
            _set_name(e, name)
            type_index = sum(1 for x in parts.entries if u32(x, 0x0C) == ENEMY)
            struct.pack_into("<iiii", e, 0x08, int(name[-4:]), ENEMY, type_index, model)
            struct.pack_into("<I", e, q(e, 0x60), entity)
            struct.pack_into("<h", e, q(e, 0x68) + WALK_ROUTE, -1)
            k = None
        struct.pack_into("<ii", e, q(e, 0x68) + THINK, think, npc)
        struct.pack_into("<3f", e, 0x20, *position)
        struct.pack_into("<3f", e, 0x2C, 0.0, rotation, 0.0)
        if k is None:
            insert_enemy(lists, bytes(e))
        else:
            parts.entries[k] = bytes(e)
        print(f"{key}: {name}, entity {entity}, Think {think}, NpcParam {npc}, at "
              f"{tuple(round(c, 3) for c in position)}, rotation y {rotation:.1f}")
        regions += [(f"W101_{key}_arena", room, arena, 0.0, radius, *arena_ids),
                    (f"W101_{key}_sigil", room, (sigil[0], sigil[1] + SIGIL_LIFT, sigil[2]), facing, 0.5, *sigil_ids)]
    for key, room, entity, region_id in WATCHED:
        if only and key not in only:
            continue
        if room not in frames:
            room_position, room_rotation = transform(parts.entries[find(parts, room)])
            frames[room] = (room_position, room_rotation[1])
        regions.append((f"W101_{key}_room", room, (0.0, WATCH_RADIUS - WATCH_DEPTH, 0.0), 0.0, WATCH_RADIUS, entity,
                        region_id))
    count_users(lists)
    put_regions(lists, regions, frames)
    check_grouped(lists)
    return write_msb(header, lists)


if __name__ == "__main__":
    only = None
    if "--only" in sys.argv:  # --only Rattlebones[,Blackhope...]
        k = sys.argv.index("--only")
        only = set(sys.argv[k + 1].split(","))
        del sys.argv[k:k + 2]
    template = load(sys.argv[2])
    del sys.argv[2]
    run(lambda raw: place_bosses(raw, template, only))
