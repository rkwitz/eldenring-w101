"""Run Soulstruct's "Export Asset" on the selected asset FLVER (e.g. aeg030_518).

Soulstruct 3.2.1 has no panel button for it and looks for the GEOMBND in asset/aegXXX/, but Elden Ring keeps
assets in asset/aeg/aegXXX/. This patches the path and runs the export into the Soulstruct Project Root.

Soulstruct also fails with a bare KeyError ("Error: 'UVTexture1'") when the mesh keeps a UV layer that its material
doesn't use, so those layers are removed and the export retried. Afterwards blender_fix_asset.py adds what Soulstruct
leaves out of Elden Ring FLVERs, so set Soulstruct's Project Root to reference/eldenring/project.
"""

import os
import re
import runpy
import sys
from pathlib import Path

import bpy

module = next(m for name, m in sys.modules.items() if name.endswith("flver.models.operators.export_operators"))


def binder_path(settings, model_stem):
    stem = model_stem.lower()
    return Path(f"asset/aeg/{stem[:6]}/{stem}.geombnd")


module.ExportAssetFLVER._get_binder_path = staticmethod(binder_path)
obj = bpy.context.active_object
if obj.type != "MESH":
    obj = next(child for child in obj.children if child.type == "MESH")
mesh = obj.data
for _ in range(8):
    try:
        print(bpy.ops.export_scene.asset_flver())
        break
    except RuntimeError as ex:
        unused = re.search(r"Error: '([^']+)'$", str(ex).strip())
        if not unused or unused.group(1) not in mesh.uv_layers:
            raise
        print(f"Removing UV layer '{unused.group(1)}', which the material doesn't use, and exporting again")
        mesh.uv_layers.remove(mesh.uv_layers[unused.group(1)])

runpy.run_path(str(Path(os.environ.get("W101ER_ROOT", Path.home() / "Documents" / "game_modding")) / "eldenring-w101" / "tools" / "blender_fix_asset.py"))
