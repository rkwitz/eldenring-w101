"""Every Wizard101 model, texture and data file the build converts, straight from the WADs (round 35, R-BUILD).

build.py runs this for its "w101" step. For each model the .nif is read out of its WAD and converted with
nif2obj.py. The textures its MTL names (Textures/<stem>.nif) are searched in TEXTURE_WADS and converted with
tex2dds.py. After that come the classic street's normals, window states and quest window, the open Bone Cage, the
mob paths and the quest text dump. Every OBJ matches the author's files (ignoring nif2obj's later "#vc" lines).

Usage: python w101_convert.py <Wizard101 Data\\GameData> <reference\\w101> [--only NAME ...]
"""
import argparse
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from w101_sfx import entries  # noqa: E402

ZONE = "WizardCity-WC_Streets-WC_Unicorn.wad"
CLASSIC_ZONE = "ClassicMode-WC_Streets-WC_Unicorn.wad"
CLASSIC_DATA = "ClassicMode-WorldData.wad"
WORLD_DATA = "WizardCity-WorldData.wad"
CLASSIC_MAZE = "ClassicMode-WC_Streets-Interiors-WC_Unicorn_HedgeMaze.wad"

# name: (output OBJ under reference\w101, WAD, entry)
MODELS = {
    "street_hd": ("converted/WC_Z06_Unicorn_Way.obj", ZONE, "WC_Z06_Unicorn_Way.nif"),
    "street": ("converted_classic/WC_Z06_Unicorn_Way_Classic.obj", CLASSIC_ZONE, "WC_Z06_Unicorn_Way.nif"),
    "room": ("converted_rattlebones_classic/WC_Generic_Gauntlet_NoDoor_Classic.obj", CLASSIC_DATA,
             "WC_Generic_Gauntlet_NoDoor.nif"),
    "sky1": ("converted_sky_classic/layer1.obj", CLASSIC_DATA, "Skyboxes/HD_WC_Hub_Skybox_01_Layer1_NK.nif"),
    "sky2": ("converted_sky_classic/layer2.obj", CLASSIC_DATA, "Skyboxes/HD_WC_Hub_Skybox_01_Layer2_NK.nif"),
    "cage": ("converted_bonecage/WC_BoneCage.obj", WORLD_DATA, "StateObjects/WC_BoneCage/HD_FX_LampCage.nif"),
    "clubhouse": ("converted_clubhouse_hd/WC_Unicorn_H4_ClubHouse.obj", WORLD_DATA, "SP_Z00I01_ClubHouse.nif"),
    "maze": ("converted_hedgemaze_classic/WC_Unicorn_HedgeMaze_Classic.obj", CLASSIC_MAZE,
             "WC_Z06I02_Life_Hedge_Maze.nif"),
}
LANG_TABLES = "WizQst.*|WizardShops|QuestTitle|Quest|WizardQuestGoals|WC-NPCs|WizardMobs|WizardZone|Interactables|CinematicDialogs"
TEXTURE_WADS = [CLASSIC_ZONE, ZONE, CLASSIC_MAZE, CLASSIC_DATA, WORLD_DATA, "Root.wad"]
# md5 of each output without "#vc" lines (the author's files, 2026-11-05)
EXPECTED = {
    "converted/WC_Z06_Unicorn_Way.obj": "90d13a09", "converted_classic/WC_Z06_Unicorn_Way_Classic.obj": "d5d8cad2",
    "converted_rattlebones_classic/WC_Generic_Gauntlet_NoDoor_Classic.obj": "44c31735",
    "converted_sky_classic/layer1.obj": "e4ea8920", "converted_sky_classic/layer2.obj": "36a377aa",
    "converted_bonecage/WC_BoneCage.obj": "46e548ac", "converted_bonecage/WC_BoneCage_open.obj": "c23e43fd",
    "converted_clubhouse_hd/WC_Unicorn_H4_ClubHouse.obj": "bd109e08",
    "converted_hedgemaze_classic/WC_Unicorn_HedgeMaze_Classic.obj": "77ca8eb6",
    "converted_quest_window/WC_Unicorn_T2_quest.obj": "d0acac4c",
    "converted_classic/WC_Z06_Unicorn_Way_Classic.normals": "2e87d8d1",
    "converted_classic/window_states.tsv": "3440f3e7",
    "converted_classic/unicorn_way_pathing.json": "c80a0277",
}


class Wads:
    def __init__(self, game_data):
        self.root, self.index = Path(game_data), {}

    def entries(self, wad):
        if wad not in self.index:
            self.index[wad] = {name: rest for name, *rest in entries(self.root / wad)}
        return self.index[wad]

    def read(self, wad, name):
        """The entry's bytes, or None when it is missing or not downloaded yet (all zeros)."""
        info = self.entries(wad).get(name)
        if info is None:
            return None
        offset, size, stored, compressed = info
        with open(self.root / wad, "rb") as f:
            f.seek(offset)
            raw = f.read(stored if compressed else size)
        if not raw.strip(b"\0"):
            return None
        return zlib.decompress(raw) if compressed else raw


def py(*args):
    subprocess.run([sys.executable, *map(str, args)], check=True, stdout=subprocess.DEVNULL)


def model(wads, ref, name, tmp):
    out, wad, entry = MODELS[name]
    out = ref / out
    out.parent.mkdir(parents=True, exist_ok=True)
    data = wads.read(wad, entry)
    if data is None:
        raise SystemExit(f"{wad}: {entry} is missing or not downloaded (start Wizard101 in Classic Mode and visit it)")
    nif = tmp / f"{name}.nif"
    nif.write_bytes(data)
    py(HERE / "nif2obj.py", nif, out)
    stems = sorted({line.split("/")[-1].strip()[:-4] for line in out.with_suffix(".mtl").read_text().splitlines()
                    if line.startswith("map_Kd ")})
    texnif = tmp / f"{name}_textures"
    texnif.mkdir()
    missing = []
    for stem in stems:
        data = next((d for d in (wads.read(w, f"Textures/{stem}.nif") for w in TEXTURE_WADS) if d), None)
        if data is None:
            missing.append(stem)
        else:
            (texnif / f"{stem}.nif").write_bytes(data)
    py(HERE / "tex2dds.py", texnif, out.parent / "textures")
    print(f"  {out.relative_to(ref)}: {len(stems) - len(missing)} textures" +
          (f", missing {missing}" if missing else ""))
    return nif


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("game_data")
    ap.add_argument("ref")
    ap.add_argument("--only", nargs="*")
    a = ap.parse_args(argv)
    wads, ref = Wads(a.game_data), Path(a.ref)
    only = set(a.only or list(MODELS) + ["extras", "pathing", "lang"])
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        nifs = {name: model(wads, ref, name, tmp) for name in MODELS if name in only}
        if "extras" in only:
            street = nifs.get("street") or model(wads, ref, "street", tmp)
            classic = ref / "converted_classic"
            py(HERE / "nif_normals.py", street, classic / "WC_Z06_Unicorn_Way_Classic.normals")
            py(HERE / "w101_window_states.py", street, classic / "window_states.tsv")
            (ref / "converted_quest_window").mkdir(parents=True, exist_ok=True)
            py(HERE / "w101_quest_window.py", street, "WC_Unicorn_T2_quest",
               ref / "converted_quest_window" / "WC_Unicorn_T2_quest.obj")
            cage = ref / "converted_bonecage"
            if not (cage / "WC_BoneCage.obj").exists():
                model(wads, ref, "cage", tmp)
            py(HERE / "w101_bone_cage.py", cage / "WC_BoneCage.obj", cage / "WC_BoneCage_open.obj")
            print("  normals, window states, quest window, open cage")
        if "pathing" in only:
            manifest = tmp / "root"
            manifest.mkdir()
            (manifest / "TemplateManifest.xml").write_bytes(wads.read("Root.wad", "TemplateManifest.xml"))
            r = subprocess.run([sys.executable, str(HERE / "w101_pathing.py"), "--wad", str(wads.root / ZONE),
                                "--wad", str(wads.root / CLASSIC_ZONE),
                                "--obj", str(ref / "converted_classic" / "WC_Z06_Unicorn_Way_Classic.obj"),
                                "--out", str(ref / "converted_classic"), "--root", str(manifest)],
                               capture_output=True, text=True)
            if not (ref / "converted_classic" / "unicorn_way_pathing.json").exists():
                raise SystemExit(r.stdout[-2000:] + r.stderr[-2000:])
            print("  unicorn_way_pathing.json" + ("" if r.returncode == 0 else " (its picture needs matplotlib)"))
        if "lang" in only:
            out = ref / "extracted" / "quests" / "uw_lang.json"
            out.parent.mkdir(parents=True, exist_ok=True)
            py("-I", HERE / "w101_lang.py", "dump", wads.root / "Root.wad", out, LANG_TABLES)
            print(f"  {out.relative_to(ref)}")


if __name__ == "__main__":
    main()
