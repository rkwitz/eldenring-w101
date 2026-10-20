"""Build the classic Wizard101 sky as an asset around Unicorn Way: aeg030_523, a dome placed on the street.

Elden Ring draws its own sky from sky-sphere assets with special materials that the whole open world shares, so instead
of changing those, this wraps the street in W101's classic skybox (Commons skybox layer 1, reference/w101/
converted_sky_classic/layer1.obj) scaled up to a dome that encloses the whole zone. It also covers the far Limgrave
landscape. Vanilla aeg030_523 has no collision and isn't placed anywhere near The First Step, so it's free to reuse.

The dome uses the street's plain lit material with every normal pointing up, so the sun lights it evenly instead of
shading its sides; distance fog still tints it a little.

Writes, in the Smithbox project:
  asset/aeg/aeg030/aeg030_523.geombnd.dcx      the dome FLVER (in the street's coordinates, so it is placed with the
                                                street's own transform by part AEG030_523_9000, see msb_add_sky.py)
  asset/aet/aet030/aet030_523.tpf.dcx (and _l)  the sky texture

Run in Blender's Scripting tab like blender_switch_street.py (it only uses Soulstruct for the game's compression and
binder), then relaunch the game.
"""

import importlib
import math
import re
import sys
from pathlib import Path

from soulstruct.containers import Binder
from soulstruct.dcx import compress, decompress

ROOT = Path.home() / "Documents" / "game_modding"
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import flver_build  # noqa: E402
import flver_fix  # noqa: E402
import flver_textures  # noqa: E402
import tpf  # noqa: E402

for module in (flver_fix, flver_build, flver_textures, tpf):
    importlib.reload(module)

ASSET = "aeg030_523"
flver_textures.AET = ASSET.replace("aeg", "AET").upper()
flver_textures.FLAT_NORMAL = f"{flver_textures.AET}_n"

PROJECT = ROOT / "reference" / "eldenring" / "project" / "asset"
GAME = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset")
STREET_GEOMBND = PROJECT / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx.before_textures"  # single-mesh FLVER template
VANILLA = GAME / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"  # its plain lit material
AET_TEMPLATE = GAME / "aet" / "aet030" / "aet030_000.tpf.dcx"  # for its DCX type
GEOMBND = PROJECT / "aeg" / "aeg030" / f"{ASSET}.geombnd.dcx"
TEXTURES = [PROJECT / "aet" / "aet030" / f"{ASSET.replace('aeg', 'aet')}{suffix}.tpf.dcx" for suffix in ("", "_l")]
SKY = ROOT / "reference" / "w101" / "converted_sky_classic"
STREET_OBJ = ROOT / "reference" / "w101" / "converted_classic" / "WC_Z06_Unicorn_Way_Classic.obj"
MARGIN = 1.6  # dome radius over the distance from the zone's centre to its furthest corner


def zone_bounds():
    lo, hi = [math.inf] * 3, [-math.inf] * 3
    with open(STREET_OBJ) as f:
        for line in f:
            if line.startswith("v "):
                p = [float(x) for x in line.split()[1:4]]
                lo = [min(a, b) for a, b in zip(lo, p)]
                hi = [max(a, b) for a, b in zip(hi, p)]
    return lo, hi


def build_sky():
    lo, hi = zone_bounds()
    centre = ((lo[0] + hi[0]) / 2, 0.0, (lo[2] + hi[2]) / 2)  # on the street's ground level
    reach = math.hypot(hi[0] - lo[0], hi[2] - lo[2]) / 2
    positions, uvs, faces = flver_build.parse_obj(SKY / "layer1.obj")
    radius = max(math.hypot(x, z) for x, _, z in positions)
    scale = MARGIN * reach / radius
    positions = [(centre[0] + x * scale, centre[1] + y * scale, centre[2] + z * scale) for x, y, z in positions]
    print(f"Sky dome: radius {radius * scale:.0f} m around ({centre[0]:.0f}, {centre[2]:.0f}) of the street")

    template = flver_fix.flver_in_binder(STREET_GEOMBND)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces, fixed_normal=(0.0, 1.0, 0.0))
    texture_of_material = flver_textures.parse_mtl(SKY / "layer1.mtl")
    stems = sorted(set(texture_of_material.values()))
    dds = [(SKY / "textures" / f"{stem}.dds").read_bytes() for stem in stems]
    face_textures = [stems.index(texture_of_material[m]) if m in texture_of_material else 0 for m in materials]
    flver = flver_textures.split_materials(single, vanilla, face_textures, stems, alpha=set())

    binder = Binder.from_path(STREET_GEOMBND)
    for entry in binder.entries:
        entry.path = re.sub(r"(?i)aeg030_518", ASSET.upper(), entry.path)
        if entry.name.lower().endswith(".flver"):
            entry.set_uncompressed_data(flver)
    binder.write(GEOMBND)
    print(f"Wrote {GEOMBND}: {len(materials)} triangles")

    textures = [(flver_textures.texture_name(i), data) for i, data in enumerate(dds)]
    textures.append((flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, 64)))
    _, dcx_type = decompress(AET_TEMPLATE)
    packed = compress(tpf.build_tpf(textures), dcx_type)
    for path in TEXTURES:
        path.write_bytes(packed)
    print(f"Wrote the sky texture to {TEXTURES[0]} (and _l). Relaunch the game.")


build_sky()
