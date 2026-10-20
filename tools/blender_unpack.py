"""Decompress Elden Ring game files that the cloud tools can't read (Oodle "KRAK" DCX) into reference/eldenring/unpacked.

Run in Blender's Scripting tab (it only uses Soulstruct's DCX support). Edit FILES to add more; paths are relative to
the game folder. Nothing in the game or the Smithbox project is changed.
"""

from pathlib import Path

from soulstruct.dcx import decompress

GAME = Path(r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game")
OUT = Path.home() / "Documents" / "game_modding" / "reference" / "eldenring" / "unpacked"
FILES = [
    "map/mapstudio/m60_42_36_00.msb.dcx",
    "map/mapstudio/m60_21_18_01.msb.dcx",
    "map/mapstudio/m60_10_09_02.msb.dcx",
    "map/mapstudio/m60_42_37_00.msb.dcx",
    "map/mapstudio/m60_41_36_00.msb.dcx",
    "map/mapstudio/m60_42_35_00.msb.dcx",
    "map/mapstudio/m60_43_36_00.msb.dcx",
    "map/mapstudio/m18_00_00_00.msb.dcx",
    # Elden Ring sky domes, as templates for the Wizard101 sky
    "asset/aeg/aeg096/aeg096_000.geombnd.dcx",
    "asset/aeg/aeg096/aeg096_010.geombnd.dcx",
    "asset/aeg/aeg096/aeg096_101.geombnd.dcx",
    # Text (place names for the "Unicorn Way" title) and the common events, too big for the cloud's Oodle decoder
    "msg/engus/item.msgbnd.dcx",
    "msg/engus/item_dlc02.msgbnd.dcx",
    "msg/engus/menu.msgbnd.dcx",
    "event/common_func.emevd.dcx",
]

for name in FILES:
    source = GAME / name
    if not source.exists():
        print(f"Missing {source}")
        continue
    data, _ = decompress(source)
    target = OUT / name[: -len(".dcx")]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    print(f"{name}: {len(data):,} bytes")
print(f"Done, files are in {OUT}")
