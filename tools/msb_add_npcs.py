"""Unicorn Way's Wizard101 NPCs as Elden Ring characters in m60_42_36_00 (w101_quests.CHARACTERS; round 15).

Each character is a copy of vanilla patroller c4300_9020 from m60_43_36_00 (as msb_add_boss.py) with the model,
Think / NpcParam / CharaInit rows and entity id changed, TalkID 0 (no talk ESD: emevd_quests.py talks through the
"Talk" action button and ER's dialog box), no patrol route, inserted at the end of the enemy block with every later
part index remapped; new models go in name order among the character models (c0000 and c3200 are in the map already).

Positions are W101 zone positions in the frame's FLVER coordinates (W101 (x, z, y) * 0.01) through the frame part's
transform. W101 orientation is a yaw in radians about z with forward +y, so the FLVER heading is phi = 90 + yaw
(degrees, in the x-z plane); through the frame's y rotation a it is (cos(phi - a), sin(phi - a)) in the world, and
an ER character with y rotation r faces (-sin r, -cos r), so r = atan2(-dx, -dz). O'Ryan has no street yaw (W101
places him in front of house H1): he takes the giver's facing, ry + ORYAN_FACING + 180.
Rerunning moves the existing characters and updates their rows.

Usage: python msb_add_npcs.py map.msb.dcx template_map.msb out.msb.dcx
  template_map.msb: vanilla m60_43_36_00.msb (uncompressed, or DFLT DCX)
"""

import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import entity_ids, run  # noqa: E402
from msb_add_ghost import TEMPLATE_PART, THINK, WALK_ROUTE, add_model, check_grouped, count_users, load, q, u32  # noqa: E402
from msb_add_mobs import ENEMY, insert_enemy  # noqa: E402
from msb_add_quests import FRAMES  # noqa: E402
from msb_add_room import to_world  # noqa: E402
from msb_floor import _list, _set_name, entry_name, find, read_msb, transform, write_msb  # noqa: E402
import w101_quests as Q  # noqa: E402

TALK_ID, CHARA_INIT, COLLISION = 0x10, 0x18, 0x1C  # in the enemy type data (Smithbox MSBE meta)


def rotation_of(yaw, ry):
    if yaw is None:
        return (ry + Q.ORYAN_FACING + 180.0) % 360.0 - 180.0
    phi = math.radians(90.0 + yaw - ry)
    dx, dz = math.cos(phi), math.sin(phi)
    return math.degrees(math.atan2(-dx, -dz))


def part_name(k, model):
    return f"{model}_{Q.CHARACTER_PART_ID + k}"


def place_npcs(raw, template_raw):
    header, lists = read_msb(raw)
    _, template_lists = read_msb(template_raw)
    check_grouped(lists)
    parts = _list(lists, "PARTS_PARAM_ST")
    frames = {}
    for key, part in FRAMES.items():
        position, rotation = transform(parts.entries[find(parts, part)])
        frames[key] = (position, rotation[1])
    for k, (slug, title, frame, local, yaw, model, think, npc, chara_init, entity) in enumerate(Q.CHARACTERS):
        name = part_name(k, model)
        frame_position, ry = frames[frame]
        position = to_world(frame_position, ry, local)
        rotation = rotation_of(yaw, ry)
        names = [entry_name(e) for e in parts.entries]
        if name in names:
            i = names.index(name)
            e = bytearray(parts.entries[i])
        else:
            assert entity not in entity_ids(lists), f"entity id {entity} is taken"
            index = add_model(lists, template_lists, model)
            template = _list(template_lists, "PARTS_PARAM_ST")
            e = bytearray(template.entries[find(template, TEMPLATE_PART)])
            _set_name(e, name)
            type_index = sum(1 for x in parts.entries if u32(x, 0x0C) == ENEMY)
            struct.pack_into("<iiii", e, 0x08, Q.CHARACTER_PART_ID + k, ENEMY, type_index, index)
            struct.pack_into("<I", e, q(e, 0x60), entity)
            i = None
        t = q(e, 0x68)
        struct.pack_into("<iii", e, t + THINK, think, npc, 0)
        assert TALK_ID == THINK + 8
        struct.pack_into("<ii", e, t + CHARA_INIT, chara_init, -1)
        struct.pack_into("<h", e, t + WALK_ROUTE, -1)
        struct.pack_into("<3f", e, 0x20, *position)
        struct.pack_into("<3f", e, 0x2C, 0.0, rotation, 0.0)
        if i is None:
            insert_enemy(lists, bytes(e))
        else:
            parts.entries[i] = bytes(e)
        print(f"{slug}\t{title}\t{name}\tentity {entity}\tThink {think}\tNpc {npc}\tCharaInit {chara_init}\t"
              f"world {tuple(round(c, 4) for c in position)}\trot {rotation:.1f}")
    count_users(lists)
    check_grouped(lists)
    return write_msb(header, lists)


if __name__ == "__main__":
    template = load(sys.argv[2])
    del sys.argv[2]
    run(lambda raw: place_npcs(raw, template))
