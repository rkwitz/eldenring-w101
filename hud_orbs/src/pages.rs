//! What each spellbook page shows: Wizard101's own page layout (layout.rs) filled with the closest Elden Ring data.
//!
//! Every page keeps Wizard101's art, positions, fonts and colours; these functions only pick which of the layout's
//! sub-pages are shown, put Elden Ring text where Wizard101 puts its own, and grey out buttons that have nothing to
//! do here (Wizard101 greys buttons the same way when they don't apply).

use hudhook::imgui::ImColor32;

use crate::game::{self, Sheet};
use crate::itemdata::ItemData;
use crate::layout::{Layout, TextStyle, View};
use crate::names::Names;
use crate::quests::Active;

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
/// The frame behind a memorised spell's card, and the shade over a memory slot not unlocked yet.
const MEMORIZED: ImColor32 = ImColor32::from_rgba(255, 214, 64, 230);
const LOCKED: ImColor32 = ImColor32::from_rgba(20, 12, 6, 170);

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
    /// Crafting: show cookbooks instead of materials, and which cookbook is open on the left page.
    pub cookbooks: bool,
    pub cookbook: usize,
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
    pub items: &'a ItemData,
    /// The quests in progress (from the quest event flags).
    pub quests: &'a [Active],
    /// (label, value) per settings row.
    pub settings: Vec<(String, String)>,
    pub map_opening: bool,
    /// Elden Ring's map is open behind the book (the page is a window onto it).
    pub map_open: bool,
    /// With the map open: a copy of the game's screen (the map) to draw on the page, and the screen's width /
    /// height. None: the page is cut out to show the map behind it.
    pub map_picture: Option<f32>,
    /// Per ACHIEVEMENTS entry: earned (its event flag), or None when not tracked / not readable.
    pub achieved: &'a [Option<bool>],
}

/// Elden Ring's achievements, in the game's own order (AwardAchievement IDs 0-41), with the event flag that awards
/// it (common.emevd event 9300 is run per achievement with its flag: the bosses), 0 where the game awards it some
/// other way (endings, collections) that the book doesn't follow.
pub const ACHIEVEMENTS: [(&str, &str, u32); 42] = [
    ("Elden Ring", "Obtained all achievements", 0),
    ("Elden Lord", "Achieved the \"Elden Lord\" ending", 0),
    ("Age of the Stars", "Achieved the \"Age of the Stars\" ending", 0),
    ("Lord of Frenzied Flame", "Achieved the \"Lord of Frenzied Flame\" ending", 0),
    ("Shardbearer Godrick", "Defeated Shardbearer Godrick", 10000800),
    ("Shardbearer Radahn", "Defeated Shardbearer Radahn", 9130),
    ("Shardbearer Morgott", "Defeated Shardbearer Morgott", 11000800),
    ("Shardbearer Rykard", "Defeated Shardbearer Rykard", 16000800),
    ("Shardbearer Malenia", "Defeated Shardbearer Malenia", 15000800),
    ("Shardbearer Mohg", "Defeated Shardbearer Mohg", 12050800),
    ("Maliketh the Black Blade", "Defeated Maliketh the Black Blade", 13000800),
    ("Hoarah Loux the Warrior", "Defeated Hoarah Loux the Warrior", 11050800),
    ("Dragonlord Placidusax", "Defeated Dragonlord Placidusax", 13000830),
    ("Great Rune", "Restored the power of a Great Rune", 0),
    ("Erdtree Aflame", "Used the Rune of Death to set the Erdtree aflame", 0),
    ("Legendary Armaments", "Acquired all legendary armaments", 0),
    ("Legendary Ashen Remains", "Acquired all legendary ashen remains", 0),
    ("Legendary Sorceries and Incantations", "Acquired all legendary sorceries and incantations", 0),
    ("Rennala, Queen of the Full Moon", "Defeated Rennala, Queen of the Full Moon", 14000800),
    ("Lichdragon Fortissax", "Defeated Lichdragon Fortissax", 12030850),
    ("Godskin Duo", "Defeated the Godskin Duo", 13000850),
    ("Fire Giant", "Defeated the Fire Giant", 1052520800),
    ("Dragonkin Soldier of Nokstella", "Defeated the Dragonkin Soldier of Nokstella", 12010800),
    ("Regal Ancestor Spirit", "Defeated the Regal Ancestor Spirit", 12090800),
    ("Valiant Gargoyle", "Defeated the Valiant Gargoyle", 12020800),
    ("Margit, the Fell Omen", "Defeated Margit, the Fell Omen", 10000850),
    ("Red Wolf of Radagon", "Defeated the Red Wolf of Radagon", 14000850),
    ("Godskin Noble", "Defeated the Godskin Noble", 16000850),
    ("Magma Wyrm Makar", "Defeated Magma Wyrm Makar", 39200800),
    ("Godfrey the First Lord", "Defeated Godfrey the First Lord", 11000850),
    ("Mohg the Omen", "Defeated Mohg the Omen", 35000800),
    ("Mimic Tear", "Defeated the Mimic Tear", 12020850),
    ("Loretta, Knight of the Haligtree", "Defeated Loretta, Knight of the Haligtree", 15000850),
    ("Astel, Naturalborn of the Void", "Defeated Astel, Naturalborn of the Void", 12040800),
    ("Leonine Misbegotten", "Defeated the Leonine Misbegotten", 1043300800),
    ("Royal Knight Loretta", "Defeated Royal Knight Loretta", 1035500800),
    ("Elemer of the Briar", "Defeated Elemer of the Briar", 1039540800),
    ("Ancestor Spirit", "Defeated the Ancestor Spirit", 12080800),
    ("Commander Niall", "Defeated Commander Niall", 1051570800),
    ("Roundtable Hold", "Arrived at the Roundtable Hold", 105),
    ("Legendary Talismans", "Acquired all legendary talismans", 0),
    ("Spirit Ashes", "Acquired spirit ashes", 110),
];

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

    pub fn step_pet(&mut self, delta: i32) {
        let n = self.pets.max(1) as i32;
        self.pet = (self.pet as i32 + delta).rem_euclid(n) as usize;
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

/// The union of the named nodes' rectangles (a row of tab buttons), for the LB / RB hints.
fn row_of(l: &Layout, names: &[&str]) -> Option<[f32; 4]> {
    names.iter().filter_map(|n| l.find(n)).map(|i| l.nodes[i].rect).reduce(|a, b| {
        [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
    })
}

/// An item's Elden Ring icon in `r`, if there is one.
fn icon(v: &mut View, items: &ItemData, item_id: u32, r: [f32; 4]) -> bool {
    match items.icon(item_id) {
        Some(path) => {
            v.image(r, path);
            true
        }
        None => false,
    }
}

/// The flasks: (crimson charges, crimson max, cerulean charges, cerulean max). Flask goods 1000-1025 are the
/// Flask of Crimson Tears +0..+12 and 1050-1075 Cerulean, in pairs: the even ID is the empty flask (stack of 1,
/// EquipParamGoods icon 15), the odd ID the filled one (stack of up to 20, icon 16) whose quantity is the charges.
fn flasks(sheet: &Sheet) -> (u32, u32, u32, u32) {
    let mut charges = [0u32; 2];
    for (id, n) in sheet.items.iter().chain(&sheet.key_items) {
        if id >> 28 != 4 {
            continue;
        }
        let raw = id & 0x0FFF_FFFF;
        if (1000..1026).contains(&raw) && raw % 2 == 1 {
            charges[0] += n;
        } else if (1050..1076).contains(&raw) && raw % 2 == 1 {
            charges[1] += n;
        }
    }
    (charges[0], sheet.flasks[0] as u32, charges[1], sheet.flasks[1] as u32)
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

/// Fills the page for `tab`.
pub fn bind(tab: Tab, l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    match tab {
        Tab::Character => character(l, d, st, v),
        Tab::Backpack => backpack(l, d, st, v),
        Tab::Pets => pets(l, d, st, v),
        Tab::Deck => deck(l, d, st, v),
        Tab::Quests => quests(l, d, st, v),
        Tab::Map => map(l, d, v),
        Tab::Crafting => crafting(l, d, st, v),
        Tab::Help => help(l, st, v),
        Tab::Settings => settings(l, d, st, v),
        Tab::Close => {}
    }
}

/// The character pictures (book.rs draws them with model.rs): head to feet, and the portrait circle's face.
pub const MODEL_FULL: &str = "@model/0";
pub const MODEL_PORTRAIT: &str = "@model/1";

fn sheet_name(d: &Data) -> String {
    d.sheet.map(|s| s.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| "Tarnished".into())
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
            if let Some(i) = DECK_TABS.iter().position(|(n, _, _)| *n == name) {
                (st.sub, st.page) = (i, 0);
            }
        }
        (Tab::Map, "PlusMinusButton") => return Action::OpenMap,
        (Tab::Crafting, "Reagent_Toggle" | "Reagent_Toggle2") => {
            st.cookbooks = !st.cookbooks;
            st.page = 0;
        }
        (Tab::Crafting, _) if st.cookbooks && name.starts_with("Item_") => {
            if let Ok(n) = name["Item_".len()..].parse::<usize>() {
                st.cookbook = st.page * 8 + n.saturating_sub(1);
            }
        }
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

/// What the character page works out from the equipment (PlayerGameData's own poise / equip load fields are not
/// what the game shows).
struct Derived {
    /// Physical, strike, slash, pierce, magic, fire, lightning, holy damage negation from armour, percent.
    negation: [f32; 8],
    poise: f32,
    load: f32,
    max_load: f32,
}

/// Elden Ring's max equip load for an Endurance level (before talismans).
fn base_equip_load(endurance: u32) -> f32 {
    let e = endurance as f32;
    match endurance {
        0..=8 => 45.0,
        9..=25 => 45.0 + 27.0 * ((e - 8.0) / 17.0).powf(1.1),
        26..=60 => 72.0 + 48.0 * (1.0 - (1.0 - (e - 25.0) / 35.0).powf(1.2)),
        _ => 120.0 + 40.0 * ((e - 60.0) / 39.0).min(1.0),
    }
}

fn stats3(sheet: &Sheet, items: &ItemData) -> Derived {
    let e = &sheet.equipment;
    let mut kept = [1.0f32; 8];
    let (mut poise, mut load) = (0.0, 0.0);
    for slot in 12..16 {
        if let Some(a) = u32::try_from(e[slot]).ok().and_then(|id| items.armour(id)) {
            for i in 0..8 {
                kept[i] *= 1.0 - a.absorption[i] / 100.0;
            }
            poise += a.poise;
            load += a.weight;
        }
    }
    for slot in 0..6 {
        if e[slot] > 0 && e[slot] != 110000 {
            load += items.weapon_weight(e[slot] as u32);
        }
    }
    let talismans: Vec<u32> = (17..21).filter(|&s| e[s] > 0).map(|s| e[s] as u32).collect();
    for t in &talismans {
        load += items.talisman_weight(*t);
    }
    let has = |id: u32| talismans.contains(&id);
    let mut max_load = base_equip_load(sheet.attributes[2]);
    // Arsenal Charm (+1), Great-Jar's Arsenal, Erdtree's Favor (+1, +2).
    for (id, k) in [(1030, 1.15), (1031, 1.17), (1032, 1.19), (1040, 1.05), (1041, 1.065), (1042, 1.08)] {
        if has(id) {
            max_load *= k;
        }
    }
    if has(1210) {
        poise *= 1.33; // Bull-Goat's Talisman
    }
    Derived { negation: kept.map(|k| (1.0 - k) * 100.0), poise, load, max_load }
}

// ---------------------------------------------------------------------------------------------------------------
// Character: CharStatsSpellbookPage.gui. Left page: the character; right page: the Stats sub-page (attributes,
// resistances) or the Badges sub-page (equipment), picked with the page's own tab buttons.

/// The energy box's left and right (page pixels), and the two flask lines in the box under "Flasks".
const ENERGY_BOX: [f32; 2] = [96.0, 264.0];
const FLASK_ROWS: [[f32; 4]; 2] = [[280.0, 526.0, 364.0, 549.0], [280.0, 550.0, 364.0, 573.0]];

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
        // Centred in the box under "Energy" (Wizard101 leaves room on the right for its refill button).
        if let Some(e) = l.path("wndEnergyFrame/txtEnergy") {
            let r = l.nodes[e].rect;
            v.rect.insert(e, [ENERGY_BOX[0], r[1], ENERGY_BOX[1], r[3]]);
        }
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
    let (hp, hp_max, fp, fp_max) = flasks(sheet);
    v.set(l.path("wndEnergyFrame/lblTimer"), centre("Flasks"));
    v.set(l.path("wndEnergyFrame/txtEnergyTimer"), String::new());
    // Crimson (health) above Cerulean (mana), each with its flask's icon.
    let filled = |range: std::ops::Range<u32>, fallback: u32| {
        sheet.items.iter().map(|(id, _)| *id).find(|id| id >> 28 == 4 && range.contains(&(id & 0x0FFF_FFFF)) && id % 2 == 1)
            .unwrap_or(0x4000_0000 | fallback)
    };
    let flask_rows = [
        (FLASK_ROWS[0], filled(1000..1026, 1001), format!("{hp}/{hp_max}"), ImColor32::from_rgb(150, 18, 18)),
        (FLASK_ROWS[1], filled(1050..1076, 1051), format!("{fp}/{fp_max}"), ImColor32::from_rgb(24, 52, 160)),
    ];
    for (r, id, text, colour) in flask_rows {
        let size = r[3] - r[1];
        let has_icon = icon(v, d.items, id, [r[0], r[1], r[0] + size, r[3]]);
        let left = if has_icon { r[0] + size + 2.0 } else { r[0] };
        v.add([left, r[1], r[2], r[3]], text, TextStyle::new("WizardNormal", 0.42, colour).centred());
    }
    v.tip([FLASK_ROWS[0][0], FLASK_ROWS[0][1], FLASK_ROWS[1][2], FLASK_ROWS[1][3]],
          vec!["Flasks".into(), format!("Crimson Tears: {hp} of {hp_max}"), format!("Cerulean Tears: {fp} of {fp_max}")]);
    // The portrait: the character's head (hat on), drawn from Elden Ring's models, on the starry background.
    if let Some(p) = l.find("wndCharacterPortrait") {
        v.node_images.insert(p, MODEL_PORTRAIT.into());
    }
    if let Some(tabs) = row_of(l, &["StatsTabButton", "BadgeTabButton", "AdvancedStatsTabButton", "ActivitiesTabButton",
                                    "PvPTabButton", "TimersTabButton", "SecondarySchoolTabButton"]) {
        v.tab_row = Some(tabs);
    }

    if sub == 0 {
        // Three sections, eight columns each under Wizard101's school icons (Fire Ice Storm Myth Life Death Balance
        // Shadow): each Elden Ring stat sits under the school it is closest to.
        let st3 = stats3(sheet, d.items);
        v.set(l.find_text("Damage (%/+)"), centre("Attributes"));
        v.set(l.find_text("Resistance (%/+)"), centre("Damage Negation (%)"));
        v.set(l.find_text("Accuracy (%)"), centre("Resistances & Load"));
        let attr = |i: usize| sheet.attributes[i].to_string();
        let neg = |i: usize| format!("{:.1}", st3.negation[i]);
        let rows: [(&str, &str, [&str; 8], [String; 8], [&str; 8]); 2] = [
            ("DamageStats", "DamageStatsFlat",
             ["STR", "VIG", "DEX", "INT", "FTH", "ARC", "END", "MND"],
             [attr(3), attr(0), attr(4), attr(5), attr(6), attr(7), attr(2), attr(1)],
             ["Strength", "Vigor", "Dexterity", "Intelligence", "Faith", "Arcane", "Endurance", "Mind"]),
            ("DmgReduceStats", "DmgReduceStatsFlat",
             ["FIRE", "MAG", "LTNG", "STRK", "HOLY", "PIER", "PHYS", "SLSH"],
             [neg(5), neg(4), neg(6), neg(1), neg(7), neg(3), neg(0), neg(2)],
             ["Fire", "Magic", "Lightning", "Strike", "Holy", "Pierce", "Physical", "Slash"]),
        ];
        for (labels_window, values_window, labels, values, about) in rows {
            let (Some(lw), Some(vw)) = (l.find(labels_window), l.find(values_window)) else { continue };
            for (i, col) in l.columns(lw).into_iter().enumerate().take(8) {
                v.set(Some(col), centre(labels[i]));
                v.style.insert(col, TextStyle::new("WizardNormal", 0.36, BLACK));
            }
            for (i, col) in l.columns(vw).into_iter().enumerate().take(8) {
                v.set(Some(col), centre(&values[i]));
                v.style.insert(col, TextStyle::new("WizardNormal", 0.45, BLACK));
                let r = l.nodes[col].rect;
                let top = l.nodes[lw].rect[1] - 26.0;
                v.tip([r[0], top, r[2], r[3]], vec![about[i].to_string(), values[i].clone()]);
            }
        }
        if let Some(acc) = l.find("AccuracyStats") {
            let labels = ["ROB", "FOC", "POISE", "DISC", "IMM", "VIT", "LOAD", "MAX"];
            let about = ["Robustness (hemorrhage, frostbite)", "Focus (sleep, madness)", "Poise",
                         "Discovery (item finding)", "Immunity (poison, scarlet rot)", "Vitality (death blight)",
                         "Equip load", "Max equip load"];
            let values = [sheet.resistances[1].to_string(), sheet.resistances[2].to_string(),
                          format!("{:.0}", st3.poise), format!("{:.0}", 100 + sheet.attributes[7]),
                          sheet.resistances[0].to_string(), sheet.resistances[3].to_string(),
                          format!("{:.1}", st3.load), format!("{:.1}", st3.max_load)];
            for (i, col) in l.columns(acc).into_iter().enumerate().take(8) {
                let r = l.nodes[col].rect;
                v.set(Some(col), centre(&values[i]));
                v.style.insert(col, TextStyle::new("WizardNormal", 0.42, BLACK));
                // The label under the box (this section has one row).
                v.add([r[0], r[3] - 8.0, r[2], r[3] + 8.0], labels[i].to_string(),
                      TextStyle::new("WizardNormal", 0.3, BLACK).centred());
                v.tip([r[0], r[1] - 26.0, r[2], r[3] + 8.0], vec![about[i].to_string(), values[i].clone()]);
            }
            let ratio = if st3.max_load > 0.0 { st3.load / st3.max_load } else { 0.0 };
            let roll = match ratio {
                x if x <= 0.3 => "Light load",
                x if x <= 0.7 => "Medium load",
                x if x <= 1.0 => "Heavy load",
                _ => "Overloaded",
            };
            v.tip(l.nodes[acc].rect, vec!["Equip load".into(), format!("{:.1} / {:.1}", st3.load, st3.max_load),
                                          roll.into()]);
        }
    } else {
        // Wizard101's badges are what you have done: Elden Ring's achievements, earned ones first.
        let earned = d.achieved.iter().filter(|a| **a == Some(true)).count();
        v.set(l.find("BadgeFilterTitle"), centre(&format!("Achievements {earned}/{}", ACHIEVEMENTS.len())));
        hide(l, v, &["BadgeFilterBack"]);
        let mut order: Vec<usize> = (0..ACHIEVEMENTS.len()).collect();
        order.sort_by_key(|i| {
            let state = d.achieved.get(*i).copied().flatten();
            (state != Some(true), state.is_none(), *i)
        });
        let rows: Vec<usize> = (0..7).filter_map(|i| l.find(&format!("Badge_{i}"))).collect();
        let page = paging(l, v, st, order.len(), rows.len(), ("PageNumber", "PageBack", "PageForward"));
        for (i, row) in rows.iter().enumerate() {
            let Some(&a) = order.get(page * rows.len() + i) else {
                v.show(Some(*row), false);
                continue;
            };
            let (name, about, _) = ACHIEVEMENTS[a];
            let state = d.achieved.get(a).copied().flatten();
            let r = l.nodes[*row].rect;
            v.set(Some(*row), String::new());
            let status = match state {
                Some(true) => "Earned",
                Some(false) => "Not yet earned",
                None => "Not tracked",
            };
            if state != Some(true) {
                v.greyed.insert(*row);
            }
            let fade = |c: ImColor32| if state == Some(true) { c } else { ImColor32::from_rgba(c.r, c.g, c.b, 140) };
            // Inside the row's frame (its border is about 8 px).
            v.add([r[0] + 16.0, r[1] + 7.0, r[2] - 16.0, r[1] + 22.0], status.to_string(),
                  TextStyle::new("WizardFancy", 0.32, fade(LIGHT_YELLOW)).centred());
            v.add([r[0] + 16.0, r[1] + 22.0, r[2] - 16.0, r[3] - 12.0], name.to_string(),
                  TextStyle::new("WizardFancy", 0.4, fade(YELLOW)).centred());
            v.tip(r, vec![name.to_string(), about.to_string(), status.to_string()]);
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

/// Where the Backpack's picture looks for category `sub` (centre height, view height, metres): helms at the
/// head, chest armour at the body, gauntlets and greaves at the legs, everything else the whole wizard.
pub fn backpack_zoom(sub: usize) -> [f32; 2] {
    match BACKPACK[sub % BACKPACK.len()].1 {
        Filter::Head => [1.58, 0.62],
        Filter::Chest => [1.22, 1.0],
        Filter::ArmsLegs => [0.42, 0.95],
        _ => crate::model::Framing::Full.view(true),
    }
}

fn backpack(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = BACKPACK.len();
    let (button, filter, title) = BACKPACK[st.sub % BACKPACK.len()];
    let chosen = l.find(button);
    if let Some(c) = chosen {
        v.checked.insert(c);
    }
    marker(l, v, "TabBackground", "Tab_All", chosen);
    v.tab_row = row_of(l, &BACKPACK.map(|b| b.0));
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
    let mut items: Vec<(String, u32, bool, u32)> = source
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
        .map(|(id, n)| (names.item(*id), *n, equipped.contains(id), *id))
        .collect();
    items.sort();
    v.set(l.find("inventorySpace"), centre(&items.len().to_string()));
    let page = paging(l, v, st, items.len(), rows.len(), ("PageNumber", "leftscroll", "rightscroll"));
    for (i, row) in rows.iter().enumerate() {
        let fist = l.find_under(*row, "fist");
        match items.get(page * rows.len() + i) {
            Some((name, count, is_equipped, id)) => {
                let r = l.nodes[*row].rect;
                let h = r[3] - r[1];
                let left = if icon(v, d.items, *id, [r[0] + 14.0, r[1] + 3.0, r[0] + 14.0 + h - 6.0, r[3] - 3.0]) {
                    r[0] + 18.0 + h - 6.0
                } else {
                    r[0] + 22.0
                };
                let right = if *count > 1 { r[2] - 52.0 } else { r[2] - 14.0 };
                v.add([left, r[1], right, r[3]], name.clone(), TextStyle::new("WizardFancy", 0.42, LIGHT_YELLOW).centred());
                if *count > 1 {
                    v.add([r[2] - 50.0, r[1], r[2] - 16.0, r[3]], format!("x{count}"),
                          TextStyle::new("WizardFancy", 0.42, LIGHT_YELLOW).right());
                }
                v.show(fist, *is_equipped);
                v.tip(r, d.items.tooltip(*id, *count, names, *is_equipped));
            }
            None => {
                v.show(Some(*row), false);
            }
        }
    }
    // The portrait window shows the character, drawn from Elden Ring's models (model.rs, book.rs).
    if let Some(p) = l.find("3DCharacter") {
        v.node_images.insert(p, MODEL_FULL.into());
        v.tip(l.nodes[p].rect, vec![sheet_name(d), if crate::input::pad_connected() {
            "Right stick: turn (it turns by itself)".into() } else { "Drag: turn; click: stop or start".into() }]);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Pets: PetStatsSpellbookPage.gui. Wizard101's pets are closest to Torrent and the spirit ashes.

/// Wizard101's thin gold message-box frame (Art_Message_Box02), for boxed list entries, and the parchment inside.
fn box_frame() -> [Option<String>; 9] {
    ["TL", "T", "TR", "L", "R", "BL", "B", "BR"].map(|p| Some(format!("GUI/Art/Art_Message_Box02_{p}.png")))
        .into_iter().chain([None]).collect::<Vec<_>>().try_into().unwrap_or_default()
}
const TALENT_BOX: ImColor32 = ImColor32::from_rgb(206, 182, 136);

fn pets(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = 1;
    grey(l, v, &["EquipButton", "SnackButton", "EquipJewelButton", "ToggleTalentDisplay", "PetTomeButton",
                 "PetBadgesButton"]);
    hide(l, v, &["wndRatingBkg", "wndAgingBkg", "eggHatchTimer", "_NOT_USED_SnackToggleWin", "ctrlGenetics", "cardBkg",
                 "spellCard", "spellCardSmall1", "spellCardSmall2", "spellCardSmall3", "spriteSchoolIcon"]);
    // (name, kind, upgrade level, item)
    let mut pets: Vec<(String, String, Option<u32>, u32)> = Vec::new();
    if let Some(sheet) = d.sheet {
        if let Some((id, _)) = sheet.key_items.iter().find(|(id, _)| d.names.item(*id) == "Spectral Steed Whistle") {
            pets.push(("Torrent".into(), "Spectral Steed".into(), None, *id));
        }
        let mut ashes: Vec<(String, String, Option<u32>, u32)> = sheet
            .items
            .iter()
            .chain(&sheet.key_items)
            .filter_map(|(id, _)| {
                let name = d.names.item(*id);
                is_spirit_ash(&name).then(|| {
                    let level = (id & 0x0FFF_FFFF) % 100;
                    (name, "Spirit Ash".to_string(), Some(level.min(10)), *id)
                })
            })
            .collect();
        ashes.sort();
        ashes.dedup_by(|a, b| a.0 == b.0);
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
    let (name, kind, level, id) = &pets[i];
    v.set(l.find("txtPetName"), centre(name));
    // Between the arrow buttons.
    if let (Some(t), Some(a), Some(b)) = (l.find("txtPetName"), l.find("btnPrevPet"), l.find("btnNextPet")) {
        let r = l.nodes[t].rect;
        v.rect.insert(t, [r[0].max(l.nodes[a].rect[2] + 2.0), r[1], r[2].min(l.nodes[b].rect[0] - 2.0), r[3]]);
    }
    v.set(l.find("txtStageRace"), centre(&format!("{kind}  ({} of {})", i + 1, pets.len())));
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
    // Laid out as PetStatsSpellbookPage is: the portrait (the item's picture) top left with its rating under it,
    // the talents column below; the pet's stage over the stat block on the right. Labels and values use the
    // page's own WizardFancy (0.5 for labels and values, 0.4 for list entries); prose uses WizardNormal, as
    // Wizard101's descriptions do, since WizardFancy is all small capitals.
    let label = TextStyle::new("WizardFancy", 0.5, BLACK);
    let value = TextStyle::new("WizardFancy", 0.5, BLACK).right();
    let heading = TextStyle::new("WizardFancy", 0.5, BLACK).centred();
    let entry = TextStyle::new("WizardFancy", 0.4, BLACK);
    let prose = TextStyle::new("WizardNormal", 0.42, BLACK).wrapped();
    let rule = ImColor32::from_rgba(90, 60, 30, 110);
    if let Some(p) = l.find("wndPortrait") {
        let r = l.nodes[p].rect;
        icon(v, d.items, *id, [r[0] + 8.0, r[1] + 8.0, r[2] - 8.0, r[3] - 8.0]);
    }
    // Beside the portrait, where Wizard101 shows the pet's card: what kind of companion it is.
    let side = [236.0, 60.0, 368.0, 196.0];
    v.add([side[0], side[1], side[2], side[1] + 24.0], kind.clone(), heading.clone());
    v.fills.push(([side[0] + 10.0, side[1] + 27.0, side[2] - 10.0, side[1] + 28.5], rule));
    let summon = if level.is_some() { "Summon at a Rebirth Monument" } else { "Call with the Spectral Steed Whistle" };
    v.add([side[0] + 4.0, side[1] + 34.0, side[2] - 4.0, side[3]], summon, prose.clone().centred());

    // The talents column (ctrlGenetics' place, from under the portrait): each talent in its own box, as Wizard101
    // lists a pet's talents.
    let column = l.find("ctrlGenetics").map(|i| l.nodes[i].rect).unwrap_or([76.0, 206.0, 369.0, 580.0]);
    let talents: Vec<&str> = if level.is_some() {
        vec!["Fights at your side", "Upgrade with Glovewort", "Recalled when you rest", "One summon at a time"]
    } else {
        vec!["Double jump", "Rides spiritsprings", "Outdoors only", "Heals on Rowa Raisins"]
    };
    let mut y = column[1] - 24.0;
    v.add([column[0], y, column[2], y + 24.0], "Talents", heading.clone());
    y += 28.0;
    for t in &talents {
        let b = [column[0] + 16.0, y, column[2] - 16.0, y + 30.0];
        v.fills.push(([b[0] + 3.0, b[1] + 3.0, b[2] - 3.0, b[3] - 3.0], TALENT_BOX));
        v.frames.push((b, box_frame()));
        v.add([b[0] + 8.0, b[1], b[2] - 8.0, b[3]], *t, entry.clone().centred());
        y += 35.0;
    }
    y += 10.0;
    v.add([column[0], y, column[2], y + 24.0], "Lore", heading.clone());
    y += 30.0;
    let tip = d.items.tooltip(*id, 1, d.names, false);
    let about = tip.last().filter(|_| tip.len() > 1).cloned().unwrap_or_default();
    v.add([column[0] + 12.0, y, column[2] - 10.0, column[3]], about, prose.clone());

    // The stage line over the bars, as Wizard101 names a pet's age there.
    v.show(l.find("wndAgingBkg"), true);
    hide(l, v, &["lblBaby", "lblTeen", "lblAdult", "lblAncient", "lblEpic", "lblMega", "lblUltra"]);
    if let Some(bar) = l.find("prgrAging").and_then(|b| l.nodes[b].parent) {
        v.show(Some(bar), false);
    }
    // (Wizard101's stage name; the upgrade bar below says it.)
    v.show(l.find("lblCurrentLevel"), false);

    // The stat block: label left, value right, a rule under each, as PetStatBlockControl draws its stats.
    if let Some(block) = l.find("ctrlStatBlock") {
        let r = l.nodes[block].rect;
        v.add([r[0], r[1] + 12.0, r[2], r[1] + 36.0], "Stats", heading.clone());
        let owned = pets.iter().filter(|p| p.2.is_some()).count();
        let rows: Vec<(&str, String)> = match level {
            Some(level) => vec![
                ("Upgrade", format!("+{level} / 10")),
                ("Summoned", "Rebirth Monuments".into()),
                ("Upgraded by", "Roderika".into()),
                ("Spirit ashes", owned.to_string()),
            ],
            None => vec![
                ("Mount", "Torrent".into()),
                ("Summoned", "Whistle".into()),
                ("Double jump", "Yes".into()),
                ("Spiritsprings", "Yes".into()),
                ("Spirit ashes", owned.to_string()),
            ],
        };
        let mut y = r[1] + 44.0;
        for (name, val) in rows {
            v.add([r[0] + 24.0, y, r[0] + 170.0, y + 24.0], name, label.clone());
            v.add([r[0] + 150.0, y, r[2] - 24.0, y + 24.0], val, value.clone());
            v.fills.push(([r[0] + 20.0, y + 27.0, r[2] - 20.0, y + 28.5], rule));
            y += 34.0;
        }
        let note = if level.is_some() {
            "Spirit ashes are summoned near Rebirth Monuments, and grow stronger with Grave or Ghost Glovewort."
        } else {
            "Torrent comes when you blow the whistle outdoors, and fades away indoors and in most boss fights."
        };
        v.add([r[0] + 24.0, y + 6.0, r[2] - 24.0, r[3] - 10.0], note, prose.clone());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Deck: DeckConfiguration.gui. Left: the deck (memorised spells); right: the spell list as cards.

/// The deck's tabs: Elden Ring's own spell classes (tools/er_book_data.py sorts each spell into one), each on the
/// Wizard101 school button whose symbol fits it: (button, class ("" = all), title). Sorceries on the left row,
/// incantations on the right (DeckConfiguration's two rows: six and five buttons).
const DECK_TABS: [(&str, &str, &str); 11] = [
    ("Cards_All", "", "Spell Deck"),
    ("Cards_Myth", "Glintstone", "Glintstone Sorceries"),
    ("Cards_Ice", "Carian", "Carian & Crystal Sorceries"),
    ("Cards_Balance", "Gravity", "Gravity Sorceries"),
    ("Cards_Shadow", "Night", "Night Sorceries"),
    ("Cards_Death", "Death", "Death, Blood & Rot"),
    ("Cards_Fire", "Fire", "Fire & Magma"),
    ("Cards_Life", "Golden Order", "Golden Order Incantations"),
    ("Cards_Storm", "Dragon Cult", "Dragon Cult Incantations"),
    ("Cards_Astral", "Frenzied Flame", "Frenzied Flame"),
    ("Cards_MonsterMagic", "Dragon Communion", "Dragon Communion"),
];
/// Where the buttons go (DeckConfiguration's button positions, left to right).
const DECK_TAB_X: [f32; 11] = [74.0, 123.0, 172.0, 221.0, 270.0, 319.0, 414.0, 463.0, 512.0, 561.0, 610.0];

/// The card art's height in its 128 x 256 texture.
const CARD_H: f32 = 196.0;

/// Wizard101's card frame for a class tab button (Astral's three schools share the Moon's; Monster Magic, here
/// Dragon Communion, is Fire's).
fn card_frame(button: &str) -> &'static str {
    match button {
        "Cards_Fire" | "Cards_MonsterMagic" => "GUI/Cards/School_Fire_Blnk.png",
        "Cards_Ice" => "GUI/Cards/School_Ice_Blnk.png",
        "Cards_Storm" => "GUI/Cards/School_Storm_Blnk.png",
        "Cards_Myth" => "GUI/Cards/School_Myth_Blnk.png",
        "Cards_Life" => "GUI/Cards/School_Life_Blnk.png",
        "Cards_Death" => "GUI/Cards/School_Death_Blnk.png",
        "Cards_Balance" => "GUI/Cards/School_Balance_Blnk.png",
        "Cards_Shadow" => "GUI/Cards/School_Shadow_Blnk.png",
        "Cards_Astral" => "GUI/Cards/School_Moon_Blnk.png",
        _ => "GUI/Cards/School_Treasure_Blnk.png",
    }
}

/// A spell's class tab.
fn school(items: &ItemData, item_id: u32) -> String {
    let raw = item_id & 0x0FFF_FFFF;
    match items.spell(raw) {
        Some(s) if !s.school.is_empty() => s.school.clone(),
        _ if (4000..6000).contains(&(raw % 2_000_000)) => "Glintstone".into(),
        _ => "Golden Order".into(),
    }
}

fn deck(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = DECK_TABS.len();
    hide(l, v, &["FusionBackground", "TieredSpellMPUnlockedBackground", "TieredSpellMPUnlockedList", "SpellFusionList",
                 "SchoolPipPanel", "CloseTSMPUnlockedPageButton", "SpellTreasureList", "SchoolPageSpellList",
                 "TutorialLogBackground1", "EquipBorder"]);
    grey(l, v, &["PrevDeck", "NextDeck", "NewDeckName", "EquipButton", "Help", "QuickSellButton", "NextItemSpells",
                 "PrevItemSpells", "TreasureCardButton", "FusionCardButton"]);
    // The class buttons, in order (Shadow and Monster Magic are hidden in Wizard101's layout).
    let mut row = None;
    for (i, (button, class, title)) in DECK_TABS.iter().enumerate() {
        let Some(b) = l.find(button) else { continue };
        let r = l.nodes[b].rect;
        let x = DECK_TAB_X[i];
        let nr = [x, r[1], x + (r[2] - r[0]), r[3]];
        v.rect.insert(b, nr);
        v.show(Some(b), true);
        v.tip(nr, vec![if class.is_empty() { "All Spells".to_string() } else { title.to_string() }]);
        row = Some(row.map_or(nr, |a: [f32; 4]| [a[0].min(nr[0]), a[1].min(nr[1]), a[2].max(nr[2]), a[3].max(nr[3])]));
    }
    v.tab_row = row;
    let (button, tab_school, title) = DECK_TABS[st.sub % DECK_TABS.len()];
    let chosen = l.find(button);
    if let (Some(c), Some(m), Some(first)) = (chosen, l.find("TabBackground"), l.find("Cards_All")) {
        v.checked.insert(c);
        let (mr, fr) = (l.nodes[m].rect, l.nodes[first].rect);
        let x = DECK_TAB_X[st.sub % DECK_TABS.len()] - fr[0];
        v.rect.insert(m, [mr[0] + x, mr[1], mr[2] + x, mr[3]]);
    }
    v.set(l.find("DeckTitle"), centre(title));
    let Some(sheet) = d.sheet else { return };
    let memorized: Vec<u32> = sheet.spells.iter().filter(|s| **s >= 0).map(|s| *s as u32).collect();
    let used: u32 = memorized.iter().map(|s| d.items.spell(*s).map_or(1, |x| x.slots.max(1))).sum();
    v.set(l.find("DeckName"), centre(&format!("Memory Slots {used}/{}", sheet.magic_slots)));
    // The deck (Wizard101's grid of the cards in the deck): every spell carried, as small cards; memorised ones
    // are marked.
    let mut carried: Vec<(String, u32)> =
        sheet.items.iter().filter(|(id, _)| is_spell(*id)).map(|(id, _)| (d.names.item(*id), *id & 0x0FFF_FFFF)).collect();
    carried.sort_by_key(|(_, id)| (!memorized.contains(id), *id));
    carried.dedup_by(|a, b| a.1 == b.1);
    if let Some(grid) = l.find("CardsInDeck") {
        let node = &l.nodes[grid];
        let [cw, ch] = node.card.unwrap_or([33.0, 33.0]);
        let [gx, _] = node.gap.unwrap_or([6.0, 6.0]);
        let r = node.rect;
        let columns = (((r[2] - r[0]) + gx) / (cw + gx)).floor().max(1.0) as usize;
        let x0 = r[0] + ((r[2] - r[0]) - (columns as f32 * (cw + gx) - gx)) / 2.0;
        let rows = (((r[3] - r[1]) + gx) / (ch + gx)).floor().max(1.0) as usize;
        for (i, (_, id)) in carried.iter().take(columns * rows).enumerate() {
            let x = x0 + (i % columns) as f32 * (cw + gx);
            let y = r[1] + 3.0 + (i / columns) as f32 * (ch + gx);
            let cell = [x, y, x + cw, y + ch];
            // Wizard101's deck shows each card's picture: here the spell's own icon.
            if memorized.contains(id) {
                v.fills.push(([x - 2.0, y - 2.0, x + cw + 2.0, y + ch + 2.0], MEMORIZED));
            }
            icon(v, d.items, 0x4000_0000 | id, cell);
            v.tip(cell, d.items.tooltip(0x4000_0000 | id, 1, d.names, memorized.contains(id)));
        }
    }
    // The loadout (Wizard101's item-card slots below the deck): Elden Ring's fourteen memory slots in order, the
    // ones not unlocked yet dimmed.
    if let Some(slots) = l.find("ItemSpells") {
        let r = l.nodes[slots].rect;
        let (cw, ch) = ((r[2] - r[0]) / 8.0, (r[3] - r[1]) / 2.0);
        for i in 0..16 {
            let spell = sheet.spells.get(i).copied().unwrap_or(-1);
            let (x, y) = (r[0] + (i % 8) as f32 * cw, r[1] + (i / 8) as f32 * ch);
            let inner = [x + cw * 0.16, y + ch * 0.16, x + cw * 0.84, y + ch * 0.84];
            if i >= (sheet.magic_slots as usize).min(14) {
                v.fills.push((inner, LOCKED));
                continue;
            }
            if spell >= 0 {
                let id = 0x4000_0000 | spell as u32;
                icon(v, d.items, id, inner);
                v.tip([x, y, x + cw, y + ch], d.items.tooltip(id, 1, d.names, true));
            } else {
                v.tip([x, y, x + cw, y + ch], vec![format!("Memory slot {}", i + 1), "Empty".into()]);
            }
        }
    }
    let mut known: Vec<(String, u32)> = sheet
        .items
        .iter()
        .filter(|(id, _)| is_spell(*id))
        .filter(|(id, _)| tab_school.is_empty() || school(d.items, *id) == tab_school)
        .map(|(id, _)| (d.names.item(*id), *id))
        .collect();
    known.sort();
    known.dedup_by(|a, b| a.0 == b.0);
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
    for (i, (name, id)) in known.iter().skip(st.page * per_page).take(per_page).enumerate() {
        let (cx, cy) = ((i % columns) as f32, (i / columns) as f32);
        let x = r[0] + cx * (cw + gx);
        let y = r[1] + cy * (ch + gy);
        let raw = id & 0x0FFF_FFFF;
        let spell = d.items.spell(raw);
        // The class's Wizard101 school: its card frame, and its tab button's symbol in the frame's corner.
        let class = school(d.items, *id);
        let tab = DECK_TABS.iter().position(|t| t.1 == class).unwrap_or(1);
        let frame = card_frame(DECK_TABS[tab].0);
        let card = |fx: f32, fy: f32| [x + fx / 128.0 * cw, y + fy / CARD_H * ch];
        // The picture: the spell's icon in the frame's window, on a dark backing.
        let (w0, w1) = (card(10.0, 33.0), card(119.0, 117.0));
        v.fills.push(([w0[0], w0[1], w1[0], w1[1]], ImColor32::from_rgb(28, 22, 34)));
        let side = (w1[1] - w0[1]).min(w1[0] - w0[0]);
        let ix = (w0[0] + w1[0] - side) / 2.0;
        icon(v, d.items, *id, [ix, w0[1], ix + side, w0[1] + side]);
        v.images.push(([x, y, x + cw, y + ch], frame.into(), [0.0, 0.0, 1.0, CARD_H / 256.0]));
        if let Some(b) = l.find(DECK_TABS[tab].0) {
            let n = &l.nodes[b];
            if let Some(img) = &n.img {
                let (c0, c1) = (card(101.0, 28.0), card(125.0, 52.0));
                v.images.push(([c0[0], c0[1], c1[0], c1[1]], img.clone(), n.frames[0].unwrap_or(n.uv)));
            }
        }
        let (t0, t1) = (card(8.0, 4.0), card(120.0, 28.0));
        v.add([t0[0], t0[1], t1[0], t1[1]], name.clone(), TextStyle::new("WizardFancy", 0.34, YELLOW).centred().outlined());
        if let Some(spell) = spell {
            // The cost in the pip triangle, the class and kind on the parchment.
            let (p0, p1) = (card(2.0, 28.0), card(32.0, 46.0));
            v.add([p0[0], p0[1], p1[0], p1[1]], spell.fp.to_string(),
                  TextStyle::new("WizardFancy", 0.36, WHITE).centred().outlined());
            let (b0, b1) = (card(8.0, 124.0), card(120.0, 188.0));
            v.add([b0[0], b0[1], b1[0], b1[1]], format!("{}\n{}", spell.er_school, spell.kind),
                  TextStyle::new("WizardNormal", 0.3, BLACK).centred());
        }
        v.tip([x, y, x + cw, y + ch], d.items.tooltip(*id, 1, d.names, sheet.spells.contains(&(raw as i32))));
    }
    if known.is_empty() {
        v.add([r[0], r[1] + 40.0, r[2], r[1] + 80.0], "No spells", TextStyle::new("WizardFancy", 0.6, BLACK).centred());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Quests: QuestListWindow.gui; the cards are QuestInfoWindow.gui (drawn by book.rs into each card window).

/// Quest cards per page (QuestListWindow's four card windows).
pub const QUESTS_PER_PAGE: usize = 4;

fn quests(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = 1;
    grey(l, v, &["SelectWorldButton", "SelectWorldIconButton", "TutorialLogButton", "QuestLogButton"]);
    for counter in ["QuestLogAllCounter", "QuestLogMainlineAndSideCounter", "QuestLogActivitiesCounter",
                    "QuestLogReadyToTurnInCounter", "QuestLogHiddenCounter"] {
        v.show(l.find(counter), false);
    }
    let pages = d.quests.len().div_ceil(QUESTS_PER_PAGE).max(1);
    st.page = st.page.min(pages - 1);
    if st.page == 0 {
        grey(l, v, &["btnPrevPage"]);
    }
    if st.page + 1 >= pages {
        grey(l, v, &["btnNextPage"]);
    }
    if d.quests.is_empty() {
        v.add([110.0, 200.0, 680.0, 240.0], "Your quest log is empty.", TextStyle::new("WizardFancy", 0.7, BLACK).centred());
        v.add([150.0, 260.0, 640.0, 400.0],
              "Quests are given to you by the people of Unicorn Way. Talk to them to take on a quest; it will be \
               written here with your current goal.",
              TextStyle::new("WizardNormal", 0.5, BLACK).centred().wrapped());
    }
}

/// Fills quest card `q` (QuestInfoWindow.gui).
pub fn quest_card(card: &Layout, q: &Active, v: &mut View) {
    for n in ["wndReward1", "wndReward2", "wndReward3", "wndReward4", "wndReward5", "btnHideQuest", "btnUnhideQuest"] {
        v.show(card.find(n), false);
    }
    // Wizard101's own styles, on one line (shrunk to fit) instead of wrapping over the next text.
    let set = |v: &mut View, name: &str, text: String| {
        if let Some(id) = card.find(name) {
            v.text.insert(id, centre(&text));
            if let Some(mut style) = card.nodes[id].text_style.clone() {
                style.wrap = false;
                style.middle = true;
                v.style.insert(id, style);
            }
        }
    };
    set(v, "txtName", q.title.clone());
    set(v, "txtGoal", q.goal.clone());
    // The other goals open now, across both objective boxes.
    hide(card, v, &["txtGoalObjective1", "txtGoalObjective2"]);
    if !q.other_goals.is_empty() {
        v.add([18.0, 134.0, 255.0, 168.0], format!("Also: {}", q.other_goals.join("; ")),
              TextStyle::new("WizardNormal", 0.36, BLACK).centred().wrapped());
    }
    match q.progress {
        Some((n, of)) => set(v, "txtGoalCounter", format!("{n}/{of}")),
        None => {
            v.show(card.find("imgGoalCounter"), false);
            v.show(card.find("txtGoalCounter"), false);
        }
    }
    set(v, "txtZone", q.place.clone());
    let world = if q.ready { format!("Ready: return to {}", q.turn_in) } else { "Wizard City".to_string() };
    set(v, "txtWorld", world);
    v.show(card.find("LeftMainline"), q.main);
    v.show(card.find("RightMainline"), q.main);
}

// ---------------------------------------------------------------------------------------------------------------
// Map: MapSpellbookPage.gui. Wizard101 draws the zone map here; this book opens Elden Ring's own map instead.

/// The part of the map page that is cut out to show Elden Ring's map while it is open.
pub const MAP_HOLE: [f32; 4] = [36.0, 56.0, 724.0, 524.0];
/// The copy of the game's screen while its map is open (book.rs).
pub const MAP_PICTURE: &str = "@screen/map";

/// The map's place on the page: the screen's shape (`aspect` = width / height), as big as fits in MAP_HOLE with a
/// margin of page around it, as if the map were printed on the book's two pages.
pub fn map_rect(aspect: f32) -> [f32; 4] {
    let h = MAP_HOLE;
    let (room_w, room_h) = (h[2] - h[0] - 16.0, h[3] - h[1] - 16.0);
    let (w, ht) = if room_w / room_h > aspect { (room_h * aspect, room_h) } else { (room_w, room_w / aspect) };
    let (cx, cy) = ((h[0] + h[2]) / 2.0, (h[1] + h[3]) / 2.0);
    [cx - w / 2.0, cy - ht / 2.0, cx + w / 2.0, cy + ht / 2.0]
}

/// The copied map screen on the page: all of MAP_HOLE inside a margin, and the part of the screen (uv) of that
/// shape, from its middle (a 16:9 screen loses a little of each side rather than being letterboxed).
pub fn map_picture_rect(aspect: f32) -> ([f32; 4], [f32; 4]) {
    let h = MAP_HOLE;
    let r = [h[0] + 8.0, h[1] + 8.0, h[2] - 8.0, h[3] - 8.0];
    let room = (r[2] - r[0]) / (r[3] - r[1]);
    let uv = if aspect > room {
        let keep = room / aspect;
        [(1.0 - keep) / 2.0, 0.0, (1.0 + keep) / 2.0, 1.0]
    } else {
        let keep = aspect / room;
        [0.0, (1.0 - keep) / 2.0, 1.0, (1.0 + keep) / 2.0]
    };
    (r, uv)
}

fn map(l: &Layout, d: &Data, v: &mut View) {
    v.set(l.find("ZoneName"), centre("The Lands Between"));
    v.show(l.find("MapLoadingIndicator"), d.map_opening);
    v.set(l.path("MapLoadingIndicator/PackageNameText"), centre("Opening Map..."));
    hide(l, v, &["txtGoHomeTimer", "txtGotoDormTimer"]);
    grey(l, v, &["GoHomeButton", "GotoDormButton"]);
    if d.map_open {
        // The whole map screen, scaled onto the page in its own shape; or, without a copy of the screen, a window
        // of that shape onto the map behind the book.
        match d.map_picture {
            Some(aspect) => {
                // The page's whole map area, the screen's sides cut off to fit (not squeezed).
                let (r, uv) = map_picture_rect(aspect);
                v.fills.push(([r[0] - 6.0, r[1] - 6.0, r[2] + 6.0, r[3] + 6.0], ImColor32::from_rgb(74, 46, 24)));
                v.images.push((r, MAP_PICTURE.into(), uv));
                v.picture = Some(r);
            }
            None => v.hole = Some(map_rect(16.0 / 9.0)),
        }
        v.add([MAP_HOLE[0], MAP_HOLE[3] + 4.0, MAP_HOLE[2], MAP_HOLE[3] + 26.0],
              "Close the map as usual, or click another tab of the book", TextStyle::new("WizardFancy", 0.45, BLACK).centred());
        return;
    }
    if !d.map_opening {
        let body = TextStyle::new("WizardNormal", 0.5, BLACK).centred().wrapped();
        v.add([120.0, 200.0, 680.0, 230.0], "World Map", TextStyle::new("WizardFancy", 0.8, BLACK).centred());
        v.add([150.0, 250.0, 650.0, 420.0],
              "Elden Ring's map opens inside the book as soon as you turn to this page, with its markers, grace \
               travel and zoom. Close it as usual and the book comes back; travel to a grace and the book closes.\n\n\
               Press A, Enter or the magnifier below to open the map again.",
              body);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Crafting: RecipeWindow.gui. Right: crafting materials (or cookbooks, with the Reagents button); left: the kit.

/// Reagent cards (top left corners) on the crafting pages, two by two on each page as in ReagentWindow.gui, moved
/// in to clear the page's doodles and buttons, and their size (Wizard101's 99 x 153 card, a little smaller).
const REAGENT_CARDS: [[f32; 2]; 8] = [[92.0, 124.0], [214.0, 124.0], [92.0, 312.0], [214.0, 312.0],
                                      [448.0, 124.0], [576.0, 124.0], [448.0, 312.0], [576.0, 312.0]];
const REAGENT_CARD: [f32; 2] = [96.0, 147.0];

/// A crafting material's Wizard101 school, by what it is: fire and embers Fire; frost, crystal and silver Ice;
/// lightning and feathers Storm; beasts and their parts Myth; plants and fruit Life; poison, rot, blood and graves
/// Death; glintstone, butterflies and the uncanny Shadow (Astral); stones and the rest Balance.
fn reagent_school(name: &str) -> &'static str {
    const RULES: &[(&str, &str)] = &[
        ("Smoldering", "Fire"), ("Glint", "Shadow"), ("Butterfly", "Shadow"), ("Empyrean", "Shadow"),
        ("Eye of", "Shadow"), ("Finger", "Shadow"), ("Calculus", "Shadow"), ("Black Pyrefly", "Shadow"),
        ("Shadow", "Shadow"), ("Cracked Crystal", "Shadow"), ("Fire", "Fire"), ("Ember", "Fire"),
        ("Volcanic", "Fire"), ("Calorbloom", "Fire"), ("Furnace", "Fire"), ("Pyrefly", "Fire"), ("Melted", "Fire"),
        ("Rimed", "Ice"), ("Frozen", "Ice"), ("Crystal", "Ice"), ("Winter", "Ice"), ("Dew", "Ice"),
        ("Pearlescent", "Ice"), ("Silver", "Ice"), ("Fulgur", "Storm"), ("Storm", "Storm"), ("Pinion", "Storm"),
        ("Plumage", "Storm"), ("Feather", "Storm"), ("Gas Stone", "Storm"), ("Poison", "Death"), ("Toxic", "Death"),
        ("Gravel", "Balance"), ("Grave", "Death"), ("Blood", "Death"), ("Human Bone", "Death"), ("Resin", "Death"),
        ("Putrescence", "Death"), ("Mold", "Death"), ("Miranda", "Death"), ("Deep-Purple", "Death"),
        ("Sanguine", "Death"), ("Ghostflame", "Death"), ("Scarlet", "Death"), ("Dung", "Myth"), ("Horn", "Myth"),
        ("Herba", "Life"), ("Erdleaf", "Life"), ("Flower", "Life"), ("Lily", "Life"), ("Altus", "Life"),
        ("Rowa", "Life"), ("Rada", "Life"), ("Fruit", "Life"), ("Mushroom", "Life"), ("Moss", "Life"),
        ("Bud", "Life"), ("Sunflower", "Life"), ("Sanctuary", "Life"), ("Gold Firefly", "Life"), ("Grape", "Life"),
        ("Burgeon", "Life"), ("Leaf", "Life"), ("Blessed", "Life"), ("Beast", "Myth"), ("Meat", "Myth"), ("Flesh", "Myth"), ("Liver", "Myth"),
        ("Fang", "Myth"), ("Fowl", "Myth"), ("Egg", "Myth"), ("Ovary", "Myth"), ("Excrement", "Myth"),
    ];
    const MORE_MYTH: [&str; 6] = ["Dragonfly", "Centipede", "Scorpion", "Maggot", "Cricket", "Scale"];
    RULES.iter().find(|(k, _)| name.contains(k)).map(|(_, s)| *s)
        .or_else(|| MORE_MYTH.iter().any(|k| name.contains(k)).then_some("Myth"))
        .unwrap_or("Balance")
}

fn crafting(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = 1;
    v.set(l.find("title"), centre("Crafting"));
    // RecipeWindow is a window of its own in Wizard101, with a full-size background that would cover the book's
    // tabs: draw it on the book's own page instead.
    if let Some(station) = l.find("StationBackground") {
        v.no_back.insert(station);
        if let Some(edge) = l.find_under(station, "Doodle") {
            v.show(Some(edge), false);
        }
    }
    // Exit and the furniture tab sit where the book's own tabs are (the book has its Close tab).
    hide(l, v, &["BonusInfoIcon", "CompletedText", "UnequipButton", "TimerSlot_01", "Exit", "Tab_FurnitureStuff"]);
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
    let mut list: Vec<(String, u32, u32)> = if st.cookbooks {
        sheet.key_items.iter().map(|(id, n)| (d.names.item(*id), *n, *id)).filter(|(n, _, _)| n.contains("Cookbook")).collect()
    } else {
        sheet.items.iter().filter(|(id, _)| is_material(*id)).map(|(id, n)| (d.names.item(*id), *n, *id)).collect()
    };
    list.sort();
    if st.cookbooks && !list.is_empty() {
        // The chosen cookbook (click one on the right), open on the left page: its text and recipes.
        st.cookbook = st.cookbook.min(list.len() - 1);
        let (name, _, id) = &list[st.cookbook];
        hide(l, v, &["portrait_background", "card_background", "IngredientBackground"]);
        // The potion doodle down the left edge would sit under the text.
        for (i, n) in l.nodes.iter().enumerate() {
            if n.name == "Doodle" && n.rect[2] < 200.0 {
                v.show(Some(i), false);
            }
        }
        v.set(l.find("labelItemHighlighted"), centre(name));
        // Inside the banner's frame.
        if let Some(t) = l.find("labelItemHighlighted") {
            let r = l.nodes[t].rect;
            v.rect.insert(t, [r[0] + 20.0, r[1], r[2] - 20.0, r[3]]);
        }
        let text = d.items.caption(id & 0x0FFF_FFFF).unwrap_or_else(|| "This cookbook's pages are blank.".into());
        // Long recipe lists in smaller type, so they fit the page.
        let size = if text.lines().count() > 12 { 0.34 } else { 0.4 };
        // Centred, and smaller still if it would run past the page (layout.rs shrinks wrapped text to its box).
        v.add([80.0, 168.0, 336.0, 520.0], text, TextStyle::new("WizardNormal", size, BLACK).centred().wrapped());
    }
    if !st.cookbooks {
        // Wizard101's reagents page (ReagentWindow.gui): the materials as item cards, four on each page, each with
        // its picture, a small school symbol top right and the count bottom right.
        hide(l, v, &["portrait_background", "card_background", "IngredientBackground"]);
        // The kit's name banner (labelItemHighlighted's sprite) would sit behind the first row of cards.
        v.show(l.find("labelItemHighlighted").and_then(|t| l.nodes[t].parent), false);
        for (i, n) in l.nodes.iter().enumerate() {
            if n.name == "Doodle" && n.rect[2] < 200.0 {
                v.show(Some(i), false);
            }
        }
        for r in &rows {
            v.show(Some(*r), false);
        }
        let page = paging(l, v, st, list.len(), REAGENT_CARDS.len(), ("PageNumber", "leftscroll", "rightscroll"));
        for (slot, &[x, y]) in REAGENT_CARDS.iter().enumerate() {
            let Some((name, count, id)) = list.get(page * REAGENT_CARDS.len() + slot) else { continue };
            let (w, h) = (REAGENT_CARD[0], REAGENT_CARD[1]);
            let card = |fx: f32, fy: f32| [x + fx / 128.0 * w, y + fy / CARD_H * h];
            // The picture fills the card's window; the card (all reagents the same colour) goes over it.
            let (w0, w1) = (card(10.0, 33.0), card(119.0, 117.0));
            v.fills.push(([w0[0], w0[1], w1[0], w1[1]], ImColor32::from_rgb(52, 40, 30)));
            let side = (w1[1] - w0[1]).min(w1[0] - w0[0]);
            let ix = (w0[0] + w1[0] - side) / 2.0;
            icon(v, d.items, *id, [ix, w0[1], ix + side, w0[1] + side]);
            v.images.push(([x, y, x + w, y + h], "GUI/Cards/School_Treasure_Blnk.png".into(), [0.0, 0.0, 1.0, CARD_H / 256.0]));
            let school = reagent_school(name);
            let (s0, s1) = (card(100.0, 29.0), card(126.0, 53.0));
            v.images.push(([s0[0], s0[1], s1[0], s1[1]], format!("GUI/Icons/Icon_{school}.png"), [0.0, 0.0, 1.0, 1.0]));
            let (t0, t1) = (card(6.0, 4.0), card(122.0, 30.0));
            v.add([t0[0], t0[1], t1[0], t1[1]], name.clone(), TextStyle::new("WizardFancy", 0.34, YELLOW).centred().outlined());
            let (b0, b1) = (card(8.0, 122.0), card(120.0, 190.0));
            v.add([b0[0], b0[1] + 4.0, b1[0], b1[1] - 18.0], format!("{school} Reagent"),
                  TextStyle::new("WizardNormal", 0.34, BLACK).centred().wrapped());
            v.add([b0[0], b1[1] - 16.0, b1[0] - 2.0, b1[1]], format!("x{count}"), TextStyle::new("WizardFancy", 0.38, BLACK).right());
            let mut tip = d.items.tooltip(*id, *count, d.names, false);
            tip.insert(1.min(tip.len()), format!("{school} reagent"));
            v.tip([x, y, x + w, y + h], tip);
        }
        if list.is_empty() {
            v.add([420.0, 200.0, 718.0, 240.0], "No materials yet", TextStyle::new("WizardFancy", 0.55, BLACK).centred());
        }
        return;
    }
    let page = paging(l, v, st, list.len(), rows.len(), ("PageNumber", "leftscroll", "rightscroll"));
    for (i, row) in rows.iter().enumerate() {
        match list.get(page * rows.len() + i) {
            Some((name, _, _)) => {
                let r = l.nodes[*row].rect;
                if page * rows.len() + i == st.cookbook {
                    v.checked.insert(*row);
                }
                v.add([r[0] + 22.0, r[1], r[2] - 22.0, r[3]], name.clone(),
                      TextStyle::new("WizardFancy", 0.42, LIGHT_YELLOW).centred());
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
      Wizard101; the round buttons along the top of a page are its categories. Point at an item for its stats."),
    ("Tab_Combat", "Controller",
     "LT / RT or D-pad up and down: previous / next page of the book (the tabs on the right).\n\
      LB / RB: previous / next category along the top of the page.\nD-pad left and right: turn the page of a list.\n\
      Right stick: turn your character in the Backpack (it turns by itself; a click on it stops it).\n\
      A: use the tab (the World Map tab opens Elden Ring's map).\nB or View: close the book."),
    ("Tab_Interface", "Keyboard and Mouse",
     "Click a tab or a button; point at an item for its stats. Up / Down arrows: previous / next page of the book. \
      Q / E: previous / next category. Left / Right arrows or the mouse wheel: turn the page of a list. Drag your \
      character in the Backpack to turn it. Enter: use the tab. B or Esc: close the book."),
    ("Tab_Character", "The Pages",
     "Character: level, health, mana (FP), energy (stamina), flasks, attributes and achievements.\nBackpack: your \
      character and everything you carry.\nPets: Torrent and your spirit ashes.\nDeck: your spells by Elden Ring's \
      schools, and the memorized ones in their memory slots.\nQuests: the Unicorn Way quests you have taken on.\nWorld Map: Elden Ring's map.\nCrafting: \
      materials and cookbooks.\nSettings: the HUD's settings."),
];

fn help(l: &Layout, st: &mut State, v: &mut View) {
    st.subs = HELP.len();
    let (button, title, text) = HELP[st.sub % HELP.len()];
    let chosen = l.find(button);
    if let Some(c) = chosen {
        v.checked.insert(c);
    }
    marker(l, v, "TabBackground", "Tab_Questing", chosen);
    v.tab_row = row_of(l, &HELP.map(|h| h.0));
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
