"""Round 26 (W2): Wizard101's window states on Unicorn Way's buildings, for the street asset.

Wizard101's classic street (WC_Z06_Unicorn_Way.nif) has, for every building with a door, three copies of its
windows under nodes WC_Unicorn_<building>_off / _on / _quest (T1, T2, H01..H13, Boss01): the same triangles with
  _off    WC_Window01_JT, its dark part (dark yellow fading to black)
  _on     WC_Window01_JT, its lit part (white / yellow)
  _quest  WC_Window03_JT, the blue copy, self-lit (the building holds the current quest goal)
and the client shows one of them (w101_quest_window.py). nif2obj.py exported all three on top of each other, so the
street has drawn whichever won the depth test. blender_switch_street.py now keeps one per building with this
module: _on for the buildings the player can enter here (ENTERABLE: the doors with warps, msb_add_room.py /
msb_add_interiors.py), _off for the rest, and no _quest copies (the blue windows are their own asset aeg030_529,
which emevd.py --quest-window switches on while the tower holds the current goal).

Step 1 (container or any PC with pyffi, like w101_quest_window.py):
    python w101_window_states.py WC_Z06_Unicorn_Way.nif window_states.tsv
  writes one line per window triangle: building, state, centroid x y z (the street OBJ's axes, nif2obj's
  (x, z, -y) / 100) and UV centroid u v (OBJ convention, v flipped). Wizard101 data: keep it next to the converted
  street (reference/w101/converted_classic/window_states.tsv), never in the repository.
Step 2 (blender_switch_street.py, no pyffi): `window_filter(positions, uvs, faces, rows)` -> which faces to drop.
"""
import math
import re
import sys
from pathlib import Path

# The doors with warps here, matched by position (the nif's window centroids vs the door triggers in the street FLVER
# frame, OBJ z = -FLVER z): T1 Lady Blackhope's tower (door -105.1, 19.2), T2 Rattlebones' tower (-164.9, 191.8),
# H05 Fear of Fire's house (msb_add_interiors H4_DOOR -127.2, 149.3; windows centred -127.2, 152.1), Boss01 the
# hedge-maze gate (msb_add_garden STREET_DOOR -295.9, 195.0; windows -296.5, 195.0).
ENTERABLE = {"T1", "T2", "H05", "Boss01"}
NODE = re.compile(r"WC_Unicorn_(\w+?)_(off|on|quest)$")
MATCH = 0.02  # m: a street face is a building's window when its centroid is this close to one of the nif's


def extract(nif_path):
    sys.path.insert(0, str(Path(__file__).parent))
    import nif2obj
    from nif2obj import NifFormat, apply, tri_shape, walk
    from w101_quest_window import _parent_matrix
    data = NifFormat.Data()
    with open(nif_path, "rb") as f:
        data.read(f)
    identity = [[1.0 if i == j else 0.0 for j in range(4)] for i in range(4)]
    rows = []
    for root in data.roots:
        for block in root.tree():
            m = NODE.match(getattr(block, "name", b"").decode(errors="replace"))
            if not m:
                continue
            meshes = []
            walk(block, _parent_matrix(root, block, identity), meshes)
            for mesh, matrix in meshes:
                pos, idx, uv, _ = tri_shape(mesh)
                world = [tuple(c * 0.01 for c in apply(matrix, p)) for p in pos]
                flat = [i[0] for i in idx]
                for tri in zip(flat[0::3], flat[1::3], flat[2::3]):
                    c = [sum(world[k][i] for k in tri) / 3 for i in range(3)]
                    u = sum(uv[k][0] for k in tri) / 3
                    v = sum(uv[k][1] for k in tri) / 3
                    rows.append((m.group(1), m.group(2), c[0], c[2], -c[1], u, 1.0 - v))
    return rows


def load(path):
    rows = []
    for line in Path(path).read_text().splitlines():
        b, state, *nums = line.split("\t")
        rows.append((b, state, *map(float, nums)))
    return rows


def window_filter(positions, uvs, faces, rows, enterable=ENTERABLE):
    """Indices of `faces` ((material, corners) as flver_build.parse_obj) to leave out, and a count per reason. A
    face matching a building's window (centroid within MATCH) takes the state whose triangle there has the nearest
    UV centroid; it stays only if that state is the building's wanted one (_on if enterable, else _off)."""
    grid = {}
    for row in rows:
        grid.setdefault(tuple(round(c, 1) for c in row[2:5]), []).append(row)
    drop, counts = set(), {}
    for k, (material, corners) in enumerate(faces):
        if not material or "window0" not in material.lower():
            continue
        c = [sum(positions[v][i] for v, _ in corners) / 3 for i in range(3)]
        here = [r for r in grid.get(tuple(round(x, 1) for x in c), []) if math.dist(r[2:5], c) < MATCH]
        if not here:
            continue
        quest = "window03" in material.lower()
        here = [r for r in here if (r[1] == "quest") == quest] or here
        if all(t is not None for _, t in corners):
            u = sum(uvs[t][0] for _, t in corners) / 3
            v = sum(uvs[t][1] for _, t in corners) / 3
            row = min(here, key=lambda r: math.hypot(r[5] - u, r[6] - v))
        else:
            row = here[0]
        building, state = row[0], row[1]
        if quest:
            state = "quest"
        wanted = "on" if building in enterable else "off"
        key = f"{building} {state}"
        counts[key] = counts.get(key, 0) + 1
        if state != wanted:
            drop.add(k)
    return drop, counts


if __name__ == "__main__":
    out = extract(sys.argv[1])
    Path(sys.argv[2]).write_text("".join("\t".join([b, s] + [f"{x:.5f}" for x in n]) + "\n" for b, s, *n in out))
    print(f"{len(out)} window triangles of {len({r[0] for r in out})} buildings -> {sys.argv[2]}")
