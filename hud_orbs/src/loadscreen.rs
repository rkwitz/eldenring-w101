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
const SHOW: f32 = 1.0;
const FADE: f32 = 0.35;

pub const IMAGE: &str = "GUI/LoadScreen_W01_WC_01.png";
pub const BAR: &str = "GUI/Art/Art_Progress_Bar_Large.png";
pub const FILL: &str = "GUI/Bar/Bar_Exp01.png";

#[derive(Default)]
pub struct LoadScreen {
    shown_at: Option<Instant>,
}

impl LoadScreen {
    /// Shows the loading screen from now.
    pub fn show(&mut self) {
        tracing::info!("door warp: showing the loading screen");
        self.shown_at = Some(Instant::now());
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
