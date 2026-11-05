//! Round 32 (G2, Ryan: "NPCs turn to face you when you talk to them, and turn back to their original facing when the
//! talk ends", as in Wizard101). While the book shows a conversation, the picker or the shop (Book::talking), the
//! street NPC nearest the player (within REACH m, markers::NPCS) is turned toward him at TURN_RATE; when the talk
//! ends it turns back to the yaw it had when the talk began, then is left to its AI again.
//!
//! The turn is written into the NPC's physics orientation every frame (Game::set_yaw, the orientation and the
//! interpolated one), so its AI can't turn it away mid-talk.
//!
//! Round 35 (N3, Ryan: "Private Conley kind of like glitches when he tries to turn to talk to you ... he can't like
//! fully turn around"): each frame's step used to start from the yaw read back from the game. Connelly is the one
//! street NPC on his default idle (animation 0, not a forced loop, w101_quests.IDLE_BY_SLUG), so his AI keeps turning
//! him back toward his own heading between our writes; starting every step from the read-back yaw let the two
//! cancel out partway (he stopped short and jittered there). The turn is now driven by our own yaw (`yaw`), written
//! every frame whatever the game did to it in between, so he reaches the player and comes back like the others.
//! Where the read-back yaw differs from what we wrote the frame before by over 1 degree, that is logged (at most
//! twice a second): "npc turn: X was turned N deg away from our yaw". Facing convention: Game::yaw_of (forward = -(sin a,
//! cos a)): the prompt's calibrated "-z" and the MSB door regions (W101_Garden_in faces away from W101_Garden_exit
//! with rot 153 only under -z) agree on it. Log: "npc turn: X turns to face the player" / "turned back".

use std::time::Instant;

use crate::game::Game;

const REACH: f32 = 3.5;
/// Radians per second (Wizard101's NPCs turn in about a third of a second).
const TURN_RATE: f32 = 8.0;

#[derive(Default)]
pub struct NpcTurn {
    /// The NPC being turned: entity id, name, its yaw when the talk began.
    npc: Option<(u32, &'static str, f32)>,
    last: Option<Instant>,
    /// Round 35 (N3): the yaw we set last frame (the turn is integrated from this, not from the game's yaw).
    yaw: f32,
    logged: Option<Instant>,
}

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl NpcTurn {
    pub fn frame(&mut self, game: Option<&Game>, talking: bool) {
        let now = Instant::now();
        let dt = self.last.map_or(0.0, |t| (now - t).as_secs_f32()).min(0.1);
        self.last = Some(now);
        let Some(game) = game else {
            self.npc = None;
            return;
        };
        let Some(player) = game.position_of(None) else { return };
        if talking && self.npc.is_none() {
            let d = |p: [f32; 3]| (p[0] - player[0]).hypot(p[2] - player[2]);
            let near = game.entities(&crate::markers::MODELS, &crate::markers::entity_ids()).into_iter()
                .filter(|(_, at)| d(*at) < REACH).min_by(|a, b| d(a.1).total_cmp(&d(b.1)));
            if let Some((id, at)) = near {
                if let (Some(home), Some(npc)) = (game.yaw_of(Some(id)), crate::markers::NPCS.iter().find(|n| n.2 == id)) {
                    tracing::info!("npc turn: {} ({:.1} m away) turns to face the player (yaw now {:.0} deg)", npc.1,
                                   d(at), home.to_degrees());
                    self.npc = Some((id, npc.1, home));
                    self.yaw = home;
                }
            }
        }
        let Some((id, name, home)) = self.npc else { return };
        let Some(at) = game.position_of(Some(id)) else {
            self.npc = None;
            return;
        };
        // forward = -(sin a, cos a) toward the player: a = atan2(-dx, -dz)
        let want = if talking { (at[0] - player[0]).atan2(at[2] - player[2]) } else { home };
        let Some(read_back) = game.yaw_of(Some(id)) else { return };
        let drift = wrap(read_back - self.yaw);
        if drift.abs() > 1.0f32.to_radians() && self.logged.map_or(true, |t| (now - t).as_secs_f32() > 0.5) {
            self.logged = Some(now);
            tracing::info!("npc turn: {name} was turned {:.1} deg away from our yaw (ours {:.0}, game {:.0})",
                           drift.to_degrees(), self.yaw.to_degrees(), read_back.to_degrees());
        }
        let diff = wrap(want - self.yaw);
        let step = TURN_RATE * dt;
        let next = if diff.abs() <= step { want } else { wrap(self.yaw + step * diff.signum()) };
        self.yaw = next;
        game.set_yaw(Some(id), next);
        if !talking && diff.abs() <= step {
            tracing::info!("npc turn: {name} turned back to {:.0} deg", home.to_degrees());
            self.npc = None;
        }
    }
}
