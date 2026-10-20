//! Wizard101 health and mana globes for Elden Ring, drawn over the game with hudhook (Dear ImGui on DirectX 12).
//!
//! Loaded by Mod Engine 2 (`external_dlls`). Next to the DLL it expects a `w101_orbs` folder with the globe art
//! (made by tools/w101_hud_art.py) and optionally `w101_orbs.ini` to move or resize the globes.
//! The layout is Wizard101's own HUDWindow.gui, in its pixels on a 768-pixel-tall screen: each globe's glass is
//! Health_Back stretched over the globe's window, with the liquid, its surface and the number at fixed spots inside
//! it. Like Wizard101, the globes sit in the bottom-left corner.

mod game;

use std::path::{Path, PathBuf};
use std::time::Instant;

use hudhook::hooks::dx12::ImguiDx12Hooks;
use hudhook::imgui::{self, DrawListMut, FontConfig, FontSource, ImColor32, TextureId};
use hudhook::{ImguiRenderLoop, RenderContext};

const FRAMES_PER_SECOND: f32 = 12.0;
/// Bottom edge of the lower (mana) globe on Wizard101's 768-pixel-tall screen.
const BOTTOM: f32 = 722.0;

/// One globe from HUDWindow.gui. `window` is in screen pixels, the rest is relative to it.
struct Layout {
    window: [f32; 4],
    liquid: [f32; 4],
    chop: [f32; 2],
    text: [f32; 2],
    text_size: f32,
}

const HEALTH: Layout = Layout {
    window: [0.0, 570.0, 118.0, 718.0],    // spriteHealthBkg
    liquid: [15.0, 13.0, 104.0, 101.0],    // progressHealthSolid
    chop: [86.0, 9.0],                     // spriteHealthChop (16,52)-(102,61), moved to the surface
    text: [59.5, 57.0],                    // textHealth, centred in HealthGlobeRect
    text_size: 24.0,                       // WizardFancy at 0.7
};

const MANA: Layout = Layout {
    window: [72.0, 618.0, 156.0, 722.0],   // spriteManaBkg
    liquid: [11.0, 9.0, 73.0, 71.0],       // progressManaSolid
    chop: [60.0, 7.0],                     // spriteManaChop (12,36)-(72,43)
    text: [42.0, 40.0],                    // textMana, centred in ManaGlobeRect
    text_size: 24.0 * 0.642857 / 0.7,      // WizardFancy at 0.642857
};

struct Art {
    back: TextureId,
    health: TextureId,
    health_chop: TextureId,
    mana: TextureId,
    mana_chop: TextureId,
}

struct Settings {
    left: f32,
    bottom: f32,
    scale: f32,
    debug: bool,
}

struct Orbs {
    folder: PathBuf,
    settings: Settings,
    art: Option<Art>,
    game: Option<Option<game::Game>>,
    shown: [f32; 2],
    start: Instant,
    error: Option<String>,
    perf: Perf,
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
        Orbs {
            settings: load_settings(&folder.join("w101_orbs.ini")),
            folder,
            art: None,
            game: None,
            shown: [1.0, 1.0],
            start: Instant::now(),
            error: None,
            perf: Perf::new(),
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
    }

    fn render(&mut self, ui: &mut imgui::Ui) {
        let begin = Instant::now();
        self.draw(ui);
        self.perf.frame(begin.elapsed().as_secs_f32());
    }
}

impl Orbs {
    fn draw(&mut self, ui: &mut imgui::Ui) {
        // The pattern scan reads the whole executable, so it runs once.
        let game = self.game.get_or_insert_with(game::Game::find);
        if game.is_none() {
            self.error.get_or_insert_with(|| "Elden Ring's player data wasn't found".into());
        }
        let draw = ui.get_background_draw_list();
        if let Some(error) = &self.error {
            draw.add_text([20.0, 20.0], ImColor32::from_rgb(255, 80, 80), format!("w101_orbs: {error}"));
        }
        let (Some(art), Some(Some(game))) = (&self.art, &self.game) else { return };
        if !game::hud_shown() || game::faded() > 0.3 {
            return;
        }
        let Some(stats) = game.stats() else { return };

        let height = ui.io().display_size[1];
        let s = self.settings.scale * height / 768.0;
        // Screen position of Wizard101's (0, 0), so that its bottom-left corner lands `left`, `bottom` from ours.
        let origin = [self.settings.left * s, height - (BOTTOM + self.settings.bottom) * s];
        let frame = (self.start.elapsed().as_secs_f32() * FRAMES_PER_SECOND) as usize % 9;
        let target = [fraction(stats.hp, stats.max_hp), fraction(stats.fp, stats.max_fp)];
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
        number(stats.hp, &HEALTH, origin, s);
        number(stats.fp, &MANA, origin, s);
        if self.settings.debug {
            draw.add_text([20.0, 50.0], ImColor32::WHITE, format!("{stats:?}"));
        }
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

/// The value in the middle of a globe, white with a dark outline like Wizard101's.
fn number(value: i32, layout: &Layout, origin: [f32; 2], s: f32) {
    use hudhook::imgui::sys;
    let text = value.to_string();
    let size = layout.text_size * s;
    let centre = [origin[0] + (layout.window[0] + layout.text[0]) * s, origin[1] + (layout.window[1] + layout.text[1]) * s];
    unsafe {
        let draw = sys::igGetBackgroundDrawList();
        let font = sys::igGetFont();
        let (begin, end) = (text.as_ptr().cast(), text.as_ptr().add(text.len()).cast());
        let mut extent = sys::ImVec2 { x: 0.0, y: 0.0 };
        sys::ImFont_CalcTextSizeA(&mut extent, font, size, f32::MAX, 0.0, begin, end, std::ptr::null_mut());
        let at = [centre[0] - extent.x / 2.0, centre[1] - extent.y / 2.0];
        let outline = u32::from(ImColor32::from_rgba(0, 0, 0, 220));
        for (dx, dy) in [(-1.5, 0.0), (1.5, 0.0), (0.0, -1.5), (0.0, 1.5)] {
            let pos = sys::ImVec2 { x: at[0] + dx, y: at[1] + dy };
            sys::ImDrawList_AddText_FontPtr(draw, font, size, pos, outline, begin, end, 0.0, std::ptr::null());
        }
        let pos = sys::ImVec2 { x: at[0], y: at[1] };
        sys::ImDrawList_AddText_FontPtr(draw, font, size, pos, u32::from(ImColor32::WHITE), begin, end, 0.0, std::ptr::null());
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

/// `key = value` lines: left and bottom (extra gap from the screen's bottom-left corner, in Wizard101 pixels; 0 is
/// exactly where Wizard101 puts the globes), scale (1 = Wizard101's size on a 768-pixel-tall screen) and debug (1
/// shows the raw numbers).
fn load_settings(path: &Path) -> Settings {
    let mut settings = Settings { left: 0.0, bottom: 0.0, scale: 1.0, debug: false };
    for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        match key.trim() {
            "left" => settings.left = value.parse().unwrap_or(settings.left),
            "bottom" => settings.bottom = value.parse().unwrap_or(settings.bottom),
            "scale" => settings.scale = value.parse().unwrap_or(settings.scale),
            "debug" => settings.debug = value == "1",
            _ => {}
        }
    }
    settings
}

hudhook::hudhook!(ImguiDx12Hooks, Orbs::new());
