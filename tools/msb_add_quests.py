"""Map side of the Unicorn Way quests (tools/w101_quests.py; events: emevd.py --quests) in m60_42_36_00:

  - entity ids on the 10 street mobs (c4180_9000..9003, c3500_9000..9003, c3450_9000..9001 -> w101_quests.MOBS),
    so the kill-count events can watch them; nothing else about them changes
  - the quest givers: one placeholder asset each (w101_quests.NPCS: vanilla statues / an armour stand, to be swapped
    for converted W101 NPC meshes later by changing the model), with the entity id the talk action button uses
  - the four Bone Cages (placeholder prison cage asset, w101_quests.CAGES), which the cage events disable when opened
  - the goal spheres (w101_quests.REGIONS), Other-type regions like the door regions

Assets are copies of the street part (as msb_add_garden.put_asset) placed in the frame of the street, the garden or an
interior, turned like it plus the NPC's facing; regions use msb_add_garden.put_regions. Running it again moves the
existing entries instead of adding them twice.

Usage: python msb_add_quests.py map.msb.dcx out.msb.dcx   (DFLT DCX as Smithbox saves)
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import STREET, entity_ids, put_asset, put_regions, run  # noqa: E402
from msb_floor import ENTITY_DATA_FIELD, _list, entry_name, find, read_msb, set_transform, transform, write_msb  # noqa: E402
import w101_quests as Q  # noqa: E402

FRAMES = {"street": STREET, "garden": "AEG030_555_9000", "AEG030_519_9001": "AEG030_519_9001",
          "AEG030_557_9000": "AEG030_557_9000"}
PART_ID = 9300  # part header 0x08 of the first new asset (9300.. are free: ours use 9000-9202)


def entity_of_part(e):
    return struct.unpack_from("<I", e, struct.unpack_from("<q", e, ENTITY_DATA_FIELD)[0])[0]


def set_entity(e, entity):
    e = bytearray(e)
    struct.pack_into("<I", e, struct.unpack_from("<q", e, ENTITY_DATA_FIELD)[0], entity)
    return bytes(e)


def place_quests(raw):
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    names = [entry_name(e) for e in parts.entries]
    frames = {}
    for key, part in FRAMES.items():
        position, rotation = transform(parts.entries[find(parts, part)])
        assert rotation[0] == 0 and rotation[2] == 0, f"{part} is tilted"
        frames[key] = (position, rotation[1])

    # 1. mob entity ids
    taken = entity_ids(lists)
    for kind, ids in Q.MOBS.items():
        for k, entity in enumerate(ids):
            name = Q.MOB_PARTS[kind] % k
            i = names.index(name)
            old = entity_of_part(parts.entries[i])
            assert old in (0, entity), f"{name} already has entity id {old}"
            assert old == entity or entity not in taken, f"entity id {entity} is taken"
            parts.entries[i] = set_entity(parts.entries[i], entity)
            print(f"{name}: entity {entity}")

    # 2. givers and cages (assets)
    assets = [(f"{model}_{9000 + k}", frame, local, facing, model, entity)
              for k, (slug, frame, local, facing, model, entity) in enumerate(Q.NPCS)]
    assets += [(f"{Q.CAGE_MODEL}_{9100 + c}", "street", local, 0.0, Q.CAGE_MODEL, entity)
               for c, (local, entity) in enumerate(Q.CAGES)]
    used_part_ids = {struct.unpack_from("<i", e, 0x08)[0] for e in parts.entries
                     if entry_name(e) not in {a[0] for a in assets}}
    for k, (name, frame, local, facing, model, entity) in enumerate(assets):
        part_id = PART_ID + k
        assert part_id not in used_part_ids, f"part id {part_id} is taken"
        position, ry = frames[frame]
        exists = name in [entry_name(e) for e in parts.entries]
        if not exists:
            assert entity not in taken, f"entity id {entity} is taken"
        put_asset(lists, name, model, position, ry, local, part_id=part_id)
        i = find(parts, name)
        rotation = (ry + facing + 180.0) % 360.0 - 180.0
        parts.entries[i] = set_entity(set_transform(parts.entries[i], rotation=(0.0, rotation, 0.0)), entity)
        print(f"  entity {entity}, facing {rotation:.1f}")

    # 3. goal spheres
    put_regions(lists, [(f"W101_Q_{slug}", frame, local, 0.0, radius, entity, region_id)
                        for slug, frame, local, radius, entity, region_id in Q.REGIONS], frames)
    return write_msb(header, lists)


if __name__ == "__main__":
    run(place_quests)
