"""Write the HKX collision files of the vanilla and the generated aeg030_518_l uncompressed to reference/eldenring/work/dump,
so they can be compared outside Blender (only Soulstruct can decompress them).

Run in Blender's Scripting tab.
"""

from pathlib import Path

from soulstruct.containers import Binder

ROOT = Path.home() / "Documents" / "game_modding"
NAME = "aeg030_518_l.geomhkxbnd.dcx"
PROJECT = ROOT / "reference" / "eldenring" / "project" / "asset" / "aeg" / "aeg030"
SOURCES = {
    "vanilla": Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset\aeg\aeg030") / NAME,
    "generated": next(p for p in (PROJECT / NAME, PROJECT / (NAME + ".off")) if p.exists()),
}
OUTPUT = ROOT / "reference" / "eldenring" / "work" / "dump"
OUTPUT.mkdir(parents=True, exist_ok=True)

for label, path in SOURCES.items():
    for entry in Binder.from_path(path).entries:
        target = OUTPUT / f"{label}_{Path(entry.name).name}"
        target.write_bytes(entry.get_uncompressed_data())
        print(f"{path.name}: {entry.name} -> {target.name}")
