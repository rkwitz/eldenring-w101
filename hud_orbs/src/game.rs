//! Reads the player's HP and FP out of Elden Ring's memory.
//!
//! The game's globals are found by byte pattern (the same patterns community tools use), so small patches don't
//! break them. The live values come from the player character (WorldChrMan -> player -> stat module); the save's
//! copy in PlayerGameData is the fallback when those offsets look wrong. Without a player character there are none.
//! Level and runes held come from PlayerGameData (offsets from the eldenring crate's struct).

use std::mem::size_of;

use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Memory::{
    VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS,
};

const WORLD_CHR_MAN: &str = "48 8B 05 ?? ?? ?? ?? 48 85 C0 74 0F 48 39 88";
const GAME_DATA_MAN: &str = "48 8B 05 ?? ?? ?? ?? 48 85 C0 74 05 48 8B 40 58 C3 C3";

const PLAYER: usize = 0x1E508; // WorldChrMan -> main player ChrIns (checked against game version 2.7.1.0)
const MODULES: usize = 0x190; // ChrIns -> module bag; its first entry is the stat module
const STAT_HP: usize = 0x138; // hp, max hp, (base max hp), _, fp, max fp
const STAT_FP: usize = 0x148;
const STAT_SP: usize = 0x154; // stamina, max stamina (CSChrDataModule in the eldenring crate)
pub use crate::sheet::*;

/// Size of a WorldChrMan characters-by-distance entry (ChrInsDistanceEntry).
const DISTANCE_ENTRY: usize = 16;
const PLAYER_GAME_DATA: usize = 0x8; // GameDataMan -> PlayerGameData: hp 0x10, max 0x14, fp 0x1C, max 0x20, sp 0x2C, max 0x30

/// Elden Ring's own "change equipment" function (EquipGameData, ChrAsm slot 0-21, &item ID, inventory index, 1, 1,
/// 0), the one its Equipment menu calls; pattern and arguments from The Grand Archives' cheat table (equipItem),
/// pattern match minus 0x17 is the function start.
const EQUIP_GEAR: &str = "?? 8B F1 ?? 8B D8 ?? 63 EA ?? 8B F9";
const EQUIP_GEAR_BACK: usize = 0x17;
/// PlayerGameData -> EquipGameData (the cheat table's offset; const-asserted against the crate below).
const EQUIP_GAME_DATA: usize = 0x2B0;

pub struct Game {
    world_chr_man: usize,
    game_data_man: usize,
    /// The equip function, if its pattern was found (exactly once).
    equip_gear: Option<usize>,
    /// The loaded characters (ChrIns, model), re-read every half second, and the models last logged.
    characters: std::sync::Mutex<(Option<std::time::Instant>, Vec<(usize, u32)>)>,
    models_logged: std::sync::Mutex<Vec<u32>>,
    /// Round 21 (quests Q2): the camera of the last few frames, newest last (`tick_camera`).
    cameras: std::sync::Mutex<std::collections::VecDeque<crate::markers::Camera>>,
    /// Round 22 (H5): the last camera sample taken, when, and since when samples have been refused (`tick_camera`).
    camera_guard: std::sync::Mutex<(Option<(crate::markers::Camera, std::time::Instant)>, Option<std::time::Instant>)>,
    /// Round 23 (H2): other characters' positions at the last camera sample taken (ChrIns, physics position), to tell
    /// a world rebase (everything shifts together) from a bad read (`tick_camera`).
    rebase_refs: std::sync::Mutex<Vec<(usize, [f32; 3])>>,
}

impl Game {
    /// Finds the game's globals, or None when a pattern is missing (another game version).
    pub fn find() -> Option<Game> {
        let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
        let sections = code_sections(base);
        Some(Game {
            world_chr_man: rip_target(&sections, WORLD_CHR_MAN)?,
            game_data_man: rip_target(&sections, GAME_DATA_MAN)?,
            equip_gear: scan_unique(&sections, EQUIP_GEAR).map(|at| at - EQUIP_GEAR_BACK),
            characters: Default::default(),
            models_logged: Default::default(),
            cameras: Default::default(),
            camera_guard: Default::default(),
            rebase_refs: Default::default(),
        })
    }

    /// The player's position in global map-chunk coordinates (metres) with the map block the player is in and the
    /// block the chunk coordinates are relative to (`block_origin`: when it changes, the coordinates jump although
    /// the player didn't move); None without a player character.
    pub fn position(&self) -> Option<([f32; 3], i32, i32)> {
        use eldenring::cs::ChrIns;
        use std::mem::offset_of;
        let player = self.player()?;
        let [x, y, z, _] = read::<[f32; 4]>(player + offset_of!(ChrIns, chunk_position))?;
        let block = read::<i32>(player + offset_of!(ChrIns, block_id))?;
        let origin = read::<i32>(player + offset_of!(ChrIns, block_origin))?;
        [x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1.0e6).then_some(([x, y, z], block, origin))
    }

    /// Round 32 (U1, Ryan: "floating text over the player is much too high on Torrent"): the top of the player's own
    /// posed model box over the player's physics position (the camera's player point), in metres, when it reads as
    /// a body (0.5-3 m); None otherwise. On Torrent the physics position is the rider's (round 28's prompt log: dy
    /// +1.39 m over an NPC's feet), so a fixed 1.75 m over it put the numbers a whole body too high.
    pub fn player_head(&self) -> Option<f32> {
        let chr = self.player()?;
        let p = Self::chr_physics(chr)?;
        let (top, _) = self.model_top(chr, p)?;
        let h = top[1] - p[1];
        (0.5..3.0).contains(&h).then_some(h)
    }

    /// The main player's ChrIns (scene probe).
    pub fn player_address(&self) -> Option<usize> {
        self.player()
    }

    /// Every loaded character (ChrIns, model), read now (scene probe).
    pub fn loaded_characters(&self) -> Vec<(usize, u32)> {
        self.characters()
    }

    /// A character's physics position (the camera's space), or None.
    pub fn chr_physics(chr: usize) -> Option<[f32; 3]> {
        use eldenring::cs::{CSChrPhysicsModule, ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let modules = read::<usize>(chr + offset_of!(ChrIns, modules))?;
        let physics = read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics))?;
        let [x, y, z, _] = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, position))?;
        [x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1.0e6).then_some([x, y, z])
    }

    fn player(&self) -> Option<usize> {
        read::<usize>(self.world_chr_man).and_then(|wcm| read::<usize>(wcm + PLAYER))
    }

    /// The game camera (CSCamera's first perspective camera: rows right, up, forward, position; vertical fov in
    /// radians) and the player's physics position, in the same space as the camera; None when they can't be read.
    pub fn camera(&self) -> Option<crate::markers::Camera> {
        let cameras = self.cameras.lock().unwrap_or_else(|e| e.into_inner());
        cameras.front().copied().or_else(|| self.camera_now())
    }

    /// Round 21 (quests Q2): once per frame, before anything draws. Ryan: the "!" and the name plates lagged behind
    /// the character as the camera turned. The overlay draws at Present, when the game thread is already stepping
    /// the next frame, so CSCam holds a newer camera than the picture being shown (it can't hold an older one).
    /// `camera()` then answers with the camera of `delay` frames ago (w101_orbs.ini marker_frames, default 1).
    ///
    /// Round 22 (H5, Ryan: the "?" "glitches to the side of the screen" while running toward the NPC; names "randomly
    /// slide across the screen"). His round 21 log shows single frames where every NPC is ~30 m nearer than the frame
    /// before and after (markers: private_connelly 229 m then 261 m 15 ms later, 202/230, 167/199, 134/166, 101/133)
    /// : a camera/player read from a moment the game was mid-update. A sample whose player or camera moved further
    /// than anyone can in that time, or whose camera is far from the player, is refused and the last good one kept;
    /// if the jump lasts GUARD_HOLD (a warp, a respawn) it is taken. Every refusal is logged ("camera: refused").
    ///
    /// Round 23 (H2, Ryan: "all of the name tags appear at once and they all disappear"). His round 22 log has 33
    /// refusals of 32-40 m every 3.5-4.5 s while running, each also refusing all five NPC positions by the same
    /// distance, then "jump kept for 0.35 s, taken" with the player's MSB position continuous across it: the game moves
    /// its whole coordinate frame (player, camera and every character together) every ~35 m travelled. Those are not
    /// bad reads: the guard froze the camera for 0.35 s each time (names and markers stuck on the screen, creature
    /// plates and wisps 35 m off, then all snapping back). Now a jump that the other loaded characters made too (same
    /// vector within 2 m, `rebase`) is taken at once, and the frames queued for `delay` are moved by it, so the camera
    /// of a frame ago matches this frame's characters. Logged ("camera: world shifted ...").
    ///
    /// Round 26 (O2, Ryan: "single frames where every name tag/character marker jumps to a wrong screen spot then
    /// back", 3rd report). His 21:58 log: 27 shifts of 32-40 m (vectors on an 8 m grid: (232,-32,-8), (0,-88,80), ...)
    /// were all "refused" and then "jump kept for 0.35 s, taken"; not one was recognised as a shift, because
    /// `rebase` compared against the first 8 of 99-270 loaded characters (most of them elsewhere in the map, which do
    /// not move with the street) and the shift never got a majority. The NPC positions read in the same frame did
    /// move with the player (markers: "refused a position ... moved 40.2 m against the player": against the held old
    /// camera), so for 0.35 s everything was drawn from the frozen camera, and on the frame the jump was taken the
    /// one-frame-old camera (marker_frames=1) was still the unshifted one while the characters were shifted: the
    /// markers line at 21:48:20.549 has Connelly 50 m / player MSB [-49, 96, -16] between 14 m / [-24, 96, -48] the
    /// frame before and 17 m / [-28, 96, -45] the frame after. Now: player and camera jumping by the same vector
    /// (within 1.5 m) in one frame IS the shift (a bad read doesn't move both identically), taken at once with the
    /// queued cameras moved by it; a jump that is still taken after the hold empties the queue (no stale camera);
    /// a camera far from the player with no jump (the load screen's 240 m) no longer logs "world shifted (0, 0, 0)"
    /// every frame (391 lines in that log).
    pub fn tick_camera(&self, delay: usize) {
        const GUARD_HOLD: f32 = 0.35;
        let mut now = self.camera_now();
        let refs = self.reference_positions();
        let mut shift: Option<[f32; 3]> = None;
        let mut restart = false;
        {
            let mut guard = self.camera_guard.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(c) = now {
                let at = std::time::Instant::now();
                let mut bad = guard.0.as_ref().and_then(|(last, when)| {
                    let dt = at.duration_since(*when).as_secs_f32().min(1.0);
                    let d = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                    let cam = |c: &crate::markers::Camera| [c.matrix[3][0], c.matrix[3][1], c.matrix[3][2]];
                    let (player_jump, camera_jump, apart) = (d(c.player, last.player), d(cam(&c), cam(last)), d(cam(&c), c.player));
                    (player_jump > 2.5 + 30.0 * dt || camera_jump > 3.0 + 40.0 * dt || apart > 30.0)
                        .then(|| format!("player moved {player_jump:.1} m, camera {camera_jump:.1} m in {:.0} ms, camera {apart:.1} m from the player", dt * 1000.0))
                });
                // A jump the camera made with the player (or every other character made too) is the world moving, not
                // a bad read.
                if bad.is_some() {
                    if let Some((last, _)) = guard.0.as_ref() {
                        let jump = [c.player[0] - last.player[0], c.player[1] - last.player[1], c.player[2] - last.player[2]];
                        let cam_jump = [c.matrix[3][0] - last.matrix[3][0], c.matrix[3][1] - last.matrix[3][1],
                                        c.matrix[3][2] - last.matrix[3][2]];
                        let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                        let together = len([jump[0] - cam_jump[0], jump[1] - cam_jump[1], jump[2] - cam_jump[2]]) < 1.5;
                        let apart = len([c.matrix[3][0] - c.player[0], c.matrix[3][1] - c.player[1], c.matrix[3][2] - c.player[2]]);
                        let old = self.rebase_refs.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        let moved = len(jump) > 1.0 && apart <= 30.0;
                        if moved && (together || rebase(jump, &old, &refs)) {
                            let grid = jump.iter().all(|v| (v / 8.0 - (v / 8.0).round()).abs() * 8.0 < 1.0);
                            tracing::info!("camera: world shifted by ({:.1}, {:.1}, {:.1}) m ({}{}); taken",
                                           jump[0], jump[1], jump[2],
                                           if together { "player and camera together" } else { "with the other characters" },
                                           if grid { ", on the 8 m grid" } else { "" });
                            shift = Some(jump);
                            bad = None;
                            guard.1 = None;
                        }
                    }
                }
                match bad {
                    Some(why) if guard.1.is_none_or(|since| since.elapsed().as_secs_f32() < GUARD_HOLD) => {
                        if guard.1.is_none() {
                            tracing::info!("camera: refused a sample ({why})");
                            guard.1 = Some(at);
                        }
                        now = guard.0.map(|(last, _)| last);
                    }
                    _ => {
                        if guard.1.take().is_some() && bad.is_some() {
                            tracing::info!("camera: jump kept for {GUARD_HOLD} s, taken (warp or respawn)");
                            restart = true;
                        }
                        guard.0 = Some((c, at));
                        *self.rebase_refs.lock().unwrap_or_else(|e| e.into_inner()) = refs;
                    }
                }
            } else {
                *guard = (None, None);
            }
        }
        let mut cameras = self.cameras.lock().unwrap_or_else(|e| e.into_inner());
        if restart {
            cameras.clear();
        }
        if let Some(d) = shift {
            for c in cameras.iter_mut() {
                for k in 0..3 {
                    c.matrix[3][k] += d[k];
                    c.player[k] += d[k];
                }
            }
        }
        match now {
            Some(c) => {
                cameras.push_back(c);
                while cameras.len() > delay + 1 {
                    cameras.pop_front();
                }
            }
            None => cameras.clear(),
        }
    }

    /// Round 23 (H2): up to 8 loaded characters other than the player (from the half-second cache) and their physics
    /// positions now, for `rebase`.
    fn reference_positions(&self) -> Vec<(usize, [f32; 3])> {
        use eldenring::cs::{CSChrPhysicsModule, ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let player = self.player();
        let list = self.characters.lock().unwrap_or_else(|e| e.into_inner()).1.clone();
        list.iter().filter(|(chr, _)| Some(*chr) != player).filter_map(|&(chr, _)| {
            let modules = read::<usize>(chr + offset_of!(ChrIns, modules))?;
            let physics = read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics))?;
            let [x, y, z, _] = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, position))?;
            [x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1.0e6).then_some((chr, [x, y, z]))
        }).take(8).collect()
    }

    /// Round 26 (O1): CSCam's near and far planes (metres), for reading the scene depth buffer (occlusion.rs).
    pub fn clip_planes(&self) -> Option<(f32, f32)> {
        use eldenring::cs::{CSCam, CSCamera};
        use std::mem::offset_of;
        let camera = from_singleton::static_of::<CSCamera>()?;
        let camera = read::<usize>(camera.as_ptr() as usize)?;
        let cam = read::<usize>(camera + offset_of!(CSCamera, pers_cam_1))?;
        let near = read::<f32>(cam + offset_of!(CSCam, near_plane))?;
        let far = read::<f32>(cam + offset_of!(CSCam, far_plane))?;
        (near.is_finite() && far.is_finite() && (0.001..10.0).contains(&near)).then_some((near, far))
    }

    /// Round 26 (O3): the box the game keeps round character `chr`'s model as it is posed this frame
    /// (CSChrModelIns -> CSFD4ModelItem -> CSFD4LocationGxModelMatricesAndAabbExporter.aabb), as (centre x, z, top y)
    /// in the physics position's space. The box is world space when it holds the character's physics position `at`
    /// (else, near the origin, model space: then the top is over `at`). None when it can't be read or is neither.
    pub fn model_top(&self, chr: usize, at: [f32; 3]) -> Option<([f32; 3], bool)> {
        use eldenring::cs::{CSChrModelIns, CSFD4LocationGxModelMatricesAndAabbExporter, CSFD4ModelItem, CSModelIns, ChrIns};
        use std::mem::offset_of;
        let model = read::<usize>(chr + offset_of!(ChrIns, chr_model_ins)).filter(|p| *p >= 0x10000)?;
        let item = read::<usize>(model + offset_of!(CSChrModelIns, model_ins) + offset_of!(CSModelIns, model_item))
            .filter(|p| *p >= 0x10000)?;
        let exporter = read::<usize>(item + offset_of!(CSFD4ModelItem, location_aabb_exporter)).filter(|p| *p >= 0x10000)?;
        let [min, max] = read::<[[f32; 4]; 2]>(exporter + offset_of!(CSFD4LocationGxModelMatricesAndAabbExporter, aabb))?;
        if !min.iter().chain(&max).take(3).all(|v| v.is_finite()) || (0..3).any(|k| max[k] < min[k] || max[k] - min[k] > 60.0) {
            return None;
        }
        let c = [(min[0] + max[0]) / 2.0, (min[2] + max[2]) / 2.0];
        let holds = (0..3).all(|k| at[k] >= min[k] - 1.5 && at[k] <= max[k] + 1.5);
        if holds && max[1] > at[1] + 0.3 {
            return Some(([c[0], max[1], c[1]], true));
        }
        let local = c[0].abs() < 12.0 && c[1].abs() < 12.0 && min[1] > -3.0 && max[1] > 0.3 && max[1] < 40.0;
        local.then_some(([at[0], at[1] + max[1], at[2]], false))
    }

    /// Round 29 (T1): Torrent (character c8000 in WorldChrMan's summon/buddy ChrSet, which holds spirit ashes and
    /// Torrent) fading out (`out`) or back in, through his ChrIns base_transparency_modifier (negative fades the base
    /// transparency to 0, positive back to 1; the game's own fade flags set -1 / 1). Returns his ChrIns.
    pub fn fade_torrent(&self, out: bool) -> Option<usize> {
        use eldenring::cs::{ChrIns, ChrSet, ChrSetEntry, WorldChrMan};
        use std::mem::{offset_of, size_of};
        let wcm = read::<usize>(self.world_chr_man)?;
        let set = wcm + offset_of!(WorldChrMan, summon_buddy_chr_set);
        let capacity = read::<u32>(set + offset_of!(ChrSet<ChrIns>, capacity))? as usize;
        let entries = read::<usize>(set + offset_of!(ChrSet<ChrIns>, entries))?;
        let entry = size_of::<ChrSetEntry<ChrIns>>();
        let bytes = read_bytes(entries, capacity.min(256) * entry)?;
        let chr = bytes.chunks_exact(entry).map(|e| usize::from_le_bytes(e[..8].try_into().unwrap()))
            .filter(|c| *c >= 0x10000 && *c % 8 == 0)
            .find(|c| read::<u32>(c + offset_of!(ChrIns, character_id)) == Some(8000))?;
        let at = chr + offset_of!(ChrIns, base_transparency_modifier);
        read::<f32>(at)?;
        unsafe { (at as *mut f32).write_volatile(if out { -1.0 } else { 1.0 }) };
        Some(chr)
    }

    /// Round 32 (G2/G7): the physics module (CSChrPhysicsModule address) of a character: the player (`None`),
    /// Torrent (`Some(TORRENT)`, character id 8000 in the summon/buddy set, as fade_torrent) or the street NPC with
    /// event entity id `id` (from the cached character list, as `entities`).
    pub fn physics_of(&self, who: Option<u32>) -> Option<usize> {
        use eldenring::cs::{ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let chr = match who {
            None => self.player()?,
            Some(TORRENT) => self.fade_torrent_chr()?,
            Some(id) => {
                let list = self.characters.lock().unwrap_or_else(|e| e.into_inner()).1.clone();
                list.into_iter().map(|(c, _)| c)
                    .find(|c| read::<u32>(c + offset_of!(ChrIns, event_entity_id)) == Some(id))?
            },
        };
        let modules = read::<usize>(chr + offset_of!(ChrIns, modules))?;
        read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics)).filter(|p| *p >= 0x10000)
    }

    /// Round 32: Torrent's ChrIns (character id 8000 in summon_buddy_chr_set), None when he isn't loaded.
    fn fade_torrent_chr(&self) -> Option<usize> {
        use eldenring::cs::{ChrIns, ChrSet, ChrSetEntry, WorldChrMan};
        use std::mem::{offset_of, size_of};
        let wcm = read::<usize>(self.world_chr_man)?;
        let set = wcm + offset_of!(WorldChrMan, summon_buddy_chr_set);
        let capacity = read::<u32>(set + offset_of!(ChrSet<ChrIns>, capacity))? as usize;
        let entries = read::<usize>(set + offset_of!(ChrSet<ChrIns>, entries))?;
        let entry = size_of::<ChrSetEntry<ChrIns>>();
        let bytes = read_bytes(entries, capacity.min(256) * entry)?;
        bytes.chunks_exact(entry).map(|e| usize::from_le_bytes(e[..8].try_into().unwrap()))
            .filter(|c| *c >= 0x10000 && *c % 8 == 0)
            .find(|c| read::<u32>(c + offset_of!(ChrIns, character_id)) == Some(8000))
    }

    /// Round 32: a character's yaw `a` (radians; its quaternion is a turn about +y by `a`, and the character faces
    /// -z of that frame: forward = -(sin a, cos a), as the prompt's calibrated "-z" and the MSB rotations show).
    pub fn yaw_of(&self, who: Option<u32>) -> Option<f32> {
        use eldenring::cs::CSChrPhysicsModule;
        use std::mem::offset_of;
        let physics = self.physics_of(who)?;
        let [x, y, z, w] = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, orientation))?;
        if ![x, y, z, w].iter().all(|v| v.is_finite()) || ((x * x + y * y + z * z + w * w) - 1.0).abs() > 0.1 {
            return None;
        }
        Some((2.0 * (x * z + w * y)).atan2(1.0 - 2.0 * (x * x + y * y)))
    }

    /// Round 32: turns a character to yaw `a` (see `yaw_of`): both the orientation and the interpolated one.
    pub fn set_yaw(&self, who: Option<u32>, a: f32) -> bool {
        use eldenring::cs::CSChrPhysicsModule;
        use std::mem::offset_of;
        let Some(physics) = self.physics_of(who) else { return false };
        let q = [0.0, (a / 2.0).sin(), 0.0, (a / 2.0).cos()];
        for field in [offset_of!(CSChrPhysicsModule, orientation), offset_of!(CSChrPhysicsModule, interpolated_orientation)] {
            if read::<[f32; 4]>(physics + field).is_none() {
                return false;
            }
            unsafe { ((physics + field) as *mut [f32; 4]).write_volatile(q) };
        }
        true
    }

    /// Round 32 (G7): a character's physics position.
    pub fn position_of(&self, who: Option<u32>) -> Option<[f32; 3]> {
        use eldenring::cs::CSChrPhysicsModule;
        use std::mem::offset_of;
        let physics = self.physics_of(who)?;
        let [x, y, z, _] = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, position))?;
        [x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1.0e6).then_some([x, y, z])
    }

    /// Round 32 (G7): moves a character to physics position `to` (position and last-update position, then asks the
    /// character proxy to take it, chr_proxy_pos_update_requested).
    pub fn set_position(&self, who: Option<u32>, to: [f32; 3]) -> bool {
        use eldenring::cs::CSChrPhysicsModule;
        use std::mem::offset_of;
        let Some(physics) = self.physics_of(who) else { return false };
        for field in [offset_of!(CSChrPhysicsModule, position), offset_of!(CSChrPhysicsModule, last_update_position)] {
            let Some(old) = read::<[f32; 4]>(physics + field) else { return false };
            unsafe { ((physics + field) as *mut [f32; 4]).write_volatile([to[0], to[1], to[2], old[3]]) };
        }
        let flag = physics + offset_of!(CSChrPhysicsModule, chr_proxy_pos_update_requested);
        unsafe { (flag as *mut bool).write_volatile(true) };
        true
    }

    /// The camera as CSCam holds it right now (see `camera`).
    fn camera_now(&self) -> Option<crate::markers::Camera> {
        use eldenring::cs::{CSCam, CSCamera, CSChrPhysicsModule, ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let player = self.player()?;
        let modules = read::<usize>(player + offset_of!(ChrIns, modules))?;
        let physics = read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics))?;
        let [px, py, pz, _] = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, position))?;
        let camera = from_singleton::static_of::<CSCamera>()?;
        let camera = read::<usize>(camera.as_ptr() as usize)?;
        let cam = read::<usize>(camera + offset_of!(CSCamera, pers_cam_1))?;
        let matrix = read::<[[f32; 4]; 4]>(cam + offset_of!(CSCam, matrix))?;
        let fov = read::<f32>(cam + offset_of!(CSCam, fov))?;
        let ok = matrix.iter().flatten().chain(&[px, py, pz]).all(|v| v.is_finite() && v.abs() < 1.0e6);
        (ok && (0.1..3.0).contains(&fov)).then_some(crate::markers::Camera { matrix, fov, player: [px, py, pz] })
    }

    /// The characters near the player whose model (character_id) is in `models` and that are alive: (model,
    /// physics position in the camera's space, hit capsule height in metres). Every character the game has loaded is
    /// looked at: the map's characters (WorldChrMan's open-field set and its per-map sets) and the by-distance list
    /// (which only holds some of them: round 8 found only the Lost Souls in it).
    ///
    /// Round 26 (O3): and the top of the model's posed box (Game::model_top: (x, top y, z) over which the plate
    /// floats, following the character's pose), when it can be read.
    pub fn enemies(&self, models: &[u32]) -> Vec<(u32, [f32; 3], f32, Option<[f32; 3]>)> {
        use eldenring::cs::{CSChrDataModule, CSChrPhysicsModule, ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let mut out = Vec::new();
        let list = {
            let mut cache = self.characters.lock().unwrap_or_else(|e| e.into_inner());
            if cache.0.is_none_or(|t| t.elapsed() > std::time::Duration::from_millis(500)) {
                *cache = (Some(std::time::Instant::now()), self.characters());
            }
            cache.1.clone()
        };
        for (chr, model) in list {
            if !models.contains(&model) {
                continue;
            }
            // Still the same character (the list is half a second old).
            if read::<u32>(chr + offset_of!(ChrIns, character_id)) != Some(model) {
                continue;
            }
            let Some(modules) = read::<usize>(chr + offset_of!(ChrIns, modules)) else { continue };
            let hp = read::<usize>(modules + offset_of!(ChrInsModuleContainer, data))
                .and_then(|data| read::<i32>(data + offset_of!(CSChrDataModule, hp)));
            if hp.is_none_or(|hp| hp <= 0) {
                continue;
            }
            let Some(physics) = read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics)) else { continue };
            let Some([x, y, z, _]) = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, position)) else { continue };
            let height = read::<f32>(physics + offset_of!(CSChrPhysicsModule, hit_height)).unwrap_or(0.0);
            if [x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1.0e6) {
                let top = self.model_top(chr, [x, y, z]);
                log_model_top(model, [x, y, z], top);
                // Round 31 (NT1): nothing until the anchor has settled (settle).
                let Some(top) = settle(chr, model, [x, y, z], top.map(|t| t.0)) else { continue };
                out.push((model, [x, y, z], if height.is_finite() && (0.2..30.0).contains(&height) { height } else { 0.0 },
                          top));
            }
        }
        out
    }

    /// w101 music (round 20 M2/M3): the loaded characters of model `models` with their HP (0 when dead), as
    /// (ChrIns address, model, physics position, hp). Uses the same half-second character list as `enemies`.
    pub fn combatants(&self, models: &[u32]) -> Vec<(usize, u32, [f32; 3], i32)> {
        use eldenring::cs::{CSChrDataModule, CSChrPhysicsModule, ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let list = {
            let mut cache = self.characters.lock().unwrap_or_else(|e| e.into_inner());
            if cache.0.is_none_or(|t| t.elapsed() > std::time::Duration::from_millis(500)) {
                *cache = (Some(std::time::Instant::now()), self.characters());
            }
            cache.1.clone()
        };
        let mut out = Vec::new();
        for (chr, model) in list {
            if !models.contains(&model) || read::<u32>(chr + offset_of!(ChrIns, character_id)) != Some(model) {
                continue;
            }
            let Some(modules) = read::<usize>(chr + offset_of!(ChrIns, modules)) else { continue };
            let Some(hp) = read::<usize>(modules + offset_of!(ChrInsModuleContainer, data))
                .and_then(|data| read::<i32>(data + offset_of!(CSChrDataModule, hp))) else { continue };
            let Some(physics) = read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics)) else { continue };
            let Some([x, y, z, _]) = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, position)) else { continue };
            if [x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1.0e6) {
                out.push((chr, model, [x, y, z], hp.max(0)));
            }
        }
        out
    }

    /// w101 music (round 20 M3): GameDataMan's boss fight state (the eldenring crate's public layout): whether a
    /// boss fight is active and the entity ID of the boss whose health bar is up (0 = none).
    pub fn boss_fight(&self) -> Option<(bool, u32)> {
        use eldenring::cs::GameDataMan;
        use std::mem::offset_of;
        let gdm = read::<usize>(self.game_data_man).filter(|p| *p != 0)?;
        let active = read::<u8>(gdm + offset_of!(GameDataMan, boss_fight_active))?;
        let entity = read::<u32>(gdm + offset_of!(GameDataMan, boss_health_bar_entity_id))?;
        Some((active != 0, entity))
    }

    /// w101 npcvis (round 18 G1): the loaded characters of model `models` whose map entity ID (ChrIns
    /// event_entity_id, the MSB part's EntityID) is in `ids`: (entity, physics position in the camera's space).
    pub fn entities(&self, models: &[u32], ids: &[u32]) -> Vec<(u32, [f32; 3])> {
        let mut dead = Vec::new();
        use eldenring::cs::{CSChrPhysicsModule, ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let list = {
            let mut cache = self.characters.lock().unwrap_or_else(|e| e.into_inner());
            if cache.0.is_none_or(|t| t.elapsed() > std::time::Duration::from_millis(500)) {
                *cache = (Some(std::time::Instant::now()), self.characters());
            }
            cache.1.clone()
        };
        let mut out = Vec::new();
        for (chr, model) in list {
            if !models.contains(&model) || read::<u32>(chr + offset_of!(ChrIns, character_id)) != Some(model) {
                continue;
            }
            let Some(id) = read::<u32>(chr + offset_of!(ChrIns, event_entity_id)).filter(|i| ids.contains(i)) else { continue };
            let Some(modules) = read::<usize>(chr + offset_of!(ChrIns, modules)) else { continue };
            // Round 26 (npcs N7): a dead street NPC is left out and reported to markers.rs, which then shows no
            // marker, name or talk prompt for it (it is immortal by EMEVD, so this only catches the unexpected).
            let hp = read::<usize>(modules + offset_of!(ChrInsModuleContainer, data))
                .and_then(|data| read::<i32>(data + offset_of!(eldenring::cs::CSChrDataModule, hp)));
            if hp.is_some_and(|hp| hp <= 0) {
                dead.push(id);
                continue;
            }
            let Some(physics) = read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics)) else { continue };
            let Some([x, y, z, _]) = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, position)) else { continue };
            if [x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1.0e6) {
                out.push((id, [x, y, z]));
            }
        }
        crate::markers::set_dead(&dead);
        out
    }

    /// Round 19 (quests E2): why a loaded street NPC may not be seen. Per entity in `ids` (model in `models`): the
    /// ChrIns render bits (chr_flags1c4 bit 3 = render group (MSB draw groups) on, chr_flags1c5 bit 3 = rendering on)
    /// and the activation flags (bit 3 = activation enabled), raw, for the log.
    pub fn entity_render_bits(&self, models: &[u32], ids: &[u32]) -> Vec<(u32, u8, u8, u32)> {
        use eldenring::cs::ChrIns;
        use std::mem::offset_of;
        let list = self.characters.lock().unwrap_or_else(|e| e.into_inner()).1.clone();
        let mut out = Vec::new();
        for (chr, model) in list {
            if !models.contains(&model) {
                continue;
            }
            let Some(id) = read::<u32>(chr + offset_of!(ChrIns, event_entity_id)).filter(|i| ids.contains(i)) else { continue };
            let (Some(a), Some(b), Some(c)) = (read::<u8>(chr + offset_of!(ChrIns, chr_flags1c4)),
                                                read::<u8>(chr + offset_of!(ChrIns, chr_flags1c5)),
                                                read::<u32>(chr + offset_of!(ChrIns, chr_activation_flags))) else { continue };
            out.push((id, a, b, c));
        }
        out
    }

    /// Every loaded character and its model: (ChrIns address, character_id).
    fn characters(&self) -> Vec<(usize, u32)> {
        use eldenring::cs::{ChrIns, ChrSet, ChrSetEntry, OpenFieldChrSet, WorldChrMan};
        use std::mem::{offset_of, size_of};
        let mut chrs: Vec<usize> = Vec::new();
        let Some(wcm) = read::<usize>(self.world_chr_man) else { return vec![] };
        // The by-distance list (DLVector: allocator, first, last, end).
        let list = wcm + offset_of!(WorldChrMan, chr_inses_by_distance);
        if let (Some(first), Some(last)) = (read::<usize>(list + 8), read::<usize>(list + 16)) {
            if last >= first && first >= 0x10000 {
                let count = ((last - first) / DISTANCE_ENTRY).min(2048);
                if let Some(entries) = read_bytes(first, count * DISTANCE_ENTRY) {
                    chrs.extend(entries.chunks_exact(DISTANCE_ENTRY).map(|e| usize::from_le_bytes(e[..8].try_into().unwrap())));
                }
            }
        }
        // A ChrSet's entries: capacity, then an array of (ChrIns pointer, status bytes).
        let entry = size_of::<ChrSetEntry<ChrIns>>();
        let mut set = |at: usize| {
            let (Some(capacity), Some(entries)) = (read::<u32>(at + offset_of!(ChrSet<ChrIns>, capacity)),
                                                   read::<usize>(at + offset_of!(ChrSet<ChrIns>, entries))) else { return };
            let n = (capacity as usize).min(4096);
            if n == 0 {
                return;
            }
            if let Some(bytes) = read_bytes(entries, n * entry) {
                chrs.extend(bytes.chunks_exact(entry).map(|e| usize::from_le_bytes(e[..8].try_into().unwrap())));
            }
        };
        set(wcm + offset_of!(WorldChrMan, open_field_chr_set) + offset_of!(OpenFieldChrSet, base));
        let sets = wcm + offset_of!(WorldChrMan, chr_sets);
        for i in 0..196 {
            if let Some(p) = read::<usize>(sets + 8 * i).filter(|p| *p >= 0x10000) {
                set(p);
            }
        }
        // Only real characters: 8-aligned, with an RTTI class named *Ins (CS::PlayerIns, CS::EnemyIns, ...). The
        // lists also hold table slots and other objects, whose "character_id" was garbage (scene probe run 1).
        chrs.retain(|c| *c >= 0x10000 && *c % 8 == 0);
        chrs.retain(|c| crate::probe::rtti(*c).is_some_and(|n| n.ends_with("Ins")));
        chrs.sort_unstable();
        chrs.dedup();
        let out: Vec<(usize, u32)> = chrs.into_iter()
            .filter_map(|c| read::<u32>(c + offset_of!(ChrIns, character_id)).map(|m| (c, m)))
            .collect();
        let mut models: Vec<u32> = out.iter().map(|c| c.1).collect();
        models.sort_unstable();
        models.dedup();
        let mut seen = self.models_logged.lock().unwrap_or_else(|e| e.into_inner());
        if *seen != models && models.len() < 400 {
            tracing::info!("characters: {} loaded, models {:?}", out.len(), models);
            *seen = models;
        }
        out
    }

    /// The player's character sheet for the spellbook, from PlayerGameData; None without a character.
    pub fn sheet(&self) -> Option<Sheet> {
        use eldenring::cs::{EquipInventoryDataListEntry, EquipMagicData, PlayerGameData};
        use std::mem::{offset_of, size_of};
        let pgd = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA))?;
        let u32_at = |offset: usize| read::<u32>(pgd + offset).unwrap_or(0);
        // character_name ([u16; 17]) is private in the crate; gender follows it directly.
        let name_at = pgd + offset_of!(PlayerGameData, gender) - 34;
        let name: Vec<u16> = (0..17).map_while(|i| read::<u16>(name_at + 2 * i).filter(|c| *c != 0)).collect();
        let level = u32_at(offset_of!(PlayerGameData, level));
        if !(1..=713).contains(&level) {
            return None;
        }
        let equipment = read::<[i32; 22]>(pgd + offset_of!(PlayerGameData, equipment.chr_asm.equipment_param_ids))
            .unwrap_or([-1; 22]);
        let spells = read::<usize>(pgd + offset_of!(PlayerGameData, equipment.equip_magic_data))
            .and_then(|magic| read::<[[i32; 2]; 14]>(magic + offset_of!(EquipMagicData, entries)))
            .map(|entries| entries.map(|[id, _charges]| id))
            .unwrap_or([-1; 14]);
        let list = |head: usize, len: usize| -> Vec<(u32, u32)> {
            let (Some(head), Some(len)) = (read::<usize>(pgd + head), read::<u32>(pgd + len)) else { return vec![] };
            let size = size_of::<EquipInventoryDataListEntry>();
            let bytes = read_bytes(head, size * (len as usize).min(4096)).unwrap_or_default();
            let id_at = offset_of!(EquipInventoryDataListEntry, item_id);
            let quantity_at = offset_of!(EquipInventoryDataListEntry, quantity);
            bytes
                .chunks_exact(size)
                .map(|e| {
                    let field = |at: usize| u32::from_le_bytes(e[at..at + 4].try_into().unwrap());
                    (field(id_at), field(quantity_at))
                })
                .filter(|(id, n)| *id != u32::MAX && *id != 0 && *n > 0)
                .fold(Vec::<(u32, u32)>::new(), |mut out, (id, n)| {
                    // Several copies of a weapon or armour piece are separate entries: one row each, summed.
                    match out.iter_mut().find(|(seen, _)| *seen == id) {
                        Some(entry) => entry.1 += n,
                        None => out.push((id, n)),
                    }
                    out
                })
        };
        let items = list(
            offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data.normal_items_head),
            offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data.normal_items_len),
        );
        let key_items = list(
            offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data.key_items_head),
            offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data.key_items_len),
        );
        let quick_items = self.quick_slots(pgd).map(|slots| slots.map(|(_, _, id)| id)).unwrap_or([0; 10]);
        Some(Sheet {
            quick_items,
            name: String::from_utf16_lossy(&name),
            level,
            runes: u32_at(offset_of!(PlayerGameData, rune_count)),
            rune_memory: u32_at(offset_of!(PlayerGameData, rune_memory)),
            attributes: [
                offset_of!(PlayerGameData, vigor),
                offset_of!(PlayerGameData, mind),
                offset_of!(PlayerGameData, endurance),
                offset_of!(PlayerGameData, strength),
                offset_of!(PlayerGameData, dexterity),
                offset_of!(PlayerGameData, intelligence),
                offset_of!(PlayerGameData, faith),
                offset_of!(PlayerGameData, arcane),
            ]
            .map(u32_at),
            resistances: [
                offset_of!(PlayerGameData, poison_resist),
                offset_of!(PlayerGameData, bleed_resist),
                offset_of!(PlayerGameData, sleep_resist),
                offset_of!(PlayerGameData, death_resist),
            ]
            .map(u32_at),
            poise: read::<f32>(pgd + offset_of!(PlayerGameData, poise)).unwrap_or(0.0),
            max_equip_load: read::<f32>(pgd + offset_of!(PlayerGameData, max_equip_load)).unwrap_or(0.0),
            flasks: [
                read::<u8>(pgd + offset_of!(PlayerGameData, max_hp_flask)).unwrap_or(0),
                read::<u8>(pgd + offset_of!(PlayerGameData, max_fp_flask)).unwrap_or(0),
            ],
            magic_slots: read::<u8>(pgd + offset_of!(PlayerGameData, unlocked_magic_slots)).unwrap_or(0),
            talisman_slots: read::<u8>(pgd + offset_of!(PlayerGameData, unlocked_talisman_slots)).unwrap_or(0),
            weapon_slots: read::<[u32; 2]>(pgd + offset_of!(PlayerGameData, equipment.chr_asm.equipment.selected_slots))
                .unwrap_or([0; 2]),
            gender: read::<u8>(pgd + offset_of!(PlayerGameData, gender)).unwrap_or(1),
            face: read_bytes(pgd + offset_of!(PlayerGameData, face_data.face_data_buffer.buffer), 276).unwrap_or_default(),
            equipment,
            spells,
            items,
            key_items,
        })
    }

    /// Round 20 (quests Q3): gives the player `runes` (a quest reward): PlayerGameData's rune count, and its rune
    /// memory (all runes ever held) as picking runes up does. Elden Ring's EMEVD and ItemLotParam have no way to
    /// give runes as such (only items such as Golden Runes), so the DLL adds them. The new count, or None.
    pub fn add_runes(&self, runes: u32) -> Option<u32> {
        use eldenring::cs::PlayerGameData;
        use std::mem::offset_of;
        let pgd = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA))?;
        let level = read::<u32>(pgd + offset_of!(PlayerGameData, level))?;
        if !(1..=713).contains(&level) {
            return None;
        }
        let (count_at, memory_at) = (pgd + offset_of!(PlayerGameData, rune_count), pgd + offset_of!(PlayerGameData, rune_memory));
        let count = read::<u32>(count_at)?.saturating_add(runes).min(999_999_999);
        let memory = read::<u32>(memory_at)?.saturating_add(runes).min(999_999_999);
        unsafe {
            (count_at as *mut u32).write_volatile(count);
            (memory_at as *mut u32).write_volatile(memory);
        }
        Some(count)
    }

    /// Round 27 (B2): takes `runes` from the runes held (the shop's buys; rune memory, the total ever gathered, is
    /// left alone); None when fewer are held or the count can't be reached. Returns the runes held after.
    pub fn spend_runes(&self, runes: u32) -> Option<u32> {
        use eldenring::cs::PlayerGameData;
        use std::mem::offset_of;
        let pgd = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA))?;
        let level = read::<u32>(pgd + offset_of!(PlayerGameData, level))?;
        if !(1..=713).contains(&level) {
            return None;
        }
        let count_at = pgd + offset_of!(PlayerGameData, rune_count);
        let count = read::<u32>(count_at)?.checked_sub(runes)?;
        unsafe { (count_at as *mut u32).write_volatile(count) };
        Some(count)
    }

    /// Puts `spell` (a magic param ID, -1 = none) in memory slot `index` (EquipMagicData's entries), as Elden
    /// Ring's own Memorize Spell menu does at a grace; false when the slot can't be reached.
    pub fn set_spell(&self, index: usize, spell: i32) -> bool {
        use eldenring::cs::{EquipMagicData, EquipMagicItem, PlayerGameData};
        use std::mem::offset_of;
        if index >= 14 {
            return false;
        }
        let Some(pgd) = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA)) else {
            return false;
        };
        let Some(magic) = read::<usize>(pgd + offset_of!(PlayerGameData, equipment.equip_magic_data)) else { return false };
        let at = magic + offset_of!(EquipMagicData, entries) + index * size_of::<EquipMagicItem>();
        if read::<i32>(at).is_none() {
            return false;
        }
        unsafe { (at as *mut i32).write_volatile(spell) };
        // The spell selected for casting must be a memorised one: the first, if it was this slot's.
        let selected = magic + offset_of!(EquipMagicData, selected_slot);
        if spell < 0 && read::<i32>(selected) == Some(index as i32) {
            let entries = read::<[[i32; 2]; 14]>(magic + offset_of!(EquipMagicData, entries)).unwrap_or([[-1, 0]; 14]);
            let first = entries.iter().position(|e| e[0] >= 0).map_or(0, |i| i as i32);
            unsafe { (selected as *mut i32).write_volatile(first) };
        }
        true
    }

    /// Equips the inventory item `item_id` (with its category bits) in ChrAsm `slot` (weapons 0-5 L1 R1 L2 R2 L3 R3,
    /// arrows 6/8, bolts 7/9, armour 12-15, talismans 17-20) through Elden Ring's own equip function, as its
    /// Equipment menu does; `item_id` None empties a talisman slot. Err says why not.
    pub fn equip(&self, slot: u32, item_id: Option<u32>) -> Result<(), &'static str> {
        use eldenring::cs::{EquipInventoryDataListEntry, InventoryItemsData, PlayerGameData};
        use std::mem::offset_of;
        let function = self.equip_gear.ok_or("Elden Ring's equip function wasn't found (game version?)")?;
        if slot > 21 {
            return Err("not an equipment slot");
        }
        let pgd = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA))
            .ok_or("no character")?;
        let equip_game_data = pgd + EQUIP_GAME_DATA;
        let items = pgd + offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data);
        let (id, index) = match item_id {
            None => (u32::MAX, -1i32),
            Some(id) => {
                let head = read::<usize>(items + offset_of!(InventoryItemsData, normal_items_head)).ok_or("no inventory")?;
                let len = read::<u32>(items + offset_of!(InventoryItemsData, normal_items_len)).unwrap_or(0) as usize;
                let key_capacity = read::<u32>(items + offset_of!(InventoryItemsData, key_items_capacity)).unwrap_or(0);
                let size = size_of::<EquipInventoryDataListEntry>();
                let bytes = read_bytes(head, size * len.min(4096)).ok_or("no inventory")?;
                let at = offset_of!(EquipInventoryDataListEntry, item_id);
                let i = bytes.chunks_exact(size)
                    .position(|e| u32::from_le_bytes(e[at..at + 4].try_into().unwrap()) == id)
                    .ok_or("not in the inventory")?;
                // The game numbers key items first, then the normal items.
                (id, (i as u32 + key_capacity) as i32)
            }
        };
        let item = id;
        type Equip = unsafe extern "C" fn(usize, i32, *const u32, i32, i32, i32, i32) -> u64;
        let equip: Equip = unsafe { std::mem::transmute::<usize, Equip>(function) };
        unsafe { equip(equip_game_data, slot as i32, &item, index, 1, 1, 0) };
        Ok(())
    }

    /// The normal inventory's entries (gaitem handle, item ID) and the key item list's capacity: the game numbers
    /// the key items first, so normal entry i is inventory index capacity + i.
    fn normal_inventory(&self, pgd: usize) -> Option<(Vec<(u32, u32)>, u32)> {
        use eldenring::cs::{EquipInventoryDataListEntry, InventoryItemsData, PlayerGameData};
        use std::mem::offset_of;
        let items = pgd + offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data);
        let head = read::<usize>(items + offset_of!(InventoryItemsData, normal_items_head))?;
        let len = read::<u32>(items + offset_of!(InventoryItemsData, normal_items_len)).unwrap_or(0) as usize;
        let key_capacity = read::<u32>(items + offset_of!(InventoryItemsData, key_items_capacity)).unwrap_or(0);
        let size = size_of::<EquipInventoryDataListEntry>();
        let bytes = read_bytes(head, size * len.min(4096))?;
        let (h, at) = (offset_of!(EquipInventoryDataListEntry, gaitem_handle), offset_of!(EquipInventoryDataListEntry, item_id));
        let field = |e: &[u8], o: usize| u32::from_le_bytes(e[o..o + 4].try_into().unwrap());
        Some((bytes.chunks_exact(size).map(|e| (field(e, h), field(e, at))).collect(), key_capacity))
    }

    /// EquipItemData's ten quick item slots: (gaitem handle, inventory index, item ID or 0).
    fn quick_slots(&self, pgd: usize) -> Option<[(u32, i32, u32); 10]> {
        use eldenring::cs::PlayerGameData;
        use std::mem::offset_of;
        let raw = read::<[(u32, i32); 10]>(pgd + offset_of!(PlayerGameData, equipment.equip_item_data.quick_slots))?;
        let (normal, key_capacity) = self.normal_inventory(pgd)?;
        Some(raw.map(|(handle, index)| {
            let id = (index >= key_capacity as i32)
                .then(|| normal.get((index - key_capacity as i32) as usize).map(|e| e.1))
                .flatten()
                .filter(|id| *id != u32::MAX)
                .unwrap_or(0);
            (handle, index, id)
        }))
    }

    /// Puts the inventory item `item_id` (a spirit ash: Elden Ring uses them from the quick items) in the first
    /// empty quick item slot (`on`), or empties the slots that hold it, by writing EquipItemData's slot entries
    /// (gaitem handle and inventory index) as the Equipment menu leaves them.
    pub fn set_quick_item(&self, item_id: u32, on: bool) -> Result<(), &'static str> {
        use eldenring::cs::PlayerGameData;
        use std::mem::offset_of;
        let pgd = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA))
            .ok_or("no character")?;
        let slots = self.quick_slots(pgd).ok_or("the quick items can't be read")?;
        let at = pgd + offset_of!(PlayerGameData, equipment.equip_item_data.quick_slots);
        let empty = slots.iter().find(|s| s.1 < 0).map(|s| (s.0, s.1)).unwrap_or((0, -1));
        tracing::info!("quick items before: {slots:?}");
        let write = |slot: usize, (handle, index): (u32, i32)| unsafe {
            ((at + slot * 8) as *mut u32).write_volatile(handle);
            ((at + slot * 8 + 4) as *mut i32).write_volatile(index);
        };
        if !on {
            slots.iter().enumerate().filter(|(_, s)| s.2 == item_id).for_each(|(i, _)| write(i, empty));
            return Ok(());
        }
        if slots.iter().any(|s| s.2 == item_id) {
            return Ok(());
        }
        let (normal, key_capacity) = self.normal_inventory(pgd).ok_or("no inventory")?;
        let i = normal.iter().position(|e| e.1 == item_id).ok_or("not in the inventory")?;
        let free = slots.iter().position(|s| s.1 < 0).ok_or("the quick item slots are full")?;
        write(free, (normal[i].0, (i as u32 + key_capacity) as i32));
        Ok(())
    }

    /// Every candidate value, for `stats` and for the debug log.
    pub fn probe(&self) -> Probe {
        let player = read::<usize>(self.world_chr_man).and_then(|wcm| read::<usize>(wcm + PLAYER));
        let stat = player.and_then(|p| read::<usize>(p + MODULES)).and_then(|modules| read::<usize>(modules));
        let live = stat.and_then(|stat| {
            Some(Stats {
                hp: read(stat + STAT_HP)?,
                max_hp: read(stat + STAT_HP + 4)?,
                fp: read(stat + STAT_FP)?,
                max_fp: read(stat + STAT_FP + 4)?,
                sp: read(stat + STAT_SP)?,
                max_sp: read(stat + STAT_SP + 4)?,
                level: None,
            })
        });
        let pgd = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA));
        let saved = pgd.and_then(|pgd| {
            Some(Stats {
                hp: read(pgd + 0x10)?,
                max_hp: read(pgd + 0x14)?,
                fp: read(pgd + 0x1C)?,
                max_fp: read(pgd + 0x20)?,
                sp: read(pgd + 0x2C)?,
                max_sp: read(pgd + 0x30)?,
                level: None,
            })
        });
        let level = pgd.and_then(|pgd| {
            use eldenring::cs::PlayerGameData;
            let level = read::<u32>(pgd + std::mem::offset_of!(PlayerGameData, level))?;
            let runes = read::<u32>(pgd + std::mem::offset_of!(PlayerGameData, rune_count))?;
            (1..=713).contains(&level).then_some((level, runes))
        });
        Probe { player, stat, live, saved, hud: hud_values(), level }
    }
}

/// The candidates `Game::probe` found.
#[derive(Debug)]
pub struct Probe {
    pub player: Option<usize>,
    /// The stat module's address; only shown in the debug log.
    #[allow(dead_code)]
    pub stat: Option<usize>,
    pub live: Option<Stats>,
    pub saved: Option<Stats>,
    /// (hp, fp, max fp, stamina, max stamina) from the HUD's FrontEndViewValues.
    pub hud: Option<[i32; 5]>,
    pub level: Option<(u32, u32)>,
}

impl Probe {
    /// Current stats, or None while no character is loaded (title screen, loading screens): the player character
    /// has to exist in the world. Its own stat module gives the live values; the save's copy is the fallback if
    /// those offsets ever look wrong. Stamina comes from the values Elden Ring's own HUD shows (CSFeMan's
    /// FrontEndViewValues), else the stat module, never from the save's copy, which isn't updated while you run.
    pub fn stats(&self) -> Option<Stats> {
        self.player?;
        let live = self.live.filter(plausible);
        let mut stats = live.or(self.saved.filter(plausible))?;
        // Stamina: what Elden Ring's own (hidden) bar shows, which is live by definition; then the stat module.
        match self.hud {
            Some([_, _, _, sp, max_sp]) if (1..100_000).contains(&max_sp) && (0..=max_sp).contains(&sp) => {
                (stats.sp, stats.max_sp) = (sp, max_sp);
            }
            _ if live.is_none() => stats.sp = stats.max_sp,
            _ => {}
        }
        Some(Stats { level: self.level, ..stats })
    }
}

/// What Elden Ring's own HUD is showing: CSFeMan copies hp, fp and stamina into FrontEndViewValues every frame.
fn hud_values() -> Option<[i32; 5]> {
    use eldenring::cs::{CSFeManImp, FrontEndViewValues};
    use std::mem::offset_of;
    let fe_man = from_singleton::static_of::<CSFeManImp>()?;
    let values = read::<usize>(fe_man.as_ptr() as usize)? + offset_of!(CSFeManImp, frontend_values);
    Some([
        read(values + offset_of!(FrontEndViewValues, player_hp))?,
        read(values + offset_of!(FrontEndViewValues, fp))?,
        read(values + offset_of!(FrontEndViewValues, fp_max))?,
        read(values + offset_of!(FrontEndViewValues, stamina))?,
        read(values + offset_of!(FrontEndViewValues, stamina_max))?,
    ])
}

/// An event flag (CSEventFlagMan), or None when the flag manager isn't there.
pub fn event_flag(flag: u32) -> Option<bool> {
    let man = from_singleton::static_of::<eldenring::cs::CSEventFlagMan>()?;
    let man = unsafe { (*man.as_ptr()).as_ref()? };
    Some(man.virtual_memory_flag.get_flag(flag))
}

/// Whether the event flag manager holds the 1000-flag group `group` (flag / 1000) at all: flags in a group it
/// doesn't hold read as off and can't be set, by the game's EMEVD or by us.
pub fn flag_group_exists(group: u32) -> Option<bool> {
    let man = from_singleton::static_of::<eldenring::cs::CSEventFlagMan>()?;
    let man = unsafe { (*man.as_ptr()).as_ref()? };
    Some(man.virtual_memory_flag.flag_block_descriptors.find(&group).is_some())
}

/// Elden Ring's count of loading screens shown (CSNowLoadingHelper, by name from the game's reflection data): it
/// goes up by one as each of its loading screens starts (fast travel, map warps, death, loading a save, ...).
pub fn loading_screens() -> Option<i32> {
    let helper = from_singleton::static_of::<eldenring::cs::CSNowLoadingHelper>()?;
    let helper = read::<usize>(helper.as_ptr() as usize)?;
    read::<i32>(helper + std::mem::offset_of!(eldenring::cs::CSNowLoadingHelper, menu_load_counter))
}

/// Round 32: `Game::physics_of` etc. take this for Torrent.
pub const TORRENT: u32 = u32::MAX;

/// Sets or clears an event flag.
pub fn set_event_flag(flag: u32, on: bool) {
    let Some(man) = from_singleton::static_of::<eldenring::cs::CSEventFlagMan>() else { return };
    if let Some(man) = unsafe { (*man.as_ptr()).as_mut() } {
        man.virtual_memory_flag.set_flag(flag, on);
    }
}

/// Whether Elden Ring is showing its own HP/FP bars right now. The front-end manager (CSFeMan, found by name through
/// the game's reflection data, so patches don't move it) switches its HUD state away from "default" while a menu,
/// the map or a loading screen is up. If CSFeMan can't be found at all, the globes stay on.
pub fn hud_shown() -> bool {
    const DEFAULT: u8 = 3; // CSFeManHudState::Default
    let Some(fe_man) = from_singleton::static_of::<eldenring::cs::CSFeManImp>() else { return true };
    read::<usize>(fe_man.as_ptr() as usize)
        .and_then(|fe| read::<u8>(fe + std::mem::offset_of!(eldenring::cs::CSFeManImp, hud_state)))
        == Some(DEFAULT)
}

/// CSFeMan's raw HUD state (3 = default, the HUD is up), for the scene probe.
pub fn hud_state() -> Option<u8> {
    let fe_man = from_singleton::static_of::<eldenring::cs::CSFeManImp>()?;
    read::<usize>(fe_man.as_ptr() as usize)
        .and_then(|fe| read::<u8>(fe + std::mem::offset_of!(eldenring::cs::CSFeManImp, hud_state)))
}

/// How far the game has faded the screen out (0 = clear, 1 = covered), the most opaque of its fade plates. Loading
/// screens and cutscene transitions fade out first, so the globes go with them.
pub fn faded() -> f32 {
    let Some(fade) = from_singleton::static_of::<eldenring::cs::CSFade>() else { return 0.0 };
    let Some(fade) = read::<usize>(fade.as_ptr() as usize) else { return 0.0 };
    let plates = fade + std::mem::offset_of!(eldenring::cs::CSFade, fade_plates);
    let alpha = std::mem::offset_of!(eldenring::cs::CSFD4FadePlate, current_color) + 12;
    (0..9)
        .filter_map(|i| read::<usize>(plates + 8 * i).and_then(|plate| read::<f32>(plate + alpha)))
        .filter(|a| a.is_finite())
        .fold(0.0, f32::max)
        .clamp(0.0, 1.0)
}

fn plausible(s: &Stats) -> bool {
    (1..100_000).contains(&s.max_hp) && (0..=s.max_hp).contains(&s.hp) && (0..100_000).contains(&s.max_fp)
        && (0..=s.max_fp.max(0)).contains(&s.fp) && (0..100_000).contains(&s.max_sp)
        && (0..=s.max_sp.max(0)).contains(&s.sp)
}

/// `len` bytes at `address`, or None if any of them can't be read.
pub(crate) fn read_bytes(address: usize, len: usize) -> Option<Vec<u8>> {
    if address < 0x10000 {
        return None;
    }
    let mut buffer = vec![0u8; len];
    let mut got = 0;
    let ok = unsafe {
        ReadProcessMemory(GetCurrentProcess(), address as _, buffer.as_mut_ptr().cast(), len, &mut got)
    };
    (ok != 0 && got == len).then_some(buffer)
}

/// Reads a value without crashing on a stale pointer: ReadProcessMemory on our own process fails cleanly when the
/// memory isn't readable. (VirtualQuery did the same job but walks the page tables of the whole region, which in
/// Elden Ring's multi-gigabyte heaps took several milliseconds per call and dropped the game to a few fps.)
pub(crate) fn read<T: Copy>(address: usize) -> Option<T> {
    if address < 0x10000 {
        return None;
    }
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut got = 0;
    let ok = unsafe {
        ReadProcessMemory(GetCurrentProcess(), address as _, value.as_mut_ptr().cast(), size_of::<T>(), &mut got)
    };
    (ok != 0 && got == size_of::<T>()).then(|| unsafe { value.assume_init() })
}

/// (start, length) of each executable section of the module at `base`.
pub(crate) fn code_sections(base: usize) -> Vec<(usize, usize)> {
    unsafe {
        let pe = base + *((base + 0x3C) as *const u32) as usize;
        let count = *((pe + 6) as *const u16) as usize;
        let optional_size = *((pe + 20) as *const u16) as usize;
        let table = pe + 24 + optional_size;
        (0..count)
            .filter_map(|i| {
                let section = table + 40 * i;
                let characteristics = *((section + 36) as *const u32);
                let size = *((section + 8) as *const u32) as usize;
                let rva = *((section + 12) as *const u32) as usize;
                (characteristics & 0x2000_0000 != 0).then_some((base + rva, size))
            })
            .collect()
    }
}

/// The readable parts of [start, start + size), split where the memory protection changes.
pub(crate) fn readable_regions(start: usize, size: usize) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut at = start;
    while at < start + size {
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(at as _, &mut info, size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 {
            break;
        }
        let end = (info.BaseAddress as usize + info.RegionSize).min(start + size);
        if info.State == MEM_COMMIT && info.Protect & (PAGE_NOACCESS | PAGE_GUARD) == 0 {
            regions.push((at, end - at));
        }
        at = end;
    }
    regions
}

/// Address of the only match of `pattern` in the code sections; None when there is none or more than one.
fn scan_unique(sections: &[(usize, usize)], pattern: &str) -> Option<usize> {
    let pattern: Vec<Option<u8>> = pattern
        .split_whitespace()
        .map(|t| if t == "??" { None } else { u8::from_str_radix(t, 16).ok() })
        .collect();
    let mut found = None;
    for (start, size) in sections.iter().flat_map(|&(start, size)| readable_regions(start, size)) {
        let bytes = unsafe { std::slice::from_raw_parts(start as *const u8, size) };
        for (i, w) in bytes.windows(pattern.len()).enumerate() {
            if w.iter().zip(&pattern).all(|(b, p)| p.map_or(true, |p| p == *b)) {
                if found.is_some() {
                    return None;
                }
                found = Some(start + i);
            }
        }
    }
    found
}

/// Address that the first match of a `mov reg, [rip+disp32]` pattern loads from.
fn rip_target(sections: &[(usize, usize)], pattern: &str) -> Option<usize> {
    let pattern: Vec<Option<u8>> = pattern
        .split_whitespace()
        .map(|t| if t == "??" { None } else { u8::from_str_radix(t, 16).ok() })
        .collect();
    for (start, size) in sections.iter().flat_map(|&(start, size)| readable_regions(start, size)) {
        let bytes = unsafe { std::slice::from_raw_parts(start as *const u8, size) };
        let hit = bytes
            .windows(pattern.len())
            .position(|w| w.iter().zip(&pattern).all(|(b, p)| p.map_or(true, |p| p == *b)));
        if let Some(i) = hit {
            let disp = i32::from_le_bytes(bytes[i + 3..i + 7].try_into().unwrap());
            return Some((start + i + 7).wrapping_add_signed(disp as isize));
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------------------------
// The spellbook's Settings page (options.rs): Elden Ring's own options, graphics settings and quit.

/// GameDataMan -> GameSettings (Elden Ring's options: camera, sound, display; saved with the game's user data).
const GAME_SETTINGS: usize = 0x58;

impl Game {
    fn game_settings(&self) -> Option<usize> {
        let gdm = read::<usize>(self.game_data_man)?;
        read::<usize>(gdm + GAME_SETTINGS).filter(|p| *p != 0)
    }

    /// Round 21 M3 (wisps.rs): adds `percent` of the main player's max HP (or FP) to its current value in the
    /// player's CSChrDataModule (the stat module: hp/max hp at 0x138, fp/max fp at 0x148, the eldenring crate's
    /// layout, checked below), capped at the max. Nothing for a dead player or a full stat (round 25 H6). Returns (before, after).
    pub fn add_player_stat(&self, fp: bool, percent: f32) -> Option<(i32, i32)> {
        let player = read::<usize>(self.world_chr_man).and_then(|wcm| read::<usize>(wcm + PLAYER))?;
        let stat = read::<usize>(player + MODULES).and_then(|modules| read::<usize>(modules))?;
        let hp = read::<i32>(stat + STAT_HP)?;
        let at = stat + if fp { STAT_FP } else { STAT_HP };
        let (now, max) = (read::<i32>(at)?, read::<i32>(at + 4)?);
        if hp <= 0 || !(1..100_000).contains(&max) || !(0..=max).contains(&now) {
            return None;
        }
        // Round 25 (H6, Ryan: "wisps can't be picked up when that stat is full"): Wizard101 leaves a wisp where it
        // is when its stat is full (Ryan's 14:39 log: "picked up a Health wisp ... HP 1704 -> 1704").
        if now >= max {
            return None;
        }
        let new = (now + ((max as f32 * percent / 100.0).round() as i32).max(1)).min(max);
        unsafe { (at as *mut i32).write_volatile(new) };
        Some((now, new))
    }

    /// Round 25 (H3): the main player's facing on the ground (radians, atan2 of its forward x, z in the camera's space),
    /// from CSChrPhysicsModule's orientation quaternion (x, y, z, w; forward = local +z).
    pub fn player_yaw(&self) -> Option<f32> {
        use eldenring::cs::{CSChrPhysicsModule, ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let chr = self.player()?;
        let modules = read::<usize>(chr + offset_of!(ChrIns, modules))?;
        let physics = read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics))?;
        let [x, y, z, w] = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, orientation))?;
        if ![x, y, z, w].iter().all(|v| v.is_finite()) || ((x * x + y * y + z * z + w * w) - 1.0).abs() > 0.1 {
            return None;
        }
        // The quaternion applied to (0, 0, 1).
        let fx = 2.0 * (x * z + w * y);
        let fz = 1.0 - 2.0 * (x * x + y * y);
        Some(fx.atan2(fz))
    }

    /// Round 25 (H3): ActionButtonParam row `row`'s ER prompt shape (the vanilla regulation's values, read from the
    /// game's param data).
    pub fn action_button(&self, row: u32) -> Option<ActionButton> {
        let at = param_row(b"ACTIONBUTTON_PARAM_ST", row)?;
        let b = read_bytes(at, 0x38)?;
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let i = |o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        Some(ActionButton { region: b[0], radius: f(0x0C), angle: i(0x10), height: f(0x1C), base: f(0x20),
                            angle_check: b[0x24], allow_angle: i(0x28), text_id: i(0x34) })
    }

    /// Round 25 (H3): writes ActionButtonParam row `row`'s textId (the text Elden Ring's own prompt shows).
    pub fn set_action_button_text(&self, row: u32, text_id: i32) -> bool {
        use windows_sys::Win32::System::Diagnostics::Debug::WriteProcessMemory;
        let Some(at) = param_row(b"ACTIONBUTTON_PARAM_ST", row) else { return false };
        let mut written = 0;
        unsafe {
            WriteProcessMemory(GetCurrentProcess(), (at + 0x34) as _, (&text_id as *const i32).cast(), 4, &mut written)
                != 0 && written == 4
        }
    }

    /// One byte of Elden Ring's GameSettings (options.rs has the offsets).
    pub fn option(&self, offset: usize) -> Option<u8> {
        read::<u8>(self.game_settings()? + offset)
    }

    /// Writes one byte of GameSettings: the same value Elden Ring's own options menu writes. The game saves it
    /// with its user data on its next save.
    pub fn set_option(&self, offset: usize, value: u8) -> bool {
        let Some(at) = self.game_settings().map(|g| g + offset) else { return false };
        if read::<u8>(at).is_none() {
            return false;
        }
        unsafe { (at as *mut u8).write_volatile(value) };
        true
    }
}

/// CSWindow's persistent screen config (what Elden Ring writes to GraphicsConfig.xml), as its 25 u32 fields.
fn window_config() -> Option<usize> {
    use eldenring::cs::CSWindowImp;
    let window = from_singleton::static_of::<CSWindowImp>()?;
    let window = read::<usize>(window.as_ptr() as usize).filter(|p| *p != 0)?;
    Some(window + std::mem::offset_of!(CSWindowImp, persistent_window_config))
}

/// The graphics settings Elden Ring will save (CSWindowScreenConfig's fields in order), or None.
pub fn graphics() -> Option<[u32; 25]> {
    read::<[u32; 25]>(window_config()?)
}

/// Sets one field of the persistent screen config (applied by Elden Ring when it next starts, or saved by it).
pub fn set_graphics(index: usize, value: u32) -> bool {
    let Some(at) = window_config().map(|c| c + index * 4).filter(|_| index < 25) else { return false };
    if read::<u32>(at).is_none() {
        return false;
    }
    unsafe { (at as *mut u32).write_volatile(value) };
    true
}

/// Quit to the title screen the way Elden Ring's system menu does (it saves first): the menu data's quit-out
/// byte, [[CSMenuMan] + 0x8] + 0x5D (the chain the Elden Ring practice tool's "quitout" uses).
pub fn quit_to_title() -> bool {
    let Some(menu) = from_singleton::static_of::<eldenring::cs::CSMenuManImp>() else { return false };
    let Some(menu) = read::<usize>(menu.as_ptr() as usize).filter(|p| *p != 0) else { return false };
    let Some(data) = read::<usize>(menu + 0x8).filter(|p| *p != 0) else { return false };
    let at = data + 0x5D;
    if read::<u8>(at).is_none() {
        return false;
    }
    unsafe { (at as *mut u8).write_volatile(1) };
    true
}

/// Elden Ring's quit-out byte (quit_to_title's) is set: a quit to the title screen is under way. Its system menu's
/// quit-out sets it too (the practice tool's "quitout" relies on that). The teleport poof skips that loading screen.
pub fn quitting_out() -> bool {
    let Some(menu) = from_singleton::static_of::<eldenring::cs::CSMenuManImp>() else { return false };
    let Some(menu) = read::<usize>(menu.as_ptr() as usize).filter(|p| *p != 0) else { return false };
    let Some(data) = read::<usize>(menu + 0x8).filter(|p| *p != 0) else { return false };
    read::<u8>(data + 0x5D).is_some_and(|b| b != 0)
}

/// Asks Elden Ring's window to close (WM_CLOSE: the game's own exit, as Alt+F4). Used from the title screen only.
pub fn close_window() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let Some(window) = from_singleton::static_of::<eldenring::cs::CSWindowImp>() else { return false };
    let Some(window) = read::<usize>(window.as_ptr() as usize).filter(|p| *p != 0) else { return false };
    let Some(hwnd) = read::<isize>(window + std::mem::offset_of!(eldenring::cs::CSWindowImp, window_handle))
        .filter(|h| *h != 0) else { return false };
    unsafe { PostMessageW(hwnd as _, WM_CLOSE, 0, 0) != 0 }
}

// The hand-written offsets above must agree with the eldenring crate's struct layouts.
const _: () = {
    use eldenring::cs::{CSChrDataModule, ChrInsDistanceEntry, PlayerGameData, WorldChrMan};
    use std::mem::{offset_of, size_of};
    assert!(offset_of!(WorldChrMan, main_player) == PLAYER);
    assert!(size_of::<ChrInsDistanceEntry>() == DISTANCE_ENTRY);
    assert!(size_of::<eldenring::DLVector<ChrInsDistanceEntry>>() == 32);
    assert!(offset_of!(CSChrDataModule, hp) == STAT_HP);
    assert!(offset_of!(CSChrDataModule, fp) == STAT_FP);
    assert!(offset_of!(CSChrDataModule, stamina) == STAT_SP);
    assert!(offset_of!(PlayerGameData, current_hp) == 0x10);
    assert!(offset_of!(PlayerGameData, equipment) == EQUIP_GAME_DATA);
    assert!(offset_of!(PlayerGameData, current_fp) == 0x1C);
    assert!(offset_of!(PlayerGameData, current_stamina) == 0x2C);
    {
        use eldenring::cs::{CSWindowScreenConfig, GameDataMan, GameSettings};
        use crate::options::opt;
        assert!(offset_of!(GameDataMan, game_settings) == GAME_SETTINGS);
        assert!(size_of::<CSWindowScreenConfig>() == 25 * 4);
        assert!(offset_of!(CSWindowScreenConfig, grass_quality) == 24 * 4);
        assert!(offset_of!(GameSettings, camera_speed) == opt::CAMERA_SPEED);
        assert!(offset_of!(GameSettings, controller_rumble_strength) == opt::RUMBLE);
        assert!(offset_of!(GameSettings, brightness) == opt::BRIGHTNESS);
        assert!(offset_of!(GameSettings, music_volume) == opt::MUSIC);
        assert!(offset_of!(GameSettings, sfx_volume) == opt::SFX);
        assert!(offset_of!(GameSettings, voice_volume) == opt::VOICE);
        assert!(offset_of!(GameSettings, display_blood) == opt::BLOOD);
        assert!(offset_of!(GameSettings, show_subtitles) == opt::SUBTITLES);
        assert!(offset_of!(GameSettings, hud_type) == opt::HUD);
        assert!(offset_of!(GameSettings, reverse_camera_xaxis) == opt::INVERT_X);
        assert!(offset_of!(GameSettings, reverse_camera_yaxis) == opt::INVERT_Y);
        assert!(offset_of!(GameSettings, auto_lock_on) == opt::AUTO_LOCK);
        assert!(offset_of!(GameSettings, camera_auto_wall_recovery) == opt::WALL_RECOVERY);
        assert!(offset_of!(GameSettings, reset_camera_yaxis) == opt::RESET_Y);
        assert!(offset_of!(GameSettings, cinematic_effects) == opt::CINEMATIC);
        assert!(offset_of!(GameSettings, manual_attack_aiming) == opt::MANUAL_AIM);
        assert!(offset_of!(GameSettings, auto_target) == opt::AUTO_TARGET);
        assert!(offset_of!(GameSettings, start_offline) == opt::START_OFFLINE);
        assert!(offset_of!(GameSettings, hdr_brightness) == opt::HDR_BRIGHTNESS);
        assert!(offset_of!(GameSettings, hdr_max_brightness) == opt::HDR_PEAK);
        assert!(offset_of!(GameSettings, hdr_contrast) == opt::HDR_CONTRAST);
        assert!(offset_of!(GameSettings, master_volume) == opt::MASTER);
        assert!(offset_of!(GameSettings, mark_new_items) == opt::MARK_NEW);
        assert!(offset_of!(GameSettings, show_recent_items) == opt::RECENT_ITEMS);
        assert!(offset_of!(GameSettings, show_tutorials) == opt::TUTORIALS);
        assert!(offset_of!(GameSettings, camera_auto_rotation) == opt::AUTO_ROTATION);
    }
};

/// Round 31 (NT1, Ryan: "enemy name plates flicker when a group first loads; they must have the right position and
/// opacity from the first frame"). Round 30's "tags first frames" log (w101_orbs.log.round31_input, the group loads
/// at 23:02:14, 23:04:48 and 23:05:52, 53 characters each in one frame) proves what the plate anchor does then:
/// - the posed box is not readable on a character's first frames: 28 of 30 Dark Sprites, 52 of 76 Skeletal Pirates,
///   18 of 60 Lost Souls and 2 of 3 Rattlebones start with no box (plate from the capsule / fixed height); where it
///   turns up within 3 s it does so 0.02-0.18 s later (far ones, not animated, stay without one);
/// - the first box is the spawn pose, not the idle one: Dark Sprites read 2.53 m at +0.07 s, then 1.99 m at
///   +0.47-0.55 s; Lost Souls 3.00 -> 3.33 m at +0.08 s; Fear of Fire 7.19 -> 7.41 -> 7.61 m by +0.68 s.
/// So a Dark Sprite's plate stood at 1.80 m (fixed), jumped to 2.88 m, then dropped to 2.34 m inside half a second
/// (nameplates::anchor: box + 0.35), and Rattlebones' moves from 5.95 m to 10.60 m when his box is first read: the
/// flicker. Now each character's anchor settles first: for SETTLE seconds
/// after it is first seen it is not passed on (no plate, no floater anchor), so the first frame drawn has the
/// settled pose; after that, a box that can't be read for a frame keeps the last box's offset from the character
/// instead of falling back to the capsule height (the log also shows box -> none at +2.41 s for a Skeletal Pirate).
/// The state of a character not seen for FORGET seconds is dropped (a reload or respawn settles again).
/// Returns None while settling, else the box top to use (or None for the capsule / fixed height).
fn settle(chr: usize, model: u32, at: [f32; 3], top: Option<[f32; 3]>) -> Option<Option<[f32; 3]>> {
    const SETTLE: f32 = 0.75;
    const FORGET: f32 = 2.0;
    struct State { chr: usize, first: std::time::Instant, last: std::time::Instant, held: Option<[f32; 3]>, logged: bool }
    static STATES: std::sync::Mutex<Vec<State>> = std::sync::Mutex::new(Vec::new());
    let mut states = STATES.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    states.retain(|s| now.duration_since(s.last).as_secs_f32() < FORGET);
    let i = match states.iter().position(|s| s.chr == chr) {
        Some(i) => i,
        None => {
            states.push(State { chr, first: now, last: now, held: None, logged: false });
            states.len() - 1
        }
    };
    let s = &mut states[i];
    s.last = now;
    if let Some(t) = top {
        s.held = Some([t[0] - at[0], t[1] - at[1], t[2] - at[2]]);
    }
    let age = now.duration_since(s.first).as_secs_f32();
    if age < SETTLE {
        return None;
    }
    if !s.logged {
        s.logged = true;
        match s.held {
            Some(h) => tracing::info!("tags settled: c{model} {chr:#x} after {age:.2} s, box top {:.2} m{}", h[1],
                                      if top.is_none() { " (held: box not readable this frame)" } else { "" }),
            None => tracing::info!("tags settled: c{model} {chr:#x} after {age:.2} s, no box yet: capsule / fixed height"),
        }
    }
    Some(top.or(s.held.map(|h| [at[0] + h[0], at[1] + h[1], at[2] + h[2]])))
}

/// Round 26 (O3): logs a model's box against its physics position, once per model and then every 20 s (proof the
/// plate follows the pose: "tags: c4960 box top ...").
fn log_model_top(model: u32, at: [f32; 3], top: Option<([f32; 3], bool)>) {
    static LOGGED: std::sync::Mutex<Vec<(u32, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());
    let mut logged = LOGGED.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    if logged.iter().any(|(m, t)| *m == model && now.duration_since(*t).as_secs() < 20) {
        return;
    }
    logged.retain(|(m, _)| *m != model);
    logged.push((model, now));
    match top {
        Some((t, world)) => tracing::info!(
            "tags: c{model} model box top {:.2} m over its physics position, centre {:.2} m from it ({} space)",
            t[1] - at[1], (t[0] - at[0]).hypot(t[2] - at[2]), if world { "world" } else { "model" }),
        None => tracing::info!("tags: c{model} model box not readable; plate from the capsule / fixed height"),
    }
}

/// Round 23 (H2): whether the player's `jump` is a world rebase: more than half of the characters seen both at the
/// last sample (`old`) and now (`new`) moved by the same vector (within 2 m), and at least one was seen.
pub fn rebase(jump: [f32; 3], old: &[(usize, [f32; 3])], new: &[(usize, [f32; 3])]) -> bool {
    const MATCH: f32 = 2.0;
    let (mut same, mut seen) = (0, 0);
    for (chr, p) in new {
        let Some((_, q)) = old.iter().find(|(c, _)| c == chr) else { continue };
        seen += 1;
        let d = [p[0] - q[0] - jump[0], p[1] - q[1] - jump[1], p[2] - q[2] - jump[2]];
        if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < MATCH {
            same += 1;
        }
    }
    seen > 0 && same * 2 > seen
}

/// Round 25 (H3): an ActionButtonParam row's prompt shape: region type (0 cylinder), radius, angle (degrees), height
/// and base height offset (metres), and textId.
#[derive(Clone, Copy, Debug)]
pub struct ActionButton {
    pub region: u8,
    pub radius: f32,
    pub angle: i32,
    pub height: f32,
    pub base: f32,
    /// Round 27 (A1): angleCheckType (0x24) and allowAngle (0x28, the player's facing tolerance either side, degrees;
    /// 180 = any). `angle` (0x10) is the region's own sector round the target's front (180 = all round).
    pub angle_check: u8,
    pub allow_angle: i32,
    pub text_id: i32,
}

/// Round 25 (H3): the data address of row `row` of the param table named `name` (its PARAMDEF type), found through
/// SoloParamRepository's holders as probe.rs finds MenuOffscrRendParam; tables are looked up once and kept.
fn param_row(name: &[u8], row: u32) -> Option<usize> {
    use eldenring::cs::{ParamResCap, SoloParamRepository};
    use eldenring::fd4::FD4ParamResCap;
    use std::mem::offset_of;
    static FILES: std::sync::Mutex<Vec<(Vec<u8>, usize)>> = std::sync::Mutex::new(Vec::new());
    let mut files = FILES.lock().unwrap_or_else(|e| e.into_inner());
    let file = match files.iter().find(|f| f.0 == name) {
        Some(f) => f.1,
        None => {
            let repo = from_singleton::static_of::<SoloParamRepository>()?;
            let repo = read::<usize>(repo.as_ptr() as usize)?;
            let holders = repo + offset_of!(SoloParamRepository, solo_param_holders);
            const HOLDER: usize = 72;
            let file_of = |index: usize| -> Option<usize> {
                let res_cap = read::<usize>(holders + index * HOLDER + 8)?;
                let param = read::<usize>(res_cap + offset_of!(ParamResCap, param_res_cap))?;
                read::<usize>(param + offset_of!(FD4ParamResCap, data))
            };
            let named = |file: usize| -> bool {
                let Some(header) = read_bytes(file, 0x40) else { return false };
                let got = if header[0x2D] & 0x80 != 0 {
                    let offset = u32::from_le_bytes(header[0x10..0x14].try_into().unwrap()) as usize;
                    read_bytes(file + offset, name.len() + 1)
                } else {
                    Some(header[0x0C..(0x0C + name.len() + 1).min(0x40)].to_vec())
                };
                got.is_some_and(|g| g.starts_with(name) && g.get(name.len()).is_none_or(|c| *c == 0))
            };
            let file = (0..200).find_map(|i| file_of(i).filter(|f| named(*f)))?;
            files.push((name.to_vec(), file));
            file
        }
    };
    let header = read_bytes(file, 0x40)?;
    let rows = u16::from_le_bytes(header[0x0A..0x0C].try_into().unwrap()) as usize;
    let format = header[0x2D];
    let wide = format & 0x04 != 0;
    let extended = wide || (format & 0x01 != 0 && format & 0x02 != 0);
    let first = 0x30 + if extended { 0x10 } else { 0 };
    let entry = if wide { 24 } else { 12 };
    let table = read_bytes(file + first, rows.min(20_000) * entry)?;
    table.chunks_exact(entry).find(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()) == row).map(|d| {
        let offset = if wide { u64::from_le_bytes(d[8..16].try_into().unwrap()) as usize }
                     else { u32::from_le_bytes(d[4..8].try_into().unwrap()) as usize };
        file + offset
    })
}
