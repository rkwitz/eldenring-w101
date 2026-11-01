//! Wizard101's quest markers over the Unicorn Way quest givers (round 15: Elden Ring characters talked to with the
//! game's "Talk" prompt, tools/msb_add_npcs.py), and the Wizard101 names over every street NPC.
//!
//! Round 17 (F1): the markers are Wizard101's own wizbangs (WizBangs.xml: StartQuest = Wizbang/Quest_Marker_Start.nif,
//! the yellow "!", while they have a quest to offer; CompleteQuestGoal = Quest_Marker_Complete.nif, the yellow "?",
//! while a current goal is to talk to them, which includes handing a quest in). Each nif is a camera-facing quad
//! 76.2 units (0.762 m) square centred 38.71 units over the marker's origin, with the red Quest_Spiral (59 units)
//! turning behind it once every 6.67 s; the sign does not bob or pulse. Wizard101 hangs it on the character's
//! ChatNode (Human_WCGuard_A.nif: 2.90 m, 1.01 m over the head bone). The textures come from tools/w101_quest_art.py.
//!
//! Round 18 (G1): the NPCs are found among the loaded characters by their MSB entity ID and the marker and name hang
//! over where the game has them (the camera's physics space). Only a character the game hasn't loaded falls back to
//! its MSB spot (m60_42_36_00, tile-local metres), placed by the player's chunk position and block origin. Each
//! change of state (loaded or not, marker, distance band, MSB error) is logged as "markers:". The flags are those of
//! tools/w101_quests.py (quest_flags.json).

use std::collections::HashMap;

use hudhook::imgui::{DrawListMut, ImColor32};

use crate::layout::Textures;

pub const OFFER: &str = "Wizbang/Quest_Start.png";
pub const TALK: &str = "Wizbang/Quest_Complete.png";
pub const SPIRAL: &str = "Wizbang/Quest_Spiral.png";

/// The tile the givers are in (m60_42_36_00).
const TILE: [i32; 2] = [42, 36];

/// The street NPCs (tools/w101_quests.py CHARACTERS, placed by msb_add_npcs.py: Elden Ring characters with the
/// "Talk" prompt): (slug, Wizard101 name (WC-NPCs.lang, as their ObjectData WC-ST01-NPC0n names them), MSB entity
/// ID, model (ChrIns character_id), MSB position (tile-local metres), the name plate's bottom over the feet, the
/// marker's origin over the feet).
///
/// Round 18 (G1): they are found by their entity ID among the loaded characters (Game::entities) and the plate and
/// marker hang over where the game has them, as the creature plates do. Only a character the game hasn't loaded falls
/// back to its MSB spot, moved by the offset seen between the MSB spot and the game's position of the ones that are
/// loaded (round 17 placed everything from the MSB spots alone, through the player's chunk/block-origin offset).
///
/// Plate: just over the top of the Elden Ring stand-in's head, as Wizard101 floats a name (round 18: the head tops
/// measured in npc_portrait FULL renders of the posed characters: Connelly's helm 1.69 m, Melina's hood 1.72; round
/// 17's 2.2 left half a metre of air). Marker: Wizard101's ChatNode, where the sign's bottom sits, scaled to the
/// stand-in by the Wizard City guard's proportion (ChatNode 290 units over the feet / head bone 189.2 = 1.53, times
/// the stand-in's head bone: c0000 about 1.55 m, Melina 1.5 m), which leaves the sign just over the name. Round 17
/// used the Seraph's proportion (285.9 / 152.4) for Melina, a winged creature twice her size.
pub const NPCS: [(&str, &str, u32, u32, [f32; 3], f32, f32); 5] = [
    ("private_connelly", "Private Connelly", 1042360820, 0, [5.0029, 94.1828, -44.0778], 1.9, 2.38),
    ("ceren_nightchant", "Ceren Nightchant", 1042360821, 0, [-84.8379, 94.4878, -18.4484], 1.9, 2.38),
    ("olivia_dawnwillow", "Olivia Dawnwillow", 1042360822, 0, [-65.8655, 92.8248, -7.9762], 1.85, 2.3),
    ("lady_oriel", "Lady Oriel", 1042360823, 2180, [-335.1056, 93.4988, 103.5473], 1.9, 2.3),
    ("private_oryan", "Private O'Ryan", 1042360824, 0, [-160.8270, 93.4878, -54.0237], 1.9, 2.38),
];
/// Their models and entity IDs, for Game::entities.
pub const MODELS: [u32; 2] = [0, 2180];
pub fn entity_ids() -> Vec<u32> {
    NPCS.iter().map(|n| n.2).collect()
}

/// Wizard101's colour for a friendly NPC's name: NPCBehaviorTemplate m_nameColor of every Unicorn Way NPC's
/// ObjectData (WC-ST01-NPC01..09: bytes 00 FF 00 FF, B G R A), pure green. Wizard101 colours creature names the same
/// way (nameplates.rs).
pub const NPC_NAME_COLOUR: [u8; 3] = [0, 255, 0];

/// NPC plates show this far away (metres), fading over the last part, as the creature plates.
const PLATE_RANGE: f32 = 40.0;

/// Round 18 (G3): each street NPC's dialogue portrait is its Elden Ring stand-in's face on Wizard101's blue portrait
/// background and gold ring (made offline by tools/w101_npc_portraits.py with the book's FLVER renderer; on Ryan's
/// PC as art/GUI/NpcPortraits/ER/<slug>.png, never in the repo). A conversation whose speaker has one shows it in
/// place of the Wizard101 picture; Gamma (the intro) keeps Wizard101's owl.
pub const ER_PORTRAITS: &str = "GUI/NpcPortraits/ER";

/// Points the conversations of dialogue.tsv at the Elden Ring portraits that are installed under `book`/art.
pub fn er_portraits(requests: &mut [crate::talk::Request], book: &std::path::Path) {
    for r in requests.iter_mut() {
        let picture = format!("{ER_PORTRAITS}/{}.png", r.speaker);
        if book.join("art").join(&picture).exists() {
            r.portrait = picture;
        }
    }
}

/// Quests given by talking to their giver: (giver, assigned flag, the flag that makes it available).
/// Round 18 (quests F2): the main quests are offered too (Wizard101's flow: each one handed out by its giver on
/// yes), so a giver whose quest was declined shows the "!" again (w101_quests.QUESTS "assign").
const OFFERS: &[(&str, u32, u32)] = &[
    ("private_connelly", 1042362010, 1042362009), // Ghost Hunters once Unicorn Way is complete
    ("ceren_nightchant", 1042362020, 1042362019), // Skeleton Crew once Ghost Hunters is complete
    ("ceren_nightchant", 1042362030, 1042362029), // Monsters and Mazes once Skeleton Crew is complete
    ("lady_oriel", 1042362040, 1042362039), // Finding Out Why once Monsters and Mazes is complete
    ("lady_oriel", 1042362050, 1042362049), // Who Could It Be Now? once Finding Out Why is complete
    ("ceren_nightchant", 1042362060, 1042362059), // Sinister Skeleton once Who Could It Be Now? is complete
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
/// Quest_Marker_*.nif: the billboard's height over the marker's origin, the sign's and the spiral's quad sides (W101
/// units = cm), and the spiral's turn (its z rotation keys: -2 pi over 6.67 s).
const CENTRE: f32 = 0.3871;
const SIGN: f32 = 0.762;
const SPIRAL_SIDE: f32 = 0.59;
const SPIRAL_TURN: f32 = 6.6667;

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

/// One street NPC's place: index into NPCS, feet in the camera's physics space, and whether the game has the
/// character loaded (else the place is its MSB spot).
pub type Spot = (usize, [f32; 3], bool);

/// Where each street NPC stands: the game's own position of the character when it is loaded (`entities` =
/// Game::entities), else its MSB spot (`position` = Game::position) moved by the mean offset seen between MSB spot
/// and game position of the loaded ones.
pub fn locate(camera: &Camera, position: Option<([f32; 3], i32, i32)>, entities: &[(u32, [f32; 3])]) -> Vec<Spot> {
    let p = camera.player;
    let msb = |i: usize| {
        let (chunk, _, origin) = position?;
        let d = offset(NPCS[i].4, chunk, origin)?;
        Some([p[0] + d[0], p[1] + d[1], p[2] + d[2]])
    };
    let found = |i: usize| entities.iter().find(|e| e.0 == NPCS[i].2).map(|e| e.1);
    let (mut shift, mut n) = ([0.0f32; 3], 0.0f32);
    for i in 0..NPCS.len() {
        if let (Some(f), Some(m)) = (found(i), msb(i)) {
            shift = [shift[0] + f[0] - m[0], shift[1] + f[1] - m[1], shift[2] + f[2] - m[2]];
            n += 1.0;
        }
    }
    let shift = if n > 0.0 { shift.map(|v| v / n) } else { [0.0; 3] };
    (0..NPCS.len())
        .filter_map(|i| match found(i) {
            Some(f) => Some((i, f, true)),
            None => msb(i).map(|m| (i, [m[0] + shift[0], m[1] + shift[1], m[2] + shift[2]], false)),
        })
        .collect()
}

/// Logs the street NPCs' state when it changes (round 18 G1: Ryan saw no marker and the log could not say why): per
/// NPC whether the game has it loaded, the marker it gets, its distance (in 50 m steps for the change test), and how
/// far its MSB spot is from where the game has it.
fn log_state(camera: &Camera, position: Option<([f32; 3], i32, i32)>, entities: &[(u32, [f32; 3])], spots: &[Spot],
             flag: &dyn Fn(u32) -> bool) {
    static LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    let p = camera.player;
    let (mut key, mut line) = (String::new(), String::new());
    for &(i, at, loaded) in spots {
        let slug = NPCS[i].0;
        let kind = marker(slug, flag);
        let d = ((at[0] - p[0]).powi(2) + (at[2] - p[2]).powi(2)).sqrt();
        let err = position.and_then(|(chunk, _, origin)| {
            let o = offset(NPCS[i].4, chunk, origin)?;
            let f = entities.iter().find(|e| e.0 == NPCS[i].2)?.1;
            Some(((p[0] + o[0] - f[0]).powi(2) + (p[1] + o[1] - f[1]).powi(2) + (p[2] + o[2] - f[2]).powi(2)).sqrt())
        });
        key += &format!("{slug}:{loaded}:{kind:?}:{} ", (d / 50.0) as i32);
        line += &format!(" {slug} {} marker {kind:?} {d:.0} m{};", if loaded { "loaded" } else { "at its MSB spot" },
                         err.map(|e| format!(" (MSB spot {e:.1} m off)")).unwrap_or_default());
    }
    if spots.is_empty() {
        key = format!("none {position:?}");
        line = format!(" none placed (position {position:?}, {} loaded)", entities.len());
    }
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if *last != key {
        tracing::info!("markers:{line}");
        *last = key;
    }
}

/// The Wizard101 name plates of the street NPCs in view (book.rs draws them with the creature plates): the name in
/// Wizard101's NPC green, no second line (none of these NPCs has a title in Wizard101's data).
pub fn npc_plates(camera: &Camera, spots: &[Spot], screen: [f32; 2]) -> Vec<crate::nameplates::Plate> {
    let mut out = Vec::new();
    let p = camera.player;
    for &(i, at, _) in spots {
        let (_, name, _, _, _, lift, _) = NPCS[i];
        let d = ((at[0] - p[0]).powi(2) + (at[1] - p[1]).powi(2) + (at[2] - p[2]).powi(2)).sqrt();
        if d > PLATE_RANGE {
            continue;
        }
        let Some((on_screen, z)) = camera.project([at[0], at[1] + lift, at[2]], screen) else { continue };
        let [x, y] = on_screen;
        if x < -200.0 || x > screen[0] + 200.0 || y < -100.0 || y > screen[1] + 100.0 {
            continue;
        }
        let alpha = ((PLATE_RANGE - d) / (PLATE_RANGE * 0.2)).clamp(0.0, 1.0);
        let k = (screen[1] / 768.0) * (8.0 / z.max(1.0)).clamp(0.7, 1.15);
        out.push(crate::nameplates::Plate { at: on_screen, k, alpha, name, line: String::new(), school: "",
                                             colour: NPC_NAME_COLOUR });
    }
    out
}

/// Draws the markers over the NPCs at `spots` (locate), `position` / `entities` as for locate (for the log), `time`
/// seconds for the spiral's turn.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    draw: &DrawListMut,
    textures: &Textures,
    camera: &Camera,
    position: Option<([f32; 3], i32, i32)>,
    entities: &[(u32, [f32; 3])],
    spots: &[Spot],
    flags: &HashMap<u32, bool>,
    time: f32,
    screen: [f32; 2],
) {
    let flag = |f: u32| flags.get(&f) == Some(&true);
    log_state(camera, position, entities, spots, &flag);
    let p = camera.player;
    let mut drawn: Vec<(f32, [f32; 2], f32, u8, &str)> = Vec::new();
    for &(i, at, _) in spots {
        let (slug, _, _, _, _, _, lift) = NPCS[i];
        let Some(kind) = marker(slug, &flag) else { continue };
        let distance = ((at[0] - p[0]).powi(2) + (at[2] - p[2]).powi(2)).sqrt();
        let alpha = (255.0 * ((RANGE - distance) / (RANGE * 0.2)).clamp(0.0, 1.0)) as u8;
        if alpha == 0 {
            continue;
        }
        let Some((on_screen, z)) = camera.project([at[0], at[1] + lift + CENTRE, at[2]], screen) else { continue };
        // Wizard101 draws it at its world size; far away it is kept to a still readable 1.5 % of the screen.
        let metre = camera.scale(z, screen[1]).max(screen[1] * 0.015 / SIGN);
        drawn.push((z, on_screen, metre, alpha, if kind == Marker::Offer { OFFER } else { TALK }));
    }
    // Far ones first, so near ones cover them.
    drawn.sort_by(|a, b| b.0.total_cmp(&a.0));
    // The spiral turns clockwise on the screen, a full turn every SPIRAL_TURN seconds.
    let angle = (time / SPIRAL_TURN).fract() * std::f32::consts::TAU;
    for (_, at, metre, alpha, name) in drawn {
        let tint = ImColor32::from_rgba(255, 255, 255, alpha);
        if let Some((spiral, _)) = textures.get(SPIRAL) {
            let h = SPIRAL_SIDE * metre / 2.0;
            let (sin, cos) = angle.sin_cos();
            let corner = |x: f32, y: f32| [at[0] + (x * cos - y * sin) * h, at[1] + (x * sin + y * cos) * h];
            draw.add_image_quad(spiral, corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0))
                .col(tint)
                .build();
        }
        let Some((texture, _)) = textures.get(name) else { continue };
        let h = SIGN * metre / 2.0;
        draw.add_image(texture, [at[0] - h, at[1] - h], [at[0] + h, at[1] + h]).col(tint).build();
    }
}
