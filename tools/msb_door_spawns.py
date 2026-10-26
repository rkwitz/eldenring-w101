"""SpawnPoint regions for the riding door warps of m60_42_36_00 (emevd.rattlebones_doors, riding events).

2003[14] Warp Player sends the player to an "initial area entity", and every vanilla use of it targets a SpawnPoint
region (type 8; e.g. m12_02_00_00 "領域 m12_08からのワープ地点", m32_08_00_00 "復帰ポイント ..."), while our door
arrival points are Other-type spheres (what 2004[41] takes). So each arrival region gets a SpawnPoint twin at the same
position and rotation (SPAWNS: arrival entity -> spawn entity, region id), a byte copy of the m12_02 one with name,
type index, region id, entity id, position and rotation set, inserted in its type's place in the region list with
every region index the events hold remapped (msb_add_indoor.insert_region). Rerunning moves them onto their arrival
regions again.

Usage: python msb_door_spawns.py map.msb.dcx vanilla_dir out.msb.dcx   (vanilla_dir: uncompressed m12_02_00_00.msb)
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import entity_ids, run  # noqa: E402
from msb_add_ghost import check_grouped, q, u32  # noqa: E402
from msb_add_indoor import insert_region  # noqa: E402
from msb_floor import _list, _set_name, entry_name, read_msb, write_msb  # noqa: E402

SPAWN_POINT = 8
TEMPLATE = ("m12_02_00_00.msb", "領域 m12_08からのワープ地点")
# arrival region entity -> (spawn region name, spawn entity, region id)
SPAWNS = {
    1042362801: ("W101_RB_in_spawn", 1042362840, 9247),
    1042362803: ("W101_RB_out_spawn", 1042362841, 9248),
    1042362805: ("W101_G_in_spawn", 1042362842, 9249),
    1042362807: ("W101_G_out_spawn", 1042362843, 9250),
    1042362817: ("W101_T1_in_spawn", 1042362844, 9251),
    1042362819: ("W101_T1_out_spawn", 1042362845, 9252),
    1042362821: ("W101_H4_in_spawn", 1042362846, 9253),
    1042362823: ("W101_H4_out_spawn", 1042362847, 9254),
}


def entity_of(e):
    return u32(e, q(e, 0x50) + 4) & 0xFFFFFFFF


def door_spawns(raw, vanilla_dir):
    header, lists = read_msb(raw)
    _, vanilla = read_msb((Path(vanilla_dir) / TEMPLATE[0]).read_bytes())
    template = next(e for e in _list(vanilla, "POINT_PARAM_ST").entries if entry_name(e) == TEMPLATE[1])
    assert u32(template, 0x08) == SPAWN_POINT
    ours = {name for name, _, _ in SPAWNS.values()}
    for arrival, (name, entity, region_id) in SPAWNS.items():
        regions = _list(lists, "POINT_PARAM_ST").entries
        source = next(e for e in regions if entity_of(e) == arrival)
        existing = [k for k, e in enumerate(regions) if entry_name(e) == name]
        e = bytearray(regions[existing[0]] if existing else template)
        if not existing:
            assert entity not in entity_ids(lists), f"entity {entity} is taken"
            assert region_id not in {u32(r, 0x2C) for r in regions if entry_name(r) not in ours}, f"region id {region_id}"
            _set_name(e, name)
            struct.pack_into("<i", e, 0x0C, sum(u32(r, 0x08) == SPAWN_POINT for r in regions))
            struct.pack_into("<i", e, 0x2C, region_id)
            struct.pack_into("<I", e, q(e, 0x50) + 4, entity)
        e[0x14:0x2C] = source[0x14:0x2C]  # position and rotation of the arrival region
        if existing:
            regions[existing[0]] = bytes(e)
        else:
            insert_region(lists, bytes(e), SPAWN_POINT)
        x, y, z = struct.unpack_from("<3f", e, 0x14)
        print(f"{name}: SpawnPoint {entity} at ({x:.2f}, {y:.2f}, {z:.2f}), facing {struct.unpack_from('<f', e, 0x24)[0]:.1f}"
              f" (arrival {arrival})")
    check_grouped(lists)
    return write_msb(header, lists)


if __name__ == "__main__":
    vanilla_dir = sys.argv[2]
    del sys.argv[2]
    run(lambda raw: door_spawns(raw, vanilla_dir))
