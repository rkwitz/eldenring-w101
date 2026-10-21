"""Switch the Unicorn Way street between Wizard101's classic look and its current HD look.

STYLE = "classic" builds everything from the Classic Mode zone in one run, with no importing or exporting by hand:
  1. The FLVER, straight from reference/w101/converted_classic/WC_Z06_Unicorn_Way_Classic.obj (flver_build.py),
     split into one material per texture (flver_textures.py), into the Smithbox project's aeg030_518.geombnd.dcx.
  2. The classic textures, into project/asset/aet/aet030/aet030_518.tpf.dcx and its _l copy (tpf.py).
  3. The collision for the classic street, into aeg030_518_l.geomhkxbnd.dcx (hkx_collision.py).
The first classic run keeps the HD files as .hd copies; STYLE = "hd" puts them back.

Run in Blender's Scripting tab (it only uses Soulstruct for the game's compression); watch Window > Toggle System
Console. Building the collision takes about a minute. Relaunch the game afterwards.
"""

import importlib
import re
import shutil
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
import flver_floor  # noqa: E402
import flver_textures  # noqa: E402
import hkx_collision  # noqa: E402
import hkx_tag  # noqa: E402
import tpf  # noqa: E402

for module in (dds_color, flver_fix, flver_build, flver_floor, flver_textures, hkx_tag, hkx_collision, tpf):
    importlib.reload(module)

STYLE = "classic"  # "classic" or "hd"
# W101's colours look muted under Elden Ring's lighting: the classic textures are made this much more saturated and
# brighter (1.0 and 1.0 keep them as they are). See dds_color.py.
SATURATION = 1.3
BRIGHTNESS = 1.1

PROJECT = ROOT / "reference" / "eldenring" / "project" / "asset"
GAME = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset")
GEOMBND = PROJECT / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"
SINGLE_MESH = GEOMBND.with_name(GEOMBND.name + ".before_textures")  # our HD export: layout, bone and binder template
COLLISION = PROJECT / "aeg" / "aeg030" / "aeg030_518_l.geomhkxbnd.dcx"
TEXTURES = [PROJECT / "aet" / "aet030" / name for name in ("aet030_518.tpf.dcx", "aet030_518_l.tpf.dcx")]
VANILLA = GAME / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"
GAME_COLLISION = GAME / "aeg" / "aeg030" / COLLISION.name
AET_TEMPLATE = GAME / "aet" / "aet030" / "aet030_000.tpf.dcx"  # for its DCX type
CLASSIC = ROOT / "reference" / "w101" / "converted_classic"
# Your own edit of the classic street, exported from Blender (File > Export > Wavefront OBJ, default axes, with UVs
# and materials) under this name, is used instead of the converted original when it exists.
EDITED = CLASSIC / "WC_Z06_Unicorn_Way_Classic_edited.obj"
OBJ = EDITED if EDITED.exists() else CLASSIC / "WC_Z06_Unicorn_Way_Classic.obj"
MTL = OBJ.with_suffix(".mtl")
OUTPUTS = [GEOMBND, COLLISION, *TEXTURES]


def backdrop(positions, face):
    """The classic zone hides its edges behind flat, untextured boards that run from y -5 to 22.27 (27 m tall): in
    Elden Ring they show as grey walls around the street, and their collision blocks the way, so they are left out."""
    points = [positions[v] for v, _ in face[1]]
    ys = [p[1] for p in points]
    spread = lambda axis: max(p[axis] for p in points) - min(p[axis] for p in points)
    flat = spread(0) < 0.01 or spread(2) < 0.01  # not rounding: one board's corners are 115.6249 and 115.6252
    return flat and min(ys) < -4.9 and max(ys) > 22.2


def tunnel_inside(positions, face):
    """The inside of the archway to the Commons (behind its front wall at z 10.91): its ramp and walls would stick into
    the Stranded Graveyard's lift landing, because the street is placed with the graveyard door in that archway."""
    points = [positions[v] for v, _ in face[1]]
    x, y, z = (sum(p[i] for p in points) / 3 for i in range(3))
    return abs(x) < 4.0 and 11.35 < z < 16.0 and y < 5.5


def hd_copy(path):
    return path.with_name(path.name + ".hd")


def build_classic():
    start = time.time()
    for path in OUTPUTS:
        if not hd_copy(path).exists():
            shutil.copy2(path, hd_copy(path))

    template = flver_fix.flver_in_binder(SINGLE_MESH)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    print(f"Building from {OBJ.name}")
    positions, uvs, faces = flver_build.parse_obj(OBJ)
    walls = [f for f in faces if backdrop(positions, f)]
    # Your own edit decides for itself what's left of the archway; only the converted original gets it cut.
    cut = (lambda f: False) if OBJ == EDITED else (lambda f: tunnel_inside(positions, f))
    tunnel = [f for f in faces if cut(f)]
    faces = [f for f in faces if not backdrop(positions, f) and not cut(f)]
    print(f"Left out {len(walls)} triangles of the zone's backdrop walls and {len(tunnel)} inside the Commons archway")
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces)
    texture_of_material = flver_textures.parse_mtl(MTL)
    if OBJ == EDITED:
        # Blender may drop a texture it couldn't load or rename a material "name.001"; the original MTL fills in.
        original = flver_textures.parse_mtl(CLASSIC / "WC_Z06_Unicorn_Way_Classic.mtl")
        for name in {m for m, _ in faces} - set(texture_of_material):
            base = re.sub(r"\.\d+$", "", name or "")
            if base in original:
                texture_of_material[name] = original[base]
        missing = {m for m, _ in faces} - set(texture_of_material)
        if missing:
            print(f"No texture for {len(missing)} materials of your edit: {sorted(m or '' for m in missing)[:5]}")
    stems = sorted(set(texture_of_material.values()))
    dds = [dds_color.boost((CLASSIC / "textures" / f"{stem}.dds").read_bytes(), SATURATION, BRIGHTNESS)
           for stem in stems]
    alpha = {i for i, data in enumerate(dds) if tpf.has_alpha(data)}
    face_textures = [stems.index(texture_of_material[m]) if m in texture_of_material else None for m in materials]
    split = flver_textures.split_materials(single, vanilla, face_textures, stems, alpha)
    binder = Binder.from_path(SINGLE_MESH)
    entry = next(e for e in binder.entries if e.name.lower().endswith(".flver"))
    entry.set_uncompressed_data(split)
    binder.write(GEOMBND)
    print(f"Wrote {GEOMBND}: {len(faces)} triangles, {len(stems)} textures ({len(alpha)} with transparency)")

    textures = [(flver_textures.texture_name(i), data) for i, data in enumerate(dds)]
    textures.append((flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, 64)))
    _, dcx_type = decompress(AET_TEMPLATE)
    packed = compress(tpf.build_tpf(textures), dcx_type)
    for path in TEXTURES:
        path.write_bytes(packed)
    print(f"Wrote {len(textures)} textures to {TEXTURES[0]} (and _l)")

    game_binder = Binder.from_path(GAME_COLLISION)
    hkx = next(e for e in game_binder.entries if e.name.lower().endswith(".hkx")).get_uncompressed_data()
    triangles = list(flver_floor._triangles(split))
    data, _ = hkx_collision.build_hkx(hkx, triangles)
    bnd, dcx_type = decompress(GAME_COLLISION)
    COLLISION.write_bytes(compress(hkx_collision.replace_binder_entry(bnd, data), dcx_type))
    print(f"Wrote collision for {len(triangles)} triangles to {COLLISION}")
    print(f"Classic Unicorn Way is in place ({time.time() - start:.0f} s). Relaunch the game.")


def restore_hd():
    missing = [p for p in OUTPUTS if not hd_copy(p).exists()]
    if missing:
        raise RuntimeError(f"No HD copy of {missing[0].name}: the street has never been switched to classic.")
    for path in OUTPUTS:
        shutil.copy2(hd_copy(path), path)
    print("HD Unicorn Way is back in place. Relaunch the game.")


if STYLE == "classic":
    build_classic()
elif STYLE == "hd":
    restore_hd()
else:
    raise ValueError(f"STYLE must be 'classic' or 'hd', not {STYLE!r}")
