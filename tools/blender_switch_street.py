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
import w101_window_states  # noqa: E402

for module in (dds_color, flver_fix, flver_build, flver_floor, flver_textures, hkx_tag, hkx_collision, tpf,
               w101_window_states):
    importlib.reload(module)

STYLE = "classic"  # "classic" or "hd"
# W101's colours look muted under Elden Ring's lighting: the classic textures are made this much more saturated and
# brighter (1.0 and 1.0 keep them as they are). See dds_color.py.
SATURATION = 1.4  # sRGB textures (tpf.build_tpf) at 1.4x saturation: Ryan, 2026-10-27, wanted more than the true colours
BRIGHTNESS = 1.0
# Round 35 (G13, Ryan: "the actual road is a little bit too dark, but the sidewalk is fine"; round35/color_ref
# w101_road.png vs er_road.png). Screen colour over texture colour (same pixel classes in the texture, ours after the
# 1.4 saturation), in linear light: W101 lights every surface alike, cobbles 1.57/1.30/0.65, green bands 1.77/1.52/0.80,
# sidewalk flagstones 1.60/1.37/0.70. Ours: flagstones 1.13/1.15/0.65 but cobbles 0.91/0.82/0.54, bands 0.70/0.67/0.47,
# and the opaque grass R/G 0.86-0.95/0.59-0.71. The sidewalk (WC_Sidewalk_Unicorn) is the only one of them with
# transparent texels, so it alone gets the alpha-tested material m60_fort_215 (M_AMSN_V__Edge); the road and grass get
# the opaque m60_fort_220, which draws the same texture under the same light about 0.78x as bright (cobbles 0.80 /
# 0.71 / 0.83 of the flagstones' gains). Opaque textures are now brightened to make up for it: 1.12 on the sRGB values
# is 1.28x in linear light (1.12^2.2).
OPAQUE_BRIGHTNESS = 1.12

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
# Round 26 (W2): Wizard101's per-building window states (w101_window_states.py output, Wizard101 data).
WINDOW_STATES = CLASSIC / "window_states.tsv"
# Wizard101's own vertex normals of the classic zone (nif_normals.py, from ClassicMode-WC_Streets-WC_Unicorn.wad's
# WC_Z06_Unicorn_Way.nif). Without them every triangle got its own flat normal and Elden Ring's sun lit each one as
# a facet (round 24: trees with one triangle near white and the next much darker); with them the street shades
# smoothly like W101. If the file is missing, normals are averaged from the geometry instead (close, not exact).
NORMALS = CLASSIC / "WC_Z06_Unicorn_Way_Classic.normals"
# Blue channel of the flat normal map AET030_518_n = Elden Ring's glossiness (_n: red/green normal, blue inverted
# roughness). It was 64 (a faint sheen); classic W101 has no specular at all (no NiSpecularProperty in the zone), so 0.
GLOSS = 0
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


# Round 35 (G12/G12b/G12c): the Stranded Graveyard (m18) door stands in the Commons gatehouse, so parts of the
# gatehouse reach behind the door plane into the graveyard: its arch bases, the tower plinths beside the door and the
# side walls of the gate passage (x +-7, back to 14 m, through the room behind the door). Cutting them did not work:
# a cut wide enough left holes in the street (G12, commons_holes.png), and boxes guessed from m18's props left shards
# against its walls (G12b, door_clip3.png). Ryan: "Can you just move these vertexes back, like translate them so
# they're like inside of the Elden Ring wall so they're not visible?"
# So nothing is cut. street_graveyard_fix.py (run outside Blender) reads the graveyard's real walls: the render meshes
# of the m18 assets around the door, placed by m18_00_00_00.msb (its map collision has nothing at that height). For
# each column across the door it finds where the graveyard's open space ends toward the street, and moves every
# street vertex behind the door along the door's normal, toward the street (x and y stay), far enough that each of
# its triangles lies inside the walls or in front of the door plane at every point. Neighbouring triangles stay
# joined. Its output, graveyard_fix.tsv next to the classic OBJ, is applied here; without it the street is built as
# it is.
GRAVEYARD_FIX = CLASSIC / "graveyard_fix.tsv"


def graveyard_fix(positions, path):
    """Round 35 (G12c): apply street_graveyard_fix.py's output, lines "v x y z x' y' z'" (OBJ coordinates: a vertex
    and where it moves; every vertex at that position moves). Returns (vertices moved, lines in the file)."""
    key = lambda p: tuple(round(c, 4) for c in p)
    moves = {}
    for line in path.read_text().splitlines():
        cols = line.split()
        if cols and cols[0] == "v":
            values = [float(c) for c in cols[1:7]]
            moves[key(values[:3])] = tuple(values[3:])
    moved = 0
    for i, p in enumerate(positions):
        target = moves.get(key(p))
        if target is not None:
            positions[i] = target
            moved += 1
    return moved, len(moves)


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
    # Round 33 (G9, Ryan: "from the graveyard side of the gate, W101 street models poke through the wall"): the cut
    # used to be skipped for your own edit ("it decides for itself what's left of the archway"), but your edit
    # (WC_Z06_Unicorn_Way_Classic_edited.obj, 2026-10-21) still has 69 triangles inside the archway box (30 grass,
    # 30 street, 9 WC_Wall_01), the ramp and walls that stick into the Stranded Graveyard's lift landing behind the
    # door. Both OBJs get it cut now.
    tunnel = [f for f in faces if tunnel_inside(positions, f)]
    faces = [f for f in faces if not backdrop(positions, f) and not tunnel_inside(positions, f)]
    print(f"Left out {len(walls)} triangles of the zone's backdrop walls and {len(tunnel)} inside the Commons archway")
    if GRAVEYARD_FIX.exists():  # round 35 (G12c)
        moved, listed = graveyard_fix(positions, GRAVEYARD_FIX)
        print(f"Stranded Graveyard behind the door: {moved} vertices moved out of it ({listed} positions listed)")
    else:
        print(f"No {GRAVEYARD_FIX.name}: the street pokes into the Stranded Graveyard (run street_graveyard_fix.py)")
    # Round 26 (W2): one window state per building (w101_window_states.py): lit (_on) where a door leads in, dark
    # (_off) elsewhere, no blue _quest copies (that is asset aeg030_529, switched by its event).
    if WINDOW_STATES.exists():
        drop, counts = w101_window_states.window_filter(positions, uvs, faces, w101_window_states.load(WINDOW_STATES))
        faces = [f for k, f in enumerate(faces) if k not in drop]
        print(f"Window states: {len(drop)} window triangles left out; lit: "
              f"{sorted(w101_window_states.ENTERABLE)}; matched {sum(counts.values())}")
    else:
        print(f"No {WINDOW_STATES}: every window state is drawn on top of the others (run w101_window_states.py)")
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
    reference = flver_build.read_normals(NORMALS) if NORMALS.exists() else None
    if reference is None:
        print(f"No {NORMALS.name}: normals are smoothed from the geometry instead of W101's own")
    drawn = [m in texture_of_material for m, _ in faces]  # untextured faces are dropped by split_materials anyway
    normals, used = flver_build.corner_normals(positions, [f for f, d in zip(faces, drawn) if d], reference)
    normals = iter(normals)
    normals = [next(normals) if d else None for d in drawn]
    print(f"Smooth normals: {used['w101']} corners from W101, {used['smoothed']} averaged from the geometry")
    single, materials = flver_build.build_single_mesh(template, positions, uvs, faces, normals=normals)
    stems = sorted(set(texture_of_material.values()))
    raw = [(CLASSIC / "textures" / f"{stem}.dds").read_bytes() for stem in stems]
    alpha = {i for i, data in enumerate(raw) if tpf.has_alpha(data)}
    # Round 35 (G13): opaque textures (m60_fort_220) get OPAQUE_BRIGHTNESS on top; alpha-tested ones (m60_fort_215) not.
    dds = [dds_color.boost(data, SATURATION, BRIGHTNESS * (1.0 if i in alpha else OPAQUE_BRIGHTNESS))
           for i, data in enumerate(raw)]
    alpha = {i for i, data in enumerate(dds) if tpf.has_alpha(data)}
    face_textures = [stems.index(texture_of_material[m]) if m in texture_of_material else None for m in materials]
    split = flver_textures.split_materials(single, vanilla, face_textures, stems, alpha)
    binder = Binder.from_path(SINGLE_MESH)
    entry = next(e for e in binder.entries if e.name.lower().endswith(".flver"))
    entry.set_uncompressed_data(split)
    binder.write(GEOMBND)
    print(f"Wrote {GEOMBND}: {len(faces)} triangles, {len(stems)} textures ({len(alpha)} with transparency)")

    textures = [(flver_textures.texture_name(i), data) for i, data in enumerate(dds)]
    textures.append((flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, GLOSS)))
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
