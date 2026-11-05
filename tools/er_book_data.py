"""Elden Ring data for the hud_orbs spellbook (w101_orbs\\book\\er): item stats, short item texts, item icons and the
world map, all read from the game's own files (run it in the cloud container, which has zstandard, cryptography,
Pillow and pyooz; the game files are staged from the PC).

Usage:
  python er_book_data.py stats <regulation.bin> <Smithbox PARAM\\ER\\Defs> <item_dlc02.msgbnd.dcx> <out>
      -> out/er_item_stats.tsv, out/er_item_text.tsv
  python er_book_data.py cards <regulation.bin> <Smithbox PARAM\\ER\\Defs> <er_item_stats.tsv> <out.tsv>
      -> the same rows with each spell's (M) card= text (re)written from the params, nothing else changed
  python er_book_data.py icons <menu\\hi\\01_common.tpf.dcx> <menu\\hi\\01_common.sblytbnd.dcx> <out> [size]
      -> out/icons/<icon id>.png (every MENU_ItemIcon sprite, size px, default 128) and out/icons/map_*.png
  python er_book_data.py map-ranges <71_maptile.tpfbhd> <ranges.txt>
      -> "offset size name" of the Lands Between's map tiles at zoom levels L0 (41x41 tiles) and L2 (11x11), each in
         its fully revealed, unburnt variant (the name ends in a bitmask of map fragments); cut
         those ranges out of 71_maptile.tpfbdt on the PC into one slices file, in order
  python er_book_data.py map <ranges.txt> <slices.bin> <out>
      -> out/map/<L0|L2>_<column>_<row>.png

er_item_stats.tsv, one row per param row, `<cat>\\t<id>\\t<key>=<value>...` (cat as er_item_names.tsv: W weapon base
row (item id - upgrade level), P armour, A talisman, G goods, M spell (goods id), E Ash of War, R ReinforceParamWeapon):
  W: icon, wt, atk = physical,magic,fire,lightning,holy base attack, sc = str,dex,int,fai,arc scaling, req = the
     same requirements, rf = reinforce type (R row rf + level), kind = weapon class
  R: atk = attack rates, sc = scaling rates
  P: icon, wt, slot (0 head .. 3 legs), cut = physical,strike,slash,pierce,magic,fire,lightning,holy absorption %,
     res = immunity,robustness,focus,vitality, poise
  A: icon, wt;  E: icon;  G: icon, type (goodsType), max, rank (rarity + 1: the reagent card rank), and for crafting
     materials sg (sortGroupId: 10/15 creature parts, 20/25 plants, 30/35/50 stones) and shop (1 = a shop sells it)
  M: fp, slots, req = int,fai,arc, school (the W101 school tab: Fire Ice Storm Myth Life Death Balance Astral),
     erschool (Elden Ring's own school), card (the card text in Wizard101's card grammar, see Cards.card: the
     attack values of the cast's AtkParam_Pc rows (at 100 spell scaling), heals, buffs, wards and their seconds,
     with $dT$ $Damage$ $OT_Damage$ $Heal$ $Ward$ $Rounds$ for Wizard101's symbols)
er_item_text.tsv: `<cat>\\t<id>\\t<one-line Info text>`; category C: a cookbook's full caption (its recipes), line
breaks as "\\n".
"""

import io
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from dcx_krak import decompress  # noqa: E402
from er_params import Regulation  # noqa: E402
from er_text import bnd4, read_fmg  # noqa: E402

WEAPON_KINDS = {
    1: "Dagger", 3: "Straight Sword", 5: "Greatsword", 7: "Colossal Sword", 9: "Curved Sword", 11: "Curved Greatsword",
    13: "Katana", 14: "Twinblade", 15: "Thrusting Sword", 16: "Heavy Thrusting Sword", 17: "Axe", 19: "Greataxe",
    21: "Hammer", 23: "Great Hammer", 24: "Flail", 25: "Spear", 28: "Great Spear", 29: "Halberd", 31: "Reaper",
    35: "Fist", 37: "Claw", 39: "Whip", 41: "Colossal Weapon", 50: "Light Bow", 51: "Bow", 53: "Greatbow",
    55: "Crossbow", 56: "Ballista", 57: "Glintstone Staff", 61: "Sacred Seal", 65: "Small Shield", 67: "Medium Shield",
    69: "Greatshield", 81: "Arrow", 83: "Greatarrow", 85: "Bolt", 86: "Greatbolt", 87: "Torch", 88: "Hand-to-Hand",
    89: "Perfume Bottle", 90: "Thrusting Shield", 91: "Throwing Blade", 92: "Backhand Blade", 93: "Light Greatsword",
    94: "Great Katana", 95: "Beast Claw",
}

# Spell classes: (goodsType, sortGroupId) -> (Elden Ring's school, the book's tab). goodsType 5 / 17 = sorcery
# (attack / support), 16 / 18 = incantation; sortGroupId is the in-game sort group, one per school. The tabs are
# Elden Ring's own classes (pages.rs DECK_TABS gives each a Wizard101 school symbol): Glintstone, Carian (with crystal
# and cold), Gravity (with primeval), Night, Death (Destined Death: death sorceries, Godskin black flame and Maliketh's
# bestial incantations; with Mohg's blood and Malenia's rot), Fire (fire monks, giants, Messmer, and the magma
# sorceries, which share the night sorceries' sort group), Golden Order (with Two Fingers, Erdtree, Miquella), Dragon
# Cult, Frenzied Flame, Dragon Communion.
SCHOOLS = {
    ("S", 10): ("Glintstone", "Glintstone"), ("S", 20): ("Carian", "Carian"), ("S", 30): ("Night", "Night"),
    ("S", 40): ("Crystal and Cold", "Carian"), ("S", 50): ("Gravity and Primeval", "Gravity"), ("S", 60): ("Death", "Death"),
    ("I", 10): ("Two Fingers", "Golden Order"), ("I", 20): ("Golden Order", "Golden Order"),
    ("I", 25): ("Miquella", "Golden Order"), ("I", 30): ("Dragon Cult", "Dragon Cult"),
    ("I", 35): ("Erdtree", "Golden Order"), ("I", 40): ("Fire Monk and Giants", "Fire"),
    ("I", 45): ("Messmer's Flame", "Fire"), ("I", 50): ("Godskin and Bestial", "Death"),
    ("I", 60): ("Blood and Rot", "Death"), ("I", 70): ("Frenzied Flame", "Frenzied Flame"),
    ("I", 80): ("Dragon Communion", "Dragon Communion"),
}
# Magma Shot, Gelmir's Fury, Roiling Magma, Rykard's Rancor (sort group 30 with the night sorceries).
MAGMA = {4800, 4810, 4820, 4830}

ELEMENTS = [("atkPhys", "physical"), ("atkMag", "magic"), ("atkFire", "fire"), ("atkThun", "lightning"),
            ("atkDark", "holy")]
DEF_ELEMENTS = [("Physics", "physical"), ("Magic", "magic"), ("Fire", "fire"), ("Thunder", "lightning"),
                ("Dark", "holy")]
STATUSES = [("poizonAttackPower", "poison"), ("diseaseAttackPower", "scarlet rot"), ("bloodAttackPower", "blood loss"),
            ("freezeAttackPower", "frostbite"), ("sleepAttackPower", "sleep"), ("madnessAttackPower", "madness"),
            ("curseAttackPower", "death blight")]


def num(v):
    return f"{v:g}" if isinstance(v, float) else str(v)


def secs(v):
    return f"{round(v):d}s" if v >= 1 else f"{v:.1f}s"


class Cards:
    """Spell card texts from the params a cast uses (MagicParam refs -> Bullet chains -> AtkParam_Pc attack
    values and SpEffectParam effects), read-only from the vanilla regulation."""

    def __init__(self, magic, bullets, attacks, effects):
        self.magic, self.bullets, self.attacks, self.effects = magic, bullets, attacks, effects

    def attack(self, atk_id):
        a = self.attacks.get(atk_id)
        if not a:
            return {}, []
        dmg = {n: a[k] for k, n in ELEMENTS if a[k] > 0}
        return dmg, [a[f"spEffectId{i}"] for i in range(5) if a[f"spEffectId{i}"] > 0]

    def walk(self, bullet_id, facts, parent=None, depth=0, seen=None, area=0.0):
        seen = set() if seen is None else seen
        b = self.bullets.get(bullet_id)
        if b is None or bullet_id in seen or depth > 8:
            return
        seen.add(bullet_id)
        dmg, sp = self.attack(b["atkId_Bullet"])
        count = max(1, b["numShoot"])
        if count > 3 and b["shootAngleInterval"] != 0:
            count = 1  # a fan of projectiles: one target takes one
        if dmg and dmg != parent:
            value = sum(dmg.values())
            # Sparks and splashes a hit scatters (many small bullets) are not the spell's hit.
            if not (parent and count >= 6) and not (parent and value < 0.25 * sum(parent.values())):
                facts["hits"].append((value, count, dmg))
        for s in sp + [b[f"spEffectId{i}"] for i in range(5) if b[f"spEffectId{i}"] > 0]:
            self.effect(s, facts, target=True, area=area)
        if b["spEffectIDForShooter"] > 0:
            self.effect(b["spEffectIDForShooter"], facts, target=False)
        child = dmg or parent
        if b["HitBulletID"] > 0:
            self.walk(b["HitBulletID"], facts, child, depth + 1, seen, area)
        iv = b["intervalCreateBulletId"]
        if iv > 0:
            tick = max(b["intervalCreateTimeMin"], 0.01)
            sub = self.bullets.get(iv)
            sub_dmg = self.attack(sub["atkId_Bullet"])[0] if sub else {}
            if sub_dmg and b["life"] >= 1.0 and tick >= 0.2 and sub_dmg != dmg:
                facts["dots"].append((sum(sub_dmg.values()), tick, b["life"]))
                seen.add(iv)
                for s in self.attack(sub["atkId_Bullet"])[1] + [sub[f"spEffectId{i}"] for i in range(5)
                                                                 if sub[f"spEffectId{i}"] > 0]:
                    self.effect(s, facts, target=True, area=max(area, b["life"]))
            else:
                self.walk(iv, facts, child, depth + 1, seen, max(area, b["life"]) if b["life"] >= 1.0 else area)

    def effect(self, sp_id, facts, target, depth=0, area=0.0):
        s = self.effects.get(sp_id)
        if s is None or depth > 2 or sp_id in facts["seen_sp"]:
            return
        facts["seen_sp"].add(sp_id)
        if s["invocationConditionsStateChange1"] > 0:
            return  # only against undead or other conditions
        time = s["effectEndurance"]
        if 0 < time <= 2.0 and area > time:
            time = area  # renewed while the caster's field lasts
        for k, name in STATUSES:
            if s[k] > 0:
                facts["status"].setdefault(name, s[k])
        if s["changeHpPoint"] < 0:
            if 0 < time < 1.0:
                pass
            elif time > 0:
                facts["hots"].append((-s["changeHpPoint"], max(s["motionInterval"], 1.0), time))
            else:
                facts["heals"].append(-s["changeHpPoint"])
        elif s["changeHpRate"] < 0 and time <= 0:
            facts["heal_pct"].append(-s["changeHpRate"])
        up = {n: s[f"atkEnemyDmgCorrectRate_{k}"] for k, n in DEF_ELEMENTS if s[f"atkEnemyDmgCorrectRate_{k}"] > 1.001}
        for k, n in [("physicsAttackPowerRate", "physical"), ("magicAttackPowerRate", "magic"),
                     ("fireAttackPowerRate", "fire"), ("thunderAttackPowerRate", "lightning"),
                     ("darkAttackPowerRate", "holy")]:
            if s[k] > 1.001:
                up.setdefault(n, s[k])
        if up:
            facts["buffs"].append((up, time))
        flat = {n: s[k] for k, n in [("physicsAttackPower", "physical"), ("magicAttackPower", "magic"),
                                     ("fireAttackPower", "fire"), ("thunderAttackPower", "lightning"),
                                     ("darkAttackPower", "holy")] if s[k] > 0}
        if flat:
            facts["enchant"].append((flat, time))
        if s["atkOccurrenceSpEffectId"] > 0 and time >= 1.0:
            facts["armament"] = time
        down = {n: s[f"defEnemyDmgCorrectRate_{k}"] for k, n in DEF_ELEMENTS
                if 0 < s[f"defEnemyDmgCorrectRate_{k}"] < 0.999}
        for k, n in [("slashDamageCutRate", "slash"), ("magicDamageCutRate", "magic"), ("fireDamageCutRate", "fire"),
                     ("thunderDamageCutRate", "lightning"), ("darkDamageCutRate", "holy"),
                     ("neutralDamageCutRate", "physical")]:
            if 0 < s[k] < 0.999:
                down.setdefault(n, s[k])
        if down and (time == 0 or time >= 1.0):
            facts["wards"].append((down, time))
        if target and time >= 1.0 and not any(s[k] > 0 for k, _ in STATUSES):
            tick = s["motionInterval"] if s["motionInterval"] > 0 else 1.0
            if s["changeHpRate"] > 0:
                facts["burns"].append((s["changeHpRate"] * time / tick, time))
            elif s["changeHpPoint"] > 1:
                facts["dots"].append((s["changeHpPoint"], max(tick, 1.0), time))
        for nxt in (s["cycleOccurrenceSpEffectId"], s["atkOccurrenceSpEffectId"], s["replaceSpEffectId"]):
            if nxt > 0:
                self.effect(nxt, facts, target, depth + 1, area)

    def facts(self, goods_id):
        m = self.magic.get(goods_id)
        out = None
        for i in range(1, 11):
            cat, ref = m[f"refCategory{i}"], m[f"refId{i}"]
            if ref <= 0:
                continue
            facts = {"hits": [], "dots": [], "status": {}, "heals": [], "heal_pct": [], "hots": [], "buffs": [],
                     "enchant": [], "wards": [], "burns": [], "armament": 0, "seen_sp": set()}
            if cat == 1:
                self.walk(ref, facts)
            elif cat == 0:
                dmg, sp = self.attack(ref)
                if dmg:
                    facts["hits"].append((sum(dmg.values()), 1, dmg))
                for s in sp:
                    self.effect(s, facts, True)
            elif cat == 2:
                self.effect(ref, facts, False)
            if any(facts[k] for k in facts if k not in ("seen_sp", "armament")):
                if out is None:
                    out = facts
                else:
                    # Later references (charged casts, follow-ups) only add what the first lacks.
                    for k in ("heals", "hots", "buffs", "enchant", "wards", "status", "burns"):
                        if not out[k] and facts[k]:
                            out[k] = facts[k]
        return out

    def card(self, goods_id):
        """The spell's card text in Wizard101's card grammar (Spell.lang: "#1:$eA$ + #2:$eA4$ #3:$dT_image$#4:$Damage_image$
        over 3 #5:$Rounds_image$"), with $dT$ (the card's school), $Damage$, $OT_Damage$, $Heal$, $OT_Heal$, $Ward$ and
        $Rounds$ for the symbols; "" when the params say nothing countable. SHORT_CARDS wins (round 18: two lines)."""
        if goods_id in SHORT_CARDS:
            return SHORT_CARDS[goods_id]
        f = self.facts(goods_id)
        if f is None:
            return ""
        parts = []
        hits = f["hits"][:4]
        terms = [f"{value}" + (f" x{count}" if count > 1 else "") for value, count, _ in hits]
        line = ""
        for value, tick, life in f["dots"][:1]:
            if tick >= 1.0:
                # Wizard101's damage over time: the hit, then the total of the ticks ("100 + 210 ... over 3 rounds").
                terms.append(f"{round(value * life / tick)}")
                line = " + ".join(terms) + f" $dT${'$Damage$' if hits else '$OT_Damage$'} over {secs(life)} $Rounds$"
            elif terms:
                line = " + ".join(terms) + f" $dT$$Damage$ + {value} per hit over {secs(life)} $Rounds$"
            else:
                line = f"{value} $dT$$OT_Damage$ per hit over {secs(life)} $Rounds$"
        if terms and not line:
            line = " + ".join(terms) + " $dT$$Damage$"
        if line:
            parts.append(line)
        for pct, time in f["burns"][:1]:
            parts.append(f"+{num(round(pct, 1))}% max HP $OT_Damage$ over {secs(time)} $Rounds$")
        armament = "Armament " if f["armament"] and not f["hits"] and not f["enchant"] else ""
        for name, value in f["status"].items():
            parts.append(f"{armament}+{value} {name}")
        if f["heals"]:
            parts.append(f"{max(f['heals'])} $Heal$")
        elif f["heal_pct"]:
            parts.append(f"{num(round(max(f['heal_pct']), 1))}% $Heal$")
        for value, tick, time in f["hots"][:1]:
            parts.append(f"{value} $Heal$ per second over {secs(time)} $Rounds$")
        for up, time in f["buffs"][:2]:
            pct = sorted({round((r - 1) * 100) for r in up.values()})
            what = "all" if len(up) >= 5 else " & ".join(up)
            parts.append(f"+{'/'.join(map(str, pct))}% {what} $Damage$" + (f" for {secs(time)} $Rounds$" if time > 0 else ""))
        for flat, time in f["enchant"][:1]:
            what = " + ".join(f"{v} {n}" for n, v in flat.items())
            parts.append(f"Armament +{what} $Damage$" + (f" for {secs(time)} $Rounds$" if time > 0 else ""))
        for down, time in f["wards"][:1]:
            pct = sorted({round((1 - r) * 100) for r in down.values()})
            what = "all" if len(down) >= 5 else " & ".join(down)
            parts.append(f"$Ward$ -{'/'.join(map(str, pct))}% {what} damage taken" + (f" for {secs(time)} $Rounds$" if time > 0 else ""))
        return ", ".join(parts)


# Round 18 (Ryan C2): every card text in at most two lines of the card (pages::spell_card, SpellDescription 0.9 on
# a 97 px card), the numbers kept: the generated texts shortened (status names short, "/s", "/hit", no "for"/"over",
# no "Armament"/"damage taken" where needed) and the cards with no numbers given a short line led by their $Type_<kind>$
# icon. Keyed by goods ID; Cards.card returns these instead of its own text.
SHORT_CARDS = {
    4050: '44+49+59+69 $dT$$Damage$',
    4130: '+23% magic $Damage$ 30s $Rounds$',
    4140: '$Type_Global$ Lights the area',
    4210: '54 $dT$$OT_Damage$/hit over 5s $Rounds$',
    4220: '45+45+49+49 $dT$$Damage$',
    4361: '330 $dT$$Damage$, +270 frost',
    4370: '58 $dT$$OT_Damage$/hit over 2s $Rounds$',
    4400: '199 $dT$$Damage$, +100 frost',
    4410: '99+74 $dT$$Damage$, +80 frostbite',
    4431: '133 $dT$$Damage$, +120 frost',
    4460: '+75 magic $Damage$ for 90s $Rounds$',
    4470: '$Ward$ -30/70% all 60s $Rounds$',
    4480: '$Type_Manipulation$ Cures sleep/madness',
    4500: '127x3 +60 +51x3 $dT$$Damage$',
    4630: '$Ward$ Deflects spells',
    4640: '$Ward$ Repels spells',
    4650: '$Ward$ Draws in spells',
    4660: '$Type_Cloak$ Invisible weapon',
    4670: '$Type_Cloak$ Nearly invisible',
    4900: '200 $dT$$Damage$, +95 bleed',
    4910: '212 $dT$$Damage$, +25 bleed',
    5010: '312 $dT$$Damage$, +130 frost',
    5040: '513 $dT$$Damage$, +50 blight',
    6040: '$Type_Manipulation$ Cures poison, rot',
    6050: '+20% fire, phys $Damage$ 30s',
    6060: '$Ward$ -60% fire for 70s $Rounds$',
    6210: '244 $dT$$Damage$ +2%HP $OT_Damage$ 2s',
    6230: '255 $dT$$Damage$ +2%HP $OT_Damage$ 2s',
    6240: '106 $dT$$Damage$ +2%HP $OT_Damage$ 2s',
    6250: '+65 fire $Damage$ for 7s $Rounds$',
    6260: '$Ward$ -35% phys 70s $Rounds$',
    6270: '+2% HP $OT_Damage$ over 2s $Rounds$',
    6310: '203 $dT$$Damage$ +3 bleed',
    6320: '+40 fire $Damage$ +2 bleed 60s',
    6330: '$Ward$ -60% magic 70s $Rounds$',
    6340: '$Ward$ -30% affinity 70s',
    6400: '$Type_Damage$ Pushes foes away',
    6430: '8 $Heal$/s over 90s $Rounds$',
    6431: '12 $Heal$/s over 90s $Rounds$',
    6440: '$Type_Manipulation$ Cures poison',
    6441: '$Type_Manipulation$ Cures 3 ailments',
    6450: '$Ward$ -35% fire for 90s $Rounds$',
    6460: '$Ward$ -35% magic 90s $Rounds$',
    6470: '$Ward$ -35% lightning 90s',
    6480: '$Ward$ -35% holy 90s $Rounds$',
    6490: '$Ward$ -60% holy 70s $Rounds$',
    6500: '20+32 $dT$$Damage$ over 2s $Rounds$',
    6510: '$Type_Cloak$ Silent footsteps',
    6520: '$Type_Cloak$ Shadow lures foes',
    6530: '$Type_Cloak$ Hides you in darkness',
    6600: '+15% $Damage$ $Ward$ -10% 80s $Rounds$',
    6730: '$Type_Manipulation$ Cures all, dispels',
    6740: '$Type_Enchantment$ Shield resists more',
    6760: '$Ward$ Strikes back',
    6770: '+75 holy $Damage$ for 90s $Rounds$',
    6780: '$Type_Manipulation$ Cures blight',
    6840: '5 $Heal$/s over 120s $Rounds$',
    6850: '$Type_Manipulation$ Cures bleed, frost',
    6921: '547 $dT$$Damage$ +120 frost',
    6960: '+75 lightning $Damage$ 90s $Rounds$',
    6970: '+75 lightning $Damage$ 70s $Rounds$',
    6971: '$Type_Charm$ Lightning body',
    7020: '135 x3 $dT$$Damage$ +110 frost',
    7021: '158 $dT$$Damage$, +110 frost',
    7030: '131 x3 $dT$$Damage$ +65 rot',
    7031: '155 $dT$$Damage$, +65 rot',
    7210: '52+15 $dT$$Damage$, +40 bleed',
    7240: '40 $dT$$Damage$, +150 rot',
    7330: '22 $dT$$Damage$ +35 mad 25%/40s',
    7530: '225 $dT$$Damage$ +2%HP $OT_Damage$ 2s',
    7903: '$Ward$ -60% lightning 70s',
    2004300: '$Type_Cloak$ Hides you in haze',
    2004900: '+87 $Damage$ +25 bleed 30s',
    2004910: '63 $dT$$Damage$, +25 bleed',
    2006200: '328 $dT$$Damage$, +170 frost',
    2006210: '80+200 $dT$$Damage$ +40 frost',
    2006670: '$OT_Heal$ Heals allies near',
    2006710: '200 $dT$$Damage$ + 40/hit 2s $Rounds$',
    2006910: '$Type_Charm$ Lightning buff',
    2007200: '100 $dT$$Damage$, +80 rot',
    2007410: '214+240 $dT$$Damage$ 2s $Rounds$',
    2007810: '62 $dT$$OT_Damage$/hit over 2s $Rounds$',
}


TEXT_TABLES = {"WeaponInfo": "W", "ProtectorInfo": "P", "AccessoryInfo": "A", "GoodsInfo": "G", "GemInfo": "E",
               "MagicInfo": "M"}


def fmt(values):
    return ",".join(f"{v:g}" if isinstance(v, float) else str(v) for v in values)


def stats(regulation, defs, msgbnd, out):
    reg = Regulation(regulation, defs)
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    lines = []
    weapons = reg.param("EquipParamWeapon")
    for row_id, w in weapons.items():
        if row_id < 100 or w["iconId"] <= 0:
            continue
        lines.append(("W", row_id, {
            "icon": w["iconId"], "wt": round(w["weight"], 1),
            "atk": fmt([w["attackBasePhysics"], w["attackBaseMagic"], w["attackBaseFire"], w["attackBaseThunder"],
                        w["attackBaseDark"]]),
            "sc": fmt([round(w[k], 1) for k in ("correctStrength", "correctAgility", "correctMagic", "correctFaith",
                                                "correctLuck")]),
            "req": fmt([w[k] for k in ("properStrength", "properAgility", "properMagic", "properFaith", "properLuck")]),
            "rf": w["reinforceTypeId"], "kind": WEAPON_KINDS.get(w["wepType"], ""),
            "model": w["equipModelId"],
        }))
    for row_id, r in reg.param("ReinforceParamWeapon").items():
        lines.append(("R", row_id, {
            "atk": fmt([round(r[k], 3) for k in ("physicsAtkRate", "magicAtkRate", "fireAtkRate", "thunderAtkRate",
                                                 "darkAtkRate")]),
            "sc": fmt([round(r[k], 3) for k in ("correctStrengthRate", "correctAgilityRate", "correctMagicRate",
                                                "correctFaithRate", "correctLuckRate")]),
        }))
    for row_id, p in reg.param("EquipParamProtector").items():
        if p["iconIdM"] <= 0:
            continue
        cut = [p[k] for k in ("neutralDamageCutRate", "blowDamageCutRate", "slashDamageCutRate", "thrustDamageCutRate",
                              "magicDamageCutRate", "fireDamageCutRate", "thunderDamageCutRate", "darkDamageCutRate")]
        lines.append(("P", row_id, {
            "icon": p["iconIdM"], "wt": round(p["weight"], 1), "slot": p["protectorCategory"],
            "cut": fmt([round((1 - c) * 100, 1) for c in cut]),
            "res": fmt([p["resistPoison"] + p["resistDisease"], p["resistBlood"] + p["resistFreeze"],
                        p["resistSleep"] + p["resistMadness"], p["resistCurse"]]),
            "poise": round(p["toughnessCorrectRate"] * 1000, 1),
            # The parts file (parts\\<hd|bd|am|lg>_<m|f>_<model>) and the "#NN#" meshes of all worn parts it hides.
            "model": p["equipModelId"], "mcat": p["equipModelCategory"],
            "hide": ".".join(str(n) for n in range(96) if p.get(f"invisibleFlag_SexVer{n:02}")),
        }))
    for row_id, a in reg.param("EquipParamAccessory").items():
        if a["iconId"] > 0:
            lines.append(("A", row_id, {"icon": a["iconId"], "wt": round(a["weight"], 1)}))
    for row_id, e in reg.param("EquipParamGem").items():
        if e["iconId"] > 0:
            lines.append(("E", row_id, {"icon": e["iconId"]}))
    goods = reg.param("EquipParamGoods")
    magic = reg.param("Magic", "MagicParam")
    cards = spell_cards(reg, magic)
    # Goods some shop sells (the merchants' and the bell bearings' lineups), for the crafting page's "bought" tab.
    sold = {r["equipId"] for r in reg.param("ShopLineupParam").values() if r["equipType"] == 3}
    for row_id, g in goods.items():
        if g["iconId"] > 0:
            row = {"icon": g["iconId"], "type": g["goodsType"], "max": g["maxNum"], "rank": g["rarity"] + 1}
            if 15000 <= row_id % 2_000_000 < 22000:
                # Crafting materials: the game's sort group (10/15 creature parts, 20/25 plants, 30/35/50 stones)
                # and whether a shop sells it.
                row["sg"] = g["sortGroupId"]
                row["shop"] = int(row_id in sold)
            lines.append(("G", row_id, row))
        kind = {5: "S", 17: "S", 16: "I", 18: "I"}.get(g["goodsType"])
        m = magic.get(row_id)
        if kind and m:
            er_school, tab = SCHOOLS.get((kind, g["sortGroupId"]), ("Sorcery" if kind == "S" else "Incantation",
                                                                    "Glintstone" if kind == "S" else "Golden Order"))
            if row_id in MAGMA:
                er_school, tab = "Magma", "Fire"
            lines.append(("M", row_id, {
                "fp": m["mp"], "slots": m["slotLength"],
                "req": fmt([m["requirementIntellect"], m["requirementFaith"], m["requirementLuck"]]),
                "school": tab, "erschool": er_school, "kind": "Sorcery" if kind == "S" else "Incantation",
                "card": cards.card(row_id),
            }))
    with open(out / "er_item_stats.tsv", "w", encoding="utf-8", newline="\n") as f:
        for cat, row_id, values in lines:
            f.write("\t".join([cat, str(row_id)] + [f"{k}={v}" for k, v in values.items()]) + "\n")
    print(f"er_item_stats.tsv: {len(lines)} rows")

    files = bnd4(decompress(Path(msgbnd).read_bytes()))
    texts = {}
    for path in sorted(files, key=lambda p: ("_dlc" in p, p)):
        stem = path.replace("\\", "/").rsplit("/", 1)[-1].removesuffix(".fmg")
        cat = TEXT_TABLES.get(stem.split("_dlc")[0])
        if cat is None:
            continue
        for text_id, text in read_fmg(files[path]).items():
            text = (text or "").strip()
            if text and not text.startswith("[ERROR]"):
                texts[(cat, text_id)] = re.sub(r"\s+", " ", text)
    # The cookbooks' full captions (their recipe lists), category C, line breaks written as "\\n".
    goods_names, captions = {}, {}
    for path in sorted(files, key=lambda p: ("_dlc" in p, p)):
        stem = path.replace("\\", "/").rsplit("/", 1)[-1].removesuffix(".fmg").split("_dlc")[0]
        if stem in ("GoodsName", "GoodsCaption"):
            table = goods_names if stem == "GoodsName" else captions
            for text_id, text in read_fmg(files[path]).items():
                if text and not text.startswith("[ERROR]"):
                    table[text_id] = text.strip()
    for text_id, name in goods_names.items():
        if "Cookbook" in name and captions.get(text_id):
            texts[("C", text_id)] = captions[text_id].replace("\r", "").replace("\n", "\\n")
    with open(out / "er_item_text.tsv", "w", encoding="utf-8", newline="\n") as f:
        for (cat, text_id), text in sorted(texts.items()):
            f.write(f"{cat}\t{text_id}\t{text}\n")
    print(f"er_item_text.tsv: {len(texts)} texts")


def spell_cards(reg, magic=None):
    return Cards(magic or reg.param("Magic", "MagicParam"), reg.param("Bullet", "BulletParam"),
                 reg.param("AtkParam_Pc", "AtkParam"), reg.param("SpEffectParam", "SpEffect"))


def cards(regulation, defs, stats_tsv, out):
    """Rewrites the card= field of every spell row of an existing er_item_stats.tsv."""
    c = spell_cards(Regulation(regulation, defs))
    lines, n = [], 0
    for line in open(stats_tsv, encoding="utf-8"):
        parts = line.rstrip("\n").split("\t")
        if parts[0] == "M" and len(parts) > 1:
            parts = [p for p in parts if not p.startswith("card=")] + [f"card={c.card(int(parts[1]))}"]
            n += 1
        lines.append("\t".join(parts))
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{n} spell cards written to {out}")


def tpf_textures(data):
    """{name: DDS bytes} of a PC TPF."""
    count = struct.unpack_from("<i", data, 8)[0]
    out = {}
    for i in range(count):
        offset, size = struct.unpack_from("<Ii", data, 0x10 + 20 * i)
        name_offset = struct.unpack_from("<I", data, 0x10 + 20 * i + 12)[0]
        end = name_offset
        while data[end:end + 2] != b"\0\0":
            end += 2
        out[data[name_offset:end].decode("utf-16-le")] = data[offset:offset + size]
    return out


def icons(tpf, sblyt, out, size=128):
    from PIL import Image
    out = Path(out) / "icons"
    out.mkdir(parents=True, exist_ok=True)
    textures = tpf_textures(decompress(Path(tpf).read_bytes()))
    layouts = bnd4(decompress(Path(sblyt).read_bytes()))
    count = 0
    for path, xml in layouts.items():
        atlas = path.replace("\\", "/").rsplit("/", 1)[-1].removesuffix(".layout")
        if not (atlas.startswith("SB_Icon") or atlas.startswith("SB_MapCursor")) or atlas not in textures:
            continue
        image = Image.open(io.BytesIO(textures[atlas]))
        image.load()
        for m in re.finditer(rb'<SubTexture name="([^"]+)" x="(\d+)" y="(\d+)" width="(\d+)" height="(\d+)"', xml):
            name = m.group(1).decode()
            x, y, w, h = (int(v) for v in m.groups()[1:])
            sprite = image.crop((x, y, x + w, y + h))
            icon = re.match(r"MENU_ItemIcon_(\d+)\.png", name)
            if icon:
                sprite.resize((size, size), Image.LANCZOS).save(out / f"{int(icon.group(1))}.png", optimize=True)
            else:
                sprite.save(out / ("map_" + name.removeprefix("MENU_MAP_").removesuffix(".png") + ".png"))
            count += 1
    print(f"{count} icons in {out}")


def bhf4(data):
    """[(name, offset, size)] of a BHF4 (.tpfbhd) header."""
    assert data[:4] == b"BHF4", data[:4]
    count = struct.unpack_from("<i", data, 0x0C)[0]
    entry_size = struct.unpack_from("<q", data, 0x20)[0]
    unicode = data[0x30]
    out = []
    for i in range(count):
        at = 0x40 + i * entry_size
        # 0x24-byte entries: flags, -1, size (q), uncompressed size (q), offset (u32), id, name offset
        size = struct.unpack_from("<q", data, at + 8)[0]
        offset, _, name_offset = struct.unpack_from("<IiI", data, at + 24)
        end = name_offset
        if unicode:
            while data[end:end + 2] != b"\0\0":
                end += 2
            name = data[name_offset:end].decode("utf-16-le")
        else:
            end = data.index(b"\0", name_offset)
            name = data[name_offset:end].decode("shift-jis")
        out.append((name, offset, size))
    return out


def map_ranges(bhd, ranges):
    best = {}
    for name, offset, size in bhf4(Path(bhd).read_bytes()):
        m = re.search(r"M00_(L[02])_(\d+)_(\d+)_([0-9a-fA-F]+)\.tpf", name)
        if not m:
            continue
        mask = int(m.group(4), 16)
        key = (m.group(1), int(m.group(2)), int(m.group(3)))
        # Bits 0-14 = the map fragments (WorldMapPieceParam rows 0-14) revealed; bits 15-20 are later states of the
        # world (the ashen capital and others), left out.
        score = (mask < 1 << 15, bin(mask).count("1"), -mask)
        if key not in best or score > best[key][0]:
            best[key] = (score, offset, size, name.replace("\\", "/").rsplit("/", 1)[-1])
    with open(ranges, "w", newline="\n") as f:
        for _, offset, size, name in (best[k] for k in sorted(best)):
            f.write(f"{offset} {size} {name}\n")
    print(f"{len(best)} tiles, {sum(v[2] for v in best.values())} bytes")


def map_tiles(ranges, slices, out):
    from PIL import Image
    out = Path(out) / "map"
    out.mkdir(parents=True, exist_ok=True)
    data = Path(slices).read_bytes()
    at = 0
    for line in open(ranges):
        _, size, name = line.split()
        size = int(size)
        m = re.search(r"M00_(L[02])_(\d+)_(\d+)_", name)
        tpf = decompress(data[at:at + size])
        at += size
        (dds,) = tpf_textures(tpf).values()
        image = Image.open(io.BytesIO(dds))
        image.load()
        image.convert("RGB").save(out / f"{m.group(1)}_{int(m.group(2))}_{int(m.group(3))}.jpg", quality=88)
    print(f"map tiles in {out}")


def main():
    command = sys.argv[1]
    if command == "stats":
        stats(*sys.argv[2:6])
    elif command == "cards":
        cards(*sys.argv[2:6])
    elif command == "icons":
        icons(sys.argv[2], sys.argv[3], sys.argv[4], int(sys.argv[5]) if len(sys.argv) > 5 else 128)
    elif command == "map-ranges":
        map_ranges(sys.argv[2], sys.argv[3])
    elif command == "map":
        map_tiles(sys.argv[2], sys.argv[3], sys.argv[4])
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
