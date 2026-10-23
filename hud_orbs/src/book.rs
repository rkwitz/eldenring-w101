//! Wizard101's spellbook as an Elden Ring menu: SpellBookPrefsWindow.gui's 800x600 book (Deck_Background) with its
//! tab buttons down the right edge, pages filled with what Elden Ring has that is closest to each Wizard101 page.
//!
//! Open and close with B on the keyboard or View on the controller (instead of Elden Ring's map), close with Esc or
//! the controller's B too. Tabs: click, Up/Down, D-pad up/down or LB/RB; list pages: the arrows, Left/Right, D-pad
//! left/right or the mouse wheel. The World Map tab opens Elden Ring's own map and comes back to the book when the map
//! closes. While the book is open the game gets no controller, keyboard or mouse input (see input.rs).

use std::path::Path;
use std::time::Instant;

use hudhook::imgui::{DrawListMut, ImColor32, Key, MouseButton, TextureId, Ui};
use hudhook::RenderContext;

use crate::font::BitmapFont;
use crate::game::{self, Sheet};
use crate::input;
use crate::names::Names;

/// The book window (SpellBookPrefsWindow.gui DeckConfiguration 192,4-992,604, centred) in Wizard101 pixels.
const BOOK: [f32; 2] = [800.0, 600.0];
/// Wizard101's screen height; the book scales with the screen like the HUD.
const SCREEN: f32 = 768.0;
/// Tab buttons: x 746-786, 40x37 each, in SpellBookPrefsWindow.gui order and positions.
const TAB_X: f32 = 746.0;
const TAB_SIZE: [f32; 2] = [40.0, 37.0];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
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

/// (tab, button art, top edge, title)
const TABS: [(Tab, &str, f32, &str); 10] = [
    (Tab::Character, "Button_Tab_Character", 19.0, "Character"),
    (Tab::Backpack, "Button_Tab_Inventory", 76.0, "Backpack"),
    (Tab::Pets, "Button_Tab_Pet", 132.0, "Pets"),
    (Tab::Deck, "Button_Tab_DeckBuild", 189.0, "Deck"),
    (Tab::Quests, "Button_Tab_Quest", 247.0, "Quests"),
    (Tab::Map, "Button_Tab_WorldMap", 304.0, "World Map"),
    (Tab::Crafting, "Button_Tab_Flask", 363.0, "Crafting"),
    (Tab::Help, "Button_Tab_Help", 423.0, "Help"),
    (Tab::Settings, "Button_Tab_Options", 483.0, "Settings"),
    (Tab::Close, "Button_Exit", 538.0, "Close"),
];

/// Text areas of the left and right pages (x0, y0, x1, y1), inside the page borders of Deck_Background.
const LEFT_PAGE: [f32; 4] = [105.0, 38.0, 345.0, 556.0];
const RIGHT_PAGE: [f32; 4] = [438.0, 38.0, 678.0, 556.0];
const LINE: f32 = 19.0;
const INK: ImColor32 = ImColor32::from_rgb(58, 36, 18);
const TITLE_INK: ImColor32 = ImColor32::from_rgb(118, 28, 20);
const FAINT_INK: ImColor32 = ImColor32::from_rgb(120, 96, 70);

struct Art {
    background: TextureId,
    tabs: Vec<TextureId>,
    previous: TextureId,
    next: TextureId,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum MapState {
    Idle,
    /// The map press was sent; waiting for the game's HUD to give way to the map.
    Opening(Instant),
    Open,
}

pub struct Book {
    pub open: bool,
    /// Keep the game's input blocked after closing until the controller buttons are released.
    holding: bool,
    closed_at: Option<Instant>,
    tab: usize,
    page: usize,
    map: MapState,
    art: Option<Art>,
    text: Option<BitmapFont>,
    names: Option<Names>,
    sheet: Option<Sheet>,
    sheet_time: Option<Instant>,
    error: Option<String>,
}

/// What the book needs from the rest of the overlay each frame.
pub struct Context<'a> {
    pub game: Option<&'a game::Game>,
    pub stats: Option<game::Stats>,
    /// Normal play: ER's HUD in its default state, no fade, a character loaded.
    pub gameplay: bool,
    /// ER's HUD is in its default state (false while ER's map or menus are open).
    pub hud: bool,
    pub fancy: Option<&'a BitmapFont>,
    pub folder: &'a Path,
    pub settings: Vec<(String, String)>,
}

impl Book {
    pub fn new() -> Book {
        Book {
            open: false,
            holding: false,
            closed_at: None,
            tab: 0,
            page: 0,
            map: MapState::Idle,
            art: None,
            text: None,
            names: None,
            sheet: None,
            sheet_time: None,
            error: None,
        }
    }

    pub fn load(&mut self, folder: &Path, render: &mut dyn RenderContext) {
        let mut load = |name: &str| -> Result<TextureId, String> {
            let path = folder.join(format!("{name}.png"));
            let image = image::open(&path).map_err(|e| format!("{}: {e}", path.display()))?.into_rgba8();
            render.load_texture(&image, image.width(), image.height()).map_err(|e| format!("{name}: {e:?}"))
        };
        let art = (|| {
            Ok::<_, String>(Art {
                background: load("Deck_Background")?,
                tabs: TABS.iter().map(|(_, art, _, _)| load(art)).collect::<Result<_, _>>()?,
                previous: load("Button_PagePrevious")?,
                next: load("Button_PageNext")?,
            })
        })();
        match art {
            Ok(art) => self.art = Some(art),
            Err(e) => self.error = Some(e),
        }
        match BitmapFont::load(folder, "WizardNormal", render) {
            Ok(font) => self.text = Some(font),
            Err(e) => {
                self.error.get_or_insert(e);
            }
        };
        if let Some(e) = &self.error {
            tracing::warn!("spellbook art not loaded, the book is off: {e}");
        }
    }

    /// Whether the game's keyboard, mouse and controller input are blocked.
    pub fn blocking(&self) -> bool {
        self.open || self.holding
    }

    /// Handles input and the map round trip, then draws the book if it is open.
    pub fn frame(&mut self, ui: &Ui, cx: &Context) {
        if self.art.is_none() || self.text.is_none() {
            input::take_pressed();
            input::set_gameplay(false);
            return;
        }
        input::set_gameplay(cx.gameplay && !self.open);
        // Before taking the presses, so that the press that closed ER's map doesn't close the reopened book.
        self.follow_map(cx);
        let pressed = input::take_pressed();
        if !self.open {
            let quiet = self.closed_at.is_none_or(|t| t.elapsed().as_secs_f32() > 0.25);
            if self.holding && quiet && input::held().0 == 0 {
                self.holding = false;
            }
            let key_b = ui.is_key_pressed_no_repeat(Key::B);
            if cx.gameplay && self.map == MapState::Idle && (pressed & input::BACK != 0 || key_b) {
                self.set_open(true);
            }
            input::set_book_open(self.blocking());
            return;
        }
        if !cx.gameplay {
            self.set_open(false);
            return;
        }
        self.refresh(cx);
        let [width, height] = ui.io().display_size;
        let s = height / SCREEN;
        let origin = [(width - BOOK[0] * s) / 2.0, (height - BOOK[1] * s) / 2.0];
        let to_book = |p: [f32; 2]| [(p[0] - origin[0]) / s, (p[1] - origin[1]) / s];
        let mouse = to_book(ui.io().mouse_pos);
        let clicked = ui.is_mouse_clicked(MouseButton::Left);

        // Closing: Esc, B (keyboard or controller), View, or the Close tab.
        if pressed & (input::B | input::BACK) != 0
            || ui.is_key_pressed_no_repeat(Key::Escape)
            || ui.is_key_pressed_no_repeat(Key::B)
        {
            self.set_open(false);
            return;
        }
        let last = TABS.len() - 2; // Settings; Close is only for the mouse
        if pressed & (input::DPAD_UP | input::LB) != 0 || ui.is_key_pressed(Key::UpArrow) {
            self.select(if self.tab == 0 { last } else { (self.tab - 1).min(last) }, false);
        }
        if pressed & (input::DPAD_DOWN | input::RB) != 0 || ui.is_key_pressed(Key::DownArrow) {
            self.select(if self.tab >= last { 0 } else { self.tab + 1 }, false);
        }
        let wheel = ui.io().mouse_wheel;
        if pressed & input::DPAD_LEFT != 0 || ui.is_key_pressed(Key::LeftArrow) || wheel > 0.0 {
            self.page = self.page.saturating_sub(1);
        }
        if pressed & input::DPAD_RIGHT != 0 || ui.is_key_pressed(Key::RightArrow) || wheel < 0.0 {
            self.page += 1;
        }
        let activate = pressed & input::A != 0 || ui.is_key_pressed_no_repeat(Key::Enter);
        let mut hovered_tab = None;
        for (i, (_, _, top, _)) in TABS.iter().enumerate() {
            if inside(mouse, [TAB_X, *top, TAB_X + TAB_SIZE[0], top + TAB_SIZE[1]]) {
                hovered_tab = Some(i);
                if clicked {
                    self.select(i, true);
                }
            }
        }
        if activate {
            self.select(self.tab, true);
        }
        if !self.open {
            return;
        }
        let page_buttons = self.page_buttons();
        if clicked && inside(mouse, page_buttons.0) {
            self.page = self.page.saturating_sub(1);
        }
        if clicked && inside(mouse, page_buttons.1) {
            self.page += 1;
        }
        if TABS[self.tab].0 == Tab::Map && clicked && inside(mouse, MAP_BUTTON) {
            self.select(self.tab, true);
            return;
        }
        let draw = ui.get_foreground_draw_list();
        self.page = self.draw(&draw, cx, origin, s, hovered_tab, mouse);
    }

    fn set_open(&mut self, open: bool) {
        if open == self.open {
            return;
        }
        self.open = open;
        input::take_pressed();
        if open {
            self.page = 0;
            self.sheet_time = None;
        } else {
            self.holding = true;
            self.closed_at = Some(Instant::now());
        }
        input::set_book_open(self.blocking());
    }

    /// Shows tab `i`; `activate` also does what the tab does (World Map opens ER's map, Close closes the book).
    fn select(&mut self, i: usize, activate: bool) {
        if i != self.tab {
            self.page = 0;
        }
        self.tab = i;
        match TABS[i].0 {
            Tab::Close if activate => self.set_open(false),
            Tab::Map if activate => {
                self.set_open(false);
                self.map = MapState::Opening(Instant::now());
                input::set_gameplay(false);
                input::set_book_open(false);
                self.holding = false;
                input::press_map();
                tracing::info!("spellbook: opening Elden Ring's map (controller: {})", input::pad_connected());
            }
            _ => {}
        }
    }

    /// The map round trip: wait for ER's HUD to leave (map open), then reopen the book when it comes back.
    fn follow_map(&mut self, cx: &Context) {
        match self.map {
            MapState::Opening(_) if !cx.hud => self.map = MapState::Open,
            MapState::Opening(since) if since.elapsed().as_secs_f32() > 4.0 => {
                tracing::warn!("spellbook: Elden Ring's map didn't open");
                self.map = MapState::Idle;
            }
            MapState::Open if cx.gameplay => {
                self.map = MapState::Idle;
                self.set_open(true);
                self.tab = TABS.iter().position(|t| t.0 == Tab::Map).unwrap_or(0);
            }
            _ => {}
        }
    }

    fn refresh(&mut self, cx: &Context) {
        if self.names.as_ref().is_none_or(|n| n.is_empty()) {
            self.names = Some(Names::load(&cx.folder.join("er_item_names.tsv")));
        }
        if self.sheet_time.is_none_or(|t| t.elapsed().as_secs_f32() > 0.5) {
            self.sheet = cx.game.and_then(|g| g.sheet());
            self.sheet_time = Some(Instant::now());
        }
    }

    /// The previous and next page buttons at the bottom of the right page.
    fn page_buttons(&self) -> ([f32; 4], [f32; 4]) {
        let y = RIGHT_PAGE[3] - 8.0;
        ([RIGHT_PAGE[0], y, RIGHT_PAGE[0] + 26.0, y + 26.0], [RIGHT_PAGE[2] - 26.0, y, RIGHT_PAGE[2], y + 26.0])
    }

    /// Draws the open book; returns the list page, clamped to the pages there are.
    fn draw(&self, draw: &DrawListMut, cx: &Context, origin: [f32; 2], s: f32, hovered: Option<usize>, mouse: [f32; 2]) -> usize {
        let (Some(art), Some(text)) = (&self.art, &self.text) else { return 0 };
        let mut page = self.page;
        let p = |x: f32, y: f32| [origin[0] + x * s, origin[1] + y * s];
        draw.add_image(art.background, p(0.0, 0.0), p(BOOK[0] + 1.0, BOOK[1])).build();
        for (i, (_, _, top, _)) in TABS.iter().enumerate() {
            let lit = hovered == Some(i) || self.tab == i;
            let (u0, u1) = if lit { (0.5, 1.0) } else { (0.0, 0.5) };
            draw.add_image(art.tabs[i], p(TAB_X, *top), p(TAB_X + TAB_SIZE[0], top + TAB_SIZE[1]))
                .uv_min([u0, 0.0])
                .uv_max([u1, 1.0])
                .build();
        }
        let pen = Pen { draw, text, fancy: cx.fancy, origin, s };
        let mut left = Column::new(LEFT_PAGE);
        let mut right = Column::new(RIGHT_PAGE);
        let tab = TABS[self.tab].0;
        pen.title(&mut left, TABS[self.tab].3);
        let names = self.names.as_ref();
        let have_names = names.is_some_and(|n| !n.is_empty());
        let none = Names::default();
        let names = names.unwrap_or(&none);
        let mut list: Option<(&str, Vec<(String, String)>)> = None;
        match (tab, &self.sheet) {
            (Tab::Map, _) => {
                pen.para(&mut left, "Elden Ring's own map opens when you choose this tab, with everything it can \
                    do: markers, grace travel, zoom. Close it as usual and the book comes back here.");
                left.gap(10.0);
                let lit = inside(mouse, MAP_BUTTON);
                pen.line(&mut left, "> Open the map again (A / Enter / click)", if lit { TITLE_INK } else { INK });
                pen.title(&mut right, "Wizard101");
                pen.para(&mut right, "In Wizard101 this page shows the world map of the area you are in.");
            }
            (Tab::Help, _) => {
                for (key, what) in HELP {
                    if key.is_empty() {
                        left.gap(8.0);
                        pen.line(&mut left, what, TITLE_INK);
                    } else {
                        pen.pair(&mut left, key, what);
                    }
                }
                pen.title(&mut right, "About");
                pen.para(&mut right, "Wizard101's spellbook for Elden Ring: each page shows the closest thing \
                    Elden Ring has. Pages marked 'not yet' are placeholders.");
            }
            (Tab::Settings, _) => {
                for (key, value) in &cx.settings {
                    pen.pair(&mut left, key, value);
                }
                left.gap(10.0);
                pen.para(&mut left, "Change these in w101_orbs.ini next to w101_orbs.dll and restart the game.");
                pen.title(&mut right, "Not yet");
                pen.para(&mut right, "Wizard101's settings page (sound, graphics, chat) has no counterpart here; \
                    use Elden Ring's own system menu.");
            }
            (Tab::Quests, _) => {
                pen.para(&mut left, "Elden Ring keeps no quest log, so this page has nothing to show yet.");
                left.gap(10.0);
                pen.line(&mut left, "Not yet: NPC questlines from the game's event flags.", FAINT_INK);
                pen.title(&mut right, "Not yet");
            }
            (Tab::Crafting, Some(sheet)) => {
                pen.para(&mut left, "Elden Ring's crafting kit and cookbooks.");
                left.gap(8.0);
                let kit = sheet.key_items.iter().any(|(id, _)| names.item(*id) == "Crafting Kit");
                pen.pair(&mut left, "Crafting Kit", if kit { "yes" } else { "not found" });
                let books = sheet.key_items.iter().filter(|(id, _)| names.item(*id).contains("Cookbook")).count();
                pen.pair(&mut left, "Cookbooks", &books.to_string());
                left.gap(8.0);
                pen.line(&mut left, "Not yet: recipes and crafting from the book.", FAINT_INK);
                let rows = materials(sheet, names);
                list = Some(("Materials", rows));
            }
            (_, None) => pen.para(&mut left, "No character loaded."),
            (Tab::Character, Some(sheet)) => character(&pen, &mut left, &mut right, sheet, cx.stats),
            (Tab::Backpack, Some(sheet)) => {
                equipped(&pen, &mut left, sheet, names);
                let mut rows: Vec<(String, String)> = sheet
                    .items
                    .iter()
                    .map(|(id, n)| (names.item(*id), if *n > 1 { format!("x{n}") } else { String::new() }))
                    .collect();
                rows.sort();
                list = Some(("Backpack", rows));
            }
            (Tab::Pets, Some(sheet)) => {
                let torrent = sheet.key_items.iter().any(|(id, _)| names.item(*id) == "Spectral Steed Whistle");
                pen.pair(&mut left, "Torrent", if torrent { "summonable" } else { "not yet met" });
                left.gap(8.0);
                pen.para(&mut left, "Wizard101's pets are closest to Elden Ring's spirit ashes, listed on the \
                    right. Not yet: pet stats, talents and training.");
                let mut rows: Vec<(String, String)> = sheet
                    .items
                    .iter()
                    .chain(&sheet.key_items)
                    .map(|(id, _)| names.item(*id))
                    .filter(|n| n.contains("Ashes") || n.contains("Puppet"))
                    .map(|n| (n, String::new()))
                    .collect();
                rows.sort();
                rows.dedup();
                list = Some(("Spirit Ashes", rows));
            }
            (Tab::Deck, Some(sheet)) => {
                pen.line(&mut left, &format!("Memory slots: {}", sheet.magic_slots), INK);
                left.gap(4.0);
                for (i, spell) in sheet.spells.iter().take(sheet.magic_slots.max(1) as usize).enumerate() {
                    let name = if *spell < 0 { "(empty)".to_string() } else { names.spell(*spell) };
                    pen.pair(&mut left, &format!("{}", i + 1), &name);
                }
                let mut rows: Vec<(String, String)> = sheet
                    .items
                    .iter()
                    .filter(|(id, _)| is_spell(*id))
                    .map(|(id, _)| (names.item(*id), String::new()))
                    .collect();
                rows.sort();
                list = Some(("Spells known", rows));
            }
            (Tab::Close, _) => {}
        }
        if !have_names && matches!(tab, Tab::Backpack | Tab::Deck | Tab::Pets | Tab::Crafting) {
            left.gap(10.0);
            pen.para(&mut left, "Item names are missing: run tools/er_item_names.py to make er_item_names.tsv.");
        }
        if let Some((title, rows)) = list {
            pen.title(&mut right, title);
            let per_page = (((right.bottom - 34.0) - right.y) / LINE).floor().max(1.0) as usize;
            let pages = rows.len().div_ceil(per_page).max(1);
            page = page.min(pages - 1);
            if rows.is_empty() {
                pen.line(&mut right, "(none)", FAINT_INK);
            }
            for (name, value) in rows.iter().skip(page * per_page).take(per_page) {
                pen.pair(&mut right, name, value);
            }
            let (previous, next) = self.page_buttons();
            if page > 0 {
                draw.add_image(art.previous, p(previous[0], previous[1]), p(previous[2], previous[3])).build();
            }
            if page + 1 < pages {
                draw.add_image(art.next, p(next[0], next[1]), p(next[2], next[3])).build();
            }
            let label = format!("{} / {}", page + 1, pages);
            let centre = [(RIGHT_PAGE[0] + RIGHT_PAGE[2]) / 2.0, previous[1] + 13.0];
            pen.centred(&label, centre, FAINT_INK);
            page
        } else {
            0
        }
    }
}

const MAP_BUTTON: [f32; 4] = [LEFT_PAGE[0], 160.0, LEFT_PAGE[2], 200.0];

const HELP: [(&str, &str); 14] = [
    ("", "Opening the book"),
    ("B key", "open / close"),
    ("View button", "open / close"),
    ("Esc, pad B", "close"),
    ("", "Tabs"),
    ("Click", "choose a tab"),
    ("Up / Down", "previous / next tab"),
    ("D-pad, LB/RB", "previous / next tab"),
    ("Enter, pad A", "use the tab"),
    ("", "Lists"),
    ("Left / Right", "previous / next page"),
    ("D-pad left/right", "previous / next page"),
    ("Wheel, arrows", "previous / next page"),
    ("", "The map: World Map tab"),
];

fn character(pen: &Pen, left: &mut Column, right: &mut Column, sheet: &Sheet, stats: Option<game::Stats>) {
    if !sheet.name.is_empty() {
        pen.line(left, &sheet.name, TITLE_INK);
    }
    pen.pair(left, "Level", &sheet.level.to_string());
    pen.pair(left, "Runes held", &sheet.runes.to_string());
    if sheet.level < 713 {
        pen.pair(left, "Next level", &game::level_up_cost(sheet.level).to_string());
    }
    left.gap(8.0);
    if let Some(stats) = stats {
        pen.pair(left, "Health", &format!("{} / {}", stats.hp, stats.max_hp));
        pen.pair(left, "Mana (FP)", &format!("{} / {}", stats.fp, stats.max_fp));
        pen.pair(left, "Energy (stamina)", &format!("{} / {}", stats.sp, stats.max_sp));
    }
    pen.pair(left, "Crimson Tears", &sheet.flasks[0].to_string());
    pen.pair(left, "Cerulean Tears", &sheet.flasks[1].to_string());
    pen.title(right, "Attributes");
    for (name, value) in
        ["Vigor", "Mind", "Endurance", "Strength", "Dexterity", "Intelligence", "Faith", "Arcane"].iter().zip(sheet.attributes)
    {
        pen.pair(right, name, &value.to_string());
    }
    right.gap(8.0);
    pen.title(right, "Resistance");
    for (name, value) in ["Immunity", "Robustness", "Focus", "Vitality"].iter().zip(sheet.resistances) {
        pen.pair(right, name, &value.to_string());
    }
    pen.pair(right, "Poise", &format!("{:.0}", sheet.poise));
    pen.pair(right, "Max equip load", &format!("{:.1}", sheet.max_equip_load));
}

fn equipped(pen: &Pen, left: &mut Column, sheet: &Sheet, names: &Names) {
    pen.line(left, "Equipped", TITLE_INK);
    let e = &sheet.equipment;
    let weapon = |id: i32| if id <= 0 || id == 110000 { "-".to_string() } else { names.weapon(id) };
    for (label, slot) in [("Right 1", 1), ("Right 2", 3), ("Right 3", 5), ("Left 1", 0), ("Left 2", 2), ("Left 3", 4)] {
        pen.pair(left, label, &weapon(e[slot]));
    }
    left.gap(4.0);
    for (label, slot) in [("Head", 12), ("Chest", 13), ("Arms", 14), ("Legs", 15)] {
        let id = e[slot];
        let name = if id < 0 { "-".to_string() } else { names.find('P', id as u32).unwrap_or("-").to_string() };
        pen.pair(left, label, &name);
    }
    left.gap(4.0);
    for (i, slot) in (17..=20).enumerate() {
        let id = e[slot];
        let name = if id <= 0 { "-".to_string() } else { names.lookup('A', id as u32) };
        pen.pair(left, &format!("Talisman {}", i + 1), &name);
    }
    left.gap(4.0);
    for (label, slot) in [("Arrows", 6), ("Bolts", 7)] {
        pen.pair(left, label, &weapon(e[slot]));
    }
}

/// Sorceries and incantations (goods 4000-7999, and the DLC's 2_004_000-2_007_999).
fn is_spell(item_id: u32) -> bool {
    let raw = item_id & 0x0FFF_FFFF;
    item_id >> 28 == 4 && ((4000..8000).contains(&raw) || (2_004_000..2_008_000).contains(&raw))
}

/// Crafting materials: goods 15000-21999 (base game) and 2_015_000-2_021_999 (DLC).
fn materials(sheet: &Sheet, names: &Names) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = sheet
        .items
        .iter()
        .filter(|(id, _)| {
            let raw = id & 0x0FFF_FFFF;
            id >> 28 == 4 && ((15000..22000).contains(&raw) || (2_015_000..2_022_000).contains(&raw))
        })
        .map(|(id, n)| (names.item(*id), format!("x{n}")))
        .collect();
    rows.sort();
    rows
}

fn inside(p: [f32; 2], r: [f32; 4]) -> bool {
    p[0] >= r[0] && p[0] < r[2] && p[1] >= r[1] && p[1] < r[3]
}

/// A column of text on a page, in book pixels.
struct Column {
    x: f32,
    y: f32,
    right: f32,
    bottom: f32,
}

impl Column {
    fn new(r: [f32; 4]) -> Column {
        Column { x: r[0], y: r[1], right: r[2], bottom: r[3] }
    }

    fn gap(&mut self, h: f32) {
        self.y += h;
    }

    fn full(&self) -> bool {
        self.y + LINE > self.bottom
    }
}

/// Draws text into columns: WizardNormal for text, WizardFancy for titles.
struct Pen<'a, 'b> {
    draw: &'a DrawListMut<'b>,
    text: &'a BitmapFont,
    fancy: Option<&'a BitmapFont>,
    origin: [f32; 2],
    s: f32,
}

impl Pen<'_, '_> {
    fn at(&self, x: f32, y: f32) -> [f32; 2] {
        [self.origin[0] + x * self.s, self.origin[1] + y * self.s]
    }

    /// Font pixels to book pixels for body text (about 16 book pixels per line of the font).
    fn text_scale(&self) -> f32 {
        16.0 / self.text.line_height
    }

    fn title(&self, column: &mut Column, title: &str) {
        let centre = [(column.x + column.right) / 2.0, column.y + 13.0];
        match self.fancy {
            Some(fancy) => {
                let scale = 26.0 / fancy.line_height;
                let fitted = fancy.fit(title, (column.right - column.x) / scale);
                fancy.draw(self.draw, &fitted, self.at(centre[0], centre[1]), scale * self.s, TITLE_INK);
            }
            None => self.centred(title, centre, TITLE_INK),
        }
        column.y += 32.0;
    }

    fn centred(&self, text: &str, centre: [f32; 2], colour: ImColor32) {
        self.text.draw(self.draw, text, self.at(centre[0], centre[1]), self.text_scale() * self.s, colour);
    }

    fn line(&self, column: &mut Column, text: &str, colour: ImColor32) {
        if column.full() {
            return;
        }
        let scale = self.text_scale();
        let fitted = self.text.fit(text, (column.right - column.x) / scale);
        self.text.draw_at(self.draw, &fitted, self.at(column.x, column.y), scale * self.s, colour);
        column.y += LINE;
    }

    /// `label` on the left, `value` right-aligned.
    fn pair(&self, column: &mut Column, label: &str, value: &str) {
        if column.full() {
            return;
        }
        let scale = self.text_scale();
        let width = (column.right - column.x) / scale;
        let value_width = self.text.width(value).min(width * 0.62);
        let value = self.text.fit(value, value_width + 1.0);
        let label = self.text.fit(label, width - value_width - 8.0);
        self.text.draw_at(self.draw, &label, self.at(column.x, column.y), scale * self.s, INK);
        let x = column.right - self.text.width(&value) * scale;
        self.text.draw_at(self.draw, &value, self.at(x, column.y), scale * self.s, INK);
        column.y += LINE;
    }

    /// Word-wrapped paragraph.
    fn para(&self, column: &mut Column, text: &str) {
        let scale = self.text_scale();
        let width = (column.right - column.x) / scale;
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if self.text.width(&candidate) > width && !line.is_empty() {
                self.line(column, &line, INK);
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            self.line(column, &line, INK);
        }
    }
}
