//! Elden Ring item stats, icons and short texts for the spellbook, from `w101_orbs/book/er/er_item_stats.tsv` and
//! `er_item_text.tsv` (made by tools/er_book_data.py from regulation.bin and the item FMGs). Icons are
//! `w101_orbs/book/er/icons/<icon id>.png`.

use std::collections::HashMap;
use std::path::Path;

use crate::names::Names;

/// Wizard101's "all schools" icon and its damage and resist stat icons (GUI/Icons).
const ICON_ALL: &str = "GUI/Icons/Icon_All.png";
const TYPE_DAMAGE: &str = "GUI/Icons/Type_Damage.png";
const TYPE_RESIST: &str = "GUI/Icons/Type_Resist.png";

/// Every icon a tooltip row can show (for loading).
pub const TIP_ICONS: [&str; 14] = [
    ICON_ALL, TYPE_DAMAGE, TYPE_RESIST, "GUI/Icons/Type_Stun_Resist.png", "GUI/Icons/Type_Pip.png",
    "GUI/Icons/Type_PowerPips.png", "GUI/Icons/Icon_SM_Myth.png", "GUI/Icons/Icon_SM_Fire.png",
    "GUI/Icons/Icon_SM_Storm.png", "GUI/Icons/Icon_SM_Life.png", "GUI/Icons/Icon_SM_Ice.png",
    "GUI/Icons/Icon_SM_Death.png", "GUI/Icons/Icon_SM_Balance.png", "GUI/Icons/Icon_Shadow.png",
];

/// A tooltip stat row as Wizard101 writes them ("+45" then the school's icon and the stat's icon, the stat's name
/// being the icons' own hover text): "@value|school icon|stat icon|name". book.rs draws it.
pub fn stat_row(value: &str, school: &str, stat: &str, name: &str) -> String {
    format!("@{value}|{school}|{stat}|{name}")
}

#[derive(Clone, Copy, Default)]
pub struct Weapon {
    pub weight: f32,
    /// Physical, magic, fire, lightning, holy.
    pub attack: [f32; 5],
    /// Strength, dexterity, intelligence, faith, arcane.
    pub scaling: [f32; 5],
    pub requirements: [u32; 5],
    pub reinforce: u32,
    pub kind: &'static str,
    /// The parts model (wp_a_<model>).
    pub model: u32,
}

#[derive(Clone, Copy, Default)]
pub struct Armour {
    pub weight: f32,
    pub slot: u32,
    /// Physical, strike, slash, pierce, magic, fire, lightning, holy absorption in percent.
    pub absorption: [f32; 8],
    /// Immunity, robustness, focus, vitality.
    pub resistance: [u32; 4],
    pub poise: f32,
    /// The parts model and kind (equipModelCategory: 5 head, 2 body, 1 arms, 6 legs).
    pub model: u32,
    pub model_kind: u32,
    /// invisibleFlag_SexVerNN bits: the `#NN#` meshes of all worn parts this piece hides.
    pub hide: u128,
}

#[derive(Clone, Default)]
pub struct Spell {
    pub fp: u32,
    pub slots: u32,
    /// Intelligence, faith, arcane.
    pub requirements: [u32; 3],
    /// The Wizard101 school tab (Fire Ice Storm Myth Life Death Balance Astral) and Elden Ring's own school.
    pub school: String,
    pub er_school: String,
    pub kind: String,
}

#[derive(Default)]
pub struct ItemData {
    weapons: HashMap<u32, Weapon>,
    reinforce: HashMap<u32, ([f32; 5], [f32; 5])>,
    armour: HashMap<u32, Armour>,
    talisman_weights: HashMap<u32, f32>,
    icons: HashMap<(char, u32), u32>,
    spells: HashMap<u32, Spell>,
    /// Goods rarity + 1 (the reagent cards' rank).
    ranks: HashMap<u32, u32>,
    text: HashMap<(char, u32), String>,
    kinds: Vec<&'static str>,
}

fn list<const N: usize, T: std::str::FromStr + Default + Copy>(text: &str) -> [T; N] {
    let mut out = [T::default(); N];
    for (o, v) in out.iter_mut().zip(text.split(',')) {
        *o = v.trim().parse().unwrap_or_default();
    }
    out
}

/// Elden Ring's scaling letter for a scaling value.
pub fn grade(value: f32) -> &'static str {
    match value {
        v if v >= 175.0 => "S",
        v if v >= 140.0 => "A",
        v if v >= 90.0 => "B",
        v if v >= 60.0 => "C",
        v if v >= 25.0 => "D",
        v if v > 0.0 => "E",
        _ => "-",
    }
}

impl ItemData {
    pub fn load(folder: &Path) -> ItemData {
        let mut data = ItemData::default();
        let text = std::fs::read_to_string(folder.join("er_item_stats.tsv")).unwrap_or_default();
        for line in text.lines() {
            let mut parts = line.split('\t');
            let (Some(cat), Some(id)) = (parts.next(), parts.next().and_then(|i| i.parse::<u32>().ok())) else { continue };
            let fields: HashMap<&str, &str> = parts.filter_map(|p| p.split_once('=')).collect();
            let get = |k: &str| fields.get(k).copied().unwrap_or("");
            let num = |k: &str| get(k).parse::<f32>().unwrap_or(0.0);
            let cat = cat.chars().next().unwrap_or(' ');
            match cat {
                'W' => {
                    let kind = data.intern(get("kind"));
                    data.weapons.insert(id, Weapon {
                        weight: num("wt"),
                        attack: list(get("atk")),
                        scaling: list(get("sc")),
                        requirements: list(get("req")),
                        reinforce: num("rf") as u32,
                        kind,
                        model: num("model") as u32,
                    });
                    data.icons.insert(('W', id), num("icon") as u32);
                }
                'R' => {
                    data.reinforce.insert(id, (list(get("atk")), list(get("sc"))));
                }
                'P' => {
                    data.armour.insert(id, Armour {
                        weight: num("wt"),
                        slot: num("slot") as u32,
                        absorption: list(get("cut")),
                        resistance: list(get("res")),
                        poise: num("poise"),
                        model: num("model") as u32,
                        model_kind: num("mcat") as u32,
                        hide: get("hide").split('.').filter_map(|n| n.parse::<u32>().ok()).filter(|n| *n < 128)
                            .fold(0, |m, n| m | (1u128 << n)),
                    });
                    data.icons.insert(('P', id), num("icon") as u32);
                }
                'A' | 'E' => {
                    if cat == 'A' {
                        data.talisman_weights.insert(id, num("wt"));
                    }
                    data.icons.insert((cat, id), num("icon") as u32);
                }
                'G' => {
                    data.icons.insert(('G', id), num("icon") as u32);
                    if num("rank") > 0.0 {
                        data.ranks.insert(id, num("rank") as u32);
                    }
                }
                'M' => {
                    data.spells.insert(id, Spell {
                        fp: num("fp") as u32,
                        slots: num("slots") as u32,
                        requirements: list(get("req")),
                        school: get("school").to_string(),
                        er_school: get("erschool").to_string(),
                        kind: get("kind").to_string(),
                    });
                }
                _ => {}
            }
        }
        let text = std::fs::read_to_string(folder.join("er_item_text.tsv")).unwrap_or_default();
        for line in text.lines() {
            let mut parts = line.splitn(3, '\t');
            let (Some(cat), Some(id), Some(t)) = (parts.next(), parts.next(), parts.next()) else { continue };
            if let (Some(cat), Ok(id)) = (cat.chars().next(), id.parse::<u32>()) {
                data.text.insert((cat, id), t.to_string());
            }
        }
        tracing::info!("item data: {} weapons, {} armour, {} spells, {} texts", data.weapons.len(), data.armour.len(),
                       data.spells.len(), data.text.len());
        data
    }

    /// A cookbook's full caption: what it is, then the recipes it teaches, one per line.
    pub fn caption(&self, goods_id: u32) -> Option<String> {
        self.text.get(&('C', goods_id)).map(|t| t.replace("\\n", "\n"))
    }

    fn intern(&mut self, s: &str) -> &'static str {
        if let Some(k) = self.kinds.iter().find(|k| **k == s) {
            return k;
        }
        let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
        self.kinds.push(leaked);
        leaked
    }


    /// The param row and category letter of an inventory item ID (category bits on top).
    fn row(item_id: u32) -> (char, u32) {
        let raw = item_id & 0x0FFF_FFFF;
        match item_id >> 28 {
            0 => ('W', raw - raw % 100),
            1 => ('P', raw),
            2 => ('A', raw),
            4 => ('G', raw),
            8 => ('E', raw),
            _ => (' ', raw),
        }
    }

    /// The art name (relative to the book folder's `art`) of an item's icon.
    pub fn icon(&self, item_id: u32) -> Option<String> {
        let key = Self::row(item_id);
        let icon = self.icons.get(&key).or_else(|| {
            // Weapons: affinity rows share the base weapon's icon.
            (key.0 == 'W').then(|| self.icons.get(&('W', key.1 - key.1 % 10000))).flatten()
        })?;
        Some(format!("@er/icons/{icon}.png"))
    }

    /// A weapon's kind ("Dagger", "Small Shield", "Arrow", ...), from its param ID.
    pub fn weapon_kind(&self, weapon_id: u32) -> Option<&'static str> {
        self.weapons.get(&(weapon_id - weapon_id % 100)).or_else(|| self.weapons.get(&(weapon_id - weapon_id % 10000)))
            .map(|w| w.kind)
    }

    /// A weapon's parts model (wp_a_<model>), from its param ID (with or without upgrade / affinity).
    pub fn weapon_model(&self, weapon_id: u32) -> Option<u32> {
        let row = weapon_id - weapon_id % 100;
        let w = self.weapons.get(&row).or_else(|| self.weapons.get(&(row - row % 10000)))?;
        (w.model > 0).then_some(w.model)
    }

    /// A weapon's weight, from its param ID (with or without upgrade / affinity).
    pub fn weapon_weight(&self, weapon_id: u32) -> f32 {
        let row = weapon_id - weapon_id % 100;
        self.weapons.get(&row).or_else(|| self.weapons.get(&(row - row % 10000))).map_or(0.0, |w| w.weight)
    }

    pub fn armour(&self, protector_id: u32) -> Option<&Armour> {
        self.armour.get(&protector_id)
    }

    pub fn talisman_weight(&self, accessory_id: u32) -> f32 {
        self.talisman_weights.get(&accessory_id).copied().unwrap_or(0.0)
    }

    /// An armour piece's parts file stem (e.g. "bd_m_1520") and hidden mesh groups.
    pub fn armour_model(&self, protector_id: u32) -> Option<(String, u128)> {
        let a = self.armour.get(&protector_id)?;
        let prefix = match a.model_kind {
            5 => "hd",
            2 => "bd",
            1 => "am",
            6 => "lg",
            _ => return None,
        };
        Some((format!("{prefix}_m_{:04}", a.model), a.hide))
    }

    pub fn spell(&self, goods_id: u32) -> Option<&Spell> {
        self.spells.get(&goods_id)
    }



    /// The tooltip for an inventory item, in Wizard101's item tooltip order: the name, the stat rows (each a
    /// value and its school and stat icons, see `stat_row`), the requirement lines, then the game's description.
    pub fn tooltip(&self, item_id: u32, count: u32, names: &Names, equipped: bool) -> Vec<String> {
        let (cat, row) = Self::row(item_id);
        let raw = item_id & 0x0FFF_FFFF;
        let mut lines = vec![names.item(item_id)];
        let mut sub = Vec::new();
        // Elden Ring's damage types as Wizard101 schools: physical = all schools, magic = Myth (glintstone),
        // fire = Fire, lightning = Storm, holy = Life.
        const TYPES: [(&str, &str); 5] =
            [("Physical", ICON_ALL), ("Magic", "GUI/Icons/Icon_SM_Myth.png"), ("Fire", "GUI/Icons/Icon_SM_Fire.png"),
             ("Lightning", "GUI/Icons/Icon_SM_Storm.png"), ("Holy", "GUI/Icons/Icon_SM_Life.png")];
        let short = ["Str", "Dex", "Int", "Fai", "Arc"];
        match cat {
            'W' => {
                let level = raw % 100;
                let w = self.weapons.get(&row).or_else(|| self.weapons.get(&(row - row % 10000)));
                if let Some(w) = w {
                    let (atk_rate, sc_rate) = self.reinforce.get(&(w.reinforce + level)).copied()
                        .unwrap_or(([1.0; 5], [1.0; 5]));
                    for i in (0..5).filter(|&i| w.attack[i] > 0.0) {
                        sub.push(stat_row(&format!("+{:.0}", w.attack[i] * atk_rate[i]), TYPES[i].1, TYPE_DAMAGE,
                                          &format!("{} Damage", TYPES[i].0)));
                    }
                    let scaling: Vec<String> = (0..5).filter(|&i| w.scaling[i] > 0.0)
                        .map(|i| format!("{} {}", short[i], grade(w.scaling[i] * sc_rate[i]))).collect();
                    if !scaling.is_empty() {
                        sub.push(format!("Scaling {}", scaling.join(" ")));
                    }
                    let req: Vec<String> = (0..5).filter(|&i| w.requirements[i] > 0)
                        .map(|i| format!("{} {}", short[i], w.requirements[i])).collect();
                    if !req.is_empty() {
                        sub.push(format!("Requires {}", req.join(" ")));
                    }
                    sub.push(format!("{} - Weight {:.1}", if w.kind.is_empty() { "Weapon" } else { w.kind }, w.weight));
                }
            }
            'P' => {
                if let Some(a) = self.armour.get(&row) {
                    for (t, i) in [(0, 0), (1, 4), (2, 5), (3, 6), (4, 7)] {
                        if a.absorption[i] > 0.0 {
                            sub.push(stat_row(&format!("+{:.1}%", a.absorption[i]), TYPES[t].1, TYPE_RESIST,
                                              &format!("{} Resist", TYPES[t].0)));
                        }
                    }
                    if a.poise > 0.0 {
                        sub.push(stat_row(&format!("+{:.0}", a.poise), "", "GUI/Icons/Type_Stun_Resist.png", "Poise"));
                    }
                    let res: Vec<String> = ["Imm", "Rob", "Foc", "Vit"].iter().zip(a.resistance)
                        .filter(|(_, v)| *v > 0).map(|(n, v)| format!("{n} {v}")).collect();
                    if !res.is_empty() {
                        sub.push(res.join(" "));
                    }
                    let kind = ["Helm", "Chest Armour", "Gauntlets", "Leg Armour"].get(a.slot as usize).unwrap_or(&"Armour");
                    sub.push(format!("{kind} - Weight {:.1}", a.weight));
                }
            }
            'G' => {
                if let Some(s) = self.spells.get(&raw) {
                    // The spell's school as the deck's Wizard101 school.
                    let k = s.school.as_str();
                    let school = [("Glintstone", "Icon_SM_Myth"), ("Carian", "Icon_SM_Ice"), ("Crystal", "Icon_SM_Ice"),
                                  ("Gravity", "Icon_SM_Balance"), ("Night", "Icon_Shadow"), ("Death", "Icon_SM_Death"),
                                  ("Blood", "Icon_SM_Death"), ("Godskin", "Icon_SM_Death"), ("Fire", "Icon_SM_Fire"),
                                  ("Magma", "Icon_SM_Fire"), ("Messmer", "Icon_SM_Fire"), ("Golden", "Icon_SM_Life"),
                                  ("Erdtree", "Icon_SM_Life"), ("Two Fingers", "Icon_SM_Life"), ("Miquella", "Icon_SM_Life"),
                                  ("Dragon Cult", "Icon_SM_Storm"), ("Dragon Communion", "Icon_SM_Death")]
                        .iter().find(|(key, _)| k.contains(key)).map_or(ICON_ALL.to_string(), |(_, i)| format!("GUI/Icons/{i}.png"));
                    sub.push(stat_row(&s.fp.to_string(), &school, "GUI/Icons/Type_Pip.png", "FP Cost"));
                    sub.push(stat_row(&s.slots.to_string(), "", "GUI/Icons/Type_PowerPips.png", "Memory Slots"));
                    let req: Vec<String> = ["Int", "Fai", "Arc"].iter().zip(s.requirements).filter(|(_, v)| *v > 0)
                        .map(|(n, v)| format!("{n} {v}")).collect();
                    if !req.is_empty() {
                        sub.push(format!("Requires {}", req.join(" ")));
                    }
                    sub.push(format!("{} - {}", s.er_school, s.kind));
                }
            }
            _ => {}
        }
        if count > 1 {
            sub.push(format!("Owned: {count}"));
        }
        if equipped {
            sub.push("Equipped".to_string());
        }
        lines.extend(sub);
        let text_key = match cat {
            'W' => ('W', row - row % 10000),
            other => (other, raw),
        };
        if let Some(t) = self.text.get(&text_key).or_else(|| self.text.get(&(cat, row))) {
            lines.push(t.clone());
        }
        // A cookbook: the recipes it teaches.
        let recipes = if cat == 'G' { self.recipes(raw) } else { vec![] };
        if !recipes.is_empty() {
            lines.push(format!("Recipes ({}):", recipes.len()));
            lines.extend(recipes.into_iter().map(|r| format!("- {r}")));
        }
        lines
    }

    /// The recipes a cookbook teaches (its caption's "- Name" lines after "Acquire the knowledge to craft the
    /// following:"), and the caption's description before them.
    pub fn cookbook(&self, goods_id: u32) -> Option<(String, Vec<String>)> {
        let text = self.caption(goods_id)?.replace('\r', "");
        let (about, list) = match text.find("Acquire the knowledge") {
            Some(i) => (text[..i].trim().to_string(), text[i..].to_string()),
            None => (text.trim().to_string(), text.clone()),
        };
        let recipes = list.lines().filter_map(|l| l.trim().strip_prefix('-')).map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty()).collect();
        Some((about, recipes))
    }

    /// A goods item's rank (its rarity + 1), 1 when unknown.
    pub fn rank(&self, goods_id: u32) -> u32 {
        self.ranks.get(&goods_id).copied().unwrap_or(1)
    }

    pub fn recipes(&self, goods_id: u32) -> Vec<String> {
        self.cookbook(goods_id).map(|c| c.1).unwrap_or_default()
    }
}
