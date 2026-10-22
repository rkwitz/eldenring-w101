"""Copy named regions from another version of a map back into it, unchanged, keeping the region list grouped by type.

Used 2026-10-22 to restore m60_42_36_00's two Connection regions (type 21: "m18_00_00接続　出口" to the Stranded
Graveyard exit, "m32_01接続" to the catacomb) after a Smithbox clean-up deleted them; they tell the game which
neighbouring map to load across the door.

Each region goes at the end of its type's block (or, if the block is gone, before the first block of a higher type,
"Other" (-1) counting as last, as in vanilla maps), with the next per-type index. Every region index held by events
(common RegionIndex, PatrolInfo points) is remapped. Regions holding a part index (ActivationPartIndex) or a region id
already used are refused, since part indices differ between versions.

Usage: python msb_copy_regions.py map.msb.dcx source.msb.dcx out.msb.dcx "region name" ["region name" ...]
"""

import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_add_ghost import EVENT_PATROL, check_grouped, load, q, u32  # noqa: E402
from msb_floor import _list, entry_name, read_msb, write_msb  # noqa: E402


def _order(kind):
    return float("inf") if kind == -1 else kind


def remap_regions(lists, shift):
    events = _list(lists, "EVENT_PARAM_ST").entries
    for k, e in enumerate(events):
        e = bytearray(e)
        common = q(e, 0x18)
        if u32(e, common + 4) >= 0:
            struct.pack_into("<i", e, common + 4, shift(u32(e, common + 4)))
        if u32(e, 0x0C) == EVENT_PATROL:
            td = q(e, 0x20)
            old = struct.unpack_from("<64h", e, td + 16)
            struct.pack_into("<64h", e, td + 16, *[shift(i) if i >= 0 else -1 for i in old])
        events[k] = bytes(e)


def copy_regions(raw, source_raw, names):
    header, lists = read_msb(raw)
    _, source_lists = read_msb(source_raw)
    check_grouped(lists)
    regions = _list(lists, "POINT_PARAM_ST").entries
    source = {entry_name(e): e for e in _list(source_lists, "POINT_PARAM_ST").entries}
    for name in names:
        assert name not in {entry_name(e) for e in regions}, f"{name} is already in the map"
        e = bytearray(source[name])
        kind = u32(e, 0x08)
        assert u32(e, q(e, 0x50)) == -1, f"{name} has an activation part"
        assert u32(e, 0x2C) not in {u32(x, 0x2C) for x in regions}, f"{name}: region id {u32(e, 0x2C)} is taken"
        same = [i for i, x in enumerate(regions) if u32(x, 0x08) == kind]
        at = same[-1] + 1 if same else next(
            (i for i, x in enumerate(regions) if _order(u32(x, 0x08)) > _order(kind)), len(regions))
        struct.pack_into("<i", e, 0x0C, len(same))
        remap_regions(lists, lambda i: i + 1 if i >= at else i)
        regions.insert(at, bytes(e))
        print(f"{name}: type {kind}, inserted at region {at}, type index {len(same)}")
    check_grouped(lists)
    return write_msb(header, lists)


def main():
    src, source, dst, *names = sys.argv[1:]
    data = Path(src).read_bytes()
    assert data[:4] == b"DCX\0" and data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed map"
    raw = zlib.decompress(data[0x4C:])
    out = copy_regions(raw, load(source), names)
    packed = zlib.compress(out, 9)
    head = bytearray(data[:0x4C])
    struct.pack_into(">II", head, 0x1C, len(out), len(packed))
    Path(dst).write_bytes(bytes(head) + packed)


if __name__ == "__main__":
    main()
