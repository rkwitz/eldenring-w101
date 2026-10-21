"""Place the classic Wizard101 sky dome (asset aeg030_523, built by blender_build_sky.py) in m60_42_36_00.

The dome is modelled around its own origin, so its part AEG030_523_9000 sits at the dome's centre on the street
(sky_bake.CENTRE in street coordinates, carried through the street part's transform) and turns with the street. Its
PartsDrawParam override is row 845 (vanilla: never fades, 9999 m draw distance, casts and receives no shadows). The
street's row 36 would let the closed dome cast sun shadows over the whole street. If you move the street in Smithbox,
run this again (it updates the part in place).

Usage: python msb_add_sky.py map.msb.dcx out.msb.dcx
"""

import math
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_floor import (ENTITY_DATA_FIELD, _list, _set_name, add_model, entry_name, find, read_msb,  # noqa: E402
                       set_transform, transform, write_msb)
from sky_bake import CENTRE  # noqa: E402

STREET, STREET_MODEL = "AEG030_518_9000", "AEG030_518"
SKY, SKY_MODEL = "AEG030_523_9000", "AEG030_523"
DRAW_PARAM = 845


def sky_position(street_position, street_rotation):
    """World position of the dome centre: MSB parts map FLVER (x, y, z) (= OBJ with z negated) to
    position + (x cos + z sin, y, -x sin + z cos) for their y rotation."""
    assert street_rotation[0] == 0 and street_rotation[2] == 0, "street is tilted"
    x, y, z = CENTRE[0], CENTRE[1], -CENTRE[2]
    c, s = math.cos(math.radians(street_rotation[1])), math.sin(math.radians(street_rotation[1]))
    return (street_position[0] + x * c + z * s, street_position[1] + y, street_position[2] - x * s + z * c)


def place_sky(raw):
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    names = [entry_name(e) for e in parts.entries]
    street = parts.entries[find(parts, STREET)]
    if SKY in names:
        k = names.index(SKY)
        e = bytearray(parts.entries[k])
    else:
        model = add_model(lists, SKY_MODEL, STREET_MODEL)
        e = bytearray(street)
        _set_name(e, SKY)
        struct.pack_into("<i", e, 0x10, sum(1 for x in parts.entries if x[0x0C:0x10] == e[0x0C:0x10]))  # type index
        struct.pack_into("<i", e, 0x14, model)
        k = None
    entity = struct.unpack_from("<q", e, ENTITY_DATA_FIELD)[0]
    struct.pack_into("<i", e, entity, 0)  # no entity id
    e[entity + 4] = 1  # use the PartsDrawParam override below
    struct.pack_into("<h", e, entity + 8, DRAW_PARAM)
    position, rotation = transform(street)
    e = set_transform(bytes(e), sky_position(position, rotation), rotation)
    if k is None:
        parts.entries.append(e)
    else:
        parts.entries[k] = e
    print(f"{SKY}: position {tuple(round(c, 3) for c in transform(e)[0])}, rotation {transform(e)[1]}, "
          f"PartsDrawParam {DRAW_PARAM}")
    return write_msb(header, lists)


def main():
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    data = src.read_bytes()
    assert data[:4] == b"DCX\0" and data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed map"
    out = place_sky(zlib.decompress(data[0x4C:]))
    packed = zlib.compress(out, 9)
    head = bytearray(data[:0x4C])
    struct.pack_into(">II", head, 0x1C, len(out), len(packed))
    dst.write_bytes(bytes(head) + packed)


if __name__ == "__main__":
    main()
