"""Place the classic Wizard101 sky dome (asset aeg030_523, built by blender_build_sky.py) in m60_42_36_00.

The dome is modelled in the street's own coordinates, so its part AEG030_523_9000 is a copy of the street part with the
same transform and draw settings (including its PartsDrawParam override, so it doesn't fade out), and no entity id.
If you move the street in Smithbox, move AEG030_523_9000 the same way.

Usage: python msb_add_sky.py map.msb.dcx out.msb.dcx
"""

import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_floor import ENTITY_DATA_FIELD, _list, _set_name, add_model, entry_name, find, read_msb, write_msb  # noqa: E402

STREET, STREET_MODEL = "AEG030_518_9000", "AEG030_518"
SKY, SKY_MODEL = "AEG030_523_9000", "AEG030_523"


def add_sky(raw):
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    assert SKY not in [entry_name(e) for e in parts.entries], f"{SKY} is already in the map"
    model = add_model(lists, SKY_MODEL, STREET_MODEL)
    e = bytearray(parts.entries[find(parts, STREET)])
    _set_name(e, SKY)
    struct.pack_into("<i", e, 0x14, model)
    struct.pack_into("<i", e, struct.unpack_from("<q", e, ENTITY_DATA_FIELD)[0], 0)
    parts.entries.append(bytes(e))
    return write_msb(header, lists)


def main():
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    data = src.read_bytes()
    assert data[:4] == b"DCX\0" and data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed map"
    out = add_sky(zlib.decompress(data[0x4C:]))
    packed = zlib.compress(out, 9)
    head = bytearray(data[:0x4C])
    struct.pack_into(">II", head, 0x1C, len(out), len(packed))
    dst.write_bytes(bytes(head) + packed)


if __name__ == "__main__":
    main()
