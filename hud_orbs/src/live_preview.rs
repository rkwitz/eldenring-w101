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
/// The book's full character picture this frame (its size in pixels, when it was drawn): book.rs reports it.
static PICTURE: Mutex<Option<([u32; 2], Instant)>> = Mutex::new(None);

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// book.rs: the full character picture is on the page at this size this frame.
pub fn note_box(size: [u32; 2]) {
    if enabled() {
        *PICTURE.lock().unwrap_or_else(|e| e.into_inner()) = Some((size, Instant::now()));
    }
}

fn picture() -> Option<[u32; 2]> {
    PICTURE.lock().unwrap_or_else(|e| e.into_inner())
        .filter(|p| p.1.elapsed() < Duration::from_millis(300)).map(|p| p.0)
}

/// probe.rs's creation hooks: a 2D render target or depth texture the game just made.
pub fn on_created(width: u64, height: u32, format: u32, flags: u32, resource: usize) {
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
    }
    sh.last_made = Some(now);
    sh.burst += 1;
    // The final picture by its stack; if no stack matches (another game version), the first square RGBA8 target
    // of the scene (run 2: also the final picture).
    let keep = by_stack || sh.burst == 1;
    if !keep || sh.targets.len() >= 16 {
        return;
    }
    unsafe { probe::com_add_ref(resource) };
    sh.targets.push(Target { resource, size: height, set, by_stack, retired: None });
    if KEPT_LINES.fetch_add(1, Ordering::Relaxed) < 64 {
        tracing::info!("live: kept {width}x{height} fmt 28 {resource:#x} (set {set:?}, {})",
                       if by_stack { "stack matched" } else { "first of its scene, stack not matched" });
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Drive {
    Off,
    Armed,
    Opening(Instant),
    Open,
    Closing(Instant, bool),
    Failed,
}

pub struct LivePreview {
    set: usize,
    state: u32,
    drive: Drive,
    hud: Option<u8>,
    hud_since: Instant,
    /// The book's own picture (the software renderer's texture) while the live one stands in for it.
    soft: Option<(hudhook::imgui::TextureId, [f32; 2])>,
    showing: Option<usize>,
    shown_once: Vec<usize>,
    last_attempt: Option<Instant>,
    keys: [bool; 2],
    why_not: String,
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
        let set = get("live_preview_slot").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0).min(3);
        let state = get("live_preview_state")
            .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
            .unwrap_or(DEFAULT_STATE);
        let drive = get("live_preview_drive").as_deref() == Some("1");
        ENABLED.store(true, Ordering::Relaxed);
        tracing::info!("live: live preview on (set {set}, assumed state {state:#x}, drive {drive}); F9 cycles the \
                        set, F10 arms / disarms the book-driven mirror");
        Some(LivePreview {
            set,
            state,
            drive: if drive { Drive::Armed } else { Drive::Off },
            hud: None,
            hud_since: Instant::now(),
            soft: None,
            showing: None,
            shown_once: Vec::new(),
            last_attempt: None,
            keys: [false; 2],
            why_not: String::new(),
        })
    }

    fn hud_stable(&self, state: u8, seconds: f32) -> bool {
        self.hud == Some(state) && self.hud_since.elapsed().as_secs_f32() >= seconds
    }

    /// The book may stay open with Elden Ring's HUD hidden: the book drives the mirror menu, or Elden Ring's own
    /// mirror scene is up (phase 1).
    pub fn holds_book(&self) -> bool {
        matches!(self.drive, Drive::Opening(_) | Drive::Open | Drive::Closing(..))
            || (self.hud == Some(0) && self.current().is_some())
    }

    /// The target to show: the configured set's final picture (by stack), else the first one kept.
    fn current(&self) -> Option<(usize, u32)> {
        let sh = SHARED.lock().unwrap_or_else(|e| e.into_inner());
        let live: Vec<&Target> = sh.targets.iter().filter(|t| t.retired.is_none()).collect();
        live.iter().find(|t| t.by_stack && t.set == Some(self.set))
            .or_else(|| live.iter().filter(|t| t.by_stack).nth(self.set))
            .or_else(|| live.first())
            .map(|t| (t.resource, t.size))
    }

    fn say(&mut self, why: &str) {
        if self.why_not != why {
            tracing::info!("live: {why}");
            self.why_not = why.to_string();
        }
    }

    /// Before the frame is drawn (render thread): hotkeys, target lifetimes, and the copy into the book's picture.
    pub fn before_render(&mut self, render: &mut dyn RenderContext, textures: &mut Textures) {
        let hud = crate::game::hud_state();
        if hud != self.hud {
            self.hud = hud;
            self.hud_since = Instant::now();
        }
        if hud == Some(3) && !SCANNED.swap(true, Ordering::Relaxed) {
            std::thread::spawn(find_dispatcher);
        }
        self.hotkeys();
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
        let box_size = picture();
        let target = if hud != Some(3) { self.current() } else { None };
        let copied = match (box_size, target) {
            (Some([bw, bh]), Some((resource, size))) if bw > 0 && bh > 0 => {
                // The middle of the square picture, cut to the book picture's shape.
                let aspect = bw as f32 / bh as f32;
                let region = if aspect <= 1.0 {
                    let w = ((size as f32 * aspect).round() as u32).clamp(8, size);
                    [(size - w) / 2, 0, (size - w) / 2 + w, size]
                } else {
                    let h = ((size as f32 / aspect).round() as u32).clamp(8, size);
                    [0, (size - h) / 2, size, (size - h) / 2 + h]
                };
                let id = render.copy_texture(0, resource, self.state, region);
                if id.is_none() {
                    self.say("copy texture not ready yet (made this frame, or the source couldn't be copied)");
                }
                id.map(|id| (id, resource, region))
            }
            (None, _) => {
                self.say("the book's character picture isn't on screen");
                None
            }
            (_, None) => {
                self.say(if hud == Some(3) { "no menu up (the mirror scene only draws in its menu)" }
                         else { "menu up, but no mirror scene picture kept" });
                None
            }
            _ => None,
        };
        match copied {
            Some((id, resource, region)) => {
                if textures.get(MODEL_FULL).is_some_and(|t| t.0 != id) {
                    self.soft = textures.get(MODEL_FULL);
                }
                textures.set(MODEL_FULL, id, [(region[2] - region[0]) as f32, (region[3] - region[1]) as f32]);
                if !self.shown_once.contains(&resource) {
                    self.shown_once.push(resource);
                    tracing::info!("live: drawing {resource:#x} (set {}) in the book's picture, region {region:?}, \
                                    state {:#x}", self.set, self.state);
                }
                self.showing = Some(resource);
                self.why_not.clear();
            }
            None => {
                if self.showing.take().is_some() {
                    if let Some((id, size)) = self.soft.take() {
                        textures.set(MODEL_FULL, id, size);
                    }
                    tracing::info!("live: back to the software picture");
                }
            }
        }
        crate::input::pass_right_stick(self.showing.is_some());
    }

    fn hotkeys(&mut self) {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_F10, VK_F9};
        if !crate::input::game_focused() {
            return;
        }
        let down = unsafe { [GetAsyncKeyState(VK_F9 as i32) < 0, GetAsyncKeyState(VK_F10 as i32) < 0] };
        if down[0] && !self.keys[0] {
            self.set = (self.set + 1) % 4;
            tracing::info!("live: F9, showing set {}", self.set);
        }
        if down[1] && !self.keys[1] {
            self.drive = match self.drive {
                Drive::Off | Drive::Failed => Drive::Armed,
                Drive::Open | Drive::Opening(_) => {
                    self.close_menu(FORCE_CLOSE_MENU);
                    Drive::Closing(Instant::now(), false)
                }
                d @ Drive::Closing(..) => d,
                Drive::Armed => Drive::Off,
            };
            tracing::info!("live: F10, book-driven mirror {:?}", self.drive);
        }
        self.keys = down;
    }

    /// Each frame after the HUD (UI thread): the book-driven mirror menu, and the world copy over Elden Ring's
    /// menu while the book drives it.
    pub fn frame(&mut self, ui: &Ui, textures: &Textures) {
        let shown = picture().is_some();
        let now = Instant::now();
        self.drive = match self.drive {
            Drive::Armed if shown && self.hud_stable(3, 1.0) && crate::game::faded() < 0.05
                && self.last_attempt.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) => {
                let dispatch = DISPATCH.load(Ordering::Relaxed);
                if dispatch == 0 {
                    tracing::info!("live: book-driven mirror off: the EzState dispatcher wasn't found");
                    Drive::Failed
                } else {
                    self.last_attempt = Some(now);
                    tracing::info!("live: opening the cosmetics mirror (OpenCharaMakeMenu 1) for the book");
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
                Drive::Off
            }
            Drive::Open if !shown => {
                tracing::info!("live: the book's picture went; closing the mirror (ForceCloseMenu)");
                self.close_menu(FORCE_CLOSE_MENU);
                Drive::Closing(now, false)
            }
            Drive::Closing(t, _) if self.hud == Some(3) => {
                tracing::info!("live: mirror closed {:.0} ms after the close call", t.elapsed().as_secs_f32() * 1000.0);
                self.last_attempt = Some(now);
                Drive::Armed
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
            d => d,
        };
        if matches!(self.drive, Drive::Opening(_) | Drive::Open | Drive::Closing(..)) && self.hud != Some(3) {
            // Elden Ring's mirror menu stays hidden behind the world as it was before the book opened.
            let size = ui.io().display_size;
            let draw = ui.get_background_draw_list();
            match textures.get(TELEPORT_WORLD) {
                Some((world, _)) => draw.add_image(world, [0.0, 0.0], size).build(),
                None => draw.add_rect([0.0, 0.0], size, ImColor32::from_rgba(12, 9, 6, 255)).filled(true).build(),
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
