"""Decompress or compress a DCX file (Oodle KRAK compression only works through Soulstruct on Ryan's PC).

MODE = "unpack": write SOURCE's contents uncompressed to TARGET.
MODE = "pack": compress SOURCE into TARGET with the DCX type of REFERENCE.
Other scripts set MODE, SOURCE, TARGET and REFERENCE through runpy's init_globals; the defaults dump the Stranded
Graveyard map from the Smithbox project to reference/eldenring/work/dump.
"""

from pathlib import Path

from soulstruct.dcx import compress, decompress

ROOT = Path.home() / "Documents" / "game_modding"
PROJECT_MAP = ROOT / "reference" / "eldenring" / "project" / "map" / "mapstudio" / "m18_00_00_00.msb.dcx"

MODE = globals().get("MODE", "unpack")
SOURCE = Path(globals().get("SOURCE", PROJECT_MAP))
TARGET = Path(globals().get("TARGET", ROOT / "reference" / "eldenring" / "work" / "dump" / "m18_00_00_00.msb"))
REFERENCE = Path(globals().get("REFERENCE", PROJECT_MAP))

if MODE == "unpack":
    data, dcx_type = decompress(SOURCE)
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    TARGET.write_bytes(data)
    print(f"Unpacked {SOURCE.name} ({dcx_type.name}) to {TARGET} ({len(data)} bytes)")
elif MODE == "pack":
    _, dcx_type = decompress(REFERENCE)
    TARGET.write_bytes(compress(SOURCE.read_bytes(), dcx_type))
    print(f"Packed {SOURCE.name} into {TARGET} ({dcx_type.name})")
else:
    raise ValueError(f"Unknown MODE {MODE!r}")
