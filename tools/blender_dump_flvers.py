"""Write uncompressed copies of the exported and vanilla aeg030_518 FLVERs to reference/eldenring/work/dump/.

Elden Ring compresses files with Oodle, which only Soulstruct (inside Blender) can decompress on this PC. Run this in
Blender's Scripting tab so the raw FLVERs can be compared byte for byte.
"""

from pathlib import Path

from soulstruct.containers import Binder

ROOT = Path.home() / "Documents" / "game_modding"
GAME_ASSET = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\asset\aeg\aeg030")
SOURCES = {
    "vanilla": GAME_ASSET / "aeg030_518.geombnd.dcx",
    "export": ROOT / "reference" / "eldenring" / "project" / "asset" / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx",
}

out_dir = ROOT / "reference" / "eldenring" / "work" / "dump"
out_dir.mkdir(parents=True, exist_ok=True)
for label, path in SOURCES.items():
    binder = Binder.from_path(path)
    for entry in binder.entries:
        if entry.name.lower().endswith(".flver"):
            out = out_dir / f"{label}_{entry.name}"
            out.write_bytes(entry.get_uncompressed_data())
            print(f"Wrote {out}")
