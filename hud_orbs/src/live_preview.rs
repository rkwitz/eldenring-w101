//! Live character preview (ini `live_preview = 1`, off by default; the software renderer, model.rs, stays the
//! fallback and is what the book shows whenever anything here is missing). An experiment toward a live, rotating
//! front view of the player in the Backpack / Character picture, drawn by Elden Ring's own mirror (CharaMake)
//! offscreen scene: see second_scene_research/probe_run2_findings.md.
//!
//! What it does:
//! - Picks the mirror scene's final picture as the game creates it (probe.rs's D3D12 creation hooks call
//!   `on_created`): a square RGBA8 render target whose creation stack holds FINAL_CHAIN (round 13 run 2: the
//!   first target of each of the scene's four sets, 784x784 at 2560x1440, the only one with background and full
//!   lighting). Each kept target is AddRef'd and released a few seconds after the menu closes. Which of the four
//!   sets is shown: ini `live_preview_slot` (0-3, default 0; run 2: 0 and 2 wore the equipment, 1 and 3 were
//!   bare), F9 cycles it.
//! - Phase 1 (ER's own mirror): while a menu is up and such a target exists, the book may be opened over it
//!   (View); the Backpack / Character picture then shows a GPU copy of that target, cropped to the picture's shape,
//!   made every frame on the game's queue (hudhook RenderContext::copy_texture; the target is assumed to be in
//!   ALL_SHADER_RESOURCE, ini `live_preview_state` overrides it). The controller's right stick reaches the game
//!   while it shows (ER's CharaMake turns its model with it); nothing else does.
//! - Phase 2 (the book drives it, unproven, armed with F10 or ini `live_preview_drive = 1`): when the book shows
//!   the Backpack / Character picture in normal play, the cosmetics CharaMake menu is opened through the EzState
//!   talk-event dispatcher the way The Grand Archives' public table does ("Cosmetics (Mirror)":
//!   OpenCharaMakeMenu(1), dispatcher found by its public byte pattern). Elden Ring's menu is covered by the world
//!   as it was before the book opened (the teleport poof's screen copy). When the picture goes away (book closed,
//!   another tab), the menu is closed with ForceCloseMenu, then CloseMenu; if it stays open the log says so and
//!   the book lets go (close it in Elden Ring with B).
//!
//! - Round 15:
//!   - Idle (A1): Elden Ring's mirror model does play an idle animation (Ryan), yet the copied set-0 picture stood
//!     still. So which target ER redraws is now measured, not assumed: every square RGBA8 render target of the
//!     scene is kept, and while the menu is up each is read back twice about a second apart (a 64x64 sample grid);
//!     the ones that changed and have the scene's dark brown background are the live pictures, and those are shown
//!     (equipped / bare by their set). "live: anim" lines log every target's change. The shown picture is then
//!     checked about every 1.5 s; if it stops changing the survey runs again. A slow 2D breathing
//!     (`breath_strips`, layout.rs) is only a fallback, ini `live_preview_fake_breath = 1`, used when no target
//!     changes.
//!   - Sets (A2): sets 0 and 2 are the same equipped picture; of the bare ones, set 1's head skin can come out black
//!     (its face skin not filled in; eyes, beard and hair fine), set 3's was right every time. F9 now toggles
//!     between the equipped set (ini `live_preview_slot`, 0) and the bare one (ini `live_preview_bare`, 3).
//!   - World (A3): the game keeps running under the mirror menu (60 fps, the camera and the player moved during
//!     F10 visits); what stood still was our cover, a screenshot. The cover is now a live copy of the world scene's
//!     own picture (a screen-sized RGBA8 render target the world renderer made, before Elden Ring's menu is drawn
//!     over it). Which target that is gets measured in normal play: each candidate is read back once next to the
//!     back buffer and the one that matches it (correlation of a 32x18 brightness grid) wins; "live: world" lines
//!     log it. Until one is found, the old screenshot stays.
//!
//! - Round 16:
//!   - Equipped set (A1): set 2 by default, not 0. Ryan's round 15 log: with F9 on set 3 the stick turned the model,
//!     on set 0 it didn't; set 0 is the equipped picture ER made once and doesn't redraw (as set 1 is for the bare
//!     body), set 2 its live twin. The survey's pick of one kind never stands in for the other.
//!   - Slow spin (A2): the book's own turning (its spin, a drag; book.rs reports the rate with the picture) is given
//!     to the game as a right stick x while the real stick rests (input::inject_right_x); ini `live_preview_turn`,
//!     `_turn_rate`, `_stick_dead`, `_turn_sign`; `live_preview_fake_pad = 1` makes up a controller without one.
//!   - Framing (A3): the whole square is copied; the Backpack picture shows its category's zoom of it and the
//!     Character page's portrait the head (image_strips; ini `live_preview_frame` = what the square covers); an
//!     arrow on its own keeps the software picture.
//!
//! - Round 17:
//!   - Portrait (B1): the Character page's portrait shows set 0, the equipped picture Elden Ring draws once when the
//!     mirror opens (facing the camera) and never redraws or turns, copied in its own slot (2). The right stick
//!     only reaches the game while the Backpack's live picture is up, never for the portrait alone.
//!   - World camera (B3): round 16's log (probe "cam" lines) showed the world camera circling the player while the
//!     mirror was up: the stick that turns the mirror's model (ours and the real one) also turns Elden Ring's
//!     camera under the menu, so after the book closed the camera looked another way, and the screenshot cover
//!     jumped to that. Now: the cover is the last clean frame before the mirror opened (capture slot 3, every frame
//!     while the book is open and no Elden Ring menu is up), its camera yaw is kept, and before the mirror closes
//!     the camera is turned back to it with an injected right stick x (input::inject_camera_x, closed loop on the
//!     camera matrix; the direction learnt from the spin, else probed), also continuously while the mirror is up
//!     and the Backpack picture isn't (Character page). Only stick x reaches the game (y would tilt the camera).
//!     After the menu closes the cover stays a moment and fades out (ini live_close_hold / live_close_fade). The
//!     mirror also stays up for live_preview_linger seconds on other pages (quick tab flips don't reopen it); it
//!     closes at once when the book closes or goes to the map page.
//!
//! - Round 18 (A1-A3; Ryan: the game and the preview run at the same time, never frozen; tame ER's mirror first):
//!   - Default on (A2): the book drives the mirror without F10 (ini `live_preview_drive = 0` turns it off).
//!   - World (A1): round 17's log never matched an RGBA8 world candidate (best r 0.53); the world scene's own
//!     picture before ER's UI is most likely its R10G10B10A2 target (EROverlay: the main target is format 24), so
//!     formats 24 and 10 are candidates too. Once found, the world behind the book stays a live copy while the
//!     mirror opens, is up and closes; when it closes the copy stands until Elden Ring's screen is back (no hold,
//!     no fade of an old screenshot). Without it the cover goes at once (ini live_close_hold 0 / live_close_fade
//!     0.15 by default).
//!   - Walking (A1): with the book driving the mirror, W/A/S/D and the left stick still reach the game (ini
//!     `live_walk = 0` stops it); "live: walk" lines log whether the player really moved under the menu. When it
//!     did, the mirror stays up for the whole book (ini live_preview_linger overrides).
//!   - No camera spin: the book's own slow spin no longer turns ER's model (the same stick turns the world
//!     camera, now visible live behind the book); a drag still does (ini `live_preview_auto_spin = 1` brings the
//!     spin back).
//!   - Background (A3): the picture is drawn with ER's flat scene background (RGB 23,20,14) keyed out
//!     (hudhook BLEND_KEY), so the window's own Wizard101 sky (portrait_back, Background_Inventory_Sky) shows.
//!
//! Fail-safe: no target, no copy, no dispatcher, or the menu not opening / closing: the software picture stays and
//! a "live:" line says why. Nothing here writes game memory; phase 2 calls one game function.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use hudhook::imgui::{ImColor32, Ui};
use hudhook::RenderContext;

use crate::layout::Textures;
use crate::probe;

/// Return addresses (RVAs, the game version of round 13) in a row on the creation stack of the mirror scene's
/// final picture (and of no other 784x784 target).
const FINAL_CHAIN: [usize; 3] = [0x19f4347, 0x1afc21b, 0x1a20ea7];
/// Deeper return addresses telling the scene's four sets apart (set 0..3, in creation order).
const SCENE_SLOTS: [usize; 4] = [0x8ae371, 0x8ae3af, 0x8ae3ed, 0x8ae42f];
/// The Grand Archives' pattern for the EzState talk-event dispatcher (tga/ezstate.h), and the function's start
/// before it.
const DISPATCH_PATTERN: &str = "48 8B DA 4C 8B F1 33 FF 89 7D";
const DISPATCH_BACK: usize = 0x47;
/// EzState external events (TGA's EzState_code.cea).
const OPEN_CHARA_MAKE_MENU: i32 = 81;
const FORCE_CLOSE_MENU: i32 = 67;
const CLOSE_MENU: i32 = 32;
/// D3D12_RESOURCE_STATE_ALL_SHADER_RESOURCE: the state the probe's readbacks assumed (they came out right).
const DEFAULT_STATE: u32 = 0xC0;
/// The book's full character picture (pages::MODEL_FULL) and the teleport poof's world copy (teleport.rs).
const MODEL_FULL: &str = crate::pages::MODEL_FULL;
const TELEPORT_WORLD: &str = "@screen/teleport_world";
/// The live copy of the world scene's picture (A3), and the copy slot it uses (slot 0 is the character).
const LIVE_WORLD: &str = "@screen/live_world";
const WORLD_SLOT: usize = 1;
/// The bare set F9 shows unless the ini says otherwise (set 1's face skin can be black, A2).
const DEFAULT_BARE: usize = 3;
/// A world candidate must correlate at least this well with the back buffer.
const WORLD_MATCH: f32 = 0.85;
/// The brightness grid compared (columns, rows).
const GRID: [usize; 2] = [32, 18];
/// Breathing (A1): seconds per breath, how far the head lifts and how much the chest widens (fractions of the
/// picture).
const BREATH_PERIOD: f32 = 4.2;
const BREATH_LIFT: f32 = 0.006;
const BREATH_WIDEN: f32 = 0.005;
const BREATH_STRIPS: usize = 24;
/// The animation survey: the sample grid's side, the mean byte difference that counts as "changed", and the
/// scene's background (run 2: RGB 23,20,14) with its tolerance.
const ANIM_GRID: usize = 64;
const ANIM_CHANGED: f32 = 0.1;
const ANIM_BACKGROUND: [f32; 3] = [23.0, 20.0, 14.0];
const ANIM_BACKGROUND_TOLERANCE: f32 = 18.0;
/// A new scene counter (each burst of square targets), so the survey knows when to start over.
static SCENE: AtomicU32 = AtomicU32::new(0);
/// The fake breathing may draw (ini on and no changing target found).
static FAKE_BREATH: AtomicBool = AtomicBool::new(false);

static ENABLED: AtomicBool = AtomicBool::new(false);
/// The dispatcher's address (0: not looked for or not found), and whether the scan ran.
static DISPATCH: AtomicUsize = AtomicUsize::new(0);
static SCANNED: AtomicBool = AtomicBool::new(false);
/// "live:" lines about kept targets (capped).
static KEPT_LINES: AtomicU32 = AtomicU32::new(0);

struct Target {
    resource: usize,
    size: u32,
    set: Option<usize>,
    by_stack: bool,
    retired: Option<Instant>,
}

#[derive(Default)]
struct Shared {
    targets: Vec<Target>,
    last_made: Option<Instant>,
    burst: u32,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared { targets: Vec::new(), last_made: None, burst: 0 });

/// Screen-shaped RGBA8 render targets the game made (candidates for the world scene's picture, A3).
struct WorldTarget {
    resource: usize,
    width: u32,
    height: u32,
    /// The return addresses that tell it apart (RVAs after the render-target helper), for the log.
    stack: Vec<usize>,
    batch: u32,
    retired: Option<Instant>,
}

struct World {
    targets: Vec<WorldTarget>,
    last_made: Option<Instant>,
    batch: u32,
}

static WORLD: Mutex<World> = Mutex::new(World { targets: Vec::new(), last_made: None, batch: 0 });
/// The live picture is in the book this frame (the breathing applies to it only).
static SHOWING: AtomicBool = AtomicBool::new(false);
static BREATH_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
/// The book's full character picture this frame, as book.rs reports it: its size in pixels, its framing (centre
/// height and view height in metres, model::Framing / pages::backpack_zoom), how fast the book turns it (radians per
/// second: its slow spin or a drag), and when it was drawn.
#[derive(Clone, Copy)]
struct Shown {
    size: [u32; 2],
    view: [f32; 2],
    turn: f32,
    at: Instant,
}

static PICTURE: Mutex<Option<Shown>> = Mutex::new(None);
/// The Character page's portrait this frame (its size, when it was drawn).
static PORTRAIT: Mutex<Option<([u32; 2], Instant)>> = Mutex::new(None);
/// The part of the live square picture each book picture shows (uv rect; full picture, portrait), while it shows.
static CROPS: Mutex<[Option<[f32; 4]>; 2]> = Mutex::new([None; 2]);
/// The portrait's framing (model::Framing::Portrait with a helm: a hat reaches well above the head).
const PORTRAIT_VIEW: [f32; 2] = [1.66, 0.62];
/// What the mirror's square picture covers by default (centre height, view height in metres): row 0 of
/// MenuOffscrRendParam looks at y 0.85 from 2.66 m with a 40 degree fov, about 1.94 m tall, which is the book's own
/// full framing (model::Framing::Full, 0.9 / 1.92) within a few centimetres; so the full picture is the whole square.
const MIRROR_FRAME: [f32; 2] = [0.9, 1.92];
/// The book's equipped set unless the ini says otherwise (A1, round 16). Runs 2 and 15: set 1 (the first bare one)
/// can have a black face (not kept up), set 3 (the second) was right every time and is the one that turns with the
/// stick (Ryan, round 15 log: F9 to set 3 turned, set 0 didn't). The equipped pair is made in the same order, so
/// set 0 is the equipped one ER doesn't redraw and set 2 its live twin.
const DEFAULT_EQUIPPED: usize = 2;
/// B1: the portrait's still front picture (set 0: drawn once when the mirror opens) and its copy slot.
const STILL_SET: usize = 0;
const PORTRAIT_SLOT: usize = 2;
/// B3: the world as it was just before the mirror opened (capture slot 3).
const COVER: &str = "@screen/live_cover";
const COVER_SLOT: usize = 3;
/// B3 defaults (ini live_preview_linger, live_close_hold, live_close_fade): seconds the mirror stays up on another
/// page, the cover stays after Elden Ring's menu closed, and it takes to fade out.
const LINGER: f32 = 3.0;
const CLOSE_HOLD: f32 = 0.0;
const CLOSE_FADE: f32 = 0.15;
/// B3: the camera counts as back within this yaw (radians, about 2 degrees); the return gives up after this long.
const RETURN_TOLERANCE: f32 = 0.035;
const RETURN_MAX: f32 = 2.0;
/// The book is open on a page that may keep the mirror up (not closed, not the map page), as book.rs says.
static BOOK_KEEP: AtomicBool = AtomicBool::new(false);
/// Round 18: the book's own slow spin turns ER's model (ini live_preview_auto_spin), walking under the book-driven
/// mirror (ini live_walk) and whether the mirror is up for the book now.
static AUTO_SPIN: AtomicBool = AtomicBool::new(false);
static WALK: AtomicBool = AtomicBool::new(true);
static MIRROR_UP: AtomicBool = AtomicBool::new(false);
/// Round 18 A1: world candidates' formats (R8G8B8A8, R10G10B10A2, R16G16B16A16_FLOAT) and how many are kept.
const WORLD_FORMATS: [u32; 3] = [28, 24, 10];
const WORLD_KEEP: usize = 40;

/// Round 19 A3: the live copy of the world scene's picture behind the book (ini live_world = 1). Off by default: the
/// picture that matched the screen (R10G10B10A2) shows the world grey and untextured while ER's mirror menu is up, so
/// the frame from before the mirror opened stands behind the book instead.
static WORLD_LIVE: AtomicBool = AtomicBool::new(false);

/// book.rs: W/A/S/D and the left stick reach the game while the book drives the mirror (round 18 A1).
pub fn walk_under_mirror() -> bool {
    enabled() && WALK.load(Ordering::Relaxed) && MIRROR_UP.load(Ordering::Relaxed)
}

/// layout.rs: the live picture is drawn with the mirror scene's flat background keyed out (round 18 A3); the book
/// draws on the foreground list.
pub fn key_blend(on: bool) {
    use hudhook::imgui::sys;
    let mode = if on { hudhook::BLEND_KEY } else { hudhook::BLEND_NORMAL };
    unsafe { sys::ImDrawList_AddCallback(sys::igGetForegroundDrawList(), Some(hudhook::blend_callback), mode as *mut _) };
}

/// book.rs, every frame: the book is open on a page that may keep the hidden mirror up (B3).
pub fn note_book(keep: bool) {
    BOOK_KEEP.store(keep, Ordering::Relaxed);
}

fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(t) - std::f32::consts::PI
}

/// Turning the world camera back (B3).
#[derive(Default)]
struct Return {
    start: Option<Instant>,
    start_error: f32,
    frames: u32,
    prev_yaw: Option<f32>,
    moved: f32,
    ok: u32,
    flipped: bool,
}
/// Turning the mirror's model by the stick (A1, A2): ER's CharaMake turn speed at full stick (radians per second,
/// not measured: ini `live_preview_turn_rate`) and the dead zone it ignores (ini `live_preview_stick_dead`).
const TURN_RATE: f32 = 3.0;
const STICK_DEAD: f32 = 0.24;

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// book.rs: the full character picture is on the page at this size and framing this frame, turning at `turn`.
pub fn note_box(size: [u32; 2], view: [f32; 2], turn: f32) {
    if enabled() {
        *PICTURE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Shown { size, view, turn, at: Instant::now() });
    }
}

/// book.rs: the Character page's portrait is on the page at this size this frame (A3).
pub fn note_portrait(size: [u32; 2]) {
    if enabled() {
        *PORTRAIT.lock().unwrap_or_else(|e| e.into_inner()) = Some((size, Instant::now()));
    }
}

fn picture() -> Option<Shown> {
    PICTURE.lock().unwrap_or_else(|e| e.into_inner()).filter(|p| p.at.elapsed() < Duration::from_millis(300))
}

fn portrait() -> Option<[u32; 2]> {
    PORTRAIT.lock().unwrap_or_else(|e| e.into_inner())
        .filter(|p| p.1.elapsed() < Duration::from_millis(300)).map(|p| p.0)
}

/// A live picture is wanted: the full picture or the portrait is on the page.
fn wanted() -> bool {
    picture().is_some() || portrait().is_some()
}

/// probe.rs's creation hooks: a 2D render target or depth texture the game just made.
pub fn on_created(width: u64, height: u32, format: u32, flags: u32, resource: usize) {
    if enabled() && resource != 0 && WORLD_FORMATS.contains(&format) && flags & 0x1 != 0 && width != height as u64
        && width >= 1024 && height >= 576 {
        on_world_created(width as u32, height, format, resource);
        return;
    }
    if !enabled() || resource == 0 || format != 28 || flags & 0x1 == 0 || width != height as u64
        || !(256..=2048).contains(&height) {
        return;
    }
    let mut frames = [std::ptr::null_mut::<std::ffi::c_void>(); 40];
    let n = unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace(
            1, frames.len() as u32, frames.as_mut_ptr(), std::ptr::null_mut())
    } as usize;
    let rvas: Vec<usize> = frames[..n.min(40)].iter().map(|f| *f as usize).filter(|f| probe::in_image(*f))
        .map(probe::rva).collect();
    let by_stack = rvas.windows(3).any(|w| w == FINAL_CHAIN);
    let set = rvas.iter().find_map(|r| SCENE_SLOTS.iter().position(|s| s == r));
    let now = Instant::now();
    let mut sh = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    if sh.last_made.is_none_or(|t| now - t > Duration::from_secs(3)) {
        // A new scene: the last one's pictures go.
        for t in sh.targets.iter_mut().filter(|t| t.retired.is_none()) {
            t.retired = Some(now);
        }
        sh.burst = 0;
        SCENE.fetch_add(1, Ordering::Relaxed);
    }
    sh.last_made = Some(now);
    sh.burst += 1;
    // Every square RGBA8 target of the scene is kept: the animation survey finds which ones ER redraws. Without a
    // survey result the final picture by its stack is shown, else the first of the scene (run 2: also the final).
    if sh.targets.iter().filter(|t| t.retired.is_none()).count() >= 48 {
        return;
    }
    unsafe { probe::com_add_ref(resource) };
    let first = sh.burst == 1;
    sh.targets.push(Target { resource, size: height, set, by_stack, retired: None });
    if (by_stack || first) && KEPT_LINES.fetch_add(1, Ordering::Relaxed) < 64 {
        tracing::info!("live: kept {width}x{height} fmt 28 {resource:#x} (set {set:?}, {})",
                       if by_stack { "stack matched" } else { "first of its scene, stack not matched" });
    }
}

/// A screen-shaped RGBA8 render target: kept as a candidate for the world scene's picture. A new batch (more than
/// 3 s after the last one: a load, a resolution change) replaces the old ones.
fn on_world_created(width: u32, height: u32, format: u32, resource: usize) {
    let mut frames = [std::ptr::null_mut::<std::ffi::c_void>(); 24];
    let n = unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace(
            1, frames.len() as u32, frames.as_mut_ptr(), std::ptr::null_mut())
    } as usize;
    let rvas: Vec<usize> = frames[..n.min(24)].iter().map(|f| *f as usize).filter(|f| probe::in_image(*f))
        .map(probe::rva).collect();
    // The allocator frames are the same for every target; keep what follows 0x19f4347 (the render-target helper).
    let stack: Vec<usize> = match rvas.iter().position(|r| *r == FINAL_CHAIN[0]) {
        Some(i) => rvas[i + 1..].iter().take(4).copied().collect(),
        None => rvas.iter().rev().take(4).copied().collect(),
    };
    let now = Instant::now();
    let mut w = WORLD.lock().unwrap_or_else(|e| e.into_inner());
    if w.last_made.is_none_or(|t| now - t > Duration::from_secs(3)) {
        w.batch += 1;
        for t in w.targets.iter_mut().filter(|t| t.retired.is_none()) {
            t.retired = Some(now);
        }
    }
    w.last_made = Some(now);
    if w.targets.iter().filter(|t| t.retired.is_none()).count() >= WORLD_KEEP {
        return;
    }
    unsafe { probe::com_add_ref(resource) };
    let batch = w.batch;
    tracing::info!("live: world candidate {width}x{height} fmt {format} {resource:#x} (batch {batch}, stack {:x?})",
                   stack);
    w.targets.push(WorldTarget { resource, width, height, stack, batch, retired: None });
}

/// A 32x18 grid of brightness (0-255) over a readback, for the formats the game and the swap chain use.
fn brightness_grid(rb: &hudhook::Readback) -> Option<Vec<f32>> {
    let (w, h, pitch) = (rb.width as usize, rb.height as usize, rb.pitch as usize);
    let bpp = match rb.format {
        28 | 29 | 87 | 91 | 24 => 4,
        10 => 8,
        _ => return None,
    };
    if w < GRID[0] * 4 || h < GRID[1] * 4 || rb.data.len() < pitch * h {
        return None;
    }
    let half = |b: u16| -> f32 {
        let (sign, exp, frac) = ((b >> 15) as u32, ((b >> 10) & 0x1f) as i32, (b & 0x3ff) as f32);
        let v = if exp == 0 {
            frac / 1024.0 * 2f32.powi(-14)
        } else if exp == 31 {
            0.0
        } else {
            (1.0 + frac / 1024.0) * 2f32.powi(exp - 15)
        };
        if sign == 1 { 0.0 } else { (v / (1.0 + v)).powf(1.0 / 2.2) * 255.0 }
    };
    let pixel = |x: usize, y: usize| -> f32 {
        let o = y * pitch + x * bpp;
        let d = &rb.data[o..o + bpp];
        let (r, g, b) = match rb.format {
            28 | 29 => (d[0] as f32, d[1] as f32, d[2] as f32),
            87 | 91 => (d[2] as f32, d[1] as f32, d[0] as f32),
            24 => {
                let v = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
                let c = |s: u32| ((v >> s) & 1023) as f32 * 255.0 / 1023.0;
                (c(0), c(10), c(20))
            }
            _ => (half(u16::from_le_bytes([d[0], d[1]])), half(u16::from_le_bytes([d[2], d[3]])),
                  half(u16::from_le_bytes([d[4], d[5]]))),
        };
        0.299 * r + 0.587 * g + 0.114 * b
    };
    let mut grid = Vec::with_capacity(GRID[0] * GRID[1]);
    for cy in 0..GRID[1] {
        for cx in 0..GRID[0] {
            let mut sum = 0.0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = ((cx * 4 + sx) * 2 + 1) * w / (GRID[0] * 8);
                    let y = ((cy * 4 + sy) * 2 + 1) * h / (GRID[1] * 8);
                    sum += pixel(x, y);
                }
            }
            grid.push(sum / 16.0);
        }
    }
    Some(grid)
}

/// Correlation of two grids (1: the same picture up to brightness and contrast) and their mean difference.
fn compare(a: &[f32], b: &[f32]) -> (f32, f32) {
    let n = a.len().min(b.len()) as f32;
    if n == 0.0 {
        return (0.0, 255.0);
    }
    let (ma, mb) = (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n);
    let (mut sab, mut saa, mut sbb, mut diff) = (0.0, 0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
        diff += (x - y).abs();
    }
    let r = if saa > 1e-3 && sbb > 1e-3 { sab / (saa * sbb).sqrt() } else { 0.0 };
    (r, diff / n)
}

/// layout.rs, for the book's character pictures while the live picture is in them: the strips (top and bottom as
/// fractions of the book picture, and the uv rect of the live square each shows). One strip for the part of the
/// square the picture's framing covers (the Backpack zooms on a category, the portrait on the head); many for the
/// fallback breathing. None: draw the image as it is.
pub fn image_strips(name: &str) -> Option<Vec<([f32; 2], [f32; 4])>> {
    let k = match name {
        MODEL_FULL => 0,
        n if n == crate::pages::MODEL_PORTRAIT => 1,
        _ => return None,
    };
    if !SHOWING.load(Ordering::Relaxed) {
        return None;
    }
    let crop = CROPS.lock().unwrap_or_else(|e| e.into_inner())[k]?;
    let inside = |uv: [f32; 4]| [crop[0] + uv[0] * (crop[2] - crop[0]), crop[1] + uv[1] * (crop[3] - crop[1]),
                                 crop[0] + uv[2] * (crop[2] - crop[0]), crop[1] + uv[3] * (crop[3] - crop[1])];
    if k == 0 && FAKE_BREATH.load(Ordering::Relaxed) {
        return Some(breath_strips().into_iter().map(|(y, uv)| (y, inside(uv))).collect());
    }
    Some(vec![([0.0, 1.0], crop)])
}

/// The fallback breathing (ini live_preview_fake_breath): strips of the picture, each with its uv rect.
fn breath_strips() -> Vec<([f32; 2], [f32; 4])> {
    let t = BREATH_START.get_or_init(Instant::now).elapsed().as_secs_f32();
    // 0 (breathed out) .. 1 (in), eased.
    let b = 0.5 - 0.5 * (t * std::f32::consts::TAU / BREATH_PERIOD).cos();
    // How much each height lifts (head and shoulders all of it, nothing from the hips down) and widens (the chest).
    let lift = |y: f32| BREATH_LIFT * b * ((0.55 - y) / (0.55 - 0.18)).clamp(0.0, 1.0);
    let widen = |y: f32| BREATH_WIDEN * b * (1.0 - ((y - 0.3) / 0.15).powi(2)).max(0.0);
    (0..BREATH_STRIPS).map(|k| {
        let (y0, y1) = (k as f32 / BREATH_STRIPS as f32, (k + 1) as f32 / BREATH_STRIPS as f32);
        let half = 0.5 / (1.0 + widen((y0 + y1) / 2.0));
        ([y0, y1], [0.5 - half, (y0 + lift(y0)).min(1.0), 0.5 + half, (y1 + lift(y1)).min(1.0)])
    }).collect()
}

/// The uv rect of the mirror's square picture that a book picture of `size` pixels framed as `view` (centre height,
/// view height in metres) shows, given what the square covers (`frame`), kept inside the square (the sampler
/// wraps).
fn crop(frame: [f32; 2], view: [f32; 2], size: [u32; 2]) -> [f32; 4] {
    let h = (view[1] / frame[1]).clamp(0.05, 1.0);
    let w = (h * size[0] as f32 / size[1].max(1) as f32).min(1.0);
    let v0 = (0.5 - (view[0] + view[1] / 2.0 - frame[0]) / frame[1]).clamp(0.0, 1.0 - h);
    let u0 = 0.5 - w / 2.0;
    [u0, v0, u0 + w, v0 + h]
}

/// Finding the world scene's picture among the candidates: the back buffer and each candidate are read back in
/// turn (one readback every few frames) and compared.
#[derive(Default)]
struct WorldSearch {
    batch: u32,
    /// Candidates still to compare, and the readback asked for.
    queue: Vec<usize>,
    pending: Option<usize>,
    screen: Option<Vec<f32>>,
    scores: Vec<(usize, f32, f32)>,
    frames: u32,
    /// The chosen picture (resource, width, height) for `batch`, or when the search last gave up.
    chosen: Option<(usize, u32, u32)>,
    gave_up: Option<Instant>,
    /// While a menu is up: the last grid of the chosen picture, to log whether the world still moves under it.
    menu_grid: Option<Vec<f32>>,
    menu_samples: u32,
    menu_said: Option<bool>,
}

/// 64x64 RGB point samples of a square RGBA8 readback, and the mean colour of its four corner cells (the scene's
/// background in the final picture).
fn anim_sample(rb: &hudhook::Readback) -> Option<(Vec<u8>, [f32; 3])> {
    if !matches!(rb.format, 28 | 29) || rb.width < ANIM_GRID as u32 || rb.height < ANIM_GRID as u32 {
        return None;
    }
    let (w, h, pitch) = (rb.width as usize, rb.height as usize, rb.pitch as usize);
    if rb.data.len() < pitch * h {
        return None;
    }
    let mut out = Vec::with_capacity(ANIM_GRID * ANIM_GRID * 3);
    for gy in 0..ANIM_GRID {
        for gx in 0..ANIM_GRID {
            let (x, y) = ((gx * 2 + 1) * w / (ANIM_GRID * 2), (gy * 2 + 1) * h / (ANIM_GRID * 2));
            let o = y * pitch + x * 4;
            out.extend_from_slice(&rb.data[o..o + 3]);
        }
    }
    let mut corner = [0.0f32; 3];
    for (gx, gy) in [(0, 0), (ANIM_GRID - 1, 0), (0, ANIM_GRID - 1), (ANIM_GRID - 1, ANIM_GRID - 1)] {
        let o = (gy * ANIM_GRID + gx) * 3;
        for c in 0..3 {
            corner[c] += out[o + c] as f32 / 4.0;
        }
    }
    Some((out, corner))
}

fn anim_diff(a: &[u8], b: &[u8]) -> f32 {
    let n = a.len().min(b.len()).max(1) as f32;
    a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as f32).sum::<f32>() / n
}

/// Which of the scene's square targets ER redraws while its menu is up (A1).
#[derive(Default)]
struct AnimSurvey {
    scene: u32,
    started: bool,
    queue: Vec<usize>,
    second: bool,
    first: Vec<(usize, Vec<u8>)>,
    pending: Option<usize>,
    frames: u32,
    /// (resource, set, by stack, change, final-looking background)
    results: Vec<(usize, Option<usize>, bool, f32, bool)>,
    done: bool,
    /// The live pictures found: equipped, bare.
    picks: [Option<usize>; 2],
    /// Watching the shown picture: its last sample, checks in a row without change, surveys run this scene.
    watch_last: Option<(usize, Vec<u8>)>,
    still: u32,
    runs: u32,
    said_still: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Drive {
    Off,
    Armed,
    Opening(Instant),
    Open,
    Closing(Instant, bool),
    /// B3: the book let go; the camera is being turned back before the mirror closes.
    Returning(Instant),
    /// B3: Elden Ring's menu closed; the cover stays a moment, then fades.
    Settling(Instant, bool),
    Failed,
}

pub struct LivePreview {
    set: usize,
    /// F9 toggles between these two sets (equipped, bare).
    sets: [usize; 2],
    world_state: u32,
    search: WorldSearch,
    /// The live world copy is ready this frame.
    world_live: bool,
    anim: AnimSurvey,
    fake_breath: bool,
    state: u32,
    drive: Drive,
    hud: Option<u8>,
    hud_since: Instant,
    /// The book's own pictures (the software renderer's textures: full, portrait) while the live one stands in.
    soft: [Option<(hudhook::imgui::TextureId, [f32; 2])>; 2],
    /// What the mirror's square covers (centre height, view height in metres; ini `live_preview_frame`).
    frame: [f32; 2],
    /// Turning the mirror's model by the stick the way the book turns its picture (ini `live_preview_turn`), the
    /// assumed full-stick rate, dead zone and direction, and whether a controller may be made up for it.
    turn: bool,
    turn_rate: f32,
    stick_dead: f32,
    turn_sign: f32,
    fake_pad: bool,
    turn_said: bool,
    showing: Option<usize>,
    shown_once: Vec<usize>,
    last_attempt: Option<Instant>,
    keys: [bool; 2],
    why_not: String,
    // B1 / B3 (round 17).
    still_said: bool,
    linger: f32,
    close_hold: f32,
    close_fade: f32,
    cover_at: Option<Instant>,
    cover_view: Option<[f32; 2]>,
    saved_view: Option<[f32; 2]>,
    view: Option<[f32; 2]>,
    last_turn: Option<(f32, f32)>,
    learn: f32,
    cam_sign: Option<f32>,
    probe_sign: f32,
    ret: Return,
    returned: bool,
    gave_up: bool,
    last_shown: Instant,
    wanted_since: Option<Instant>,
    // Round 18.
    /// The ini set live_preview_linger (else it grows to the whole book once walking under the mirror works).
    linger_fixed: bool,
    /// The player's position this frame (lib.rs), and the walk check: where the player was when a move key went
    /// down under the mirror, when, and what was logged.
    player: Option<[f32; 3]>,
    walk_from: Option<([f32; 3], Instant)>,
    walk_lines: u32,
    walk_works: Option<bool>,
}

impl LivePreview {
    /// Reads the ini's live_preview keys (the Settings page doesn't know them: saving there drops them, which
    /// takes effect next launch). None when off.
    pub fn new(ini: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(ini).unwrap_or_default();
        let get = |key: &str| text.lines().filter_map(|l| l.split_once('=')).find(|(k, _)| k.trim() == key)
            .map(|(_, v)| v.trim().to_string());
        if get("live_preview").as_deref() != Some("1") {
            return None;
        }
        let set = get("live_preview_slot").and_then(|v| v.parse::<usize>().ok()).unwrap_or(DEFAULT_EQUIPPED).min(3);
        let float = |key: &str, default: f32| get(key).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite())
            .unwrap_or(default);
        let frame = get("live_preview_frame")
            .and_then(|v| v.split_once(',').and_then(|(a, b)| Some([a.trim().parse().ok()?, b.trim().parse().ok()?])))
            .filter(|f: &[f32; 2]| f[1] > 0.1)
            .unwrap_or(MIRROR_FRAME);
        let turn = get("live_preview_turn").as_deref() != Some("0");
        let turn_rate = float("live_preview_turn_rate", TURN_RATE).max(0.1);
        let stick_dead = float("live_preview_stick_dead", STICK_DEAD).clamp(0.0, 0.9);
        let turn_sign = if float("live_preview_turn_sign", 1.0) < 0.0 { -1.0 } else { 1.0 };
        let fake_pad = get("live_preview_fake_pad").as_deref() == Some("1");
        let bare = get("live_preview_bare").and_then(|v| v.parse::<usize>().ok()).unwrap_or(DEFAULT_BARE).min(3);
        let world_state = get("live_world_state")
            .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_STATE);
        let state = get("live_preview_state")
            .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_STATE);
        // Round 18 A2: on by default.
        let drive = get("live_preview_drive").as_deref() != Some("0");
        AUTO_SPIN.store(get("live_preview_auto_spin").as_deref() == Some("1"), Ordering::Relaxed);
        WORLD_LIVE.store(get("live_world").as_deref() == Some("1"), Ordering::Relaxed);
        WALK.store(get("live_walk").as_deref() != Some("0"), Ordering::Relaxed);
        let linger_fixed = get("live_preview_linger").is_some();
        let fake_breath = get("live_preview_fake_breath").as_deref() == Some("1");
        let linger = float("live_preview_linger", LINGER).clamp(0.0, 600.0);
        let close_hold = float("live_close_hold", CLOSE_HOLD).clamp(0.0, 5.0);
        let close_fade = float("live_close_fade", CLOSE_FADE).clamp(0.0, 5.0);
        ENABLED.store(true, Ordering::Relaxed);
        tracing::info!("live: live preview on (set {set}, bare set {bare}, assumed state {state:#x}, world state \
                        {world_state:#x}, drive {drive}, frame {frame:?}, turn {turn} (full stick {turn_rate} rad/s, \
                        dead zone {stick_dead}, sign {turn_sign}, fake pad {fake_pad}); walk {}, auto spin {}, close \
                        hold {close_hold} fade {close_fade}); F9 toggles equipped / bare, F10 arms / disarms the \
                        book-driven mirror", WALK.load(Ordering::Relaxed), AUTO_SPIN.load(Ordering::Relaxed));
        Some(LivePreview {
            set,
            sets: [set, bare],
            world_state,
            search: WorldSearch::default(),
            world_live: false,
            anim: AnimSurvey::default(),
            fake_breath,
            state,
            drive: if drive { Drive::Armed } else { Drive::Off },
            hud: None,
            hud_since: Instant::now(),
            soft: [None; 2],
            frame,
            turn,
            turn_rate,
            stick_dead,
            turn_sign,
            fake_pad,
            turn_said: false,
            showing: None,
            shown_once: Vec::new(),
            last_attempt: None,
            keys: [false; 2],
            why_not: String::new(),
            still_said: false,
            linger,
            close_hold,
            close_fade,
            cover_at: None,
            cover_view: None,
            saved_view: None,
            view: None,
            last_turn: None,
            learn: 0.0,
            cam_sign: None,
            probe_sign: 1.0,
            ret: Return::default(),
            returned: true,
            gave_up: false,
            last_shown: Instant::now(),
            wanted_since: None,
            linger_fixed,
            player: None,
            walk_from: None,
            walk_lines: 0,
            walk_works: None,
        })
    }

    fn hud_stable(&self, state: u8, seconds: f32) -> bool {
        self.hud == Some(state) && self.hud_since.elapsed().as_secs_f32() >= seconds
    }

    /// The book may stay open with Elden Ring's HUD hidden: the book drives the mirror menu, or Elden Ring's own
    /// mirror scene is up (phase 1).
    pub fn holds_book(&self) -> bool {
        self.drives() || (self.hud == Some(0) && self.current().is_some())
    }

    /// The target to show: the survey's live picture of the wanted kind (equipped / bare), else the configured
    /// set's final picture (by stack), else the first one kept. The other kind's live picture never stands in (A1:
    /// the equipped look stays equipped).
    fn current(&self) -> Option<(usize, u32)> {
        let sh = SHARED.lock().unwrap_or_else(|e| e.into_inner());
        let live: Vec<&Target> = sh.targets.iter().filter(|t| t.retired.is_none()).collect();
        let bare = usize::from(self.set == self.sets[1]);
        let picked = self.anim.picks[bare];
        live.iter().find(|t| Some(t.resource) == picked)
            .or_else(|| live.iter().find(|t| t.by_stack && t.set == Some(self.set)))
            .or_else(|| live.iter().filter(|t| t.by_stack).nth(self.set))
            .or_else(|| live.first())
            .map(|t| (t.resource, t.size))
    }

    /// B1: the still front picture for the portrait (set 0's final picture, drawn once when the mirror opened).
    fn still(&self) -> Option<(usize, u32)> {
        let sh = SHARED.lock().unwrap_or_else(|e| e.into_inner());
        sh.targets.iter().find(|t| t.retired.is_none() && t.by_stack && t.set == Some(STILL_SET))
            .map(|t| (t.resource, t.size))
    }

    fn say(&mut self, why: &str) {
        if self.why_not != why {
            tracing::info!("live: {why}");
            self.why_not = why.to_string();
        }
    }

    /// Before the frame is drawn (render thread): hotkeys, target lifetimes, and the copy into the book's picture.
    /// `camera`: the world camera's forward row (Game::camera), for B3's turning back.
    pub fn before_render(&mut self, render: &mut dyn RenderContext, textures: &mut Textures,
                         camera: Option<[f32; 3]>, player: Option<[f32; 3]>) {
        let hud = crate::game::hud_state();
        self.player = player;
        self.walk_check();
        self.view = camera.filter(|f| f.iter().all(|v| v.is_finite()))
            .map(|f| [f[0].atan2(f[2]), f[1].clamp(-1.0, 1.0).asin()]);
        if hud != self.hud {
            self.hud = hud;
            self.hud_since = Instant::now();
        }
        if hud == Some(3) && !SCANNED.swap(true, Ordering::Relaxed) {
            std::thread::spawn(find_dispatcher);
        }
        self.hotkeys();
        // B3: the world as it is now, while the book is open and nothing of Elden Ring's covers it: the cover once
        // the mirror opens (the last of these copies is the frame before it), with the camera's direction then.
        if self.drive == Drive::Armed && crate::input::book_open() && hud == Some(3) && self.hud_stable(3, 0.3)
            && crate::game::faded() < 0.05 {
            if let Some(id) = render.capture_screen(COVER_SLOT) {
                textures.set(COVER, id, [1.0, 1.0]);
                self.cover_at = Some(Instant::now());
                self.cover_view = self.view;
            }
        }
        // Menu closed for a while: the scene's pictures go (the game frees them; ours are the last references).
        let menu_gone = self.hud_stable(3, 1.5) || hud.is_none();
        {
            let mut sh = SHARED.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            for t in sh.targets.iter_mut() {
                if menu_gone && t.retired.is_none() {
                    t.retired = Some(now);
                }
            }
            sh.targets.retain(|t| {
                let old = t.retired.is_some_and(|r| now - r > Duration::from_secs(3));
                if old {
                    unsafe { probe::com_release(t.resource) };
                }
                !old
            });
        }
        {
            let mut w = WORLD.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            w.targets.retain(|t| {
                let old = t.retired.is_some_and(|r| now - r > Duration::from_secs(3));
                if old {
                    unsafe { probe::com_release(t.resource) };
                }
                !old
            });
        }
        self.world_search(render, hud);
        self.anim_survey(render, hud);
        let full = picture().filter(|b| b.size[0] > 0 && b.size[1] > 0);
        let face = portrait().filter(|s| s[0] > 0 && s[1] > 0);
        let target = if hud != Some(3) { self.current() } else { None };
        // B1: the portrait shows the still front picture (its own copy), never the turning one.
        let still = match (face, target) {
            (Some(_), Some(_)) => self.still().and_then(|(r, size)| {
                render.copy_texture(PORTRAIT_SLOT, r, self.state, [0, 0, size, size]).map(|id| (id, r, size))
            }),
            _ => None,
        };
        let copied = match (full.is_some() || face.is_some(), target) {
            (true, Some((resource, size))) => {
                // The whole square, every frame (one fixed size: the copy texture is made once); each book picture
                // shows its own part of it (image_strips): the Backpack's category zoom, the Character page's head.
                let id = render.copy_texture(0, resource, self.state, [0, 0, size, size]);
                if id.is_none() {
                    self.say("copy texture not ready yet (made this frame, or the source couldn't be copied)");
                }
                id.map(|id| (id, resource, size))
            }
            (false, _) => {
                self.say("the book's character picture isn't on screen");
                None
            }
            (_, None) => {
                self.say(if hud == Some(3) { "no menu up (the mirror scene only draws in its menu)" }
                         else { "menu up, but no mirror scene picture kept" });
                None
            }
        };
        let names = [MODEL_FULL, crate::pages::MODEL_PORTRAIT];
        match copied {
            Some((id, resource, size)) => {
                let crops = [full.map(|b| crop(self.frame, b.view, b.size)), face.map(|s| crop(self.frame, PORTRAIT_VIEW, s))];
                if let Some((_, r, _)) = still.filter(|_| !self.still_said) {
                    self.still_said = true;
                    tracing::info!("live: the portrait shows the still front picture {r:#x} (set {STILL_SET}), not the \
                                    turning one");
                }
                let ours = [Some(id), still.map(|s| s.0)];
                for (k, name) in names.iter().enumerate() {
                    let (id, size) = match still.filter(|_| k == 1) {
                        Some((still_id, _, still_size)) => (still_id, still_size),
                        None => (id, size),
                    };
                    match crops[k] {
                        Some(_) => {
                            // The software picture is kept to put back (never one of the live copies).
                            if textures.get(name).is_some_and(|t| !ours.contains(&Some(t.0))) {
                                self.soft[k] = textures.get(name);
                            }
                            textures.set(name, id, [size as f32; 2]);
                        }
                        // This picture isn't live (not on the page, or an arrow on its own): its own texture back.
                        None => {
                            if let Some((soft, soft_size)) = self.soft[k].take() {
                                textures.set(name, soft, soft_size);
                            }
                        }
                    }
                }
                *CROPS.lock().unwrap_or_else(|e| e.into_inner()) = crops;
                if !self.shown_once.contains(&resource) {
                    self.shown_once.push(resource);
                    tracing::info!("live: drawing {resource:#x} (set {}) in the book's picture ({size}x{size}, \
                                    crops {crops:?}), state {:#x}", self.set, self.state);
                }
                self.showing = Some(resource);
                self.why_not.clear();
            }
            None => {
                if self.showing.take().is_some() {
                    for (k, name) in names.iter().enumerate() {
                        if let Some((soft, soft_size)) = self.soft[k].take() {
                            textures.set(name, soft, soft_size);
                        }
                    }
                    *CROPS.lock().unwrap_or_else(|e| e.into_inner()) = [None; 2];
                    tracing::info!("live: back to the software picture");
                }
            }
        }
        // B1: the right stick reaches the game only for the Backpack's live picture, never the portrait alone.
        let returning = matches!(self.drive, Drive::Returning(_));
        crate::input::pass_right_stick(self.showing.is_some() && full.is_some() && !returning);
        // A1 / A2: the book's turning of its picture (the slow spin like Wizard101's, or a mouse drag) turns Elden
        // Ring's model: CharaMake turns it by the right stick, so that stick is given to the game while the real one
        // is at rest. The survey then also sees the redrawn picture change.
        let rate = full.filter(|_| self.showing.is_some() && self.turn && !returning).map_or(0.0, |b| b.turn);
        let stick = if rate.abs() < 1e-3 {
            0.0
        } else {
            self.turn_sign * rate.signum() * (self.stick_dead + (1.0 - self.stick_dead)
                * (rate.abs() / self.turn_rate).min(1.0))
        };
        crate::input::inject_right_x(stick, self.fake_pad);
        let camera_x = self.camera_return(full.is_some());
        crate::input::inject_camera_x(camera_x);
        self.last_turn = self.view.map(|v| (v[0], if camera_x != 0.0 { camera_x } else { stick }));
        if stick != 0.0 && !self.turn_said {
            self.turn_said = true;
            tracing::info!("live: turning the mirror's model with right stick x {stick:.2} (book turn {rate:.2} rad/s; \
                            ini live_preview_turn_rate / _stick_dead / _turn_sign tune it, live_preview_turn = 0 \
                            stops it)");
        }
        SHOWING.store(self.showing.is_some(), Ordering::Relaxed);
        FAKE_BREATH.store(self.fake_breath && self.anim.done && self.anim.picks.iter().all(Option::is_none),
                          Ordering::Relaxed);
        // The world behind the book while Elden Ring's menu covers the screen: a live copy of the world scene's own
        // picture, once it is known.
        self.world_live = false;
        // Round 18 A1: also while the mirror closes, until Elden Ring's own screen is back (frame()).
        let settling = matches!(self.drive, Drive::Settling(..));
        if WORLD_LIVE.load(Ordering::Relaxed)
            && ((hud.is_some_and(|h| h != 3) && (self.showing.is_some() || self.drives())) || settling) {
            if let Some((resource, w, h)) = self.search.chosen {
                if let Some(id) = render.copy_texture(WORLD_SLOT, resource, self.world_state, [0, 0, w, h]) {
                    textures.set(LIVE_WORLD, id, [w as f32, h as f32]);
                    self.world_live = true;
                }
                self.world_moving(render, resource);
            }
        } else {
            self.search.menu_grid = None;
            self.search.menu_samples = 0;
        }
    }

    /// Round 18 A1: whether the player really walks under the book-driven mirror (W/A/S/D or the left stick held
    /// for a second: did the position change?). Logged a few times; once it works the mirror may stay up for the
    /// whole book.
    fn walk_check(&mut self) {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        let keys = [b'W', b'A', b'S', b'D'].iter().any(|k| unsafe { GetAsyncKeyState(*k as i32) } < 0);
        let s = crate::input::sticks();
        let moving = walk_under_mirror() && crate::input::game_focused() && (keys || s[0].hypot(s[1]) > 0.5);
        let (Some(pos), true) = (self.player, moving) else {
            self.walk_from = None;
            return;
        };
        let (from, at) = *self.walk_from.get_or_insert((pos, Instant::now()));
        if at.elapsed() < Duration::from_secs(1) {
            return;
        }
        let moved = ((pos[0] - from[0]).powi(2) + (pos[2] - from[2]).powi(2)).sqrt();
        let works = moved > 0.3;
        if self.walk_lines < 6 && (self.walk_works != Some(works) || self.walk_lines == 0) {
            self.walk_lines += 1;
            tracing::info!("live: walk: {} under the mirror menu ({moved:.2} m in a second of {})",
                           if works { "the player walks" } else { "the player did NOT move" },
                           if keys { "W/A/S/D" } else { "left stick" });
        }
        if works && self.walk_works != Some(true) && !self.linger_fixed {
            self.linger = 600.0;
            tracing::info!("live: walking works under the mirror, so it stays up for the whole book");
        }
        self.walk_works = Some(works);
        self.walk_from = Some((pos, Instant::now()));
    }

    /// B3: the right stick x that turns the world camera back to its yaw from before the mirror opened (0: none),
    /// while the book lets go (Returning) or the mirror is up without the Backpack picture. Learns which way the
    /// stick turns the camera from the spin; sets `returned` once the camera is back (or the return gave up).
    fn camera_return(&mut self, backpack: bool) -> f32 {
        // Learning the stick's direction: last frame's stick against how the yaw moved since.
        if let (Some(v), Some((yaw, stick))) = (self.view, self.last_turn) {
            let dy = wrap(v[0] - yaw);
            if stick.abs() > self.stick_dead + 0.05 && dy.abs() > 1e-4 && dy.abs() < 0.5 {
                self.learn += stick.signum() * dy.signum();
                if self.cam_sign.is_none() && self.learn.abs() >= 8.0 {
                    self.cam_sign = Some(self.learn.signum());
                    tracing::info!("live: camera: a positive right stick x turns the camera's yaw {}",
                                   if self.learn > 0.0 { "up" } else { "down" });
                }
            }
        }
        let wanted = match self.drive {
            Drive::Returning(_) => true,
            Drive::Open => !backpack && self.hud.is_some_and(|h| h != 3),
            _ => false,
        };
        let (Some(v), Some(target)) = (self.view, self.saved_view) else {
            self.returned = true;
            return 0.0;
        };
        let error = wrap(target[0] - v[0]);
        self.returned = self.gave_up || error.abs() < RETURN_TOLERANCE;
        if !wanted || self.gave_up {
            self.ret = Return::default();
            return 0.0;
        }
        let r = &mut self.ret;
        let start = *r.start.get_or_insert_with(Instant::now);
        if r.frames == 0 {
            r.start_error = error.abs();
        }
        r.frames += 1;
        if let Some(prev) = r.prev_yaw.replace(v[0]) {
            r.moved += wrap(v[0] - prev).abs();
        }
        r.ok = if error.abs() < RETURN_TOLERANCE { r.ok + 1 } else { 0 };
        if r.ok >= 3 {
            if r.frames > 3 {
                tracing::info!("live: camera back where it was before the mirror (yaw {:.1} deg off, pitch {:.1} deg \
                                off) after {:.0} ms", error.to_degrees(), (target[1] - v[1]).to_degrees(),
                               start.elapsed().as_secs_f32() * 1000.0);
            }
            self.ret = Return::default();
            return 0.0;
        }
        if start.elapsed().as_secs_f32() > RETURN_MAX || (r.frames > 40 && r.moved < 0.005) {
            tracing::info!("live: camera: couldn't turn it back (yaw {:.1} deg off after {:.0} ms, moved {:.1} deg; \
                            no controller? ini live_preview_fake_pad = 1 makes one up); left as it is",
                           error.to_degrees(), start.elapsed().as_secs_f32() * 1000.0, r.moved.to_degrees());
            self.gave_up = true;
            self.returned = true;
            self.ret = Return::default();
            return 0.0;
        }
        if self.cam_sign.is_none() && !r.flipped && r.frames == 20 && error.abs() > r.start_error + 0.02 {
            r.flipped = true;
            self.probe_sign = -self.probe_sign;
            tracing::info!("live: camera: turning the wrong way; the other way now");
        }
        let sign = self.cam_sign.unwrap_or(self.probe_sign);
        let low = (self.stick_dead + 0.08).min(1.0);
        let magnitude = if error.abs() > 0.6 { 1.0 } else { low + (1.0 - low) * error.abs() / 0.6 };
        sign * error.signum() * magnitude.min(1.0)
    }

    /// While Elden Ring's menu is up with a scene: reads every square target back twice, about a second apart, and
    /// picks the ones that changed (ER redraws them: the idle animation) and show the scene's background. Then
    /// watches the shown one and surveys again if it stops changing.
    fn anim_survey(&mut self, render: &mut dyn RenderContext, hud: Option<u8>) {
        let scene = SCENE.load(Ordering::Relaxed);
        let (live, settled): (Vec<(usize, Option<usize>, bool)>, bool) = {
            let sh = SHARED.lock().unwrap_or_else(|e| e.into_inner());
            (sh.targets.iter().filter(|t| t.retired.is_none()).map(|t| (t.resource, t.set, t.by_stack)).collect(),
             sh.last_made.is_some_and(|t| t.elapsed() > Duration::from_millis(1500)))
        };
        let book = picture().is_some();
        let showing = self.showing;
        let a = &mut self.anim;
        if a.scene != scene {
            *a = AnimSurvey { scene, ..Default::default() };
        }
        a.frames += 1;
        if hud == Some(3) || hud.is_none() || live.is_empty() || !settled {
            return;
        }
        if !a.started {
            a.started = true;
            a.runs += 1;
            a.queue = live.iter().map(|t| t.0).collect();
            tracing::info!("live: anim: surveying {} square targets of scene {scene} (book open {book})",
                           a.queue.len());
        }
        // A readback asked for earlier.
        if let Some(p) = a.pending {
            match render.take_readback_of(p) {
                Some(rb) => {
                    a.pending = None;
                    let Some((grid, corner)) = anim_sample(&rb) else { return };
                    if !a.done {
                        if !a.second {
                            a.first.push((p, grid));
                        } else if let Some((_, before)) = a.first.iter().find(|f| f.0 == p) {
                            let change = anim_diff(before, &grid);
                            let background = (0..3).all(|c| (corner[c] - ANIM_BACKGROUND[c]).abs()
                                                         <= ANIM_BACKGROUND_TOLERANCE);
                            let (set, by_stack) = live.iter().find(|t| t.0 == p).map(|t| (t.1, t.2))
                                .unwrap_or((None, false));
                            tracing::info!("live: anim {p:#x} (set {set:?}, stack {by_stack}): change {change:.2}, \
                                            corners {:.0},{:.0},{:.0}{}", corner[0], corner[1], corner[2],
                                           if background { " (scene background)" } else { "" });
                            a.results.push((p, set, by_stack, change, background));
                        }
                    } else {
                        // Watching the shown picture.
                        if let Some((r, before)) = a.watch_last.replace((p, grid.clone())) {
                            if r == p {
                                let change = anim_diff(&before, &grid);
                                if change > ANIM_CHANGED {
                                    a.still = 0;
                                    a.said_still = false;
                                } else {
                                    a.still += 1;
                                }
                                if a.still == 4 && !a.said_still {
                                    a.said_still = true;
                                    tracing::info!("live: anim: the shown picture {p:#x} stopped changing (book open \
                                                    {book}){}", if a.runs < 3 { "; surveying again" } else { "" });
                                    if a.runs < 3 {
                                        let (scene, runs) = (a.scene, a.runs);
                                        *a = AnimSurvey { scene, runs, ..Default::default() };
                                    }
                                }
                            }
                        }
                    }
                }
                None if a.frames > 120 => a.pending = None, // never came: ask again
                None => {}
            }
            return;
        }
        if !a.done {
            if a.queue.is_empty() {
                if !a.second {
                    a.second = true;
                    a.queue = a.first.iter().map(|f| f.0).collect();
                    return;
                }
                // The verdict: changing targets with the scene's background, equipped (sets 0, 2) and bare (1, 3).
                let live_pictures: Vec<_> = a.results.iter().filter(|r| r.3 > ANIM_CHANGED && r.4).collect();
                let pick = |bare: bool| live_pictures.iter()
                    .filter(|r| r.1.is_some_and(|s| (s % 2 == 1) == bare))
                    .max_by(|x, y| (x.2, x.3).partial_cmp(&(y.2, y.3)).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|r| r.0);
                a.picks = [pick(false), pick(true)];
                if a.picks.iter().all(Option::is_none) {
                    // Changing pictures without a set: shown for both.
                    a.picks = [live_pictures.first().map(|r| r.0); 2];
                }
                a.done = true;
                let changing = a.results.iter().filter(|r| r.3 > ANIM_CHANGED).count();
                tracing::info!("live: anim: {changing} of {} targets changed, {} of them look final; showing \
                                equipped {:?}, bare {:?}{}", a.results.len(), live_pictures.len(), a.picks[0],
                               a.picks[1], if live_pictures.is_empty() {
                                   " (none changed: the set picture stays; ini live_preview_fake_breath = 1 adds a \
                                    drawn breathing)" } else { "" });
                return;
            }
            if a.frames >= 3 && render.request_readback(a.queue[0], self.state) {
                a.pending = Some(a.queue.remove(0));
                a.frames = 0;
            }
            return;
        }
        // Watch the shown picture about every 1.5 s.
        if let Some(r) = showing.filter(|_| a.frames >= 90) {
            if render.request_readback(r, self.state) {
                a.pending = Some(r);
                a.frames = 0;
            }
        }
    }

    fn drives(&self) -> bool {
        matches!(self.drive, Drive::Opening(_) | Drive::Open | Drive::Closing(..) | Drive::Returning(_))
    }

    /// In normal play (Elden Ring's HUD up, no fade), finds which world candidate is the world scene's picture.
    fn world_search(&mut self, render: &mut dyn RenderContext, hud: Option<u8>) {
        let (batch, live): (u32, Vec<(usize, u32, u32, Vec<usize>)>) = {
            let w = WORLD.lock().unwrap_or_else(|e| e.into_inner());
            (w.batch, w.targets.iter().filter(|t| t.retired.is_none() && t.batch == w.batch)
                .map(|t| (t.resource, t.width, t.height, t.stack.clone())).collect())
        };
        let plain = hud == Some(3) && self.hud_stable(3, 2.0) && crate::game::faded() < 0.05;
        let se = &mut self.search;
        if se.batch != batch {
            // A new batch: search again (the old choice went with its batch).
            *se = WorldSearch { batch, queue: live.iter().map(|t| t.0).collect(), ..Default::default() };
        }
        if se.chosen.is_some_and(|c| !live.iter().any(|t| t.0 == c.0)) {
            se.chosen = None;
        }
        // A readback asked for earlier.
        if let Some(p) = se.pending.filter(|_| se.chosen.is_none()) {
            if let Some(rb) = render.take_readback_of(p) {
                se.pending = None;
                let grid = brightness_grid(&rb);
                if p == hudhook::READBACK_SCREEN {
                    if grid.is_none() {
                        tracing::info!("live: world: back buffer format {} not compared; no live world", rb.format);
                        se.gave_up = Some(Instant::now());
                        se.queue.clear();
                    }
                    se.screen = grid;
                } else if let (Some(g), Some(screen)) = (grid, se.screen.take()) {
                    let (r, diff) = compare(&g, &screen);
                    let stack = live.iter().find(|t| t.0 == p).map(|t| t.3.clone()).unwrap_or_default();
                    tracing::info!("live: world candidate {p:#x}: correlation {r:.3}, mean difference {diff:.1} \
                                    (stack {stack:x?})");
                    se.scores.push((p, r, diff));
                }
            } else if se.frames > 120 {
                se.pending = None; // never came (another readback went first): ask again
            }
        }
        se.frames += 1;
        if se.chosen.is_some() || se.pending.is_some() || !plain || se.frames < 6 {
            return;
        }
        if se.gave_up.is_some_and(|t| t.elapsed() < Duration::from_secs(60)) {
            return;
        }
        if se.queue.is_empty() {
            if se.scores.is_empty() {
                // Nothing compared yet (or a retry after giving up): every candidate of this batch.
                se.queue = live.iter().map(|t| t.0).collect();
                se.gave_up = None;
                return;
            }
            let best = se.scores.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1));
            match best.filter(|b| b.1 >= WORLD_MATCH) {
                Some((resource, r, diff)) => {
                    if let Some(t) = live.iter().find(|t| t.0 == resource) {
                        tracing::info!("live: world picture is {resource:#x} {}x{} (correlation {r:.3}, mean \
                                        difference {diff:.1}, stack {:x?}); it stands behind the book over Elden \
                                        Ring's menus", t.1, t.2, t.3);
                        se.chosen = Some((t.0, t.1, t.2));
                    }
                }
                None => {
                    tracing::info!("live: world: no candidate matched the screen (best {:?}); the screenshot stays \
                                    behind the book; trying again in 60 s", best);
                    se.gave_up = Some(Instant::now());
                }
            }
            se.scores.clear();
            return;
        }
        // The back buffer, then the next candidate, a few frames apart.
        let next = if se.screen.is_none() { hudhook::READBACK_SCREEN } else { se.queue[0] };
        if render.request_readback(next, if next == hudhook::READBACK_SCREEN { 0 } else { self.world_state }) {
            if next != hudhook::READBACK_SCREEN {
                se.queue.remove(0);
            }
            se.pending = Some(next);
            se.frames = 0;
        }
    }

    /// While a menu is up: reads the world picture back about once a second (10 times at most per menu) and logs
    /// whether it changes, i.e. whether Elden Ring still draws the world under its menu.
    fn world_moving(&mut self, render: &mut dyn RenderContext, resource: usize) {
        let se = &mut self.search;
        se.frames += 1;
        if se.pending == Some(resource) {
            if let Some(rb) = render.take_readback_of(resource) {
                se.pending = None;
                if let Some(g) = brightness_grid(&rb) {
                    if let Some(prev) = se.menu_grid.replace(g.clone()) {
                        let (_, diff) = compare(&prev, &g);
                        let moving = diff > 0.05;
                        if se.menu_said != Some(moving) {
                            tracing::info!("live: world under the menu {} (mean difference {diff:.2} between two \
                                            looks a second apart)",
                                           if moving { "is drawn and moving" } else { "did not change" });
                            se.menu_said = Some(moving);
                        }
                    }
                }
            } else if se.frames > 240 {
                se.pending = None;
            }
            return;
        }
        if se.pending.is_none() && se.menu_samples < 10 && se.frames >= 120
            && render.request_readback(resource, self.world_state) {
            se.pending = Some(resource);
            se.menu_samples += 1;
            se.frames = 0;
        }
    }

    fn hotkeys(&mut self) {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_F10, VK_F9};
        if !crate::input::game_focused() {
            return;
        }
        let down = unsafe { [GetAsyncKeyState(VK_F9 as i32) < 0, GetAsyncKeyState(VK_F10 as i32) < 0] };
        if down[0] && !self.keys[0] {
            self.set = if self.set == self.sets[1] { self.sets[0] } else { self.sets[1] };
            tracing::info!("live: F9, showing set {} ({})", self.set,
                           if self.set == self.sets[1] { "bare" } else { "equipped" });
        }
        if down[1] && !self.keys[1] {
            self.drive = match self.drive {
                Drive::Off | Drive::Failed => Drive::Armed,
                Drive::Open | Drive::Opening(_) | Drive::Returning(_) => {
                    self.close_menu(FORCE_CLOSE_MENU);
                    Drive::Closing(Instant::now(), false)
                }
                d @ (Drive::Closing(..) | Drive::Settling(..)) => d,
                Drive::Armed => Drive::Off,
            };
            tracing::info!("live: F10, book-driven mirror {:?}", self.drive);
        }
        self.keys = down;
    }

    /// Each frame after the HUD (UI thread): the book-driven mirror menu, and the world copy over Elden Ring's
    /// menu while the book drives it.
    pub fn frame(&mut self, ui: &Ui, textures: &Textures) {
        let shown = wanted();
        let now = Instant::now();
        if shown {
            self.last_shown = now;
            self.wanted_since.get_or_insert(now);
        } else {
            self.wanted_since = None;
        }
        // B3: the mirror opens over a fresh copy of the world (the frame before it), or after a second without one.
        let cover_fresh = self.cover_at.is_some_and(|t| t.elapsed() < Duration::from_millis(200));
        let cover_wait = self.wanted_since.is_some_and(|t| t.elapsed() > Duration::from_secs(1));
        let keep = BOOK_KEEP.load(Ordering::Relaxed);
        self.drive = match self.drive {
            Drive::Armed if shown && self.hud_stable(3, 1.0) && crate::game::faded() < 0.05
                && (cover_fresh || cover_wait)
                && self.last_attempt.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) => {
                let dispatch = DISPATCH.load(Ordering::Relaxed);
                if dispatch == 0 {
                    tracing::info!("live: book-driven mirror off: the EzState dispatcher wasn't found");
                    Drive::Failed
                } else {
                    self.last_attempt = Some(now);
                    // The camera's direction in the cover: turned back to before the mirror closes (B3).
                    self.saved_view = if cover_fresh { self.cover_view } else { self.view };
                    self.gave_up = false;
                    self.ret = Return::default();
                    tracing::info!("live: opening the cosmetics mirror (OpenCharaMakeMenu 1) for the book{}",
                                   if cover_fresh { "" } else { " (no fresh world copy for the cover)" });
                    unsafe { ezstate_event(dispatch, &[OPEN_CHARA_MAKE_MENU, 1]) };
                    tracing::info!("live: OpenCharaMakeMenu returned");
                    Drive::Opening(now)
                }
            }
            Drive::Opening(t) if self.hud.is_some_and(|h| h != 3) => {
                tracing::info!("live: Elden Ring's menu is up {:.0} ms after the open call", t.elapsed().as_secs_f32() * 1000.0);
                Drive::Open
            }
            Drive::Opening(t) if t.elapsed() > Duration::from_secs(5) => {
                tracing::info!("live: no menu 5 s after OpenCharaMakeMenu; book-driven mirror off");
                Drive::Failed
            }
            Drive::Open if self.hud == Some(3) => {
                tracing::info!("live: Elden Ring's menu closed by itself; book-driven mirror off (F10 arms it again)");
                Drive::Settling(now, false)
            }
            // B3: the mirror stays up while the book is open on another page for a while (quick tab flips don't
            // close and reopen it); it lets go when the book closes, goes to the map page or the time is up.
            // The map page needs Elden Ring's map menu: the mirror goes at once (no turning back first).
            Drive::Open if !shown && !keep && crate::input::book_open() => {
                tracing::info!("live: the book went to the map page; closing the mirror (ForceCloseMenu)");
                self.close_menu(FORCE_CLOSE_MENU);
                Drive::Closing(now, false)
            }
            Drive::Open if !shown && !(keep && self.last_shown.elapsed().as_secs_f32() < self.linger) => {
                tracing::info!("live: the book's picture went ({}); turning the camera back, then closing the mirror",
                               if keep { "another page for a while" } else { "book closed" });
                Drive::Returning(now)
            }
            Drive::Returning(_) if self.hud == Some(3) => {
                tracing::info!("live: Elden Ring's menu closed by itself while the camera turned back");
                Drive::Settling(now, true)
            }
            Drive::Returning(_) if shown && keep => Drive::Open,
            Drive::Returning(t) if self.returned || t.elapsed().as_secs_f32() > RETURN_MAX + 0.5 => {
                tracing::info!("live: closing the mirror (ForceCloseMenu) {:.0} ms after the book let go",
                               t.elapsed().as_secs_f32() * 1000.0);
                self.close_menu(FORCE_CLOSE_MENU);
                Drive::Closing(now, false)
            }
            Drive::Closing(t, _) if self.hud == Some(3) => {
                tracing::info!("live: mirror closed {:.0} ms after the close call", t.elapsed().as_secs_f32() * 1000.0);
                Drive::Settling(now, true)
            }
            Drive::Closing(t, false) if t.elapsed() > Duration::from_millis(2500) => {
                tracing::info!("live: still open 2.5 s after ForceCloseMenu; trying CloseMenu");
                self.close_menu(CLOSE_MENU);
                Drive::Closing(t, true)
            }
            Drive::Closing(t, true) if t.elapsed() > Duration::from_secs(6) => {
                tracing::info!("live: the mirror didn't close; close it in Elden Ring (B). Book-driven mirror off");
                Drive::Failed
            }
            // Round 18 A1: with the live world copy behind, it stands until Elden Ring's screen is back (its fade
            // done), then goes at once: what shows is the game as it is, never an old picture fading.
            Drive::Settling(t, rearm) if self.search.chosen.is_some()
                && ((self.hud == Some(3) && crate::game::faded() < 0.05) || t.elapsed() > Duration::from_millis(1500)) => {
                tracing::info!("live: Elden Ring's screen back {:.0} ms after the mirror closed",
                               t.elapsed().as_secs_f32() * 1000.0);
                self.last_attempt = Some(now);
                if rearm { Drive::Armed } else { Drive::Off }
            }
            Drive::Settling(t, rearm) if self.search.chosen.is_none()
                && t.elapsed().as_secs_f32() >= self.close_hold + self.close_fade => {
                self.last_attempt = Some(now);
                if rearm { Drive::Armed } else { Drive::Off }
            }
            d => d,
        };
        // Elden Ring's mirror menu stays hidden behind the world: live once the world scene's picture is known (A3),
        // else the frame before the mirror opened (B3), else the teleport's older copy. After the menu closed it stays
        // a moment (Elden Ring's menu going) and fades into the game.
        MIRROR_UP.store(matches!(self.drive, Drive::Open) && self.hud != Some(3), Ordering::Relaxed);
        let menu_up = self.hud != Some(3);
        let alpha = match self.drive {
            Drive::Settling(..) if self.world_live => 1.0,
            Drive::Settling(t, _) => {
                1.0 - ((t.elapsed().as_secs_f32() - self.close_hold) / self.close_fade.max(1e-3)).clamp(0.0, 1.0)
            }
            Drive::Opening(_) if !menu_up => if self.cover_at.is_some() { 1.0 } else { 0.0 },
            _ if self.drives() && menu_up => 1.0,
            _ if self.world_live && self.showing.is_some() && menu_up => 1.0,
            _ => 0.0,
        };
        if alpha > 0.0 {
            let size = ui.io().display_size;
            let draw = ui.get_background_draw_list();
            let world = if self.world_live { textures.get(LIVE_WORLD) }
                else { textures.get(COVER).filter(|_| self.cover_at.is_some()).or_else(|| textures.get(TELEPORT_WORLD)) };
            let a = (alpha * 255.0).round() as u8;
            match world {
                Some((world, _)) => draw.add_image(world, [0.0, 0.0], size)
                    .col(ImColor32::from_rgba(255, 255, 255, a)).build(),
                None => draw.add_rect([0.0, 0.0], size, ImColor32::from_rgba(12, 9, 6, a)).filled(true).build(),
            }
        }
    }

    fn close_menu(&mut self, event: i32) {
        let dispatch = DISPATCH.load(Ordering::Relaxed);
        if dispatch != 0 {
            tracing::info!("live: EzState event {event}");
            unsafe { ezstate_event(dispatch, &[event]) };
        }
    }
}

/// The dispatcher, by TGA's public pattern (exactly one match, in an executable section).
fn find_dispatcher() {
    let regions: Vec<(usize, usize)> = probe::sections().into_iter().filter(|s| s.2).map(|s| (s.0, s.1)).collect();
    let hits = probe::scan_all(&regions, &probe::parse_pattern(DISPATCH_PATTERN));
    if hits.len() == 1 {
        let at = hits[0] - DISPATCH_BACK;
        DISPATCH.store(at, Ordering::Relaxed);
        tracing::info!("live: EzState talk-event dispatcher at rva {:#x}", probe::rva(at));
    } else {
        tracing::info!("live: EzState dispatcher pattern matched {} times; the book can't open the mirror", hits.len());
    }
}

// The fake talk event TGA builds (tga/ezstate.h): an EzStateTalkEvent with a zeroed NpcTalkIns and menu job, and
// an external event whose arguments are (value, type) pairs, argument 0 being the event id.

#[repr(C)]
struct Arg {
    value: u64,
    kind: i32,
    _pad: i32,
}

#[repr(C)]
struct ExtEvent {
    vtable: *const usize,
    args: *mut Arg,
    count: i32,
}

#[repr(C)]
struct TalkEvent {
    vtable: usize,
    unk08: *mut u64,
    npc_talk_ins: *mut u64,
    unk18: u32,
}

unsafe extern "system" fn ev_stub(_: *mut ExtEvent) {}
unsafe extern "system" fn ev_id(this: *mut ExtEvent) -> i32 {
    if (*this).count > 0 { (*(*this).args).value as i32 } else { 0 }
}
unsafe extern "system" fn ev_count(this: *mut ExtEvent) -> i32 {
    (*this).count
}
unsafe extern "system" fn ev_arg(this: *mut ExtEvent, index: i32) -> *mut Arg {
    let i = index.clamp(0, ((*this).count - 1).max(0));
    (*this).args.add(i as usize)
}

/// A zeroed, 8-aligned block that is never freed (the menu may keep pointers into the fake talk instance).
fn leaked(bytes: usize) -> *mut u64 {
    Box::leak(vec![0u64; bytes / 8].into_boxed_slice()).as_mut_ptr()
}

unsafe fn ezstate_event(dispatch: usize, values: &[i32]) {
    static VTABLE: std::sync::OnceLock<[usize; 5]> = std::sync::OnceLock::new();
    let vtable = VTABLE.get_or_init(|| [ev_stub as *const () as usize, ev_stub as *const () as usize, ev_id as *const () as usize,
                                        ev_count as *const () as usize,
                                        ev_arg as *const () as usize]);
    let handle = u64::MAX; // no ChrIns
    let unk08 = leaked(0x100);
    *unk08.add(0x30 / 8) = handle;
    let talk_ins = leaked(0x200);
    *talk_ins.add(0x40 / 8) = handle;
    let job = leaked(0x100);
    *job.add(0x18 / 8) = talk_ins as u64;
    *talk_ins.add(0x98 / 8) = job as u64;
    let mut args: Vec<Arg> = values.iter().map(|v| Arg { value: *v as u32 as u64, kind: 2, _pad: 0 }).collect();
    let mut ext = ExtEvent { vtable: vtable.as_ptr(), args: args.as_mut_ptr(), count: args.len() as i32 };
    let mut talk = TalkEvent { vtable: 0, unk08, npc_talk_ins: talk_ins, unk18: 0 };
    let f: unsafe extern "system" fn(*mut TalkEvent, *mut ExtEvent) = std::mem::transmute(dispatch);
    f(&mut talk, &mut ext);
}
