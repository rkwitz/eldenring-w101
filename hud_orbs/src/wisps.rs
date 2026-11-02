//! Wizard101's health and mana wisps on Unicorn Way (round 21 M3, round 22 S5).
//!
//! In Wizard101 the wisps on Unicorn Way are part of the zone, not of fights: its spawnData.xml has three groups
//! ("Spawn Wisps 01 (1)", "Spawn Wisps 01", "Spawn point Wisps 03"; respawn 10 s) that put WC_WispHealth (about
//! 80 %) and WC_WispMana_UW (about 20 %) on the nodes of "Path Wisps 01..03" (pathData.xml / pathNodeData.bin),
//! which run along the street's sidewalks; they drift along those paths. A wisp is a small glowing cloud with
//! sparkles over a coloured glow on the ground (_Shared-WorldData Character/WilOTheWisps/FX_Wisp_Red.nif /
//! FX_Wisp.nif + GlowShadow_Red/Blue.nif) that restores health / mana when walked into, with a burst and the sound
//! Sound/GUI/ui_health_powerup_01.wav.
//!
//! Here (round 22 S5, Ryan: "these shouldn't spawn when enemies die ... they just float around on the sidewalk in a
//! pathing"): the three W101 paths, moved into the street's MSB with the quest_nav.tsv fit (PATHS), each carry
//! wisp_count wisps (ini, default 5 since round 23) that float along the path loop; a picked-up one comes back on a random node of
//! its path after RESPAWN seconds. MSB spots become physics positions through the player's MSB position this frame
//! (lib.rs: questhelper::player_msb_now) and Camera::player. They are drawn from the wisp nif's own textures
//! (tools/w101_wisp_art.py -> w101_orbs/wisps/), projected with the markers' camera. Walking within PICKUP metres
//! restores wisp_restore percent (default 20) of the player's max HP (red) or FP (blue) (Game::add_player_stat) and
//! plays the W101 sound through the music thread's mixer. ini wisps = 0 turns them off.
//!
//! Round 23 (H4-H6, Ryan: "The wisps flying around look malnourished ... you can see them through walls ... When you
//! collect them, it should spin around you"):
//! - H4 fuller: FX_Wisp_Red.nif is six particle systems on one spot (cloud Main and Add, sparkles, focus sparks, the
//!   glow), so the cloud is five bigger puffs over a bright core glow (GLOW tinted), eight sparkles round it, a wider
//!   ground glow, and wisp_count defaults to 5 per path.
//! - H5: hidden behind walls like the NPC markers (questhelper::Nav::sees from the player's MSB spot to the wisp's,
//!   held HIDE_AFTER s before fading over FADE s); none from a room under the street or the hedge maze garden.
//!   Round 24 (H1, "STILL visible through walls"): that walk-graph test let 19 % of the blocked sight lines in Ryan's
//!   round 23 log through; now the line runs from the camera to the cloud through the street mesh's voxels
//!   (Nav::sees_3d, street_sight.bin from tools/w101_street_sight.py). Log "wisps: n of m in sight".
//! - H6: W101's pickup is ObjectData CombatBonus_WispHealth's WispDespawn effect Character/FX_WispRed_Dsppr.nif
//!   (FX_WispBlue_Dsppr for mana) played on the player, with Sound/GUI/ui_health_powerup_01.wav: its "Mover" node
//!   carries the wisp cloud (FX_Cloud_Leave) at chest height, moving 0.94 -> 1.04 -> 0.79 -> 0.99 -> 0.94 m over
//!   4 s (MOVER, its NiTransformData keys / 100), with "Wind02", two half-ring ribbons (17.6 and 11.6 units) round
//!   it, and yellow sparkles leaving (FX_SparkleYllwExit). Here the picked-up wisp spirals round the player for SPIN
//!   s (SPIN_TURNS turns, the radius closing from where it was to 0.3 m, its height on MOVER at twice the speed)
//!   trailing a white wind ribbon, then bursts into yellow sparkles and is gone.

use std::path::PathBuf;
use std::time::Instant;

use hudhook::imgui::{DrawListMut, ImColor32, Ui};

use crate::game::Game;
use crate::layout::Textures;
use crate::markers::Camera;

/// The art and sound (tools/w101_wisp_art.py writes them to w101_orbs/wisps; textures load from w101_orbs/book/art).
const DIR: &str = "../../wisps/";
const CLOUD: [&str; 2] = ["Wisp_Cloud_Red.png", "Wisp_Cloud_Blue.png"];
const SPARK: [&str; 2] = ["Wisp_Spark_Red.png", "Wisp_Spark_Blue.png"];
const GLOW: &str = "Wisp_Glow.png";
const SOUND: &str = "w101_orbs/wisps/ui_health_powerup_01.wav";
/// Horizontal pick-up distance (metres) and the height range it counts over.
const PICKUP: f32 = 1.0;
const PICKUP_HEIGHT: f32 = 2.2;
/// Cloud centre over the path, bob amplitude and period, sideways sway (metres, seconds).
const HOVER: f32 = 0.9;
const BOB: f32 = 0.12;
const BOB_PERIOD: f32 = 2.2;
const SWAY: f32 = 0.5;
/// Drift speed along the path (metres per second), each wisp a little faster or slower.
const SPEED: f32 = 0.45;
/// Sizes (metres): cloud puff, sparkle, ground glow, core glow (round 23 H4: fuller).
/// Round 25 (H5, Ryan's W101 screenshot ref/5_w101_wisp.png next to ours): W101's cloud is a dense red ball about
/// 1.1 m across whose middle burns white (its Add cloud adds light on itself), with many small four-point sparkles
/// (about a tenth of the cloud) scattered round it out to twice its radius, each at its own angle. So: PUFFS puffs
/// over a red body glow, the puffs and a white CORE / HOT centre drawn adding light (hudhook's additive pipeline),
/// SPARKLES sparkles of SPARKLE m, each with its own turn and spin speed, drifting up through a shell 0.4..1.0 m.
const PUFF: f32 = 0.95;
const PUFFS: usize = 7;
const BODY: f32 = 1.25;
const SPARKLE: f32 = 0.085;
const SPARKLES: usize = 16;
const GROUND: f32 = 1.5;
const CORE: f32 = 0.7;
const HOT: f32 = 0.26;
/// Cloud radius for the sight line's free end (round 25 H4).
const CLOUD_RADIUS: f32 = 0.45;
/// Round 23 (H6): the pickup spiral (seconds, turns), the sparkle burst after it, the fade-in of a new wisp.
const SPIN: f32 = 1.6;
const SPIN_TURNS: f32 = 2.5;
const BURST: f32 = 0.5;
const FADE_IN: f32 = 1.5;
/// FX_WispRed_Dsppr.nif's Mover height keys (seconds, W101 units / 100 = metres above the feet).
const MOVER: [(f32, f32); 5] = [(0.0, 0.94), (1.0, 1.04), (2.5, 0.79), (3.47, 0.99), (4.0, 0.94)];
/// W101's spawn groups respawn after 10 s.
const RESPAWN: f32 = 10.0;
/// W101's spawn weights: WC_WispHealth 82 / 79 / 76 against WC_WispMana_UW 18 / 21 / 24.
const MANA_CHANCE: [f32; 3] = [0.18, 0.21, 0.24];
/// Wisps farther than this from the player aren't drawn.
const DRAW_RANGE: f32 = 80.0;
/// Round 23 (H5) faded a wisp out 0.3 s after it went out of sight; round 25 (H4, Ryan: "still visible through
/// walls for a moment before fading ... hide instantly") hides it on the frame its sight line is blocked and only
/// eases it back in over SHOW seconds.
const SHOW: f32 = 0.15;

/// Wizard101's wisp paths on Unicorn Way (zone pathData.xml "Path Wisps 01..03", nodes from pathNodeData.bin), moved
/// into m60_42_36_00 MSB x, y, z (round 20 make_nav.py's frame -> MSB fit; y = W101 z / 100 + 93.999). Each is a
/// loop (its last node is one step from its first).
const PATHS: [&[[f32; 3]]; 3] = [
    // Path Wisps 01
    &[[-104.97, 94.30, -37.62], [-110.20, 94.30, -35.00], [-121.28, 94.30, -37.63], [-129.66, 94.30, -38.76], [-137.39, 94.30, -32.45], [-141.11, 94.30, -23.84], [-144.65, 94.30, -13.63], [-151.98, 93.40, -8.82], [-161.30, 93.00, -12.96], [-164.23, 94.30, -20.08], [-161.21, 94.30, -30.39], [-157.67, 94.30, -37.62], [-153.71, 94.30, -45.13], [-154.94, 92.80, -51.42], [-160.51, 92.80, -58.66], [-160.32, 92.80, -65.47], [-156.79, 92.80, -72.00], [-150.93, 92.80, -74.89], [-143.63, 92.89, -75.92], [-138.06, 94.30, -79.90], [-132.53, 94.30, -86.74], [-128.41, 94.30, -95.35], [-121.99, 94.30, -102.20], [-115.05, 94.30, -101.96], [-109.03, 94.30, -97.37], [-108.41, 94.30, -87.31], [-113.49, 94.30, -78.80], [-118.55, 94.30, -72.17], [-119.72, 94.30, -63.59], [-118.62, 94.30, -60.27], [-110.74, 94.30, -54.92], [-104.13, 94.30, -51.92], [-102.19, 94.30, -48.90], [-102.74, 94.30, -43.21]],
    // Path Wisps 02
    &[[-179.36, 94.30, -22.85], [-172.54, 94.30, -17.87], [-170.38, 93.00, -4.62], [-177.78, 93.77, 3.68], [-187.48, 94.30, -3.04], [-196.34, 94.30, -9.06], [-204.49, 94.30, -14.35], [-214.01, 94.30, -12.88], [-218.62, 94.30, -5.07], [-223.65, 94.30, 5.96], [-228.65, 94.30, 13.27], [-237.15, 94.30, 15.30], [-243.14, 94.30, 11.69], [-245.68, 94.30, 4.09], [-239.47, 94.30, -9.33], [-234.84, 94.30, -18.46], [-234.42, 94.30, -24.53], [-238.42, 94.30, -30.24], [-249.46, 94.30, -36.00], [-258.57, 94.30, -40.98], [-265.36, 93.59, -46.17], [-269.24, 93.00, -58.16], [-265.91, 93.00, -64.27], [-256.07, 93.73, -65.30], [-245.79, 94.30, -58.19], [-232.23, 94.30, -49.39], [-225.07, 93.86, -47.96], [-220.53, 92.80, -50.71], [-214.19, 92.80, -55.65], [-207.15, 92.80, -54.88], [-201.33, 92.80, -51.56], [-198.97, 92.80, -43.77], [-197.06, 93.76, -36.15], [-191.42, 94.30, -30.47]],
    // Path Wisps 03
    &[[-331.07, 94.00, -20.93], [-322.70, 94.00, -25.18], [-315.45, 94.30, -25.23], [-305.26, 94.30, -18.60], [-300.65, 93.30, -15.88], [-299.02, 93.63, -17.60], [-296.07, 94.30, -27.66], [-291.47, 94.30, -40.70], [-287.51, 93.96, -53.96], [-281.75, 93.00, -59.92], [-274.08, 93.00, -58.33], [-270.51, 93.00, -48.75], [-269.07, 94.30, -41.38], [-273.95, 94.30, -28.27], [-278.79, 94.30, -15.12], [-280.04, 94.26, -10.26], [-279.98, 93.30, -2.46], [-279.83, 93.30, 4.61], [-282.71, 93.30, 10.72], [-289.13, 93.30, 14.39], [-296.64, 93.30, 13.92], [-306.54, 93.30, 6.48], [-313.06, 94.00, 1.60], [-320.92, 94.30, -3.73], [-327.57, 94.30, -8.32]],
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Health,
    Mana,
}

struct Wisp {
    kind: Kind,
    path: usize,
    /// Distance along its path loop (metres) and drift speed (negative: the other way round).
    along: f32,
    speed: f32,
    born: Instant,
    seed: f32,
    /// Set when picked up: (when, its offset from the player then (world metres)); it spirals round the player, bursts,
    /// then waits RESPAWN seconds.
    picked: Option<(Instant, [f32; 3])>,
    /// Round 23 (H5): how much of it shows (0..1; round 25: 0 at once when out of sight, eased back in).
    seen: f32,
}

pub struct Wisps {
    folder: PathBuf,
    enabled: bool,
    per_path: usize,
    restore: f32,
    wisps: Vec<Wisp>,
    /// Each path's length and its nodes' running distances.
    lengths: Vec<(f32, Vec<f32>)>,
    rng: u64,
    wanted: bool,
    logged: bool,
    /// Round 24 (H1): the last "in sight" log line (when, how many).
    sight_log: Option<(Instant, usize)>,
}

/// Round 23 (H6): FX_WispRed_Dsppr.nif's Mover height (metres above the feet) `t` seconds in (its keys, linear).
fn mover(t: f32) -> f32 {
    let mut prev = MOVER[0];
    for &(k, h) in &MOVER[1..] {
        if t <= k {
            let f = ((t - prev.0) / (k - prev.0)).clamp(0.0, 1.0);
            let s = f * f * (3.0 - 2.0 * f);
            return prev.1 + (h - prev.1) * s;
        }
        prev = (k, h);
    }
    prev.1
}

/// Round 23 (H6): where a wisp picked up `t` seconds ago is, as an offset from the player's feet (world metres), from
/// `from` (its offset when picked up): round the player SPIN_TURNS times over SPIN seconds, closing in to 0.3 m.
fn spiral(from: [f32; 3], t: f32) -> [f32; 3] {
    let f = (t / SPIN).clamp(0.0, 1.0);
    let r0 = (from[0] * from[0] + from[2] * from[2]).sqrt().clamp(0.5, 1.0);
    let a0 = from[2].atan2(from[0]);
    let r = r0 + (0.3 - r0) * f * f;
    let a = a0 + SPIN_TURNS * std::f32::consts::TAU * (f * (2.0 - f));
    let h = from[1] + (mover(t * 2.0) - from[1]) * (f * 4.0).min(1.0);
    [r * a.cos(), h, r * a.sin()]
}

impl Wisps {
    pub fn new(folder: &std::path::Path) -> Wisps {
        let lengths = PATHS.iter().map(|p| {
            let mut run = vec![0.0];
            for i in 0..p.len() {
                let (a, b) = (p[i], p[(i + 1) % p.len()]);
                run.push(run[i] + ((b[0] - a[0]).powi(2) + (b[2] - a[2]).powi(2)).sqrt());
            }
            (run[p.len()], run)
        }).collect();
        let mut w = Wisps {
            folder: folder.to_path_buf(),
            enabled: true,
            per_path: 5,
            restore: 20.0,
            wisps: Vec::new(),
            lengths,
            rng: 0x9E37_79B9_7F4A_7C15 ^ std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64),
            wanted: false,
            logged: false,
            sight_log: None,
        };
        let ini = folder.join("w101_orbs.ini");
        for line in std::fs::read_to_string(ini).unwrap_or_default().lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            let num = value.parse::<f32>().ok();
            match key.trim() {
                "wisps" => w.enabled = value != "0",
                "wisp_count" => w.per_path = num.map_or(w.per_path, |v| v.clamp(0.0, 20.0) as usize),
                "wisp_restore" => w.restore = num.map_or(w.restore, |v| v.clamp(1.0, 100.0)),
                _ => {}
            }
        }
        tracing::info!("wisps: {} ({} per path on Wizard101's 3 sidewalk paths, {:.0} m / {:.0} m / {:.0} m; restore \
                        {:.0}% of max; respawn {RESPAWN} s)", if w.enabled { "on" } else { "off" }, w.per_path,
                       w.lengths[0].0, w.lengths[1].0, w.lengths[2].0, w.restore);
        w
    }

    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A new wisp on a random node of path `path`.
    fn spawn(&mut self, path: usize, now: Instant) -> Wisp {
        let node = (self.random() * PATHS[path].len() as f32) as usize % PATHS[path].len();
        let kind = if self.random() < MANA_CHANCE[path] { Kind::Mana } else { Kind::Health };
        let speed = SPEED * (0.7 + 0.6 * self.random()) * if self.random() < 0.5 { 1.0 } else { -1.0 };
        Wisp { kind, path, along: self.lengths[path].1[node], speed, born: now, seed: self.random() * 100.0, picked: None,
               seen: 0.0 }
    }

    /// The MSB spot `along` metres round path `path` (on the path, no hover).
    fn point(&self, path: usize, along: f32) -> [f32; 3] {
        let (len, run) = &self.lengths[path];
        let p = PATHS[path];
        let d = along.rem_euclid(*len);
        let i = run.partition_point(|r| *r <= d).saturating_sub(1).min(p.len() - 1);
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        let t = ((d - run[i]) / (run[i + 1] - run[i]).max(1e-3)).clamp(0.0, 1.0);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
    }

    /// Every frame: drift, respawns, pick-ups, drawing. `gameplay` = normal play with the HUD up; `street` = the
    /// player's MSB position on Unicorn Way this frame (None off the street or when it can't be read); `nav` = the
    /// street's walk area, for what walls hide (round 23 H5).
    pub fn frame(&mut self, ui: &Ui, textures: &mut Textures, game: Option<&Game>, gameplay: bool,
                 street: Option<[f32; 3]>, nav: &crate::questhelper::Nav) {
        let (Some(g), true, Some(msb)) = (game, self.enabled, street) else { return };
        if !self.wanted {
            self.wanted = true;
            textures.want(CLOUD.iter().chain(SPARK.iter()).chain([GLOW].iter()).map(|n| format!("{DIR}{n}")));
        }
        let Some(camera) = g.camera() else { return };
        let player = camera.player;
        let now = Instant::now();
        let dt = ui.io().delta_time.clamp(0.0, 0.1);
        if self.wisps.is_empty() {
            for path in 0..PATHS.len() {
                for _ in 0..self.per_path {
                    let w = self.spawn(path, now - std::time::Duration::from_secs(5));
                    self.wisps.push(w);
                }
            }
        }
        // Drift (also while the book is open: W101's world goes on), and respawn the picked-up ones.
        for i in 0..self.wisps.len() {
            match self.wisps[i].picked {
                Some((t, _)) if (now - t).as_secs_f32() > SPIN + BURST + RESPAWN => {
                    let path = self.wisps[i].path;
                    self.wisps[i] = self.spawn(path, now);
                }
                Some(_) => {}
                None => {
                    let w = &mut self.wisps[i];
                    w.along += w.speed * dt;
                }
            }
        }
        // MSB -> physics: the player's offset from its MSB spot.
        let to_world = |p: [f32; 3]| [p[0] - msb[0] + player[0], p[1] - msb[1] + player[1], p[2] - msb[2] + player[2]];
        let time = ui.time() as f32;
        // (cloud centre, ground point) in the world, and the cloud's MSB spot.
        let spots: Vec<([f32; 3], [f32; 3], [f32; 3])> = self.wisps.iter().map(|w| {
            if let Some((t, from)) = w.picked {
                let o = spiral(from, (now - t).as_secs_f32());
                let c = [player[0] + o[0], player[1] + o[1], player[2] + o[2]];
                return (c, [c[0], player[1], c[2]], msb);
            }
            let ground = self.point(w.path, w.along);
            let ahead = self.point(w.path, w.along + 0.5);
            // Sway sideways across the path, bob up and down.
            let (dx, dz) = (ahead[0] - ground[0], ahead[2] - ground[2]);
            let n = (dx * dx + dz * dz).sqrt().max(1e-3);
            let sway = SWAY * ((time * 0.35 + w.seed).sin() * 0.7 + (time * 0.9 + w.seed * 1.3).sin() * 0.3);
            let bob = BOB * (time * std::f32::consts::TAU / BOB_PERIOD + w.seed).sin();
            let centre = [ground[0] - dz / n * sway, ground[1] + HOVER + bob, ground[2] + dx / n * sway];
            (to_world(centre), to_world([centre[0], ground[1], centre[2]]), centre)
        }).collect();
        if !self.logged {
            self.logged = true;
            let near = spots.iter().map(|s| ((s.0[0] - player[0]).powi(2) + (s.0[2] - player[2]).powi(2)).sqrt())
                .fold(f32::MAX, f32::min);
            tracing::info!("wisps: {} on the sidewalk paths, nearest {near:.0} m (player at MSB [{:.0}, {:.0}, {:.0}])",
                           self.wisps.len(), msb[0], msb[1], msb[2]);
        }
        // Round 23 (H5): behind a wall (off the street's walk area between the player and the wisp), or the player in
        // a room under the street or in the hedge maze garden: out of sight.
        // Round 24 (H1): the sight line runs from the camera (what the screen shows) to the cloud through the street's
        // own geometry (Nav::sees_3d, street_sight.bin), not the walk graph.
        use crate::questhelper::{in_garden, in_room};
        let away = in_room(msb) || in_garden(msb);
        let m = &camera.matrix;
        let eye = [m[3][0] - player[0] + msb[0], m[3][1] - player[1] + msb[1], m[3][2] - player[2] + msb[2]];
        // A camera read far from the player (a bad frame) falls back to the player's head.
        let eye = if (eye[0] - msb[0]).hypot(eye[2] - msb[2]) < 12.0 { eye } else { [msb[0], msb[1] + 1.6, msb[2]] };
        let mut in_sight = 0;
        for (w, s) in self.wisps.iter_mut().zip(&spots) {
            // Round 26 (O1): with the game's own depth test on, the street sight test is not used (occlusion.rs).
            let seen = w.picked.is_some() || crate::occlusion::active()
                || (!away && nav.sees_3d_near(eye, s.2, CLOUD_RADIUS));
            in_sight += seen as usize;
            w.seen = if seen { (w.seen + dt / SHOW).min(1.0) } else { 0.0 };
        }
        if self.sight_log.is_none_or(|(t, n)| n != in_sight && (now - t).as_secs_f32() > 5.0) {
            self.sight_log = Some((now, in_sight));
            tracing::info!("wisps: {in_sight} of {} in sight from the camera at MSB [{:.1}, {:.1}, {:.1}]{}",
                           self.wisps.len(), eye[0], eye[1], eye[2], if away { " (player in a room or the garden)" } else { "" });
        }
        for (w, (p, _, _)) in self.wisps.iter_mut().zip(&spots) {
            if w.picked.is_some() || !gameplay {
                continue;
            }
            let d = ((p[0] - player[0]).powi(2) + (p[2] - player[2]).powi(2)).sqrt();
            if d < PICKUP && (p[1] - HOVER - player[1]).abs() < PICKUP_HEIGHT {
                let fp = w.kind == Kind::Mana;
                if let Some((from, to)) = g.add_player_stat(fp, self.restore) {
                    tracing::info!("wisps: picked up a {:?} wisp on path {}, {} {from} -> {to}", w.kind, w.path + 1,
                                   if fp { "FP" } else { "HP" });
                    w.picked = Some((now, [p[0] - player[0], p[1] - player[1], p[2] - player[2]]));
                    w.seen = 1.0;
                    crate::music::play_file(self.folder.join(SOUND));
                }
            }
        }
        if !gameplay {
            return;
        }
        let draw = ui.get_background_draw_list();
        let screen = ui.io().display_size;
        // Far ones first so near ones draw over them.
        let mut order: Vec<(f32, usize)> = spots.iter().enumerate()
            .filter(|(i, s)| self.wisps[*i].picked.is_none_or(|(t, _)| (now - t).as_secs_f32() < SPIN + BURST)
                && self.wisps[*i].seen > 0.0
                && ((s.0[0] - player[0]).powi(2) + (s.0[2] - player[2]).powi(2)).sqrt() < DRAW_RANGE)
            .filter_map(|(i, s)| camera.project(s.0, screen).map(|(_, z)| (z, i))).collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (z, i) in order {
            let w = &self.wisps[i];
            // Round 26 (O1): hidden wherever the game's scene is in front of it (occlusion.rs).
            crate::occlusion::begin(z, CLOUD_RADIUS, "", [0.0, 0.0]);
            let picked = w.picked.map(|(t, from)| ((now - t).as_secs_f32(), from));
            draw_wisp(&draw, textures, &camera, Look { kind: w.kind, seed: w.seed, age: (now - w.born).as_secs_f32(),
                      picked, seen: w.seen }, (spots[i].0, spots[i].1), time, screen);
        }
        crate::occlusion::end();
    }
}

/// What a wisp looks like this frame: kind, its random seed, seconds since it spawned, (seconds since picked up, its
/// offset from the player then), how much of it is in sight.
pub struct Look {
    kind: Kind,
    seed: f32,
    age: f32,
    picked: Option<(f32, [f32; 3])>,
    seen: f32,
}

/// Preview (hud_orbs/preview "wisp:<t>"): a health wisp floating at `at` (world) and, when `picked` is given, a mana
/// wisp picked up that many seconds ago by a player standing at `player` (it was at `at` then).
pub fn preview(ui: &Ui, textures: &mut Textures, camera: &Camera, player: [f32; 3], at: [f32; 3], picked: Option<f32>,
               time: f32) {
    textures.want(CLOUD.iter().chain(SPARK.iter()).chain([GLOW].iter()).map(|n| format!("{DIR}{n}")));
    let draw = ui.get_background_draw_list();
    let screen = ui.io().display_size;
    match picked {
        None => {
            let look = Look { kind: Kind::Health, seed: 1.7, age: 10.0, picked: None, seen: 1.0 };
            draw_wisp(&draw, textures, camera, look, (at, [at[0], player[1], at[2]]), time, screen);
        }
        Some(t) => {
            let from = [at[0] - player[0], at[1] - player[1], at[2] - player[2]];
            let o = spiral(from, t);
            let c = [player[0] + o[0], player[1] + o[1], player[2] + o[2]];
            let look = Look { kind: Kind::Mana, seed: 3.1, age: 10.0, picked: Some((t, from)), seen: 1.0 };
            draw_wisp(&draw, textures, camera, look, (c, [c[0], player[1], c[2]]), time, screen);
        }
    }
}

/// Round 25 (H5): switches the background list's blending for what follows (hudhook's additive pipeline, as
/// teleport.rs does on the foreground list).
unsafe fn blend(additive: bool) {
    use hudhook::imgui::sys;
    let mode = if additive { hudhook::BLEND_ADDITIVE } else { hudhook::BLEND_NORMAL };
    sys::ImDrawList_AddCallback(sys::igGetBackgroundDrawList(), Some(hudhook::blend_callback), mode as *mut _);
}

fn draw_wisp(draw: &DrawListMut, textures: &Textures, camera: &Camera, w: Look, spot: ([f32; 3], [f32; 3]),
             time: f32, screen: [f32; 2]) {
    let k = (w.kind == Kind::Mana) as usize;
    let (Some((cloud, _)), Some((spark, _)), Some((glow, _))) = (textures.get(&format!("{DIR}{}", CLOUD[k])),
        textures.get(&format!("{DIR}{}", SPARK[k])), textures.get(&format!("{DIR}{GLOW}"))) else { return };
    let tint = if k == 0 { [1.0, 0.35, 0.3] } else { [0.35, 0.55, 1.0] };
    let mut alpha = (w.age / FADE_IN).min(1.0) * w.seen;
    let mut grow = 1.0;
    let centre = spot.0;
    let tau = std::f32::consts::TAU;
    let px_at = |z: f32, metres: f32| metres * screen[1] / (2.0 * z * (camera.fov / 2.0).tan());
    if let Some((t, from)) = w.picked {
        // Round 23 (H6): the wind ribbon (FX_WispRed_Dsppr's Wind02) trailing the last 0.2 s of the spiral round the
        // player, and after SPIN s the yellow sparkles leaving (FX_SparkleYllwExit) while the cloud shrinks away.
        let feet = [centre[0] - spiral(from, t)[0], spot.1[1], centre[2] - spiral(from, t)[2]];
        let fade = (1.0 - (t - SPIN) / BURST).clamp(0.0, 1.0);
        for ribbon in 0..2 {
            let lag = 0.06 * ribbon as f32;
            let points: Vec<([f32; 2], f32)> = (0..24).filter_map(|i| {
                let s = (t - lag - 0.2 * i as f32 / 23.0).max(0.0);
                let o = spiral(from, s.min(SPIN));
                let r = if ribbon == 0 { 1.0 } else { 11.6 / 17.6 }; // Wind02's outer and inner half rings
                let p = [feet[0] + o[0] * r, feet[1] + o[1] - 0.12 * ribbon as f32, feet[2] + o[2] * r];
                camera.project(p, screen).map(|(q, z)| (q, z))
            }).collect();
            for (i, pair) in points.windows(2).enumerate() {
                let a = (1.0 - i as f32 / 23.0) * 0.45 * fade * alpha;
                let width = px_at(pair[0].1, 0.025).max(1.5);
                draw.add_line(pair[0].0, pair[1].0, ImColor32::from_rgba_f32s(1.0, 1.0, 1.0, a)).thickness(width).build();
            }
        }
        if t > SPIN {
            let f = ((t - SPIN) / BURST).clamp(0.0, 1.0);
            for i in 0..10 {
                let a = i as f32 / 10.0 * tau + w.seed;
                let up = 0.4 + 0.5 * ((i * 7 % 10) as f32 / 10.0);
                let p = [centre[0] + a.cos() * 0.7 * f, centre[1] + up * f, centre[2] + a.sin() * 0.7 * f];
                let Some((s, sz)) = camera.project(p, screen) else { continue };
                let r = px_at(sz, SPARKLE * 1.2) / 2.0;
                draw.add_image(glow, [s[0] - r, s[1] - r], [s[0] + r, s[1] + r])
                    .col([1.0, 0.92, 0.35, (1.0 - f) * alpha]).build();
            }
            grow = 1.0 - 0.8 * f;
            alpha *= 1.0 - f;
        }
    }
    if alpha <= 0.0 {
        return;
    }
    let Some((c, z)) = camera.project(centre, screen) else { return };
    let px = |metres: f32| px_at(z, metres);
    // Ground glow (GlowShadow): flattened, under the wisp's drift point, not bobbing.
    let ground = [spot.1[0], spot.1[1] + 0.05, spot.1[2]];
    if let Some((g, _)) = camera.project(ground, screen) {
        let (rw, rh) = (px(GROUND) / 2.0, px(GROUND) / 2.0 * 0.35);
        draw.add_image(glow, [g[0] - rw, g[1] - rh], [g[0] + rw, g[1] + rh])
            .col([tint[0], tint[1], tint[2], 0.5 * alpha]).build();
    }
    // Round 25 (H5): a soft red body under the puffs, so the ball reads full and red on the bright street too.
    let body = px(BODY) / 2.0 * grow;
    let body_tint = if k == 0 { [1.0, 0.1, 0.06] } else { [0.12, 0.3, 1.0] };
    draw.add_image(glow, [c[0] - body, c[1] - body], [c[0] + body, c[1] + body])
        .col([body_tint[0], body_tint[1], body_tint[2], 0.8 * alpha]).build();
    // Cloud: PUFFS puffs turning at their own speeds round the centre (FX_Cloud Main), dense and saturated.
    for i in 0..PUFFS {
        let f = i as f32;
        let turn = time * (0.45 + 0.2 * f) * if i % 2 == 0 { 1.0 } else { -1.0 } + f * 2.1 + w.seed;
        let off = [px(0.12) * (turn * 0.7 + f).cos(), px(0.09) * (turn * 0.9 + f).sin()];
        let r = px(PUFF) / 2.0 * grow * (1.0 - 0.07 * f);
        let corner = |a: f32| [c[0] + off[0] + r * 1.414 * a.cos(), c[1] + off[1] + r * 1.414 * a.sin()];
        let q = std::f32::consts::FRAC_PI_4;
        let a = [corner(turn - 3.0 * q), corner(turn - q), corner(turn + q), corner(turn + 3.0 * q)];
        draw.add_image_quad(cloud, a[0], a[1], a[2], a[3]).col([1.0, 1.0, 1.0, alpha * (0.9 - 0.06 * f)]).build();
    }
    // The centre and sparkles add light (W101's Add cloud and sparks, Gamebryo ONE/ONE).
    unsafe { blend(true) };
    // The white glowing centre (FX_Wisp's focus): a warm-white glow and a small hot white heart, pulsing a little.
    let pulse = 1.0 + 0.08 * (time * 2.3 + w.seed).sin();
    let core = px(CORE) / 2.0 * grow * pulse;
    draw.add_image(glow, [c[0] - core, c[1] - core], [c[0] + core, c[1] + core])
        .col([1.0, 0.8 + 0.2 * k as f32, 0.75 + 0.25 * k as f32, 0.5 * alpha]).build();
    let hot = px(HOT) / 2.0 * grow * pulse;
    draw.add_image(glow, [c[0] - hot, c[1] - hot], [c[0] + hot, c[1] + hot]).col([1.0, 1.0, 1.0, 0.8 * alpha]).build();
    // Sparkles (FX_SparkleBlue / FX_FocusSpark emitters): small four-point stars scattered round the ball, each with
    // its own angle and spin (round 25 H5: they all stood at one angle before), drifting up and twinkling.
    for i in 0..SPARKLES {
        let f = i as f32;
        let h = |n: f32| ((f * 12.9898 + n * 78.233 + w.seed * 3.7).sin() * 43758.547).fract().abs();
        let a = time * 0.35 * if i % 2 == 0 { 1.0 } else { -1.0 } + h(1.0) * tau;
        let life = 2.4 + 1.6 * h(2.0);
        let phase = (time / life + h(3.0)).fract();
        let rad = (0.45 + 0.65 * h(4.0)) * grow;
        let up = (phase - 0.5) * 0.9 + (h(5.0) - 0.5) * 0.5;
        let p = [centre[0] + rad * a.cos(), centre[1] + up, centre[2] + rad * a.sin()];
        let Some((s, sz)) = camera.project(p, screen) else { continue };
        let r = px_at(sz, SPARKLE * (0.7 + 0.6 * h(6.0))) / 2.0 * 1.414;
        let spin = h(7.0) * tau + time * (1.5 + 2.5 * h(8.0)) * if h(9.0) < 0.5 { 1.0 } else { -1.0 };
        let corner = |d: f32| [s[0] + r * (spin + d).cos(), s[1] + r * (spin + d).sin()];
        let q = std::f32::consts::FRAC_PI_4;
        let twinkle = (phase * std::f32::consts::PI).sin() * (0.6 + 0.4 * (time * 7.0 + f * 1.9).sin());
        draw.add_image_quad(spark, corner(-3.0 * q), corner(-q), corner(q), corner(3.0 * q))
            .col([1.0, 1.0, 1.0, alpha * twinkle.max(0.0)]).build();
    }
    unsafe { blend(false) };
}
