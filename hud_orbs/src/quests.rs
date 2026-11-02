//! The Unicorn Way quests for the Quests page and the quest offers of the dialogue box (talk.rs):
//! `w101_orbs/book/quests.tsv` (made by tools/w101_book_quests.py) plus the quest event flags the map's events set.
//!
//! Quest n uses flags 1042360300 + 10n (round 21; 1042362000 + 10n before, see `saved`): +0 assigned (stays on), +1..+6 goal i done, +8 ready to turn in, +9 complete.
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
    /// Round 18: the quest card's goal picture (QuestInfoWindow imgGoalObjective1: the creature's or NPC's
    /// portrait, the Bone Cage), an art path, or "".
    pub picture: String,
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
    /// Round 20 (Q3): the reward in runes (REWARDS).
    pub reward: u32,
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
    /// Round 18: the quest's slug, and the goal the card and the quest arrow point to (0-based: the first current
    /// one, or once ready the last, the turn-in), with its picture.
    pub slug: String,
    pub goal_index: usize,
    pub picture: String,
    /// Round 20 (Q3): the reward in runes, shown at the bottom of the card.
    pub reward: u32,
}

/// Round 20 (Q3): each quest's reward in runes (tools/w101_quests.py REWARDS, keep in step): Wizard101's quest
/// rewards are server-side, so these are Wizard City-sized gold amounts given as the same number of runes.
pub const REWARDS: &[(&str, u32)] = &[
    ("unicorn_way", 25), ("ghost_hunters", 35), ("skeleton_crew", 45), ("monsters_and_mazes", 55),
    ("finding_out_why", 75), ("who_could_it_be_now", 60), ("sinister_skeleton", 150),
];
/// Quest base + 7 (w101_quests.REWARD_OFFSET): on once the reward has been given (book.rs sets it).
pub const REWARD_OFFSET: u32 = 7;

/// Round 20 (Q4): Wizard101 goal pictures of an older quests.tsv -> the Elden Ring stand-in's picture
/// (w101_quests.GOAL_PICTURES), used when that file is in the book's art folder.
const ER_PICTURES: &[(&str, &str)] = &[
    ("GUI/NpcPortraits/Portrait_Ghost_LostSoul_MOB_A.png", "GUI/NpcPortraits/ER/lost_soul.png"),
    ("GUI/NpcPortraits/Portrait_Skeleton_Pirate_A_MOB.png", "GUI/NpcPortraits/ER/skeletal_pirate.png"),
    ("GUI/NpcPortraits/Fairy_DarkMinion_MOB.png", "GUI/NpcPortraits/ER/dark_fairy.png"),
    ("GUI/NpcPortraits/Human_WCGuard_A.png", "GUI/NpcPortraits/ER/private_connelly.png"),
    ("GUI/NpcPortraits/Art_Portrait_Boy_Life02.png", "GUI/NpcPortraits/ER/ceren_nightchant.png"),
    ("GUI/NpcPortraits/Human_WCShopKeepF_A_NPC.png", "GUI/NpcPortraits/ER/olivia_dawnwillow.png"),
    ("GUI/NpcPortraits/Portrait_Seraph_Aerial_B_NPC.png", "GUI/NpcPortraits/ER/lady_oriel.png"),
    ("GUI/NpcPortraits/Human_WCGuard_B.png", "GUI/NpcPortraits/ER/private_oryan.png"),
];

/// Round 22 (H7, Ryan: "Monsters and Mazes, hedge maze, there's no picture"): a goal with no picture in quests.tsv
/// gets its place's (goal column "place"): the Hedge Maze is Wizard101's green place disc of a hedge garden,
/// _Shared-WorldData.wad GUI/QuestButtons/Art_Quest_AV_WalledGarden (tools/w101_quest_arrow.py; w101_quests.py
/// GOAL_PICTURES["hedge_maze"]).
const PLACE_PICTURES: &[(&str, &str)] = &[("The Hedge Maze", "GUI/QuestButtons/Art_Quest_AV_WalledGarden.png")];

#[derive(Default)]
pub struct Quests {
    pub quests: Vec<Quest>,
}

/// Round 21 (Q1): the quest state's flags moved from 1042362xxx (a block every map load clears: Ryan's round 20 log
/// read them all off after each warp/reload, so the reset ran again and Unicorn Way paid twice) to the saved
/// 1042360xxx block (w101_quests.QUEST_BASE). A quests.tsv written before that still holds the old numbers.
pub const fn saved(f: u32) -> u32 {
    match f {
        1042362000..=1042362159 => f - 1042362000 + 1042360300, // quests: QUEST_BASE + 10n
        1042362500..=1042362543 => f - 2000,                     // kill counters and cages
        _ => f,
    }
}

impl Quests {
    pub fn load(path: &Path) -> Quests {
        let mut out = Quests::default();
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut index: HashMap<String, usize> = HashMap::new();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let num = |i: usize| saved(f.get(i).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0));
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
                        reward: REWARDS.iter().find(|r| r.0 == f[1]).map_or(0, |r| r.1),
                        ..Quest::default()
                    });
                }
                Some("G") if f.len() >= 9 => {
                    let Some(&q) = index.get(f[1]) else { continue };
                    let counter = if let Some(list) = f[6].strip_prefix("list:") {
                        Counter::Flags(list.split(',').filter_map(|v| v.parse().ok()).map(saved).collect())
                    } else {
                        match f[6].parse::<u32>() {
                            Ok(0) | Err(_) => Counter::None,
                            Ok(base) => Counter::Bits(saved(base)),
                        }
                    };
                    out.quests[q].goals.push(Goal {
                        flag: num(3),
                        after: f[4].split(',').filter_map(|v| v.parse().ok()).collect(), // goal numbers, not flags
                        count: num(5).max(1),
                        counter,
                        text: f[7].into(),
                        place: f[8].into(),
                        picture: {
                            let p = f.get(9).copied().unwrap_or_default();
                            let art = path.parent().map(|b| b.join("art"));
                            match ER_PICTURES.iter().find(|e| e.0 == p) {
                                Some((_, er)) if art.as_ref().is_some_and(|a| a.join(er).exists()) => er.to_string(),
                                // Round 22 (H7): the Hedge Maze goals had no picture in quests.tsv.
                                _ if p.is_empty() => PLACE_PICTURES.iter()
                                    .find(|e| e.0 == f[8] && art.as_ref().is_some_and(|a| a.join(e.1).exists()))
                                    .map_or(String::new(), |e| e.1.to_string()),
                                _ => p.to_string(),
                            }
                        },
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
        let n = out.flags().len();
        tracing::info!("quests: {} quests, {n} saved flags ({}..{})", out.quests.len(),
                       out.flags().first().unwrap_or(&0), out.flags().last().unwrap_or(&0));
        out
    }

    /// Every flag the quests read, to fetch from the game once per refresh.
    pub fn flags(&self) -> Vec<u32> {
        let mut out = Vec::new();
        for q in &self.quests {
            out.extend([q.assigned, q.ready, q.complete, q.assigned + REWARD_OFFSET]);
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
            let (goal, place, progress, goal_index) = match current.first() {
                Some((i, text, place, progress)) if !flag(q.ready) => (text.clone(), place.clone(), *progress, *i),
                _ => (format!("Return to {}", q.turn_in), String::new(), None, q.goals.len().saturating_sub(1)),
            };
            let picture = q.goals.get(goal_index).map(|g| g.picture.clone()).unwrap_or_default();
            out.push(Active {
                title: q.title.clone(),
                main: q.main,
                goal,
                progress,
                other_goals: if flag(q.ready) { Vec::new() } else { current.iter().skip(1).map(|c| c.1.clone()).collect() },
                place: if place.is_empty() { "Unicorn Way".into() } else { place },
                ready,
                turn_in: q.turn_in.clone(),
                slug: q.slug.clone(),
                goal_index,
                picture,
                reward: q.reward,
            });
        }
        out.sort_by_key(|a| !a.main);
        out
    }

    /// Round 20 (Q3): quests complete whose reward hasn't been given: (title, runes, reward flag).
    pub fn rewards_due(&self, flag: &dyn Fn(u32) -> bool) -> Vec<(String, u32, u32)> {
        self.quests
            .iter()
            .filter(|q| q.reward > 0 && flag(q.complete) && !flag(q.assigned + REWARD_OFFSET))
            .map(|q| (q.title.clone(), q.reward, q.assigned + REWARD_OFFSET))
            .collect()
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
