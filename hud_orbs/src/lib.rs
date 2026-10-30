//! Wizard101 health, mana and energy (stamina) globes for Elden Ring, drawn over the game with hudhook (Dear ImGui on DirectX 12).
//!
//! Loaded by Mod Engine 2 (`external_dlls`). Next to the DLL it expects a `w101_orbs` folder with the globe art
//! (made by tools/w101_hud_art.py) and optionally `w101_orbs.ini` to move or resize the globes.
//! The layout is Wizard101's own HUDWindow.gui, in its pixels on a 768-pixel-tall screen: each globe's glass is
//! Health_Back (Energy_Back for energy) stretched over the globe's window, with the liquid, its surface and the number at fixed spots inside
//! it. Like Wizard101, the globes sit in the bottom-left corner, the experience bar (here: progress through the
//! current Elden Ring level, runes held / level-up cost) centred along the bottom and the spellbook in the bottom-right.
//! The numbers use Wizard101's HUD font, the WizardFancy bitmap font (`w101_orbs/WizardFancy.xml` and its atlas PNGs,
//! also made by tools/w101_hud_art.py); without those files they fall back to imgui's default font.

mod book;
mod font;
mod game;
mod input;
mod itemdata;
mod layout;
mod loadscreen;
mod markers;
mod nameplates;
mod steam;
mod teleport;
mod model;
mod names;
mod pages;
mod probe;
mod quests;
mod settings;
mod sheet;

use std::path::{Path, PathBuf};
use std::time::Instant;

use hudhook::hooks::dx12::ImguiDx12Hooks;
use hudhook::imgui::{self, DrawListMut, FontConfig, FontSource, ImColor32, TextureId};
use hudhook::{ImguiRenderLoop, MessageFilter, RenderContext};

use font::BitmapFont;
pub(crate) use settings::{load_settings, save_settings, Settings};

const FRAMES_PER_SECOND: f32 = 12.0;
/// Bottom edge of the lowest (energy) globe on Wizard101's 768-pixel-tall screen.
const BOTTOM: f32 = 723.0;
const HUD_HEIGHT: f32 = 768.0;
/// XPBar (centred horizontally) and its progressXPSolid inside it.
const XP_BAR: [f32; 4] = [326.0, 733.0, 830.0, 752.0];
const XP_FILL: [f32; 4] = [18.0, 1.0, 486.0, 18.0];
/// btnSpellbook is 128x128 (948,571-1076,699 in HUDWindow.gui, which has no docking flags for it); Wizard101 puts it
/// flush in the bottom-right corner: TutorialSpellbook.lua moves it to (GetScreenWidth() - 128,
/// GetScreenHeight() - 128). Button_Spellbook3 is two tiles side by side and the normal state shows the first.
const SPELLBOOK_SIZE: f32 = 128.0;
/// Text colour of the globe numbers: WindowStyle m_Color FF00FFFF (Wizard101 colours are AABBGGRR), yellow.
const TEXT_COLOUR: ImColor32 = ImColor32::from_rgb(255, 255, 0);
/// Size of the level number next to the experience bar, as a WizardFancy scale.
const LEVEL_SCALE: f32 = 0.6;

/// One globe from HUDWindow.gui. `window` is in screen pixels, the rest is relative to it.
struct Layout {
    window: [f32; 4],
    liquid: [f32; 4],
    chop: [f32; 2],
    text: [f32; 2],
    /// WizardFancy's scale in HUDWindow.gui.
    font_scale: f32,
}

const HEALTH: Layout = Layout {
    window: [0.0, 570.0, 118.0, 718.0],    // spriteHealthBkg
    liquid: [15.0, 13.0, 104.0, 101.0],    // progressHealthSolid
    chop: [86.0, 9.0],                     // spriteHealthChop (16,52)-(102,61), moved to the surface
    text: [59.5, 57.0],                    // textHealth, centred in HealthGlobeRect
    font_scale: 0.7,
};

const MANA: Layout = Layout {
    window: [72.0, 618.0, 156.0, 722.0],   // spriteManaBkg
    liquid: [11.0, 9.0, 73.0, 71.0],       // progressManaSolid
    chop: [60.0, 7.0],                     // spriteManaChop (12,36)-(72,43)
    text: [42.0, 40.0],                    // textMana, centred in ManaGlobeRect
    font_scale: 0.642857,
};

/// Wizard101's energy globe (spriteEnergyBkg, drawn after the mana globe) shows Elden Ring's stamina.
const ENERGY: Layout = Layout {
    window: [29.0, 646.0, 95.0, 723.0],    // spriteEnergyBkg, glass Energy_Back
    liquid: [11.0, 6.0, 55.0, 50.0],       // progressEnergySolid
    chop: [44.0, 7.0],                     // spriteEnergyChop (11,22)-(55,29)
    text: [33.0, 28.0],                    // textEnergy, centred in EnergyGlobeRect (5,0)-(61,56)
    font_scale: 0.502857,
};

struct Art {
    back: TextureId,
    health: TextureId,
    health_chop: TextureId,
    mana: TextureId,
    mana_chop: TextureId,
    energy_back: TextureId,
    energy: TextureId,
    energy_chop: TextureId,
    xp_fill: TextureId,
    xp_frame: TextureId,
    spellbook: TextureId,
}

struct Orbs {
    folder: PathBuf,
    settings: Settings,
    art: Option<Art>,
    font: Option<BitmapFont>,
    book: book::Book,
    loadscreen: loadscreen::LoadScreen,
    game: Option<Option<game::Game>>,
    shown: [f32; 4],
    start: Instant,
    error: Option<String>,
    perf: Perf,
    logged_size: [f32; 2],
    last_probe_log: Instant,
    /// Whether the game window had focus last frame.
    focused: bool,
    /// The event flag group check is logged once, when the game is up.
    flag_check_logged: bool,
    /// Wizard101's teleport poof before warps (teleport.rs).
    teleport: teleport::Teleport,
    /// The opt-in scene probe (ini scene_probe = 1).
    probe: Option<probe::Probe>,
}

/// Frame timing written to w101_orbs.log every few seconds, to see what the overlay costs the game.
struct Perf {
    since: Instant,
    last_frame: Option<Instant>,
    frames: u32,
    worst_gap: f32,
    inside: f32,
    worst_inside: f32,
    initializations: u32,
}

impl Perf {
    fn frame(&mut self, inside: f32) {
        let now = Instant::now();
        if let Some(last) = self.last_frame {
            self.worst_gap = self.worst_gap.max((now - last).as_secs_f32());
        }
        self.last_frame = Some(now);
        self.frames += 1;
        self.inside += inside;
        self.worst_inside = self.worst_inside.max(inside);
        let elapsed = (now - self.since).as_secs_f32();
        if elapsed >= 5.0 {
            tracing::info!(
                "{:.1} fps, slowest frame {:.0} ms, orbs code {:.3} ms average / {:.3} ms worst, {} initializations",
                self.frames as f32 / elapsed, self.worst_gap * 1000.0, self.inside * 1000.0 / self.frames as f32,
                self.worst_inside * 1000.0, self.initializations,
            );
            *self = Perf { since: now, last_frame: Some(now), initializations: self.initializations, ..Perf::new() };
        }
    }

    fn new() -> Self {
        Perf { since: Instant::now(), last_frame: None, frames: 0, worst_gap: 0.0, inside: 0.0, worst_inside: 0.0,
               initializations: 0 }
    }
}

impl Orbs {
    fn new() -> Self {
        let folder = dll_folder();
        start_log(&folder.join("w101_orbs.log"));
        let settings = load_settings(&folder.join("w101_orbs.ini"));
        let probe = settings.scene_probe.then(|| {
            tracing::info!("probe: scene probe on (nudge {})", settings.scene_probe_nudge);
            for line in probe::install_d3d_hooks() {
                tracing::info!("probe: {line}");
            }
            probe::Probe::new()
        });
        Orbs {
            settings,
            probe,
            folder,
            art: None,
            font: None,
            book: book::Book::new(),
            loadscreen: loadscreen::LoadScreen::default(),
            game: None,
            shown: [1.0, 1.0, 0.0, 1.0],
            start: Instant::now(),
            error: None,
            perf: Perf::new(),
            logged_size: [0.0, 0.0],
            last_probe_log: Instant::now(),
            focused: true,
            flag_check_logged: false,
            teleport: teleport::Teleport::default(),
        }
    }

    fn load_art(&self, render: &mut dyn RenderContext) -> Result<Art, String> {
        let art = self.folder.join("w101_orbs");
        let mut load = |name: &str| -> Result<TextureId, String> {
            let path = art.join(format!("{name}.png"));
            let image = image::open(&path).map_err(|e| format!("{}: {e}", path.display()))?.into_rgba8();
            render.load_texture(&image, image.width(), image.height()).map_err(|e| format!("{name}: {e:?}"))
        };
        Ok(Art {
            back: load("Health_Back")?,
            health: load("Health_Liquid")?,
            health_chop: load("Health_Chop_Anim")?,
            mana: load("Mana_Liquid")?,
            mana_chop: load("Mana_Chop_Anim")?,
            energy_back: load("Energy_Back")?,
            energy: load("Energy_Liquid")?,
            energy_chop: load("Energy_Chop_Anim")?,
            xp_fill: load("Bar_Exp01")?,
            xp_frame: load("Art_Bar_Exp")?,
            spellbook: load("Button_Spellbook3")?,
        })
    }

}

impl ImguiRenderLoop for Orbs {
    fn initialize<'a>(&'a mut self, ctx: &mut imgui::Context, render: &'a mut dyn RenderContext) {
        self.perf.initializations += 1;
        tracing::info!("initialize #{}", self.perf.initializations);
        ctx.set_ini_filename(None);
        ctx.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(FontConfig { size_pixels: 32.0, ..FontConfig::default() }),
        }]);
        match self.load_art(render) {
            Ok(art) => self.art = Some(art),
            Err(e) => self.error = Some(e),
        }
        match BitmapFont::load(&self.folder.join("w101_orbs"), "WizardFancy", render) {
            Ok(font) => self.font = Some(font),
            Err(e) => tracing::warn!("WizardFancy font not loaded, using imgui's default font: {e}"),
        }
        self.book.load(&self.folder.join("w101_orbs").join("book"), render);
        self.book.textures.want([loadscreen::IMAGE.to_string()]);
        let flip = self.folder.join("w101_orbs").join("book").join("page_flip.tsv");
        self.loadscreen.load(&self.folder, &self.settings.loadscreen_image, &flip, render);
        self.book.load_art(render);
        for line in input::install() {
            tracing::info!("input hook: {line}");
        }
    }

    fn before_render<'a>(&'a mut self, ctx: &mut imgui::Context, render: &'a mut dyn RenderContext) {
        // Alt-tab: imgui and the input hooks keep whatever was held when the window went away (hudhook passes no
        // focus messages to imgui), which left the open book unresponsive. Clear it all on every focus change.
        let focused = input::game_focused();
        if focused != self.focused {
            self.focused = focused;
            unsafe { imgui::sys::ImGuiIO_AddFocusEvent(ctx.io_mut() as *mut imgui::Io as *mut imgui::sys::ImGuiIO, focused) };
            input::reset_after_focus_change();
            self.book.focus_changed(focused);
        }
        self.book.load_art(render);
        self.book.capture(render, self.settings.bookmap);
        let game = self.game.as_ref().and_then(|g| g.as_ref());
        let plain = game.is_some() && !self.book.open && !self.loadscreen.holding() && game::hud_shown()
            && game::faded() < 0.05;
        self.teleport.capture(render, &mut self.book.textures, plain, game.and_then(|g| g.camera()));
        // Elden Ring hides and confines the cursor; the open book needs it.
        ctx.io_mut().mouse_draw_cursor = false; // the book draws its own
        if self.book.open {
            unsafe { windows_sys::Win32::UI::WindowsAndMessaging::ClipCursor(std::ptr::null()) };
        }
    }

    fn render(&mut self, ui: &mut imgui::Ui) {
        let begin = Instant::now();
        let stats = self.draw(ui);
        let game = self.game.as_ref().and_then(|g| g.as_ref());
        let hud = game::hud_shown();
        let faded = game::faded();
        let position = game.and_then(|g| g.position());
        let settings_path = self.folder.join("w101_orbs.ini");
        let mut cx = book::Context {
            game,
            stats,
            gameplay: game.is_some() && hud && faded <= 0.3 && stats.is_some(),
            hud,
            faded,
            position,
            settings: &mut self.settings,
            settings_path: &settings_path,
        };
        self.book.frame(ui, &mut cx);
        if !self.loadscreen.holding() {
            self.book.draw_markers(ui, &cx);
            self.book.draw_nameplates(ui, &cx);
        }
        // Polled every frame: the screen goes up on the first frame the door flag is on, or Elden Ring starts a
        // loading screen of its own. The flag is cleared when the screen starts to fade (or right away when the
        // loading screen is turned off).
        let door = game.is_some() && game::event_flag(loadscreen::DOOR_FLAG) == Some(true);
        let loading = self.loadscreen.game_loading_started(game::loading_screens());
        let loading = self.teleport.frame(ui, &mut self.book.textures, game, loading, door, stats.map(|s| s.hp));
        if !self.loadscreen.holding() && (door || loading) {
            if self.settings.loadscreen {
                self.loadscreen.show(if door { "door warp flag" } else { "Elden Ring loading screen" });
            } else if door {
                game::set_event_flag(loadscreen::DOOR_FLAG, false);
            }
        }
        if !self.flag_check_logged && game.is_some() && stats.is_some() {
            self.flag_check_logged = true;
            let group = loadscreen::DOOR_FLAG / 1000;
            tracing::info!("event flags: group {group} (door / quest / warp flags) held: {:?}; loading screens so far: {:?}",
                           game::flag_group_exists(group), game::loading_screens());
        }
        // Still warping: no game data, faded out, no player, or Elden Ring's own loading screen (its HUD is not
        // in its default state then).
        let busy = game.is_none() || faded > 0.3 || stats.is_none() || !hud;
        self.loadscreen.draw(ui, &self.book.textures, busy, stats.is_some());
        if let Some(probe) = &mut self.probe {
            probe.tick(self.game.as_ref().and_then(|g| g.as_ref()), self.settings.scene_probe_nudge);
        }
        self.perf.frame(begin.elapsed().as_secs_f32());
    }

    fn message_filter(&self, _io: &imgui::Io) -> MessageFilter {
        if self.book.blocking() { MessageFilter::InputAll } else { MessageFilter::empty() }
    }
}

impl Orbs {
    /// Draws the HUD; returns the player's stats when there is a character on screen.
    fn draw(&mut self, ui: &mut imgui::Ui) -> Option<game::Stats> {
        // The pattern scan reads the whole executable, so it runs once.
        let game = self.game.get_or_insert_with(game::Game::find);
        if game.is_none() {
            self.error.get_or_insert_with(|| "Elden Ring's player data wasn't found".into());
        }
        let draw = ui.get_background_draw_list();
        if let Some(error) = &self.error {
            draw.add_text([20.0, 20.0], ImColor32::from_rgb(255, 80, 80), format!("w101_orbs: {error}"));
        }
        let (Some(art), Some(Some(game))) = (&self.art, &self.game) else { return None };
        if !game::hud_shown() || game::faded() > 0.3 {
            return None;
        }
        let probe = game.probe();
        if self.settings.debug && self.last_probe_log.elapsed().as_secs_f32() >= 1.0 {
            self.last_probe_log = Instant::now();
            tracing::info!("probe {probe:x?}");
        }
        let Some(stats) = probe.stats() else { return None };

        let height = ui.io().display_size[1];
        let s = self.settings.scale * height / 768.0;
        // Screen position of Wizard101's (0, 0), so that its bottom-left corner lands `left`, `bottom` from ours.
        let origin = [self.settings.left * s, height - (BOTTOM + self.settings.bottom) * s];
        let frame = (self.start.elapsed().as_secs_f32() * FRAMES_PER_SECOND) as usize % 9;
        let xp = stats.level.map_or(0.0, |(level, runes)| game::level_progress(level, runes));
        let target = [fraction(stats.hp, stats.max_hp), fraction(stats.fp, stats.max_fp), xp,
                      fraction(stats.sp, stats.max_sp)];
        // Ease toward the new value so hits drain the globe instead of snapping.
        for (shown, target) in self.shown.iter_mut().zip(target) {
            *shown += (target - *shown) * 0.2;
            if (target - *shown).abs() < 0.002 {
                *shown = target;
            }
        }
        // Same order as HUDWindow.gui, so the mana globe sits in front of the health globe.
        globe(&draw, &HEALTH, art.back, art.health, art.health_chop, origin, s, self.shown[0], frame);
        globe(&draw, &MANA, art.back, art.mana, art.mana_chop, origin, s, self.shown[1], frame);
        number(&draw, self.font.as_ref(), stats.hp, &HEALTH, origin, s);
        globe(&draw, &ENERGY, art.energy_back, art.energy, art.energy_chop, origin, s, self.shown[3], frame);
        number(&draw, self.font.as_ref(), stats.fp, &MANA, origin, s);
        number(&draw, self.font.as_ref(), stats.sp, &ENERGY, origin, s);

        // Experience bar, centred along the bottom; spellbook in the bottom-right corner.
        let s = self.settings.scale * height / HUD_HEIGHT;
        let width = ui.io().display_size[0];
        let bar = [(width - (XP_BAR[2] - XP_BAR[0]) * s) / 2.0, height - (HUD_HEIGHT - XP_BAR[1]) * s];
        let p = |at: [f32; 2], x: f32, y: f32| [at[0] + x * s, at[1] + y * s];
        if stats.level.is_some() {
            let [l, t, r, b] = XP_FILL;
            let fill = self.shown[2];
            if fill > 0.0 {
                // PROGRESS_UV: the left part of the bar texture is shown, not squashed.
                draw.add_image(art.xp_fill, p(bar, l, t), p(bar, l + (r - l) * fill, b)).uv_max([fill, 1.0]).build();
            }
        }
        draw.add_image(art.xp_frame, bar, p(bar, XP_BAR[2] - XP_BAR[0], XP_BAR[3] - XP_BAR[1])).build();
        if let (true, Some((level, _)), Some(font)) = (self.settings.level, stats.level, self.font.as_ref()) {
            let text = level.to_string();
            let scale = LEVEL_SCALE * s;
            let w = font.width(&text) * scale;
            let centre = [bar[0] - 6.0 * s - w / 2.0, bar[1] + (XP_BAR[3] - XP_BAR[1]) * s / 2.0];
            outlined(&draw, font, &text, centre, scale, s);
        }
        let book = [width - SPELLBOOK_SIZE * s, height - SPELLBOOK_SIZE * s];
        if self.logged_size != [width, height] {
            self.logged_size = [width, height];
            tracing::info!("screen {width}x{height}, HUD scale {s:.3}, xp bar at {bar:?}, spellbook at {book:?}");
        }
        draw.add_image(art.spellbook, book, [width, height])
            .uv_max([0.5, 1.0])
            .build();
        if self.settings.debug {
            draw.add_text([20.0, 50.0], ImColor32::WHITE, format!("{stats:?}"));
        }
        Some(stats)
    }
}

fn fraction(value: i32, max: i32) -> f32 {
    if max <= 0 { 0.0 } else { (value as f32 / max as f32).clamp(0.0, 1.0) }
}

/// UV corners of tile `frame` in a 3x3 sprite sheet.
fn tile(frame: usize) -> ([f32; 2], [f32; 2]) {
    let (u, v) = ((frame % 3) as f32 / 3.0, (frame / 3) as f32 / 3.0);
    ([u, v], [u + 1.0 / 3.0, v + 1.0 / 3.0])
}

/// One globe: the glass, the liquid filled from the bottom to `fill` (Wizard101's PROGRESS_UV: the bottom of the
/// frame is shown, not squashed), and the sloshing surface on top of it.
#[allow(clippy::too_many_arguments)]
fn globe(draw: &DrawListMut, layout: &Layout, back: TextureId, liquid: TextureId, chop: TextureId, origin: [f32; 2],
         s: f32, fill: f32, frame: usize) {
    let [x0, y0, x1, y1] = layout.window;
    let p = |x: f32, y: f32| [origin[0] + (x0 + x) * s, origin[1] + (y0 + y) * s];
    draw.add_image(back, p(0.0, 0.0), p(x1 - x0, y1 - y0)).build();
    if fill <= 0.0 {
        return;
    }
    let [left, top, right, bottom] = layout.liquid;
    let [chop_width, chop_height] = layout.chop;
    let surface = bottom - (bottom - top) * fill;
    let (uv0, uv1) = tile(frame);
    let v = uv1[1] - (uv1[1] - uv0[1]) * fill;
    draw.add_image(liquid, p(left, surface), p(right, bottom)).uv_min([uv0[0], v]).uv_max(uv1).build();
    if fill < 0.98 {
        let (c0, c1) = tile(frame);
        let middle = (left + right) / 2.0;
        // The liquid is a sphere, so the surface narrows toward the top and bottom.
        let chop_width = chop_width * (1.0 - (2.0 * fill - 1.0).powi(2)).sqrt().max(0.25);
        draw.add_image(chop, p(middle - chop_width / 2.0, surface - chop_height / 2.0),
                       p(middle + chop_width / 2.0, surface + chop_height / 2.0)).uv_min(c0).uv_max(c1).build();
    }
}

/// Yellow WizardFancy text with a dark outline, centred on `centre`.
fn outlined(draw: &DrawListMut, font: &BitmapFont, text: &str, centre: [f32; 2], scale: f32, s: f32) {
    let outline = ImColor32::from_rgba(0, 0, 0, 220);
    let o = 1.5 * s.max(1.0);
    for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o), (o, o)] {
        font.draw(draw, text, [centre[0] + dx, centre[1] + dy], scale, outline);
    }
    font.draw(draw, text, centre, scale, TEXT_COLOUR);
}

/// The value in the middle of a globe, yellow with a dark outline, in WizardFancy when it loaded.
fn number(draw: &DrawListMut, font: Option<&BitmapFont>, value: i32, layout: &Layout, origin: [f32; 2], s: f32) {
    use hudhook::imgui::sys;
    let text = value.to_string();
    let centre = [origin[0] + (layout.window[0] + layout.text[0]) * s, origin[1] + (layout.window[1] + layout.text[1]) * s];
    let outline = ImColor32::from_rgba(0, 0, 0, 220);
    if let Some(font) = font {
        outlined(draw, font, &text, centre, layout.font_scale * s, s);
        return;
    }
    // imgui's default font: about the height WizardFancy would have.
    let size = 24.0 / 0.7 * layout.font_scale * s;
    unsafe {
        let draw = sys::igGetBackgroundDrawList();
        let font = sys::igGetFont();
        let (begin, end) = (text.as_ptr().cast(), text.as_ptr().add(text.len()).cast());
        let mut extent = sys::ImVec2 { x: 0.0, y: 0.0 };
        sys::ImFont_CalcTextSizeA(&mut extent, font, size, f32::MAX, 0.0, begin, end, std::ptr::null_mut());
        let at = [centre[0] - extent.x / 2.0, centre[1] - extent.y / 2.0];
        let outline = u32::from(outline);
        for (dx, dy) in [(-1.5, 0.0), (1.5, 0.0), (0.0, -1.5), (0.0, 1.5)] {
            let pos = sys::ImVec2 { x: at[0] + dx, y: at[1] + dy };
            sys::ImDrawList_AddText_FontPtr(draw, font, size, pos, outline, begin, end, 0.0, std::ptr::null());
        }
        let pos = sys::ImVec2 { x: at[0], y: at[1] };
        sys::ImDrawList_AddText_FontPtr(draw, font, size, pos, u32::from(TEXT_COLOUR), begin, end, 0.0, std::ptr::null());
    }
}

/// Sends hudhook's own messages and ours to `path` (replaced on each launch).
fn start_log(path: &Path) {
    let Ok(file) = std::fs::File::create(path) else { return };
    let _ = tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(file))
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::new("info,hudhook=debug"))
        .try_init();
}

fn dll_folder() -> PathBuf {
    use windows_sys::Win32::System::LibraryLoader::{
        GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
        GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    };
    let mut module = std::ptr::null_mut();
    let mut buffer = [0u16; 1024];
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            dll_folder as *const u16,
            &mut module,
        );
        let len = GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as u32) as usize;
        let path = PathBuf::from(String::from_utf16_lossy(&buffer[..len]));
        path.parent().map(Path::to_path_buf).unwrap_or_default()
    }
}

hudhook::hudhook!(ImguiDx12Hooks, Orbs::new());
