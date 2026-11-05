//! Wizard101's nameplates over the Unicorn Way enemies, as the game shows them over a creature in the world
//! (/mnt/project-files/w101_reference/nameplate_lost_soul.png): no plate, the name in big outlined capitals in the
//! creature's Wizard101 name colour (yellow for the Lost Soul), and under it the school's icon and "RANK n" (book.rs
//! draws them). The street NPCs' plates (markers.rs) are drawn the same way in Wizard101's NPC green.
//!
//! The enemies are Elden Ring characters re-dressed by the mod; they are told apart by their model (ChrIns
//! character_id, c4180 = 4180) and read from WorldChrMan's characters-by-distance list (game.rs). The Lost Soul's
//! rank 1 is from the reference screenshot; Wizard101 keeps creature ranks on its servers (the client's Unicorn
//! Way data has none), so the other ranks are unverified.

use crate::markers::Camera;

#[derive(Clone, Copy, PartialEq)]
pub enum Title {
    Normal,
    Elite,
    Boss,
}

/// (model, name, rank, school ("" = none shown), title, the creature's height in metres when the game's hit capsule
/// can't be read, the plate's lift in metres when the capsule is the wrong measure of what is seen).
///
/// The capsule (NpcParam hitHeight, read live as CSChrPhysicsModule hit_height) fits the Lost Soul (3.0) and the
/// Skeletal Pirate (1.9): capsule + GAP puts the plate about 0.35 m over what is seen (tools/er_chr_height.py: the
/// pirate's idle head bone 1.91 m, mesh top 2.16; the Lost Soul's hood 2.98, mesh top 3.19). It does not fit the
/// other two, so they use their seen height + 0.35 instead:
/// - Dark Sprite (c3450 Winged Misbegotten): capsule 2.0-2.2, but it crouches: idle head bone 1.03 m, highest bone
///   (wing tip L_Wing04) 1.37 -> seen top 1.45, plate 1.8.
/// - Rattlebones (c4960 giant skeleton torso): capsule 1.0 (the hit box is the crawling body), but the torso rears
///   up: head bone 4.61 m, eyelids 5.33 in the model's pose -> skull top about 5.6, plate 5.95.
pub const CREATURES: [(u32, &str, u32, &str, Title, f32, Option<f32>, [u8; 3]); 7] = [
    (4180, "Lost Soul", 1, "Death", Title::Normal, 3.0, None, EASY),
    (3500, "Skeletal Pirate", 1, "Death", Title::Normal, 1.9, None, NORMAL),
    (3450, "Dark Sprite", 1, "Life", Title::Elite, 2.2, Some(1.8), ELITE),
    (4960, "Rattlebones", 3, "Death", Title::Boss, 1.0, Some(5.95), BOSS),
    (3300, "Lady Blackhope", 3, "Life", Title::Boss, 2.8, None, BOSS),
    (4340, "Fear of Fire", 3, "Fire", Title::Boss, 3.2, None, BOSS),
    (2120, "Seraph", 2, "Life", Title::Normal, 2.8, None, NORMAL),
];

/// Wizard101's name colours (round 18 G2): NPCBehaviorTemplate m_nameColor in the creatures' ObjectData (Root.wad,
/// bytes B G R A), one per difficulty (m_mobTitle): Easy 239,228,17 (Ghost-Blue-L01 Lost Soul; the reference
/// screenshot's "LOST SOUL" measures 223,221,36), Normal 255,165,0 (Skeleton-Pirate-L01 Skeletal Pirate), Elite
/// 255,0,0, Boss 153,0,255 (Skeleton-Silver-WCBoss-L01 Rattlebones, Ghost-Green-WCBoss-L02 Lady Blackhope). Friendly
/// NPCs are 0,255,0 (markers::NPC_NAME_COLOUR). Dark Sprite, Fear of Fire and Seraph have no Unicorn Way template of
/// that name; they take the colour of the title the mod gives them.
const EASY: [u8; 3] = [239, 228, 17];
const NORMAL: [u8; 3] = [255, 165, 0];
const ELITE: [u8; 3] = [255, 0, 0];
const BOSS: [u8; 3] = [153, 0, 255];

/// Plates show this far away (metres), fading over the last part.
const RANGE: f32 = 40.0;
/// How far above the top of the creature (its hit capsule) the plate's bottom floats, metres: Wizard101 floats the
/// name clearly over the head.
const GAP: f32 = 0.55;

/// Round 28 (W1, Ryan: "Lost Souls overlap each other"): every 10 s in play, how close the loaded Lost Souls stand
/// to each other (each one's nearest other Lost Soul), so a log shows whether the evenly spaced starts
/// (tools/msb_more_mobs.even_spacing: 9-12.7 m apart along each loop) hold while they walk.
pub fn log_spacing(enemies: &[(u32, [f32; 3], f32, Option<[f32; 3]>)]) {
    use std::sync::Mutex;
    use std::time::Instant;
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| t.elapsed().as_secs() < 10) {
        return;
    }
    let souls: Vec<[f32; 3]> = enemies.iter().filter(|e| e.0 == 4180).map(|e| e.1).collect();
    if souls.len() < 2 {
        return;
    }
    *last = Some(Instant::now());
    let mut nearest: Vec<f32> = souls.iter().enumerate().map(|(i, a)| {
        souls.iter().enumerate().filter(|(j, _)| *j != i)
            .map(|(_, b)| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt())
            .fold(f32::MAX, f32::min)
    }).collect();
    nearest.sort_by(f32::total_cmp);
    let close = nearest.iter().filter(|d| **d < 3.0).count();
    tracing::info!("lost souls: {} loaded, nearest other Lost Soul min/median/max {:.1}/{:.1}/{:.1} m, {} within 3 m \
                    (they start 9-12.7 m apart on each loop)", souls.len(), nearest[0], nearest[nearest.len() / 2],
                   nearest[nearest.len() - 1], close);
}

pub fn models() -> Vec<u32> {
    CREATURES.iter().map(|c| c.0).collect()
}

/// One nameplate to draw: screen position of its bottom centre, its size factor (pixels per unit), its
/// opacity, and the creature.
pub struct Plate {
    pub at: [f32; 2],
    pub k: f32,
    pub alpha: f32,
    pub name: &'static str,
    pub line: String,
    pub school: &'static str,
    /// The name's (and rank line's) colour, sRGB.
    pub colour: [u8; 3],
    /// Round 26 (O1): its depth from the camera (metres), for the scene depth test (occlusion.rs).
    pub z: f32,
}

/// How far the plate's bottom floats over the top of the model's posed box (round 26 O3), metres: as the seen
/// heights + 0.35 above.
const BOX_GAP: f32 = 0.35;

/// Round 29 (N1): a posed box top higher than this many times the creature's known height is not its mesh.
const BOX_CAP: f32 = 2.5;

fn log_box_rejected(model: u32, over: f32, known: f32) {
    use std::sync::Mutex;
    static SEEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if !seen.contains(&model) {
        seen.push(model);
        tracing::info!("tags: c{model} model box top {over:.2} m over its feet is more than {BOX_CAP} x its height \
                        {known:.2} m: not its mesh, plate from the capsule / fixed height");
    }
}

/// Round 30 (floaters.rs): the point (physics space) the plate of creature `model` at `p` (capsule height, posed box
/// top as for plates) hangs its bottom on.
/// Round 31 (FT6, Ryan: "start closer to the head, from each character's real height"): the top of creature `model`
/// (arguments as `anchor`), where its floating numbers start: the posed box top when it is the mesh's (BOX_CAP),
/// else its seen top (CREATURES' plate lift less BOX_GAP), else the hit capsule's height, else its known height.
pub fn head(model: u32, p: [f32; 3], height: f32, top: Option<[f32; 3]>) -> Option<[f32; 3]> {
    let &(_, _, _, _, _, tall, seen, _) = CREATURES.iter().find(|c| c.0 == model)?;
    let known = seen.unwrap_or(tall);
    if let Some(t) = top.filter(|t| t[1] - p[1] <= BOX_CAP * known) {
        return Some(t);
    }
    let h = seen.map(|s| s - BOX_GAP).unwrap_or(if height > 0.0 { height } else { tall });
    Some([p[0], p[1] + h, p[2]])
}

pub fn anchor(model: u32, p: [f32; 3], height: f32, top: Option<[f32; 3]>) -> Option<[f32; 3]> {
    let &(_, _, _, _, _, tall, seen, _) = CREATURES.iter().find(|c| c.0 == model)?;
    let lift = seen.unwrap_or(if height > 0.0 { height } else { tall } + GAP);
    // Round 29 (N1, Ryan: Skeleton Pirates' names way too high): the round 29 log reads c3500's posed box top
    // 8.36-8.38 m over its physics position, for a 2.16 m mesh (er_chr_height.py), so that box is not the posed
    // mesh. A box top more than BOX_CAP x the creature's known height over its feet is not used (Rattlebones
    // 10.3 m of 5.95, Fear of Fire 7.5 of 3.2 and Blackhope 2.56 of 2.8 stay under it).
    let known = seen.unwrap_or(tall);
    let top = top.filter(|t| {
        let ok = t[1] - p[1] <= BOX_CAP * known;
        if !ok {
            log_box_rejected(model, t[1] - p[1], known);
        }
        ok
    });
    Some(match top {
        Some(t) => [t[0], t[1] + BOX_GAP, t[2]],
        None => [p[0], p[1] + lift, p[2]],
    })
}

/// The plates for the enemies (model, physics position, hit capsule height or 0, top of the posed model box) in view,
/// farthest first (nearer ones over them).
///
/// Round 26 (O3, Ryan: Rattlebones' boss tag should stay on his head as he moves): the plate floats over the top of
/// the box the game keeps round the model as it is posed this frame (Game::model_top), not a fixed height over
/// the physics position (which, for the Giant Skeleton Torso, is the crawling body, not the rearing skull); the
/// fixed heights stay for when the box can't be read.
pub fn plates(camera: &Camera, enemies: &[(u32, [f32; 3], f32, Option<[f32; 3]>)], screen: [f32; 2]) -> Vec<Plate> {
    let mut out: Vec<(f32, Plate)> = Vec::new();
    for &(model, p, height, top) in enemies {
        let Some(&(_, name, rank, school, title, _, _, colour)) = CREATURES.iter().find(|c| c.0 == model) else { continue };
        let Some(anchor) = anchor(model, p, height, top) else { continue };
        let d = ((p[0] - camera.player[0]).powi(2) + (p[1] - camera.player[1]).powi(2)
            + (p[2] - camera.player[2]).powi(2)).sqrt();
        if d > RANGE {
            continue;
        }
        let Some((at, z)) = camera.project(anchor, screen) else { continue };
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
        out.push((z, Plate { at, k, alpha, name, line: format!("Rank {rank}{what}"), school, colour, z }));
    }
    out.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.into_iter().map(|p| p.1).collect()
}
