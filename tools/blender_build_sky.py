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
with a white ambient light from every side (FLAT_LIGHTING, AMBIENT, OVERHEAD), with eye adaptation and bloom off
(FIXED_EXPOSURE, NO_BLOOM: with them on, every glow we tried, 4.0 down to 0.25, came out blown out, and the sky's
brightness changed with where you looked). These edits apply to the whole overworld. SKY_CALIBRATE builds a dome
of 8 numbered slices at 8 brightnesses, so one run in game picks the brightness (see SKY_CALIBRATE).

The dome's shape (sky_bake.hull) covers the street OBJ and every Wizard101 part placed in m60_42_36_00 (placed_parts:
the garden at road level, the rooms, the quest windows), read from the project MSB and the built FLVERs; the build
stops if any part would stick out. Rebuild the sky after moving or rebuilding those parts.

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

import numpy as np
from soulstruct.containers import Binder
from soulstruct.dcx import compress, decompress

ROOT = Path.home() / "Documents" / "game_modding"
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import flver_build  # noqa: E402
import flver_fix  # noqa: E402
import msb_floor  # noqa: E402
import flver_textures  # noqa: E402
import gparam_fog  # noqa: E402
import sky_bake  # noqa: E402
import tpf  # noqa: E402

for module in (flver_fix, flver_build, flver_textures, gparam_fog, msb_floor, sky_bake, tpf):
    importlib.reload(module)

ASSET = "aeg030_523"
flver_textures.AET = ASSET.replace("aeg", "AET").upper()
flver_textures.FLAT_NORMAL = f"{flver_textures.AET}_n"

PROJECT = ROOT / "reference" / "eldenring" / "project" / "asset"
GAME = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset")
STREET_GEOMBND = PROJECT / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx.before_textures"  # single-mesh FLVER template
VANILLA = GAME / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"  # its plain lit material
EMISSIVE_VANILLA = GAME.parent / flver_textures.EMISSIVE_ASSET  # the vanilla emissive material we copy (path starts at asset\)
AET_TEMPLATE = GAME / "aet" / "aet030" / "aet030_000.tpf.dcx"  # for its DCX type
GEOMBND = PROJECT / "aeg" / "aeg030" / f"{ASSET}.geombnd.dcx"
TEXTURES = [PROJECT / "aet" / "aet030" / f"{ASSET.replace('aeg', 'aet')}{suffix}.tpf.dcx" for suffix in ("", "_l")]
SKY = ROOT / "reference" / "w101" / "converted_sky_classic"
SKY_TEXTURE, CLOUD_TEXTURE = "WC_Skybox01_KH__19706315", "WC_Skybox_Clouds_KH__d376b071"
STREET_OBJ = ROOT / "reference" / "w101" / "converted_classic" / "WC_Z06_Unicorn_Way_Classic.obj"
MSB = ROOT / "reference" / "eldenring" / "project" / "map" / "mapstudio" / "m60_42_36_00.msb.dcx"  # where our parts sit
STREET_PART = "AEG030_518_9000"
SKY_MATERIAL = "emissive"  # "emissive": unshaded (see above); "lit": the street's material (shows the texture, shaded)
# Emissive only. How bright the sky is drawn: SKY_GLOW (GXMD 0xF4 of the emissive material, 1.0 in vanilla) times
# SKY_BRIGHTNESS (<= 1, the texture's own light intensity). Glow 4.0, 1.0 and 0.25 were all blown out (2026-10-25/26)
# with Elden Ring's eye adaptation and bloom on (now off, FIXED_EXPOSURE and NO_BLOOM below). On the calibration
# dome (2026-10-26) Ryan picked slice 6-7 for brightness: calibration(6.5) = glow 0.0743, brightness 0.149. That
# texture was still read as linear (legacy DXT1 header, now SKY_SRGB): shown washed out and flat, "just different
# colours". Read as sRGB the same values come out much darker, so SKY_BRIGHTNESS 0.6 keeps the sky's mean on-screen
# light where slice 6.5 had it (compared offline), with the colour and swirls back. Brighter: raise SKY_BRIGHTNESS
# (up to 1.0), then SKY_GLOW; darker: lower SKY_BRIGHTNESS.
SKY_GLOW = 0.0743
SKY_BRIGHTNESS = 0.6
SKY_SRGB = True  # DX10 BC1_UNORM_SRGB texture, like vanilla colour maps (False: the old legacy DXT1, read as linear)
# SKY_CALIBRATE: instead of one brightness, the dome is cut into 8 slices around (45 degrees each, numbered 1 to 8 in
# big white digits in the sky), slice k drawn at glow CALIBRATION_GLOW * 2^(-(k-1)/2) and texture brightness
# 2^(-(k-1)/2): each slice half as bright as the one before if the glow parameter works as we think, at least
# 1/1.4 as bright if only the texture counts. Ryan walks the street, says which number looks like the Wizard101
# sky, and `calibration(k)` gives the SKY_GLOW and SKY_BRIGHTNESS that draw the whole dome exactly like slice k.
SKY_CALIBRATE = False  # done 2026-10-26: slice 6-7
CALIBRATION_GLOW = 0.5
CALIBRATION_SLICES = 8
SKY_SATURATION = 1.3  # pushes the colours away from grey (1.0 = as graded); a bit above the reference for the tone map
# Ryan's Wizard101 screenshot of the Unicorn Way sky (2026-10-25), as mean colour by elevation: lavender above,
# pink-white wisps, pale blue at the bottom. The bake is recoloured to it (sky_bake.grade) keeping SKY_DETAIL of its
# swirls; None keeps the bake's own colours. Elevations are Wizard101's (the dome shows W101 15 deg at 34 deg up, 28 at
# 71, see sky_bake.STRETCH). 2026-10-25 took the screenshot's bottom as the horizon, which left blue only below 3.5
# deg W101 (dome 3.5): hidden behind the street's skyline (20-40 deg up from the street), so Ryan saw all purple
# (2026-10-27). The screenshot's bottom is in fact just above the skyline: its 0..35 now spans W101 15..28 (dome
# 34..71 deg), blue below, as Wizard101's own texture is (its blue band reaches ~15 deg).
# 2026-10-27 round 9: "moved the sky a little too high", wants just a little blue: shifted 5 deg down, 0..35 now spans
# W101 10..23 (dome 19.5..55 deg), blue fading out by W101 11.3 (dome 23), just above the lowest skyline.
# 2026-10-29 round 11: "still showing too much blue, lower it more": 4 deg down again, 0..35 now spans W101 6..19
# (dome 8..45.5 deg), blue fading out by W101 7.3 (dome 11.7), pink by dome 15: all of it under the street's skyline
# (20-40 deg up), so above the rooftops only pink and lavender show.
SKY_TARGET = [(-30, (150, 178, 245)), (-1, (155, 180, 245)), (6.0, (161, 183, 245)), (7.3, (185, 193, 252)),
              (8.6, (220, 204, 254)), (9.9, (238, 201, 254)), (11.2, (240, 190, 255)), (12.5, (235, 180, 253)),
              (13.8, (223, 166, 249)), (15.1, (215, 155, 242)), (16.4, (208, 151, 237)), (19.0, (206, 151, 236)),
              (90, (206, 151, 236))]
SKY_DETAIL = 1.2  # 0.5 (2026-10-25) flattened the swirls; 1.0 = the bake's own swirls and clouds, above = stronger
# Lighting for the whole overworld (gparam_fog.flat_lighting): True = no sun, no shadows, ambient light of intensity
# AMBIENT from every side (raise it if everything is too dark, lower it if washed out; vanilla daylight sun is about
# 0.5). False = Elden Ring's own lighting (fog stays off either way).
FLAT_LIGHTING = True
# 2026-10-26: the Wizard101 textures are now sRGB at their true colours (tpf.build_tpf; before they were read as
# linear with a 1.3/1.1 boost), which decodes about 2.5x darker on average (street 2.5, room 2.6, garden 2.25,
# interiors 2.5), so both lights are 2.5x what they were (1.5, 0.8) to keep the zone as bright as Ryan last saw it.
AMBIENT = 2.6  # 3.3 "still too bright" (2026-10-27 round 9, ~21% down); 3.75 "a little too bright" (2026-10-27); 0.5 looked very dark (2026-10-23), 0.8 still "like lighting is off" (2026-10-25)
OVERHEAD = 1.4  # 1.75 with AMBIENT 3.3, 2.0 with AMBIENT 3.75. A soft, shadowless white light straight down (directional light 1), 0 = none
# Elden Ring's eye adaptation (auto exposure) re-exposes the screen by what fills the view: with a big glowing sky it
# darkened everything, and the same glow looked different by where you looked. FIXED_EXPOSURE turns it off in every
# file written (the tone map keeps each light set's own ToneMap-Exposure, as vanilla default.gparam does). NO_BLOOM
# turns off bloom (light set 100 blooms everything above 0.25 at 1.5x, a white haze over a bright sky); it also
# writes default.gparam (Bloom only), which applies game-wide.
FIXED_EXPOSURE = True
NO_BLOOM = True
GAME_GPARAMS = GAME.parent / "param" / "drawparam"
PROJECT_GPARAMS = PROJECT.parent / "param" / "drawparam"


def remove_fog():
    """Write fogless, evenly lit copies of the overworld's light-set and weather gparams (gparam_fog.py: remove_fog
    and, with FLAT_LIGHTING, flat_lighting; FIXED_EXPOSURE, NO_BLOOM) to the project, in the game's own compression
    (an earlier DFLT copy from gparam_fog.py's command line may not load)."""
    PROJECT_GPARAMS.mkdir(parents=True, exist_ok=True)
    names = sorted(GAME_GPARAMS.glob("m00_00_*_weatheroutdoor.gparam.dcx"))
    # Every overworld light set (m60_00_XXXX: XXXX is a collision's LightSetID; ours are -1, which keeps the last one)
    names += [p for p in sorted(GAME_GPARAMS.glob("m60_00_*.gparam.dcx")) if "capturemap" not in p.name]
    written, exposure, bloom = 0, 0, 0
    for path in names:
        data, dcx_type = decompress(path)
        new, changed = gparam_fog.remove_fog(data)
        if FLAT_LIGHTING:
            new, lit = gparam_fog.flat_lighting(new, AMBIENT, OVERHEAD)
            changed += lit
        if FIXED_EXPOSURE:
            new, fixed = gparam_fog.fixed_exposure(new)
            changed += fixed
            exposure += bool(fixed)
        if NO_BLOOM:
            new, glare = gparam_fog.no_bloom(new)
            changed += glare
            bloom += bool(glare)
        if changed:
            (PROJECT_GPARAMS / path.name).write_bytes(compress(new, dcx_type))
            written += 1
    default = GAME_GPARAMS / "default.gparam.dcx"
    if NO_BLOOM and default.exists():
        data, dcx_type = decompress(default)
        new, glare = gparam_fog.no_bloom(data)
        if glare:
            (PROJECT_GPARAMS / default.name).write_bytes(compress(new, dcx_type))
            written, bloom = written + 1, bloom + 1
    elif (PROJECT_GPARAMS / default.name).exists():
        (PROJECT_GPARAMS / default.name).unlink()  # an earlier NO_BLOOM run's copy
    lighting = f", no sun, ambient {AMBIENT}, overhead {OVERHEAD}" if FLAT_LIGHTING else ""
    print(f"Wrote {written} gparams (no fog{lighting}; auto exposure off in {exposure}, bloom off in {bloom}) to "
          f"{PROJECT_GPARAMS} (delete that folder to undo)")


def inward(point):
    length = math.sqrt(sum(c * c for c in point)) or 1.0
    return tuple(-c / length for c in point)


def calibration(k):
    """(SKY_GLOW, SKY_BRIGHTNESS) that draw the whole dome like calibration slice k (1..CALIBRATION_SLICES)."""
    step = 2.0 ** (-(k - 1) / 2)
    return CALIBRATION_GLOW * step, step


def placed_parts():
    """{part name: points (n, 3) in street OBJ coordinates} covering every Wizard101 asset placed in our map (the
    project's m60_42_36_00 MSB: parts AEG030_5xx except the sky itself, e.g. the street, the garden at road level,
    the quest windows, the rooms and interiors underground), from their built FLVERs (project, else game) and
    their MSB positions, so the dome is shaped around all of them, not just the street OBJ (the garden's backdrop
    trees stuck out through the old dome, 2026-10-26)."""
    data, _ = decompress(MSB)
    _, lists = msb_floor.read_msb(data)
    transforms = {msb_floor.entry_name(e): msb_floor.transform(e) for e in msb_floor._list(lists, "PARTS_PARAM_ST").entries}
    street_position, street_rotation = transforms[STREET_PART]
    out = {}
    for name, (position, rotation) in sorted(transforms.items()):
        model = name.rsplit("_", 1)[0].lower()
        if not re.fullmatch(r"aeg030_5\d\d", model) or model == ASSET:
            continue
        assert rotation[0] == 0 and rotation[2] == 0, f"{name} is tilted"
        geombnd = PROJECT / "aeg" / "aeg030" / f"{model}.geombnd.dcx"
        geombnd = geombnd if geombnd.exists() else GAME / "aeg" / "aeg030" / f"{model}.geombnd.dcx"
        positions, faces = sky_bake.flver_mesh(flver_fix.flver_in_binder(geombnd))
        out[name] = sky_bake.to_street(sky_bake.surface_points(positions, faces), position, rotation[1],
                                       street_position, street_rotation[1])
    return out


def build_sky():
    street = STREET_OBJ.with_name(STREET_OBJ.stem + "_edited.obj")
    street = street if street.exists() else STREET_OBJ  # the OBJ the game's street was built from
    parts = placed_parts()
    radii = sky_bake.hull(np.concatenate([sky_bake.zone_points(street)] + list(parts.values())))
    print(f"Sky dome around {street.name} and {len(parts)} placed parts: {radii.min():.0f}..{radii.max():.0f} m "
          f"from {sky_bake.CENTRE}")
    for name, points in parts.items():
        distance, radius = sky_bake.dome_radius_at(points, radii)
        outside = int((distance > radius).sum())
        print(f"  {name}: {'OUTSIDE THE DOME at ' + str(outside) + ' points' if outside else 'inside'}, "
              f"closest {float((radius - distance).min()):.0f} m from the dome")
        assert not outside, f"{name} sticks out of the sky dome"
    assert "#vc" in (SKY / "layer2.obj").read_text(), "layer2.obj has no vertex colours: convert it again (nif2obj.py)"
    image = sky_bake.bake(SKY / "layer1.obj", (SKY / "textures" / f"{SKY_TEXTURE}.dds").read_bytes(),
                          SKY / "layer2.obj", (SKY / "textures" / f"{CLOUD_TEXTURE}.dds").read_bytes())
    if SKY_TARGET:
        image = sky_bake.grade(image, SKY_TARGET, SKY_DETAIL)
    emissive = SKY_MATERIAL == "emissive"
    calibrate = SKY_CALIBRATE and emissive
    slices = CALIBRATION_SLICES
    if calibrate:
        # Slice k (columns of the texture = azimuth): brightness calibration(k)[1], digits k at two heights.
        width = image.shape[1] // slices
        steps = [calibration(k + 1) for k in range(slices)]
        image = np.concatenate([sky_bake.dim(image[:, k * width:(k + 1) * width], steps[k][1], SKY_SATURATION)
                                for k in range(slices)], 1)
        labels = [((k + 0.5) * 360 / slices, str(k + 1)) for k in range(slices)]
        for band in ((12.0, 26.0), (40.0, 54.0)):
            image = sky_bake.draw_labels(image, labels, band)
    else:
        brightness = min(1.0, SKY_BRIGHTNESS) if emissive else 1.0
        image = sky_bake.dim(image, brightness, SKY_SATURATION)
    dds = sky_bake.dxt1_dds(image, dither=1 / 64, srgb=SKY_SRGB)
    (SKY / "sky_baked.dds").write_bytes(dds)

    positions, uvs, faces = sky_bake.dome(radii)
    template = flver_fix.flver_in_binder(STREET_GEOMBND)
    vanilla = flver_fix.flver_in_binder(VANILLA)
    # Smooth normals pointing at the dome's centre (its origin): with each face's own normal every triangle, and every
    # ring of them, caught the light a little differently and showed as facets and horizontal bands (2026-10-23).
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces, vertex_normal=inward)
    assert len(materials) == len(faces), "dome() faces and the built triangles differ"
    assert SKY_MATERIAL in ("emissive", "lit"), SKY_MATERIAL
    opaque, gx, face_materials, shared = None, None, [0] * len(materials), None  # None: the street's lit material
    if emissive:
        source = flver_fix.flver_in_binder(EMISSIVE_VANILLA)
        opaque = flver_textures.emissive_material(vanilla, source, SKY_GLOW)
        if calibrate:
            face_materials, shared = sky_bake.face_sectors(faces, slices), 0
            gx = {k: flver_textures.set_gxmd(opaque["gx"], flver_textures.EMISSIVE_GLOW_PARAM, [steps[k][0]])
                  for k in range(slices)}
    flver = flver_textures.split_materials(single, vanilla, face_materials, [SKY_TEXTURE] * (slices if gx else 1),
                                           alpha=set(), opaque=opaque, gx=gx, shared_texture=shared)

    binder = Binder.from_path(STREET_GEOMBND)
    for entry in binder.entries:
        entry.path = re.sub(r"(?i)aeg030_518", ASSET.upper(), entry.path)
        if entry.name.lower().endswith(".flver"):
            entry.set_uncompressed_data(flver)
    binder.write(GEOMBND)
    if calibrate:
        look = "; ".join(f"{k + 1}: glow {g:.4g}, brightness {b:.3g}" for k, (g, b) in enumerate(steps))
        print(f"Wrote {GEOMBND}: {len(materials)} triangles, CALIBRATION dome, slices {look}")
    else:
        print(f"Wrote {GEOMBND}: {len(materials)} triangles, {SKY_MATERIAL} material, glow {SKY_GLOW}, brightness "
              f"{SKY_BRIGHTNESS}")

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
