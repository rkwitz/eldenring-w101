"""Record every change our map files make to the vanilla Elden Ring maps as an entry-level patch, and apply it again
(round 35, R-DOC). For build_map.py.

Why a patch and not only the msb_* scripts: the deployed maps were built over 35 rounds by those scripts *and* by
Ryan's own Smithbox edits in between: the street part placed by hand, the First Step grace moved, terrain m423600
lowered 6.4 m, rocks deleted. The Blender footprint step and Smithbox saves also came in between. Their order and the
inputs of the time are not recorded, so the chain alone cannot rebuild the maps. The patch is that whole result,
written as edits to vanilla.

An MSB is six lists of self-contained entries (msb_floor.read_msb). For each list of the target, the patch gives:

- the list's entries in order. Each entry is one of these:
  - "v": a vanilla entry (by index) with byte patches;
  - "s": an earlier entry of the target (by list and index, e.g. a copied enemy) with byte patches;
  - "l": a literal entry, our own parts, used when no base comes within half its size.
- the list's name and version, when they differ from vanilla.

The byte patches are runs of (offset, new bytes). Vanilla entries the target drops are simply not listed. The output
is DFLT DCX exactly as Smithbox saves it (zlib level 9). The vanilla input may be the game's KRAK .dcx (pyooz) or
uncompressed. The patch is gzip JSON and holds our numbers and names. Whether it may be published is Ryan's call.

Usage:
  python msb_patch.py make  VANILLA.msb[.dcx] TARGET.msb.dcx PATCH.msbpatch.gz
  python msb_patch.py apply VANILLA.msb[.dcx] PATCH.msbpatch.gz OUT.msb.dcx
"""
import gzip
import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import msb_floor  # noqa: E402

# DFLT DCX header as Smithbox writes it; sizes at 0x1C (uncompressed) and 0x20 (compressed), big-endian.
DFLT_HEADER = bytes.fromhex("44435800000100000000001800000024000000440000004c44435300000000000000000044435000"
                            "44464c540000002009000000000000000000000000000000000101004443410000000008")


def read(path):
    data = Path(path).read_bytes()
    if data[:4] != b"DCX\0":
        return data
    if data[0x28:0x2C] == b"DFLT":
        return zlib.decompress(data[0x4C:])
    import dcx_krak
    return dcx_krak.decompress(data)


def dflt(raw):
    packed = zlib.compress(raw, 9)
    head = bytearray(DFLT_HEADER)
    struct.pack_into(">II", head, 0x1C, len(raw), len(packed))
    return bytes(head) + packed


def runs(base, new):
    """[(offset, hex)] that turn `base` (same length) into `new`; runs closer than 8 bytes are merged."""
    out, k, n = [], 0, len(new)
    while k < n:
        if base[k] == new[k]:
            k += 1
            continue
        start, end = k, k + 1
        while end < n:
            nxt = next((j for j in range(end, min(n, end + 8)) if base[j] != new[j]), None)
            if nxt is None:
                break
            end = nxt + 1
        out.append([start, new[start:end].hex()])
        k = end
    return out


def cost(base, new):
    return sum(len(h) // 2 + 4 for _, h in runs(base, new))


def make(vanilla_path, target_path, patch_path):
    vraw, traw = read(vanilla_path), read(target_path)
    vhead, vlists = msb_floor.read_msb(vraw)
    thead, tlists = msb_floor.read_msb(traw)
    assert len(vlists) == len(tlists)
    lists = []
    stats = {"v": 0, "s": 0, "l": 0, "same": 0}
    done = []  # (list index, entry index, bytes) of target entries already written, for "s" bases
    for li, (vl, tl) in enumerate(zip(vlists, tlists)):
        by_name = {}
        for vi, e in enumerate(vl.entries):
            by_name.setdefault(msb_floor.entry_name(e), []).append(vi)
        by_len = {}
        for vi, e in enumerate(vl.entries):
            by_len.setdefault(len(e), []).append(("v", vi, e))
        entries = []
        for ti, e in enumerate(tl.entries):
            name = msb_floor.entry_name(e)
            best = None
            for vi in by_name.get(name, []):
                b = vl.entries[vi]
                if len(b) == len(e):
                    c = cost(b, e)
                    if best is None or c < best[0]:
                        best = (c, "v", vi, b)
            if best is None or best[0] > 0:
                for kind, idx, b in by_len.get(len(e), [])[:400]:
                    c = cost(b, e)
                    if best is None or c < best[0]:
                        best = (c, kind, idx, b)
                for sli, sti, b in reversed(done[-400:]):
                    if len(b) == len(e):
                        c = cost(b, e)
                        if best is None or c < best[0]:
                            best = (c, "s", [sli, sti], b)
            if best is not None and best[0] <= len(e) // 2:
                c, kind, idx, b = best
                entries.append({kind: idx, "p": runs(b, e)})
                stats[kind] += 1
                stats["same"] += c == 0
            else:
                entries.append({"l": e.hex()})
                stats["l"] += 1
            done.append((li, ti, e))
        item = {"entries": entries}
        if tl.name != vl.name or tl.version != vl.version:
            item.update(name=tl.name.hex(), version=tl.version)
        lists.append(item)
    patch = {"objpatch": "msb 1", "vanilla_md5": hashlib.md5(vraw).hexdigest(),
             "target_md5": hashlib.md5(Path(target_path).read_bytes()).hexdigest(),
             "header": thead.hex() if thead != vhead else None, "lists": lists}
    with gzip.open(patch_path, "wt") as f:
        json.dump(patch, f, separators=(",", ":"))
    print(f"{patch_path}: {stats['v']} vanilla-based ({stats['same']} unchanged), {stats['s']} copies, "
          f"{stats['l']} literal; {Path(patch_path).stat().st_size} bytes")


def apply(vanilla_path, patch_path, out_path=None):
    with gzip.open(patch_path, "rt") as f:
        patch = json.load(f)
    vraw = read(vanilla_path)
    if hashlib.md5(vraw).hexdigest() != patch["vanilla_md5"]:
        raise SystemExit(f"{vanilla_path}: not the vanilla map this patch was made from (game version?)")
    vhead, vlists = msb_floor.read_msb(vraw)
    out_lists = []
    for li, (vl, item) in enumerate(zip(vlists, patch["lists"])):
        entries = []
        for op in item["entries"]:
            if "l" in op:
                entries.append(bytes.fromhex(op["l"]))
                continue
            base = vl.entries[op["v"]] if "v" in op else out_lists[op["s"][0]].entries[op["s"][1]] \
                if op["s"][0] < li else entries[op["s"][1]]
            e = bytearray(base)
            for off, h in op["p"]:
                b = bytes.fromhex(h)
                e[off:off + len(b)] = b
            entries.append(bytes(e))
        name = bytes.fromhex(item["name"]) if "name" in item else vl.name
        out_lists.append(msb_floor.ParamList(item.get("version", vl.version), name, entries))
    head = bytes.fromhex(patch["header"]) if patch["header"] else vhead
    out = dflt(msb_floor.write_msb(head, out_lists))
    ok = hashlib.md5(out).hexdigest() == patch["target_md5"]
    if out_path:
        Path(out_path).write_bytes(out)
        print(f"wrote {out_path} (md5 {hashlib.md5(out).hexdigest()}, {'= target' if ok else 'DIFFERS from target'})")
    return out, ok


if __name__ == "__main__":
    if len(sys.argv) != 5 or sys.argv[1] not in ("make", "apply"):
        raise SystemExit(__doc__)
    if sys.argv[1] == "make":
        make(*sys.argv[2:])
    else:
        sys.exit(0 if apply(*sys.argv[2:])[1] else 1)
