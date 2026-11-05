"""Round 26 (W3): Rattlebones' Giant Skeleton Torso (c4960) can no longer turn invisible.

Reads the game's chr/c4960.anibnd.dcx, sets every TAE opacity event to 1 (er_chr_opaque.py: why, and what changes)
and writes it to the Smithbox project's chr folder (ModEngine loads it over the game's), in the game's own DCX
compression. Deleting project/chr/c4960.anibnd.dcx gives back the vanilla one. Run by blender_build_all.py (step
"boss") or alone in Blender's Scripting tab; relaunch the game afterwards.
"""
import os
import importlib
import sys
from pathlib import Path
try:  # config.toml at the repo root (tools/w101er_config.py), then the default
    from w101er_config import setting as _setting
except ImportError:
    _setting = os.environ.get

from soulstruct.dcx import compress, decompress

ROOT = Path(_setting("W101ER_ROOT", Path.home() / "Documents" / "game_modding"))
sys.path.insert(0, str(Path(os.environ.get("W101ER_TOOLS", ROOT / "eldenring-w101" / "tools"))))
import er_chr_opaque  # noqa: E402
import er_text  # noqa: E402

for module in (er_text, er_chr_opaque):
    importlib.reload(module)

GAME = Path(_setting("ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game")) / r"chr"
PROJECT = ROOT / "reference" / "eldenring" / "project" / "chr"
CHARACTERS = ["c4960"]  # Rattlebones (msb_add_boss.BOSSES)

for chr_id in CHARACTERS:
    source = GAME / f"{chr_id}.anibnd.dcx"
    data, dcx_type = decompress(source)
    new, changes = er_chr_opaque.opaque(bytes(data))
    left = er_chr_opaque.check(new)
    assert not left, f"{chr_id}: opacity events still below 1: {left[:5]}"
    PROJECT.mkdir(parents=True, exist_ok=True)
    target = PROJECT / source.name
    target.write_bytes(compress(new, dcx_type))
    print(f"{chr_id}: {len(changes)} opacity events set to 1 in {len({a for a, _ in changes})} animations "
          f"-> {target} ({dcx_type.name})")
