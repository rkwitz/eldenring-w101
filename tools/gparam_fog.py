"""Turn off Elden Ring's distance fog (GPARAM VolumetricFog) in drawparam files, stdlib only (+ pyooz for KRAK input).

Elden Ring's fog is the VolumetricFog group of the .gparam files in param/drawparam: the map's light set (The First
Step's collisions use light set 100 = m60_00_0100, the misty start area: 0.005/m fixed density plus height fog,
analytical fog on; m60_00_0000 is the overworld base) and the weather layers (m00_00_XXXX_weatheroutdoor, one per
weather, each with its own fog). For each file this sets every time-of-day value of
  VolumetricFog Enable, VolumetricFog EnableAnalyticalFog                -> off
  VolumetricFog Global FixedDensity, VolumetricFog Global HeightFogDensity Scale -> 0
and leaves everything else (lighting, FogVolume local fog boxes) alone. The weather files apply to the whole overworld,
so their edit is global.

GPARAM (version 5, "filt"): header ints at 0x10 group count, 0x18 group offset table, 0x1C group headers, 0x20 param
header offset tables, 0x24 param headers, 0x28 values, 0x2C value ids. A group header is (param count, offset of its
param offset table), then two UTF-16 names; a param header is (values offset, value ids offset, type byte, value
count byte, 2 pad), then two UTF-16 names. Types used here: 0x0B bool (1 byte each), 0x09 float.

Usage: python gparam_fog.py out_dir file.gparam.dcx ...
  Reads KRAK (needs the pyooz module), DFLT or uncompressed files; writes DFLT .gparam.dcx files with the same names.
"""

import struct
import sys
import zlib
from pathlib import Path

OFF = {"VolumetricFog Enable": 0, "VolumetricFog EnableAnalyticalFog": 0,
       "VolumetricFog Global FixedDensity": 0.0, "VolumetricFog Global HeightFogDensity Scale": 0.0}
DFLT_HEADER = bytes.fromhex(
    "44435800000100000000001800000024000000440000004c4443530000000000000000004443"
    "500044464c540000002009000000000000000000000000000000000101004443410000000008")  # DCX DFLT as Smithbox writes it, sizes at 0x1C


def _utf16z(data, offset):
    end = offset
    while data[end:end + 2] != b"\0\0":
        end += 2
    return data[offset:end].decode("utf-16-le"), end + 2


def params(data):
    """[(group name, param name, type, value count, values offset)] of a GPARAM."""
    assert data[:8] == "filt".encode("utf-16-le") and struct.unpack_from("<i", data, 8)[0] == 5, "not an ER GPARAM"
    count = struct.unpack_from("<i", data, 0x10)[0]
    group_table, groups, param_tables, param_headers, values, _ = struct.unpack_from("<6i", data, 0x18)
    out = []
    for g in range(count):
        group = groups + struct.unpack_from("<i", data, group_table + 4 * g)[0]
        param_count, table = struct.unpack_from("<2i", data, group)
        group_name, _ = _utf16z(data, group + 8)
        for k in range(param_count):
            header = param_headers + struct.unpack_from("<i", data, param_tables + table + 4 * k)[0]
            value_offset, _, kind, value_count = struct.unpack_from("<iiBB", data, header)
            name, _ = _utf16z(data, header + 12)
            out.append((group_name, name, kind, value_count, values + value_offset))
    return out


def remove_fog(data):
    """(new GPARAM bytes, [names of changed params])."""
    out = bytearray(data)
    changed = []
    for _, name, kind, count, offset in params(data):
        if name not in OFF:
            continue
        if kind == 0x0B:
            out[offset:offset + count] = bytes([int(OFF[name])]) * count
        elif kind == 0x09:
            struct.pack_into(f"<{count}f", out, offset, *[float(OFF[name])] * count)
        else:
            raise ValueError(f"unexpected type {kind:#x} for {name}")
        changed.append(name)
    return bytes(out), changed


def read_dcx(raw):
    if raw[:4] != b"DCX\0":
        return raw
    usize, csize = struct.unpack_from(">II", raw, 0x1C)
    kind = raw[0x28:0x2C]
    if kind == b"DFLT":
        return zlib.decompress(raw[0x4C:0x4C + csize])
    if kind == b"KRAK":
        import ooz  # pyooz

        return ooz.decompress(raw[0x4C:0x4C + csize], usize)
    raise ValueError(f"unsupported DCX {kind!r}")


def write_dflt(data):
    packed = zlib.compress(data, 9)
    header = bytearray(DFLT_HEADER)
    struct.pack_into(">II", header, 0x1C, len(data), len(packed))
    return bytes(header) + packed


def main():
    out_dir = Path(sys.argv[1])
    out_dir.mkdir(parents=True, exist_ok=True)
    for path in map(Path, sys.argv[2:]):
        data = read_dcx(path.read_bytes())
        new, changed = remove_fog(data)
        if not changed:
            print(f"{path.name}: no fog params")
            continue
        (out_dir / path.name).write_bytes(write_dflt(new))
        print(f"{path.name}: {', '.join(sorted(set(n.replace('VolumetricFog ', '') for n in changed)))}")


if __name__ == "__main__":
    main()
