"""Clear every Elden Ring part the player can see inside the Wizard101 sky dome round Unicorn Way.

Ryan, round 9 (2026-10-27): "Some ER terrain is still visible above one of the W101 walls", "behind the hedge maze he
sees ER's stone/natural map", "he can still see birds flying in the sky". The sky dome (asset aeg030_523, part
AEG030_523_9000 in m60_42_36_00, see sky_bake.py) is a closed hull round the street, the garden and the rooms, so
whatever is outside it is hidden and whatever is inside can show, over the walls or behind the hedges. Seen from the
street, the north-west end (the garden door) and the garden have open views north: the W101 silhouette is under 5
degrees there, and vanilla Stormhill (m60_41_37_00 / 42_37 / 43_37, 100-170 m high) stood in them.

The dome's FLVER is read (sky_bake.flver_mesh) and turned into its radius per direction from the part's origin
(1-degree bins over every triangle, the larger corner radius, gaps filled from the nearest bin). A point is inside when
its distance from the origin is under MARGIN times that radius (MARGIN leaves room for the sky being rebuilt larger).

For every map given (m60 tiles of all three sizes, project copies or vanilla):
  - assets, dummy assets and dummy enemies whose position, or a point HEIGHTS m above it (their models are not read),
    is inside: moved SINK m down, as footprint_clear.py does (no index changes); footprint_clear.KEEP stays;
  - ground map pieces of every 256 m tile whose square reaches inside the dome (TILE_HEIGHTS), in all three sizes:
    named m[NN]XXZZnn, where NN is any far-view prefix (footprint_clear.GROUND only knew 1x/2x, so far-view copies such
    as m04403600_0000 of tile 40_36 in m60_20_18_01 / m60_10_09_02 and m33433500_0000 of 43_35 stayed up under the
    street);
  - SFX and WindSFX regions inside: sunk;
  - enemies inside: deleted the way msb_audit.py deletes them (part indices remapped, orphaned treasure / retry /
    mount events dropped), except ours (msb_audit.OUR_ENTITIES) and c1000 helpers; animals that fly (FLYING) are
    deleted within FLYING_MARGIN times the radius, as they wander in.
Map pieces and collision of other tiles, and anything already sunk, are left alone, so rerunning changes nothing more.

Usage: python dome_clear.py home.msb.dcx dome.geombnd.dcx out_dir map.msb[.dcx]... [--apply]
  home: m60_42_36_00 (the dome part's transform); dome: the project's asset/aeg/aeg030/aeg030_523.geombnd.dcx.
  Prints one line per finding; with --apply writes every changed map to out_dir as DFLT DCX.
"""

import math
import re
import struct
import sys
import zlib
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import dcx_krak  # noqa: E402
from footprint_clear import KEEP, SINK, SIZE, to_home  # noqa: E402
from msb_add_ghost import check_grouped, count_users, q, u32  # noqa: E402
from msb_add_mobs import ENEMY, remap_parts  # noqa: E402
from msb_audit import HELPERS, OUR_ENTITIES, _orphaned, _renumber, dflt  # noqa: E402
from msb_floor import _list, entry_name, read_msb, transform, write_msb  # noqa: E402

DOME_PART = "AEG030_523_9000"
MARGIN = 1.25
FLYING_MARGIN = 2.0
HEIGHTS = (0.0, 15.0, 40.0)
TILE_HEIGHTS = (0.0, 50.0, 100.0, 150.0, 200.0, 300.0)
TILE_STEP = 16.0
MAP_PIECE, ASSET, DUMMY_ASSET, DUMMY_ENEMY = 0, 13, 9, 10
SFX_REGIONS = (5, 6)
GROUND = re.compile(r"^m\d*(\d\d)(\d\d)\d\d(?:_\d+)?$")  # m413700_0000, m15413700_0000, m04403600_0000
# Smithbox aliases of characters that fly: birds, bats, dragonflies, crows, drakes
FLYING = {"c6001", "c6040", "c6070", "c6072", "c6080", "c6081", "c6082",  # eagle, owl, murre, pigeon, dragonflies
          "c5401", "c5430", "c5460", "c5470", "c5471", "c5472",  # DLC eagle, owl, murre, dragonflies
          "c4200", "c4201", "c5530", "c4210", "c4560", "c4561", "c5090",  # bats, warhawk, giant crows, gravebird
          "c4980", "c6260", "c4500", "c4505"}  # death rite birds, flying dragons
KIND = {2: "enemy", 9: "dummy asset", 10: "dummy enemy", 13: "asset"}


def load(path):
    data = Path(path).read_bytes()
    if data[:4] != b"DCX\0":
        return data
    return zlib.decompress(data[0x4C:]) if data[0x28:0x2C] == b"DFLT" else dcx_krak.decompress(data)


class Dome:
    """Radius of the sky dome per direction, in m60_42_36_00 coordinates."""

    def __init__(self, home_raw, dome_geombnd, bins=1.0):
        import sky_bake
        from scipy.ndimage import distance_transform_edt
        _, lists = read_msb(home_raw)
        part = next(e for e in _list(lists, "PARTS_PARAM_ST").entries if entry_name(e) == DOME_PART)
        (px, py, pz), (_, ry, _) = transform(part)
        self.origin = np.array((px, py, pz))
        raw = load(dome_geombnd)
        positions, faces = sky_bake.flver_mesh(raw[raw.find(b"FLVER\0"):])
        p = np.asarray(positions, float)
        c, s = math.cos(math.radians(ry)), math.sin(math.radians(ry))
        world = np.stack([p[:, 0] * c + p[:, 2] * s, p[:, 1], -p[:, 0] * s + p[:, 2] * c], 1)  # about the origin
        self.bins = bins
        rows, cols = int(180 / bins), int(360 / bins)
        grid = np.zeros((rows, cols))
        el, az, r = self._angles(world)
        for tri in np.asarray(faces):
            e, a, rr = el[tri], az[tri], r[tri]
            if a.max() - a.min() > 180:  # crosses azimuth 0
                a = np.where(a < 180, a + 360, a)
            r0, r1 = int((90 - e.max()) / bins), int((90 - e.min()) / bins)
            c0, c1 = int(a.min() / bins), int(a.max() / bins)
            for row in range(max(r0, 0), min(r1, rows - 1) + 1):
                cs = np.arange(c0, c1 + 1) % cols
                grid[row, cs] = np.maximum(grid[row, cs], rr.max())
        empty = grid == 0
        if empty.any():
            _, (i, j) = distance_transform_edt(empty, return_indices=True)
            grid = grid[i, j]
        self.grid = grid

    def _angles(self, d):
        r = np.linalg.norm(d, axis=1)
        el = np.degrees(np.arcsin(np.clip(d[:, 1] / np.maximum(r, 1e-9), -1, 1)))
        az = np.degrees(np.arctan2(d[:, 0], d[:, 2])) % 360
        return el, az, r

    def ratio(self, points):
        """Distance from the dome's origin / dome radius in that direction, for (n, 3) home-local points."""
        d = np.atleast_2d(np.asarray(points, float)) - self.origin
        el, az, r = self._angles(d)
        rows, cols = self.grid.shape
        row = np.clip(((90 - el) / self.bins).astype(int), 0, rows - 1)
        col = (az / self.bins).astype(int) % cols
        return r / self.grid[row, col]


def tiles_inside(dome, margin=MARGIN):
    """(x, z) of the 256 m tiles whose square reaches inside the dome at any of TILE_HEIGHTS."""
    found = set()
    for x in range(36, 50):
        for z in range(30, 44):
            x0, z0 = (x - 42) * 256 - 128, (z - 36) * 256 - 128
            g = np.arange(0, 256 + TILE_STEP / 2, TILE_STEP)
            gx, gz = np.meshgrid(x0 + g, z0 + g)
            for y in TILE_HEIGHTS:
                pts = np.stack([gx.ravel(), np.full(gx.size, y), gz.ravel()], 1)
                if (dome.ratio(pts) < margin).any():
                    found.add((x, z))
                    break
    return found


def ours(e):
    name = entry_name(e)
    return any(k in name for k in KEEP) or (u32(e, 0x0C) == ENEMY and u32(e, q(e, 0x60)) in OUR_ENTITIES)


def clear(name, raw, dome, tiles, margin=MARGIN, apply=False):
    """(new map bytes or None, [finding lines]) for map `name` (m60_XX_ZZ_LL)."""
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST").entries
    models = [entry_name(m) for m in _list(lists, "MODEL_PARAM_ST").entries]
    lines, delete, sink = [], set(), []
    for k, e in enumerate(parts):
        kind = u32(e, 0x0C)
        x, y, z = struct.unpack_from("<3f", e, 0x20)
        if y < -SINK / 2 or ours(e):
            continue
        model = models[u32(e, 0x14)] if u32(e, 0x14) >= 0 else "?"
        label = f"{name}: {KIND.get(kind, 'map piece')} {entry_name(e)} ({model}) at ({x:.1f}, {y:.1f}, {z:.1f})"
        if kind == MAP_PIECE:
            m = GROUND.match(entry_name(e))
            if m and (int(m.group(1)), int(m.group(2))) in tiles:
                lines.append(f"{label}: sunk {SINK:.0f} m (ground of tile {m.group(1)}_{m.group(2)}, inside the dome)")
                sink.append(k)
            continue
        if kind not in KIND:
            continue
        hx, hz = to_home(name, x, z)
        r = min(dome.ratio([(hx, y + h, hz)])[0] for h in HEIGHTS)
        if kind == ENEMY:
            if model in HELPERS:
                continue
            if r < margin or (model in FLYING and r < FLYING_MARGIN):
                rows = struct.unpack_from("<ii", e, q(e, 0x68) + 0x08)
                why = "flying animal" if model in FLYING else "inside the dome"
                lines.append(f"{label}, entity {u32(e, q(e, 0x60))}, Think/NpcParam {rows[0]}/{rows[1]}: deleted "
                             f"({why}, {r:.2f} x radius)")
                delete.add(k)
        elif r < margin:
            lines.append(f"{label}: sunk {SINK:.0f} m (inside the dome, {r:.2f} x radius)")
            sink.append(k)
    sfx = []
    regions = _list(lists, "POINT_PARAM_ST").entries
    for k, e in enumerate(regions):
        x, y, z = struct.unpack_from("<3f", e, 0x14)
        if u32(e, 0x08) in SFX_REGIONS and y > -SINK / 2:
            hx, hz = to_home(name, x, z)
            r = dome.ratio([(hx, y, hz)])[0]
            if r < margin:
                lines.append(f"{name}: effect region {entry_name(e)} ({x:.1f}, {y:.1f}, {z:.1f}): sunk {SINK:.0f} m "
                             f"(inside the dome, {r:.2f} x radius)")
                sfx.append(k)
    if not apply or not (delete or sink or sfx):
        return None, lines
    for k in sfx:
        e = bytearray(regions[k])
        x, y, z = struct.unpack_from("<3f", e, 0x14)
        struct.pack_into("<3f", e, 0x14, x, y - SINK, z)
        regions[k] = bytes(e)
    for k in sink:
        e = bytearray(parts[k])
        x, y, z = struct.unpack_from("<3f", e, 0x20)
        struct.pack_into("<3f", e, 0x20, x, y - SINK, z)
        parts[k] = bytes(e)
    if delete:
        events = _list(lists, "EVENT_PARAM_ST").entries
        drop = [i for i, ev in enumerate(events) if _orphaned(ev, delete)]
        for i in reversed(drop):
            lines.append(f"{name}: event {entry_name(events[i])} (type {u32(events[i], 0x0C)}) removed with its parts")
            del events[i]
        _renumber(events, 0x0C)
        gone = sorted(delete)
        remap_parts(lists, lambda i: -1 if i in delete else i - sum(1 for g in gone if g < i))
        for k in reversed(gone):
            del parts[k]
        _renumber(parts, 0x0C)
        count_users(lists)
    check_grouped(lists)
    return write_msb(header, lists), lines


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    home, dome_path, out, *maps = args
    apply = "--apply" in sys.argv
    template = Path(home).read_bytes()
    assert template[0x28:0x2C] == b"DFLT", "home map must be DFLT"
    dome = Dome(load(home), dome_path)
    tiles = tiles_inside(dome)
    print("256 m tiles reaching inside the dome:", sorted(tiles))
    for path in maps:
        name = Path(path).name.split(".")[0]
        assert name.split("_")[3] in SIZE, name
        raw = load(path)
        assert write_msb(*read_msb(raw)) == raw, f"{name} does not round-trip"
        new, lines = clear(name, raw, dome, tiles, apply=apply)
        print("\n".join(lines) or f"{name}: nothing found")
        if new is not None:
            Path(out).mkdir(parents=True, exist_ok=True)
            (Path(out) / f"{name}.msb.dcx").write_bytes(dflt(new, template))


if __name__ == "__main__":
    main()
