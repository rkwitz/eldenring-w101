//! Wizard101's teleport "poof" on the player before a warp (Ryan, 2026-10-29: "the sparkles ... where the player is,
//! and they poof and disappear ... before the actual teleport happens").
//!
//! Wizard101 plays _Shared-WorldData.wad Character/FX_Disappear.nif (the client's m_poofEffectAsset) on a character
//! that teleports away; the character has no animation of its own and just vanishes inside it (the book-warp events
//! hide the player at 0.35 s). The effect, from the .nif (Gamebryo particle systems, all world-space):
//!   - FX_CastSignSpark / FX_CastSignSpark01: gold and blue-white four-point stars (FX_DisappearSparks._2 / ._1,
//!     drawn additively), 272.7 a second each for 0.367 s, born at the corners of a small box that the node
//!     "FX SparkEmit" carries along a path over those 0.367 s: a helix of radius 100 units round the body, one turn
//!     near the feet, then about 1.2 turns up to chest height (z 17 -> 164) and a curl into the middle at z 179. So
//!     the stars draw a rising spiral. They don't move (speed 0); Wind01 pushes up at 90 units/s² and Gravity01
//!     (turned upside down in the file) pulls down at 180, so they sink slowly. Size 15 +- 4.5 units x the system's
//!     scale 1.95, life 0.533 +- 0.2, shrinking over their last 0.167 s, spinning at 300 +- 100 degrees a second
//!     from upright;
//!   - FX_StunSparkle / FX_StunSparkle01: the same stars as quick twinkles (blue 409 a second, size 20; gold 272.7,
//!     size 15) born on the surface of a body-shaped mesh ("Editable Mesh", z 10..330 units) and flung out along
//!     its normals at 300 +- 300 units/s; they grow over 0.033 s and live 0.1 s;
//!   - Cylinder01 / Cylinder02: two open tubes (radius 89 / 69, height 229 / 275) textured FX_LeaveToob_01 (bands of
//!     orange and blue light), its v scrolled once over 0.367 s so the bands rise; vertex colours 0.65 on the sides,
//!     0 at the rims (they fade out at the top and bottom), drawn additively;
//!   - FX_Smoke: 20 puffs (FX_SmokeChimney, size 100) born on the body mesh at 0.267-0.367 s, rising (Wind02, 180
//!     units/s²) for 1.333 s and spinning; colour and alpha keys 0.18 -> 0.6 at 0.2 -> 0.18, lit (emissive 0.5),
//!     alpha-blended.
//!   All the sparks' colour keys run white -> half -> nothing over their life. Gamebryo's variations are taken as
//!   +- half the stored value.
//! Its textures and the body mesh / spark path come from tools/w101_teleport_art.py (book\art\teleport: PNGs and
//! fx_disappear.tsv); without the textures nothing is drawn, without the .tsv a cylinder and a plain helix stand in.
//!
//! Two triggers:
//!   - the spellbook's map buttons: their events (tools/emevd.py book_warps) freeze the player, set POOF_FLAG, play
//!     Elden Ring's Unseen Form burst on him and hide him at 0.35 s (SpEffect 9910, camouflage at alpha 0), and only
//!     1.2 s later bring up the load screen and warp.
//!     The poof is drawn live, in the world, on the player's feet.
//!   - Elden Ring's own fast travel (and any other game warp that is not ours, death excluded): the engine starts
//!     the warp from its map, so the world is not on screen any more and the warp can't be held back from EMEVD.
//!     The overlay keeps a copy of the world every few frames of plain gameplay (screen copy slot 2) with the camera
//!     it was seen from; when Elden Ring's loading screen count goes up, it shows that copy with the poof on the
//!     player (projected with the stored camera) for POOF_FROZEN seconds, and only then lets the Wizard101 loading
//!     screen come up. Elden Ring keeps loading underneath, so nothing is delayed.

use std::time::Instant;

use hudhook::imgui::{DrawListMut, ImColor32, TextureId, Ui};
use hudhook::RenderContext;

use crate::layout::Textures;
use crate::markers::Camera;

/// Set by the book-warp events when the poof starts (tools/emevd.py POOF_FLAG); cleared here.
pub const POOF_FLAG: u32 = 1042362912;
/// The textures (book\art\teleport, from tools/w101_teleport_art.py): blue star, gold star, tube bands, smoke.
pub const ART: [&str; 4] = ["teleport/sparks_blue.png", "teleport/sparks_gold.png", "teleport/tube.png", "teleport/smoke.png"];
const WORLD: &str = "@screen/teleport_world";
const SLOT: usize = 2;
/// A world copy every this many frames of plain gameplay.
const CAPTURE_EVERY: u32 = 6;
/// Seconds the frozen poof plays before the loading screen (fast travel).
const POOF_FROZEN: f32 = 1.3;
/// Seconds the live poof is drawn (the book warps' load screen comes at 1.2 s and covers the rest).
const POOF_LIVE: f32 = 1.75;
/// A world copy older than this is not used (seconds).
const SHOT_MAX_AGE: f32 = 600.0;
/// After a book warp's POOF_FLAG, Elden Ring's loading screens are that warp's (seconds).
const BOOK_WINDOW: f32 = 20.0;
/// After Elden Ring's quit-out byte is seen set, its loading screen is the quit to the title screen (seconds).
const QUIT_WINDOW: f32 = 30.0;
/// The player must have been loaded this recently when a loading screen starts (not at the title screen; seconds).
const PLAYER_RECENT: f32 = 2.0;

#[derive(Default)]
pub struct Teleport {
    frames: u32,
    /// The camera of the world copy in slot 2 and when it was taken.
    shot: Option<(Camera, Instant)>,
    poof: Option<Poof>,
    book_at: Option<Instant>,
    /// Elden Ring's loading screen started during a frozen poof: let it through when the poof ends.
    held: bool,
    alive: bool,
    /// When the player character was last loaded (HP read), and when the quit-out byte was last seen set.
    player_at: Option<Instant>,
    quit_at: Option<Instant>,
    wanted: bool,
    missing_logged: bool,
    /// FX_Disappear.nif's body mesh and spark path (book\art\teleport\fx_disappear.tsv).
    shape: Shape,
    /// Round 29 (T1): Torrent faded out with the player (book warp): 0 not yet, 1 faded, 2 faded back.
    torrent: u8,
}

/// Round 29 (T1, Ryan: "when you teleport and your character fades out, Torrent must fade out too"): the book-warp
/// events hide the player with SpEffect 9910 at POOF_HIDE_AT s (tools/emevd.py); Torrent gets no SpEffect from
/// EMEVD (no entity id), so the overlay fades him with his own ChrIns base transparency at the same moment
/// (Game::fade_torrent), and back in after TORRENT_BACK s in case the warp did not happen (a warp's map load
/// removes him anyway; he comes back whole when called again).
const POOF_HIDE_AT: f32 = 0.35;
const TORRENT_BACK: f32 = 8.0;

impl Teleport {
    /// Reads the effect's body mesh and spark path from the art folder (book\art); a stand-in shape without it.
    pub fn load(&mut self, art: &std::path::Path) {
        let path = art.join("teleport").join("fx_disappear.tsv");
        match Shape::load(&path) {
            Some(shape) => {
                tracing::info!("teleport: {} body triangles, {} path keys", shape.tris.len(), shape.path.len());
                self.shape = shape;
            }
            None => tracing::warn!("teleport: {} missing or bad (tools/w101_teleport_art.py); a plain cylinder and                                     helix stand in", path.display()),
        }
    }

    /// Before the frame: keeps a copy of the world (with its camera) while the game is plainly on screen.
    /// `plain`: game HUD up, no fade, book shut, no load screen.
    pub fn capture(&mut self, render: &mut dyn RenderContext, textures: &mut Textures, plain: bool,
                   camera: Option<Camera>) {
        self.frames = self.frames.wrapping_add(1);
        if self.poof.is_some() || !plain || self.frames % CAPTURE_EVERY != 0 {
            return;
        }
        let Some(camera) = camera else { return };
        if let Some(id) = render.capture_screen(SLOT) {
            textures.set(WORLD, id, [1.0, 1.0]);
            self.shot = Some((camera, Instant::now()));
        }
    }

    /// Each frame, before the loading screen is told about `loading` (Elden Ring started a loading screen this
    /// frame): starts and draws the poof, and returns `loading` as the loading screen should see it (held back
    /// while a fast-travel poof plays, then let through). `door`: the mod's own door/warp load screen flag is on, `hp`: the player's HP when he is on screen.
    pub fn frame(&mut self, ui: &Ui, textures: &mut Textures, game: Option<&crate::game::Game>, loading: bool,
                 door: bool, hp: Option<i32>) -> bool {
        if !self.wanted {
            self.wanted = true;
            textures.want(ART.iter().map(|s| s.to_string()));
        }
        let art: Vec<Option<TextureId>> = ART.iter().map(|n| textures.get(n).map(|t| t.0)).collect();
        let ready = art.iter().all(Option::is_some);
        if let Some(hp) = hp {
            self.alive = hp > 0;
            self.player_at = Some(Instant::now());
        }
        if game.is_some() && crate::game::quitting_out() {
            self.quit_at = Some(Instant::now());
        }
        if game.is_some() && crate::game::event_flag(POOF_FLAG) == Some(true) {
            crate::game::set_event_flag(POOF_FLAG, false);
            self.book_at = Some(Instant::now());
            self.torrent = 0;
            match (ready, game.and_then(|g| g.camera())) {
                (true, Some(camera)) => {
                    tracing::info!("teleport: book warp poof at {:?}", camera.player);
                    self.poof = Some(Poof::new(camera.player, None, POOF_LIVE));
                }
                _ => self.log_missing(ready),
            }
        }
        if let (Some(g), Some(at)) = (game, self.book_at) {
            let t = at.elapsed().as_secs_f32();
            if self.torrent == 0 && t >= POOF_HIDE_AT {
                self.torrent = 1;
                match g.fade_torrent(true) {
                    Some(chr) => tracing::info!("teleport: Torrent ({chr:#x}) fading out with the player"),
                    None => tracing::info!("teleport: no Torrent loaded to fade out"),
                }
            } else if self.torrent == 1 && t >= TORRENT_BACK && !loading {
                self.torrent = 2;
                if let Some(chr) = g.fade_torrent(false) {
                    tracing::info!("teleport: Torrent ({chr:#x}) faded back in");
                }
            }
        }
        let mut out = loading;
        if loading && !door && self.poof.is_none() {
            let book = self.book_at.is_some_and(|t| t.elapsed().as_secs_f32() < BOOK_WINDOW);
            // Used once: the next warp needs a copy of the world it starts from (after loading a save, none yet).
            let shot = self.shot.take().filter(|(_, at)| at.elapsed().as_secs_f32() < SHOT_MAX_AGE);
            // A quit-out to the title screen (Ryan's log, round 15) or a load from the title screen isn't a warp.
            let quit = self.quit_at.take().is_some_and(|t| t.elapsed().as_secs_f32() < QUIT_WINDOW);
            let title = !self.player_at.is_some_and(|t| t.elapsed().as_secs_f32() < PLAYER_RECENT);
            if quit || title {
                tracing::info!("teleport: loading screen {}, no poof",
                               if quit { "of a quit-out to the title screen" } else { "from the title screen" });
            }
            match (book || quit || title, self.alive, shot, ready, textures.get(WORLD)) {
                (false, true, Some((camera, at)), true, Some(_)) => {
                    tracing::info!("teleport: Elden Ring warp, poof on the world copy from {:.1} s ago",
                                   at.elapsed().as_secs_f32());
                    self.poof = Some(Poof::new(camera.player, Some(camera), POOF_FROZEN));
                    self.held = true;
                    out = false;
                }
                (false, true, _, false, _) => self.log_missing(false),
                _ => {}
            }
        }
        if let Some(poof) = &mut self.poof {
            let t = poof.start.elapsed().as_secs_f32();
            if t >= poof.seconds {
                self.poof = None;
                if std::mem::take(&mut self.held) {
                    out = true;
                }
            } else {
                let camera = poof.frozen.or_else(|| game.and_then(|g| g.camera()));
                let draw = ui.get_foreground_draw_list();
                let size = ui.io().display_size;
                if poof.frozen.is_some() {
                    if let Some((world, _)) = textures.get(WORLD) {
                        draw.add_image(world, [0.0, 0.0], size).build();
                    }
                }
                poof.step(t, &self.shape);
                if let (Some(camera), true) = (camera, ready) {
                    let art: Vec<TextureId> = art.into_iter().flatten().collect();
                    poof.draw(&draw, &camera, &art, size, t);
                    // Back to alpha blending for whatever is drawn after.
                    unsafe { blend(false) };
                }
            }
        }
        out
    }

    fn log_missing(&mut self, ready: bool) {
        if !self.missing_logged {
            self.missing_logged = true;
            if ready {
                tracing::warn!("teleport: no camera, poof skipped");
            } else {
                tracing::warn!("teleport: art missing (book\\art\\teleport, tools/w101_teleport_art.py), poof skipped");
            }
        }
    }
}


/// Metres per Wizard101 unit for the effect: its emitter mesh (z 10..330 units) spans the player's 1.9 m.
const UNIT: f32 = 1.9 / 320.0;
/// The sparks' emitting time (the path's length, the tubes' scroll).
const EMIT_SECONDS: f32 = 0.367;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// Rising-spiral star (CastSignSpark), born on the path.
    Spark,
    /// Quick twinkle (StunSparkle), flung off the body.
    Twinkle,
    Smoke,
}

/// One of FX_Disappear.nif's particle systems.
struct System {
    kind: Kind,
    /// Index in ART.
    texture: usize,
    /// Births a second, from `start` to `stop` seconds.
    rate: f32,
    start: f32,
    stop: f32,
    /// Size (units) and its variation.
    radius: f32,
    radius_var: f32,
    life: f32,
    life_var: f32,
    speed: f32,
    speed_var: f32,
    /// Net upward acceleration (units/s²) and turbulence.
    lift: f32,
    turbulence: f32,
    /// Spin (radians a second) and its variation; the sign is random.
    spin: f32,
    spin_var: f32,
    /// Growing (seconds from birth) and shrinking (seconds before death) of the size.
    grow: f32,
    fade: f32,
}

const SYSTEMS: [System; 5] = [
    // FX_CastSignSpark (gold, FX_DisappearSparks._2) and FX_CastSignSpark01 (blue): system scale 1.95.
    System { kind: Kind::Spark, texture: 1, rate: 272.7, start: 0.0, stop: EMIT_SECONDS, radius: 15.0 * 1.95,
             radius_var: 4.5 * 1.95, life: 0.533, life_var: 0.2, speed: 0.0, speed_var: 0.0, lift: 90.0 - 180.0,
             turbulence: 6.0, spin: 5.236, spin_var: 1.745, grow: 0.0, fade: 0.167 },
    System { kind: Kind::Spark, texture: 0, rate: 272.7, start: 0.0, stop: EMIT_SECONDS, radius: 15.0 * 1.95,
             radius_var: 4.5 * 1.95, life: 0.533, life_var: 0.2, speed: 0.0, speed_var: 0.0, lift: 90.0 - 180.0,
             turbulence: 6.0, spin: 5.236, spin_var: 1.745, grow: 0.0, fade: 0.167 },
    // FX_StunSparkle (blue) and FX_StunSparkle01 (gold).
    System { kind: Kind::Twinkle, texture: 0, rate: 409.1, start: 0.0, stop: EMIT_SECONDS, radius: 20.0,
             radius_var: 0.0, life: 0.1, life_var: 0.0, speed: 300.0, speed_var: 300.0, lift: 0.0, turbulence: 0.0,
             spin: 3.491, spin_var: 0.611, grow: 0.0333, fade: 0.0 },
    System { kind: Kind::Twinkle, texture: 1, rate: 272.7, start: 0.0, stop: EMIT_SECONDS, radius: 15.0,
             radius_var: 0.0, life: 0.1, life_var: 0.0, speed: 300.0, speed_var: 300.0, lift: 0.0, turbulence: 0.0,
             spin: 3.491, spin_var: 0.611, grow: 0.0333, fade: 0.0 },
    // FX_Smoke.
    System { kind: Kind::Smoke, texture: 3, rate: 200.0, start: 0.267, stop: EMIT_SECONDS, radius: 100.0,
             radius_var: 0.0, life: 1.333, life_var: 0.0, speed: 0.0, speed_var: 0.0, lift: 180.0, turbulence: 2.0,
             spin: 3.491, spin_var: 0.611, grow: 0.0, fade: 0.0 },
];

/// The two tubes: radius, then rings of (height, texture v, vertex colour) up the side, all in units
/// (Cylinder01 / Cylinder02's vertices).
const TUBES: [(f32, [(f32, f32, f32); 4]); 2] = [
    (89.0, [(0.0, 0.49, 0.0), (15.0, 0.47, 0.65), (147.0, 0.19, 0.65), (229.0, 0.0, 0.0)]),
    (69.0, [(0.0, 0.49, 0.0), (18.0, 0.47, 0.65), (176.0, 0.19, 0.65), (275.0, 0.0, 0.0)]),
];
/// The small box the path carries: the sparks are born at its corners (units).
const BOX: [[f32; 3]; 8] = [[-8.0, -8.0, 0.0], [-8.0, -8.0, 15.0], [-8.0, 8.0, 0.0], [-8.0, 8.0, 15.0],
                            [8.0, -8.0, 0.0], [8.0, -8.0, 15.0], [8.0, 8.0, 0.0], [8.0, 8.0, 15.0]];

/// FX_Disappear.nif's body mesh (triangles, Wizard101 units, z up, from the feet) and the spark path (percent, x,
/// y, z), from book\art\teleport\fx_disappear.tsv.
#[derive(Default)]
pub struct Shape {
    tris: Vec<[[f32; 3]; 3]>,
    path: Vec<[f32; 4]>,
}

impl Shape {
    /// "v x y z" vertices, "t a b c" triangles (vertex indices), "p percent x y z" path keys.
    fn load(path: &std::path::Path) -> Option<Shape> {
        let text = std::fs::read_to_string(path).ok()?;
        let (mut verts, mut tris, mut keys) = (Vec::new(), Vec::new(), Vec::new());
        for line in text.lines() {
            let mut f = line.split_whitespace();
            let tag = f.next();
            let n: Vec<f32> = f.filter_map(|x| x.parse().ok()).collect();
            match (tag, n.len()) {
                (Some("v"), 3) => verts.push([n[0], n[1], n[2]]),
                (Some("t"), 3) => tris.push([n[0] as usize, n[1] as usize, n[2] as usize]),
                (Some("p"), 4) => keys.push([n[0], n[1], n[2], n[3]]),
                _ => {}
            }
        }
        let tris: Vec<[[f32; 3]; 3]> = tris.iter()
            .filter_map(|t| Some([*verts.get(t[0])?, *verts.get(t[1])?, *verts.get(t[2])?]))
            .collect();
        (!tris.is_empty() && keys.len() >= 2).then_some(Shape { tris, path: keys })
    }

    /// Where the path is at `f` (0..1 of the emitting time), units.
    fn path_at(&self, f: f32) -> [f32; 3] {
        let f = f.clamp(0.0, 1.0);
        if self.path.len() < 2 {
            // Stand-in: the same helix without the file (radius 100, 2.3 turns, up to z 179).
            let a = -f * std::f32::consts::TAU / 0.388;
            let r = if f < 0.854 { 100.0 } else { 100.0 * (1.0 - f) / 0.146 };
            return [r * a.cos(), r * a.sin(), 1.7 + 177.5 * f];
        }
        let i = self.path.iter().position(|k| k[0] >= f).unwrap_or(self.path.len() - 1).max(1);
        let (a, b) = (self.path[i - 1], self.path[i]);
        let k = ((f - a[0]) / (b[0] - a[0]).max(1e-6)).clamp(0.0, 1.0);
        [a[1] + k * (b[1] - a[1]), a[2] + k * (b[2] - a[2]), a[3] + k * (b[3] - a[3])]
    }

    /// A random point on the body's surface and the outward normal there (units), from three random numbers.
    fn body_point(&self, r: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        if self.tris.is_empty() {
            // Stand-in: a cylinder of radius 97 from z 10 to 330.
            let a = r[0] * std::f32::consts::TAU;
            return ([97.0 * a.cos(), 97.0 * a.sin(), 10.0 + 320.0 * r[1]], [a.cos(), a.sin(), 0.0]);
        }
        let [a, b, c] = self.tris[((r[0] * self.tris.len() as f32) as usize).min(self.tris.len() - 1)];
        let (mut u, mut v) = (r[1], r[2]);
        if u + v > 1.0 {
            (u, v) = (1.0 - u, 1.0 - v);
        }
        let p: [f32; 3] = std::array::from_fn(|k| a[k] + u * (b[k] - a[k]) + v * (c[k] - a[k]));
        let (e1, e2) = (sub(b, a), sub(c, a));
        let mut n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
        n = n.map(|x| x / len);
        if n[0] * p[0] + n[1] * p[1] < 0.0 {
            n = n.map(|x| -x);
        }
        (p, n)
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Wizard101 units round the feet (z up) -> metres in Elden Ring's axes (y up).
fn metres(p: [f32; 3]) -> [f32; 3] {
    [p[0] * UNIT, p[2] * UNIT, p[1] * UNIT]
}

/// Switches the overlay's blending for what is drawn next on the foreground list (hudhook's additive pipeline).
unsafe fn blend(additive: bool) {
    use hudhook::imgui::sys;
    let mode = if additive { hudhook::BLEND_ADDITIVE } else { hudhook::BLEND_NORMAL };
    sys::ImDrawList_AddCallback(sys::igGetForegroundDrawList(), Some(hudhook::blend_callback), mode as *mut _);
}

struct Particle {
    system: usize,
    /// Units, z up, round the feet.
    pos: [f32; 3],
    vel: [f32; 3],
    age: f32,
    life: f32,
    radius: f32,
    angle: f32,
    spin: f32,
}

struct Poof {
    start: Instant,
    last: f32,
    /// The player's feet (physics position) when it began.
    feet: [f32; 3],
    /// The camera of the world copy (fast travel), or None to follow the live camera.
    frozen: Option<Camera>,
    seconds: f32,
    particles: Vec<Particle>,
    owed: [f32; 5],
    rng: u32,
}

impl Poof {
    fn new(feet: [f32; 3], frozen: Option<Camera>, seconds: f32) -> Poof {
        let seed = (feet[0].to_bits() ^ feet[2].to_bits().rotate_left(13)) | 1;
        Poof { start: Instant::now(), last: 0.0, feet, frozen, seconds, particles: Vec::new(), owed: [0.0; 5], rng: seed }
    }

    fn random(&mut self) -> f32 {
        // xorshift32
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    /// -v/2 .. v/2.
    fn vary(&mut self, v: f32) -> f32 {
        (self.random() - 0.5) * v
    }

    /// Births, moves and ages the particles up to `t` seconds (in steps of at most 1/60 s).
    fn step(&mut self, t: f32, shape: &Shape) {
        while self.last < t - 1e-6 {
            let dt = (t - self.last).min(1.0 / 60.0);
            self.tick(self.last, dt, shape);
            self.last += dt;
        }
    }

    fn tick(&mut self, from: f32, dt: f32, shape: &Shape) {
        for (s, sys) in SYSTEMS.iter().enumerate() {
            let open = ((from + dt).min(sys.stop) - from.max(sys.start)).max(0.0);
            self.owed[s] += sys.rate * open;
            while self.owed[s] >= 1.0 {
                self.owed[s] -= 1.0;
                let (pos, vel) = match sys.kind {
                    Kind::Spark => {
                        // At a corner of the box the path carries (its bottom at the path).
                        let p = shape.path_at(from / EMIT_SECONDS);
                        let c = BOX[((self.random() * 8.0) as usize).min(7)];
                        ([p[0] + c[0], p[1] + c[1], p[2] + c[2] - 7.5], [0.0; 3])
                    }
                    _ => {
                        let r = [self.random(), self.random(), self.random()];
                        let (p, n) = shape.body_point(r);
                        let v = (sys.speed + self.vary(sys.speed_var)).max(0.0);
                        (p, n.map(|x| x * v))
                    }
                };
                let sign = if self.random() < 0.5 { -1.0 } else { 1.0 };
                let p = Particle {
                    system: s,
                    pos,
                    vel,
                    age: 0.0,
                    life: (sys.life + self.vary(sys.life_var)).max(0.02),
                    radius: sys.radius + self.vary(sys.radius_var),
                    angle: 0.0,
                    spin: (sys.spin + self.vary(sys.spin_var)) * sign,
                };
                self.particles.push(p);
            }
        }
        for i in 0..self.particles.len() {
            let sys = &SYSTEMS[self.particles[i].system];
            let jitter = if sys.turbulence > 0.0 {
                [self.vary(2.0), self.vary(2.0), self.vary(2.0)].map(|j| j * sys.turbulence * dt)
            } else {
                [0.0; 3]
            };
            let p = &mut self.particles[i];
            p.vel[2] += sys.lift * dt;
            for k in 0..3 {
                p.vel[k] += jitter[k];
                p.pos[k] += p.vel[k] * dt;
            }
            p.age += dt;
            p.angle += p.spin * dt;
        }
        self.particles.retain(|p| p.age < p.life);
    }

    fn draw(&self, draw: &DrawListMut, camera: &Camera, art: &[TextureId], size: [f32; 2], t: f32) {
        let at = |p: [f32; 3]| {
            let m = metres(p);
            [self.feet[0] + m[0], self.feet[1] + m[1], self.feet[2] + m[2]]
        };
        let pixels = |z: f32| size[1] / 2.0 / (z * (camera.fov / 2.0).tan());
        let quad = |p: &Particle, texture: TextureId, half: f32, col: ImColor32| {
            let Some((centre, z)) = camera.project(at(p.pos), size) else { return };
            let half = half * UNIT * pixels(z);
            if half < 0.5 {
                return;
            }
            let (s, c) = p.angle.sin_cos();
            let corner = |dx: f32, dy: f32| [centre[0] + (dx * c - dy * s) * half, centre[1] + (dx * s + dy * c) * half];
            draw.add_image_quad(texture, corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0))
                .col(col)
                .build();
        };
        // The smoke (alpha-blended, lit: emissive 0.5 plus its colour key).
        for p in self.particles.iter().filter(|p| SYSTEMS[p.system].kind == Kind::Smoke) {
            let x = p.age / p.life;
            let k = if x < 0.2 { 0.18 + 0.42 * x / 0.2 } else { 0.6 - 0.42 * (x - 0.2) / 0.8 };
            let light = ((0.5 + k).min(1.0) * 255.0) as u8;
            quad(p, art[SYSTEMS[p.system].texture], p.radius, ImColor32::from_rgba(light, light, light, (k * 255.0) as u8));
        }
        // Everything else adds light, as in Wizard101 (ONE/ONE).
        unsafe { blend(true) };
        if t < EMIT_SECONDS {
            self.tubes(draw, camera, art[2], size, t / EMIT_SECONDS);
        }
        for pass in [Kind::Spark, Kind::Twinkle] {
            for p in self.particles.iter().filter(|p| SYSTEMS[p.system].kind == pass) {
                let sys = &SYSTEMS[p.system];
                let x = p.age / p.life;
                let mut scale = 1.0;
                if sys.fade > 0.0 {
                    scale = ((p.life - p.age) / sys.fade).min(1.0);
                }
                if sys.grow > 0.0 {
                    scale = (p.age / sys.grow).min(1.0);
                }
                let k = (keys(x) * 255.0) as u8;
                quad(p, art[sys.texture], p.radius * scale, ImColor32::from_rgba(k, k, k, 255));
            }
        }
        unsafe { blend(false) };
    }

    /// The tubes' bands, `phase` 0..1 through their scroll: texture v + phase (the bands come in at the bottom and
    /// leave at the top), times the side's vertex colour.
    fn tubes(&self, draw: &DrawListMut, camera: &Camera, texture: TextureId, size: [f32; 2], phase: f32) {
        let at = |p: [f32; 3]| {
            let m = metres(p);
            [self.feet[0] + m[0], self.feet[1] + m[1], self.feet[2] + m[2]]
        };
        const AROUND: usize = 24;
        const ROWS: usize = 24;
        for (radius, rings) in &TUBES {
            let top = rings[3].0;
            // Height -> (texture v, vertex colour), between the rings.
            let ring = |z: f32| {
                let i = rings.iter().position(|r| r.0 >= z).unwrap_or(3).max(1);
                let (a, b) = (rings[i - 1], rings[i]);
                let k = ((z - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
                (a.1 + k * (b.1 - a.1), a.2 + k * (b.2 - a.2))
            };
            for row in 0..ROWS {
                let (z0, z1) = (top * row as f32 / ROWS as f32, top * (row + 1) as f32 / ROWS as f32);
                let ((v0, _), (v1, _), (_, c)) = (ring(z0), ring(z1), ring((z0 + z1) / 2.0));
                let (v0, v1) = (v0 + phase, v1 + phase);
                // Past the texture's end (clamped in Wizard101: its dark last row): nothing.
                if c <= 0.0 || v0.min(v1) >= 1.0 {
                    continue;
                }
                let (v0, v1) = (v0.min(0.999), v1.min(0.999));
                let col = (c * 255.0) as u8;
                for k in 0..AROUND {
                    let (a0, a1) = (k as f32 / AROUND as f32 * std::f32::consts::TAU,
                                    (k + 1) as f32 / AROUND as f32 * std::f32::consts::TAU);
                    let corner = |a: f32, z: f32| camera.project(at([radius * a.cos(), radius * a.sin(), z]), size);
                    let (Some(p0), Some(p1), Some(p2), Some(p3)) =
                        (corner(a0, z0), corner(a1, z0), corner(a1, z1), corner(a0, z1)) else { continue };
                    let (u0, u1) = (k as f32 / AROUND as f32, (k + 1) as f32 / AROUND as f32);
                    draw.add_image_quad(texture, p0.0, p1.0, p2.0, p3.0)
                        .uv([u0, v0], [u1, v0], [u1, v1], [u0, v1])
                        .col(ImColor32::from_rgba(col, col, col, 255))
                        .build();
                }
            }
        }
    }
}

/// The sparks' colour keys: 1 at birth, 0.5 at 0.75 of their life, 0 at the end.
fn keys(x: f32) -> f32 {
    if x < 0.75 { 1.0 - 0.5 * x / 0.75 } else { 0.5 * (1.0 - x) / 0.25 }
}

/// The preview renderer: the live poof `t` seconds in, on feet at `feet` seen by `camera` (no game needed).
#[allow(dead_code)]
pub fn preview(ui: &Ui, textures: &mut Textures, shape: Option<&std::path::Path>, camera: &Camera, feet: [f32; 3], t: f32) -> bool {
    textures.want(ART.iter().map(|s| s.to_string()));
    let art: Vec<TextureId> = ART.iter().filter_map(|n| textures.get(n).map(|t| t.0)).collect();
    if art.len() != ART.len() {
        return false;
    }
    let shape = shape.and_then(Shape::load).unwrap_or_default();
    let mut poof = Poof::new(feet, None, POOF_LIVE);
    poof.step(t, &shape);
    poof.draw(&ui.get_foreground_draw_list(), camera, &art, ui.io().display_size, t);
    true
}
