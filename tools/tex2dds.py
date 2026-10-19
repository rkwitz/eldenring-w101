"""Convert Wizard101 texture .nif files (one NiPixelData block, Gamebryo 20.6) to .dds.

Usage: python tex2dds.py input_dir output_dir

Standard library only. Handles DXT1/DXT3/DXT5 and 32-bit RGBA; other formats
are reported and skipped.
"""
import struct
import sys
from pathlib import Path

FOURCC = {4: b"DXT1", 5: b"DXT3", 6: b"DXT5"}


def parse(data):
    name = data.find(b"NiPixelData")
    if name < 0:
        raise ValueError("no NiPixelData block")
    # type index (u16), block size (u32), num strings, max string length, num groups (u32 each)
    pos = name + len(b"NiPixelData") + 2 + 4 + 12
    pixel_format = struct.unpack_from("<I", data, pos)[0]
    # The mipmap table follows the palette ref (-1): count, bytes per pixel (u32), then (w, h, offset) rows.
    pal = data.find(b"\xff\xff\xff\xff", pos + 8)
    while pal >= 0:
        count = struct.unpack_from("<I", data, pal + 4)[0]
        if 1 <= count <= 16:
            mips = [struct.unpack_from("<III", data, pal + 12 + 12 * i) for i in range(count)]
            if mips[0][2] == 0 and all(0 < w <= 8192 and 0 < h <= 8192 for w, h, _ in mips):
                break
        pal = data.find(b"\xff\xff\xff\xff", pal + 1)
    else:
        raise ValueError("mipmap table not found")
    bpp = struct.unpack_from("<I", data, pal + 8)[0]
    table_end = pal + 12 + 12 * count
    num_bytes, num_faces = struct.unpack_from("<II", data, table_end)
    pixels = data[table_end + 8: table_end + 8 + num_bytes]
    return pixel_format, bpp, mips, pixels


def dds_header(pixel_format, bpp, mips):
    width, height, _ = mips[0]
    flags = 0x1 | 0x2 | 0x4 | 0x1000 | (0x20000 if len(mips) > 1 else 0)
    caps = 0x1000 | (0x400008 if len(mips) > 1 else 0)
    if pixel_format in FOURCC:
        block = 8 if pixel_format == 4 else 16
        pitch = max(1, (width + 3) // 4) * max(1, (height + 3) // 4) * block
        pf = struct.pack("<II4sIIIII", 32, 0x4, FOURCC[pixel_format], 0, 0, 0, 0, 0)
        flags |= 0x80000
    elif pixel_format == 1 and bpp in (0, 32):
        pitch = width * 4
        pf = struct.pack("<II4sIIIII", 32, 0x41, b"\0\0\0\0", 32, 0xFF, 0xFF00, 0xFF0000, 0xFF000000)
        flags |= 0x8
    else:
        return None
    return (b"DDS " + struct.pack("<7I", 124, flags, height, width, pitch, 0, len(mips))
            + b"\0" * 44 + pf + struct.pack("<5I", caps, 0, 0, 0, 0))


def main(src, dst):
    dst = Path(dst)
    dst.mkdir(parents=True, exist_ok=True)
    done = 0
    for path in sorted(Path(src).glob("*.nif")):
        try:
            pixel_format, bpp, mips, pixels = parse(path.read_bytes())
        except ValueError as err:
            print(f"skip {path.name}: {err}")
            continue
        header = dds_header(pixel_format, bpp, mips)
        if header is None:
            print(f"skip {path.name}: pixel format {pixel_format}, {bpp} bpp")
            continue
        (dst / (path.stem + ".dds")).write_bytes(header + pixels)
        done += 1
    print(f"wrote {done} textures to {dst}")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
