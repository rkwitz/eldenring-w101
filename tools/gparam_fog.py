"""Turn off Elden Ring's distance fog (GPARAM VolumetricFog) in drawparam files, stdlib only (+ pyooz for KRAK input).

Elden Ring's fog is the VolumetricFog group of the .gparam files in param/drawparam: the map's light set (The First
Step's collisions use light set 100 = m60_00_0100, the misty start area: 0.005/m fixed density plus height fog,
analytical fog on; m60_00_0000 is the overworld base) and the weather layers (m00_00_XXXX_weatheroutdoor, one per
weather, each with its own fog). For each file this sets every time-of-day value of
  VolumetricFog Enable, VolumetricFog EnableAnalyticalFog                -> off
  VolumetricFog Global FixedDensity, VolumetricFog Global HeightFogDensity Scale -> 0
and leaves everything else (FogVolume local fog boxes) alone. The weather files apply to the whole overworld,
so their edit is global.

fixed_exposure() and no_bloom() (also blender_build_sky.py only) turn off eye adaptation and bloom.

flat_lighting() (used by blender_build_sky.py, not the command line) also makes the LightSet group light everything
evenly, as Wizard101 does (unlit textures plus baked vertex colours): every directional light (the sun DiffColor0/
SpecColor0 that draws the shadows, the fill light 1 and the local lights 1 and 2) gets intensity 0 (a colour's 4th
value is its intensity), and the hemisphere ambient (Hemi Color Up and Down) becomes white at one intensity from
above and below; optionally directional light 1 (no shadows) stays on as a white light straight down. Vanilla: sun
0.2 to 0.6 by day (2.5 in light set 100), hemi up 0.1 (1.0 in some), hemi down 0.

GPARAM (version 5, "filt"): header ints at 0x10 group count, 0x18 group offset table, 0x1C group headers, 0x20 param
header offset tables, 0x24 param headers, 0x28 values, 0x2C value ids. A group header is (param count, offset of its
param offset table), then two UTF-16 names; a param header is (values offset, value ids offset, type byte, value
count byte, 2 pad), then two UTF-16 names. Types used here: 0x0B bool (1 byte each), 0x09 float.

Usage: python gparam_fog.py out_dir file.gparam.dcx ...
  Reads KRAK (needs the pyooz module), DFLT or uncompressed files; writes DFLT .gparam.dcx files with the same names.
"""

import math
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


DIRECTIONAL = ("Directional Light DiffColor0", "Directional Light SpecColor0", "Directional Light DiffColor1",
               "Directional Light SpecColor1", "Directional Light LocalLight DiffColor1",
               "Directional Light LocalLight SpecColor1", "Directional Light LocalLight DiffColor2",
               "Directional Light LocalLight SpecColor2")
HEMI = ("Directional Light Hemi Color Up", "Directional Light Hemi Color Down")
COLOUR = 0x0E  # 4 floats: r, g, b, intensity


OVERHEAD_LIGHT, OVERHEAD_ANGLE = "Directional Light DiffColor1", "Directional Light Angle1"
ANGLE = 0x0C  # 2 floats (pitch, yaw in radians) in 16 bytes; vanilla light 1 mostly points straight down (-pi/2, 0)


def flat_lighting(data, ambient, overhead=0.0):
    """(new GPARAM bytes, [names of changed params]): white hemisphere ambient of intensity `ambient` from every
    side and no directional light (see the module docstring), except, with `overhead` > 0, directional light 1 (the
    fill light; only light 0, the sun, casts shadows) as white light of that intensity straight down."""
    out = bytearray(data)
    changed = []
    for _, name, kind, count, offset in params(data):
        if name == OVERHEAD_ANGLE and overhead > 0:
            if kind != ANGLE:
                raise ValueError(f"unexpected type {kind:#x} for {name}")
            for k in range(count):
                struct.pack_into("<2f", out, offset + 16 * k, -math.pi / 2, 0.0)
            changed.append(name)
            continue
        if name not in DIRECTIONAL and name not in HEMI:
            continue
        if kind != COLOUR:
            raise ValueError(f"unexpected type {kind:#x} for {name}")
        for k in range(count):
            at = offset + 16 * k
            if name in HEMI:
                struct.pack_into("<4f", out, at, 1.0, 1.0, 1.0, ambient)
            elif name == OVERHEAD_LIGHT:
                struct.pack_into("<4f", out, at, 1.0, 1.0, 1.0, overhead)
            else:
                struct.pack_into("<f", out, at + 12, 0.0)
        changed.append(name)
    return bytes(out), changed


AUTO_EXPOSURE = "GS Presents - ToneMap-Enable AutoExposure"  # type 0x07 (one byte per value)
BLOOM_OFF = {"Gs Presents-Glare Luminance": 0.0, "Gs Presents-GlareBloom Alpha": 0.0}  # floats


def fixed_exposure(data):
    """(new GPARAM bytes, [names of changed params]) with auto exposure (eye adaptation) off: the tone map then uses
    its ToneMap-Exposure value as is, as vanilla default.gparam and the overcast/night weathers (0031..0083) do. With
    it on, a big bright sky filling the view pulls the exposure down (by up to 1.5 stops, Adaption Min) and a view of
    the street pushes it up, so the same sky looks different by where you look and every glow change is partly
    undone or overshot. Files without a Tone Map group (e.g. m60_00_0100) are left alone."""
    out = bytearray(data)
    changed = []
    for _, name, kind, count, offset in params(data):
        if name == AUTO_EXPOSURE:
            if kind not in (0x07, 0x0B):
                raise ValueError(f"unexpected type {kind:#x} for {name}")
            out[offset:offset + count] = bytes(count)
            changed.append(name)
    return bytes(out), changed


def no_bloom(data):
    """(new GPARAM bytes, [names of changed params]) without bloom (glare): Glare Luminance and GlareBloom Alpha 0.
    Light set 100 (The First Step and our map) blooms everything above 0.25 at 1.5x, which turned a large bright sky
    into a white haze; default.gparam blooms above 0.5. Files without a Bloom group are left alone."""
    out = bytearray(data)
    changed = []
    for _, name, kind, count, offset in params(data):
        if name in BLOOM_OFF:
            if kind != 0x09:
                raise ValueError(f"unexpected type {kind:#x} for {name}")
            struct.pack_into(f"<{count}f", out, offset, *[BLOOM_OFF[name]] * count)
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
