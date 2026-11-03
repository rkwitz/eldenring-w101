//! Round 30 (FT1-FT5, Ryan's Wizard101 screenshots round30/ref): Wizard101's floating numbers over heads.
//! - FT1 a wisp picked up: "+N" (the HP or FP it gave) over the player, green for health (ref 1 "+110").
//! - FT2 damage dealt: the HP an enemy lost, red, with Wizard101's damage fist after it (ref 2 "105").
//! - FT3 damage taken: the same over the player.
//! - FT4 an enemy killed: Wizard101's gold reward circle round Elden Ring's rune (the quest cards' rune piece, made
//!   from Art_Quest_Gold, the very circle of ref 3's "2 GOLD") and "N RUNES", the runes that kill gave; no XP line.
//! - FT5 a boss (a creature with a Boss title, nameplates::CREATURES: Rattlebones) killed: "VICTORY!" over the player
//!   above the runes, as in ref 3.
//!
//! What Wizard101's data gives (searched Root.wad and _Shared-WorldData.wad): the words, CombatMessages.lang
//! "Victory" -> "Victory!" (shown in capitals, ref 3) and "UpdateXP" -> "%d XP"; the damage icon,
//! GUI/Icons/Type_Damage.dds (the fist of ref 2, 32x32); the gold circle, GUI/QuestPage/Art_Quest_Gold.dds; the
//! font family, WizardFancy (WizardPopupText.gui's ControlText: WizardFancy, drop shadow, white tinted by the client).
//! There is no .gui, effect or setting for the floating numbers (the client draws them in code), so their colours,
//! outline, size, rise and fade are matched to the screenshots: colours sampled from the letters (flat, no gradient:
//! the VICTORY! rows all average 188,244,241), sizes from their height against the nameplate's capitals, and the rise
//! and fade (not measurable from still pictures) are RISE / HOLD / FADE below.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use hudhook::imgui::{DrawListMut, ImColor32};

use crate::font::BitmapFont;
use crate::layout::Textures;
use crate::markers::Camera;

/// ref 1 "+110": green letters with a dark green edge.
const HEAL: ([u8; 3], Option<[u8; 3]>) = ([24, 216, 12], Some([0, 40, 0]));
/// No mana reference picture: Wizard101's blue of its mana globe text, with a dark blue edge (not from a screenshot).
const MANA: ([u8; 3], Option<[u8; 3]>) = ([40, 130, 255], Some([0, 16, 64]));
/// ref 2 "105": red with a darker red edge.
const DAMAGE: ([u8; 3], Option<[u8; 3]>) = ([214, 72, 64], Some([96, 48, 44]));
/// ref 3: "2 GOLD" yellow and "VICTORY!" pale cyan, no edge.
const GOLD: ([u8; 3], Option<[u8; 3]>) = ([196, 196, 12], None);
const VICTORY: ([u8; 3], Option<[u8; 3]>) = ([188, 246, 242], None);

/// Font scale of the numbers, times the nameplate's size factor: their capitals are 1.8 x the nameplate name's (ref 2:
/// 25 px "105" over a 14 px "DARK FAIRY", whose WizardFancy scale is 0.5 here).
const SCALE: f32 = 0.9;
/// "VICTORY!" is 26 px against 30 px "2 GOLD" (ref 3).
const VICTORY_SCALE: f32 = 0.78;
/// Icons against the capitals' height: the fist is 40 px by 25 px digits (ref 2), the gold circle 63 by 30 (ref 3).
const FIST: f32 = 1.6;
const COIN: f32 = 2.1;
/// The capitals' height as a share of WizardFancy's line height (measured on the preview render).
const CAP: f32 = 0.62;
/// Layout pixels (768-high screen, times the size factor) it rises over its life, and its life: full for HOLD
/// seconds, then fading out over FADE. Not in Wizard101's data (see above).
/// Round 31 (FT6, Ryan: "rise faster; the fade time is fine"): 40 -> 100 over the same life.
const RISE: f32 = 100.0;
const HOLD: f32 = 1.2;
const FADE: f32 = 0.8;
const VICTORY_HOLD: f32 = 2.0;
const VICTORY_FADE: f32 = 1.0;
/// Floaters on one head that came up within this many seconds of each other stack one line apart (ref 3).
const STACK: f32 = 0.6;
/// Over the player: the top of an Elden Ring character (1.75 m). Round 31 (FT6): the head itself, no plate gap.
const PLAYER_TOP: f32 = 1.75;
/// Runes gained this soon after an enemy died are that kill's.
const KILL_WINDOW: f32 = 3.0;
/// Enemies farther than this (metres) show no damage numbers (the nameplates' own range is near this).
const RANGE: f32 = 40.0;

pub const FIST_ART: &str = "GUI/Icons/Type_Damage.png";
pub const RUNE_ART: &str = crate::pages::REWARD_RUNES;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Heal(i32),
    Mana(i32),
    Damage(i32),
    Runes(u32),
    Victory,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Over {
    Player,
    /// An enemy (its ChrIns address) and the last place over its head (physics space).
    Enemy(usize, [f32; 3]),
}

struct Floater {
    kind: Kind,
    over: Over,
    born: Instant,
    row: f32,
}

/// Wisp pickups (wisps.rs), taken in by the next frame.
static WISPS: Mutex<Vec<Kind>> = Mutex::new(Vec::new());

/// wisps.rs: a wisp gave `amount` HP (or FP when `fp`).
pub fn wisp(fp: bool, amount: i32) {
    if amount > 0 {
        WISPS.lock().unwrap_or_else(|e| e.into_inner()).push(if fp { Kind::Mana(amount) } else { Kind::Heal(amount) });
    }
}

#[derive(Default)]
pub struct Floaters {
    live: Vec<Floater>,
    player_hp: Option<(i32, i32)>,
    runes: Option<u32>,
    enemy_hp: HashMap<usize, i32>,
    last_kill: Option<Instant>,
}

impl Floaters {
    fn add(&mut self, kind: Kind, over: Over, now: Instant) {
        let same = |a: Over, b: Over| match (a, b) {
            (Over::Player, Over::Player) => true,
            (Over::Enemy(x, _), Over::Enemy(y, _)) => x == y,
            _ => false,
        };
        let row = self.live.iter().filter(|f| same(f.over, over) && (now - f.born).as_secs_f32() < STACK).count() as f32;
        tracing::info!("floaters: {kind:?} over {}", if over == Over::Player { "the player".to_string() } else { "an enemy".into() });
        self.live.push(Floater { kind, over, born: now, row });
    }

    /// Reads what changed since the last frame: the player's HP (FT3), the runes held (FT4), the creatures' HP (FT2,
    /// kills, FT5 for a boss) and the wisps picked up (FT1). `creatures`: Game::combatants (ChrIns, model, physics
    /// position, HP); `anchor`: each living enemy's head top (nameplates::head, physics space) by model and position.
    pub fn observe(&mut self, hp: Option<(i32, i32)>, runes: Option<u32>, creatures: &[(usize, u32, [f32; 3], i32)],
                   anchor: &dyn Fn(u32, [f32; 3]) -> Option<[f32; 3]>, player: [f32; 3]) {
        let now = Instant::now();
        for kind in std::mem::take(&mut *WISPS.lock().unwrap_or_else(|e| e.into_inner())) {
            self.add(kind, Over::Player, now);
        }
        if let (Some((last, last_max)), Some((hp, max))) = (self.player_hp, hp) {
            if hp < last && max == last_max {
                self.add(Kind::Damage(last - hp), Over::Player, now);
            }
        }
        self.player_hp = hp;
        let mut seen = HashMap::new();
        for &(chr, model, p, hp) in creatures {
            seen.insert(chr, hp);
            let Some(&last) = self.enemy_hp.get(&chr) else { continue };
            let d = ((p[0] - player[0]).powi(2) + (p[1] - player[1]).powi(2) + (p[2] - player[2]).powi(2)).sqrt();
            if hp >= last || d > RANGE {
                continue;
            }
            let at = self.live.iter().rev().find_map(|f| match f.over {
                Over::Enemy(c, at) if c == chr => Some(at),
                _ => None,
            });
            if let Some(at) = anchor(model, p).or(at) {
                self.add(Kind::Damage(last - hp), Over::Enemy(chr, at), now);
            }
            if hp <= 0 && last > 0 {
                self.last_kill = Some(now);
                let boss = crate::nameplates::CREATURES.iter()
                    .any(|c| c.0 == model && matches!(c.4, crate::nameplates::Title::Boss));
                tracing::info!("floaters: c{model} killed{}", if boss { " (a boss: VICTORY!)" } else { "" });
                if boss {
                    self.add(Kind::Victory, Over::Player, now);
                }
            }
        }
        self.enemy_hp = seen;
        if let (Some(last), Some(runes)) = (self.runes, runes) {
            if runes > last && self.last_kill.is_some_and(|t| (now - t).as_secs_f32() < KILL_WINDOW) {
                self.add(Kind::Runes(runes - last), Over::Player, now);
            }
        }
        self.runes = runes;
        // The enemies' anchors follow them while they live.
        for f in &mut self.live {
            if let Over::Enemy(chr, at) = &mut f.over {
                if let Some(&(_, model, p, _)) = creatures.iter().find(|c| c.0 == *chr && c.3 > 0) {
                    if let Some(a) = anchor(model, p) {
                        *at = a;
                    }
                }
            }
        }
    }

    /// Preview: floaters (kind, over the player or the enemy anchor `enemy`, seconds since they came up).
    #[allow(dead_code)]
    pub fn preview(&mut self, list: &[(Kind, Option<[f32; 3]>, f32)]) {
        let now = Instant::now();
        self.live.clear();
        for (i, (kind, enemy, age)) in list.iter().enumerate() {
            let over = enemy.map_or(Over::Player, |a| Over::Enemy(1 + i, a));
            let row = list[..i].iter().filter(|o| o.1.is_some() == enemy.is_some() && (o.2 - age).abs() < STACK).count() as f32;
            self.live.push(Floater { kind: *kind, over, born: now - std::time::Duration::from_secs_f32(*age), row });
        }
    }

    pub fn art() -> [String; 2] {
        [FIST_ART.to_string(), RUNE_ART.to_string()]
    }

    /// Draws the live floaters over their heads; `visible` false only lets time run.
    pub fn draw(&mut self, draw: &DrawListMut, textures: &Textures, fonts: &HashMap<String, BitmapFont>, camera: &Camera,
                screen: [f32; 2], visible: bool) {
        let now = Instant::now();
        let life = |k: Kind| if k == Kind::Victory { VICTORY_HOLD + VICTORY_FADE } else { HOLD + FADE };
        self.live.retain(|f| (now - f.born).as_secs_f32() < life(f.kind));
        let Some(font) = fonts.get("WizardFancy").filter(|_| visible) else { return };
        // Farther ones first.
        let mut order: Vec<(f32, usize)> = Vec::new();
        for (i, f) in self.live.iter().enumerate() {
            let at = match f.over {
                Over::Player => [camera.player[0], camera.player[1] + PLAYER_TOP, camera.player[2]],
                Over::Enemy(_, at) => at,
            };
            if let Some((_, z)) = camera.project(at, screen) {
                order.push((z, i));
            }
        }
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, i) in order {
            let f = &self.live[i];
            let at = match f.over {
                Over::Player => [camera.player[0], camera.player[1] + PLAYER_TOP, camera.player[2]],
                Over::Enemy(_, at) => at,
            };
            let Some(([x, y], z)) = camera.project(at, screen) else { continue };
            let age = (now - f.born).as_secs_f32();
            let (hold, fade) = if f.kind == Kind::Victory { (VICTORY_HOLD, VICTORY_FADE) } else { (HOLD, FADE) };
            let alpha = if age < hold { 1.0 } else { (1.0 - (age - hold) / fade).clamp(0.0, 1.0) };
            // The nameplates' size factor (nameplates::plates).
            let k = (screen[1] / 768.0) * (8.0 / z.max(1.0)).clamp(0.7, 1.15);
            let (text, (colour, edge), scale, icon) = match f.kind {
                Kind::Heal(n) => (format!("+{n}"), HEAL, SCALE, None),
                Kind::Mana(n) => (format!("+{n}"), MANA, SCALE, None),
                Kind::Damage(n) => (n.to_string(), DAMAGE, SCALE, Some((FIST_ART, FIST, false))),
                Kind::Runes(n) => (format!("{n} RUNES"), GOLD, SCALE, Some((RUNE_ART, COIN, true))),
                Kind::Victory => ("VICTORY!".to_string(), VICTORY, VICTORY_SCALE, None),
            };
            let sc = scale * k;
            let line = font.line_height * SCALE * k;
            let cap = font.line_height * sc * CAP;
            let text_w = font.width(&text) * sc;
            let side = icon.map_or(0.0, |(_, f, _)| cap * f);
            let gap = if icon.is_some() { cap * 0.2 } else { 0.0 };
            let total = text_w + gap + side;
            // Round 31 (FT6): its bottom starts on the character's head top (nameplates::head, PLAYER_TOP), one
            // line lower per stacked row, and rises RISE over its life (round 30: its top under the plate).
            let top = y - line + f.row * line - RISE * k * (age / (hold + fade));
            let left = x - total / 2.0;
            crate::occlusion::begin(z, crate::occlusion::OWN_BODY, "floater", [x, top + line / 2.0]);
            let a = (255.0 * alpha) as u8;
            let rgba = |c: [u8; 3]| ImColor32::from_rgba(c[0], c[1], c[2], a);
            let (text_x, icon_x) = match icon {
                Some((_, _, true)) => (left + side + gap, left),
                _ => (left, left + text_w + gap),
            };
            if let Some(e) = edge {
                let o = (1.4 * k).max(1.0);
                for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o), (-o, -o), (o, o), (-o, o), (o, -o)] {
                    font.draw_at(draw, &text, [text_x + dx, top + dy], sc, rgba(e));
                }
            }
            font.draw_at(draw, &text, [text_x, top], sc, rgba(colour));
            if let Some((art, _, _)) = icon {
                if let Some((t, _)) = textures.get(art) {
                    // Centred on the capitals' middle (the glyph cells start a little over the capitals).
                    let mid = top + font.line_height * sc * 0.5;
                    draw.add_image(t, [icon_x, mid - side / 2.0], [icon_x + side, mid + side / 2.0])
                        .col(ImColor32::from_rgba(255, 255, 255, a)).build();
                }
            }
        }
        crate::occlusion::end();
    }
}
