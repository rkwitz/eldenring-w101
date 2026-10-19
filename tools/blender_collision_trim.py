"""After saving the generated collision in Smithbox, trim the padding blender_collision_room.py added."""

import runpy
from pathlib import Path

runpy.run_path(
    str(Path.home() / "Documents" / "game_modding" / "eldenring-w101" / "tools" / "blender_collision_room.py"),
    init_globals={"MODE": "trim"},
)
