"""Copy Wizard101's map art for the hud_orbs book's World Map page (hud_orbs/src/pages.rs, map section).

Wizard101's spellbook map (MapSpellbookPage.gui) draws a painted map per zone, kept in GUI-WorldData.wad:

  Maps/Map_WC_Unicorn_Way.dds   512x512, Unicorn Way. Maps/Map_WC_Unicorn_Way.xml (BINd) names the zone
                                (WizardZone_UnicornWay), this picture and its size on the page, 720x600, and lists
                                the map's icons in those 720x600 pixels (NPC portraits, nameplates, quest buttons).
  Maps/Map_WC.dds               512x512, the whole of Wizard City (the world map, as from your house).

and the marker art in _Shared-WorldData.wad:

  GUI/Buttons/Button_Player_Position.dds   256x128: the green "you are here" arrow, 8 frames of 64x64 (4 x 2).
  GUI/QuestButtons/Art_Quest_Teleporter.dds 128x128: Wizard101's teleporter icon (the grace marker).

Round 16 additions (what Map_WC_Unicorn_Way.xml's icon list uses, in _Shared-WorldData.wad):

  GUI/Nameplates/NamePlate_128x64_{Elite,Mob,NPC,Shop}.dds   the parchment name plates a map icon shows on hover
                                (Elite purple for bosses, Mob red, NPC green, Shop tan), drawn at the icon's plate
                                rectangle with the name inside its margins.
  GUI/Nameplates/Location_Long_2_Line_52_pixels_high.dds, POI_128x64.dds   the always-shown place labels (Arena, The
                                Commons; Hedge Maze, Unicorn Park), drawn over the painting with crisp text.
  GUI/NpcPortraits/Art_Portrait_GhostLady_Green.dds, Portrait_Skeleton_Armored_A_MOB.dds   Lady Blackhope's and
                                Rattlebones' map icons.
  GUI/Buttons/Button_Zoom.dds   the map's PlusMinusButton (4x2 frames: plus on top, minus below; the page's layout
                                uses the bottom row): written as Button_Zoom_Plus.png with the plus row in both rows,
                                for the button while Elden Ring's map is open.

No larger Unicorn Way painting exists in the install: every Maps/Map_*.dds is 512x512 DXT5, the classic-mode WADs have
no maps, and GUI/Backgrounds/UWMap.dds (989x743, the tutorial's picture of the map page) is a screenshot of the same
512 picture. So the zone maps are upscaled once here (4x super-resolution with OpenCV's EDSR model when --sr is given,
otherwise Lanczos) and written at MAP_SIZE, twice the page's 720x600 map area: at 2560x1440 that area is 1350x1125
screen pixels, so the GPU only shrinks the picture a little instead of stretching 512 pixels 2.6 times.

Usage: python -I w101_map_art.py <Wizard101 Data\\GameData> <out folder> [--sr EDSR_x4.pb]
  out folder = w101_orbs\\book\\art next to the DLL. Wizard101 data: never commit the PNGs.
  EDSR_x4.pb: https://github.com/Saafke/EDSR_Tensorflow (models/EDSR_x4.pb); needs opencv-contrib-python.

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
    ("_Shared-WorldData.wad", "GUI/Buttons/Button_Zoom.dds", "GUI/Buttons/Button_Zoom_Plus.png"),
    ("_Shared-WorldData.wad", "GUI/NpcPortraits/Art_Portrait_GhostLady_Green.dds",
     "GUI/NpcPortraits/Art_Portrait_GhostLady_Green.png"),
    ("_Shared-WorldData.wad", "GUI/NpcPortraits/Portrait_Skeleton_Armored_A_MOB.dds",
     "GUI/NpcPortraits/Portrait_Skeleton_Armored_A_MOB.png"),
] + [("_Shared-WorldData.wad", f"GUI/Nameplates/{n}.dds", f"GUI/Nameplates/{n}.png")
     for n in ("NamePlate_128x64_Elite", "NamePlate_128x64_Mob", "NamePlate_128x64_NPC", "NamePlate_128x64_Shop",
               "Location_Long_2_Line_52_pixels_high", "POI_128x64")]
# The zone maps, written upscaled at twice the page's 720x600 map area.
MAPS = {"GUI/Maps/Map_WC_Unicorn_Way.png", "GUI/Maps/Map_WC.png"}
MAP_SIZE = (1440, 1200)


def upscale(image, sr_model):
    """The 512x512 map at MAP_SIZE: EDSR x4 (OpenCV dnn_superres) when a model is given, else Lanczos; then a light
    unsharp mask, as the painting's ink lines stay soft after either."""
    from PIL import ImageFilter
    if sr_model:
        import cv2
        import numpy as np
        sr = cv2.dnn_superres.DnnSuperResImpl_create()
        sr.readModel(str(sr_model))
        sr.setModel("edsr", 4)
        rgba = np.asarray(image)
        up = sr.upsample(np.ascontiguousarray(rgba[:, :, 2::-1]))[:, :, ::-1]
        alpha = np.asarray(image.getchannel("A").resize((up.shape[1], up.shape[0]), Image.LANCZOS))
        big = Image.fromarray(np.dstack([up, alpha]).astype("uint8"), "RGBA")
    else:
        big = image.resize((image.width * 4, image.height * 4), Image.LANCZOS)
    return big.resize(MAP_SIZE, Image.LANCZOS).filter(ImageFilter.UnsharpMask(1.5, 40, 2))


def plus_row(image):
    """Button_Zoom with its top (plus) row copied over the bottom (minus) row."""
    w, h = image.size
    out = image.copy()
    out.paste(image.crop((0, 0, w, h // 2)), (0, h // 2))
    return out


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
    args = sys.argv[1:]
    sr_model = None
    if "--sr" in args:
        i = args.index("--sr")
        sr_model = Path(args[i + 1])
        del args[i:i + 2]
    if len(args) != 2:
        sys.exit(__doc__)
    game_data, out = Path(args[0]), Path(args[1])
    for wad in sorted({w for w, _, _ in ART}):
        wanted = {name: png for w, name, png in ART if w == wad}
        found = wad_files(game_data / wad, set(wanted))
        for name, png in sorted(wanted.items()):
            if name not in found:
                sys.exit(f"{name} is missing from {wad}")
            image = Image.open(BytesIO(found[name])).convert("RGBA")
            if png in MAPS:
                image = upscale(image, sr_model)
            elif png.endswith("Button_Zoom_Plus.png"):
                image = plus_row(image)
            target = out / png
            target.parent.mkdir(parents=True, exist_ok=True)
            image.save(target)
            print(f"{png} {image.size[0]}x{image.size[1]}")


if __name__ == "__main__":
    main()
