"""Where the quest arrow points (round 18, F4): every goal of w101_quests.QUESTS as places in the deployed
m60_42_36_00.msb, read from the map itself (part and region positions by entity id), printed as the Rust table of
hud_orbs/src/questhelper.rs (TARGETS).

  talk     the character (msb_add_npcs.py, entities 1042360820..)
  goto     the goal's sphere (w101_quests.REGIONS)
  defeat   every enemy of the kind (w101_quests.MOBS); the HUD takes the nearest
  cages    every Bone Cage (w101_quests.CAGES); the nearest
  boss     the boss (w101_quests.BOSSES)
A goal in a room (Blackhope Tower, the Archives, the Thief's House) also gets the street side of its door (the room's
"out" arrival sphere, msb_door_spawns.SPAWNS' keys): the HUD points at the door unless the player is near the room.

Usage: python w101_quest_targets.py m60_42_36_00.msb.dcx
"""
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import w101_quests as Q  # noqa: E402
from msb_floor import _list, entry_name, read_msb, transform  # noqa: E402

# The rooms' arrival spheres: (inside, street side of the door), msb_door_spawns.SPAWNS.
ROOMS = {"T2": (1042362801, 1042362803), "T1": (1042362817, 1042362819), "H4": (1042362821, 1042362823)}
ROOM_OF = {"rattlebones": "T2", "lady_blackhope": "T1", "fear_of_fire": "H4", "T1": "T1", "T2": "T2", "H4": "H4"}


def entities(raw):
    _header, lists = read_msb(raw)
    out = {}
    for e in _list(lists, "PARTS_PARAM_ST").entries:
        try:
            ent = struct.unpack_from("<I", e, struct.unpack_from("<q", e, 0x60)[0])[0]
        except struct.error:
            continue
        if ent:
            out[ent] = (entry_name(e), transform(e)[0])
    for e in _list(lists, "POINT_PARAM_ST").entries:
        ent = struct.unpack_from("<I", e, struct.unpack_from("<q", e, 0x50)[0] + 4)[0]
        if ent:
            out[ent] = (entry_name(e), struct.unpack_from("<3f", e, 0x14))
    return out


def main(argv):
    raw = Path(argv[1]).read_bytes()
    if raw[:4] == b"DCX\0":
        raw = zlib.decompress(raw[0x4C:])
    at = entities(raw)
    npc = {c[0]: c[-1] for c in Q.CHARACTERS}
    rows = []
    for q in Q.QUESTS:
        for i, g in enumerate(q["goals"]):
            kind, target = g["type"], g["target"]
            if kind == "talk":
                ids = [npc[target]]
            elif kind == "goto":
                ids = [Q.REGION_ENTITY[target]]
            elif kind == "defeat":
                ids = Q.MOBS[target]
            elif kind == "cages":
                ids = [entity for _local, entity in Q.CAGES]
            elif kind == "boss":
                ids = [Q.BOSSES[target]]
            else:
                raise ValueError(kind)
            room = ROOM_OF.get(target)
            if room and kind == "boss":
                ids = [ROOMS[room][0]] if Q.BOSSES[target] not in at else ids
            missing = [e for e in ids if e not in at]
            assert not missing, f"{q['slug']} goal {i + 1}: entities {missing} not in the map"
            points = [at[e][1] for e in ids]
            door = at[ROOMS[room][1]][1] if room else None
            rows.append((q["slug"], i, points, door))
    print("const TARGETS: &[(&str, usize, &[[f32; 3]], Option<[f32; 3]>)] = &[")
    for slug, i, points, door in rows:
        pts = ", ".join("[" + ", ".join(f"{c:.3f}" for c in p) + "]" for p in points)
        d = "None" if door is None else "Some([" + ", ".join(f"{c:.3f}" for c in door) + "])"
        print(f'    ("{slug}", {i}, &[{pts}], {d}),')
    print("];")


if __name__ == "__main__":
    main(sys.argv)
