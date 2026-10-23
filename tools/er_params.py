"""Read Elden Ring's game params from regulation.bin with Smithbox's paramdef XMLs, stdlib + cryptography + zstandard.

regulation.bin = 16-byte IV + AES-256-CBC (the game's well-known key) of a DCX (ZSTD in 2.x, DFLT before) of a BND4 of
<Name>.param files. A PARAM is a 0x40-byte header (row count at 0x0A) then one 24-byte entry per row (id, pad, data
offset, name offset); each row is a fixed-size struct laid out as the paramdef's fields (bitfields of the same type
pack into one unit of that type, arrays as [n], fixstr/fixstrW as byte / UTF-16 arrays).

    from er_params import Regulation
    reg = Regulation("regulation.bin", r"...\\Smithbox\\Assets\\PARAM\\ER\\Defs")
    goods = reg.param("EquipParamGoods")   # {row id: {field: value}}

Usage: python er_params.py regulation.bin <Defs folder> <Param> [row id]   (prints rows)
"""

import re
import struct
import sys
import zlib
from pathlib import Path
from xml.etree import ElementTree

sys.path.insert(0, str(Path(__file__).parent))
from er_text import bnd4  # noqa: E402

KEY = bytes.fromhex("99BFFC366A6BC8C6F5827D093602D676C42892A01C207FB024D3AF4E493FEF99")

TYPES = {"u8": "B", "s8": "b", "u16": "H", "s16": "h", "u32": "I", "s32": "i", "f32": "f", "angle32": "f",
         "b32": "i", "f64": "d", "dummy8": "B", "fixstr": "s", "fixstrW": "S"}
SIZES = {"B": 1, "b": 1, "H": 2, "h": 2, "I": 4, "i": 4, "f": 4, "d": 8, "s": 1, "S": 2}


def decrypt(data):
    """The BND4 bytes inside regulation.bin."""
    from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
    d = Cipher(algorithms.AES(KEY), modes.CBC(data[:16])).decryptor()
    raw = d.update(data[16:]) + d.finalize()
    if raw[:4] != b"DCX\0":
        return raw
    usize, csize = struct.unpack_from(">II", raw, 0x1C)
    kind, body = raw[0x28:0x2C], raw[0x4C:0x4C + csize]
    if kind == b"ZSTD":
        import zstandard
        return zstandard.ZstdDecompressor().decompress(body, max_output_size=usize)
    if kind == b"DFLT":
        return zlib.decompress(body)
    raise ValueError(f"unsupported DCX {kind}")


def layout(def_path):
    """[(name, struct code, count, bits)] of a paramdef XML; bits = None for whole fields."""
    fields = []
    for f in ElementTree.parse(def_path).getroot().iter("Field"):
        if f.get("RemovedVersion") or f.find("RemovedRevision") is not None:
            continue  # fields older regulation versions had (the game's regulation.bin is the newest)
        m = re.match(r"(\w+)\s+(\w+)(?:\[(\d+)\])?(?::\s*(\d+))?", f.get("Def"))
        kind, name, count, bits = m.group(1), m.group(2), int(m.group(3) or 1), m.group(4)
        fields.append((name, TYPES[kind], count, int(bits) if bits else None))
    return fields


def read_row(data, at, fields):
    out = {}
    unit_code, unit_value, unit_used = None, 0, 0
    pos = at
    for name, code, count, bits in fields:
        size = SIZES[code]
        if bits is not None:
            if unit_code != code or unit_used + bits > size * 8:
                unit_code, unit_used = code, 0
                unit_value = struct.unpack_from("<" + {"B": "B", "b": "B", "H": "H", "h": "H", "I": "I", "i": "I"}[code],
                                                data, pos)[0]
                pos += size
            out[name] = (unit_value >> unit_used) & ((1 << bits) - 1)
            unit_used += bits
            continue
        unit_code = None
        if code == "s":
            out[name] = data[pos:pos + count].split(b"\0")[0].decode("shift-jis", "replace")
        elif code == "S":
            out[name] = data[pos:pos + 2 * count].decode("utf-16-le", "replace").split("\0")[0]
        elif count == 1:
            out[name] = struct.unpack_from("<" + code, data, pos)[0]
        else:
            out[name] = list(struct.unpack_from(f"<{count}{code}", data, pos))
        pos += size * count
    return out, pos - at


class Regulation:
    def __init__(self, regulation, defs):
        self.files = {k.replace("\\", "/").rsplit("/", 1)[-1].removesuffix(".param"): v
                      for k, v in bnd4(decrypt(Path(regulation).read_bytes())).items()}
        self.defs = Path(defs)

    def param(self, name, def_name=None):
        data = self.files[name]
        fields = layout(self.defs / f"{def_name or name}.xml")
        count = struct.unpack_from("<H", data, 0x0A)[0]
        rows = {}
        entries = [struct.unpack_from("<iiqq", data, 0x40 + 24 * i) for i in range(count)]
        for i, (row_id, _, offset, _) in enumerate(entries):
            row, size = read_row(data, offset, fields)
            if i + 1 < count:
                assert size == entries[i + 1][2] - offset, f"{name}: paramdef size {size} != row size"
            rows[row_id] = row
        return rows


def main():
    reg = Regulation(sys.argv[1], sys.argv[2])
    name = sys.argv[3]
    rows = reg.param(name, {"Magic": "MagicParam"}.get(name))
    for row_id, row in rows.items():
        if len(sys.argv) > 4 and row_id != int(sys.argv[4]):
            continue
        print(row_id, row)


if __name__ == "__main__":
    main()
