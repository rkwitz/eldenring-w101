"""Lay invisible floor under the Unicorn Way street in the Stranded Graveyard map (see flver_floor.py and msb_floor.py).

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
from soulstruct.dcx import compress, decompress

ROOT = Path.home() / "Documents" / "game_modding"
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import flver_floor  # noqa: E402
import msb_floor  # noqa: E402

importlib.reload(flver_floor)
importlib.reload(msb_floor)

PROJECT = ROOT / "reference" / "eldenring" / "project"
MAP = PROJECT / "map" / "mapstudio" / "m18_00_00_00.msb.dcx"
GEOMBND = PROJECT / "asset" / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"

backup = MAP.with_name(MAP.name + ".before_floor")
if not backup.exists():
    shutil.copy2(MAP, backup)

flver = next(e for e in Binder.from_path(GEOMBND).entries if e.name.lower().endswith(".flver"))
tiles = flver_floor.floor_tiles(flver.get_uncompressed_data())

data, dcx_type = decompress(MAP)
assert msb_floor.write_msb(*msb_floor.read_msb(data)) == data, "map does not round-trip"
MAP.write_bytes(compress(msb_floor.lay_floor(data, tiles, flver_floor.PIECE_HALF_THICKNESS), dcx_type))
print(f"Wrote {MAP} ({dcx_type.name})")
