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
  asset/aeg/aeg030/aeg030_519_l.geomhkxbnd.dcx    collision for every triangle (from the vanilla 519 binder)
Place it with msb_add_room.py. Run in Blender's Scripting tab like blender_switch_street.py (it only uses Soulstruct
for the game's compression and binder), then relaunch the game.
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
import flver_floor  # noqa: E402
import flver_textures  # noqa: E402
import hkx_collision  # noqa: E402
import hkx_tag  # noqa: E402
import tpf  # noqa: E402

for module in (dds_color, flver_fix, flver_build, flver_floor, flver_textures, hkx_tag, hkx_collision, tpf):
    importlib.reload(module)

ASSET = "aeg030_519"
flver_textures.AET = ASSET.replace("aeg", "AET").upper()
flver_textures.FLAT_NORMAL = f"{flver_textures.AET}_n"
SATURATION, BRIGHTNESS = 1.3, 1.1  # same colour boost as the classic street (dds_color.py)

PROJECT = ROOT / "reference" / "eldenring" / "project" / "asset"
GAME = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset")
STREET_GEOMBND = PROJECT / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx.before_textures"  # single-mesh FLVER template
VANILLA = GAME / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"  # its plain lit materials
GAME_COLLISION = GAME / "aeg" / "aeg030" / f"{ASSET}_l.geomhkxbnd.dcx"
AET_TEMPLATE = GAME / "aet" / "aet030" / "aet030_000.tpf.dcx"  # for its DCX type
GEOMBND = PROJECT / "aeg" / "aeg030" / f"{ASSET}.geombnd.dcx"
COLLISION = PROJECT / "aeg" / "aeg030" / GAME_COLLISION.name
TEXTURES = [PROJECT / "aet" / "aet030" / f"{ASSET.replace('aeg', 'aet')}{suffix}.tpf.dcx" for suffix in ("", "_l")]
ROOM = ROOT / "reference" / "w101" / "converted_rattlebones_classic"
OBJ = ROOM / "WC_Generic_Gauntlet_NoDoor_Classic.obj"


def build_room():
    start = time.time()
    template = flver_fix.flver_in_binder(STREET_GEOMBND)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    positions, uvs, faces = flver_build.parse_obj(OBJ)
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces)
    texture_of_material = flver_textures.parse_mtl(OBJ.with_suffix(".mtl"))
    stems = sorted(set(texture_of_material.values()))
    dds = [dds_color.boost((ROOM / "textures" / f"{stem}.dds").read_bytes(), SATURATION, BRIGHTNESS) for stem in stems]
    alpha = {i for i, data in enumerate(dds) if tpf.has_alpha(data)}
    face_textures = [stems.index(texture_of_material[m]) if m in texture_of_material else None for m in materials]
    flver = flver_textures.split_materials(single, vanilla, face_textures, stems, alpha)

    binder = Binder.from_path(STREET_GEOMBND)
    for entry in binder.entries:
        entry.path = re.sub(r"(?i)aeg030_518", ASSET.upper(), entry.path)
        if entry.name.lower().endswith(".flver"):
            entry.set_uncompressed_data(flver)
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
    triangles = list(flver_floor._triangles(flver))
    data, _ = hkx_collision.build_hkx(hkx, triangles)
    bnd, dcx_type = decompress(GAME_COLLISION)
    COLLISION.write_bytes(compress(hkx_collision.replace_binder_entry(bnd, data), dcx_type))
    print(f"Wrote collision for {len(triangles)} triangles to {COLLISION}")
    print(f"Rattlebones' room is built ({time.time() - start:.0f} s). Place it with msb_add_room.py, then relaunch.")


build_room()
