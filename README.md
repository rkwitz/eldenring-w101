<img width="2560" height="1440" alt="20261105005507_1" src="https://github.com/user-attachments/assets/eed363a7-0b73-407c-9178-bb5d0601d691" />

# Elden Ring Wizard101

A personal, offline mod that rebuilds Wizard101 inside Elden Ring. The goal is to play through Wizard101's world as an Elden Ring character: Wizard101's maps, with its mobs and bosses replaced by matching Elden Ring enemies, its quests and NPCs, its spellbook and HUD, and its music. Elden Ring supplies the player, the combat and the enemy AI. The tools in this repository bring the rest across from Wizard101.

This is a proof of concept, built one zone at a time.

## Status: Unicorn Way is the only zone so far

Wizard City's Unicorn Way is the first zone, and for now the only one. It has been rebuilt at The First Step (map tile `m60_42_36_00`); you leave the Stranded Graveyard and walk out onto the street. What works there:

- **Map:** the street in its Classic Mode look, with its own sky, lighting, collision and navmesh, and the side areas you can enter from it (the hedge maze garden, Rattlebones' room and the other boss interiors).
- **Enemies:** the street's ghosts, skeletal pirates and dark sprites are Elden Ring enemies chosen to match them, walking Wizard101's patrol paths. Rattlebones and the street's other bosses are Elden Ring bosses.
- **Quests and NPCs:** only the quest line that leads to defeating Rattlebones, plus the talk with the NPC after it. The street NPCs are Elden Ring stand-ins with Wizard101 names, portraits, dialogue and voice lines, quest markers and a shop.
- **UI and HUD:** a DLL draws Wizard101's health and mana globes, nameplates, dialogue, quest goals, floating damage numbers, loading screen and the spellbook (character, backpack, deck, map, crafting, help and settings pages) over the game.
- **Music and sound:** the street, battle and boss tracks, and Wizard101's interface sounds, in place of Elden Ring's music.

Other Wizard City zones and the worlds beyond are not started. Adding a zone means converting its map, picking its enemies, and writing its quests, the same way Unicorn Way was done.

## Code only, no game files

This repository holds **code only**: Python tools, a Rust DLL and a small C++ navmesh baker. It does not contain anything from either game, and never will. One script, `build.py`, makes every model, texture, sound and game file from your own installs of Wizard101 and Elden Ring.

---

## Legal note (read first)

- You must own both games. Neither game's files are, or ever will be, in this repository. Do not publish anything the tools write.
- Elden Ring mods are only for **offline** play. Turn Easy Anti-Cheat off (Mod Engine 2 does this). Never take a modded game online.
- Tools from third parties keep their own licences. hudhook is vendored under `hud_orbs/vendor/hudhook` with its LICENSE. Recast is zlib-licensed and is not included.

---

## How it fits together

| Part | What it does | Where |
| --- | --- | --- |
| Wizard101 conversion | Unpacks the WADs and converts zone models, textures, patrol paths, quest text, voice and music | `tools/w101_*.py`, `tools/nif2obj.py`, `tools/tex2dds.py` |
| Elden Ring assets | Builds the street, rooms, sky and collision as Elden Ring assets in Blender | `tools/blender_*.py`, `tools/flver_*.py`, `tools/hkx_*.py` |
| Maps, events, params | Places the street, enemies, NPCs and doors, writes the quest events and a minimal `regulation.bin` | `tools/msb_*.py`, `tools/build_map.py`, `tools/emevd*.py`, `tools/er_*.py`, `patches/` |
| Navmesh | Bakes a navmesh so enemies can walk the street | `tools/er_navmesh.py`, `tools/recast_bake.cpp` |
| HUD DLL | Wizard101's interface, quests, music and sound in game | `hud_orbs/` |
| Build | Runs all of the above in order | `build.py`, `config.example.toml`, [docs/BUILD_DETAILS.md](docs/BUILD_DETAILS.md) |

The steps below build the Unicorn Way zone.

---

## Build it

1. **Install the apps:** Python 3.10+, Blender with the Soulstruct for Blender add-on (`io_soulstruct`), Smithbox, UXM Selective Unpacker, Mod Engine 2, Rust with the `x86_64-pc-windows-gnu` target, and MinGW-w64 (MSYS2). The versions used are in [docs/BUILD_DETAILS.md](docs/BUILD_DETAILS.md#prerequisites). build.py installs the Python packages it needs.
2. **Unpack Elden Ring with UXM Selective Unpacker:** at least `asset`, `chr`, `event`, `map`, `material`, `menu`, `msg`, `param` and `sfx`.
3. **Prepare Wizard101:** install it, turn on **Classic Mode**, and play the Unicorn Way quest line up to Rattlebones once, talking to the NPCs. The client only downloads some files and voice lines the first time it needs them.
4. **Back up your save:** `%APPDATA%\EldenRing\<id>\ER0000.sl2`.
5. **Build:** in the repository folder, copy the config, set your paths in it, then run the build:
   ```
   copy config.example.toml config.toml
   python build.py
   ```
6. **Play:** once the build has finished, start the game with Mod Engine 2's `launchmod_eldenring.bat`. Run the build again only after you change something.

`python build.py --dry-run` shows what would run. `--only STEP` and `--from STEP` run part of the build, `--force` redoes steps that are up to date, and `--list` names the steps. Each step and the command it runs are in [docs/BUILD_DETAILS.md](docs/BUILD_DETAILS.md).

### What you provide, what you get

- **You provide:** the Elden Ring `Game` folder, unpacked (`game` in `config.toml`); the Wizard101 install (`wizard101`); a work folder (`work_root`, default `Documents\game_modding`); and where Smithbox, Mod Engine 2 and Blender are, if build.py does not find them.
- **You get:** everything under `<work_root>\reference\`. That is the Smithbox project, which is also Mod Engine's mod folder, a menu mod folder, and the HUD DLL with its data. build.py points Mod Engine 2's `config_eldenring.toml` at them.
- Nothing is written into the game or Wizard101 folders. The full tree is in [docs/BUILD_DETAILS.md](docs/BUILD_DETAILS.md#the-work-root).

> The game was checked against the Elden Ring version whose vanilla `regulation.bin` has md5 `a631bf54…`. A newer patch changes the params and maps, so `verify` will report different md5s and some steps may need adapting.

---

## Troubleshooting

| Symptom | Cause / fix |
| --- | --- |
| Game crashes at launch after a param change | A regulation.bin that Smithbox re-sorted. Rebuild it with `er_regulation.py u6b`, or restore `regulation.bin.vanilla`. |
| Enemies stand still on the street | A navmesh was replaced instead of appended. Run `er_navmesh.py --append` and check it with `navmesh_check.py`. |
| Vanilla props poke up through the street, or our own props vanish | `footprint_clear.KEEP` must list every vanilla-model part we add. Rerun footprint and dome if the dome grows more than 25%. |
| Street fades out in the distance | PartsDrawParam row 36 must be on the street part (`msb_*` scripts set it). |
| Quest progress resets on every load | Flags 1042362xxx are cleared on map load. Saved state uses 1042360xxx. |
| Pieces of the street show inside the graveyard | `graveyard_fix.tsv` is missing or was made from a different OBJ. Rerun `python build.py --only street`. |
| No HUD | Check that `external_dlls` points at the DLL. Check `orbs\w101_orbs.log`. |
| A KRAK `.dcx` will not open in Python | Run `blender_unpack.py` in Blender, or use `dcx_krak.py` (pyooz) for small files. |
| Blender scripts cannot find files | Set the paths in `config.toml`, or set `W101ER_ROOT` and `ER_GAME` before you start Blender. |
| build.py stops at `check` | It names each missing piece. Fix them, or pass `--keep-going` to run what it can. |
| A step fails halfway | Fix it, then `python build.py --from <step>`. Finished steps are skipped when up to date. |

---

## Known gaps

1. **The recorded patches** in `patches/` recreate the author's hand edits: `street/WC_Z06_Unicorn_Way_Classic_edited.objpatch.gz` (6.7 MB) and `maps/*.msbpatch.gz` (about 330 KB for 46 maps). They hold no game data on their own:
   - The OBJ patch holds references into your own converted OBJs plus Blender's float noise. Its only literal geometry is our floor and 12 other values.
   - The map patches hold byte edits to vanilla map entries, plus our own entries. `patches/maps/MD5SUMS` lists them.

   They only work on top of your own game files.
2. **Voice lines** exist only after the Wizard101 client has played them.
3. **The navmesh** is not part of build.py; see step 7 in the build details. Only appended navmeshes load.
4. **Not yet run through build.py on Windows:** the Blender steps (`unpack`, `blender`, and the fairy sheet in `orbs`), `text`, the `orbs` art and sound tools that read the larger WADs, and `dll`. Each of them was run by hand before; see *Verification*.
5. **The DLL** builds with the same code and imports, but not byte for byte (PE timestamp and import order).

---

## Verification

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

Step numbers above refer to [docs/BUILD_DETAILS.md](docs/BUILD_DETAILS.md#steps).

**build.py (2026-11-05):** a fresh clone, with only a `config.toml` pointing at copies of the real game files, ran `check`, `project`, `w101`, `street`, `regulation`, `menus`, `data`, `maps`, `orbs`, `modengine` and `verify` into an empty work root. 14 of the 18 md5s `verify` knows were built and matched, none differed, including `m60_42_36_00.msb.dcx` and `.emevd.dcx`. `er_item_names.tsv` and `idle_pose.tsv` need the Blender `unpack` step and `Game\chr`; `er_recipes.tsv` and `shop.tsv` need `er_item_names.tsv`, so the author's copies were put in for `maps`. `unpack`, `blender`, `text`, `dll`, the `orbs` tools that read the larger Wizard101 WADs, and the launch were not part of that run.
