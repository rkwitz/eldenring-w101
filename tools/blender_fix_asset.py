"""Fix the exported Unicorn Way asset so Smithbox (and the game) can read it. See flver_fix.py for what it changes.

Run in Blender's Scripting tab. It rewrites the GEOMBND in the Smithbox project folder in place.
"""

import os
import importlib
import sys
from pathlib import Path

ROOT = Path(os.environ.get("W101ER_ROOT", Path.home() / "Documents" / "game_modding"))
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import flver_fix  # noqa: E402

importlib.reload(flver_fix)  # pick up edits without restarting Blender

NAME = "aeg030_518.geombnd.dcx"
GEOMBND = ROOT / "reference" / "eldenring" / "project" / "asset" / "aeg" / "aeg030" / NAME
VANILLA = Path(os.environ.get("ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game")) / r"asset\aeg\aeg030" / NAME

# Match the vertex layout and texture paths the game's own asset uses for material 0, which is the one our mesh keeps.
vanilla = flver_fix.flver_in_binder(VANILLA)
print("Vertex layout:", flver_fix.material_layout(vanilla))
print("Fixed" if flver_fix.fix_binder(GEOMBND, vanilla) else "Already fixed", GEOMBND)
