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
    /// The crafting page (crafting.rs).
    pub craft: crate::crafting::Craft,
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
    /// The Backpack shows Wizard101's Equipment Sets page (EquipmentManagerWindow) instead of the items.
    pub gear_view: bool,
    /// The chosen equipment set (row) there.
    pub gear_chosen: usize,
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
    /// A teleport marker on the map page (the grace): the warp flag to set, as Action::Warp.
    Warp(u32),
    /// An equipment set's row (index): choose it; a double click equips it.
    GearSet(usize),
    /// Round 20 (Q7): a quest card on the Quests page (index into Data::quests): track it.
    Quest(usize),
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
    /// The Equipment Sets page: save what is worn now as a new set, equip set n, delete set n, move set n up (-1)
    /// or down (+1).
    GearSave,
    GearEquip(usize),
    GearDelete(usize),
    GearMove(usize, i32),
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
    /// The player's place (Game::position: chunk position, block, block origin), for the map page.
    pub position: Option<([f32; 3], i32, i32)>,
    /// Per ACHIEVEMENTS entry: earned (its event flag), or None when not tracked / not readable.
    pub achieved: &'a [Option<bool>],
    /// The character's saved equipment sets (the Backpack's Equipment Sets page).
    pub gear_sets: &'a [GearSet],
}

/// Elden Ring's achievements in Steam's own order (API names ACH00-ACH41) and with Steam's own display names (the log's
/// "steam: 42 achievements" line, 2026-11-01), with the event flag that awards it (common.emevd event 9300 is run per
/// achievement with its flag: the bosses), 0 where the game awards it some other way (endings, collections, the
/// upgrade) that the book doesn't follow. Steam's list is matched by API name (index), then by display name.
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
    ("Hoarah Loux, Warrior", "Defeated Hoarah Loux, Warrior", 11050800),
    ("Dragonlord Placidusax", "Defeated Dragonlord Placidusax", 13000830),
    ("God-Slaying Armament", "Upgraded any armament to the maximum degree", 0),
    ("Legendary Armaments", "Acquired all legendary armaments", 0),
    ("Legendary Ashen Remains", "Acquired all legendary ashen remains", 0),
    ("Legendary Sorceries and Incantations", "Acquired all legendary sorceries and incantations", 0),
    ("Legendary Talismans", "Acquired all legendary talismans", 0),
    ("Rennala, Queen of the Full Moon", "Defeated Rennala, Queen of the Full Moon", 14000800),
    ("Lichdragon Fortissax", "Defeated Lichdragon Fortissax", 12030850),
    ("Godskin Duo", "Defeated the Godskin Duo", 13000850),
    ("Fire Giant", "Defeated the Fire Giant", 1052520800),
    ("Dragonkin Soldier of Nokstella", "Defeated the Dragonkin Soldier of Nokstella", 12010800),
    ("Regal Ancestor Spirit", "Defeated the Regal Ancestor Spirit", 12090800),
    ("Valiant Gargoyles", "Defeated the Valiant Gargoyles", 12020800),
    ("Margit, the Fell Omen", "Defeated Margit, the Fell Omen", 10000850),
    ("Red Wolf of Radagon", "Defeated the Red Wolf of Radagon", 14000850),
    ("Godskin Noble", "Defeated the Godskin Noble", 16000850),
    ("Magma Wyrm Makar", "Defeated Magma Wyrm Makar", 39200800),
    ("Godfrey, First Elden Lord", "Defeated Godfrey, First Elden Lord", 11000850),
    ("Mohg, the Omen", "Defeated Mohg, the Omen", 35000800),
    ("Mimic Tear", "Defeated the Mimic Tear", 12020850),
    ("Loretta, Knight of the Haligtree", "Defeated Loretta, Knight of the Haligtree", 15000850),
    ("Astel, Naturalborn of the Void", "Defeated Astel, Naturalborn of the Void", 12040800),
    ("Leonine Misbegotten", "Defeated the Leonine Misbegotten", 1043300800),
    ("Royal Knight Loretta", "Defeated Royal Knight Loretta", 1035500800),
    ("Elemer of the Briar", "Defeated Elemer of the Briar", 1039540800),
    ("Ancestor Spirit", "Defeated the Ancestor Spirit", 12080800),
    ("Commander Niall", "Defeated Commander Niall", 1051570800),
    ("Roundtable Hold", "Arrived at the Roundtable Hold", 105),
    ("Great Rune", "Restored the power of a Great Rune", 0),
    ("Erdtree Aflame", "Used the Rune of Death to set the Erdtree aflame", 0),
];

/// The Wizard101 badge picture for achievement `index`: the endings and "Elden Ring" the blue-ribboned medal, the
/// Shardbearers the red-ribboned one, the legendary collections the starred one, the places the winged one, the
/// other bosses the plain medals in turn.
fn badge_art(index: usize) -> &'static str {
    const ART: [&str; 6] = ["GUI/Art/Art_Badge01.png", "GUI/Art/Art_Badge02.png", "GUI/Art/Art_Badge03.png",
                            "GUI/Art/Art_Badge04.png", "GUI/Art/Art_Badge05.png", "GUI/Art/Art_Badge06.png"];
    let name = ACHIEVEMENTS[index].0;
    match index {
        0..=3 | 40 | 41 => ART[5],
        _ if name.starts_with("Shardbearer") => ART[3],
        _ if name.starts_with("Legendary") || index == 13 => ART[2],
        39 => ART[4],
        _ => ART[index % 2],
    }
}

impl State {
    pub fn reset(&mut self) {
        self.page = 0;
        self.sub = 0;
        self.pet = 0; // the Pets page opens on the equipped spirit ash
        self.filter_node = None;
        self.gear_view = false;
    }

    pub fn step_sub(&mut self, delta: i32) {
        let n = self.subs.max(1) as i32;
        self.sub = ((self.sub as i32 + delta).rem_euclid(n)) as usize;
        self.page = 0;
        self.filter_node = None;
    }

    /// How many subs (category tabs) the current tab has, for LB / RB.
    pub(crate) fn set_subs(&mut self, subs: usize) {
        self.subs = subs;
    }

    pub fn step_page(&mut self, delta: i32) {
        self.page = (self.page as i32 + delta).max(0) as usize;
    }

    pub fn step_pet(&mut self, delta: i32) {
        let n = self.pets.max(1) as i32;
        self.pet = (self.pet as i32 + delta).rem_euclid(n) as usize;
    }
}

pub(crate) fn centre(text: &str) -> String {
    format!("<center>{text}</center>")
}

pub(crate) fn hide(l: &Layout, v: &mut View, names: &[&str]) {
    for n in names {
        v.show(l.find(n), false);
    }
}

pub(crate) fn grey(l: &Layout, v: &mut View, names: &[&str]) {
    for n in names {
        if let Some(id) = l.find(n) {
            v.greyed.insert(id);
        }
    }
}

/// Moves a tab marker (Tab_Top03) under a button, keeping its offset from the first button.
pub(crate) fn marker(l: &Layout, v: &mut View, marker: &str, first: &str, chosen: Option<usize>) {
    let (Some(m), Some(f), Some(c)) = (l.find(marker), l.find(first), chosen) else { return };
    let (mr, fr, cr) = (l.nodes[m].rect, l.nodes[f].rect, l.nodes[c].rect);
    v.rect.insert(m, [cr[0] + mr[0] - fr[0], cr[1] + mr[1] - fr[1], cr[0] + mr[2] - fr[0], cr[1] + mr[3] - fr[1]]);
}

/// "p/n" in a page number box, greying the arrows at the ends; returns the clamped page.
pub(crate) fn paging(l: &Layout, v: &mut View, st: &mut State, count: usize, per_page: usize, ids: (&str, &str, &str)) -> usize {
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
pub(crate) fn row_of(l: &Layout, names: &[&str]) -> Option<[f32; 4]> {
    names.iter().filter_map(|n| l.find(n)).map(|i| l.nodes[i].rect).reduce(|a, b| {
        [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
    })
}

/// An item's Elden Ring icon in `r`, if there is one.
pub(crate) fn icon(v: &mut View, items: &ItemData, item_id: u32, r: [f32; 4]) -> bool {
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
        Tab::Backpack if st.gear_view => gear_sets(l, d, st, v),
        Tab::Backpack => backpack(l, d, st, v),
        Tab::Pets => pets(l, d, st, v),
        Tab::Deck => deck(l, d, st, v),
        Tab::Quests => quests(l, d, st, v),
        Tab::Map => map(l, d, v, st),
        Tab::Crafting => crate::crafting::bind(l, d, st, v),
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
        // The Equipment Sets page (EquipmentManagerWindow) and the Backpack's button to it.
        (Tab::Backpack, "EquipmentManager") => {
            st.gear_view = true;
            st.page = 0;
        }
        (Tab::Backpack, "Backpack_Toggle" | "Backpack_Toggle2") if st.gear_view => st.gear_view = false,
        (Tab::Backpack, "Create") if st.gear_view => return Action::GearSave,
        (Tab::Backpack, "Equip") if st.gear_view => return Action::GearEquip(st.gear_chosen),
        (Tab::Backpack, "Delete") if st.gear_view => return Action::GearDelete(st.gear_chosen),
        (Tab::Backpack, "MoveUpButton") if st.gear_view => return Action::GearMove(st.gear_chosen, -1),
        (Tab::Backpack, "MoveDownButton") if st.gear_view => return Action::GearMove(st.gear_chosen, 1),
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
        (Tab::Crafting, _) if crate::crafting::click(l, id, st) => {}
        (Tab::Help, _) if name.starts_with("Tab_") => {
            if let Some(i) = HELP.iter().position(|(n, _)| *n == name) {
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

/// Wizard101's rank titles (Root.wad Locale/en-US/Levels.lang, in its order: Novice at level 1 .. Eternal), on a
/// ladder for Elden Ring's levels 1-713 (the book's own spread, Ryan round 16): ten levels a rank up to Archmage
/// at 110 (roughly where a first playthrough ends), then wider steps for the levels of later playthroughs.
const RANKS: [(u32, &str); 19] = [
    (1, "Novice"), (10, "Apprentice"), (20, "Initiate"), (30, "Journeyman"), (40, "Adept"), (50, "Magus"),
    (60, "Master"), (70, "Grandmaster"), (80, "Legendary"), (95, "Transcendent"), (110, "Archmage"),
    (125, "Promethean"), (150, "Exalted"), (175, "Prodigious"), (200, "Champion"), (250, "Paragon"),
    (300, "Prime"), (400, "Supreme"), (550, "Eternal"),
];

/// The rank title for an Elden Ring level.
pub fn rank_title(level: u32) -> &'static str {
    RANKS.iter().rev().find(|(from, _)| level >= *from).map_or(RANKS[0].1, |r| r.1)
}

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
    // Wizard101's GUI.lang StatsPageLevel "#1:$RANK$ (Level #2:$LEVEL$)" in CharLevel, the rank from
    // Elden Ring's level (rank_title).
    v.set(l.find("CharLevel"), format!("{} (Level {})", rank_title(sheet.level), sheet.level));
    v.set(l.find("CharTitle"), "Tarnished");
    // The school badge on the portrait's ring: the character's school as the reagent cards' Icon_<School> symbol
    // on their medallion. Elden Ring has no school, so it follows the casting stat: Intelligence = Myth (the
    // deck's Glintstone), Faith = Life (Golden Order), Arcane = Death; none of them past 20 = Balance.
    // Drawn as Wizard101 draws it: the node's own Art_School_<School> badge (the symbol on the school's colour,
    // filling the gold ring), no hover text (Ryan, round 16).
    if let Some(b) = l.find("SchoolIcon") {
        let [int, fai, arc] = [sheet.attributes[5], sheet.attributes[6], sheet.attributes[7]];
        let school = if int.max(fai).max(arc) <= 20 { "Balance" }
            else if int >= fai && int >= arc { "Myth" } else if fai >= arc { "Life" } else { "Death" };
        v.node_art.insert(b, school_art(school));
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
    v.plain_tip([FLASK_ROWS[0][0], FLASK_ROWS[0][1], FLASK_ROWS[1][2], FLASK_ROWS[1][3]],
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
                v.plain_tip([r[0], top, r[2], r[3]], vec![format!("{}: {}", about[i], values[i])]);
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
                // The label under the box (this section has one row); round 22 B3: below the box's bottom line,
                // not on it.
                v.add([r[0], r[3] + 1.0, r[2], r[3] + 17.0], labels[i].to_string(),
                      TextStyle::new("WizardNormal", 0.3, BLACK).centred());
                v.plain_tip([r[0], r[1] - 26.0, r[2], r[3] + 17.0], vec![format!("{}: {}", about[i], values[i])]);
            }
            let ratio = if st3.max_load > 0.0 { st3.load / st3.max_load } else { 0.0 };
            let roll = match ratio {
                x if x <= 0.3 => "Light load",
                x if x <= 0.7 => "Medium load",
                x if x <= 1.0 => "Heavy load",
                _ => "Overloaded",
            };
            v.plain_tip(l.nodes[acc].rect, vec!["Equip load".into(), format!("{:.1} / {:.1}", st3.load, st3.max_load),
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
            // Earned or not shows only as the row being greyed out (Ryan, round 15: no status line).
            if state != Some(true) {
                v.greyed.insert(*row);
            }
            let fade = |c: ImColor32| if state == Some(true) { c } else { ImColor32::from_rgba(c.r, c.g, c.b, 140) };
            // The row's own button art (Button_Item) lies on the background's bar, the badge's medal
            // (Art_Badge01-06) exactly over the background's badge outline, the name inside the frame's gold
            // line, centred (shrunk to fit its width).
            let bar = [BADGE_BAR[0], r[1] + BADGE_BAR[1], BADGE_BAR[2], r[1] + BADGE_BAR[3]];
            v.rect.insert(*row, bar);
            let medal = [r[0] + BADGE_MEDAL[0], r[1] + BADGE_MEDAL[1], r[0] + BADGE_MEDAL[2], r[1] + BADGE_MEDAL[3]];
            // Not earned: the medal faded, as Wizard101 shows badges still to earn (tinted, so it keeps its shape).
            let tint = if state == Some(true) { ImColor32::WHITE } else { ImColor32::from_rgba(200, 185, 160, 120) };
            v.tinted.push((medal, badge_art(a).into(), [0.0, 0.0, 1.0, 1.0], tint));
            let (bw, bh) = (bar[2] - bar[0], bar[3] - bar[1]);
            let inner = [bar[0] + BADGE_TEXT[0] * bw, bar[1] + BADGE_TEXT[1] * bh,
                         bar[0] + BADGE_TEXT[2] * bw, bar[1] + BADGE_TEXT[3] * bh];
            let pad = (inner[3] - inner[1]) * 0.12;
            v.add([inner[0] + 2.0, inner[1] + pad, inner[2] - 2.0, inner[3] - pad], name.to_string(),
                  TextStyle::new("WizardFancy", 0.0, fade(YELLOW)).centred());
            // Hovering a badge: Wizard101's plain tooltip (ToolTip.gui) with the badge's description, centred, as
            // its badge list shows the WizardBadges.lang text under each title.
            let mut row_rect = r;
            row_rect[0] = medal[0].min(r[0]);
            row_rect[2] = bar[2].max(r[2]);
            v.bubbles.push((row_rect, about.to_string()));
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
/// A saved equipment set: its name and what it puts in each of GEAR_SLOTS (ChrAsm slot, param ID as
/// PlayerGameData's equipment_param_ids has it).
#[derive(Clone, Debug, PartialEq)]
pub struct GearSet {
    pub name: String,
    pub slots: Vec<(u32, i32)>,
}

/// The ChrAsm slots an equipment set keeps: weapons (left 0 2 4, right 1 3 5), arrows and bolts (6-9), armour
/// (12-15), talismans (17-20).
pub const GEAR_SLOTS: [u32; 18] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 12, 13, 14, 15, 17, 18, 19, 20];
/// Wizard101's Equipment Sets page (the Backpack's EquipmentManager button), its rows and Wizard101's set names
/// (EquipmentSetNames.lang "$PART0$ $PART1$ Set $PART2$": a school, a purpose from its second wheel, a number).
pub const GEAR_PAGE: &str = "EquipmentManagerWindow";
pub const GEAR_ROWS: usize = 9;
const GEAR_PURPOSES: [&str; 9] = ["Main", "Questing", "Boss", "Dungeon", "PvP", "Speed", "Attack", "Shield", "Healer"];

/// What the character's equipment is in a set's terms.
pub fn gear_now(sheet: &Sheet) -> Vec<(u32, i32)> {
    GEAR_SLOTS.iter().map(|s| (*s, sheet.equipment[*s as usize])).collect()
}

/// A new set's name (Wizard101's naming), the n-th set of a character whose school is `school`.
pub fn gear_name(school: &str, n: usize) -> String {
    format!("{school} {} Set {}", GEAR_PURPOSES[n % GEAR_PURPOSES.len()], n % 9 + 1)
}

/// The character's Wizard101 school (the Character page's badge): from the highest casting stat, Balance when none
/// is past 20.
pub fn char_school(sheet: &Sheet) -> &'static str {
    let [int, fai, arc] = [sheet.attributes[5], sheet.attributes[6], sheet.attributes[7]];
    if int.max(fai).max(arc) <= 20 { "Balance" }
    else if int >= fai && int >= arc { "Myth" } else if fai >= arc { "Life" } else { "Death" }
}

/// An empty slot's param ID: unarmed (110000) in a hand, the bare head / body / arms / legs (10000-10300), -1 or 0
/// for arrows and talismans.
pub fn gear_empty(slot: u32, id: i32) -> bool {
    match slot {
        0..=5 => id == 110000 || id < 0,
        12..=15 => id < 0 || (10000..=10300).contains(&id) && id % 100 == 0,
        _ => id <= 0,
    }
}

/// The inventory item ID (with its category bits) of a set's param ID in `slot`.
pub fn gear_item(slot: u32, id: i32) -> u32 {
    let category = match slot {
        12..=15 => 0x1000_0000,
        17..=20 => 0x2000_0000,
        _ => 0,
    };
    category | id as u32
}

/// Wizard101's Equipment Sets page: the character picture on the left as on the Backpack, the saved sets as
/// item rows on the right (the closed hand beside the one being worn), and along the bottom its Create (+),
/// Equip (fist), Delete (-), Move Down and Move Up buttons; the Backpack button under the picture goes back.
fn gear_sets(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = 1;
    if let Some(c) = l.find("Tab_EquipSet") {
        v.checked.insert(c);
        v.plain_tip(l.nodes[c].rect, vec!["Equipment Sets".into()]);
    }
    v.set(l.find("title"), centre("Equipment Sets"));
    v.set(l.find("Backpack_Toggle2"), centre("Backpack"));
    for (b, tip) in [("Create", "Save what you wear as a new set"), ("Equip", "Equip"), ("Delete", "Delete"),
                     ("MoveUpButton", "Move Up"), ("MoveDownButton", "Move Down"), ("Backpack_Toggle", "Backpack"),
                     ("Backpack_Toggle2", "Backpack")] {
        if let Some(n) = l.find(b) {
            v.plain_tip(l.nodes[n].rect, vec![tip.into()]);
        }
    }
    if let Some(p) = l.find("3DCharacter") {
        v.node_images.insert(p, MODEL_FULL.into());
    }
    let sets = d.gear_sets;
    st.gear_chosen = st.gear_chosen.min(sets.len().saturating_sub(1));
    let now = d.sheet.map(gear_now);
    for i in 0..GEAR_ROWS {
        let (Some(row), icon) = (l.find(&format!("Item_{i}")), l.find(&format!("equippedIcon{i}"))) else { continue };
        let Some(set) = sets.get(i) else {
            v.show(Some(row), false);
            v.show(icon, false);
            continue;
        };
        let r = l.nodes[row].rect;
        v.add([r[0] + 26.0, r[1], r[2] - 14.0, r[3]], set.name.clone(),
              TextStyle::new("WizardFancy", 0.42, LIGHT_YELLOW).centred());
        let worn = now.as_ref().is_some_and(|n| *n == set.slots);
        v.show(icon, worn);
        if i == st.gear_chosen {
            v.checked.insert(row);
        }
        // What the set holds, in the plain tooltip.
        let names: Vec<String> = set.slots.iter().filter(|(slot, id)| !gear_empty(*slot, *id))
            .map(|(slot, id)| d.names.item(gear_item(*slot, *id))).collect();
        v.plain_tip(r, vec![set.name.clone(), names.join("\n")]);
        st.hits.push((r, Hit::GearSet(i)));
    }
    if sets.is_empty() {
        v.add([415.0, 200.0, 692.0, 300.0], "No equipment sets yet.\nPut on what you want, then click +.",
              TextStyle::new("WizardNormal", 0.45, BLACK).centred().wrapped());
    }
    if sets.is_empty() {
        grey(l, v, &["Equip", "Delete", "MoveUpButton", "MoveDownButton"]);
    }
    if st.gear_chosen == 0 {
        grey(l, v, &["MoveUpButton"]);
    }
    if st.gear_chosen + 1 >= sets.len() {
        grey(l, v, &["MoveDownButton"]);
    }
    if sets.len() >= GEAR_ROWS || d.sheet.is_none() {
        grey(l, v, &["Create"]);
    }
}

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
    // Round 20 B1: the greyed Monstrology (top left) and Housing (top right) arrows beside the tabs are gone.
    hide(l, v, &["Tab_HousingStuff", "Tab_MonsterMagic", "Tab_Photography"]);
    // Only the equip button and the equipment sets button stay along the bottom.
    hide(l, v, &["Lock_Item", "Socket_Item", "btnFeedPet", "RecyclingBinButton", "QuickSell_Item", "Trash_Item"]);
    // Wizard101's hover texts (the buttons' m_sTip, in the plain tooltip): each tab's category, the equipment
    // sets button.
    for (b, _, title) in BACKPACK {
        if let Some(n) = l.find(b) {
            v.plain_tip(l.nodes[n].rect, vec![if b == "Tab_All" { "All Items" } else { title }.to_string()]);
        }
    }
    if let Some(n) = l.find("EquipmentManager") {
        v.plain_tip(l.nodes[n].rect, vec!["Equipment Sets".into()]);
    }
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
                // Round 21 (Ryan B3): only the name on the row; the count is in the hover tooltip ("Owned: n").
                v.add([left, r[1], r[2] - 14.0, r[3]], name.clone(), TextStyle::new("WizardFancy", 0.42, LIGHT_YELLOW).centred());
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
            v.plain_tip(l.nodes[b].rect, vec![if *equipped { "Unequip" } else { "Equip" }.into()]);
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
/// PetTalentControl.gui's sprtFrame art.
const TALENT_FRAME: &str = "GUI/Art/Art_Text03.png";
/// Wizard101's pet ages (GUI.lang PetAge1-7), over the page's aging bar (wndAgingBkg: lblBaby .. lblUltra).
const PET_AGES: [(&str, &str); 7] = [("lblBaby", "Baby"), ("lblTeen", "Teen"), ("lblAdult", "Adult"),
                                     ("lblAncient", "Ancient"), ("lblEpic", "Epic"), ("lblMega", "Mega"),
                                     ("lblUltra", "Ultra")];

/// A spirit ash's upgrade level (+0 .. +10) as a Wizard101 pet age (index into PET_AGES): the eleven levels spread
/// over the seven ages, +10 alone being Ultra (+0-1 Baby, +2-3 Teen, +4 Adult, +5-6 Ancient, +7 Epic, +8-9 Mega).
pub fn pet_age(level: u32) -> usize {
    (level.min(10) as usize * 7 / 11).min(6)
}

fn pets(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = 1;
    // Only the equip button stays along the bottom (Ryan, round 16: no greyed Pedigree, Snack, Jewel, Tome or
    // Badges buttons).
    hide(l, v, &["SnackButton", "EquipJewelButton", "ToggleTalentDisplay", "PetTomeButton", "PetBadgesButton"]);
    hide(l, v, &["wndRatingBkg", "wndAgingBkg", "eggHatchTimer", "_NOT_USED_SnackToggleWin", "ctrlGenetics", "cardBkg",
                 "spellCard", "spellCardSmall1", "spellCardSmall2", "spellCardSmall3"]);
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
        return;
    }
    st.pets = pets.len();
    let i = st.pet % pets.len();
    st.pet = i;
    let (name, kind, level, id) = &pets[i];
    // The equip button (Wizard101's, which makes a pet the one that follows you): a spirit ash goes into the
    // quick items, where Elden Ring uses it from; the open hand when it is there already (a click takes it out),
    // the fist when not. Torrent is always at hand.
    let equipped = level.is_some() && d.sheet.is_some_and(|sheet| sheet.quick_items.contains(id));
    match (d.sheet, level) {
        (Some(_), Some(_)) => {
            st.equip_target = Some(*id);
            if equipped {
                v.node_art.insert(l.find("EquipButton").unwrap_or(usize::MAX), UNEQUIP_ART.into());
            }
        }
        _ => grey(l, v, &["EquipButton"]),
    }
    if let (Some(b), Some(_)) = (l.find("EquipButton"), level) {
        v.plain_tip(l.nodes[b].rect, vec![if equipped { "Unequip" } else { "Equip" }.into()]);
    }
    for (b, text) in [("btnPrevPet", "Previous Pet"), ("btnNextPet", "Next Pet")] {
        if let (Some(n), true) = (l.find(b), pets.len() > 1) {
            v.plain_tip(l.nodes[n].rect, vec![text.into()]);
        }
    }
    // The closed hand right of the name (wndEquippedIcon) only on the spirit ash that is equipped.
    v.show(l.find("wndEquippedIcon"), equipped);
    // The pet's school (spriteSchoolIcon, Art_School_<School>, left of "Spirit Ash (n of m)").
    let school = if level.is_some() { ash_school(name) } else { "Life" };
    if let Some(i) = l.find("spriteSchoolIcon") {
        v.show(Some(i), true);
        v.node_art.insert(i, school_art(school));
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
    // No Pet Derby here (Wizard101 puts the Derby tab beside it), so the Talents tab runs the column's full width
    // (the talent rows' 293).
    // PetGeneticsControl's btnTalents at its own size (0,-3 - 149,28), centred over the rows (Ryan, round 16),
    // with lblTalents (7,2 - 79,21) and txtTalents (85,2 - 140,21, right): yellow WizardFancy 0.45, drop shadow.
    let tx = ox + (293.0 - 149.0) / 2.0;
    let tab = [tx, oy - 3.0, tx + 149.0, oy + 28.0];
    v.backs.push((tab, "GUI/Shopping/Button_Item.png".into(), [0.0, 0.0, 1.0, 0.125]));
    let gold = TextStyle::new("WizardFancy", 0.45, YELLOW).shadowed();
    v.add([tx + 7.0, oy + 2.0, tx + 79.0, oy + 21.0], "Talents", gold.clone());
    v.add([tx + 85.0, oy + 2.0, tx + 140.0, oy + 21.0], format!("{0}/{0}", talents.len()), gold.right());
    let mut y = oy + 24.0;
    for (t, cell) in &talents {
        // PetTalentControl.gui, in its own drawing order: the window's Background_Pet_Talent, then sprtFrame
        // (0,0 - 295,35: Art_Text03, the dark red panel with its thin gold line and ragged red edge, which the
        // client shows on a manifested talent), the name on top in the control's yellow WizardFancy 0.4
        // (m_Color FF00FFFF), then the icon.
        let row = [ox, y, ox + 293.0, y + 35.0];
        v.backs.push((row, TALENT_ROW.into(), [0.0, 0.0, 1.0, 1.0]));
        v.images.push(([ox, y, ox + 295.0, y + 35.0], TALENT_FRAME.into(), [0.0, 0.0, 1.0, 1.0]));
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

    // Wizard101's pet age bar over the experience (hat) bar: the age's name (lblCurrentLevel, WizardFancy 0.4),
    // the seven-cell bar (Art_Bar_Exp07 over prgrAging's fill) filled to the age, and each cell's age name on
    // hover (lblBaby .. lblUltra). A spirit ash's age is its upgrade level (pet_age); Torrent has none.
    match level {
        Some(level) => {
            let age = pet_age(*level);
            v.show(l.find("wndAgingBkg"), true);
            v.set(l.find("lblCurrentLevel"), centre(PET_AGES[age].1));
            v.progress.extend(l.find("prgrAging").map(|p| (p, (age + 1) as f32 / PET_AGES.len() as f32)));
            for (node, name) in PET_AGES {
                if let Some(n) = l.find(node) {
                    v.bubbles.push((l.nodes[n].rect, name.to_string()));
                }
            }
        }
        None => v.show(l.find("wndAgingBkg"), false),
    }

    // The stat block: label left, value right, a rule under each, as PetStatBlockControl draws its stats.
    if let Some(block) = l.find("ctrlStatBlock") {
        let r = l.nodes[block].rect;
        v.add([r[0], r[1] + 12.0, r[2], r[1] + 36.0], "Stats", heading.clone());
        let owned = pets.iter().filter(|p| p.2.is_some()).count();
        let rows: Vec<(&str, String)> = match level {
            // (The upgrade level is the hat bar's alone, Ryan round 15; the age is the age bar's.)
            // (No age: the age bar above shows it, Ryan round 16.)
            Some(_) => vec![
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

/// The count at the deck's bottom right: a normal spell card's symbol (Button_Item_Cards' normal frame: the spell
/// card in the gold ring, Wizard101's own "Spells" symbol) where Wizard101 shows its treasure card one.
const SPELL_COUNT_ICON: &str = "GUI/Buttons/Button_Item_Cards.png";

/// A spell card as Wizard101 builds one (the School_<school>_Blnk frame, 128 x CARD_H of its 256 art): the name
/// across the top band, the cost in the pip triangle, the school symbol in the corner circle, the picture in the
/// window with the type icon in its box, and on the parchment below what it does, in Wizard101's card text font
/// (SpellDescription, black, centred): Elden Ring's effect line for the spell, then its memory slots and the
/// attributes it needs. `r` is the card's rectangle (any size; everything scales with it).
fn spell_card(l: &Layout, d: &Data, v: &mut View, id: u32, name: &str, r: [f32; 4]) {
    let [x, y, x1, y1] = r;
    let (cw, ch) = (x1 - x, y1 - y);
    let k = cw / 97.0;
    let raw = id & 0x0FFF_FFFF;
    let spell = d.items.spell(raw);
    // The class's Wizard101 school: its card frame, and its tab button's symbol in the frame's corner.
    let class = school(d.items, id);
    let tab = DECK_TABS.iter().position(|t| t.1 == class).unwrap_or(1);
    let frame = card_frame(DECK_TABS[tab].0);
    let card = |fx: f32, fy: f32| [x + fx / 128.0 * cw, y + fy / CARD_H * ch];
    // The picture: the spell's icon in the frame's window, on Wizard101's parchment spiral (the reagent cards'
    // backing, Ryan round 18 C5; it was a dark fill).
    let (w0, w1) = (card(10.0, 33.0), card(119.0, 117.0));
    crate::crafting::parchment_spiral(v, [w0[0], w0[1], w1[0], w1[1]]);
    let side = (w1[1] - w0[1]).min(w1[0] - w0[0]);
    let ix = (w0[0] + w1[0] - side) / 2.0;
    icon(v, d.items, id, [ix, w0[1], ix + side, w0[1] + side]);
    v.images.push(([x, y, x1, y1], frame.into(), [0.0, 0.0, 1.0, CARD_H / 256.0]));
    // The school symbol in the corner circle: the reagent cards' Icon_<School> art (Ryan, round 10: use those
    // symbols everywhere); Astral has none, so its tab button's own symbol.
    // Round 21 (Ryan B2): centred on the circle's silver ring (x 100..124, y 26..51 in the frame art; measured in
    // the 2560x1440 render too: ring centre 112.1, 38.8); the box was 101..125 x 28..52, about 1 px right and low.
    let [sx, sy] = CARD_SCHOOL_CENTRE;
    let (c0, c1) = (card(sx - 12.0, sy - 12.0), card(sx + 12.0, sy + 12.0));
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
    // Round 19 (Ryan D7): smaller, inside the box's frame as Wizard101's type symbol sits (the Sunbird and
    // Lightning Bats help pictures: the fist covers about 70% of the grey box), not filling it.
    let kind = spell_type(d.items, raw);
    // Round 20 (Ryan B4): a bit bigger again, and centred on the box's brown inside (101..123 x 102..122 in the
    // frame art, above its lit bottom edge at 123); it sat about 1.5 px high at (111.5, 110.5).
    let half = 23.0 * CARD_TYPE_FILL / 2.0;
    let [tx, ty] = CARD_TYPE_CENTRE;
    let (y0, y1) = (card(tx - half, ty - half), card(tx + half, ty + half));
    v.images.push(([y0[0], y0[1], y1[0], y1[1]], kind.into(), [0.0, 0.0, 1.0, 1.0]));
    // The accuracy in the bottom-left circle (Ryan D6): Wizard101 prints the percentage over the frame's
    // bullseye, which shows dimmed under it (Background_Tutorial_Card's Sunbird "100%",
    // Background_Help_Combat_Card_Description's Lightning Bats "70%"): yellow, black-edged digits across the
    // circle. Elden Ring's spells always land, so 100%.
    let (a0, a1) = (card(2.0, 97.0), card(28.0, 123.0));
    v.over_discs.push(([a0[0], a0[1], a1[0], a1[1]], ImColor32::from_rgba(20, 16, 40, 105)));
    v.add([a0[0] - 2.0 * k, a0[1], a1[0] + 2.0 * k, a1[1]], "100%",
          TextStyle::new("WizardFancy", CARD_ACCURACY_SCALE * k, PIP_YELLOW).centred().outlined());
    // The name inside the title band (Ryan round 19 D4-D5): white with a black edge like Wizard101's card names,
    // one size on every card (CARD_NAME_SCALE), one row; names too long for it use a chosen short name
    // (CARD_SHORT_NAMES), never "...".
    let name = card_name(name);
    let (t0, t1) = (card(CARD_NAME[0], CARD_NAME[1]), card(CARD_NAME[2], CARD_NAME[3]));
    v.add([t0[0], t0[1], t1[0], t1[1]], name, TextStyle::new("WizardFancy", CARD_NAME_SCALE * k, WHITE).centred().outlined());
    if let Some(spell) = spell {
        // The cost in the pip triangle: Wizard101's yellow number with a black outline (its reagent card's rank,
        // w101_reference/reagent_card_balance_azoth.png: (239,233,111) inside a black edge, the digit about 14 of
        // the card's 130 px), a little smaller than the name.
        let (p0, p1) = (card(3.0, 29.0), card(30.0, 45.0));
        v.add([p0[0], p0[1], p1[0], p1[1]], spell.fp.to_string(),
              TextStyle::new("WizardFancy", 0.3 * k, PIP_YELLOW).centred().outlined());
        let mut needs: Vec<String> = ["Int", "Fai", "Arc"].iter().zip(spell.requirements).filter(|(_, v)| *v > 0)
            .map(|(n, v)| format!("{n} {v}")).collect();
        // The memory slots as the item tooltip shows them (Type_PowerPips and the count): with the words "2 slots"
        // the longest line ("2 slots Fai 28 Arc 17") would not fit at the description's size (Ryan round 19 D1).
        needs.insert(0, format!("<image;{SLOT_ICON}>{}", spell.slots.max(1)));
        // What it does, as Wizard101's cards say it (Spell.lang, e.g. Fire Elf "#1:$eA$ + #2:$eA4$
        // #3:$dT_image$#4:$Damage_image$ over 3 #5:$Rounds_image$"): the numbers from the cast's params
        // (tools/er_book_data.py), then the school symbol and the type symbol inline, at the text's size, one
        // paragraph that wraps; only the memory slots and attribute needs get their own line.
        let school_pic = deck_school_icon(DECK_TABS[tab].0).unwrap_or_else(|| school_art("Astral"));
        let effect = if spell.card.is_empty() {
            // Nothing countable in the params (utility spells): the type symbol and Elden Ring's own line.
            let info = d.items.info(raw).unwrap_or("").trim().to_string();
            format!("<image;{kind}> {info}")
        } else {
            let mut text = spell.card.clone();
            if kind == "GUI/Icons/Type_AOE.png" {
                // Area spells: Wizard101's "to all enemies" symbol (Icons_All_Enemies; the words no longer fit
                // the two lines, Ryan round 18 C2).
                text = text.replacen("$dT$$Damage$", "$dT$$Damage$$All_Enemies$", 1);
            }
            card_symbols(&text, &school_pic)
        };
        // Wizard101's three card text lines in the parchment box (Ryan round 18 C1-C4): what it does on the first
        // two, from the box's first line down (a one-line text sits on the first line, not in the middle), the
        // slots and needs on the third; at Wizard101's card text size, clear of the box's edges. The texts are
        // written to fit two lines (tools/er_book_data.py SHORT_CARDS); a longer one shrinks to fit.
        // Round 21 (Ryan B1): the three rows evenly spaced, the first at the box's top and the needs line's
        // bottom at CARD_NEEDS_BOTTOM (round 20 B3): the pitch is half the room between them, for the description's
        // two lines too (its leading), so no gap under line 2 is bigger than the one under line 1.
        let line = font_line(CARD_TEXT_SCALE * k) / ch * CARD_H;
        let pitch = (CARD_NEEDS_BOTTOM - line - CARD_TEXT_TOP) / 2.0;
        let (b0, b1) = (card(CARD_TEXT_X[0], CARD_TEXT_TOP), card(CARD_TEXT_X[1], CARD_TEXT_TOP + 2.0 * pitch));
        let mut style = TextStyle::new("SpellDescription", CARD_TEXT_SCALE * k, BLACK).centred().wrapped();
        style.middle = false;
        style.picture = CARD_SYMBOL_SIZE;
        style.leading = pitch / line;
        v.add([b0[0], b0[1], b1[0], b1[1]], effect, style);
        // Round 20 (Ryan B3): the needs line sits at the bottom of the parchment box (its inside ends at 188 in
        // the frame art), no longer right under the two description lines.
        let top = CARD_TEXT_TOP + 2.0 * pitch;
        let (n0, n1) = (card(CARD_NEEDS_X[0], top), card(CARD_NEEDS_X[1], top + line));
        let mut needs_style = TextStyle::new("SpellDescription", CARD_TEXT_SCALE * k, BLACK).centred();
        needs_style.picture = CARD_SYMBOL_SIZE;
        v.add([n0[0], n0[1], n1[0], n1[1]], needs.join(" "), needs_style);
    }
}

/// The hovered list card: its size (times the card's) and Wizard101's yellow card highlight around it
/// (_Shared-WorldData GUI/Art/Art_Card_Selection: a 4 px pale yellow frame in 128 x 128), `CARD_HIGHLIGHT_EDGE`
/// page pixels outside the card.
const CARD_HOVER_SCALE: f32 = 1.08;
const CARD_HIGHLIGHT: &str = "GUI/Art/Art_Card_Selection.png";
const CARD_HIGHLIGHT_EDGE: f32 = 2.0;

/// The name's box in the card art (128 x CARD_H): the title band (y 2..24) less 6 px each side; Wizard101's own
/// names run nearly edge to edge (Lightning Bats: 90% of the card's width).
const CARD_NAME: [f32; 4] = [6.0, 4.0, 122.0, 25.0];
/// The name's WizardFancy scale on a 97 px card (Ryan round 19 D5: one size). Measured on Wizard101's cards in
/// Background_Tutorial_Card (SUNBIRD: 49% of the card's width = 0.28) and Background_Help_Combat_Card_Description
/// (LIGHTNING BATS 0.28, JUDGEMENT 0.30).
const CARD_NAME_SCALE: f32 = 0.27;
/// The accuracy's WizardFancy scale on a 97 px card ("100%" across the circle, as on the Sunbird picture).
const CARD_ACCURACY_SCALE: f32 = 0.2;
/// How much of the type box (23 px) the type symbol covers (Ryan round 19 D7: 0.72, too small; round 20 B4: 0.86)
/// and its centre in the frame art.
const CARD_TYPE_FILL: f32 = 0.86;
const CARD_TYPE_CENTRE: [f32; 2] = [112.0, 112.5];
/// The centre of the school symbol's corner circle in the frame art (Ryan round 21 B2).
const CARD_SCHOOL_CENTRE: [f32; 2] = [112.25, 39.0];
/// Short card names for the Elden Ring spells whose names do not fit one row at CARD_NAME_SCALE (wider than 320
/// WizardFancy pixels, the box's 87.9 page px / 0.27): chosen by hand, every one checked against the font's glyph
/// widths (Ryan round 19 D4: one row, never "...").
const CARD_SHORT_NAMES: &[(&str, &str)] = &[
    ("Great Glintstone Shard", "Great Shard"),
    ("Swift Glintstone Shard", "Swift Shard"),
    ("Glintstone Cometshard", "Cometshard"),
    ("Founding Rain of Stars", "Rain of Stars"),
    ("Glintblade Phalanx", "Glintblades"),
    ("Greatblade Phalanx", "Greatblades"),
    ("Rennala's Full Moon", "Full Moon"),
    ("Ranni's Dark Moon", "Dark Moon"),
    ("Loretta's Greatbow", "Greatbow"),
    ("Loretta's Mastery", "Bow Mastery"),
    ("Glintstone Icecrag", "Icecrag"),
    ("Carian Greatsword", "Greatsword"),
    ("Adula's Moonblade", "Moonblade"),
    ("Scholar's Armament", "Scholar's Blade"),
    ("Shattering Crystal", "Shatter Crystal"),
    ("Carian Retaliation", "Retaliation"),
    ("Meteorite of Astel", "Astel Meteor"),
    ("Briars of Punishment", "Punishment"),
    ("Ancient Death Rancor", "Death Rancor"),
    ("Explosive Ghostflame", "Ghostflame"),
    ("Great Oracular Bubble", "Great Bubble"),
    ("Flame, Fall Upon Them", "Flamefall"),
    ("Flame, Grant Me Strength", "Flame Strength"),
    ("Flame, Protect Me", "Flame Ward"),
    ("Giantsflame Take Thee", "Giantsflame"),
    ("Flame of the Fell God", "Fell God Flame"),
    ("Scouring Black Flame", "Scouring Flame"),
    ("Black Flame Ritual", "Flame Ritual"),
    ("Black Flame Blade", "Blackflame Blade"),
    ("Black Flame's Protection", "Black Ward"),
    ("Bloodflame Talons", "Blood Talons"),
    ("Protection of the Erdtree", "Erdtree Ward"),
    ("Blessing of the Erdtree", "Erdtree Blessing"),
    ("Flame Fortification", "Fire Guard"),
    ("Magic Fortification", "Magic Guard"),
    ("Lightning Fortification", "Storm Guard"),
    ("Divine Fortification", "Divine Guard"),
    ("Lord's Divine Fortification", "Lord's Guard"),
    ("Night Maiden's Mist", "Maiden's Mist"),
    ("Assassin's Approach", "Assassin's Step"),
    ("Triple Rings of Light", "Triple Rings"),
    ("Radagon's Rings of Light", "Radagon's Rings"),
    ("Law of Regression", "Regression"),
    ("Litany of Proper Death", "Proper Death"),
    ("Gurranq's Beast Claw", "Gurranq's Claw"),
    ("Bestial Constitution", "Bestial Cure"),
    ("Ancient Dragons' Lightning Strike", "Ancient Strike"),
    ("Frozen Lightning Spear", "Frozen Spear"),
    ("Ancient Dragons' Lightning Spear", "Ancient Spear"),
    ("Fortissax's Lightning Spear", "Fortissax Spear"),
    ("Lansseax's Glaive", "Lansseax Glaive"),
    ("Electrify Armament", "Electrify"),
    ("Vyke's Dragonbolt", "Vyke's Bolt"),
    ("Dragonbolt Blessing", "Dragonbolt"),
    ("Theodorix's Magma", "Theodorix"),
    ("Glintstone Breath", "Glint Breath"),
    ("Smarag's Glintstone Breath", "Smarag's Breath"),
    ("Placidusax's Ruin", "Placidusax"),
    ("Inescapable Frenzy", "Frenzy Grip"),
    ("The Flame of Frenzy", "Frenzy Flame"),
    ("Unendurable Frenzy", "Wild Frenzy"),
    ("Aspects of the Crucible: Tail", "Crucible Tail"),
    ("Aspects of the Crucible: Horns", "Crucible Horns"),
    ("Aspects of the Crucible: Breath", "Crucible Breath"),
    ("Golden Lightning Fortification", "Golden Guard"),
    ("Miriam's Vanishing", "Vanishing"),
    ("Rellana's Twin Moons", "Twin Moons"),
    ("Gravitational Missile", "Gravity Missile"),
    ("Impenetrable Thorns", "Iron Thorns"),
    ("Rings of Spectral Light", "Spectral Rings"),
    ("Vortex of Putrescence", "Putrid Vortex"),
    ("Mass of Putrescence", "Putrid Mass"),
    ("Furious Blade of Ansbach", "Ansbach's Blade"),
    ("Aspects of the Crucible: Thorns", "Crucible Thorns"),
    ("Aspects of the Crucible: Bloom", "Crucible Bloom"),
    ("Wrath from Afar", "Distant Wrath"),
    ("Multilayered Ring of Light", "Layered Rings"),
    ("Knight's Lightning Spear", "Knight's Spear"),
    ("Dragonbolt of Florissax", "Florissax Bolt"),
    ("Bayle's Flame Lightning", "Bayle's Fury"),
    ("Ghostflame Breath", "Ghost Breath"),
    ("Rotten Butterflies", "Rot Butterflies"),
    ("Pest-Thread Spears", "Thread Spears"),
    ("Midra's Flame of Frenzy", "Midra's Frenzy"),
    ("Fleeting Microcosm", "Microcosm"),
    ("Cherishing Fingers", "Fingers"),
    ("Divine Beast Tornado", "Beast Tornado"),
    ("Divine Bird Feathers", "Bird Feathers"),
];

/// The name a card shows: its short name if it has one.
fn card_name(name: &str) -> &str {
    CARD_SHORT_NAMES.iter().find(|(long, _)| *long == name).map_or(name, |(_, short)| short)
}
/// The parchment box (School_*_Blnk: x 8..120, y 129..188 of the art) and the text in it: the first line's top
/// (its letters start about 5 px under the box's top edge), the description's and the needs line's sides.
const CARD_TEXT_TOP: f32 = 128.0;
const CARD_TEXT_X: [f32; 2] = [12.0, 116.0];
const CARD_NEEDS_X: [f32; 2] = [7.0, 121.0];
/// Where the needs line's cell ends (frame art pixels; Ryan round 20 B3).
const CARD_NEEDS_BOTTOM: f32 = 189.0;
/// The memory slots symbol on the needs line (the item tooltip's "Memory Slots" icon).
const SLOT_ICON: &str = "GUI/Icons/Type_PowerPips.png";

/// SpellDescription's line height (its 21 px glyph cells) at `scale`, in page pixels.
fn font_line(scale: f32) -> f32 {
    21.0 * scale
}

/// The card symbols' side in SpellDescription line heights: about the digits' height, as Wizard101's card
/// symbols sit in its card text.
const CARD_SYMBOL_SIZE: f32 = 0.62;

/// The pip cost's yellow (sampled from Wizard101's own card, w101_reference/reagent_card_balance_azoth.png).
const PIP_YELLOW: ImColor32 = ImColor32::from_rgb(239, 233, 111);

/// Card text symbols (Spell.lang's $..._image$, MadlibIcons.xml's art) as inline pictures: $dT$ is the card's
/// school.
fn card_symbols(text: &str, school: &str) -> String {
    const SYMBOLS: [(&str, &str); 9] = [
        ("$Damage$", "GUI/Icons/Type_Damage.png"), ("$OT_Damage$", "GUI/Icons/Type_DamageOverTime.png"),
        ("$Heal$", "GUI/Icons/Type_Heal.png"), ("$OT_Heal$", "GUI/Icons/Type_HealOverTime.png"),
        ("$Ward$", "GUI/Icons/Type_Ward.png"), ("$Charm$", "GUI/Icons/Type_Charm.png"),
        ("$Rounds$", "GUI/Icons/Icons_Rounds.png"), ("$All_Enemies$", "GUI/Icons/Icons_All_Enemies.png"),
        ("$Self$", "GUI/Icons/Icon_Self.png"),
    ];
    let mut out = text.replace("$dT$", &format!("<image;{school}>"));
    for (token, art) in SYMBOLS {
        out = out.replace(token, &format!("<image;{art}>"));
    }
    // $Type_<kind>$: the card type icon GUI/Icons/Type_<kind> (the cards without numbers lead with their type).
    while let Some(at) = out.find("$Type_") {
        let Some(len) = out[at + 1..].find('$') else { break };
        let kind = out[at + 6..at + 1 + len].to_string();
        out.replace_range(at..at + 2 + len, &format!("<image;GUI/Icons/Type_{kind}.png>"));
    }
    out
}

/// SpellDescription's scale on a list-sized (97 px) card. Round 18 (C4), measured on Wizard101's own card pictures
/// (_Shared-WorldData GUI/Backgrounds/Background_Help_Interface_Deck_Building, its deck page's list card:
/// "105-145" is 58% of the card's width = 56 SpellDescription pixels on 97 px, scale 1.0; Background_Tutorial_Card's
/// Sunbird: digits 11% of the card's width, 1.06; the help card description 0.9 to 1.4): 0.9, the largest at
/// which three lines (two of text, the slots and needs) fit the parchment box. Round 17's 0.64 came from a
/// reagent card's name, which Wizard101 draws smaller. Round 19 (Ryan D1, from playing Wizard101): that was too
/// big for the two description lines and the needs line (0.6) too small; all three lines now share one size,
/// 0.72, which keeps the longest needs line ("2 Fai 28 Arc 17" with the slot symbol) on one row. Round 20 (Ryan B2):
/// a little bigger, 0.78, still one size and the longest needs line still one row.
const CARD_TEXT_SCALE: f32 = 0.78;

fn deck(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.subs = DECK_TABS.len();
    hide(l, v, &["FusionBackground", "TieredSpellMPUnlockedBackground", "TieredSpellMPUnlockedList", "SpellFusionList",
                 "SchoolPipPanel", "CloseTSMPUnlockedPageButton", "SpellTreasureList", "SchoolPageSpellList",
                 "TutorialLogBackground1", "EquipBorder"]);
    // One deck (Elden Ring's memory slots): no deck arrows, no renaming, no equip button. No help button, quick
    // sell, treasure cards or spell fusion either (Ryan, round 15: not even greyed).
    hide(l, v, &["PrevDeck", "NextDeck", "NewDeckName", "EquipButton", "equipFist", "Help", "QuickSellButton",
                 "TreasureCardButton", "FusionCardButton"]);
    // The item-card strip's page arrows are gone too (Ryan round 18 C6; Wizard101's own deck page shows none while
    // the strip has one page).
    hide(l, v, &["NextItemSpells", "PrevItemSpells"]);
    // The spell count's symbol: a normal spell card, not Wizard101's treasure card one.
    if let Some(c) = l.find("TreasureCardIcon") {
        v.show(Some(c), false);
        v.images.push((l.nodes[c].rect, SPELL_COUNT_ICON.into(), [0.0, 0.0, 0.5, 0.5]));
    }
    // The class buttons, in order (Shadow and Monster Magic are hidden in Wizard101's layout).
    let mut row = None;
    for (i, (button, class, title)) in DECK_TABS.iter().enumerate() {
        let Some(b) = l.find(button) else { continue };
        let r = l.nodes[b].rect;
        let x = DECK_TAB_X[i];
        let nr = [x, r[1], x + (r[2] - r[0]), r[3]];
        v.rect.insert(b, nr);
        v.show(Some(b), true);
        v.plain_tip(nr, vec![if class.is_empty() { "All Spells".to_string() } else { title.to_string() }]);
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
    // Wizard101's item-card row under the deck (ItemSpells, its Background_Deck_Equipment strip of empty
    // placeholder squares) stays, empty as Wizard101 shows it when no gear gives a spell (Ryan, round 17); its page
    // arrows greyed.
    let Some(sheet) = d.sheet else { return };
    let total = memory_slots(sheet);
    // Ryan, round 17: the title is just "Memory Slots" (the free slots show as empty cells).
    v.set(l.find("DeckName"), centre("Memory Slots"));
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
                    // Hovering it shows the spell's card (as Wizard101's deck shows a card when you point at
                    // it), the size of the list's cards (Ryan, round 16), above the slot and the cursor, as
                    // Wizard101's card tips sit (Ryan round 18 C8; it was under the slot).
                    let [cw, ch] = l.find("AllPageSpellList").and_then(|n| l.nodes[n].card).unwrap_or([97.0, 148.0]);
                    // Round 19 (Ryan D2): directly on top of the slot, no gap, its left edge on the slot's.
                    let (gx, gy) = (cell[0], cell[1] - ch);
                    let mut card = View::default();
                    spell_card(l, d, &mut card, id, &d.names.item(id), [gx, gy, gx + cw, gy + ch]);
                    v.hovers.push((cell, Box::new(card)));
                    st.hits.push((cell, Hit::Forget(i)));
                }
                None => v.plain_tip(cell, vec![format!("Memory slot {}: empty", c + 1),
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
        // Everything is on the card itself: no tooltip over it (Ryan, round 16).
        spell_card(l, d, v, *id, name, [x, y, x + cw, y + ch]);
        let memorised = sheet.spells.contains(&(raw as i32));
        // Under the cursor the card grows a little inside Wizard101's yellow card highlight (Art_Card_Selection),
        // as Wizard101 lights the card you point at (Ryan round 18 C7).
        let grow = cw * (CARD_HOVER_SCALE - 1.0) / 2.0;
        let big = [x - grow, y - grow * ch / cw, x + cw + grow, y + ch + grow * ch / cw];
        let mut lit = View::default();
        spell_card(l, d, &mut lit, *id, name, big);
        if memorised {
            lit.fills.push((big, IN_DECK));
        }
        let edge = CARD_HIGHLIGHT_EDGE;
        lit.images.push(([big[0] - edge, big[1] - edge, big[2] + edge, big[3] + edge], CARD_HIGHLIGHT.into(),
                         [0.0, 0.0, 1.0, 1.0]));
        v.hovers.push(([x, y, x + cw, y + ch], Box::new(lit)));
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
    // Round 20 (Q7): a click on a card tracks that quest (book.rs: the pulsing glow, the arrow at its top left, the
    // Quest Helper follows it).
    for i in 0..QUESTS_PER_PAGE {
        let k = st.page * QUESTS_PER_PAGE + i;
        if k >= d.quests.len() {
            break;
        }
        if let Some(w) = l.find(&format!("wndQuestInfo{i}")) {
            st.hits.push((l.nodes[w].rect, Hit::Quest(k)));
        }
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
    // Round 18 (quests F3): the goal's picture in Wizard101's goal square (imgGoalObjective1, hidden in the .gui
    // until the client fills it): the creature to defeat, the NPC to talk to, the Bone Cage.
    // Round 21 (Q3, Ryan): centred across the card (272 wide: 103..169), not in the left goal box.
    if !q.picture.is_empty() {
        if let Some(id) = card.find("imgGoalObjective1") {
            v.node_art.insert(id, q.picture.clone());
            v.show(Some(id), true);
            v.rect.insert(id, GOAL_PICTURE);
        }
    }
    // The other goals open now, across both objective boxes.
    hide(card, v, &["txtGoalObjective1", "txtGoalObjective2"]);
    if !q.other_goals.is_empty() {
        v.add([18.0, 134.0, 255.0, 168.0], format!("Also: {}", q.other_goals.join("; ")),
              TextStyle::new("WizardNormal", 0.36, BLACK).centred().wrapped());
    }
    match q.progress {
        Some((n, of)) => {
            set(v, "txtGoalCounter", format!("{n}/{of}"));
            // Round 20 (Q5, Ryan): the count sits over the bottom of the goal's picture (imgGoalObjective1,
            // 40,67..106,133), not to its right: the counter scroll (110 x 28 at 142,107, its text 17,9 in) made 90
            // wide and centred under the picture's middle, overlapping its lower edge.
            if !q.picture.is_empty() {
                if let (Some(scroll), Some(text)) = (card.find("imgGoalCounter"), card.find("txtGoalCounter")) {
                    // Round 21 (Q3): centred with the picture (card middle x 136).
                    v.rect.insert(scroll, [91.0, 115.0, 181.0, 138.0]);
                    v.rect.insert(text, [105.0, 121.0, 167.0, 138.0]);
                }
            }
        }
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
    // Round 20 (Q3): the reward at the bottom of the card, in Wizard101's reward slot (wndReward3, the middle one of
    // the five along the red band): the gold piece's ring around Elden Ring's rune, the amount on the scroll under it.
    if q.reward > 0 {
        let slot = 3;
        v.show(card.find(&format!("wndReward{slot}")), true);
        if let Some(id) = card.find(&format!("imgReward{slot}Symbol")) {
            v.node_art.insert(id, REWARD_RUNES.into());
        }
        if let Some(id) = card.find(&format!("imgReward{slot}Scroll")) {
            v.node_art.insert(id, REWARD_SCROLL.into());
        }
        set(v, &format!("txtReward{slot}Amount"), q.reward.to_string());
    }
}

/// Round 20 (Q3): the card's reward symbol (round 21 Q4: Wizard101's Art_Quest_Gold, the gold reward's ring and
/// orange inside, around Elden Ring's Lord's Rune icon; made by round21/quests/make_art.py) and Wizard101's scroll under the amount.
pub const REWARD_RUNES: &str = "GUI/QuestPage/Art_Quest_Rune_Piece.png";
pub const REWARD_SCROLL: &str = "GUI/QuestPage/Art_Quest_Exp_Scroll.png";
/// Round 20 (Q7): Wizard101's quest-tracker icon (_Shared-WorldData.wad GUI/QuestPage) and its glow round the
/// selected quest card (Art_Quest_Glow, a 256 square frame of soft yellow light).
pub const QUEST_TRACKER: &str = "GUI/QuestPage/Button_Quest_Tracker02.png";
pub const QUEST_GLOW: &str = "GUI/QuestPage/Art_Quest_Glow.png";
/// Round 21 (Q5): the selected quest's lit paper (see quest_tracked).
pub const QUEST_PAPER_LIT: &str = "GUI/QuestPage/Art_Quest_Paper_Lit.png";
/// Round 21 (Q3): imgGoalObjective1 (66 square, 40,67 in the .gui) centred on the card.
pub const GOAL_PICTURE: [f32; 4] = [103.0, 67.0, 169.0, 133.0];

/// Round 22 (H8, Ryan: "the current quest should be pulsing yellow, not constantly yellow; a slow pulse"): the lit
/// paper's alpha, rising and falling smoothly once every QUEST_PULSE seconds between QUEST_PULSE_LOW and 1. Wizard101
/// drives such glows from code with its WinAnimAlphaFade (Root.wad Scripts/API/WindowAnimations.lua:
/// CreateWinAnimAlphaFade(cycleTime, cycle), an alpha fade that cycles); the quest page's own cycle time is not in the
/// client's files, so the period is a slow breath.
const QUEST_PULSE: f32 = 2.4;
const QUEST_PULSE_LOW: f32 = 0.15;

pub fn quest_pulse() -> f32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    // The preview renders a moment of the pulse (env PREVIEW_PULSE_T, seconds).
    static FIXED: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    let fixed = *FIXED.get_or_init(|| std::env::var("PREVIEW_PULSE_T").ok().and_then(|v| v.parse().ok()));
    let t = fixed.unwrap_or_else(|| START.get_or_init(std::time::Instant::now).elapsed().as_secs_f32());
    let wave = 0.5 - 0.5 * (t / QUEST_PULSE * std::f32::consts::TAU).cos();
    QUEST_PULSE_LOW + (1.0 - QUEST_PULSE_LOW) * wave
}

/// Round 20 (Q7): the tracked quest's card: Wizard101's tracker arrow in imgActiveQuest (32 square), moved from
/// 36,44 (where it covered the goal text's first letters) into the card's top-left corner under the star.
pub fn quest_tracked(card: &Layout, v: &mut View) {
    // Round 21 (Q5, Ryan): no border; the selected quest's paper itself is lit: Wizard101's Art_Quest_Glow_01 light
    // laid on Art_Quest_Frame's parchment only (made by round21/quests/make_art.py), drawn right over the card's
    // paper, under everything on it.
    if let Some(id) = card.find("wndQuestInfo") {
        v.node_images.insert(id, QUEST_PAPER_LIT.into());
        v.node_image_alpha.insert(id, quest_pulse());
    }
    if let Some(id) = card.find("imgActiveQuest") {
        v.node_art.insert(id, QUEST_TRACKER.into());
        v.show(Some(id), true);
        v.rect.insert(id, [4.0, 38.0, 36.0, 70.0]);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Map: MapSpellbookPage.gui with Wizard101's own maps (tools/w101_map_art.py): Unicorn Way's zone map with the grace
// to travel to and the player's green arrow; at the Roundtable Hold, the whole of Wizard City with no marker (as when
// the map is opened from your house). Elden Ring's map (fast travel to every other grace) is behind the magnifier.

/// The part of the map page that is cut out to show Elden Ring's map while it is open.
pub const MAP_HOLE: [f32; 4] = [36.0, 56.0, 724.0, 524.0];
/// The copy of the game's screen while its map is open (book.rs).
pub const MAP_PICTURE: &str = "@screen/map";
/// Wizard101's maps (GUI-WorldData.wad Maps/*.dds) and markers (_Shared-WorldData.wad).
pub const MAP_UNICORN_WAY: &str = "GUI/Maps/Map_WC_Unicorn_Way.png";
pub const MAP_WIZARD_CITY: &str = "GUI/Maps/Map_WC.png";
/// The green "you are here" arrow: 8 frames of 64x64, 4 across and 2 down, its tip at the bottom middle.
pub const MAP_PLAYER: &str = "GUI/Buttons/Button_Player_Position.png";
/// Wizard101's teleporter icon (GUI/QuestButtons), for the grace.
pub const MAP_GRACE: &str = "GUI/QuestButtons/Art_Quest_Teleporter.png";
/// Where the map picture goes: Map_WC_Unicorn_Way.xml gives the map 720 x 600 pixels, the page's full height. The
/// picture is painted as the open book itself (spine, gold corners), so it sits at the page's left edge, its spine
/// and corners on the book's (centred, they showed twice); the zone name's scroll (ControlSprite 120,0..680,53) and
/// the buttons (y 529..579) sit on its wooden frame. Its icons' rectangles are in these 720 x 600 pixels.
pub const MAP_AREA: [f32; 4] = [0.0, 0.0, 720.0, 600.0];
/// Icon size on the map: Map_WC_Unicorn_Way.xml draws quest buttons (Location_Universal, the bone cages) 39 x 39.
const MAP_ICON: f32 = 39.0;
/// The arrow's size on the page and its frame rate (the texture's 8 frames).
const MAP_ARROW: f32 = 40.0;
const MAP_ARROW_FPS: f32 = 8.0;
/// The part of the Unicorn Way picture inside its painted frame (0..1 of the picture): the player's arrow shows only
/// in here.
const MAP_INSIDE: [f32; 4] = [0.121, 0.074, 0.961, 0.918];
/// The First Step grace (AEG099_090_9011, entity 1042361981) in tile m60_42_36_00 (x, z), from the deployed MSB.
const MAP_GRACE_AT: [f32; 2] = [-15.62, -60.549];
/// The grace icon's nudge on the page (page pixels): Ryan (round 16 H2) had it slightly to the left.
const MAP_GRACE_NUDGE: [f32; 2] = [-10.0, 0.0];
/// The PlusMinusButton's art while Elden Ring's map is open: Button_Zoom's plus row (tools/w101_map_art.py).
const MAP_ZOOM_PLUS: &str = "GUI/Buttons/Button_Zoom_Plus.png";

/// Wizard101's parchment name plates (_Shared-WorldData GUI/Nameplates): what a map icon shows while the mouse is on
/// it (Elite purple for bosses), and the place labels that are always shown.
const MAP_PLATE_ELITE: &str = "GUI/Nameplates/NamePlate_128x64_Elite.png";
const MAP_LABEL_SCROLL: &str = "GUI/Nameplates/Location_Long_2_Line_52_pixels_high.png";
const MAP_LABEL_POI: &str = "GUI/Nameplates/POI_128x64.png";
/// The plates' text: Comic Sans bold (WizardNormal), the dark brown of Wizard101's own map labels (82,51,29, measured
/// on the tutorial's picture of the map page, _Shared-WorldData GUI/Backgrounds/UWMap.dds).
const MAP_PLATE_FONT: &str = "WizardNormal";
const MAP_PLATE_INK: ImColor32 = ImColor32::from_rgb(82, 51, 29);
/// Measured on that picture: the text is WizardNormal at 0.5 ("Skeletal" 63, "COMMONS" 88 page pixels wide, as
/// drawn there) with lines 20 page pixels apart (closer than the font's own 47-pixel cell, so each line is placed by
/// itself).
const MAP_PLATE_SCALE: f32 = 0.5;
const MAP_PLATE_LINE: f32 = 20.0;

/// A map icon from Map_WC_Unicorn_Way.xml's icon list (rectangles in its 720 x 600 pixels): picture and rect, its
/// hover plate and that plate's size, and the plate's text (WCNameplates.lang).
///
/// The hover plate as the tutorial's picture shows it (the Skeletal Pirate's): sized to its text (13 page pixels
/// either side and 6 above and below, room for the plate art's torn edges; WizardNormal 0.5 widths below), its left
/// edge 8 pixels inside the icon's right edge, centred on the icon's height, the icon drawn over it.
struct MapIcon {
    picture: &'static str,
    rect: [f32; 4],
    plate: &'static str,
    plate_size: [f32; 2],
    text: &'static str,
}

/// The bosses of Unicorn Way's towers that have rooms here: Lady Blackhope (tower T1; WCNameplates_00000178,
/// "Blackhope" 75 wide) and Rattlebones (tower T2; WCNameplates_00000173, 91 wide), both on the Elite plate. Wizard101
/// draws their tower icon (LocationIconUnicornWayTower) in the same spots before the quests reach them.
const MAP_BOSSES: [MapIcon; 2] = [
    MapIcon { picture: "GUI/NpcPortraits/Art_Portrait_GhostLady_Green.png", rect: [203.0, 216.0, 252.0, 265.0],
              plate: MAP_PLATE_ELITE, plate_size: [101.0, 52.0], text: "Lady\nBlackhope" },
    MapIcon { picture: "GUI/NpcPortraits/Portrait_Skeleton_Armored_A_MOB.png", rect: [518.0, 273.0, 567.0, 322.0],
              plate: MAP_PLATE_ELITE, plate_size: [117.0, 32.0], text: "Rattlebones" },
];

/// Unicorn Way's place labels, always shown as Wizard101 does (its painting has them too, small and blurred; the
/// game draws them again over the painting in its font). The rectangles are where the tutorial's picture shows them:
/// each is the icon list's rectangle grown to fit its text and drawn on the same centre, covering the painted one.
const MAP_LABELS: [(&str, [f32; 4], &str); 4] = [
    (MAP_LABEL_SCROLL, [90.0, 417.0, 190.0, 465.0], "ARENA"),
    (MAP_LABEL_SCROLL, [45.0, 282.0, 172.0, 338.0], "The\nCOMMONS"),
    (MAP_LABEL_POI, [512.0, 68.0, 578.0, 128.0], "Hedge\nMaze"),
    (MAP_LABEL_POI, [282.0, 442.0, 365.0, 500.0], "Unicorn\nPark"),
];

/// WizardNormal's cell is 47 font pixels high and its capitals fill rows 13 to 38 of it (WizardNormal.xml and its
/// atlas): a line's box is one cell (at MAP_PLATE_SCALE), and the capitals' middle is this far down it.
const MAP_PLATE_CELL: f32 = 47.0 * MAP_PLATE_SCALE;
const MAP_PLATE_CAPS_MID: f32 = 25.5 * MAP_PLATE_SCALE;

/// Where the parchment's middle is in a plate's art (0..1 of its height, measured on the 128 x 64 pictures' opaque
/// rows): the scroll label's parchment is rows 3..54 (its curled ends hang below it), the others are centred.
fn map_plate_mid(plate: &str) -> f32 {
    if plate == MAP_LABEL_SCROLL { 28.5 / 64.0 } else { 32.5 / 64.0 }
}

/// A parchment plate with its lines of text centred in it (the map's labels and hover plates): the capitals' middle
/// of the lines on the parchment's middle (round 17 A1: they sat low, each cell's top on its line's top).
fn map_plate(v: &mut View, plate: &str, r: [f32; 4], text: &str) {
    v.image(r, plate);
    let lines: Vec<&str> = text.split('\n').collect();
    let mid = r[1] + (r[3] - r[1]) * map_plate_mid(plate);
    let first = mid - (lines.len() - 1) as f32 * MAP_PLATE_LINE / 2.0;
    for (i, line) in lines.iter().enumerate() {
        let y = first + i as f32 * MAP_PLATE_LINE - MAP_PLATE_CAPS_MID;
        v.add([r[0], y, r[2], y + MAP_PLATE_CELL], line.trim(),
              TextStyle::new(MAP_PLATE_FONT, MAP_PLATE_SCALE, MAP_PLATE_INK).centred());
    }
}

/// A map icon that shows its parchment plate while the mouse is on it (MapIcon).
fn map_icon(v: &mut View, icon: &MapIcon) {
    v.image(icon.rect, icon.picture);
    let r = icon.rect;
    let (x, y) = (r[2] - 8.0, (r[1] + r[3]) / 2.0 - icon.plate_size[1] / 2.0);
    let mut hover = View::default();
    map_plate(&mut hover, icon.plate, [x, y, x + icon.plate_size[0], y + icon.plate_size[1]], icon.text);
    hover.image(r, icon.picture);
    v.hovers.push((r, Box::new(hover)));
}
/// Elden Ring tile m60_42_36_00 and the Roundtable Hold's map (m11_10).
const STREET_TILE: [i32; 2] = [42, 36];
const ROUNDTABLE: [u32; 2] = [11, 10];

/// Where a point of tile m60_42_36_00 (x, z metres) is on Wizard101's Unicorn Way picture (0..1, y down).
///
/// Composed from (1) the street part AEG030_518_9000 in the MSB: position (-8.562, -75.911), rotation -27.01 degrees,
/// world = pos + (fx cos + fz sin, -fx sin + fz cos) of its FLVER coordinates, which are Wizard101's (x, z, y) / 100;
/// and (2) Wizard101 (x, y) to the 512-pixel picture: p = 0.0097679 e^(i 110.868 deg) (x - i y) + (125.363 + 305.760 i),
/// fitted by laying the street's walk graph (zone.nav) over the picture's paths (1 picture pixel = about 1 m).
pub fn street_uv(x: f32, z: f32) -> [f32; 2] {
    const A: f32 = 0.001_415_04;
    const B: f32 = 0.001_279_58;
    [-A * x + B * z + 0.329_868, B * x + A * z + 0.715_560]
}

/// Which Wizard101 map the page shows for the player's place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MapPlace {
    /// Unicorn Way, with the player's arrow where they are (0..1 of the picture) when they are on it.
    Street(Option<[f32; 2]>),
    /// Wizard City as a whole, without markers (the Roundtable Hold: Wizard101's house).
    WizardCity,
}

/// The map for a Game::position (chunk position, block, block origin).
pub fn map_place(position: Option<([f32; 3], i32, i32)>) -> MapPlace {
    let Some((chunk, block, origin)) = position else { return MapPlace::Street(None) };
    let b = block as u32;
    if [b >> 24, b >> 16 & 0xFF] == ROUNDTABLE {
        return MapPlace::WizardCity;
    }
    let Some(o) = crate::markers::tile(origin) else { return MapPlace::Street(None) };
    let local = [chunk[0] + o[0] - STREET_TILE[0] as f32 * 256.0, chunk[2] + o[1] - STREET_TILE[1] as f32 * 256.0];
    let uv = street_uv(local[0], local[1]);
    let i = MAP_INSIDE;
    let inside = uv[0] > i[0] && uv[0] < i[2] && uv[1] > i[1] && uv[1] < i[3];
    MapPlace::Street(inside.then_some(uv))
}

/// A point of the map picture (0..1) on the page.
fn map_at(uv: [f32; 2]) -> [f32; 2] {
    let a = MAP_AREA;
    [a[0] + uv[0] * (a[2] - a[0]), a[1] + uv[1] * (a[3] - a[1])]
}

/// The grace marker's rectangle on the page.
pub fn map_grace_rect() -> [f32; 4] {
    let p = map_at(street_uv(MAP_GRACE_AT[0], MAP_GRACE_AT[1]));
    let p = [p[0] + MAP_GRACE_NUDGE[0], p[1] + MAP_GRACE_NUDGE[1]];
    let h = MAP_ICON / 2.0;
    [p[0] - h, p[1] - h, p[0] + h, p[1] + h]
}

/// The old picture of Elden Ring's map screen on the page: (map rect, the frame around it) for a screen of `aspect`
/// (width / height), as big as fits in MAP_HOLE.
pub fn map_picture_rect(aspect: f32) -> ([f32; 4], [f32; 4]) {
    let h = MAP_HOLE;
    let area = [h[0] + 8.0, h[1] + 8.0, h[2] - 8.0, h[3] - 8.0];
    let (room_w, room_h) = (area[2] - area[0], area[3] - area[1]);
    let (w, ht) = if room_w / room_h > aspect { (room_h * aspect, room_h) } else { (room_w, room_w / aspect) };
    let (cx, cy) = ((area[0] + area[2]) / 2.0, (area[1] + area[3]) / 2.0);
    let map = [cx - w / 2.0, cy - ht / 2.0, cx + w / 2.0, cy + ht / 2.0];
    (map, [map[0] - 6.0, map[1] - 6.0, map[2] + 6.0, map[3] + 6.0])
}

/// The map's place on the page while Elden Ring's map shows through without a copy of the screen.
pub fn map_rect(aspect: f32) -> [f32; 4] {
    let h = MAP_HOLE;
    let (room_w, room_h) = (h[2] - h[0] - 16.0, h[3] - h[1] - 16.0);
    let (w, ht) = if room_w / room_h > aspect { (room_h * aspect, room_h) } else { (room_w, room_w / aspect) };
    let (cx, cy) = ((h[0] + h[2]) / 2.0, (h[1] + h[3]) / 2.0);
    [cx - w / 2.0, cy - ht / 2.0, cx + w / 2.0, cy + ht / 2.0]
}

fn map(l: &Layout, d: &Data, v: &mut View, st: &mut State) {
    v.show(l.find("MapLoadingIndicator"), d.map_opening);
    v.set(l.path("MapLoadingIndicator/PackageNameText"), centre("Opening Map..."));
    hide(l, v, &["txtGoHomeTimer", "txtGotoDormTimer"]);
    // Fishing and skeleton keys: nothing in Elden Ring.
    hide(l, v, &["FishingSprite", "SkeletonKeySprite"]);
    // The buttons' hover texts: MapSpellbookPage.gui gives each an m_sTip (GUI_00001513 "Commons", GUI_00001514 "Zoom",
    // GUI_00001515 "Go Home"), which Wizard101 shows in ToolTip.gui's white bubble; ours say where they go.
    let zoom = if d.map_open { "Back to Wizard101's map" } else { "Elden Ring's map" };
    for (button, text) in [("GoHomeButton", "Travel to The First Step"), ("GotoDormButton", "Travel to the Roundtable Hold"),
                           ("PlusMinusButton", zoom)] {
        if let Some(b) = l.find(button) {
            v.plain_tip(l.nodes[b].rect, vec![text.to_string()]);
        }
    }
    if d.map_open {
        // The magnifier turns to plus (Button_Zoom's top row) while Elden Ring's map is open; clicking it goes back.
        if let Some(b) = l.find("PlusMinusButton") {
            v.node_art.insert(b, MAP_ZOOM_PLUS.into());
        }
        // Elden Ring's own map, opened with the magnifier: drawn on the page as before.
        v.set(l.find("ZoneName"), centre("The Lands Between"));
        match d.map_picture {
            Some(aspect) => {
                let (r, frame) = map_picture_rect(aspect);
                v.fills.push((frame, ImColor32::from_rgb(74, 46, 24)));
                v.images.push((r, MAP_PICTURE.into(), [0.0, 0.0, 1.0, 1.0]));
                v.picture = Some(r);
            }
            None => v.hole = Some(map_rect(16.0 / 9.0)),
        }
        v.add([MAP_HOLE[0], MAP_HOLE[3] + 4.0, MAP_HOLE[2], MAP_HOLE[3] + 26.0],
              "Close the map as usual, or click the magnifier or another tab of the book", TextStyle::new("WizardFancy", 0.45, BLACK).centred());
        return;
    }
    // Wizard101's map under the page's own controls: the page's root window draws it in MAP_AREA.
    let place = map_place(d.position);
    let (zone, picture) = match place {
        MapPlace::WizardCity => ("Wizard City", MAP_WIZARD_CITY),
        MapPlace::Street(_) => ("Unicorn Way", MAP_UNICORN_WAY),
    };
    v.set(l.find("ZoneName"), centre(zone));
    v.rect.insert(0, MAP_AREA);
    v.node_images.insert(0, picture.into());
    let MapPlace::Street(player) = place else { return };
    for (plate, r, text) in MAP_LABELS {
        map_plate(v, plate, r, text);
    }
    for icon in &MAP_BOSSES {
        map_icon(v, icon);
    }
    // The grace: click (or A) to travel there. Round 17 A2: nothing on hover (no name plate), only the icon.
    let grace = map_grace_rect();
    v.image(grace, MAP_GRACE);
    st.hits.push((grace, Hit::Warp(WARP_UNICORN_WAY)));
    if let Some(uv) = player {
        let p = map_at(uv);
        let frame = (map_clock() * MAP_ARROW_FPS) as usize % 8;
        let (fu, fv) = ((frame % 4) as f32 * 0.25, (frame / 4) as f32 * 0.5);
        // The arrow's tip (bottom middle of its frame, about 97% down) on the player's spot.
        let r = [p[0] - MAP_ARROW / 2.0, p[1] - MAP_ARROW * 0.97, p[0] + MAP_ARROW / 2.0, p[1] + MAP_ARROW * 0.03];
        v.images.push((r, MAP_PLAYER.into(), [fu, fv, fu + 0.25, fv + 0.5]));
    }
}

/// Seconds since the map page was first drawn (the arrow's animation).
fn map_clock() -> f32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f32()
}

// ---------------------------------------------------------------------------------------------------------------
// Crafting: RecipeWindow.gui (crafting.rs); its materials page here.

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

/// The materials page (Reagent_Toggle on the crafting page, crafting.rs): Wizard101's reagents page.
pub(crate) fn materials(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    let rows: Vec<usize> = (1..=8).filter_map(|i| l.find(&format!("Item_{i}"))).collect();
    let Some(sheet) = d.sheet else { return };
    let mut list: Vec<(String, u32, u32)> =
        sheet.items.iter().filter(|(id, _)| is_material(*id)).map(|(id, n)| (d.names.item(*id), *n, *id)).collect();
    list.sort();
    // ReagentWindow's category tabs (REAGENT_TABS): all, harvested, rare harvested, bought, dropped. Elden
    // Ring's own sort groups say what a material is: creature parts (10, 15) are dropped, plants and stones
    // (20-50) harvested, rare ones (rarity above 0) rare harvested; bought is what a shop sells.
    st.subs = REAGENT_TABS.len();
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
}

/// Wizard101's reagents page (ReagentWindow.gui) on RecipeWindow's pages: its title, its category tabs on
/// Art_CraftingTab (all reagents, harvested, rare harvested, bought, dropped; Wizard101's class project tab is left
/// out, Elden Ring has none) with Wizard101's plain tooltip (ToolTip.gui) on hover,
/// the reagents as Background_Reagent cards, four on each page at ReagentWindow's places (each with its picture
/// on a dark band, its name on the paper below and the count in the sun badge), the page buttons under the left
/// page, and the toggle back to the recipes with Wizard101's book.
fn reagents(l: &Layout, d: &Data, st: &mut State, v: &mut View, list: &[(String, u32, u32)], rows: &[usize]) {
    v.set(l.find("title"), centre("Materials"));
    hide(l, v, &["portrait_background", "card_background", "IngredientBackground", "TabLayoutEquipment",
                 "TabBackground", "CreateButton", "EquipButton", "FiltersButton"]);
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
        // Frames: normal top left, highlighted top right (the chosen tab).
        let r = [x, 52.0, x + 40.0, 92.0];
        let uv = if i == chosen { [0.5, 0.0, 1.0, 0.5] } else { [0.0, 0.0, 0.5, 0.5] };
        v.images.push((r, format!("GUI/Buttons/{button}.png"), uv));
        v.plain_tip(r, vec![tip.to_string()]);
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
        // The card's round window: the material's picture as Wizard101's reagent pictures are drawn (the
        // medallion, the object filling its window).
        crate::crafting::reagent_picture(v, d.items, *id, rect(64.0 - 48.0, 77.0 - 48.0, 64.0 + 48.0, 77.0 + 48.0), 0.75);
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
        // Round 22 B2: no hover on the reagent cards (name, rank, school and count are on the card).
    }
    if list.is_empty() {
        let text = if chosen == 0 { "No materials yet" } else { "None of these yet" };
        v.add([61.0, 200.0, 387.0, 240.0], text, TextStyle::new("WizardFancy", 0.55, BLACK).centred());
    }
}

/// ReagentWindow's category tabs: (button art, what it is in Wizard101).
const REAGENT_TABS: [(&str, &str); 5] = [
    ("Button_Crafting_Reagents", "All Materials"), ("Button_Crafting_Harvest", "Harvested Materials"),
    ("Button_Crafting_RareHarvest", "Rare Harvested Materials"), ("Button_Gold_Bag", "Bought Materials"),
    ("Button_Crafting_WorldDrop", "Dropped Materials"),
];
/// Wizard101's reagent card.
const REAGENT_FRAME: &str = "GUI/Cards/School_Resource_Blnk.png";
/// Its blank medallion (the round picture behind the card's window).
const REAGENT_MEDALLION: &str = crate::crafting::MEDALLION;

/// A Wizard101 school's round badge (GUI/Schools/Art_School_<School>, 128 x 128: the symbol on the school's colour
/// in a gold ring), as the Character page's SchoolIcon and the Pets page's spriteSchoolIcon show it.
pub fn school_art(school: &str) -> String {
    format!("GUI/Schools/Art_School_{school}.png")
}

/// The Wizard101 school of a spirit ash (the book's own choice, Ryan round 16), by what the summon is, with the
/// deck's school meanings: Myth = glintstone sorcery and conjured copies, Ice = Carian and crystal, Fire = fire
/// and the Fire Monks / Messmer's soldiers, Storm = wind, storm and Farum Azula, Life = the Golden Order, the
/// Erdtree and beasts, Death = the dead, rot and blood, Shadow = night and sleep, Balance = everyone else
/// (soldiers, sellswords and nobles).
pub fn ash_school(name: &str) -> &'static str {
    const SCHOOLS: [(&str, &[&str]); 8] = [
        ("Shadow", &["Nightmaiden", "Dolores", "Black Knife", "Tiche"]),
        ("Death", &["Skeletal", "Putrid", "Rot", "Rotten", "Rat", "Jarwight", "Bloodfiend", "Gravebird",
                    "Mausoleum", "Dung Eater", "Lhutel", "Fingercreeper", "Man-Fly", "Man-Serpent", "Ghost"]),
        ("Fire", &["Fire Monk", "Pumpkin", "Messmer", "Fanged Imp", "Bigmouth", "Spider Scorpion", "Inquisitor"]),
        ("Ice", &["Crystalian", "Raya Lucaria", "Jellyfish", "Twinsage", "Latenna"]),
        ("Myth", &["Glintstone", "Sorcerer", "Page", "Clayman", "Albinauric", "Mimic", "Marionette", "Avionette"]),
        ("Storm", &["Nepheli", "Azula", "Warhawk", "Winged Misbegotten", "Horned Warrior", "Stormhawk"]),
        ("Life", &["Oracle", "Leyndell", "Haligtree", "Therolina", "Miranda", "Ancestral", "Wolf", "Land Squirt",
                   "Finger Maiden"]),
        ("Balance", &[]),
    ];
    SCHOOLS.iter().find(|(_, words)| words.iter().any(|w| name.contains(w))).map_or("Balance", |s| s.0)
}

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

/// One help page: heading, text, and an icon list under the text (icon art, what it means), as Wizard101's Icon
/// Dictionary pages (HelpInterfaceIcons*.gui) lay theirs out.
type HelpPage = (&'static str, &'static str, &'static [(&'static str, &'static str)]);

/// (topic button, spreads (left page, right page)). The spreads are Wizard101's own help pages (Help*.gui and
/// Help.lang, round 15 survey) wherever Elden Ring has the same thing, told in Elden Ring's terms; the book's own
/// controls come first. Topics with nothing in Elden Ring (Housing, Crown Shop, Gardening, Fishing) stay greyed;
/// Wizard101's Mounts and Socketing tips are under Pets and Crafting.
const HELP: [(&str, &[(HelpPage, HelpPage)]); 6] = [
    ("Tab_Questing", &[
        (("The Spellbook",
          "Open and close the book with Elden Ring's map button (B on the keyboard).\n\nEsc also closes it.", &[]),
         ("Tabs and Categories",
          "The tabs down the right edge are the book's pages, as in Wizard101.\n\nThe round buttons along the top of \
           a page are its categories.\n\nPoint at an item for its stats.", &[])),
        // HelpQuestingTips1: the quest markers.
        (("Quest Markers",
          "A yellow exclamation point over someone's head means that person has a quest for you.\n\nA yellow \
           question mark means you should talk to that person: they are part of a quest you are working on.\n\n\
           The Quests page lists the Unicorn Way quests you have taken on.", &[]),
         // HelpInterfaceTips1: the Commons and Go Home buttons.
         ("Getting Around",
          "The World Map page shows Wizard101's map of Unicorn Way (Wizard City at the Roundtable Hold). Click the \
           grace on it, or press A with the stick cursor on it, to travel there.\n\nThe arrows take you to Unicorn \
           Way, as Wizard101's Commons button does, and the house takes you home, to the Roundtable Hold.\n\nThe \
           magnifier opens Elden Ring's map, for fast travel to any other Site of Grace you have found.", &[])),
        // HelpQuestingTips1 (wisps) and HelpInterfaceTips1 (potions), HelpCrownShop2-3 (elixirs).
        (("Health and Mana",
          "The Flask of Crimson Tears restores Health and the Flask of Cerulean Tears restores Mana (FP), as \
           Wizard101's potions do.\n\nResting at a Site of Grace refills your flasks, your Health and your Mana.\
           \n\nThe Character page shows how many drinks are left.", &[]),
         ("Elixirs",
          "Wizard101's elixirs are Elden Ring's Flask of Wondrous Physick: mix two Crystal Tears into it at a Site \
           of Grace, then drink it for their effects.\n\nOther boosts come from the consumables in your \
           Backpack.", &[])),
    ]),
    ("Tab_Combat", &[
        // (Round 19 B1: no controller button page in the book.)
        // HelpInterfaceIcons2-3: the schools and stat icons, as the item tooltips use them.
        (("Schools",
          "Item stats show Elden Ring's damage types with Wizard101's school symbols:", &[
              ("GUI/Icons/Icon_All.png", "Physical: all schools"),
              ("GUI/Icons/Icon_Myth.png", "Magic: Myth"),
              ("GUI/Icons/Icon_Fire.png", "Fire"),
              ("GUI/Icons/Icon_Storm.png", "Lightning: Storm"),
              ("GUI/Icons/Icon_Life.png", "Holy: Life"),
          ]),
         // HelpCombatTips2 (criticals) and HelpCombatTips5 (stuns).
         ("Criticals and Stuns",
          "A Critical in Elden Ring is a backstab, a riposte after a parry, or a hit on a staggered foe: it does \
           bonus damage.\n\nStun Resist is poise: the more you have, the less you flinch when you are hit.\n\n\
           Heavy hits break a foe's stance and stun it.", &[])),
        // HelpCombatTips1: fallen combatants.
        (("Defeat",
          "If your Health reaches zero you wake at the last Site of Grace you rested at, and your runes stay where \
           you fell.\n\nGo back and touch the glow to take them back. Fall again on the way and they are lost.", &[]),
         // HelpCrownShop5: henchmen.
         ("Spirit Allies",
          "Wizard101's henchmen are Elden Ring's spirit ashes: near a Rebirth Monument, use one from your quick \
           items and its spirits fight beside you until the fight is over.", &[])),
    ]),
    ("Tab_Interface", &[
        (("Keyboard",
          "Up / Down arrows: previous / next page of the book.\n\nQ / E: previous / next category.\n\nLeft / Right \
           arrows: turn the page of a list.\n\nEnter: use the tab.\n\nB or Esc: close the book.", &[]),
         ("Mouse",
          "Click a tab or a button; point at an item for its stats.\n\nThe mouse wheel turns the page of a list.\n\n\
           Drag your character in the Backpack to turn it.\n\nClick an item to choose it; double-click it to equip \
           it, or to take it off when it is equipped.", &[])),
        // HelpInterfaceIcons3-6: the stat icons.
        (("Icon Dictionary", "", &[
              ("GUI/Icons/Type_Damage.png", "Damage (attack power)"),
              ("GUI/Icons/Type_Resist.png", "Resist (damage negation)"),
              ("GUI/Icons/Type_Stun_Resist.png", "Stun Resist (poise)"),
              ("GUI/Icons/Type_Pip.png", "Pips (FP cost)"),
              ("GUI/Icons/Type_PowerPips.png", "Memory slots"),
              ("GUI/Icons/Type_Heal.png", "Heal"),
              ("GUI/Icons/Type_Ward.png", "Ward (negation, barriers)"),
              ("GUI/Icons/Type_Charm.png", "Charm (boosts)"),
          ]),
         // HelpInterfaceTips2-3: the equipped mark and badges.
         ("Backpack and Badges",
          "An equipped item has a closed hand next to it in the Backpack. Double-click an item to equip it.\n\n\
           Badges are Elden Ring's achievements: the Character page's badge tab lists them, greyed out until you \
           earn them. Point at one to see what earns it.", &[])),
    ]),
    ("Tab_Character", &[
        (("The Pages",
          "Character: level, health, mana (FP), energy (stamina), flasks, attributes and badges.\n\nBackpack: \
           everything you carry. The fist equips the chosen item, the open hand takes it off.\n\nPets: Torrent and \
           your spirit ashes (the fist puts one in your quick items).", &[]),
         ("More Pages",
          "Deck: your memory slots, and your spells by school. Click a spell to memorize it, a slot to forget it, \
           anywhere.\n\nQuests: the Unicorn Way quests you have taken on.\n\nWorld Map: Wizard101's map of Unicorn Way; its magnifier opens Elden Ring's \
           map.\n\n\
           Crafting: Elden Ring's recipes by category; choose one and Create to craft it. Reagents shows your \
           materials.\n\nSettings: the HUD's settings.", &[])),
        // HelpCharacterTips1: training points.
        (("Levels",
          "Runes are Elden Ring's experience. Rest at a Site of Grace and spend them to gain a level.\n\nEach level \
           raises one attribute: Vigor (Health), Mind (Mana), Endurance (Energy), Strength, Dexterity, \
           Intelligence, Faith or Arcane.", &[]),
         ("Spells",
          "Spells are bought from sorcerers and priests, or found.\n\nEach takes memory slots in your deck; Memory \
           Stones give you more slots. Point at a spell in the deck to see its card.", &[])),
    ]),
    ("Tab_Crafting", &[
        // HelpCraftingTips0-1: station, recipes, reagents.
        (("Crafting",
          "Crafting needs the Crafting Kit, which Merchant Kale sells at the Church of Elleh.\n\nRecipes come from \
           cookbooks, found or bought around the Lands Between.\n\nMaterials are gathered from plants and \
           creatures, or bought from merchants.", &[]),
         ("The Crafting Page",
          "The Crafting tab lists what you can craft and the materials you carry, by kind.", &[])),
        // HelpSocketing1-3: jewels.
        (("Ashes of War",
          "Wizard101's jewels are Elden Ring's Ashes of War. At a Site of Grace, with a Whetblade, an Ash of War \
           goes on a weapon and gives it a skill and an affinity.\n\nUnlike a jewel, an Ash of War can be moved to \
           another weapon whenever you like.", &[]),
         ("Upgrades",
          "Smithing Stones upgrade weapons at a smithing table, such as Smithing Master Hewg's in the Roundtable \
           Hold.\n\nGlovewort upgrades spirit ashes with Roderika, the spirit tuner in the Roundtable Hold.", &[])),
    ]),
    ("Tab_Pets", &[
        // HelpPetTips1-2: pets and their ages.
        (("Pets",
          "Your pets are Torrent, the spectral steed, and your spirit ashes.\n\nSpirit ashes are summoned near \
           Rebirth Monuments. The fist on the Pets page puts the chosen ash in your quick items.", &[]),
         ("Pet Ages",
          "A spirit ash grows as you upgrade it with Grave or Ghost Glovewort, from +0 to +10.\n\nThe age bar on the \
           Pets page shows it as a Wizard101 pet age: Baby, Teen, Adult, Ancient, Epic, Mega and, at +10, Ultra.",
          &[])),
        // HelpCrownShop1 and HelpMount1-2: mounts.
        (("Mounts",
          "Torrent is your mount: use the Spectral Steed Whistle to call him and ride.\n\nMounts can't be ridden \
           everywhere. Torrent fades away indoors and in most boss fights, as Wizard101 sets you down for a \
           duel.", &[]),
         // HelpPetTips2: talents.
         ("Talents",
          "Wizard101's pet talents are what Torrent or a spirit ash can do, listed on the Pets page.\n\nTorrent can \
           double jump, and spiritsprings carry him high into the air.", &[])),
    ]),
];

/// The help spread's two pages (page pixels, inside the book's margins and clear of the spine).
/// Round 18 E1: they start under the tab strip (HELP_STRIP ends at 120, its torn edge about 112).
const HELP_PAGES: [[f32; 4]; 2] = [[84.0, 132.0, 360.0, 528.0], [412.0, 132.0, 688.0, 528.0]];
/// The strip behind the topic buttons that sets the two pages apart below it (Ryan round 18 E1): Wizard101's help
/// windows' "toolbar" sprite (e.g. HelpCombatCardDescription.gui: GUI/InventoryPage/Art_Inventory_Tab_Background at
/// 62,12 .. 718,120, as on the Backpack and Crafting pages). Drawn in mainBackground's place, under the buttons.
const HELP_STRIP: [f32; 4] = [62.0, 12.0, 718.0, 120.0];
const HELP_STRIP_ART: &str = "GUI/InventoryPage/Art_Inventory_Tab_Background.png";
/// The topic buttons' pitch (HelpSpellbookPage's TabGroup1: 59 px from 76).
const HELP_TAB_X: f32 = 76.0;
const HELP_TAB_PITCH: f32 = 59.0;
/// An icon list's rows: the icon's size and the row pitch (page pixels).
const HELP_ICON: f32 = 30.0;
const HELP_ICON_ROW: f32 = 38.0;

fn help(l: &Layout, st: &mut State, v: &mut View) {
    st.subs = HELP.len();
    let (button, spreads) = HELP[st.sub % HELP.len()];
    let chosen = l.find(button);
    if let Some(c) = chosen {
        v.checked.insert(c);
    }
    if let Some(bg) = l.find("mainBackground") {
        v.rect.insert(bg, HELP_STRIP);
        v.node_images.insert(bg, HELP_STRIP_ART.into());
    }
    hide(l, v, &["TabGroup2"]);
    // Round 18 E2: the topics Elden Ring has nothing for (Housing = Button_Dorm, Crown Shop = Button_Crown_Shop,
    // Gardening = Button_Garden_All) and the arrow to Wizard101's second topic row (NextHelpPage,
    // Button_Category_Arrow: Mounts, Fishing, Socketing) are gone rather than greyed; the topics left close up
    // along TabGroup1's row. To bring one back, add it to HELP and drop it from this list.
    hide(l, v, &["Tab_Housing", "Tab_CrownShop", "Tab_Gardening", "NextHelpPage"]);
    let mut row: Option<[f32; 4]> = None;
    for (i, (button, _)) in HELP.iter().enumerate() {
        let Some(b) = l.find(button) else { continue };
        let r = l.nodes[b].rect;
        let x = HELP_TAB_X + i as f32 * HELP_TAB_PITCH;
        let nr = [x, r[1], x + (r[2] - r[0]), r[3]];
        v.rect.insert(b, nr);
        row = Some(row.map_or(nr, |a| [a[0].min(nr[0]), a[1].min(nr[1]), a[2].max(nr[2]), a[3].max(nr[3])]));
        if Some(b) == chosen {
            if let (Some(m), Some(first)) = (l.find("TabBackground"), l.find("Tab_Questing")) {
                let (mr, fr) = (l.nodes[m].rect, l.nodes[first].rect);
                let dx = x - fr[0];
                v.rect.insert(m, [mr[0] + dx, mr[1], mr[2] + dx, mr[3]]);
            }
        }
    }
    v.tab_row = row;
    // Each topic's spreads turn with the page arrows under the book, as Wizard101's help pages do.
    let page = paging(l, v, st, spreads.len(), 1, ("pageText", "leftscroll", "rightscroll"));
    let (left, right) = spreads[page];
    for ((heading, text, icons), r) in [left, right].into_iter().zip(HELP_PAGES) {
        v.add([r[0], r[1], r[2], r[1] + 36.0], heading, TextStyle::new("WizardFancy", 0.7, BLACK).centred());
        v.fills.push(([r[0] + 40.0, r[1] + 42.0, r[2] - 40.0, r[1] + 43.5], ImColor32::from_rgba(90, 60, 30, 110)));
        let mut body = TextStyle::new("WizardNormal", 0.46, BLACK).wrapped();
        body.middle = false;
        let mut y = r[1] + 54.0;
        if !text.is_empty() {
            let bottom = if icons.is_empty() { r[3] } else { y + 60.0 };
            v.add([r[0], y, r[2], bottom], text, body);
            y = bottom + 6.0;
        }
        for (art, what) in icons.iter() {
            v.image([r[0] + 20.0, y, r[0] + 20.0 + HELP_ICON, y + HELP_ICON], *art);
            v.add([r[0] + 30.0 + HELP_ICON, y, r[2], y + HELP_ICON], *what, TextStyle::new("WizardNormal", 0.46, BLACK));
            y += HELP_ICON_ROW;
        }
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
/// OptionValue's text: Settings.gui has WizardFancy 0.6 in a 22-pixel red box (Button_Red01's top quarter); the text
/// size is 0.54 (Ryan, round 15/17: the size is right).
const VALUE_SCALE: f32 = 0.54;
/// The red value banner, the same on every row (Ryan, round 17 E1: Settings.gui's 22-pixel box was too short for the
/// text and rows 6-7 drew their own): 462..654 wide as in Settings.gui, 28 high, centred on the row's arrow buttons
/// (row top + 6..+48, middle +27). The value's text box is the banner itself: WizardFancy's capitals fill rows 5..36
/// of its 43-pixel cell (WizardFancy.xml), the cell's middle, so the layout's vertical centring centres them in it.
const VALUE_BANNER: [f32; 4] = [462.0, 13.0, 654.0, 41.0];
/// The value text's sides: inside the banner, between the arrows (as before round 17).
const VALUE_TEXT_X: [f32; 2] = [474.0, 642.0];

/// One settings row as the page shows it: label and value.
pub type SettingRow = (String, String);

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
    // Quit, OK, Help, Idle and the option rows have no tooltip in Settings.gui (empty m_sTip); only the tab buttons
    // do. The options are just clicked (Ryan, round 16 L2).
    v.set(l.find("QuitButton"), centre("Quit"));
    v.set(l.find("IdleButton"), centre("Exit Game"));
    // Every label is black, the last-changed row too (Ryan, round 16 L1: no white highlight where the caret was).
    let label_style = TextStyle::new("WizardFancy", 0.6857, BLACK);
    let value_style = TextStyle::new("WizardFancy", VALUE_SCALE, ImColor32::from_rgb(0xCC, 0xCC, 0x00)).centred();
    for i in 0..SETTINGS_PAGE_ROWS {
        let shown = i < rows.len();
        let (label, value) = d.settings.get(i).cloned().unwrap_or_default();
        // The row's top: Settings.gui's own rows, or rows 6 and 7 in their art (OptionHardwareCursor's, moved down).
        let top = match SETTINGS_ROWS.get(i) {
            Some(name) => {
                let Some(r) = l.find(name) else { continue };
                v.show(Some(r), shown);
                // The label and value are drawn below instead (one line, fitted), so the layout's are left empty.
                v.set(l.find_under(r, "OptionText"), "");
                let top = l.nodes[r].rect[1];
                if let Some(value) = l.find_under(r, "OptionValue") {
                    v.set(Some(value), "");
                    // Its red back (drawn under the arrows) as the banner of every row.
                    let b = VALUE_BANNER;
                    v.rect.insert(value, [b[0], top + b[1], b[2], top + b[3]]);
                }
                top
            }
            None => EXTRA_ROWS[i - SETTINGS_ROWS.len()],
        };
        if !shown {
            continue;
        }
        if i >= SETTINGS_ROWS.len() {
            let left = [436.0, top + 6.0, 478.0, top + 48.0];
            let right = [638.0, top + 6.0, 680.0, top + 48.0];
            let b = VALUE_BANNER;
            v.images.push(([b[0], top + b[1], b[2], top + b[3]], "GUI/Buttons/Button_Red01.png".into(),
                           [0.0, 0.0, 1.0, 0.25]));
            v.images.push((left, "GUI/Buttons/Button_Arrow01.png".into(), [0.0, 0.0, 0.25, 0.5]));
            v.images.push((right, "GUI/Buttons/Button_Arrow01.png".into(), [0.0, 0.5, 0.25, 1.0]));
            st.hits.push((left, Hit::Setting(i, -1)));
            st.hits.push((right, Hit::Setting(i, 1)));
        }
        // OptionText (136..386 at +12..+42) may run on to just before the left arrow (436); OptionValue's text is
        // centred in the red banner, clear of the arrows' rims. Both are one line that shrinks to fit.
        v.add([136.0, top + 12.0, 430.0, top + 42.0], label, label_style.clone());
        v.add([VALUE_TEXT_X[0], top + VALUE_BANNER[1], VALUE_TEXT_X[1], top + VALUE_BANNER[3]], value, value_style.clone());
    }
}
