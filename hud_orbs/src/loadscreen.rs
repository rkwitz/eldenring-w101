//! A placeholder Wizard101 loading screen for the mod's doors.
//!
//! The doors in the Wizard101 areas are instant EMEVD warps (no Elden Ring loading screen). Each door event turns on
//! event flag `DOOR_FLAG` right before its warp; the overlay shows Wizard City's loading screen
//! (GUI/LoadScreen_W01_WC_01) with Wizard101's loading bar filling for `SHOW` seconds, fades it out, and turns the
//! flag back off.

use std::time::Instant;

use hudhook::imgui::{ImColor32, Ui};

use crate::layout::Textures;

/// Set by the door events (m60_42_36_00.emevd) just before a door warp; cleared here.
pub const DOOR_FLAG: u32 = 1042366900;
/// The door events: flag on, 0.75 s, fade-out and warp, fade-in about 1 s after the flag. The screen holds
/// fully up this long from the flag, covering all of it.
const SHOW: f32 = 1.8;
const FADE: f32 = 0.35;
/// The longest the screen waits for the game to come back from a warp (its own fade or loading) before fading out.
const MAX_WAIT: f32 = 15.0;

pub const IMAGE: &str = "GUI/LoadScreen_W01_WC_01.png";
pub const BAR: &str = "GUI/Art/Art_Progress_Bar_Large.png";
pub const FILL: &str = "GUI/Bar/Bar_Exp01.png";

#[derive(Default)]
pub struct LoadScreen {
    shown_at: Option<Instant>,
    /// When the fade-out began: after SHOW seconds, once the game has finished its warp.
    fading_at: Option<Instant>,
}

impl LoadScreen {
    /// Whether the screen is up and still holding (the door flag is left on until then).
    pub fn holding(&self) -> bool {
        self.shown_at.is_some() && self.fading_at.is_none()
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
        // Wizard101's night-sky purple behind the (square) picture, which fills the screen's height.
        draw.add_rect([0.0, 0.0], [w, h], a(ImColor32::from_rgb(18, 12, 40))).filled(true).build();
        if let Some((image, [iw, ih])) = textures.get(IMAGE) {
            let size = [h * iw / ih, h];
            let x = (w - size[0]) / 2.0;
            draw.add_image(image, [x, 0.0], [x + size[0], size[1]]).col(a(ImColor32::WHITE)).build();
        }
        // The loading bar along the bottom: its frame, then the fill over the frame's empty middle.
        if let Some((bar, [bw, bh])) = textures.get(BAR) {
            let width = w * 0.6;
            let height = width * bh / bw;
            let x = (w - width) / 2.0;
            let y = h - height * 2.2;
            draw.add_image(bar, [x, y], [x + width, y + height]).col(a(ImColor32::WHITE)).build();
            let fill = (t / SHOW).min(1.0);
            if let Some((f, _)) = textures.get(FILL) {
                let inset = [width * 16.0 / bw, height * 7.0 / bh];
                let x0 = x + inset[0];
                let x1 = x0 + (width - inset[0] * 2.0) * fill;
                draw.add_image(f, [x0, y + inset[1]], [x1, y + height - inset[1]])
                    .uv_max([fill, 1.0])
                    .col(a(ImColor32::WHITE))
                    .build();
            }
        }
    }
}
