"""Rebuild everything Unicorn Way needs in the Smithbox project, in one run.

Run in Blender's Scripting tab (watch Window > Toggle System Console); then relaunch the game. Each step is read fresh
from its file on disk, so an old copy open in Blender's text editor can't be run by mistake. Steps, in order:

  street     blender_switch_street.py     street model, textures and collision (aeg030_518)
  footprint  blender_clear_footprint.py   hide Elden Ring ground, rocks and props under the street, in every map
  sky        blender_build_sky.py         sky dome (aeg030_523) and the no-fog, no-sun lighting files
  room       blender_build_room.py        Rattlebones' and Lady Blackhope's tower room (aeg030_519)
  garden     blender_build_garden.py      hedge-maze garden (aeg030_555) and the glowing quest window (aeg030_529)
  interiors  blender_build_interiors.py   Fear of Fire's house (aeg030_557)
  cage       blender_build_cage.py        Wizard101's Bone Cage, closed and open (aeg030_558 / 559)
  fog        blender_build_fog.py         the Commons gate's fog, seen from anywhere (aeg030_530)
  boss       blender_build_boss.py        Rattlebones (c4960) never fades out of sight (project chr/c4960.anibnd.dcx)
  portraits  blender_npc_portraits.py     the NPCs' dialogue portraits, rendered from the game's models (orbs/w101_orbs/book/
                                          art/GUI/NpcPortraits/ER; a few minutes; SKIP = {"portraits"} once they are made)

SKIP leaves steps out (e.g. SKIP = {"street", "footprint"} when only a room changed). A failing step is reported and
the rest still run. Map placement, events and the navmesh are not rebuilt here; they are already in the project.
Reload the maps in Smithbox before saving them after a run.
"""

import runpy
import time
import traceback
from pathlib import Path

SKIP = set()

TOOLS = Path.home() / "Documents" / "game_modding" / "eldenring-w101" / "tools"
STEPS = [
    ("street", "blender_switch_street.py"),
    ("footprint", "blender_clear_footprint.py"),
    ("sky", "blender_build_sky.py"),
    ("room", "blender_build_room.py"),
    ("garden", "blender_build_garden.py"),
    ("interiors", "blender_build_interiors.py"),
    ("cage", "blender_build_cage.py"),
    ("fog", "blender_build_fog.py"),
    ("boss", "blender_build_boss.py"),
    ("portraits", "blender_npc_portraits.py"),
]

results = []
for name, script in STEPS:
    if name in SKIP:
        results.append((name, "skipped", 0.0))
        continue
    print(f"\n===== {name}: {script} =====")
    start = time.time()
    try:
        runpy.run_path(str(TOOLS / script), run_name="__main__")
        results.append((name, "ok", time.time() - start))
    except Exception:  # keep going so one broken step doesn't block the others
        traceback.print_exc()
        results.append((name, "FAILED", time.time() - start))

print("\n===== Summary =====")
for name, status, seconds in results:
    print(f"{name:10} {status:8} {seconds:6.1f} s")
failed = [name for name, status, _ in results if status == "FAILED"]
print("Some steps failed; see the errors above." if failed else "All done. Relaunch the game.")
