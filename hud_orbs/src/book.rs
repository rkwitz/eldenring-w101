//! Wizard101's spellbook as an Elden Ring menu: SpellBookPrefsWindow.gui's book with its tab buttons down the right
//! edge, and for each tab Wizard101's own page layout (CharStatsSpellbookPage.gui, InventorySpellbookPage.gui, ...)
//! drawn from the .lay files tools/w101_book_layout.py makes, filled with Elden Ring's data by pages.rs.
//!
//! Open and close with B on the keyboard or View on the controller (instead of Elden Ring's map), close with Esc or
//! the controller's B too. Book tabs: click, Up/Down, D-pad up/down or LT/RT; a page's own category tabs along its
//! top: click, Q/E or LB/RB (the hints next to them show which); list pages: the page's arrows, Left/Right, D-pad
//! left/right or the mouse wheel. The mouse works as in a menu: the book draws its own cursor and follows the system
//! cursor, or DirectInput's mouse movement when Elden Ring keeps the system cursor locked.
//!
//! The Backpack's portrait window is a hole in the book: the book moves so that the window sits over the player and
//! the camera can turn (right stick, or drag in the window). The World Map tab opens Elden Ring's own map and draws
//! the book around it, the map showing through the page; the book comes back when the map closes, unless the player
//! travelled (to a grace). While the book is open the game gets no other input (see input.rs).
//!
//! Quests: the quest event flags (quests.rs) fill the Quests page, and an NPC's talk flag shows its dialogue in a
//! Wizard101-style box (then the flag is cleared).

use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use hudhook::imgui::{ImColor32, Key, MouseButton, Ui};
use hudhook::RenderContext;

use crate::font::BitmapFont;
use crate::game::{self, Sheet};
use crate::input;
use crate::itemdata::ItemData;
use crate::layout::{self, Canvas, Layout, TextStyle, Textures, View};
use crate::names::Names;
use crate::pages::{self, Action, Data, Tab, TABS};
use crate::quests::{Active, Quests, Talk};

/// The book window (SpellBookPrefsWindow.gui, 800x600, centred) in Wizard101 pixels.
const BOOK: [f32; 2] = [800.0, 600.0];
/// Wizard101's screen height; the book scales with the screen like the HUD.
const SCREEN: f32 = 768.0;
/// How far the player has to move while Elden Ring's map is open to count as travel (map-chunk metres).
const TRAVEL: f32 = 30.0;
/// The point on the player the Backpack's portrait window centres on (metres above the feet).
const PORTRAIT_HEIGHT: f32 = 0.95;
/// How often the quest flags are read.
const QUEST_POLL: Duration = Duration::from_millis(250);
/// How long after a talk press the quest flags are compared (the map's events update them on the same press).
const TALK_SETTLE: Duration = Duration::from_millis(500);

const GOLD: ImColor32 = ImColor32::from_rgb(214, 170, 72);
const PARCHMENT_DARK: ImColor32 = ImColor32::from_rgba(28, 18, 10, 236);
const YELLOW: ImColor32 = ImColor32::from_rgb(255, 255, 0);
const WHITE: ImColor32 = ImColor32::from_rgb(255, 255, 255);

#[derive(Clone, Copy, PartialEq, Debug)]
enum MapState {
    Idle,
    /// The map press was sent; waiting for the game's HUD to give way to the map.
    Opening(Instant),
    Open,
}

/// The mouse this frame, in screen pixels.
#[derive(Clone, Copy, Default)]
struct Pointer {
    pos: [f32; 2],
    delta: [f32; 2],
    down: bool,
    clicked: bool,
    wheel: f32,
}

struct Dialogue {
    talk: Talk,
    line: usize,
    opened: Instant,
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
    map_from: Option<([f32; 3], i32, i32)>,
    travelled: bool,
    folder: PathBuf,
    prefs: Option<Layout>,
    layouts: HashMap<&'static str, Layout>,
    pub textures: Textures,
    fonts: HashMap<String, BitmapFont>,
    names: Option<Names>,
    items: ItemData,
    sheet: Option<Sheet>,
    sheet_time: Option<Instant>,
    state: pages::State,
    quests: Quests,
    quest_flags: Vec<u32>,
    flags: HashMap<u32, bool>,
    flags_time: Option<Instant>,
    active: Vec<Active>,
    /// A talk press waiting for the quest flags to settle: NPC name, the flags before it, when.
    talk: Option<(String, HashMap<u32, bool>, Instant)>,
    dialogue: Option<Dialogue>,
    /// The book's cursor (screen pixels), the system cursor last seen, DirectInput's buttons last seen.
    cursor: [f32; 2],
    last_os: [f32; 2],
    di_buttons: u8,
    last_click: Option<Instant>,
    /// Turning the camera by dragging in the portrait window.
    dragging: bool,
    /// Where the book is moved to (screen pixels from centred), easing towards its target.
    shift: [f32; 2],
    last_frame: Instant,
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
    pub position: Option<([f32; 3], i32, i32)>,
    pub settings: &'a mut crate::Settings,
    pub settings_path: &'a Path,
}

fn tab_index(tab: Tab) -> usize {
    TABS.iter().position(|t| t.0 == tab).unwrap_or(0)
}

/// A small button hint (LB, RB, Q, ...) in page pixels.
fn badge(canvas: &Canvas, r: [f32; 4], text: &str) {
    let a = canvas.alpha;
    let fade = |c: ImColor32| ImColor32::from_rgba(c.r, c.g, c.b, (c.a as f32 * a) as u8);
    let (p0, p1) = (canvas.at(r[0], r[1]), canvas.at(r[2], r[3]));
    let round = 5.0 * canvas.s;
    canvas.draw.add_rect(p0, p1, fade(PARCHMENT_DARK)).filled(true).rounding(round).build();
    canvas.draw.add_rect(p0, p1, fade(GOLD)).rounding(round).thickness(1.5 * canvas.s).build();
    canvas.text(&format!("<center>{text}</center>"), [r[0] + 2.0, r[1], r[2] - 2.0, r[3]],
                &TextStyle::new("WizardFancy", 0.0, fade(YELLOW)), false);
}

/// The book's mouse cursor (Elden Ring hides the system one): an arrow with a dark outline.
fn draw_cursor(draw: &hudhook::imgui::DrawListMut, p: [f32; 2]) {
    let k = 1.2;
    let pts = [p, [p[0], p[1] + 20.0 * k], [p[0] + 5.0 * k, p[1] + 15.0 * k], [p[0] + 14.0 * k, p[1] + 14.0 * k]];
    let dark = ImColor32::from_rgba(20, 12, 4, 255);
    draw.add_triangle(pts[0], pts[1], pts[2], WHITE).filled(true).build();
    draw.add_triangle(pts[0], pts[2], pts[3], WHITE).filled(true).build();
    draw.add_polyline(vec![pts[0], pts[1], pts[2], pts[3], pts[0]], dark).thickness(1.5).build();
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
            items: ItemData::default(),
            sheet: None,
            sheet_time: None,
            state: pages::State::default(),
            quests: Quests::default(),
            quest_flags: Vec::new(),
            flags: HashMap::new(),
            flags_time: None,
            active: Vec::new(),
            talk: None,
            dialogue: None,
            cursor: [0.0; 2],
            last_os: [-1.0; 2],
            di_buttons: 0,
            last_click: None,
            dragging: false,
            shift: [0.0; 2],
            last_frame: Instant::now(),
        }
    }

    /// Loads the fonts, layouts, item data and quests from `folder` (w101_orbs/book); the art loads when it is
    /// first drawn.
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
        self.items = ItemData::load(&folder.join("er"));
        self.quests = Quests::load(&folder.join("quests.tsv"));
        self.quest_flags = self.quests.flags();
        tracing::info!("spellbook: {} page layouts, {} fonts, {} quests", self.layouts.len(), self.fonts.len(),
                       self.quests.quests.len());
    }

    /// Loads queued art (render thread, before the frame).
    pub fn load_art(&mut self, render: &mut dyn RenderContext) {
        if self.textures.pending() {
            let folder = self.folder.join("art");
            self.textures.load_wanted(&folder, render);
        }
    }

    /// Opens the book on `tab` / sub-page `sub` (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_show(&mut self, tab: Tab, sub: usize) {
        self.dialogue = None;
        self.map = MapState::Idle;
        self.set_open(true);
        self.opened_at = Instant::now() - Duration::from_secs(1);
        self.select(tab, false);
        self.state.reset();
        self.state.sub = sub;
    }

    /// Shows the book around Elden Ring's open map (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_map(&mut self) {
        self.preview_show(Tab::Map, 0);
        self.set_open(false);
        self.map = MapState::Open;
    }

    /// Shows pet `pet` (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_pet(&mut self, pet: usize) {
        self.state.pet = pet;
    }

    /// Shows the dialogue of a talk press on `npc` (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_talk(&mut self, npc: &str) {
        self.set_open(false);
        let before = |_: u32| false;
        let after = |f: u32| game::event_flag(f) == Some(true);
        if let Some(talk) = self.quests.talk(npc, &before, &after) {
            self.dialogue = Some(Dialogue { talk, line: 0, opened: Instant::now() - Duration::from_secs(1) });
        }
    }

    /// Whether the game's keyboard, mouse and controller input are blocked.
    pub fn blocking(&self) -> bool {
        self.open || self.holding || self.dialogue.is_some()
    }

    /// Handles input and the map round trip, then draws the book if it is open.
    pub fn frame(&mut self, ui: &Ui, cx: &mut Context) {
        if self.prefs.is_none() || self.fonts.is_empty() {
            input::take_pressed();
            input::set_gameplay(false);
            return;
        }
        let dt = self.last_frame.elapsed().as_secs_f32().min(0.1);
        self.last_frame = Instant::now();
        input::set_gameplay(cx.gameplay && !self.open && self.dialogue.is_none());
        // Before taking the presses, so that the press that closed ER's map doesn't close the reopened book.
        self.follow_map(cx);
        let pressed = input::take_pressed();
        let pointer = self.pointer(ui);
        if cx.game.is_some() {
            self.poll_quests(cx);
        }
        if self.dialogue.is_some() {
            self.dialogue_frame(ui, pressed, &pointer);
            return;
        }
        if !self.open {
            input::set_camera_pass(false);
            self.shift = [0.0; 2];
            self.dragging = false;
            if self.map == MapState::Open && cx.game.is_some() && cx.faded < 0.5 {
                // Elden Ring's map is open: the book around it.
                self.refresh(cx);
                self.draw(ui, cx, &Pointer { pos: [-1.0e4; 2], ..Pointer::default() }, dt, true);
            }
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
        // Book tabs: LT / RT, D-pad up / down, Up / Down.
        let last = TABS.len() - 2; // Settings; Close is only for the mouse
        let current = tab_index(self.tab);
        if pressed & (input::DPAD_UP | input::LT) != 0 || ui.is_key_pressed(Key::UpArrow) {
            self.select(TABS[if current == 0 || current > last { last } else { current - 1 }].0, false);
        }
        if pressed & (input::DPAD_DOWN | input::RT) != 0 || ui.is_key_pressed(Key::DownArrow) {
            self.select(TABS[if current >= last { 0 } else { current + 1 }].0, false);
        }
        // The page's own tabs along its top: LB / RB (or Y / X), Q / E.
        let sub_prev = pressed & (input::LB | input::Y) != 0 || ui.is_key_pressed(Key::Q);
        let sub_next = pressed & (input::RB | input::X) != 0 || ui.is_key_pressed(Key::E);
        for (step, on) in [(-1, sub_prev), (1, sub_next)] {
            if !on {
                continue;
            }
            match self.tab {
                Tab::Pets => self.state.step_pet(step),
                _ => self.state.step_sub(step),
            }
        }
        let left = pressed & input::DPAD_LEFT != 0 || ui.is_key_pressed(Key::LeftArrow);
        let right = pressed & input::DPAD_RIGHT != 0 || ui.is_key_pressed(Key::RightArrow);
        if self.tab == Tab::Settings {
            if left || right {
                let row = self.state.sub;
                self.change_setting(row, if left { -1 } else { 1 }, cx);
            }
        } else {
            if left || pointer.wheel > 0.0 {
                self.state.step_page(-1);
            }
            if right || pointer.wheel < 0.0 {
                self.state.step_page(1);
            }
        }
        if pressed & input::A != 0 || ui.is_key_pressed_no_repeat(Key::Enter) {
            self.select(self.tab, true);
            if !self.open {
                return;
            }
        }

        let (tab_hit, page_hit, in_hole) = self.draw(ui, cx, &pointer, dt, false);

        // Turning the camera: drag in the portrait window.
        if pointer.clicked && in_hole {
            self.dragging = true;
        }
        if !pointer.down {
            self.dragging = false;
        }
        if self.dragging && pointer.delta[0] != 0.0 {
            input::turn_camera(0.0, (pointer.delta[0] * 2.0) as i32);
        }

        if !pointer.clicked {
            return;
        }
        let prefs = self.prefs.as_ref().expect("checked above");
        if let Some(hit) = tab_hit {
            let name = prefs.nodes[hit].name.clone();
            if let Some((tab, _, _)) = TABS.iter().find(|t| t.1 == name) {
                self.select(*tab, true);
            }
            return;
        }
        let Some(hit) = page_hit else { return };
        let page_name = TABS[tab_index(self.tab)].2;
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

    /// The mouse: the system cursor when it moves, else DirectInput's movement (Elden Ring can hold the system
    /// cursor still); clicks from either, counted once.
    fn pointer(&mut self, ui: &Ui) -> Pointer {
        let (dx, dy, wheel, buttons, _) = input::take_mouse();
        let io = ui.io();
        let [width, height] = io.display_size;
        let before = self.cursor;
        let os = io.mouse_pos;
        let valid = os[0] >= 0.0 && os[1] >= 0.0 && os[0] < 1.0e5 && os[1] < 1.0e5;
        if self.cursor == [0.0, 0.0] {
            self.cursor = [width / 2.0, height / 2.0];
        }
        if valid && os != self.last_os {
            self.cursor = os;
        } else if dx != 0 || dy != 0 {
            self.cursor = [self.cursor[0] + dx as f32, self.cursor[1] + dy as f32];
        }
        if valid {
            self.last_os = os;
        }
        self.cursor = [self.cursor[0].clamp(0.0, (width - 1.0).max(0.0)), self.cursor[1].clamp(0.0, (height - 1.0).max(0.0))];
        let di_down = buttons & 1 != 0;
        let di_clicked = di_down && self.di_buttons & 1 == 0;
        self.di_buttons = buttons;
        let mut clicked = ui.is_mouse_clicked(MouseButton::Left) || di_clicked;
        if clicked {
            if self.last_click.is_some_and(|t| t.elapsed() < Duration::from_millis(150)) {
                clicked = false;
            } else {
                self.last_click = Some(Instant::now());
            }
        }
        Pointer {
            pos: self.cursor,
            delta: [self.cursor[0] - before[0], self.cursor[1] - before[1]],
            down: io.mouse_down[0] || di_down,
            clicked,
            wheel: if io.mouse_wheel != 0.0 { io.mouse_wheel } else { wheel as f32 / 120.0 },
        }
    }

    /// Fills and draws the current page; returns the book tab and page button under the mouse, and whether the
    /// mouse is in the page's hole. `map_view`: Elden Ring's map is open behind the book.
    fn draw(&mut self, ui: &Ui, cx: &Context, pointer: &Pointer, dt: f32, map_view: bool)
            -> (Option<usize>, Option<usize>, bool) {
        let tab = if map_view { Tab::Map } else { self.tab };
        let page_name = TABS[tab_index(tab)].2;
        let mut view = View::default();
        let mut settings = settings_rows(cx.settings);
        if tab != Tab::Settings {
            settings.clear();
        }
        let none = Names::default();
        let data = Data {
            sheet: self.sheet.as_ref(),
            stats: cx.stats,
            names: self.names.as_ref().unwrap_or(&none),
            items: &self.items,
            quests: &self.active,
            settings,
            map_opening: matches!(self.map, MapState::Opening(_)),
            map_open: map_view,
        };
        if let Some(page) = self.layouts.get(page_name) {
            pages::bind(tab, page, &data, &mut self.state, &mut view);
        }
        let prefs = self.prefs.as_ref().expect("checked by frame");
        let mut prefs_view = View::default();
        if let Some(b) = prefs.find(TABS[tab_index(tab)].1) {
            prefs_view.checked.insert(b);
        }

        // Ask for the art this frame needs; it loads before the next frame.
        let mut wanted = prefs.images();
        if let Some(page) = self.layouts.get(page_name) {
            wanted.extend(page.images());
        }
        if tab == Tab::Quests {
            if let Some(card) = self.layouts.get(pages::QUEST_CARD) {
                wanted.extend(card.images());
            }
        }
        wanted.extend(view.images.iter().map(|i| i.1.clone()));
        self.textures.want(wanted);

        // Where the book goes: centred, or (Backpack) moved so that the portrait window is over the player.
        let [width, height] = ui.io().display_size;
        let s = height / SCREEN;
        let base = [(width - BOOK[0] * s) / 2.0, (height - BOOK[1] * s) / 2.0];
        let mut target = [0.0, 0.0];
        let camera = tab == Tab::Backpack && view.hole.is_some() && !map_view;
        if camera {
            let h = view.hole.expect("checked");
            let on_screen = cx.game.and_then(|g| g.player_on_screen([width, height], PORTRAIT_HEIGHT));
            if let Some(p) = on_screen {
                let centre = [base[0] + (h[0] + h[2]) / 2.0 * s, base[1] + (h[1] + h[3]) / 2.0 * s];
                // Keep the whole book on the screen.
                target = [(p[0] - centre[0]).clamp(-base[0], base[0]), (p[1] - centre[1]).clamp(-base[1], base[1])];
            }
        }
        input::set_camera_pass(camera);
        let ease = (dt * 10.0).min(1.0);
        for i in 0..2 {
            self.shift[i] += (target[i] - self.shift[i]) * ease;
            if (target[i] - self.shift[i]).abs() < 0.5 {
                self.shift[i] = target[i];
            }
        }
        let origin = [base[0] + self.shift[0], base[1] + self.shift[1]];
        let mouse = [(pointer.pos[0] - origin[0]) / s, (pointer.pos[1] - origin[1]) / s];

        let draw = ui.get_foreground_draw_list();
        let canvas = Canvas {
            draw: &draw,
            textures: &self.textures,
            fonts: &self.fonts,
            origin,
            s,
            mouse,
            mouse_down: pointer.down,
            alpha: if map_view { 1.0 } else { (self.opened_at.elapsed().as_secs_f32() / 0.15).min(1.0) },
            hole: Cell::new(view.hole),
        };
        // The book, then the page, then the tabs on top (they stick out of the page).
        let bkg = prefs.find("Bkg").unwrap_or(1);
        layout::draw(prefs, &prefs_view, &canvas, |i| i <= bkg);
        let mut page_hit = None;
        if let Some(page) = self.layouts.get(page_name) {
            page_hit = layout::draw(page, &view, &canvas, |_| true).button;
            if tab == Tab::Quests {
                self.draw_quest_cards(page, &canvas);
            }
        }
        layout::draw_extra(&view, &canvas);
        let tab_hit = layout::draw(prefs, &prefs_view, &canvas, |i| i > bkg).button;
        canvas.hole.set(None);

        if !map_view {
            // Hints: LB / RB (Q / E) beside the page's category tabs, LT / RT (Up / Down) at the ends of the book's tabs.
            let pad = input::pad_connected();
            if let Some(r) = view.tab_row {
                let mid = (r[1] + r[3]) / 2.0;
                let (l, rr) = if pad { ("LB", "RB") } else { ("Q", "E") };
                badge(&canvas, [r[0] - 36.0, mid - 12.0, r[0] - 4.0, mid + 12.0], l);
                badge(&canvas, [r[2] + 4.0, mid - 12.0, r[2] + 36.0, mid + 12.0], rr);
            }
            let first = prefs.find(TABS[0].1).map(|i| prefs.nodes[i].rect);
            let close = prefs.find(TABS[TABS.len() - 1].1).map(|i| prefs.nodes[i].rect);
            let (up, down) = if pad { ("LT", "RT") } else { ("Up", "Down") };
            if let (Some(f), Some(c)) = (first, close) {
                badge(&canvas, [f[0] - 2.0, f[1] - 24.0, f[2] + 2.0, f[1] - 2.0], up);
                badge(&canvas, [c[0] - 2.0, c[3] + 2.0, c[2] + 2.0, c[3] + 24.0], down);
            }
            // The hover text of what the mouse is over.
            if let Some((_, lines)) = view.tips.iter().rev().find(|(r, _)| layout::inside(mouse, *r)) {
                self.draw_tip(&canvas, lines, mouse, [width, height]);
            }
            draw_cursor(&draw, pointer.pos);
        }
        let in_hole = view.hole.is_some_and(|h| layout::inside(mouse, h));
        (tab_hit, page_hit, in_hole)
    }

    /// A hover box beside the mouse: the title, then the lines, wrapped.
    fn draw_tip(&self, canvas: &Canvas, lines: &[String], mouse: [f32; 2], screen: [f32; 2]) {
        let Some(font) = self.fonts.get("WizardNormal") else { return };
        let width = 300.0;
        let pad = 10.0;
        let title_h = 26.0;
        let scale = 0.4;
        let line_h = font.line_height * scale;
        let rows: Vec<f32> = lines
            .iter()
            .skip(1)
            .map(|l| ((font.width(l) * scale / (width - 2.0 * pad)).ceil().max(1.0) + 0.3).floor() * line_h + 2.0)
            .collect();
        let height = pad * 2.0 + title_h + rows.iter().sum::<f32>();
        // Page pixels of the screen's edges.
        let right = (screen[0] - canvas.origin[0]) / canvas.s;
        let bottom = (screen[1] - canvas.origin[1]) / canvas.s;
        let top = -canvas.origin[1] / canvas.s;
        let mut x = mouse[0] + 22.0;
        if x + width > right {
            x = mouse[0] - 12.0 - width;
        }
        let y = (mouse[1] + 8.0).min(bottom - height).max(top);
        let (p0, p1) = (canvas.at(x, y), canvas.at(x + width, y + height));
        canvas.draw.add_rect(p0, p1, PARCHMENT_DARK).filled(true).rounding(6.0 * canvas.s).build();
        canvas.draw.add_rect(p0, p1, GOLD).rounding(6.0 * canvas.s).thickness(2.0 * canvas.s).build();
        canvas.text(&lines[0], [x + pad, y + pad, x + width - pad, y + pad + title_h],
                    &TextStyle::new("WizardFancy", 0.5, YELLOW), false);
        let mut ly = y + pad + title_h;
        for (line, h) in lines.iter().skip(1).zip(rows) {
            canvas.text(line, [x + pad, ly, x + width - pad, ly + h], &TextStyle::new("WizardNormal", scale, WHITE).wrapped(),
                        false);
            ly += h;
        }
    }

    /// QuestListWindow's four card windows, each with QuestInfoWindow.gui drawn into it for a quest in progress.
    fn draw_quest_cards(&self, page: &Layout, canvas: &Canvas) {
        let Some(card) = self.layouts.get(pages::QUEST_CARD) else { return };
        for i in 0..pages::QUESTS_PER_PAGE {
            let Some(quest) = self.active.get(self.state.page * pages::QUESTS_PER_PAGE + i) else { break };
            let Some(window) = page.find(&format!("wndQuestInfo{i}")) else { continue };
            let mut view = View::default();
            pages::quest_card(card, quest, &mut view);
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
                hole: Cell::new(None),
            };
            layout::draw(card, &view, &inner, |_| true);
            layout::draw_extra(&view, &inner);
        }
    }

    /// Reads the quest flags; a talk flag that is on is cleared and, once the quest flags have settled, its dialogue
    /// is shown.
    fn poll_quests(&mut self, cx: &Context) {
        if self.quests.quests.is_empty() || self.flags_time.is_some_and(|t| t.elapsed() < QUEST_POLL) {
            return;
        }
        self.flags_time = Some(Instant::now());
        let first = self.flags.is_empty();
        let before = std::mem::take(&mut self.flags);
        let mut now: HashMap<u32, bool> = HashMap::new();
        for f in &self.quest_flags {
            if let Some(on) = game::event_flag(*f) {
                now.insert(*f, on);
            }
        }
        for (_, flag, name) in &self.quests.npcs {
            if now.get(flag) == Some(&true) {
                game::set_event_flag(*flag, false);
                now.insert(*flag, false);
                // A flag left on from before the HUD started has nothing to compare with.
                if !first && self.talk.is_none() {
                    self.talk = Some((name.clone(), before.clone(), Instant::now()));
                }
            }
        }
        self.flags = now;
        let flags = &self.flags;
        self.active = self.quests.active(&|f| flags.get(&f) == Some(&true));
        let ready = self.talk.as_ref().is_some_and(|t| t.2.elapsed() >= TALK_SETTLE);
        if ready && cx.gameplay && !self.open {
            let (npc, before, _) = self.talk.take().expect("checked");
            let talk = self.quests.talk(&npc, &|f| before.get(&f) == Some(&true), &|f| flags.get(&f) == Some(&true));
            if let Some(talk) = talk {
                tracing::info!("dialogue: {} ({}, {:?})", talk.npc, talk.quest, talk.event);
                self.dialogue = Some(Dialogue { talk, line: 0, opened: Instant::now() });
                input::set_book_open(true);
            }
        }
    }

    /// The NPC dialogue box: A, Enter, Space or a click for the next line; B or Esc closes it.
    fn dialogue_frame(&mut self, ui: &Ui, pressed: u16, pointer: &Pointer) {
        input::set_book_open(true);
        input::set_camera_pass(false);
        let Some(d) = self.dialogue.as_mut() else { return };
        let settled = d.opened.elapsed() > Duration::from_millis(250);
        let next = pressed & input::A != 0
            || ui.is_key_pressed_no_repeat(Key::Enter)
            || ui.is_key_pressed_no_repeat(Key::Space)
            || ui.is_key_pressed_no_repeat(Key::E)
            || pointer.clicked;
        let close = pressed & (input::B | input::BACK) != 0 || ui.is_key_pressed_no_repeat(Key::Escape);
        if settled && next {
            d.line += 1;
        }
        if (settled && close) || d.line >= d.talk.lines.len() {
            self.dialogue = None;
            self.holding = true;
            self.closed_at = Some(Instant::now());
            input::set_book_open(self.blocking());
            return;
        }
        // A box along the bottom of the screen, in page pixels of a centred book.
        let [width, height] = ui.io().display_size;
        let s = height / SCREEN;
        let draw = ui.get_foreground_draw_list();
        let canvas = Canvas {
            draw: &draw,
            textures: &self.textures,
            fonts: &self.fonts,
            origin: [(width - BOOK[0] * s) / 2.0, height - 190.0 * s],
            s,
            mouse: [-1.0e4; 2],
            mouse_down: false,
            alpha: (d.opened.elapsed().as_secs_f32() / 0.15).min(1.0),
            hole: Cell::new(None),
        };
        let r = [40.0, 0.0, 760.0, 150.0];
        let (p0, p1) = (canvas.at(r[0], r[1]), canvas.at(r[2], r[3]));
        draw.add_rect(p0, p1, PARCHMENT_DARK).filled(true).rounding(10.0 * s).build();
        draw.add_rect(p0, p1, GOLD).rounding(10.0 * s).thickness(3.0 * s).build();
        canvas.text(&d.talk.npc, [r[0] + 22.0, r[1] + 10.0, r[2] - 200.0, r[1] + 40.0],
                    &TextStyle::new("WizardFancy", 0.6, YELLOW).outlined(), false);
        canvas.text(&format!("<right>{}</right>", d.talk.quest), [r[2] - 300.0, r[1] + 12.0, r[2] - 22.0, r[1] + 36.0],
                    &TextStyle::new("WizardFancy", 0.42, GOLD), false);
        canvas.text(&d.talk.lines[d.line], [r[0] + 24.0, r[1] + 46.0, r[2] - 24.0, r[3] - 26.0],
                    &TextStyle::new("WizardNormal", 0.5, WHITE).wrapped(), false);
        let more = if input::pad_connected() { "A: next   B: close" } else { "Enter or click: next   Esc: close" };
        canvas.text(&format!("<right>{}/{}   {more}</right>", d.line + 1, d.talk.lines.len()),
                    [r[0] + 24.0, r[3] - 26.0, r[2] - 24.0, r[3] - 6.0], &TextStyle::new("WizardNormal", 0.36, GOLD), false);
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
            self.shift = [0.0; 2];
        } else {
            self.holding = true;
            self.closed_at = Some(Instant::now());
            self.dragging = false;
            input::set_camera_pass(false);
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

    fn open_map_now(&mut self, position: Option<([f32; 3], i32, i32)>) {
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
            if let (Some((from, from_block, from_origin)), Some((at, block, origin))) = (self.map_from, cx.position) {
                let d = ((at[0] - from[0]).powi(2) + (at[1] - from[1]).powi(2) + (at[2] - from[2]).powi(2)).sqrt();
                if d > TRAVEL || block != from_block || origin != from_origin {
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
