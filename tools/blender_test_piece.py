"""Swap the street asset for a small test piece of it (see flver_test_piece.py) to find out why the game won't draw it.

Run in Blender's Scripting tab after blender_fix_asset.py. The first run keeps the full street as
aeg030_518.geombnd.dcx.full next to the asset; the piece is always cut from that copy.
"""

import importlib
import shutil
import sys
from pathlib import Path

from soulstruct.containers import Binder

ROOT = Path.home() / "Documents" / "game_modding"
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import flver_fix  # noqa: E402
import flver_test_piece  # noqa: E402

importlib.reload(flver_fix)
importlib.reload(flver_test_piece)

GEOMBND = ROOT / "reference" / "eldenring" / "project" / "asset" / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"
FULL = GEOMBND.with_name(GEOMBND.name + ".full")

if not FULL.exists():
    shutil.copy2(GEOMBND, FULL)
binder = Binder.from_path(FULL)
entry = next(e for e in binder.entries if e.name.lower().endswith(".flver"))
piece = flver_test_piece.small_piece(entry.get_uncompressed_data())
entry.set_uncompressed_data(piece)
binder.write(GEOMBND)
print(f"Wrote a {len(piece) // 1024} KB test piece to {GEOMBND} (full street kept in {FULL.name})")
