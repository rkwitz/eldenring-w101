"""Copy Wizard101's health and mana globe art out of Root.wad as PNGs for the hud_orbs add-on.

Usage: python w101_hud_art.py path\\to\\Wizard101\\Data\\GameData output_dir

Needs Pillow (pip install pillow) to turn the DDS files into PNG.
"""
import struct
import sys
import zlib
from io import BytesIO
from pathlib import Path

from PIL import Image

FILES = ["Health_Back", "Health_Liquid", "Health_Chop_Anim", "Mana_Liquid", "Mana_Chop_Anim"]


def wad_files(path, wanted):
    """{name: bytes} for the WAD entries whose names are in `wanted`."""
    data = Path(path).read_bytes()
    assert data[:5] == b"KIWAD", f"{path} is not a Wizard101 WAD"
    version, count = struct.unpack_from("<II", data, 5)
    pos = 13 + (1 if version >= 2 else 0)
    found = {}
    for _ in range(count):
        offset, size, zsize, compressed, _crc, name_len = struct.unpack_from("<IIIBII", data, pos)
        pos += 21
        name = data[pos:pos + name_len - 1].decode()
        pos += name_len
        if name in wanted:
            raw = data[offset:offset + (zsize if compressed else size)]
            found[name] = zlib.decompress(raw) if compressed else raw
    return found


def main():
    game_data, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    names = {f"GUI/HUD/{stem}.dds": stem for stem in FILES}
    found = wad_files(game_data / "Root.wad", names)
    for name, stem in names.items():
        if name not in found:
            sys.exit(f"{name} is missing from Root.wad")
        image = Image.open(BytesIO(found[name])).convert("RGBA")
        image.save(out / f"{stem}.png")
        print(f"{stem}.png {image.size[0]}x{image.size[1]}")


if __name__ == "__main__":
    main()
