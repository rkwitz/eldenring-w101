"""Build Wizard101's Bone Cage (round 22 P2) as two Elden Ring assets; msb_add_quests.py places them, emevd_quests.py
swaps them when a cage is opened.

  aeg030_558  closed cage: the street lamp with the bone cage hanging off its arm (w101_quests.CAGE_MODEL)
  aeg030_559  the same with the cage door swung open (w101_quests.CAGE_OPEN_MODEL)

Inputs (tools/w101_bone_cage.py's docstring has the commands; Wizard101 files, so not in the repo):
  reference/w101/converted_bonecage/WC_BoneCage.obj, WC_BoneCage_open.obj (+ .mtl) and textures/*.dds
Both slots are unnamed vanilla pieces our maps don't use (as aeg030_555 / 557); deleting the written files gives them
back. Uses blender_build_interiors.build_interior with lit materials (the street's assets are lit, not emissive),
keeping the cage door (the house build leaves out "Editable Poly" objects) and leaving the glow cards / particle
planes out. If the game has no collision binder for a slot, aeg030_557's (a vanilla binder of one HKX) is used as the
template with its name changed.

Run in Blender's Scripting tab like blender_build_interiors.py, then relaunch the game.
"""

import os
import re
import tempfile
from pathlib import Path

ROOT = Path(os.environ.get("W101ER_ROOT", Path.home() / "Documents" / "game_modding"))
TOOLS = ROOT / "eldenring-w101" / "tools"
CAGE = ROOT / "reference" / "w101" / "converted_bonecage"
BUILDS = [("aeg030_558", CAGE / "WC_BoneCage.obj"), ("aeg030_559", CAGE / "WC_BoneCage_open.obj")]
COLLISION_TEMPLATE = "aeg030_557"

source = (TOOLS / "blender_build_interiors.py").read_text(encoding="utf-8")
source = source[:source.rindex("\nbuild_interior(")]  # its definitions only, not the house build
ns = {"__name__": "blender_build_interiors", "__file__": str(TOOLS / "blender_build_interiors.py")}
exec(compile(source, ns["__file__"], "exec"), ns)

ns["UNLIT_GLOW"] = 0.0  # lit, like the street
ns["LEFT_OUT"] = re.compile(r"(?i)^fx_|^shadow_|^plane|^box$")  # lamp glow, shadow card, fairy glow planes, helper box
ns["NO_COLLISION"] = re.compile(r"(?i)^object118\d$|^object119\d$")  # the lamp's thin alpha cards and the cage's bars

game = ns["GAME"]
temp_game = Path(tempfile.mkdtemp(prefix="w101_cage_"))
for asset, obj in BUILDS:
    collision = game / "aeg" / "aeg030" / f"{asset}_l.geomhkxbnd.dcx"
    if collision.exists():
        ns["GAME"] = game
    else:
        data, dcx_type = ns["decompress"](game / "aeg" / "aeg030" / f"{COLLISION_TEMPLATE}_l.geomhkxbnd.dcx")
        for old, new in ((COLLISION_TEMPLATE, asset), (COLLISION_TEMPLATE.upper(), asset.upper())):
            for enc in ("ascii", "utf-16-le"):
                data = data.replace(old.encode(enc), new.encode(enc))
        folder = temp_game / "aeg" / "aeg030"
        folder.mkdir(parents=True, exist_ok=True)
        (folder / collision.name).write_bytes(ns["compress"](data, dcx_type))
        ns["GAME"] = temp_game
        print(f"{asset}: no vanilla collision binder; using {COLLISION_TEMPLATE}'s renamed")
    ns["build_interior"](asset, obj)
ns["GAME"] = game
print("Bone cages built (aeg030_558 closed, aeg030_559 open). Relaunch the game.")
