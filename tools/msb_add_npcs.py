"""Unicorn Way's Wizard101 NPCs as Elden Ring characters in m60_42_36_00 (w101_quests.CHARACTERS; round 15).

Each character is a copy of vanilla patroller c4300_9020 from m60_43_36_00 (as msb_add_boss.py) with the model,
Think / NpcParam / CharaInit rows and entity id changed, TalkID 0 (no talk ESD: emevd_quests.py talks through the
"Talk" action button and ER's dialog box), no patrol route, inserted at the end of the enemy block with every later
part index remapped; new models go in name order among the character models (c0000 and c3200 are in the map already).

Positions are W101 zone positions in the frame's FLVER coordinates (W101 (x, z, y) * 0.01) through the frame part's
transform. A W101 character faces (-sin yaw, -cos yaw) in W101 (x, y) (round 16, see w101_quests.CHARACTERS), so the
FLVER heading is phi = -90 - yaw (degrees, in the x-z plane); through the frame's y rotation a it is
(cos(phi - a), sin(phi - a)) in the world, and an ER character with y rotation r faces (-sin r, -cos r) (vanilla
check: White Mask Varre, rotation -25.6, faces The First Step's grace 7.8 m away, cos 0.99), so r = atan2(-dx, -dz),
which works out to r = yaw + a. Round 15 used phi = 90 + yaw, a mirror of the W101 facing.
Rerunning moves the existing characters and updates their rows. Parts in w101_quests.REMOVED are deleted (round 16
F1), with every later part index remapped and their models dropped when nothing else uses them.

Usage: python msb_add_npcs.py map.msb.dcx template_map.msb out.msb.dcx
  template_map.msb: vanilla m60_43_36_00.msb (uncompressed, or DFLT DCX)
"""

import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import entity_ids, run  # noqa: E402
from msb_add_ghost import (TEMPLATE_PART, THINK, WALK_ROUTE, add_model, check_grouped, count_users, load, q,  # noqa: E402
                           remove_unused_model, u32)
from msb_add_mobs import ENEMY, insert_enemy, remap_parts  # noqa: E402
from msb_add_quests import FRAMES  # noqa: E402
from msb_add_room import to_world  # noqa: E402
from msb_floor import _list, _set_name, entry_name, find, read_msb, transform, write_msb  # noqa: E402
import w101_quests as Q  # noqa: E402

TALK_ID, CHARA_INIT, COLLISION = 0x10, 0x18, 0x1C  # in the enemy type data (Smithbox MSBE meta)


def rotation_of(yaw, ry):
    phi = math.radians(-90.0 - yaw - ry)
    dx, dz = math.cos(phi), math.sin(phi)
    return math.degrees(math.atan2(-dx, -dz))


def remove_part(lists, name):
    """Delete part `name` (an enemy nothing refers to), remapping later part indices, the enemies' type indices and
    the model list. Returns False when the map has no such part."""
    parts = _list(lists, "PARTS_PARAM_ST").entries
    names = [entry_name(e) for e in parts]
    if name not in names:
        return False
    at = names.index(name)
    gone = parts[at]
    assert u32(gone, 0x0C) == ENEMY, f"{name} is not an enemy"

    def shift(i):
        assert i != at, f"something still refers to {name}"
        return i - 1 if i > at else i

    del parts[at]
    remap_parts(lists, shift)
    for k, e in enumerate(parts):
        if u32(e, 0x0C) == ENEMY and u32(e, 0x10) > u32(gone, 0x10):
            e = bytearray(e)
            struct.pack_into("<i", e, 0x10, u32(e, 0x10) - 1)
            parts[k] = bytes(e)
    remove_unused_model(lists, u32(gone, 0x14))
    return True


def part_name(k, model):
    return f"{model}_{Q.CHARACTER_PART_ID + k}"


def place_npcs(raw, template_raw):
    header, lists = read_msb(raw)
    _, template_lists = read_msb(template_raw)
    check_grouped(lists)
    for slug, name, _model in Q.REMOVED:
        if remove_part(lists, name):
            print(f"{slug}\t{name}\tremoved")
    parts = _list(lists, "PARTS_PARAM_ST")
    frames = {}
    for key, part in FRAMES.items():
        position, rotation = transform(parts.entries[find(parts, part)])
        frames[key] = (position, rotation[1])
    swapped = set()
    for k, (slug, title, frame, local, yaw, model, think, npc, chara_init, entity) in enumerate(Q.CHARACTERS):
        name = part_name(k, model)
        frame_position, ry = frames[frame]
        position = to_world(frame_position, ry, local)
        rotation = rotation_of(yaw, ry)
        names = [entry_name(e) for e in parts.entries]
        # Round 24 (N1): found by entity id, so a new stand-in model (c2180 -> c5330 ...) swaps the model in place
        # and renames the part (same length) instead of adding a second character with the same entity id.
        by_entity = [k for k, x in enumerate(parts.entries) if u32(x, 0x0C) == ENEMY and u32(x, q(x, 0x60)) == entity]
        if by_entity:
            i = by_entity[0]
            if entry_name(parts.entries[i]) != name:
                old_model = entry_name(_list(lists, "MODEL_PARAM_ST").entries[u32(parts.entries[i], 0x14)])
                index = add_model(lists, template_lists, model)  # may remap the stored parts' model indices
                e = bytearray(parts.entries[i])
                struct.pack_into("<i", e, 0x14, index)
                _set_name(e, name)
                parts.entries[i] = bytes(e)
                swapped.add(old_model)
                print(f"{slug}\tmodel {old_model} -> {model}")
            e = bytearray(parts.entries[i])
        elif name in names:
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
    for old_model in swapped:  # a stand-in model nothing uses any more
        names = [entry_name(e) for e in _list(lists, "MODEL_PARAM_ST").entries]
        if old_model in names:
            remove_unused_model(lists, names.index(old_model))
    count_users(lists)
    check_grouped(lists)
    return write_msb(header, lists)


if __name__ == "__main__":
    template = load(sys.argv[2])
    del sys.argv[2]
    run(lambda raw: place_npcs(raw, template))
