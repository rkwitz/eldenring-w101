//! The Unicorn Way quests for the Quests page and NPC dialogue: `w101_orbs/book/quests.tsv` (made by
//! tools/w101_book_quests.py) plus the quest event flags the map's events set.
//!
//! Quest n uses flags 1042366000 + 10n: +0 assigned (stays on), +1..+6 goal i done, +8 ready to turn in, +9 complete.
//! A quest is shown while assigned and not complete; its current goals are those not done whose "after" goals are.

use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Goal {
    pub flag: u32,
    pub after: Vec<usize>,
    pub count: u32,
    pub counter: Counter,
    pub text: String,
    pub place: String,
}

#[derive(Clone, Debug, Default)]
pub enum Counter {
    #[default]
    None,
    /// A 4-bit event value on four flags, the first the most significant.
    Bits(u32),
    /// How many of these flags are on.
    Flags(Vec<u32>),
}

#[derive(Clone, Debug, Default)]
pub struct Quest {
    pub slug: String,
    pub title: String,
    pub main: bool,
    pub assigned: u32,
    pub ready: u32,
    pub complete: u32,
    pub giver: String,
    pub turn_in: String,
    pub goals: Vec<Goal>,
    /// Dialogue lines in client order.
    pub text: Vec<String>,
}

/// A quest in progress as the Quests page shows it.
#[derive(Clone, Debug)]
pub struct Active {
    pub title: String,
    pub main: bool,
    /// The current goal's text, and its count (done, needed) for counted goals.
    pub goal: String,
    pub progress: Option<(u32, u32)>,
    pub other_goals: Vec<String>,
    pub place: String,
    pub ready: bool,
    pub turn_in: String,
}

#[derive(Default)]
pub struct Quests {
    pub quests: Vec<Quest>,
    /// NPC slug -> (talk flag, name).
    pub npcs: Vec<(String, u32, String)>,
    /// Dialogue for a talk press: (quest slug, event: offer, goal<n>, turnin or underway) -> lines.
    pub dialogue: HashMap<(String, String), Vec<String>>,
}

/// What a talk press did, from the quest flags that changed on it (the quests README): assigned on = the offer, a
/// goal flag on = that goal's talk, complete on = the turn-in; none of those = the quest is underway.
#[derive(Clone, Debug, PartialEq)]
pub enum TalkEvent {
    Offer,
    Goal(usize),
    TurnIn,
    Underway,
}

/// A dialogue to show for a talk press.
#[derive(Clone, Debug)]
pub struct Talk {
    pub npc: String,
    pub quest: String,
    pub event: TalkEvent,
    pub lines: Vec<String>,
}

impl Quests {
    pub fn load(path: &Path) -> Quests {
        let mut out = Quests::default();
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut index: HashMap<String, usize> = HashMap::new();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let num = |i: usize| f.get(i).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
            match f.first().copied() {
                Some("Q") if f.len() >= 9 => {
                    index.insert(f[1].to_string(), out.quests.len());
                    out.quests.push(Quest {
                        slug: f[1].into(),
                        title: f[2].into(),
                        main: f[3] == "main",
                        assigned: num(4),
                        ready: num(5),
                        complete: num(6),
                        giver: f[7].into(),
                        turn_in: f[8].into(),
                        ..Quest::default()
                    });
                }
                Some("G") if f.len() >= 9 => {
                    let Some(&q) = index.get(f[1]) else { continue };
                    let counter = if let Some(list) = f[6].strip_prefix("list:") {
                        Counter::Flags(list.split(',').filter_map(|v| v.parse().ok()).collect())
                    } else {
                        match f[6].parse::<u32>() {
                            Ok(0) | Err(_) => Counter::None,
                            Ok(base) => Counter::Bits(base),
                        }
                    };
                    out.quests[q].goals.push(Goal {
                        flag: num(3),
                        after: f[4].split(',').filter_map(|v| v.parse().ok()).collect(),
                        count: num(5).max(1),
                        counter,
                        text: f[7].into(),
                        place: f[8].into(),
                    });
                }
                Some("X") if f.len() >= 4 => {
                    if let Some(&q) = index.get(f[1]) {
                        out.quests[q].text.push(f[3].to_string());
                    }
                }
                Some("D") if f.len() >= 4 => {
                    out.dialogue.entry((f[1].to_string(), f[2].to_string())).or_default().push(f[3].to_string());
                }
                Some("N") if f.len() >= 4 => {
                    out.npcs.push((f[1].into(), num(2), f[3].into()));
                }
                _ => {}
            }
        }
        out
    }

    /// Every flag the quests read, to fetch from the game once per refresh.
    pub fn flags(&self) -> Vec<u32> {
        let mut out = Vec::new();
        for q in &self.quests {
            out.extend([q.assigned, q.ready, q.complete]);
            for g in &q.goals {
                out.push(g.flag);
                match &g.counter {
                    Counter::Bits(base) => out.extend(*base..*base + 4),
                    Counter::Flags(list) => out.extend(list),
                    Counter::None => {}
                }
            }
        }
        out.extend(self.npcs.iter().map(|n| n.1));
        out.retain(|f| *f != 0);
        out.sort();
        out.dedup();
        out
    }

    /// The quests in progress, main quests first (the order of the file within each).
    pub fn active(&self, flag: &dyn Fn(u32) -> bool) -> Vec<Active> {
        let mut out: Vec<Active> = Vec::new();
        for q in &self.quests {
            if !flag(q.assigned) || flag(q.complete) {
                continue;
            }
            let done = |n: usize| q.goals.get(n.wrapping_sub(1)).is_some_and(|g| flag(g.flag));
            let mut current = Vec::new();
            for (i, g) in q.goals.iter().enumerate() {
                if flag(g.flag) || !g.after.iter().all(|a| done(*a)) {
                    continue;
                }
                let progress = match &g.counter {
                    Counter::None => None,
                    Counter::Bits(base) => Some((0..4).fold(0, |v, b| v << 1 | flag(base + b) as u32)),
                    Counter::Flags(list) => Some(list.iter().filter(|f| flag(**f)).count() as u32),
                };
                let progress = progress.filter(|_| g.count > 1).map(|n| (n.min(g.count), g.count));
                current.push((i, g.text.clone(), g.place.clone(), progress));
            }
            let ready = flag(q.ready) || current.is_empty();
            let (goal, place, progress) = match current.first() {
                Some((_, text, place, progress)) if !flag(q.ready) => (text.clone(), place.clone(), *progress),
                _ => (format!("Return to {}", q.turn_in), String::new(), None),
            };
            out.push(Active {
                title: q.title.clone(),
                main: q.main,
                goal,
                progress,
                other_goals: if flag(q.ready) { Vec::new() } else { current.iter().skip(1).map(|c| c.1.clone()).collect() },
                place: if place.is_empty() { "Unicorn Way".into() } else { place },
                ready,
                turn_in: q.turn_in.clone(),
            });
        }
        out.sort_by_key(|a| !a.main);
        out
    }

    /// The dialogue for a press of `npc_name`'s talk button: `before` are the flags before the press, `after` after.
    pub fn talk(&self, npc_name: &str, before: &dyn Fn(u32) -> bool, after: &dyn Fn(u32) -> bool) -> Option<Talk> {
        let involved = |q: &Quest| {
            q.giver == npc_name || q.turn_in == npc_name || q.goals.iter().any(|g| g.text.contains(npc_name))
        };
        let changed = |f: u32| f != 0 && !before(f) && after(f);
        let mut found: Option<(&Quest, TalkEvent)> = None;
        for q in self.quests.iter().filter(|q| involved(q)) {
            let event = if changed(q.complete) {
                Some(TalkEvent::TurnIn)
            } else if let Some(n) = q.goals.iter().position(|g| changed(g.flag)) {
                Some(TalkEvent::Goal(n + 1))
            } else if changed(q.assigned) {
                Some(TalkEvent::Offer)
            } else {
                None
            };
            if let Some(event) = event {
                found = Some((q, event));
                break;
            }
        }
        if found.is_none() {
            found = self
                .quests
                .iter()
                .find(|q| involved(q) && after(q.assigned) && !after(q.complete))
                .map(|q| (q, TalkEvent::Underway));
        }
        let (q, event) = found?;
        let key = match &event {
            TalkEvent::Offer => "offer".to_string(),
            TalkEvent::Goal(n) => format!("goal{n}"),
            TalkEvent::TurnIn => "turnin".to_string(),
            TalkEvent::Underway => "underway".to_string(),
        };
        let current = self.active(after).into_iter().find(|a| a.title == q.title);
        let lines = match self.dialogue.get(&(q.slug.clone(), key)) {
            Some(lines) => lines.clone(),
            // Without dialogue sorted by event: the quest's opening lines for the offer, else the quest state.
            None => {
                let mut lines = Vec::new();
                if event == TalkEvent::Offer {
                    lines.extend(
                        q.text.iter().filter(|t| t.len() > 30 && t.contains(' ')).take(3).cloned(),
                    );
                }
                match (&event, &current) {
                    (TalkEvent::TurnIn, _) => lines.push(format!("Quest complete: {}", q.title)),
                    (_, Some(a)) => lines.push(format!("{}: {}", q.title, a.goal)),
                    _ => {}
                }
                lines
            }
        };
        (!lines.is_empty()).then(|| Talk { npc: npc_name.to_string(), quest: q.title.clone(), event, lines })
    }

}
