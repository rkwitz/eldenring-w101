"""Build Rattlebones' tower room (Wizard101 zone WizardCity/WC_Streets/Interiors/WC_Unicorn_T2) as Elden Ring asset
aeg030_519, with its textures and collision, in one run.

W101's Unicorn Way "Street1 Tower2" door leads to zone WC_Unicorn_T2, whose geometry is the shared Wizard City tower
room WC_Generic_Gauntlet_NoDoor.nif (a round library room, about 26 m across and 12 m high, one 8-player duel sigil in
the middle). Its Classic Mode version is converted (nif2obj.py, tex2dds.py) to
reference/w101/converted_rattlebones_classic/WC_Generic_Gauntlet_NoDoor_Classic.obj and textures/.

Writes, in the Smithbox project (the first run keeps nothing: aeg030_519 is otherwise vanilla, a Caria Manor road
piece that our maps don't use; deleting these files gives it back):
  asset/aeg/aeg030/aeg030_519.geombnd.dcx        the room FLVER, origin = room centre on the floor (W101 0, 0, 0)
  asset/aet/aet030/aet030_519.tpf.dcx (and _l)    its 18 classic textures
  asset/aeg/aeg030/aeg030_519_l.geomhkxbnd.dcx    collision (from the vanilla 519 binder): walls, floor, columns and
                                                  shelf tops, the small props left out, and a smooth invisible ring
                                                  wall in front of the bookshelves (see collision_triangles)
W101's light beams (Light_rays*: additive beams from the windows to the floor, drawn with the window texture) are
left out: drawn solid they looked like glass tubes. Round 25 B1 (Ryan: "W101 is warmer/brighter wood floor,
stained-glass windows with coloured light shafts, softer", ref 8_w101_boss_room.png): the coloured light spots the
beams leave on the floor (window_floor_*) are baked into an atlas with the floor under them (room_light.py), the
stained glass (Window01) glows brighter than the walls, the wood floor gets Wizard101's paler yellow-olive tint and
every texture is less saturated. The collision also gets a catch floor 2 m under the room (see collision_triangles). The jars' glass is half transparent, so their
texture gets the alpha-blended material (flver_textures.blend_material) instead of the alpha-tested one.
Place it with msb_add_room.py. Run in Blender's Scripting tab like blender_switch_street.py (it only uses Soulstruct
for the game's compression and binder), then relaunch the game.
"""

import os
import importlib
import math
import re
import sys
import time
from pathlib import Path

from soulstruct.containers import Binder
from soulstruct.dcx import compress, decompress

ROOT = Path(os.environ.get("W101ER_ROOT", Path.home() / "Documents" / "game_modding"))
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import dds_color  # noqa: E402
import room_light  # noqa: E402
import flver_build  # noqa: E402
import flver_fix  # noqa: E402
import flver_textures  # noqa: E402
import hkx_collision  # noqa: E402
import hkx_tag  # noqa: E402
import tpf  # noqa: E402

for module in (dds_color, flver_fix, flver_build, flver_textures, hkx_tag, hkx_collision, tpf, room_light):
    importlib.reload(module)

ASSET = "aeg030_519"
flver_textures.AET = ASSET.replace("aeg", "AET").upper()
flver_textures.FLAT_NORMAL = f"{flver_textures.AET}_n"
SATURATION, BRIGHTNESS = 1.2, 1.0  # sRGB textures (tpf.build_tpf). Ryan 2026-10-27: true colours looked right but "crank
# up the saturation" (1.4); round 25 B1 next to Wizard101's own screenshot: "softer" (our walls S 0.26 vs W101 0.13)
# Round 25 B1: per-texture grade (saturation, (r, g, b) gains) instead of SATURATION. The wood floor (texture mean
# 157/121/71, orange in game: 111/87/63) to Wizard101's screenshot floor, pale yellow-olive (115/115/75), ~10% lighter.
GRADES = {"WC_Floor_Wood_01": (1.0, (0.97, 1.16, 1.2))}
GLASS_GLOW = 2.5  # the stained glass (Window01) glows this many times the walls (UNLIT_GLOW)
SPOT_GAIN = 1.25  # floor light spots: floor + window colour x vertex colour x this (Wizard101 adds it at 1.0)
GLASS_OBJECT, FLOOR_OBJECT, SPOTS = "Window01", "Object01", re.compile(r"(?i)window_floor")

PROJECT = ROOT / "reference" / "eldenring" / "project" / "asset"
GAME = Path(os.environ.get("ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game")) / r"asset"
STREET_GEOMBND = PROJECT / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx.before_textures"  # single-mesh FLVER template
VANILLA = GAME / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"  # its plain lit materials
GAME_COLLISION = GAME / "aeg" / "aeg030" / f"{ASSET}_l.geomhkxbnd.dcx"
# Wizard101 draws its rooms unlit (baked colours), and underground Elden Ring gives them almost no light: Lady
# Blackhope's room was "really dark", the walls unlit (2026-10-27). UNLIT_GLOW > 0 draws every opaque texture with
# the emissive material (flver_textures.emissive_material: black albedo, the texture as light), like the sky:
# 0.0446 = the sky's SKY_GLOW 0.0743 x SKY_BRIGHTNESS 0.6, so walls and sky keep Wizard101's brightness ratio at the
# exposure Ryan picked for the sky. Raise it if still dark, lower if glaring; 0 = lit by Elden Ring's lights again.
# 2026-10-27 round 9 "map still too bright": 0.035, down ~21% with the world lights (blender_build_sky AMBIENT/OVERHEAD).
UNLIT_GLOW = 0.04  # round 25 B1 "brighter" (+14%)
EMISSIVE_VANILLA = GAME.parent / flver_textures.EMISSIVE_ASSET  # the vanilla emissive material we copy
AET_TEMPLATE = GAME / "aet" / "aet030" / "aet030_000.tpf.dcx"  # for its DCX type
GEOMBND = PROJECT / "aeg" / "aeg030" / f"{ASSET}.geombnd.dcx"
COLLISION = PROJECT / "aeg" / "aeg030" / GAME_COLLISION.name
TEXTURES = [PROJECT / "aet" / "aet030" / f"{ASSET.replace('aeg', 'aet')}{suffix}.tpf.dcx" for suffix in ("", "_l")]
ROOM = ROOT / "reference" / "w101" / "converted_rattlebones_classic"
OBJ = ROOM / "WC_Generic_Gauntlet_NoDoor_Classic.obj"

LIGHT_SHAFTS = re.compile(r"(?i)light_rays")  # OBJ objects (nif meshes) left out of the room
PROPS = re.compile(r"(?i)book|scroll|candle|paperstack|skull|jar|editable poly")  # drawn, but no collision
# Players got stuck in the gaps between and behind the bookshelves, which stand off the wall (shelf fronts at 11.3 to
# 11.7 m from the centre, wall at 12.3 to 13 m). Collision there is one smooth ring wall instead, up to RING_TOP, with
# an opening for the door corridor (OBJ +z, |x| < DOOR_HALF_WIDTH) closed off at the sides.
RING_RADIUS, RING_TOP, RING_SEGMENTS, DOOR_HALF_WIDTH, WALL_RADIUS = 11.2, 3.0, 96, 1.15, 13.3
SHELF_RADIUS, SHELF_TOP = 11.0, 2.7  # Tower_Shell_A triangles beyond this radius and below this height: the shelves
# Round 25 B2: Rattlebones (Giant Skeleton Torso) dropped through the floor into the empty space under the room and
# never came back. A catch floor this far under the room floor, out to CATCH_RADIUS (past the outer wall), stops a
# character that falls through or is pushed out of the room; emevd.boss_watchdog then puts him back on the sigil.
CATCH_DEPTH, CATCH_RADIUS = 2.0, 16.0


def face_objects(path):
    """The OBJ object ("o" name) of every triangle flver_build.parse_obj returns, in the same order."""
    names, name = [], None
    with open(path) as f:
        for line in f:
            if line.startswith("o "):
                name = line[2:].strip()
            elif line.startswith("f "):
                names += [name] * (len(line.split()) - 3)
    return names


def collision_triangles(positions, faces):
    """FLVER-space triangles (OBJ (x, y, -z), as flver_build writes vertices) for the room's collision: every
    triangle of `faces` except props and the bookshelves, plus the ring wall (both windings)."""
    def flver(p):
        return (p[0], p[1], -p[2])

    triangles = []
    for (_, name), corners in faces:
        if PROPS.search(name) or LIGHT_SHAFTS.search(name) or SPOTS.search(name):
            continue
        points = [positions[v] for v, _ in corners]
        if name == "Tower_Shell_A" and max(p[1] for p in points) < SHELF_TOP and \
                min(math.hypot(p[0], p[2]) for p in points) > SHELF_RADIUS:
            continue
        triangles.append(tuple(flver(p) for p in points))

    def quad(a, b):  # vertical wall from floor to RING_TOP between OBJ (x, z) points a and b, both windings
        corners = [(a[0], 0.0, a[1]), (b[0], 0.0, b[1]), (b[0], RING_TOP, b[1]), (a[0], RING_TOP, a[1])]
        corners = [flver(c) for c in corners]
        for tri in ((0, 1, 2), (0, 2, 3)):
            triangles.append(tuple(corners[i] for i in tri))
            triangles.append(tuple(corners[i] for i in reversed(tri)))

    gap = math.asin(DOOR_HALF_WIDTH / RING_RADIUS)  # angle from OBJ +z (the door) to the ring's ends
    angles = [gap + (2 * math.pi - 2 * gap) * k / RING_SEGMENTS for k in range(RING_SEGMENTS + 1)]
    ring = [(RING_RADIUS * math.sin(a), RING_RADIUS * math.cos(a)) for a in angles]
    for a, b in zip(ring, ring[1:]):
        quad(a, b)
    for end in (ring[0], ring[-1]):  # close the space behind the shelves off from the door corridor
        quad(end, (end[0], math.sqrt(WALL_RADIUS ** 2 - end[0] ** 2)))
    catch = [(CATCH_RADIUS * math.cos(2 * math.pi * k / 32), -CATCH_DEPTH, CATCH_RADIUS * math.sin(2 * math.pi * k / 32))
             for k in range(32)]
    for a, b in zip(catch, catch[1:] + catch[:1]):  # a flat disc (fan from the centre), both windings
        tri = (flver((0.0, -CATCH_DEPTH, 0.0)), flver(a), flver(b))
        triangles += [tri, tri[::-1]]
    return triangles


def build_room():
    start = time.time()
    template = flver_fix.flver_in_binder(STREET_GEOMBND)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    positions, uvs, faces = flver_build.parse_obj(OBJ)
    faces = [((material, name), corners) for (material, corners), name in zip(faces, face_objects(OBJ))]
    collision_faces = faces
    texture_of_material = flver_textures.parse_mtl(OBJ.with_suffix(".mtl"))
    stems = sorted(set(texture_of_material.values()))

    def graded(stem):
        saturation, gains = next((g for key, g in GRADES.items() if stem.startswith(key)), (SATURATION, (1, 1, 1)))
        return saturation, gains

    dds = [dds_color.boost((ROOM / "textures" / f"{stem}.dds").read_bytes(), graded(stem)[0], BRIGHTNESS,
                           graded(stem)[1]) for stem in stems]

    # Round 25 B1: the floor light spots, baked with the floor under them into one more texture (SPOT_TEXTURE)
    floor_stem = next(s for s in stems if s.startswith("WC_Floor_Wood_01"))
    window_stem = next(s for s in stems if s.startswith("WC_Window_WizardTower_01"))
    floor_img = room_light.dxt1_decode((ROOM / "textures" / f"{floor_stem}.dds").read_bytes())
    floor_img = room_light.grade(floor_img, *graded(floor_stem))
    window_img = room_light.dxt1_decode((ROOM / "textures" / f"{window_stem}.dds").read_bytes())
    spot_names = {name for (_, name), _ in faces if SPOTS.search(name)}
    atlas, spot_uvs, spot_offset = room_light.bake_floor_spots(
        positions, uvs, room_light.obj_vertex_colours(OBJ), faces, spot_names, FLOOR_OBJECT, floor_img, window_img,
        gain=SPOT_GAIN)
    base = len(uvs)
    uvs = list(uvs) + spot_uvs
    faces = [(key, tuple((v, base + spot_offset[v]) for v, _ in corners)) if key[1] in spot_names else (key, corners)
             for key, corners in faces]
    GLASS_TEXTURE, SPOT_TEXTURE = len(stems), len(stems) + 1
    dds += [dds[stems.index(window_stem)], room_light.dxt1_dds(atlas)]
    names = stems + ["glass", "floor_spots"]

    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces)
    alpha = {i for i, data in enumerate(dds) if tpf.has_alpha(data)}  # only the jars' textures

    def texture_of(m, name):
        if name in spot_names:
            return SPOT_TEXTURE
        if m not in texture_of_material or LIGHT_SHAFTS.search(name):
            return None
        return GLASS_TEXTURE if name == GLASS_OBJECT else stems.index(texture_of_material[m])

    face_textures = [texture_of(m, name) for m, name in materials]
    opaque, gx = None, None
    if UNLIT_GLOW > 0:
        opaque = flver_textures.emissive_material(vanilla, flver_fix.flver_in_binder(EMISSIVE_VANILLA), UNLIT_GLOW)
        gx = {GLASS_TEXTURE: flver_textures.set_gxmd(opaque["gx"], flver_textures.EMISSIVE_GLOW_PARAM,
                                                     [UNLIT_GLOW * GLASS_GLOW])}
    flver = flver_textures.split_materials(single, vanilla, face_textures, names, alpha, opaque=opaque,
                                           blend_material=flver_textures.blend_material(vanilla), gx=gx)

    binder = Binder.from_path(STREET_GEOMBND)
    for entry in binder.entries:
        entry.path = re.sub(r"(?i)aeg030_518", ASSET.upper(), entry.path)
        if entry.name.lower().endswith(".flver"):
            entry.set_uncompressed_data(flver)
    binder.write(GEOMBND)
    drawn = sum(t is not None for t in face_textures)
    print(f"Wrote {GEOMBND}: {drawn} triangles, {len(names)} textures ({len(alpha)} alpha blended), "
          f"{len(spot_names)} floor light spots baked, glass glow {UNLIT_GLOW * GLASS_GLOW:.3f}")

    textures = [(flver_textures.texture_name(i), data) for i, data in enumerate(dds)]
    textures.append((flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, 64)))
    textures.append((flver_textures.black_name(), tpf.solid_dds(0, 0, 0)))  # the emissive material's albedo
    _, dcx_type = decompress(AET_TEMPLATE)
    packed = compress(tpf.build_tpf(textures), dcx_type)
    for path in TEXTURES:
        path.write_bytes(packed)
    print(f"Wrote {len(textures)} textures to {TEXTURES[0]} (and _l)")

    game_binder = Binder.from_path(GAME_COLLISION)
    hkx = next(e for e in game_binder.entries if e.name.lower().endswith(".hkx")).get_uncompressed_data()
    triangles = collision_triangles(positions, collision_faces)
    data, _ = hkx_collision.build_hkx(hkx, triangles)
    bnd, dcx_type = decompress(GAME_COLLISION)
    COLLISION.write_bytes(compress(hkx_collision.replace_binder_entry(bnd, data), dcx_type))
    print(f"Wrote collision for {len(triangles)} triangles to {COLLISION}")
    print(f"Rattlebones' room is built ({time.time() - start:.0f} s). Place it with msb_add_room.py, then relaunch.")


build_room()
