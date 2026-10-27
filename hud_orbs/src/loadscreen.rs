//! The Wizard101 loading screen, over every Elden Ring loading screen and the mod's warps.
//!
//! Two triggers: Elden Ring's own count of loading screens (CSNowLoadingHelper, `game::loading_screens`) going up,
//! which covers every loading screen the game shows (fast travel, the riding doors' map warps, death, loading a
//! save); and event flag `DOOR_FLAG`, which the door and map-page warp events turn on right before they warp (the
//! on-foot doors move the player without any loading screen of the game's). Either way the overlay covers the whole
//! screen with the loading picture (Wizard City's by default; `loadscreen_image` in w101_orbs.ini swaps it) and no
//! loading bar, over Elden Ring's own loading screen, bar and tips, until the game is back (at least `SHOW` seconds),
//! fades it out, and turns the flag back off.
//!
//! Round 9 only had the flag: Ryan's log of 2026-10-27 has no "showing the loading screen" line at all, so the flag
//! was never seen on, while the door's map warp still brought up Elden Ring's loading screen.

use std::path::Path;
use std::time::Instant;

use hudhook::imgui::{ImColor32, TextureId, Ui};
use hudhook::RenderContext;

use crate::layout::Textures;

/// Set by the warp events (m60_42_36_00.emevd doors, common.emevd map-page warps) just before a warp; cleared here.
pub const DOOR_FLAG: u32 = 1042362900;
/// The door events: flag on, 0.75 s, fade-out and warp, fade-in about 1 s after the flag. The screen holds
/// fully up this long from the flag, covering all of it.
const SHOW: f32 = 1.8;
const FADE: f32 = 0.35;
/// The longest the screen waits for the game to come back from a warp (its own fade or loading) before fading out.
const MAX_WAIT: f32 = 60.0;
/// The longest it waits without the game coming back to play at all (the title screen, if quitting there
/// counts as a loading screen).
const MAX_WAIT_NO_PLAYER: f32 = 30.0;
/// The game's loading screen count can go up a moment before it fades or drops the player, so the screen waits
/// to see the game busy before it fades; if it never does within this long, it fades anyway.
const NEVER_BUSY: f32 = 5.0;

/// Wizard City's loading screen in the book's art, used when the ini's picture can't be loaded.
pub const IMAGE: &str = "GUI/LoadScreen_W01_WC_01.png";

#[derive(Default)]
pub struct LoadScreen {
    /// The picture from w101_orbs.ini's loadscreen_image, and its size.
    image: Option<(TextureId, [f32; 2])>,
    shown_at: Option<Instant>,
    /// When the fade-out began: after SHOW seconds, once the game has finished its warp.
    fading_at: Option<Instant>,
    /// Elden Ring's loading screen count last frame.
    last_count: Option<i32>,
    /// When the game last had a character in the world (stats), while the screen is up.
    player_seen: Option<Instant>,
    /// Whether the game has been busy (faded, loading, no player) since the screen came up.
    saw_busy: bool,
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

    /// Shows the loading screen from now (`why` goes to the log).
    pub fn show(&mut self, why: &str) {
        tracing::info!("loading screen: showing ({why})");
        self.shown_at = Some(Instant::now());
        self.saw_busy = false;
        self.fading_at = None;
        self.player_seen = Some(Instant::now());
    }

    /// Whether Elden Ring has started one of its loading screens since the last call (`count`: its loading
    /// screen count now, None when it can't be read).
    pub fn game_loading_started(&mut self, count: Option<i32>) -> bool {
        let started = matches!((self.last_count, count), (Some(a), Some(b)) if a != b);
        if count.is_some() {
            self.last_count = count;
        }
        started
    }

    /// Draws the screen while it is up. `busy`: the game is still warping (faded out, loading, or no player yet);
    /// the screen stays fully up until it is done, so it covers the whole warp.
    pub fn draw(&mut self, ui: &Ui, textures: &Textures, busy: bool, player: bool) {
        let Some(at) = self.shown_at else { return };
        let t = at.elapsed().as_secs_f32();
        if player || self.player_seen.is_none() {
            self.player_seen = Some(Instant::now());
        }
        let no_player = self.player_seen.map_or(0.0, |p| p.elapsed().as_secs_f32());
        self.saw_busy |= busy;
        let back = !busy && (self.saw_busy || t >= NEVER_BUSY);
        if self.fading_at.is_none() && t >= SHOW && (back || t >= MAX_WAIT || no_player >= MAX_WAIT_NO_PLAYER) {
            tracing::info!("loading screen: fading out after {t:.1} s");
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
