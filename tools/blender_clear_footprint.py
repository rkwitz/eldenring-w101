"""Hide all Elden Ring ground, rocks and plants under and around the Unicorn Way street, in every map that draws them.

Run in Blender's Scripting tab after changing the street (it only uses Soulstruct to read the game's Oodle maps).
It computes the street's footprint from WC_Z06_Unicorn_Way_Classic_edited.obj (or the unedited OBJ) and the street
part in the project's m60_42_36_00, then, for every close-up (_00) and far-view (_01, _02) map over that footprint,
moves the parts footprint_clear.py picks 1000 m down. Project maps are edited in place (first backup kept as
.before_footprint); maps the project doesn't have yet are made from the game's own. A list of everything moved is
written to project/map/mapstudio/footprint_clear.txt. Safe to rerun. Reload the maps in Smithbox before saving them.
"""

import os
import importlib
import struct
import sys
import zlib
from pathlib import Path
try:  # config.toml at the repo root (tools/w101er_config.py), then the default
    from w101er_config import setting as _setting
except ImportError:
    _setting = os.environ.get

from soulstruct.dcx import decompress

ROOT = Path(_setting("W101ER_ROOT", Path.home() / "Documents" / "game_modding"))
sys.path.insert(0, str(Path(os.environ.get("W101ER_TOOLS", ROOT / "eldenring-w101" / "tools"))))
import msb_floor  # noqa: E402
import footprint_clear  # noqa: E402

for module in (msb_floor, footprint_clear):
    importlib.reload(module)

GAME = Path(_setting("ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game")) / "map" / "mapstudio"
PROJECT = ROOT / "reference" / "eldenring" / "project" / "map" / "mapstudio"
CLASSIC = ROOT / "reference" / "w101" / "converted_classic"
OBJ = next(p for p in (CLASSIC / "WC_Z06_Unicorn_Way_Classic_edited.obj", CLASSIC / "WC_Z06_Unicorn_Way_Classic.obj")
           if p.exists())
HOME = PROJECT / "m60_42_36_00.msb.dcx"


def read(path):
    data = path.read_bytes()
    if data[:4] == b"DCX\0" and data[0x28:0x2C] == b"DFLT":
        return zlib.decompress(data[0x4C:])
    return decompress(path)[0]


def write_dflt(path, raw, header):
    packed = zlib.compress(raw, 9)
    head = bytearray(header)
    struct.pack_into(">II", head, 0x1C, len(raw), len(packed))
    path.write_bytes(bytes(head) + packed)


header = HOME.read_bytes()[:0x4C]  # a DFLT DCX header the game accepts
position, rotation = footprint_clear.street_transform(read(HOME))
print(f"Street footprint from {OBJ.name} ...")
footprint = footprint_clear.Footprint(*footprint_clear.parse_obj(OBJ), position, rotation)
small = footprint.small_tiles()
lines = []
for name in footprint_clear.tiles_to_edit(small):
    target = PROJECT / f"{name}.msb.dcx"
    source = target if target.exists() else GAME / f"{name}.msb.dcx"
    if not source.exists():
        print(f"{name}: no such map, skipped")
        continue
    raw, report = footprint_clear.clear(name, read(source), footprint, small)
    lines += report
    if raw is None:
        print(f"{name}: nothing to move")
        continue
    backup = target.with_name(target.name + ".before_footprint")
    if target.exists() and not backup.exists():
        backup.write_bytes(target.read_bytes())
    write_dflt(target, raw, header)
    print(f"{name}: moved {len(report)} parts down ({'project' if source == target else 'new from game'})")
(PROJECT / "footprint_clear.txt").write_text("\n".join(lines) + "\n", newline="\n")
print(f"Done: {len(lines)} parts moved. List in {PROJECT / 'footprint_clear.txt'}. Relaunch the game.")
