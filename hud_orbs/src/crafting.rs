//! The crafting page: Wizard101's RecipeWindow.gui filled with Elden Ring's crafting recipes, and crafting itself.
//!
//! Recipes: every row of Elden Ring's ShopLineupParam_Recipe (its crafting menu), from book/er/er_recipes.tsv
//! (tools/er_recipes.py). A recipe is known once its cookbook flag (eventFlag_forRelease) is on, as in Elden Ring.
//!
//! Layout (RecipeWindow): the category tabs along the top (TabLayoutEquipment's checkboxes, each with the Wizard101
//! art closest to the Elden Ring category); the recipes as RecipeItem rows on the right page, Icon_Alchemy on those
//! that can be made now; on the left page the chosen recipe's name in the banner (labelItemHighlighted), its picture
//! in Art_CraftingPortrait, and its materials in IngredientLayout's 8 starred boxes (Background_Crafting_Icons01),
//! each with "have/need" on its plaque (GUI_00001120: "999/999"); Create under it; the "You made a" panel
//! (CraftingResults) after a craft. Reagent_Toggle switches to the materials page (pages.rs reagents, unchanged).
//!
//! Crafting goes through Elden Ring's own event system, never through game memory: the DLL only sets event flags.
//! One event per recipe in the mod's common.emevd (tools/emevd.py --crafting, the same TSV order) waits for REQUEST
//! with the recipe's number in INDEX's 8 flags (one flag per bit, each event tests all 8, so only one event answers);
//! it clears REQUEST, counts the materials with 2003[42] Store Item Amount Held in Event Value, checks them with
//! 3[12] IF Event Value (and the Crafting Kit with 3[4], the cookbook flag with 3[0], and room for the result), and
//! then removes the materials with 2003[24] Remove Item From Player and gives the result with 2003[43] Directly Give
//! Player Item (the amount: an event value counted up to the recipe's set size), and turns DONE on. If anything is
//! missing it turns FAILED (or FULL) on and changes nothing. The page checks the same things first.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::Instant;

use hudhook::imgui::ImColor32;

use crate::game;
use crate::layout::{Layout, TextStyle, View};
use crate::pages::{centre, grey, hide, icon, marker, paging, Data, State};

/// Event flags shared with tools/emevd.py (CRAFT_*): free flags of the mod's 1042362xxx block.
pub const REQUEST: u32 = 1042362170;
/// The recipe's number, bit k in flag INDEX + k (k = 0..8).
pub const INDEX: u32 = 1042362171;
pub const INDEX_BITS: u32 = 8;
pub const DONE: u32 = 1042362180;
pub const FAILED: u32 = 1042362181;
pub const FULL: u32 = 1042362182;
/// How long the page waits for the event before it gives up (the events aren't installed).
const ANSWER_SECONDS: f32 = 4.0;
/// How long the "You made a" panel stays.
const RESULT_SECONDS: f32 = 3.0;
/// Elden Ring's Crafting Kit (goods), needed to craft.
const CRAFTING_KIT: u32 = 8500;

/// The category tabs: (RecipeWindow checkbox, Wizard101 art, Elden Ring category). Tab n shows the recipes whose
/// er_recipes.tsv tab is n; tab 0 all.
pub const TABS: [(&str, &str, &str); 6] = [
    ("Tab_AllEquipment", "GUI/Buttons/Button_Equip_All.png", "All Recipes"),
    ("Tab_Hat", "GUI/Buttons/Button_Elixir_4S.png", "Pots and Aromatics"),
    ("Tab_Robe", "GUI/Shopping/Button_Equipment_Wands.png", "Greases"),
    ("Tab_Shoes", "GUI/Shopping/Button_Equipment_Athame.png", "Arrows and Bolts"),
    ("Tab_Weapon", "GUI/Buttons/Button_Crafting_Snacks.png", "Food and Medicine"),
    ("Tab_Athame", "GUI/Shopping/Button_Equipment_Treasure.png", "Tools and Throwables"),
];
/// RecipeWindow's other equipment tabs (no Elden Ring category).
const UNUSED_TABS: [&str; 5] = ["Tab_Amulet", "Tab_Ring", "Tab_Pet", "Tab_Deck", "Tab_Mount"];

const BLACK: ImColor32 = ImColor32::from_rgb(0, 0, 0);
/// Wizard101's button label yellow (FF80FFFF), the RecipeItem rows' text.
const LIGHT_YELLOW: ImColor32 = ImColor32::from_rgb(255, 255, 128);
/// A recipe whose cookbook isn't found yet.
const LOCKED: ImColor32 = ImColor32::from_rgb(170, 160, 140);

pub struct Recipe {
    /// Inventory item ID with category bits (goods 0x4000_0000 | id; arrows and bolts: the weapon ID).
    pub item: u32,
    /// How many one craft makes.
    pub set: u32,
    /// The most the player can carry.
    pub max: u32,
    /// The cookbook flag (0: known from the start).
    pub flag: u32,
    pub tab: usize,
    /// (goods ID, how many).
    pub materials: Vec<(u32, u32)>,
    pub cookbook: String,
    pub name: String,
}

static RECIPES: RwLock<Vec<Recipe>> = RwLock::new(Vec::new());
/// The book's Elden Ring folder (book/er), where the item icons are.
static ER_FOLDER: RwLock<Option<PathBuf>> = RwLock::new(None);
/// Each icon's square crop around its drawn part (None: no file or nothing drawn).
static CROPS: Mutex<Option<HashMap<String, Option<[f32; 4]>>>> = Mutex::new(None);

/// Reads book/er/er_recipes.tsv (tools/er_recipes.py).
pub fn load(folder: &Path) {
    *ER_FOLDER.write().unwrap() = Some(folder.to_path_buf());
    let text = std::fs::read_to_string(folder.join("er_recipes.tsv")).unwrap_or_default();
    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 11 {
            continue;
        }
        let num = |i: usize| f[i].parse::<u32>().unwrap_or(0);
        let id = num(3);
        let item = if f[2] == "G" { 0x4000_0000 | id } else { id };
        let materials = f[8].split(',').filter_map(|m| m.split_once(':'))
            .filter_map(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).collect();
        if num(0) as usize != out.len() {
            break; // rows out of order: the events wouldn't match
        }
        out.push(Recipe { item, set: num(4), max: num(5), flag: num(6), tab: num(7) as usize, materials,
                          cookbook: f[9].to_string(), name: f.get(10).unwrap_or(&"").to_string() });
    }
    tracing::info!("crafting: {} recipes", out.len());
    *RECIPES.write().unwrap() = out;
}

/// The crafting page's own state.
#[derive(Default)]
pub struct Craft {
    /// Showing the materials page (Reagent_Toggle) instead of the recipes.
    pub materials: bool,
    /// The chosen recipe (its number).
    pub recipe: usize,
    /// A craft sent to the event: (recipe, when).
    pending: Option<(usize, Instant)>,
    /// The last craft made: (recipe, when), for the "You made a" panel.
    made: Option<(usize, Instant)>,
    /// Create was clicked (acted on while binding, which has the inventory).
    create: bool,
    /// The recipes on the right page's rows this frame.
    shown: Vec<usize>,
}

impl Craft {
    /// Shows the "You made a" panel for `recipe` (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_made(&mut self, recipe: usize) {
        self.recipe = recipe;
        self.made = Some((recipe, Instant::now()));
    }
}

fn held(d: &Data, item: u32) -> u32 {
    d.sheet.map_or(0, |s| s.items.iter().chain(&s.key_items).filter(|(id, _)| *id == item).map(|(_, n)| *n).sum())
}

fn known(r: &Recipe) -> bool {
    r.flag == 0 || game::event_flag(r.flag) == Some(true)
}

fn has_kit(d: &Data) -> bool {
    held(d, 0x4000_0000 | CRAFTING_KIT) > 0
}

/// Why `r` can't be made now, or None.
fn blocker(d: &Data, r: &Recipe) -> Option<String> {
    if !known(r) {
        return Some(format!("Needs {}", if r.cookbook.is_empty() { "its cookbook" } else { &r.cookbook }));
    }
    if !has_kit(d) {
        return Some("Needs the Crafting Kit".into());
    }
    if let Some((m, n)) = r.materials.iter().find(|(m, n)| held(d, 0x4000_0000 | m) < *n) {
        return Some(format!("Not enough {} ({}/{n})", d.names.item(0x4000_0000 | m), held(d, 0x4000_0000 | m)));
    }
    if held(d, r.item) + r.set > r.max {
        return Some(format!("You can't carry more than {}", r.max));
    }
    None
}

/// What happened to the craft sent to the event, once it answers.
fn poll(d: &Data, st: &mut State) {
    let Some((recipe, sent)) = st.craft.pending else { return };
    let recipes = RECIPES.read().unwrap();
    let name = recipes.get(recipe).map(|r| result_name(d, r)).unwrap_or_default();
    if game::event_flag(REQUEST) == Some(true) {
        if sent.elapsed().as_secs_f32() > ANSWER_SECONDS {
            game::set_event_flag(REQUEST, false);
            st.craft.pending = None;
            tracing::warn!("crafting: no answer to recipe {recipe} (the crafting events aren't in common.emevd?)");
            st.note = Some(("Crafting didn't answer: the mod's crafting events aren't installed".into(), Instant::now()));
        }
        return;
    }
    st.craft.pending = None;
    let flag = |f: u32| game::event_flag(f) == Some(true);
    let text = if flag(DONE) {
        st.craft.made = Some((recipe, Instant::now()));
        format!("You made {name}")
    } else if flag(FULL) {
        format!("You can't carry more {name}")
    } else if flag(FAILED) {
        format!("Couldn't make {name}: something is missing")
    } else {
        format!("{name}: crafting didn't finish")
    };
    tracing::info!("crafting: recipe {recipe}: {text}");
    for f in [DONE, FAILED, FULL] {
        game::set_event_flag(f, false);
    }
    st.note = Some((text, Instant::now()));
}

/// Asks the recipe's event to make it (Create).
fn request(d: &Data, st: &mut State) {
    let recipes = RECIPES.read().unwrap();
    let Some(r) = recipes.get(st.craft.recipe) else { return };
    if st.craft.pending.is_some() || game::event_flag(REQUEST) == Some(true) {
        st.note = Some(("Still crafting...".into(), Instant::now()));
        return;
    }
    if let Some(why) = blocker(d, r) {
        st.note = Some((why, Instant::now()));
        return;
    }
    let index = st.craft.recipe as u32;
    for f in [DONE, FAILED, FULL] {
        game::set_event_flag(f, false);
    }
    // The number first, the request last: the event only looks at the number once REQUEST is on.
    for k in 0..INDEX_BITS {
        game::set_event_flag(INDEX + k, index >> k & 1 == 1);
    }
    game::set_event_flag(REQUEST, true);
    tracing::info!("crafting: request recipe {index} ({})", result_name(d, r));
    st.craft.pending = Some((st.craft.recipe, Instant::now()));
    st.craft.made = None;
}

/// A click on page node `name` (RecipeWindow); true when it was the crafting page's.
pub fn click(l: &Layout, id: usize, st: &mut State) -> bool {
    let name = l.nodes[id].name.as_str();
    match name {
        "Reagent_Toggle" | "Reagent_Toggle2" => {
            st.craft.materials = !st.craft.materials;
            st.page = 0;
            st.sub = 0;
        }
        _ if st.craft.materials => return false,
        "CreateButton" => st.craft.create = true,
        _ if name.starts_with("Item_") => {
            if let Ok(n) = name["Item_".len()..].parse::<usize>() {
                if let Some(r) = st.craft.shown.get(n.saturating_sub(1)) {
                    st.craft.recipe = *r;
                }
            }
        }
        _ => match TABS.iter().position(|t| t.0 == name) {
            Some(i) => (st.sub, st.page) = (i, 0),
            None => return false,
        },
    }
    true
}

/// Fills RecipeWindow: the materials page (pages.rs) or the recipes.
pub fn bind(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    st.set_subs(1);
    v.set(l.find("title"), centre("Crafting"));
    // RecipeWindow is a window of its own in Wizard101, with a full-size background that would cover the book's
    // tabs: draw it on the book's own page instead.
    if let Some(station) = l.find("StationBackground") {
        v.no_back.insert(station);
        if let Some(edge) = l.find_under(station, "Doodle") {
            v.show(Some(edge), false);
        }
    }
    // Exit and the furniture tab sit where the book's own tabs are (the book has its Close tab); no timers, card
    // crafting or equipping in Elden Ring.
    hide(l, v, &["BonusInfoIcon", "CompletedText", "UnequipButton", "TimerSlot_01", "Exit", "Tab_FurnitureStuff",
                 "EquipButton", "card_background", "TimerIcon", "StationSeal", "FiltersButton"]);
    poll(d, st);
    if st.craft.materials {
        v.set(l.find("Reagent_Toggle2"), centre("Recipes"));
        crate::pages::materials(l, d, st, v);
        return;
    }
    v.set(l.find("Reagent_Toggle2"), centre("Reagents"));
    hide(l, v, &UNUSED_TABS);
    recipes(l, d, st, v);
}

fn recipes(l: &Layout, d: &Data, st: &mut State, v: &mut View) {
    let all = RECIPES.read().unwrap();
    st.set_subs(TABS.len());
    let tab = st.sub % TABS.len();
    // The tabs: Wizard101's art, the chosen one checked, the tab marker under it.
    for (i, (node, art, label)) in TABS.iter().enumerate() {
        let Some(n) = l.find(node) else { continue };
        v.node_art.insert(n, art.to_string());
        let list: Vec<&Recipe> = all.iter().filter(|r| i == 0 || r.tab == i).collect();
        let known = list.iter().filter(|r| known(r)).count();
        v.plain_tip(l.nodes[n].rect, vec![label.to_string(), format!("{known} of {} recipes known", list.len())]);
        if i == tab {
            v.checked.insert(n);
        }
    }
    marker(l, v, "TabBackground", "Tab_AllEquipment", l.find(TABS[tab].0));
    v.tab_row = crate::pages::row_of(l, &TABS.map(|t| t.0));
    // What the pages add from here (texts, pictures, hover texts) would draw over the "You made a" panel.
    let page_layer = (v.extra.len(), v.images.len(), v.tips.len());

    // The recipes in the chosen tab, known ones first (Elden Ring's menu lists only those), then in
    // ShopLineupParam_Recipe order.
    let mut list: Vec<usize> = (0..all.len()).filter(|&i| tab == 0 || all[i].tab == tab).collect();
    list.sort_by_key(|&i| !known(&all[i]));
    if !list.contains(&st.craft.recipe) {
        st.craft.recipe = list.first().copied().unwrap_or(0);
    }
    let rows: Vec<usize> = (1..=8).filter_map(|i| l.find(&format!("Item_{i}"))).collect();
    let page = paging(l, v, st, list.len(), rows.len(), ("PageNumber", "leftscroll", "rightscroll"));
    st.craft.shown = list.iter().skip(page * rows.len()).take(rows.len()).copied().collect();
    for (i, row) in rows.iter().enumerate() {
        let r = l.nodes[*row].rect;
        // The equip fist isn't RecipeWindow's (the recipe rows only show Icon_Alchemy when it can be made).
        v.show(l.find_under(*row, "fist"), false);
        let Some(&index) = st.craft.shown.get(i) else {
            v.show(Some(*row), false);
            continue;
        };
        let recipe = &all[index];
        let name = result_name(d, recipe);
        let can = blocker(d, recipe).is_none();
        v.show(l.find_under(*row, "craftable"), can);
        if index == st.craft.recipe {
            v.checked.insert(*row);
        }
        let colour = if known(recipe) || index == st.craft.recipe { LIGHT_YELLOW } else { LOCKED };
        let label = if recipe.set > 1 { format!("{name} x{}", recipe.set) } else { name.clone() };
        v.add([r[0] + 22.0, r[1], r[2] - 26.0, r[3]], label, TextStyle::new("WizardFancy", 0.42, colour).centred());
        // Round 22 B1: no hover on the recipe rows (the ingredients are on the left page).
    }
    if list.is_empty() {
        v.add([419.0, 120.0, 718.0, 160.0], "No recipes", TextStyle::new("WizardFancy", 0.55, BLACK).centred());
    }

    // The left page: the chosen recipe.
    let Some(recipe) = all.get(st.craft.recipe) else { return };
    let name = result_name(d, recipe);
    v.set(l.find("labelItemHighlighted"), centre(&name));
    if let Some(t) = l.find("labelItemHighlighted") {
        let r = l.nodes[t].rect;
        v.rect.insert(t, [r[0] + 20.0, r[1], r[2] - 20.0, r[3]]); // inside the banner's frame
    }
    // The picture in the portrait frame's starry window (3DCharacter's place).
    if let Some(window) = l.find("3DCharacter") {
        let w = l.nodes[window].rect;
        let side = (w[2] - w[0]).min(w[3] - w[1]) - 24.0;
        let (cx, cy) = ((w[0] + w[2]) / 2.0, (w[1] + w[3]) / 2.0);
        let pic = [cx - side / 2.0, cy - side / 2.0, cx + side / 2.0, cy + side / 2.0];
        icon(v, d.items, recipe.item, pic);
        let mut tip = d.items.tooltip(recipe.item, held(d, recipe.item).max(1), d.names, false);
        tip.extend(recipe_lines(d, recipe));
        v.tip(w, tip);
        if recipe.set > 1 {
            v.add([w[2] - 60.0, w[3] - 26.0, w[2] - 4.0, w[3] - 2.0], format!("x{}", recipe.set),
                  TextStyle::new("WizardFancy", 0.5, LIGHT_YELLOW).right().outlined());
        }
    }
    // The materials in the 8 starred boxes, "have/need" on each plaque.
    for slot in 0..8 {
        let Some(n) = l.find(&format!("Ingredient0{}", slot + 1)) else { continue };
        let num = l.find_under(n, "IngredientNumber");
        let plaque = num.and_then(|t| l.nodes[t].parent);
        match recipe.materials.get(slot) {
            Some(&(m, need)) => {
                let goods = 0x4000_0000 | m;
                let have = held(d, goods);
                // Round 17 D1: the material in Wizard101's own square box (IngredientLayout's art), inside its
                // gold border and above the have/need plaque; no round medallion.
                if let Some(layout) = l.find("IngredientLayout") {
                    ingredient_picture(v, d.items, goods, l.nodes[layout].rect, slot);
                }
                v.set(num, centre(&format!("{have}/{need}")));
                // Round 22 B2: no hover on the reagents (have/need is on the plaque).
            }
            None => {
                v.show(plaque, false);
            }
        }
    }
    // Create: greyed while the recipe can't be made, or a craft is on its way.
    if blocker(d, recipe).is_some() || st.craft.pending.is_some() {
        grey(l, v, &["CreateButton"]);
    }
    if let Some(b) = l.find("CreateButton") {
        let why = blocker(d, recipe).unwrap_or_else(|| format!("Make {}", if recipe.set > 1 {
            format!("{} {name}", recipe.set)
        } else {
            name.clone()
        }));
        v.plain_tip(l.nodes[b].rect, vec![why]);
    }
    if std::mem::take(&mut st.craft.create) {
        drop(all);
        request(d, st);
        return;
    }
    // "You made a": RecipeWindow's results panel, for a few seconds after a craft.
    if let Some((made, at)) = st.craft.made {
        if at.elapsed().as_secs_f32() > RESULT_SECONDS {
            st.craft.made = None;
        } else if let Some(r) = all.get(made) {
            v.show(l.find("CraftingResultsBackground"), true);
            v.extra.truncate(page_layer.0);
            v.images.truncate(page_layer.1);
            v.tips.truncate(page_layer.2);
            hide(l, v, &["IngredientBackground"]); // its plaques' numbers draw after the nodes
            let what = result_name(d, r);
            let what = if r.set > 1 { format!("{} x{}", what, r.set) } else { what };
            v.set(l.find("createdItemText"), centre(&what));
        }
    }
}

fn recipe_lines(d: &Data, r: &Recipe) -> Vec<String> {
    let mut lines = Vec::new();
    if r.set > 1 {
        lines.push(format!("Makes {}", r.set));
    }
    for (m, n) in &r.materials {
        let goods = 0x4000_0000 | m;
        lines.push(format!("{} {}/{n}", d.names.item(goods), held(d, goods)));
    }
    match (known(r), r.cookbook.is_empty()) {
        (true, true) => lines.push("Known from the start".into()),
        (true, false) => lines.push(format!("From {}", r.cookbook)),
        (false, _) => lines.push(format!("Needs {}", if r.cookbook.is_empty() { "its cookbook" } else { &r.cookbook })),
    }
    if let Some(why) = blocker(d, r).filter(|_| known(r)) {
        lines.push(why);
    }
    lines
}

/// The part of an icon (UV) to draw so that its picture fills a square box: Elden Ring's item icons are drawn
/// inside a 128 x 128 square with see-through margins of their own (some objects take a third of it), while
/// Wizard101's reagent and ingredient pictures fill their box. The crop is the smallest square around the pixels
/// that are drawn (alpha above 16), centred on them and kept inside the picture. Worked out once per icon from its
/// file.
fn crop(art: &str) -> Option<[f32; 4]> {
    let rel = art.strip_prefix("@er/")?;
    let mut cache = CROPS.lock().unwrap();
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(c) = cache.get(art) {
        return *c;
    }
    let path = ER_FOLDER.read().unwrap().as_ref()?.join(rel);
    let c = image::open(&path).ok().and_then(|img| {
        let img = img.into_rgba8();
        let (w, h) = img.dimensions();
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        for (x, y, p) in img.enumerate_pixels() {
            if p[3] > 16 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let side = (x1 - x0).max(y1 - y0).min(w).min(h) as f32;
        let (cx, cy) = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
        let left = (cx - side / 2.0).clamp(0.0, w as f32 - side);
        let top = (cy - side / 2.0).clamp(0.0, h as f32 - side);
        Some([left / w as f32, top / h as f32, (left + side) / w as f32, (top + side) / h as f32])
    });
    cache.insert(art.to_string(), c);
    c
}

/// Wizard101's blank reagent medallion (GUI/Reagents/Resource_Nothing): the gold ring and parchment disc every
/// Wizard101 reagent picture (Resource_Black_Lotus, Resource_Bone, ...) is drawn on, filling its whole square.
pub const MEDALLION: &str = "GUI/Reagents/Resource_Nothing.png";

/// The parchment spiral of Wizard101's reagent pictures (Resource_Nothing's disc inside the gold ring: centre 64,64,
/// the ring's black inner edge 15 px in, so a radius of 48 is clear parchment) as a rectangle for `r`: the largest
/// part of the disc with `r`'s shape, centred on the spiral, so nothing of the ring shows. Ryan round 18: the spell
/// cards' picture window and the recipe's ingredient boxes use it behind their pictures, like the reagent cards.
pub(crate) fn parchment_spiral(v: &mut View, r: [f32; 4]) {
    const CENTRE: f32 = 64.0;
    const RADIUS: f32 = 48.0;
    let (w, h) = ((r[2] - r[0]).max(1.0), (r[3] - r[1]).max(1.0));
    let diag = (w * w + h * h).sqrt();
    let (hw, hh) = (RADIUS * w / diag, RADIUS * h / diag);
    let uv = [(CENTRE - hw) / 128.0, (CENTRE - hh) / 128.0, (CENTRE + hw) / 128.0, (CENTRE + hh) / 128.0];
    v.images.push((r, MEDALLION.into(), uv));
}

/// A material's picture as Wizard101 draws a reagent's (GUI/Reagents/Resource_*: 128 x 128, the medallion filling
/// the square, the object filling the medallion's window): the medallion over the whole square `r`, the item's
/// icon cropped to its drawn part (see `crop`) on it, its square `fill` of the side: 0.75 fills the window inside
/// the ring (Resource_Nothing's ring ends 13 of its 128 pixels in), 1.0 the whole square (the medallion then only
/// shows where the object leaves gaps).
pub(crate) fn reagent_picture(v: &mut View, items: &crate::itemdata::ItemData, item: u32, r: [f32; 4], fill: f32) {
    v.images.push((r, MEDALLION.into(), [0.0, 0.0, 1.0, 1.0]));
    let Some(art) = items.icon(item) else { return };
    let uv = crop(&art).unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let (cx, cy, half) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0, (r[2] - r[0]).min(r[3] - r[1]) * fill / 2.0);
    v.images.push(([cx - half, cy - half, cx + half, cy + half], art, uv));
}

/// IngredientLayout's background (RecipeWindow.gui, Background_Crafting_Icons01): Wizard101's eight square
/// ingredient boxes, 4 x 2 cells of 64 x 64 in its 256 x 128 picture, the picture stretched over the layout's
/// rectangle.
const INGREDIENT_ART: [f32; 2] = [256.0, 128.0];
/// In each 64 x 64 cell (measured on the art): the blue window inside the gold border is x 11..53, y 7..43; the plaque
/// (Art_Crafting_MiniPlaque02, RecipeWindow's IngredientNumber parent) covers the box's bottom from y 42 on.
const INGREDIENT_WINDOW: [f32; 4] = [11.0, 7.0, 53.0, 42.0];
/// Space between the picture and the border (page pixels).
const INGREDIENT_MARGIN: f32 = 2.0;

/// A material in ingredient box `slot` (0..8) of IngredientLayout (`layout`, its page rectangle): the reagent
/// cards' parchment spiral over the box's blue window and its star, and the item's icon (cropped to its drawn part, see `crop`) as large as fits inside the
/// blue window, clear of the gold border and of the have/need plaque under it.
fn ingredient_picture(v: &mut View, items: &crate::itemdata::ItemData, item: u32, layout: [f32; 4], slot: usize) {
    let (sx, sy) = ((layout[2] - layout[0]) / INGREDIENT_ART[0], (layout[3] - layout[1]) / INGREDIENT_ART[1]);
    let (cx, cy) = ((slot % 4) as f32 * 64.0, (slot / 4) as f32 * 64.0);
    let page = |p: [f32; 4]| [layout[0] + (cx + p[0]) * sx, layout[1] + (cy + p[1]) * sy,
                              layout[0] + (cx + p[2]) * sx, layout[1] + (cy + p[3]) * sy];
    let w = page(INGREDIENT_WINDOW);
    // The reagent cards' parchment spiral over the box's blue and its star (Ryan round 18, D1).
    parchment_spiral(v, w);
    let Some(art) = items.icon(item) else { return };
    let uv = crop(&art).unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let side = (w[2] - w[0]).min(w[3] - w[1]) - 2.0 * INGREDIENT_MARGIN;
    let (mx, my) = ((w[0] + w[2]) / 2.0, (w[1] + w[3]) / 2.0);
    v.images.push(([mx - side / 2.0, my - side / 2.0, mx + side / 2.0, my + side / 2.0], art, uv));
}

/// The result's name (er_item_names.tsv; er_recipes.tsv's own copy if that has none).
fn result_name(d: &Data, r: &Recipe) -> String {
    let name = d.names.item(r.item);
    if name.is_empty() || name.starts_with('#') { r.name.clone() } else { name }
}
