"""Export a simplified copy of the asset FLVER (e.g. aeg030_518) for Smithbox's collision generator.

Smithbox crashes building collision from the full Unicorn Way mesh (~160k triangles), so this decimates a temporary
copy to about TARGET_TRIANGLES, exports it as an uncompressed loose .flver to reference/eldenring/work/, fixes it with
flver_fix.py and deletes the copy. In Smithbox, use "Select External FLVER" on the written file.
"""

import importlib
import re
import sys
from pathlib import Path

import bpy

TEMPLATE = "aeg030_518"
TARGET_TRIANGLES = 15000

ROOT = Path.home() / "Documents" / "game_modding"
OUTPUT = ROOT / "reference" / "eldenring" / "work" / f"{TEMPLATE}_collision.flver"
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import flver_fix  # noqa: E402

importlib.reload(flver_fix)

source = next(o for o in bpy.data.objects if o.type == "MESH" and o.name.lower().startswith(TEMPLATE))
if bpy.context.mode != "OBJECT":
    bpy.ops.object.mode_set(mode="OBJECT")

copy = source.copy()
copy.data = source.data.copy()
copy.name = f"{source.name} Collision"
for collection in source.users_collection:
    collection.objects.link(copy)

triangles = sum(len(p.vertices) - 2 for p in copy.data.polygons)
ratio = min(1.0, TARGET_TRIANGLES / max(triangles, 1))
print(f"Decimating {triangles} triangles by {ratio:.3f}")

bpy.ops.object.select_all(action="DESELECT")
copy.select_set(True)
bpy.context.view_layer.objects.active = copy
try:
    modifier = copy.modifiers.new("Collision Decimate", "DECIMATE")
    modifier.ratio = ratio
    bpy.ops.object.modifier_apply(modifier=modifier.name)

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    for _ in range(8):
        try:
            print(bpy.ops.export_scene.flver(filepath=str(OUTPUT), dcx_type="Null"))
            break
        except RuntimeError as ex:
            unused = re.search(r"Error: '([^']+)'$", str(ex).strip())
            if not unused or unused.group(1) not in copy.data.uv_layers:
                raise
            copy.data.uv_layers.remove(copy.data.uv_layers[unused.group(1)])
finally:
    mesh = copy.data
    bpy.data.objects.remove(copy)
    bpy.data.meshes.remove(mesh)
    source.select_set(True)
    bpy.context.view_layer.objects.active = source

data = OUTPUT.read_bytes()
OUTPUT.write_bytes(flver_fix.fix_flver(data))
print(f"Wrote {OUTPUT} ({len(data) // 1024} KB). In Smithbox: Collision Generator > Select External FLVER.")
