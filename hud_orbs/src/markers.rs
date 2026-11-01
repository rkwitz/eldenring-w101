//! Wizard101's quest markers over the Unicorn Way quest givers (round 15: Elden Ring characters talked to with the
//! game's "Talk" prompt, tools/msb_add_npcs.py), and the Wizard101 names over every street NPC. Wizard101's yellow
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
/// (slug, entity, MSB position): round 15, the street characters of tools/msb_add_npcs.py (Elden Ring NPCs with
/// the "Talk" prompt, entities 1042360820-824) in place of the invisible giver assets (1042361870-874).
const GIVERS: [(&str, u32, [f32; 3]); 5] = [
    ("private_connelly", 1042360820, [5.0029, 94.1828, -44.0778]),
    ("ceren_nightchant", 1042360821, [-84.8379, 94.4878, -18.4484]),
    ("olivia_dawnwillow", 1042360822, [-65.8655, 92.8248, -7.9762]),
    ("lady_oriel", 1042360823, [-335.1056, 93.4988, 103.5473]),
    ("private_oryan", 1042360824, [-160.8270, 93.4878, -54.0237]),
];

/// Wizard101 names over the street characters (msb_add_npcs.py; the givers at the same spots as GIVERS):
/// (name, MSB position, height of the plate's bottom over the character's feet, metres: the Elden Ring stand-in's
/// head + about 0.4; c0000 humans 1.8 m, Melina 1.7). They have no patrol route and stand still, so the HUD draws the
/// plates at their placed spots. Round 16: only the NPCs a new wizard gets quests from (Diego, Ugo Kalahad and Aegon
/// Statz were taken off the street, w101_quests.REMOVED).
const NPC_PLATES: [(&str, [f32; 3], f32); 5] = [
    ("Private Connelly", [5.0029, 94.1828, -44.0778], 2.2),
    ("Ceren Nightchant", [-84.8379, 94.4878, -18.4484], 2.2),
    ("Olivia Dawnwillow", [-65.8655, 92.8248, -7.9762], 2.2),
    ("Lady Oriel", [-335.1056, 93.4988, 103.5473], 2.1),
    ("Private O'Ryan", [-160.8270, 93.4878, -54.0237], 2.2),
];
/// NPC plates show this far away (metres), fading over the last part, as the creature plates.
const PLATE_RANGE: f32 = 40.0;

/// Quests given by talking to their giver: (giver, assigned flag, the flag that makes it available).
const OFFERS: [(&str, u32, u32); 3] = [
    ("olivia_dawnwillow", 1042362100, 1042362020), // Pesky Pirates once Skeleton Crew is assigned
    ("private_connelly", 1042362110, 1042362019), // Saving Private Oryan once Ghost Hunters is complete
    ("olivia_dawnwillow", 1042362150, 1042362109), // Fear of Fire once Pesky Pirates is complete
];

/// Talk goals: (npc, quest assigned flag, quest complete flag, goal flag, flags of the goals it comes after).
const TALKS: &[(&str, u32, u32, u32, &[u32])] = &[
    ("private_connelly", 1042362000, 1042362009, 1042362001, &[]),
    ("private_connelly", 1042362010, 1042362019, 1042362012, &[1042362011]),
    ("ceren_nightchant", 1042362010, 1042362019, 1042362013, &[1042362012]),
    ("ceren_nightchant", 1042362020, 1042362029, 1042362022, &[1042362021]),
    ("lady_oriel", 1042362030, 1042362039, 1042362032, &[1042362031]),
    ("lady_oriel", 1042362040, 1042362049, 1042362044, &[1042362043]),
    ("ceren_nightchant", 1042362050, 1042362059, 1042362051, &[]),
    ("ceren_nightchant", 1042362060, 1042362069, 1042362064, &[1042362063]),
    ("olivia_dawnwillow", 1042362100, 1042362109, 1042362102, &[1042362101]),
    ("private_oryan", 1042362110, 1042362119, 1042362112, &[1042362111]),
    ("private_oryan", 1042362110, 1042362119, 1042362115, &[1042362114]),
    ("private_connelly", 1042362110, 1042362119, 1042362116, &[1042362115]),
    ("olivia_dawnwillow", 1042362150, 1042362159, 1042362153, &[1042362152]),
];

/// Markers are drawn this far (metres) and fade out over the last part.
const RANGE: f32 = 150.0;
/// Height of the marker's centre above the giver's feet (over the name plate), and its size, in metres.
const LIFT: f32 = 3.1;
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
pub(crate) fn tile(block: i32) -> Option<[f32; 2]> {
    let b = block as u32;
    if b >> 24 != 60 {
        return None;
    }
    let unit = 256.0 * (1u32 << (b & 0xFF).min(3)) as f32;
    Some([(b >> 16 & 0xFF) as f32 * unit, (b >> 8 & 0xFF) as f32 * unit])
}

/// Offset of MSB point `local` (tile TILE) from the player in the tile grid, from Game::position's chunk position
/// and block origin; None outside the open world.
fn offset(local: [f32; 3], chunk: [f32; 3], origin: i32) -> Option<[f32; 3]> {
    let origin = tile(origin)?;
    Some([
        local[0] + TILE[0] as f32 * 256.0 - (chunk[0] + origin[0]),
        local[1] - chunk[1],
        local[2] + TILE[1] as f32 * 256.0 - (chunk[2] + origin[1]),
    ])
}

/// The Wizard101 name plates of the street NPCs in view (book.rs draws them with the creature plates).
pub fn npc_plates(camera: &Camera, position: ([f32; 3], i32, i32), screen: [f32; 2]) -> Vec<crate::nameplates::Plate> {
    let (chunk, _, origin) = position;
    let mut out = Vec::new();
    for (name, local, lift) in NPC_PLATES {
        let Some(delta) = offset(local, chunk, origin) else { return out };
        let d = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        if d > PLATE_RANGE {
            continue;
        }
        let p = camera.player;
        let Some((at, z)) = camera.project([p[0] + delta[0], p[1] + delta[1] + lift, p[2] + delta[2]], screen) else {
            continue;
        };
        if at[0] < -200.0 || at[0] > screen[0] + 200.0 || at[1] < -100.0 || at[1] > screen[1] + 100.0 {
            continue;
        }
        let alpha = ((PLATE_RANGE - d) / (PLATE_RANGE * 0.2)).clamp(0.0, 1.0);
        let k = (screen[1] / 768.0) * (8.0 / z.max(1.0)).clamp(0.7, 1.15);
        out.push(crate::nameplates::Plate { at, k, alpha, name, line: String::new(), school: "" });
    }
    out
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
    let flag = |f: u32| flags.get(&f) == Some(&true);
    let bob = (time * 2.2).sin() * 0.08;
    let mut drawn: Vec<(f32, [f32; 2], f32, u8, &str)> = Vec::new();
    for (slug, _, local) in GIVERS {
        let Some(kind) = marker(slug, &flag) else { continue };
        // The giver's offset from the player in the tile grid, added to the player's physics position.
        let Some(delta) = offset(local, chunk, origin) else { return };
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
