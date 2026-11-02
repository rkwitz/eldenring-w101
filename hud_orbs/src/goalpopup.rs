//! Round 23 (hud H7, Ryan: "Make sure you also list the goal update when a quest parameter is filled. It should show
//! that on the screen and then fade out."): Wizard101's quest popups.
//!
//! Wizard101's text (Root.wad Locale/en-US/Quest.lang): QuestCompletePopup "Quest Complete: $QUEST_NAME$",
//! NewQuestPopup "New Quest: $QUEST_NAME$", *GoalCompletePopupText "Goal Complete: #1:$GOAL_NAME$ ...", *GoalReward
//! "Goal Update: #1:$GOAL_NAME$ #2:$GOAL_COUNT$ of #3:$GOAL_TOTAL$ ...", *NewGoalPopupText "New Goal: #1:$GOAL_NAME$".
//! Its window (Root.wad GUI/WizardPopupText.gui): wndWizardPopupText 550 x 90 at (300, 300) of the 768-high layout,
//! one ControlText textText filling it, WizardFancy at 0.8, white (FFFFFFFF), drop shadow, no word wrap; it is
//! placed and faded by the client's code (not in the files), so here it is centred across the screen at that height,
//! held HOLD s and faded over FADE s, one popup after another. Sound: _Shared-WorldData.wad
//! Sound/GUI/ui_quest_updated.wav (already in orbs\w101_orbs\sfx, round 22 S3), through music::sfx.
//!
//! Driven by the quest flags book.rs polls (poll_quests): `messages` compares the flags before and after.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use hudhook::imgui::{DrawListMut, ImColor32};

use crate::font::BitmapFont;
use crate::layout::{Canvas, TextStyle, Textures};
use crate::quests::{Counter, Quests};

/// WizardPopupText.gui: the window (layout pixels of a 768-high screen) and the font scale.
const WINDOW: [f32; 4] = [300.0, 300.0, 850.0, 390.0];
const SCALE: f32 = 0.8;
/// Seconds fully shown, then fading out.
const HOLD: f32 = 3.0;
const FADE: f32 = 1.0;
const SOUND: &str = "ui_quest_updated";

#[derive(Default)]
pub struct GoalPopups {
    queue: VecDeque<String>,
    shown: Option<(String, Instant)>,
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

    /// Preview: show `text` as if it came up `age` seconds ago.
    pub fn preview(&mut self, text: &str, age: f32) {
        self.shown = Some((text.to_string(), Instant::now() - std::time::Duration::from_secs_f32(age)));
    }

    /// Draws the current popup (when `visible`; time runs on regardless), taking the next one from the queue.
    pub fn draw(&mut self, draw: &DrawListMut, textures: &Textures, fonts: &HashMap<String, BitmapFont>,
                screen: [f32; 2], visible: bool) {
        if self.shown.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() > HOLD + FADE) {
            self.shown = None;
        }
        if self.shown.is_none() && visible {
            if let Some(m) = self.queue.pop_front() {
                crate::music::sfx(SOUND);
                self.shown = Some((m, Instant::now()));
            }
        }
        let (Some((text, t)), true) = (&self.shown, visible) else { return };
        let Some(font) = fonts.get("WizardFancy") else { return };
        let age = t.elapsed().as_secs_f32();
        let alpha = if age < HOLD { 1.0 } else { (1.0 - (age - HOLD) / FADE).clamp(0.0, 1.0) };
        let s = screen[1] / 768.0;
        // The window's width, widened for a long line (the window doesn't wrap), centred across the screen.
        let width = (WINDOW[2] - WINDOW[0]).max(font.width(text) * SCALE + 20.0).min(screen[0] / s);
        let canvas = Canvas {
            draw,
            textures,
            fonts,
            origin: [(screen[0] - width * s) / 2.0, WINDOW[1] * s],
            s,
            mouse: [-1.0e4; 2],
            mouse_down: false,
            alpha,
            hole: std::cell::Cell::new(None),
        };
        let style = TextStyle::new("WizardFancy", SCALE, ImColor32::from_rgba(255, 255, 255, (255.0 * alpha) as u8))
            .centred().shadowed();
        canvas.text(text, [0.0, 0.0, width, WINDOW[3] - WINDOW[1]], &style, false);
    }
}
