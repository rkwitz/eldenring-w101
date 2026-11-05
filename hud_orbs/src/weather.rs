//! Round 33 (G1, Ryan's ref 1: small cream-yellow leaves drifting in the sky left of the tower, gone as he walks up).
//! Not ours and not the map: the W101 street mesh has no tree card away from its tree (every one of its 10,007
//! tree/fern/hedge triangles has 7 neighbours within 6 m), no ER part is left inside the sky dome (dome_clear), the
//! dome is ordinary opaque geometry (PartsDrawParam 845, not the skydome phase) so nothing beyond it can show, and
//! the leaf gust of common event 1040 is already off (--no-leaf-gusts). What is left is Elden Ring's weather wind
//! effect: every WeatherParam row has WindSfxId 808006 (144 of 162 rows; 808004 / 808007 the rest), the overworld's
//! wind-blown leaves that drift round the camera and fade out. While the player is on Unicorn Way's tile
//! (questhelper::player_msb_now) the DLL sets every row's WindSfxId to -1 (none), and puts them back when he leaves.
//! Log: "weather: wind leaves off ...".

use std::collections::HashMap;

/// WEATHER_PARAM_ST: sfxId (0x00), windSfxId (0x04).
const WIND_SFX: usize = 0x04;

#[derive(Default)]
pub struct Weather {
    saved: HashMap<u32, i32>,
    off: bool,
    /// Last time the street position was known (the leaves come back only 10 s off the street).
    seen: Option<std::time::Instant>,
}

impl Weather {
    pub fn frame(&mut self, on_street: bool) {
        if on_street {
            self.seen = Some(std::time::Instant::now());
        }
        let on_street = self.seen.is_some_and(|t| t.elapsed().as_secs() < 10);
        if on_street == self.off {
            return;
        }
        let rows = crate::game::param_rows(b"WEATHER_PARAM_ST");
        if rows.is_empty() {
            return;
        }
        for (id, at) in &rows {
            let field = (at + WIND_SFX) as *mut i32;
            unsafe {
                if on_street {
                    let old = field.read_volatile();
                    if old != -1 {
                        self.saved.insert(*id, old);
                    }
                    field.write_volatile(-1);
                } else if let Some(old) = self.saved.get(id) {
                    field.write_volatile(*old);
                }
            }
        }
        tracing::info!("weather: wind leaves {} ({} WeatherParam rows, {} had a WindSfxId)",
                       if on_street { "off on Unicorn Way" } else { "back on off the street" }, rows.len(), self.saved.len());
        self.off = on_street;
    }
}
