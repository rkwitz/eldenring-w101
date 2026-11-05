//! Round 32 (G7, Ryan: "Hedge maze doors: keep the riding state through the warp: ride in -> still riding out"; and
//! G3, the maze's "Unicorn Way" title). The riding branch of the hedge maze doors (tools/emevd.py rattlebones_doors)
//! used a map warp (2003[14]): a map load, which always sets the player down on foot (nothing in EMEVD mounts him
//! again) and shows the arrival's area title. Ryan's round 31 log (w101_orbs.log.round32_input) has both maze entries
//! riding (03:05:11 and 03:08:07: "player checks no longer scored ... (riding Torrent)" before, "(on foot)" at the
//! arrival 6 s later, world shifted with the other characters = a map load), and the title showed although the
//! event turns the area welcome message off (2012[8] 0) right before that warp.
//!
//! Now the riding branch first asks the DLL: it turns REQUESTS[k].0 on and waits WAIT s. The DLL moves Torrent and
//! the rider to the arrival region (MSB position made physics through the player's MSB position, questhelper::
//! player_msb_now), turned to the region's rotation, and once Torrent is there turns DONE on; the event then skips
//! the map warp. No load: still riding, no title. When the DLL can't (no MSB position, Torrent not loaded, the move
//! didn't take), DONE stays off and the event does the old map warp. Log: "ride warp: ...".

use std::time::Instant;

use crate::game::{Game, TORRENT};

/// (request flag, arrival region MSB position, its rotation y in degrees) per hedge maze door
/// (m60_42_36_00 W101_Garden_in 1042362805 / W101_Garden_out 1042362807; emevd.py RIDE_WARPS).
pub const REQUESTS: [(u32, [f32; 3], f32, &str); 2] = [
    (1042362920, [-320.138_34, 95.678_8, 74.260_185], 152.988_08, "into the hedge maze"),
    (1042362921, [-356.827_06, 95.678_8, -34.575_61], -118.111_92, "out to Unicorn Way"),
];
pub const DONE: u32 = 1042362922;
/// Round 35: off (see `frame`).
const ENABLED: bool = false;
/// Seconds the move is written for, and after which it counts as failed when Torrent isn't there.
const HOLD: f32 = 0.25;
const GIVE_UP: f32 = 1.0;
const NEAR: f32 = 3.0;

#[derive(Default)]
pub struct RideWarp {
    /// The move on its way: request index, when it began, Torrent's and the rider's physics targets.
    pending: Option<(usize, Instant, [f32; 3], [f32; 3])>,
}

impl RideWarp {
    pub fn frame(&mut self, game: Option<&Game>) {
        // Round 35 (Ryan: "going through the gate to the hedge maze on Torrent caused me to fall out of the map and
        // die"): the first real use of this move (log 05:23:35) put Torrent "0.00 m from the arrival" in physics
        // terms, yet 1.5 s later the player was ~726 m from the street NPCs and ~50 m above them, fell and died. The
        // physics frame the target was computed in was not the one the game used once the move landed. Until that is
        // understood the DLL leaves the request alone: the door event's map warp (RIDE_WAIT later) takes the player in,
        // as before round 32 (Torrent is dismissed by the load).
        if !ENABLED {
            return;
        }
        let Some(game) = game else { return };
        if self.pending.is_none() {
            let Some(k) = REQUESTS.iter().position(|r| crate::game::event_flag(r.0) == Some(true)) else { return };
            crate::game::set_event_flag(REQUESTS[k].0, false);
            let (_, msb, ry, what) = REQUESTS[k];
            let rider = game.position_of(None);
            let torrent = game.position_of(Some(TORRENT));
            // Round 35: only when at least two loaded NPCs agree on where the player is, and they are most of the
            // loaded ones (questhelper::player_msb_consensus); otherwise the event's map warp (a loading screen,
            // Torrent dismissed) is the safe way in.
            let entities = game.entities(&crate::markers::MODELS, &crate::markers::entity_ids());
            let consensus = game.camera().and_then(|c| crate::questhelper::player_msb_consensus(&c, &entities));
            if let Some((p, agree, loaded)) = consensus {
                tracing::info!("ride warp {what}: player at MSB {p:?} ({agree} of {loaded} street NPCs agree)");
            }
            let msb_now = consensus.filter(|(_, agree, loaded)| *agree >= 2 && agree * 2 > *loaded).map(|c| c.0);
            let (Some(rider), Some(torrent), Some(msb_now)) = (rider, torrent, msb_now) else {
                tracing::info!("ride warp {what}: not moved (rider {rider:?}, Torrent {torrent:?}, MSB position \
                                {msb_now:?}); the event's map warp does it");
                return;
            };
            // MSB -> physics is a translation (the world shifts move both together; player_msb_now maps the rider's
            // physics position): Torrent (his position is at his feet) goes to the region's ground point, the rider
            // keeps his offset from Torrent.
            let target: [f32; 3] = std::array::from_fn(|i| msb[i] + rider[i] - msb_now[i]);
            let seat: [f32; 3] = std::array::from_fn(|i| target[i] + rider[i] - torrent[i]);
            tracing::info!("ride warp {what}: Torrent {torrent:?} -> {target:?}, rider -> {seat:?}, facing {ry:.0} deg");
            self.pending = Some((k, Instant::now(), target, seat));
        }
        let Some((k, since, target, seat)) = self.pending else { return };
        let t = since.elapsed().as_secs_f32();
        let a = REQUESTS[k].2.to_radians();
        if t < HOLD {
            game.set_position(Some(TORRENT), target);
            game.set_position(None, seat);
            game.set_yaw(Some(TORRENT), a);
            game.set_yaw(None, a);
            return;
        }
        let at = game.position_of(Some(TORRENT));
        let off = at.map(|p| (p[0] - target[0]).hypot(p[2] - target[2]));
        if off.is_some_and(|d| d < NEAR) {
            tracing::info!("ride warp {}: Torrent is {:.2} m from the arrival after {t:.2} s; map warp skipped (still \
                            riding, no map load)", REQUESTS[k].3, off.unwrap_or(0.0));
            crate::game::set_event_flag(DONE, true);
            self.pending = None;
        } else if t > GIVE_UP {
            tracing::info!("ride warp {}: Torrent not at the arrival ({off:?} m off); the event's map warp does it",
                           REQUESTS[k].3);
            self.pending = None;
        }
    }
}
