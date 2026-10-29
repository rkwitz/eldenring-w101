"""The player's idle pose for the hud_orbs character picture, read from Elden Ring's own animation (stdlib only).

Usage: python er_idle_pose.py <Game\\chr> <out.tsv> [animation, default a000_000000] [regulation.bin <paramdef Defs>]
  reads c0000.anibnd.dcx (Skeleton.hkx) and c0000_a00_lo.anibnd.dcx (the animation and its type compendium), takes
  the animation's first frame and writes each bone's world transform in the skeleton's model space:
  `name \\t parent \\t m00 m01 m02 m10 m11 m12 m20 m21 m22 \\t tx ty tz` (row-vector rotation: p' = p * M + t),
  first the reference pose (lines starting "R"), then the animated pose ("A").
  With regulation.bin and Smithbox's ER paramdef folder it also writes how each weapon model sits in the right hand,
  as Elden Ring attaches it: EquipParamWeapon absorpParamId -> WepAbsorpPosParam right_0 = a dummy poly of
  c0000.chrbnd's c0000.flver on R_Weapon; `H \t <model> \t <dummy> \t fx fy fz \t ux uy uz \t px py pz`
  (forward, upward and position in the bind pose; the weapon model's +Y along upward, +Z against forward).
The output is Elden Ring data: deploy it to w101_orbs\\book\\er\\idle_pose.tsv, never commit it.

The animation is an hkaSplineCompressedAnimation (Havok 2018 tagfile whose types live in the anibnd's compendium).
Frame 0 of a clamped B-spline is its first control point, so only the first control point of each track is decoded.
"""

import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from dcx_krak import decompress  # noqa: E402
from er_text import bnd4  # noqa: E402
from hkx_tag import TagFile, _sections  # noqa: E402


def read_tag(data, compendium=None):
    """A tagfile's root; its types come from `compendium` (TCM0) when the file has no TYPE section of its own."""
    t = TagFile.__new__(TagFile)
    sub = {n: (s, z) for n, s, z in _sections(data, 8, len(data))}
    t.data_start = sub["DATA"][0] + 8
    t.data = data[t.data_start:sub["DATA"][0] + sub["DATA"][1]]
    if "TYPE" in sub:
        t.raw = data
        ts, tz = sub["TYPE"]
    else:
        t.raw = compendium
        ts, tz = {n: (s, z) for n, s, z in _sections(compendium, 8, len(compendium))}["TYPE"]
    t._read_types({n: (s, z) for n, s, z in _sections(t.raw, ts + 8, ts + tz)})
    t.raw = data
    t._read_index({n: (s, z) for n, s, z in _sections(data, sub["INDX"][0] + 8, sub["INDX"][0] + sub["INDX"][1])})
    return t.root()


ROT_ALIGN = {0: 4, 1: 1, 2: 2, 3: 1, 4: 2, 5: 4}
ROT_SIZE = {0: 4, 1: 5, 2: 6, 3: 3, 4: 2, 5: 16}


def quat(b, pos, kind):
    """One quantized quaternion (x, y, z, w)."""
    if kind == 5:
        return list(struct.unpack_from("<4f", b, pos))
    if kind == 1:  # THREECOMP40: three 12-bit components, the dropped one's index, its sign
        v = int.from_bytes(b[pos:pos + 5], "little")
        mask, half, frac = 0xFFF, 0x7FF, 0.000345436
        c = [((v >> (12 * i)) & mask) for i in range(3)]
        shift, invert = (v >> 36) & 3, (v >> 38) & 1
    elif kind == 2:  # THREECOMP48
        a = struct.unpack_from("<3H", b, pos)
        mask, half, frac = 0x7FFF, 0x3FFF, 0.000043161
        c = [x & mask for x in a]
        shift = ((a[1] >> 14) & 2) | ((a[0] >> 15) & 1)
        invert = (a[2] >> 15) & 1
    else:
        raise ValueError(f"rotation quantization {kind} not handled")
    xyz = [(x - half) * frac for x in c]
    w = math.sqrt(max(0.0, 1.0 - sum(x * x for x in xyz)))
    if invert:
        w = -w
    out, k = [], 0
    for i in range(4):
        if i == shift:
            out.append(w)
        else:
            out.append(xyz[k])
            k += 1
    return out


def first_frame(anim):
    """Per transform track: (translation, rotation, scale) at frame 0."""
    b = bytes(anim["data"])
    n = anim["numberOfTransformTracks"]
    start = anim["blockOffsets"][0]
    masks = [b[start + 4 * i:start + 4 * i + 4] for i in range(n)]
    pos = start + anim["maskAndQuantizationSize"]

    def align(a):
        nonlocal pos
        pos = (pos + a - 1) // a * a

    def vector(types, quant, default):
        nonlocal pos
        value = list(default)
        if types & 0x70:
            items, degree = struct.unpack_from("<HB", b, pos)
            pos += 3 + items + degree + 2
            align(4)
            ranges = {}
            for axis in range(3):
                if types & (0x10 << axis):
                    ranges[axis] = struct.unpack_from("<2f", b, pos)
                    pos += 8
                elif types & (1 << axis):
                    value[axis] = struct.unpack_from("<f", b, pos)[0]
                    pos += 4
            size = 1 if quant == 0 else 2
            for item in range(items + 1):
                for axis in range(3):
                    if axis in ranges:
                        q = b[pos] / 255 if size == 1 else struct.unpack_from("<H", b, pos)[0] / 65535
                        if item == 0:
                            lo, hi = ranges[axis]
                            value[axis] = lo + (hi - lo) * q
                        pos += size
        else:
            for axis in range(3):
                if types & (1 << axis):
                    value[axis] = struct.unpack_from("<f", b, pos)[0]
                    pos += 4
        align(4)
        return value

    out = []
    for m in masks:
        qt, pt, rt, st = m
        t = vector(pt, qt & 3, (0.0, 0.0, 0.0))
        rq = (qt >> 2) & 0xF
        r = [0.0, 0.0, 0.0, 1.0]
        if rt & 0xF0:
            items, degree = struct.unpack_from("<HB", b, pos)
            pos += 3 + items + degree + 2
            align(ROT_ALIGN[rq])
            r = quat(b, pos, rq)
            pos += ROT_SIZE[rq] * (items + 1)
        elif rt & 0x0F:
            align(ROT_ALIGN[rq])
            r = quat(b, pos, rq)
            pos += ROT_SIZE[rq]
        align(4)
        s = vector(st, (qt >> 6) & 3, (1.0, 1.0, 1.0))
        out.append((t, r, s))
    return out


def matrix(q):
    """Row-vector rotation matrix of a unit quaternion (x, y, z, w): p' = p * M."""
    x, y, z, w = q
    n = math.sqrt(x * x + y * y + z * z + w * w) or 1.0
    x, y, z, w = x / n, y / n, z / n, w / n
    col = [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
           [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
           [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]]
    return [[col[j][i] for j in range(3)] for i in range(3)]


def mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def apply(v, m):
    return [sum(v[k] * m[k][j] for k in range(3)) for j in range(3)]


def world(parents, local):
    """World (row-vector) transforms from parent-relative (translation, rotation, scale)."""
    out = [None] * len(parents)

    def get(i):
        if out[i] is None:
            t, r, s = local[i]
            m = mul([[s[0], 0, 0], [0, s[1], 0], [0, 0, s[2]]], matrix(r))
            if parents[i] >= 0:
                pm, pt = get(parents[i])
                out[i] = (mul(m, pm), [a + b for a, b in zip(apply(t[:3], pm), pt)])
            else:
                out[i] = (m, list(t[:3]))
        return out[i]

    for i in range(len(parents)):
        get(i)
    return out


def main():
    chr_dir, out = Path(sys.argv[1]), Path(sys.argv[2])
    name = sys.argv[3] if len(sys.argv) > 3 else "a000_000000"
    base = bnd4(decompress((chr_dir / "c0000.anibnd.dcx").read_bytes()))
    skel_root = read_tag(next(v for k, v in base.items() if k.endswith("Skeleton.hkx")))
    skel = skel_root["namedVariants"][0]["variant"]["skeletons"][0]
    names = [b["name"] for b in skel["bones"]]
    parents = list(skel["parentIndices"])
    ref = [(p["translation"], p["rotation"], p["scale"]) for p in skel["referencePose"]]
    # The animation's binder: c0000_a00_lo for the plain idle, else whichever c0000_aNx holds it.
    for path in [chr_dir / "c0000_a00_lo.anibnd.dcx"] + sorted(chr_dir.glob("c0000_a*x.anibnd.dcx")):
        lo = bnd4(decompress(path.read_bytes()))
        if any(k.endswith(name + ".hkx") for k in lo):
            break
    else:
        sys.exit(f"{name}.hkx is in no c0000 anibnd")
    compendium = next(v for k, v in lo.items() if k.endswith(".compendium"))
    root = read_tag(next(v for k, v in lo.items() if k.endswith(name + ".hkx")), compendium)
    container = root["namedVariants"][0]["variant"]
    anim = container["animations"][0]
    tracks = container["bindings"][0]["transformTrackToBoneIndices"]
    posed = list(ref)
    for track, (t, r, s) in enumerate(first_frame(anim)):
        posed[tracks[track]] = (t, r, s)
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        for tag, pose in (("R", world(parents, ref)), ("A", world(parents, posed))):
            for i, (m, t) in enumerate(pose):
                f.write("\t".join([tag, names[i], str(parents[i]), " ".join(f"{v:.5f}" for row in m for v in row),
                                   " ".join(f"{v:.5f}" for v in t)]) + "\n")
        if len(sys.argv) > 5:
            holds = weapon_holds(chr_dir, Path(sys.argv[4]), Path(sys.argv[5]))
            for model, (ref, fwd, up, pos) in sorted(holds.items()):
                f.write("\t".join(["H", str(model), str(ref)] + [" ".join(f"{v:.5f}" for v in vec)
                                                                  for vec in (fwd, up, pos)]) + "\n")
            print(f"{len(holds)} weapon models' hand holds")
    print(f"{out}: {len(names)} bones, {anim['numberOfTransformTracks']} tracks of {name}")


def flver_dummies(flver):
    """{reference id: (attach bone name, forward, upward, position)} of a FLVER2's dummy polys."""
    assert flver[:6] == b"FLVER\0"
    dummies, materials, bone_count = struct.unpack_from("<iii", flver, 0x14)
    unicode = flver[0x2E]

    def text(at):
        if unicode:
            end = at
            while flver[end:end + 2] != b"\0\0":
                end += 2
            return flver[at:end].decode("utf-16le")
        return flver[at:flver.index(b"\0", at)].decode("shift_jis", "replace")
    bones_at = 0x80 + dummies * 0x40 + materials * 0x20
    bones = [text(struct.unpack_from("<i", flver, bones_at + i * 0x80 + 12)[0]) for i in range(bone_count)]
    out = {}
    for i in range(dummies):
        at = 0x80 + i * 0x40
        pos = struct.unpack_from("<3f", flver, at)
        fwd = struct.unpack_from("<3f", flver, at + 16)
        ref, _parent = struct.unpack_from("<hh", flver, at + 28)
        up = struct.unpack_from("<3f", flver, at + 32)
        attach = struct.unpack_from("<h", flver, at + 44)[0]
        name = bones[attach] if 0 <= attach < bone_count else ""
        out.setdefault((ref, name), (name, fwd, up, pos))
    return out


def weapon_holds(chr_dir, regulation, defs):
    """{weapon model id: (dummy id, forward, upward, position)} for the right hand, one-handed (right_0)."""
    from er_params import Regulation
    chrbnd = bnd4(decompress((chr_dir / "c0000.chrbnd.dcx").read_bytes()))
    dummies = flver_dummies(next(v for k, v in chrbnd.items() if k.lower().endswith(".flver")))
    reg = Regulation(str(regulation), str(defs))
    absorb = reg.param("WepAbsorpPosParam")
    holds = {}
    for row in reg.param("EquipParamWeapon").values():
        model, pos_id = row["equipModelId"], row["absorpParamId"]
        ref = absorb.get(pos_id, {}).get("right_0", -1)
        d = dummies.get((ref, "R_Weapon"))
        if model > 0 and d and model not in holds:
            holds[model] = (ref, d[1], d[2], d[3])
    return holds


if __name__ == "__main__":
    main()
