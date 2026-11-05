"""Round 26 (W1): the Commons gate's fog as our own asset aeg030_530, seen from anywhere on Unicorn Way.

Why: the fog Ryan sees up close is m18's AEG092_300_0500, a bare 5 x 6 m plane whose whole look comes from its
material, Field_PhantomWall_Loding.matxml ("M_PhantomWall_Loading": two mask textures, the rest is the shader). Its
round 25 copy in m60 (AEG092_300_9101) has the street's draw settings (PartsDrawParam 36, any distance) and
AssetGeometryParam 92300 has nothing that limits it (refDrawParamId 2930 is overridden by the part, no co-op-only
flag), yet Ryan still saw no fog from afar. What that shader does with distance (or outside m18) is in
Game\\material\\allmaterial.bnd.dcx, which we can't read. So instead of depending on it, this builds a plane with an
ordinary material that the game draws at every distance like our other assets: the street's M_AMSN_V shader alpha
blended (flver_textures.blend_material, as the tower room's glass jars) with a white mist texture (fog_texture.py),
two-sided, 5 m wide and 6 m tall like AEG092_300. msb_fog_gate.py places it (AEG030_530_9102) on the phantom wall's
spot.

Writes, in the Smithbox project (aeg030_530 is an otherwise unused vanilla slot, AssetGeometryParam 30530: no hit,
no co-op flag; deleting these files gives it back):
  asset/aeg/aeg030/aeg030_530.geombnd.dcx         the plane (no collision)
  asset/aet/aet030/aet030_530.tpf.dcx (and _l)    the mist texture
Run by blender_build_all.py (step "fog") or alone in Blender's Scripting tab; relaunch the game afterwards.
"""
import os
import importlib
import re
import sys
from pathlib import Path
try:  # config.toml at the repo root (tools/w101er_config.py), then the default
    from w101er_config import setting as _setting
except ImportError:
    _setting = os.environ.get

from soulstruct.containers import Binder
from soulstruct.dcx import compress, decompress

ROOT = Path(_setting("W101ER_ROOT", Path.home() / "Documents" / "game_modding"))
sys.path.insert(0, str(Path(os.environ.get("W101ER_TOOLS", ROOT / "eldenring-w101" / "tools"))))
import flver_build  # noqa: E402
import flver_fix  # noqa: E402
import flver_textures  # noqa: E402
import fog_texture  # noqa: E402
import tpf  # noqa: E402

for module in (flver_fix, flver_build, flver_textures, fog_texture, tpf):
    importlib.reload(module)

ASSET = "aeg030_530"
# Round 27 (A6, Ryan: "too narrow and too transparent"): the archway it stands in is 8 m wide and 5.5 m high
# (blender_switch_street.tunnel_inside: |x| < 4, y < 5.5), so the plane is 8.4 m wide (rounds 26: 5 x 6 as AEG092_300)
# and fog_texture.py makes it nearly opaque. Elden Ring's own fog walls (msb_fog_gate.FOG_WALLS) stand 0.5 m in
# front of it; it stays as the view from afar.
WIDTH, HEIGHT = 8.4, 6.0
PROJECT = ROOT / "reference" / "eldenring" / "project" / "asset"
GAME = Path(_setting("ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game")) / r"asset"
STREET_GEOMBND = PROJECT / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx.before_textures"  # single-mesh FLVER template
VANILLA = GAME / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"  # its plain lit materials
AET_TEMPLATE = GAME / "aet" / "aet030" / "aet030_000.tpf.dcx"  # for its DCX type


def plane():
    """Positions, UVs and faces (flver_build.parse_obj's form) of the plane, both sides."""
    w = WIDTH / 2
    positions = [(-w, 0.0, 0.0), (w, 0.0, 0.0), (w, HEIGHT, 0.0), (-w, HEIGHT, 0.0)]
    uvs = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]  # OBJ v up: the texture's top row at the top
    quad = [(0, 0), (1, 1), (2, 2), (3, 3)]
    front = [(quad[0], quad[1], quad[2]), (quad[0], quad[2], quad[3])]
    back = [(c, b, a) for a, b, c in front]
    return positions, uvs, [("fog", f) for f in front + back]


def build():
    flver_textures.AET = ASSET.replace("aeg", "AET").upper()
    flver_textures.FLAT_NORMAL = f"{flver_textures.AET}_n"
    template = flver_fix.flver_in_binder(STREET_GEOMBND)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    positions, uvs, faces = plane()
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces)
    flver = flver_textures.split_materials(single, vanilla, [0] * len(materials), ["fog"], alpha={0},
                                           blend_material=flver_textures.blend_material(vanilla))
    binder = Binder.from_path(STREET_GEOMBND)
    for entry in binder.entries:
        entry.path = re.sub(r"(?i)aeg030_518", ASSET.upper(), entry.path)
        if entry.name.lower().endswith(".flver"):
            entry.set_uncompressed_data(flver)
    geombnd = PROJECT / "aeg" / "aeg030" / f"{ASSET}.geombnd.dcx"
    binder.write(geombnd)
    textures = [(flver_textures.texture_name(0), fog_texture.fog_dds()),
                (flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, 0))]
    _, dcx_type = decompress(AET_TEMPLATE)
    packed = compress(tpf.build_tpf(textures), dcx_type)
    for suffix in ("", "_l"):
        (PROJECT / "aet" / "aet030" / f"{ASSET.replace('aeg', 'aet')}{suffix}.tpf.dcx").write_bytes(packed)
    print(f"Commons gate fog: {geombnd} ({len(materials)} triangles) and its mist texture. Place it with "
          f"msb_fog_gate.py (AEG030_530_9102), then relaunch.")


build()
