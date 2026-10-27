//! The Wizard101 loading screen for the mod's warps.
//!
//! The door and map-page warps are EMEVD warps. Each event turns on event flag `DOOR_FLAG` right before its warp; the
//! overlay then covers the whole screen with the loading picture (Wizard City's by default; `loadscreen_image` in
//! w101_orbs.ini swaps it) and no loading bar, over Elden Ring's own loading screen and bar, until the game is back
//! (at least `SHOW` seconds), fades it out, and turns the flag back off.

use std::path::Path;
use std::time::Instant;

use hudhook::imgui::{ImColor32, TextureId, Ui};
use hudhook::RenderContext;

use crate::layout::Textures;

/// Set by the warp events (m60_42_36_00.emevd doors, common.emevd map-page warps) just before a warp; cleared here.
pub const DOOR_FLAG: u32 = 1042366900;
/// The door events: flag on, 0.75 s, fade-out and warp, fade-in about 1 s after the flag. The screen holds
/// fully up this long from the flag, covering all of it.
const SHOW: f32 = 1.8;
const FADE: f32 = 0.35;
/// The longest the screen waits for the game to come back from a warp (its own fade or loading) before fading out.
const MAX_WAIT: f32 = 60.0;

/// Wizard City's loading screen in the book's art, used when the ini's picture can't be loaded.
pub const IMAGE: &str = "GUI/LoadScreen_W01_WC_01.png";

#[derive(Default)]
pub struct LoadScreen {
    /// The picture from w101_orbs.ini's loadscreen_image, and its size.
    image: Option<(TextureId, [f32; 2])>,
    shown_at: Option<Instant>,
    /// When the fade-out began: after SHOW seconds, once the game has finished its warp.
    fading_at: Option<Instant>,
}

impl LoadScreen {
    /// Whether the screen is up and still holding (the door flag is left on until then).
    pub fn holding(&self) -> bool {
        self.shown_at.is_some() && self.fading_at.is_none()
    }

    /// Loads the picture `path` (relative to `folder`, or absolute); without it the book's Wizard City picture is
    /// used.
    pub fn load(&mut self, folder: &Path, path: &str, render: &mut dyn RenderContext) {
        let path = folder.join(path);
        match image::open(&path) {
            Ok(image) => {
                let image = image.into_rgba8();
                match render.load_texture(&image, image.width(), image.height()) {
                    Ok(id) => self.image = Some((id, [image.width() as f32, image.height() as f32])),
                    Err(e) => tracing::warn!("loading screen picture {}: {e:?}", path.display()),
                }
            }
            Err(e) => tracing::warn!("loading screen picture {}: {e}", path.display()),
        }
    }

    /// Shows the loading screen from now.
    pub fn show(&mut self) {
        tracing::info!("door warp: showing the loading screen");
        self.shown_at = Some(Instant::now());
        self.fading_at = None;
    }

    /// Draws the screen while it is up. `busy`: the game is still warping (faded out, loading, or no player yet);
    /// the screen stays fully up until it is done, so it covers the whole warp.
    pub fn draw(&mut self, ui: &Ui, textures: &Textures, busy: bool) {
        let Some(at) = self.shown_at else { return };
        let t = at.elapsed().as_secs_f32();
        if self.fading_at.is_none() && t >= SHOW && (!busy || t >= MAX_WAIT) {
            self.fading_at = Some(Instant::now());
            // The warp is over: clear the door flag (it stays on while the screen is up).
            crate::game::set_event_flag(DOOR_FLAG, false);
        }
        let alpha = match self.fading_at {
            None => 1.0,
            Some(f) => 1.0 - f.elapsed().as_secs_f32() / FADE,
        };
        if alpha <= 0.0 {
            self.shown_at = None;
            return;
        }
        let a = |c: ImColor32| ImColor32::from_rgba(c.r, c.g, c.b, (c.a as f32 * alpha) as u8);
        let [w, h] = ui.io().display_size;
        let draw = ui.get_foreground_draw_list();
        // The whole screen: Wizard101's night-sky purple, then the picture as large as fits (the default one is
        // square, so the full height, centred). Nothing of Elden Ring's loading screen shows through.
        draw.add_rect([0.0, 0.0], [w, h], a(ImColor32::from_rgb(18, 12, 40))).filled(true).build();
        if let Some((image, [iw, ih])) = self.image.or_else(|| textures.get(IMAGE)) {
            let k = (w / iw).min(h / ih);
            let size = [iw * k, ih * k];
            let (x, y) = ((w - size[0]) / 2.0, (h - size[1]) / 2.0);
            draw.add_image(image, [x, y], [x + size[0], y + size[1]]).col(a(ImColor32::WHITE)).build();
        }
    }
}
