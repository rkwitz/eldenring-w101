"""Give the Unicorn Way street (asset aeg030_518) its Wizard101 textures.

1. Splits the street FLVER in the Smithbox project's aeg030_518.geombnd.dcx into one material per W101 texture
   (flver_textures.py), using the OBJ it was imported from to tell which triangle had which texture. The first run
   keeps the single-material binder as aeg030_518.geombnd.dcx.before_textures and later runs start from that copy.
2. Packs the converted W101 textures into project\\asset\\aet\\aet030\\aet030_518.tpf.dcx (and a copy as the
   low-detail aet030_518_l.tpf.dcx), where the game finds the texture names the new materials use (tpf.py).

Run in Blender's Scripting tab; watch Window > Toggle System Console.
"""

import os
import importlib
import shutil
import sys
from pathlib import Path
try:  # config.toml at the repo root (tools/w101er_config.py), then the default
    from w101er_config import setting as _setting
except ImportError:
    _setting = os.environ.get

from soulstruct.containers import Binder
from soulstruct.dcx import compress, decompress

ROOT = Path(_setting("W101ER_ROOT", Path.home() / "Documents" / "game_modding"))
sys.path.insert(0, str(Path(os.environ.get("W101ER_TOOLS", ROOT / "eldenring-w101" / "tools"))))
import flver_fix  # noqa: E402
import flver_textures  # noqa: E402
import tpf  # noqa: E402

for module in (flver_fix, flver_textures, tpf):
    importlib.reload(module)

PROJECT = ROOT / "reference" / "eldenring" / "project"
GAME = Path(_setting("ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game"))
GEOMBND = PROJECT / "asset" / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"
VANILLA = GAME / "asset" / "aeg" / "aeg030" / "aeg030_518.geombnd.dcx"
AET = PROJECT / "asset" / "aet" / "aet030"
AET_TEMPLATE = GAME / "asset" / "aet" / "aet030" / "aet030_000.tpf.dcx"  # for its DCX type
CONVERTED = ROOT / "reference" / "w101" / "converted"
OBJ = CONVERTED / "WC_Z06_Unicorn_Way.obj"
MTL = CONVERTED / "WC_Z06_Unicorn_Way.mtl"

backup = GEOMBND.with_name(GEOMBND.name + ".before_textures")
if not backup.exists():
    shutil.copy2(GEOMBND, backup)
source = flver_fix.flver_in_binder(backup)
vanilla = flver_fix.flver_in_binder(VANILLA)

stems = sorted(set(flver_textures.parse_mtl(MTL).values()))
dds = [(CONVERTED / "textures" / f"{stem}.dds").read_bytes() for stem in stems]
alpha = {i for i, data in enumerate(dds) if tpf.has_alpha(data)}
print(f"{len(stems)} textures, {len(alpha)} with transparency")

positions, faces = flver_textures.parse_obj(OBJ)
materials, missing = flver_textures.obj_face_materials(source, positions, faces)
texture_of = {name: stems.index(stem) for name, stem in flver_textures.parse_mtl(MTL).items()}
face_textures = [texture_of.get(m) for m in materials]
print(f"Matched {len(materials) - missing} of {len(materials)} triangles to the OBJ ({missing} took a neighbour's)")

split = flver_textures.split_materials(source, vanilla, face_textures, stems, alpha)
binder = Binder.from_path(backup)
entry = next(e for e in binder.entries if e.name.lower().endswith(".flver"))
entry.set_uncompressed_data(split)
binder.write(GEOMBND)
print(f"Wrote {GEOMBND} with {flver_fix._counts(split)[1]['material']} materials")

textures = [(flver_textures.texture_name(i), data) for i, data in enumerate(dds)]
textures.append((flver_textures.FLAT_NORMAL, tpf.solid_dds(128, 128, 64)))
_, dcx_type = decompress(AET_TEMPLATE)
packed = compress(tpf.build_tpf(textures), dcx_type)
AET.mkdir(parents=True, exist_ok=True)
for name in ("aet030_518.tpf.dcx", "aet030_518_l.tpf.dcx"):
    (AET / name).write_bytes(packed)
print(f"Wrote {len(textures)} textures to {AET / 'aet030_518.tpf.dcx'} (and _l), {dcx_type.name}")
