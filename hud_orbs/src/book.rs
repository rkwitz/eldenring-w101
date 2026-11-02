//! Wizard101's spellbook as an Elden Ring menu: SpellBookPrefsWindow.gui's book with its tab buttons down the right
//! edge, and for each tab Wizard101's own page layout (CharStatsSpellbookPage.gui, InventorySpellbookPage.gui, ...)
//! drawn from the .lay files tools/w101_book_layout.py makes, filled with Elden Ring's data by pages.rs.
//!
//! Open and close with B on the keyboard or View on the controller (instead of Elden Ring's map), close with Esc or
//! the controller's B too. Book tabs: click, Up/Down, D-pad up/down or LT/RT; a page's own category tabs along its
//! top: click, Q/E or LB/RB (no hints are drawn); list pages: the page's arrows, Left/Right, D-pad
//! left/right or the mouse wheel. The mouse works as in a menu: the book draws its own cursor and follows the system
//! cursor, or DirectInput's mouse movement when Elden Ring keeps the system cursor locked.
//!
//! The Backpack's character window and the Character page's portrait show the player drawn from Elden Ring's own
//! models by model.rs (standing, weapon in hand, whatever the character is doing in the world); the Backpack's
//! turns with the right stick or by dragging. Turning to the World Map tab opens Elden Ring's own map and draws the
//! book around it, the map showing through the page and nothing else of the game around the book; the book comes
//! back when the map closes (or a book tab is clicked), unless the player travelled (to a grace). While the book is
//! open the game gets no input at all, the mouse included (see input.rs).
//!
//! Quests: the quest event flags (quests.rs) fill the Quests page; a talk event's request flag shows that
//! conversation in Wizard101's own dialogue box and quest offer (talk.rs), answered through DIALOG_OK / DIALOG_CANCEL.

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
use crate::model::{Framing, Look, Renderer, Request};
use crate::names::Names;
use crate::pages::{self, Action, Data, Hit, Tab, TABS};
use crate::markers;
use crate::nameplates;
use crate::quests::{Active, Quests};
use crate::talk::{self, Conversation, Press};

/// The book window (SpellBookPrefsWindow.gui, 800x600, centred) in Wizard101 pixels.
const BOOK: [f32; 2] = [800.0, 600.0];
/// Wizard101's screen height; the book scales with the screen like the HUD.
const SCREEN: f32 = 768.0;
/// How far the player has to move while Elden Ring's map is open to count as travel (map-chunk metres).
const TRAVEL: f32 = 30.0;
/// Turning speed of the character picture: right stick (radians per second), mouse drag (radians per pixel).
const TURN_MOUSE: f32 = 0.012;
/// The map page's stick cursor: dead zone, and full-tilt speed in screen heights per second.
const MAP_STICK_DEAD: f32 = 0.2;
const MAP_STICK_SPEED: f32 = 0.8;
/// Wizard101's item tooltip (itemStatsInfoViewed.gui / itemStatsInfoEquipped.gui): Art_Message_Box's frame
/// pieces (the blue ones for what is worn) around Background_Message_Box04 (black, at alpha A0), the name in
/// WizardFancy 0.55 light blue centred at 15..38, the stat lines from y 40 with 15 px margins, and for equipped items
/// the fist (Button_Equip) over the top left corner (round 19 C4; it was the top right).
fn tip_frame(blue: bool) -> [Option<String>; 9] {
    let kind = if blue { "Art_Message_Box_Blue" } else { "Art_Message_Box" };
    let mut out: [Option<String>; 9] = Default::default();
    for (i, p) in ["TL", "T", "TR", "L", "R", "BL", "B", "BR"].iter().enumerate() {
        out[i] = Some(format!("GUI/Art/{kind}_{p}.png"));
    }
    out
}
const TIP_WIDTH: f32 = 200.0;
const MAX_TIP: f32 = 290.0;
const TIP_BACK: ImColor32 = ImColor32::from_rgba(0, 0, 0, 205);
/// The item's name: the client's item-title markup "<center><COLOR;FFFFFF00>" (Wizard101 colours are AABBGGRR,
/// as its .lang "Bronze" FF457DE4 / "Gold" FF2ACFFF show): light blue.
const TIP_TITLE: ImColor32 = ImColor32::from_rgb(0, 255, 255);
/// The stat lines: the font is the tooltip window's text style (itemStatsInfoViewed controlWidget: WizardFancy 0.55),
/// in rows as tall as the title (15..39), in the window style's own colour: itemStatsInfoViewed.gui's
/// m_Color 8000FFFF (AABBGGRR) = yellow. Round 15 used white from a "<color;FFFFFFFF>" markup string in the
/// client; Ryan (round 18 B1, from playing Wizard101) says the stats are yellow, which the .gui's style agrees with.
const TIP_TEXT: ImColor32 = ImColor32::from_rgb(255, 255, 0);
const TIP_STAT_SCALE: f32 = 0.55;
const TIP_NOTE: ImColor32 = ImColor32::from_rgb(200, 196, 184);
const EQUIP_FIST: &str = "GUI/InventoryPage/Button_Equip.png";
/// A tooltip row's school and stat icons (Wizard101's 32 px icons drawn small, beside the value).
const TIP_ICON: f32 = 20.0;

/// Nameplate text sizes (WizardFancy scale at 768 pixels of screen height): the reference crop
/// (w101_reference/nameplate_lost_soul.png, taken at about 1.1x Wizard101's 768 size like the reagent card beside it)
/// has the name about 20 pixels tall, the rank line about 14.
const NAME_SCALE: f32 = 0.5;
const RANK_SCALE: f32 = 0.34;

/// The Backpack picture's own turning (radians per second), like Wizard101's slowly turning wizard.
const SPIN: f32 = 1.0;
/// How quickly the Backpack picture zooms to a category (per second).
const ZOOM_RATE: f32 = 6.0;
/// Settings' Quit / Exit Game: seconds to click again to confirm, and seconds at the title screen before closing.
const QUIT_CONFIRM: f32 = 4.0;
const QUIT_SETTLE: f32 = 3.0;

/// How long a page note (pages::State::note) shows.
const NOTE_TIME: Duration = Duration::from_secs(4);
/// How often the quest flags are read.
const QUEST_POLL: Duration = Duration::from_millis(250);

const GOLD: ImColor32 = ImColor32::from_rgb(214, 170, 72);
const PARCHMENT_DARK: ImColor32 = ImColor32::from_rgba(28, 18, 10, 236);
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
    /// When Elden Ring's map started to close (a book tab clicked, or its HUD back): from then until the book is
    /// back, the page to come is drawn over the world as it was before the map opened, so nothing flashes.
    map_leaving: Option<Instant>,
    /// When the book reopened after the map: for a moment it stays up even if the game isn't quite back yet.
    map_back: Option<Instant>,
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
    /// Round 20 (Q7): the quest picked on the Quests page (its slug); the Quest Helper follows it. None: the first
    /// quest in progress (main quests first), as before.
    tracked: Option<String>,
    /// Round 20 (Q8): Wizard101's walk graph of the street for the Quest Helper's step count (quest_nav.tsv).
    pub nav: crate::questhelper::Nav,
    /// Round 21 (Q6): the Quest Helper's last state, logged when it changes ("quest helper: ...").
    helper_state: String,
    /// Round 22 (H2): the bearing the quest arrow shows (eased).
    arrow_bearing: Option<f32>,
    /// Round 22 (H6): the street NPCs in sight of the player (markers::Sight).
    sight: markers::Sight,
    /// Round 22 (H1, H4): w101_orbs.ini `scale` (the experience bar's size), for docking over the bar.
    hud_scale: f32,
    quest_flags: Vec<u32>,
    flags: HashMap<u32, bool>,
    flags_time: Option<Instant>,
    /// Round 23 (H7): Wizard101's quest popups ("Goal Update: ...", goalpopup.rs).
    pub popups: crate::goalpopup::GoalPopups,
    /// Round 25 (H2): Wizard101's quest picker (picker.rs) while one NPC's events ask for several conversations.
    picker: Option<crate::picker::Picker>,
    /// The picker's rows follow the mouse once it moves (else the controller's focus row is lit).
    picker_mouse: bool,
    /// The preview's picker (no event flags behind it).
    picker_pinned: bool,
    /// Round 25 (H3): Wizard101's interact prompt (prompt.rs) and its box's top this frame (screen pixels).
    pub prompt: crate::prompt::Prompt,
    prompt_top: Option<f32>,
    /// The preview's prompt (none in the game).
    preview_prompt: Option<crate::prompt::Target>,
    active: Vec<Active>,
    /// The conversations the map's talk events can ask for (talk.rs, dialogue.tsv), and the one on screen.
    requests: Vec<talk::Request>,
    dialogue: Option<Conversation>,
    /// The book's cursor (screen pixels), the system cursor last seen, DirectInput's buttons last seen.
    cursor: [f32; 2],
    last_os: [f32; 2],
    di_buttons: u8,
    last_click: Option<Instant>,
    /// The last page thing clicked (pages::Hit) and when, for double clicks.
    last_hit: Option<(pages::Hit, Instant)>,
    /// Turning the character picture by dragging in its window.
    dragging: bool,
    /// The character pictures: renderer, their textures (id, size) per slot, the Backpack's turn, last turned.
    model: Option<Renderer>,
    model_textures: [Option<(hudhook::imgui::TextureId, [u32; 2])>; 2],
    yaw: f32,
    turned: Option<Instant>,
    /// The Backpack's picture turns by itself (like Wizard101's): direction +1 / -1, 0 = stopped by a click. A drag
    /// turns it by hand and sets the direction it carries on in.
    spin: f32,
    spin_last: f32,
    /// How far the current drag has turned the picture (a click without turning stops / restarts the spin).
    drag_turned: f32,
    was_dragging: bool,
    /// The Backpack picture's (centre height, view height) now, moving toward the category's zoom.
    zoom: Option<[f32; 2]>,
    /// A book tab clicked while Elden Ring's map was open: shown once the map has closed.
    after_map: Option<Tab>,
    /// A warp button (pages::WARP_*) clicked while Elden Ring's map was open: its event flag is set once the map
    /// has closed (the warp event freezes the player and plays the poof in the world, so the map must be gone).
    warp_after_map: Option<u32>,
    /// The book was opened on the World Map page: Elden Ring's map opens by itself once the button or key that
    /// opened the book is let go (Ryan, 2026-10-30: no "press A" step).
    map_on_open: bool,
    /// Settings' Quit clicked once (to the desktop, when): the next click confirms.
    quit_asked: Option<(bool, Instant)>,
    /// Settings changes waiting for OK (options::Staged).
    staged: crate::options::Staged,
    /// Exit Game confirmed: since when, and since when no player character has been loaded.
    quitting: Option<(Instant, Option<Instant>)>,
    face_logged: bool,
    flasks_logged: bool,
    /// Per pages::ACHIEVEMENTS entry, read with the sheet.
    achieved: Vec<Option<bool>>,
    /// The plain tooltip (ToolTip.gui) being shown: its text and for how many seconds (it fades in, EFFECT_FADEIN).
    bubble: Option<(String, f32)>,
    last_frame: Instant,
    /// Screen copies for the map page: frames counted (the world is copied every 10th frame while the book is
    /// shut), whether a copy of the world from before the map opened exists, whether the map was copied this frame.
    frames: u64,
    world_ready: bool,
    map_shot: bool,
    /// The Equipment Sets page: every character's saved sets (character name, set), the file they are kept in
    /// (next to w101_orbs.ini), and the set being put on: its name and the (slot, item) equips still to make, one
    /// per frame, as Elden Ring's Equipment menu makes them one at a time.
    gear: Vec<(String, pages::GearSet)>,
    gear_path: Option<PathBuf>,
    gear_queue: Option<(String, Vec<(u32, Option<u32>)>, usize)>,
}

/// The equipment sets file, beside the ini: one set per line, "character <tab> set name <tab> slot:param,...".
const GEAR_FILE: &str = "w101_orbs_gear_sets.tsv";

/// How long the book stays up after the map round trip while the game is not yet back to normal play.
const MAP_BACK_GRACE: f32 = 1.5;

/// The copy of the screen from before Elden Ring's map opened, drawn behind the book while the map is on its page.
const WORLD_PICTURE: &str = "@screen/world";

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

/// The layout a tab shows: its page, or on the Backpack the Equipment Sets page while that is open.
fn page_of(tab: Tab, state: &pages::State) -> &'static str {
    if tab == Tab::Backpack && state.gear_view { pages::GEAR_PAGE } else { TABS[tab_index(tab)].2 }
}

/// Wizard101's plain tooltip (ToolTip.gui): the ToolTip_Bckgrnd widget's nine bubble pieces (16 px each, white,
/// rounded) around ToolTipText, which sits 8 px inside them (8,8 .. 192,42 of 200 x 50) in black WizardNormal 0.4,
/// centred both ways; the container fades in (EFFECT_FADEIN).
const BUBBLE: [&str; 9] = ["GUI/Tooltip/bubble_TL.png", "GUI/Tooltip/bubble_T.png", "GUI/Tooltip/bubble_TR.png",
                           "GUI/Tooltip/bubble_L.png", "GUI/Tooltip/bubble_R.png", "GUI/Tooltip/bubble_BL.png",
                           "GUI/Tooltip/bubble_B.png", "GUI/Tooltip/bubble_BR.png", "GUI/Tooltip/bubble_C.png"];
/// The bookmarks' hover texts, in TABS order (Wizard101's, without its key bindings; Options is the Settings page).
const BOOKMARK_TIPS: [&str; 10] = ["Character", "Backpack", "Pets", "Spell Deck", "Quests", "Map", "Crafting", "Help",
                                   "Options", "Close"];
const BUBBLE_PAD: f32 = 8.0;
const BUBBLE_SCALE: f32 = 0.4;
/// The widest a bubble's text runs before it wraps (page pixels), and the seconds it takes to fade in.
const BUBBLE_TEXT_MAX: f32 = 240.0;
const BUBBLE_FADE: f32 = 0.2;

/// Wizard101's plain tooltip (see BUBBLE) with `text`, beside the mouse (below and right of the wand, kept on the
/// screen), at opacity `fade`.
fn draw_bubble(canvas: &Canvas, text: &str, mouse: [f32; 2], screen: [f32; 2], fade: f32) {
    let Some(font) = canvas.fonts.get("WizardNormal") else { return };
    let width_of = |t: &str| font.width(t) * BUBBLE_SCALE;
    // Word-wrapped lines, each centred.
    let mut lines: Vec<String> = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && width_of(&next) > BUBBLE_TEXT_MAX {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = next;
            }
        }
        lines.push(line);
    }
    let line_h = font.line_height * BUBBLE_SCALE;
    let text_w = lines.iter().map(|l| width_of(l)).fold(0.0, f32::max);
    let (w, h) = (text_w + 2.0 * BUBBLE_PAD + 6.0, lines.len() as f32 * line_h + 2.0 * BUBBLE_PAD + 2.0);
    let right = (screen[0] - canvas.origin[0]) / canvas.s;
    let bottom = (screen[1] - canvas.origin[1]) / canvas.s;
    let mut x = mouse[0] + 14.0;
    if x + w > right - 4.0 {
        x = mouse[0] - 6.0 - w;
    }
    let mut y = mouse[1] + 26.0;
    if y + h > bottom - 4.0 {
        y = mouse[1] - 6.0 - h;
    }
    let faded = Canvas {
        draw: canvas.draw,
        textures: canvas.textures,
        fonts: canvas.fonts,
        origin: canvas.origin,
        s: canvas.s,
        mouse: canvas.mouse,
        mouse_down: false,
        alpha: canvas.alpha * fade,
        hole: Cell::new(None),
    };
    let pieces: [Option<String>; 9] = BUBBLE.map(|p| Some(p.to_string()));
    // (The canvas's alpha fades the pieces and the text.)
    faded.border(&pieces, [x, y, x + w, y + h], ImColor32::WHITE);
    let black = ImColor32::from_rgb(0, 0, 0);
    let mut ly = y + BUBBLE_PAD + 1.0;
    for line in &lines {
        faded.text(&format!("<center>{line}</center>"), [x + BUBBLE_PAD, ly, x + w - BUBBLE_PAD, ly + line_h],
                   &TextStyle::new("WizardNormal", BUBBLE_SCALE, black), false);
        ly += line_h;
    }
}

/// Wizard101's wand cursor (GUI/cursor.dds, from GUI/Cursors.xml's "Default").
pub const CURSOR: &str = "GUI/cursor.png";

/// The book's mouse cursor (Elden Ring hides the system one): Wizard101's star wand, its hot spot at the star's
/// tip (5, 5 of the 45-pixel cursor.cur); an arrow until the picture has loaded.
fn draw_cursor(draw: &hudhook::imgui::DrawListMut, textures: &Textures, p: [f32; 2], screen_h: f32) {
    if let Some((wand, _)) = textures.get(CURSOR) {
        let size = 42.0 * (screen_h / 1080.0).max(0.75);
        let hot = size * 5.0 / 45.0;
        let at = [p[0] - hot, p[1] - hot];
        draw.add_image(wand, at, [at[0] + size, at[1] + size]).build();
        return;
    }
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
            map_leaving: None,
            map_back: None,
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
            tracked: None,
            nav: Default::default(),
            helper_state: String::new(),
            arrow_bearing: None,
            sight: Default::default(),
            hud_scale: 1.0,
            quest_flags: Vec::new(),
            flags: HashMap::new(),
            flags_time: None,
            popups: Default::default(),
            picker: None,
            picker_mouse: false,
            picker_pinned: false,
            prompt: Default::default(),
            prompt_top: None,
            preview_prompt: None,
            active: Vec::new(),
            requests: Vec::new(),
            dialogue: None,
            cursor: [0.0; 2],
            last_os: [-1.0; 2],
            di_buttons: 0,
            last_click: None,
            last_hit: None,
            dragging: false,
            model: None,
            model_textures: [None, None],
            yaw: 0.0,
            turned: None,
            spin: -1.0,
            spin_last: -1.0,
            drag_turned: 0.0,
            was_dragging: false,
            zoom: None,
            after_map: None,
            warp_after_map: None,
            map_on_open: false,
            quit_asked: None,
            staged: Default::default(),
            quitting: None,
            face_logged: false,
            flasks_logged: false,
            achieved: Vec::new(),
            bubble: None,
            last_frame: Instant::now(),
            frames: 0,
            world_ready: false,
            map_shot: false,
            gear: Vec::new(),
            gear_path: None,
            gear_queue: None,
        }
    }

    /// Loads the fonts, layouts, item data and quests from `folder` (w101_orbs/book); the art loads when it is
    /// first drawn.
    pub fn load(&mut self, folder: &Path, render: &mut dyn RenderContext) {
        self.folder = folder.to_path_buf();
        for font in ["WizardFancy", "WizardNormal", "SpellDescription", "SpellTitle_sm"] {
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
        for (_, _, page) in TABS.iter().filter(|t| !t.2.is_empty())
            .chain([&(Tab::Quests, "", pages::QUEST_CARD), &(Tab::Backpack, "", pages::GEAR_PAGE)]) {
            match Layout::load(&folder.join(format!("{page}.lay"))) {
                Ok(l) => {
                    self.layouts.insert(page, l);
                }
                Err(e) => tracing::warn!("spellbook page: {e}"),
            }
        }
        self.items = ItemData::load(&folder.join("er"));
        crate::crafting::load(&folder.join("er"));
        self.quests = Quests::load(&folder.join("quests.tsv"));
        self.nav = crate::questhelper::Nav::load(&folder.join("quest_nav.tsv"));
        self.quest_flags = self.quests.flags();
        self.quest_flags.extend(markers::flags());
        // w101_quests: the talk events' conversation requests (round 17) and Wizard101's dialogue box.
        self.requests = talk::load(&folder.join("dialogue.tsv"));
        markers::er_portraits(&mut self.requests, folder); // round 18 G3: the Elden Ring faces
        // Round 25 (H3): the interact prompt's key and whether to blank Elden Ring's own prompt (w101_orbs.ini).
        if let Some(orbs) = folder.parent().and_then(|p| p.parent()) {
            self.prompt.load(&orbs.join("w101_orbs.ini"));
        }
        self.quest_flags.extend(self.requests.iter().map(|r| r.flag));
        for page in [talk::WINDOW, talk::IMAGE, crate::questhelper::LAYOUT] {
            match Layout::load(&folder.join(format!("{page}.lay"))) {
                Ok(l) => {
                    self.layouts.insert(page, l);
                }
                Err(e) => tracing::warn!("dialogue: {e}"),
            }
        }
        self.quest_flags.sort();
        self.quest_flags.dedup();
        // The game's parts folder: next to eldenring.exe (W101_PARTS overrides, for the preview).
        let parts = std::env::var_os("W101_PARTS").map(PathBuf::from).or_else(|| {
            std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("parts")))
        });
        let idle = folder.join("er").join("idle_pose.tsv");
        self.model = parts.map(|p| Renderer::new(p).with_idle(idle));
        tracing::info!("spellbook: {} page layouts, {} fonts, {} quests", self.layouts.len(), self.fonts.len(),
                       self.quests.quests.len());
    }

    /// Loads queued art and new character pictures (render thread, before the frame).
    pub fn load_art(&mut self, render: &mut dyn RenderContext) {
        if self.textures.pending() {
            let folder = self.folder.join("art");
            self.textures.load_wanted(&folder, render);
        }
        let Some(renderer) = self.model.as_ref() else { return };
        for slot in 0..2 {
            let Some(frame) = renderer.take(slot) else { continue };
            let [w, h] = frame.size;
            let id = match self.model_textures[slot] {
                Some((id, size)) if size == frame.size => render.replace_texture(id, &frame.rgba, w, h).ok().map(|_| id),
                _ => render.load_texture(&frame.rgba, w, h).ok(),
            };
            match id {
                Some(id) => {
                    self.model_textures[slot] = Some((id, frame.size));
                    let name = [pages::MODEL_FULL, pages::MODEL_PORTRAIT][slot];
                    self.textures.set(name, id, [w as f32, h as f32]);
                }
                None => tracing::warn!("character picture {slot}: texture upload failed"),
            }
        }
    }

    /// Asks the renderer for this frame's screen copies (before the frame is drawn): while Elden Ring's map is open,
    /// the map, to draw on the map page; otherwise the world (every frame while the book is open, every 10th while
    /// it is shut), so that the last copy from before the map opened can stand behind the book. `enabled`: the
    /// bookmap setting.
    pub fn capture(&mut self, render: &mut dyn RenderContext, enabled: bool) {
        self.frames += 1;
        self.map_shot = false;
        if !enabled {
            self.world_ready = false;
            return;
        }
        match self.map {
            MapState::Idle if self.open || self.frames % 10 == 0 => {
                if let Some(id) = render.capture_screen(0) {
                    self.textures.set(WORLD_PICTURE, id, [1.0, 1.0]);
                    self.world_ready = true;
                }
            }
            // Once the map is closing, the last copy of it stays (the closing map is not copied).
            MapState::Open if self.map_leaving.is_none() && self.after_map.is_none() => {
                if let Some(id) = render.capture_screen(1) {
                    self.textures.set(pages::MAP_PICTURE, id, [1.0, 1.0]);
                    self.map_shot = true;
                }
            }
            _ => {}
        }
    }

    /// Waits until the character pictures asked for are drawn (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_wait_models(&self) {
        if let Some(r) = &self.model {
            r.wait_idle(Duration::from_secs(20));
        }
    }

    /// What the character looks like: worn armour, the weapon in the right hand, body type, hair.
    fn look(&self, sheet: &Sheet) -> Look {
        let e = &sheet.equipment;
        let mut look = Look { male: sheet.gender != 0, hair_colour: [0.22, 0.15, 0.09], ..Look::default() };
        for slot in 12..16 {
            // 10000-10300 are the bare head, body, arms and legs.
            if let Some((stem, hide)) = e[slot].try_into().ok().and_then(|id: u32| self.items.armour_model(id)) {
                look.armour.push(stem);
                look.hide |= hide;
            }
        }
        let right = e[1 + 2 * (sheet.weapon_slots[1].min(2) as usize)];
        look.weapon = u32::try_from(right).ok().and_then(|id| self.items.weapon_model(id));
        look.hair = hair_model(&sheet.face);
        look
    }

    /// Asks for the character pictures the page shows, at the size they are drawn.
    fn request_models(&mut self, view: &View, page: Option<&Layout>, s: f32, dt: f32, pointer_turn: f32) {
        let (Some(page), Some(sheet)) = (page, self.sheet.as_ref()) else { return };
        let mut look = self.look(sheet);
        if self.tab == Tab::Backpack && self.state.item_model.is_some() {
            // An arrow or bolt on its own.
            look = Look { item: self.state.item_model, ..Look::default() };
        }
        if !self.face_logged && !sheet.face.is_empty() {
            self.face_logged = true;
            let hex: Vec<String> = sheet.face.iter().take(64).map(|b| format!("{b:02x}")).collect();
            tracing::info!("character: gender {} face {} look {:?}", sheet.gender, hex.join(""), look);
        }
        // Round 19 A5: the right stick moves the wand now; the picture turns by its slow spin and drags only.
        let stick = 0.0f32;
        let turn = pointer_turn;
        if turn != 0.0 {
            self.yaw = (self.yaw + turn) % std::f32::consts::TAU;
            self.turned = Some(Instant::now());
            self.drag_turned += turn.abs();
            // Let go, it carries on the way it was turned.
            self.spin_last = turn.signum();
            if self.spin != 0.0 || stick.abs() > 0.2 {
                self.spin = self.spin_last;
            }
        }
        if self.dragging && !self.was_dragging {
            self.drag_turned = 0.0;
        }
        if !self.dragging && self.was_dragging {
            if self.drag_turned < 0.02 {
                // A click on the picture stops it, another starts it again.
                self.spin = if self.spin == 0.0 { self.spin_last } else { 0.0 };
            } else {
                self.spin = self.spin_last;
            }
        }
        self.was_dragging = self.dragging;
        let full = view.node_images.values().any(|n| n == pages::MODEL_FULL);
        let by_hand = self.dragging || self.turned.is_some_and(|t| t.elapsed() < Duration::from_millis(300));
        let spinning = full && !by_hand && self.spin != 0.0;
        if spinning {
            self.yaw = (self.yaw + self.spin * SPIN * dt) % std::f32::consts::TAU;
        }
        // Wizard101's backpack zooms on what the category is worn on.
        let target = if self.tab == Tab::Backpack && look.item.is_none() {
            pages::backpack_zoom(self.state.sub)
        } else {
            Framing::Full.view(true)
        };
        let zoom = self.zoom.get_or_insert(target);
        let moving = (zoom[0] - target[0]).abs() + (zoom[1] - target[1]).abs() > 0.002;
        if moving {
            let k = 1.0 - (-dt * ZOOM_RATE).exp();
            *zoom = [zoom[0] + (target[0] - zoom[0]) * k, zoom[1] + (target[1] - zoom[1]) * k];
        } else {
            *zoom = target;
        }
        let zoom = *zoom;
        let fast = by_hand || spinning || moving;
        let Some(renderer) = self.model.as_mut() else { return };
        for (node, name) in &view.node_images {
            let (slot, framing, yaw, view) = match name.as_str() {
                pages::MODEL_FULL => (0, Framing::Full, self.yaw, Some(zoom)),
                pages::MODEL_PORTRAIT => (1, Framing::Portrait, 0.0, None),
                _ => continue,
            };
            let r = page.nodes[*node].rect;
            let size = [((r[2] - r[0]) * s).round().max(8.0) as u32, ((r[3] - r[1]) * s).round().max(8.0) as u32];
            // The live mirror picture (live_preview.rs) stands in for the character, not for an arrow on its own;
            // it is framed like this picture and turned the way this one turns (its slow spin, a drag).
            if slot == 0 && look.item.is_none() {
                // Round 18: the slow spin turns ER's model only when the ini asks (its stick turns the world camera).
                let rate = if by_hand { pointer_turn / dt.max(1e-3) }
                    else if spinning { self.spin * SPIN } else { 0.0 };
                crate::live_preview::note_box(size, zoom, rate);
            } else if slot == 1 {
                crate::live_preview::note_portrait(size);
            }
            renderer.request(slot, Request { look: look.clone(), framing, yaw, size, fast: fast && slot == 0, view });
        }
    }

    /// Opens the book on `tab` / sub-page `sub` (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_show(&mut self, tab: Tab, sub: usize) {
        self.dialogue = None;
        self.picker = None;
        self.map = MapState::Idle;
        self.set_open(true);
        self.opened_at = Instant::now() - Duration::from_secs(1);
        self.tab = tab;
        self.state.reset();
        self.state.sub = sub;
    }

    /// The Backpack's Equipment Sets page with three sets, the first being what `game`'s character wears (the
    /// preview renderer).
    #[allow(dead_code)]
    pub fn preview_gear(&mut self, game: &game::Game) {
        let Some(sheet) = game.sheet() else { return };
        let worn = pages::gear_now(&sheet);
        let mut other = worn.clone();
        for (slot, id) in other.iter_mut() {
            if *slot == 12 {
                *id = 10000;
            }
            if *slot == 17 {
                *id = -1;
            }
        }
        let school = pages::char_school(&sheet);
        self.gear = (0..3).map(|n| (sheet.name.clone(), pages::GearSet {
            name: pages::gear_name(school, n), slots: if n == 0 { worn.clone() } else { other.clone() } })).collect();
        self.gear_path = Some(PathBuf::from("/nonexistent/gear.tsv"));
        self.state.gear_view = true;
        self.state.gear_chosen = 1;
    }

    /// Turns the current page's list to page `page` (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_page(&mut self, page: usize) {
        self.state.page = page;
    }

    /// Shows the book around Elden Ring's open map (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_map(&mut self) {
        self.preview_show(Tab::Map, 0);
        self.set_open(false);
        self.map = MapState::Open;
    }

    /// Closes the book for a gameplay frame (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_closed(&mut self) {
        self.set_open(false);
        self.holding = false;
    }

    /// Shows the crafting page's materials (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_materials(&mut self) {
        self.state.craft.materials = true;
    }

    /// Shows the crafting page with recipe `recipe` chosen, and its "You made a" panel if `made` (the preview).
    #[allow(dead_code)]
    pub fn preview_recipe(&mut self, recipe: usize, made: bool) {
        self.state.craft.materials = false;
        self.state.craft.recipe = recipe;
        if made {
            self.state.craft.preview_made(recipe);
        }
    }

    /// Shows tab `tab` of the Settings page (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_settings(&mut self, tab: usize) {
        self.state.page = tab;
    }

    /// Shows pet `pet` (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_pet(&mut self, pet: usize) {
        self.state.pet = pet;
    }

    /// Round 25: the quest picker for conversations `keys` (dialogue.tsv), row `focus` lit (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_picker(&mut self, keys: &[&str], focus: usize) {
        self.set_open(false);
        let rows: Vec<crate::picker::Row> = keys.iter().filter_map(|k| self.requests.iter().position(|r| r.key == *k))
            .map(|i| crate::picker::Row {
                request: i,
                icon: crate::picker::icon(&self.requests[i].kind),
                label: self.quests.quests.iter().find(|q| q.slug == self.requests[i].quest)
                    .map_or(self.requests[i].quest.clone(), |q| q.title.clone()),
            }).collect();
        let Some(first) = rows.first().map(|r| r.request) else { return };
        self.picker_pinned = true;
        self.picker = Some(crate::picker::Picker { speaker: self.requests[first].speaker.clone(),
            name: self.requests[first].name.clone(), portrait: self.requests[first].portrait.clone(), rows, focus,
            opened: Instant::now() - Duration::from_secs(1) });
    }

    /// Round 25: the interact prompt for `target` with the controller's (`pad`) or the keyboard's key (the preview).
    #[allow(dead_code)]
    pub fn preview_prompt(&mut self, target: Option<crate::prompt::Target>, pad: bool) {
        self.preview_prompt = target;
        self.prompt.pad = pad;
        self.prompt.preview = true;
    }

    /// Shows conversation `key` (dialogue.tsv) on page `page` (the preview renderer).
    #[allow(dead_code)]
    pub fn preview_talk(&mut self, key: &str, page: usize) {
        self.set_open(false);
        if let Some(k) = self.requests.iter().position(|r| r.key == key) {
            if let Some(mut c) = self.conversation(k) {
                c.page = page.min(c.pages.len().saturating_sub(1));
                c.opened = Instant::now() - Duration::from_secs(1);
                self.dialogue = Some(c);
            }
        }
    }

    /// Draws the quest markers over the quest givers (markers.rs) in normal play, from the last quest flags read.
    pub fn draw_markers(&mut self, ui: &Ui, cx: &Context) {
        self.draw_prompt(ui, cx);
        self.textures.want([markers::OFFER, markers::TALK, markers::SPIRAL].map(String::from));
        if !cx.gameplay || self.blocking() || self.map != MapState::Idle || self.flags.is_empty() {
            return;
        }
        let Some(game) = cx.game else { return };
        let Some(camera) = game.camera() else { return };
        // Round 18 G1: the NPCs where the game has them (their MSB spots only for ones it hasn't loaded).
        // Round 22 (H5): positions that jump for a frame are refused (markers::steady).
        let position = game.position();
        let entities = markers::steady(&camera, &game.entities(&markers::MODELS, &markers::entity_ids()));
        let spots = markers::locate(&camera, position, &entities);
        let draw = ui.get_background_draw_list();
        let render = game.entity_render_bits(&markers::MODELS, &markers::entity_ids());
        // Round 22 (H6): who is in sight (the player's MSB position as the Quest Helper finds it).
        let player = crate::questhelper::player_msb_from_npcs(&camera, &entities)
            .or_else(|| position.and_then(crate::questhelper::player_msb));
        self.sight.update(&self.nav, player, ui.time() as f32);
        markers::draw(&draw, &self.textures, &camera, position, &entities, &spots, &render, &self.flags, &self.sight,
                      ui.time() as f32, ui.io().display_size);
    }

    /// Round 23 (H7): the quest popups (goalpopup.rs), shown in normal play while no book or map is up. Round 25 (H1):
    /// over the dialogue box too, as Wizard101 shows them over its quest picker.
    pub fn draw_goal_popup(&mut self, ui: &Ui, cx: &Context) {
        let visible = cx.gameplay && !self.open && !self.holding && self.map == MapState::Idle;
        let draw = ui.get_foreground_draw_list();
        self.popups.draw(&draw, &self.textures, &self.fonts, ui.io().display_size, visible,
                         ui.io().delta_time.clamp(0.0, 0.1));
    }

    /// Wizard101's Quest Helper (questhelper.rs, round 18 quests F4): the yellow arrow toward the current goal of
    /// the first quest in progress, and the goal's name, on the street in normal play.
    pub fn draw_quest_helper(&mut self, ui: &Ui, cx: &Context) {
        use crate::questhelper as qh;
        // Round 21 (Q6): Ryan saw no helper with a quest in progress and the log could not say why; every reason it
        // isn't drawn is logged once when it changes.
        let (active, goal, player, camera) = match self.quest_helper_target(cx) {
            Ok(v) => v,
            Err(why) => {
                self.helper_log(why.to_string());
                self.arrow_bearing = None;
                return;
            }
        };
        let steps = qh::steps(&self.nav, player, goal);
        self.helper_log(format!("showing {} goal {} ({:?}), about {} steps, player at MSB [{:.0}, {:.0}, {:.0}]",
                                active.slug, active.goal_index, active.goal, steps / 10 * 10, player[0], player[1],
                                player[2]));
        let Some(layout) = self.layouts.get(qh::LAYOUT) else { return };
        self.textures.want(layout.images());
        let screen = ui.io().display_size;
        // Round 25 (H3): over Wizard101's interact prompt while it is up (its box's top less PROMPT_GAP), as in Ryan's
        // screenshot ref/4 (the goal's name 28 layout pixels over the prompt's scroll).
        const PROMPT_GAP: f32 = 14.0;
        let bar_top = match self.prompt_top {
            Some(top) => top - PROMPT_GAP * screen[1] / 768.0,
            None => talk::xp_bar_top(screen, self.hud_scale),
        };
        let (s, origin) = qh::placement(layout, screen, bar_top);
        let draw = ui.get_background_draw_list();
        // Round 22 (H2): the bearing eased (about 0.1 s) so a jittery read doesn't shake the arrow.
        let target = qh::bearing(&camera, player, goal);
        let bearing = match self.arrow_bearing {
            Some(b) => {
                let k = 1.0 - (-ui.io().delta_time.clamp(0.0, 0.1) * 12.0).exp();
                let turn = (target - b + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                b + turn * k
            }
            None => target,
        };
        self.arrow_bearing = Some(bearing);
        qh::draw_arrow(&draw, layout, bearing, origin, s, 1.0);
        let canvas = Canvas {
            draw: &draw,
            textures: &self.textures,
            fonts: &self.fonts,
            origin,
            s,
            mouse: [-1.0e4; 2],
            mouse_down: false,
            alpha: 1.0,
            hole: Cell::new(None),
        };
        layout::draw(layout, &qh::view(layout, &active, steps), &canvas, |_| true);
    }

    /// Round 21 (Q6): what the Quest Helper shows (quest, goal point and player, both in MSB, camera), or why it is
    /// hidden.
    fn quest_helper_target(&self, cx: &Context) -> Result<(Active, [f32; 3], [f32; 3], markers::Camera), &'static str> {
        use crate::questhelper as qh;
        if !cx.gameplay || !cx.hud {
            return Err("hidden: not in normal play");
        }
        if self.blocking() || self.map != MapState::Idle {
            return Err("hidden: book, dialogue or map open");
        }
        if self.flags.is_empty() {
            return Err("hidden: quest flags not read yet");
        }
        let Some(game) = cx.game else { return Err("hidden: no game") };
        if self.layouts.get(qh::LAYOUT).is_none() {
            return Err("hidden: QuestHelperHud.lay not loaded");
        }
        let Some(camera) = game.camera() else { return Err("hidden: no camera") };
        if self.active.is_empty() {
            return Err("hidden: no quest in progress");
        }
        // The player's MSB position: from the street NPCs the game has loaded (their MSB spot plus the player's
        // offset from them in the physics space), else from the block origin (round 20's only way).
        let entities = markers::steady(&camera, &game.entities(&markers::MODELS, &markers::entity_ids()));
        let player = qh::player_msb_from_npcs(&camera, &entities).or_else(|| game.position().and_then(qh::player_msb));
        let Some(player) = player else { return Err("hidden: player not on Unicorn Way's tile") };
        // Round 25 (H7, Ryan: "No quest arrow inside the boss rooms/towers (Rattlebones etc.)"): the tower and boss
        // rooms all sit under the street (questhelper::in_room); Wizard101 shows no helper arrow inside them.
        if qh::in_room(player) {
            return Err("hidden: player in a tower / boss room");
        }
        let tracked = self.tracked_index().and_then(|i| self.active.get(i));
        let Some((active, goal)) = tracked.into_iter().chain(self.active.iter())
            .find_map(|a| qh::target(a, player).map(|g| (a, g)))
        else {
            return Err("hidden: no goal of the quests in progress has a place");
        };
        Ok((active.clone(), goal, player, camera))
    }

    fn helper_log(&mut self, state: String) {
        if state != self.helper_state {
            tracing::info!("quest helper: {state}");
            self.helper_state = state;
        }
    }

    /// Round 20 (Q7): the tracked quest's index in `active`: the one picked on the Quests page while it is in
    /// progress, else the first (Wizard101 follows the main quest unless another is picked).
    fn tracked_index(&self) -> Option<usize> {
        if self.active.is_empty() {
            return None;
        }
        Some(self.tracked.as_ref().and_then(|t| self.active.iter().position(|a| &a.slug == t)).unwrap_or(0))
    }

    /// Wizard101's nameplates over the Unicorn Way enemies (nameplates.rs) in normal play.
    pub fn draw_nameplates(&mut self, ui: &Ui, cx: &Context) {
        if !cx.gameplay || self.blocking() || self.map != MapState::Idle {
            return;
        }
        let Some(game) = cx.game else { return };
        let Some(camera) = game.camera() else { return };
        let enemies = game.enemies(&nameplates::models());
        let screen = ui.io().display_size;
        let mut plates = nameplates::plates(&camera, &enemies, screen);
        // w101_quests: the street NPCs' names (markers.rs), as the creature plates without the rank line
        let entities = markers::steady(&camera, &game.entities(&markers::MODELS, &markers::entity_ids()));
        let player = crate::questhelper::player_msb_from_npcs(&camera, &entities)
            .or_else(|| game.position().and_then(crate::questhelper::player_msb));
        self.sight.update(&self.nav, player, ui.time() as f32);
        plates.extend(markers::npc_plates(&camera, &markers::locate(&camera, game.position(), &entities), &self.sight,
                                          screen));
        if plates.is_empty() {
            return;
        }
        let draw = ui.get_background_draw_list();
        let Some(fancy) = self.fonts.get("WizardFancy") else { return };
        let mut icons = Vec::new();
        for plate in plates {
            // As Wizard101 shows them (reference nameplate_lost_soul.png): no plate, the name in big WizardFancy
            // capitals with a dark outline in the name colour, and under it the school's icon and "RANK n" in it too.
            let icon = (!plate.school.is_empty()).then(|| format!("GUI/Icons/Icon_{}.png", plate.school));
            let name = plate.name.to_uppercase();
            let line = plate.line.to_uppercase();
            let (name_scale, line_scale, icon_side) = (NAME_SCALE, RANK_SCALE, RANK_SCALE * 38.0);
            let name_w = fancy.width(&name) * name_scale;
            let line_w = fancy.width(&line) * line_scale + if icon.is_some() { icon_side + 3.0 } else { 0.0 };
            let w = name_w.max(line_w) + 8.0;
            // An NPC's plate has no second line: its name sits at the plate's bottom.
            let line_h = if line.is_empty() && icon.is_none() { 0.0 } else { fancy.line_height * line_scale };
            let name_h = fancy.line_height * name_scale;
            let h = name_h + line_h;
            let origin = [plate.at[0] - w / 2.0 * plate.k, plate.at[1] - h * plate.k];
            let canvas = Canvas {
                draw: &draw,
                textures: &self.textures,
                fonts: &self.fonts,
                origin,
                s: plate.k,
                mouse: [-1.0e4; 2],
                mouse_down: false,
                alpha: plate.alpha,
                hole: Cell::new(None),
            };
            let tint = ImColor32::from_rgba(255, 255, 255, (255.0 * plate.alpha) as u8);
            // Round 18 G2: Wizard101's name colour (NPCBehaviorTemplate m_nameColor, nameplates.rs / markers.rs).
            let [r, g, b] = plate.colour;
            let yellow = ImColor32::from_rgba(r, g, b, (255.0 * plate.alpha) as u8);
            canvas.text(&name, [0.0, 0.0, w, name_h], &TextStyle::new("WizardFancy", name_scale, yellow).centred().outlined(), false);
            let x0 = (w - line_w) / 2.0;
            if let Some(icon) = &icon {
                let iy = name_h + (line_h - icon_side) / 2.0;
                canvas.image(icon, [x0, iy, x0 + icon_side, iy + icon_side], [0.0, 0.0, 1.0, 1.0], tint);
                icons.push(icon.clone());
            }
            let tx = x0 + if icon.is_some() { icon_side + 3.0 } else { 0.0 };
            canvas.text(&line, [tx, name_h, w, h], &TextStyle::new("WizardFancy", line_scale, yellow).outlined(), false);
        }
        self.textures.want(icons);
    }

    /// The game window lost or regained focus (alt-tab): drop the mouse state the book was tracking (a button
    /// held when the window went away, a drag), so the open book takes clicks, keys and the pad again at once.
    pub fn focus_changed(&mut self, focused: bool) {
        tracing::info!("spellbook: game window {} (book open: {})", if focused { "focused" } else { "lost focus" }, self.open);
        self.di_buttons = 0;
        self.dragging = false;
        self.was_dragging = false;
        self.last_click = None;
        self.last_hit = None;
        self.last_os = [-1.0, -1.0];
        self.last_frame = Instant::now();
    }

    /// Whether Wizard101's dialogue box (talk.rs) is up.
    pub fn talking(&self) -> bool {
        self.dialogue.is_some() || self.picker.is_some()
    }

    /// Whether the game's keyboard, mouse and controller input are blocked.
    pub fn blocking(&self) -> bool {
        self.open || self.holding || self.talking()
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
        self.hud_scale = cx.settings.scale;
        input::set_gameplay(cx.gameplay && !self.open && !self.talking());
        // M1: the character still walks with the book open (WASD, left stick), except on the map page.
        // Not over Elden Ring's own menus, except the book's own live mirror (round 18 A1: the game keeps going under
        // it; live_preview logs whether the player really walks there).
        input::set_move_through(self.open && cx.gameplay && (cx.hud || crate::live_preview::walk_under_mirror())
            && !self.talking() && self.tab != Tab::Map);
        // Round 17 B3: the hidden live mirror may stay up while the book is open off the map page.
        crate::live_preview::note_book(self.open && !self.talking() && self.tab != Tab::Map);
        // Before taking the presses, so that the press that closed ER's map doesn't close the reopened book.
        self.follow_map(cx);
        self.quit_follow(cx);
        let pressed = input::take_pressed();
        let pointer = self.pointer(ui, dt);
        if cx.game.is_some() {
            self.poll_quests(cx);
        }
        self.gear_step(cx);
        if self.dialogue.is_some() {
            self.dialogue_frame(ui, pressed, &pointer);
            return;
        }
        if self.picker.is_some() {
            self.picker_frame(ui, pressed, &pointer);
            return;
        }
        if !self.open {
            self.dragging = false;
            let map_shown = self.map == MapState::Open && cx.game.is_some() && cx.faded < 0.5 && !self.travelled;
            if map_shown && self.warp_after_map.is_some() {
                // Leaving the map for a warp: the world as it was before the map opened stands in front of the
                // closing map; the book itself is gone (the poof plays in the world next).
                if let Some((world, _)) = self.textures.get(WORLD_PICTURE) {
                    let size = ui.io().display_size;
                    ui.get_foreground_draw_list().add_image(world, [0.0, 0.0], size).build();
                }
            } else if map_shown && (self.after_map.is_some() || self.map_leaving.is_some()) {
                // A book tab was clicked on the map page, or the map is closing: the page to come straight away,
                // over the world as it was before the map opened, while Elden Ring closes its map behind it (the
                // book is never off the screen, so the game doesn't flash through).
                self.refresh(cx);
                self.draw(ui, cx, &pointer, dt, false);
            } else if map_shown {
                // Elden Ring's map is open: the book around it. A click on another book tab closes the map and
                // shows that tab when the map has gone.
                self.refresh(cx);
                let (tab_hit, page_hit, _, _) = self.draw(ui, cx, &pointer, dt, true);
                if pointer.clicked && self.after_map.is_none() && self.warp_after_map.is_none() {
                    let prefs = self.prefs.as_ref().expect("checked above");
                    let name = tab_hit.map(|h| prefs.nodes[h].name.clone()).unwrap_or_default();
                    // The page's warp buttons (the arrows: The First Step; the house: Roundtable Hold) stay on the
                    // page around the map: close the map, then warp (follow_map).
                    let action = match (tab_hit, page_hit, self.layouts.get(TABS[tab_index(Tab::Map)].2)) {
                        (None, Some(hit), Some(page)) => pages::click(Tab::Map, page, hit, &mut self.state),
                        _ => Action::None,
                    };
                    let warp = match action {
                        Action::Warp(flag) => Some(flag),
                        _ => None,
                    };
                    if matches!(action, Action::OpenMap) {
                        // The magnifier (plus while Elden Ring's map is open): back to Wizard101's map, the way a
                        // book tab goes back (follow_map reopens the book on the Map page once the map is gone).
                        tracing::info!("spellbook: magnifier on Elden Ring's map; back to the World Map page");
                        self.after_map = Some(Tab::Map);
                        self.tab = Tab::Map;
                        self.opened_at = Instant::now() - Duration::from_secs(1);
                        input::press_map();
                    } else if let Some(flag) = warp {
                        tracing::info!("map page: warp flag {flag} (closing Elden Ring's map first)");
                        self.warp_after_map = Some(flag);
                        self.after_map = None;
                        self.opened_at = Instant::now() - Duration::from_secs(1);
                        input::press_map();
                    } else if let Some((tab, _, _)) = TABS.iter().find(|t| t.1 == name && t.0 != Tab::Map) {
                        tracing::info!("spellbook: leaving the map for {tab:?}");
                        self.after_map = Some(*tab);
                        self.tab = *tab;
                        self.state.reset();
                        self.opened_at = Instant::now() - Duration::from_secs(1);
                        input::press_map();
                    }
                }
            }
            let quiet = self.closed_at.is_none_or(|t| t.elapsed().as_secs_f32() > 0.25);
            if self.holding && quiet && input::held().0 == 0 {
                self.holding = false;
            }
            let key_b = ui.is_key_pressed_no_repeat(Key::B);
            if cx.gameplay && self.map == MapState::Idle && (pressed & input::BACK != 0 || key_b) {
                self.set_open(true);
                // The World Map page shows Wizard101's maps now; Elden Ring's map opens only from its magnifier.
                self.map_on_open = false;
            }
            input::set_book_open(self.blocking());
            return;
        }
        if !cx.gameplay {
            if self.map_back.is_some_and(|t| t.elapsed().as_secs_f32() < MAP_BACK_GRACE) && cx.faded < 0.5 {
                // Just back from the map and the game not quite settled (a frame of HUD change or fade): the book
                // stays, without input, rather than closing and flashing the game.
                self.refresh(cx);
                self.draw(ui, cx, &pointer, dt, false);
                return;
            }
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
        // Opened on the World Map page: open Elden Ring's map as soon as View / B is let go (pressing it again
        // while still held would reach the game as a second press and close the map again).
        if self.map_on_open {
            if self.tab != Tab::Map {
                self.map_on_open = false;
            } else if input::held().0 & input::BACK == 0 && !ui.is_key_down(Key::B)
                && self.opened_at.elapsed().as_secs_f32() > 0.05 {
                self.map_on_open = false;
                tracing::info!("spellbook: opened on the World Map page; opening Elden Ring's map");
                self.open_map(cx);
                return;
            }
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
                Tab::Settings => {
                    // Down the rows, then on to the next tab's (and back).
                    let tabs = crate::options::TABS.len();
                    let rows = |t: usize| crate::options::TABS[t % tabs].2.len().max(1);
                    let (mut page, mut sub) = (self.state.page % tabs, self.state.sub as i32 + step);
                    if sub < 0 {
                        page = (page + tabs - 1) % tabs;
                        sub = rows(page) as i32 - 1;
                    } else if sub as usize >= rows(page) {
                        page = (page + 1) % tabs;
                        sub = 0;
                    }
                    (self.state.page, self.state.sub) = (page, sub as usize);
                }
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
        let mut pointer = pointer;
        let a_click = pressed & input::A != 0 || ui.is_key_pressed_no_repeat(Key::Enter);
        if a_click {
            // A / Enter clicks where the wand is (round 19 A5: the right stick moves it on every page). Round 22 B4:
            // on Settings too; A used to be only the window's OK there, so Exit Game / Quit never got A's click.
            pointer.clicked = true;
        }

        let (tab_hit, page_hit, in_model, mouse) = self.draw(ui, cx, &pointer, dt, false);
        if a_click && self.tab == Tab::Settings && tab_hit.is_none() && page_hit.is_none()
            && !self.state.hits.iter().any(|(r, _)| layout::inside(mouse, *r)) {
            // A / Enter with the wand over nothing on Settings: still the Settings window's OK.
            self.settings_ok(cx);
            return;
        }

        // Turning the character picture: drag in its window.
        if pointer.clicked && in_model {
            self.dragging = true;
        }
        if !pointer.down {
            self.dragging = false;
        }

        if !pointer.clicked {
            return;
        }
        // The page's own things (backpack items, spell cards, deck slots).
        let hit = self.state.hits.iter().rev().find(|(r, _)| layout::inside(mouse, *r)).map(|h| h.1);
        if let Some(pages::Hit::Setting(row, step)) = hit {
            self.change_setting(row, step, cx);
            return;
        }
        if let Some(pages::Hit::Warp(flag)) = hit {
            // The grace on the map page: the same warp as the page's travel buttons.
            tracing::info!("map page: grace marker, warp flag {flag}");
            game::set_event_flag(flag, true);
            self.set_open(false);
            return;
        }
        if let Some(pages::Hit::Quest(k)) = hit {
            // Round 20 (Q7): track this quest (its card glows, Wizard101's tracker arrow at its top left).
            if let Some(a) = self.active.get(k) {
                tracing::info!("quests page: tracking {}", a.slug);
                self.tracked = Some(a.slug.clone());
            }
            return;
        }
        if let Some(hit) = hit {
            crate::music::sfx("HudSelect"); // round 22 S3: the .gui buttons' m_sSelectSound
            let double = self.last_hit.is_some_and(|(h, t)| h == hit && t.elapsed() < Duration::from_millis(450));
            self.last_hit = Some((hit, Instant::now()));
            self.page_thing(hit, double, cx);
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
        let page_name = page_of(self.tab, &self.state);
        let action = match self.layouts.get(page_name) {
            Some(page) => pages::click(self.tab, page, hit, &mut self.state),
            None => Action::None,
        };
        // Round 22 S3: W101's click sounds (DeckConfiguration.gui PageUp/PageDown ui_hudgenericnext_click, other
        // buttons HudSelect; a warp plays the teleport marker sound). Tabs and closing play their own.
        match action {
            Action::None | Action::Tab(_) | Action::Close => {}
            Action::Setting(..) => crate::music::sfx("ui_hudgenericnext_click"),
            Action::Warp(_) => crate::music::sfx("ui_teleport_marker"),
            _ => crate::music::sfx("HudSelect"),
        }
        match action {
            Action::None => {}
            Action::OpenMap => self.open_map(cx),
            Action::Close => self.set_open(false),
            Action::Tab(tab) => self.select(tab, false),
            Action::Setting(row, step) => self.change_setting(row, step, cx),
            Action::SettingsOk => self.settings_ok(cx),
            Action::Quit(desktop) => self.quit(desktop),
            Action::Warp(flag) => {
                tracing::info!("map page: warp flag {flag}");
                game::set_event_flag(flag, true);
                self.set_open(false);
            }
            Action::Equip(id) => {
                if let Some(sheet) = self.sheet.clone() {
                    self.equip(id, &sheet, cx);
                }
            }
            Action::GearSave => self.gear_save(cx),
            Action::GearEquip(i) => self.gear_equip(i, cx),
            Action::GearDelete(i) => self.gear_edit(i, 0, cx),
            Action::GearMove(i, step) => self.gear_edit(i, step, cx),
        }
    }

    /// A click on one of the page's own things. Spells: a card on the right memorises it (in the first free memory
    /// slot, if the slots left allow), a slot in the deck forgets it, a double click on a spell in the Backpack
    /// memorises or forgets it; anywhere, not only at a grace. Other items: Elden Ring has no equip call the book
    /// can make, so a double click says where to equip it.
    fn page_thing(&mut self, hit: Hit, double: bool, cx: &Context) {
        let Some(sheet) = self.sheet.clone() else { return };
        let note = |st: &mut pages::State, text: String| st.note = Some((text, Instant::now()));
        let learn = |book: &mut Book, spell: i32| {
            let name = book.names.as_ref().map(|n| n.item(0x4000_0000 | spell as u32)).unwrap_or_default();
            let total = pages::memory_slots(&sheet);
            let used = pages::slots_used(&sheet, &book.items);
            let cost = book.items.spell(spell as u32).map_or(1, |s| s.slots.max(1));
            let free = sheet.spells.iter().take(total as usize).position(|s| *s < 0);
            match (used + cost <= total, free, cx.game) {
                (false, _, _) | (_, None, _) => note(&mut book.state,
                    format!("Not enough memory slots for {name} ({cost} needed, {} free)", total.saturating_sub(used))),
                (true, Some(i), Some(game)) if game.set_spell(i, spell) => {
                    note(&mut book.state, format!("Memorised {name}"));
                    book.sheet_time = None;
                }
                _ => note(&mut book.state, "The memory slots can't be reached".into()),
            }
        };
        match hit {
            Hit::Learn(spell) => learn(self, spell),
            Hit::Forget(i) => {
                let spell = sheet.spells[i];
                let name = self.names.as_ref().map(|n| n.item(0x4000_0000 | spell as u32)).unwrap_or_default();
                // Elden Ring keeps the memorised spells packed at the front of the slots: close the gap.
                let mut rest: Vec<i32> = sheet.spells.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, s)| *s).collect();
                rest.push(-1);
                let ok = cx.game.is_some_and(|g| (i..14).all(|j| g.set_spell(j, rest[j])));
                if ok {
                    note(&mut self.state, format!("Forgot {name}"));
                    self.sheet_time = None;
                }
            }
            Hit::Item(id) if double && pages::is_spell(id) => {
                let raw = (id & 0x0FFF_FFFF) as i32;
                match sheet.spells.iter().position(|s| *s == raw) {
                    Some(i) => self.page_thing(Hit::Forget(i), false, cx),
                    None => learn(self, raw),
                }
            }
            Hit::Item(id) if double => {
                self.state.selected = Some(id);
                self.equip(id, &sheet, cx);
            }
            Hit::Item(id) => self.state.selected = Some(id),
            Hit::Sub(i) => {
                self.state.sub = i;
                self.state.page = 0;
            }
            Hit::GearSet(i) => {
                self.state.gear_chosen = i;
                if double {
                    self.gear_equip(i, cx);
                }
            }
            Hit::Setting(..) | Hit::Warp(_) | Hit::Quest(_) => {} // handled before (book::frame)
        }
    }

    /// The current character's equipment sets (loading the file the first time).
    fn gear_sets_of_character(&self) -> Vec<pages::GearSet> {
        let Some(sheet) = self.sheet.as_ref() else { return Vec::new() };
        self.gear.iter().filter(|(c, _)| *c == sheet.name).map(|(_, set)| set.clone()).collect()
    }

    fn gear_load(&mut self, cx: &Context) {
        if self.gear_path.is_some() {
            return;
        }
        let path = cx.settings_path.with_file_name(GEAR_FILE);
        self.gear = std::fs::read_to_string(&path).unwrap_or_default().lines().filter_map(|line| {
            let mut f = line.split('\t');
            let (who, name, slots) = (f.next()?, f.next()?, f.next()?);
            let slots = slots.split(',').filter_map(|e| {
                let (slot, id) = e.split_once(':')?;
                Some((slot.trim().parse().ok()?, id.trim().parse().ok()?))
            }).collect();
            Some((who.to_string(), pages::GearSet { name: name.to_string(), slots }))
        }).collect();
        tracing::info!("equipment sets: {} from {}", self.gear.len(), path.display());
        self.gear_path = Some(path);
    }

    fn gear_save_file(&self) {
        let Some(path) = &self.gear_path else { return };
        let text: String = self.gear.iter().map(|(who, set)| {
            let slots: Vec<String> = set.slots.iter().map(|(s, id)| format!("{s}:{id}")).collect();
            format!("{who}\t{}\t{}\n", set.name.replace('\t', " "), slots.join(","))
        }).collect();
        if let Err(e) = std::fs::write(path, text) {
            tracing::warn!("equipment sets not saved to {}: {e}", path.display());
        }
    }

    /// Indices into self.gear of the current character's sets, in order.
    fn gear_rows(&self) -> Vec<usize> {
        let Some(sheet) = self.sheet.as_ref() else { return Vec::new() };
        self.gear.iter().enumerate().filter(|(_, (c, _))| *c == sheet.name).map(|(i, _)| i).collect()
    }

    /// The Equipment Sets page's +: what the character wears now becomes a new set (Wizard101's set name).
    fn gear_save(&mut self, cx: &Context) {
        self.gear_load(cx);
        let Some(sheet) = self.sheet.clone() else { return };
        let rows = self.gear_rows();
        if rows.len() >= pages::GEAR_ROWS {
            self.state.note = Some(("No room for more equipment sets".into(), Instant::now()));
            return;
        }
        let used: Vec<String> = rows.iter().map(|i| self.gear[*i].1.name.clone()).collect();
        let school = pages::char_school(&sheet);
        let name = (0..).map(|n| pages::gear_name(school, n)).find(|n| !used.contains(n)).unwrap_or_default();
        self.gear.push((sheet.name.clone(), pages::GearSet { name: name.clone(), slots: pages::gear_now(&sheet) }));
        self.gear_save_file();
        self.state.gear_chosen = rows.len();
        self.state.note = Some((format!("Saved {name}"), Instant::now()));
        tracing::info!("equipment set saved: {name}");
    }

    /// Delete (step 0) or move (step -1 / +1) the character's set at row `row`.
    fn gear_edit(&mut self, row: usize, step: i32, cx: &Context) {
        self.gear_load(cx);
        let rows = self.gear_rows();
        let Some(&at) = rows.get(row) else { return };
        if step == 0 {
            let (_, set) = self.gear.remove(at);
            self.state.note = Some((format!("Deleted {}", set.name), Instant::now()));
            self.state.gear_chosen = row.saturating_sub(1);
        } else {
            let Some(&other) = rows.get((row as i32 + step).max(0) as usize).filter(|o| **o != at) else { return };
            self.gear.swap(at, other);
            self.state.gear_chosen = (row as i32 + step) as usize;
        }
        self.gear_save_file();
    }

    /// Puts on the character's set at row `row`: every slot that differs from what is worn, through Elden Ring's
    /// own equip function (Game::equip), one slot per frame (gear_step). Items no longer in the inventory are
    /// skipped and counted.
    fn gear_equip(&mut self, row: usize, cx: &Context) {
        self.gear_load(cx);
        let (Some(sheet), Some(&at)) = (self.sheet.clone(), self.gear_rows().get(row)) else { return };
        let set = self.gear[at].1.clone();
        if cx.game.is_none() {
            self.state.note = Some(("Equipping needs the game".into(), Instant::now()));
            return;
        }
        let have: std::collections::HashSet<u32> = sheet.items.iter().map(|(id, _)| *id).collect();
        let mut missing = 0;
        let mut steps = Vec::new();
        for &(slot, id) in &set.slots {
            if sheet.equipment[slot as usize] == id || (pages::gear_empty(slot, id) && pages::gear_empty(slot, sheet.equipment[slot as usize])) {
                continue;
            }
            if pages::gear_empty(slot, id) {
                steps.push((slot, None));
            } else if have.contains(&pages::gear_item(slot, id)) {
                steps.push((slot, Some(pages::gear_item(slot, id))));
            } else {
                missing += 1;
            }
        }
        tracing::info!("equipment set {}: {} changes, {missing} missing", set.name, steps.len());
        if steps.is_empty() {
            let text = if missing > 0 { format!("{}: {missing} items are no longer in your inventory", set.name) }
                       else { format!("You are already wearing {}", set.name) };
            self.state.note = Some((text, Instant::now()));
            return;
        }
        self.gear_queue = Some((set.name, steps, missing));
    }

    /// One equip of the set being put on (see gear_equip).
    fn gear_step(&mut self, cx: &Context) {
        if cx.game.is_some() {
            self.gear_load(cx);
        }
        let Some((name, steps, missing)) = self.gear_queue.as_mut() else { return };
        let Some(game) = cx.game else {
            self.gear_queue = None;
            return;
        };
        if let Some((slot, item)) = (!steps.is_empty()).then(|| steps.remove(0)) {
            // An empty hand: Elden Ring's unarmed "weapon", else the slot emptied.
            let done = if slot <= 5 && item.is_none() {
                game.equip(slot, Some(110000)).or_else(|_| game.equip(slot, None))
            } else {
                game.equip(slot, item)
            };
            if let Err(why) = done {
                tracing::info!("equipment set {name}: slot {slot} item {item:?}: {why}");
                *missing += 1;
            }
            self.sheet_time = None;
        }
        if steps.is_empty() {
            let text = match *missing {
                0 => format!("Equipped {name}"),
                n => format!("Equipped {name} ({n} items could not be put on)"),
            };
            self.state.note = Some((text, Instant::now()));
            self.gear_queue = None;
        }
    }

    /// Equips the item, or takes it off when it is equipped (the slot is left empty): a double click in the
    /// Backpack, or the equip button there and on the Pets page.
    fn equip(&mut self, id: u32, sheet: &Sheet, cx: &Context) {
        let name = self.names.as_ref().map(|n| n.item(id)).unwrap_or_default();
        let text = match (pages::equip_slot(sheet, &self.items, &name, id), cx.game) {
            (None, _) => format!("{name} can't be equipped"),
            (Some(_), None) => "Equipping needs the game".into(),
            (Some(how), Some(game)) => {
                let (done, on) = match how {
                    pages::Equip::Gear(slot, item) => (game.equip(slot, item), item.is_some()),
                    pages::Equip::Quick(on) => (game.set_quick_item(id, on), on),
                };
                match done {
                    Ok(()) => {
                        self.sheet_time = None;
                        match (on, how) {
                            (true, pages::Equip::Quick(_)) => format!("{name} is in your quick items"),
                            (true, _) => format!("Equipped {name}"),
                            (false, _) => format!("Took off {name}"),
                        }
                    }
                    Err(why) => format!("Couldn't equip {name}: {why}"),
                }
            }
        };
        tracing::info!("equip {id:#x}: {text}");
        self.state.note = Some((text, Instant::now()));
    }

    /// The mouse: the system cursor when it moves, else DirectInput's movement (Elden Ring can hold the system
    /// cursor still); clicks from either, counted once.
    fn pointer(&mut self, ui: &Ui, dt: f32) -> Pointer {
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
        if self.open && (self.tab != Tab::Map || self.map == MapState::Idle) {
            // The map page: either stick moves the cursor over the map (the map itself stays still). Round 19 A5:
            // every other page: the right stick moves the wand (the camera is locked while the book is open; the
            // left stick still walks).
            let [lx, ly, rx, ry] = input::sticks();
            let (lx, ly) = if self.tab == Tab::Map { (lx, ly) } else { (0.0, 0.0) };
            let live = |v: f32| if v.abs() < MAP_STICK_DEAD { 0.0 } else { (v.abs() - MAP_STICK_DEAD) / (1.0 - MAP_STICK_DEAD) * v.signum() };
            let (x, y) = ((live(lx) + live(rx)).clamp(-1.0, 1.0), (live(ly) + live(ry)).clamp(-1.0, 1.0));
            let step = height * MAP_STICK_SPEED * dt;
            self.cursor = [self.cursor[0] + x * x.abs() * step, self.cursor[1] - y * y.abs() * step];
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
            // Round 19 A5: the controller's A held drags like the mouse button (turning the picture with the wand).
            down: io.mouse_down[0] || di_down || (self.open && input::held().0 & input::A != 0),
            clicked,
            wheel: if io.mouse_wheel != 0.0 { io.mouse_wheel } else { wheel as f32 / 120.0 },
        }
    }

    /// Fills and draws the current page; returns the book tab and page button under the mouse, and whether the
    /// mouse is in the page's hole. `map_view`: Elden Ring's map is open behind the book.
    fn draw(&mut self, ui: &Ui, cx: &Context, pointer: &Pointer, dt: f32, map_view: bool)
            -> (Option<usize>, Option<usize>, bool, [f32; 2]) {
        let tab = if map_view { Tab::Map } else { self.tab };
        let page_name = page_of(tab, &self.state);
        let mut view = View::default();
        let settings = if tab == Tab::Settings { settings_rows(self.state.page, cx, &self.staged) } else { Vec::new() };
        let none = Names::default();
        let gear_sets = self.gear_sets_of_character();
        let data = Data {
            sheet: self.sheet.as_ref(),
            stats: cx.stats,
            names: self.names.as_ref().unwrap_or(&none),
            items: &self.items,
            quests: &self.active,
            settings,
            map_opening: matches!(self.map, MapState::Opening(_)) || self.map_on_open,
            map_open: map_view,
            map_picture: (map_view && self.world_ready && self.map_shot).then(|| {
                let [w, h] = ui.io().display_size;
                w / h.max(1.0)
            }),
            position: cx.position,
            achieved: &self.achieved,
            gear_sets: &gear_sets,
        };
        if let Some(page) = self.layouts.get(page_name) {
            pages::bind(tab, page, &data, &mut self.state, &mut view);
        }
        // The book, centred.
        let [width, height] = ui.io().display_size;
        let s = height / SCREEN;
        let origin = [(width - BOOK[0] * s) / 2.0, (height - BOOK[1] * s) / 2.0];
        let mouse = [(pointer.pos[0] - origin[0]) / s, (pointer.pos[1] - origin[1]) / s];

        // The character pictures this page shows; dragging in the Backpack's turns it.
        let model_node = view.node_images.iter().find(|(_, n)| n.as_str() == pages::MODEL_FULL).map(|(i, _)| *i);
        let page_layout = self.layouts.get(page_name);
        let in_model = model_node.zip(page_layout).is_some_and(|(i, l)| layout::inside(mouse, l.nodes[i].rect));
        let drag = if self.dragging { -pointer.delta[0] * TURN_MOUSE / s } else { 0.0 };
        if !map_view {
            let layouts = std::mem::take(&mut self.layouts);
            self.request_models(&view, layouts.get(page_name), s, dt, drag);
            self.layouts = layouts;
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
            // Round 20: the art the cards set themselves (goal pictures, reward, tracker, glow; draw_quest_cards).
            wanted.extend(self.active.iter().map(|a| a.picture.clone()).filter(|p| !p.is_empty()));
            wanted.extend([pages::REWARD_RUNES, pages::REWARD_SCROLL, pages::QUEST_TRACKER, pages::QUEST_PAPER_LIT].map(String::from));
        }
        wanted.extend(view.images.iter().map(|i| i.1.clone()));
        wanted.extend(view.tinted.iter().map(|i| i.1.clone()));
        wanted.extend(view.node_art.values().cloned());
        // Art drawn in a node's place (the map page's Wizard101 map); "@..." pictures are skipped by want().
        wanted.extend(view.node_images.values().cloned());
        wanted.extend(view.backs.iter().map(|i| i.1.clone()));
        wanted.extend(view.frames.iter().flat_map(|f| f.1.iter().flatten().cloned()));
        for (_, hv) in &view.hovers {
            wanted.extend(hv.images.iter().map(|i| i.1.clone()));
            wanted.extend(hv.backs.iter().map(|i| i.1.clone()));
            wanted.extend(hv.extra.iter().flat_map(|e| layout::text_pictures(&e.1)));
        }
        // Pictures inside texts (the spell cards' school and type symbols).
        wanted.extend(view.extra.iter().flat_map(|e| layout::text_pictures(&e.1)));
        wanted.extend(BUBBLE.iter().map(|p| p.to_string()));
        wanted.push(CURSOR.into());
        wanted.extend(tip_frame(false).into_iter().chain(tip_frame(true)).flatten());
        wanted.push(EQUIP_FIST.into());
        wanted.extend(crate::itemdata::TIP_ICONS.iter().map(|i| i.to_string()));
        self.textures.want(wanted);

        let draw = ui.get_foreground_draw_list();
        let leaving_map = !map_view && self.map == MapState::Open;
        if (map_view && view.picture.is_some()) || leaving_map {
            // The map is on the page (or being left for another page): the world as it was when the map opened,
            // behind the book.
            if let Some((world, _)) = self.textures.get(WORLD_PICTURE) {
                draw.add_image(world, [0.0, 0.0], [width, height]).build();
            }
        }
        if let Some(h) = view.hole.filter(|_| map_view) {
            // Only the map shows through the book: the rest of the screen around the book is covered.
            let dark = ImColor32::from_rgba(12, 9, 6, 255);
            let (p0, p1) = ([origin[0] + h[0] * s, origin[1] + h[1] * s], [origin[0] + h[2] * s, origin[1] + h[3] * s]);
            for (a, b) in [([0.0, 0.0], [width, p0[1]]), ([0.0, p1[1]], [width, height]), ([0.0, p0[1]], [p0[0], p1[1]]),
                           ([p1[0], p0[1]], [width, p1[1]])] {
                if b[0] > a[0] && b[1] > a[1] {
                    draw.add_rect(a, b, dark).filled(true).build();
                }
            }
        }
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
        if let Some((note, _)) = self.state.note.as_ref().filter(|n| n.1.elapsed() < NOTE_TIME && !map_view) {
            // What the last click did, in a strip along the bottom of the pages.
            let r = [150.0, 548.0, 650.0, 574.0];
            let (p0, p1) = (canvas.at(r[0], r[1]), canvas.at(r[2], r[3]));
            draw.add_rect(p0, p1, PARCHMENT_DARK).filled(true).rounding(6.0 * s).build();
            draw.add_rect(p0, p1, GOLD).rounding(6.0 * s).thickness(1.5 * s).build();
            canvas.text(&format!("<center>{note}</center>"), [r[0] + 6.0, r[1], r[2] - 6.0, r[3]],
                        &TextStyle::new("WizardNormal", 0.42, WHITE), false);
        }
        let tab_hit = layout::draw(prefs, &prefs_view, &canvas, |i| i > bkg).button;
        canvas.hole.set(None);

        if !map_view {
            // No button hints on the book (Ryan, round 19 B1): Wizard101 shows none.
            // What the mouse is over: an enlarged card (the deck), Wizard101's plain tooltip (badges), or the item
            // tooltip.
            if let Some((_, hv)) = view.hovers.iter().rev().find(|(r, _)| layout::inside(mouse, *r)) {
                layout::draw_extra(hv, &canvas);
            }
            // The book's bookmarks have Wizard101's own hover texts (SpellBookPrefsWindow's m_sTip: GUI.lang
            // 00001702-00001710, PetsTabTooltip), in the same plain tooltip.
            let bookmark = TABS.iter().zip(BOOKMARK_TIPS).find_map(|((_, node, _), text)| {
                prefs.find(node).map(|i| prefs.nodes[i].rect).filter(|r| layout::inside(mouse, *r)).map(|r| (r, text.to_string()))
            });
            match view.bubbles.iter().rev().find(|(r, _)| layout::inside(mouse, *r)).cloned().or(bookmark) {
                Some((_, text)) => {
                    let text = &text;
                    let step = ui.io().delta_time.clamp(0.0, 0.25);
                    match self.bubble.as_mut() {
                        Some(b) if b.0 == *text => b.1 += step,
                        _ => self.bubble = Some((text.clone(), step)),
                    }
                    let since = self.bubble.as_ref().map_or(0.0, |b| b.1);
                    draw_bubble(&canvas, text, mouse, [width, height], (since / BUBBLE_FADE).min(1.0));
                }
                None => self.bubble = None,
            }
            if let Some((_, lines)) = view.tips.iter().rev().find(|(r, _)| layout::inside(mouse, *r)) {
                self.draw_tip(&canvas, lines, mouse, [width, height]);
            }
            draw_cursor(&draw, &self.textures, pointer.pos, height);
        } else if !view.hole.or(view.picture).is_some_and(|h| layout::inside(mouse, h))
            && layout::inside(mouse, [0.0, 0.0, BOOK[0], BOOK[1]])
        {
            // Over the book's frame around the map: the book's cursor, for its tabs, and the page buttons' plain
            // tooltips (the magnifier's "back" among them).
            if let Some((_, text)) = view.bubbles.iter().rev().find(|(r, _)| layout::inside(mouse, *r)) {
                draw_bubble(&canvas, text, mouse, [width, height], 1.0);
            }
            draw_cursor(&draw, &self.textures, pointer.pos, height);
        }
        (tab_hit, page_hit, in_model, mouse)
    }

    /// A hover box beside the mouse in Wizard101's item tooltip style (see tip_frame): the name first, then one
    /// stat per line, every line centred (the stat layouts are H_CENTER); a long last line (the game's description) wraps in a quieter colour. "Equipped" in the lines
    /// makes it the equipped variant (blue frame, fist badge) instead of a line of its own.
    fn draw_tip(&self, canvas: &Canvas, lines: &[String], mouse: [f32; 2], screen: [f32; 2]) {
        let (Some(fancy), Some(normal)) = (self.fonts.get("WizardFancy"), self.fonts.get("WizardNormal")) else { return };
        let Some((title, rest)) = lines.split_first() else { return };
        let equipped = rest.iter().any(|l| l == "Equipped");
        let rest: Vec<&String> = rest.iter().filter(|l| *l != "Equipped").collect();
        let (title_scale, scale, note_scale, margin) = (0.55, TIP_STAT_SCALE, 0.42, 15.0);
        let line_h = fancy.line_height * scale;
        let note_h = normal.line_height * note_scale;
        let title_h = fancy.line_height * title_scale;
        // A stat row: "@value|school icon|stat icon|name" (itemdata::stat_row).
        let row_of = |l: &str| -> Option<(String, Vec<String>)> {
            let mut parts = l.strip_prefix('@')?.split('|');
            let value = parts.next()?.to_string();
            let icons = parts.take(2).filter(|p| !p.is_empty()).map(str::to_string).collect();
            Some((value, icons))
        };
        let icon = TIP_ICON.min(line_h);
        let row_h = line_h.max(icon + 2.0);
        let row_width = |value: &str, icons: usize| fancy.width(value) * scale + 4.0 + icons as f32 * (icon + 1.0);
        let is_note = |i: usize, l: &str| i + 1 == rest.len() && !l.starts_with('@')
            && fancy.width(l) * scale > MAX_TIP - 2.0 * margin;
        // As wide as the widest stat line needs (the description wraps), between Wizard101's 200 and MAX_TIP.
        let widest = rest.iter().enumerate().filter(|(i, l)| !is_note(*i, l))
            .map(|(_, l)| match row_of(l) {
                Some((v, icons)) => row_width(&v, icons.len()),
                None => fancy.width(l) * scale,
            })
            .chain([fancy.width(title) * title_scale])
            .fold(0.0, f32::max);
        let width = (widest + 2.0 * margin + 6.0).clamp(TIP_WIDTH, MAX_TIP);
        let inner = width - 2.0 * margin;
        let rows = |w: f32| (w / inner * 1.08).ceil().max(1.0);
        // One row when the name fits the window's width (the 1.08 slack of `rows` made a name that just fits
        // two rows tall, a gap under it: Ryan round 19 C2).
        let title_w = fancy.width(title) * title_scale;
        let title_rows = if title_w <= inner { 1.0 } else { rows(title_w) };
        let heights: Vec<f32> = rest.iter().enumerate().map(|(i, l)| {
            if l.starts_with('@') {
                row_h
            } else if is_note(i, l) {
                rows(normal.width(l) * note_scale) * note_h
            } else {
                rows(fancy.width(l) * scale) * line_h
            }
        }).collect();
        let title_bottom = margin + (title_rows * title_h).max(23.0);
        let height = title_bottom + heights.iter().sum::<f32>() + 4.0 * (rest.len() > 1) as u8 as f32 + margin;
        // Page pixels of the screen's edges.
        let right = (screen[0] - canvas.origin[0]) / canvas.s;
        let bottom = (screen[1] - canvas.origin[1]) / canvas.s;
        let top = -canvas.origin[1] / canvas.s + if equipped { 14.0 } else { 0.0 };
        // Left of the cursor (Ryan round 19 C3); to its right only where the screen's left edge is in the way.
        let left = -canvas.origin[0] / canvas.s;
        let mut x = mouse[0] - 12.0 - width;
        if x < left + 4.0 + if equipped { 14.0 } else { 0.0 } {
            x = (mouse[0] + 22.0).min(right - width - 4.0);
        }
        let y = (mouse[1] + 8.0).min(bottom - height).max(top);
        let (p0, p1) = (canvas.at(x + 4.0, y + 4.0), canvas.at(x + width - 4.0, y + height - 4.0));
        let back = ImColor32::from_rgba(TIP_BACK.r, TIP_BACK.g, TIP_BACK.b, (TIP_BACK.a as f32 * canvas.alpha) as u8);
        canvas.draw.add_rect(p0, p1, back).filled(true).rounding(6.0 * canvas.s).build();
        canvas.border(&tip_frame(equipped), [x, y, x + width, y + height], ImColor32::WHITE);
        canvas.text(title, [x + margin, y + margin, x + width - margin, y + title_bottom],
                    &TextStyle::new("WizardFancy", title_scale, TIP_TITLE).centred().wrapped(), false);
        // The stats start right under the name (Ryan round 19 C2).
        let mut ly = y + title_bottom;
        for (i, (line, h)) in rest.iter().zip(heights).enumerate() {
            if let Some((value, icons)) = row_of(line) {
                // "+45" then the icons, on one line, centred as a whole.
                let vw = fancy.width(&value) * scale;
                let x0 = x + (width - row_width(&value, icons.len())) / 2.0;
                canvas.text(&value, [x0, ly, x0 + vw + 2.0, ly + h],
                            &TextStyle::new("WizardFancy", scale, TIP_TEXT), false);
                let mut ix = x0 + vw + 4.0;
                let iy = ly + (h - icon) / 2.0;
                for img in &icons {
                    canvas.image(img, [ix, iy, ix + icon, iy + icon], [0.0, 0.0, 1.0, 1.0], ImColor32::WHITE);
                    ix += icon + 1.0;
                }
                ly += h;
                continue;
            }
            let note = is_note(i, line);
            if note && i > 0 {
                ly += 4.0;
            }
            let style = if note {
                TextStyle::new("WizardNormal", note_scale, TIP_NOTE)
            } else {
                TextStyle::new("WizardFancy", scale, TIP_TEXT)
            };
            canvas.text(line, [x + margin, ly, x + width - margin, ly + h], &style.centred().wrapped(), false);
            ly += h;
        }
        if equipped {
            // At the window's top left (Ryan round 19 C4).
            canvas.image(EQUIP_FIST, [x - 14.0, y - 14.0, x + 25.0, y + 22.0],
                         [0.0, 0.0, 0.5, 0.5], ImColor32::WHITE);
        }
    }

    /// QuestListWindow's four card windows, each with QuestInfoWindow.gui drawn into it for a quest in progress.
    fn draw_quest_cards(&self, page: &Layout, canvas: &Canvas) {
        let Some(card) = self.layouts.get(pages::QUEST_CARD) else { return };
        let tracked = self.tracked_index();
        for i in 0..pages::QUESTS_PER_PAGE {
            let k = self.state.page * pages::QUESTS_PER_PAGE + i;
            let Some(quest) = self.active.get(k) else { break };
            let Some(window) = page.find(&format!("wndQuestInfo{i}")) else { continue };
            let mut view = View::default();
            pages::quest_card(card, quest, &mut view);
            let r = page.nodes[window].rect;
            if tracked == Some(k) {
                pages::quest_tracked(card, &mut view);
            }
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

    /// Reads the quest flags. A conversation request (talk.rs) that is on opens Wizard101's dialogue box; one that
    /// went off while its box is up (the player walked away, the event withdrew it) closes the box.
    fn poll_quests(&mut self, cx: &Context) {
        if self.quests.quests.is_empty() || self.flags_time.is_some_and(|t| t.elapsed() < QUEST_POLL) {
            return;
        }
        self.flags_time = Some(Instant::now());
        let first = self.flags.is_empty();
        let mut now: HashMap<u32, bool> = HashMap::new();
        for f in &self.quest_flags {
            if let Some(on) = game::event_flag(*f) {
                now.insert(*f, on);
            }
        }
        // Round 19 (quests E1): every quest flag that is on, at load and whenever that set changes.
        let on_of = |m: &HashMap<u32, bool>| {
            let mut v: Vec<u32> = m.iter().filter(|(_, on)| **on).map(|(f, _)| *f).collect();
            v.sort_unstable();
            v
        };
        let on_now = on_of(&now);
        if first || on_now != on_of(&self.flags) {
            tracing::info!("quest flags{}: on {:?} ({} read; quest reset v3 done: {:?})", if first { " at load" } else { "" },
                           on_now, now.len(), now.get(&markers::QUEST_RESET));
        }
        // Round 23 (H7): what changed since the last poll, as Wizard101's popups (not at load, nor before the quest
        // reset has run on an old save).
        if !first && now.get(&markers::QUEST_RESET) == Some(&true) {
            self.popups.changes(&self.quests, &self.flags, &now);
        }
        self.flags = now;
        let flags = &self.flags;
        self.active = self.quests.active(&|f| flags.get(&f) == Some(&true));
        let on = |f: u32| flags.get(&f) == Some(&true);
        // Round 20 (Q3): a quest just completed gives its reward in runes, once (its reward flag; only after the
        // quest reset has run, so that the old flags of a save from before it give nothing).
        if on(markers::QUEST_RESET) {
            if let Some(game) = cx.game {
                for (title, runes, flag) in self.quests.rewards_due(&on) {
                    match game.add_runes(runes) {
                        Some(held) => {
                            game::set_event_flag(flag, true);
                            tracing::info!("quest reward: {title} complete, {runes} runes given ({held} held now)");
                            crate::music::sfx("ui_reward_gold");
                        }
                        None => tracing::warn!("quest reward: {title}: the rune count can't be reached"),
                    }
                }
            }
        }
        if let Some(d) = &self.dialogue {
            if !on(self.requests[d.request].flag) {
                tracing::info!("dialogue: {} withdrawn by its event", self.requests[d.request].key);
                self.end_dialogue();
            }
            return;
        }
        // Round 25 (H2): rows withdrawn by their events leave the picker; with fewer than two left it closes (one
        // left opens as a plain conversation on the next poll).
        if self.picker.is_some() && self.picker_pinned {
            return;
        }
        if let Some(p) = &mut self.picker {
            let requests = &self.requests;
            p.rows.retain(|r| on(requests[r.request].flag));
            if p.rows.len() < 2 {
                tracing::info!("quest picker: closed, {} conversation(s) still asked for", p.rows.len());
                self.picker = None;
                self.end_dialogue();
            }
            return;
        }
        let Some(k) = self.requests.iter().position(|r| on(r.flag)) else { return };
        if first {
            // Left on in the save from a conversation cut short by quitting: its event no longer waits for it.
            for r in self.requests.iter().filter(|r| on(r.flag)) {
                game::set_event_flag(r.flag, false);
            }
            return;
        }
        if !cx.gameplay || self.open {
            return; // the box comes once the book or the game's menu is closed
        }
        // Round 25 (H2): the same NPC's events asking for more than one quest conversation at once: Wizard101's
        // quest picker first.
        let speaker = self.requests[k].speaker.clone();
        let rows: Vec<crate::picker::Row> = self.requests.iter().enumerate()
            .filter(|(_, r)| on(r.flag) && r.speaker == speaker && r.quest != "-")
            .map(|(i, r)| crate::picker::Row {
                request: i,
                icon: crate::picker::icon(&r.kind),
                label: self.quests.quests.iter().find(|q| q.slug == r.quest).map_or(r.quest.clone(), |q| q.title.clone()),
            })
            .collect();
        if rows.len() >= 2 {
            tracing::info!("quest picker: {} offers {:?}", self.requests[k].name,
                           rows.iter().map(|r| self.requests[r.request].key.as_str()).collect::<Vec<_>>());
            crate::music::sfx("ui_chatbubble_popup");
            self.picker = Some(crate::picker::Picker { speaker, name: self.requests[k].name.clone(),
                portrait: self.requests[k].portrait.clone(), rows, focus: 0, opened: Instant::now() });
            input::set_book_open(true);
            return;
        }
        self.open_conversation(k);
    }

    /// Opens conversation `k` (its request flag is on), or answers it OK when it has no lines.
    fn open_conversation(&mut self, k: usize) {
        match self.conversation(k) {
            Some(c) => {
                tracing::info!("dialogue: {} ({} pages{})", self.requests[k].key, c.pages.len(),
                               if c.offer { ", offer" } else { "" });
                crate::music::sfx("ui_chatbubble_popup");
                crate::music::voice(Some((self.requests[k].key.as_str(), 0)));
                self.dialogue = Some(c);
                input::set_book_open(true);
            }
            None => {
                tracing::info!("dialogue: {} has no lines; answered OK", self.requests[k].key);
                Self::answer(&self.requests[k], talk::Answer::Ok);
            }
        }
    }

    /// The conversation for request `k`: its lines, or for a quest offer without them the quest's own text.
    fn conversation(&self, k: usize) -> Option<Conversation> {
        let r = &self.requests[k];
        let mut pages = r.pages.clone();
        if pages.is_empty() && r.offer() {
            pages = self.quests.offer_lines(&r.card);
        }
        (!pages.is_empty()).then(|| Conversation { request: k, pages, page: 0, offer: r.offer(), opened: Instant::now() })
    }

    /// Answers request `r` for its talk event: the answer flag on, the request off.
    fn answer(r: &talk::Request, answer: talk::Answer) {
        tracing::info!("dialogue: {} answered {answer:?}", r.key);
        game::set_event_flag(if answer == talk::Answer::Ok { talk::DIALOG_OK } else { talk::DIALOG_CANCEL }, true);
        game::set_event_flag(r.flag, false);
    }

    fn end_dialogue(&mut self) {
        // Round 23 S1: every way a conversation ends (answer, close, withdrawn by its event) stops its voice line.
        crate::music::voice(None);
        self.dialogue = None;
        self.holding = true;
        self.closed_at = Some(Instant::now());
        input::set_book_open(self.blocking());
    }

    /// Wizard101's dialogue box (talk.rs): its buttons by mouse; A / Enter / Space / E for NEXT (DONE) or ACCEPT,
    /// LB / Left / Backspace for BACK, B / Esc to close (on an offer's last page: DECLINE).
    fn dialogue_frame(&mut self, ui: &Ui, pressed: u16, pointer: &Pointer) {
        input::set_book_open(true);
        let (Some(window), Some(d)) = (self.layouts.get(talk::WINDOW), self.dialogue.as_ref()) else {
            // No DialogWindow.lay: answer at once rather than leave the event waiting.
            if let Some(d) = self.dialogue.take() {
                Self::answer(&self.requests[d.request], talk::Answer::Ok);
            }
            self.end_dialogue();
            return;
        };
        let request = &self.requests[d.request];
        let screen = ui.io().display_size;
        let (s, origin) = talk::placement(window, screen, talk::xp_bar_top(screen, self.hud_scale));
        let view = d.view(window, request);
        // Ask for the art this frame needs (the box, its buttons, the portrait, the quest card); it loads before the
        // next frame.
        let mut wanted = window.images();
        wanted.extend(view.node_art.values().cloned());
        wanted.extend(view.text.values().flat_map(|t| layout::text_pictures(t)));
        if d.choosing() {
            if let Some(card) = self.layouts.get(pages::QUEST_CARD) {
                wanted.extend(card.images());
            }
            wanted.extend(self.quests.offer_card(&request.card).map(|a| a.picture).filter(|p| !p.is_empty()));
            wanted.extend([pages::REWARD_RUNES, pages::REWARD_SCROLL].map(String::from));
        }
        wanted.push(CURSOR.into());
        self.textures.want(wanted);
        let draw = ui.get_foreground_draw_list();
        let canvas = Canvas {
            draw: &draw,
            textures: &self.textures,
            fonts: &self.fonts,
            origin,
            s,
            mouse: [(pointer.pos[0] - origin[0]) / s, (pointer.pos[1] - origin[1]) / s],
            mouse_down: pointer.down,
            alpha: (d.opened.elapsed().as_secs_f32() / 0.15).min(1.0),
            hole: Cell::new(None),
        };
        // An offer's last page: the quest card (QuestInfoWindow, as on the Quests page) in DialogImage's square.
        if d.choosing() {
            if let (Some(card), Some(active)) = (self.layouts.get(pages::QUEST_CARD), self.quests.offer_card(&request.card)) {
                let square = talk::picture_rect(self.layouts.get(talk::IMAGE), screen);
                let c = card.nodes[0].rect;
                let (cw, ch) = (c[2] - c[0], c[3] - c[1]);
                let k = ((square[2] - square[0]) / cw).min((square[3] - square[1]) / ch);
                let at = [(square[0] + square[2] - cw * k) / 2.0, (square[1] + square[3] - ch * k) / 2.0];
                let mut view = View::default();
                pages::quest_card(card, &active, &mut view);
                let inner = Canvas {
                    draw: &draw,
                    textures: &self.textures,
                    fonts: &self.fonts,
                    origin: at,
                    s: k,
                    mouse: [-1.0e4; 2],
                    mouse_down: false,
                    alpha: canvas.alpha,
                    hole: Cell::new(None),
                };
                layout::draw(card, &view, &inner, |_| true);
                layout::draw_extra(&view, &inner);
            }
        }
        let hits = layout::draw(window, &view, &canvas, |_| true);
        draw_cursor(&draw, &self.textures, pointer.pos, screen[1]);
        if d.opened.elapsed() < Duration::from_millis(250) {
            return;
        }
        let key = |k: Key| ui.is_key_pressed_no_repeat(k);
        let click = hits.button.filter(|_| pointer.clicked).and_then(|b| talk::button(&window.nodes[b].name));
        let press = click.or_else(|| {
            if pressed & input::A != 0 || key(Key::Enter) || key(Key::Space) || key(Key::E) {
                Some(if d.choosing() { Press::Accept } else { Press::Next })
            } else if pressed & input::LB != 0 || key(Key::LeftArrow) || key(Key::Backspace) {
                Some(Press::Back)
            } else if pressed & (input::B | input::BACK) != 0 || key(Key::Escape) {
                Some(if d.choosing() { Press::Decline } else { Press::Close })
            } else {
                None
            }
        });
        let Some(press) = press else { return };
        // Round 20 (Q1): which input answered, so the log shows the controller's A at work.
        let source = if click.is_some() { "mouse" } else if pressed != 0 { "controller" } else { "keyboard" };
        tracing::info!("dialogue: {press:?} by {source} (page {}, choosing {})", d.page + 1, d.choosing());
        let d = self.dialogue.as_mut().expect("checked");
        // Round 22 S3/S4: Wizard101's button sound, and the new page's voice line.
        if let Some(sound) = d.press_sound(press) {
            crate::music::sfx(sound);
        }
        let before = d.page;
        if let Some(answer) = d.press(press) {
            Self::answer(&self.requests[d.request], answer);
            self.end_dialogue();
        } else if d.page != before {
            crate::music::voice(Some((self.requests[d.request].key.as_str(), d.page)));
        }
    }

    /// Round 25 (H2): Wizard101's quest picker (picker.rs): the parchment of quests over DialogWindow's box. A row
    /// (click, or Up / Down and A / Enter) opens its conversation; Cancel (btnRight, B / Esc) answers them all cancel.
    fn picker_frame(&mut self, ui: &Ui, pressed: u16, pointer: &Pointer) {
        input::set_book_open(true);
        let (Some(window), Some(p)) = (self.layouts.get(talk::WINDOW), self.picker.as_ref()) else {
            self.picker = None;
            self.end_dialogue();
            return;
        };
        let screen = ui.io().display_size;
        let (s, origin) = talk::placement(window, screen, talk::xp_bar_top(screen, self.hud_scale));
        let view = crate::picker::dialog_view(window, p);
        let mut wanted = window.images();
        wanted.extend(view.node_art.values().cloned());
        wanted.extend(crate::picker::art());
        wanted.push(CURSOR.into());
        self.textures.want(wanted);
        let draw = ui.get_foreground_draw_list();
        let alpha = (p.opened.elapsed().as_secs_f32() / 0.15).min(1.0);
        let moved = pointer.delta != [0.0, 0.0] || pointer.clicked;
        let mouse = if moved || self.picker_mouse { pointer.pos } else { [-1.0e4; 2] };
        let hover = crate::picker::draw(&draw, &self.textures, &self.fonts, screen, p, mouse, alpha);
        let canvas = Canvas {
            draw: &draw,
            textures: &self.textures,
            fonts: &self.fonts,
            origin,
            s,
            mouse: [(pointer.pos[0] - origin[0]) / s, (pointer.pos[1] - origin[1]) / s],
            mouse_down: pointer.down,
            alpha,
            hole: Cell::new(None),
        };
        let hits = layout::draw(window, &view, &canvas, |_| true);
        draw_cursor(&draw, &self.textures, pointer.pos, screen[1]);
        if moved {
            self.picker_mouse = true;
        }
        if p.opened.elapsed() < Duration::from_millis(250) {
            return;
        }
        let key = |k: Key| ui.is_key_pressed_no_repeat(k);
        let button = hits.button.filter(|_| pointer.clicked).map(|b| window.nodes[b].name.as_str());
        let rows = p.rows.len();
        let mut pick = hover.filter(|_| pointer.clicked);
        let mut cancel = matches!(button, Some("btnRight") | Some("btnExit"));
        if pressed & (input::DPAD_UP | input::DPAD_DOWN) != 0 || key(Key::UpArrow) || key(Key::DownArrow) {
            let down = pressed & input::DPAD_DOWN != 0 || key(Key::DownArrow);
            let p = self.picker.as_mut().expect("checked");
            p.focus = (p.focus + if down { 1 } else { rows - 1 }) % rows;
            self.picker_mouse = false;
            crate::music::sfx("HudSelect");
        } else if pressed & input::A != 0 || key(Key::Enter) || key(Key::Space) || key(Key::E) {
            pick = Some(hover.unwrap_or(p.focus));
        } else if pressed & (input::B | input::BACK) != 0 || key(Key::Escape) {
            cancel = true;
        }
        if let Some(i) = pick {
            let Some(row) = self.picker.as_ref().and_then(|p| p.rows.get(i)).cloned() else { return };
            tracing::info!("quest picker: picked {} ({})", row.label, self.requests[row.request].key);
            crate::music::sfx("HudSelect");
            self.picker = None;
            self.picker_mouse = false;
            self.open_conversation(row.request);
            if self.dialogue.is_none() {
                self.end_dialogue();
            }
        } else if cancel {
            tracing::info!("quest picker: cancelled");
            crate::music::sfx("HudCancel");
            let p = self.picker.take().expect("checked");
            for row in &p.rows {
                Self::answer(&self.requests[row.request], talk::Answer::Cancel);
            }
            self.picker_mouse = false;
            self.end_dialogue();
        }
    }

    /// Round 25 (H3): Wizard101's interact prompt (prompt.rs) at the street NPC or Bone Cage the player stands at,
    /// in normal play; remembers its top for the quest helper.
    fn draw_prompt(&mut self, ui: &Ui, cx: &Context) {
        self.prompt_top = None;
        let Some(game) = cx.game else { return };
        self.prompt.setup(game);
        // The input used last decides the key's picture: a controller button or stick, else a key or the mouse.
        let (held, _, _) = input::held();
        if self.prompt.preview {
        } else if held != 0 || input::sticks().iter().any(|v| v.abs() > 0.3) {
            self.prompt.pad = true;
        } else if ui.io().keys_down.iter().any(|k| *k) || ui.io().mouse_delta != [0.0, 0.0] {
            self.prompt.pad = false;
        }
        if !cx.gameplay || self.blocking() || self.map != MapState::Idle || self.flags.is_empty() {
            return;
        }
        let Some(camera) = game.camera() else { return };
        let entities = markers::steady(&camera, &game.entities(&markers::MODELS, &markers::entity_ids()));
        let spots = markers::locate(&camera, game.position(), &entities);
        let player_msb = crate::questhelper::player_msb_from_npcs(&camera, &entities)
            .or_else(|| game.position().and_then(crate::questhelper::player_msb));
        let flags = &self.flags;
        let target = match self.preview_prompt.clone() {
            Some(t) => Some(t),
            None => self.prompt.update(camera.player, game.player_yaw(), &spots, player_msb,
                                       &|f| flags.get(&f) == Some(&true)),
        };
        let Some(target) = target else { return };
        let mut art = self.prompt.art();
        art.push(target.portrait.clone());
        self.textures.want(art);
        let screen = ui.io().display_size;
        let draw = ui.get_background_draw_list();
        self.prompt_top = Some(self.prompt.draw(&draw, &self.textures, &self.fonts, screen,
                                                talk::xp_bar_top(screen, self.hud_scale), &target));
    }

    /// Changes row `row` of the Settings page's current tab by `step` (options.rs). The row's value shows the change;
    /// there is no pending-changes strip (Ryan, round 16 L3), only a note when the change couldn't be made.
    fn change_setting(&mut self, row: usize, step: i32, cx: &mut Context) {
        let tab = self.state.page % crate::options::TABS.len();
        let Some(r) = crate::options::TABS[tab].2.get(row) else { return };
        self.state.sub = row;
        match crate::options::change(r, step, cx.game, cx.settings, &mut self.staged) {
            Ok(done) => tracing::info!("settings: staged {done}"),
            Err(why) => {
                tracing::warn!("settings: {} not changed: {why}", r.label);
                self.state.note = Some((why, Instant::now()));
            }
        }
    }

    /// The Settings window's OK (button, A or Enter): applies the staged changes and closes the book. If something
    /// couldn't be applied, the book stays open with the reason.
    fn settings_ok(&mut self, cx: &mut Context) {
        match crate::options::apply(&mut self.staged, cx.game, cx.settings, cx.settings_path) {
            Ok(n) => {
                tracing::info!("settings: OK, {n} setting(s) applied");
                self.set_open(false);
            }
            Err(why) => {
                tracing::warn!("settings: OK, not all applied: {why}");
                self.state.note = Some((why, Instant::now()));
            }
        }
    }

    /// Settings' Quit (to Elden Ring's title screen: a second click within QUIT_CONFIRM seconds confirms) and Exit
    /// Game (then to the desktop, on the first click; Ryan, round 15). Elden Ring's own quit-out saves first; the
    /// window is closed only once the title screen is up (quit_follow).
    fn quit(&mut self, desktop: bool) {
        let what = if desktop { "Exit Game" } else { "Quit" };
        let confirmed = self.quit_asked.is_some_and(|(d, t)| d == desktop && t.elapsed().as_secs_f32() <= QUIT_CONFIRM);
        if !desktop && !confirmed {
            self.quit_asked = Some((desktop, Instant::now()));
            self.state.note = Some((format!("Click {what} again to save and go back to the title screen"), Instant::now()));
            return;
        }
        self.quit_asked = None;
        if game::quit_to_title() {
            tracing::info!("settings: {what}: Elden Ring's quit-out requested (to the desktop: {desktop})");
            self.quitting = desktop.then(|| (Instant::now(), None));
            self.set_open(false);
        } else {
            tracing::warn!("settings: {what}: Elden Ring's quit-out couldn't be reached");
            self.state.note = Some(("Elden Ring's quit can't be reached; use its system menu".into(), Instant::now()));
        }
    }

    /// Exit Game: once Elden Ring has saved and gone back to its title screen (no player character for
    /// QUIT_SETTLE seconds), its window is asked to close (WM_CLOSE, Elden Ring's own exit).
    fn quit_follow(&mut self, cx: &Context) {
        let Some((since, gone)) = self.quitting else { return };
        if since.elapsed().as_secs_f32() > 90.0 {
            tracing::warn!("settings: Exit Game: the title screen didn't come; not closing");
            self.quitting = None;
            return;
        }
        let in_world = cx.game.is_some_and(|g| g.player_address().is_some());
        match (in_world, gone) {
            (true, _) => self.quitting = Some((since, None)),
            (false, None) => self.quitting = Some((since, Some(Instant::now()))),
            (false, Some(t)) if t.elapsed().as_secs_f32() > QUIT_SETTLE && since.elapsed().as_secs_f32() > 3.0 => {
                tracing::info!("settings: Exit Game: at the title screen; closing Elden Ring's window");
                self.quitting = None;
                if !game::close_window() {
                    tracing::warn!("settings: Exit Game: Elden Ring's window couldn't be closed");
                }
            }
            _ => {}
        }
    }

    fn set_open(&mut self, open: bool) {
        if open == self.open {
            return;
        }
        self.open = open;
        input::take_pressed();
        // Round 22 S3: Wizard101's spellbook sounds (Root.wad / _Shared-WorldData Sound/GUI).
        crate::music::sfx(if open { "ui_hudspellbook_open" } else { "ui_hudspellbook_close" });
        if open {
            self.opened_at = Instant::now();
            self.state.page = 0;
            self.state.pet = 0; // the Pets page opens on the equipped spirit ash
            self.sheet_time = None;
        } else {
            if !self.staged.is_empty() {
                tracing::info!("settings: book closed without OK; changes dropped");
            }
            self.staged.clear();
            self.map_on_open = false;
            self.holding = true;
            self.closed_at = Some(Instant::now());
            self.dragging = false;
        }
        input::set_book_open(self.blocking());
    }
    /// Shows `tab` (Close closes the book). `_activate`: a click or A on the tab itself (the World Map tab used to
    /// open Elden Ring's map; now its page is Wizard101's map).
    fn select(&mut self, tab: Tab, _activate: bool) {
        if tab != self.tab {
            self.state.reset();
            if tab != Tab::Close {
                crate::music::sfx("ui_book_pageturn"); // round 22 S3: W101's book page turn
            }
        }
        if tab != Tab::Close {
            self.tab = tab;
        }
        match tab {
            Tab::Close => self.set_open(false),
            // The World Map page is Wizard101's map; Elden Ring's opens from the page's magnifier (Action::OpenMap).
            _ => {}
        }
    }

    fn open_map(&mut self, cx: &Context) {
        self.open_map_now(cx.position);
    }

    fn open_map_now(&mut self, position: Option<([f32; 3], i32, i32)>) {
        self.set_open(false);
        self.map = MapState::Opening(Instant::now());
        self.map_leaving = None;
        self.map_back = None;
        self.warp_after_map = None;
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
            MapState::Opening(_) if !cx.hud => {
                input::end_map_press();
                self.map = MapState::Open;
            }
            MapState::Opening(since) if since.elapsed().as_secs_f32() > 4.0 => {
                self.after_map = None;
                self.warp_after_map = None;
                tracing::warn!("spellbook: Elden Ring's map didn't open");
                self.map = MapState::Idle;
            }
            MapState::Open if cx.gameplay => {
                input::end_map_press();
                self.map = MapState::Idle;
                let closing = self.map_leaving.take().map_or(0.0, |t| t.elapsed().as_secs_f32() * 1000.0);
                if let Some(flag) = self.warp_after_map.take() {
                    // The map's warp button: now that the map is gone, the warp event takes over (book stays shut).
                    self.after_map = None;
                    if self.travelled {
                        tracing::info!("spellbook: travelled from the map; warp flag {flag} dropped");
                    } else {
                        tracing::info!("spellbook: Elden Ring's map closed ({closing:.0} ms); warp flag {flag} on");
                        game::set_event_flag(flag, true);
                    }
                } else if self.travelled {
                    tracing::info!("spellbook: travelled from the map; the book stays closed");
                    self.after_map = None;
                } else {
                    tracing::info!("spellbook: Elden Ring's map closed ({closing:.0} ms after it began closing); \
                                    book back on {:?}", self.after_map.unwrap_or(Tab::Map));
                    self.set_open(true);
                    // Straight back at full strength: it was on the screen all along (no fade-in from nothing).
                    self.opened_at = Instant::now() - Duration::from_secs(1);
                    self.map_back = Some(Instant::now());
                    self.tab = self.after_map.take().unwrap_or(Tab::Map);
                }
            }
            MapState::Open => {
                if self.map_leaving.is_none()
                    && (self.after_map.is_some() || self.warp_after_map.is_some() || cx.hud || cx.faded >= 0.5) {
                    tracing::info!("spellbook: Elden Ring's map closing (hud {}, faded {:.2}, tab clicked {}, warp {:?})",
                                   cx.hud, cx.faded, self.after_map.is_some(), self.warp_after_map);
                    self.map_leaving = Some(Instant::now());
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
            if let (false, Some(sheet)) = (self.flasks_logged, self.sheet.as_ref()) {
                self.flasks_logged = true;
                let flasks: Vec<String> = sheet.items.iter().chain(&sheet.key_items)
                    .filter(|(id, _)| id >> 28 == 4 && (1000..1076).contains(&(id & 0x0FFF_FFFF)))
                    .map(|(id, n)| format!("{}x{n}", id & 0x0FFF_FFFF))
                    .collect();
                tracing::info!("flask items (even = empty, odd = filled): {} (max {:?})", flasks.join(" "), sheet.flasks);
            }
            // The player's Steam achievements (what Steam shows); the playthrough's boss flags only without Steam.
            let names: Vec<&str> = pages::ACHIEVEMENTS.iter().map(|a| a.0).collect();
            self.achieved = match crate::steam::achievements(&names) {
                Some(steam) => steam,
                None => pages::ACHIEVEMENTS
                    .iter()
                    .map(|(_, _, flag)| if *flag == 0 || cx.game.is_none() { None } else { game::event_flag(*flag) })
                    .collect(),
            };
        }
    }
}

/// The hair model (hr_a_<n>) chosen in the character creator: FaceDataBuffer's model IDs come first
/// (face, hair, eyes, ...), read here as the second little-endian u32. Unverified against a real save: the log
/// line "character: ... face ..." shows the bytes; a value with no hair file just leaves the hair out.
fn hair_model(face: &[u8]) -> Option<u32> {
    let v = u32::from_le_bytes(face.get(4..8)?.try_into().ok()?);
    (v < 1000).then_some(v)
}

/// The Settings page's rows on tab `tab` (options::TABS): label and value.
pub fn settings_rows(tab: usize, cx: &Context, staged: &crate::options::Staged) -> Vec<pages::SettingRow> {
    let graphics = staged.graphics();
    crate::options::TABS[tab % crate::options::TABS.len()].2.iter().map(|row| {
        let value = crate::options::value(row, cx.game, cx.settings, graphics.as_ref(), staged)
            .unwrap_or_else(|| "-".into());
        (row.label.to_string(), value)
    }).collect()
}
