"""Run Soulstruct's "Export Asset" on the selected asset FLVER (e.g. aeg030_518).

Soulstruct 3.2.1 has no panel button for it and looks for the GEOMBND in asset/aegXXX/, but Elden Ring keeps
assets in asset/aeg/aegXXX/. This patches the path and runs the export into the Soulstruct Project Root.
"""

import sys
from pathlib import Path

import bpy

module = next(m for name, m in sys.modules.items() if name.endswith("flver.models.operators.export_operators"))


def binder_path(settings, model_stem):
    stem = model_stem.lower()
    return Path(f"asset/aeg/{stem[:6]}/{stem}.geombnd")


module.ExportAssetFLVER._get_binder_path = staticmethod(binder_path)
print(bpy.ops.export_scene.asset_flver())
