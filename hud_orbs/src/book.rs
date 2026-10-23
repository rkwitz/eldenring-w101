//! Wizard101's spellbook as an Elden Ring menu: SpellBookPrefsWindow.gui's book with its tab buttons down the right
//! edge, and for each tab Wizard101's own page layout (CharStatsSpellbookPage.gui, InventorySpellbookPage.gui, ...)
//! drawn from the .lay files tools/w101_book_layout.py makes, filled with Elden Ring's data by pages.rs.
//!
//! Open and close with B on the keyboard or View on the controller (instead of Elden Ring's map), close with Esc or
//! the controller's B too. Tabs: click, Up/Down, D-pad up/down or LB/RB; list pages: the page's arrows, Left/Right,
//! D-pad left/right or the mouse wheel; categories: the page's buttons or Y/X. The World Map tab opens Elden Ring's
//! own map and comes back to the book when the map closes, unless the player travelled (to a grace). While the book
//! is open the game gets no controller, keyboard or mouse input (see input.rs).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use hudhook::imgui::{Key, MouseButton, Ui};
use hudhook::RenderContext;

use crate::font::BitmapFont;
use crate::game::{self, Sheet};
use crate::input;
use crate::layout::{self, Canvas, Layout, Textures, View};
use crate::names::Names;
use crate::pages::{self, Action, Data, Tab, TABS};

/// The book window (SpellBookPrefsWindow.gui, 800x600, centred) in Wizard101 pixels.
const BOOK: [f32; 2] = [800.0, 600.0];
/// Wizard101's screen height; the book scales with the screen like the HUD.
const SCREEN: f32 = 768.0;
/// How far the player has to move while Elden Ring's map is open to count as travel (map-chunk metres).
const TRAVEL: f32 = 30.0;

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
    opened_at: Instant,
    tab: Tab,
    map: MapState,
    /// Where the player was when the map opened, and whether a loading screen or a long jump happened since.
    map_from: Option<([f32; 3], i32)>,
    travelled: bool,
    folder: PathBuf,
    prefs: Option<Layout>,
    layouts: HashMap<&'static str, Layout>,
    pub textures: Textures,
    fonts: HashMap<String, BitmapFont>,
    names: Option<Names>,
    sheet: Option<Sheet>,
    sheet_time: Option<Instant>,
    state: pages::State,
}

/// What the book needs from the rest of the overlay each frame.
pub struct Context<'a> {
    pub game: Option<&'a game::Game>,
    pub stats: Option<game::Stats>,
    /// Normal play: ER's HUD in its default state, no fade, a character loaded.
    pub gameplay: bool,
    /// ER's HUD is in its default state (false while ER's map or menus are open).
    pub hud: bool,
    /// ER's fade-to-black, 0 to 1 (loading screens).
    pub faded: f32,
    pub position: Option<([f32; 3], i32)>,
    pub settings: &'a mut crate::Settings,
    pub settings_path: &'a Path,
}

fn tab_index(tab: Tab) -> usize {
    TABS.iter().position(|t| t.0 == tab).unwrap_or(0)
}

impl Book {
    pub fn new() -> Book {
        Book {
            open: false,
            holding: false,
            closed_at: None,
            opened_at: Instant::now(),
            tab: Tab::Character,
            map: MapState::Idle,
            map_from: None,
            travelled: false,
            folder: PathBuf::new(),
            prefs: None,
            layouts: HashMap::new(),
            textures: Textures::default(),
            fonts: HashMap::new(),
            names: None,
            sheet: None,
            sheet_time: None,
            state: pages::State::default(),
        }
    }

    /// Loads the fonts and layouts from `folder` (w101_orbs/book); the art loads when it is first drawn.
    pub fn load(&mut self, folder: &Path, render: &mut dyn RenderContext) {
        self.folder = folder.to_path_buf();
        for font in ["WizardFancy", "WizardNormal"] {
            match BitmapFont::load(folder, font, render) {
                Ok(f) => {
                    self.fonts.insert(font.to_string(), f);
                }
                Err(e) => tracing::warn!("spellbook font: {e}"),
            }
        }
        match Layout::load(&folder.join("SpellBookPrefsWindow.lay")) {
            Ok(l) => self.prefs = Some(l),
            Err(e) => tracing::warn!("spellbook not loaded (run tools/w101_book_layout.py): {e}"),
        }
        for (_, _, page) in TABS.iter().filter(|t| !t.2.is_empty()).chain([&(Tab::Quests, "", pages::QUEST_CARD)]) {
            match Layout::load(&folder.join(format!("{page}.lay"))) {
                Ok(l) => {
                    self.layouts.insert(page, l);
                }
                Err(e) => tracing::warn!("spellbook page: {e}"),
            }
        }
        tracing::info!("spellbook: {} page layouts, {} fonts", self.layouts.len(), self.fonts.len());
    }

    /// Loads queued art (render thread, before the frame).
    pub fn load_art(&mut self, render: &mut dyn RenderContext) {
        if self.textures.pending() {
            let folder = self.folder.join("art");
            self.textures.load_wanted(&folder, render);
        }
    }

    pub fn has_art(&self, name: &str) -> bool {
        self.textures.get(name).is_some()
    }

    /// Whether the game's keyboard, mouse and controller input are blocked.
    pub fn blocking(&self) -> bool {
        self.open || self.holding
    }

    /// Handles input and the map round trip, then draws the book if it is open.
    pub fn frame(&mut self, ui: &Ui, cx: &mut Context) {
        if self.prefs.is_none() || self.fonts.is_empty() {
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

        // Closing: Esc, B (keyboard or controller), View.
        if pressed & (input::B | input::BACK) != 0
            || ui.is_key_pressed_no_repeat(Key::Escape)
            || ui.is_key_pressed_no_repeat(Key::B)
        {
            self.set_open(false);
            return;
        }
        let last = TABS.len() - 2; // Settings; Close is only for the mouse
        let current = tab_index(self.tab);
        if pressed & (input::DPAD_UP | input::LB) != 0 || ui.is_key_pressed(Key::UpArrow) {
            self.select(TABS[if current == 0 || current > last { last } else { current - 1 }].0, false);
        }
        if pressed & (input::DPAD_DOWN | input::RB) != 0 || ui.is_key_pressed(Key::DownArrow) {
            self.select(TABS[if current >= last { 0 } else { current + 1 }].0, false);
        }
        let wheel = ui.io().mouse_wheel;
        let left = pressed & input::DPAD_LEFT != 0 || ui.is_key_pressed(Key::LeftArrow);
        let right = pressed & input::DPAD_RIGHT != 0 || ui.is_key_pressed(Key::RightArrow);
        if self.tab == Tab::Settings {
            if left || right {
                let row = self.state.sub;
                self.change_setting(row, if left { -1 } else { 1 }, cx);
            }
        } else {
            if left || wheel > 0.0 {
                self.state.step_page(-1);
            }
            if right || wheel < 0.0 {
                self.state.step_page(1);
            }
        }
        if pressed & input::Y != 0 {
            self.state.step_sub(-1);
        }
        if pressed & input::X != 0 {
            self.state.step_sub(1);
        }
        if pressed & input::A != 0 || ui.is_key_pressed_no_repeat(Key::Enter) {
            self.select(self.tab, true);
            if !self.open {
                return;
            }
        }

        let [width, height] = ui.io().display_size;
        let s = height / SCREEN;
        let origin = [(width - BOOK[0] * s) / 2.0, (height - BOOK[1] * s) / 2.0];
        let mouse = [(ui.io().mouse_pos[0] - origin[0]) / s, (ui.io().mouse_pos[1] - origin[1]) / s];
        let clicked = ui.is_mouse_clicked(MouseButton::Left);

        // Fill the page.
        let page_name = TABS[tab_index(self.tab)].2;
        let mut view = View::default();
        let mut settings = settings_rows(cx.settings);
        if self.tab != Tab::Settings {
            settings.clear();
        }
        let none = Names::default();
        let data = Data {
            sheet: self.sheet.as_ref(),
            stats: cx.stats,
            names: self.names.as_ref().unwrap_or(&none),
            settings,
            map_opening: matches!(self.map, MapState::Opening(_)),
        };
        if let Some(page) = self.layouts.get(page_name) {
            pages::bind(self.tab, page, &data, &mut self.state, &mut view);
        }
        let prefs = self.prefs.as_ref().expect("checked above");
        let mut prefs_view = View::default();
        if let Some(b) = prefs.find(TABS[tab_index(self.tab)].1) {
            prefs_view.checked.insert(b);
        }

        // Ask for the art this frame needs; it loads before the next frame.
        let mut wanted = prefs.images();
        if let Some(page) = self.layouts.get(page_name) {
            wanted.extend(page.images());
        }
        if self.tab == Tab::Quests {
            if let Some(card) = self.layouts.get(pages::QUEST_CARD) {
                wanted.extend(card.images());
            }
        }
        wanted.extend(view.images.iter().map(|i| i.1.clone()));
        self.textures.want(wanted);

        let draw = ui.get_foreground_draw_list();
        let canvas = Canvas {
            draw: &draw,
            textures: &self.textures,
            fonts: &self.fonts,
            origin,
            s,
            mouse,
            mouse_down: ui.is_mouse_down(MouseButton::Left),
            alpha: (self.opened_at.elapsed().as_secs_f32() / 0.15).min(1.0),
        };
        // The book, then the page, then the tabs on top (they stick out of the page).
        let bkg = prefs.find("Bkg").unwrap_or(1);
        layout::draw(prefs, &prefs_view, &canvas, |i| i <= bkg);
        let mut page_hit = None;
        if let Some(page) = self.layouts.get(page_name) {
            page_hit = layout::draw(page, &view, &canvas, |_| true).button;
            if self.tab == Tab::Quests {
                self.draw_quest_cards(page, &canvas);
            }
        }
        layout::draw_extra(&view, &canvas);
        let tab_hit = layout::draw(prefs, &prefs_view, &canvas, |i| i > bkg).button;

        if !clicked {
            return;
        }
        if let Some(hit) = tab_hit {
            let name = prefs.nodes[hit].name.clone();
            if let Some((tab, _, _)) = TABS.iter().find(|t| t.1 == name) {
                self.select(*tab, true);
            }
            return;
        }
        let Some(hit) = page_hit else { return };
        let action = match self.layouts.get(page_name) {
            Some(page) => pages::click(self.tab, page, hit, &mut self.state),
            None => Action::None,
        };
        match action {
            Action::None => {}
            Action::OpenMap => self.open_map(cx),
            Action::Close => self.set_open(false),
            Action::Tab(tab) => self.select(tab, false),
            Action::Setting(row, step) => self.change_setting(row, step, cx),
        }
    }

    /// QuestListWindow's four card windows, each with QuestInfoWindow.gui drawn into it.
    fn draw_quest_cards(&self, page: &Layout, canvas: &Canvas) {
        let Some(card) = self.layouts.get(pages::QUEST_CARD) else { return };
        for i in 0..4 {
            let Some(window) = page.find(&format!("wndQuestInfo{i}")) else { continue };
            let mut view = View::default();
            if !pages::quest_card(card, i, &mut view) {
                continue;
            }
            let r = page.nodes[window].rect;
            let inner = Canvas {
                draw: canvas.draw,
                textures: canvas.textures,
                fonts: canvas.fonts,
                origin: canvas.at(r[0], r[1]),
                s: canvas.s,
                mouse: [canvas.mouse[0] - r[0], canvas.mouse[1] - r[1]],
                mouse_down: canvas.mouse_down,
                alpha: canvas.alpha,
            };
            layout::draw(card, &view, &inner, |_| true);
        }
    }

    fn change_setting(&mut self, row: usize, step: i32, cx: &mut Context) {
        let s = &mut *cx.settings;
        let step = step as f32;
        match row {
            0 => s.scale = ((s.scale + 0.05 * step) * 100.0).round().clamp(50.0, 200.0) / 100.0,
            1 => s.left = (s.left + 5.0 * step).clamp(-200.0, 600.0),
            2 => s.bottom = (s.bottom + 5.0 * step).clamp(-200.0, 600.0),
            3 => s.level = !s.level,
            4 => s.loadscreen = !s.loadscreen,
            _ => return,
        }
        self.state.sub = row;
        match crate::save_settings(cx.settings_path, s) {
            Ok(()) => tracing::info!("settings saved: {:?}", settings_rows(s)),
            Err(e) => tracing::warn!("settings not saved: {e}"),
        }
    }

    fn set_open(&mut self, open: bool) {
        if open == self.open {
            return;
        }
        self.open = open;
        input::take_pressed();
        if open {
            self.opened_at = Instant::now();
            self.state.page = 0;
            self.sheet_time = None;
        } else {
            self.holding = true;
            self.closed_at = Some(Instant::now());
        }
        input::set_book_open(self.blocking());
    }

    /// Shows `tab`; `activate` also does what the tab does (World Map opens ER's map, Close closes the book).
    fn select(&mut self, tab: Tab, activate: bool) {
        if tab != self.tab {
            self.state.reset();
        }
        if tab != Tab::Close {
            self.tab = tab;
        }
        match tab {
            Tab::Close => self.set_open(false),
            Tab::Map if activate => self.open_map_now(None),
            _ => {}
        }
    }

    fn open_map(&mut self, cx: &Context) {
        self.open_map_now(cx.position);
    }

    fn open_map_now(&mut self, position: Option<([f32; 3], i32)>) {
        self.set_open(false);
        self.map = MapState::Opening(Instant::now());
        self.map_from = position;
        self.travelled = false;
        input::set_gameplay(false);
        input::set_book_open(false);
        self.holding = false;
        input::press_map();
        tracing::info!("spellbook: opening Elden Ring's map (controller: {})", input::pad_connected());
    }

    /// The map round trip: wait for ER's HUD to leave (map open), then reopen the book when it comes back, unless
    /// the player travelled to a grace meanwhile (a loading screen or a jump in position).
    fn follow_map(&mut self, cx: &Context) {
        if self.map != MapState::Idle {
            if self.map_from.is_none() {
                self.map_from = cx.position;
            }
            if cx.faded > 0.9 {
                self.travelled = true;
            }
            if let (Some((from, from_block)), Some((at, block))) = (self.map_from, cx.position) {
                let d = ((at[0] - from[0]).powi(2) + (at[1] - from[1]).powi(2) + (at[2] - from[2]).powi(2)).sqrt();
                if d > TRAVEL || block != from_block {
                    self.travelled = true;
                }
            }
        }
        match self.map {
            MapState::Opening(_) if !cx.hud => self.map = MapState::Open,
            MapState::Opening(since) if since.elapsed().as_secs_f32() > 4.0 => {
                tracing::warn!("spellbook: Elden Ring's map didn't open");
                self.map = MapState::Idle;
            }
            MapState::Open if cx.gameplay => {
                self.map = MapState::Idle;
                if self.travelled {
                    tracing::info!("spellbook: travelled from the map; the book stays closed");
                } else {
                    self.set_open(true);
                    self.tab = Tab::Map;
                }
            }
            _ => {}
        }
    }

    fn refresh(&mut self, cx: &Context) {
        if self.names.as_ref().is_none_or(|n| n.is_empty()) {
            let path = self.folder.parent().unwrap_or(&self.folder).join("er_item_names.tsv");
            self.names = Some(Names::load(&path));
        }
        if self.sheet_time.is_none_or(|t| t.elapsed().as_secs_f32() > 0.5) {
            self.sheet = cx.game.and_then(|g| g.sheet());
            self.sheet_time = Some(Instant::now());
        }
    }
}

/// The settings page's rows: (label, value).
pub fn settings_rows(s: &crate::Settings) -> Vec<(String, String)> {
    let on = |b: bool| if b { "On" } else { "Off" }.to_string();
    vec![
        ("Globe Size".into(), format!("{:.2}", s.scale)),
        ("Globes Right".into(), format!("{:.0}", s.left)),
        ("Globes Up".into(), format!("{:.0}", s.bottom)),
        ("Level Number".into(), on(s.level)),
        ("Loading Screens".into(), on(s.loadscreen)),
    ]
}
