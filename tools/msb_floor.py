"""Edit an uncompressed Elden Ring map file (MSBE) directly, without Soulstruct (stdlib only).

Soulstruct can't read the Stranded Graveyard map ("Offset for unused struct subtype_data in MSBOtherRegion is non-zero"),
so this reads the file as its six param lists of raw entries and only changes what it needs to: it adds models, appends
copies of an asset part and moves parts. Entries only hold offsets relative to themselves, so rewriting the lists with
new tables keeps every other entry byte for byte (`write_msb(read_msb(data)) == data`).

Usage: python msb_floor.py map.msb street.flver [out.msb]   (lays the invisible floor, see flver_floor.py)
"""

import struct
import sys
from pathlib import Path

ASSET_PART = 13
ASSET_MODEL = 10
ENTITY_DATA_FIELD = 0x60  # part header field holding the offset of its entity data (entity id first)


class ParamList:
    def __init__(self, version, name, entries):
        self.version, self.name, self.entries = version, name, entries

    def title(self):
        return self.name.decode("utf-16-le", "replace").split("\0")[0]


def read_msb(data):
    """(file header, [ParamList]) of a map file."""
    lists, offset = [], 16
    while True:
        version, count, name_offset = struct.unpack_from("<iiq", data, offset)
        table = struct.unpack_from(f"<{count}q", data, offset + 16)
        starts, following = list(table[:-1]), table[-1]
        assert name_offset == offset + 16 + 8 * count, "unexpected list layout"
        end = following or len(data)
        name = data[name_offset:starts[0] if starts else end]
        lists.append(ParamList(version, name, [data[a:b] for a, b in zip(starts, starts[1:] + [end])]))
        if not following:
            return data[:16], lists
        offset = following


def write_msb(header, lists):
    out = bytearray(header)
    for i, plist in enumerate(lists):
        start = len(out)
        table_end = start + 16 + 8 * (len(plist.entries) + 1)
        offsets, position = [], table_end + len(plist.name)
        for entry in plist.entries:
            offsets.append(position)
            position += len(entry)
        following = position if i + 1 < len(lists) else 0
        out += struct.pack("<iiq", plist.version, len(plist.entries) + 1, table_end)
        out += struct.pack(f"<{len(offsets) + 1}q", *offsets, following)
        out += plist.name
        for entry in plist.entries:
            out += entry
    return bytes(out)


def entry_name(entry):
    offset = struct.unpack_from("<q", entry, 0)[0]
    end = offset
    while entry[end:end + 2] != b"\0\0":
        end += 2
    return entry[offset:end].decode("utf-16-le")


def _set_name(entry, name):
    offset = struct.unpack_from("<q", entry, 0)[0]
    old = len(entry_name(entry))
    assert len(name) <= old, f"{name} is longer than the copied part's name"
    entry[offset:offset + 2 * old + 2] = (name + "\0").encode("utf-16-le").ljust(2 * old + 2, b"\0")


def _list(lists, title):
    return next(p for p in lists if p.title() == title)


def find(plist, name):
    return next(i for i, e in enumerate(plist.entries) if entry_name(e) == name)


def add_model(lists, name, template):
    """Index of model `name`, adding it as a copy of model `template` (same name length, e.g. AEG099_001)."""
    models = _list(lists, "MODEL_PARAM_ST")
    names = [entry_name(e) for e in models.entries]
    if name in names:
        return names.index(name)
    assert len(name) == len(template)
    entry = bytearray(models.entries[names.index(template)])
    old, new = template.encode("utf-16-le"), name.encode("utf-16-le")
    entry[:] = entry.replace(old, new).replace(old.lower(), new.lower())
    model_type = struct.unpack_from("<i", entry, 8)[0]
    same_type = [struct.unpack_from("<i", e, 12)[0] for e in models.entries if struct.unpack_from("<i", e, 8)[0] == model_type]
    struct.pack_into("<i", entry, 12, max(same_type) + 1)
    models.entries.append(bytes(entry))
    return len(models.entries) - 1


def set_transform(entry, position=None, rotation=None):
    entry = bytearray(entry)
    if position is not None:
        struct.pack_into("<3f", entry, 0x20, *position)
    if rotation is not None:
        struct.pack_into("<3f", entry, 0x2C, *rotation)
    return bytes(entry)


def transform(entry):
    return struct.unpack_from("<3f", entry, 0x20), struct.unpack_from("<3f", entry, 0x2C)


def remove_parts(lists, prefix):
    """Drop the parts named `prefix`..., which must be the last parts (as `add_parts` leaves them), so no other part
    changes index. Returns how many were removed."""
    parts = _list(lists, "PARTS_PARAM_ST").entries
    keep = len(parts)
    while keep and entry_name(parts[keep - 1]).startswith(prefix):
        keep -= 1
    assert not any(entry_name(e).startswith(prefix) for e in parts[:keep]), f"{prefix} parts are not all at the end"
    removed = len(parts) - keep
    del parts[keep:]
    return removed


def add_parts(lists, template, model_index, placements, first_id=8000):
    """Append a copy of asset part `template` for each (name, position, rotation), using model `model_index`, with no
    entity id. Assets are the last part type, so the copies go at the end of the parts list."""
    parts = _list(lists, "PARTS_PARAM_ST").entries
    source = parts[find(_list(lists, "PARTS_PARAM_ST"), template)]
    assert struct.unpack_from("<i", parts[-1], 0x0C)[0] == ASSET_PART == struct.unpack_from("<i", source, 0x0C)[0]
    type_index = struct.unpack_from("<i", parts[-1], 0x10)[0]
    for k, (name, position, rotation) in enumerate(placements):
        entry = bytearray(set_transform(source, position, rotation))
        _set_name(entry, name)
        type_index += 1
        struct.pack_into("<iiii", entry, 0x08, first_id + k, ASSET_PART, type_index, model_index)
        struct.pack_into("<i", entry, struct.unpack_from("<q", entry, ENTITY_DATA_FIELD)[0], 0)
        parts.append(bytes(entry))
    models = _list(lists, "MODEL_PARAM_ST").entries
    users = sum(struct.unpack_from("<i", e, 0x14)[0] == model_index for e in parts)
    model = bytearray(models[model_index])
    struct.pack_into("<i", model, 0x18, users)
    models[model_index] = bytes(model)


STREET = "AEG030_518_9000"
FLOOR_MODEL = "AEG099_002"  # invisible wall, collision box x -7.466..7.459, y 0..8.343, z +-0.065
FLOOR_TEMPLATE_MODEL = "AEG099_001"
FLOOR_TEMPLATE_PART = "AEG099_001_9000"  # a vanilla invisible wall asset in m18
FLOOR_PREFIX = FLOOR_MODEL + "_8"  # part names AEG099_002_8000 and up
GROUND_Y = 13.8  # where Ryan placed the street in Smithbox; the road surface sits at the street's own height 0


def lay_floor(data, tiles, half_thickness, street=STREET, ground_y=GROUND_Y):
    """Map bytes with the street levelled at `ground_y`, unrotated, and two flat invisible walls (tipped +90 and -90
    degrees about X, since which way the game turns them is unverified) on each (x, height, z) tile of the street."""
    header, lists = read_msb(data)
    parts = _list(lists, "PARTS_PARAM_ST")
    removed = remove_parts(lists, FLOOR_PREFIX)
    index = find(parts, street)
    (x, _, z), _ = transform(parts.entries[index])
    parts.entries[index] = set_transform(parts.entries[index], (x, ground_y, z), (0.0, 0.0, 0.0))
    model = add_model(lists, FLOOR_MODEL, FLOOR_TEMPLATE_MODEL)
    placements = []
    for tx, height, tz in tiles:
        for tilt in (90.0, -90.0):
            placements.append((f"{FLOOR_PREFIX}{len(placements):03d}", (x + tx, ground_y + height - half_thickness,
                                                                        z + tz), (tilt, 0.0, 0.0)))
    add_parts(lists, FLOOR_TEMPLATE_PART, model, placements)
    print(f"Removed {removed} old floor pieces, placed {len(placements)} over {len(tiles)} tiles; "
          f"street at ({x:.3f}, {ground_y}, {z:.3f})")
    return write_msb(header, lists)


def main():
    import flver_floor

    source, flver = Path(sys.argv[1]), Path(sys.argv[2])
    target = Path(sys.argv[3]) if len(sys.argv) > 3 else source
    data = source.read_bytes()
    assert write_msb(*read_msb(data)) == data, "map does not round-trip"
    tiles = flver_floor.floor_tiles(flver.read_bytes())
    target.write_bytes(lay_floor(data, tiles, flver_floor.PIECE_HALF_THICKNESS))


if __name__ == "__main__":
    main()
