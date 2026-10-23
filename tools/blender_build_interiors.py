"""Build the empty boss interiors of Unicorn Way that need their own asset; place them with msb_add_interiors.py.

Fear of Fire's house (W101 zone WizardCity/WC_Streets/Interiors/WC_Unicorn_H4) uses the Alchemy clubhouse,
SP_Z00I01_ClubHouse.nif (HD only: WizardCity-WorldData.wad; no classic copy was found), converted (nif2obj.py,
tex2dds.py) to reference/w101/converted_clubhouse_hd/WC_Unicorn_H4_ClubHouse.obj and textures/: a 37 m wide hall with
a raised back room, about 77 m deep and 18 m high, door at FLVER +z. Left out: camera volumes (CamSpace*), the untextured
duel circle guides, window sun-ray and shadow cards (FX_*, Shadow_*), candle flames, and the two potion meshes whose
texture (Potion_A01) is not in the game files. No collision for cobwebs, candles, jars, potions, scrolls and the sink.
Lady Blackhope's tower (WC_Unicorn_T1) is the same room as Rattlebones' (aeg030_519, blender_build_room.py), so it
needs no build.

Writes, in the Smithbox project (aeg030_557 is otherwise a vanilla piece our maps don't use; deleting these files gives
it back):
  asset/aeg/aeg030/aeg030_557.geombnd.dcx, aeg030_557_l.geomhkxbnd.dcx   the house FLVER and its collision
  asset/aet/aet030/aet030_557.tpf.dcx (and _l)                           its textures
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

SATURATION, BRIGHTNESS = 1.3, 1.1  # same colour boost as the street and Rattlebones' room

PROJECT = ROOT / "reference" / "eldenring" / "project" / "asset"
GAME = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset")
STREET_GEOMBND = PROJECT / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx.before_textures"  # single-mesh FLVER template
VANILLA = GAME / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"  # its plain lit materials
AET_TEMPLATE = GAME / "aet" / "aet030" / "aet030_000.tpf.dcx"  # for its DCX type
HOUSE_OBJ = ROOT / "reference" / "w101" / "converted_clubhouse_hd" / "WC_Unicorn_H4_ClubHouse.obj"

LEFT_OUT = re.compile(r"(?i)^camspace|^duelingcircle|^fx_|^shadow_|^editable poly$|^polymesh$")  # OBJ objects
NO_COLLISION = re.compile(r"(?i)cobweb|candle|jar_|potion|scroll|plane005|sink|^stool_0\d+:14")  # drawn, walk-through


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


def build_interior(asset, obj):
    start = time.time()
    flver_textures.AET = asset.replace("aeg", "AET").upper()
    flver_textures.FLAT_NORMAL = f"{flver_textures.AET}_n"
    geombnd = PROJECT / "aeg" / "aeg030" / f"{asset}.geombnd.dcx"
    collision = PROJECT / "aeg" / "aeg030" / f"{asset}_l.geomhkxbnd.dcx"
    game_collision = GAME / "aeg" / "aeg030" / collision.name
    texture_files = [PROJECT / "aet" / "aet030" / f"{asset.replace('aeg', 'aet')}{s}.tpf.dcx" for s in ("", "_l")]

    template = flver_fix.flver_in_binder(STREET_GEOMBND)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    positions, uvs, faces = flver_build.parse_obj(obj)
    faces = [((material, name), corners) for (material, corners), name in zip(faces, face_objects(obj))]
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces)
    folder = obj.parent / "textures"
    texture_of_material = {m: t for m, t in flver_textures.parse_mtl(obj.with_suffix(".mtl")).items()
                           if (folder / f"{t}.dds").exists()}
    kept = [m is not None and m in texture_of_material and not LEFT_OUT.search(name or "") for m, name in materials]
    stems = sorted({texture_of_material[m] for (m, _), k in zip(materials, kept) if k})
    dds = [dds_color.boost((folder / f"{stem}.dds").read_bytes(), SATURATION, BRIGHTNESS) for stem in stems]
    alpha = {i for i, data in enumerate(dds) if tpf.has_alpha(data)}  # cobwebs, glass, scrolls: alpha-tested
    face_textures = [stems.index(texture_of_material[m]) if k else None for (m, _), k in zip(materials, kept)]
    flver = flver_textures.split_materials(single, vanilla, face_textures, stems, alpha)

    binder = Binder.from_path(STREET_GEOMBND)
    for entry in binder.entries:
        entry.path = re.sub(r"(?i)aeg030_518", asset.upper(), entry.path)
        if entry.name.lower().endswith(".flver"):
            entry.set_uncompressed_data(flver)
    binder.write(geombnd)
    textures = [(flver_textures.texture_name(i), data) for i, data in enumerate(dds)]
    textures.append((flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, 64)))
    _, dcx_type = decompress(AET_TEMPLATE)
    packed = compress(tpf.build_tpf(textures), dcx_type)
    for path in texture_files:
        path.write_bytes(packed)
    print(f"Wrote {geombnd}: {sum(t is not None for t in face_textures)} triangles, {len(stems)} textures "
          f"({len(alpha)} alpha-tested), textures in {texture_files[0]} (and _l)")

    triangles = [tuple((positions[v][0], positions[v][1], -positions[v][2]) for v, _ in corners)
                 for ((_, name), corners), t in zip(faces, face_textures)
                 if t is not None and not NO_COLLISION.search(name or "")]
    game_binder = Binder.from_path(game_collision)
    hkx = next(e for e in game_binder.entries if e.name.lower().endswith(".hkx")).get_uncompressed_data()
    data, _ = hkx_collision.build_hkx(hkx, triangles)
    bnd, dcx_type = decompress(game_collision)
    collision.write_bytes(compress(hkx_collision.replace_binder_entry(bnd, data), dcx_type))
    print(f"Wrote collision for {len(triangles)} triangles to {collision} ({time.time() - start:.0f} s)")


build_interior("aeg030_557", HOUSE_OBJ)
print("Fear of Fire's house is built. Place it with msb_add_interiors.py, then relaunch.")
