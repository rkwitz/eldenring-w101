"""Pack DDS textures into an Elden Ring (PC) TPF texture binder, stdlib only.

A PC TPF is a 16-byte header, one 20-byte entry per texture, the texture names (UTF-16) and then each whole DDS file.
The game looks textures up by name: a FLVER texture path ending in AET030_518_000_a.tif is found in
asset/aet/aet030/aet030_518.tpf.dcx (folder from the first 6 characters, file from the first 10).
"""

import struct

FORMATS = {b"DXT1": 0, b"DXT3": 3, b"DXT5": 5}  # TPF format byte for each DDS four-character code


def dds_info(dds):
    """(four-character code, width, height, mipmap count) of a DDS file."""
    assert dds[:4] == b"DDS ", "not a DDS file"
    height, width = struct.unpack_from("<II", dds, 12)
    mipmaps = max(1, struct.unpack_from("<I", dds, 28)[0])
    return dds[84:88], width, height, mipmaps


def has_alpha(dds):
    """Whether a DXT1/3/5 texture has any transparent texels (DXT1: a 3-colour block using index 3)."""
    fourcc, width, height, _ = dds_info(dds)
    if fourcc != b"DXT1":
        return True
    for block in range(max(1, width // 4) * max(1, height // 4)):
        c0, c1, bits = struct.unpack_from("<HHI", dds, 128 + 8 * block)
        if c0 <= c1 and any((bits >> (2 * k)) & 3 == 3 for k in range(16)):
            return True
    return False


def solid_dds(r, g, b):
    """A 4x4 DXT1 DDS of one colour, e.g. a flat normal map."""
    colour = (r >> 3) << 11 | (g >> 2) << 5 | (b >> 3)
    pixel_format = struct.pack("<II4s5I", 32, 0x4, b"DXT1", 0, 0, 0, 0, 0)
    header = struct.pack("<4s7I44x", b"DDS ", 124, 0x81007, 4, 4, 8, 0, 1) + pixel_format
    header += struct.pack("<5I", 0x1000, 0, 0, 0, 0)
    # Colour 0 above colour 1 keeps the block in 4-colour (opaque) mode.
    return header + struct.pack("<HHI", colour, max(colour - 1, 0), 0)


def build_tpf(textures):
    """TPF bytes for [(name, dds bytes)]."""
    entries_end = 16 + 20 * len(textures)
    names = bytearray()
    name_offsets = []
    for name, _ in textures:
        name_offsets.append(entries_end + len(names))
        names += name.encode("utf-16-le") + b"\0\0"
    data_start = entries_end + len(names)
    data_start += -data_start % 16
    data = bytearray()
    entries = bytearray()
    for (name, dds), name_offset in zip(textures, name_offsets):
        data += bytes(-len(data) % 16)
        fourcc, _, _, mipmaps = dds_info(dds)
        entries += struct.pack("<IiBBBBIi", data_start + len(data), len(dds), FORMATS[fourcc], 0, mipmaps, 0,
                               name_offset, 0)
        data += dds
    # Platform PC (0), flags 3, names in UTF-16 (1).
    header = struct.pack("<4siiBBBx", b"TPF\0", len(data), len(textures), 0, 3, 1)
    padding = bytes(data_start - entries_end - len(names))
    return header + bytes(entries) + bytes(names) + padding + bytes(data)
