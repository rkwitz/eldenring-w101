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

GEOMBND = ROOT / "reference" / "eldenring" / "project" / "asset" / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"
print("Fixed" if flver_fix.fix_binder(GEOMBND) else "Already fixed", GEOMBND)
