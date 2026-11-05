# Elden Ring on Unicorn Way

A personal, offline Elden Ring mod. You leave the Stranded Graveyard and walk out onto Wizard101's Unicorn Way, which has been rebuilt at The First Step (map tile `m60_42_36_00`). The street has its classic look, its sky, its NPCs and its quest line up to Rattlebones. A HUD DLL draws Wizard101's globes, spellbook, dialogue, shop and music over the game.

This repository holds **code only**: Python tools, a Rust DLL and a small C++ navmesh baker. It does not contain anything from either game. You make every model, texture, sound and game file yourself, from your own installs of Wizard101 and Elden Ring.

> **Status.** Every step is scripted, and the results were checked against the files the author is running (see *Verification*). Steps 3 and 7 also need two patch files that are not in this repository yet. Read *Known gaps* before you start.

---

## 1. Legal note (read first)

- You must own both games. Neither game's files are, or ever will be, in this repository. Do not publish anything the tools write.
- The Wizard101 Terms of Use (wizard101.com, *Terms of Use*) forbid reverse engineering the client and making derivative works. Following this README extracts and converts Wizard101 files, and that may break those terms even for private use. KingsIsle could ask for this project to be taken down. This is not legal advice. Decide for yourself.
- Elden Ring mods are only for **offline** play. Turn Easy Anti-Cheat off (Mod Engine 2 does this). Never take a modded game online.
- Tools from third parties keep their own licences. hudhook is vendored under `hud_orbs/vendor/hudhook` with its LICENSE. Recast is zlib-licensed and is not included.

---

## 2. Prerequisites

### Games

| Game | Where | Notes |
| --- | --- | --- |
| Wizard101 (Windows) | `C:\ProgramData\KingsIsle Entertainment\Wizard101` | Use **Classic Mode**. Play the Unicorn Way quest line up to Rattlebones once, then talk to the NPC afterwards. The client downloads voice lines and some WAD entries only when it first needs them, so this step is required. |
| Elden Ring (Steam, Windows) | `C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING` | It was checked against the version whose vanilla `Game\regulation.bin` has md5 `a631bf54…`. A newer patch changes the params, so the output md5s will differ. Expect to adapt. |

### Apps (Windows)

| App | Version used | For |
| --- | --- | --- |
| Blender | 5.1.2 | Assembling and exporting assets (`tools/blender_*.py`) |
| Soulstruct for Blender (`io_soulstruct`) | 3.2.1 (release zip) | FLVER and collision import and export inside Blender |
| Smithbox | 2.2.5 or later (.NET 10 build) | The project folder, map editing, paramdef XMLs (`Assets\PARAM\ER\Defs`) |
| UXM Selective Unpacker | latest release | Unpacking Elden Ring's archives |
| Mod Engine 2 | preview release | Launching the game with the mod folders and the DLL |
| NifSkope | 2.0 dev | Optional. Only for looking at `.nif` files |
| Python | 3.10+ (3.13 used) | Every tool in `tools/` |
| Rust | 1.97.0 + target `x86_64-pc-windows-gnu` | The HUD DLL |
| MinGW-w64 | binutils 2.41+ (Ubuntu 24.04 `mingw-w64`, or MSYS2 on Windows) | Linker for the DLL |
| g++ + Recast source | any recent | Only if you rebuild the navmesh (`tools/recast_bake.cpp`) |

Install the Python packages:

```
pip install numpy pillow pyffi==2.2.3 cryptography zstandard pyooz matplotlib scipy opencv-python
```

Which tools need which package:

- `pyffi` is used by `nif2obj.py` and `w101_quest_window.py`. These scripts patch around pyffi's missing `time.clock` themselves.
- `pyooz` reads Oodle KRAK DCX files (`dcx_krak.py`).
- `cryptography` and `zstandard` are used by `er_params.py` and `er_regulation.py`.
- `matplotlib`, `scipy` and `opencv-python` are only for preview pictures and the map art. `w101_map_art.py` uses OpenCV's super-resolution model when you pass `--sr`.

The Blender scripts use Blender's own Python, with Soulstruct installed as an add-on: `Edit > Preferences > Add-ons > Install from Disk`.

---

## 3. Folder layout

Everything lives under one root. By default the root is `%USERPROFILE%\Documents\game_modding`. You can override it with the environment variable **`W101ER_ROOT`**. The Elden Ring `Game` folder can be overridden with **`ER_GAME`**. Every script reads both variables. Set them before you start Blender if your paths differ.

```
game_modding\                      (W101ER_ROOT)
├─ eldenring-w101\                 this repository (code only)
├─ tools\                          Smithbox\, UXM Selective Unpacker\, ModEngine\, NifSkope\
└─ reference\                      everything from the games; never commit
   ├─ w101\
   │  ├─ wads\                     copies of the zone WADs
   │  ├─ extracted\WC_Unicorn\     unpacked zone
   │  ├─ converted\ converted_classic\ converted_rattlebones_classic\ converted_sky_classic\
   │  │  converted_bonecage\ converted_clubhouse_hd\ converted_hedgemaze_classic\ converted_quest_window\
   │  └─ extracted\quests\uw_lang.json
   └─ eldenring\
      ├─ unpacked\                 KRAK files decompressed by blender_unpack.py
      ├─ work\                     scratch
      ├─ project\                  Smithbox project = Mod Engine mod folder (map, asset, event, msg, param, chr, regulation.bin)
      ├─ mod\                      second Mod Engine mod folder (menu\*.gfx)
      └─ orbs\                     the HUD DLL, w101_orbs.ini and its data folder w101_orbs\
```

---

## 4. Steps

Run the commands from `eldenring-w101\tools`. `GD` means `…\Wizard101\Data\GameData`. `GAME` means `…\ELDEN RING\Game`. `ORBS` means `reference\eldenring\orbs\w101_orbs`.

### Step 1: Unpack Elden Ring

1. Run UXM Selective Unpacker on `GAME`. Unpack at least `asset`, `chr`, `map`, `material`, `menu`, `msg`, `event`, `param` and `sfx`. Soulstruct and the tools read the unpacked files.
2. Create a Smithbox project for Elden Ring with its folder at `reference\eldenring\project`. Smithbox writes `project.json`. Copy the vanilla `GAME\regulation.bin` into it and keep a copy named `regulation.bin.vanilla`.
3. Big Oodle-KRAK files (msgbnd, common_func and others) can only be read through Soulstruct on Windows. Run `blender_unpack.py` in Blender's Scripting tab. It writes them to `reference\eldenring\unpacked\`.

**Expected:** you have `GAME\map\mapstudio\m60_42_36_00.msb.dcx` and `m18_00_00_00.msb.dcx`, plus `GAME\menu\01_000_fe.gfx`.

### Step 2: Extract and convert Wizard101

1. Copy `GD\WizardCity-WC_Streets-WC_Unicorn.wad`, `WizardCity-WorldData.wad` and `ClassicMode-WC_Streets-WC_Unicorn.wad` into `reference\w101\wads\`. Unpack them with wizwad (`pip install wizwad`, then run `wizwad extract`) into `reference\w101\extracted\`.
2. Convert the classic street:
   ```
   python nif2obj.py  <ClassicMode-WC_Streets-WC_Unicorn.wad>\WC_Z06_Unicorn_Way.nif  reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic.obj
   python nif2obj.py  <WizardCity-WC_Streets-WC_Unicorn.wad>\WC_Z06_Unicorn_Way.nif  reference\w101\converted\WC_Z06_Unicorn_Way.obj
   python nif_normals.py <classic .nif> reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic.normals
   python tex2dds.py  <folder of texture .nif files> reference\w101\converted_classic\textures
   python w101_window_states.py <WC_Z06_Unicorn_Way.nif> reference\w101\converted_classic\window_states.tsv
   ```
3. Convert the other pieces the same way. Each Blender build script's docstring names its source `.nif` and the `converted_*` folder it reads:
   - Rattlebones' tower room: `converted_rattlebones_classic`, `WC_Generic_Gauntlet_NoDoor_Classic`;
   - the sky: `converted_sky_classic` (`sky_bake.py`);
   - the hedge maze: `converted_hedgemaze_classic`;
   - the Bone Cage: `w101_bone_cage.py`;
   - the quest window: `w101_quest_window.py`;
   - the clubhouse interior.
4. Write the mob paths. Put `TemplateManifest.xml` from `Root.wad` (wizwad) alone in a folder, then run:
   ```
   python w101_pathing.py --wad GD\WizardCity-WC_Streets-WC_Unicorn.wad --wad GD\ClassicMode-WC_Streets-WC_Unicorn.wad --obj reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic.obj --out reference\w101\converted_classic --root <that folder>
   ```
   This writes `unicorn_way_pathing.json` (md5 `c80a0277…`). Adding the mobs' `ObjectData` xmls to the folder adds their walk speeds to the file, which changes its md5. The picture it draws at the end needs matplotlib.
5. Dump the quest text you will see in game:
   ```
   python -I w101_lang.py dump GD\Root.wad reference\w101\extracted\quests\uw_lang.json "WizQst.*|WizardShops"
   ```

**Expected:** `WC_Z06_Unicorn_Way_Classic.obj` (about 117,000 vertices), its `.mtl`, `.normals` and `textures\*.dds`.

### Step 3: The author's edit of the street OBJ

The author edited the street in Blender and exported his whole scene as `WC_Z06_Unicorn_Way_Classic_edited.obj` (Blender 5.1.2). The scene holds:

- the classic street with his deletions (the archway tunnel and some props);
- the HD street, twice;
- our invisible floor.

`blender_switch_street.py` uses `_edited.obj` when it exists and the converted OBJ otherwise. `obj_patch.py` recreates his file from your two converted OBJs:

```
python obj_patch.py apply WC_Z06_Unicorn_Way_Classic_edited.objpatch.gz reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic_edited.obj reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic.obj reference\w101\converted\WC_Z06_Unicorn_Way.obj
```

The result is byte for byte his file (md5 `f6aab2eb…`). Copy `WC_Z06_Unicorn_Way_Classic.mtl` to `_edited.mtl`; the build fills in missing materials from the original MTL. The patch file is not in this repository (see Known gaps). Without it, delete `_edited.obj` and the build cuts the tunnel itself.

Then move the street's vertices out of the Stranded Graveyard. This needs only the game files and your OBJ:

```
python street_graveyard_fix.py reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic_edited.obj  GAME  GAME\map\mapstudio\m18_00_00_00.msb.dcx  reference\w101\converted_classic\graveyard_fix.tsv
```

**Expected** with the author's OBJ: 152 positions and 167 vertices moved, with 0 open samples left. Your numbers will differ if your edit differs.

### Step 4: Build the Elden Ring assets (Blender)

Open Blender. In the Scripting tab, open `tools\blender_build_all.py` and run it, with `Window > Toggle System Console` open so you can watch. It runs these steps in order:

| Step | Script | What it builds |
| --- | --- | --- |
| street | `blender_switch_street.py` | aeg030_518: FLVER, textures, collision |
| footprint | `blender_clear_footprint.py` | sinks Elden Ring ground and props under the street in every m60 map that draws it |
| sky | `blender_build_sky.py` | aeg030_523 dome and the no-fog drawparams |
| room | `blender_build_room.py` | aeg030_519, the tower room |
| garden | `blender_build_garden.py` | aeg030_555 maze and aeg030_529 quest window |
| interiors | `blender_build_interiors.py` | aeg030_557 |
| cage | `blender_build_cage.py` | aeg030_558 and 559 |
| fog | `blender_build_fog.py` | aeg030_530, the Commons gate fog |
| boss | `blender_build_boss.py` | `chr\c4960.anibnd.dcx` that never fades |
| portraits | `blender_npc_portraits.py` | `ORBS\book\art\GUI\NpcPortraits\ER\*.png` |

A failing step is reported and the remaining steps still run. Use `SKIP = {...}` at the top of the script to rerun only some of them. When the run is done, reload the maps in Smithbox before you save anything in Smithbox.

**Expected:** `project\asset\aeg\aeg030\aeg030_5xx.geombnd.dcx` and `_l.geomhkxbnd.dcx`, `project\param\drawparam\*.gparam.dcx`, and the portraits.

### Step 5: Regulation (params)

```
python er_regulation.py u6b  <project>\regulation.bin.vanilla  <Smithbox>\Assets\PARAM\ER\Defs  <project>\regulation.bin
```

This makes a minimal regulation. Enemy drops are picked up automatically, and the NPC rows the map needs are added. It writes the file in the same layout Smithbox uses. **Do not** let Smithbox re-sort and save regulation.bin; a sorted rebuild crashed the game at launch. Keep `regulation.bin.vanilla` so you can roll back.

### Step 6: Menus and text

```
python er_hud_hide_bars.py  GAME\menu  reference\eldenring\mod\menu
python er_text.py              <unpacked>\msg\engus\item_dlc02.msgbnd   <project>\msg\engus\item_dlc02.msgbnd.dcx
python er_text.py --npc-names  <project>\msg\engus\item_dlc02.msgbnd.dcx <project>\msg\engus\item_dlc02.msgbnd.dcx.new
python er_text.py --dialogue npc_dialogue.json <unpacked>\msg\engus\menu_dlc02.msgbnd <project>\msg\engus\menu_dlc02.msgbnd.dcx <any project DFLT .dcx>
```

These commands do three jobs:

- `er_hud_hide_bars.py` hides Elden Ring's HP, FP and stamina bars and the rune counter.
- The plain `er_text.py` run adds PlaceName 610900 "Unicorn Way".
- `--npc-names` adds the four boss-bar names. Rename the `.new` output over the input when it is done.
- `--dialogue` adds the NPCs' dialogue pages. `npc_dialogue.json` comes from step 8 (`w101_npc_dialogue.py`).

Run the same commands on `item.msgbnd` and `menu.msgbnd` as well. The game reads the `_dlc02` files when the DLC is installed.

### Step 7: Map and events (msb, emevd, navmesh)

Run this step after step 8: it needs `npc_dialogue.json`, `er_recipes.tsv` and `shop.tsv` from that step.

```
python build_map.py <out folder> --dialogue npc_dialogue.json --recipes ORBS\book\er\er_recipes.tsv --shop ORBS\book\er\shop.tsv --patches <folder of *.msbpatch.gz> [--compare <project>]
```

Then copy `<out folder>\event` and `<out folder>\map` over the project folder.

- **Events** are replayed from the vanilla files with the event scripts, in the order they were first run. The order is listed in `build_map.py`. The output is byte for byte the deployed `m60_42_36_00.emevd.dcx` (`b19bcc3a…`), `common.emevd.dcx` and `m60.emevd.dcx`.
- **Maps** are the vanilla maps plus a recorded patch per map (`msb_patch.py`). This covers `m60_42_36_00` (`d4a5029e…`) and the 44 neighbouring m60 tiles that the footprint step edits. It also covers `m18_00_00_00`, whose content is vanilla.

  The maps need a patch because the author's own Smithbox edits are mixed into the script chain:
  - he placed the street part `AEG030_518_9000` at (6.13, 89.59, -86.64), rotation Y 4, by hand;
  - he moved the First Step grace onto the street;
  - he lowered terrain `m423600` by 6.4 m;
  - he deleted rocks and the tunnel rock behind the graveyard door.

  Smithbox saves and the Blender footprint step also came in between the script runs.
- **The map scripts** (`msb_move_street.py`, `msb_add_*.py`, `msb_npc_seats.py`, `msb_fog_gate.py`, `msb_door_spawns.py`, `msb_doors_garden.py`, `msb_audit.py`, `footprint_clear.py`, `dome_clear.py`) are how the content was authored. Use them to change the map, then record a new patch with `python msb_patch.py make VANILLA.msb.dcx NEW.msb.dcx NEW.msbpatch.gz`.
- **Navmesh:**
  ```
  g++ -O2 -I<recast>/Recast/Include recast_bake.cpp <recast>/Recast/Source/*.cpp -o recast_bake
  python er_navmesh.py <map>.msb.dcx GAME\map\m60 <out> --append --asset AEG030_518_9000=aeg030_518_l.geomhkxbnd.dcx
  ```
  Only **appended** navmeshes load. Check the result with `navmesh_check.py`. This was not rerun for the verification.

### Step 8: HUD DLL data folder (`ORBS`)

Each command copies or derives the art and sound the DLL uses. All of these need only the two game installs:

```
python w101_hud_art.py        GD  ORBS
python w101_spellbook_art.py  GD  ORBS\book
python w101_book_layout.py    GD  ORBS\book
python -I w101_quest_art.py   GD  ORBS\book
python -I w101_quest_arrow.py GD  ORBS\book
python -I w101_map_art.py     GD  ORBS\book [--sr EDSR_x4.pb]
python w101_teleport_art.py   GD  ORBS\book\art\teleport
python -I w101_wisp_art.py    GD  ORBS
python w101_page_flip.py      GD  ORBS\book\page_flip.tsv
python w101_music.py          GD  ORBS\music
python -I w101_sfx.py         GD  ORBS\sfx
python -I w101_voice.py       GD  ORBS\voice
python w101_prompt_glyphs.py  ORBS\book\art\GUI\Art
python blender_fairy_sheet.py GD  ORBS\fairies          (Blender)
python er_item_names.py <unpacked>\msg\engus\item_dlc02.msgbnd  ORBS\er_item_names.tsv
python er_book_data.py stats <project>\regulation.bin <Defs> <item_dlc02.msgbnd.dcx> ORBS\book\er   (see its docstring for icons/menu)
python er_recipes.py <regulation.bin> <Defs> ORBS\er_item_names.tsv <er_item_text.tsv> ORBS\book\er\er_recipes.tsv
python w101_shop.py  <regulation.bin> <Defs> ORBS\er_item_names.tsv ORBS\book\er\shop.tsv
python er_idle_pose.py GAME\chr ORBS\book\er\idle_pose.tsv
python -I w101_npc_dialogue.py reference\w101\extracted\quests\uw_lang.json npc_dialogue.json ORBS\book\voice.tsv
python -I w101_book_dialogue.py npc_dialogue.json ORBS\book\dialogue.tsv
python -I w101_quests_json.py reference\w101\extracted\quests\uw_lang.json unicorn_way_quests.json
python w101_book_quests.py unicorn_way_quests.json ORBS\book\quests.tsv
python w101_quest_nav.py <converted_classic>\unicorn_way_pathing.json ORBS\book\quest_nav.tsv
python w101_road_mask.py <classic .obj> unicorn_way_pathing.json <map>.msb.dcx ORBS\road_mask.tsv
python -I w101_street_sight.py <edited .obj> ORBS\book\quest_nav.tsv ORBS\book\street_sight.bin
```

Where the docstrings and these lines disagree on an output name, follow the docstring.

`w101_voice.py` only finds the lines the client has already streamed. It reports any line that is not downloaded yet.

### Step 9: Build the DLL

On Linux, or on Windows with MSYS2:

```
rustup target add x86_64-pc-windows-gnu
cd hud_orbs
cargo build --release --target x86_64-pc-windows-gnu
```

Copy `target\x86_64-pc-windows-gnu\release\w101_orbs.dll` to `reference\eldenring\orbs\`. The DLL looks for `w101_orbs.ini` and `w101_orbs\` next to itself. A minimal `w101_orbs.ini`:

```
left = 0
bottom = 0
scale = 1
debug = 0
loadscreen_image = w101_orbs/book/art/GUI/LoadScreen.png
music_battle = w101_orbs/music/WC_HauntedWay_Combat.ogg
```

### Step 10: Mod Engine 2 and launch

In `tools\ModEngine\config_eldenring.toml` (use your own absolute paths):

```toml
[modengine]
debug = false
external_dlls = [ "C:\\…\\reference\\eldenring\\orbs\\w101_orbs.dll" ]

[extension.mod_loader]
enabled = true
loose_params = false
mods = [
    { enabled = true, name = "unicorn-way",      path = "C:\\…\\reference\\eldenring\\mod" },
    { enabled = true, name = "smithbox-project", path = "C:\\…\\reference\\eldenring\\project" },
]

[extension.scylla_hide]
enabled = false
```

Start the game with `launchmod_eldenring.bat`. Back up your save first (`%APPDATA%\EldenRing\<id>\ER0000.sl2`). Load a character, go to the Stranded Graveyard and open the exit gate.

---

## 5. Troubleshooting

| Symptom | Cause / fix |
| --- | --- |
| Game crashes at launch after a param change | A regulation.bin that Smithbox re-sorted. Rebuild it with `er_regulation.py u6b`, or restore `regulation.bin.vanilla`. |
| Enemies stand still on the street | A navmesh was replaced instead of appended. Run `er_navmesh.py --append` and check it with `navmesh_check.py`. |
| Vanilla props poke up through the street, or our own props vanish | `footprint_clear.KEEP` must list every vanilla-model part we add. Rerun footprint and dome if the dome grows more than 25%. |
| Street fades out in the distance | PartsDrawParam row 36 must be on the street part (`msb_*` scripts set it). |
| Quest progress resets on every load | Flags 1042362xxx are cleared on map load. Saved state uses 1042360xxx. |
| Pieces of the street show inside the graveyard | `graveyard_fix.tsv` is missing or was made from a different OBJ. Rerun step 3. |
| No HUD | Check that `external_dlls` points at the DLL. Check `orbs\w101_orbs.log`. |
| A KRAK `.dcx` will not open in Python | Run `blender_unpack.py` in Blender, or use `dcx_krak.py` (pyooz) for small files. |
| Blender scripts cannot find files | Set `W101ER_ROOT` and `ER_GAME` before you start Blender. |

---

## 6. Known gaps

1. **The two patch files are not in this repository yet:** `WC_Z06_Unicorn_Way_Classic_edited.objpatch.gz` (about 6.7 MB, or 3.8 MB with `--no-normals`) and the `*.msbpatch.gz` map patches (about 330 KB for all 46 maps). Their contents:
   - The OBJ patch holds references into your own converted OBJs plus Blender's float noise. Its only literal geometry is our floor and 12 other values.
   - The map patches hold byte edits to vanilla map entries, plus our own entries.

   The author still has to decide whether they may be published. Until he does, steps 3 and 7 can be rebuilt only with a copy from him.
2. **Voice lines** exist only after the Wizard101 client has played them (step 2 prerequisite).
3. **Blender renders and the navmesh** (portraits, fairy sheet, every Blender build step, `er_navmesh.py`) were not rerun for the verification. Renders also depend on the Blender and Cycles versions.
4. **The DLL** builds with the same code and imports, but not byte for byte (PE timestamp and import order).

## 7. Verification

The author's tree was checked on 2026-11-05 (repo commit 8e633a5, then this README commit). Each step was run from a fresh clone against real game files, read-only:

| Output | Result |
| --- | --- |
| `regulation.bin` (step 5) | md5 `04d5b270…` = deployed |
| `01_000_fe.gfx` / `01_001_fe_soul.gfx` (step 6) | `9a420b8b…` / `abcb1fab…` = deployed |
| `graveyard_fix.tsv` (step 3) | 152 positions, 167 vertices, 0 open samples |
| `book\quests.tsv` (`w101_quests_json.py` + `w101_book_quests.py`) | `9096ca8f…` = deployed |
| `book\quest_nav.tsv` (`w101_quest_nav.py`) | `8c93b664…` = deployed |
| `book\street_sight.bin` | `4598d7eb…` = deployed |
| `road_mask.tsv` | `c9306eb1…` = deployed |
| voice (step 8, `w101_voice.py`) | 55/55 found. 44 byte-identical to the deployed copies. 6 differ only in their ID3 tag (the audio frames are identical). 5 are lines not yet deployed. |
| DLL (step 9) | builds. It differs from the deployed DLL only in the PE timestamp, the import-table order and the call references into that table: same imports, same size. |
| `unicorn_way_pathing.json` (step 2, from the WADs) | `c80a0277…` = the copy in use |
| classic and HD street OBJs (step 2, `nif2obj.py` from the WADs) | equal to the author's files apart from the `#vc` comment lines that `nif2obj.py` added later (`obj_patch.py` ignores those lines) |
| `_edited.obj` (step 3, `obj_patch.py apply` on those OBJs) | `f6aab2eb…` = the author's file, byte for byte. With `--no-normals` the data `parse_obj` reads is identical, and `street_sight.bin` built from it is again `4598d7eb…`. |
| events (step 7) | `m60_42_36_00.emevd.dcx` `b19bcc3a…`, `common.emevd.dcx` and `m60.emevd.dcx` = deployed, byte for byte |
| maps (step 7) | 45 of 46 = deployed, byte for byte, including `m60_42_36_00.msb.dcx` `d4a5029e…`. `m18_00_00_00` has identical content but was compressed differently. |
| `dialogue.tsv` / `voice.tsv` (step 8) | `e9406a1b…` / `cbdad56d…` = deployed |
| Blender steps 4 and 8 (fairies), navmesh (step 7), launch (step 10) | not run here (they need Blender or the game) |
