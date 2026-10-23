"""Build Unicorn Way's hedge-maze garden as Elden Ring asset aeg030_555 (with collision), and the glowing "quest inside"
windows of Rattlebones' tower as asset aeg030_529, in one run. Place both with msb_add_garden.py.

Garden: the double door at the west end of Unicorn Way leads to zone WizardCity/WC_Streets/Interiors/
WC_Unicorn_HedgeMaze, whose classic geometry WC_Z06I02_Life_Hedge_Maze.nif (ClassicMode-WC_Streets-Interiors-
WC_Unicorn_HedgeMaze.wad, textures there and in ClassicMode-WorldData.wad) is converted (nif2obj.py, tex2dds.py) to
reference/w101/converted_hedgemaze_classic/WC_Unicorn_HedgeMaze_Classic.obj and textures/. About 80 m across (hedges,
a unicorn statue, the Life school entrance arch, flower beds), framed by a cliff, a city wall tower and giant backdrop
trees up to 500 m away and 192 m tall (cut off at CLIP_TOP, see below). Butterfly cards (animated in W101) and the untextured "Bone" meshes are left
out. Collision covers every drawn triangle except flowers, flower-bed decals and the tree cards (NO_COLLISION).

Window: Wizard101 shows a building's WC_Unicorn_<zone>_quest window mesh (WC_Window03_JT, a blue copy of the window
texture that is also its own glow map) while the player has a quest goal inside; see w101_quest_window.py, which cuts
Rattlebones' tower's 14 arched windows out of the classic street nif into reference/w101/converted_quest_window/
WC_Unicorn_T2_quest.obj, 3 cm in front of the street's own copies. The asset draws them with the emissive material
(flver_textures.emissive_material: black albedo, the texture as emitted light), so they glow day and night.

Writes, in the Smithbox project (aeg030_555 and aeg030_529 are otherwise vanilla pieces our maps don't use; deleting
these files gives them back):
  asset/aeg/aeg030/aeg030_555.geombnd.dcx, aeg030_555_l.geomhkxbnd.dcx   the garden FLVER and its collision
  asset/aet/aet030/aet030_555.tpf.dcx (and _l)                           its classic textures
  asset/aeg/aeg030/aeg030_529.geombnd.dcx                                the windows (no collision)
  asset/aet/aet030/aet030_529.tpf.dcx (and _l)                           the window texture
Run in Blender's Scripting tab like blender_build_room.py (it only uses Soulstruct for the game's compression and
binder), then relaunch the game.
"""

import importlib
import re
import sys
import time
from pathlib import Path

from soulstruct.containers import Binder
from soulstruct.dcx import compress, decompress

ROOT = Path.home() / "Documents" / "game_modding"
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import dds_color  # noqa: E402
import flver_build  # noqa: E402
import flver_fix  # noqa: E402
import flver_textures  # noqa: E402
import hkx_collision  # noqa: E402
import hkx_tag  # noqa: E402
import tpf  # noqa: E402

for module in (dds_color, flver_fix, flver_build, flver_textures, hkx_tag, hkx_collision, tpf):
    importlib.reload(module)

SATURATION, BRIGHTNESS = 1.3, 1.1  # same colour boost as the classic street (dds_color.py)
# The glowing window: the emissive material has no strength setting, and Elden Ring's exposure and bloom turn emitted
# light near full brightness white (Ryan's test 2026-10-23: "the light is white, it should be blue"). The texture is
# darkened to a quarter and saturated instead, which keeps W101's pale blue panes (rendered comparison, 0.22-0.25).
WINDOW_SATURATION, WINDOW_BRIGHTNESS = 1.8, 0.25
# The garden sits msb_add_garden.GARDEN_DEPTH (50) m under the street, so nothing of it may reach the street's ground:
# its giant backdrop (cliff, city wall tower, trees up to 192 m tall) is cut off at CLIP_TOP m above the garden floor.
CLIP_TOP = 48.0

PROJECT = ROOT / "reference" / "eldenring" / "project" / "asset"
GAME = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset")
STREET_GEOMBND = PROJECT / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx.before_textures"  # single-mesh FLVER template
VANILLA = GAME / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"  # its plain lit materials
AET_TEMPLATE = GAME / "aet" / "aet030" / "aet030_000.tpf.dcx"  # for its DCX type
W101 = ROOT / "reference" / "w101"
GARDEN_OBJ = W101 / "converted_hedgemaze_classic" / "WC_Unicorn_HedgeMaze_Classic.obj"
WINDOW_OBJ = W101 / "converted_quest_window" / "WC_Unicorn_T2_quest.obj"

LEFT_OUT = re.compile(r"(?i)^bone|butterfly")  # OBJ objects or materials not drawn
NO_COLLISION = re.compile(r"(?i)flower|design|dirt|^tree0")  # drawn, but walk-through


def use_asset(asset):
    flver_textures.AET = asset.replace("aeg", "AET").upper()
    flver_textures.FLAT_NORMAL = f"{flver_textures.AET}_n"
    return {
        "geombnd": PROJECT / "aeg" / "aeg030" / f"{asset}.geombnd.dcx",
        "collision": PROJECT / "aeg" / "aeg030" / f"{asset}_l.geomhkxbnd.dcx",
        "game_collision": GAME / "aeg" / "aeg030" / f"{asset}_l.geomhkxbnd.dcx",
        "textures": [PROJECT / "aet" / "aet030" / f"{asset.replace('aeg', 'aet')}{s}.tpf.dcx" for s in ("", "_l")],
    }


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


def clip_below(positions, uvs, faces, top):
    """(positions, uvs, faces) with every triangle cut off at height `top` (OBJ y): parts above it are dropped,
    triangles crossing it are clipped (new corners interpolated in position and UV) and fanned back into triangles.
    `faces` are flver_build.parse_obj triangles [(key, ((v, vt), (v, vt), (v, vt)))]; the keys are kept."""
    positions, uvs, out = list(positions), list(uvs), []
    for key, corners in faces:
        heights = [positions[v][1] for v, _ in corners]
        if max(heights) <= top:
            out.append((key, corners))
            continue
        polygon = []
        for k in range(3):
            a, b = corners[k], corners[(k + 1) % 3]
            ya, yb = positions[a[0]][1], positions[b[0]][1]
            if ya <= top:
                polygon.append(a)
            if (ya <= top) != (yb <= top):
                t = (top - ya) / (yb - ya)
                pa, pb = positions[a[0]], positions[b[0]]
                positions.append(tuple(pa[i] + (pb[i] - pa[i]) * t for i in range(3)))
                uv = None
                if a[1] is not None and b[1] is not None:
                    ua, ub = uvs[a[1]], uvs[b[1]]
                    uvs.append(tuple(ua[i] + (ub[i] - ua[i]) * t for i in range(2)))
                    uv = len(uvs) - 1
                polygon.append((len(positions) - 1, uv))
        for k in range(1, len(polygon) - 1):
            out.append((key, (polygon[0], polygon[k], polygon[k + 1])))
    return positions, uvs, out


def write_asset(paths, asset, flver, textures):
    binder = Binder.from_path(STREET_GEOMBND)
    for entry in binder.entries:
        entry.path = re.sub(r"(?i)aeg030_518", asset.upper(), entry.path)
        if entry.name.lower().endswith(".flver"):
            entry.set_uncompressed_data(flver)
    binder.write(paths["geombnd"])
    _, dcx_type = decompress(AET_TEMPLATE)
    packed = compress(tpf.build_tpf(textures), dcx_type)
    for path in paths["textures"]:
        path.write_bytes(packed)
    print(f"Wrote {paths['geombnd']} and {len(textures)} textures to {paths['textures'][0]} (and _l)")


def build_garden(asset="aeg030_555"):
    paths = use_asset(asset)
    template = flver_fix.flver_in_binder(STREET_GEOMBND)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    positions, uvs, faces = flver_build.parse_obj(GARDEN_OBJ)
    faces = [((material, name), corners) for (material, corners), name in zip(faces, face_objects(GARDEN_OBJ))]
    count = len(faces)
    positions, uvs, faces = clip_below(positions, uvs, faces, CLIP_TOP)
    print(f"  {count} garden triangles, {len(faces)} after cutting the backdrop off at {CLIP_TOP} m")
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces)
    texture_of_material = flver_textures.parse_mtl(GARDEN_OBJ.with_suffix(".mtl"))
    stems = sorted({t for m, t in texture_of_material.items() if not LEFT_OUT.search(m)})
    folder = GARDEN_OBJ.parent / "textures"
    dds = [dds_color.boost((folder / f"{stem}.dds").read_bytes(), SATURATION, BRIGHTNESS) for stem in stems]
    alpha = {i for i, data in enumerate(dds) if tpf.has_alpha(data)}  # hedges, trees, flowers: alpha-tested
    face_textures = [stems.index(texture_of_material[m]) if m in texture_of_material and texture_of_material[m] in stems
                     and not LEFT_OUT.search(name or "") else None for m, name in materials]
    flver = flver_textures.split_materials(single, vanilla, face_textures, stems, alpha)
    textures = [(flver_textures.texture_name(i), data) for i, data in enumerate(dds)]
    textures.append((flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, 64)))
    write_asset(paths, asset, flver, textures)
    print(f"  {sum(t is not None for t in face_textures)} triangles drawn, {len(alpha)} alpha-tested textures")

    triangles = [tuple((positions[v][0], positions[v][1], -positions[v][2]) for v, _ in corners)
                 for ((_, name), corners), t in zip(faces, face_textures)
                 if t is not None and not NO_COLLISION.search(name or "")]
    game_binder = Binder.from_path(paths["game_collision"])
    hkx = next(e for e in game_binder.entries if e.name.lower().endswith(".hkx")).get_uncompressed_data()
    data, _ = hkx_collision.build_hkx(hkx, triangles)
    bnd, dcx_type = decompress(paths["game_collision"])
    paths["collision"].write_bytes(compress(hkx_collision.replace_binder_entry(bnd, data), dcx_type))
    print(f"Wrote collision for {len(triangles)} triangles to {paths['collision']}")


def build_window(asset="aeg030_529"):
    paths = use_asset(asset)
    template = flver_fix.flver_in_binder(STREET_GEOMBND)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    positions, uvs, faces = flver_build.parse_obj(WINDOW_OBJ)
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces)
    (stem,) = set(flver_textures.parse_mtl(WINDOW_OBJ.with_suffix(".mtl")).values())
    texture = W101 / "converted_classic" / "textures" / f"{stem}.dds"  # the street's converted copy
    dds = dds_color.boost(texture.read_bytes(), WINDOW_SATURATION, WINDOW_BRIGHTNESS)
    flver = flver_textures.split_materials(single, vanilla, [0] * len(materials), [stem], alpha=set(),
                                           opaque=flver_textures.emissive_material(vanilla))
    textures = [(flver_textures.texture_name(0), dds), (flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, 64)),
                (flver_textures.black_name(), tpf.solid_dds(0, 0, 0))]
    write_asset(paths, asset, flver, textures)
    print(f"  {len(materials)} glowing window triangles")


start = time.time()
build_garden()
build_window()
print(f"Garden and window built ({time.time() - start:.0f} s). Place them with msb_add_garden.py, then relaunch.")
