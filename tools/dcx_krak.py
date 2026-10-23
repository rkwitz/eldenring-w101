"""Read DCX files (DFLT or KRAK) without the game's Oodle DLL, using pyooz (`pip install pyooz`).

pyooz's decompress fails on any KRAK stream longer than one 256 KB block ("Could not decompress requested amount").
FromSoftware's streams start every 256 KB block with a header whose restart flag (0x80) is set, which means the
block is coded on its own (its LZ offsets start at the block, not the stream), and ooz gets that wrong. Decoding the
stream one block at a time, each as a separate stream, is exact (checked byte for byte against the full decode of
m60_42_36_00.nvmhktbnd.dcx, 3.9 MB).

Block layout: 2-byte block header (low nibble 0xC; 0x40 = stored uncompressed; second byte & 0x7F = codec, 6 =
Kraken, & 0x80 = checksums), then for compressed blocks a 3-byte big-endian quantum header whose low 18 bits are the
compressed size - 1 (3 more checksum bytes if checksums are on).

Usage: python dcx_krak.py in.dcx out.bin
"""

import struct
import sys
import zlib

BLOCK = 0x40000


def kraken(stream, size):
    import ooz
    out = bytearray()
    pos = 0
    while len(out) < size:
        n = min(BLOCK, size - len(out))
        head, codec = stream[pos], stream[pos + 1]
        assert head & 0x0F == 0x0C and head & 0x30 == 0, f"bad block header {head:#x} at {pos}"
        if head & 0x40:
            length = 2 + n
        else:
            quantum = int.from_bytes(stream[pos + 2:pos + 5], "big") & 0x3FFFF
            assert quantum != 0x3FFFF, "special (memset / whole-match) quantum not handled"
            length = 5 + quantum + 1 + (3 if codec & 0x80 else 0)
        out += ooz.decompress(bytes(stream[pos:pos + length]), n)
        pos += length
    return bytes(out)


def decompress(data):
    """Raw bytes of a DCX file (or the bytes themselves if not DCX)."""
    if data[:4] != b"DCX\0":
        return data
    usize, csize = struct.unpack_from(">II", data, 0x1C)
    kind = data[0x28:0x2C]
    body = data[0x4C:0x4C + csize]
    if kind == b"DFLT":
        out = zlib.decompress(body)
    elif kind == b"KRAK":
        out = kraken(body, usize)
    else:
        raise ValueError(f"unsupported DCX type {kind}")
    assert len(out) == usize
    return out


if __name__ == "__main__":
    with open(sys.argv[1], "rb") as f:
        raw = decompress(f.read())
    with open(sys.argv[2], "wb") as f:
        f.write(raw)
