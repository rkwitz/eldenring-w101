"""Wizard101's Bone Cage (round 22 P2) as OBJs for blender_build_cage.py: closed and opened.

Wizard101: the four quest cages of Finding Out Why (Unicorn Way objects WC-FairyCage02..05, ObjectData
WC/WC-FairyCage02.xml) all use StateObjects/WC_BoneCage/HD_FX_LampCage.nif (WizardCity-WorldData.wad): a 4.3 m street
lamp post (WC_LampPost_20, texture HD_WC_LampPost_01_EM) with a bone cage (Fairy_Cage01, texture HD_WC_Cage01_JT)
hanging off its arm at model -y. The fairy inside is only particles (FX Mote Inside). StateData/FairyCageStates.xml:
interacting plays animation Escape (HD_FX_LampCage_Escape.kf: node CageDoor swings, FX_SparkleExit / FX_MoteExit
burst out, sound envgen_chestwood_open.ogg) and leaves the cage in EmptyCage (door open).

nif2obj.py gives the closed cage (the nif's rest pose). The .kf files use NiPathEvaluator blocks pyffi cannot read,
so the open door here is the door mesh (CageDoor's only mesh, OBJ object "Editable Poly", the cage's front panel at
OBJ +z) swung OPEN_DEGREES outwards about its left vertical edge.

Usage: python nif2obj.py HD_FX_LampCage.nif WC_BoneCage.obj
       python tex2dds.py <dir with HD_WC_Cage01_JT__a03a6661.nif, HD_WC_LampPost_01_EM__80172aec.nif> textures
       python w101_bone_cage.py WC_BoneCage.obj WC_BoneCage_open.obj
"""

import math
import sys
from pathlib import Path

DOOR = "Editable Poly"
OPEN_DEGREES = 110.0


def open_cage(src, dst):
    lines = Path(src).read_text().splitlines()
    door, current = set(), None
    for line in lines:
        if line.startswith("o "):
            current = line[2:].strip()
        elif line.startswith("f ") and current == DOOR:
            door |= {int(t.split("/")[0]) - 1 for t in line.split()[1:]}
    assert door, f"no {DOOR!r} object"
    vertices = [i for i, line in enumerate(lines) if line.startswith("v ")]
    points = {k: [float(c) for c in lines[vertices[k]].split()[1:4]] for k in door}
    hinge_x = min(p[0] for p in points.values())
    hinge_z = sum(p[2] for p in points.values()) / len(points)
    a = math.radians(OPEN_DEGREES)
    for k, (x, y, z) in points.items():
        dx, dz = x - hinge_x, z - hinge_z
        nx, nz = hinge_x + dx * math.cos(a) - dz * math.sin(a), hinge_z + dx * math.sin(a) + dz * math.cos(a)
        lines[vertices[k]] = f"v {nx:.6f} {y:.6f} {nz:.6f}"
    text = "\n".join(lines) + "\n"
    Path(dst).write_text(text.replace(f"mtllib {Path(src).stem}.mtl", f"mtllib {Path(dst).stem}.mtl"))
    Path(dst).with_suffix(".mtl").write_text(Path(src).with_suffix(".mtl").read_text())
    print(f"door: {len(door)} vertices swung {OPEN_DEGREES} deg about x {hinge_x:.3f}, z {hinge_z:.3f} -> {dst}")


if __name__ == "__main__":
    open_cage(sys.argv[1], sys.argv[2])
