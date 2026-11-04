"""Write Elden Ring's regulation.bin: the read side of er_params.py turned round, laid out exactly as Smithbox saves it.

Round 33 (U6b). Checked on Ryan's Game\regulation.bin (md5 a631bf54, version 11730034): rebuilt, it decompresses
to the same BND4 byte for byte (181 params re-laid-out), wrapped as Smithbox wraps it. An earlier hand-written regulation crashed the game at launch (project/regulation.bin.sorted_crash).
Against Smithbox's file (regulation.bin.unsorted = .prev, which launches) it differs only in PartsDrawParam's row
order and in its wrapping: a random IV, and a ZSTD frame with the content size and a 4 MB window. Smithbox's file
has an all-zero IV and a frame without the content size and with a 64 KB window (zstd level 15, windowLog 16: the
DCX header's level byte is 0x0F). This writer keeps the IV and recompresses with Smithbox's settings, so an
unchanged rebuild is byte-identical to its input (`roundtrip`), and changed files keep every other byte.

  BND4: header, entries and names are kept; the files are put back one after another in entry order, each on a
  16-byte boundary (as in the input), with the entries' sizes and offsets updated.
  PARAM: the 0x40-byte header (strings offset at 0x00 and 0x10, data start at 0x30, row count at 0x0A), the row
  table sorted by id, the rows' data in id order, then the param type string and row names unchanged. A new row
  shares the name of the row it copies (vanilla rows share names already).

Usage:
  python er_regulation.py roundtrip regulation.bin
  python er_regulation.py u6b regulation.bin <Smithbox PARAM\\ER\\Defs> out.bin
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from er_params import KEY, SIZES, layout  # noqa: E402


def _aes(iv, data, encrypt):
    from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
    c = Cipher(algorithms.AES(KEY), modes.CBC(iv))
    x = c.encryptor() if encrypt else c.decryptor()
    return x.update(data) + x.finalize()


def unpack(data):
    """(iv, DCX header, padding tail after the frame, BND4 bytes) of a regulation.bin."""
    import zstandard
    iv = data[:16]
    raw = _aes(iv, data[16:], False)
    assert raw[:4] == b"DCX\0" and raw[0x28:0x2C] == b"ZSTD", "expected a ZSTD DCX"
    usize, csize = struct.unpack_from(">II", raw, 0x1C)
    bnd = zstandard.ZstdDecompressor().decompress(raw[0x4C:0x4C + csize], max_output_size=usize)
    return iv, raw[:0x4C], raw[0x4C + csize:], bnd


def pack(iv, head, bnd):
    """regulation.bin bytes: BND4 -> ZSTD DCX (Smithbox's settings) -> zero pad to 16 + a full PKCS7 block -> AES."""
    import zstandard
    params = zstandard.ZstdCompressionParameters.from_level(15, window_log=16, write_content_size=False,
                                                            write_checksum=False)
    body = zstandard.ZstdCompressor(compression_params=params).compress(bnd)
    head = bytearray(head)
    struct.pack_into(">II", head, 0x1C, len(bnd), len(body))
    assert head[0x24:0x2C] == b"DCP\0ZSTD", "unexpected DCX header"
    head[0x30] = 15  # the level byte as Smithbox writes it (FromSoftware's file says 21), matching this frame
    raw = bytes(head) + body
    raw += b"\0" * ((-len(raw)) % 16) + b"\x10" * 16
    return iv + _aes(iv, raw, True)


def bnd_files(bnd):
    """[(entry offset, name, data)] of the BND4 in entry order."""
    n, hs = struct.unpack_from("<i", bnd, 0x0C)[0], struct.unpack_from("<q", bnd, 0x20)[0]
    out = []
    for k in range(n):
        o = 0x40 + k * hs
        csize, usize = struct.unpack_from("<qq", bnd, o + 8)
        doff, _, noff = struct.unpack_from("<IiI", bnd, o + 24)
        assert csize == usize
        e = noff
        while bnd[e:e + 2] != b"\0\0":
            e += 2
        out.append((o, bnd[noff:e].decode("utf-16-le"), bnd[doff:doff + csize]))
    return out


def bnd_build(bnd, files):
    """The BND4 `bnd` with its files' data replaced by `files` (same order), laid out as the input."""
    entries = bnd_files(bnd)
    first = min(struct.unpack_from("<I", bnd, o + 24)[0] for o, _, _ in entries)
    out = bytearray(bnd[:first])
    for (o, _, _), data in zip(entries, files):
        out += b"\0" * ((-len(out)) % 16)
        struct.pack_into("<qq", out, o + 8, len(data), len(data))
        struct.pack_into("<I", out, o + 24, len(out))
        out += data
    return bytes(out)


def field_offset(fields, name):
    """Byte offset of whole field `name` in a row (er_params.read_row's layout)."""
    unit_code, unit_used, pos = None, 0, 0
    for fname, code, count, bits in fields:
        size = SIZES[code]
        if bits is not None:
            if unit_code != code or unit_used + bits > size * 8:
                unit_code, unit_used = code, 0
                pos += size
            unit_used += bits
            continue
        unit_code = None
        if fname == name:
            return pos
        pos += size * count
    raise KeyError(name)


class Param:
    """A PARAM file's rows {id: (data bytes, name offset)} and the bytes around them."""

    def __init__(self, p):
        n = struct.unpack_from("<H", p, 0x0A)[0]
        table = [struct.unpack_from("<iiqq", p, 0x40 + 24 * i) for i in range(n)]
        ids = [t[0] for t in table]
        assert ids == sorted(ids) and len(set(ids)) == n, "rows not sorted / duplicated"
        self.size = table[1][2] - table[0][2]
        assert all(t[2] == table[0][2] + i * self.size for i, t in enumerate(table)), "rows not in id order"
        assert struct.unpack_from("<I", p, 0x30)[0] == 0x40 + 24 * n == table[0][2]
        self.head, self.pad = p[:0x40], [t[1] for t in table]
        self.data_end = table[0][2] + n * self.size
        self.strings = p[self.data_end:]
        self.rows = {t[0]: [p[t[2]:t[2] + self.size], t[3]] for t in table}
        self.original = p

    def copy_row(self, src, new):
        assert new not in self.rows, f"row {new} exists"
        self.rows[new] = [self.rows[src][0], self.rows[src][1]]

    def set_s32(self, row, offset, value):
        d = bytearray(self.rows[row][0])
        old = struct.unpack_from("<i", d, offset)[0]
        struct.pack_into("<i", d, offset, value)
        self.rows[row][0] = bytes(d)
        return old

    def empty_name(self):
        """A name offset whose row name is empty (rows without names share one), else the first row's."""
        for _, off in self.rows.values():
            at = off - self.data_end
            if 0 <= at < len(self.strings) and self.strings[at:at + 2] == b"\0\0":
                return off
        return self.rows[min(self.rows)][1]

    def build(self):
        ids = sorted(self.rows)
        n = len(ids)
        ds = 0x40 + 24 * n
        shift = ds + n * self.size - self.data_end  # how far the strings move
        head = bytearray(self.head)
        struct.pack_into("<H", head, 0x0A, n)
        for at in (0x00, 0x10, 0x30):
            v = struct.unpack_from("<I", head, at)[0]
            struct.pack_into("<I", head, at, ds if at == 0x30 else v + shift)
        out = bytearray(head)
        for k, i in enumerate(ids):
            out += struct.pack("<iiqq", i, 0, ds + k * self.size, self.rows[i][1] + shift)
        for i in ids:
            out += self.rows[i][0]
        return bytes(out + self.strings)


def roundtrip(path):
    data = Path(path).read_bytes()
    iv, head, tail, bnd = unpack(data)
    files = [d for _, _, d in bnd_files(bnd)]
    files = [Param(d).build() if _is_sorted_param(d) else d for d in files]
    again = pack(iv, head, bnd_build(bnd, files))
    same = again == data
    # FromSoftware's own file (Game\regulation.bin, 1.17.3: zstd level 21 byte in the DCX header, 64 MB window, no
    # PKCS7 block) can't be recompressed byte for byte with python-zstandard; then the check is the content: the
    # rebuilt file must decrypt and decompress to exactly the input's BND4, with the same IV and DCX header fields.
    inner = unpack(again)
    equivalent = inner[3] == bnd and inner[0] == iv
    print(f"{path}: {len(data)} bytes, rebuilt {len(again)}, identical: {same}, "
          f"equivalent (same IV, BND4 byte-identical after {sum(map(_is_sorted_param, files))} params re-laid-out): "
          f"{equivalent}")
    return same or equivalent


def _is_sorted_param(d):
    try:
        Param(d)
        return True
    except (AssertionError, IndexError, struct.error):
        return False


# Round 33 (U6b): the street mobs' NpcParam rows (msb_add_mobs.MOBS) are vanilla rows that 54 vanilla enemies in
# 9 other maps use too (scan of Ryan's 1347 mapstudio MSBs: 41800010 x32, 34500012 x15, 35004010 x7), so the
# street gets its own copies without the enemy drop, and the drop becomes a map lot the street's emevd awards on
# death (2003[4] Award Item Lot takes ItemLotParam_map rows: all 2003[4] lots in our emevds are map rows).
# (vanilla NpcParam, new NpcParam, vanilla ItemLotParam_enemy chain start, new ItemLotParam_map chain start)
U6B = (
    (41800010, 41800090, 418000010, 1042368600),  # Lost Soul (Spirit Jellyfish c4180)
    (35004010, 35004090, 350000400, 1042368610),  # Skeletal Pirate (Skeleton c3500)
    (34500012, 34500092, 345000000, 1042368620),  # Dark Sprite (Winged Misbegotten c3450)
)


def u6b(path, defs, out):
    data = Path(path).read_bytes()
    iv, head, tail, bnd = unpack(data)
    entries = bnd_files(bnd)
    name_of = {nm.rsplit("\\", 1)[-1].removesuffix(".param"): k for k, (_, nm, _) in enumerate(entries)}
    files = [d for _, _, d in entries]
    npc, enemy_lots, map_lots = (Param(files[name_of[n]]) for n in ("NpcParam", "ItemLotParam_enemy",
                                                                       "ItemLotParam_map"))
    lot_at = field_offset(layout(Path(defs) / "NpcParam.xml"), "itemLotId_enemy")
    for old, new, lot, map_lot in U6B:
        npc.copy_row(old, new)
        was = npc.set_s32(new, lot_at, -1)
        assert was == lot, f"NpcParam {old} itemLotId_enemy {was} != {lot}"
        print(f"NpcParam {new} = copy of {old}, itemLotId_enemy {was} -> -1")
        k = 0
        while lot + k in enemy_lots.rows:
            assert map_lot + k not in map_lots.rows, f"ItemLotParam_map {map_lot + k} exists"
            assert enemy_lots.size == map_lots.size
            map_lots.rows[map_lot + k] = [enemy_lots.rows[lot + k][0], map_lots.empty_name()]
            print(f"ItemLotParam_map {map_lot + k} = copy of ItemLotParam_enemy {lot + k}")
            k += 1
        assert map_lot + k not in map_lots.rows, f"ItemLotParam_map {map_lot + k} would extend the chain"
    files[name_of["NpcParam"]] = npc.build()
    files[name_of["ItemLotParam_map"]] = map_lots.build()
    result = pack(iv, head, bnd_build(bnd, files))
    Path(out).write_bytes(result)
    _check(result, data, defs)
    print(f"wrote {out}: {len(result)} bytes")


def _check(result, data, defs):
    """Re-read the output: same files as the input except the two params, and every changed param sorted."""
    a, b = bnd_files(unpack(data)[3]), bnd_files(unpack(result)[3])
    assert [x[1] for x in a] == [x[1] for x in b]
    changed = [x[1].rsplit("\\", 1)[-1] for x, y in zip(a, b) if x[2] != y[2]]
    assert changed == ["ItemLotParam_map.param", "NpcParam.param"], changed
    for (_, nm, d) in b:
        if nm.endswith(("NpcParam.param", "ItemLotParam_map.param")):
            Param(d)
    print("check: only", ", ".join(changed), "changed; both sorted")


if __name__ == "__main__":
    if sys.argv[1] == "roundtrip":
        sys.exit(0 if roundtrip(sys.argv[2]) else 1)
    if sys.argv[1] == "u6b":
        u6b(*sys.argv[2:5])
