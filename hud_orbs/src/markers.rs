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
///
/// Round 24 (N1, Ryan's picks; tools/w101_quests.py BERNAHL ...): Connelly = Knight Bernahl, Ceren = Brother Corhyn,
/// O'Ryan = Castellan Edgar (all c0000 in their own looks), Olivia = Finger Reader Enia (c2170), Lady Oriel = St.
/// Trina (c5330). Heights measured the round 18 way (plate = seen top + 0.21; marker = 1.53 x head bone, and at least
/// plate + 0.48, the c0000 gap, so the sign never sits on the name):
/// - Bernahl / Corhyn / Edgar (npc_portrait renders in the book's idle pose): hair / blindfold top 1.69 / 1.69 / 1.72
///   m, head bone about 1.55 -> plates 1.9 / 1.9 / 1.93, markers 2.38 / 2.38 / 2.41.
/// - Enia (tools/er_chr_height.py, idle a000_000020): head bone 1.96, hood top 2.19 (bind mesh top 2.14 over bind
///   head 1.91) -> plate 2.4, marker 3.0.
/// - Trina: a small girl asleep on her side inside tall flowers: head bone 1.22, but the flowers reach 1.97 (mesh top
///   and highest bone Flower_B103) -> plate 2.18 over the flowers, marker 2.66 (plate + 0.48).
/// Round 25 (M3, Ryan: "look at each one in its real pose: Enia's tag too low; Corhyn kneels so his tag goes lower").
/// The c0000 NPCs don't stand: EMEVD plays 90100, which their NpcParam SpEffects (9845 / 9861 / 9828, animIdOffset
/// 345 / 361 / 328 million) turn into a345_090100 (Bernahl sits, head bone 1.23 m), a361_090100 (Corhyn kneels, head
/// 0.87) and a328_090100 (Edgar sits, head 1.04) (c0000_a9x.anibnd, first = last frame; round 24 measured the book's
/// standing pose). Tops = head bone + the standing hair / blindfold gap (0.14 / 0.14 / 0.17). Enia (a000_000020, sat
/// in her chair) holds her staff with cloth ribbons up to 3.10 m ([cloth]c2170_wand_07; head 1.96), so the round 24
/// plate (2.4) sat inside it. Trina now rises from her flowers (emevd idle a000_030000: head 1.22, flowers 1.96):
/// unchanged. Same rule: plate = top + 0.21, marker = max(1.53 x head bone, plate + 0.48).
/// Round 25 (M7): Connelly and O'Ryan moved to their Wizard101 places (tools/w101_quests.CHARACTERS).
pub const NPCS: [(&str, &str, u32, u32, [f32; 3], f32, f32); 5] = [
    ("private_connelly", "Private Connelly", 1042360820, 0, [-29.7745, 94.5018, -61.8978], 1.58, 2.06),
    ("ceren_nightchant", "Ceren Nightchant", 1042360821, 0, [-84.8379, 94.4878, -18.4484], 1.22, 1.70),
    ("olivia_dawnwillow", "Olivia Dawnwillow", 1042360822, 2170, [-65.8655, 92.8248, -7.9762], 3.31, 3.79),
    ("lady_oriel", "Lady Oriel", 1042360823, 5330, [-336.1025, 93.4988, 105.5051], 2.18, 2.66),
    ("private_oryan", "Private O'Ryan", 1042360824, 0, [-4.7818, 94.1998, -63.9439], 1.42, 1.90),
];
/// Their models and entity IDs, for Game::entities.
pub const MODELS: [u32; 3] = [0, 2170, 5330];
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
    // Round 19 (E3): Unicorn Way is Merle Ambrose's, offered by his message anywhere on the street (emevd_quests.py)
    // and again on reaching Connelly after a "no"; until it is accepted Connelly shows the "!" so the player can
    // find him. QUEST_RESET is on in every save once the round 19 reset has run.
    // Round 20 (Q2): no Ambrose offer; the "!" is how the line starts, and talking to Connelly takes Unicorn Way.
    ("private_connelly", 1042360300, QUEST_RESET),
    ("private_connelly", 1042360310, 1042360309), // Ghost Hunters once Unicorn Way is complete
    ("ceren_nightchant", 1042360320, 1042360319), // Skeleton Crew once Ghost Hunters is complete
    ("ceren_nightchant", 1042360330, 1042360329), // Monsters and Mazes once Skeleton Crew is complete
    ("lady_oriel", 1042360340, 1042360339), // Finding Out Why once Monsters and Mazes is complete
    ("lady_oriel", 1042360350, 1042360349), // Who Could It Be Now? once Finding Out Why is complete
    ("ceren_nightchant", 1042360360, 1042360359), // Sinister Skeleton once Who Could It Be Now? is complete
    // Round 23 (Q3, Ryan): only the line to Rattlebones and Ceren after him; the side quests (Pesky Pirates, Saving
    // Private O'Ryan, The Power is Ours) are removed (w101_quests.REMOVED_QUESTS).
];

/// w101_quests.QUEST_RESET: the one-shot quest reset has run (the quest events wait for it). Round 21 (Q1): version 3
/// in the saved 1042360xxx block (round 20's 1042362691 sat in a block every map load clears, so it re-ran each load).
pub const QUEST_RESET: u32 = 1042360691;

/// Talk goals: (npc, quest assigned flag, quest complete flag, goal flag, flags of the goals it comes after).
const TALKS: &[(&str, u32, u32, u32, &[u32])] = &[
    ("private_connelly", 1042360300, 1042360309, 1042360301, &[]),
    ("private_connelly", 1042360310, 1042360319, 1042360312, &[1042360311]),
    ("ceren_nightchant", 1042360310, 1042360319, 1042360313, &[1042360312]),
    ("ceren_nightchant", 1042360320, 1042360329, 1042360322, &[1042360321]),
    ("lady_oriel", 1042360330, 1042360339, 1042360332, &[1042360331]),
    ("lady_oriel", 1042360340, 1042360349, 1042360344, &[1042360343]),
    ("ceren_nightchant", 1042360350, 1042360359, 1042360351, &[]),
    ("ceren_nightchant", 1042360360, 1042360369, 1042360364, &[1042360363]),
];

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
    let mut out: Vec<u32> = OFFERS.iter().flat_map(|o| [o.1, o.1 + 9, o.2]).collect();
    for (_, assigned, complete, goal, after) in TALKS.iter().copied() {
        out.extend([assigned, complete, goal]);
        out.extend(after.iter().copied());
    }
    out.retain(|f| *f != 0);
    out
}

/// The marker over `npc`, from the quest flags: "!" when one of its quests is available and not yet assigned, else
/// "?" when a goal to talk to it is current (its quest in progress, the goal not done, the goals before it done).
/// Round 23 (Q1/Q2): in the order the NPC's talk event checks them (emevd_quests.npc_event): Unicorn Way's greet,
/// then a current talk goal ("?"), then an offer, which needs the quest neither assigned nor complete (+9).
pub fn marker(npc: &str, flag: &dyn Fn(u32) -> bool) -> Option<Marker> {
    let offer = |(giver, assigned, after): &(&str, u32, u32)| {
        *giver == npc && flag(*after) && !flag(*assigned) && !flag(*assigned + 9)
    };
    if OFFERS.first().is_some_and(offer) {
        return Some(Marker::Offer); // greet: Connelly takes Unicorn Way
    }
    let talk = TALKS.iter().any(|(who, assigned, complete, goal, after)| {
        *who == npc && flag(*assigned) && !flag(*complete) && !flag(*goal) && after.iter().all(|a| flag(*a))
    });
    if talk {
        return Some(Marker::Talk);
    }
    OFFERS.iter().any(offer).then_some(Marker::Offer)
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
             render: &[(u32, u8, u8, u32)], flag: &dyn Fn(u32) -> bool) {
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
        // Round 19 (E2): height over the player (fallen under the street?) and the game's render bits.
        let bits = render.iter().find(|r| r.0 == NPCS[i].2);
        let shown = bits
            .map(|b| format!(" render group {} render {} activation {:#x}", b.1 >> 3 & 1, b.2 >> 3 & 1, b.3))
            .unwrap_or_default();
        key += &format!("{slug}:{loaded}:{kind:?}:{}:{}:{shown} ", (d / 50.0) as i32, ((at[1] - p[1]) / 5.0) as i32);
        line += &format!(" {slug} {} marker {kind:?} {d:.0} m, {:+.1} m up{shown}{};",
                         if loaded { "loaded" } else { "at its MSB spot" }, at[1] - p[1],
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
pub fn npc_plates(camera: &Camera, spots: &[Spot], sight: &Sight, screen: [f32; 2]) -> Vec<crate::nameplates::Plate> {
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
        let alpha = ((PLATE_RANGE - d) / (PLATE_RANGE * 0.2)).clamp(0.0, 1.0) * sight.alpha(i);
        if alpha <= 0.0 {
            continue;
        }
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
    render: &[(u32, u8, u8, u32)],
    flags: &HashMap<u32, bool>,
    sight: &Sight,
    time: f32,
    screen: [f32; 2],
) {
    let flag = |f: u32| flags.get(&f) == Some(&true);
    log_state(camera, position, entities, spots, render, &flag);
    let p = camera.player;
    let mut drawn: Vec<(f32, [f32; 2], f32, u8, &str)> = Vec::new();
    for &(i, at, _) in spots {
        let (slug, _, _, _, _, _, lift) = NPCS[i];
        let Some(kind) = marker(slug, &flag) else { continue };
        // Round 22 (H6, Ryan: "the name fades away at a certain distance; make those symbols also fade"): faded with
        // distance exactly as the names are (PLATE_RANGE, 3D distance), and out of sight behind walls (`sight`).
        let d3 = ((at[0] - p[0]).powi(2) + (at[1] - p[1]).powi(2) + (at[2] - p[2]).powi(2)).sqrt();
        let fade = ((PLATE_RANGE - d3) / (PLATE_RANGE * 0.2)).clamp(0.0, 1.0) * sight.alpha(i);
        let alpha = (255.0 * fade) as u8;
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

/// Round 22 (H6): whether each street NPC can be seen from the player (MSB positions; questhelper::Nav::sees), eased
/// in and out over FADE seconds so a marker doesn't blink at a corner. In a room under the street nobody on the
/// street is in sight; the garden (hedge maze) and the street don't see each other.
///
/// Round 23 (H2): a name is hidden only after it has been out of sight for HIDE_AFTER seconds (a glimpse of a corner
/// no longer blinks every name), and the player position a world shift moved for a frame can't hide them all.
#[derive(Default)]
pub struct Sight {
    fade: [f32; NPCS.len()],
    unseen: [f32; NPCS.len()],
    time: Option<f32>,
}

const FADE: f32 = 0.25;
const HIDE_AFTER: f32 = 0.3;

impl Sight {
    pub fn update(&mut self, nav: &crate::questhelper::Nav, player: Option<[f32; 3]>, time: f32) {
        use crate::questhelper::{in_garden, in_room};
        let dt = self.time.map_or(1.0, |t| (time - t).clamp(0.0, 1.0));
        self.time = Some(time);
        if dt <= 0.0 {
            return; // already updated this frame (markers and names both call it)
        }
        for (i, npc) in NPCS.iter().enumerate() {
            let spot = npc.4;
            let seen = match player {
                None => true,
                Some(p) if in_room(p) => false,
                Some(p) if in_garden(p) != in_garden(spot) => false,
                Some(p) if in_garden(p) => true,
                // Round 24 (H1): through the street's geometry (Nav::sees_3d), head height to name height.
                Some(p) => nav.sees_3d([p[0], p[1] + 1.6, p[2]], [spot[0], spot[1] + 2.0, spot[2]]),
            };
            self.unseen[i] = if seen { 0.0 } else { self.unseen[i] + dt };
            let shown = self.unseen[i] < HIDE_AFTER;
            let step = dt / FADE;
            self.fade[i] = if shown { (self.fade[i] + step).min(1.0) } else { (self.fade[i] - step).max(0.0) };
        }
    }

    pub fn alpha(&self, i: usize) -> f32 {
        self.fade.get(i).copied().unwrap_or(1.0)
    }
}

/// Round 22 (H5): the street NPCs' positions (Game::entities) held steady: a position that jumps, relative to the
/// player, further in one read than the player can move (a read from a moment the game was mid-update) is refused and
/// the last good one kept (moved with the player), unless the jump lasts HOLD seconds. Logged once per refusal.
pub fn steady(camera: &Camera, entities: &[(u32, [f32; 3])]) -> Vec<(u32, [f32; 3])> {
    use std::time::Instant;
    const HOLD: f32 = 0.35;
    // id -> (offset from the player, when taken, refused since)
    static LAST: std::sync::Mutex<Vec<(u32, [f32; 3], Instant, Option<Instant>)>> = std::sync::Mutex::new(Vec::new());
    // The player's own jumps already passed the camera's check (Game::tick_camera): one this big is a warp, after
    // which every offset is new.
    static PLAYER: std::sync::Mutex<Option<[f32; 3]>> = std::sync::Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let p = camera.player;
    let now = Instant::now();
    {
        let mut player = PLAYER.lock().unwrap_or_else(|e| e.into_inner());
        if player.is_some_and(|q| (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2) > 20.0 * 20.0) {
            last.clear();
        }
        *player = Some(p);
    }
    entities.iter().map(|&(id, at)| {
        let offset = [at[0] - p[0], at[1] - p[1], at[2] - p[2]];
        if !offset.iter().all(|v| v.is_finite()) {
            return (id, at);
        }
        let Some(k) = last.iter().position(|l| l.0 == id) else {
            last.push((id, offset, now, None));
            return (id, at);
        };
        let (_, old, when, since) = last[k];
        let dt = now.duration_since(when).as_secs_f32().min(1.0);
        let jump = ((offset[0] - old[0]).powi(2) + (offset[1] - old[1]).powi(2) + (offset[2] - old[2]).powi(2)).sqrt();
        if jump > 2.5 + 30.0 * dt && since.is_none_or(|s| s.elapsed().as_secs_f32() < HOLD) {
            if since.is_none() {
                tracing::info!("markers: refused a position of {id} (moved {jump:.1} m against the player in {:.0} ms)",
                               dt * 1000.0);
                last[k].3 = Some(now);
            }
            return (id, [p[0] + old[0], p[1] + old[1], p[2] + old[2]]);
        }
        last[k] = (id, offset, now, None);
        (id, at)
    }).collect()
}
