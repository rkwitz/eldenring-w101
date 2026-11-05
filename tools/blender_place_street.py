"""Place the Unicorn Way street (asset aeg030_518) at The First Step, in Limgrave map tile m60_42_36_00.

Reads the Smithbox project's copy of the map if there is one, otherwise the game's, and writes the project copy (the
first run over an existing project copy keeps it as .before_street). The street's origin (on the road) lands on The First
Step's grace, with the road surface at the grace's height; rerunning moves the street there again, so change
GRACE or the offsets below to move it.

Close Smithbox first (or reload the map afterwards), then run in Blender's Scripting tab.
"""

import os
import importlib
import shutil
import sys
from pathlib import Path

from soulstruct.dcx import compress, decompress

ROOT = Path(os.environ.get("W101ER_ROOT", Path.home() / "Documents" / "game_modding"))
sys.path.insert(0, str(ROOT / "eldenring-w101" / "tools"))
import msb_floor  # noqa: E402

importlib.reload(msb_floor)

MAP_NAME = "m60_42_36_00"
MAPSTUDIO = Path("map") / "mapstudio" / f"{MAP_NAME}.msb.dcx"
TARGET = ROOT / "reference" / "eldenring" / "project" / MAPSTUDIO
GAME = Path(os.environ.get("ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game")) / MAPSTUDIO
GRACE = (-12.83, 90.7, -54.5)  # The First Step grace (AEG099_090_9011, entity 1042361981)
# The point of the street model that lands on the grace, in the model's own coordinates. It is the model's origin
# (which is on the road): the game seems to hide assets by the distance from the player to their origin, and with
# (-126.4, 0.0, 116.2), 170 m from the origin, the street was invisible at the grace though its collision was there.
ROAD_CENTER = (0.0, 0.0, 0.0)
ROTATION = (0.0, 0.0, 0.0)

source = TARGET if TARGET.exists() else GAME
data, dcx_type = decompress(source)
assert msb_floor.write_msb(*msb_floor.read_msb(data)) == data, "map does not round-trip"
position = tuple(GRACE[k] - ROAD_CENTER[k] for k in range(3))
data = msb_floor.place_asset(data, "AEG030_518", "AEG003_061", "AEG003_061_1000", msb_floor.STREET, position, ROTATION)

if TARGET.exists():
    backup = TARGET.with_name(TARGET.name + ".before_street")
    if not backup.exists():
        shutil.copy2(TARGET, backup)
TARGET.parent.mkdir(parents=True, exist_ok=True)
TARGET.write_bytes(compress(data, dcx_type))
print(f"Placed {msb_floor.STREET} at {position} in {TARGET} (from {source.name}, {dcx_type.name})")
