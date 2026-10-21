"""Build the classic Wizard101 sky as an asset around Unicorn Way: aeg030_523, a dome placed on the street.

Wizard101 draws the sky camera-attached: a 12-sided prism of the swirl sky (layer 1, flat purple cap overhead) and a
band of scrolling clouds (layer 2), unlit (see sky_bake.py). Elden Ring can't attach an asset to the camera this way,
so this bakes what the Wizard101 camera sees in every direction into one texture and wraps it on a closed dome centred
on the zone. In each direction the dome sits just beyond the farthest Wizard101 geometry (sky_bake.hull: 150 m overhead,
up to ~700 m behind the giant backdrop trees), so it hides Elden Ring's land and sky outside the zone. The clouds are
frozen (no UV scrolling in our material).

The dome uses the street's plain lit material with every normal pointing up, so the sun lights it evenly. Elden Ring
has no unlit material we can point at our own texture (the aeg096 sky domes use Field_Skysphere matbins with their own
sky textures). Its MSB part must not receive shadows or point lights (msb_add_sky.py). Distance fog would wash it
out, so this also writes fogless gparams (gparam_fog.py) to the project, for the whole overworld.

Writes, in the Smithbox project:
  asset/aeg/aeg030/aeg030_523.geombnd.dcx      the dome FLVER, centred on its own origin; part AEG030_523_9000 sits
                                                at the dome centre on the street (msb_add_sky.py computes it)
  asset/aet/aet030/aet030_523.tpf.dcx (and _l)  the baked sky texture (DXT1 1024x512, U = azimuth, V = elevation)
and reference/w101/converted_sky_classic/sky_baked.dds (the same texture, to look at).

Needs converted_sky_classic/layer1.obj and layer2.obj from nif2obj.py with vertex colours (#vc lines).
Run in Blender's Scripting tab like blender_switch_street.py (it only uses Soulstruct for the game's compression and
binder, and Blender's numpy), then relaunch the game.
"""

import importlib
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
import gparam_fog  # noqa: E402
import sky_bake  # noqa: E402
import tpf  # noqa: E402

for module in (flver_fix, flver_build, flver_textures, gparam_fog, sky_bake, tpf):
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
SKY_TEXTURE, CLOUD_TEXTURE = "WC_Skybox01_KH__19706315", "WC_Skybox_Clouds_KH__d376b071"
STREET_OBJ = ROOT / "reference" / "w101" / "converted_classic" / "WC_Z06_Unicorn_Way_Classic.obj"
GAME_GPARAMS = GAME.parent / "param" / "drawparam"
PROJECT_GPARAMS = PROJECT.parent / "param" / "drawparam"


def remove_fog():
    """Write fogless copies of the overworld's light-set and weather gparams (gparam_fog.py) to the project, in the
    game's own compression (an earlier DFLT copy from gparam_fog.py's command line may not load)."""
    PROJECT_GPARAMS.mkdir(parents=True, exist_ok=True)
    names = sorted(GAME_GPARAMS.glob("m00_00_*_weatheroutdoor.gparam.dcx"))
    names += [GAME_GPARAMS / f"m60_00_{light_set}.gparam.dcx" for light_set in ("0000", "0100")]
    written = 0
    for path in names:
        data, dcx_type = decompress(path)
        new, changed = gparam_fog.remove_fog(data)
        if changed:
            (PROJECT_GPARAMS / path.name).write_bytes(compress(new, dcx_type))
            written += 1
    print(f"Wrote {written} fogless gparams to {PROJECT_GPARAMS} (delete that folder to get the fog back)")


def build_sky():
    street = STREET_OBJ.with_name(STREET_OBJ.stem + "_edited.obj")
    street = street if street.exists() else STREET_OBJ  # the OBJ the game's street was built from
    radii = sky_bake.hull(sky_bake.zone_points(street))
    print(f"Sky dome around {street.name}: {radii.min():.0f}..{radii.max():.0f} m from {sky_bake.CENTRE}")
    assert "#vc" in (SKY / "layer2.obj").read_text(), "layer2.obj has no vertex colours: convert it again (nif2obj.py)"
    image = sky_bake.bake(SKY / "layer1.obj", (SKY / "textures" / f"{SKY_TEXTURE}.dds").read_bytes(),
                          SKY / "layer2.obj", (SKY / "textures" / f"{CLOUD_TEXTURE}.dds").read_bytes())
    dds = sky_bake.dxt1_dds(image)
    (SKY / "sky_baked.dds").write_bytes(dds)

    positions, uvs, faces = sky_bake.dome(radii)
    template = flver_fix.flver_in_binder(STREET_GEOMBND)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces, fixed_normal=(0.0, 1.0, 0.0))
    flver = flver_textures.split_materials(single, vanilla, [0] * len(materials), [SKY_TEXTURE], alpha=set())

    binder = Binder.from_path(STREET_GEOMBND)
    for entry in binder.entries:
        entry.path = re.sub(r"(?i)aeg030_518", ASSET.upper(), entry.path)
        if entry.name.lower().endswith(".flver"):
            entry.set_uncompressed_data(flver)
    binder.write(GEOMBND)
    print(f"Wrote {GEOMBND}: {len(materials)} triangles")

    textures = [(flver_textures.texture_name(0), dds), (flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, 64))]
    _, dcx_type = decompress(AET_TEMPLATE)
    packed = compress(tpf.build_tpf(textures), dcx_type)
    for path in TEXTURES:
        path.write_bytes(packed)
    print(f"Wrote the sky texture to {TEXTURES[0]} (and _l). Place part AEG030_523_9000 with msb_add_sky.py, then "
          "relaunch the game.")


build_sky()
remove_fog()
