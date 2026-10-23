"""Cut one building's "quest inside" window glow out of a classic Wizard101 street .nif, as a small OBJ.

Every door on Unicorn Way's classic zone (WC_Z06_Unicorn_Way.nif) has three window meshes under nodes named after the
interior zone the door leads to, for example WC_Unicorn_T2 (Rattlebones' tower):
  WC_Unicorn_T2_off     shown by default (flags 16): the dark arched windows of WC_Window01_JT (u 0.14..0.27)
  WC_Unicorn_T2_on      hidden (flags 17): the same windows on the lit, yellow part of WC_Window01_JT (u 0..0.13)
  WC_Unicorn_T2_quest   hidden (flags 17): the same UVs on WC_Window03_JT, a blue copy of that texture, with the
                        texture also as the NiTexturingProperty glow map, so it is self-lit
The client shows the _quest node when the player has a quest goal inside (the HD zones do the same with
HD_WC_Door_A01_Quest_Window_Off / _Quest switch nodes). nif2obj.py exports all three, so the street asset has them on
top of each other.

This writes the chosen node's triangles (in the Rattlebones case 14 arched windows, 84 triangles, around the tower at
7.9..9.6 m and 17.7..19.4 m) as an OBJ centred on the windows, each triangle pushed PUSH m out from the tower's axis so
it draws in front of the street's own copies. The script prints the centre in the street FLVER frame (= W101
(x, z, y) * 0.01, msb_add_window.py's WINDOW_CENTRE).

Usage: python w101_quest_window.py WC_Z06_Unicorn_Way.nif WC_Unicorn_T2_quest out.obj   (needs pyffi, like nif2obj)
"""
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import nif2obj  # noqa: E402
from nif2obj import NifFormat, apply, tri_shape, walk  # noqa: E402

PUSH = 0.03  # m outward, enough to win the depth test against the coplanar street copies at 20+ m away


def quest_window(nif, node_name, scale=0.01):
    data = NifFormat.Data()
    with open(nif, "rb") as f:
        data.read(f)
    identity = [[1.0 if i == j else 0.0 for j in range(4)] for i in range(4)]
    meshes = []
    for root in data.roots:
        for block in root.tree():
            if getattr(block, "name", b"").decode(errors="replace") == node_name:
                parent = _parent_matrix(root, block, identity)
                walk(block, parent, meshes)
    assert meshes, f"no node {node_name!r}"
    triangles, texture = [], None
    for mesh, matrix in meshes:
        pos, idx, uv, _ = tri_shape(mesh)
        texture = texture or nif2obj.texture_name(mesh)
        world = [tuple(c * scale for c in apply(matrix, p)) for p in pos]  # W101 (x, y, z) in m, z up
        flat = [i[0] for i in idx]
        for a, b, c in zip(flat[0::3], flat[1::3], flat[2::3]):
            triangles.append([(world[k], uv[k]) for k in (a, b, c)])
    axis = [sum(p[i] for t in triangles for p, _ in t) / (3 * len(triangles)) for i in (0, 1)]
    pushed = []
    for t in triangles:
        cx, cy = (sum(p[i] for p, _ in t) / 3 - axis[i] for i in (0, 1))
        r = math.hypot(cx, cy) or 1.0
        pushed.append([((p[0] + PUSH * cx / r, p[1] + PUSH * cy / r, p[2]), q) for p, q in t])
    lo = [min(p[i] for t in pushed for p, _ in t) for i in range(3)]
    hi = [max(p[i] for t in pushed for p, _ in t) for i in range(3)]
    centre = [(lo[i] + hi[i]) / 2 for i in range(3)]
    centre[2] = lo[2]  # origin at the bottom of the lowest window
    return pushed, centre, Path(texture.replace("\\", "/")).stem


def _parent_matrix(node, target, matrix):
    """World matrix of `target`'s parent, searching down from `node` (whose parent matrix is `matrix`)."""
    m = nif2obj.mat_mul(nif2obj.local_matrix(node), matrix)
    for child in getattr(node, "children", []) or []:
        if child is target:
            return m
        if child is not None:
            found = _parent_matrix(child, target, m)
            if found is not None:
                return found
    return None


def write_obj(triangles, centre, texture, dst):
    dst = Path(dst)
    lines = [f"mtllib {dst.with_suffix('.mtl').name}", f"o {dst.stem}"]
    for t in triangles:
        for (x, y, z), _ in t:
            x, y, z = x - centre[0], y - centre[1], z - centre[2]
            lines.append(f"v {x:.5f} {z:.5f} {-y:.5f}")  # same axes as nif2obj: OBJ (x, z, -y)
    for t in triangles:
        for _, (u, v) in t:
            lines.append(f"vt {u:.5f} {1.0 - v:.5f}")
    lines.append(f"usemtl {texture}")
    for k in range(len(triangles)):
        a = 3 * k + 1
        lines.append(f"f {a}/{a} {a + 1}/{a + 1} {a + 2}/{a + 2}")
    dst.write_text("\n".join(lines) + "\n")
    dst.with_suffix(".mtl").write_text(f"newmtl {texture}\nmap_Kd textures/{texture}.dds\n")


if __name__ == "__main__":
    tris, c, tex = quest_window(sys.argv[1], sys.argv[2])
    write_obj(tris, c, tex, sys.argv[3])
    print(f"{len(tris)} triangles, texture {tex}; centre W101 {tuple(round(v * 100, 1) for v in c)}, "
          f"street FLVER frame {tuple(round(v, 3) for v in (c[0], c[2], c[1]))}")
