//! Elden Ring item stats, icons and short texts for the spellbook, from `w101_orbs/book/er/er_item_stats.tsv` and
//! `er_item_text.tsv` (made by tools/er_book_data.py from regulation.bin and the item FMGs). Icons are
//! `w101_orbs/book/er/icons/<icon id>.png`.

use std::collections::HashMap;
use std::path::Path;

use crate::names::Names;

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
    icons: HashMap<(char, u32), u32>,
    spells: HashMap<u32, Spell>,
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
                    });
                    data.icons.insert(('P', id), num("icon") as u32);
                }
                'A' | 'E' => {
                    data.icons.insert((cat, id), num("icon") as u32);
                }
                'G' => {
                    data.icons.insert(('G', id), num("icon") as u32);
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

    pub fn spell(&self, goods_id: u32) -> Option<&Spell> {
        self.spells.get(&goods_id)
    }



    /// The tooltip for an inventory item: name, then stat lines, then the game's one-line description.
    pub fn tooltip(&self, item_id: u32, count: u32, names: &Names, equipped: bool) -> Vec<String> {
        let (cat, row) = Self::row(item_id);
        let raw = item_id & 0x0FFF_FFFF;
        let mut lines = vec![names.item(item_id)];
        let mut sub = Vec::new();
        match cat {
            'W' => {
                let level = raw % 100;
                let w = self.weapons.get(&row).or_else(|| self.weapons.get(&(row - row % 10000)));
                if let Some(w) = w {
                    let (atk_rate, sc_rate) = self.reinforce.get(&(w.reinforce + level)).copied()
                        .unwrap_or(([1.0; 5], [1.0; 5]));
                    let names = ["Phy", "Mag", "Fire", "Ligt", "Holy"];
                    let attack: Vec<String> = (0..5)
                        .filter(|&i| w.attack[i] > 0.0)
                        .map(|i| format!("{} {:.0}", names[i], w.attack[i] * atk_rate[i]))
                        .collect();
                    sub.push(if w.kind.is_empty() { "Weapon".to_string() } else { w.kind.to_string() });
                    if !attack.is_empty() {
                        sub.push(format!("Attack: {}", attack.join("  ")));
                    }
                    let stats = ["Str", "Dex", "Int", "Fai", "Arc"];
                    let scaling: Vec<String> = (0..5)
                        .filter(|&i| w.scaling[i] > 0.0)
                        .map(|i| format!("{} {}", stats[i], grade(w.scaling[i] * sc_rate[i])))
                        .collect();
                    if !scaling.is_empty() {
                        sub.push(format!("Scaling: {}", scaling.join("  ")));
                    }
                    let req: Vec<String> = (0..5)
                        .filter(|&i| w.requirements[i] > 0)
                        .map(|i| format!("{} {}", stats[i], w.requirements[i]))
                        .collect();
                    if !req.is_empty() {
                        sub.push(format!("Requires: {}", req.join("  ")));
                    }
                    sub.push(format!("Weight {:.1}", w.weight));
                }
            }
            'P' => {
                if let Some(a) = self.armour.get(&row) {
                    sub.push(["Helm", "Chest Armour", "Gauntlets", "Leg Armour"].get(a.slot as usize).unwrap_or(&"Armour")
                        .to_string());
                    sub.push(format!("Absorb: Phy {:.1}  Mag {:.1}  Fire {:.1}  Ligt {:.1}  Holy {:.1}",
                                     a.absorption[0], a.absorption[4], a.absorption[5], a.absorption[6], a.absorption[7]));
                    sub.push(format!("Resist: Imm {}  Rob {}  Foc {}  Vit {}", a.resistance[0], a.resistance[1],
                                     a.resistance[2], a.resistance[3]));
                    sub.push(format!("Poise {:.0}   Weight {:.1}", a.poise, a.weight));
                }
            }
            'G' => {
                if let Some(s) = self.spells.get(&raw) {
                    sub.push(format!("{} ({})", s.kind, s.er_school));
                    let req: Vec<String> = ["Int", "Fai", "Arc"].iter().zip(s.requirements)
                        .filter(|(_, v)| *v > 0)
                        .map(|(n, v)| format!("{n} {v}"))
                        .collect();
                    sub.push(format!("FP {}   Slots {}{}", s.fp, s.slots,
                                     if req.is_empty() { String::new() } else { format!("   Requires: {}", req.join(" ")) }));
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
        lines
    }
}
