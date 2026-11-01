//! The Unicorn Way quests for the Quests page and the quest offers of the dialogue box (talk.rs):
//! `w101_orbs/book/quests.tsv` (made by tools/w101_book_quests.py) plus the quest event flags the map's events set.
//!
//! Quest n uses flags 1042362000 + 10n: +0 assigned (stays on), +1..+6 goal i done, +8 ready to turn in, +9 complete.
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
    #[allow(dead_code)]
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

    /// The quest card of quest `slug` as it would be just after accepting it (talk.rs's offer page): its first
    /// current goals with nothing done.
    pub fn offer_card(&self, slug: &str) -> Option<Active> {
        let q = self.quests.iter().find(|q| q.slug == slug)?;
        self.active(&|f| f == q.assigned).into_iter().find(|a| a.title == q.title)
    }

    /// Lines for an offer that dialogue.tsv has no lines for: the quest's own opening lines (client order).
    pub fn offer_lines(&self, slug: &str) -> Vec<String> {
        let Some(q) = self.quests.iter().find(|q| q.slug == slug) else { return Vec::new() };
        let mut lines: Vec<String> = q.text.iter().filter(|t| t.len() > 30 && t.contains(' ')).take(3).cloned().collect();
        if lines.is_empty() {
            lines.push(q.title.clone());
        }
        lines
    }
}
