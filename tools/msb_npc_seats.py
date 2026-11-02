"""Round 25 (M2): the vanilla seats under the street NPCs whose Elden Ring idle is a sitting pose.

Ryan: "Connelly (Bernahl) sits on something in vanilla, Olivia (Enia) too: place the vanilla seat asset under them."
Vanilla (Game\\map\\mapstudio, read only):
  - Finger Reader Enia c2170_9000 in m11_10_00_00 (Roundtable Hold, (-348.22, -20.18, -328.24) rot 150.7) sits in
    AEG221_605_4500, 0.19 m from her root: in her own frame (x right, z back) (-0.135, +0.084, +0.106), turned -0.7
    deg. AEG221_605 is a 2.6 x 2.2 x 1.2 m chair; her idle a000_000020 has the pelvis 1.28 m and the feet 0.78 m up.
  - Knight Bernahl c0000_9000 in m60_42_38_00 (Warmaster's Shack, rot 127.5) idles in a345_090100 (NpcParam
    523260000 SpEffect 9845: animIdOffset 345000000), seated with the pelvis 0.65 m up. His seat is AEG201_043_1003
    (a 0.45 x 0.52 x 0.45 m stool: 0.52 + the pelvis' ~0.13 over a seat = 0.65), the only seat-sized asset by him: in
    his frame (+0.990, +0.046, +0.440), turned +17.3 deg. Kept exactly as vanilla lays it out (rather than straight
    under his root, where the stool's collision could lift him by 0.52 m).
Offsets are from the NPC's root on the ground; w101_quests.CHARACTERS spots are SPAWN_LIFT (0.2 m) over the W101
ground, so the seats go that much lower.
Seats are copies of the street part (msb_add_garden.put_asset: PartsDrawParam row 36, no entity id), at the NPC's
place from w101_quests.CHARACTERS (as msb_add_npcs.py), so rerun this after msb_add_npcs.py. Idempotent.

Usage: python msb_npc_seats.py map.msb.dcx out.msb.dcx
"""
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import put_asset, run  # noqa: E402
from msb_add_npcs import rotation_of  # noqa: E402
from msb_add_quests import FRAMES  # noqa: E402
from msb_add_room import to_world  # noqa: E402
from msb_floor import _list, find, read_msb, transform, write_msb  # noqa: E402
import w101_quests as Q  # noqa: E402

SPAWN_LIFT = 0.2  # w101_quests.CHARACTERS local y = W101 ground (z * 0.01) + 0.2
# slug: (seat part, model, part header id, offset in the NPC's frame (x, y, z), turn from the NPC's rotation)
SEATS = {
    "olivia_dawnwillow": ("AEG221_605_9500", "AEG221_605", 9500, (-0.135, 0.084, 0.106), -0.7),
    "private_connelly": ("AEG201_043_9501", "AEG201_043", 9501, (0.990, 0.046, 0.440), 17.3),
}


def place_seats(raw):
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    for slug, (name, model, part_id, offset, turn) in SEATS.items():
        _slug, _title, frame, local, yaw, *_rest = Q.CHARACTER[slug]
        frame_position, frame_rotation = transform(parts.entries[find(parts, FRAMES[frame])])
        npc = to_world(frame_position, frame_rotation[1], local)
        ry = rotation_of(yaw, frame_rotation[1])
        world = to_world(npc, ry, (offset[0], offset[1] - SPAWN_LIFT, offset[2]))
        put_asset(lists, name, model, world, ry + turn, (0.0, 0.0, 0.0), part_id=part_id)
        print(f"{slug}: seat {name} under the NPC at {tuple(round(c, 3) for c in npc)} rot {ry:.1f}")
    return write_msb(header, lists)


if __name__ == "__main__":
    run(place_seats)
