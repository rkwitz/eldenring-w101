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
//! The key: the controller's button when a controller was used last (or is the only input seen), else the keyboard's
//! interact key.
//!
//! Round 26 (N4, Ryan: talking showed Elden Ring's "Switch actions": two action buttons were up at once): the talk and
//! cage events now wait for ONE row, picked by EMEVD 4[32] IF Character Riding Mount (6000 / 9260 on foot, 6001 /
//! 9300 on Torrent), and set RIDING_FLAG; the prompt measures only that row's shape.
//! Round 26 (N5, Ryan: "it says Press A but A is my jump"): the key and button are Elden Ring's real binding, learned
//! at runtime: when a talk / examine fires (a dialogue request flag 1042362400-499 or a Bone Cage flag goes on) while
//! this prompt is up, the pad button or keyboard key pressed last (within LEARN_WINDOW) is the binding; it is logged
//! ("prompt: learned ...") and kept in w101_orbs\interact_binding.txt for the next start. Before that: ini
//! interact_pad / interact_key, else Elden Ring's default layout (Event Action = Y on an Xbox pad, E on the keyboard).
//! Round 26 (N6): own glyph art (tools/w101_prompt_glyphs.py): Xbox-style A / B / X / Y face buttons, a dark blank
//! button with the label for any other pad input, and a key cap without Wizard101's thin black outline.
//!
//! Round 27 (A1, Ryan: "you almost have to face AWAY from the NPC to get the prompt"). Root cause: Game::player_yaw
//! took the physics quaternion's local +z as the character's forward, unproven. Ryan's 22:58-23:06 log: 7 of 8
//! talks Elden Ring started came while this prompt said "none" (e.g. 23:04:27 none -> 23:04:28 Connelly's dialogue),
//! i.e. our facing test failed where Elden Ring's passed, and passed when he turned away: the yaw was 180 degrees off.
//! Fix, proven at runtime instead of assumed: while the player runs in a straight line (the character turns to where
//! it runs), the heading of its movement is compared with the quaternion's +z; FACING_VOTES agreeing samples decide
//! +z or -z (logged "prompt: facing calibrated ..."). Until then -z is used: an Elden Ring character with y rotation r
//! faces (-sin r, -cos r), i.e. its local forward is -z (tools/msb_add_npcs.py's vanilla check: White Mask Varre,
//! rotation -25.6, faces The First Step's grace, cos 0.99). The param's fields are now read as Elden Ring defines them (Paramdex
//! ActionButtonParam): `angle` (0x10) is the region's sector round the target (180 = all round, max 180), the
//! player's facing tolerance is allowAngle (0x28, either side; 6000/6001 = 90, cages 180 = any).
//! Round 27 (A2): with no portrait (Diego / Ugo until theirs exist) the line is centred in the whole box; a glyph whose
//! picture isn't loaded is drawn as a disc / cap with its letter, never left out; each prompt logs once whether its
//! portrait and glyph pictures were drawn.

//!
//! Round 28 (S2, Ryan: "the talk prompt doesn't appear at all on Torrent; talking still works"). Root cause, from
//! Ryan's 02:30-02:42 log (orbs\w101_orbs.log.round28_input): RIDING_FLAG 1042362570 was on 02:32:50-02:33:00 and
//! 02:33:05-02:33:18 (he talked to Diego on Torrent at 02:32:53 and 02:33:13), and the riding rows were read
//! (6001: radius 1.2, height 1.0, base -0.25; 9300: radius 1.5, height 1.5, base -0.25). At 02:32:55.896 the prompt
//! went to "none" with Diego 1.1 m away (inside 1.2) and the facing 47 degrees off (inside allowAngle 90 + 10): the
//! only test left is the height, dy = player - target within base - 0.5 .. base + height + 0.5 = -0.75 .. 1.25 m.
//! On Torrent the player's position is the rider in the saddle, above Torrent's feet, so dy sits at or over that top
//! (it passed for a moment at 02:32:55.6 and 02:32:59.4 and failed the rest of the time), while Elden Ring's own
//! test passed (its talks fired). The log has no dy value, so the saddle height itself is not measured yet. Fix: on Torrent the height window's top is raised by
//! RIDER_LIFT (the saddle); radius and facing stay Elden Ring's. Each riding approach within 4 m logs its distance,
//! dy and result once a second ("prompt: riding ..."), and a talk that fires logs where the player was
//! ("prompt: talk fired ..."), so the next log shows Elden Ring's own reach on Torrent.
use std::collections::HashMap;

use hudhook::imgui::{DrawListMut, ImColor32};

use crate::font::BitmapFont;
use crate::layout::Textures;

pub const TALK_ROWS: [u32; 2] = [6000, 6001];
pub const EXAMINE_ROWS: [u32; 2] = [9260, 9300];
/// Round 26 (N4): set by the talk / cage events while the player rides Torrent (tools/w101_quests.py RIDING_FLAG).
pub const RIDING_FLAG: u32 = 1042362570;
/// Dialogue request flags (tools/w101_quests.py DIALOGUE_FLAG + key) and the Bone Cage flags (CAGE_FLAG + i).
const REQUESTS: std::ops::Range<u32> = 1042362400..1042362500;
/// A press this long before the talk / examine fired is taken as the interact binding.
const LEARN_WINDOW: f32 = 1.0; // the flags are polled every 0.25 s (book.rs QUEST_POLL)
/// The file the learned binding is kept in (next to w101_orbs.ini): "pad=Y" / "key=E" lines.
const BINDING_FILE: &str = "interact_binding.txt";
/// Finding Out Why (quest 4): assigned, and complete (the cages' Examine button is on only in between).
const FOW_ASSIGNED: u32 = 1042360340;
const FOW_COMPLETE: u32 = 1042360349;
/// (radius, height, base offset, angle) when the param can't be read: about a vanilla talk prompt.
const FALLBACK: (f32, f32, f32, i32) = (2.0, 2.5, -0.5, 90);
/// Round 27 (A1): samples needed to decide which way the player's quaternion faces, and the speed range (m/s) of a
/// sample (running or walking straight; faster is a world shift / teleport).
const FACING_VOTES: u32 = 40;
const FACING_SPEED: std::ops::Range<f32> = 1.2..12.0;
/// Round 28 (S2): metres added to the top of a riding row's height window (the rider sits on Torrent's back, about
/// 1.3-1.6 m over its feet; 2.0 leaves room for Torrent's gait).
const RIDER_LIFT: f32 = 2.0;

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
const KEY_CAP: &str = "GUI/Art/Art_Key_Blank.png";
/// Pad glyphs: GUI/Art/Art_XButton_<A|B|X|Y>.png, others on the blank button with their label.
const PAD_BLANK: &str = "GUI/Art/Art_XButton_Blank.png";
const PAD_FACES: [&str; 4] = ["A", "B", "X", "Y"];
/// XInput button bits (input.rs) and their labels, for learning and drawing the pad binding.
const PAD_NAMES: [(u16, &str); 12] = [(0x1000, "A"), (0x2000, "B"), (0x4000, "X"), (0x8000, "Y"), (0x0100, "LB"),
    (0x0200, "RB"), (0x0400, "LT"), (0x0800, "RT"), (0x0001, "\u{2191}"), (0x0002, "\u{2193}"), (0x0004, "\u{2190}"),
    (0x0008, "\u{2192}")];
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
    /// The pad button's label (PAD_NAMES), Elden Ring's default Event Action Y until learned or set in the ini.
    pad_button: String,
    /// Where the learned binding is kept (w101_orbs folder), once known.
    folder: Option<std::path::PathBuf>,
    /// The last pad / keyboard press: (label, seconds ago), aged every update.
    last_pad: Option<(String, f32)>,
    last_key: Option<(String, f32)>,
    held_before: u16,
    keys_before: Vec<bool>,
    requests_before: Vec<u32>,
    clock: Option<std::time::Instant>,
    hide_er: bool,
    /// Elden Ring's prompt shapes (row, shape, examine) once read, and whether its texts were blanked.
    shapes: Vec<(u32, crate::game::ActionButton, bool)>,
    blanked: bool,
    /// The controller was the last input used.
    pub pad: bool,
    current: Option<Target>,
    /// The preview renderer sets the device itself.
    pub preview: bool,
    /// Round 27 (A1): added to Game::player_yaw (0 = +z forward, PI = -z forward, the default) and whether the
    /// movement calibration has decided it.
    facing: Option<f32>,
    calibrated: bool,
    /// Votes (quaternion +z along the movement, against it) and the last sample (position, time, yaw).
    votes: [u32; 2],
    last_move: Option<([f32; 3], std::time::Instant, f32)>,
    /// Round 27 (A2): the current prompt's drawing was logged.
    drawn_log: std::sync::atomic::AtomicBool,
    /// Round 28 (S2): the nearest NPC's measures this frame (for the talk-fired log) and when a riding line was logged.
    probe: String,
    riding_log: Option<std::time::Instant>,
}

impl Default for Prompt {
    fn default() -> Prompt {
        Prompt { key: "E".into(), pad_button: "Y".into(), folder: None, last_pad: None, last_key: None,
                 held_before: 0, keys_before: Vec::new(), requests_before: Vec::new(), clock: None, hide_er: true,
                 shapes: Vec::new(), blanked: false, pad: false, current: None, preview: false, facing: Some(std::f32::consts::PI), calibrated: false,
                 votes: [0, 0], last_move: None, drawn_log: std::sync::atomic::AtomicBool::new(false),
                 probe: String::new(), riding_log: None }
    }
}

/// Wizard101's words for the prompt, split round the key's picture.
pub fn words(examine: bool) -> (&'static str, &'static str) {
    ("Press", if examine { "to Examine" } else { "to Talk" })
}

impl Prompt {
    /// Reads w101_orbs.ini's interact_key, interact_pad and hide_er_prompt, then the learned binding
    /// (BINDING_FILE next to it), which wins.
    pub fn load(&mut self, ini: &std::path::Path) {
        self.folder = ini.parent().map(|p| p.to_path_buf());
        let learned = ini.with_file_name(BINDING_FILE);
        for (file, learned) in [(ini.to_path_buf(), false), (learned, true)] {
            for line in std::fs::read_to_string(&file).unwrap_or_default().lines() {
                let Some((k, v)) = line.split_once('=') else { continue };
                let v = v.trim();
                match k.trim() {
                    "interact_key" | "key" if !v.is_empty() => self.key = v.chars().take(5).collect(),
                    "interact_pad" | "pad" if PAD_NAMES.iter().any(|p| p.1 == v) => self.pad_button = v.into(),
                    "hide_er_prompt" if !learned => self.hide_er = v != "0",
                    _ => {}
                }
            }
        }
        tracing::info!("prompt: interact key {} / pad {}", self.key, self.pad_button);
    }

    /// Round 26 (N5): watches the inputs (`pad_held`: XInput bits, input::held) and the talk / examine flags, and
    /// learns which pad button and key Elden Ring talks with. Call every frame in normal play.
    pub fn observe(&mut self, pad_held: u16, flag: &dyn Fn(u32) -> bool, cages: u32) {
        let now = std::time::Instant::now();
        let dt = self.clock.map_or(0.0, |c| now.duration_since(c).as_secs_f32());
        self.clock = Some(now);
        for last in [&mut self.last_pad, &mut self.last_key] {
            if let Some((_, age)) = last {
                *age += dt;
            }
        }
        let pressed = pad_held & !self.held_before;
        self.held_before = pad_held;
        if let Some((_, name)) = PAD_NAMES.iter().find(|(bit, _)| pressed & bit != 0) {
            self.last_pad = Some((name.to_string(), 0.0));
        }
        let keys = keys_down();
        if let Some(k) = keys.iter().enumerate().find(|(i, d)| **d && !self.keys_before.get(*i).copied().unwrap_or(false))
            .and_then(|(i, _)| key_name(i as u8)) {
            self.last_key = Some((k, 0.0));
        }
        self.keys_before = keys;
        let on: Vec<u32> = REQUESTS.chain(crate::questhelper::CAGE_FLAG..crate::questhelper::CAGE_FLAG + cages)
            .filter(|f| flag(*f)).collect();
        let fired = on.iter().any(|f| !self.requests_before.contains(f));
        self.requests_before = on;
        if fired && !self.probe.is_empty() {
            tracing::info!("prompt: talk fired with the prompt {} ({})",
                           if self.current.is_some() { "up" } else { "NOT up" }, self.probe);
        }
        if !fired || self.current.is_none() {
            return;
        }
        let fresh = |l: &Option<(String, f32)>| l.as_ref().filter(|(_, age)| *age <= LEARN_WINDOW).map(|l| l.clone());
        let (pad, key) = (fresh(&self.last_pad), fresh(&self.last_key));
        // The newer of the two presses did it.
        let changed = match (pad, key) {
            (Some(p), k) if k.as_ref().is_none_or(|k| p.1 <= k.1) => {
                let changed = p.0 != self.pad_button;
                self.pad_button = p.0;
                changed
            }
            (_, Some(k)) => {
                let changed = k.0 != self.key;
                self.key = k.0;
                changed
            }
            _ => return,
        };
        if changed {
            tracing::info!("prompt: learned Elden Ring's interact binding: pad {} / key {}", self.pad_button, self.key);
            if let Some(folder) = &self.folder {
                let text = format!("# learned by hud_orbs prompt.rs (round 26 N5)\npad={}\nkey={}\n", self.pad_button,
                                   self.key);
                if let Err(e) = std::fs::write(folder.join(BINDING_FILE), text) {
                    tracing::warn!("prompt: could not save {BINDING_FILE}: {e}");
                }
            }
        }
    }

    /// Sets the pad button's label (the preview).
    #[allow(dead_code)]
    pub fn set_pad_button(&mut self, label: &str) {
        self.pad_button = label.into();
    }

    /// The art the prompt draws.
    pub fn art(&self) -> Vec<String> {
        let mut art: Vec<String> = [BACKGROUND, KEY_CAP, PAD_BLANK, CAGE_PICTURE].map(String::from).to_vec();
        art.extend(PAD_FACES.iter().map(|f| format!("GUI/Art/Art_XButton_{f}.png")));
        art
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
    fn inside(&self, examine: bool, riding: bool, player: [f32; 3], yaw: Option<f32>, at: [f32; 3]) -> bool {
        // Round 26 (N4): only the row the events wait for now: the second of each pair on Torrent.
        let row = |r: &u32| (if examine { EXAMINE_ROWS } else { TALK_ROWS })[riding as usize] == *r;
        let shapes: Vec<(f32, f32, f32, i32)> = self.shapes.iter().filter(|s| s.2 == examine && row(&s.0)).map(|(_, b, _)| {
            (if b.radius > 0.1 { b.radius } else { FALLBACK.0 }, if b.height > 0.1 { b.height } else { FALLBACK.1 },
             b.base, b.allow_angle)
        }).collect();
        let lift = if riding { RIDER_LIFT } else { 0.0 };
        if shapes.is_empty() {
            return self.inside_shape(FALLBACK, lift, player, yaw, at);
        }
        shapes.into_iter().any(|s| self.inside_shape(s, lift, player, yaw, at))
    }

    /// The angle (degrees, 0..180) between facing `yaw` (radians, atan2(x, z)) and the direction from `from` to `at`.
    fn off_by(yaw: f32, from: [f32; 3], at: [f32; 3]) -> f32 {
        let to = (at[0] - from[0]).atan2(at[2] - from[2]);
        ((to - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs().to_degrees()
    }

    fn inside_shape(&self, (radius, height, base, allow): (f32, f32, f32, i32), lift: f32, player: [f32; 3],
                    yaw: Option<f32>, at: [f32; 3]) -> bool {
        let (dx, dz) = (at[0] - player[0], at[2] - player[2]);
        let dy = player[1] - at[1];
        if dx.hypot(dz) > radius || dy < base - 0.5 || dy > base + height + 0.5 + lift {
            return false;
        }
        // allowAngle: the player facing the target within that many degrees either side (180: any way), once the
        // facing is proven (A1). The region's own `angle` is 180 (all round) on all four rows: no sector to test.
        match (yaw, self.facing) {
            (Some(y), Some(f)) if allow > 0 && allow < 180 && dx.hypot(dz) > 0.3 =>
                Self::off_by(y + f, player, at) <= allow as f32 + 10.0,
            _ => true,
        }
    }

    /// Round 27 (A1): learns which way the player's quaternion faces from its straight-line movement (see the top).
    fn calibrate(&mut self, player: [f32; 3], yaw: Option<f32>) {
        let Some(yaw) = yaw else { return };
        let now = std::time::Instant::now();
        let Some((before, then, yaw_before)) = self.last_move.replace((player, now, yaw)) else { return };
        if self.calibrated {
            return;
        }
        let dt = now.duration_since(then).as_secs_f32();
        let step = (player[0] - before[0]).hypot(player[2] - before[2]);
        let turned = ((yaw - yaw_before + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI).abs();
        if !(0.005..0.2).contains(&dt) || !FACING_SPEED.contains(&(step / dt)) || turned > 0.05 {
            return;
        }
        let off = Self::off_by(yaw, before, player);
        if off < 30.0 {
            self.votes[0] += 1;
        } else if off > 150.0 {
            self.votes[1] += 1;
        }
        let [plus, minus] = self.votes;
        if plus.max(minus) >= FACING_VOTES && plus.min(minus) * 5 <= plus.max(minus) {
            self.facing = Some(if plus > minus { 0.0 } else { std::f32::consts::PI });
            self.calibrated = true;
            tracing::info!("prompt: facing calibrated: the player runs along his quaternion's {} ({plus} samples along \
                            +z, {minus} along -z); the talk prompt now tests his facing (allowAngle)",
                           if plus > minus { "+z" } else { "-z" });
        }
    }

    /// Picks what the prompt is for this frame: the nearest loaded street NPC (`spots`, markers::locate) or shut Bone
    /// Cage (MSB spots made physics through `player_msb`) the player stands at the prompt of. Logs changes.
    pub fn update(&mut self, player: [f32; 3], yaw: Option<f32>, spots: &[crate::markers::Spot],
                  player_msb: Option<[f32; 3]>, flag: &dyn Fn(u32) -> bool) -> Option<Target> {
        self.calibrate(player, yaw);
        let d = |p: [f32; 3]| (p[0] - player[0]).hypot(p[2] - player[2]);
        let riding = flag(RIDING_FLAG);
        let mut best: Option<(f32, Target)> = None;
        // Round 27 (A1): the nearest loaded NPC within 4 m, for the log (distance, facing off by, both conventions).
        let near = spots.iter().filter(|s| s.2 && d(s.1) < 4.0).min_by(|a, b| d(a.1).total_cmp(&d(b.1)));
        let near_text = match (near, yaw) {
            (Some((i, at, _)), Some(y)) => format!(" ({} {:.1} m, facing off by {:.0} deg with +z / {:.0} with -z, \
                                                    using {}, votes {:?})", crate::markers::NPCS[*i].1, d(*at),
                                                   Self::off_by(y, player, *at),
                                                   Self::off_by(y + std::f32::consts::PI, player, *at),
                                                   match (self.facing, self.calibrated) {
                                                       (Some(f), c) => format!("{} ({})", if f == 0.0 { "+z" } else { "-z" },
                                                                               if c { "calibrated" } else { "default" }),
                                                       (None, _) => "none".into(),
                                                   },
                                                   self.votes),
            _ => String::new(),
        };
        for (i, at, loaded) in spots {
            if !loaded || !self.inside(false, riding, player, yaw, *at) {
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
                if self.inside(true, riding, player, yaw, at) && best.as_ref().is_none_or(|b| d(at) < b.0) {
                    best = Some((d(at), Target { name: "Bone Cage".into(), portrait: CAGE_PICTURE.into(),
                                                 examine: true }));
                }
            }
        }
        // Round 28 (S2): the nearest NPC's distance and height for the logs (talk fired; riding approaches).
        self.probe = match near {
            Some((i, at, _)) => format!("{} {:.2} m, dy {:+.2} m, {}", crate::markers::NPCS[*i].1, d(*at),
                                        player[1] - at[1], if riding { "on Torrent" } else { "on foot" }),
            None => String::new(),
        };
        if riding && !self.probe.is_empty() && self.riding_log.is_none_or(|t| t.elapsed().as_secs_f32() >= 1.0) {
            tracing::info!("prompt: riding: {} -> {}", self.probe, best.as_ref().map_or("none", |b| b.1.name.as_str()));
            self.riding_log = Some(std::time::Instant::now());
        }
        let now = best.map(|b| b.1);
        if now != self.current {
            match &now {
                Some(t) => tracing::info!("prompt: {} \"Press {} {}\"{near_text}", t.name,
                                          if self.pad { &self.pad_button } else { &self.key }, words(t.examine).1),
                None => tracing::info!("prompt: none{near_text}"),
            }
            self.current = now.clone();
            self.drawn_log.store(false, std::sync::atomic::Ordering::Relaxed);
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
        let portrait = textures.get(&target.portrait);
        if let Some((t, _)) = portrait {
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
        // Round 27 (A2): no portrait -> centred in the whole box (inside its border, TEXT_RIGHT each side).
        let area = [left + if portrait.is_some() { PORTRAIT } else { TEXT_RIGHT } * s, left + w - TEXT_RIGHT * s];
        let mut x = (area[0] + area[1] - total) / 2.0;
        let middle = top + TEXT_Y * s;
        let text_top = middle - normal.line_height * k / 2.0;
        normal.draw_at(draw, before, [x, text_top], k, black);
        x += normal.width(before) * k + space;
        // Round 26 (N6): a face button's own picture carries its letter; any other pad input and the keyboard key
        // get their label written on the blank button / key cap.
        let face = format!("GUI/Art/Art_XButton_{}.png", self.pad_button);
        let (picture, letter, white) = if self.pad {
            if PAD_FACES.contains(&self.pad_button.as_str()) { (face.as_str(), None, true) }
            else { (PAD_BLANK, Some(self.pad_button.as_str()), true) }
        } else {
            (KEY_CAP, Some(self.key.as_str()), false)
        };
        let glyph = textures.get(picture);
        let face_letter = (self.pad && letter.is_none()).then_some(self.pad_button.as_str());
        if let Some((t, _)) = glyph {
            draw.add_image(t, [x, middle - icon / 2.0], [x + icon, middle + icon / 2.0]).build();
        } else {
            // Round 27 (A2): the picture isn't loaded (yet): a pad disc in the face button's colour / a pale key cap.
            let c = [x + icon / 2.0, middle];
            if self.pad {
                let fill = match self.pad_button.as_str() {
                    "A" => [96, 176, 64], "B" => [200, 60, 50], "X" => [60, 110, 200], "Y" => [220, 180, 40],
                    _ => [70, 70, 76],
                };
                draw.add_circle(c, icon * 0.47, ImColor32::from_rgba(fill[0], fill[1], fill[2], 255)).filled(true).build();
                draw.add_circle(c, icon * 0.47, ImColor32::from_rgba(30, 30, 30, 255)).thickness(1.5 * s).build();
            } else {
                draw.add_rect([x + icon * 0.06, middle - icon * 0.44], [x + icon * 0.94, middle + icon * 0.44],
                              ImColor32::from_rgba(236, 232, 224, 255)).filled(true).rounding(icon * 0.15).build();
                draw.add_rect([x + icon * 0.06, middle - icon * 0.44], [x + icon * 0.94, middle + icon * 0.44],
                              ImColor32::from_rgba(90, 86, 80, 255)).rounding(icon * 0.15).thickness(1.2 * s).build();
            }
        }
        if !self.drawn_log.swap(true, std::sync::atomic::Ordering::Relaxed) {
            tracing::info!("prompt: drawn for {}: portrait {} {}, glyph {} {}", target.name, target.portrait,
                           if portrait.is_some() { "shown" } else { "NOT loaded (line centred in the box)" }, picture,
                           if glyph.is_some() { "shown" } else { "NOT loaded (drawn disc / cap)" });
        }
        let letter = letter.or(if glyph.is_none() { face_letter } else { None });
        if let Some(letter) = letter {
            // The label a little above the middle on the key cap (its front lip is at the bottom), centred on a pad
            // button; larger than round 25's (Ryan's screenshot: the E was hard to read).
            let n = letter.chars().count();
            let lk = k * 0.72 * if n > 2 { 0.5 } else if n > 1 { 0.66 } else { 1.0 };
            let lw = normal.width(letter) * lk;
            let lift = if white { 0.0 } else { icon * 0.07 };
            let colour = if white { ImColor32::from_rgba(255, 255, 255, 255) } else { ImColor32::from_rgba(40, 36, 32, 255) };
            normal.draw_at(draw, letter, [x + (icon - lw) / 2.0, middle - lift - normal.line_height * lk / 2.0], lk,
                           colour);
        }
        x += icon + space;
        normal.draw_at(draw, after, [x, text_top], k, black);
        top
    }
}

/// Round 26 (N5): which virtual keys are down now (index = VK code), read in the game only.
#[cfg(windows)]
fn keys_down() -> Vec<bool> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    (0..256).map(|vk| unsafe { GetAsyncKeyState(vk) } < 0).collect()
}

#[cfg(not(windows))]
fn keys_down() -> Vec<bool> {
    Vec::new()
}

/// The label of virtual key `vk` as a key cap shows it, or None for mouse buttons, modifiers and movement keys
/// (W A S D and the arrows are held while walking up, so they are never the talk key).
fn key_name(vk: u8) -> Option<String> {
    match vk {
        b'W' | b'A' | b'S' | b'D' => None,
        b'0'..=b'9' | b'B'..=b'Z' => Some((vk as char).to_string()),
        0x0D => Some("Enter".into()),
        0x20 => Some("Space".into()),
        0x09 => Some("Tab".into()),
        0x70..=0x7B => Some(format!("F{}", vk - 0x6F)),
        _ => None,
    }
}
