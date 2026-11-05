"""Build Elden Ring on Unicorn Way from your own Wizard101 and Elden Ring installs, in one run.

  python build.py                    every step, skipping what is up to date
  python build.py --dry-run          show what would run
  python build.py --only maps        one step (or several: --only maps dll)
  python build.py --from blender     that step and every one after it
  python build.py --force            redo steps even when their outputs are up to date
  python build.py --list             the steps

Paths: a flag, else the environment variable, else config.toml (copy config.example.toml), else the default.

| Flag | Variable | config.toml | Default |
| --- | --- | --- | --- |
| --root | W101ER_ROOT | work_root | %USERPROFILE%\\Documents\\game_modding (build.py creates everything under it) |
| --game | ER_GAME | game | Steam's ELDEN RING\\Game, unpacked with UXM Selective Unpacker |
| --w101 | W101_INSTALL | wizard101 | C:\\ProgramData\\KingsIsle Entertainment\\Wizard101 |
| --blender | BLENDER | blender | found under Program Files |
| --smithbox | SMITHBOX | smithbox | <work root>\\tools\\Smithbox |
| --modengine | MODENGINE | modengine | <work root>\\tools\\ModEngine |

See README.md, and docs/BUILD_DETAILS.md for what each step does.
"""
import argparse
import glob
import hashlib
import importlib.util
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent
TOOLS = REPO / "tools"
sys.path.insert(0, str(TOOLS))
from w101er_config import CONFIG, setting  # noqa: E402
PIP = {"numpy": "numpy", "PIL": "pillow", "pyffi": "pyffi==2.2.3", "scipy": "scipy", "cryptography": "cryptography",
       "zstandard": "zstandard", "ooz": "pyooz"}
W101_WADS = ["WizardCity-WC_Streets-WC_Unicorn.wad", "ClassicMode-WC_Streets-WC_Unicorn.wad", "ClassicMode-WorldData.wad",
             "WizardCity-WorldData.wad", "ClassicMode-WC_Streets-Interiors-WC_Unicorn_HedgeMaze.wad", "Root.wad",
             "_Shared-WorldData.wad", "GUI-WorldData.wad", "Sound-WorldData.wad", "Music-WorldData.wad",
             "Mob-WorldData.wad"]
UXM_FILES = ["regulation.bin", "map/mapstudio/m60_42_36_00.msb.dcx", "map/mapstudio/m18_00_00_00.msb.dcx",
             "event/m60_42_36_00.emevd.dcx", "event/common.emevd.dcx", "menu/01_000_fe.gfx", "msg/engus/item_dlc02.msgbnd.dcx",
             "chr/c4960.anibnd.dcx", "oo2core_6_win64.dll"]
# Deterministic outputs and their md5 (first 8 hex digits) in the author's build of 2026-11-05. A mismatch can mean
# a newer game version, so it is reported, not fatal.
KNOWN = {
    "reference/w101/converted_classic/WC_Z06_Unicorn_Way_Classic_edited.obj": "f6aab2eb",
    "reference/w101/converted_classic/graveyard_fix.tsv": "56798ca2",
    "reference/eldenring/project/regulation.bin": "04d5b270",
    "reference/eldenring/mod/menu/01_000_fe.gfx": "9a420b8b",
    "reference/eldenring/mod/menu/01_001_fe_soul.gfx": "abcb1fab",
    "reference/eldenring/project/event/m60_42_36_00.emevd.dcx": "b19bcc3a",
    "reference/eldenring/project/map/mapstudio/m60_42_36_00.msb.dcx": "d4a5029e",
    "reference/eldenring/orbs/w101_orbs/book/quests.tsv": "9096ca8f",
    "reference/eldenring/orbs/w101_orbs/book/quest_nav.tsv": "8c93b664",
    "reference/eldenring/orbs/w101_orbs/book/street_sight.bin": "4598d7eb",
    "reference/eldenring/orbs/w101_orbs/road_mask.tsv": "c9306eb1",
    "reference/eldenring/orbs/w101_orbs/book/dialogue.tsv": "e9406a1b",
    "reference/eldenring/orbs/w101_orbs/book/voice.tsv": "cbdad56d",
    "reference/eldenring/orbs/w101_orbs/book/page_flip.tsv": "f2ae70f9",
    "reference/eldenring/orbs/w101_orbs/er_item_names.tsv": "feb01bd6",
    "reference/eldenring/orbs/w101_orbs/book/er/er_recipes.tsv": "f0d718fc",
    "reference/eldenring/orbs/w101_orbs/book/er/shop.tsv": "93693831",
    "reference/eldenring/orbs/w101_orbs/book/er/idle_pose.tsv": "d738572a",
}


class Build:
    def __init__(self, a):
        self.a = a
        self.root = Path(a.root or setting("W101ER_ROOT", Path.home() / "Documents" / "game_modding"))
        self.game = Path(a.game or setting(
            "ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game"))
        self.w101 = Path(a.w101 or setting("W101_INSTALL", r"C:\ProgramData\KingsIsle Entertainment\Wizard101"))
        self.gd = self.w101 / "Data" / "GameData"
        self.ref = self.root / "reference"
        self.w = self.ref / "w101"
        self.e = self.ref / "eldenring"
        self.project = self.e / "project"
        self.orbs = self.e / "orbs" / "w101_orbs"
        self.unpacked = self.e / "unpacked"
        self.smithbox = Path(a.smithbox or setting("SMITHBOX", self.root / "tools" / "Smithbox"))
        self.defs = self.smithbox / "Assets" / "PARAM" / "ER" / "Defs"
        self.modengine = Path(a.modengine or setting("MODENGINE", self.root / "tools" / "ModEngine"))
        self.blender = a.blender or setting("BLENDER") or find_blender()
        self.env = dict(os.environ, W101ER_ROOT=str(self.root), ER_GAME=str(self.game), W101ER_TOOLS=str(TOOLS))
        self.results = []

    # -- helpers -----------------------------------------------------------------------------------------------------
    def run(self, *args, cwd=None):
        args = [str(x) for x in args]
        shown = " ".join(f'"{x}"' if " " in x else x for x in args)
        print(f"    $ {shown}")
        if self.a.dry_run:
            return
        subprocess.run(args, check=True, cwd=cwd, env=self.env)

    def py(self, script, *args):
        self.run(sys.executable, TOOLS / script, *args)

    def blend(self, script, *args):
        if not self.blender:
            raise StepError("Blender not found: pass --blender path\\to\\blender.exe or set BLENDER")
        self.run(self.blender, "--background", "--python", TOOLS / "blender_headless.py", "--", script, *args)

    def fresh(self, outputs, inputs=()):
        """True when every output exists and is newer than every input (so the step can be skipped)."""
        if self.a.force:
            return False
        outs = [Path(o) for o in outputs]
        if not outs or not all(o.exists() for o in outs):
            return False
        newest = max((Path(i).stat().st_mtime for i in inputs if Path(i).exists()), default=0)
        return min(o.stat().st_mtime for o in outs) >= newest

    # -- steps -------------------------------------------------------------------------------------------------------
    def check(self):
        problems = []
        missing = [m for m, _ in PIP.items() if importlib.util.find_spec(m) is None]
        if missing:
            print(f"    installing Python packages: {[PIP[m] for m in missing]}")
            self.run(sys.executable, "-m", "pip", "install", *[PIP[m] for m in missing])
        if not self.gd.exists():
            problems.append(f"Wizard101 not found at {self.w101} (pass --w101)")
        else:
            for wad in W101_WADS:
                if not (self.gd / wad).exists():
                    problems.append(f"Wizard101: {wad} missing. Turn on Classic Mode (Settings), then walk onto "
                                    "Unicorn Way and into the hedge maze once, so the client downloads it")
        for rel in UXM_FILES:
            if not (self.game / rel).exists():
                problems.append(f"Elden Ring: {self.game / rel} missing. Unpack the game with UXM Selective Unpacker "
                                "(all of asset, chr, event, map, material, menu, msg, param, sfx)")
                break
        if not self.defs.exists():
            problems.append(f"Smithbox paramdefs not found at {self.defs} (pass --smithbox)")
        if not self.blender:
            problems.append("Blender not found (pass --blender or set BLENDER); the blender and unpack steps need it")
        if shutil.which("cargo") is None:
            problems.append("Rust not found: install rustup, then `rustup target add x86_64-pc-windows-gnu`")
        else:
            targets = subprocess.run(["rustup", "target", "list", "--installed"], capture_output=True, text=True).stdout
            if "x86_64-pc-windows-gnu" not in targets:
                problems.append("Rust target missing: `rustup target add x86_64-pc-windows-gnu`")
            linker = "gcc" if platform.system() == "Windows" else "x86_64-w64-mingw32-gcc"
            if shutil.which(linker) is None:
                problems.append(f"MinGW-w64 not found ({linker}): install it (MSYS2 on Windows) for the DLL")
        if not (self.modengine / "modengine2_launcher.exe").exists() and not (self.modengine / "launchmod_eldenring.bat").exists():
            problems.append(f"Mod Engine 2 not found at {self.modengine} (pass --modengine)")
        for p in problems:
            print(f"    ! {p}")
        if problems and not self.a.keep_going:
            raise StepError(f"{len(problems)} prerequisite(s) missing (--keep-going runs anyway)")
        return f"{len(problems)} problem(s), going on (--keep-going)" if problems else "all present"

    def folders(self):
        """Every folder the steps write into, under the work root (the full tree is in docs/BUILD_DETAILS.md)."""
        return [self.project / "map" / "mapstudio", self.project / "event", self.project / "msg" / "engus",
                self.project / "menu", self.e / "mod" / "menu", self.orbs / "book" / "er", self.orbs / "book" / "art",
                self.unpacked, self.e / "work", self.w / "extracted" / "quests", self.w / "converted_classic"]

    def project_folder(self):
        # main() has created the folders
        pj = self.project / "project.json"
        if not pj.exists():
            print(f"    writing {pj}")
            if not self.a.dry_run:
                pj.write_text('{\n  "ProjectName": "eldenring-w101",\n  "GameRoot": ' + jstr(self.game) +
                              ',\n  "GameType": 8,\n  "PinnedParams": [],\n  "PinnedRows": {},\n  "PinnedFields": {},\n'
                              '  "UseLooseParams": false,\n  "PartialParams": false,\n  "LastFmgLanguageUsed": "engus"\n}\n')
        vanilla = self.project / "regulation.bin.vanilla"
        if not vanilla.exists():
            print(f"    copying the game's regulation.bin to {vanilla}")
            if not self.a.dry_run:
                shutil.copy2(self.game / "regulation.bin", vanilla)

    def w101_files(self):
        out = self.w / "converted_classic" / "unicorn_way_pathing.json"
        if self.fresh([out, self.w / "extracted" / "quests" / "uw_lang.json"], [TOOLS / "w101_convert.py"]):
            return "up to date"
        self.py("w101_convert.py", self.gd, self.w)

    def street(self):
        classic = self.w / "converted_classic"
        edited = classic / "WC_Z06_Unicorn_Way_Classic_edited.obj"
        patch = REPO / "patches" / "street" / "WC_Z06_Unicorn_Way_Classic_edited.objpatch.gz"
        if not self.fresh([edited], [patch, classic / "WC_Z06_Unicorn_Way_Classic.obj"]):
            self.py("obj_patch.py", "apply", patch, edited, classic / "WC_Z06_Unicorn_Way_Classic.obj",
                    self.w / "converted" / "WC_Z06_Unicorn_Way.obj")
            if not self.a.dry_run:
                shutil.copy2(classic / "WC_Z06_Unicorn_Way_Classic.mtl", classic / "WC_Z06_Unicorn_Way_Classic_edited.mtl")
        fix = classic / "graveyard_fix.tsv"
        if not self.fresh([fix], [edited, TOOLS / "street_graveyard_fix.py"]):
            self.py("street_graveyard_fix.py", edited, self.game, self.game / "map" / "mapstudio" / "m18_00_00_00.msb.dcx", fix)

    def unpack(self):
        if self.fresh([self.unpacked / "msg" / "engus" / "item_dlc02.msgbnd", self.unpacked / "msg" / "engus" / "menu_dlc02.msgbnd"],
                      [TOOLS / "blender_unpack.py"]):
            return "up to date"
        self.blend("blender_unpack.py")

    def blender_steps(self):
        import ast
        tree = ast.parse((TOOLS / "blender_build_all.py").read_text(encoding="utf-8"))
        steps = next(ast.literal_eval(n.value) for n in tree.body
                     if isinstance(n, ast.Assign) and getattr(n.targets[0], "id", "") == "STEPS")
        for name, script in steps:
            if self.a.skip_blender and name in self.a.skip_blender:
                continue
            print(f"   {name}: {script}")
            self.blend(script)

    def regulation(self):
        out = self.project / "regulation.bin"
        if self.fresh([out], [self.project / "regulation.bin.vanilla", TOOLS / "er_regulation.py"]):
            return "up to date"
        self.py("er_regulation.py", "u6b", self.project / "regulation.bin.vanilla", self.defs, out)

    def menus(self):
        menu = self.e / "mod" / "menu"
        if self.fresh([menu / "01_000_fe.gfx", menu / "01_001_fe_soul.gfx"], [TOOLS / "er_hud_hide_bars.py"]):
            return "up to date"
        self.py("er_hud_hide_bars.py", self.game / "menu", menu)

    def text(self):
        msg, src = self.project / "msg" / "engus", self.unpacked / "msg" / "engus"
        if not self.a.dry_run:
            msg.mkdir(parents=True, exist_ok=True)
        dialogue = self.e / "work" / "npc_dialogue.json"
        header = self.project / "event" / "m60_42_36_00.emevd.dcx"  # any DFLT .dcx of the project gives the header
        for name in ("item", "item_dlc02"):
            out, tmp = msg / f"{name}.msgbnd.dcx", msg / f"{name}.msgbnd.dcx.tmp"
            if self.fresh([out], [src / f"{name}.msgbnd", TOOLS / "er_text.py"]):
                continue
            tmp2 = tmp.with_suffix(".tmp2")
            self.py("er_text.py", src / f"{name}.msgbnd", tmp)  # PlaceName 610900 "Unicorn Way"
            self.py("er_text.py", "--npc-names", tmp, tmp2)  # the boss bars' names
            if not self.a.dry_run:
                sys.path.insert(0, str(TOOLS))
                import msb_patch
                raw = tmp2.read_bytes()
                out.write_bytes(raw if raw[:4] == b"DCX\0" else msb_patch.dflt(raw))
                tmp.unlink()
                tmp2.unlink()
        for name in ("menu", "menu_dlc02"):
            out = msg / f"{name}.msgbnd.dcx"
            if self.fresh([out], [src / f"{name}.msgbnd", dialogue, TOOLS / "er_text.py"]):
                continue
            self.py("er_text.py", "--dialogue", dialogue, src / f"{name}.msgbnd", out, header)

    def data(self):
        return self.orbs_data(part="data")

    def orbs_art(self):
        return self.orbs_data(part="art")

    def orbs_data(self, part):
        gd, o, b = self.gd, self.orbs, self.orbs / "book"
        work = self.e / "work"
        reg = self.project / "regulation.bin"
        item = self.game / "msg" / "engus" / "item_dlc02.msgbnd.dcx"
        lang = self.w / "extracted" / "quests" / "uw_lang.json"
        classic = self.w / "converted_classic"
        jobs = [
            ([o / "Globe_Health.png"], ["w101_hud_art.py", gd, o]),
            ([b / "index.txt"], ["w101_spellbook_art.py", gd, b]),
            ([b / "MapSpellbookPage.lay"], ["w101_book_layout.py", gd, b]),
            ([b / "art" / "Quest"], ["-I", "w101_quest_art.py", gd, b]),
            ([b / "Quest" / "WC_QuestArrow.tsv"], ["-I", "w101_quest_arrow.py", gd, b]),
            ([b / "art" / "GUI" / "Maps" / "Map_WC_Unicorn_Way.png"], ["-I", "w101_map_art.py", gd, b / "art"]),
            ([b / "art" / "teleport" / "fx_disappear.tsv"], ["w101_teleport_art.py", gd, b / "art" / "teleport"]),
            ([o / "wisps"], ["-I", "w101_wisp_art.py", gd, o]),
            ([b / "page_flip.tsv"], ["w101_page_flip.py", gd, b / "page_flip.tsv"]),
            ([o / "music" / "WC_HauntedWay_Combat.ogg"], ["w101_music.py", gd, o / "music"]),
            ([o / "sfx" / "HudSelect.wav"], ["-I", "w101_sfx.py", gd, o / "sfx"]),
            ([o / "voice"], ["-I", "w101_voice.py", gd, o / "voice"]),
            ([b / "art" / "GUI" / "Art" / "Art_Key_Blank.png"], ["w101_prompt_glyphs.py", b / "art" / "GUI" / "Art"]),
            ([o / "er_item_names.tsv"], ["er_item_names.py", self.unpacked / "msg" / "engus" / "item_dlc02.msgbnd",
                                         o / "er_item_names.tsv"]),
            ([b / "er" / "er_item_stats.tsv"], ["er_book_data.py", "stats", reg, self.defs, item, b / "er"]),
            ([b / "er" / "icons"], ["er_book_data.py", "icons", self.game / "menu" / "hi" / "01_common.tpf.dcx",
                                    self.game / "menu" / "hi" / "01_common.sblytbnd.dcx", b / "er"]),
            ([b / "er" / "er_recipes.tsv"], ["er_recipes.py", reg, self.defs, o / "er_item_names.tsv",
                                             b / "er" / "er_item_text.tsv", b / "er" / "er_recipes.tsv"]),
            ([b / "er" / "shop.tsv"], ["w101_shop.py", reg, self.defs, o / "er_item_names.tsv", b / "er" / "shop.tsv"]),
            ([b / "er" / "idle_pose.tsv"], ["er_idle_pose.py", self.game / "chr", b / "er" / "idle_pose.tsv"]),
            ([work / "npc_dialogue.json"], ["-I", "w101_npc_dialogue.py", lang, work / "npc_dialogue.json", b / "voice.tsv"]),
            ([b / "dialogue.tsv"], ["-I", "w101_book_dialogue.py", work / "npc_dialogue.json", b / "dialogue.tsv"]),
            ([work / "unicorn_way_quests.json"], ["-I", "w101_quests_json.py", lang, work / "unicorn_way_quests.json"]),
            ([b / "quests.tsv"], ["w101_book_quests.py", work / "unicorn_way_quests.json", b / "quests.tsv"]),
            ([b / "quest_nav.tsv"], ["w101_quest_nav.py", classic / "unicorn_way_pathing.json", b / "quest_nav.tsv"]),
            ([o / "road_mask.tsv"], ["w101_road_mask.py", classic / "WC_Z06_Unicorn_Way_Classic.obj",
                                     classic / "unicorn_way_pathing.json",
                                     self.project / "map" / "mapstudio" / "m60_42_36_00.msb.dcx", o / "road_mask.tsv"]),
            ([b / "street_sight.bin"], ["-I", "w101_street_sight.py", classic / "WC_Z06_Unicorn_Way_Classic_edited.obj",
                                        b / "quest_nav.tsv", b / "street_sight.bin"]),
        ]
        if not self.a.dry_run:
            for d in (o, b, b / "er", work):
                d.mkdir(parents=True, exist_ok=True)
        failed = []
        data_scripts = {'er_item_names.py', 'er_book_data.py', 'er_recipes.py', 'w101_shop.py', 'er_idle_pose.py', 'w101_npc_dialogue.py', 'w101_book_dialogue.py', 'w101_quests_json.py', 'w101_book_quests.py', 'w101_quest_nav.py'}
        for outputs, args in jobs:
            script = next(x for x in args if str(x).endswith(".py"))
            if (script in data_scripts) != (part == "data"):
                continue
            if self.fresh(outputs, [TOOLS / script]):
                continue
            iso = ["-I"] if args[0] == "-I" else []
            rest = args[1:] if iso else args
            try:
                self.run(sys.executable, *iso, TOOLS / rest[0], *rest[1:])
            except subprocess.CalledProcessError:
                failed.append(script)
        if part == "art" and self.blender and not (o / "fairies" / "fairy_sheet.png").exists():
            self.run(self.blender, "--background", "--python", TOOLS / "blender_fairy_sheet.py", "--", gd, o / "fairies")
        if failed:
            raise StepError(f"failed: {failed}")

    def maps(self):
        b = self.orbs / "book"
        self.py("build_map.py", self.project, "--game", self.game,
                "--dialogue", self.e / "work" / "npc_dialogue.json", "--recipes", b / "er" / "er_recipes.tsv",
                "--shop", b / "er" / "shop.tsv", "--patches", REPO / "patches" / "maps")

    def dll(self):
        crate = REPO / "hud_orbs"
        out = crate / "target" / "x86_64-pc-windows-gnu" / "release" / "w101_orbs.dll"
        self.run("cargo", "build", "--release", "--target", "x86_64-pc-windows-gnu", cwd=crate)
        dst = self.e / "orbs" / "w101_orbs.dll"
        if not self.a.dry_run:
            if dst.exists():
                shutil.copy2(dst, dst.with_name(dst.name + ".before_build"))
            shutil.copy2(out, dst)
        ini = self.e / "orbs" / "w101_orbs.ini"
        if not ini.exists() and not self.a.dry_run:
            ini.write_text("left = 0\nbottom = 0\nscale = 1\ndebug = 0\n"
                           "loadscreen_image = w101_orbs/book/art/GUI/LoadScreen_W01_WC_01.png\n"
                           "music_battle = w101_orbs/music/WC_HauntedWay_Combat.ogg\n")

    def modengine_config(self):
        cfg = self.modengine / "config_eldenring.toml"
        text = ("[modengine]\ndebug = false\nexternal_dlls = [ " + jstr(self.e / "orbs" / "w101_orbs.dll") + " ]\n\n"
                "[extension.mod_loader]\nenabled = true\nloose_params = false\nmods = [\n"
                "    { enabled = true, name = \"unicorn-way\", path = " + jstr(self.e / "mod") + " },\n"
                "    { enabled = true, name = \"smithbox-project\", path = " + jstr(self.project) + " }\n]\n\n"
                "[extension.scylla_hide]\nenabled = false\n")
        if cfg.exists() and cfg.read_text() == text:
            return "up to date"
        print(f"    writing {cfg}")
        if not self.a.dry_run:
            if cfg.exists() and not cfg.with_name(cfg.name + ".before_build").exists():
                shutil.copy2(cfg, cfg.with_name(cfg.name + ".before_build"))
            cfg.write_text(text)

    def verify(self):
        good = bad = absent = 0
        for rel, want in KNOWN.items():
            p = self.root / rel
            if not p.exists():
                absent += 1
                print(f"    -  {rel}: not built")
                continue
            got = hashlib.md5(p.read_bytes()).hexdigest()[:8]
            if got == want:
                good += 1
            else:
                bad += 1
                print(f"    !  {rel}: md5 {got}, the author's build has {want} (a newer game version?)")
        print(f"    {good} of {len(KNOWN)} known outputs match the author's build, {bad} differ, {absent} not built")
        return f"{good}/{len(KNOWN)} match"


STEPS = [
    ("check", Build.check, "prerequisites (and missing pip packages)"),
    ("project", Build.project_folder, "Smithbox project folder, project.json, vanilla regulation copy"),
    ("w101", Build.w101_files, "Wizard101 models, textures, normals, mob paths, quest text (w101_convert.py)"),
    ("street", Build.street, "the author's street edit (obj_patch.py) and graveyard_fix.tsv"),
    ("unpack", Build.unpack, "Blender: decompress Oodle game files (blender_unpack.py)"),
    ("blender", Build.blender_steps, "Blender: the blender_build_all.py STEPS (assets, sky, rooms, portraits)"),
    ("regulation", Build.regulation, "regulation.bin (er_regulation.py u6b)"),
    ("menus", Build.menus, "HUD-less menus (er_hud_hide_bars.py)"),
    ("data", Build.data, "item names, stats, recipes, shop, idle pose, dialogue, quests, quest walk graph"),
    ("text", Build.text, "place name, boss names and NPC dialogue in the msgbnds (er_text.py)"),
    ("maps", Build.maps, "events replayed and maps patched from vanilla (build_map.py)"),
    ("orbs", Build.orbs_art, "the HUD's Wizard101 art, sounds, voice, road mask, street sight, fairies"),
    ("dll", Build.dll, "the HUD DLL (cargo)"),
    ("modengine", Build.modengine_config, "Mod Engine 2 config"),
    ("verify", Build.verify, "md5 of the deterministic outputs"),
]


class StepError(Exception):
    pass


def jstr(path):
    return '"' + str(path).replace("\\", "\\\\") + '"'


def find_blender():
    found = shutil.which("blender")
    if found:
        return found
    hits = sorted(glob.glob(r"C:\Program Files\Blender Foundation\Blender *\blender.exe"))
    return hits[-1] if hits else None


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0], formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog="\n".join(f"  {n:<11} {d}" for n, _, d in STEPS))
    ap.add_argument("--only", nargs="+", metavar="STEP")
    ap.add_argument("--from", dest="start", metavar="STEP")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--force", action="store_true")
    ap.add_argument("--keep-going", action="store_true", help="go on after a missing prerequisite or a failed step")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--root")
    ap.add_argument("--game")
    ap.add_argument("--w101")
    ap.add_argument("--blender")
    ap.add_argument("--smithbox")
    ap.add_argument("--modengine")
    ap.add_argument("--skip-blender", nargs="*", metavar="NAME", default=["footprint"],
                    help="blender_build_all STEPS to leave out (default: footprint, whose result is in patches/maps)")
    a = ap.parse_args()
    names = [n for n, _, _ in STEPS]
    if a.list:
        print("\n".join(f"{n:<11} {d}" for n, _, d in STEPS))
        return
    for n in (a.only or []) + ([a.start] if a.start else []):
        if n not in names:
            ap.error(f"unknown step {n!r}; steps: {', '.join(names)}")
    chosen = a.only or names[names.index(a.start):] if a.start else a.only or names
    b = Build(a)
    print(f"config {CONFIG if CONFIG.exists() else '(none; see config.example.toml)'}\nroot {b.root}\ngame {b.game}\n"
          f"wizard101 {b.w101}\nsmithbox {b.smithbox}\nmodengine {b.modengine}\nblender {b.blender}\n")
    if not a.dry_run:
        for d in b.folders():
            d.mkdir(parents=True, exist_ok=True)
    for name, fn, desc in STEPS:
        if name not in chosen:
            continue
        print(f"== {name}: {desc}")
        t = time.time()
        try:
            note = fn(b)
            status = note or "done"
        except (StepError, subprocess.CalledProcessError, OSError) as error:
            status = f"FAILED: {error}"
            print(f"    {status}")
            b.results.append((name, status, time.time() - t))
            if not a.keep_going:
                break
            continue
        b.results.append((name, status, time.time() - t))
    print("\nSummary")
    for name, status, secs in b.results:
        print(f"  {name:<11} {status}  ({secs:.0f} s)")
    if any(s.startswith("FAILED") for _, s, _ in b.results):
        sys.exit(1)
    if not a.dry_run and "verify" in chosen:
        print("\nDone. Start the game with Mod Engine 2 (launchmod_eldenring.bat); back up your save first.")


if __name__ == "__main__":
    main()
