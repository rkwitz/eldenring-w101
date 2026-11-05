//! Round 30 (FA1, Ryan: "the hedge maze fairies, flying round the maze like the wisps"; ref 6_hedge_maze_fairies.png):
//! Wizard101's own hedge maze sprites, drawn by the overlay in the maze only.
//!
//! What W101 has (WizardCity-WC_Streets-Interiors-WC_Unicorn_HedgeMaze.wad, round 29 research): spawnData.xml puts
//! template 82309 (Root.wad ObjectData/NPCSprite.xml: model Character/DarkSprite/MC_Spirit_Model.nif, animations
//! MC_Spirit_Anims.kfm, PathMovementBehavior PT_LOOP, no combat) on four paths of pathData.xml / pathNodeData.bin:
//! "Path 0" 2 alive, "Path 1" 3, "Path 2" 3, "Path" 6. Their nodes are moved into m60_42_36_00 MSB coordinates with
//! the garden frame AEG030_555_9000 (msb_add_quests.FRAMES["garden"]: W101 (x, y, z) -> local (x, z, y) / 100,
//! checked on Lady Oriel's spot, which lands on markers::NPCS exactly).
//!
//! The look: tools/blender_fairy_sheet.py renders MC_Spirit_Model.nif with its own two textures
//! (SC_Spirit_Body / SC_Spirit_Wing, the wing alpha-tested at 150 as its NiAlphaProperty), skinned and posed by
//! MC_Dark_Spirit_Walk.kf (the clip a path-moving NPC plays; 2.33 s; its MoveNode bob and wing flaps included)
//! into SHEET: VIEWS view angles round the fairy (row 0 = seen from the front, then every 360/VIEWS degrees with the
//! camera moving to the fairy's right) x FRAMES frames, 10 degrees above, orthographic, transparent. A fairy is a
//! camera-facing square of SIZE metres centred CENTRE_UP metres over its path point, showing the row nearest the
//! angle it is seen from (it faces where it flies) and the frame of its own clip time.
//!
//! Hidden behind the hedges by the game's own depth (occlusion.rs, as the wisps); without the depth test (ini
//! depth_occlusion off, or no depth yet) they are not drawn at all, so they never show through a hedge.
//! Speed: NPCSprite.xml carries no speed the client reads, so FAIRY_SPEED is our own (ini fairy_speed).

use std::time::Instant;

use hudhook::imgui::{ImColor32, Ui};

use crate::game::Game;
use crate::layout::Textures;
use crate::markers::Camera;

/// The sprite sheet (w101_orbs/fairies/, loaded through book/art like the wisps' art).
const SHEET: &str = "../../fairies/fairy_sheet.png";
const VIEWS: usize = 8;
const FRAMES: usize = 48;
/// Round 31 (FA1, Ryan: "a bit too big ... too slow ... they clip into the ground a lot ... resolution too low"):
/// the sheet is now 224 px cells (was 128) of the model pitched forward 50 degrees (blender_fairy_sheet.py PITCH, read
/// off ref 6). SCALE shrinks the W101-size model (our choice, from Ryan's "a bit too big"; W101's NPCSprite template
/// gives no scale the client reads). LIFT raises the whole fairy so its lowest point (0.30 m over the model root in
/// the sheet, x SCALE) flies at least CLEAR m over the path, and the depth test gets DEPTH_LEEWAY m of leeway (the
/// fairy's own half-depth), so ground just in front of it no longer cuts its legs off.
const SCALE: f32 = 0.8;
const CLEAR: f32 = 0.6;
const LIFT: f32 = CLEAR - 0.303_36 * SCALE;
const DEPTH_LEEWAY: f32 = 0.4;
/// MC_Dark_Spirit_Walk.kf's length (seconds).
const CLIP: f32 = 2.333_333_4;
/// The render's square (2 x its radius, W101 units / 100) and its centre's height over the model's root (metres).
const SIZE: f32 = 2.0 * 0.801_442_3 * SCALE;
const CENTRE_UP: f32 = 0.848_623_7 * SCALE + LIFT;
/// Metres per second along the path (not in W101's client data; ini fairy_speed).
const FAIRY_SPEED: f32 = 2.0;
/// Fairies farther than this from the player aren't drawn.
const DRAW_RANGE: f32 = 70.0;
/// Fade in over this long when the depth test comes back (seconds).
const SHOW: f32 = 0.3;

/// W101's hedge maze fairy paths (name, how many fly it, MSB nodes; each a loop).
const PATHS: [(&str, usize, &[[f32; 3]]); 4] = [
    ("Path 0", 2, &[[-326.36, 93.30, 103.75], [-328.53, 93.30, 97.85], [-327.15, 94.30, 90.50], [-323.78, 94.30, 85.35], [-320.01, 94.30, 84.36], [-314.81, 94.30, 86.55], [-312.66, 94.30, 90.23], [-307.65, 94.30, 92.90], [-302.96, 94.30, 92.47], [-300.47, 94.30, 96.65], [-305.28, 94.30, 105.53], [-309.98, 94.30, 112.65], [-316.29, 94.30, 113.76], [-321.60, 94.30, 110.93], [-324.48, 93.63, 108.12]]),
    ("Path 1", 3, &[[-345.37, 93.30, 107.46], [-344.96, 93.30, 112.27], [-345.90, 94.30, 120.05], [-347.81, 94.30, 125.09], [-352.03, 94.30, 130.18], [-357.95, 94.30, 125.92], [-367.17, 94.30, 121.81], [-370.81, 94.30, 117.28], [-368.82, 94.30, 110.46], [-364.98, 94.30, 104.54], [-363.18, 94.30, 98.83], [-357.95, 94.30, 96.57], [-353.35, 94.30, 98.04], [-347.95, 93.96, 102.11]]),
    ("Path 2", 3, &[[-325.00, 94.30, 115.78], [-329.38, 94.30, 119.98], [-334.65, 94.30, 121.23], [-339.50, 94.30, 119.61], [-342.60, 93.30, 113.48], [-343.49, 93.30, 105.97], [-346.16, 94.30, 97.39], [-345.02, 94.30, 94.06], [-342.05, 94.30, 91.02], [-335.13, 94.30, 90.15], [-331.75, 93.30, 98.45], [-330.67, 93.30, 107.45]]),
    ("Path", 6, &[[-317.94, 94.31, 85.76], [-320.21, 94.31, 82.19], [-328.14, 94.31, 87.41], [-328.94, 94.30, 78.45], [-328.86, 94.30, 74.16], [-333.78, 94.30, 78.22], [-338.00, 94.30, 77.44], [-344.26, 94.30, 73.99], [-344.67, 94.30, 70.64], [-349.19, 94.30, 71.68], [-355.78, 94.30, 84.55], [-362.39, 94.31, 84.76], [-357.61, 94.31, 86.91], [-365.90, 94.31, 87.69], [-367.89, 94.31, 93.97], [-365.92, 94.30, 99.94], [-358.40, 94.31, 95.99], [-354.52, 94.31, 96.72], [-356.58, 94.31, 91.89], [-350.05, 94.31, 97.68], [-344.43, 93.30, 101.96], [-338.22, 93.30, 106.17], [-339.48, 93.30, 112.65], [-330.43, 93.30, 111.52], [-341.54, 93.30, 107.29], [-339.90, 93.30, 96.88], [-337.64, 93.30, 101.90], [-332.12, 93.30, 96.55], [-327.36, 93.30, 106.02], [-332.54, 93.30, 104.54], [-335.46, 93.30, 115.35], [-342.23, 94.31, 117.14], [-343.73, 94.31, 127.35], [-346.26, 94.30, 133.97], [-341.41, 94.31, 134.54], [-328.48, 94.31, 141.34], [-324.76, 94.31, 140.69], [-321.19, 94.31, 133.46], [-316.31, 94.31, 125.75], [-311.03, 94.30, 126.88], [-313.33, 94.31, 122.79], [-306.75, 94.31, 125.55], [-303.34, 94.31, 117.39], [-304.67, 94.31, 112.10], [-312.06, 94.31, 113.50], [-319.00, 94.31, 115.16], [-327.82, 93.86, 113.80], [-330.99, 93.30, 103.11], [-336.06, 93.77, 94.15], [-328.32, 94.31, 92.11], [-330.25, 94.31, 88.19], [-327.03, 94.31, 83.03]]),
];

struct Fairy {
    path: usize,
    along: f32,
    speed: f32,
    seed: f32,
}

pub struct Fairies {
    fairies: Vec<Fairy>,
    lengths: Vec<(f32, Vec<f32>)>,
    enabled: bool,
    wanted: bool,
    seen: f32,
    /// The last "fairies:" log line (when, what).
    log: Option<(Instant, String)>,
}

impl Fairies {
    pub fn new(folder: &std::path::Path) -> Fairies {
        let lengths: Vec<(f32, Vec<f32>)> = PATHS.iter().map(|(_, _, p)| {
            let mut run = vec![0.0];
            for i in 0..p.len() {
                let (a, b) = (p[i], p[(i + 1) % p.len()]);
                run.push(run[i] + ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt());
            }
            (run[p.len()], run)
        }).collect();
        let (mut enabled, mut speed) = (true, FAIRY_SPEED);
        for line in std::fs::read_to_string(folder.join("w101_orbs.ini")).unwrap_or_default().lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            match key.trim() {
                "fairies" => enabled = value.trim() != "0",
                "fairy_speed" => speed = value.trim().parse::<f32>().map_or(speed, |v| v.clamp(0.1, 5.0)),
                _ => {}
            }
        }
        // Spread evenly round each loop (W101 respawns them on random nodes; even spacing reads the same).
        let mut fairies = Vec::new();
        let mut seed = 0.37_f32;
        for (path, (_, count, _)) in PATHS.iter().enumerate() {
            for k in 0..*count {
                seed = (seed * 97.13 + 0.618).fract();
                fairies.push(Fairy { path, along: lengths[path].0 * k as f32 / *count as f32,
                                     speed: speed * (0.85 + 0.3 * seed), seed: seed * CLIP });
            }
        }
        tracing::info!("fairies: {} ({} on Wizard101's {} hedge maze paths, {:.0} / {:.0} / {:.0} / {:.0} m, {speed} m/s)",
                       if enabled { "on" } else { "off" }, fairies.len(), PATHS.len(), lengths[0].0, lengths[1].0,
                       lengths[2].0, lengths[3].0);
        Fairies { fairies, lengths, enabled, wanted: false, seen: 0.0, log: None }
    }

    /// MSB point and travel direction `along` metres round path `path`.
    fn point(&self, path: usize, along: f32) -> ([f32; 3], [f32; 2]) {
        let (len, run) = &self.lengths[path];
        let p = PATHS[path].2;
        let d = along.rem_euclid(*len);
        let i = run.partition_point(|r| *r <= d).saturating_sub(1).min(p.len() - 1);
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        let t = ((d - run[i]) / (run[i + 1] - run[i]).max(1e-3)).clamp(0.0, 1.0);
        let dir = [b[0] - a[0], b[2] - a[2]];
        let n = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt().max(1e-3);
        ([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t], [dir[0] / n, dir[1] / n])
    }

    fn say(&mut self, now: Instant, what: String) {
        if self.log.as_ref().is_none_or(|(t, s)| *s != what && (now - *t).as_secs_f32() > 5.0) {
            tracing::info!("fairies: {what}");
            self.log = Some((now, what));
        }
    }

    /// Every frame (after the wisps): fly, and draw them while the player is in the hedge maze garden.
    /// `street` = the player's MSB position this frame (questhelper::player_msb_now).
    pub fn frame(&mut self, ui: &Ui, textures: &mut Textures, game: Option<&Game>, gameplay: bool,
                 street: Option<[f32; 3]>) {
        let (Some(g), true, Some(msb)) = (game, self.enabled, street) else { return };
        let now = Instant::now();
        let dt = ui.io().delta_time.clamp(0.0, 0.1);
        for f in &mut self.fairies {
            f.along += f.speed * dt;
        }
        if !crate::questhelper::in_garden(msb) {
            self.seen = 0.0;
            return;
        }
        if !self.wanted {
            self.wanted = true;
            textures.want([SHEET.to_string()]);
        }
        let Some(camera) = g.camera() else { return };
        if !crate::occlusion::active() {
            self.seen = 0.0;
            self.say(now, "in the maze, not drawn: the game's depth test is off (no way to hide them behind hedges)".into());
            return;
        }
        if textures.get(SHEET).is_none() {
            self.say(now, format!("in the maze, not drawn: {SHEET} not loaded (tools/blender_fairy_sheet.py)"));
            return;
        }
        self.seen = (self.seen + dt / SHOW).min(1.0);
        if !gameplay {
            return;
        }
        let player = camera.player;
        let to_world = |p: [f32; 3]| [p[0] - msb[0] + player[0], p[1] - msb[1] + player[1], p[2] - msb[2] + player[2]];
        let time = ui.time() as f32;
        let looks: Vec<([f32; 3], [f32; 2], f32)> = self.fairies.iter().map(|f| {
            let (p, dir) = self.point(f.path, f.along);
            (to_world(p), dir, time + f.seed)
        }).collect();
        let near = looks.iter().filter(|l| (l.0[0] - player[0]).hypot(l.0[2] - player[2]) < DRAW_RANGE).count();
        self.say(now, format!("{near} of {} within {DRAW_RANGE} m in the maze", self.fairies.len()));
        draw(ui, textures, &camera, &looks, self.seen);
    }
}

/// The sheet cell for a fairy at `at` flying along `dir` (world x, z) seen from `eye`, `clock` seconds into its clip:
/// (row, column).
fn cell(at: [f32; 3], dir: [f32; 2], eye: [f32; 3], clock: f32) -> (usize, usize) {
    let c = [eye[0] - at[0], eye[2] - at[2]];
    // Its right in Elden Ring's left-handed world (x right, y up, z forward): (dir.z, -dir.x).
    let right = [dir[1], -dir[0]];
    let yaw = (c[0] * right[0] + c[1] * right[1]).atan2(c[0] * dir[0] + c[1] * dir[1]);
    let step = std::f32::consts::TAU / VIEWS as f32;
    let row = (yaw / step).round().rem_euclid(VIEWS as f32) as usize % VIEWS;
    let col = ((clock / CLIP).rem_euclid(1.0) * FRAMES as f32) as usize % FRAMES;
    (row, col)
}

/// Draws fairies (world point, flight direction, clip clock) far to near, each behind the scene's depth.
fn draw(ui: &Ui, textures: &Textures, camera: &Camera, looks: &[([f32; 3], [f32; 2], f32)], alpha: f32) {
    let Some((sheet, _)) = textures.get(SHEET) else { return };
    let screen = ui.io().display_size;
    let eye = [camera.matrix[3][0], camera.matrix[3][1], camera.matrix[3][2]];
    let mut order: Vec<(f32, [f32; 2], usize)> = looks.iter().enumerate().filter_map(|(i, l)| {
        let centre = [l.0[0], l.0[1] + CENTRE_UP, l.0[2]];
        camera.project(centre, screen).map(|(p, z)| (z, p, i))
    }).filter(|(z, _, _)| *z < DRAW_RANGE).collect();
    order.sort_by(|a, b| b.0.total_cmp(&a.0));
    let list = ui.get_background_draw_list();
    let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
    for (z, p, i) in order {
        let (at, dir, clock) = looks[i];
        let (row, col) = cell(at, dir, eye, clock);
        let half = SIZE / 2.0 * screen[1] / (2.0 * z * (camera.fov / 2.0).tan());
        let (u, v) = (1.0 / FRAMES as f32, 1.0 / VIEWS as f32);
        crate::occlusion::begin(z, DEPTH_LEEWAY, "", [0.0, 0.0]);
        list.add_image(sheet, [p[0] - half, p[1] - half], [p[0] + half, p[1] + half])
            .uv_min([col as f32 * u, row as f32 * v]).uv_max([(col + 1) as f32 * u, (row + 1) as f32 * v])
            .col(ImColor32::from_rgba(255, 255, 255, a)).build();
    }
    crate::occlusion::end();
}

/// Preview (hud_orbs/preview "fairy:<t>"): fairies at these world spots and flight directions, `t` seconds in.
pub fn preview(ui: &Ui, textures: &mut Textures, camera: &Camera, looks: &[([f32; 3], [f32; 2], f32)]) {
    textures.want([SHEET.to_string()]);
    draw(ui, textures, camera, looks, 1.0);
}
