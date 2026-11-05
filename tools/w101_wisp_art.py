"""Wizard101's health and mana wisps for the hud_orbs DLL (round 21 M3), as PNGs + the pickup sound under
w101_orbs\\wisps on Ryan's PC (never committed: Wizard101 art).

Root.wad ObjectData/CombatBonus_WispHealth.xml (red) / CombatBonus_WispMana.xml (blue) are the wisps a won fight
drops: model _Shared-WorldData Character/WilOTheWisps/FX_Wisp_Red.nif / FX_Wisp.nif (a particle cloud: FX_WispCloud
puffs plus FX_FocusSpark sparkles, over a GlowShadow_Red/Blue ground glow, FX_Glow_05), WanderMovementBehavior,
DespawnTimedBehavior, InteractWispAddHealth / InteractWispAddMana, the pick-up burst Character/FX_WispRed_Dsppr.nif /
FX_WispBlue_Dsppr.nif and the sound Sound/GUI/ui_health_powerup_01.wav (both wisps).

Usage: python -I w101_wisp_art.py path\\to\\Wizard101\\Data\\GameData path\\to\\orbs\\w101_orbs   (stdlib + Pillow)
"""
import struct
import sys
import zlib
from io import BytesIO
from pathlib import Path

from PIL import Image

SHARED = "_Shared-WorldData.wad"
FILES = [  # (wad, entry, output under w101_orbs/wisps)
    (SHARED, "Textures/FX_WispCloud_Red__6c0acfa2.nif", "Wisp_Cloud_Red.png"),
    (SHARED, "Textures/FX_WispCloud__b4758f83.nif", "Wisp_Cloud_Blue.png"),
    (SHARED, "Textures/FX_FocusSpark_Red__74c600c6.nif", "Wisp_Spark_Red.png"),
    (SHARED, "Textures/FX_FocusSpark_Blue__b06ae421.nif", "Wisp_Spark_Blue.png"),
    (SHARED, "Textures/FX_Cloud_Fog_03__b6a74427.nif", "Wisp_Fog.png"),
    (SHARED, "Textures/FX_Glow_05__eb85cc.nif", "Wisp_Glow.png"),
    ("Root.wad", "Sound/GUI/ui_health_powerup_01.wav", "ui_health_powerup_01.wav"),
]


def wad_read(path, wanted):
    with open(path, "rb") as f:
        assert f.read(5) == b"KIWAD", f"{path} is not a Wizard101 WAD"
        version, count = struct.unpack("<II", f.read(8))
        if version >= 2:
            f.read(1)
        for _ in range(count):
            offset, size, zsize, zipped, _crc, name_len = struct.unpack("<IIIBII", f.read(21))
            if f.read(name_len).rstrip(b"\0").decode("latin-1") == wanted:
                f.seek(offset)
                raw = f.read(zsize if zipped else size)
                return zlib.decompress(raw) if zipped else raw
    raise KeyError(f"{wanted} not in {path}")


def dds(four, w, h, data):
    header = (b"DDS " + struct.pack("<7I44x", 124, 0x1007, h, w, len(data), 0, 1)
              + struct.pack("<2I4s5I", 32, 4, four, 0, 0, 0, 0, 0) + struct.pack("<5I", 0x1000, 0, 0, 0, 0))
    return Image.open(BytesIO(header + data)).convert("RGBA")


def first_pixel_image(data):
    """Top mip of the first NiPixelData (as w101_quest_art.pixel_images: mip table, byte count -> format)."""
    pos = 0
    while True:
        pos = data.find(b"\xff\xff\xff\xff", pos)
        if pos < 0 or pos + 12 > len(data):
            raise ValueError("no NiPixelData")
        count, bpp = struct.unpack_from("<II", data, pos + 4)
        if 1 <= count <= 16 and bpp <= 8 and pos + 12 + 12 * count + 8 <= len(data):
            mips = [struct.unpack_from("<III", data, pos + 12 + 12 * i) for i in range(count)]
            if mips[0][2] == 0 and all(0 < w <= 4096 and 0 < h <= 4096 and w & (w - 1) == 0 and h & (h - 1) == 0
                                       for w, h, _ in mips):
                end = pos + 12 + 12 * count
                nbytes, faces = struct.unpack_from("<II", data, end)
                px = data[end + 8:end + 8 + nbytes]
                w, h = mips[0][0], mips[0][1]
                blocks = sum(max(1, a // 4) * max(1, b // 4) for a, b, _ in mips)
                texels = sum(a * b for a, b, _ in mips)
                if nbytes == blocks * 8 * faces:
                    return dds(b"DXT1", w, h, px[:max(1, w // 4) * max(1, h // 4) * 8])
                if nbytes == blocks * 16 * faces:
                    return dds(b"DXT5", w, h, px[:max(1, w // 4) * max(1, h // 4) * 16])
                if nbytes == texels * 4 * faces:
                    return Image.frombytes("RGBA", (w, h), px[:w * h * 4])
                if nbytes == texels * 3 * faces:
                    return Image.frombytes("RGB", (w, h), px[:w * h * 3]).convert("RGBA")
                if nbytes == texels * faces:  # 8-bit (alpha / luminance)
                    g = Image.frombytes("L", (w, h), px[:w * h])
                    return Image.merge("RGBA", (g, g, g, g))
        pos += 1


ADDITIVE = {"Wisp_Cloud_Red.png", "Wisp_Cloud_Blue.png", "Wisp_Spark_Red.png", "Wisp_Spark_Blue.png", "Wisp_Glow.png"}


def additive_to_alpha(image):
    """The wisp nif draws these additively (black = nothing; DXT1, no alpha). The DLL's ImGui draw lists blend by
    alpha, so alpha = the brightest channel and the colour is divided by it: alpha blending then adds the same light."""
    out = image.convert("RGBA")
    px = out.load()
    w, h = out.size
    for y in range(h):
        for x in range(w):
            r, g, b, _ = px[x, y]
            a = max(r, g, b)
            px[x, y] = (r * 255 // a, g * 255 // a, b * 255 // a, a) if a else (0, 0, 0, 0)
    return out


def main():
    game_data, out = Path(sys.argv[1]), Path(sys.argv[2]) / "wisps"
    out.mkdir(parents=True, exist_ok=True)
    for wad, entry, target in FILES:
        data = wad_read(game_data / wad, entry)
        if not data.strip(b"\0"):
            print(f"{target}: {entry} not downloaded yet (all zeros)")
            continue
        if target.endswith(".png"):
            image = first_pixel_image(data)
            if target in ADDITIVE:
                image = additive_to_alpha(image)
            image.save(out / target)
            print(target, image.size, image.getextrema())
        else:
            (out / target).write_bytes(data)
            print(target, len(data), "bytes")


if __name__ == "__main__":
    main()
