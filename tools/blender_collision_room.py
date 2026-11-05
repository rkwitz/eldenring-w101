"""Make room in the asset collision binder for Smithbox's collision generator, or trim it again afterwards.

Smithbox 2.2.5 re-serializes a collision HKX into a fixed-size buffer the size of the original file
(HavokBank.SaveHavokFile_HKX3: `new MemoryStream(file.Bytes.ToArray())`), so saving a generated shape bigger than the
template's collision fails with "Memory stream is not expandable".

MODE = "pad": before generating, append zeros to the HKX inside the project's aeg030_518_l.geomhkxbnd so the
    buffer is big enough. Restart Smithbox afterwards so it reloads the file.
MODE = "trim": after saving in Smithbox, cut the HKX back to the length its TAG0 header declares.

Run in Blender's Scripting tab (Soulstruct handles the Oodle compression).
"""

import os
import shutil
import struct
from pathlib import Path
try:  # config.toml at the repo root (tools/w101er_config.py), then the default
    from w101er_config import setting as _setting
except ImportError:
    _setting = os.environ.get

from soulstruct.containers import Binder

MODE = globals().get("MODE", "pad")  # blender_collision_trim.py runs this with "trim"
PAD_TO = 8 * 1024 * 1024  # bytes; ~15k triangles need roughly 1 MB

ROOT = Path(_setting("W101ER_ROOT", Path.home() / "Documents" / "game_modding"))
NAME = "aeg030_518_l.geomhkxbnd.dcx"
PROJECT = ROOT / "reference" / "eldenring" / "project" / "asset" / "aeg" / "aeg030" / NAME
GAME = Path(_setting("ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game")) / r"asset\aeg\aeg030" / NAME


def tag_length(data):
    size = struct.unpack_from(">I", data, 0)[0] & 0x3FFFFFFF
    if data[4:8] != b"TAG0" or not 8 < size <= len(data):
        raise ValueError(f"Not a Havok TAG0 file (header {data[:8]!r}).")
    return size


if not PROJECT.exists():
    shutil.copy2(GAME, PROJECT)

binder = Binder.from_path(PROJECT)
for entry in binder.entries:
    if not entry.name.lower().endswith(".hkx"):
        continue
    data = entry.get_uncompressed_data()
    length = tag_length(data)
    if MODE == "pad":
        new = data[:length] + bytes(max(0, PAD_TO - length))
    elif MODE == "trim":
        new = data[:length]
    else:
        raise ValueError(f"Unknown MODE {MODE!r}")
    entry.set_uncompressed_data(new)
    print(f"{entry.name}: {len(data)} -> {len(new)} bytes (Havok data {length} bytes)")

binder.write(force=True)
print(f"Wrote {PROJECT}")
