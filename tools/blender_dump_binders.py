"""Write the game's aeg030_518_l collision binder and the project's pass-through test copy of it (renamed .passthrough) uncompressed to reference/eldenring/work/dump, so
the two BND4 files can be compared byte by byte outside Blender (only Soulstruct can undo their Oodle compression).

Run in Blender's Scripting tab.
"""

from pathlib import Path

from soulstruct.dcx import decompress

ROOT = Path.home() / "Documents" / "game_modding"
NAME = "aeg030_518_l.geomhkxbnd.dcx"
SOURCES = {
    "game": Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset\aeg\aeg030") / NAME,
    "project": ROOT / "reference" / "eldenring" / "project" / "asset" / "aeg" / "aeg030" / (NAME + ".passthrough"),
}
OUTPUT = ROOT / "reference" / "eldenring" / "work" / "dump"

for label, path in SOURCES.items():
    data, dcx_type = decompress(path)
    target = OUTPUT / f"{label}_aeg030_518_l.geomhkxbnd"
    target.write_bytes(data)
    print(f"{path} ({dcx_type.name}) -> {target.name}, {len(data)} bytes")
