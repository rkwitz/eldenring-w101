"""Build real collision for the Unicorn Way street from its model and put it in the Smithbox project.

Takes the game's own aeg030_518_l collision binder as the template, replaces its HKX with an Elden Ring compressed
mesh of every triangle of the exported street (hkx_collision.py), checks the result by reading it back, and writes the
binder to the project's asset folder. An existing project copy is kept as aeg030_518_l.geomhkxbnd.dcx.before_writer;
renaming the new file away (e.g. to .off) makes the game use the vanilla road's collision again.

SOURCE = "vanilla" rebuilds the game's own aeg030_518_l collision (154 triangles) with our writer instead, and LIMIT keeps only that
many street triangles nearest the road centre: both are for finding out why the game crashes on the full street.

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
SOURCE = "vanilla"  # "street" (the exported Unicorn Way) or "vanilla" (the game's own road collision)
LIMIT = None  # e.g. 2000: keep only the street triangles nearest ROAD_CENTER
ROAD_CENTER = (-126.4, 0.0, 116.2)

start = time.time()
binder = Binder.from_path(GAME)
entry = next(e for e in binder.entries if e.name.lower().endswith(".hkx"))
if SOURCE == "street":
    model = GEOMBND
    triangles = list(flver_floor._triangles(flver_fix.flver_in_binder(GEOMBND)))
else:
    model = GAME
    triangles = list(hkx_collision.decode(hkx_collision.body_of(hkx_tag.read(entry.get_uncompressed_data())[1])["shape"]).values())
if LIMIT:
    triangles.sort(key=lambda t: sum((sum(v[k] for v in t) / 3 - ROAD_CENTER[k]) ** 2 for k in range(3)))
    triangles = triangles[:LIMIT]
print(f"Read {len(triangles)} triangles from {model}")

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
