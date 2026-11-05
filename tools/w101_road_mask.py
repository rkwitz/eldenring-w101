"""Round 24 (M1): where Unicorn Way's road is, for the HUD DLL's sidewalk check (hud_orbs/src/sidewalk.rs).

Ryan: "enemies should not aggro while the player is on the sidewalk; only when the player steps into the road (as in
W101, where mobs stay on the road paths)". In Wizard101's street model the road is the Street* meshes and the
pavements are the Sidewalk* meshes; every mob path node of the Lost Souls, Skeletal Pirates and Dark Fairies lies on a
Street* triangle (55 of 59; the other 4 are 2.3-3.5 m off it, 3 of them inside a Combat Sigil's 7 m radius;
checked with this file's --check). So:
  R  road triangles   = Street* (street FLVER, moved into the map with the street part's transform)
  C  road circles     = every Combat Sigil, radius 7 m (W101: stepping into the circle starts the duel)
  B  the street's box = the road triangles' bounds plus MARGIN on each side
Everything else at street level is off the road: Sidewalk*, but also the big ground meshes (Editable*, Mesh*: lawns,
plazas) where half of Ryan's round 23 positions were. The DLL sets flag SIDEWALK_FLAG while the player is inside the
box, at street height, out of the hedge maze, and on no R triangle or C circle;
emevd.py --sidewalk puts SpEffect 99006 (vanilla: enemies' sight and hearing of the player x0, no visual effect, not
saved) on the player while the flag is on.

The output is Wizard101 data (made from its street model), so it goes to Ryan's PC, never into the repo:
    python w101_road_mask.py WC_Z06_Unicorn_Way_Classic.obj unicorn_way_pathing.json m60_42_36_00.msb.dcx out.tsv
(OBJ: reference\\w101\\converted_classic; pathing json: tools/w101_pathing.py output; the msb gives the street part's
position.) Lines: "R x1 z1 x2 z2 x3 z3", "C x z r", "B min_x min_z max_x max_z", map (MSB) metres. Add --check to print the mob nodes.
"""
import json
import math
import re
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from msb_floor import _list, entry_name, read_msb  # noqa: E402

STREET = "AEG030_518_9000"
ROAD = {"Street"}
MARGIN = 30.0
SIGIL_RADIUS = 7.0  # W101 Combat Sigil radius 700 units
MOB_GROUPS = ("Ghost", "Skeleton", "Fair")


def street_transform(msb_path):
    data = Path(msb_path).read_bytes()
    if data[:4] == b"DCX\0":
        data = zlib.decompress(data[0x4C:])
    _h, lists = read_msb(data)
    part = next(e for e in _list(lists, "PARTS_PARAM_ST").entries if entry_name(e) == STREET)
    return struct.unpack_from("<3f", part, 0x20), struct.unpack_from("<3f", part, 0x2C)[1]


def to_map(position, yaw, x, z):
    """Street FLVER (x, z) -> map (x, z) (msb_add_room.to_world without the height)."""
    c, s = math.cos(math.radians(yaw)), math.sin(math.radians(yaw))
    return position[0] + x * c + z * s, position[2] - x * s + z * c


def read_triangles(obj_path):
    """{group kind (name without its number): [triangle of (x, z) street FLVER points]}; FLVER = OBJ with z negated."""
    vertices, kinds, kind = [], {}, ""
    for line in open(obj_path, encoding="utf-8", errors="replace"):
        if line.startswith(("o ", "g ")):
            parts = line.split()
            kind = re.sub(r"[_#]?\d+$", "", parts[1]) if len(parts) > 1 else ""
        elif line.startswith("v "):
            x, _y, z = map(float, line.split()[1:4])
            vertices.append((x, -z))
        elif line.startswith("f "):
            idx = [int(t.split("/")[0]) for t in line.split()[1:]]
            idx = [i - 1 if i > 0 else len(vertices) + i for i in idx]
            for t in range(1, len(idx) - 1):
                kinds.setdefault(kind, []).append((vertices[idx[0]], vertices[idx[t]], vertices[idx[t + 1]]))
    return kinds


def inside(p, tri):
    (x, z), (a, b, c) = p, tri
    side = lambda p1, p2: (x - p2[0]) * (p1[1] - p2[1]) - (p1[0] - p2[0]) * (z - p2[1])
    d = side(a, b), side(b, c), side(c, a)
    return not (min(d) < 0 < max(d))


def main(obj_path, pathing_path, msb_path, out_path, check=False):
    position, yaw = street_transform(msb_path)
    kinds = read_triangles(obj_path)
    pathing = json.load(open(pathing_path))
    road = kinds.get("Street", [])
    sigils = [(s["pos"][0] * 0.01, s["pos"][1] * 0.01) for s in pathing["sigils"]]
    if check:
        paths = {p["id"]: p for p in pathing["paths"]}
        for group in pathing["spawn_groups"]:
            if not group["name"].startswith(MOB_GROUPS):
                continue
            for entry in group["entries"]:
                for n in paths[entry["path_id"]]["nodes"]:
                    pos = pathing["nodes"][str(n)]["pos"]
                    q = (pos[0] * 0.01, pos[1] * 0.01)
                    on_road = any(inside(q, t) for t in road)
                    sigil = min(math.dist(q, s) for s in sigils)
                    print(f"{group['name']:16} node {n:4} road {on_road!s:5} nearest sigil {sigil:5.1f} m")
    lines = [f"# Unicorn Way road mask (tools/w101_road_mask.py): street {STREET} at {tuple(round(c, 3) for c in position)} "
             f"yaw {yaw:.3f}; {len(road)} road triangles, {len(sigils)} sigils"]
    corners = []
    for tri in road:
        pts = [to_map(position, yaw, x, z) for x, z in tri]
        corners += pts
        lines.append("R" + "".join(f" {c:.3f}" for p in pts for c in p))
    for x, z in sigils:
        mx, mz = to_map(position, yaw, x, z)
        lines.append(f"C {mx:.3f} {mz:.3f} {SIGIL_RADIUS}")
    xs, zs = [p[0] for p in corners], [p[1] for p in corners]
    lines.append(f"B {min(xs) - MARGIN:.3f} {min(zs) - MARGIN:.3f} {max(xs) + MARGIN:.3f} {max(zs) + MARGIN:.3f}")
    Path(out_path).write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"{out_path}: {len(road)} road triangles, {len(sigils)} sigils, box {lines[-1][2:]}")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--check"]
    main(*args, check="--check" in sys.argv)
