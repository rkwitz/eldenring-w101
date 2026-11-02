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
    pub fn frame(&mut self, playing: bool, street: Option<[f32; 3]>) {
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
