//! Elden Ring item and spell names from `w101_orbs/er_item_names.tsv` (made by tools/er_item_names.py from the
//! game's English FMGs): `<category>\t<id>\t<name>` lines, category W weapon, P armour, A talisman, G goods,
//! E Ash of War, M magic.

use std::collections::HashMap;
use std::path::Path;

#[derive(Default)]
pub struct Names {
    names: HashMap<(char, u32), String>,
}

impl Names {
    pub fn load(path: &Path) -> Names {
        let mut names = HashMap::new();
        for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
            let mut parts = line.splitn(3, '\t');
            let (Some(category), Some(id), Some(name)) = (parts.next(), parts.next(), parts.next()) else { continue };
            let (Some(category), Ok(id)) = (category.chars().next(), id.parse::<u32>()) else { continue };
            names.insert((category, id), name.to_string());
        }
        Names { names }
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The name of an item ID with its category bits (as in the inventory), with "+N" for upgraded weapons.
    pub fn item(&self, item_id: u32) -> String {
        let raw = item_id & 0x0FFF_FFFF;
        match item_id >> 28 {
            0 => self.weapon(raw as i32),
            1 => self.lookup('P', raw),
            2 => self.lookup('A', raw),
            4 => self.lookup('G', raw),
            8 => self.lookup('E', raw),
            _ => format!("#{item_id:x}"),
        }
    }

    /// A weapon param ID: affinity variants have their own names; the last two digits are the upgrade level.
    pub fn weapon(&self, id: i32) -> String {
        if id < 0 {
            return String::new();
        }
        let id = id as u32;
        let level = id % 100;
        let name = self
            .names
            .get(&('W', id - level))
            .or_else(|| self.names.get(&('W', id - id % 10000)))
            .cloned()
            .unwrap_or_else(|| format!("Weapon {id}"));
        if level > 0 { format!("{name} +{level}") } else { name }
    }

    /// A spell (magic param ID); spells share their IDs with the goods that teach them.
    pub fn spell(&self, id: i32) -> String {
        if id < 0 {
            return String::new();
        }
        self.names
            .get(&('G', id as u32))
            .or_else(|| self.names.get(&('M', id as u32)))
            .cloned()
            .unwrap_or_else(|| format!("Spell {id}"))
    }

    pub fn lookup(&self, category: char, id: u32) -> String {
        self.names.get(&(category, id)).cloned().unwrap_or_else(|| format!("{category} {id}"))
    }

    pub fn find(&self, category: char, id: u32) -> Option<&str> {
        self.names.get(&(category, id)).map(String::as_str)
    }
}
