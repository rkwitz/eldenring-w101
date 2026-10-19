"""Lay invisible floor under the Unicorn Way street in the Stranded Graveyard map (see flver_floor.py).

Each tile gets two aeg099_002 invisible walls laid flat, one tipped +90 degrees about X and one -90, so together they
cover the tile whichever way the game turns them. The script also levels the street with the grace room floor and
clears its rotation. Rerunning replaces the floor it placed before. The first run keeps the map as
m18_00_00_00.msb.dcx.before_floor.

Close Smithbox first (or reload the map afterwards), then run in Blender's Scripting tab.
"""

import importlib
import shutil
import sys
from pathlib import Path

from soulstruct.containers import Binder
from soulstruct.eldenring.maps.msb import MSB

ROOT = Path.home() / "Documents" / "game_modding"
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import flver_floor  # noqa: E402

importlib.reload(flver_floor)

PROJECT = ROOT / "reference" / "eldenring" / "project"
MAP = PROJECT / "map" / "mapstudio" / "m18_00_00_00.msb.dcx"
GEOMBND = PROJECT / "asset" / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"
STREET = "AEG030_518_9000"
FLOOR_MODEL = "AEG099_002"
FLOOR_PREFIX = FLOOR_MODEL + "_8"  # part names AEG099_002_8000 and up
GROUND_Y = -10.685  # grace room floor; the road surface sits at the street's own height 0

backup = MAP.with_name(MAP.name + ".before_floor")
if not backup.exists():
    shutil.copy2(MAP, backup)

flver = next(e for e in Binder.from_path(GEOMBND).entries if e.name.lower().endswith(".flver"))
tiles = flver_floor.floor_tiles(flver.get_uncompressed_data())

msb = MSB.from_path(MAP)
street = msb.assets.find_entry_name(STREET)
street.translate = type(street.translate)((street.translate.x, GROUND_Y, street.translate.z))
street.rotate = type(street.rotate)((0.0, 0.0, 0.0))

for old in [part for part in msb.assets if part.name.startswith(FLOOR_PREFIX)]:
    msb.assets.remove(old)

model = next((m for m in msb.asset_models if m.name == FLOOR_MODEL), None)
if model is None:
    model = msb.asset_models.duplicate(street.model, index_offset=-1, name=FLOOR_MODEL)
    model.sib_path = f"N:\\GR\\data\\Asset\\Environment\\geometry\\AEG099\\{FLOOR_MODEL}\\sib\\{FLOOR_MODEL}.sib"

count = 0
for x, height, z in tiles:
    for tilt in (90.0, -90.0):
        part = msb.assets.duplicate(street, index_offset=-1, name=f"{FLOOR_PREFIX}{count:03d}")
        part.model = model
        part.entity_id = 0
        part.translate = type(street.translate)((
            street.translate.x + x, street.translate.y + height - flver_floor.PIECE_HALF_THICKNESS,
            street.translate.z + z))
        part.rotate = type(street.rotate)((tilt, 0.0, 0.0))
        count += 1

msb.write(MAP, force=True)
print(f"Placed {count} floor pieces over {len(tiles)} tiles in {MAP.name}; street at y={GROUND_Y}")
