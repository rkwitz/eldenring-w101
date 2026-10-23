//! What each spellbook page shows: Wizard101's own page layout (layout.rs) filled with the closest Elden Ring data.
//!
//! Every page keeps Wizard101's art, positions, fonts and colours; these functions only pick which of the layout's
//! sub-pages are shown, put Elden Ring text where Wizard101 puts its own, and grey out buttons that have nothing to
//! do here (Wizard101 greys buttons the same way when they don't apply).

use hudhook::imgui::ImColor32;

use crate::game::{self, Sheet};
use crate::layout::{Layout, TextStyle, View};
use crate::names::Names;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tab {
    Character,
    Backpack,
    Pets,
    Deck,
    Quests,
    Map,
    Crafting,
    Help,
    Settings,
    Close,
}

/// (tab, button in SpellBookPrefsWindow.gui, page layout), top to bottom.
pub const TABS: [(Tab, &str, &str); 10] = [
    (Tab::Character, "CharStats", "CharStatsSpellbookPage"),
    (Tab::Backpack, "Inventory", "InventorySpellbookPage"),
    (Tab::Pets, "PetStats", "PetStatsSpellbookPage"),
    (Tab::Deck, "Deck", "DeckConfiguration"),
    (Tab::Quests, "Quest", "QuestListWindow"),
    (Tab::Map, "Maps", "MapSpellbookPage"),
    (Tab::Crafting, "Crafting", "RecipeWindow"),
    (Tab::Help, "Help", "HelpSpellbookPage"),
    (Tab::Settings, "Preferences", "Settings"),
    (Tab::Close, "Close_Button", ""),
];

/// The quest card template drawn into each of QuestListWindow's four card windows.
pub const QUEST_CARD: &str = "QuestInfoWindow";

const BLACK: ImColor32 = ImColor32::from_rgb(0, 0, 0);
const YELLOW: ImColor32 = ImColor32::from_rgb(255, 255, 0);
/// Wizard101's button label yellow (FF80FFFF).
const LIGHT_YELLOW: ImColor32 = ImColor32::from_rgb(255, 255, 128);
const WHITE: ImColor32 = ImColor32::from_rgb(255, 255, 255);

/// Per-tab choices the player makes in the book.
#[derive(Default)]
pub struct State {
    /// List page on the current tab.
    pub page: usize,
    /// Sub-page or category on the current tab (character page, backpack category, spell filter, help topic,
    /// settings row).
    pub sub: usize,
    /// The chosen pet (spirit ash).
    pub pet: usize,
    /// Crafting: show cookbooks instead of materials.
    pub cookbooks: bool,
    /// The layout node of the chosen filter button (deck), to show it pressed.
    pub filter_node: Option<usize>,
    /// How many subs/pages the current tab has (for wrapping), set while binding.
    subs: usize,
    /// How many pets there are to page through.
    pets: usize,
}

pub enum Action {
    None,
    OpenMap,
    Close,
    Tab(Tab),
    /// Settings row, step (-1 or +1).
    Setting(usize, i32),
}

/// Everything the pages show.
pub struct Data<'a> {
    pub sheet: Option<&'a Sheet>,
    pub stats: Option<game::Stats>,
    pub names: &'a Names,
    /// (label, value) per settings row.
    pub settings: Vec<(String, String)>,
    pub map_opening: bool,
}

impl State {
    pub fn reset(&mut self) {
        self.page = 0;
        self.sub = 0;
        self.filter_node = None;
    }

    pub fn step_sub(&mut self, delta: i32) {
        let n = self.subs.max(1) as i32;
        self.sub = ((self.sub as i32 + delta).rem_euclid(n)) as usize;
        self.page = 0;
        self.filter_node = None;
    }

    pub fn step_page(&mut self, delta: i32) {
        self.page = (self.page as i32 + delta).max(0) as usize;
    }
}

fn centre(text: &str) -> String {
    format!("<center>{text}</center>")
}

fn hide(l: &Layout, v: &mut View, names: &[&str]) {
    for n in names {
        v.show(l.find(n), false);
    }
}

fn grey(l: &Layout, v: &mut View, names: &[&str]) {
    for n in names {
        if let Some(id) = l.find(n) {
            v.greyed.insert(id);
        }
    }
}

/// Moves a tab marker (Tab_Top03) under a button, keeping its offset from the first button.
fn marker(l: &Layout, v: &mut View, marker: &str, first: &str, chosen: Option<usize>) {
    let (Some(m), Some(f), Some(c)) = (l.find(marker), l.find(first), chosen) else { return };
    let (mr, fr, cr) = (l.nodes[m].rect, l.nodes[f].rect, l.nodes[c].rect);
    v.rect.insert(m, [cr[0] + mr[0] - fr[0], cr[1] + mr[1] - fr[1], cr[0] + mr[2] - fr[0], cr[1] + mr[3] - fr[1]]);
}

/// "p/n" in a page number box, greying the arrows at the ends; returns the clamped page.
fn paging(l: &Layout, v: &mut View, st: &mut State, count: usize, per_page: usize, ids: (&str, &str, &str)) -> usize {
    let pages = count.div_ceil(per_page.max(1)).max(1);
    st.page = st.page.min(pages - 1);
    v.set(l.find(ids.0), centre(&format!("{}/{}", st.page + 1, pages)));
    if st.page == 0 {
        grey(l, v, &[ids.1]);
    }
    if st.page + 1 >= pages {
        grey(l, v, &[ids.2]);
    }
    st.page
}

fn sorcery(item_id: u32) -> bool {
    let raw = item_id & 0x0FFF_FFFF;
    item_id >> 28 == 4 && ((4000..6000).contains(&raw) || (2_004_000..2_006_000).contains(&raw))
}

pub fn is_spell(item_id: u32) -> bool {
    let raw = item_id & 0x0FFF_FFFF;
    item_id >> 28 == 4 && ((4000..8000).contains(&raw) || (2_004_000..2_008_000).contains(&raw))
}

fn is_material(item_id: u32) -> bool {
    let raw = item_id & 0x0FFF_FFFF;
    item_id >> 28 == 4 && ((15000..22000).contains(&raw) || (2_015_000..2_022_000).contains(&raw))
}

fn is_spirit_ash(name: &str) -> bool {
    name.contains("Ashes") || name.contains("Puppet")
}

/// Inventory item IDs (with their category bits) of what the player has equipped.
fn equipped_ids(sheet: &Sheet) -> Vec<u32> {
    let e = &sheet.equipment;
    let mut out = Vec::new();
    for slot in 0..12 {
        if e[slot] > 0 && e[slot] != 110000 {
            out.push(e[slot] as u32);
        }
    }
    for slot in 12..16 {
        if e[slot] >= 0 {
            out.push(0x1000_0000 | e[slot] as u32);
        }
    }
    for slot in 17..21 {
        if e[slot] > 0 {
            out.push(0x2000_0000 | e[slot] as u32);
        }
    }
    out
}

/// (slot, item) lines of the equipment.
fn equipment_lines(sheet: &Sheet, names: &Names) -> Vec<(String, String)> {
    let e = &sheet.equipment;
    let weapon = |id: i32| if id <= 0 || id == 110000 { "-".to_string() } else { names.weapon(id) };
    let mut out = Vec::new();
    for (label, slot) in [("Right 1", 1), ("Right 2", 3), ("Right 3", 5), ("Left 1", 0), ("Left 2", 2), ("Left 3", 4)] {
        out.push((label.to_string(), weapon(e[slot])));
    }
    for (label, slot) in [("Head", 12), ("Chest", 13), ("Arms", 14), ("Legs", 15)] {
        let id = e[slot];
        let name = if id < 0 { "-".to_string() } else { names.find('P', id as u32).unwrap_or("-").to_string() };
        out.push((label.to_string(), name));
    }
    for (i, slot) in (17..=20).enumerate() {
        let id = e[slot];
        out.push((format!("Talisman {}", i + 1), if id <= 0 { "-".to_string() } else { names.lookup('A', id as u32) }));
    }
    for (label, slot) in [("Arrows", 6), ("Bolts", 7)] {
        out.push((label.to_string(), weapon(e[slot])));
    }
    out
}

/// Fills the page for `tab`.
pub fn bind(tab: Tab, l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    match tab {
        Tab::Character => character(l, d, st, v),
        Tab::Backpack => backpack(l, d, st, v),
        Tab::Pets => pets(l, d, st, v),
        Tab::Deck => deck(l, d, st, v),
        Tab::Quests => quests(l, st, v),
        Tab::Map => map(l, d, v),
        Tab::Crafting => crafting(l, d, st, v),
        Tab::Help => help(l, st, v),
        Tab::Settings => settings(l, d, st, v),
        Tab::Close => {}
    }
}

/// What a click on page node `id` does.
pub fn click(tab: Tab, l: &Layout, id: usize, st: &mut State) -> Action {
    let name = l.nodes[id].name.as_str();
    let parent = l.nodes[id].parent.map(|p| l.nodes[p].name.as_str()).unwrap_or("");
    match (tab, name) {
        (_, "PageBack" | "leftscroll" | "PageUp" | "btnPrevPage" | "AdvancedStatsPageBack") => st.step_page(-1),
        (_, "PageForward" | "rightscroll" | "PageDown" | "btnNextPage" | "AdvancedStatsPageForward") => st.step_page(1),
        (Tab::Character, "StatsTabButton") => (st.sub, st.page) = (0, 0),
        (Tab::Character, "BadgeTabButton") => (st.sub, st.page) = (1, 0),
        (Tab::Backpack, _) if name.starts_with("Tab_") => {
            if let Some(i) = BACKPACK.iter().position(|(n, _, _)| *n == name) {
                (st.sub, st.page) = (i, 0);
            }
        }
        (Tab::Pets, "btnPrevPet") => st.pet = (st.pet + st.pets.max(1) - 1) % st.pets.max(1),
        (Tab::Pets, "btnNextPet") => st.pet = (st.pet + 1) % st.pets.max(1),
        (Tab::Deck, _) if name.starts_with("Cards_") => {
            st.sub = match name {
                "Cards_All" => 0,
                "Cards_Ice" | "Cards_Storm" | "Cards_Myth" => 1,
                _ => 2,
            };
            st.page = 0;
            st.filter_node = Some(id);
        }
        (Tab::Map, "PlusMinusButton") => return Action::OpenMap,
        (Tab::Crafting, "Reagent_Toggle" | "Reagent_Toggle2") => {
            st.cookbooks = !st.cookbooks;
            st.page = 0;
        }
        (Tab::Crafting, "Exit") => return Action::Close,
        (Tab::Help, _) if name.starts_with("Tab_") => {
            if let Some(i) = HELP.iter().position(|(n, _, _)| *n == name) {
                (st.sub, st.page) = (i, 0);
            }
        }
        (Tab::Settings, "OkButton") => return Action::Close,
        (Tab::Settings, "HelpButton") => return Action::Tab(Tab::Help),
        (Tab::Settings, "OptionButtonLeft" | "OptionButtonRight") => {
            let row_window = l.nodes[id].parent.and_then(|p| l.nodes[p].parent);
            let row = row_window.and_then(|w| SETTINGS_ROWS.iter().position(|r| *r == l.nodes[w].name));
            if let Some(row) = row {
                st.sub = row;
                return Action::Setting(row, if name == "OptionButtonLeft" { -1 } else { 1 });
            }
        }
        _ => {
            let _ = parent;
        }
    }
    Action::None
}

// ---------------------------------------------------------------------------------------------------------------
// Character: CharStatsSpellbookPage.gui. Left page: the character; right page: the Stats sub-page (attributes,
// resistances) or the Badges sub-page (equipment), picked with the page's own tab buttons.

const CHARACTER_SUBPAGES: [&str; 9] = [
    "StatsPageBG", "BadgeFilterPageBG", "BadgePageBG", "AdvancedStatsPageBG", "PvPPageBG", "TimersPageBG",
    "ActivitiesPageBG", "NewPvPBG", "SecondarySchoolsPageBG",
];

fn character(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = 2;
    let sub = st.sub % 2;
    hide(l, v, &CHARACTER_SUBPAGES);
    v.show(l.find(if sub == 0 { "StatsPageBG" } else { "BadgePageBG" }), true);
    hide(l, v, &["BonusInfoIcon", "UnlockXPButton", "LockXPButton", "RefillButton"]);
    grey(l, v, &["AdvancedStatsTabButton", "ActivitiesTabButton", "PvPTabButton", "TimersTabButton", "SecondarySchoolTabButton"]);
    let chosen = l.find(if sub == 0 { "StatsTabButton" } else { "BadgeTabButton" });
    marker(l, v, "TabBackground", "StatsTabButton", chosen);
    if let Some(c) = chosen {
        v.checked.insert(c);
    }
    let Some(sheet) = d.sheet else {
        v.set(l.find("CharacterName"), centre("No character loaded"));
        return;
    };
    let name = if sheet.name.is_empty() { "Tarnished".to_string() } else { sheet.name.clone() };
    v.set(l.find("CharacterName"), centre(&name));
    v.set(l.find("CharLevel"), format!("Level {}", sheet.level));
    v.set(l.find("CharTitle"), "Tarnished");
    if let Some(s) = d.stats {
        v.set(l.find("Health"), centre(&format!("{}/{}", s.hp, s.max_hp)));
        v.set(l.find("Mana"), centre(&format!("{}/{}", s.fp, s.max_fp)));
        v.set(l.path("wndEnergyFrame/txtEnergy"), centre(&format!("{}/{}", s.sp, s.max_sp)));
    }
    v.set(l.find("ExperienceLabel"), centre("Next Level"));
    let cost = if sheet.level < 713 { game::level_up_cost(sheet.level).to_string() } else { "-".into() };
    v.set(l.find("ExpText"), centre(&format!("{}/{}", sheet.runes, cost)));
    v.set(l.find_text("Training"), centre("Memory\nSlots"));
    v.set(l.find("TrainingPoints"), centre(&sheet.magic_slots.to_string()));
    v.set(l.find_text("<center>Gold</center>"), centre("Runes"));
    v.set(l.find("Gold"), centre(&sheet.runes.to_string()));
    v.show(l.find("GoldMax"), false);
    let spells = sheet.items.iter().filter(|(id, _)| is_spell(*id)).count();
    v.set(l.find_text("<center>Crowns</center>"), centre("Spells"));
    v.set(l.find("Crowns"), centre(&spells.to_string()));
    v.set(l.find_text("Blue Arena"), centre("Key\nItems"));
    v.set(l.find("PvPCurrency"), centre(&sheet.key_items.len().to_string()));
    v.set(l.path("wndEnergyFrame/lblTimer"), centre("Flasks"));
    v.set(l.path("wndEnergyFrame/txtEnergyTimer"), centre(&format!("{}/{}", sheet.flasks[0], sheet.flasks[1])));

    if sub == 0 {
        let small = |colour| TextStyle::new("WizardNormal", 0.45, colour).centred();
        // Damage row -> attributes; resistance row -> resistances and poise; accuracy row -> equip load.
        v.set(l.find_text("Damage (%/+)"), centre("Attributes"));
        v.set(l.find_text("Resistance (%/+)"), centre("Resistance"));
        v.set(l.find_text("Accuracy (%)"), centre("Max Equip Load"));
        hide(l, v, &["DamageIcons", "ResistIcons", "AccuracyIcons"]);
        let rows = [
            ("DamageStats", "DamageStatsFlat",
             ["VIG", "MND", "END", "STR", "DEX", "INT", "FTH", "ARC"].map(String::from),
             sheet.attributes.map(|a| a.to_string())),
            ("DmgReduceStats", "DmgReduceStatsFlat",
             ["", "IMM", "ROB", "FOC", "VIT", "POISE", "", ""].map(String::from),
             ["".into(), sheet.resistances[0].to_string(), sheet.resistances[1].to_string(),
              sheet.resistances[2].to_string(), sheet.resistances[3].to_string(), format!("{:.0}", sheet.poise),
              "".into(), "".into()]),
        ];
        for (labels_window, values_window, labels, values) in rows {
            let (Some(lw), Some(vw)) = (l.find(labels_window), l.find(values_window)) else { continue };
            let vr = l.nodes[vw].rect;
            for (i, col) in l.columns(lw).into_iter().enumerate().take(8) {
                v.set(Some(col), centre(&labels[i]));
                let cr = l.nodes[col].rect;
                v.add([cr[0] - 4.0, vr[1], cr[2] + 4.0, vr[1] + 24.0], values[i].clone(), small(BLACK));
            }
        }
        if let Some(acc) = l.find("AccuracyStats") {
            for c in l.children(acc) {
                v.show(Some(c), false);
            }
            let r = l.nodes[acc].rect;
            v.add(r, format!("{:.1}", sheet.max_equip_load), TextStyle::new("WizardNormal", 0.5, BLACK).centred());
        }
    } else {
        v.set(l.find("BadgeFilterTitle"), centre("Equipment"));
        hide(l, v, &["BadgeFilterBack"]);
        let lines = equipment_lines(sheet, d.names);
        let rows: Vec<usize> = (0..7).filter_map(|i| l.find(&format!("Badge_{i}"))).collect();
        let page = paging(l, v, st, lines.len(), rows.len(), ("PageNumber", "PageBack", "PageForward"));
        for (i, row) in rows.iter().enumerate() {
            match lines.get(page * rows.len() + i) {
                Some((slot, item)) => {
                    let r = l.nodes[*row].rect;
                    v.set(Some(*row), String::new());
                    v.add([r[0] + 14.0, r[1] + 4.0, r[2] - 14.0, r[1] + 24.0], slot.clone(),
                          TextStyle::new("WizardFancy", 0.36, LIGHT_YELLOW).centred());
                    v.add([r[0] + 14.0, r[1] + 22.0, r[2] - 14.0, r[3] - 6.0], item.clone(),
                          TextStyle::new("WizardFancy", 0.45, YELLOW).centred());
                }
                None => v.show(Some(*row), false),
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Backpack: InventorySpellbookPage.gui. Category buttons along the top, eight item rows on the right, the
// character portrait (here: what is equipped) on the left.

#[derive(Clone, Copy)]
enum Filter {
    All,
    Head,
    Chest,
    ArmsLegs,
    Weapons,
    AshesOfWar,
    Talismans,
    Ammo,
    SpiritAshes,
    KeyItems,
    Spells,
    Consumables,
}

/// (category button, filter, page title)
const BACKPACK: [(&str, Filter, &str); 12] = [
    ("Tab_All", Filter::All, "Backpack"),
    ("Tab_Hat", Filter::Head, "Helms"),
    ("Tab_Robe", Filter::Chest, "Chest Armor"),
    ("Tab_Shoes", Filter::ArmsLegs, "Gauntlets & Greaves"),
    ("Tab_Weapon", Filter::Weapons, "Weapons"),
    ("Tab_Athame", Filter::AshesOfWar, "Ashes of War"),
    ("Tab_Amulet", Filter::Talismans, "Talismans"),
    ("Tab_Ring", Filter::Ammo, "Arrows & Bolts"),
    ("Tab_Pet", Filter::SpiritAshes, "Spirit Ashes"),
    ("Tab_Mount", Filter::KeyItems, "Key Items"),
    ("Tab_Deck", Filter::Spells, "Spells"),
    ("Tab_Elixir", Filter::Consumables, "Consumables"),
];

fn backpack(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = BACKPACK.len();
    let (button, filter, title) = BACKPACK[st.sub % BACKPACK.len()];
    let chosen = l.find(button);
    if let Some(c) = chosen {
        v.checked.insert(c);
    }
    marker(l, v, "TabBackground", "Tab_All", chosen);
    v.set(l.find("title"), centre(title));
    for prefix in ["crowns", "pvp", "rental", "crafted", "lock"] {
        for i in 0..8 {
            v.show(l.find(&format!("{prefix}{i}")), false);
        }
    }
    grey(l, v, &["Tab_HousingStuff", "Tab_MonsterMagic", "Tab_Photography", "Equip_Item", "Lock_Item", "Socket_Item",
                 "btnFeedPet", "RecyclingBinButton", "QuickSell_Item", "EquipmentManager"]);
    let rows: Vec<usize> = (1..=8).filter_map(|i| l.find(&format!("Item_{i}"))).collect();
    let Some(sheet) = d.sheet else {
        for r in &rows {
            v.show(Some(*r), false);
        }
        return;
    };
    let names = d.names;
    let equipped = equipped_ids(sheet);
    let source: Vec<(u32, u32)> = match filter {
        Filter::KeyItems => sheet.key_items.clone(),
        Filter::SpiritAshes => sheet.items.iter().chain(&sheet.key_items).copied().collect(),
        _ => sheet.items.clone(),
    };
    let mut items: Vec<(String, u32, bool)> = source
        .iter()
        .filter(|(id, _)| {
            let raw = id & 0x0FFF_FFFF;
            let category = id >> 28;
            match filter {
                Filter::All | Filter::KeyItems => true,
                Filter::Head => category == 1 && (raw / 100) % 10 == 0,
                Filter::Chest => category == 1 && (raw / 100) % 10 == 1,
                Filter::ArmsLegs => category == 1 && (raw / 100) % 10 >= 2,
                Filter::Weapons => category == 0 && !(50_000_000..54_000_000).contains(&raw),
                Filter::Ammo => category == 0 && (50_000_000..54_000_000).contains(&raw),
                Filter::AshesOfWar => category == 8,
                Filter::Talismans => category == 2,
                Filter::SpiritAshes => is_spirit_ash(&names.item(*id)),
                Filter::Spells => is_spell(*id),
                Filter::Consumables => {
                    category == 4 && !is_spell(*id) && !is_material(*id) && !is_spirit_ash(&names.item(*id))
                }
            }
        })
        .map(|(id, n)| (names.item(*id), *n, equipped.contains(id)))
        .collect();
    items.sort();
    v.set(l.find("inventorySpace"), centre(&items.len().to_string()));
    let page = paging(l, v, st, items.len(), rows.len(), ("PageNumber", "leftscroll", "rightscroll"));
    for (i, row) in rows.iter().enumerate() {
        let fist = l.find_under(*row, "fist");
        match items.get(page * rows.len() + i) {
            Some((name, count, is_equipped)) => {
                let r = l.nodes[*row].rect;
                v.add([r[0] + 22.0, r[1], r[2] - 48.0, r[3]], name.clone(), TextStyle::new("WizardFancy", 0.42, LIGHT_YELLOW));
                if *count > 1 {
                    v.add([r[2] - 50.0, r[1], r[2] - 16.0, r[3]], format!("x{count}"),
                          TextStyle::new("WizardFancy", 0.42, LIGHT_YELLOW).right());
                }
                v.show(fist, *is_equipped);
            }
            None => {
                v.show(Some(*row), false);
            }
        }
    }
    // The portrait shows the character in Wizard101; here, what the character has equipped.
    if let Some(p) = l.find("3DCharacter") {
        let r = l.nodes[p].rect;
        let name = if sheet.name.is_empty() { "Tarnished".to_string() } else { sheet.name.clone() };
        v.add([r[0] + 10.0, r[1] + 8.0, r[2] - 10.0, r[1] + 34.0], name,
              TextStyle::new("WizardFancy", 0.55, YELLOW).centred().outlined());
        let mut y = r[1] + 40.0;
        for (slot, item) in equipment_lines(sheet, names).into_iter().take(14) {
            if item == "-" {
                continue;
            }
            v.add([r[0] + 14.0, y, r[2] - 14.0, y + 18.0], format!("{slot}: {item}"),
                  TextStyle::new("WizardNormal", 0.36, WHITE).outlined());
            y += 21.0;
            if y > r[3] - 24.0 {
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Pets: PetStatsSpellbookPage.gui. Wizard101's pets are closest to Torrent and the spirit ashes.

fn pets(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = 1;
    grey(l, v, &["EquipButton", "SnackButton", "EquipJewelButton", "ToggleTalentDisplay", "PetTomeButton",
                 "PetBadgesButton"]);
    hide(l, v, &["wndRatingBkg", "wndAgingBkg", "eggHatchTimer", "_NOT_USED_SnackToggleWin"]);
    let mut pets: Vec<(String, String, Option<u32>)> = Vec::new();
    if let Some(sheet) = d.sheet {
        if sheet.key_items.iter().any(|(id, _)| d.names.item(*id) == "Spectral Steed Whistle") {
            pets.push(("Torrent".into(), "Spectral Steed".into(), None));
        }
        let mut ashes: Vec<(String, String, Option<u32>)> = sheet
            .items
            .iter()
            .chain(&sheet.key_items)
            .filter_map(|(id, _)| {
                let name = d.names.item(*id);
                is_spirit_ash(&name).then(|| {
                    let level = (id & 0x0FFF_FFFF) % 100;
                    (name, "Spirit Ash".to_string(), Some(level.min(10)))
                })
            })
            .collect();
        ashes.sort();
        ashes.dedup();
        pets.extend(ashes);
    }
    if pets.is_empty() {
        v.show(l.find("wndContent"), false);
        v.show(l.find("wndMessage"), true);
        v.set(l.path("wndMessage/txtMessage"), centre("You do not have Torrent or any spirit ashes yet."));
        grey(l, v, &["PetTomeButton"]);
        return;
    }
    st.pets = pets.len();
    let i = st.pet % pets.len();
    st.pet = i;
    let (name, kind, level) = &pets[i];
    v.set(l.find("txtPetName"), centre(name));
    v.set(l.find("txtStageRace"), centre(kind));
    if pets.len() < 2 {
        grey(l, v, &["btnPrevPet", "btnNextPet"]);
    }
    match level {
        Some(level) => {
            v.set(l.find("lblExperience"), "Upgrade");
            v.set(l.find("txtExperience"), format!("<right>+{level}/10"));
            v.progress.extend(l.find("prgrExperience").map(|p| (p, *level as f32 / 10.0)));
        }
        None => {
            v.set(l.find("lblExperience"), "Steed");
            v.set(l.find("txtExperience"), "<right>Ready");
            v.progress.extend(l.find("prgrExperience").map(|p| (p, 1.0)));
        }
    }
    if let Some(block) = l.find("ctrlStatBlock") {
        let r = l.nodes[block].rect;
        let title = TextStyle::new("WizardFancy", 0.55, BLACK).centred();
        let body = TextStyle::new("WizardNormal", 0.42, BLACK).wrapped();
        v.add([r[0] + 20.0, r[1] + 16.0, r[2] - 20.0, r[1] + 42.0], if level.is_some() { "Spirit Ash" } else { "Torrent" }, title);
        let text = if level.is_some() {
            format!("Summon {name} near a Rebirth Monument. Upgrade it with Grave or Ghost Glovewort at Roderika's \
                     side in the Roundtable Hold.\n\nSpirit ashes owned: {}", pets.iter().filter(|p| p.2.is_some()).count())
        } else {
            "Torrent, the spectral steed. Call him with the Spectral Steed Whistle while outdoors; he can double \
             jump and ride spiritsprings."
                .to_string()
        };
        v.add([r[0] + 24.0, r[1] + 50.0, r[2] - 24.0, r[3] - 16.0], text, body);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Deck: DeckConfiguration.gui. Left: the deck (memorised spells); right: the spell list as cards.

fn deck(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = 3;
    hide(l, v, &["FusionBackground", "TieredSpellMPUnlockedBackground", "TieredSpellMPUnlockedList", "SpellFusionList",
                 "SchoolPipPanel", "CloseTSMPUnlockedPageButton", "SpellTreasureList", "SchoolPageSpellList",
                 "TutorialLogBackground1", "EquipBorder", "CardsInDeck"]);
    grey(l, v, &["PrevDeck", "NextDeck", "NewDeckName", "EquipButton", "Help", "QuickSellButton", "NextItemSpells",
                 "PrevItemSpells", "TreasureCardButton", "FusionCardButton", "Cards_Shadow", "Cards_MonsterMagic"]);
    let chosen = st.filter_node.or_else(|| l.find(["Cards_All", "Cards_Storm", "Cards_Life"][st.sub % 3]));
    if let Some(c) = chosen {
        v.checked.insert(c);
    }
    marker(l, v, "TabBackground", "Cards_All", chosen);
    v.set(l.find("DeckTitle"), centre(["Spell Deck", "Sorceries", "Incantations"][st.sub % 3]));
    let Some(sheet) = d.sheet else { return };
    v.set(l.find("DeckName"), centre(&format!("Memorized ({} slots)", sheet.magic_slots)));
    if let Some(list) = l.find("CardsInDeck") {
        let r = l.nodes[list].rect;
        let mut y = r[1] + 4.0;
        for (i, spell) in sheet.spells.iter().take(sheet.magic_slots.max(1) as usize).enumerate() {
            let name = if *spell < 0 { "(empty)".to_string() } else { d.names.spell(*spell) };
            v.add([r[0] + 6.0, y, r[0] + 34.0, y + 20.0], format!("{}.", i + 1), TextStyle::new("WizardFancy", 0.45, BLACK));
            v.add([r[0] + 34.0, y, r[2] - 6.0, y + 20.0], name, TextStyle::new("WizardNormal", 0.42, BLACK));
            y += 22.0;
        }
    }
    let mut known: Vec<String> = sheet
        .items
        .iter()
        .filter(|(id, _)| is_spell(*id))
        .filter(|(id, _)| match st.sub % 3 {
            1 => sorcery(*id),
            2 => !sorcery(*id),
            _ => true,
        })
        .map(|(id, _)| d.names.item(*id))
        .collect();
    known.sort();
    v.set(l.find("TreasureCardCount"), centre(&known.len().to_string()));
    let Some(list) = l.find("AllPageSpellList") else { return };
    let node = &l.nodes[list];
    let [cw, ch] = node.card.unwrap_or([97.0, 148.0]);
    let [gx, gy] = node.gap.unwrap_or([27.0, 8.0]);
    let r = node.rect;
    let columns = (((r[2] - r[0]) + gx) / (cw + gx)).floor().max(1.0) as usize;
    let rows = (((r[3] - r[1]) + gy) / (ch + gy)).floor().max(1.0) as usize;
    let per_page = columns * rows;
    let pages = known.len().div_ceil(per_page).max(1);
    st.page = st.page.min(pages - 1);
    if st.page == 0 {
        grey(l, v, &["PageUp"]);
    }
    if st.page + 1 >= pages {
        grey(l, v, &["PageDown"]);
    }
    for (i, name) in known.iter().skip(st.page * per_page).take(per_page).enumerate() {
        let (cx, cy) = ((i % columns) as f32, (i / columns) as f32);
        let x = r[0] + cx * (cw + gx);
        let y = r[1] + cy * (ch + gy);
        v.images.push(([x, y, x + cw, y + ch], "GUI/Cards/School_Treasure_Blnk.png".into(), [0.0, 0.0, 1.0, 1.0]));
        v.add([x + 8.0, y + 12.0, x + cw - 8.0, y + ch * 0.45], name.clone(),
              TextStyle::new("WizardFancy", 0.36, YELLOW).centred().wrapped().outlined());
    }
    if known.is_empty() {
        v.add([r[0], r[1] + 40.0, r[2], r[1] + 80.0], "No spells", TextStyle::new("WizardFancy", 0.6, BLACK).centred());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Quests: QuestListWindow.gui; the cards are QuestInfoWindow.gui (drawn by book.rs into each card window).

fn quests(l: &Layout, st: &mut State, v: &mut View) {
    st.subs = 1;
    grey(l, v, &["btnPrevPage", "btnNextPage", "SelectWorldButton", "SelectWorldIconButton", "TutorialLogButton",
                 "QuestLogButton"]);
    for counter in ["QuestLogAllCounter", "QuestLogMainlineAndSideCounter", "QuestLogActivitiesCounter",
                    "QuestLogReadyToTurnInCounter", "QuestLogHiddenCounter"] {
        v.show(l.find(counter), false);
    }
}

/// The text of quest card `i` (QuestInfoWindow.gui), or None for an empty card.
pub fn quest_card(card: &Layout, i: usize, v: &mut View) -> bool {
    for n in ["wndReward1", "wndReward2", "wndReward3", "wndReward4", "wndReward5", "btnHideQuest", "btnUnhideQuest",
              "imgGoalCounter", "txtGoalCounter"] {
        v.show(card.find(n), false);
    }
    let (name, goal, left, right, zone, world) = match i {
        0 => ("The Lands Between", "Elden Ring keeps no quest log.", "NPC questlines", "Great Runes",
              "Not yet: read from the game's event flags", "Elden Ring"),
        1 => ("Unicorn Way", "Wizard City comes to Limgrave.", "Rattlebones", "The hedge maze",
              "Not yet: quest goals for the Wizard101 areas", "Wizard101"),
        _ => return false,
    };
    v.set(card.find("txtName"), centre(name));
    v.set(card.find("txtGoal"), centre(goal));
    v.set(card.find("txtGoalObjective1"), centre(left));
    v.set(card.find("txtGoalObjective2"), centre(right));
    v.set(card.find("txtZone"), centre(zone));
    v.set(card.find("txtWorld"), centre(world));
    true
}

// ---------------------------------------------------------------------------------------------------------------
// Map: MapSpellbookPage.gui. Wizard101 draws the zone map here; this book opens Elden Ring's own map instead.

fn map(l: &Layout, d: &Data, v: &mut View) {
    v.set(l.find("ZoneName"), centre("The Lands Between"));
    v.show(l.find("MapLoadingIndicator"), d.map_opening);
    v.set(l.path("MapLoadingIndicator/PackageNameText"), centre("Opening Map..."));
    hide(l, v, &["txtGoHomeTimer", "txtGotoDormTimer"]);
    grey(l, v, &["GoHomeButton", "GotoDormButton"]);
    if !d.map_opening {
        let body = TextStyle::new("WizardNormal", 0.5, BLACK).centred().wrapped();
        v.add([120.0, 200.0, 680.0, 230.0], "World Map", TextStyle::new("WizardFancy", 0.8, BLACK).centred());
        v.add([150.0, 250.0, 650.0, 420.0],
              "Elden Ring's own map opens from this page, with markers, grace travel and zoom. \
               Close it as usual and the book comes back here; travel to a grace and the book stays closed.\n\n\
               Press A, Enter or the magnifier below to open the map.",
              body);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Crafting: RecipeWindow.gui. Right: crafting materials (or cookbooks, with the Reagents button); left: the kit.

fn crafting(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = 1;
    v.set(l.find("title"), centre("Crafting"));
    hide(l, v, &["BonusInfoIcon", "CompletedText", "UnequipButton", "TimerSlot_01"]);
    grey(l, v, &["CreateButton", "FiltersButton", "EquipButton", "Tab_FurnitureStuff", "Tab_Hat", "Tab_Robe",
                 "Tab_Shoes", "Tab_Weapon", "Tab_Athame", "Tab_Amulet", "Tab_Ring", "Tab_Pet", "Tab_Deck", "Tab_Mount"]);
    if let Some(all) = l.find("Tab_AllEquipment") {
        v.checked.insert(all);
    }
    for i in 1..=8 {
        let n = format!("Ingredient0{i}");
        if let Some(slot) = l.find(&n) {
            if let Some(num) = l.find_under(slot, "IngredientNumber") {
                v.set(Some(num), String::new());
            }
        }
    }
    v.set(l.find("Reagent_Toggle2"), centre(if st.cookbooks { "Materials" } else { "Cookbooks" }));
    let rows: Vec<usize> = (1..=8).filter_map(|i| l.find(&format!("Item_{i}"))).collect();
    let Some(sheet) = d.sheet else { return };
    let kit = sheet.key_items.iter().any(|(id, _)| d.names.item(*id) == "Crafting Kit");
    v.set(l.find("labelItemHighlighted"), centre(if kit { "Crafting Kit" } else { "No Crafting Kit yet" }));
    let mut list: Vec<(String, u32)> = if st.cookbooks {
        sheet.key_items.iter().map(|(id, n)| (d.names.item(*id), *n)).filter(|(n, _)| n.contains("Cookbook")).collect()
    } else {
        sheet.items.iter().filter(|(id, _)| is_material(*id)).map(|(id, n)| (d.names.item(*id), *n)).collect()
    };
    list.sort();
    let page = paging(l, v, st, list.len(), rows.len(), ("PageNumber", "leftscroll", "rightscroll"));
    for (i, row) in rows.iter().enumerate() {
        match list.get(page * rows.len() + i) {
            Some((name, count)) => {
                let r = l.nodes[*row].rect;
                v.add([r[0] + 22.0, r[1], r[2] - 52.0, r[3]], name.clone(), TextStyle::new("WizardFancy", 0.42, LIGHT_YELLOW));
                if !st.cookbooks {
                    v.add([r[2] - 56.0, r[1], r[2] - 18.0, r[3]], format!("x{count}"),
                          TextStyle::new("WizardFancy", 0.42, LIGHT_YELLOW).right());
                }
            }
            None => v.show(Some(*row), false),
        }
    }
    if list.is_empty() {
        let r = l.nodes[rows.first().copied().unwrap_or(0)].rect;
        v.add([r[0], r[1], r[2], r[1] + 40.0], if st.cookbooks { "No cookbooks yet" } else { "No materials yet" },
              TextStyle::new("WizardFancy", 0.55, BLACK).centred());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Help: HelpSpellbookPage.gui. Its topic buttons pick the help text (Wizard101 loads its help pages from other
// layouts; here the text is drawn on the book's pages in Wizard101's fonts).

/// (topic button, title, text)
const HELP: [(&str, &str, &str); 4] = [
    ("Tab_Questing", "The Spellbook",
     "Open and close the book with B on the keyboard or the View button on the controller (Elden Ring's map button). \
      Esc or the controller's B button also close it.\n\nThe tabs down the right edge are the book's pages, as in \
      Wizard101. While the book is open the game doesn't move."),
    ("Tab_Combat", "Controller",
     "LB / RB or D-pad up and down: previous / next tab.\nD-pad left and right: turn the page of a list.\n\
      Y / X: previous / next category on the page (backpack categories, spell filters, help topics, settings).\n\
      A: use the tab (the World Map tab opens Elden Ring's map).\nB or View: close the book."),
    ("Tab_Interface", "Keyboard and Mouse",
     "Click a tab or a button. Up / Down arrows: previous / next tab. Left / Right arrows or the mouse wheel: turn \
      the page. Enter: use the tab. B or Esc: close the book."),
    ("Tab_Character", "The Pages",
     "Character: level, health, mana (FP), energy (stamina), attributes and equipment.\nBackpack: everything you \
      carry, by category.\nPets: Torrent and your spirit ashes.\nDeck: memorized and known spells.\nQuests: not \
      yet.\nWorld Map: Elden Ring's map.\nCrafting: materials and cookbooks.\nSettings: the HUD's settings."),
];

fn help(l: &Layout, st: &mut State, v: &mut View) {
    st.subs = HELP.len();
    let (button, title, text) = HELP[st.sub % HELP.len()];
    let chosen = l.find(button);
    if let Some(c) = chosen {
        v.checked.insert(c);
    }
    marker(l, v, "TabBackground", "Tab_Questing", chosen);
    hide(l, v, &["TabGroup2"]);
    grey(l, v, &["Tab_Housing", "Tab_Crafting", "Tab_CrownShop", "Tab_Pets", "Tab_Gardening", "NextHelpPage",
                 "leftscroll", "rightscroll"]);
    v.set(l.find("pageText"), centre("1/1"));
    v.add([90.0, 112.0, 690.0, 146.0], title, TextStyle::new("WizardFancy", 0.8, BLACK).centred());
    v.add([110.0, 160.0, 670.0, 520.0], text, TextStyle::new("WizardNormal", 0.5, BLACK).wrapped());
}

// ---------------------------------------------------------------------------------------------------------------
// Settings: Settings.gui's Graphics page, its five option rows used for the HUD's settings (w101_orbs.ini).

pub const SETTINGS_ROWS: [&str; 5] =
    ["OptionResolution", "OptionFullscreen", "OptionUISize", "OptionTextureDetail", "OptionHardwareCursor"];

fn settings(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = SETTINGS_ROWS.len();
    for t in ["AudioOptionsText", "RealmsText", "GameplayOptionsText", "PrivacyOptionsText", "AdvVideoOptionsText"] {
        v.show(l.find(t), false);
    }
    v.set(l.find("VideoOptionsText"), centre("Settings"));
    v.show(l.find("VideoOptions"), true);
    grey(l, v, &["AdvVideoButton", "AudioButton", "GameplayButton", "AdvGameplayButton", "PrivacyButton",
                 "RealmsButton", "CreditsButton", "MoreGameplayButton", "AnotherGameplayButton", "QuitButton",
                 "IdleButton"]);
    if let Some(b) = l.find("VideoButton") {
        v.checked.insert(b);
    }
    for (i, row) in SETTINGS_ROWS.iter().enumerate() {
        let Some(r) = l.find(row) else { continue };
        let (label, value) = d.settings.get(i).cloned().unwrap_or_default();
        let text = if i == st.sub % SETTINGS_ROWS.len() { format!("> {label}") } else { label };
        v.set(l.find_under(r, "OptionText"), text);
        v.set(l.find_under(r, "OptionValue"), centre(&value));
    }
}
