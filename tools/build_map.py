"""Rebuild the Smithbox project's map and event files from vanilla Elden Ring (round 35, R-DOC).

Events are replayed from vanilla with our scripts, in the order they were first run. The result is byte for byte the
deployed files: m60_42_36_00.emevd.dcx b19bcc3a, common.emevd.dcx and m60.emevd.dcx.

  m60_42_36_00  reset_flags_on_load (door flag 1042368540), title_on_door_exit, midday, window_off, clear_weather,
                boss_bar x3 (Rattlebones, Lady Blackhope, Fear of Fire), boss_setup, no_guidance_wisps,
                emevd_quests.add_quests (--dialogue), sidewalk, quest_window, boss_resurface, Rattlebones' doors,
                interior doors, welcome_cards, boss_watchdog, garden doors, fog_wall, drops, cage_ride_press, idle_patch
  common        book_warps, crafting (er_recipes.tsv), shop (shop.tsv), no_leaf_gusts, no_leaves
  m60           no_leaves, midday (1060002990, 1060002991)

Maps (every *.msbpatch.gz in --patches, msb_patch.py) are vanilla plus the recorded patch. They are byte for byte the
deployed maps (m60_42_36_00.msb.dcx d4a5029e and 44 neighbours). The exception is m18_00_00_00: its content is vanilla
and equals the deployed file, but the deployed file was compressed by Smithbox's own deflate, so its .dcx bytes differ.
Why the maps need a patch is explained in msb_patch.py: Ryan's hand edits in Smithbox and the script chain are mixed.

Usage:
  python build_map.py OUT_DIR --dialogue npc_dialogue.json --recipes er_recipes.tsv --shop shop.tsv
                      [--patches DIR] [--game GAME] [--compare PROJECT]
OUT_DIR gets event/*.emevd.dcx and map/mapstudio/*.msb.dcx, laid out like the Smithbox project folder, so copying
it over the project installs it. --compare prints, for each file, whether it equals PROJECT's copy.
"""
import argparse
import hashlib
import os
import zlib
from pathlib import Path
try:  # config.toml at the repo root (tools/w101er_config.py), then the default
    from w101er_config import setting as _setting
except ImportError:
    _setting = os.environ.get

import dcx_krak
import emevd as E
import emevd_quests as EQ
import msb_patch

GAME = Path(_setting("ER_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game"))


def vanilla(path):
    data = path.read_bytes()
    if data[:4] != b"DCX\0":
        return data
    return zlib.decompress(data[0x4C:]) if data[0x28:0x2C] == b"DFLT" else dcx_krak.decompress(data)


def _bars(d):
    for boss in E.BOSSES[:3]:  # Seraph (E.BOSSES[3]) is not on the street any more (Ryan's quest scope, 2026-11-02)
        d = E.boss_bar(d, *boss)
    return d


M60_42_36 = [
    lambda d: E.reset_flags_on_load(d, 1042362990, [1042368540]),
    E.title_on_door_exit, E.midday, E.window_off, E.clear_weather, _bars,
    lambda d: E.boss_setup(d, E.BOSSES[:3]),
    E.no_guidance_wisps, EQ.add_quests, E.sidewalk, E.quest_window, E.boss_resurface,
    lambda d: E.rattlebones_doors(d, E.RATTLEBONES_DOORS),
    lambda d: E.rattlebones_doors(d, E.INTERIOR_DOORS),
    E.welcome_cards, E.boss_watchdog,
    lambda d: E.rattlebones_doors(d, E.GARDEN_DOORS),
    E.fog_wall, E.drops, EQ.cage_ride_press, EQ.idle_patch,
]


def events(game, dialogue, recipes, shop):
    EQ.DIALOGUE.update(EQ.load_dialogue(dialogue))
    rec, rows = E.read_recipes(recipes), E.read_shop(shop)
    chains = {
        "m60_42_36_00": M60_42_36,
        "common": [E.book_warps, lambda d: E.crafting(d, rec), lambda d: E.shop(d, rows), E.no_leaf_gusts,
                   lambda d: E.no_leaves(d)[0]],
        "m60": [lambda d: E.no_leaves(d)[0], lambda d: E.midday(d, (1060002990, 1060002991))],
    }
    for name, steps in chains.items():
        d = vanilla(game / "event" / f"{name}.emevd.dcx")
        for step in steps:
            d = step(d)
        yield f"event/{name}.emevd.dcx", msb_patch.dflt(d)


def maps(game, patches):
    for patch in sorted(Path(patches).glob("*.msbpatch.gz")):
        name = patch.name.split(".")[0]
        out, ok = msb_patch.apply(game / "map" / "mapstudio" / f"{name}.msb.dcx", patch)
        yield f"map/mapstudio/{name}.msb.dcx", out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("out")
    ap.add_argument("--dialogue", required=True)
    ap.add_argument("--recipes", required=True)
    ap.add_argument("--shop", required=True)
    ap.add_argument("--patches")
    ap.add_argument("--game", default=str(GAME))
    ap.add_argument("--compare")
    a = ap.parse_args()
    game, out = Path(a.game), Path(a.out)
    files = list(events(game, a.dialogue, a.recipes, a.shop))
    if a.patches:
        files += list(maps(game, a.patches))
    same = 0
    for rel, data in files:
        path = out / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        line = f"{rel}: md5 {hashlib.md5(data).hexdigest()}"
        if a.compare:
            other = Path(a.compare) / rel
            if other.exists():
                ref = other.read_bytes()
                if ref == data:
                    verdict, same = "= project", same + 1
                elif ref[:4] == b"DCX\0" and ref[0x28:0x2C] == b"DFLT" and zlib.decompress(ref[0x4C:]) == \
                        zlib.decompress(data[0x4C:]):
                    verdict = "same content as project (compressed differently)"
                else:
                    verdict = "DIFFERS from project"
            else:
                verdict = "not in project"
            line += f"  {verdict}"
        print(line)
    print(f"{len(files)} files written to {out}" + (f", {same} byte-identical to {a.compare}" if a.compare else ""))


if __name__ == "__main__":
    main()
