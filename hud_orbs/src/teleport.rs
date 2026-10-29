//! Wizard101's teleport "poof" on the player before a warp (Ryan, 2026-10-29: "the sparkles ... where the player is,
//! and they poof and disappear ... before the actual teleport happens").
//!
//! Wizard101 plays _Shared-WorldData.wad Character/FX_Disappear.nif (the client's m_poofEffectAsset) on a character
//! that teleports away; the character has no animation of its own and just vanishes inside it. The effect, from the
//! .nif (Gamebryo particle systems, all emitting from a body-sized mesh for the first 0.367 s):
//!   - FX_CastSignSpark / FX_CastSignSpark01: gold and blue-white four-point stars (FX_DisappearSparks._2 / ._1),
//!     272.7 a second each, standing still where born and pulled upward (two +Z gravity fields, 90 + 180 units/s²),
//!     living 0.533 s (variation 0.2), shrinking away over their last 0.167 s;
//!   - FX_StunSparkle / FX_StunSparkle01: the same stars as quick twinkles, 409 / 272.7 a second, flung out at
//!     0-600 units/s and gone after 0.1 s;
//!   - Cylinder01 / Cylinder02: two open tubes (radius 89 / 69, height 229 / 275 units) round the body, textured
//!     FX_LeaveToob_01 (bands of orange and blue light) scrolling once up over 0.367 s;
//!   - FX_Smoke: 20 soft grey puffs (FX_SmokeChimney, radius 100) born at 0.267-0.367 s, drifting up (180 units/s²)
//!     for 1.33 s, alpha 0.18 -> 0.6 -> 0.18.
//!   All the sparks' colour keys run white -> half -> nothing over their life.
//! Its textures come from tools/w101_teleport_art.py (book\art\teleport\*.png); without them nothing is drawn.
//!
//! Two triggers:
//!   - the spellbook's map buttons: their events (tools/emevd.py book_warps) freeze the player, set POOF_FLAG, play
//!     Elden Ring's Unseen Form burst on him and fade him out, and only 1.2 s later bring up the load screen and warp.
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
const POOF_LIVE: f32 = 1.6;
/// A world copy older than this is not used (seconds).
const SHOT_MAX_AGE: f32 = 600.0;
/// After a book warp's POOF_FLAG, Elden Ring's loading screens are that warp's (seconds).
const BOOK_WINDOW: f32 = 20.0;
/// Metres per Wizard101 unit for the effect: its emitter mesh (z 10..330 units) spans the player's 1.9 m.
const UNIT: f32 = 1.9 / 320.0;

/// Wizard101's emitter shape, in metres round the feet: radius and height span of the body mesh.
const BODY_RADIUS: f32 = 97.0 * UNIT;
const BODY_LOW: f32 = 10.0 * UNIT;
const BODY_HIGH: f32 = 330.0 * UNIT;
const EMIT_SECONDS: f32 = 0.367;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// Rising star (CastSignSpark).
    Spark,
    /// Quick twinkle (StunSparkle).
    Twinkle,
    Smoke,
}

/// (kind, texture index in ART, births a second, radius in units, life, life variation, speed, speed variation,
/// upward acceleration in units/s², emitting from, emitting until): FX_Disappear.nif's systems.
const SYSTEMS: [(Kind, usize, f32, f32, f32, f32, f32, f32, f32, f32, f32); 5] = [
    (Kind::Spark, 1, 272.7, 15.0 * 1.95, 0.533, 0.2, 0.0, 0.0, 270.0, 0.0, EMIT_SECONDS),
    (Kind::Spark, 0, 272.7, 15.0 * 1.95, 0.533, 0.2, 0.0, 0.0, 270.0, 0.0, EMIT_SECONDS),
    (Kind::Twinkle, 0, 409.1, 20.0, 0.1, 0.0, 300.0, 300.0, 0.0, 0.0, EMIT_SECONDS),
    (Kind::Twinkle, 1, 272.7, 15.0, 0.1, 0.0, 300.0, 300.0, 0.0, 0.0, EMIT_SECONDS),
    (Kind::Smoke, 3, 200.0, 100.0, 1.333, 0.0, 0.0, 0.0, 180.0, 0.267, EMIT_SECONDS),
];
/// The two tubes: radius, height (units).
const TUBES: [(f32, f32); 2] = [(89.0, 229.0), (69.0, 275.0)];

struct Particle {
    system: usize,
    pos: [f32; 3],
    vel: [f32; 3],
    age: f32,
    life: f32,
    angle: f32,
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
    wanted: bool,
    missing_logged: bool,
}

impl Teleport {
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
        }
        if game.is_some() && crate::game::event_flag(POOF_FLAG) == Some(true) {
            crate::game::set_event_flag(POOF_FLAG, false);
            self.book_at = Some(Instant::now());
            match (ready, game.and_then(|g| g.camera())) {
                (true, Some(camera)) => {
                    tracing::info!("teleport: book warp poof at {:?}", camera.player);
                    self.poof = Some(Poof::new(camera.player, None, POOF_LIVE));
                }
                _ => self.log_missing(ready),
            }
        }
        let mut out = loading;
        if loading && !door && self.poof.is_none() {
            let book = self.book_at.is_some_and(|t| t.elapsed().as_secs_f32() < BOOK_WINDOW);
            // Used once: the next warp needs a copy of the world it starts from (after loading a save, none yet).
            let shot = self.shot.take().filter(|(_, at)| at.elapsed().as_secs_f32() < SHOT_MAX_AGE);
            match (book, self.alive, shot, ready, textures.get(WORLD)) {
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
                poof.step(t);
                if let (Some(camera), true) = (camera, ready) {
                    let art: Vec<TextureId> = art.into_iter().flatten().collect();
                    poof.draw(&draw, &camera, &art, size, t);
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

    /// Births, moves and ages the particles up to `t` seconds.
    fn step(&mut self, t: f32) {
        let dt = (t - self.last).clamp(0.0, 0.1);
        let from = self.last;
        self.last = t;
        for (s, &(_, _, rate, _, life, life_var, speed, speed_var, _, start, stop)) in SYSTEMS.iter().enumerate() {
            let open = (t.min(stop) - from.max(start)).max(0.0);
            self.owed[s] += rate * open;
            while self.owed[s] >= 1.0 {
                self.owed[s] -= 1.0;
                // A point in the body's volume, flung outward (the mesh's normals) and a little up.
                let a = self.random() * std::f32::consts::TAU;
                let r = BODY_RADIUS * self.random().sqrt();
                let y = BODY_LOW + (BODY_HIGH - BODY_LOW) * self.random();
                let v = (speed + speed_var * (self.random() - 0.5) * 2.0).max(0.0) * UNIT;
                let up = self.random() - 0.3;
                let p = Particle {
                    system: s,
                    pos: [r * a.cos(), y, r * a.sin()],
                    vel: [v * a.cos(), v * up, v * a.sin()],
                    age: 0.0,
                    life: (life + life_var * (self.random() - 0.5)).max(0.05),
                    angle: self.random() * std::f32::consts::TAU,
                };
                self.particles.push(p);
            }
        }
        for p in &mut self.particles {
            let lift = SYSTEMS[p.system].8 * UNIT;
            p.vel[1] += lift * dt;
            for k in 0..3 {
                p.pos[k] += p.vel[k] * dt;
            }
            p.age += dt;
        }
        self.particles.retain(|p| p.age < p.life);
    }

    fn draw(&self, draw: &DrawListMut, camera: &Camera, art: &[TextureId], size: [f32; 2], t: f32) {
        let at = |p: [f32; 3]| [self.feet[0] + p[0], self.feet[1] + p[1], self.feet[2] + p[2]];
        let pixels = |z: f32| size[1] / 2.0 / (z * (camera.fov / 2.0).tan());
        // The tubes: bands of light running up them once over the emitting time.
        if t < EMIT_SECONDS {
            let phase = t / EMIT_SECONDS;
            const AROUND: usize = 16;
            const ROWS: usize = 12;
            for &(radius, height) in &TUBES {
                let (radius, height) = (radius * UNIT, height * UNIT);
                for row in 0..ROWS {
                    let (f0, f1) = (row as f32 / ROWS as f32, (row + 1) as f32 / ROWS as f32);
                    // The texture's bands (its lower half, v 0.5..1) start just under the tube and leave at the top.
                    let v = |f: f32| 1.0 - (f - (2.0 * phase - 1.0)) * 0.5;
                    let (v0, v1) = (v(f0), v(f1));
                    if !(0.0..=1.0).contains(&v0) || !(0.0..=1.0).contains(&v1) {
                        continue;
                    }
                    for k in 0..AROUND {
                        let (a0, a1) = (k as f32 / AROUND as f32 * std::f32::consts::TAU,
                                        (k + 1) as f32 / AROUND as f32 * std::f32::consts::TAU);
                        let corner = |a: f32, f: f32| camera.project(at([radius * a.cos(), f * height, radius * a.sin()]), size);
                        let (Some(p0), Some(p1), Some(p2), Some(p3)) =
                            (corner(a0, f0), corner(a1, f0), corner(a1, f1), corner(a0, f1)) else { continue };
                        let (u0, u1) = (k as f32 / AROUND as f32, (k + 1) as f32 / AROUND as f32);
                        draw.add_image_quad(art[2], p0.0, p1.0, p2.0, p3.0)
                            .uv([u0, v0], [u1, v0], [u1, v1], [u0, v1])
                            .col(ImColor32::from_rgba(255, 255, 255, 200))
                            .build();
                    }
                }
            }
        }
        // Smoke under the sparks, then the sparks; each a camera-facing square.
        for pass in [Kind::Smoke, Kind::Spark, Kind::Twinkle] {
            for p in self.particles.iter().filter(|p| SYSTEMS[p.system].0 == pass) {
                let (kind, texture, _, radius, ..) = SYSTEMS[p.system];
                let Some((centre, z)) = camera.project(at(p.pos), size) else { continue };
                let x = p.age / p.life;
                let (scale, alpha) = match kind {
                    // Colour keys 0: 1, 0.75: 0.5, 1: 0 (white to nothing; additive in Wizard101, so brightness =
                    // alpha here); shrinking over the last 0.167 s.
                    Kind::Spark => ((p.life - p.age).min(0.167) / 0.167, keys(x)),
                    // Growing over the first 0.033 s.
                    Kind::Twinkle => ((p.age / 0.033).min(1.0), keys(x)),
                    // 0.18 -> 0.6 at 0.2 -> 0.18, and out over its last tenth so it doesn't pop.
                    Kind::Smoke => (1.0, if x < 0.2 { 0.18 + 0.42 * x / 0.2 } else { 0.6 - 0.42 * (x - 0.2) / 0.8 }
                                         * ((1.0 - x) / 0.1).min(1.0)),
                };
                let half = radius * UNIT * scale * pixels(z);
                if half < 0.5 || alpha <= 0.0 {
                    continue;
                }
                let (s, c) = p.angle.sin_cos();
                let corner = |dx: f32, dy: f32| [centre[0] + (dx * c - dy * s) * half, centre[1] + (dx * s + dy * c) * half];
                draw.add_image_quad(art[texture], corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0))
                    .col(ImColor32::from_rgba(255, 255, 255, (alpha.clamp(0.0, 1.0) * 255.0) as u8))
                    .build();
            }
        }
    }
}

/// The sparks' colour keys: 1 at birth, 0.5 at 0.75 of their life, 0 at the end.
fn keys(x: f32) -> f32 {
    if x < 0.75 { 1.0 - 0.5 * x / 0.75 } else { 0.5 * (1.0 - x) / 0.25 }
}
