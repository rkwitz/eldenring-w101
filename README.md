# Elden Ring on Unicorn Way

A personal, offline Elden Ring mod. You leave the Stranded Graveyard and walk out onto Wizard101's Unicorn Way, which has been rebuilt at The First Step (map tile `m60_42_36_00`). The street has its classic look, its sky, its NPCs and its quest line up to Rattlebones. A HUD DLL draws Wizard101's globes, spellbook, dialogue, shop and music over the game.

This repository holds **code only**: Python tools, a Rust DLL and a small C++ navmesh baker. It does not contain anything from either game. You make every model, texture, sound and game file yourself, from your own installs of Wizard101 and Elden Ring.

> **Status.** Steps 1 to 6 and 8 to 10 are scripted. They were checked against the files the author is running (see *Verification* below). Step 7 (the map and event files) is **not fully scripted yet**. Read *Known gaps* before you start.

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
   python nif2obj.py  <extracted>\WC_Z06_Unicorn_Way_Classic.nif  reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic.obj
   python nif_normals.py <same .nif> reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic.normals
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
4. Write the mob paths: `python w101_pathing.py --wad <zone>.wad --wad <ClassicMode zone>.wad --obj <street .obj> --out reference\w101\converted_classic`. This writes `unicorn_way_pathing.json`.
5. Dump the quest text you will see in game: `python -I w101_lang.py dump GD\Root.wad reference\w101\extracted\quests\uw_lang.json`.

**Expected:** `WC_Z06_Unicorn_Way_Classic.obj` (about 117,000 vertices), its `.mtl`, `.normals` and `textures\*.dds`.

### Step 3: Hand edit of the street OBJ (manual, see Known gaps)

The author edited the converted OBJ in Blender: he removed the Commons archway's tunnel and tidied some geometry. He exported it as `WC_Z06_Unicorn_Way_Classic_edited.obj` and `.mtl` (Blender 5.1.2, OBJ export, `-Z` forward and `Y` up as the importer gives it). `blender_switch_street.py` uses `_edited.obj` when it exists and the converted OBJ otherwise. Without the edit, the script cuts the tunnel itself, but the result is not byte-identical.

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

### Step 7: Map and events (msb, emevd, navmesh): partly manual

The map `project\map\mapstudio\m60_42_36_00.msb.dcx` and the events (`project\event\m60_42_36_00.emevd.dcx`, `m18_00_00_00.emevd.dcx`, `common.emevd.dcx`) were built up over many rounds. Two kinds of change went into them:

- **Hand placement in Smithbox:**
  - the street part `AEG030_518_9000` at (6.13, 89.59, -86.64), rotation Y 4;
  - the First Step grace parts moved onto the street;
  - terrain `m423600` lowered by 6.4 m;
  - a few rocks and the tunnel rock behind the graveyard door deleted.
- **The scripts below**, each applied to the map the previous one wrote. Every script's docstring gives its usage. The order is:
  1. `msb_move_street.py`
  2. `msb_level_grace.py` (one-off)
  3. `msb_name_region.py`
  4. `msb_add_room.py`
  5. `msb_add_sky.py`
  6. `msb_add_garden.py`
  7. `msb_add_interiors.py`
  8. `msb_add_indoor.py`
  9. `msb_add_mobs.py`, then `msb_more_mobs.py`
  10. `msb_add_ghost.py`
  11. `msb_add_boss.py`
  12. `msb_add_npcs.py`
  13. `msb_npc_seats.py`
  14. `msb_add_quests.py`
  15. `msb_fog_gate.py`
  16. `msb_door_spawns.py`
  17. `msb_doors_garden.py`
  18. `msb_audit.py --apply`
  19. `footprint_clear.py` and `dome_clear.py` (these edit every neighbouring m60 map)
- **Events:**
  1. `emevd.py` on `m18_00_00_00.emevd` (door reset)
  2. `emevd_quests.py --dialogue …\npc_dialogue.json` on `m60_42_36_00.emevd.dcx`
  3. `emevd.py --doors`
  4. `emevd.py --sidewalk`
  5. `emevd.py --boss-watchdog`
  6. `emevd.py --garden-doors`
  7. `emevd_quests.py --cage-ride-press`
- **Navmesh:**
  ```
  g++ -O2 -I<recast>/Recast/Include recast_bake.cpp <recast>/Recast/Source/*.cpp -o recast_bake
  python er_navmesh.py <map>.msb.dcx GAME\map\m60 <out> --append --asset AEG030_518_9000=aeg030_518_l.geomhkxbnd.dcx
  ```
  Only **appended** navmeshes load. Check the result with `navmesh_check.py`.

The order above is the author's best reconstruction. **It has not been replayed from vanilla** to the deployed md5. See Known gaps.

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

## 6. Known gaps (not reproducible by script today)

1. **Map and events (step 7).** Smithbox hand edits and many rounds of incremental script runs produced the deployed `m60_42_36_00.msb.dcx` and the emevds. Nobody has replayed them from vanilla. *Proposal:* add a `build_map.py` that runs the step 7 chain from vanilla, adds the Smithbox moves as explicit script edits, and checks the result part by part against a known-good map.
2. **The edited street OBJ (step 3).** The author's Blender export does not share a vertex order with `nif2obj.py`'s output: it has 327,728 vertices against 117,562. *Proposal:* an `obj_patch.py` that records the edit as "triangles deleted" (by their position) plus "vertices moved" against the converted OBJ, applied by `blender_switch_street.py`. The patch would contain only numbers that point at your own converted file.
3. **Voice lines.** These exist only after the Wizard101 client has played them (step 2 prerequisite).
4. **`unicorn_way_pathing.json`.** The copy in use was written by `w101_pathing.py` on 2026-10-22. It was not regenerated from the WADs for this check. Its node positions feed `quest_nav.tsv` and `road_mask.tsv`.
5. **Portraits and the fairy sheet.** Blender renders these. The output depends on Blender and Cycles versions.

---

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
| Blender steps 4 and 8 (fairies), msb/emevd/navmesh (step 7), launch (step 10) | not run here (they need Blender or the game) |
