//! Round 23 (hud H7, Ryan: "Make sure you also list the goal update when a quest parameter is filled. It should show
//! that on the screen and then fade out."): Wizard101's quest popups.
//!
//! Wizard101's text (Root.wad Locale/en-US/Quest.lang): QuestCompletePopup "Quest Complete: $QUEST_NAME$",
//! NewQuestPopup "New Quest: $QUEST_NAME$", *GoalCompletePopupText "Goal Complete: #1:$GOAL_NAME$ ...", *GoalReward
//! "Goal Update: #1:$GOAL_NAME$ #2:$GOAL_COUNT$ of #3:$GOAL_TOTAL$ ...", *NewGoalPopupText "New Goal: #1:$GOAL_NAME$".
//! Sound: _Shared-WorldData.wad Sound/GUI/ui_quest_updated.wav (orbs\w101_orbs\sfx, round 22 S3), through music::sfx.
//!
//! Round 25 (H1, Ryan's screenshots round25/ref/1_w101_quest_complete_popup.png and 3_w101_stacked_popups.png):
//! round 23 drew them white, centred at WizardPopupText.gui's y 300, one after another. Measured on the screenshots
//! (their XP bar and the scene's top give 0.905 screenshot pixels per pixel of W101's 768-high layout):
//! - colour gold-yellow, mean of the glyph pixels (231, 231, 33), brightest (252, 249, 109) -> COLOUR, with a dark
//!   edge and drop shadow (outlined + shadowed);
//! - WizardFancy, mixed case (its lower case is small capitals): "Quest Complete: Rattlebones Report" is 642 font
//!   pixels wide and 372 screenshot pixels -> SCALE 0.643; check: "New Quest: Panic in Three Streets!" (608) predicts
//!   351, measured 351;
//! - the newest line's middle at y 489 / 492 of the screenshot -> BOTTOM 517 of 768 (just over the player's feet,
//!   the lower third), the lines 26 screenshot pixels apart -> PITCH 29;
//! - several at once all show: the newest at the bottom pushing the older ones up (they slide, SLIDE s), each
//!   fading out by its own age (the old "Quest Complete" nearly gone over two fresh "New Quest" lines).
//! They show over the dialogue box too (screenshot 2: over the quest picker), not over the book or map.
//!
//! Driven by the quest flags book.rs polls (poll_quests): `messages` compares the flags before and after.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use hudhook::imgui::{DrawListMut, ImColor32};

use crate::font::BitmapFont;
use crate::layout::{Canvas, TextStyle, Textures};
use crate::quests::{Counter, Quests};

/// Round 25 (H1): measured on Ryan's screenshots (see the top): font scale, colour, the newest line's middle and
/// the line pitch (pixels of the 768-high layout).
const SCALE: f32 = 0.643;
const COLOUR: [u8; 3] = [238, 234, 36];
const BOTTOM: f32 = 517.0;
const PITCH: f32 = 29.0;
/// Seconds fully shown, then fading out; seconds an older line takes to slide up a row; most lines at once.
const HOLD: f32 = 4.0;
const FADE: f32 = 1.5;
const SLIDE: f32 = 0.2;
const MAX_LINES: usize = 6;
const SOUND: &str = "ui_quest_updated";

struct Line {
    text: String,
    born: Instant,
    /// Rows up from the bottom it is drawn at now (eased toward its slot).
    row: f32,
}

#[derive(Default)]
pub struct GoalPopups {
    /// Oldest first; the last is the newest (bottom row).
    lines: Vec<Line>,
    /// Popups that came while hidden (book or map open): shown when it closes.
    queue: VecDeque<String>,
}

fn progress(counter: &Counter, flag: &dyn Fn(u32) -> bool) -> Option<u32> {
    match counter {
        Counter::None => None,
        Counter::Bits(base) => Some((0..4).fold(0, |v, b| v << 1 | flag(base + b) as u32)),
        Counter::Flags(list) => Some(list.iter().filter(|f| flag(**f)).count() as u32),
    }
}

/// The popups for the quest flags going from `old` to `new`, in Wizard101's words, in order.
pub fn messages(quests: &Quests, old: &dyn Fn(u32) -> bool, new: &dyn Fn(u32) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    for q in &quests.quests {
        if !new(q.assigned) {
            continue;
        }
        let fresh = !old(q.assigned);
        if fresh && !new(q.complete) {
            out.push(format!("New Quest: {}", q.title));
        }
        let current = |flag: &dyn Fn(u32) -> bool, i: usize| {
            let g = &q.goals[i];
            flag(q.assigned) && !flag(g.flag) && !flag(q.complete)
                && g.after.iter().all(|a| q.goals.get(a.wrapping_sub(1)).is_some_and(|d| flag(d.flag)))
        };
        for (i, g) in q.goals.iter().enumerate() {
            if !old(g.flag) && new(g.flag) {
                out.push(format!("Goal Complete: {}", g.text));
                continue;
            }
            if g.count > 1 && !new(g.flag) {
                if let (Some(a), Some(b)) = (progress(&g.counter, old), progress(&g.counter, new)) {
                    if b > a && !fresh {
                        out.push(format!("Goal Update: {} {} of {}", g.text, b.min(g.count), g.count));
                    }
                }
            }
            if !fresh && !current(old, i) && current(new, i) {
                out.push(format!("New Goal: {}", g.text));
            }
        }
        if !old(q.complete) && new(q.complete) {
            out.push(format!("Quest Complete: {}", q.title));
        }
    }
    out
}

impl GoalPopups {
    /// Queues the popups for a change of the quest flags (book.rs poll_quests, not at load).
    pub fn changes(&mut self, quests: &Quests, old: &HashMap<u32, bool>, new: &HashMap<u32, bool>) {
        let on = |m: &HashMap<u32, bool>, f: u32| m.get(&f) == Some(&true);
        for m in messages(quests, &|f| on(old, f), &|f| on(new, f)) {
            tracing::info!("quest popup: {m}");
            self.queue.push_back(m);
        }
    }

    /// Preview: lines (text, seconds since it came up), oldest first.
    pub fn preview(&mut self, lines: &[(&str, f32)]) {
        let now = Instant::now();
        let n = lines.len();
        self.lines = lines.iter().enumerate().map(|(i, (text, age))| Line {
            text: text.to_string(),
            born: now - std::time::Duration::from_secs_f32(*age),
            row: (n - 1 - i) as f32,
        }).collect();
    }

    fn alpha(age: f32) -> f32 {
        if age < HOLD { 1.0 } else { (1.0 - (age - HOLD) / FADE).clamp(0.0, 1.0) }
    }

    /// Draws the popups (when `visible`; time runs on regardless), taking in the queued ones while visible.
    pub fn draw(&mut self, draw: &DrawListMut, textures: &Textures, fonts: &HashMap<String, BitmapFont>,
                screen: [f32; 2], visible: bool, dt: f32) {
        let now = Instant::now();
        self.lines.retain(|l| (now - l.born).as_secs_f32() < HOLD + FADE);
        if visible && !self.queue.is_empty() {
            crate::music::sfx(SOUND);
            // The new lines come in at the bottom, the older ones keep their row and slide up from there.
            for text in self.queue.drain(..) {
                self.lines.push(Line { text, born: now, row: -1.0 });
            }
            let extra = self.lines.len().saturating_sub(MAX_LINES);
            self.lines.drain(..extra);
            for l in self.lines.iter_mut().filter(|l| l.row < 0.0) {
                l.row = 0.0;
            }
        }
        let n = self.lines.len();
        let step = if SLIDE > 0.0 { dt / SLIDE } else { 1.0 };
        for (i, l) in self.lines.iter_mut().enumerate() {
            let slot = (n - 1 - i) as f32;
            l.row = if l.row < slot { (l.row + step).min(slot) } else { slot };
        }
        if !visible {
            return;
        }
        let Some(font) = fonts.get("WizardFancy") else { return };
        let s = screen[1] / 768.0;
        for l in &self.lines {
            let alpha = Self::alpha((now - l.born).as_secs_f32());
            if alpha <= 0.0 {
                continue;
            }
            // Wide enough for the line (no wrap), centred across the screen, its middle `row` pitches over BOTTOM.
            let width = (font.width(&l.text) * SCALE + 20.0).min(screen[0] / s);
            let height = font.line_height * SCALE;
            let middle = BOTTOM - l.row * PITCH;
            let canvas = Canvas {
                draw,
                textures,
                fonts,
                origin: [(screen[0] - width * s) / 2.0, (middle - height / 2.0) * s],
                s,
                mouse: [-1.0e4; 2],
                mouse_down: false,
                alpha,
                hole: std::cell::Cell::new(None),
            };
            let style = TextStyle::new("WizardFancy", SCALE,
                                       ImColor32::from_rgba(COLOUR[0], COLOUR[1], COLOUR[2], (255.0 * alpha) as u8))
                .centred().outlined().shadowed();
            canvas.text(&l.text, [0.0, 0.0, width, height], &style, false);
        }
    }
}
