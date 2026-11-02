"""Round 24 (hud H1, Ryan: "Wisps are STILL visible through walls"): a voxel map of the street's own geometry for
the hud_orbs DLL's line-of-sight test (w101_orbs/book/street_sight.bin on Ryan's PC; never committed: it is made
from Wizard101's mesh).

Why: round 23 hid wisps with questhelper::Nav::sees, which only knows Wizard101's walk graph (zone.nav) widened by
3.6 m. Checked against the real street mesh on every position in Ryan's round 23 log, that test said "seen" for 19 %
of the wisp sight lines the houses and walls actually block (the walk area reaches 3.6 m into buildings, a wall had to
be 2.4 m thick along the line, lines leaving the grid and walk-graph holes counted as open). This map is the street
mesh the game draws (reference\\w101\\converted_classic\\WC_Z06_Unicorn_Way_Classic_edited.obj, the classic street
Ryan edited), so a sight line is blocked exactly where a house, wall, hedge, statue or lamp post is.

Frame: the obj is Blender's y-up export of Wizard101's z-up zone in metres, so Wizard101 (X, Y) = (obj x, -obj z) and
height = obj y. MSB (x, z) = R (X, Y) + t, the round 20 make_nav.py rigid fit on the four street NPCs; MSB y =
obj y + 93.999 (the street's placement, as wisps.rs / questhelper.rs use it).

File (little endian): b"W1SG", u32 version 1, f32 origin x, origin z (MSB), f32 cell (m), f32 y0 (MSB), f32 slab (m),
u32 width, u32 height, then width*height u64: bit k of cell (x, z) = some surface between y0 + k*slab and
y0 + (k+1)*slab in that cell (cell x covers origin x + x*cell .. +cell).

Usage: python -I w101_street_sight.py <WC_Z06_Unicorn_Way_Classic_edited.obj> <quest_nav.tsv> <out street_sight.bin>
(numpy)
"""
import struct
import sys

import numpy as np

CELL = 0.5
SLAB = 0.5
Y0 = 88.0
SLABS = 64
STREET_Y = 93.999
MARGIN = 30.0


def fit():
    """make_nav.py's 2D rigid transform Wizard101 (X, Y) metres -> MSB (x, z), fitted on the four street NPCs."""
    frame = np.array([(26.543, 22.2), (-41.857, 85.837), (-20.198, 86.55), (-125.714, 88.655)])
    msb = np.array([(5.003, -44.078), (-84.838, -18.448), (-65.865, -7.976), (-160.827, -54.024)])
    fc, mc = frame.mean(0), msb.mean(0)
    u, _s, vt = np.linalg.svd((frame - fc).T @ (msb - mc))
    r = (u @ vt).T
    return r, mc - r @ fc


def load_obj(path):
    verts, faces = [], []
    with open(path) as f:
        for line in f:
            if line.startswith('v '):
                verts.append(line.split()[1:4])
            elif line.startswith('f '):
                idx = [int(p.split('/')[0]) for p in line.split()[1:]]
                faces.extend((idx[0], idx[i], idx[i + 1]) for i in range(1, len(idx) - 1))
    return np.array(verts, dtype=np.float64), np.array(faces) - 1


def main(argv):
    obj, nav_path, out = argv[1:4]
    v, f = load_obj(obj)
    r, t = fit()
    xz = np.c_[v[:, 0], -v[:, 2]] @ r.T + t
    p = np.c_[xz[:, 0], v[:, 1] + STREET_Y, xz[:, 1]]
    nodes = np.array([[float(x) for x in line.split('\t')[1:3]] for line in open(nav_path) if line.startswith('N\t')])
    lo, hi = nodes.min(0) - MARGIN, nodes.max(0) + MARGIN
    w, h = int((hi[0] - lo[0]) / CELL) + 1, int((hi[1] - lo[1]) / CELL) + 1
    grid = np.zeros(w * h, dtype=np.uint64)
    tri = p[f]
    a, b, c = tri[:, 0], tri[:, 1], tri[:, 2]
    longest = np.maximum.reduce([np.linalg.norm(b - a, axis=1), np.linalg.norm(c - a, axis=1),
                                 np.linalg.norm(c - b, axis=1)])
    # Sample every triangle on a barycentric grid finer than the cells (0.2 m) and mark the voxels the samples land in.
    steps = np.clip(np.ceil(longest / 0.2).astype(int), 1, 2000)
    for k in np.unique(steps):
        sel = np.where(steps == k)[0]
        i, j = np.meshgrid(np.arange(k + 1), np.arange(k + 1))
        keep = (i + j) <= k
        u = (i[keep] / k)[None, :, None]
        s = (j[keep] / k)[None, :, None]
        pts = (a[sel][:, None, :] + u * (b[sel] - a[sel])[:, None, :] + s * (c[sel] - a[sel])[:, None, :]).reshape(-1, 3)
        cx = ((pts[:, 0] - lo[0]) / CELL).astype(int)
        cz = ((pts[:, 2] - lo[1]) / CELL).astype(int)
        slab = np.floor((pts[:, 1] - Y0) / SLAB).astype(int)
        ok = (cx >= 0) & (cz >= 0) & (cx < w) & (cz < h) & (slab >= 0)
        slab = np.minimum(slab, SLABS - 1)
        np.bitwise_or.at(grid, cz[ok] * w + cx[ok], np.uint64(1) << slab[ok].astype(np.uint64))
    with open(out, 'wb') as o:
        o.write(b'W1SG' + struct.pack('<I5f2I', 1, lo[0], lo[1], CELL, Y0, SLAB, w, h))
        o.write(grid.astype('<u8').tobytes())
    print(f'{len(f)} triangles -> {w}x{h} cells of {CELL} m, {SLABS} slabs of {SLAB} m from y {Y0}; '
          f'{int((grid != 0).sum())} cells with geometry; origin ({lo[0]:.2f}, {lo[1]:.2f}) -> {out}')


if __name__ == '__main__':
    main(sys.argv)
