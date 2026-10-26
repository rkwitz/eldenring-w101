//! Wizard101's nameplates over the Unicorn Way enemies: the torn-paper plate (GUI/Nameplates/NamePlate_128x64_Mob,
//! orange; _Elite, purple, for elites and bosses) with the creature's Wizard101 name over its rank and school.
//!
//! The enemies are Elden Ring characters re-dressed by the mod; they are told apart by their model (ChrIns
//! character_id, c4180 = 4180) and read from WorldChrMan's characters-by-distance list (game.rs). Names, ranks and
//! schools are Wizard101's (WizardMobs / NPCBehavior templates of the Unicorn Way creatures); where the client data
//! gave none (Fear of Fire, the Seraph, the bosses' normal versions) they are the closest Wizard101 equivalents.

use hudhook::imgui::{DrawListMut, ImColor32};

use crate::markers::Camera;

pub const MOB: &str = "GUI/Nameplates/NamePlate_128x64_Mob.png";
pub const ELITE: &str = "GUI/Nameplates/NamePlate_128x64_Elite.png";

#[derive(Clone, Copy, PartialEq)]
pub enum Title {
    Normal,
    Elite,
    Boss,
}

/// (model, name, rank, school ("" = none shown), title, height of the plate above the feet in metres).
pub const CREATURES: [(u32, &str, u32, &str, Title, f32); 7] = [
    (4180, "Lost Soul", 2, "Death", Title::Normal, 2.3),
    (3500, "Skeletal Pirate", 1, "Death", Title::Normal, 2.3),
    (3450, "Dark Sprite", 1, "Life", Title::Elite, 1.9),
    (4960, "Rattlebones", 3, "Death", Title::Boss, 3.6),
    (3300, "Lady Blackhope", 3, "Life", Title::Boss, 2.8),
    (4340, "Fear of Fire", 3, "Fire", Title::Boss, 3.2),
    (2120, "Seraph", 2, "Life", Title::Normal, 2.8),
];

/// Plates show this far away (metres), fading over the last part.
const RANGE: f32 = 40.0;

pub fn models() -> Vec<u32> {
    CREATURES.iter().map(|c| c.0).collect()
}

/// One plate to draw: screen position of its bottom centre, its size factor (pixels per plate unit), its
/// opacity, and the creature.
pub struct Plate {
    pub at: [f32; 2],
    pub k: f32,
    pub alpha: f32,
    pub name: &'static str,
    pub line: String,
    pub school: &'static str,
    pub art: &'static str,
}

/// The plates for the enemies (model, physics position) in view, farthest first (nearer ones over them).
pub fn plates(camera: &Camera, enemies: &[(u32, [f32; 3])], screen: [f32; 2]) -> Vec<Plate> {
    let mut out: Vec<(f32, Plate)> = Vec::new();
    for &(model, p) in enemies {
        let Some(&(_, name, rank, school, title, lift)) = CREATURES.iter().find(|c| c.0 == model) else { continue };
        let d = ((p[0] - camera.player[0]).powi(2) + (p[1] - camera.player[1]).powi(2)
            + (p[2] - camera.player[2]).powi(2)).sqrt();
        if d > RANGE {
            continue;
        }
        let Some((at, z)) = camera.project([p[0], p[1] + lift, p[2]], screen) else { continue };
        if at[0] < -200.0 || at[0] > screen[0] + 200.0 || at[1] < -100.0 || at[1] > screen[1] + 100.0 {
            continue;
        }
        let alpha = ((RANGE - d) / (RANGE * 0.2)).clamp(0.0, 1.0);
        // Wizard101's plates keep a readable size: a little smaller with distance, never tiny.
        let k = (screen[1] / 768.0) * (8.0 / z.max(1.0)).clamp(0.7, 1.15);
        let what = match title {
            Title::Normal => String::new(),
            Title::Elite => " Elite".into(),
            Title::Boss => " Boss".into(),
        };
        let art = if title == Title::Normal { MOB } else { ELITE };
        out.push((z, Plate { at, k, alpha, name, line: format!("Rank {rank}{what}"), school, art }));
    }
    out.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.into_iter().map(|p| p.1).collect()
}

/// A rounded backing when the plate's art hasn't loaded.
pub fn fallback(draw: &DrawListMut, r: [f32; 4], alpha: f32) {
    let c = ImColor32::from_rgba(214, 150, 110, (230.0 * alpha) as u8);
    draw.add_rect([r[0], r[1]], [r[2], r[3]], c).filled(true).rounding(4.0).build();
}
