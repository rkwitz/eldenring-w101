//! Wizard101's quest markers over the Unicorn Way quest givers. The givers are invisible assets (AEG099_990_9000-9004,
//! entities 1042361870-874) whose "Examine" prompt only shows within 1 m, so the HUD marks them: Wizard101's yellow
//! "!" (GUI/QuestButtons/Quest_Start) while they have a quest to offer, its "?" medallion (Art_Quest_Complete) while a
//! current goal is to talk to them.
//!
//! The positions are the MSB's (m60_42_36_00, tile-local metres). The player's place in the tile grid comes from
//! ChrIns's chunk position and block origin; the camera works in the physics space, so a giver is drawn at the
//! player's physics position plus the giver's offset from the player in the tile grid. The flags are those of
//! tools/w101_quests.py (quest_flags.json).

use std::collections::HashMap;

use hudhook::imgui::{DrawListMut, ImColor32};

use crate::layout::Textures;

pub const OFFER: &str = "GUI/QuestButtons/Quest_Start.png";
pub const TALK: &str = "GUI/QuestButtons/Art_Quest_Complete.png";

/// The tile the givers are in (m60_42_36_00).
const TILE: [i32; 2] = [42, 36];
/// (slug, entity, MSB position).
const GIVERS: [(&str, u32, [f32; 3]); 5] = [
    ("private_connelly", 1042361870, [5.0029, 93.9828, -44.0778]),
    ("ceren_nightchant", 1042361871, [-84.8379, 94.2878, -18.4484]),
    ("olivia_dawnwillow", 1042361872, [-65.8655, 92.6248, -7.9762]),
    ("lady_oriel", 1042361873, [-335.1056, 93.2988, 103.5473]),
    ("private_oryan", 1042361874, [-160.8270, 93.2878, -54.0237]),
];

/// Quests given by talking to their giver: (giver, assigned flag, the flag that makes it available).
const OFFERS: [(&str, u32, u32); 3] = [
    ("olivia_dawnwillow", 1042366100, 1042366020), // Pesky Pirates once Skeleton Crew is assigned
    ("private_connelly", 1042366110, 1042366019), // Saving Private Oryan once Ghost Hunters is complete
    ("olivia_dawnwillow", 1042366150, 1042366109), // Fear of Fire once Pesky Pirates is complete
];

/// Talk goals: (npc, quest assigned flag, quest complete flag, goal flag, flags of the goals it comes after).
const TALKS: &[(&str, u32, u32, u32, &[u32])] = &[
    ("private_connelly", 1042366000, 1042366009, 1042366001, &[]),
    ("private_connelly", 1042366010, 1042366019, 1042366012, &[1042366011]),
    ("ceren_nightchant", 1042366010, 1042366019, 1042366013, &[1042366012]),
    ("ceren_nightchant", 1042366020, 1042366029, 1042366022, &[1042366021]),
    ("lady_oriel", 1042366030, 1042366039, 1042366032, &[1042366031]),
    ("lady_oriel", 1042366040, 1042366049, 1042366044, &[1042366043]),
    ("ceren_nightchant", 1042366050, 1042366059, 1042366051, &[]),
    ("ceren_nightchant", 1042366060, 1042366069, 1042366064, &[1042366063]),
    ("olivia_dawnwillow", 1042366100, 1042366109, 1042366102, &[1042366101]),
    ("private_oryan", 1042366110, 1042366119, 1042366112, &[1042366111]),
    ("private_oryan", 1042366110, 1042366119, 1042366115, &[1042366114]),
    ("private_connelly", 1042366110, 1042366119, 1042366116, &[1042366115]),
    ("olivia_dawnwillow", 1042366150, 1042366159, 1042366153, &[1042366152]),
];

/// Markers are drawn this far (metres) and fade out over the last part.
const RANGE: f32 = 150.0;
/// Height of the marker's centre above the giver's spot, and its size, in metres.
const LIFT: f32 = 2.2;
const SIZE: f32 = 0.9;

/// The game camera: rows right, up, forward, position; vertical field of view in radians; and the player's physics
/// position (the camera's space).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub matrix: [[f32; 4]; 4],
    pub fov: f32,
    pub player: [f32; 3],
}

impl Camera {
    /// Screen position (pixels of a `size` screen) and depth (metres) of world point `p`; None behind the camera.
    pub fn project(&self, p: [f32; 3], size: [f32; 2]) -> Option<([f32; 2], f32)> {
        let m = &self.matrix;
        let d = [p[0] - m[3][0], p[1] - m[3][1], p[2] - m[3][2]];
        let dot = |r: [f32; 4]| d[0] * r[0] + d[1] * r[1] + d[2] * r[2];
        let (x, y, z) = (dot(m[0]), dot(m[1]), dot(m[2]));
        if !(z > 0.3) {
            return None;
        }
        let t = (self.fov / 2.0).tan();
        let aspect = size[0] / size[1];
        let sx = (1.0 + x / (z * t * aspect)) / 2.0 * size[0];
        let sy = (1.0 - y / (z * t)) / 2.0 * size[1];
        (sx.is_finite() && sy.is_finite()).then_some(([sx, sy], z))
    }

    /// Pixels per metre at depth `z` on a screen `height` pixels high.
    fn scale(&self, z: f32, height: f32) -> f32 {
        height / 2.0 / (z * (self.fov / 2.0).tan())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Marker {
    Offer,
    Talk,
}

/// Every flag the markers read.
pub fn flags() -> Vec<u32> {
    let mut out: Vec<u32> = OFFERS.iter().flat_map(|o| [o.1, o.2]).collect();
    for (_, assigned, complete, goal, after) in TALKS.iter().copied() {
        out.extend([assigned, complete, goal]);
        out.extend(after.iter().copied());
    }
    out.retain(|f| *f != 0);
    out
}

/// The marker over `npc`, from the quest flags: "!" when one of its quests is available and not yet assigned, else
/// "?" when a goal to talk to it is current (its quest in progress, the goal not done, the goals before it done).
pub fn marker(npc: &str, flag: &dyn Fn(u32) -> bool) -> Option<Marker> {
    if OFFERS.iter().any(|(giver, assigned, after)| *giver == npc && flag(*after) && !flag(*assigned)) {
        return Some(Marker::Offer);
    }
    let talk = TALKS.iter().any(|(who, assigned, complete, goal, after)| {
        *who == npc && flag(*assigned) && !flag(*complete) && !flag(*goal) && after.iter().all(|a| flag(*a))
    });
    talk.then_some(Marker::Talk)
}

/// A map block's tile (x, z) in 256 m units and its scale, or None outside the open world (area 60).
fn tile(block: i32) -> Option<[f32; 2]> {
    let b = block as u32;
    if b >> 24 != 60 {
        return None;
    }
    let unit = 256.0 * (1u32 << (b & 0xFF).min(3)) as f32;
    Some([(b >> 16 & 0xFF) as f32 * unit, (b >> 8 & 0xFF) as f32 * unit])
}

/// Draws the markers. `position` is Game::position (chunk position, block, block origin), `time` seconds for the bob.
pub fn draw(
    draw: &DrawListMut,
    textures: &Textures,
    camera: &Camera,
    position: ([f32; 3], i32, i32),
    flags: &HashMap<u32, bool>,
    time: f32,
    screen: [f32; 2],
) {
    let (chunk, _, origin) = position;
    let Some(origin) = tile(origin) else { return };
    let flag = |f: u32| flags.get(&f) == Some(&true);
    let bob = (time * 2.2).sin() * 0.08;
    let mut drawn: Vec<(f32, [f32; 2], f32, u8, &str)> = Vec::new();
    for (slug, _, local) in GIVERS {
        let Some(kind) = marker(slug, &flag) else { continue };
        // The giver's offset from the player in the tile grid, added to the player's physics position.
        let delta = [
            local[0] + TILE[0] as f32 * 256.0 - (chunk[0] + origin[0]),
            local[1] - chunk[1],
            local[2] + TILE[1] as f32 * 256.0 - (chunk[2] + origin[1]),
        ];
        let distance = (delta[0] * delta[0] + delta[2] * delta[2]).sqrt();
        let alpha = (255.0 * ((RANGE - distance) / (RANGE * 0.2)).clamp(0.0, 1.0)) as u8;
        if alpha == 0 {
            continue;
        }
        let p = camera.player;
        let world = [p[0] + delta[0], p[1] + delta[1] + LIFT + bob, p[2] + delta[2]];
        let Some((at, z)) = camera.project(world, screen) else { continue };
        // Never smaller than a readable icon far away, nor huge up close.
        let px = (SIZE * camera.scale(z, screen[1])).clamp(screen[1] * 0.03, screen[1] * 0.11);
        drawn.push((z, at, px, alpha, if kind == Marker::Offer { OFFER } else { TALK }));
    }
    // Far ones first, so near ones cover them.
    drawn.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, at, px, alpha, name) in drawn {
        let Some((texture, _)) = textures.get(name) else { continue };
        // Quest_Start's "!" fills about the middle third of its square, the medallion all of its own.
        let side = if name == OFFER { px * 1.6 } else { px * 1.2 };
        let min = [at[0] - side / 2.0, at[1] - side / 2.0];
        let max = [min[0] + side, min[1] + side];
        draw.add_image(texture, min, max).col(ImColor32::from_rgba(255, 255, 255, alpha)).build();
    }
}
