"""Record a hand edit of a converted Wizard101 OBJ as numbers against the converted OBJs, and apply it again (round 35).

Ryan's WC_Z06_Unicorn_Way_Classic_edited.obj is a Blender 5.1.2 export of his scene. That scene holds the classic street
from nif2obj.py with his deletions, the HD street (converted/WC_Z06_Unicorn_Way.obj) twice, and our invisible floor.
No line of it can be diffed against nif2obj's output: Blender renames, reorders and rewrites every number.

The patch keeps the edited file's structure and writes each vertex and texture coordinate as a reference to a
vertex of a source OBJ plus the difference in millionths (Blender's float noise, |d| <= ~50). A value that matches
no source within 0.01 is stored literally; that is our generated floor and Blender's own normals. Object and
material names and the face indices are stored as they are.

Normals (vn) are Blender's own. Nothing we build reads them: flver_build.parse_obj, street_graveyard_fix.py,
w101_street_sight.py and sky_bake.py use v / vt / f only. With --no-normals the patch leaves them out and writes
faces as v/vt. The applied OBJ is then not byte-identical, but every reader gets identical data. Without the flag
the applied OBJ is byte-identical to the edited one.

The patch is gzip text. Each source is checked by md5 (of its lines other than nif2obj's "#vc" comments) before applying. Personal use: whether a patch may be
published is Ryan's call (it holds no Wizard101 geometry, but it is derived from it).

Usage:
  python obj_patch.py make  EDITED.obj PATCH.objpatch.gz SOURCE.obj [SOURCE.obj ...] [--no-normals]
  python obj_patch.py apply PATCH.objpatch.gz OUT.obj SOURCE.obj [SOURCE.obj ...]
  python obj_patch.py check EDITED.obj OUT.obj        (byte-identical, else compares what parse_obj reads)
"""
import gzip
import hashlib
import sys
from pathlib import Path

import numpy as np
from scipy.spatial import cKDTree

TOLERANCE = 10_000  # micro-units (0.01): farther than this from every source vertex = stored literally


def micro(s):
    """Exact integer millionths of a decimal string with up to 6 decimals ('-0.000000' -> (0, True))."""
    neg = s.startswith("-")
    whole, _, frac = s.lstrip("+-").partition(".")
    assert len(frac) <= 6, s
    value = int(whole or "0") * 1_000_000 + int((frac + "000000")[:6] or "0")
    return (-value if neg else value), neg


def text(value, negzero=False):
    sign = "-" if value < 0 or (value == 0 and negzero) else ""
    a = abs(value)
    return f"{sign}{a // 1_000_000}.{a % 1_000_000:06d}"


def md5(path):
    return hashlib.md5(Path(path).read_bytes()).hexdigest()


def source_md5(path):
    """md5 of a source OBJ without nif2obj's "#vc" vertex-colour comment lines (added after Ryan's sources were made)."""
    h = hashlib.md5()
    with open(path, "rb") as f:
        for line in f:
            if not line.startswith(b"#vc "):
                # Windows line endings (an OBJ written by Python on Windows before nif2obj fixed them) hash as Unix.
                h.update(line.replace(b"\r\n", b"\n"))
    return h.hexdigest()


def source_values(path):
    """{tag: int array (n, k)} of a source OBJ's v and vt lines, in millionths."""
    vals = {"v": [], "vt": []}
    with open(path) as f:
        for line in f:
            tag = line.split(" ", 1)[0]
            if tag in vals:
                vals[tag].append([micro(x)[0] for x in line.split()[1:4 if tag == "v" else 3]])
    return {k: np.array(v, dtype=np.int64).reshape(-1, 3 if k == "v" else 2) for k, v in vals.items()}


def make(edited, patch, sources, normals=True):
    srcs = [source_values(p) for p in sources]
    with open(edited, newline="") as f:
        lines = f.read().splitlines(keepends=True)
    trees = {}
    for tag in ("v", "vt"):
        stack = [(si, s[tag]) for si, s in enumerate(srcs) if len(s[tag])]
        allv = np.concatenate([a for _, a in stack])
        owner = np.concatenate([np.full(len(a), si) for si, a in stack])
        local = np.concatenate([np.arange(len(a)) for _, a in stack])
        trees[tag] = (cKDTree(allv.astype(np.float64)), allv, owner, local)
    data = {"v": [], "vt": []}
    for line in lines:
        tag = line.split(" ", 1)[0]
        if tag in data:
            data[tag].append([micro(x) for x in line.split()[1:]])
    resolved = {}
    for tag, rows in data.items():
        vals = np.array([[m for m, _ in r] for r in rows], dtype=np.int64)
        tree, allv, owner, local = trees[tag]
        dist, idx = tree.query(vals.astype(np.float64))
        resolved[tag] = (vals, dist, idx, allv, owner, local)
    out = [f"objpatch 1 normals={int(normals)}\n"]
    out += [f"source {i} {source_md5(p)} {Path(p).name}\n" for i, p in enumerate(sources)]
    counters = {"v": 0, "vt": 0}
    last = {}
    stats = {"ref": 0, "literal": 0, "vn": 0}
    for line in lines:
        tag = line.split(" ", 1)[0]
        if tag in counters:
            k = counters[tag]
            counters[tag] += 1
            vals, dist, idx, allv, owner, local = resolved[tag]
            neg = "".join("n" if (m == 0 and z) else "." for m, z in data[tag][k])
            if dist[k] <= TOLERANCE:
                s, j = int(owner[idx[k]]), int(local[idx[k]])
                step = j - last.get((tag, s), -1)
                last[(tag, s)] = j
                d = vals[k] - allv[idx[k]]
                out.append(f"{tag} {s} {step} {' '.join(str(int(x)) for x in d)} {neg}\n")
                stats["ref"] += 1
            else:
                out.append(f"{tag}L {' '.join(str(int(x)) for x in vals[k])} {neg}\n")
                stats["literal"] += 1
        elif tag == "vn":
            stats["vn"] += 1
            if normals:
                out.append("=" + line)
        elif tag == "f" and not normals:
            out.append("=f " + " ".join("/".join(c.split("/")[:2]) for c in line.split()[1:]) + "\n")
        elif tag == "s" and not normals:
            continue
        else:
            out.append("=" + line)
    with gzip.open(patch, "wt", newline="") as f:
        f.writelines(out)
    print(f"{patch}: {stats['ref']} values by reference, {stats['literal']} literal, "
          f"{stats['vn'] if normals else 0} normals kept; {Path(patch).stat().st_size} bytes")


def apply(patch, out_path, sources):
    with gzip.open(patch, "rt", newline="") as f:
        lines = f.read().splitlines(keepends=True)
    assert lines[0].startswith("objpatch 1"), "not an obj patch"
    srcs = {}
    body = 1
    for line in lines[1:]:
        if not line.startswith("source "):
            break
        body += 1
        _, i, digest, name = line.split(maxsplit=3)
        path = sources[int(i)]
        if source_md5(path) != digest:
            raise SystemExit(f"source {i} ({name.strip()}): {path} has md5 {source_md5(path)} (without #vc lines), "
                             f"the patch needs {digest}")
        srcs[int(i)] = source_values(path)
    last = {}
    out = []
    for line in lines[body:]:
        if line.startswith("="):
            out.append(line[1:])
            continue
        parts = line.split()
        tag = parts[0]
        neg = parts[-1]
        if tag.endswith("L"):
            tag = tag[:-1]
            vals = [int(x) for x in parts[1:-1]]
        else:
            s, step = int(parts[1]), int(parts[2])
            j = last.get((tag, s), -1) + step
            last[(tag, s)] = j
            vals = [int(a) + int(d) for a, d in zip(srcs[s][tag][j], parts[3:-1])]
        out.append(f"{tag} " + " ".join(text(v, n == "n") for v, n in zip(vals, neg)) + "\n")
    Path(out_path).write_text("".join(out), newline="")
    print(f"wrote {out_path} (md5 {md5(out_path)})")


def check(edited, applied):
    if Path(edited).read_bytes() == Path(applied).read_bytes():
        print("byte-identical")
        return True
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import flver_build
    a, b = flver_build.parse_obj(edited), flver_build.parse_obj(applied)
    same = all(x == y for x, y in zip(a, b))
    print("not byte-identical; parse_obj (positions, uvs, faces):", "identical" if same else "DIFFERENT",
          [len(x) for x in a], [len(x) for x in b])
    return same


def main(argv):
    if len(argv) < 3:
        raise SystemExit(__doc__)
    mode, args = argv[1], [a for a in argv[2:] if not a.startswith("--")]
    if mode == "make":
        make(args[0], args[1], args[2:], normals="--no-normals" not in argv)
    elif mode == "apply":
        apply(args[0], args[1], args[2:])
    elif mode == "check":
        sys.exit(0 if check(args[0], args[1]) else 1)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
