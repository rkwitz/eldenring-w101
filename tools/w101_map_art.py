"""Copy Wizard101's map art for the hud_orbs book's World Map page (hud_orbs/src/pages.rs, map section).

Wizard101's spellbook map (MapSpellbookPage.gui) draws a painted map per zone, kept in GUI-WorldData.wad:

  Maps/Map_WC_Unicorn_Way.dds   512x512, Unicorn Way. Maps/Map_WC_Unicorn_Way.xml (BINd) names the zone
                                (WizardZone_UnicornWay), this picture and its size on the page, 720x600, and lists
                                the map's icons in those 720x600 pixels (NPC portraits, nameplates, quest buttons).
  Maps/Map_WC.dds               512x512, the whole of Wizard City (the world map, as from your house).

and the marker art in _Shared-WorldData.wad:

  GUI/Buttons/Button_Player_Position.dds   256x128: the green "you are here" arrow, 8 frames of 64x64 (4 x 2).
  GUI/QuestButtons/Art_Quest_Teleporter.dds 128x128: Wizard101's teleporter icon (the grace marker).

Usage: python -I w101_map_art.py <Wizard101 Data\\GameData> <out folder>
  out folder = w101_orbs\\book\\art next to the DLL. Wizard101 data: never commit the PNGs.

Needs Pillow. The map's placement (Elden Ring tile m60_42_36_00 metres -> picture) is in pages.rs (street_uv); it was
fitted by laying Unicorn Way's walk graph (zone.nav in WizardCity-WC_Streets-WC_Unicorn.wad) over the picture.
"""
import struct
import sys
import zlib
from io import BytesIO
from pathlib import Path

from PIL import Image

# (WAD, entry, PNG path under the out folder)
ART = [
    ("GUI-WorldData.wad", "Maps/Map_WC_Unicorn_Way.dds", "GUI/Maps/Map_WC_Unicorn_Way.png"),
    ("GUI-WorldData.wad", "Maps/Map_WC.dds", "GUI/Maps/Map_WC.png"),
    ("_Shared-WorldData.wad", "GUI/Buttons/Button_Player_Position.dds", "GUI/Buttons/Button_Player_Position.png"),
    ("_Shared-WorldData.wad", "GUI/QuestButtons/Art_Quest_Teleporter.dds", "GUI/QuestButtons/Art_Quest_Teleporter.png"),
]


def wad_files(path, wanted):
    """{name: bytes} for the WAD entries whose names are in `wanted`."""
    found = {}
    with open(path, "rb") as wad:
        head = wad.read(14)
        assert head[:5] == b"KIWAD", f"{path} is not a Wizard101 WAD"
        version, count = struct.unpack_from("<II", head, 5)
        wad.seek(13 + (1 if version >= 2 else 0))
        entries = []
        for _ in range(count):
            offset, size, zsize, compressed, _crc, name_len = struct.unpack("<IIIBII", wad.read(21))
            name = wad.read(name_len)[:-1].decode(errors="replace")
            if name in wanted:
                entries.append((name, offset, zsize if compressed else size, compressed))
        for name, offset, length, compressed in entries:
            wad.seek(offset)
            raw = wad.read(length)
            found[name] = zlib.decompress(raw) if compressed else raw
    return found


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    game_data, out = Path(sys.argv[1]), Path(sys.argv[2])
    for wad in sorted({w for w, _, _ in ART}):
        wanted = {name: png for w, name, png in ART if w == wad}
        found = wad_files(game_data / wad, set(wanted))
        for name, png in sorted(wanted.items()):
            if name not in found:
                sys.exit(f"{name} is missing from {wad}")
            image = Image.open(BytesIO(found[name])).convert("RGBA")
            target = out / png
            target.parent.mkdir(parents=True, exist_ok=True)
            image.save(target)
            print(f"{png} {image.size[0]}x{image.size[1]}")


if __name__ == "__main__":
    main()
