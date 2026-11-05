"""Run one of our Blender scripts without Blender's window (round 35, R-BUILD; build.py calls it).

  blender --background --python blender_headless.py -- blender_build_sky.py

Most of our "Blender" scripts only use Soulstruct (installed in Blender as the io_soulstruct extension) for Oodle
KRAK files, so they need no window. In the GUI the add-on loads oo2core_6_win64.dll from the scene's game folder
when a .blend opens; that never happens in background mode, so this loads it from ER_GAME first. The tools folder
(W101ER_TOOLS, default this script's folder) goes first on sys.path, then the script runs as __main__.
"""
import os
import runpy
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
os.environ.setdefault("W101ER_TOOLS", str(HERE))
sys.path.insert(0, os.environ["W101ER_TOOLS"])
try:  # config.toml at the repo root (tools/w101er_config.py), then the default
    from w101er_config import setting as _setting
except ImportError:
    _setting = os.environ.get

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
if not argv:
    raise SystemExit("usage: blender --background --python blender_headless.py -- SCRIPT.py [args]")
script = Path(argv[0])
if not script.is_absolute():
    script = Path(os.environ["W101ER_TOOLS"]) / script

try:
    import soulstruct  # noqa: F401
except ImportError:
    raise SystemExit("Soulstruct is not importable in this Blender: install the Soulstruct for Blender extension "
                     "(Edit > Preferences > Add-ons > Install from Disk) and enable it")
try:
    from soulstruct.dcx import oodle
    game = Path(_setting("ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game"))
    dll = game / "oo2core_6_win64.dll"
    oodle.LOAD_DLL(str(dll) if dll.exists() else "")
except Exception as error:  # the script itself reports what it can't open
    print(f"blender_headless: could not load the Oodle DLL ({error}); KRAK files will fail")

sys.argv = [str(script)] + argv[1:]
runpy.run_path(str(script), run_name="__main__")
