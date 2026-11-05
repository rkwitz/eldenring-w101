"""quest_nav.tsv for the Quest Helper's step count: Wizard101's own walk graph of Unicorn Way (zone.nav, decoded by
tools/w101_pathing.py) moved into the street's MSB coordinates. W101 (x, y) / 100 = the street frame's (x, z); the
frame -> MSB fit is a 2D rigid transform fitted on the four street NPCs (frame positions w101_quests.CHARACTERS,
MSB positions questhelper.rs TARGETS). Round 20; moved into the repo in round 35 (R-DOC).

Usage: python w101_quest_nav.py unicorn_way_pathing.json w101_orbs\\book\\quest_nav.tsv
"""
import json
import sys
import numpy as np
frame = np.array([(26.543, 22.2), (-41.857, 85.837), (-20.198, 86.55), (-125.714, 88.655)])
msb = np.array([(5.003, -44.078), (-84.838, -18.448), (-65.865, -7.976), (-160.827, -54.024)])
best = None
for flip in (1, -1):
    f = frame * [1, flip]
    fc, mc = f.mean(0), msb.mean(0)
    h = (f - fc).T @ (msb - mc)
    u, _s, vt = np.linalg.svd(h)
    r = (u @ vt).T
    if np.linalg.det(r) < 0:
        continue
    t = mc - r @ fc
    err = np.abs((f @ r.T + t) - msb).max()
    if best is None or err < best[0]:
        best = (err, flip, r, t)
err, flip, r, t = best
print('fit max error m', err, 'flip', flip, 'rotation deg', np.degrees(np.arctan2(r[1, 0], r[0, 0])))
nav = json.load(open(sys.argv[1]))['nav']
pts = np.array([(n[0] / 100, n[1] / 100) for n in nav['nodes']]) * [1, flip]
m = pts @ r.T + t
with open(sys.argv[2], 'w') as f:
    f.write('# Unicorn Way walk graph (Wizard101 zone.nav) in m60_42_36_00 MSB x, z; E = edge\n')
    for x, z in m:
        f.write(f'N\t{x:.2f}\t{z:.2f}\n')
    seen = set()
    for a, b in nav['edges']:
        k = (min(a, b), max(a, b))
        if k not in seen:
            seen.add(k)
            f.write(f'E\t{k[0]}\t{k[1]}\n')
print(len(m), 'nodes', len(seen), 'edges; x', m[:, 0].min(), m[:, 0].max(), 'z', m[:, 1].min(), m[:, 1].max())
