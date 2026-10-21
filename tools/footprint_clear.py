"""Hide every piece of Elden Ring that sits under or around the Unicorn Way street, in every map that draws it.

Why this is needed: the street is ~730 x 720 m including its backdrop and most of it lies west and south of
m60_42_36_00, over m60_41_36_00 and other tiles. Elden Ring draws each 256 m tile (m60_XX_ZZ_00) up close, plus
far-view copies of it in a 512 m tile (m60_XX/2_ZZ/2_01) and a 1024 m tile (m60_XX/4_ZZ/4_02). Deleting a rock or
the ground in one of those maps leaves the others, so things "come back"; and the close-up tiles have collision.

What it does, for every tile of the three sizes that overlaps the street:
  - assets (rocks, grass, trees...) whose position is over the street footprint (any W101 surface, dilated by
    MARGIN) are moved 1000 m down;
  - ground map pieces and their collision (m.../h... parts) of every 256 m tile the footprint touches, and their
    far-view copies (m14XXZZ.., m24XXZZ..), are moved 1000 m down too.
Our own parts (street, sky, grace, exit door) and all characters are left alone. Moving rather than deleting keeps
every index the same, so it is safe to rerun, and parts already moved are not moved again.

Coordinates: a part in tile (x, z) of size s (256, 512, 1024) is at global (x * s + s / 2 + px, ..., z * s + s / 2 + pz).
The footprint is computed in m60_42_36_00's local coordinates from the street OBJ and the street part's transform.

Used by blender_clear_footprint.py (which can read the game's Oodle maps); can be tested in the container on DFLT or
already-decompressed maps with `python footprint_clear.py street.obj m60_42_36_00.msb.dcx map.msb ...`.
"""

import math
import re
import struct
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from msb_floor import _list, entry_name, read_msb, write_msb  # noqa: E402

HOME = 42, 36  # m60_42_36_00, whose local coordinates the footprint uses
STREET = "AEG030_518_9000"
MARGIN = 6  # metres around the W101 surfaces
SINK = 1000.0
CELL = 1.0
KEEP = ("AEG030_518", "AEG030_523", "AEG099_", "AEG110_064", "AEG008_262_1000", "AEG092_102_1000")
MAP_PIECE, COLLISION, ASSET = 0, 5, 13
SIZE = {"00": 256, "01": 512, "02": 1024}
DIVIDE = {"00": 1, "01": 2, "02": 4}


def parse_obj(path):
    vertices, faces = [], []
    with open(path) as f:
        for line in f:
            if line.startswith("v "):
                vertices.append(tuple(map(float, line.split()[1:4])))
            elif line.startswith("f "):
                ids = [int(t.split("/")[0]) - 1 for t in line.split()[1:]]
                faces += [(ids[0], ids[k], ids[k + 1]) for k in range(1, len(ids) - 1)]
    return np.array(vertices), np.array(faces)


def street_transform(home_msb):
    _, lists = read_msb(home_msb)
    for e in _list(lists, "PARTS_PARAM_ST").entries:
        if entry_name(e) == STREET:
            return struct.unpack_from("<3f", e, 0x20), struct.unpack_from("<3f", e, 0x2C)[1]
    raise ValueError(f"{STREET} not in the map")


class Footprint:
    """Cells (CELL m) of m60_42_36_00-local XZ covered by any W101 surface, dilated by MARGIN."""

    def __init__(self, vertices, faces, position, rotation_y):
        a = math.radians(rotation_y)
        fx, fy, fz = vertices[:, 0], vertices[:, 1], -vertices[:, 2]  # FLVER = OBJ with z negated
        world = np.stack([position[0] + fx * math.cos(a) + fz * math.sin(a), position[1] + fy,
                          position[2] - fx * math.sin(a) + fz * math.cos(a)], 1)
        a, b, c = world[faces[:, 0]], world[faces[:, 1]], world[faces[:, 2]]
        area = np.linalg.norm(np.cross(b - a, c - a), axis=1) / 2
        count = np.ceil(area / CELL ** 2 * 2).astype(int) + 3
        tri = np.repeat(np.arange(len(a)), count)
        rng = np.random.default_rng(0)
        r1, r2 = rng.random(len(tri)), rng.random(len(tri))
        flip = r1 + r2 > 1
        r1[flip], r2[flip] = 1 - r1[flip], 1 - r2[flip]
        points = a[tri] + (b[tri] - a[tri]) * r1[:, None] + (c[tri] - a[tri]) * r2[:, None]
        self.origin = points[:, [0, 2]].min(0) - MARGIN - 1
        size = np.ceil((points[:, [0, 2]].max(0) + MARGIN + 1 - self.origin) / CELL).astype(int) + 1
        grid = np.zeros(size, bool)
        ij = ((points[:, [0, 2]] - self.origin) / CELL).astype(int)
        grid[ij[:, 0], ij[:, 1]] = True
        r = int(math.ceil(MARGIN / CELL))  # dilate with a square of side 2r+1 (separable max filter)
        for axis in (0, 1):
            out = grid.copy()
            for d in range(1, r + 1):
                out |= np.roll(grid, d, axis) | np.roll(grid, -d, axis)
            grid = out
        self.grid = grid

    def covers(self, x, z):
        i, j = int((x - self.origin[0]) / CELL), int((z - self.origin[1]) / CELL)
        return 0 <= i < self.grid.shape[0] and 0 <= j < self.grid.shape[1] and bool(self.grid[i, j])

    def small_tiles(self):
        """(x, z) indices of the 256 m tiles the footprint touches."""
        i, j = np.nonzero(self.grid)
        x = self.origin[0] + i * CELL
        z = self.origin[1] + j * CELL
        tx = HOME[0] + np.floor((x + 128) / 256).astype(int)
        tz = HOME[1] + np.floor((z + 128) / 256).astype(int)
        return sorted(set(zip(tx.tolist(), tz.tolist())))


def tiles_to_edit(small):
    """Map names (m60_XX_ZZ_LL) of all sizes that draw the given 256 m tiles."""
    names = set()
    for level, d in DIVIDE.items():
        names |= {f"m60_{x // d:02d}_{z // d:02d}_{level}" for x, z in small}
    return sorted(names)


def to_home(name, x, z):
    """Position in a part of map `name` -> m60_42_36_00-local coordinates."""
    _, tx, tz, level = name.split("_")
    s = SIZE[level]
    home = HOME[0] * 256 + 128, HOME[1] * 256 + 128
    return int(tx) * s + s / 2 + x - home[0], int(tz) * s + s / 2 + z - home[1]


GROUND = re.compile(r"^[mh](?:[12]4)?(\d\d)(\d\d)\d\d(?:_\d+)?$")  # m413600_0000, h413600, m24413600_0000, m14...


def clear(name, raw, footprint, small):
    """(new MSB bytes or None if nothing to change, [report lines]) for map `name`."""
    header, lists = read_msb(raw)
    parts = _list(lists, "PARTS_PARAM_ST")
    small = set(small)
    report = []
    for k, e in enumerate(parts.entries):
        kind = struct.unpack_from("<i", e, 0x0C)[0]
        part = entry_name(e)
        x, y, z = struct.unpack_from("<3f", e, 0x20)
        if kind not in (MAP_PIECE, COLLISION, ASSET) or y < -SINK / 2 or any(s in part for s in KEEP):
            continue
        if kind == ASSET:
            hx, hz = to_home(name, x, z)
            hit = footprint.covers(hx, hz)
        else:
            m = GROUND.match(part)
            hit = bool(m) and (int(m.group(1)), int(m.group(2))) in small
        if hit:
            e = bytearray(e)
            struct.pack_into("<3f", e, 0x20, x, y - SINK, z)
            parts.entries[k] = bytes(e)
            report.append(f"{name}: {part} ({x:.1f}, {y:.1f}, {z:.1f}) moved down")
    return (write_msb(header, lists) if report else None), report


def main():
    obj, home, *maps = sys.argv[1:]
    import zlib

    def load(p):
        d = Path(p).read_bytes()
        return zlib.decompress(d[0x4C:]) if d[:4] == b"DCX\0" else d

    position, rotation = street_transform(load(home))
    fp = Footprint(*parse_obj(obj), position, rotation)
    small = fp.small_tiles()
    print("256 m tiles under the street:", small)
    print("maps to edit:", tiles_to_edit(small))
    for p in maps:
        name = Path(p).name.split(".")[0]
        _, report = clear(name, load(p), fp, small)
        print("\n".join(report) or f"{name}: nothing to move")


if __name__ == "__main__":
    main()
