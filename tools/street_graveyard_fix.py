"""Round 35 (G12c): move the Unicorn Way street's vertices out of the Stranded Graveyard's open space, into its walls.

The street (AEG030_518_9000) is placed with the graveyard (m18) door (AEG110_064_1000) in the Commons gatehouse, and
parts of the gatehouse reach behind the door plane into the graveyard. Ryan (door_clip3.png): "Can you just move these
vertexes back, like translate them so they're like inside of the Elden Ring wall so they're not visible?"

Everything here works in the door's frame: x across the door, y up from its floor, z out of the graveyard onto the
street (the door plane is z = 0). The graveyard's walls are the render meshes (LOD0) of the m18 assets around the
door (m18 parts between 60 and 100 m up: AEG020_980, AEG023_730..733 / 862 / 990, AEG024_151, AEG027_002 / 066 / 067
/ 068, AEG004_693; fog and invisible walls AEG092 / AEG099 left out), placed by m18_00_00_00.msb; the map's own
collision (h18) has nothing at that height.

A point is in the graveyard's open space when at least 4 of the 6 axis rays from it hit a wall from the room side
(and at most one from behind). For every point (x, y) across the door, the column toward the street (+z, from 30 m
behind the door up to the door plane) is split at each wall it crosses; front(x, y) is just past the end of its last
open stretch (0.12 m into the wall that ends it, never more than half that wall, checked to be out of the open), or
z = 0.02 when the open space reaches the door plane. Fronts are taken on a 0.1 m grid of
columns, each cell keeping the highest of itself and its 8 neighbours; a drawn street triangle behind the door needs
the highest front under it (sampled every 0.1 m), and every vertex then moves along +z (toward the street; x and y
stay) to at least the largest need of its triangles. So each triangle lies in
front of the open space at every point of it: nothing is cut, nothing is split, and neighbours stay joined.

Output (graveyard_fix.tsv, read by blender_switch_street.py), in OBJ coordinates, one line per moved vertex:
  v  x y z  x' y' z'

Usage (stdlib + numpy + pyooz, see dcx_krak.py):
  python street_graveyard_fix.py EDITED.obj GAME_DIR M18_MSB OUT.tsv
GAME_DIR is ELDEN RING/Game; M18_MSB is the m18_00_00_00.msb.dcx the game loads.
"""

import math
import sys
import warnings
from collections import defaultdict
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import dcx_krak  # noqa: E402
import flver_build  # noqa: E402
import flver_floor  # noqa: E402
from er_navmesh import bnd4_entries  # noqa: E402
from msb_floor import _list, entry_name, read_msb, transform  # noqa: E402

STREET_PART = (-8.561904907226562, 93.99879455566406, -75.91121673583984, -27.011917114257812)  # m60, yaw
DOOR_PART = (-3.522, 93.956, -86.030, -27.887)
M18_TO_M60 = (147.0, 16.0, -102.0)  # msb_fog_gate.py
UNDRAWN = {"AET501_122_[0_|_Field_01_Grass_base_|_AEG030_518]", "Unicorn_Way_street", "Unicorn_Way_floor", None}
ZONE = dict(x=9.5, z_back=-28.0, y_low=-5.5, y_high=10.5)  # street triangles considered (door frame)
MARGIN, ONTO_PLANE, GRID = 0.12, 0.02, 0.1
CELL = 1.0


def _rotation(yaw):
    c, s = math.cos(math.radians(yaw)), math.sin(math.radians(yaw))
    return c, s


def obj_to_door(p):
    """(N, 3) OBJ points -> door frame (FLVER = OBJ with z negated; street part, then the door, both yawed)."""
    sx, sy, sz, syaw = STREET_PART
    dx, dy, dz, dyaw = DOOR_PART
    a, b = math.radians(syaw), math.radians(dyaw)
    fx, fy, fz = p[..., 0], p[..., 1], -p[..., 2]
    wx = sx + fx * math.cos(a) + fz * math.sin(a) - dx
    wz = sz - fx * math.sin(a) + fz * math.cos(a) - dz
    return np.stack([wx * math.cos(b) - wz * math.sin(b), sy + fy - dy, wx * math.sin(b) + wz * math.cos(b)], -1)


def door_to_obj(q):
    """Inverse of obj_to_door (it is a rigid map)."""
    basis = obj_to_door(np.eye(3)) - obj_to_door(np.zeros(3))
    origin = obj_to_door(np.zeros(3))
    return np.linalg.solve(basis.T, (np.asarray(q) - origin).T).T


def m18_walls(game, msb_path):
    """(N, 3, 3) triangles of the m18 assets around the door, in the door frame."""
    _, lists = read_msb(dcx_krak.decompress(Path(msb_path).read_bytes()))
    door = np.array([DOOR_PART[i] - M18_TO_M60[i] for i in range(3)])
    b = math.radians(DOOR_PART[3])
    meshes, out = {}, []
    for e in _list(lists, "PARTS_PARAM_ST").entries:
        name = entry_name(e)
        (px, py, pz), (rx, ry, rz) = transform(e)
        model = name[:10].lower()
        if not name.startswith("AEG") or not 60 < py < 100 or model.startswith(("aeg092", "aeg099")):
            continue
        if abs(rx) > 0.01 or abs(rz) > 0.01:  # a few tipped-over props (candles, rubble)
            continue
        if model not in meshes:
            path = Path(game) / "asset" / "aeg" / model[:6] / f"{model}.geombnd.dcx"
            bnd = dcx_krak.decompress(path.read_bytes())
            flver = next(d for _, n, d in bnd4_entries(bnd) if n.lower().endswith(".flver"))
            meshes[model] = np.array(list(flver_floor._triangles(flver)), float)
        t = meshes[model]
        c, s = _rotation(ry)
        w = np.stack([px + t[..., 0] * c + t[..., 2] * s, py + t[..., 1], pz - t[..., 0] * s + t[..., 2] * c], -1)
        d = w - door
        out.append(np.stack([d[..., 0] * math.cos(b) - d[..., 2] * math.sin(b), d[..., 1],
                             d[..., 0] * math.sin(b) + d[..., 2] * math.cos(b)], -1))
    return np.vstack(out)


class Walls:
    """Axis-aligned ray casts against the walls (a 1 m grid per axis)."""

    def __init__(self, triangles):
        self.t = triangles
        self.n = np.cross(triangles[:, 1] - triangles[:, 0], triangles[:, 2] - triangles[:, 0])
        lo, hi = triangles.min(1), triangles.max(1)
        self.cells, self._front = {}, {}
        for ax in range(3):
            a, b = [i for i in range(3) if i != ax]
            grid = defaultdict(list)
            i0, i1 = np.floor(lo[:, a] / CELL).astype(int), np.floor(hi[:, a] / CELL).astype(int)
            j0, j1 = np.floor(lo[:, b] / CELL).astype(int), np.floor(hi[:, b] / CELL).astype(int)
            for k in range(len(triangles)):
                for i in range(i0[k], i1[k] + 1):
                    for j in range(j0[k], j1[k] + 1):
                        grid[(i, j)].append(k)
            self.cells[ax] = {key: np.array(v) for key, v in grid.items()}

    def ray(self, p, ax, sign):
        """(distance, hit from the room side?) of the first wall along axis `ax`, or (inf, None)."""
        a, b = [i for i in range(3) if i != ax]
        ks = self.cells[ax].get((int(np.floor(p[a] / CELL)), int(np.floor(p[b] / CELL))))
        if ks is None:
            return np.inf, None
        t = self.t[ks]
        c0, c1, c2 = t[:, 0], t[:, 1], t[:, 2]

        def cross(u, v):
            return (v[:, a] - u[:, a]) * (p[b] - u[:, b]) - (v[:, b] - u[:, b]) * (p[a] - u[:, a])
        d1, d2, d3 = cross(c0, c1), cross(c1, c2), cross(c2, c0)
        inside = ((d1 >= 0) & (d2 >= 0) & (d3 >= 0)) | ((d1 <= 0) & (d2 <= 0) & (d3 <= 0))
        if not inside.any():
            return np.inf, None
        ks, t = ks[inside], t[inside]
        n = self.n[ks]
        na = n[:, ax]
        ok = np.abs(na) > 1e-12
        dist = (n * (t[:, 0] - p)).sum(1) / np.where(ok, na, 1) / sign
        dist = np.where(ok & (dist > 1e-4), dist, np.inf)
        i = dist.argmin()
        if not np.isfinite(dist[i]):
            return np.inf, None
        return dist[i], bool(n[i, ax] * sign > 0)  # FLVER winding: the room side is against the computed normal

    def open_space(self, p):
        front = back = 0
        for ax in range(3):
            for sign in (1, -1):
                _, f = self.ray(p, ax, sign)
                front += f is True
                back += f is False
        return front >= 4 and back <= 1

    def target_z(self, p):
        """None if the way from p (door frame) to the door plane crosses no open space, else the z to move it to:
        inside the wall that ends the last open stretch (at most MARGIN in, never past half the wall's thickness,
        and checked to be out of the open), else onto the door plane."""
        if p[2] >= 0:
            return None
        zs, q = [p[2]], p.copy()
        while True:
            t, f = self.ray(q, 2, 1)
            if f is None or q[2] + t > 0:
                break
            zs.append(q[2] + t)
            q = q.copy()
            q[2] += t + 1e-4
        zs.append(0.0)
        last = None
        for i, (z0, z1) in enumerate(zip(zs, zs[1:])):
            if z1 - z0 < 1e-3:
                continue
            for f in (0.5, 0.2, 0.8):
                m = p.copy()
                m[2] = z0 + (z1 - z0) * f
                if self.open_space(m):
                    last = i
                    break
        if last is None:
            return None
        end = zs[last + 1]
        if end >= -1e-6:
            return ONTO_PLANE
        wall = zs[last + 2] - end
        for z in (end + min(MARGIN, wall / 2), end + wall / 2, end + wall / 4):
            m = p.copy()
            m[2] = z
            if not self.open_space(m):
                return z
        return ONTO_PLANE

    def column_fronts(self, xs, ys):
        """front (see the top) of every column of a grid, NaN where a column has no open space."""
        out = np.full((len(xs), len(ys)), np.nan)
        for i, x in enumerate(xs):
            for j, y in enumerate(ys):
                z = self.target_z(np.array([x, y, -30.0]))
                if z is not None:
                    out[i, j] = z
        return out

    def exposed(self, tri, step=0.1):
        """Points of `tri` (every `step`) behind the door plane that are in the open."""
        n = max(1, int(math.ceil(max(np.linalg.norm(tri[i] - tri[(i + 1) % 3]) for i in range(3)) / step)))
        out = []
        for i in range(n + 1):
            for j in range(n + 1 - i):
                p = tri[0] * (1 - (i + j) / n) + tri[1] * (i / n) + tri[2] * (j / n)
                if p[2] < -0.02 and self.open_space(p):
                    out.append(p)
        return out


def _grid_rows(args):
    walls, xs, ys = args
    return walls.column_fronts(xs, ys)


def grid(walls, xs, ys, workers=4):
    from multiprocessing import Pool
    chunks = np.array_split(xs, workers * 4)
    with Pool(workers) as pool:
        return np.vstack(pool.map(_grid_rows, [(walls, c, ys) for c in chunks]))


def solve(positions, faces, walls, log=print):
    """({vertex: door-frame position} for every moved vertex, open-space samples left on drawn triangles)."""
    base = obj_to_door(np.asarray(positions, float))
    corners = np.array([[v for v, _ in f[1]] for f in faces])
    tri = base[corners]
    drawn = np.array([m not in UNDRAWN for m, _ in faces])
    near = ((tri[:, :, 2].min(1) < -0.02) & (np.abs(tri[:, :, 0]).min(1) < ZONE["x"])
            & (tri[:, :, 2].max(1) > ZONE["z_back"]) & (tri[:, :, 1].min(1) < ZONE["y_high"])
            & (tri[:, :, 1].max(1) > ZONE["y_low"]) & drawn)
    near = np.where(near)[0]
    log(f"{len(near)} drawn triangles reach behind the door")
    x0, y0 = -ZONE["x"] - 2 * GRID, ZONE["y_low"] - 2 * GRID
    xs = x0 + GRID * np.arange(int((2 * ZONE["x"] + 4 * GRID) / GRID) + 1)
    ys = y0 + GRID * np.arange(int((ZONE["y_high"] - ZONE["y_low"] + 4 * GRID) / GRID) + 1)
    fronts = grid(walls, xs, ys)
    log(f"{np.isfinite(fronts).sum()} of {fronts.size} columns ({GRID} m) have open space")
    # each cell takes the highest front of itself and its 8 neighbours, so a triangle sampled every GRID covers all
    pad = np.pad(fronts, 1, constant_values=np.nan)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", RuntimeWarning)  # all-NaN cells stay NaN
        wide = np.nanmax(np.stack([pad[1 + a:1 + a + len(xs), 1 + b:1 + b + len(ys)]
                                   for a in (-1, 0, 1) for b in (-1, 0, 1)]), axis=0)

    def need(t):
        n = max(1, int(math.ceil(max(np.linalg.norm(t[i] - t[(i + 1) % 3]) for i in range(3)) / GRID)))
        w = np.array([(1 - (i + j) / n, i / n, j / n) for i in range(n + 1) for j in range(n + 1 - i)])
        pts = w @ t
        ix = np.clip(np.round((pts[:, 0] - x0) / GRID).astype(int), 0, len(xs) - 1)
        iy = np.clip(np.round((pts[:, 1] - y0) / GRID).astype(int), 0, len(ys) - 1)
        values = wide[ix, iy]
        return None if np.isnan(values).all() else float(np.nanmax(values))

    def place():
        target = {}
        for k in near:
            z = need(tri[k])
            if z is None:
                continue
            for v in corners[k]:
                if base[v][2] < z and (v not in target or target[v] < z):
                    target[v] = z
        moved = base.copy()
        for v, z in target.items():
            moved[v, 2] = z
        return target, moved

    # Then every drawn triangle is checked every 0.1 m; where a point is still in the open (the column test missed a
    # stretch, e.g. between a pillar and the wall), its cells get the wall ahead of it (or the door plane) as their
    # front and the vertices are placed again, until none is left.
    for r in range(20):
        target, moved = place()
        left = []
        for k in near:
            left += walls.exposed(moved[corners[k]])
        log(f"pass {r}: {len(target)} vertices moved, {len(left)} points still in the open")
        if not left:
            break
        for p in left:
            t, f = walls.ray(p, 2, 1)
            z = p[2] + t + min(MARGIN, 0.05) if f and p[2] + t < 0 else ONTO_PLANE
            i = int(round((p[0] - x0) / GRID))
            j = int(round((p[1] - y0) / GRID))
            for a in (-1, 0, 1):
                for b in (-1, 0, 1):
                    if 0 <= i + a < len(xs) and 0 <= j + b < len(ys):
                        old = wide[i + a, j + b]
                        wide[i + a, j + b] = z if np.isnan(old) else max(old, z)
    moves = {int(v): moved[v] for v in target}
    return moves, len(left)


def write(path, positions, moves):
    fmt = lambda p: " ".join(f"{c:.6f}" for c in p)  # noqa: E731
    lines = [f"v {fmt(positions[v])} {fmt(door_to_obj(moves[v]))}" for v in sorted(moves)]
    Path(path).write_text("\n".join(lines) + "\n")


def main():
    obj, game, msb, out = sys.argv[1:5]
    positions, _, faces = flver_build.parse_obj(Path(obj))
    walls = Walls(m18_walls(game, msb))
    moves, left = solve(positions, faces, walls)
    write(out, positions, moves)
    print(f"{out}: {len(moves)} vertices moved" + ("" if not left else f" ({left} samples STILL in the open)"))


if __name__ == "__main__":
    main()
