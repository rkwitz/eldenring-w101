# Build details

`build.py` runs every step below in order. This page says what each step does, so you can run one by hand, change
it, or find out why it failed. The README has the short version.

## How build.py maps to the steps

| build.py step | Step on this page | Runs |
| --- | --- | --- |
| `check` | Prerequisites | checks the games, apps and toolchain; `pip install`s missing Python packages |
| `project` | 1 | creates the work-root folders, `project.json` and `regulation.bin.vanilla` |
| `w101` | 2 | `tools\w101_convert.py` |
| `street` | 3 | `obj_patch.py apply`, the `_edited.mtl` copy, `street_graveyard_fix.py` |
| `unpack` | 1 | `blender --background --python tools\blender_headless.py -- blender_unpack.py` |
| `blender` | 4 | the same, once per entry of `STEPS` in `blender_build_all.py` (`--skip-blender`, default `footprint`) |
| `regulation` | 5 | `er_regulation.py u6b` |
| `menus` | 6 | `er_hud_hide_bars.py` |
| `data` | 8 | item names, stats, recipes, shop, idle pose, dialogue, quests, quest walk graph |
| `text` | 6 | `er_text.py` (place name, boss names, NPC dialogue) for `item` and `menu`, base and `_dlc02` |
| `maps` | 7 | `build_map.py` with `patches\maps` |
| `orbs` | 8 | the HUD's art, sounds, voice lines, road mask, street sight and fairy sheet |
| `dll` | 9 | `cargo build`, then copies the DLL and writes `w101_orbs.ini` if there is none |
| `modengine` | 10 | writes `config_eldenring.toml` (the old one is kept as `.before_build`) |
| `verify` | Verification | md5s of the deterministic outputs |

`--only STEP ...`, `--from STEP`, `--dry-run` and `--force` pick and redo steps. A step whose outputs are newer than
its inputs is skipped. The Blender steps always run when chosen.

`blender_headless.py` exists because the Soulstruct add-on loads `oo2core_6_win64.dll` from a scene's game folder when
a `.blend` opens. That never happens in background mode, so the wrapper loads it from the Elden Ring `Game` folder
first, then runs the script. The Blender scripts still run in the GUI's Scripting tab as before.

## Paths and config.toml

Each path is looked up as: the build.py flag, then the environment variable, then `config.toml` at the repo root (copy
`config.example.toml`; git ignores `config.toml`), then the default. The tools read the same file through
`tools\w101er_config.py`, and build.py also passes the paths on to them as environment variables.

| Flag | Variable | config.toml key | Default |
| --- | --- | --- | --- |
| `--game` | `ER_GAME` | `game` | `C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game` |
| `--w101` | `W101_INSTALL` | `wizard101` | `C:\ProgramData\KingsIsle Entertainment\Wizard101` |
| `--root` | `W101ER_ROOT` | `work_root` | `%USERPROFILE%\Documents\game_modding` |
| `--smithbox` | `SMITHBOX` | `smithbox` | `<work root>\tools\Smithbox` |
| `--modengine` | `MODENGINE` | `modengine` | `<work root>\tools\ModEngine` |
| `--blender` | `BLENDER` | `blender` | the newest `blender.exe` under Program Files |

Inside the GUI Blender, a script finds `config.toml` only if the tools folder is on its path. If it is not, the
environment variables and the defaults apply as before.

## The work root

build.py creates every folder in it. Everything below `reference\` comes from the games; never commit or share it.

```
<work root>\                          (work_root, W101ER_ROOT)
└─ reference\
   ├─ w101\
   │  ├─ converted\                   HD street
   │  ├─ converted_classic\           classic street, _edited.obj, .normals, window_states.tsv,
   │  │                               graveyard_fix.tsv, unicorn_way_pathing.json, textures\
   │  ├─ converted_rattlebones_classic\ converted_sky_classic\ converted_bonecage\
   │  │  converted_clubhouse_hd\ converted_hedgemaze_classic\ converted_quest_window\
   │  └─ extracted\quests\uw_lang.json
   └─ eldenring\
      ├─ unpacked\                    KRAK files decompressed by blender_unpack.py
      ├─ work\                        scratch (npc_dialogue.json, unicorn_way_quests.json)
      ├─ project\                     Smithbox project = Mod Engine mod folder
      │                               (project.json, regulation.bin(.vanilla), map, asset, event, msg, param, chr)
      ├─ mod\menu\                    second mod folder (*.gfx)
      └─ orbs\                        w101_orbs.dll, w101_orbs.ini and its data folder w101_orbs\
                                      (book\, music\, sfx\, voice\, fairies\, road_mask.tsv, er_item_names.tsv)
```

The repository and the apps (Smithbox, UXM, Mod Engine 2, Blender) can live anywhere.

## Prerequisites

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

## Steps

Run the commands from the repository's `tools` folder. `GD` means `…\Wizard101\Data\GameData`. `GAME` means
`…\ELDEN RING\Game`. `ORBS` means `reference\eldenring\orbs\w101_orbs`.

### Step 1: Unpack Elden Ring (UXM by hand; `project` and `unpack`)

1. Run UXM Selective Unpacker on `GAME`. Unpack at least `asset`, `chr`, `map`, `material`, `menu`, `msg`, `event`, `param` and `sfx`. Soulstruct and the tools read the unpacked files.
2. Create a Smithbox project for Elden Ring with its folder at `reference\eldenring\project`. Smithbox writes `project.json`. Copy the vanilla `GAME\regulation.bin` into it and keep a copy named `regulation.bin.vanilla`.
3. Big Oodle-KRAK files (msgbnd, common_func and others) can only be read through Soulstruct on Windows. Run `blender_unpack.py` in Blender's Scripting tab. It writes them to `reference\eldenring\unpacked\`.

**Expected:** you have `GAME\map\mapstudio\m60_42_36_00.msb.dcx` and `m18_00_00_00.msb.dcx`, plus `GAME\menu\01_000_fe.gfx`.

### Step 2: Extract and convert Wizard101 (`w101`)

`python tools\w101_convert.py GD reference\w101` reads every entry straight out of the WADs (no wizwad needed):

- `nif2obj.py` converts the eight models: the classic and HD streets, Rattlebones' tower room
  (`WC_Generic_Gauntlet_NoDoor`), the two sky layers, the Bone Cage, the clubhouse and the hedge maze. The table
  `MODELS` at the top of `w101_convert.py` names each one's WAD, entry and output folder.
- `tex2dds.py` converts the textures each `.mtl` names. They are searched in the WADs listed in `TEXTURE_WADS`; an
  entry the client has not downloaded yet (all zeros) is skipped, and the next WAD is tried.
- `nif_normals.py`, `w101_window_states.py`, `w101_quest_window.py` (`WC_Unicorn_T2_quest`) and `w101_bone_cage.py`
  (the open cage) write the extras.
- `w101_pathing.py` writes the mob paths, `unicorn_way_pathing.json` (md5 `c80a0277…`), with only
  `TemplateManifest.xml` from `Root.wad` as its template root. Adding the mobs' `ObjectData` xmls adds their walk
  speeds, which changes the md5. The picture it draws at the end needs matplotlib.
- `w101_lang.py dump` writes the quest text to `reference\w101\extracted\quests\uw_lang.json` (the tables in
  `LANG_TABLES`).

`--only street extras pathing lang` (and so on) reruns single pieces.

**Expected:** `WC_Z06_Unicorn_Way_Classic.obj` (about 117,000 vertices), its `.mtl`, `.normals` and `textures\*.dds`.
Every output's md5 without the `#vc` lines is in `EXPECTED` in `w101_convert.py`, and all of them matched the author's
files.

### Step 3: The author's edit of the street OBJ (`street`)

The author edited the street in Blender and exported his whole scene as `WC_Z06_Unicorn_Way_Classic_edited.obj` (Blender 5.1.2). The scene holds:

- the classic street with his deletions (the archway tunnel and some props);
- the HD street, twice;
- our invisible floor.

`blender_switch_street.py` uses `_edited.obj` when it exists and the converted OBJ otherwise. `obj_patch.py` recreates his file from your two converted OBJs:

```
python obj_patch.py apply eldenring-w101\patches\street\WC_Z06_Unicorn_Way_Classic_edited.objpatch.gz reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic_edited.obj reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic.obj reference\w101\converted\WC_Z06_Unicorn_Way.obj
```

The result is byte for byte his file (md5 `f6aab2eb…`). Copy `WC_Z06_Unicorn_Way_Classic.mtl` to `_edited.mtl`; the build fills in missing materials from the original MTL. The patch is in `patches/street/`.

Then move the street's vertices out of the Stranded Graveyard. This needs only the game files and your OBJ:

```
python street_graveyard_fix.py reference\w101\converted_classic\WC_Z06_Unicorn_Way_Classic_edited.obj  GAME  GAME\map\mapstudio\m18_00_00_00.msb.dcx  reference\w101\converted_classic\graveyard_fix.tsv
```

**Expected** with the author's OBJ: 152 positions and 167 vertices moved, with 0 open samples left. Your numbers will differ if your edit differs.

### Step 4: Build the Elden Ring assets (`blender`)

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

### Step 5: Regulation (`regulation`)

```
python er_regulation.py u6b  <project>\regulation.bin.vanilla  <Smithbox>\Assets\PARAM\ER\Defs  <project>\regulation.bin
```

This makes a minimal regulation. Enemy drops are picked up automatically, and the NPC rows the map needs are added. It writes the file in the same layout Smithbox uses. **Do not** let Smithbox re-sort and save regulation.bin; a sorted rebuild crashed the game at launch. Keep `regulation.bin.vanilla` so you can roll back.

### Step 6: Menus and text (`menus`, `text`)

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

### Step 7: Map and events (`maps`; the navmesh is by hand)

Run this step after step 8: it needs `npc_dialogue.json`, `er_recipes.tsv` and `shop.tsv` from that step.

```
python build_map.py <out folder> --dialogue npc_dialogue.json --recipes ORBS\book\er\er_recipes.tsv --shop ORBS\book\er\shop.tsv --patches eldenring-w101\patches\maps [--compare <project>]
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

### Step 8: HUD DLL data folder (`data`, `orbs`)

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

### Step 9: Build the DLL (`dll`)

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

### Step 10: Mod Engine 2 and launch (`modengine`)

In `<Mod Engine 2 folder>\config_eldenring.toml` (use your own absolute paths):

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
