//! Round 26 (O1, Ryan, 3rd report: name tags, quest markers and wisps show through walls, the player and other
//! characters). Rounds 23-25 guessed sight on the CPU (the walk graph, then street_sight.bin voxels from the
//! street's mesh): those know nothing of house shapes, assets, the player or the NPCs, so they could never be right.
//! Now the overlay uses the game's own scene depth: hudhook's depth.rs finds the scene depth buffer from the game's
//! D3D12 barriers at runtime, copies it on the game's queue after its depth pass, and the overlay's pixel shader
//! hides every pixel of a tag where the game drew something nearer (`begin` / `end` round each tag's drawing).
//!
//! Proof in the log ("depth:" lines): which buffer was chosen and why (the candidate table), the player check
//! (the depth under the player's chest must give the camera's distance to the player: that proves the buffer is the
//! scene's, the near plane and whether Z is reversed), and each named tag going occluded / visible with the scene
//! depth found in front of it. Until the check passes the old CPU sight tests stay in use (markers::Sight,
//! wisps.rs), and a buffer that never passes is rejected for the next candidate. ini depth_occlusion=0 turns it off.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use hudhook::depth;

/// A tag is hidden where the scene is nearer than its depth minus MARGIN + MARGIN_PER_M x depth (its own character's
/// head just under a name, the depth buffer's precision far away).
const MARGIN: f32 = 0.45;
const MARGIN_PER_M: f32 = 0.02;
/// Seconds between CPU reads of the depth (logging and the player check only; the hiding itself is per pixel on the
/// GPU every frame).
const PROBE_EVERY: f32 = 0.25;
/// Player checks that must agree before the depth test is used, and failed checks after which a buffer that never
/// agreed is rejected.
const PASSES: u32 = 3;
const MAX_FAILS: u32 = 120;
/// Round 28 (W2): extra leeway for a character's own name plate. Round 28 input log: Ugo's tag at 7.5 m read his
/// own body at 6.9 m (0.6 m in front, exactly the 0.45 + 0.02 x 7.5 margin) in the copies that had him drawn, so
/// his own head hid his name. A wall still hides a plate when it is more than ~1.1 m in front of it.
pub const OWN_BODY: f32 = 0.5;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<Option<State>> = Mutex::new(None);

#[derive(Default)]
struct State {
    /// This frame's named tags (name, screen point, depth), read back on the next probe.
    tags: Vec<(String, [f32; 2], f32)>,
    /// The probe on its way: id, the tags it holds, the player's true depth (first point) when it has one.
    pending: Option<(u64, Vec<(String, f32)>, Option<f32>, Instant)>,
    next_id: u64,
    last_probe: Option<Instant>,
    generation: u32,
    passes: [u32; 2],
    fails: u32,
    validated: bool,
    reversed: bool,
    /// A near plane worked out from the player checks when CSCam's never agreed.
    near_override: Option<f32>,
    implied_near: Vec<f32>,
    shown: HashMap<String, bool>,
    last_summary: Option<Instant>,
    last_check: Option<String>,
    no_depth_logged: bool,
    /// Round 28 (W2): player checks per copy mode since the last summary ([end, present] x [ok, missed]) and tag
    /// state changes since then.
    mode_checks: [[u32; 2]; 2],
    flips: u32,
    stale_logged: bool,
    /// Round 31 (W1): player checks not given to the copy-point choice since the last summary (on Torrent).
    riding_skips: u32,
    riding: bool,
}

/// Whether the game's depth test is in use this frame (the CPU sight tests stand down).
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Once per frame before the tags draw: the camera's planes, the depth capture's state, the last CPU read (player
/// check, tag log) and the next one.
pub fn frame(game: Option<&crate::game::Game>, camera: Option<crate::markers::Camera>, screen: [f32; 2], gameplay: bool,
             enabled: bool) {
    depth::set_wanted(enabled);
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = guard.get_or_insert_with(State::default);
    let planes = game.and_then(|g| g.clip_planes());
    let info = depth::info();
    if info.generation != s.generation {
        s.generation = info.generation;
        s.passes = [0, 0];
        s.fails = 0;
        s.validated = false;
        s.near_override = None;
        s.implied_near.clear();
        s.pending = None;
        tracing::info!("depth: captured scene depth {:#x} {}x{} DXGI format {} (choice {}), state after the game's \
                        depth pass {:#x}; checking it against the player's distance",
                       info.resource, info.width, info.height, info.format, info.generation, info.after_state);
    }
    if info.generation == 0 && !s.no_depth_logged && info.frames > 1800 {
        s.no_depth_logged = true;
        tracing::info!("depth: no scene depth captured after {} frames ({} barrier calls, {} depth passes ended); \
                        the CPU sight tests stay in use", info.frames, info.barrier_calls, info.depth_ends);
    }
    let (near, far) = match (planes, s.near_override) {
        (Some(_), Some(n)) => (n, 0.0),
        (Some(p), None) => p,
        (None, Some(n)) => (n, 0.0),
        (None, None) => (0.1, 0.0),
    };
    depth::set_projection(near, far, s.reversed);

    if let Some(p) = depth::take_probe() {
        if let Some((id, tags, player, _)) = s.pending.take() {
            if id == p.id {
                read_probe(s, &p.depth, &tags, player, near, far, gameplay, &info);
            }
        }
    }
    if s.pending.as_ref().is_some_and(|p| p.3.elapsed().as_secs_f32() > 2.0) {
        s.pending = None; // the renderer had no fresh copy to read
    }
    let tags = std::mem::take(&mut s.tags);
    if gameplay && info.fresh && s.pending.is_none() && s.last_probe.is_none_or(|t| t.elapsed().as_secs_f32() >= PROBE_EVERY) {
        if let Some(camera) = camera {
            let mut points = Vec::new();
            let chest = [camera.player[0], camera.player[1] + 1.1, camera.player[2]];
            let player = camera.project(chest, screen).filter(|(q, _)| q[0] >= 0.0 && q[1] >= 0.0
                && q[0] < screen[0] && q[1] < screen[1]);
            if let Some((q, _)) = player {
                points.push(q);
            }
            let named: Vec<(String, f32)> = tags.iter().map(|t| (t.0.clone(), t.2)).collect();
            points.extend(tags.iter().map(|t| t.1));
            s.next_id += 1;
            depth::probe(s.next_id, points);
            s.pending = Some((s.next_id, named, player.map(|p| p.1), Instant::now()));
            s.last_probe = Some(Instant::now());
        }
    }
    // Round 28 (W2): the test stays on while the last copy is under half a second old. It used to drop for single
    // frames with no fresh copy (round 28 input log: "occlusion test off (CPU sight tests)" then ON 17 ms later,
    // 5 times), which put every wisp out ("wisps: 0 of 15 in sight") and showed tags through walls for that frame.
    let on = enabled && s.validated && info.age <= depth::STALE_FRAMES;
    if s.validated && enabled && gameplay && info.age > depth::STALE_FRAMES && info.age != u64::MAX {
        if !s.stale_logged {
            s.stale_logged = true;
            tracing::info!("depth: no copy for {} frames in play (mode {}); the CPU sight tests stand in",
                           info.age, if info.present_mode { "BOUNDARY" } else { "END" });
        }
    } else if on {
        s.stale_logged = false;
    }
    if ACTIVE.swap(on, Ordering::Relaxed) != on {
        tracing::info!("depth: occlusion test {}", if on { "ON (game depth)" } else { "off (CPU sight tests)" });
    }
}

/// A CPU read of the depth: the player check, then each named tag's state.
#[allow(clippy::too_many_arguments)]
fn read_probe(s: &mut State, depth_values: &[f32], tags: &[(String, f32)], player: Option<f32>, near: f32, far: f32,
              gameplay: bool, info: &depth::Info) {
    let mut values = depth_values.iter().copied();
    if let Some(truth) = player {
        let Some(d) = values.next() else { return };
        if d.is_finite() && gameplay {
            let z = [depth::linear_with(d, near, far, true), depth::linear_with(d, near, far, false)];
            let ok = z.map(|z| (z - truth).abs() < 0.25 * truth + 0.3);
            let line = format!("depth value {d:.6} -> {:.2} m reversed / {:.2} m normal Z, the player is {truth:.2} m away \
                                (near {near:.3}, far {far:.0})", z[0], z[1]);
            for k in 0..2 {
                s.passes[k] += ok[k] as u32;
            }
            let good = if s.validated { ok[s.reversed as usize ^ 1] } else { ok[0] || ok[1] };
            s.mode_checks[info.present_mode as usize][(!good) as usize] += 1;
            // Round 29 (W1): every check also scores the copy point on trial or in use (depth::report_check).
            // Round 31 (W1): not while riding. Ryan's round 30 log (w101_orbs.log.round31_input): on Torrent the
            // physics position is the rider in the saddle, so player + 1.1 m is over the rider's head and the depth
            // there is whatever is behind him (70.26 / 12.86 m with the player 4 m away). Every check failed from the
            // first riding prompt line, the locked boundary was dropped (23:09:12, 11 of 24) and all three re-trial
            // rounds failed (23:09:21-30), so the choice gave up for good and END copies were used from then on.
            let riding = crate::game::event_flag(crate::prompt::RIDING_FLAG) == Some(true);
            if riding != s.riding {
                s.riding = riding;
                tracing::info!("depth: player checks {} the copy-point choice ({})",
                               if riding { "no longer scored for" } else { "scored for" },
                               if riding { "riding Torrent: the chest point is over the rider's head" } else { "on foot" });
            }
            if riding {
                s.riding_skips += 1;
            } else if info.age <= 1 {
                depth::report_check(good);
            }
            if !ok[0] && !ok[1] {
                s.fails += 1;
                if d > 0.0 && d < 1.0 {
                    s.implied_near.push(d * truth);
                    if s.implied_near.len() > 40 {
                        s.implied_near.remove(0);
                    }
                }
            }
            if !s.validated && s.passes.iter().any(|p| *p >= PASSES) {
                s.validated = true;
                s.reversed = s.passes[0] >= s.passes[1];
                tracing::info!("depth: player check passed {} times ({} Z): {line}; using the game's depth for tags, \
                                markers and wisps", s.passes[0].max(s.passes[1]),
                               if s.reversed { "reversed" } else { "normal" });
            }
            // CSCam's near plane never agrees but the implied ones do: use theirs (reversed, infinite far).
            if !s.validated && s.near_override.is_none() && s.fails >= 20 && s.implied_near.len() >= 20 {
                let mut v = s.implied_near.clone();
                v.sort_by(f32::total_cmp);
                let median = v[v.len() / 2];
                let spread = v.iter().filter(|x| (*x / median - 1.0).abs() < 0.1).count();
                if median > 0.001 && spread * 4 >= v.len() * 3 {
                    s.near_override = Some(median);
                    tracing::info!("depth: CSCam's near plane {near:.3} never matched; {spread} of {} checks agree on \
                                    {median:.4} (reversed Z): using it", v.len());
                }
            }
            if !s.validated && s.fails >= MAX_FAILS && s.passes == [0, 0] {
                tracing::info!("depth: buffer {:#x} never matched the player's distance in {} checks (last: {line}); \
                                rejecting it", info.resource, s.fails);
                depth::reject_current();
                s.fails = 0;
            }
            if s.last_summary.is_none_or(|t| t.elapsed().as_secs() >= 15) {
                s.last_summary = Some(Instant::now());
                tracing::info!("depth: player check {} (passes reversed {} / normal {}, fails {}): {line}; {} copies \
                                in {} frames ({} at a boundary); since the last summary: END-copy checks ok/missed {}/{}, \
                                BOUNDARY-copy checks ok/missed {}/{}, tag state changes {}, checks skipped on Torrent {}; copy mode now {}, {} frames old",
                               if ok[0] || ok[1] { "ok" } else { "MISSED" }, s.passes[0], s.passes[1],
                               s.fails, info.copies, info.frames, info.present_copies, s.mode_checks[0][0],
                               s.mode_checks[0][1], s.mode_checks[1][0], s.mode_checks[1][1], s.flips, s.riding_skips,
                               if info.present_mode { "BOUNDARY" } else { "END" }, info.age);
                s.mode_checks = [[0; 2]; 2];
                s.flips = 0;
                s.riding_skips = 0;
            }
            s.last_check = Some(line);
        }
    }
    if !s.validated {
        return;
    }
    for ((name, z), d) in tags.iter().zip(values) {
        if !d.is_finite() {
            continue;
        }
        let scene = depth::linear_with(d, near, far, s.reversed);
        let limit = z - margin(*z, OWN_BODY);
        let hidden = scene < limit;
        if s.shown.insert(name.clone(), !hidden) != Some(!hidden) {
            s.flips += 1;
            let mode = if info.present_mode { "BOUNDARY" } else { "END" };
            if hidden {
                tracing::info!("depth: tag '{name}' occluded (scene {scene:.1} m in front of the tag at {z:.1} m; \
                                {mode} copy)");
            } else {
                tracing::info!("depth: tag '{name}' visible (scene {scene:.1} m behind the tag at {z:.1} m; {mode} copy)");
            }
        }
    }
}

fn margin(z: f32, extra: f32) -> f32 {
    MARGIN + MARGIN_PER_M * z + extra
}

/// Before drawing a world tag at depth `z` (metres along the camera's forward axis) on the background draw list:
/// what follows is hidden wherever the game's scene is nearer (`extra` metres more leeway: a wisp's cloud radius).
/// `name` (empty: not logged) and `point` (screen) put it in the next depth read for the log.
pub fn begin(z: f32, extra: f32, name: &str, point: [f32; 2]) {
    if !name.is_empty() {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = guard.as_mut() {
            if s.tags.len() < 32 && !s.tags.iter().any(|t| t.0 == name) {
                s.tags.push((name.to_string(), point, z));
            }
        }
    }
    if active() {
        callback(depth::tag((z - margin(z, extra)).max(0.0)));
    }
}

/// After the tags: the depth test off again.
pub fn end() {
    if active() {
        callback(depth::DEPTH_OFF);
    }
}

fn callback(data: usize) {
    use hudhook::imgui::sys;
    unsafe { sys::ImDrawList_AddCallback(sys::igGetBackgroundDrawList(), Some(hudhook::blend_callback), data as *mut _) };
}
