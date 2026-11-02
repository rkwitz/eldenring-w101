//! Round 25 (H3, Ryan: "replace ER's 'Talk' interact prompt with W101's"): Wizard101's interact prompt, a small
//! dialogue-style bar over the experience bar with the NPC's portrait and name and "Press [key] to Talk", shown while
//! Elden Ring would show its own "Talk" (or the Bone Cage's "Examine") prompt.
//!
//! Wizard101 (Ryan's screenshot round25/ref/4_w101_press_to_talk.png, measured with the 0.905 screenshot pixels per
//! layout pixel of the other references): GUI/Dialog/Background_Dialog.dds at its own 4:1 shape, BOX layout pixels,
//! the round NpcPortraits picture (PORTRAIT) over its left end, the NPC's name in WizardFancy (NAME_SCALE, black) on
//! the title scroll, and the line in WizardNormal (TEXT_SCALE, black) centred in the rest, with the key's picture
//! (GUI/Art/Art_Keyboard_Blank.dds with the key's letter, or Art_XController_A.dds) inline; its bottom GAP over the
//! experience bar. The quest helper (arrow and goal) moves up over it, as in the screenshot. The words are Wizard101's
//! GUI.lang NPCInteractNoClick "Press &InputBindings_NPCInteract& to Talk" (NPCInteractText adds "or
//! &Icons_LeftMouseClick&", but a click doesn't talk in Elden Ring) and GUI_00006987 "Press
//! &InputBindings_NPCInteract& to Examine". The .gui of the prompt is not in Root / _Shared-WorldData /
//! WizardCity-WorldData (the client builds it in code: HUDWindow's NPCInteractionData handler), so the layout comes
//! from the screenshot.
//!
//! When: the player within Elden Ring's own prompt shape of an ActionButtonParam row of TALK_ROWS (the talk events'
//! buttons on the street NPCs: 6000 on foot, 6001 on Torrent, tools/w101_quests.py TALK_NPC_BUTTON(_RIDING)) round a
//! loaded street NPC, or of EXAMINE_ROWS (9260 on foot, 9300 on Torrent: CAGE_BUTTON(_RIDING)) round a shut Bone
//! Cage while Finding Out Why's cages goal is open, read from the game's param data (Game::action_button; radius,
//! height, base offset and facing angle), FALLBACK until it can be read. Elden Ring's own prompt text of all four rows
//! is blanked (textId -1; ini hide_er_prompt=0 keeps it); the button press itself is Elden Ring's.
//!
//! The key: the controller's A when a controller was used last (or is the only input seen), else the keyboard's
//! interact key (ini interact_key, default E: Elden Ring's default "Event action / Interact").

use std::collections::HashMap;

use hudhook::imgui::{DrawListMut, ImColor32};

use crate::font::BitmapFont;
use crate::layout::Textures;

pub const TALK_ROWS: [u32; 2] = [6000, 6001];
pub const EXAMINE_ROWS: [u32; 2] = [9260, 9300];
/// Finding Out Why (quest 4): assigned, and complete (the cages' Examine button is on only in between).
const FOW_ASSIGNED: u32 = 1042360340;
const FOW_COMPLETE: u32 = 1042360349;
/// (radius, height, base offset, angle) when the param can't be read: about a vanilla talk prompt.
const FALLBACK: (f32, f32, f32, i32) = (2.0, 2.5, -0.5, 360);

/// Layout pixels (768-high screen), measured on the screenshot.
const BOX: [f32; 2] = [420.0, 103.0];
/// Portrait: left edge at the box's, top PORTRAIT_TOP under the box's top, side PORTRAIT.
const PORTRAIT: f32 = 88.0;
const PORTRAIT_TOP: f32 = 19.0;
/// Name: left at NAME_X from the box's left, its middle NAME_Y under the box's top.
const NAME_X: f32 = 63.0;
const NAME_Y: f32 = 11.0;
const NAME_SCALE: f32 = 0.42;
/// The line: centred between the portrait and the box's right edge (less TEXT_RIGHT), its middle TEXT_Y down.
const TEXT_Y: f32 = 55.0;
const TEXT_RIGHT: f32 = 10.0;
const TEXT_SCALE: f32 = 0.6;
const ICON: f32 = 31.0;
/// The box's bottom over the experience bar's top.
const GAP: f32 = 31.0;

const BACKGROUND: &str = "GUI/Dialog/Background_Dialog.png";
const KEY_CAP: &str = "GUI/Art/Art_Keyboard_Blank.png";
const PAD_A: &str = "GUI/Art/Art_XController_A.png";
const CAGE_PICTURE: &str = "GUI/QuestButtons/Art_HD_WC_BoneCage.png";

/// What the prompt is for: who (name, portrait) and whether it is the cage's Examine.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub name: String,
    pub portrait: String,
    pub examine: bool,
}

pub struct Prompt {
    key: String,
    hide_er: bool,
    /// Elden Ring's prompt shapes (row, shape, examine) once read, and whether its texts were blanked.
    shapes: Vec<(u32, crate::game::ActionButton, bool)>,
    blanked: bool,
    /// The controller was the last input used.
    pub pad: bool,
    current: Option<Target>,
    /// The preview renderer sets the device itself.
    pub preview: bool,
}

impl Default for Prompt {
    fn default() -> Prompt {
        Prompt { key: "E".into(), hide_er: true, shapes: Vec::new(), blanked: false, pad: false, current: None, preview: false }
    }
}

/// Wizard101's words for the prompt, split round the key's picture.
pub fn words(examine: bool) -> (&'static str, &'static str) {
    ("Press", if examine { "to Examine" } else { "to Talk" })
}

impl Prompt {
    /// Reads w101_orbs.ini's interact_key and hide_er_prompt.
    pub fn load(&mut self, ini: &std::path::Path) {
        for line in std::fs::read_to_string(ini).unwrap_or_default().lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "interact_key" if !v.trim().is_empty() => self.key = v.trim().chars().take(5).collect(),
                "hide_er_prompt" => self.hide_er = v.trim() != "0",
                _ => {}
            }
        }
    }

    /// The art the prompt draws.
    pub fn art(&self) -> Vec<String> {
        [BACKGROUND, KEY_CAP, PAD_A, CAGE_PICTURE].map(String::from).to_vec()
    }

    /// Reads Elden Ring's prompt shapes once and blanks its prompt texts (hide_er_prompt).
    pub fn setup(&mut self, game: &crate::game::Game) {
        if self.shapes.is_empty() {
            let rows = TALK_ROWS.iter().map(|r| (*r, false)).chain(EXAMINE_ROWS.iter().map(|r| (*r, true)));
            let shapes: Vec<_> = rows.filter_map(|(r, e)| game.action_button(r).map(|b| (r, b, e))).collect();
            if shapes.len() == TALK_ROWS.len() + EXAMINE_ROWS.len() {
                for (r, b, _) in &shapes {
                    tracing::info!("prompt: Elden Ring's ActionButtonParam {r}: {b:?}");
                }
                self.shapes = shapes;
            }
        }
        if self.hide_er && !self.blanked && !self.shapes.is_empty() {
            let ok: Vec<String> = self.shapes.iter()
                .map(|(r, b, _)| format!("{r} {} -> -1 {}", b.text_id, game.set_action_button_text(*r, -1)))
                .collect();
            tracing::info!("prompt: Elden Ring's own prompt texts blanked: {ok:?}");
            self.blanked = true;
        }
    }

    /// Whether the player at `player` (feet, physics space) facing `yaw` is inside one of the talk (or `examine`)
    /// rows' prompt shapes round a target whose feet are at `at` (on foot or on Torrent: whichever row fits).
    fn inside(&self, examine: bool, player: [f32; 3], yaw: Option<f32>, at: [f32; 3]) -> bool {
        let shapes: Vec<(f32, f32, f32, i32)> = self.shapes.iter().filter(|s| s.2 == examine).map(|(_, b, _)| {
            (if b.radius > 0.1 { b.radius } else { FALLBACK.0 }, if b.height > 0.1 { b.height } else { FALLBACK.1 },
             b.base, b.angle)
        }).collect();
        if shapes.is_empty() {
            return self.inside_shape(FALLBACK, player, yaw, at);
        }
        shapes.into_iter().any(|s| self.inside_shape(s, player, yaw, at))
    }

    fn inside_shape(&self, (radius, height, base, angle): (f32, f32, f32, i32), player: [f32; 3], yaw: Option<f32>,
                    at: [f32; 3]) -> bool {
        let (dx, dz) = (at[0] - player[0], at[2] - player[2]);
        let dy = player[1] - at[1];
        if dx.hypot(dz) > radius || dy < base - 0.5 || dy > base + height + 0.5 {
            return false;
        }
        // The player facing the target within half the angle (0 or 360 and more: any way).
        match yaw {
            Some(y) if angle > 0 && angle < 360 && dx.hypot(dz) > 0.3 => {
                let to = dx.atan2(dz);
                let d = (to - y + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                d.abs().to_degrees() <= angle as f32 / 2.0 + 5.0
            }
            _ => true,
        }
    }

    /// Picks what the prompt is for this frame: the nearest loaded street NPC (`spots`, markers::locate) or shut Bone
    /// Cage (MSB spots made physics through `player_msb`) the player stands at the prompt of. Logs changes.
    pub fn update(&mut self, player: [f32; 3], yaw: Option<f32>, spots: &[crate::markers::Spot],
                  player_msb: Option<[f32; 3]>, flag: &dyn Fn(u32) -> bool) -> Option<Target> {
        let d = |p: [f32; 3]| (p[0] - player[0]).hypot(p[2] - player[2]);
        let mut best: Option<(f32, Target)> = None;
        for (i, at, loaded) in spots {
            if !loaded || !self.inside(false, player, yaw, *at) {
                continue;
            }
            let npc = &crate::markers::NPCS[*i];
            if best.as_ref().is_none_or(|b| d(*at) < b.0) {
                best = Some((d(*at), Target { name: npc.1.to_string(),
                                              portrait: format!("{}/{}.png", crate::markers::ER_PORTRAITS, npc.0),
                                              examine: false }));
            }
        }
        if let Some(msb) = player_msb.filter(|_| flag(FOW_ASSIGNED) && !flag(FOW_COMPLETE)) {
            for (i, c) in crate::questhelper::cages().iter().enumerate() {
                if flag(crate::questhelper::CAGE_FLAG + i as u32) {
                    continue;
                }
                let at = [c[0] - msb[0] + player[0], c[1] - msb[1] + player[1], c[2] - msb[2] + player[2]];
                if self.inside(true, player, yaw, at) && best.as_ref().is_none_or(|b| d(at) < b.0) {
                    best = Some((d(at), Target { name: "Bone Cage".into(), portrait: CAGE_PICTURE.into(),
                                                 examine: true }));
                }
            }
        }
        let now = best.map(|b| b.1);
        if now != self.current {
            match &now {
                Some(t) => tracing::info!("prompt: {} \"Press {} {}\"", t.name,
                                          if self.pad { "A" } else { &self.key }, words(t.examine).1),
                None => tracing::info!("prompt: none"),
            }
            self.current = now.clone();
        }
        now
    }

    /// Draws the prompt for `target` on a `screen` with the experience bar's top at `bar_top`; returns the box's
    /// top (screen pixels), for the quest helper to sit over it.
    pub fn draw(&self, draw: &DrawListMut, textures: &Textures, fonts: &HashMap<String, BitmapFont>, screen: [f32; 2],
                bar_top: f32, target: &Target) -> f32 {
        let s = screen[1] / 768.0;
        let (w, h) = (BOX[0] * s, BOX[1] * s);
        let left = (screen[0] - w) / 2.0;
        let top = bar_top - GAP * s - h;
        if let Some((t, _)) = textures.get(BACKGROUND) {
            draw.add_image(t, [left, top], [left + w, top + h]).build();
        }
        if let Some((t, _)) = textures.get(&target.portrait) {
            let y = top + PORTRAIT_TOP * s;
            draw.add_image(t, [left, y], [left + PORTRAIT * s, y + PORTRAIT * s]).build();
        }
        let black = ImColor32::from_rgba(0, 0, 0, 255);
        if let Some(fancy) = fonts.get("WizardFancy") {
            let k = NAME_SCALE * s;
            fancy.draw_at(draw, &target.name, [left + NAME_X * s, top + NAME_Y * s - fancy.line_height * k / 2.0], k,
                          black);
        }
        let Some(normal) = fonts.get("WizardNormal") else { return top };
        let k = TEXT_SCALE * s;
        let (before, after) = words(target.examine);
        let space = normal.width(" ") * k;
        let icon = ICON * s;
        let total = normal.width(before) * k + space + icon + space + normal.width(after) * k;
        let area = [left + PORTRAIT * s, left + w - TEXT_RIGHT * s];
        let mut x = (area[0] + area[1] - total) / 2.0;
        let middle = top + TEXT_Y * s;
        let text_top = middle - normal.line_height * k / 2.0;
        normal.draw_at(draw, before, [x, text_top], k, black);
        x += normal.width(before) * k + space;
        let (picture, letter) = if self.pad { (PAD_A, None) } else { (KEY_CAP, Some(self.key.as_str())) };
        if let Some((t, _)) = textures.get(picture) {
            draw.add_image(t, [x, middle - icon / 2.0], [x + icon, middle + icon / 2.0]).build();
        }
        if let Some(letter) = letter {
            // The key's letter on the blank key cap, as Wizard101's Art_Keyboard_X shows its X (a little above the
            // cap's middle: the cap's front edge is at its bottom).
            let lk = k * 0.55 * if letter.chars().count() > 1 { 0.6 } else { 1.0 };
            let lw = normal.width(letter) * lk;
            normal.draw_at(draw, letter, [x + (icon - lw) / 2.0, middle - icon * 0.08 - normal.line_height * lk / 2.0],
                           lk, black);
        }
        x += icon + space;
        normal.draw_at(draw, after, [x, text_top], k, black);
        top
    }
}
