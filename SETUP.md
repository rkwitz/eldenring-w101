# Setup

See README.md for the full step-by-step build. This file only lists the apps.

Everything needed to work on Elden Ring Wizard101, laid out under `Documents\game_modding`.

## Folder layout

| Path | What goes there | In Git? |
| --- | --- | --- |
| `eldenring-w101\` | Our code, scripts and docs | Yes |
| `reference\w101\` | Files extracted from your Wizard101 install (WADs, `.nif` models, textures) | Never |
| `reference\eldenring\` | Elden Ring files we export or edit (FLVER, collision, MSB copies) | Never |
| `reference\repos\` | Cloned source of community tools, for reading and scripting | No |

Cloned in `reference\repos\`: `wizwad`, `KiWad-Unpacker`, `soulstruct`, `soulstruct-blender`.

## Games

1. **Wizard101**: installed at `C:\ProgramData\KingsIsle Entertainment\Wizard101`. Game data is in `Data\GameData` (`.wad` files).
2. **Elden Ring**: installed at `C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING`. Mods run offline only, with Easy Anti-Cheat off.

Already copied into `reference\w101\wads\`: `WizardCity-WC_Streets-WC_Unicorn.wad` (the Unicorn Way zone) and `WizardCity-WorldData.wad` (shared Wizard City models). The zone is unpacked in `reference\w101\extracted\WC_Unicorn\`.

## Apps to install (Windows release downloads, not source)

| App | Used for | Get it |
| --- | --- | --- |
| Blender 5.1 or later | Assembling the map, exporting FLVER | https://www.blender.org/download/ |
| Soulstruct for Blender (release `.zip`) | FLVER and collision import/export in Blender | https://github.com/Grimrukh/soulstruct-blender/releases |
| NifSkope 2.0 | Opening Wizard101 `.nif` models, exporting OBJ | https://github.com/niftools/nifskope/releases |
| Smithbox | Elden Ring map editor, assets, collision generator | https://github.com/vawser/Smithbox/releases |
| UXM Selective Unpacker | Unpacking Elden Ring's archives so Smithbox can edit them | https://github.com/Nordgaren/UXM-Selective-Unpack/releases |
| Mod Engine 2 | Launching Elden Ring with our mod folder | https://github.com/soulsmods/ModEngine2/releases |
| Python 3.10+ | Running wizwad (`pip install wizwad`) | https://www.python.org/downloads/ |

Install the Soulstruct add-on in Blender under `Edit > Preferences > Add-ons > Install from Disk`.
