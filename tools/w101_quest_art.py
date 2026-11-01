"""Wizard101's quest markers and the dialogue portraits for the hud_orbs DLL (round 17, F1-F3), as PNGs under
w101_orbs\\book\\art on Ryan's PC (never committed: Wizard101 art).

Markers (WizBangs.xml: StartQuest -> Wizbang/Quest_Marker_Start.nif, CompleteQuestGoal -> Quest_Marker_Complete.nif):
each marker is two camera-facing quads under a NiBillboardNode 38.71 units over the marker's origin: the sign
(76.2 units square: Quest_Start "!" / Quest_Complete "?") and, 5.23 units behind it, the red Quest_Spiral (59 units
square) turning one full turn every 6.67 s (its NiTransformController's z rotation keys -4.71 .. -11.00 rad). The
sign itself neither bobs nor scales (its scale keys are 1.0 throughout). Their textures:
  Quest_Start.png    _Shared-WorldData.wad Textures/Quest_Start__e315fd4.nif (NiPixelData, DXT5 256x256)
  Quest_Spiral.png   _Shared-WorldData.wad Textures/Quest_Spiral__9549b9f2.nif (the one Quest_Marker_Start names)
  Quest_Complete.png Root.wad Wizbang/Quest_Marker_Complete.nif, its embedded Quest_Complete.tga (2nd NiPixelData)
Portraits: the NpcPortraits files the street NPCs' ObjectData templates name (w101_quests.PORTRAITS); round 18 adds
Merle Ambrose and Gamma, the goal pictures (GOAL_PICTURES), two inline icons, and the quest arrow
(Character/Arrow/Quest_Arrow_JT.nif's FX_Quest_Arrow_JT, as Quest/Quest_Arrow.png).

The dialogue box itself (DialogWindow.gui, DialogImage.gui, their art) comes from w101_book_layout.py
(--pages=DialogWindow,DialogImage).

Usage: python -I w101_quest_art.py path\\to\\Wizard101\\Data\\GameData w101_orbs\\book   (stdlib + Pillow)
"""
import struct
import sys
import zlib
from io import BytesIO
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
import w101_quests as Q  # noqa: E402

MARKERS = [  # (wad, entry, index of the NiPixelData in the file, output)
    ("_Shared-WorldData.wad", "Textures/Quest_Start__e315fd4.nif", 0, "Wizbang/Quest_Start.png"),
    ("_Shared-WorldData.wad", "Textures/Quest_Spiral__9549b9f2.nif", 0, "Wizbang/Quest_Spiral.png"),
    ("Root.wad", "Wizbang/Quest_Marker_Complete.nif", 1, "Wizbang/Quest_Complete.png"),
    # Round 18 (F4): the quest arrow under the player's feet (a flat 42.46 x 70 unit quad, its texture embedded).
    ("Root.wad", "Character/Arrow/Quest_Arrow_JT.nif", 0, "Quest/Quest_Arrow.png"),
]
# Round 18: the quest cards' goal pictures (QuestInfoWindow imgGoalObjective1, w101_quests.GOAL_PICTURES) and the
# pictures inline in the dialogue text (w101_npc_dialogue.tidy).
PICTURES = ["GUI/Emoticons/Emoticons_Question.png", "GUI/Icons/Icon_Star.png"]
WADS = ["Root.wad", "_Shared-WorldData.wad"]


def wad_entries(path):
    with open(path, "rb") as f:
        head = f.read(14)
        assert head[:5] == b"KIWAD", f"{path} is not a Wizard101 WAD"
        version, count = struct.unpack_from("<II", head, 5)
        f.seek(13 + (1 if version >= 2 else 0))
        out = {}
        for _ in range(count):
            offset, size, zsize, compressed, _crc, name_len = struct.unpack("<IIIBII", f.read(21))
            out[f.read(name_len)[:-1].decode(errors="replace")] = (offset, size, zsize, compressed)
        return out


def wad_read(path, entry):
    offset, size, zsize, compressed = wad_entries(path)[entry]
    with open(path, "rb") as f:
        f.seek(offset)
        raw = f.read(zsize if compressed else size)
    return zlib.decompress(raw) if compressed else raw


def dds(four, w, h, data):
    size = len(data)
    header = (b"DDS " + struct.pack("<7I44x", 124, 0x1007, h, w, size, 0, 1)
              + struct.pack("<2I4s5I", 32, 4, four, 0, 0, 0, 0, 0) + struct.pack("<5I", 0x1000, 0, 0, 0, 0))
    return Image.open(BytesIO(header + data)).convert("RGBA")


def pixel_images(data):
    """Top mip level of every NiPixelData in a .nif, in file order. Each is found by its mipmap table (palette ref -1,
    mip count, bytes per pixel, (width, height, offset) rows starting at offset 0), then the pixel byte count; the
    format follows from that count (DXT1 / DXT5 / RGBA / RGB over all the mips)."""
    out, pos = [], 0
    while True:
        pos = data.find(b"\xff\xff\xff\xff", pos)
        if pos < 0 or pos + 12 > len(data):
            return out
        count, bpp = struct.unpack_from("<II", data, pos + 4)
        if 1 <= count <= 16 and bpp <= 8:
            mips = [struct.unpack_from("<III", data, pos + 12 + 12 * i) for i in range(count)]
            pow2 = all(w & (w - 1) == 0 and h & (h - 1) == 0 and 0 < w <= 4096 and 0 < h <= 4096 for w, h, _ in mips)
            if pow2 and mips[0][2] == 0:
                end = pos + 12 + 12 * count
                nbytes, faces = struct.unpack_from("<II", data, end)
                pixels = data[end + 8:end + 8 + nbytes]
                w, h = mips[0][0], mips[0][1]
                blocks = sum(max(1, mw // 4) * max(1, mh // 4) for mw, mh, _ in mips)
                texels = sum(mw * mh for mw, mh, _ in mips)
                if nbytes == blocks * 8 * faces:
                    out.append(dds(b"DXT1", w, h, pixels[:max(1, w // 4) * max(1, h // 4) * 8]))
                elif nbytes == blocks * 16 * faces:
                    out.append(dds(b"DXT5", w, h, pixels[:max(1, w // 4) * max(1, h // 4) * 16]))
                elif nbytes == texels * 4 * faces:
                    out.append(Image.frombytes("RGBA", (w, h), pixels[:w * h * 4]))
                elif nbytes == texels * 3 * faces:
                    out.append(Image.frombytes("RGB", (w, h), pixels[:w * h * 3]).convert("RGBA"))
                else:
                    raise ValueError(f"NiPixelData {w}x{h}: {nbytes} bytes, unknown format")
                pos = end + 8 + nbytes
                continue
        pos += 1


def key_outside(image):
    """FX_Quest_Arrow_JT is DXT1 with no alpha: the arrow (pale yellow, black outline) on a flat orange ground. The
    nif blends it (NiAlphaProperty), so the ground is not meant to show: it is made transparent by filling it from
    the corners up to the black outline."""
    from PIL import ImageDraw
    rgb = image.convert("RGB")
    w, h = rgb.size
    for corner in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]:
        if rgb.getpixel(corner) != (255, 0, 255):
            ImageDraw.floodfill(rgb, corner, (255, 0, 255), thresh=60)
    out = image.copy()
    px, m = rgb.load(), out.load()
    for y in range(h):
        for x in range(w):
            if px[x, y] == (255, 0, 255):
                m[x, y] = (0, 0, 0, 0)
    return out


def main():
    game_data, out = Path(sys.argv[1]), Path(sys.argv[2]) / "art"
    for wad, entry, index, target in MARKERS:
        image = pixel_images(wad_read(game_data / wad, entry))[index]
        if target == "Quest/Quest_Arrow.png":
            image = key_outside(image)
        (out / target).parent.mkdir(parents=True, exist_ok=True)
        image.save(out / target)
        print(target, image.size)
    entries = {wad: wad_entries(game_data / wad) for wad in WADS}
    for portrait in sorted(set(Q.PORTRAITS.values()) | set(Q.GOAL_PICTURES.values()) | set(PICTURES)):
        source = str(Path(portrait).with_suffix(".dds")).replace("\\", "/")
        wad = next(w for w in WADS if source in entries[w])
        image = Image.open(BytesIO(wad_read(game_data / wad, source))).convert("RGBA")
        (out / portrait).parent.mkdir(parents=True, exist_ok=True)
        image.save(out / portrait)
        print(portrait, image.size)


if __name__ == "__main__":
    main()
