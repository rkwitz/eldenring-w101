"""Draw the Unicorn Way street and the invisible floor pieces under it, as the Stranded Graveyard map places them.

Makes two objects in the current scene: "Unicorn Way street" (the asset's model) and "Unicorn Way floor" (one red box
per aeg099_002 collision piece, from the boxes' sizes in flver_floor.py). Both are in the street's own coordinates, with
the game's y up turned into Blender's z up. Rerunning replaces them.

Run in Blender's Scripting tab.
"""

import importlib
import math
import struct
import sys
from pathlib import Path

import bpy
from soulstruct.containers import Binder
from soulstruct.dcx import decompress

ROOT = Path.home() / "Documents" / "game_modding"
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import flver_floor  # noqa: E402
import msb_floor  # noqa: E402

importlib.reload(flver_floor)
importlib.reload(msb_floor)

PROJECT = ROOT / "reference" / "eldenring" / "project"
MAP = PROJECT / "map" / "mapstudio" / "m18_00_00_00.msb.dcx"
GEOMBND = PROJECT / "asset" / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"
BOX = ((-7.466, 0.0, -0.065), (7.459, 8.343, 0.065))  # aeg099_002 collision, before the part's rotation


def blender(v):
    x, y, z = v
    return x, z, y


def rotate(v, degrees):
    """Game rotation (x, y, z) in degrees, applied X, then Y, then Z."""
    x, y, z = v
    a, b, c = (math.radians(d) for d in degrees)
    y, z = y * math.cos(a) - z * math.sin(a), y * math.sin(a) + z * math.cos(a)
    x, z = x * math.cos(b) + z * math.sin(b), -x * math.sin(b) + z * math.cos(b)
    x, y = x * math.cos(c) - y * math.sin(c), x * math.sin(c) + y * math.cos(c)
    return x, y, z


def replace_object(name, vertices, faces, colour):
    old = bpy.data.objects.get(name)
    if old is not None:
        bpy.data.objects.remove(old)
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(vertices, [], faces)
    obj = bpy.data.objects.new(name, mesh)
    material = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    material.diffuse_color = colour
    mesh.materials.append(material)
    bpy.context.scene.collection.objects.link(obj)
    return obj


flver = next(e for e in Binder.from_path(GEOMBND).entries if e.name.lower().endswith(".flver"))
vertices, faces = [], []
for triangle in flver_floor._triangles(flver.get_uncompressed_data()):
    faces.append((len(vertices), len(vertices) + 2, len(vertices) + 1))  # swapping y and z mirrors, so flip winding
    vertices += [blender(v) for v in triangle]
replace_object("Unicorn Way street", vertices, faces, (0.8, 0.7, 0.5, 1.0))

_, lists = msb_floor.read_msb(decompress(MAP)[0])
parts = next(p for p in lists if p.title() == "PARTS_PARAM_ST")
street, _ = msb_floor.transform(parts.entries[msb_floor.find(parts, msb_floor.STREET)])
vertices, faces = [], []
for entry in parts.entries:
    if not msb_floor.entry_name(entry).startswith(msb_floor.FLOOR_PREFIX):
        continue
    position, rotation = msb_floor.transform(entry)
    start = len(vertices)
    for i in range(8):
        corner = tuple(BOX[(i >> k) & 1][k] for k in range(3))
        turned = rotate(corner, rotation)
        vertices.append(blender([turned[k] + position[k] - street[k] for k in range(3)]))
    faces += [tuple(start + i for i in f) for f in
              ((0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3))]
replace_object("Unicorn Way floor", vertices, faces, (1.0, 0.1, 0.1, 1.0))
print(f"Drew the street and {len(faces) // 6} floor pieces")
