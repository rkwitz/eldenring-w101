"""Build real collision for the Unicorn Way street from its model and put it in the Smithbox project.

Takes the game's own aeg030_518_l collision binder as the template, replaces its HKX with an Elden Ring compressed
mesh of every triangle of the exported street (hkx_collision.py), checks the result by reading it back, and writes the
binder to the project's asset folder. An existing project copy is kept as aeg030_518_l.geomhkxbnd.dcx.before_writer;
renaming the new file away (e.g. to .off) makes the game use the vanilla road's collision again.

Run in Blender's Scripting tab. It takes about a minute; watch Window > Toggle System Console.
"""

import importlib
import shutil
import sys
import time
from pathlib import Path

from soulstruct.containers import Binder

ROOT = Path.home() / "Documents" / "game_modding"
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import flver_fix  # noqa: E402
import flver_floor  # noqa: E402
import hkx_collision  # noqa: E402
import hkx_tag  # noqa: E402

for module in (flver_fix, flver_floor, hkx_tag, hkx_collision):
    importlib.reload(module)

ASSETS = ROOT / "reference" / "eldenring" / "project" / "asset" / "aeg" / "aeg030"
GEOMBND = ASSETS / "aeg030_518.geombnd.dcx"
TARGET = ASSETS / "aeg030_518_l.geomhkxbnd.dcx"
GAME = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset\aeg\aeg030") / TARGET.name

start = time.time()
triangles = list(flver_floor._triangles(flver_fix.flver_in_binder(GEOMBND)))
print(f"Read {len(triangles)} triangles from {GEOMBND.name}")

binder = Binder.from_path(GAME)
entry = next(e for e in binder.entries if e.name.lower().endswith(".hkx"))
data, _ = hkx_collision.build_hkx(entry.get_uncompressed_data(), triangles)
print(f"Built {len(data)} bytes of collision in {time.time() - start:.0f} s; checking it")

_, root = hkx_tag.read(data)
decoded = hkx_collision.decode(hkx_collision.body_of(root)["shape"])
print(f"{len(decoded)} triangles read back ({len(triangles) - len(decoded)} degenerate ones dropped)")

if TARGET.exists():
    backup = TARGET.with_name(TARGET.name + ".before_writer")
    if not backup.exists():
        shutil.copy2(TARGET, backup)
entry.set_uncompressed_data(data)
binder.write(TARGET)
print(f"Wrote {TARGET} in {time.time() - start:.0f} s")
