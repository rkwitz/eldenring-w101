"""Wizard101's page-turn transition (the loading screen peeling away like a book page) for the hud_orbs load screen.

Usage: python w101_page_flip.py <Wizard101 Data\\GameData, or a folder with the .nif and .kf> <out page_flip.tsv> [preview folder]

Wizard101 ends a zone load with PageFlip (WizardGraphicalClient GUI\\PageFlip.cpp, WizardGUIManager:
"StaticObjects/PageTransition/Page_Flip_Actor.kfm" ... "TransitionScreenRemoved"): the loading screen is drawn into
a render target that textures both sides of Root.wad StaticObjects/PageTransition/Page_Flip_Actor.nif
(Page_FrontSide, Page_BackSide: a 160 x 120 sheet, 94 vertices, skinned to a 16-bone chain that runs along the
diagonal from the top-left corner), and the actor plays its one sequence "PageFlip" (Page_Flip_Actor.kf, 1.0667 s,
33 B-spline control points per bone), curling the sheet from the corner and off the screen.

This samples that animation at 30 frames a second, skins the sheet and writes it for loadscreen.rs:
  `S <frames> <seconds>` ; `V u v` per vertex (texture coordinates, 0..1, v down) ;
  `T a b c` per triangle ; `F <frame>` then one `P x y z` per vertex: x, y in sheet units (-1..1 across and down
  the screen at rest), z towards the viewer in sheet half-heights.
It is Wizard101 data: deploy it to w101_orbs\\book\\page_flip.tsv, never commit it.

Needs pyffi (pip install pyffi) and numpy.
"""
import sys
import time
from io import BytesIO
from pathlib import Path

import numpy as np

if not hasattr(time, "clock"):  # pyffi 2.2.3 still calls time.clock
    time.clock = time.perf_counter
from pyffi.formats.nif import NifFormat  # noqa: E402

sys.path.insert(0, str(Path(__file__).parent))
from w101_hud_art import wad_files  # noqa: E402

NIF = "StaticObjects/PageTransition/Page_Flip_Actor.nif"
KF = "StaticObjects/PageTransition/Page_Flip_Actor.kf"
FPS = 30.0


def read_nif(data):
    nif = NifFormat.Data()
    stream = BytesIO(data)
    nif.inspect(stream)
    nif.read(stream)
    return nif


def qmat(w, x, y, z):
    """Column-vector rotation matrix of a Gamebryo quaternion."""
    return np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - w * z), 2 * (x * z + w * y)],
                     [2 * (x * y + w * z), 1 - 2 * (x * x + z * z), 2 * (y * z - w * x)],
                     [2 * (x * z - w * y), 2 * (y * z + w * x), 1 - 2 * (x * x + y * y)]])


def euler(x, y, z):
    cx, sx, cy, sy, cz, sz = np.cos(x), np.sin(x), np.cos(y), np.sin(y), np.cos(z), np.sin(z)
    rx = np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]])
    ry = np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]])
    rz = np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]])
    return rx @ ry @ rz


def nif_rotation(r):
    """A NIF Matrix33 (row-vector storage) as a column-vector matrix."""
    return np.array([[r.m_11, r.m_12, r.m_13], [r.m_21, r.m_22, r.m_23], [r.m_31, r.m_32, r.m_33]]).T


def affine(rot, t, s=1.0):
    m = np.eye(4)
    m[:3, :3] = rot * s
    m[:3, 3] = [t[0], t[1], t[2]]
    return m


def bspline(points, u):
    """Clamped uniform cubic B-spline (Gamebryo NiBSplineBasis<3>) through `points` at u in 0..1."""
    points = np.asarray(points, dtype=float)
    n = len(points)
    if n < 4:
        return points[min(int(u * (n - 1) + 0.5), n - 1)]
    knots = np.concatenate([[0.0] * 3, np.arange(n - 2, dtype=float), [float(n - 3)] * 3])
    t = min(max(u, 0.0), 1.0) * (n - 3)
    k = min(int(t) + 3, n - 1)
    d = [points[j + k - 3].copy() for j in range(4)]
    for r in range(1, 4):
        for j in range(3, r - 1, -1):
            i = j + k - 3
            denom = knots[i + 4 - r] - knots[i]
            a = 0.0 if denom == 0 else (t - knots[i]) / denom
            d[j] = (1 - a) * d[j - 1] + a * d[j]
    return d[3]


def keys_linear(keys, t):
    if not keys:
        return 0.0
    if t <= keys[0][0]:
        return keys[0][1]
    for (t0, v0), (t1, v1) in zip(keys, keys[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * (t - t0) / max(t1 - t0, 1e-9)
    return keys[-1][1]


def track(interp, node, duration):
    """A function t -> local 4x4 transform for one controlled block."""
    bind_r, bind_t = nif_rotation(node.rotation), node.translation.as_list()
    if isinstance(interp, NifFormat.NiBSplineCompTransformInterpolator):
        rots = list(interp.get_rotations()) if interp.rotation_offset != 65535 else None
        trans = list(interp.get_translations()) if interp.translation_offset != 65535 else None

        def at(t):
            u = (t - interp.start_time) / max(interp.stop_time - interp.start_time, 1e-9)
            r = bind_r
            if rots:
                q = bspline(rots, u)
                q = q / np.linalg.norm(q)
                r = qmat(*q)
            tr = bspline(trans, u) if trans else bind_t
            return affine(r, tr)
        return at
    data = interp.data
    if data is None:
        def static(_t):
            return affine(bind_r, bind_t)
        return static
    if data.rotation_type == 4:  # XYZ_ROTATION_KEY: three float key lists
        axes = [[(k.time, k.value) for k in data.xyz_rotations[a].keys] for a in range(3)]

        def at_euler(t):
            return affine(euler(*(keys_linear(axes[a], t) for a in range(3))), bind_t)
        return at_euler
    quats = [(k.time, k.value) for k in data.quaternion_keys]

    def at_quat(t):
        if not quats:
            return affine(bind_r, bind_t)
        q = min(quats, key=lambda k: abs(k[0] - t))[1]
        return affine(qmat(q.w, q.x, q.y, q.z), bind_t)
    return at_quat


def main():
    game_data, out = Path(sys.argv[1]), Path(sys.argv[2])
    preview = Path(sys.argv[3]) if len(sys.argv) > 3 else None
    if (game_data / "Root.wad").exists():
        found = wad_files(game_data / "Root.wad", {NIF, KF})
    else:  # a folder holding the two files already copied out of Root.wad
        found = {name: (game_data / Path(name).name).read_bytes() for name in (NIF, KF)}
    nif, kf = read_nif(found[NIF]), read_nif(found[KF])
    seq = kf.blocks[0]
    duration = seq.stop_time - seq.start_time
    nodes = {b.name: b for b in nif.blocks if isinstance(b, NifFormat.NiNode)}
    parent = {}
    for b in nodes.values():
        for c in b.children:
            if c is not None:
                parent[c.name] = b.name
    tracks = {}
    for cb in seq.controlled_blocks:
        name = cb.get_node_name()
        if name in nodes:
            tracks[name] = track(cb.interpolator, nodes[name], duration)
    shape = next(b for b in nif.blocks if isinstance(b, NifFormat.NiTriShape) and b.name == b"Page_FrontSide")
    data, skin = shape.data, shape.skin_instance
    verts = np.array([[v.x, v.y, v.z, 1.0] for v in data.vertices])
    uvs = [(uv.u, uv.v) for uv in data.uv_sets[0]]
    tris = [(t.v_1, t.v_2, t.v_3) for t in data.triangles]
    bones = [b.name for b in skin.bones]
    skin_to_bone = []
    weights = np.zeros((len(verts), len(bones)))
    for i, bd in enumerate(skin.data.bone_list):
        st = bd.skin_transform
        skin_to_bone.append(affine(nif_rotation(st.rotation), st.translation.as_list(), st.scale))
        for w in bd.vertex_weights:
            weights[w.index, i] = w.weight
    weights /= np.maximum(weights.sum(axis=1, keepdims=True), 1e-9)

    def world(name, t, cache):
        if name not in cache:
            node = nodes[name]
            local = tracks[name](t) if name in tracks else affine(nif_rotation(node.rotation),
                                                                    node.translation.as_list(), node.scale)
            cache[name] = (world(parent[name], t, cache) if name in parent else np.eye(4)) @ local
        return cache[name]

    frames = int(round(duration * FPS)) + 1
    # Rest sheet: x -80..80 (left to right), z 60..-60 (top to bottom), y towards the back. Screen units: x/80, -z/60,
    # and towards the viewer = -y/60.
    lines = [f"S\t{frames}\t{duration:.4f}"]
    lines += [f"V\t{u:.5f}\t{v:.5f}" for u, v in uvs]
    lines += [f"T\t{a}\t{b}\t{c}" for a, b, c in tris]
    for f in range(frames):
        t = min(f / FPS, duration)
        cache = {}
        mats = [world(b, t, cache) @ skin_to_bone[i] for i, b in enumerate(bones)]
        pos = np.zeros((len(verts), 3))
        for i, m in enumerate(mats):
            pos += weights[:, i:i + 1] * (verts @ m.T)[:, :3]
        lines.append(f"F\t{f}")
        lines += [f"P\t{p[0] / 80:.4f}\t{-p[2] / 60:.4f}\t{-p[1] / 60:.4f}" for p in pos]
        if preview is not None:
            draw_preview(preview / f"flip_{f:02}.png", pos, tris)
    out.write_text("\n".join(lines) + "\n", newline="\n")
    print(f"{out}: {frames} frames over {duration:.3f} s, {len(verts)} vertices, {len(tris)} triangles")


def draw_preview(path, pos, tris):
    from PIL import Image, ImageDraw
    path.parent.mkdir(parents=True, exist_ok=True)
    w, h = 480, 270
    img = Image.new("RGB", (w, h), (40, 60, 40))
    d = ImageDraw.Draw(img)
    proj = [((p[0] / 80) * w / 2 + w / 2, (-p[2] / 60) * h / 2 + h / 2) for p in pos]
    for a, b, c in sorted(tris, key=lambda t: -sum(pos[i][1] for i in t)):
        d.polygon([proj[a], proj[b], proj[c]], fill=(200, 180, 120), outline=(90, 60, 30))
    img.save(path)


if __name__ == "__main__":
    main()
