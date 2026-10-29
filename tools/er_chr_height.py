"""How tall Elden Ring characters stand in their idle pose, for the hud_orbs nameplates (stdlib only).

Usage: python er_chr_height.py <Game\\chr> c4960 c3450 ... [--anim a000_000000]
  For each character: the skeleton (cXXXX.anibnd.dcx Skeleton.hkx), the first frame of its idle animation (a000_000000, else a000_000020; searched
  in cXXXX.anibnd.dcx and cXXXX_divNN.anibnd.dcx), and the FLVER's bounding box (cXXXX.chrbnd.dcx). Prints, in
  metres above the character's root (its feet / physics position): the head bone's height, the highest bone, and
  the bind-pose mesh box top. Elden Ring data is only printed, never written to the repository.
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from dcx_krak import decompress  # noqa: E402
from er_idle_pose import first_frame, read_tag, world  # noqa: E402
from er_text import bnd4  # noqa: E402


def idle_pose(chr_dir, chr_id, anim_name):
    base = bnd4(decompress((chr_dir / f"{chr_id}.anibnd.dcx").read_bytes()))
    skel_data = next((v for k, v in base.items() if k.lower().endswith("skeleton.hkx")), None)
    binders = [base] + [bnd4(decompress(p.read_bytes())) for p in sorted(chr_dir.glob(f"{chr_id}_div*.anibnd.dcx"))]
    compendium = None
    for b in binders:
        compendium = next((v for k, v in b.items() if k.endswith(".compendium")), compendium)
    skel_root = read_tag(skel_data, compendium)
    skel = skel_root["namedVariants"][0]["variant"]["skeletons"][0]
    names = [b["name"] for b in skel["bones"]]
    parents = list(skel["parentIndices"])
    ref = [(p["translation"], p["rotation"], p["scale"]) for p in skel["referencePose"]]
    posed, used = list(ref), "reference pose"
    # The idle (a000_000000); characters without one stand in a000_000020 at rest.
    names_tried = [anim_name, "a000_000020"]
    found = [(b, n) for n in names_tried for b in binders if any(k.endswith(n + ".hkx") for k in b)]
    if found:
        anim_name = found[0][1]
    for b in binders:
        comp = next((v for k, v in b.items() if k.endswith(".compendium")), compendium)
        hit = next((v for k, v in b.items() if k.endswith(anim_name + ".hkx")), None)
        if hit is None:
            continue
        container = read_tag(hit, comp)["namedVariants"][0]["variant"]
        anim = container["animations"][0]
        tracks = container["bindings"][0]["transformTrackToBoneIndices"]
        for track, trs in enumerate(first_frame(anim)):
            posed[tracks[track]] = trs
        used = anim_name
        break
    return names, world(parents, posed), world(parents, ref), used


def flver_box(chr_dir, chr_id):
    chrbnd = bnd4(decompress((chr_dir / f"{chr_id}.chrbnd.dcx").read_bytes()))
    flver = next(v for k, v in chrbnd.items() if k.lower().endswith(".flver"))
    lo = struct.unpack_from("<3f", flver, 0x28)
    hi = struct.unpack_from("<3f", flver, 0x34)
    return lo, hi


def main():
    chr_dir = Path(sys.argv[1])
    anim = sys.argv[sys.argv.index("--anim") + 1] if "--anim" in sys.argv else "a000_000000"
    ids = [a for a in sys.argv[2:] if a.startswith("c") and a[1:].isdigit()]
    for chr_id in ids:
        names, posed, ref, used = idle_pose(chr_dir, chr_id, anim)
        heads = [(n, posed[i][1][1], ref[i][1][1]) for i, n in enumerate(names) if n.lower() in ("head", "neck")]
        top = max(range(len(names)), key=lambda i: posed[i][1][1])
        lo, hi = flver_box(chr_dir, chr_id)
        print(f"{chr_id} ({used}): head/neck {[(n, round(a, 2), round(r, 2)) for n, a, r in heads]} "
              f"highest bone {names[top]} {posed[top][1][1]:.2f}, mesh box y {lo[1]:.2f}..{hi[1]:.2f}")


if __name__ == "__main__":
    main()
