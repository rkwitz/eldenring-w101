"""Write the model (FLVER) and collision (HKX) of a few vanilla assets uncompressed to reference/eldenring/work/dump, so
they can be measured outside Blender (only Soulstruct can decompress them). Edit ASSETS to choose which.

Run in Blender's Scripting tab.
"""

from pathlib import Path

from soulstruct.containers import Binder

ASSETS = ["aeg099_331", "aeg099_002", "aeg099_239", "aeg099_001", "aeg099_003", "aeg099_251"]  # invisible collision

ROOT = Path.home() / "Documents" / "game_modding"
GAME_ASSETS = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset\aeg")
OUTPUT = ROOT / "reference" / "eldenring" / "work" / "dump"
OUTPUT.mkdir(parents=True, exist_ok=True)

for asset in ASSETS:
    for suffix in (".geombnd.dcx", "_l.geomhkxbnd.dcx"):
        path = GAME_ASSETS / asset[:6] / (asset + suffix)
        if not path.exists():
            print(f"Missing {path}")
            continue
        for entry in Binder.from_path(path).entries:
            target = OUTPUT / f"vanilla_{Path(entry.name).name}"
            target.write_bytes(entry.get_uncompressed_data())
            print(f"{path.name}: {entry.name} -> {target.name}")
