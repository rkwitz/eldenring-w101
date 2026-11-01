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
/// The shade over a spell card already in the deck.
const IN_DECK: ImColor32 = ImColor32::from_rgba(30, 20, 10, 120);
/// DeckConfiguration's slot frame (its top half: a spell slot; the bottom half is for treasure cards).
const DECK_SLOT: &str = "GUI/DeckConfig/Background_Slot.png";
/// Memory slot cells per row in the deck.
const DECK_COLUMNS: usize = 8;

/// The Moon of Nokstella talisman: two more memory slots.
const NOKSTELLA: i32 = 1140;

/// Memory slots every character has (PlayerCommonParam baseMagicSlotSize).
const BASE_MEMORY_SLOTS: u32 = 2;

/// Memory slots the player has, as Elden Ring's Memorize Spell menu counts them: the two everyone has, one per
/// Memory Stone (PlayerGameData.unlocked_magic_slots counts the stones: 8 in Ryan's save, which also holds 8 Memory
/// Stones), and the Moon of Nokstella's two (its SpEffect 311400 changeMagicSlot 2) while it is worn.
pub fn memory_slots(sheet: &Sheet) -> u32 {
    let nokstella = sheet.equipment[17..21].contains(&NOKSTELLA);
    (BASE_MEMORY_SLOTS + sheet.magic_slots as u32 + if nokstella { 2 } else { 0 }).min(14)
}

/// Memory slots the memorised spells take (each spell its MagicParam slot count).
pub fn slots_used(sheet: &Sheet, items: &ItemData) -> u32 {
    let mut seen = Vec::new();
    sheet.spells.iter().filter(|s| **s >= 0 && !seen.contains(*s) && { seen.push(**s); true })
        .map(|s| items.spell(*s as u32).map_or(1, |x| x.slots.max(1))).sum()
}

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
    /// The Backpack item last clicked (inventory ID), shown in the picture on talisman, arrow and key item pages.
    pub selected: Option<u32>,
    /// Set while binding: the Backpack picture shows this weapon-type model alone (an arrow or bolt) instead of
    /// the character.
    pub item_model: Option<u32>,
    /// The layout node of the chosen filter button (deck), to show it pressed.
    pub filter_node: Option<usize>,
    /// What the page's own clickable things are this frame, set while binding: (rect, what).
    pub hits: Vec<([f32; 4], Hit)>,
    /// A short message under the page (what a click did or why it couldn't), and when it was set.
    pub note: Option<(String, std::time::Instant)>,
    /// How many subs/pages the current tab has (for wrapping), set while binding.
    subs: usize,
    /// How many pets there are to page through.
    pets: usize,
    /// Set while binding: what the page's equip button acts on (the Backpack's chosen item, the Pets page's
    /// spirit ash), if it can be equipped.
    equip_target: Option<u32>,
}

/// Something on a page that a click (or double click) acts on, besides its layout buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    /// A backpack row's item (inventory ID with category bits).
    Item(u32),
    /// A spell card in the spell list (magic param ID): memorise it.
    Learn(i32),
    /// A memorised spell in the deck: memory slot index.
    Forget(usize),
    /// A category tab the page draws itself (the crafting page's reagent tabs): choose sub-page n.
    Sub(usize),
    /// A settings row's arrow drawn by the page (rows 6 and 7): row, step.
    Setting(usize, i32),
}

pub enum Action {
    None,
    OpenMap,
    /// Close the book (Settings' OK is SettingsOk now; kept for other pages).
    #[allow(dead_code)]
    Close,
    Tab(Tab),
    /// Settings row, step (-1 or +1).
    Setting(usize, i32),
    /// Settings' OK: apply the staged changes and close.
    SettingsOk,
    /// Settings: Quit (to Elden Ring's title screen; true: then to the desktop).
    Quit(bool),
    /// The equip button: equip the item (inventory ID), or take it off if it is equipped.
    Equip(u32),
    /// A warp button on the map page: the event flag that asks the mod's common.emevd event to warp the player.
    Warp(u32),
}

/// Event flags the map page's buttons set; common.emevd events (tools/emevd.py book_warps) see one come on, clear
/// it, show the HUD's load screen and warp the player with 2003[14] Warp Player: to The First Step on Unicorn Way
/// (the arrows, Wizard101's Go to the Commons) and to the Roundtable Hold (the house, Go Home).
pub const WARP_UNICORN_WAY: u32 = 1042362910;
pub const WARP_ROUNDTABLE: u32 = 1042362911;

/// Everything the pages show.
pub struct Data<'a> {
    pub sheet: Option<&'a Sheet>,
    pub stats: Option<game::Stats>,
    pub names: &'a Names,
    pub items: &'a ItemData,
    /// The quests in progress (from the quest event flags).
    pub quests: &'a [Active],
    /// The Settings page's rows on its current tab.
    pub settings: Vec<SettingRow>,
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

/// The Wizard101 badge picture for achievement `index`: the endings and "Elden Ring" the blue-ribboned medal, the
/// Shardbearers the red-ribboned one, the legendary collections the starred one, the places the winged one, the
/// other bosses the plain medals in turn.
fn badge_art(index: usize) -> &'static str {
    const ART: [&str; 6] = ["GUI/Art/Art_Badge01.png", "GUI/Art/Art_Badge02.png", "GUI/Art/Art_Badge03.png",
                            "GUI/Art/Art_Badge04.png", "GUI/Art/Art_Badge05.png", "GUI/Art/Art_Badge06.png"];
    let name = ACHIEVEMENTS[index].0;
    match index {
        0..=3 | 13 | 14 => ART[5],
        _ if name.starts_with("Shardbearer") => ART[3],
        _ if name.starts_with("Legendary") => ART[2],
        39 | 41 => ART[4],
        _ => ART[index % 2],
    }
}

impl State {
    pub fn reset(&mut self) {
        self.page = 0;
        self.sub = 0;
        self.pet = 0; // the Pets page opens on the equipped spirit ash
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

/// What equipping or taking off an item does: a ChrAsm slot and Some(item) to put there or None to empty it
/// (Elden Ring's equip function), or a spirit ash into (true) or out of (false) the quick items, where Elden Ring
/// uses them from.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Equip {
    Gear(u32, Option<u32>),
    Quick(bool),
}

/// Whether the item is worn, held, or (a spirit ash) in the quick items.
pub fn is_equipped(sheet: &Sheet, item_id: u32) -> bool {
    equipped_ids(sheet).contains(&item_id) || (item_id >> 28 == 4 && sheet.quick_items.contains(&item_id))
}

/// What a double click in the Backpack (or the equip button under it, or the Pets page's) does with an item: an
/// equipped one comes off (its slot is left empty); otherwise weapons go in the right hand's first slot, shields
/// and torches the left's, arrows and bolts their first slot, armour its own slot, talismans the first free
/// talisman slot, spirit ashes the first free quick item slot. None for items that can't be equipped.
pub fn equip_slot(sheet: &Sheet, items: &ItemData, name: &str, item_id: u32) -> Option<Equip> {
    let raw = item_id & 0x0FFF_FFFF;
    let worn = |range: std::ops::Range<usize>| {
        sheet.equipment[range.clone()].iter().position(|e| *e == raw as i32).map(|i| Equip::Gear((range.start + i) as u32, None))
    };
    match item_id >> 28 {
        0 => worn(0..12).or_else(|| {
            let kind = items.weapon_kind(raw).unwrap_or("");
            let slot = match kind {
                k if k.contains("Shield") || k == "Torch" => 0,
                "Arrow" | "Greatarrow" => 6,
                "Bolt" | "Greatbolt" => 7,
                _ => 1,
            };
            Some(Equip::Gear(slot, Some(item_id)))
        }),
        1 => worn(12..16).or_else(|| items.armour(raw).map(|a| Equip::Gear(12 + a.slot.min(3), Some(item_id)))),
        2 => worn(17..21).or_else(|| {
            let pouches = (1 + sheet.talisman_slots as usize).min(4);
            let free = sheet.equipment[17..17 + pouches].iter().position(|e| *e <= 0).unwrap_or(0);
            Some(Equip::Gear(17 + free as u32, Some(item_id)))
        }),
        4 if is_spirit_ash(name) => Some(Equip::Quick(!sheet.quick_items.contains(&item_id))),
        _ => None,
    }
}

/// Wizard101's equip buttons: the fist (Button_Equip) equips, the open hand (Button_Unequip) takes off; frames
/// normal, highlighted, pressed, greyed.
pub const EQUIP_ART: &str = "GUI/InventoryPage/Button_Equip.png";
pub const UNEQUIP_ART: &str = "GUI/Buttons/Button_Unequip.png";

/// Fills the page for `tab`.
pub fn bind(tab: Tab, l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.hits.clear();
    st.item_model = None;
    st.equip_target = None;
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
        (Tab::Backpack, "Equip_Item") | (Tab::Pets, "EquipButton") => {
            if let Some(id) = st.equip_target {
                return Action::Equip(id);
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
        (Tab::Map, "GoHomeButton") => return Action::Warp(WARP_UNICORN_WAY),
        (Tab::Map, "GotoDormButton") => return Action::Warp(WARP_ROUNDTABLE),
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
        (Tab::Settings, "OkButton") => return Action::SettingsOk,
        (Tab::Settings, "HelpButton") => return Action::Tab(Tab::Help),
        (Tab::Settings, "QuitButton") => return Action::Quit(false),
        (Tab::Settings, "IdleButton") => return Action::Quit(true),
        (Tab::Settings, _) if crate::options::TABS.iter().any(|t| t.0 == name) => {
            st.page = crate::options::TABS.iter().position(|t| t.0 == name).unwrap_or(0);
            st.sub = 0;
        }
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

/// Background_Character_Right_Badges (256 x 512 drawn at 390,0 - 720,600: 1.289 x 1.172 per art pixel) has, per
/// row, a darker badge-shaped outline (a ribbon trapezoid over a spiky seal: Art_Badge01's own silhouette) and a bar
/// beside it. Measured in the art for row 0 (row top 143): the outline's ribbon top runs x 28-70, y 123, its seal
/// tip ends at y 168 and the next row's ribbon starts at y 169, i.e. page x 426.1-480.2, y 144.2-198.3: a 54 x 54
/// square (the 64 x 64 medal art) centred 1 px right of the row's left edge. Stored as (x from the row's left,
/// y from its top, x to, y to).
const BADGE_MEDAL: [f32; 4] = [-25.9, 1.2, 28.2, 55.3];
/// The bar's outline in the art (y 130-157, x to 449 page): x from, y from the row's top, x to, y to.
const BADGE_BAR: [f32; 4] = [474.0, 9.5, 675.0, 41.0];
/// The text area inside the row's Button_Item frame (256 x 32): inside its gold line, x 11-243, y 5-24.
const BADGE_TEXT: [f32; 4] = [11.0 / 256.0, 5.0 / 32.0, 243.0 / 256.0, 24.0 / 32.0];

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
    // The school badge on the portrait's ring: the character's school as the reagent cards' Icon_<School> symbol
    // on their medallion. Elden Ring has no school, so it follows the casting stat: Intelligence = Myth (the
    // deck's Glintstone), Faith = Life (Golden Order), Arcane = Death; none of them past 20 = Balance.
    if let Some(b) = l.find("SchoolIcon") {
        let [int, fai, arc] = [sheet.attributes[5], sheet.attributes[6], sheet.attributes[7]];
        let school = if int.max(fai).max(arc) <= 20 { "Balance" }
            else if int >= fai && int >= arc { "Myth" } else if fai >= arc { "Life" } else { "Death" };
        let r = l.nodes[b].rect;
        v.node_art.insert(b, REAGENT_MEDALLION.into());
        let inset = (r[2] - r[0]) * 0.2;
        v.images.push(([r[0] + inset, r[1] + inset, r[2] - inset, r[3] - inset], school_icon(school), [0.0, 0.0, 1.0, 1.0]));
        v.tip(r, vec![format!("School: {school}"), "From your highest casting stat".into()]);
    }
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
    v.set(l.find("TrainingPoints"), centre(&memory_slots(sheet).to_string()));
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
            // The row's own button art (Button_Item) lies on the background's bar, the badge's medal
            // (Art_Badge01-06) exactly over the background's badge outline, the status and name inside the
            // frame's gold line (one small line over one larger, both shrunk to fit its width).
            let bar = [BADGE_BAR[0], r[1] + BADGE_BAR[1], BADGE_BAR[2], r[1] + BADGE_BAR[3]];
            v.rect.insert(*row, bar);
            let medal = [r[0] + BADGE_MEDAL[0], r[1] + BADGE_MEDAL[1], r[0] + BADGE_MEDAL[2], r[1] + BADGE_MEDAL[3]];
            // Not earned: the medal faded, as Wizard101 shows badges still to earn (tinted, so it keeps its shape).
            let tint = if state == Some(true) { ImColor32::WHITE } else { ImColor32::from_rgba(200, 185, 160, 120) };
            v.tinted.push((medal, badge_art(a).into(), [0.0, 0.0, 1.0, 1.0], tint));
            let (bw, bh) = (bar[2] - bar[0], bar[3] - bar[1]);
            let inner = [bar[0] + BADGE_TEXT[0] * bw, bar[1] + BADGE_TEXT[1] * bh,
                         bar[0] + BADGE_TEXT[2] * bw, bar[1] + BADGE_TEXT[3] * bh];
            let split = inner[1] + (inner[3] - inner[1]) * 0.4;
            v.add([inner[0] + 2.0, inner[1] + 0.5, inner[2] - 2.0, split], status.to_string(),
                  TextStyle::new("WizardFancy", 0.0, fade(LIGHT_YELLOW)).centred());
            v.add([inner[0] + 2.0, split, inner[2] - 2.0, inner[3] - 0.5], name.to_string(),
                  TextStyle::new("WizardFancy", 0.0, fade(YELLOW)).centred());
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
    // Wizard101 keeps "Backpack" over every category (the category's name is in its button's tip).
    let _ = title;
    v.set(l.find("title"), centre("Backpack"));
    for prefix in ["crowns", "pvp", "rental", "crafted", "lock"] {
        for i in 0..8 {
            v.show(l.find(&format!("{prefix}{i}")), false);
        }
    }
    grey(l, v, &["Tab_HousingStuff", "Tab_MonsterMagic", "Tab_Photography", "EquipmentManager"]);
    // Only the equip button and the (greyed) armor sets button stay along the bottom.
    hide(l, v, &["Lock_Item", "Socket_Item", "btnFeedPet", "RecyclingBinButton", "QuickSell_Item", "Trash_Item"]);
    let rows: Vec<usize> = (1..=8).filter_map(|i| l.find(&format!("Item_{i}"))).collect();
    let Some(sheet) = d.sheet else {
        for r in &rows {
            v.show(Some(*r), false);
        }
        return;
    };
    let names = d.names;
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
        .map(|(id, n)| (names.item(*id), *n, is_equipped(sheet, *id), *id))
        .collect();
    // What is worn or held comes first (with Wizard101's closed hand beside it), then the rest by name.
    items.sort_by(|a, b| (!a.2, &a.0, a.3).cmp(&(!b.2, &b.0, b.3)));
    v.set(l.find("inventorySpace"), centre(&items.len().to_string()));
    let page = paging(l, v, st, items.len(), rows.len(), ("PageNumber", "leftscroll", "rightscroll"));
    // The chosen item (the last one clicked, else the first): the picture and the equip button act on it.
    let shown = st.selected.filter(|s| items.iter().any(|i| i.3 == *s)).or(items.first().map(|i| i.3));
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
                // Wizard101's mark beside an item that is worn or held: the closed hand (the fist, the row's own
                // "fist" window); nothing beside the rest.
                v.show(fist, false);
                if let (Some(f), true) = (fist, *is_equipped) {
                    v.images.push((l.nodes[f].rect, EQUIP_ART.into(), [0.0, 0.0, 0.5, 0.5]));
                }
                if shown == Some(*id) {
                    v.checked.insert(*row);
                }
                v.tip(r, d.items.tooltip(*id, *count, names, *is_equipped));
                st.hits.push((r, Hit::Item(*id)));
            }
            None => {
                v.show(Some(*row), false);
            }
        }
    }
    // The equip button (Wizard101's Equip_Item): the fist equips the chosen item, the open hand (Button_Unequip)
    // takes it off when it is equipped; greyed when it can't be equipped.
    let target = shown.and_then(|id| items.iter().find(|i| i.3 == id))
        .filter(|(name, _, _, id)| equip_slot(sheet, d.items, name, *id).is_some());
    match (l.find("Equip_Item"), target) {
        (Some(b), Some((_, _, equipped, id))) => {
            st.equip_target = Some(*id);
            if *equipped {
                v.node_art.insert(b, UNEQUIP_ART.into());
            }
        }
        _ => grey(l, v, &["Equip_Item"]),
    }
    // The portrait window shows the character, drawn from Elden Ring's models (model.rs, book.rs). On the
    // arrow page it shows the chosen item's own 3D model; talismans, spirit ashes, spells, consumables, ashes of
    // war and key items have no model in the game (they are only icons), so the chosen one's icon, large and
    // still.
    if let Some(p) = l.find("3DCharacter") {
        match (filter, shown) {
            (Filter::Ammo, Some(id)) if d.items.weapon_model(id & 0x0FFF_FFFF).is_some() => {
                st.item_model = d.items.weapon_model(id & 0x0FFF_FFFF);
                v.node_images.insert(p, MODEL_FULL.into());
            }
            (Filter::Talismans | Filter::KeyItems | Filter::Ammo | Filter::SpiritAshes | Filter::Spells
             | Filter::Consumables | Filter::AshesOfWar, Some(id)) => {
                let r = l.nodes[p].rect;
                let side = (r[2] - r[0]).min(r[3] - r[1]) * 0.8;
                let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
                icon(v, d.items, id, [cx - side / 2.0, cy - side / 2.0, cx + side / 2.0, cy + side / 2.0]);
            }
            (Filter::Talismans | Filter::KeyItems | Filter::Ammo | Filter::SpiritAshes | Filter::Spells
             | Filter::Consumables | Filter::AshesOfWar, None) => {}
            _ => {
                v.node_images.insert(p, MODEL_FULL.into());
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Pets: PetStatsSpellbookPage.gui. Wizard101's pets are closest to Torrent and the spirit ashes.

/// The pet portrait's frame (spritePortraitBack, 159 square in Wizard101) on the left page.
const PET_PORTRAIT: [f32; 4] = [127.0, 56.0, 317.0, 246.0];
/// PetTalentControl.gui's row background and Wizard101's talent icon sheet (8 x 8 cells of 64 px).
const TALENT_ROW: &str = "GUI/Backgrounds/Background_Pet_Talent.png";
const TALENT_ICONS: &str = "GUI/Talents/PetTalent_Complete_Color01.png";

fn pets(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = 1;
    grey(l, v, &["SnackButton", "EquipJewelButton", "ToggleTalentDisplay", "PetTomeButton", "PetBadgesButton"]);
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
        // The equipped spirit ash (in the quick items) first, so the page opens on it; then Torrent, then the rest.
        let (worn, rest): (Vec<_>, Vec<_>) = ashes.into_iter().partition(|a| sheet.quick_items.contains(&a.3));
        pets.splice(0..0, worn);
        pets.extend(rest);
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
    // The equip button (Wizard101's, which makes a pet the one that follows you): a spirit ash goes into the
    // quick items, where Elden Ring uses it from; the open hand when it is there already (a click takes it out),
    // the fist when not. Torrent is always at hand.
    match (d.sheet, level) {
        (Some(sheet), Some(_)) => {
            st.equip_target = Some(*id);
            if sheet.quick_items.contains(id) {
                v.node_art.insert(l.find("EquipButton").unwrap_or(usize::MAX), UNEQUIP_ART.into());
            }
        }
        _ => grey(l, v, &["EquipButton"]),
    }
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
    let prose = TextStyle::new("WizardNormal", 0.42, BLACK).wrapped();
    let rule = ImColor32::from_rgba(90, 60, 30, 110);
    v.set(l.find("txtTitle"), centre(if level.is_some() { "Spirit Ashes" } else { "Spectral Steed" }));
    // The portrait bigger and centred on the left page (Wizard101's pet card beside it has no counterpart).
    let back = PET_PORTRAIT;
    if let Some(b) = l.find("spritePortraitBack") {
        v.rect.insert(b, back);
    }
    let k = (back[2] - back[0]) / 159.0;
    let window = [back[0] + 13.0 * k, back[1] + 13.0 * k, back[2] - 13.0 * k, back[3] - 13.0 * k];
    if let Some(p) = l.find("wndPortrait") {
        v.rect.insert(p, window);
        icon(v, d.items, *id, [window[0] + 8.0, window[1] + 8.0, window[2] - 8.0, window[3] - 8.0]);
    }

    // The talents, laid out as Wizard101's PetGeneticsControl.gui (321 x 379, ctrlGenetics at 76,206) lists them:
    // the "Talents n/n" tab (btnTalents: Button_Item's red frame, yellow WizardFancy 0.45 label and count), then
    // one PetTalentControl.gui row per talent (293 x 35 on Background_Pet_Talent, rows 35 apart from y 24): the
    // name centred in yellow WizardFancy 0.4 (txtName 5,9 - 249,26) and the talent's icon on the right (sprtIcon
    // 258,-1 - 293,34), one 64 px cell of GUI/Talents/PetTalent_Complete_Color01 (8 x 8), the cell each talent's
    // TalentData template names. The control sits under the (larger) portrait here instead of at y 206.
    let column = l.find("ctrlGenetics").map(|i| l.nodes[i].rect).unwrap_or([76.0, 206.0, 369.0, 580.0]);
    let (ox, oy) = (column[0], PET_PORTRAIT[3] + 7.0);
    // (talent, icon cell): Wizard101's cells: 0 health, 1 mana, 2 pips, 3 accuracy, 4 damage, 5 resist, 6 pet
    // experience and stats, 7 spell cards (May Cast), 8 pierce, 9 combat damage, 10 combat blades, 12 beguile,
    // 13 pacify (dove), 19 stun, 22 fishing, 23 energy, 25 gardening, 26 archmastery.
    let talents: Vec<(&str, usize)> = if level.is_some() {
        vec![("Fights at your side", 9), ("Upgrade with Glovewort", 6), ("Recalled when you rest", 13),
             ("One summon at a time", 7)]
    } else {
        vec![("Double jump", 19), ("Rides spiritsprings", 26), ("Outdoors only", 25), ("Heals on Rowa Raisins", 0)]
    };
    let tab = [ox, oy - 3.0, ox + 149.0, oy + 28.0];
    v.backs.push((tab, "GUI/Shopping/Button_Item.png".into(), [0.0, 0.0, 1.0, 0.125]));
    let gold = TextStyle::new("WizardFancy", 0.45, YELLOW);
    v.add([ox + 7.0, oy + 2.0, ox + 79.0, oy + 21.0], "Talents", gold.clone());
    v.add([ox + 85.0, oy + 2.0, ox + 140.0, oy + 21.0], format!("{0}/{0}", talents.len()), gold.right());
    let mut y = oy + 24.0;
    for (t, cell) in &talents {
        let row = [ox, y, ox + 293.0, y + 35.0];
        v.backs.push((row, TALENT_ROW.into(), [0.0, 0.0, 1.0, 1.0]));
        v.add([ox + 5.0, y + 9.0, ox + 249.0, y + 26.0], *t, TextStyle::new("WizardFancy", 0.4, YELLOW).centred());
        let (cx, cy) = ((cell % 8) as f32 / 8.0, (cell / 8) as f32 / 8.0);
        v.images.push(([ox + 258.0, y - 1.0, ox + 293.0, y + 34.0], TALENT_ICONS.into(),
                       [cx, cy, cx + 0.125, cy + 0.125]));
        y += 35.0;
    }
    y += 10.0;
    v.add([column[0], y, column[2], y + 24.0], "Lore", heading.clone());
    y += 30.0;
    let tip = d.items.tooltip(*id, 1, d.names, false);
    let about = tip.last().filter(|_| tip.len() > 1).cloned().unwrap_or_default();
    v.add([column[0] + 12.0, y, column[2] - 10.0, column[3]], about, prose.clone().centred());

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

/// The Icon_<School> symbol (GUI/Icons, as on the reagent cards) of a deck tab's school; None for All and Astral
/// (no Icon_Astral in Wizard101's art). Monster Magic (Dragon Communion) uses Fire's, as its card frame does.
fn deck_school_icon(button: &str) -> Option<String> {
    let school = match button {
        "Cards_MonsterMagic" => "Fire",
        b => b.strip_prefix("Cards_").filter(|s| !matches!(*s, "All" | "Astral"))?,
    };
    Some(school_icon(school))
}

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

/// The Wizard101 card type icon (GUI/Icons/Type_*, the symbol in the card's bottom-right box) for what a spell
/// does, from its Elden Ring description: heals (over time: HealOverTime), cures (Manipulation), weapon and shield
/// enchantments (Enchantment), damage negation and barriers (Ward), boosts (Charm), stealth (Cloak), life-sapping
/// (Steal), light (Global), poison, rot and madness (DamageOverTime), area attacks (AOE), the rest attacks (Damage).
fn spell_type(items: &ItemData, raw: u32) -> &'static str {
    let info = items.info(raw).unwrap_or("").to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| info.contains(w));
    let kind = if has(&["heals hp over", "continuously heals"]) {
        "HealOverTime"
    } else if has(&["ailment", "alleviates", "cures", "dispels"]) {
        "Manipulation"
    } else if has(&["heal"]) {
        "Heal"
    } else if has(&["armament", "shield"]) {
        "Enchantment"
    } else if has(&["negation", "defensive", "resistance", "deflect", "forcefield", "retaliate", "covers self",
                    "guardian", "draws in spells", "surrounds the caster"]) {
        "Ward"
    } else if has(&["raises the", "raises physical", "increases", "blessing", "bolster", "charge body"]) {
        "Charm"
    } else if has(&["conceal", "silences", "caster semi-invisible", "lures"]) {
        "Cloak"
    } else if has(&["sapping", "drain"]) {
        "Steal"
    } else if has(&["illuminate"]) {
        "Global"
    } else if has(&["poison", "rot", "madness", "frenzy", "putrescen", "bloodflies", "threads"]) {
        "DamageOverTime"
    } else if has(&["area", "surroundings", "around caster", "downpour", "rain of", "hail of"]) {
        "AOE"
    } else {
        "Damage"
    };
    match kind {
        "HealOverTime" => "GUI/Icons/Type_HealOverTime.png",
        "Manipulation" => "GUI/Icons/Type_Manipulation.png",
        "Heal" => "GUI/Icons/Type_Heal.png",
        "Enchantment" => "GUI/Icons/Type_Enchantment.png",
        "Ward" => "GUI/Icons/Type_Ward.png",
        "Charm" => "GUI/Icons/Type_Charm.png",
        "Cloak" => "GUI/Icons/Type_Cloak.png",
        "Global" => "GUI/Icons/Type_Global.png",
        "Steal" => "GUI/Icons/Type_Steal.png",
        "DamageOverTime" => "GUI/Icons/Type_DamageOverTime.png",
        "AOE" => "GUI/Icons/Type_AOE.png",
        _ => "GUI/Icons/Type_Damage.png",
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
    // One deck (Elden Ring's memory slots): no deck arrows, no renaming, no equip button.
    hide(l, v, &["PrevDeck", "NextDeck", "NewDeckName", "EquipButton", "equipFist"]);
    grey(l, v, &["Help", "QuickSellButton", "NextItemSpells", "PrevItemSpells", "TreasureCardButton", "FusionCardButton"]);
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
    // The book's title stays "Spell Deck" whichever class is shown (the class is the tab's tip).
    let _ = title;
    v.set(l.find("DeckTitle"), centre("Spell Deck"));
    // Wizard101's item-card row under the deck has no Elden Ring counterpart.
    hide(l, v, &["ItemSpells", "NextItemSpells", "PrevItemSpells"]);
    let Some(sheet) = d.sheet else { return };
    let total = memory_slots(sheet);
    let used = slots_used(sheet, d.items);
    v.set(l.find("DeckName"), centre(&format!("Memory Slots {used}/{total}")));
    // The deck (Wizard101's grid of the cards in the deck, slot frames edge to edge, Background_Slot's top half):
    // one cell per memory slot, eight to a row filling the deck's width (more slots wrap to the next row). Each memorised spell takes as many cells as
    // it takes memory slots (its icon in the first, the rest shaded), in Elden Ring's order. A click forgets it.
    if let Some(grid) = l.find("CardsInDeck") {
        let r = l.nodes[grid].rect;
        let columns = DECK_COLUMNS;
        let cell_size = ((r[2] - r[0]) / columns as f32).floor();
        let x0 = r[0] + ((r[2] - r[0]) - cell_size * columns as f32) / 2.0;
        let mut cells: Vec<Option<(usize, bool)>> = Vec::new();
        for (i, spell) in sheet.spells.iter().enumerate().filter(|(_, s)| **s >= 0) {
            let n = d.items.spell(*spell as u32).map_or(1, |x| x.slots.max(1));
            for k in 0..n {
                cells.push(Some((i, k == 0)));
            }
        }
        while cells.len() < total as usize {
            cells.push(None);
        }
        for (c, what) in cells.iter().enumerate().take(16) {
            let x = x0 + (c % columns) as f32 * cell_size;
            let y = r[1] + (c / columns) as f32 * cell_size;
            let cell = [x, y, x + cell_size, y + cell_size];
            v.images.push((cell, DECK_SLOT.into(), [0.0, 0.0, 1.0, 0.5]));
            let inset = cell_size * 0.1;
            let inner = [x + inset, y + inset, x + cell_size - inset, y + cell_size - inset];
            match *what {
                Some((i, first)) => {
                    let id = 0x4000_0000 | sheet.spells[i] as u32;
                    icon(v, d.items, id, inner);
                    if !first {
                        // The rest of a spell that takes several slots.
                        v.over_fills.push((inner, ImColor32::from_rgba(40, 25, 10, 140)));
                    }
                    let mut tip = d.items.tooltip(id, 1, d.names, false);
                    tip.push("Click: forget this spell".into());
                    v.tip(cell, tip);
                    st.hits.push((cell, Hit::Forget(i)));
                }
                None => v.tip(cell, vec![format!("Memory slot {}", c + 1), "Empty".into(),
                                         "Click a spell on the right".into()]),
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
        // The school symbol in the corner circle: the reagent cards' Icon_<School> art (Ryan, round 10: use those
        // symbols everywhere); Astral has none, so its tab button's own symbol.
        let (c0, c1) = (card(101.0, 28.0), card(125.0, 52.0));
        match deck_school_icon(DECK_TABS[tab].0) {
            Some(icon) => {
                let inset = (c1[0] - c0[0]) * 0.12;
                v.images.push(([c0[0] + inset, c0[1] + inset, c1[0] - inset, c1[1] - inset], icon, [0.0, 0.0, 1.0, 1.0]));
            }
            None => {
                if let Some(n) = l.find(DECK_TABS[tab].0).map(|b| &l.nodes[b]) {
                    if let Some(img) = &n.img {
                        v.images.push(([c0[0], c0[1], c1[0], c1[1]], img.clone(), n.frames[0].unwrap_or(n.uv)));
                    }
                }
            }
        }
        // What it does: Wizard101's type icon in the box at the picture's bottom right (98,97 .. 125,124).
        let (y0, y1) = (card(100.0, 99.0), card(123.0, 122.0));
        v.images.push(([y0[0], y0[1], y1[0], y1[1]], spell_type(d.items, raw).into(), [0.0, 0.0, 1.0, 1.0]));
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
        let memorised = sheet.spells.contains(&(raw as i32));
        let mut tip = d.items.tooltip(*id, 1, d.names, false);
        tip.push(if memorised { "Memorised (click it in the deck to forget it)".into() } else { "Click: memorise".into() });
        v.tip([x, y, x + cw, y + ch], tip);
        if memorised {
            // Already in the deck: dimmed, as Wizard101 dims cards it can't add.
            v.fills.push(([x, y, x + cw, y + ch], IN_DECK));
        } else {
            st.hits.push(([x, y, x + cw, y + ch], Hit::Learn(raw as i32)));
        }
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

/// The copied map screen on the page: (map rect, the frame around it) for a screen of `aspect` (width / height).
/// One picture of the whole screen, edge to edge, Elden Ring's own button hints along its bottom included (Ryan,
/// 2026-10-30: no black bands, no separate shrunk hint row). It fills the map area's full width; the map area's
/// height shrinks to the screen's shape (16:9 at 2560x1440: 672 x 378 of the 672 x 452 area), centred, so nothing
/// is cropped or squeezed. A screen taller than the area (narrower than about 3:2) fills its height instead.
pub fn map_picture_rect(aspect: f32) -> ([f32; 4], [f32; 4]) {
    let h = MAP_HOLE;
    let area = [h[0] + 8.0, h[1] + 8.0, h[2] - 8.0, h[3] - 8.0];
    let (room_w, room_h) = (area[2] - area[0], area[3] - area[1]);
    let (w, ht) = if room_w / room_h > aspect { (room_h * aspect, room_h) } else { (room_w, room_w / aspect) };
    let (cx, cy) = ((area[0] + area[2]) / 2.0, (area[1] + area[3]) / 2.0);
    let map = [cx - w / 2.0, cy - ht / 2.0, cx + w / 2.0, cy + ht / 2.0];
    (map, [map[0] - 6.0, map[1] - 6.0, map[2] + 6.0, map[3] + 6.0])
}

fn map(l: &Layout, d: &Data, v: &mut View) {
    v.set(l.find("ZoneName"), centre("The Lands Between"));
    v.show(l.find("MapLoadingIndicator"), d.map_opening);
    v.set(l.path("MapLoadingIndicator/PackageNameText"), centre("Opening Map..."));
    hide(l, v, &["txtGoHomeTimer", "txtGotoDormTimer"]);
    for (button, place, to) in [("GoHomeButton", "Unicorn Way", "Travel to The First Step"),
                                ("GotoDormButton", "Roundtable Hold", "Travel to the Roundtable Hold")] {
        if let Some(b) = l.find(button) {
            v.tip(l.nodes[b].rect, vec![place.into(), to.into()]);
        }
    }
    if d.map_open {
        // The whole map screen, scaled onto the page in its own shape; or, without a copy of the screen, a window
        // of that shape onto the map behind the book.
        match d.map_picture {
            Some(aspect) => {
                // The whole map screen, hints and all, as one picture filling the (shrunk) map area.
                let (r, frame) = map_picture_rect(aspect);
                v.fills.push((frame, ImColor32::from_rgb(74, 46, 24)));
                v.images.push((r, MAP_PICTURE.into(), [0.0, 0.0, 1.0, 1.0]));
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
               If it didn't open, press A, Enter or the magnifier below to try again.",
              body);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Crafting: RecipeWindow.gui. Right: crafting materials (or cookbooks, with the Reagents button); left: the kit.

/// Reagent cards (top left corners): ReagentWindow's Card01-08 (LeftPage 30,115 and RightPage 389,115 plus each
/// card's offset), and their size.
const REAGENT_CARDS: [[f32; 2]; 8] = [[61.0, 117.0], [226.0, 117.0], [61.0, 320.0], [226.0, 320.0],
                                      [391.0, 117.0], [555.0, 117.0], [391.0, 320.0], [555.0, 320.0]];
const REAGENT_CARD: [f32; 2] = [161.0, 201.0];

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
        // The book's description, then every recipe it teaches as a row with the item's picture.
        let (about, recipes) = d.items.cookbook(id & 0x0FFF_FFFF)
            .unwrap_or_else(|| ("This cookbook's pages are blank.".into(), vec![]));
        let page = [80.0, 166.0, 340.0, 524.0];
        let about_h = if recipes.is_empty() { page[3] - page[1] } else { 110.0 };
        v.add([page[0], page[1], page[2], page[1] + about_h], about,
              TextStyle::new("WizardNormal", 0.38, BLACK).centred().wrapped());
        if !recipes.is_empty() {
            let mut y = page[1] + about_h + 4.0;
            v.add([page[0], y, page[2], y + 22.0], format!("Recipes ({})", recipes.len()),
                  TextStyle::new("WizardFancy", 0.48, BLACK).centred());
            y += 26.0;
            let columns = if recipes.len() > 9 { 2 } else { 1 };
            let rows = recipes.len().div_ceil(columns);
            let row_h = ((page[3] - y) / rows as f32).min(26.0);
            let col_w = (page[2] - page[0]) / columns as f32;
            for (i, recipe) in recipes.iter().enumerate() {
                let (cx, cy) = (page[0] + (i / rows) as f32 * col_w, y + (i % rows) as f32 * row_h);
                let goods = d.names.goods_named(recipe).map(|g| 0x4000_0000 | g);
                let recipe = recipe.as_str();
                let side = row_h - 2.0;
                let has = goods.is_some_and(|g| icon(v, d.items, g, [cx + 2.0, cy + 1.0, cx + 2.0 + side, cy + 1.0 + side]));
                let left = if has { cx + side + 6.0 } else { cx + 4.0 };
                let scale = if columns == 2 { 0.32 } else { 0.38 };
                v.add([left, cy, cx + col_w - 2.0, cy + row_h], recipe.to_string(),
                      TextStyle::new("WizardNormal", scale, BLACK));
                if let Some(g) = goods {
                    v.tip([cx, cy, cx + col_w, cy + row_h], d.items.tooltip(g, 1, d.names, false));
                }
            }
        }
    }
    if !st.cookbooks {
        // ReagentWindow's category tabs (REAGENT_TABS): all, harvested, rare harvested, bought, dropped. Elden
        // Ring's own sort groups say what a material is: creature parts (10, 15) are dropped, plants and stones
        // (20-50) harvested, rare ones (rarity above 0) rare harvested; bought is what a shop sells.
        st.subs = REAGENT_TABS.len() - 1;
        let tab = st.sub % st.subs;
        list.retain(|(_, _, id)| {
            let raw = id & 0x0FFF_FFFF;
            let source = d.items.source(raw);
            let creature = source.is_some_and(|(sg, _)| sg == 10 || sg == 15);
            match tab {
                1 => source.is_some() && !creature && d.items.rank(raw) == 1,
                2 => source.is_some() && !creature && d.items.rank(raw) > 1,
                3 => source.is_some_and(|(_, shop)| shop),
                4 => creature,
                _ => true,
            }
        });
        reagents(l, d, st, v, &list, &rows);
        return;
    }
    let page = paging(l, v, st, list.len(), rows.len(), ("PageNumber", "leftscroll", "rightscroll"));
    for (i, row) in rows.iter().enumerate() {
        match list.get(page * rows.len() + i) {
            Some((name, _, id)) => {
                let r = l.nodes[*row].rect;
                v.tip(r, d.items.tooltip(*id, 1, d.names, false));
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

/// Wizard101's reagents page (ReagentWindow.gui) on RecipeWindow's pages: its title, its category tabs on
/// Art_CraftingTab (all reagents, harvested, rare harvested, bought, dropped, class project; only "all" applies),
/// the reagents as Background_Reagent cards, four on each page at ReagentWindow's places (each with its picture
/// on a dark band, its name on the paper below and the count in the sun badge), the page buttons under the left
/// page, and the toggle to the cookbooks with Wizard101's book.
fn reagents(l: &Layout, d: &Data, st: &mut State, v: &mut View, list: &[(String, u32, u32)], rows: &[usize]) {
    v.set(l.find("title"), centre("Materials"));
    hide(l, v, &["portrait_background", "card_background", "IngredientBackground", "TabLayoutEquipment",
                 "TabBackground", "CreateButton", "EquipButton"]);
    // (The filters button stays, greyed as in the crafting view.)
    // The kit's name banner and the station's doodles (ReagentWindow keeps only the one down the right edge).
    v.show(l.find("labelItemHighlighted").and_then(|t| l.nodes[t].parent), false);
    for (i, n) in l.nodes.iter().enumerate() {
        if n.name == "Doodle" && n.rect[0] < 700.0 {
            v.show(Some(i), false);
        }
    }
    for r in rows {
        v.show(Some(*r), false);
    }
    let chosen = st.sub % REAGENT_TABS.len();
    for (i, (button, tip)) in REAGENT_TABS.iter().enumerate() {
        let x = 71.0 + 49.0 * i as f32;
        v.backs.push(([x - 7.0, 44.0, x + 49.0, 106.0], "GUI/Art/Art_CraftingTab.png".into(), [0.0, 0.0, 1.0, 1.0]));
        // Frames: normal top left, highlighted top right (the chosen tab). Button_Class_Project is one picture,
        // not four; Elden Ring has no class projects, so it stays faded.
        let r = [x, 52.0, x + 40.0, 92.0];
        if i == REAGENT_TABS.len() - 1 {
            v.images.push((r, format!("GUI/Buttons/{button}.png"), [0.0, 0.0, 1.0, 1.0]));
            v.over_fills.push((r, ImColor32::from_rgba(150, 140, 120, 130)));
            v.tip(r, vec![tip.to_string(), "None in Elden Ring".into()]);
            continue;
        }
        let uv = if i == chosen { [0.5, 0.0, 1.0, 0.5] } else { [0.0, 0.0, 0.5, 0.5] };
        v.images.push((r, format!("GUI/Buttons/{button}.png"), uv));
        v.tip(r, vec![tip.to_string()]);
        st.hits.push((r, Hit::Sub(i)));
    }
    v.tab_row = Some([71.0, 52.0, 71.0 + 49.0 * (REAGENT_TABS.len() - 1) as f32 + 40.0, 92.0]);
    // The page buttons where ReagentWindow has them (PageButtons, under the left page).
    for (name, r) in [("PageNumber", [134.0, 535.0, 293.0, 570.0]), ("leftscroll", [81.0, 534.0, 119.0, 572.0]),
                      ("rightscroll", [309.0, 534.0, 347.0, 572.0])] {
        if let Some(n) = l.find(name) {
            v.rect.insert(n, r);
        }
    }
    if let Some(t) = l.find("Reagent_Toggle") {
        v.node_art.insert(t, "GUI/Buttons/Button_Crafting_Recipes.png".into());
    }
    let page = paging(l, v, st, list.len(), REAGENT_CARDS.len(), ("PageNumber", "leftscroll", "rightscroll"));
    for (slot, &[x, y]) in REAGENT_CARDS.iter().enumerate() {
        let Some((name, count, id)) = list.get(page * REAGENT_CARDS.len() + slot) else { continue };
        let (w, h) = (REAGENT_CARD[0], REAGENT_CARD[1]);
        let card = |fx: f32, fy: f32| [x + fx / 161.0 * w, y + fy / 201.0 * h];
        v.backs.push(([x, y, x + w, y + h], "GUI/Backgrounds/Background_Reagent.png".into(), [0.0, 0.0, 1.0, 1.0]));
        // The reagent card (ReagentWindow's Reagent button, 19,16 .. 118,169) is Wizard101's School_Resource_Blnk
        // (its top 128 x 195): name band, rank triangle, school circle, the medallion's round window and the
        // paper box. `at` maps that picture's pixels onto the page.
        let (a, b) = (card(19.0, 16.0), card(118.0, 169.0));
        let at = |px: f32, py: f32| [a[0] + px / 128.0 * (b[0] - a[0]), a[1] + py / 195.0 * (b[1] - a[1])];
        let rect = |x0: f32, y0: f32, x1: f32, y1: f32| { let (p, q) = (at(x0, y0), at(x1, y1)); [p[0], p[1], q[0], q[1]] };
        // The medallion behind the card's round window, the item's picture on it.
        v.images.push((rect(64.0 - 48.0, 77.0 - 48.0, 64.0 + 48.0, 77.0 + 48.0), REAGENT_MEDALLION.into(), [0.0, 0.0, 1.0, 1.0]));
        icon(v, d.items, *id, rect(64.0 - 31.0, 77.0 - 31.0, 64.0 + 31.0, 77.0 + 31.0));
        v.images.push(([a[0], a[1], b[0], b[1]], REAGENT_FRAME.into(), [0.0, 0.0, 1.0, 195.0 / 256.0]));
        // Name on the green band (white), rank in the yellow triangle, school in the circle, name on the paper.
        v.add(rect(6.0, 5.0, 122.0, 24.0), name.clone(), TextStyle::new("WizardFancy", 0.3, WHITE).centred().outlined());
        let rank = d.items.rank(id & 0x0FFF_FFFF);
        // The rank: a plain yellow number in the card's own triangle, as Wizard101 prints it.
        v.add(rect(5.0, 26.0, 30.0, 44.0), rank.to_string(),
              TextStyle::new("WizardFancy", 0.4, YELLOW).centred().outlined());
        // The school: Wizard101's school icon (GUI/Icons/Icon_<School>) inside the card's circle (centre 111.8, 39,
        // 21 across inside its rim), kept square on the page.
        let school = reagent_school(name);
        let (c, e) = (at(111.8, 39.0), at(111.8 + 10.5, 39.0 + 10.5));
        let half = (e[0] - c[0]).min(e[1] - c[1]);
        v.images.push(([c[0] - half, c[1] - half, c[0] + half, c[1] + half], school_icon(school), [0.0, 0.0, 1.0, 1.0]));
        // The name on the paper in Wizard101's card text font (SpellDescription), one size on every card: the
        // largest at which the longest material name fits the paper (two lines).
        let mut paper = TextStyle::new("SpellDescription", REAGENT_TEXT_SCALE, BLACK).centred().wrapped();
        paper.middle = false;
        v.add(rect(12.0, 132.0, 116.0, 186.0), name.clone(), paper);
        // The count on the sun (CardNumber, 128,171 .. 153,190).
        let (n0, n1) = (card(126.0, 171.0), card(155.0, 191.0));
        v.add([n0[0], n0[1], n1[0], n1[1]], count.to_string(), TextStyle::new("WizardFancy", 0.42, BLACK).centred());
        let mut tip = d.items.tooltip(*id, *count, d.names, false);
        tip.insert(1.min(tip.len()), format!("Rank {rank} {school} material"));
        v.tip([x, y, x + w, y + h], tip);
    }
    if list.is_empty() {
        let text = if chosen == 0 { "No materials yet" } else { "None of these yet" };
        v.add([61.0, 200.0, 387.0, 240.0], text, TextStyle::new("WizardFancy", 0.55, BLACK).centred());
    }
}

/// ReagentWindow's category tabs: (button art, what it is in Wizard101).
const REAGENT_TABS: [(&str, &str); 6] = [
    ("Button_Crafting_Reagents", "All Materials"), ("Button_Crafting_Harvest", "Harvested Materials"),
    ("Button_Crafting_RareHarvest", "Rare Harvested Materials"), ("Button_Gold_Bag", "Bought Materials"),
    ("Button_Crafting_WorldDrop", "Dropped Materials"), ("Button_Class_Project", "Class Project Materials"),
];
/// Wizard101's reagent card and its blank medallion (the round picture behind the card's window).
const REAGENT_FRAME: &str = "GUI/Cards/School_Resource_Blnk.png";
const REAGENT_MEDALLION: &str = "GUI/Reagents/Resource_Nothing.png";

/// A Wizard101 school's icon (GUI/Icons/Icon_<School>, 32 x 32, the art the reagent cards show in their circle).
fn school_icon(school: &str) -> String {
    format!("GUI/Icons/Icon_{school}.png")
}
/// SpellDescription's scale for the reagent cards' names: the largest at which every Elden Ring material name
/// (the longest, Tarnished Golden Sunflower) wraps into the paper box (104 x 54 card pixels, 80 x 42 on the page)
/// (worked out from SpellDescription.xml's glyph widths).
const REAGENT_TEXT_SCALE: f32 = 0.73;

// ---------------------------------------------------------------------------------------------------------------
// Help: HelpSpellbookPage.gui. Its topic buttons pick the help text (Wizard101 loads its help pages from other
// layouts; here the text is drawn on the book's pages in Wizard101's fonts).

/// (topic button, left page (heading, text), right page (heading, text)): each topic laid out as a spread, one
/// heading and its text on each page, as Wizard101's help book does.
const HELP: [(&str, (&str, &str), (&str, &str)); 4] = [
    ("Tab_Questing",
     ("The Spellbook",
      "Open and close the book with B on the keyboard or the View button on the controller (Elden Ring's map \
       button).\n\nEsc or the controller's B button also close it."),
     ("Tabs and Categories",
      "The tabs down the right edge are the book's pages, as in Wizard101.\n\nThe round buttons along the top of a \
       page are its categories.\n\nPoint at an item for its stats.")),
    ("Tab_Combat",
     ("Controller",
      "LT / RT or D-pad up and down: previous / next page of the book.\n\nLB / RB: previous / next category along \
       the top of the page.\n\nD-pad left and right: turn the page of a list."),
     ("More Buttons",
      "Right stick: turn your character in the Backpack (it turns by itself; a click on it stops it).\n\nA: use \
       the tab (the World Map tab opens Elden Ring's map).\n\nB or View: close the book.")),
    ("Tab_Interface",
     ("Keyboard",
      "Up / Down arrows: previous / next page of the book.\n\nQ / E: previous / next category.\n\nLeft / Right \
       arrows: turn the page of a list.\n\nEnter: use the tab.\n\nB or Esc: close the book."),
     ("Mouse",
      "Click a tab or a button; point at an item for its stats.\n\nThe mouse wheel turns the page of a list.\n\n\
       Drag your character in the Backpack to turn it.\n\nClick an item to choose it; double-click it to equip it, \
       or to take it off when it is equipped.")),
    ("Tab_Character",
     ("The Pages",
      "Character: level, health, mana (FP), energy (stamina), flasks, attributes and badges.\n\nBackpack: \
       everything you carry. The fist equips the chosen item, the open hand takes it off.\n\nPets: Torrent and your \
       spirit ashes (the fist puts one in your quick items)."),
     ("More Pages",
      "Deck: your memory slots, and your spells by school. Click a spell to memorize it, a slot to forget it, \
       anywhere.\n\nQuests: the Unicorn Way quests you have taken on.\n\nWorld Map: Elden Ring's map.\n\n\
       Crafting: materials by kind, and cookbooks with their recipes.\n\nSettings: the HUD's settings.")),
];

/// The help spread's two pages (page pixels, inside the book's margins and clear of the spine).
const HELP_PAGES: [[f32; 4]; 2] = [[84.0, 112.0, 360.0, 528.0], [412.0, 112.0, 688.0, 528.0]];

fn help(l: &Layout, st: &mut State, v: &mut View) {
    st.subs = HELP.len();
    let (button, left, right) = HELP[st.sub % HELP.len()];
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
    for ((heading, text), r) in [left, right].into_iter().zip(HELP_PAGES) {
        v.add([r[0], r[1], r[2], r[1] + 36.0], heading, TextStyle::new("WizardFancy", 0.7, BLACK).centred());
        v.fills.push(([r[0] + 40.0, r[1] + 42.0, r[2] - 40.0, r[1] + 43.5], ImColor32::from_rgba(90, 60, 30, 110)));
        let mut body = TextStyle::new("WizardNormal", 0.46, BLACK).wrapped();
        body.middle = false;
        v.add([r[0], r[1] + 54.0, r[2], r[3]], text, body);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Settings: Settings.gui, Wizard101's Settings window: its ten tab buttons each show a page of up to seven option
// rows (options::TABS: Elden Ring's options and graphics settings, and the HUD's own). Rows 1-5 are Settings.gui's
// Graphics rows; rows 6 and 7 (Wizard101's other pages have seven, at the same 48-pixel pitch) are drawn in their
// art. Quit returns to Elden Ring's title screen, Idle (relabelled Exit Game) quits to the desktop.

pub const SETTINGS_ROWS: [&str; 5] =
    ["OptionResolution", "OptionFullscreen", "OptionUISize", "OptionTextureDetail", "OptionHardwareCursor"];
/// Rows 6 and 7: their tops (page pixels), below OptionHardwareCursor (315) at Wizard101's 48-pixel row pitch.
const EXTRA_ROWS: [f32; 2] = [363.0, 411.0];
pub const SETTINGS_PAGE_ROWS: usize = 7;
/// OptionValue's text: Settings.gui has WizardFancy 0.6 centred in the 22-pixel red box, but WizardFancy's capitals
/// sit low in their cells, so at 0.6 they ran over the box's bottom rim (Ryan, round 15: text must stay inside its
/// box). A little smaller and lifted, they sit inside the red.
const VALUE_SCALE: f32 = 0.54;
const VALUE_LIFT: f32 = 2.5;

/// One settings row as the page shows it: label, value, and the lines of its tip.
pub type SettingRow = (String, String, Vec<String>);

fn settings(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    let tab = st.page % crate::options::TABS.len();
    let (_, title, rows) = crate::options::TABS[tab];
    st.subs = rows.len().max(1);
    for t in ["AudioOptionsText", "RealmsText", "GameplayOptionsText", "PrivacyOptionsText", "AdvVideoOptionsText"] {
        v.show(l.find(t), false);
    }
    v.set(l.find("VideoOptionsText"), centre(title));
    v.show(l.find("VideoOptions"), true);
    for (i, (button, title, _)) in crate::options::TABS.iter().enumerate() {
        if let Some(b) = l.find(button) {
            if i == tab {
                v.checked.insert(b);
            }
            // Wizard101's plain button tooltip (ToolTip.gui), as its Settings tab buttons have (m_sTip).
            v.plain_tip(l.nodes[b].rect, vec![(*title).into()]);
        }
    }
    v.set(l.find("QuitButton"), centre("Quit"));
    v.set(l.find("IdleButton"), centre("Exit Game"));
    if let Some(b) = l.find("QuitButton") {
        v.plain_tip(l.nodes[b].rect, vec!["Back to Elden Ring's title screen".into(), "(the game saves first)".into()]);
    }
    if let Some(b) = l.find("IdleButton") {
        v.plain_tip(l.nodes[b].rect, vec!["The game saves, then Elden Ring closes".into()]);
    }
    if let Some(b) = l.find("OkButton") {
        v.plain_tip(l.nodes[b].rect, vec!["Apply the changes and close".into()]);
    }
    // With a controller the row the d-pad changes shows in OptionText's own highlight colour (m_HighColor white);
    // there is no marker in front of the label (Ryan, round 15).
    let selected = crate::input::pad_connected().then_some(st.sub % st.subs);
    let label_style = TextStyle::new("WizardFancy", 0.6857, BLACK);
    let value_style = TextStyle::new("WizardFancy", VALUE_SCALE, ImColor32::from_rgb(0xCC, 0xCC, 0x00)).centred();
    for i in 0..SETTINGS_PAGE_ROWS {
        let shown = i < rows.len();
        let (label, value, tip) = d.settings.get(i).cloned().unwrap_or_default();
        // The row's top: Settings.gui's own rows, or rows 6 and 7 in their art (OptionHardwareCursor's, moved down).
        let top = match SETTINGS_ROWS.get(i) {
            Some(name) => {
                let Some(r) = l.find(name) else { continue };
                v.show(Some(r), shown);
                // The label and value are drawn below instead (one line, fitted), so the layout's are left empty.
                v.set(l.find_under(r, "OptionText"), "");
                v.set(l.find_under(r, "OptionValue"), "");
                l.nodes[r].rect[1]
            }
            None => EXTRA_ROWS[i - SETTINGS_ROWS.len()],
        };
        if !shown {
            continue;
        }
        if i >= SETTINGS_ROWS.len() {
            let left = [436.0, top + 6.0, 478.0, top + 48.0];
            let right = [638.0, top + 6.0, 680.0, top + 48.0];
            v.images.push(([462.0, top + 16.0, 654.0, top + 38.0], "GUI/Buttons/Button_Red01.png".into(),
                           [0.0, 0.0, 1.0, 0.25]));
            v.images.push((left, "GUI/Buttons/Button_Arrow01.png".into(), [0.0, 0.0, 0.25, 0.5]));
            v.images.push((right, "GUI/Buttons/Button_Arrow01.png".into(), [0.0, 0.5, 0.25, 1.0]));
            st.hits.push((left, Hit::Setting(i, -1)));
            st.hits.push((right, Hit::Setting(i, 1)));
        }
        // OptionText (136..386 at +12..+42) may run on to just before the left arrow (436); OptionValue's text stays
        // inside the red box (462..654 at +16..+38) clear of the arrows' rims. Both are one line that shrinks to fit.
        let mut label_style = label_style.clone();
        if selected == Some(i) {
            label_style.colour = ImColor32::WHITE;
        }
        v.add([136.0, top + 12.0, 430.0, top + 42.0], label, label_style);
        v.add([474.0, top + 16.0 - VALUE_LIFT, 642.0, top + 38.0 - VALUE_LIFT], value, value_style.clone());
        v.plain_tip([100.0, top, 680.0, top + 54.0], tip);
    }
}
