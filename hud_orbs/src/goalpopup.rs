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
//! Round 26 (W5, Ryan: "several at once must appear one after another with a delay, not all together"): queued
//! lines come up one at a time, STAGGER s apart, each with its own sound. "Return to <NPC>" goals (done by walking
//! up to the NPC, e.g. Sinister Skeleton's near_ceren) pop nothing, nor does the "Talk to" goal they lead to: Ryan got
//! "Goal Complete: Return to Ceren Nightchant / New Goal: Talk to Ceren Nightchant" at Ceren before talking to her;
//! the talk itself then shows "Goal Complete" and "Quest Complete" as before.
//! Round 26 (W7, Ryan: "the quest popup text is too big"): measured again at Ryan's 2560x1440 (log: "screen
//! 2560x1440, HUD scale 1.875") against ref/1 and ref/3 (their game picture is 694 px high, rows 25..718): the
//! small capitals are 14 px = 2.0% of the screen height there (29 px at 1440), the capitals 16 px = 2.3%, the lines
//! 25 px = 3.6% apart. Round 25 drew small capitals 31 px and capitals 33 px high (preview 25b at 2560x1440), so
//! SCALE goes from 0.643 to 0.60 (small capitals 29 px) and PITCH from 29 to 27.7 (52 px); the outline was the other
//! half of "too big": W101's edge is a thin dark rim, ours a heavy black outline plus shadow, now a shadow only.
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
const SCALE: f32 = 0.60;
const COLOUR: [u8; 3] = [238, 234, 36];
const BOTTOM: f32 = 517.0;
const PITCH: f32 = 27.7;
/// Seconds fully shown, then fading out; seconds an older line takes to slide up a row; most lines at once.
const HOLD: f32 = 4.0;
const FADE: f32 = 1.5;
const SLIDE: f32 = 0.2;
const MAX_LINES: usize = 6;
/// Round 26 (W5): seconds between two queued lines coming up.
const STAGGER: f32 = 0.9;
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
    /// Popups waiting to come up (one every STAGGER s while visible; while hidden they wait).
    queue: VecDeque<String>,
    /// When the last queued line came up.
    last_shown: Option<Instant>,
}

fn progress(counter: &Counter, flag: &dyn Fn(u32) -> bool) -> Option<u32> {
    match counter {
        Counter::None => None,
        Counter::Bits(base) => Some(crate::quests::event_value(*base, flag)),
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
        // Round 29 (Q1): a quest assigned and completed in one change was handed over already done (Unicorn Way's
        // "greet" at Private Connelly, w101_quests.py): Ryan never had it from Merle, so it pops nothing and his
        // first talk reads as Connelly's own fresh quest (log 03:27:57: flags 300, 301, 308, 309 on at once, then
        // "Goal Complete: Talk to Private Connelly" and "Quest Complete: Unicorn Way").
        if fresh && new(q.complete) {
            continue;
        }
        if fresh && !new(q.complete) {
            out.push(format!("New Quest: {}", q.title));
        }
        let current = |flag: &dyn Fn(u32) -> bool, i: usize| {
            let g = &q.goals[i];
            flag(q.assigned) && !flag(g.flag) && !flag(q.complete)
                && g.after.iter().all(|a| q.goals.get(a.wrapping_sub(1)).is_some_and(|d| flag(d.flag)))
        };
        // Round 26 (W5): goals done by walking up to an NPC, and the talk goal right after one, pop nothing.
        let silent = |i: usize| q.goals.get(i).is_some_and(|g| g.text.starts_with("Return to "));
        for (i, g) in q.goals.iter().enumerate() {
            let quiet_new = g.after.iter().any(|a| silent(a.wrapping_sub(1)));
            if !old(g.flag) && new(g.flag) {
                if silent(i) {
                    continue;
                }
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
            if !fresh && !quiet_new && !current(old, i) && current(new, i) {
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
        let due = self.last_shown.is_none_or(|t| (now - t).as_secs_f32() >= STAGGER);
        if visible && due && !self.queue.is_empty() {
            crate::music::sfx(SOUND);
            self.last_shown = Some(now);
            // The new line comes in at the bottom, the older ones keep their row and slide up from there.
            if let Some(text) = self.queue.pop_front() {
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
                .centred().shadowed();
            canvas.text(&l.text, [0.0, 0.0, width, height], &style, false);
        }
    }
}
