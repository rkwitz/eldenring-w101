"""Copy Wizard101's spellbook out of its WADs: every spellbook page layout (.gui) and the art those layouts use, as PNG.

Usage: python w101_spellbook_art.py path\\to\\Wizard101\\Data\\GameData output_dir

Needs Pillow. Writes output_dir/gui/*.gui, output_dir/art/<WAD path>.png and output_dir/index.txt (which page uses
which art). Art named in a layout but found in no WAD is listed as missing in the index.
"""
import re
import struct
import sys
import zlib
from io import BytesIO
from pathlib import Path

from PIL import Image

WADS = ["_Shared-WorldData.wad", "Root.wad", "GUI-WorldData.wad"]
PAGES = re.compile(r"GUI/[^/]*(SpellbookPage|DeckConfiguration|Spellbook)[^/]*\.gui$", re.I)
# Art the layouts don't name but the spellbook frame and its tabs use (book backgrounds, tab buttons).
ART_NAMES = re.compile(r"Spellbook|Tab_|^Background_(Character|Deck_Config|Options|Quest|Inventory)|^GUI_QuestBook", re.I)
FILENAME = re.compile(r"<m_sFilename>([^<]+\.(?:dds|png|tga))</m_sFilename>", re.I)


def wad_index(path):
    """(data, {name: (offset, size, zsize, compressed)}) of a WAD."""
    data = Path(path).read_bytes()
    assert data[:5] == b"KIWAD", f"{path} is not a Wizard101 WAD"
    version, count = struct.unpack_from("<II", data, 5)
    pos = 13 + (1 if version >= 2 else 0)
    entries = {}
    for _ in range(count):
        offset, size, zsize, compressed, _crc, name_len = struct.unpack_from("<IIIBII", data, pos)
        pos += 21
        entries[data[pos:pos + name_len - 1].decode()] = (offset, size, zsize, compressed)
        pos += name_len
    return data, entries


def read(wad, name):
    data, entries = wad
    offset, size, zsize, compressed = entries[name]
    raw = data[offset:offset + (zsize if compressed else size)]
    return zlib.decompress(raw) if compressed else raw


def main():
    game_data, out = Path(sys.argv[1]), Path(sys.argv[2])
    wads = [wad_index(game_data / name) for name in WADS if (game_data / name).exists()]
    lookup = {}
    for wad in wads:
        for name in wad[1]:
            lookup.setdefault(name.lower(), (wad, name))

    pages = sorted(name for _, name in lookup.values() if PAGES.search(name))
    art = {}  # WAD path -> pages using it
    for page in pages:
        text = read(*lookup[page.lower()]).decode("utf-8", "replace")
        (out / "gui").mkdir(parents=True, exist_ok=True)
        (out / "gui" / Path(page).name).write_text(text, encoding="utf-8", newline="\n")
        for filename in FILENAME.findall(text):
            path = filename if filename.lower().startswith("gui/") else f"GUI/{filename}"
            art.setdefault(path, set()).add(Path(page).stem)
    for _, name in lookup.values():
        if name.lower().startswith("gui/") and ART_NAMES.search(Path(name).name) and name.lower().endswith(".dds"):
            art.setdefault(name, set()).add("(by name)")

    lines, missing = [], []
    for path in sorted(art):
        hit = lookup.get(path.lower())
        if not hit:
            missing.append(path)
            continue
        target = out / "art" / Path(hit[1]).with_suffix(".png")
        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            image = Image.open(BytesIO(read(*hit))).convert("RGBA")
        except Exception as e:  # noqa: BLE001 - some DDS variants Pillow can't read
            missing.append(f"{path} (unreadable: {e})")
            continue
        image.save(target)
        lines.append(f"{hit[1]}\t{image.size[0]}x{image.size[1]}\t{', '.join(sorted(art[path]))}")
    (out / "index.txt").write_text("\n".join(lines + ["", "MISSING:"] + missing) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(pages)} layouts, {len(lines)} images, {len(missing)} missing -> {out}")


if __name__ == "__main__":
    main()
