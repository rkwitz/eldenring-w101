"""Build the classic Wizard101 sky as an asset around Unicorn Way: aeg030_523, a dome placed on the street.

Wizard101 draws the sky camera-attached: a 12-sided prism of the swirl sky (layer 1, flat purple cap overhead) and a
band of scrolling clouds (layer 2), unlit (see sky_bake.py). Elden Ring can't attach an asset to the camera this way,
so this bakes what the Wizard101 camera sees in every direction into one texture and wraps it on a closed dome centred
on the zone. In each direction the dome sits just beyond the farthest Wizard101 geometry (sky_bake.hull: 150 m overhead,
up to ~700 m behind the giant backdrop trees), so it hides Elden Ring's land and sky outside the zone. The clouds are
frozen (no UV scrolling in our material).

The dome is drawn unshaded, like Wizard101's APPLY_REPLACE sky (SKY_MATERIAL "emissive"): an exact copy of the one
material of vanilla asset aeg099_332 (flver_textures.emissive_material: matbin AEG099_332, shader M[AMSN_V]_Emissive,
with that FLVER's flags and GXMD parameter block), the sky texture as its EmissiveMap and a black AlbedoMap, so
light, ambient occlusion and shadows add nothing. Our earlier unlit tries (2026-10-22/23: M[A]_NoLight and
m12_00_680) gave the shaders a bare GX list without the per-material parameters every vanilla emissive material
carries, and drew one flat white or grey. The street's lit material (SKY_MATERIAL "lit") does show the texture, but
lit, Ryan saw shading, "shadows" and the dome's triangles (2026-10-25). Normals point smoothly at the centre either
way. Fog would still tint the dome, and Ryan wants even, Wizard101-like light, so this also writes gparams for every
overworld light set and weather (gparam_fog.py) without fog, without the sun and other shadow-casting lights and
with a white ambient light from every side (FLAT_LIGHTING, AMBIENT, OVERHEAD). These edits apply to the whole
overworld.

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
EMISSIVE_VANILLA = GAME / flver_textures.EMISSIVE_ASSET  # the vanilla emissive material we copy
AET_TEMPLATE = GAME / "aet" / "aet030" / "aet030_000.tpf.dcx"  # for its DCX type
GEOMBND = PROJECT / "aeg" / "aeg030" / f"{ASSET}.geombnd.dcx"
TEXTURES = [PROJECT / "aet" / "aet030" / f"{ASSET.replace('aeg', 'aet')}{suffix}.tpf.dcx" for suffix in ("", "_l")]
SKY = ROOT / "reference" / "w101" / "converted_sky_classic"
SKY_TEXTURE, CLOUD_TEXTURE = "WC_Skybox01_KH__19706315", "WC_Skybox_Clouds_KH__d376b071"
STREET_OBJ = ROOT / "reference" / "w101" / "converted_classic" / "WC_Z06_Unicorn_Way_Classic.obj"
SKY_MATERIAL = "emissive"  # "emissive": unshaded (see above); "lit": the street's material (shows the texture, shaded)
# Emissive only: the sky texture's brightness as light (1.0 = as baked). Lower it (0.5, 0.25) if the sky is white or
# glares; raise it (up to 1.0) if dark. If the sky shows a different, Elden Ring texture, tell Claude.
SKY_BRIGHTNESS = 1.0
SKY_SATURATION = 1.3  # pushes the pastel colours away from grey before Elden Ring's tone mapping (1.0 = as is)
# Lighting for the whole overworld (gparam_fog.flat_lighting): True = no sun, no shadows, ambient light of intensity
# AMBIENT from every side (raise it if everything is too dark, lower it if washed out; vanilla daylight sun is about
# 0.5). False = Elden Ring's own lighting (fog stays off either way).
FLAT_LIGHTING = True
AMBIENT = 0.8  # 0.5 looked very dark (2026-10-23)
OVERHEAD = 0.3  # a soft, shadowless white light straight down (directional light 1), 0 = none
GAME_GPARAMS = GAME.parent / "param" / "drawparam"
PROJECT_GPARAMS = PROJECT.parent / "param" / "drawparam"


def remove_fog():
    """Write fogless, evenly lit copies of the overworld's light-set and weather gparams (gparam_fog.py: remove_fog
    and, with FLAT_LIGHTING, flat_lighting) to the project, in the game's own compression (an earlier DFLT copy from
    gparam_fog.py's command line may not load)."""
    PROJECT_GPARAMS.mkdir(parents=True, exist_ok=True)
    names = sorted(GAME_GPARAMS.glob("m00_00_*_weatheroutdoor.gparam.dcx"))
    # Every overworld light set (m60_00_XXXX: XXXX is a collision's LightSetID; ours are -1, which keeps the last one)
    names += [p for p in sorted(GAME_GPARAMS.glob("m60_00_*.gparam.dcx")) if "capturemap" not in p.name]
    written = 0
    for path in names:
        data, dcx_type = decompress(path)
        new, changed = gparam_fog.remove_fog(data)
        if FLAT_LIGHTING:
            new, lit = gparam_fog.flat_lighting(new, AMBIENT, OVERHEAD)
            changed += lit
        if changed:
            (PROJECT_GPARAMS / path.name).write_bytes(compress(new, dcx_type))
            written += 1
    lighting = f", no sun, ambient {AMBIENT}, overhead {OVERHEAD}" if FLAT_LIGHTING else ""
    print(f"Wrote {written} gparams (no fog{lighting}) to {PROJECT_GPARAMS} (delete that folder to undo)")


def inward(point):
    length = math.sqrt(sum(c * c for c in point)) or 1.0
    return tuple(-c / length for c in point)


def build_sky():
    street = STREET_OBJ.with_name(STREET_OBJ.stem + "_edited.obj")
    street = street if street.exists() else STREET_OBJ  # the OBJ the game's street was built from
    radii = sky_bake.hull(sky_bake.zone_points(street))
    print(f"Sky dome around {street.name}: {radii.min():.0f}..{radii.max():.0f} m from {sky_bake.CENTRE}")
    assert "#vc" in (SKY / "layer2.obj").read_text(), "layer2.obj has no vertex colours: convert it again (nif2obj.py)"
    image = sky_bake.bake(SKY / "layer1.obj", (SKY / "textures" / f"{SKY_TEXTURE}.dds").read_bytes(),
                          SKY / "layer2.obj", (SKY / "textures" / f"{CLOUD_TEXTURE}.dds").read_bytes())
    brightness = min(1.0, SKY_BRIGHTNESS) if SKY_MATERIAL == "emissive" else 1.0
    dds = sky_bake.dxt1_dds(sky_bake.dim(image, brightness, SKY_SATURATION), dither=1 / 64)
    (SKY / "sky_baked.dds").write_bytes(dds)

    positions, uvs, faces = sky_bake.dome(radii)
    template = flver_fix.flver_in_binder(STREET_GEOMBND)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    # Smooth normals pointing at the dome's centre (its origin): with each face's own normal every triangle, and every
    # ring of them, caught the light a little differently and showed as facets and horizontal bands (2026-10-23).
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces, vertex_normal=inward)
    assert SKY_MATERIAL in ("emissive", "lit"), SKY_MATERIAL
    opaque = None  # the street's lit material
    if SKY_MATERIAL == "emissive":
        opaque = flver_textures.emissive_material(flver_fix.flver_in_binder(EMISSIVE_VANILLA))
    flver = flver_textures.split_materials(single, vanilla, [0] * len(materials), [SKY_TEXTURE], alpha=set(),
                                           opaque=opaque)

    binder = Binder.from_path(STREET_GEOMBND)
    for entry in binder.entries:
        entry.path = re.sub(r"(?i)aeg030_518", ASSET.upper(), entry.path)
        if entry.name.lower().endswith(".flver"):
            entry.set_uncompressed_data(flver)
    binder.write(GEOMBND)
    print(f"Wrote {GEOMBND}: {len(materials)} triangles, {SKY_MATERIAL} material")

    textures = [(flver_textures.texture_name(0), dds), (flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, 64)),
                (flver_textures.black_name(), tpf.solid_dds(0, 0, 0))]
    _, dcx_type = decompress(AET_TEMPLATE)
    packed = compress(tpf.build_tpf(textures), dcx_type)
    for path in TEXTURES:
        path.write_bytes(packed)
    print(f"Wrote the sky texture to {TEXTURES[0]} (and _l). Place part AEG030_523_9000 with msb_add_sky.py, then "
          "relaunch the game.")


build_sky()
remove_fog()
