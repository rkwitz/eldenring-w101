"""Copy Wizard101's HUD art (health, mana and energy globes, experience bar, spellbook button and book) and its
fonts out of Root.wad and _Shared-WorldData.wad for the hud_orbs add-on.

The globe numbers use the Glyphis bitmap font WizardFancy (HUDWindow.gui textHealth/textMana), the spellbook's text
WizardNormal: each glyph table (.xml) is copied as is and its A8 DDS atlas pages become white PNGs with the glyphs in
the alpha channel.

Usage: python w101_hud_art.py path\\to\\Wizard101\\Data\\GameData output_dir

Needs Pillow (pip install pillow) to turn the DDS files into PNG.
"""
import re
import struct
import sys
import zlib
from io import BytesIO
from pathlib import Path

from PIL import Image

# (WAD, entry); each becomes <file stem>.png.
ART = [("Root.wad", f"GUI/HUD/{stem}.dds")
       for stem in ["Health_Back", "Health_Liquid", "Health_Chop_Anim", "Mana_Liquid", "Mana_Chop_Anim",
                    "Energy_Back", "Energy_Liquid", "Energy_Chop_Anim"]] + [
    ("Root.wad", "GUI/Bar/Bar_Exp01.dds"),               # XPBar progressXPSolid
    ("Root.wad", "GUI/Art/Art_Bar_Exp.dds"),             # XPBar foreground
    ("_Shared-WorldData.wad", "GUI/HUD/Button_Spellbook3.dds"),  # btnSpellbook
    # The spellbook (SpellBookPrefsWindow.gui): the open book and its tab buttons down the right edge.
    ("Root.wad", "GUI/Deck_Background.dds"),
    ("_Shared-WorldData.wad", "GUI/Buttons/Button_Tab_Character.dds"),
    ("Root.wad", "GUI/Button_Tab_Inventory.dds"),
    ("_Shared-WorldData.wad", "GUI/Buttons/Button_Tab_Pet.dds"),
    ("Root.wad", "GUI/Button_Tab_DeckBuild.dds"),
    ("Root.wad", "GUI/Button_Tab_Quest.dds"),
    ("Root.wad", "GUI/Button_Tab_WorldMap.dds"),
    ("_Shared-WorldData.wad", "GUI/Buttons/Button_Tab_Flask.dds"),
    ("Root.wad", "GUI/Buttons/Button_Tab_Help.dds"),
    ("Root.wad", "GUI/Button_Tab_Options.dds"),
    ("Root.wad", "GUI/Buttons/Button_Exit.dds"),
    ("_Shared-WorldData.wad", "GUI/Button_PageNext.dds"),
    ("_Shared-WorldData.wad", "GUI/Button_PagePrevious.dds"),
]
FONTS = ["WizardFancy", "WizardNormal"]  # HUD numbers and titles; book text


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
            name = wad.read(name_len)[:-1].decode()
            if name in wanted:
                entries.append((name, offset, zsize if compressed else size, compressed))
        for name, offset, length, compressed in entries:
            wad.seek(offset)
            raw = wad.read(length)
            found[name] = zlib.decompress(raw) if compressed else raw
    return found


def main():
    game_data, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    for wad in sorted({wad for wad, _ in ART}):
        names = {name for w, name in ART if w == wad}
        found = wad_files(game_data / wad, names)
        for name in sorted(names):
            if name not in found:
                sys.exit(f"{name} is missing from {wad}")
            stem = Path(name).stem
            image = Image.open(BytesIO(found[name])).convert("RGBA")
            image.save(out / f"{stem}.png")
            print(f"{stem}.png {image.size[0]}x{image.size[1]}")
    for font in FONTS:
        font_xml = f"Fonts/{font}.xml"
        xml = wad_files(game_data / "Root.wad", {font_xml}).get(font_xml)
        if xml is None:
            sys.exit(f"{font_xml} is missing from Root.wad")
        (out / f"{font}.xml").write_bytes(xml)
        pages = {f"Fonts/{page}": page for page in re.findall(r'FileName="([^"]+)"', xml.decode())}
        found = wad_files(game_data / "Root.wad", pages)
        for name, page in pages.items():
            if name not in found:
                sys.exit(f"{name} is missing from Root.wad")
            alpha = a8_dds(found[name])
            image = Image.new("L", alpha.size, 255).convert("RGBA")
            image.putalpha(alpha)
            stem = Path(page).stem
            image.save(out / f"{stem}.png")
            print(f"{stem}.png {image.size[0]}x{image.size[1]}")


def a8_dds(data):
    """The alpha image of an uncompressed 8-bit alpha-only DDS (Glyphis DDS_A8 font pages)."""
    assert data[:4] == b"DDS ", "not a DDS file"
    height, width = struct.unpack_from("<II", data, 12)
    flags, _fourcc, bits = struct.unpack_from("<III", data, 80)
    assert bits == 8 and not flags & 4, "font page is not uncompressed A8"
    return Image.frombytes("L", (width, height), data[128:128 + width * height])


if __name__ == "__main__":
    main()
