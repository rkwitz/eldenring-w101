//! Round 24 (M1, Ryan: "enemies should not aggro while the player is on the sidewalk; only when the player steps into
//! the road (as in W101, where mobs stay on the road paths)").
//!
//! Wizard101's mobs walk paths on the road (every Lost Soul / Skeletal Pirate / Dark Fairy path node lies on the
//! street model's Street* road meshes or in a duel circle) and a duel starts only when the player steps into one of
//! the road's combat sigils, so walking the pavement past them is safe. Here w101_orbs/road_mask.tsv
//! (tools/w101_road_mask.py, made on the PC from the W101 street model) holds the road triangles ("R"), the 7 m
//! sigil circles ("C") and the street's box ("B"), in map (MSB) metres. While the player (lib.rs: the street MSB
//! position, questhelper::player_msb_now) is inside the box, at street height (not in a room under it) and outside
//! the hedge maze, on no road triangle and in no circle, this sets SIDEWALK_FLAG; m60_42_36_00.emevd event
//! 1042362560 (tools/emevd.py --sidewalk) puts SpEffect 99006 on the player while the flag is on (vanilla row: the
//! enemies' sight and hearing search rate of the player x0, no visual effect, not saved) and clears it when it
//! goes off. Going back onto the road clears it at once; onto the pavement only after SETTLE s on it, so stepping
//! along the kerb doesn't flicker. No position (off the street, loading) = the road (flag off).

use std::path::Path;
use std::time::Instant;

/// Event flag the DLL sets while the player is off the road (transient block: cleared on every map load, which is
/// also what we want, the DLL sets it again within CHECK s).
pub const SIDEWALK_FLAG: u32 = 1042362560;
/// Seconds between position checks.
const CHECK: f32 = 0.2;
/// Seconds on the pavement before the flag goes on.
const SETTLE: f32 = 0.5;
/// Below this map height the player is in one of the rooms under the street (questhelper::in_room).
const ROOM_HEIGHT: f32 = 70.0;

#[derive(Default)]
pub struct Sidewalk {
    road: Vec<[[f32; 2]; 3]>,
    circles: Vec<[f32; 3]>,
    bounds: Option<[f32; 4]>,
    last_check: Option<Instant>,
    off_road_since: Option<Instant>,
    /// What we last wanted the flag to be (logged on change).
    wanted: Option<bool>,
    /// Round 25 M1: the quest reset at the Roundtable Hold.
    reset: QuestReset,
}

impl Sidewalk {
    pub fn load(path: &Path) -> Sidewalk {
        let mut s = Sidewalk::default();
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                tracing::info!("sidewalk: no road mask {} ({e}); enemies see the player everywhere", path.display());
                return s;
            }
        };
        for line in text.lines() {
            let mut f = line.split_whitespace();
            let tag = f.next();
            let v: Vec<f32> = f.filter_map(|x| x.parse().ok()).collect();
            match (tag, v.len()) {
                (Some("R"), 6) => s.road.push([[v[0], v[1]], [v[2], v[3]], [v[4], v[5]]]),
                (Some("C"), 3) => s.circles.push([v[0], v[1], v[2]]),
                (Some("B"), 4) => s.bounds = Some([v[0], v[1], v[2], v[3]]),
                _ => {}
            }
        }
        tracing::info!("sidewalk: road mask {} road triangles, {} sigil circles, box {:?}", s.road.len(),
                       s.circles.len(), s.bounds);
        s
    }

    /// Whether map point `p` is on the street off the road (the pavement, lawns, plazas).
    pub fn off_road(&self, p: [f32; 3]) -> bool {
        let Some(b) = self.bounds else { return false };
        let q = [p[0], p[2]];
        if p[1] < ROOM_HEIGHT || crate::questhelper::in_garden(p) || q[0] < b[0] || q[1] < b[1] || q[0] > b[2]
            || q[1] > b[3] {
            return false;
        }
        let in_circle = self.circles.iter().any(|c| (q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2) < c[2] * c[2]);
        !in_circle && !self.road.iter().any(|t| inside(q, t))
    }

    /// Once a frame in play: `street` = the player's MSB position, `playing` = in the game with a player.
    /// `position` = Game::position (chunk position, block, block origin), for the Roundtable Hold's quest reset.
    pub fn frame(&mut self, playing: bool, street: Option<[f32; 3]>, position: Option<([f32; 3], i32, i32)>) {
        self.reset.frame(playing, position.map(|p| p.1));
        if !playing || self.bounds.is_none() {
            return;
        }
        let now = Instant::now();
        if self.last_check.is_some_and(|t| (now - t).as_secs_f32() < CHECK) {
            return;
        }
        self.last_check = Some(now);
        let off = street.is_some_and(|p| self.off_road(p));
        self.off_road_since = if off { self.off_road_since.or(Some(now)) } else { None };
        let want = self.off_road_since.is_some_and(|t| (now - t).as_secs_f32() >= SETTLE);
        if self.wanted != Some(want) {
            tracing::info!("sidewalk: player {} at MSB {:?}; flag {SIDEWALK_FLAG} {}",
                           if want { "off the road (enemies don't see him)" } else { "on the road or off the street" },
                           street.map(|p| p.map(|c| c.round() as i32)), if want { "on" } else { "off" });
            self.wanted = Some(want);
        }
        // Re-set whenever it differs: a map load clears the 1042362xxx block.
        if crate::game::event_flag(SIDEWALK_FLAG).is_some_and(|on| on != want) {
            crate::game::set_event_flag(SIDEWALK_FLAG, want);
        }
    }
}

fn inside(p: [f32; 2], t: &[[f32; 2]; 3]) -> bool {
    let side = |a: [f32; 2], b: [f32; 2]| (p[0] - b[0]) * (a[1] - b[1]) - (a[0] - b[0]) * (p[1] - b[1]);
    let d = [side(t[0], t[1]), side(t[1], t[2]), side(t[2], t[0])];
    let (lo, hi) = (d.iter().cloned().fold(f32::MAX, f32::min), d.iter().cloned().fold(f32::MIN, f32::max));
    !(lo < 0.0 && hi > 0.0)
}

/// Round 25 M1 (Ryan: "going to the Roundtable Hold resets ALL quest state so I can replay"). While the player is in
/// m11_10 (the Roundtable Hold), every quest flag goes off once per visit: the quests' assigned / goal / ready /
/// reward / complete flags, kill counters, Bone Cages, dialogue requests and answers, the intro (the same ranges as
/// tools/w101_quests.QUEST_FLAG_RANGES) and QUEST_RESET, so m60_42_36_00's reset event (emevd_quests.reset_event)
/// also runs again on the next Unicorn Way load. If the flag manager doesn't hold the quests' saved group in the
/// Roundtable, the reset waits and runs as soon as it does (log `quest reset:`).
#[derive(Default)]
struct QuestReset {
    /// The reset is owed (entered the Roundtable, not done yet).
    pending: bool,
    /// Done during this Roundtable visit.
    done: bool,
}

/// m11_10_00_00 as Game::position's block id (area << 24 | block << 16 | region << 8 | index).
const ROUNDTABLE: u32 = 0x0B0A_0000;
/// (first, last) inclusive: w101_quests.QUEST_FLAG_RANGES plus QUEST_RESET (1042360691).
const QUEST_FLAG_RANGES: [(u32, u32); 6] = [(1042360300, 1042360459), (1042362400, 1042362499),
    (1042360500, 1042360543), (1042362600, 1042362621), (1042360689, 1042360689), (1042360691, 1042360691)];

impl QuestReset {
    fn frame(&mut self, playing: bool, block: Option<i32>) {
        if !playing {
            return;
        }
        let in_roundtable = block.is_some_and(|b| (b as u32) & 0xFFFF_0000 == ROUNDTABLE);
        if in_roundtable && !self.done && !self.pending {
            self.pending = true;
            tracing::info!("quest reset: in the Roundtable Hold (block {:#010x}); clearing every quest flag",
                           block.unwrap_or(0));
        }
        if !in_roundtable {
            self.done = false;
        }
        if !self.pending {
            return;
        }
        let group = QUEST_FLAG_RANGES[0].0 / 1000;
        let held = crate::game::flag_group_exists(group);
        if held != Some(true) {
            return; // retried every frame until the manager holds the saved quest group
        }
        let mut cleared = 0;
        for (first, last) in QUEST_FLAG_RANGES {
            for f in first..=last {
                if crate::game::event_flag(f) == Some(true) {
                    crate::game::set_event_flag(f, false);
                    cleared += 1;
                }
            }
        }
        let left = QUEST_FLAG_RANGES.iter().flat_map(|&(a, b)| a..=b)
            .filter(|f| crate::game::event_flag(*f) == Some(true)).count();
        tracing::info!("quest reset: {cleared} quest flags cleared ({left} still on; {})",
                       if in_roundtable { "in the Roundtable Hold" } else { "after leaving the Roundtable Hold" });
        self.pending = false;
        self.done = in_roundtable;
    }
}
