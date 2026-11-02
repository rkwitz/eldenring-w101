//! Wizard101's health and mana wisps on Unicorn Way (round 21 M3).
//!
//! In Wizard101 a won fight can leave wisps behind (Root.wad ObjectData/CombatBonus_WispHealth.xml, red, and
//! CombatBonus_WispMana.xml, blue): small glowing clouds with sparkles over a coloured glow on the ground
//! (_Shared-WorldData Character/WilOTheWisps/FX_Wisp_Red.nif / FX_Wisp.nif + GlowShadow_Red/Blue.nif) that wander
//! about (WanderMovementBehavior), vanish after a while (DespawnTimedBehavior), and restore health / mana when walked
//! into (InteractWispAddHealth / InteractWispAddMana) with a burst (FX_WispRed/Blue_Dsppr.nif) and the sound
//! Sound/GUI/ui_health_powerup_01.wav.
//!
//! Here: when a Unicorn Way creature (nameplates::CREATURES) dies near the player on the street, a wisp may appear
//! where it fell (ini wisp_chance, percent, default 50; red or blue at random). It is drawn from the wisp nif's own
//! textures (tools/w101_wisp_art.py -> w101_orbs/wisps/), projected with the markers' camera (markers::Camera::project)
//! like the quest markers and nameplates, bobbing and drifting round where it fell. Walking within PICKUP metres
//! restores wisp_restore percent (default 20) of the player's max HP (red) or FP (blue), written to the player's
//! CSChrDataModule (Game::add_player_stat), plays the W101 sound, and bursts. ini wisps = 0 turns them off.

use std::path::PathBuf;
use std::time::Instant;

use hudhook::imgui::{DrawListMut, Ui};

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
/// Creatures that die farther away than this leave nothing (another player's fight, an unloaded corner).
const DROP_RANGE: f32 = 40.0;
/// Cloud centre over the ground, bob amplitude and period, wander radius and speed (metres, seconds).
const HOVER: f32 = 0.9;
const BOB: f32 = 0.12;
const BOB_PERIOD: f32 = 2.2;
const WANDER: f32 = 1.2;
const WANDER_SPEED: f32 = 0.25;
/// Sizes (metres): cloud puff, sparkle, ground glow.
const PUFF: f32 = 0.55;
const SPARKLE: f32 = 0.16;
const GROUND: f32 = 1.1;
/// The burst after a pick-up, and the fade at the end of a wisp's life, seconds.
const BURST: f32 = 0.6;
const FADE: f32 = 2.0;
/// No more than this many wisps at once (oldest go first).
const MAX_WISPS: usize = 12;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Health,
    Mana,
}

struct Wisp {
    kind: Kind,
    /// Where it fell (physics space, feet).
    home: [f32; 3],
    born: Instant,
    seed: f32,
    /// Set when picked up: the burst plays from here.
    picked: Option<Instant>,
}

pub struct Wisps {
    folder: PathBuf,
    enabled: bool,
    chance: f32,
    restore: f32,
    lifetime: f32,
    wisps: Vec<Wisp>,
    /// Last frame's creatures: (ChrIns, HP).
    last: Vec<(usize, i32)>,
    rng: u64,
    wanted: bool,
}

impl Wisps {
    pub fn new(folder: &std::path::Path) -> Wisps {
        let mut w = Wisps {
            folder: folder.to_path_buf(),
            enabled: true,
            chance: 0.5,
            restore: 20.0,
            lifetime: 60.0,
            wisps: Vec::new(),
            last: Vec::new(),
            rng: 0x9E37_79B9_7F4A_7C15 ^ std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64),
            wanted: false,
        };
        let ini = folder.join("w101_orbs.ini");
        for line in std::fs::read_to_string(ini).unwrap_or_default().lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            let num = value.parse::<f32>().ok();
            match key.trim() {
                "wisps" => w.enabled = value != "0",
                "wisp_chance" => w.chance = num.map_or(w.chance, |v| v.clamp(0.0, 100.0) / 100.0),
                "wisp_restore" => w.restore = num.map_or(w.restore, |v| v.clamp(1.0, 100.0)),
                "wisp_lifetime" => w.lifetime = num.map_or(w.lifetime, |v| v.clamp(5.0, 600.0)),
                _ => {}
            }
        }
        tracing::info!("wisps: {} (drop chance {:.0}%, restore {:.0}% of max, lifetime {:.0} s)",
                       if w.enabled { "on" } else { "off" }, w.chance * 100.0, w.restore, w.lifetime);
        w
    }

    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Every frame: drops for creatures that died, pick-ups, drawing. `gameplay` = normal play with the HUD up;
    /// `street` = the player is on Unicorn Way's tile (questhelper::player_msb).
    pub fn frame(&mut self, ui: &Ui, textures: &mut Textures, game: Option<&Game>, gameplay: bool, street: bool) {
        let (Some(g), true, true) = (game, self.enabled, street) else {
            self.wisps.clear();
            self.last.clear();
            return;
        };
        if !self.wanted {
            self.wanted = true;
            textures.want(CLOUD.iter().chain(SPARK.iter()).chain([GLOW].iter()).map(|n| format!("{DIR}{n}")));
        }
        let Some(camera) = g.camera() else { return };
        let player = camera.player;
        let now = Instant::now();
        // Deaths: a creature alive last frame at 0 HP now.
        let creatures = g.combatants(&crate::nameplates::models());
        for c in &creatures {
            let died = c.3 == 0 && self.last.iter().any(|l| l.0 == c.0 && l.1 > 0);
            let d = ((c.2[0] - player[0]).powi(2) + (c.2[2] - player[2]).powi(2)).sqrt();
            if died && d < DROP_RANGE {
                let roll = self.random();
                if roll < self.chance {
                    let kind = if self.random() < 0.5 { Kind::Health } else { Kind::Mana };
                    let seed = self.random() * 100.0;
                    tracing::info!("wisps: {kind:?} wisp where a creature (model {}) fell, {d:.0} m away", c.1);
                    if self.wisps.len() >= MAX_WISPS {
                        self.wisps.remove(0);
                    }
                    self.wisps.push(Wisp { kind, home: c.2, born: now, seed, picked: None });
                } else {
                    tracing::info!("wisps: creature (model {}) fell, no wisp (roll {:.2})", c.1, roll);
                }
            }
        }
        self.last = creatures.iter().map(|c| (c.0, c.3)).collect();
        // Pick-ups and expiry.
        let lifetime = self.lifetime;
        self.wisps.retain(|w| match w.picked {
            Some(t) => (now - t).as_secs_f32() < BURST,
            None => (now - w.born).as_secs_f32() < lifetime,
        });
        let time = ui.time() as f32;
        for w in &mut self.wisps {
            if w.picked.is_some() || !gameplay {
                continue;
            }
            let p = position(w, time);
            let d = ((p[0] - player[0]).powi(2) + (p[2] - player[2]).powi(2)).sqrt();
            if d < PICKUP && (p[1] - HOVER - player[1]).abs() < PICKUP_HEIGHT {
                let fp = w.kind == Kind::Mana;
                match g.add_player_stat(fp, self.restore) {
                    Some((from, to)) => {
                        tracing::info!("wisps: picked up a {:?} wisp, {} {from} -> {to}", w.kind, if fp { "FP" } else { "HP" });
                        w.picked = Some(now);
                        play_sound(&self.folder.join(SOUND));
                    }
                    None => continue,
                }
            }
        }
        if !gameplay {
            return;
        }
        let draw = ui.get_background_draw_list();
        let screen = ui.io().display_size;
        // Far ones first so near ones draw over them.
        let mut order: Vec<(f32, usize)> = self.wisps.iter().enumerate()
            .filter_map(|(i, w)| camera.project(position(w, time), screen).map(|(_, z)| (z, i))).collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, i) in order {
            draw_wisp(&draw, textures, &camera, &self.wisps[i], time, now, lifetime, screen);
        }
    }
}

/// The cloud's centre now: home + wander drift + bob, HOVER over the ground.
fn position(w: &Wisp, time: f32) -> [f32; 3] {
    let t = time * WANDER_SPEED / WANDER + w.seed;
    let (dx, dz) = (WANDER * (t.sin() * 0.7 + (t * 1.7).sin() * 0.3), WANDER * ((t * 1.3).cos() * 0.6 + (t * 0.6).sin() * 0.4));
    let bob = BOB * (time * std::f32::consts::TAU / BOB_PERIOD + w.seed).sin();
    [w.home[0] + dx * 0.6, w.home[1] + HOVER + bob, w.home[2] + dz * 0.6]
}

#[allow(clippy::too_many_arguments)]
fn draw_wisp(draw: &DrawListMut, textures: &Textures, camera: &Camera, w: &Wisp, time: f32, now: Instant,
             lifetime: f32, screen: [f32; 2]) {
    let k = (w.kind == Kind::Mana) as usize;
    let (Some((cloud, _)), Some((spark, _)), Some((glow, _))) = (textures.get(&format!("{DIR}{}", CLOUD[k])),
        textures.get(&format!("{DIR}{}", SPARK[k])), textures.get(&format!("{DIR}{GLOW}"))) else { return };
    let tint = if k == 0 { [1.0, 0.35, 0.3] } else { [0.35, 0.55, 1.0] };
    let age = (now - w.born).as_secs_f32();
    let mut alpha = (age / 0.4).min(1.0) * ((lifetime - age) / FADE).clamp(0.0, 1.0);
    let mut grow = 1.0;
    if let Some(t) = w.picked {
        let f = (now - t).as_secs_f32() / BURST;
        grow = 1.0 + 1.5 * f;
        alpha *= (1.0 - f).max(0.0);
    }
    let centre = position(w, time);
    let Some((c, z)) = camera.project(centre, screen) else { return };
    let px = |metres: f32| metres * screen[1] / (2.0 * z * (camera.fov / 2.0).tan());
    // Ground glow (GlowShadow): flattened, under the wisp's drift point, not bobbing.
    let ground = [centre[0], w.home[1] + 0.05, centre[2]];
    if let Some((g, _)) = camera.project(ground, screen) {
        let (rw, rh) = (px(GROUND) / 2.0, px(GROUND) / 2.0 * 0.35);
        draw.add_image(glow, [g[0] - rw, g[1] - rh], [g[0] + rw, g[1] + rh])
            .col([tint[0], tint[1], tint[2], 0.45 * alpha]).build();
    }
    // Cloud: three puffs turning at their own speeds round the centre (FX_Cloud Main / Add).
    for i in 0..3 {
        let f = i as f32;
        let turn = time * (0.6 + 0.35 * f) * if i % 2 == 0 { 1.0 } else { -1.0 } + f * 2.1 + w.seed;
        let off = [px(0.07) * (turn * 0.7).cos(), px(0.05) * (turn * 0.9).sin()];
        let r = px(PUFF) / 2.0 * grow * (1.0 - 0.15 * f);
        let corner = |a: f32| [c[0] + off[0] + r * 1.414 * a.cos(), c[1] + off[1] + r * 1.414 * a.sin()];
        let q = std::f32::consts::FRAC_PI_4;
        let a = [corner(turn - 3.0 * q), corner(turn - q), corner(turn + q), corner(turn + 3.0 * q)];
        draw.add_image_quad(cloud, a[0], a[1], a[2], a[3]).col([1.0, 1.0, 1.0, alpha * (0.95 - 0.2 * f)]).build();
    }
    // Sparkles orbiting and twinkling (FX_SparkleBlue / FX_FocusSpark emitters).
    for i in 0..4 {
        let f = i as f32;
        let a = time * 1.4 + f * std::f32::consts::FRAC_PI_2 + w.seed;
        let rise = ((time * 0.8 + f * 0.37 + w.seed).fract() - 0.5) * 0.5;
        let p = [centre[0] + 0.32 * grow * a.cos(), centre[1] + rise, centre[2] + 0.32 * grow * a.sin()];
        let Some((s, sz)) = camera.project(p, screen) else { continue };
        let r = SPARKLE * screen[1] / (2.0 * sz * (camera.fov / 2.0).tan()) / 2.0;
        let twinkle = 0.5 + 0.5 * (time * 6.0 + f * 1.9 + w.seed).sin();
        draw.add_image(spark, [s[0] - r, s[1] - r], [s[0] + r, s[1] + r]).col([1.0, 1.0, 1.0, alpha * twinkle]).build();
    }
}

/// Wizard101's pick-up sound, asynchronously (winmm PlaySound; the music thread's waveOut is separate).
fn play_sound(path: &std::path::Path) {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};
    if !path.is_file() {
        tracing::warn!("wisps: {} is missing (run tools/w101_wisp_art.py)", path.display());
        return;
    }
    let wide: Vec<u16> = path.as_os_str().to_string_lossy().encode_utf16().chain([0]).collect();
    unsafe { PlaySoundW(wide.as_ptr(), std::ptr::null_mut(), SND_ASYNC | SND_FILENAME | SND_NODEFAULT) };
}
