"""Copy the textures of Wizard101's teleport "poof" out of _Shared-WorldData.wad for the hud_orbs overlay
(hud_orbs/src/teleport.rs).

Wizard101 plays _Shared-WorldData.wad Character/FX_Disappear.nif on a character that teleports away (the client's
m_poofEffectAsset, next to the "Go Home" button's sound in WizardGraphicalClient.exe; Ambrose's and Malistaire's
disappearances in Wizard City use its Bright and Dark copies). It has no animation of its own on the character: the
character just vanishes inside it. Its textures are NiPixelData files under Textures/ with a hash in the name:

  FX_DisappearSparks._1  32x32 DXT1, blue-white four-point star (FX_StunSparkle, FX_CastSignSpark01)
  FX_DisappearSparks._2  32x32 DXT1, gold-white four-point star (FX_CastSignSpark, FX_StunSparkle01)
  FX_LeaveToob_01        64x256 DXT1, horizontal bands of orange and blue light over black (the two tubes)
  FX_SmokeChimney        32x32 DXT5, a soft grey puff (FX_Smoke)

The sparks and tubes are drawn additively in Wizard101 (black = nothing); the overlay draws with alpha blending, so
their PNGs get alpha = brightest channel and the colour divided by it. The smoke keeps its own alpha, stretched to
full range and its colour made a light grey (the effect's colour keys make it faint again).

Usage: python w101_teleport_art.py <Wizard101 Data\\GameData, or a folder holding the Textures\\*.nif> <out folder>
  out folder = w101_orbs\\book\\art\\teleport next to the DLL. Wizard101 data: never commit the PNGs.

Needs pyffi (pip install pyffi) and Pillow.
"""
import struct
import sys
import time
from io import BytesIO
from pathlib import Path

from PIL import Image

if not hasattr(time, "clock"):  # pyffi 2.2.3 still calls time.clock
    time.clock = time.perf_counter
from pyffi.formats.nif import NifFormat  # noqa: E402

sys.path.insert(0, str(Path(__file__).parent))
from w101_hud_art import wad_files  # noqa: E402

WAD = "_Shared-WorldData.wad"
# texture name prefix in the WAD (Textures/<prefix>__<hash>.nif) -> (PNG name, additive)
TEXTURES = {
    "FX_DisappearSparks._1__": ("sparks_blue.png", True),
    "FX_DisappearSparks._2__": ("sparks_gold.png", True),
    "FX_LeaveToob_01__": ("tube.png", True),
    "FX_SmokeChimney__2b199ddd": ("smoke.png", False),  # the copy FX_Disappear.nif names
}
DXT = {4: (b"DXT1", 8), 6: (b"DXT5", 16)}  # NiPixelData pixel_format -> DDS four-character code, block bytes


def wad_names(path):
    with open(path, "rb") as wad:
        head = wad.read(14)
        version, count = struct.unpack_from("<II", head, 5)
        wad.seek(13 + (1 if version >= 2 else 0))
        names = []
        for _ in range(count):
            _offset, _size, _zsize, _compressed, _crc, name_len = struct.unpack("<IIIBII", wad.read(21))
            names.append(wad.read(name_len)[:-1].decode(errors="replace"))
    return names


def pixel_image(data):
    """The top mip level of a NiPixelData .nif as a PIL image."""
    nif = NifFormat.Data()
    stream = BytesIO(data)
    nif.inspect(stream)
    nif.read(stream)
    pixels = nif.roots[0]
    w, h = pixels.mipmaps[0].width, pixels.mipmaps[0].height
    raw = bytes(pixels.pixel_data[0])
    if pixels.pixel_format == 0:
        return Image.frombytes("RGB", (w, h), raw[:w * h * 3]).convert("RGBA")
    four, block = DXT[pixels.pixel_format]
    size = max(1, w // 4) * max(1, h // 4) * block
    header = (b"DDS " + struct.pack("<7I44x", 124, 0x1007, h, w, size, 0, 1)
              + struct.pack("<2I4s5I", 32, 4, four, 0, 0, 0, 0, 0) + struct.pack("<5I", 0x1000, 0, 0, 0, 0))
    return Image.open(BytesIO(header + raw[:size])).convert("RGBA")


def additive_to_alpha(image):
    out = Image.new("RGBA", image.size)
    px = []
    for r, g, b, _ in list(image.getdata()):
        a = max(r, g, b)
        px.append((r * 255 // a, g * 255 // a, b * 255 // a, a) if a else (0, 0, 0, 0))
    out.putdata(px)
    return out


def stretch_alpha(image):
    """The smoke: its alpha stretched to full range, its colour a light grey (the texture is a dark sepia that
    Wizard101 lights; unlit here it read as brown)."""
    pixels = list(image.getdata())
    top = max(a for *_, a in pixels) or 255
    out = Image.new("RGBA", image.size)
    out.putdata([(225, 225, 220, a * 255 // top) for *_, a in pixels])
    return out


def main():
    source, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    if (source / WAD).is_file():
        names = [n for n in wad_names(source / WAD) if n.startswith("Textures/")]
        wanted = {n: next(p for p in TEXTURES if n[len("Textures/"):].startswith(p)) for n in names
                  if any(n[len("Textures/"):].startswith(p) for p in TEXTURES)}
        found = {wanted[n]: d for n, d in wad_files(source / WAD, set(wanted)).items()}
    else:
        found = {}
        for path in (source / "Textures").glob("*.nif"):
            for prefix in TEXTURES:
                if path.name.startswith(prefix):
                    found[prefix] = path.read_bytes()
    for prefix, (png, additive) in TEXTURES.items():
        if prefix not in found:
            sys.exit(f"Textures/{prefix}*.nif not found in {source}")
        image = pixel_image(found[prefix])
        image = additive_to_alpha(image) if additive else stretch_alpha(image)
        image.save(out / png)
        print(f"{png} {image.size[0]}x{image.size[1]}")


if __name__ == "__main__":
    main()
