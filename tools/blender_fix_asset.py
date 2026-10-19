"""Fix the exported Unicorn Way asset so Smithbox (and the game) can read it. See flver_fix.py for what it changes.

Run in Blender's Scripting tab. It rewrites the GEOMBND in the Smithbox project folder in place.
"""

import importlib
import sys
from pathlib import Path

ROOT = Path.home() / "Documents" / "game_modding"
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import flver_fix  # noqa: E402

importlib.reload(flver_fix)  # pick up edits without restarting Blender

NAME = "aeg030_518.geombnd.dcx"
GEOMBND = ROOT / "reference" / "eldenring" / "project" / "asset" / "aeg" / "aeg030" / NAME
VANILLA = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset\aeg\aeg030") / NAME

# Copy the vertex layout the game's own asset uses for material 0, which is the one our mesh keeps.
layout = flver_fix.material_layout(flver_fix.flver_in_binder(VANILLA))
print("Vertex layout:", layout)
print("Fixed" if flver_fix.fix_binder(GEOMBND, layout) else "Already fixed", GEOMBND)
