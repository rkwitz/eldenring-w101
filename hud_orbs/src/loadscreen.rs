//! A placeholder Wizard101 loading screen for the mod's doors.
//!
//! The doors in the Wizard101 areas are instant EMEVD warps (no Elden Ring loading screen), so going through one
//! is a sudden jump of the player's position. When the player moves more than `JUMP` metres in one frame while
//! Elden Ring's HUD has been up for a while (so not a fast travel or a respawn, which have Elden Ring's own loading
//! screen), this shows Wizard City's loading screen (GUI/LoadScreen_W01_WC_01) with Wizard101's loading bar filling
//! for `SHOW` seconds, then fades it out.

use std::time::Instant;

use hudhook::imgui::{ImColor32, Ui};

use crate::layout::Textures;

/// Metres in one frame that count as a warp (running is about 0.2 m per frame, Torrent about 0.5).
const JUMP: f32 = 30.0;
/// Seconds the HUD must have been up before a jump counts (Elden Ring's own loads hide it).
const SETTLED: f32 = 1.0;
const SHOW: f32 = 1.0;
const FADE: f32 = 0.35;

pub const IMAGE: &str = "GUI/LoadScreen_W01_WC_01.png";
pub const BAR: &str = "GUI/Art/Art_Progress_Bar_Large.png";
pub const FILL: &str = "GUI/Bar/Bar_Exp01.png";

#[derive(Default)]
pub struct LoadScreen {
    last: Option<([f32; 3], i32)>,
    hud_since: Option<Instant>,
    shown_at: Option<Instant>,
}

impl LoadScreen {
    /// Follows the player's position (`hud`: Elden Ring's HUD is up and not faded).
    pub fn update(&mut self, position: Option<([f32; 3], i32)>, hud: bool, enabled: bool) {
        if !hud {
            self.hud_since = None;
        } else if self.hud_since.is_none() {
            self.hud_since = Some(Instant::now());
        }
        if let (Some((from, _)), Some((at, _))) = (self.last, position) {
            let d = ((at[0] - from[0]).powi(2) + (at[1] - from[1]).powi(2) + (at[2] - from[2]).powi(2)).sqrt();
            let settled = self.hud_since.is_some_and(|t| t.elapsed().as_secs_f32() > SETTLED);
            if d > JUMP && settled && enabled {
                tracing::info!("door warp: moved {d:.0} m in one frame, showing the loading screen");
                self.shown_at = Some(Instant::now());
            }
        }
        if position.is_some() {
            self.last = position;
        }
    }

    pub fn showing(&self) -> bool {
        self.shown_at.is_some_and(|t| t.elapsed().as_secs_f32() < SHOW + FADE)
    }

    pub fn draw(&self, ui: &Ui, textures: &Textures) {
        let Some(at) = self.shown_at else { return };
        let t = at.elapsed().as_secs_f32();
        if t >= SHOW + FADE {
            return;
        }
        let alpha = if t < SHOW { 1.0 } else { 1.0 - (t - SHOW) / FADE };
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
