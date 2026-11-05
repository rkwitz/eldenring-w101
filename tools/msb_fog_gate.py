"""Round 24 (M3): the Stranded Graveyard exit (the lift-up gate Ryan pulls open into Unicorn Way) is a fog gate seen
from Unicorn Way at every distance.

Ryan: "from the Unicorn Way side, looking back at it, the gate stands alone in the archway with out-of-bounds visible
behind it; up close it becomes a fog gate. While on Unicorn Way, that gate should ALWAYS be a fog gate at any
distance." The fog is m18's own asset AEG099_230_9001 (entity 18001696, no event uses it) standing in the gate,
m18 (-150.56, 77.82, 16.27) rot y 153.0: it only exists while m18 is loaded, i.e. near the gate. This puts a copy of
it in m60_42_36_00 at the same world spot (m18 -> m60_42_36 = +(147, 16, -102), WorldMapLegacyConvParam rows
140/141, er-map-edits), as a copy of the street part, so it carries the street's draw settings (PartsDrawParam row
36: drawn at any distance, no LOD fade), no entity id, casting no shadow.

Round 25 (M6, Ryan: "the Commons fog gate still only appears near the grace"): AEG099_230 is an EMPTY model (its
FLVER has no mesh; AssetGeometryParam 99230: isCreateMultiPlayOnly, the co-op area border), so the round 24 copy
draws nothing. The fog seen up close is m18's AEG092_300_0500, a 5 x 6 m plane with the material
Field_PhantomWall_Loding (the "phantom wall" fog), at m18 (-148.535, 78.020, 12.045) rot -26.5, 4.7 m in front of
the gate: it exists only while m18 is loaded. It is now copied the same way (AEG092_300_9101). The AEG099_230 copy
is left as it was (nothing to see; only matters in co-op).

Round 26 (W1, Ryan: still no fog from far away): that copy's look is all its material's shader (M_PhantomWall_Loading:
two mask textures), which we can't read, so a third copy is our own asset aeg030_530 (blender_build_fog.py: a 5 x 6
m two-sided plane, white mist texture, the street's alpha-blended shader, drawn at any distance), AEG030_530_9102 on
the same spot and rotation as the phantom wall. Rerunning moves all three in place.

Round 27 (A6, Ryan: "the round 26 plane shows a bit, but it is too narrow and too transparent; I want Elden Ring's
normal fully opaque fog barrier over the whole archway, visible from any distance"). What the phantom wall is: m18's
AEG092_300_0500 stands with AEG099_230_9001 (AssetGeometryParam 99230 isCreateMultiPlayOnly: the co-op area border)
and its material Field_PhantomWall is the see-through co-op border wall, not a fog gate. Elden Ring's fog gate is a
different thing, proven from vanilla data: m18's own boss fog walls AEG099_001_9010 / 9011 (entities 18001850 / 851)
are shown by event 18000870 -> common_func 9005811, which runs 2005[3] Change Asset Enable State (asset, on) and
2006[4] Create Asset-following SFX (asset, dummypoly 101, SFX 3 / 4); the asset itself has no mesh (AEG099_001.flver:
10 dummies only, fog extent dummies 180-187 at x +-2.5, z +-2: a 5 m wide wall) and AssetGeometryParam 99001 is not
co-op-only. So FOG_WALLS are two AEG099_001 copies (entities 1042361892 / 893) side by side across the 8 m archway
(blender_switch_street.tunnel_inside: |x| < 4), 0.5 m on the street side of the round 26 plane, and emevd.py
--fog-wall (event 1042362860) gives them the vanilla SFX 3 at every load. The plane stays as the far view, now
8.4 m wide and nearly opaque (blender_build_fog.py / fog_texture.py), behind them.
Not proven: the SFX's own draw distance (vanilla fog gates are seen from a corridor's length, not across a map).

Round 28 (W3, Ryan: "the Commons fog wall is too far back; it must stand at the gate itself, the real Elden Ring
gate, seen from the street side; you go through the gate into the fog; not visible from behind"). The real gate is
the graveyard's lift-up exit door AEG110_064_1000 (ObjAct 1042363540) in this map, at street frame z -11.30 (its
local +z points out of the graveyard: msb_name_region.DOOR). The round 27 fog stood 3.9-4.4 m behind it, on the
graveyard side: the plane and phantom-wall copy on m18's phantom-wall spot (street z -15.70) and the two AEG099_001
fog gates at street z -15.2. All four now stand in the door's own frame (its position and yaw), on its street side:
the fog gates DOOR_WALL_Z, the plane DOOR_MIST_Z, the phantom-wall copy DOOR_PHANTOM_Z metres out of the door
(W101's archway mouth is at street z -10.91, 0.39 m out), so the fog fills the gateway and walking through the gate
is walking into the fog. The plane and the phantom-wall copy get entity ids (FOG_ENTITIES) and a box region
FOG_SIDE_REGION (entity FOG_SIDE_ENTITY, Other type, a copy of UnicornWay_title_door) covers the street side of the
door from FOG_SIDE_NEAR (just behind the door) outwards, like the Unicorn Way name box: emevd.py --fog-wall shows the
fog only while the player is inside it, so from the graveyard side (behind) there is no fog. AEG099_230_9100 (the
empty co-op border copy, already at the gate) is not moved.

Usage: python msb_fog_gate.py map.msb.dcx out.msb.dcx   (DFLT DCX as Smithbox saves; rerunning moves it in place)
"""
import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_garden import STREET, put_asset, run  # noqa: E402
from msb_floor import _list, entry_name, find, read_msb, write_msb  # noqa: E402

FOG_MODEL = "AEG099_230"
FOG_PART = "AEG099_230_9100"
FOG_PART_ID = 9400  # part header 0x08 (unique in the map; ours use 9000-9312)
M18_POSITION, M18_ROTATION = (-150.56, 77.82, 16.27), 153.0  # m18_00_00_00 AEG099_230_9001
M18_TO_M60 = (147.0, 16.0, -102.0)
# Round 25 (M6): the visible fog (see above)
WALL_MODEL = "AEG092_300"
WALL_PART = "AEG092_300_9101"
WALL_PART_ID = 9401
WALL_M18_POSITION, WALL_M18_ROTATION = (-148.534912, 78.019989, 12.045101), -26.500067  # m18 AEG092_300_0500
# Round 26 (W1): our own fog plane (blender_build_fog.py) on the phantom wall's spot
MIST_MODEL = "AEG030_530"
MIST_PART = "AEG030_530_9102"
MIST_PART_ID = 9402
# Round 27 (A6): Elden Ring's fog gate (see above): (part, header id, entity, x across the archway) in the street's
# own frame at depth FOG_WALL_Z (the plane is at z -15.7, the archway's mouth at -10.91), turned with the street.
# Round 33 (G8, Ryan: "after lifting the gate the fog wall blocks the player from walking through"): AEG099_001 is a
# solid wall. Its aeg099_001_l.geomhkxbnd holds a Havok physics scene (hknpPhysicsSceneData with bodies and shapes),
# and vanilla never walks through it: common_func 9005800 makes the player pass with the traverse animation 60060
# (2004[14] 10000 60060) instead. Event 1042362860 enables our two copies as soon as the player stands in the gateway
# (FOG_SIDE_NEAR is 0.5 m behind the door), 0.10 m in front of him, so the gate is blocked both ways. The fog gates now
# use AEG099_990: no mesh, no collision file in asset\aeg\aeg099, AssetEnvironmentGeometryParam 99990 with no special
# flags, used by vanilla in every legacy dungeon (m10-m16, m20, m21, m35, m40: 100+ parts in 40 MSBs), and its dummy
# 100 has the same frame as AEG099_001's fog dummy 101 (origin, forward -z, up +y), so the SFX 3 sits where it did
# (emevd.FOG_WALL_DUMMY 100). fog_walls_no_collision renames the round 27 parts and swaps their model in place.
FOG_WALL_MODEL = "AEG099_990"
FOG_WALL_OLD_MODEL = "AEG099_001"
FOG_WALLS = [("AEG099_990_9103", 9403, 1042361892, -2.0), ("AEG099_990_9104", 9404, 1042361893, 2.0)]
FOG_WALL_Z, FOG_WALL_Y = -15.2, 0.02  # round 27 (street frame); round 28 places them by the door instead
# Round 28 (W3): the real gate and the fog's place in its frame (metres out of the door, toward the street).
DOOR = "AEG110_064_1000"
# Round 29 (F2, Ryan: "move it back a very small amount; it clips slightly through the wall"): at +0.35 the fog stood
# 0.04 m inside W101's archway mouth (street z -10.91 = door +0.39), so its sides showed on the wall's face; it is now
# 0.25 m further back, in the gateway behind the wall face (round 28: 0.35, 0.25, 0.15).
DOOR_WALL_Z, DOOR_MIST_Z, DOOR_PHANTOM_Z, DOOR_Y = 0.10, 0.0, -0.10, 0.02
FOG_ENTITIES = {MIST_PART: 1042361894, WALL_PART: 1042361895}
FOG_SIDE_REGION, FOG_SIDE_ENTITY, FOG_SIDE_REGION_ID = "W101_fog_street_side", 1042362702, 9270
# Box in the door's frame: from FOG_SIDE_NEAR (behind the door plane, so standing in the gateway still counts) out
# over the whole street (as msb_name_region's name box: street ground spans x -320..160, z -11..255 in its frame).
FOG_SIDE_NEAR, FOG_SIDE_FAR, FOG_SIDE_LEFT, FOG_SIDE_RIGHT, FOG_SIDE_BELOW, FOG_SIDE_HEIGHT = (
    -0.5, 276.0, -330.0, 170.0, 10.0, 160.0)
TEMPLATE_REGION = "UnicornWay_title_door"  # msb_name_region.TRIGGER: an Other-type box with an entity id
OTHER = 0xFFFFFFFF


def put_copy(lists, name, model, part_id, m18_position, m18_rotation):
    """Asset `name` (a copy of the street part) at the world spot of m18's `m18_position`, keeping m18's rotation."""
    parts = _list(lists, "PARTS_PARAM_ST")
    assert all(struct.unpack_from("<i", e, 0x08)[0] != part_id or entry_name(e) == name for e in parts.entries)
    street = parts.entries[find(parts, STREET)]
    (px, py, pz), sy = struct.unpack_from("<3f", street, 0x20), struct.unpack_from("<3f", street, 0x2C)[1]
    world = tuple(a + b for a, b in zip(m18_position, M18_TO_M60))
    # inverse of msb_add_room.to_world: the point in the street's own frame
    c, s = math.cos(math.radians(sy)), math.sin(math.radians(sy))
    dx, dz = world[0] - px, world[2] - pz
    local = (dx * c - dz * s, world[1] - py, dx * s + dz * c)
    ry = (m18_rotation + 180.0) % 360.0 - 180.0
    put_asset(lists, name, model, (px, py, pz), sy, local, part_id=part_id)
    # put_asset turns the part by the street's yaw; the copy keeps m18's own rotation
    k = find(parts, name)
    e = bytearray(parts.entries[k])
    struct.pack_into("<3f", e, 0x2C, 0.0, ry, 0.0)
    parts.entries[k] = bytes(e)
    print(f"{name}: world {tuple(round(v, 3) for v in world)}, rotation y {ry}")


def fog_side_region(lists, door_position, door_yaw):
    """Add (or move) FOG_SIDE_REGION, the street-side box (see the module doc)."""
    from msb_add_indoor import insert_region
    from msb_floor import _set_name
    regions = _list(lists, "POINT_PARAM_ST").entries
    c, s = math.cos(math.radians(door_yaw)), math.sin(math.radians(door_yaw))
    lx, lz = (FOG_SIDE_LEFT + FOG_SIDE_RIGHT) / 2, (FOG_SIDE_NEAR + FOG_SIDE_FAR) / 2
    px, py, pz = door_position
    centre = (px + lx * c + lz * s, py - FOG_SIDE_BELOW, pz - lx * s + lz * c)
    size = (FOG_SIDE_RIGHT - FOG_SIDE_LEFT, FOG_SIDE_FAR - FOG_SIDE_NEAR, FOG_SIDE_HEIGHT)
    k = next((k for k, e in enumerate(regions) if entry_name(e) == FOG_SIDE_REGION), None)
    if k is None:
        template = next(e for e in regions if entry_name(e) == TEMPLATE_REGION)
        assert struct.unpack_from("<I", template, 0x08)[0] == OTHER and struct.unpack_from("<I", template, 0x10)[0] == 5
        e = bytearray(template)
        name_at, after = struct.unpack_from("<q", e, 0)[0], struct.unpack_from("<q", e, 0x30)[0]
        assert name_at + 2 * (len(FOG_SIDE_REGION) + 1) <= after, "name does not fit the template's name slot"
        _set_name(e, FOG_SIDE_REGION)
        struct.pack_into("<i", e, 0x0C, sum(struct.unpack_from("<I", r, 0x08)[0] == OTHER for r in regions))
        assert FOG_SIDE_REGION_ID not in {struct.unpack_from("<i", r, 0x2C)[0] for r in regions}
        struct.pack_into("<i", e, 0x2C, FOG_SIDE_REGION_ID)
        struct.pack_into("<i", e, struct.unpack_from("<q", e, 0x50)[0] + 4, FOG_SIDE_ENTITY)
    else:
        e = bytearray(regions[k])
    struct.pack_into("<3f", e, 0x14, *centre)
    struct.pack_into("<3f", e, 0x20, 0.0, door_yaw, 0.0)
    struct.pack_into("<3f", e, struct.unpack_from("<q", e, 0x48)[0], *size)
    if k is None:
        k = insert_region(lists, bytes(e), OTHER)
    else:
        regions[k] = bytes(e)
    print(f"{FOG_SIDE_REGION} (region {k}, entity {FOG_SIDE_ENTITY}): bottom centre "
          f"{tuple(round(v, 2) for v in centre)} rot {door_yaw:.2f} box {size}")


def fog_walls_no_collision(lists):
    """Round 33 (G8): the fog gate parts AEG099_001_9103 / 9104 (same part ids) become FOG_WALLS' AEG099_990 parts in
    place: renamed, model switched, model instance counts updated. Does nothing once done."""
    from msb_floor import _set_name, add_model
    parts = _list(lists, "PARTS_PARAM_ST")
    models = _list(lists, "MODEL_PARAM_ST").entries
    for name, part_id, _, _ in FOG_WALLS:
        old = FOG_WALL_OLD_MODEL + name[len(FOG_WALL_MODEL):]
        k = next((k for k, e in enumerate(parts.entries) if entry_name(e) == old), None)
        if k is None:
            continue
        e = bytearray(parts.entries[k])
        assert struct.unpack_from("<i", e, 0x08)[0] == part_id
        _set_name(e, name)
        struct.pack_into("<i", e, 0x14, add_model(lists, FOG_WALL_MODEL, FOG_WALL_OLD_MODEL))
        parts.entries[k] = bytes(e)
        print(f"{old} -> {name} (model {FOG_WALL_MODEL}, no collision)")
    for i, m in enumerate(models):
        if entry_name(m) in (FOG_WALL_MODEL, FOG_WALL_OLD_MODEL):
            m = bytearray(m)
            struct.pack_into("<i", m, 0x18, sum(struct.unpack_from("<i", x, 0x14)[0] == i for x in parts.entries))
            models[i] = bytes(m)


def place_fog(raw):
    header, lists = read_msb(raw)
    fog_walls_no_collision(lists)
    put_copy(lists, FOG_PART, FOG_MODEL, FOG_PART_ID, M18_POSITION, M18_ROTATION)
    parts = _list(lists, "PARTS_PARAM_ST")
    door = parts.entries[find(parts, DOOR)]
    position, yaw = struct.unpack_from("<3f", door, 0x20), struct.unpack_from("<3f", door, 0x2C)[1]
    placed = [(WALL_PART, WALL_MODEL, WALL_PART_ID, FOG_ENTITIES[WALL_PART], (0.0, DOOR_Y, DOOR_PHANTOM_Z)),
              (MIST_PART, MIST_MODEL, MIST_PART_ID, FOG_ENTITIES[MIST_PART], (0.0, DOOR_Y, DOOR_MIST_Z))]
    placed += [(name, FOG_WALL_MODEL, part_id, entity, (x, DOOR_Y, DOOR_WALL_Z)) for name, part_id, entity, x in FOG_WALLS]
    for name, model, part_id, entity, local in placed:
        assert all(struct.unpack_from("<i", e, 0x08)[0] != part_id or entry_name(e) == name for e in parts.entries)
        world = put_asset(lists, name, model, position, yaw, local, part_id=part_id, entity_id=entity)
        print(f"{name}: door frame {local}, world {tuple(round(v, 3) for v in world)}, rotation y {yaw:.2f}, "
              f"entity {entity}")
    fog_side_region(lists, position, yaw)
    return write_msb(header, lists)


if __name__ == "__main__":
    run(place_fog)
