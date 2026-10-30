//! Scene probe (ini `scene_probe = 1`, off by default): logs, with a "probe:" prefix, how Elden Ring's mirror
//! (Clouded Mirror Stand / CharaMake) preview scene works, for a second character view in the spellbook.
//!
//! Everything is read-only and every read goes through `game::read` (ReadProcessMemory on our own process, which
//! fails cleanly on a bad pointer), except the opt-in nudge (`scene_probe_nudge = 1`): after the first menu that
//! stayed open for more than five seconds without a loading fade or the player moving closes, MenuOffscrRendParam row 0's camera distance is raised by 50%, so
//! the next mirror visit shows whether that param drives the preview camera.
//!
//! What it logs:
//! - menu state: CSFeMan's HUD state changes (a menu opening or closing) and, on each edge, which pointer slots
//!   of the menu, camera, face and graphics singletons changed (with the RTTI class names of what they point to);
//! - cameras: CSCamera's four perspective cameras and FieldArea's GameRend cameras, once a second while a menu is
//!   up: position, FOV, distance to the player and how far off the player the camera looks;
//! - characters: ChrIns added or removed while the menu is up, and whether the main player changed or moved;
//! - render targets: every 2D render-target / depth texture the game creates through ID3D12Device (committed and
//!   placed resources, all versions), with size, format, flags and the game's return addresses (as RVAs). Those made
//!   before gameplay are summarised; later ones are logged one by one (the mirror's would show up when it opens);
//!   RTVs made while a menu is opening are logged too;
//! - EROverlay's public 1.16 draw-queue / UI-composite byte patterns, and RTTI classes whose names suggest a menu
//!   model, face or offscreen scene, with their vtable RVAs (background thread, once);
//! - MenuOffscrRendParam rows (camera look-at point, distance, rotation, FOV, Gparam).

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crate::game::{self, read, read_bytes, Game};

/// Probe lines per game session; past this the probe goes quiet so the log can't grow without bound.
const LINE_BUDGET: u32 = 6000;
static LINES: AtomicU32 = AtomicU32::new(0);

macro_rules! plog {
    ($($arg:tt)*) => {
        if crate::probe::LINES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < crate::probe::LINE_BUDGET {
            tracing::info!("probe: {}", format!($($arg)*));
        }
    };
}

/// eldenring.exe's base and image size.
fn image() -> (usize, usize) {
    static IMAGE: OnceLock<(usize, usize)> = OnceLock::new();
    *IMAGE.get_or_init(|| unsafe {
        let base = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) as usize;
        let pe = base + *((base + 0x3C) as *const u32) as usize;
        let size = *((pe + 24 + 56) as *const u32) as usize; // OptionalHeader.SizeOfImage
        (base, size)
    })
}

fn in_image(address: usize) -> bool {
    let (base, size) = image();
    address >= base && address < base + size
}

fn rva(address: usize) -> usize {
    address.wrapping_sub(image().0)
}

/// The RTTI class name of the object at `obj` ("CS::CSMenuManImp"), when its first qword is an eldenring.exe
/// vtable with a valid complete object locator. Cached by vtable.
pub fn rtti(obj: usize) -> Option<String> {
    let vtable = read::<usize>(obj)?;
    vtable_name(vtable)
}

fn vtable_name(vtable: usize) -> Option<String> {
    if !in_image(vtable) {
        return None;
    }
    static CACHE: OnceLock<Mutex<HashMap<usize, Option<String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(name) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&vtable) {
        return name.clone();
    }
    let name = (|| {
        let col = read::<usize>(vtable - 8).filter(|c| in_image(*c))?;
        if read::<u32>(col)? != 1 {
            return None;
        }
        let td = image().0 + read::<u32>(col + 12)? as usize;
        let bytes = read_bytes(td + 16, 160).or_else(|| read_bytes(td + 16, 64))?;
        let end = bytes.iter().position(|b| *b == 0)?;
        demangle(std::str::from_utf8(&bytes[..end]).ok()?)
    })();
    cache.lock().unwrap_or_else(|e| e.into_inner()).insert(vtable, name.clone());
    name
}

/// ".?AVCSMenuManImp@CS@@" -> "CS::CSMenuManImp"; templates and odd names are kept as they are.
fn demangle(raw: &str) -> Option<String> {
    let body = raw.strip_prefix(".?AV").or_else(|| raw.strip_prefix(".?AU"))?;
    if body.starts_with("?$") {
        return Some(body.to_string());
    }
    let body = body.strip_suffix("@@").unwrap_or(body);
    let mut parts: Vec<&str> = body.split('@').filter(|p| !p.is_empty()).collect();
    parts.reverse();
    Some(parts.join("::"))
}

/// A pointer value worth naming: an address in user space that isn't in the executable image.
fn heap_like(v: usize) -> bool {
    (0x10000..0x7FFF_FFFF_FFFF).contains(&v) && v % 8 == 0
}

// ---------------------------------------------------------------------------------------------------------------
// D3D12 resource creation hooks
// ---------------------------------------------------------------------------------------------------------------

struct Rec {
    kind: &'static str,
    width: u64,
    height: u32,
    depth: u16,
    mips: u16,
    format: u32,
    samples: u32,
    flags: u32,
    resource: usize,
    callers: Vec<usize>,
    at: f32,
}

static RECORDS: Mutex<Vec<Rec>> = Mutex::new(Vec::new());
/// Set once the player is in the world: later render targets are logged as they come.
static GAMEPLAY: AtomicBool = AtomicBool::new(false);
/// Set while a menu is opening (the first seconds after the HUD went away): RTV creations are logged then.
static ARMED: AtomicBool = AtomicBool::new(false);
/// Render targets logged one by one during gameplay, and RTVs logged in the current menu (both capped: a game that
/// made them every frame would otherwise flood the log).
static RT_LOGGED: AtomicU32 = AtomicU32::new(0);
static RTV_LOGGED: AtomicU32 = AtomicU32::new(0);
static TRAMPOLINES: [AtomicUsize; 8] = [const { AtomicUsize::new(0) }; 8];
static START: OnceLock<Instant> = OnceLock::new();

fn since_start() -> f32 {
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// Return addresses inside eldenring.exe (as RVAs) of the current call, nearest first.
fn callers() -> Vec<usize> {
    let mut frames = [std::ptr::null_mut::<c_void>(); 16];
    let n = unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace(
            1, frames.len() as u32, frames.as_mut_ptr(), std::ptr::null_mut())
    } as usize;
    frames[..n.min(16)].iter().map(|f| *f as usize).filter(|f| in_image(*f)).take(5).map(rva).collect()
}

fn fmt_callers(c: &[usize]) -> String {
    c.iter().map(|r| format!("{r:#x}")).collect::<Vec<_>>().join(",")
}

/// After a successful create: records a 2D texture that can be a render target or depth buffer.
unsafe fn note(kind: &'static str, desc: usize, ppv: usize) {
    // D3D12_RESOURCE_DESC (and DESC1's same prefix): Dimension u32 @0, Width u64 @16, Height u32 @24,
    // DepthOrArraySize u16 @28, MipLevels u16 @30, Format u32 @32, SampleDesc.Count @36, Flags u32 @48.
    let Some(d) = read_bytes(desc, 52) else { return };
    let u32_at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let flags = u32_at(48);
    if u32_at(0) != 3 || flags & 0x3 == 0 {
        return; // not TEXTURE2D, or neither ALLOW_RENDER_TARGET nor ALLOW_DEPTH_STENCIL
    }
    let rec = Rec {
        kind,
        width: u64::from_le_bytes(d[16..24].try_into().unwrap()),
        height: u32_at(24),
        depth: u16::from_le_bytes(d[28..30].try_into().unwrap()),
        mips: u16::from_le_bytes(d[30..32].try_into().unwrap()),
        format: u32_at(32),
        samples: u32_at(36),
        flags,
        resource: if ppv != 0 { read::<usize>(ppv).unwrap_or(0) } else { 0 },
        callers: callers(),
        at: since_start(),
    };
    if GAMEPLAY.load(Ordering::Relaxed) && RT_LOGGED.fetch_add(1, Ordering::Relaxed) < 400 {
        plog!("rt+ t={:.1}s {} {}x{} arr={} mips={} fmt={} samples={} flags={:#x} res={:#x} callers=[{}]",
              rec.at, rec.kind, rec.width, rec.height, rec.depth, rec.mips, rec.format, rec.samples, rec.flags,
              rec.resource, fmt_callers(&rec.callers));
    }
    let mut records = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
    if records.len() < 8192 {
        records.push(rec);
    }
}

unsafe fn tramp<T: Copy>(slot: usize) -> T {
    std::mem::transmute_copy(&TRAMPOLINES[slot].load(Ordering::SeqCst))
}

// All arguments as usize: on x64 every argument sits in its own 64-bit register or stack slot, so passing them
// through unchanged is exact.
type F8 = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize) -> i32;
type F9 = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize, usize) -> i32;
type F10 = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize, usize, usize) -> i32;
type F11 = unsafe extern "system" fn(usize, usize, usize, usize, usize, usize, usize, usize, usize, usize, usize) -> i32;
type Rtv = unsafe extern "system" fn(usize, usize, usize, usize);

/// CreateCommittedResource(heap props, heap flags, desc, state, clear, riid, ppv).
unsafe extern "system" fn committed(a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize, a6: usize, a7: usize) -> i32 {
    let r = tramp::<F8>(0)(a0, a1, a2, a3, a4, a5, a6, a7);
    if r >= 0 { note("committed", a3, a7) }
    r
}
/// CreatePlacedResource(heap, offset, desc, state, clear, riid, ppv).
unsafe extern "system" fn placed(a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize, a6: usize, a7: usize) -> i32 {
    let r = tramp::<F8>(1)(a0, a1, a2, a3, a4, a5, a6, a7);
    if r >= 0 { note("placed", a3, a7) }
    r
}
/// CreateCommittedResource1(heap props, flags, desc, state, clear, session, riid, ppv).
unsafe extern "system" fn committed1(a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize, a6: usize, a7: usize, a8: usize) -> i32 {
    let r = tramp::<F9>(2)(a0, a1, a2, a3, a4, a5, a6, a7, a8);
    if r >= 0 { note("committed1", a3, a8) }
    r
}
/// CreateCommittedResource2(heap props, flags, desc1, state, clear, session, riid, ppv).
unsafe extern "system" fn committed2(a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize, a6: usize, a7: usize, a8: usize) -> i32 {
    let r = tramp::<F9>(3)(a0, a1, a2, a3, a4, a5, a6, a7, a8);
    if r >= 0 { note("committed2", a3, a8) }
    r
}
/// CreatePlacedResource1(heap, offset, desc1, state, clear, riid, ppv).
unsafe extern "system" fn placed1(a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize, a6: usize, a7: usize) -> i32 {
    let r = tramp::<F8>(4)(a0, a1, a2, a3, a4, a5, a6, a7);
    if r >= 0 { note("placed1", a3, a7) }
    r
}
/// CreateCommittedResource3(heap props, flags, desc1, layout, clear, session, n castable, castable, riid, ppv).
unsafe extern "system" fn committed3(a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize, a6: usize, a7: usize, a8: usize, a9: usize, a10: usize) -> i32 {
    let r = tramp::<F11>(5)(a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10);
    if r >= 0 { note("committed3", a3, a10) }
    r
}
/// CreatePlacedResource2(heap, offset, desc1, layout, clear, n castable, castable, riid, ppv).
unsafe extern "system" fn placed2(a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize, a6: usize, a7: usize, a8: usize, a9: usize) -> i32 {
    let r = tramp::<F10>(6)(a0, a1, a2, a3, a4, a5, a6, a7, a8, a9);
    if r >= 0 { note("placed2", a3, a9) }
    r
}
/// CreateRenderTargetView(resource, desc, handle): logged only while a menu is opening.
unsafe extern "system" fn rtv(a0: usize, resource: usize, desc: usize, handle: usize) {
    tramp::<Rtv>(7)(a0, resource, desc, handle);
    if ARMED.load(Ordering::Relaxed) && RTV_LOGGED.fetch_add(1, Ordering::Relaxed) < 60 {
        let format = if desc != 0 { read::<u32>(desc).unwrap_or(0) } else { 0 };
        plog!("rtv t={:.1}s res={resource:#x} fmt={format} handle={handle:#x} callers=[{}]", since_start(),
              fmt_callers(&callers()));
    }
}

/// Hooks ID3D12Device's resource creation (every interface version) through a throwaway device's vtable: the
/// functions live in the D3D12 runtime and are shared by the game's device. Returns lines for the log.
pub fn install_d3d_hooks() -> Vec<String> {
    use hudhook::mh::{MhHook, MH_EnableHook, MH_Initialize};
    let _ = START.get_or_init(Instant::now);
    let mut report = Vec::new();
    unsafe {
        let _ = MH_Initialize(); // hudhook calls it again later and accepts "already initialized"
        let dll = windows_sys::Win32::System::LibraryLoader::LoadLibraryW(
            "d3d12.dll\0".encode_utf16().collect::<Vec<u16>>().as_ptr());
        if dll.is_null() {
            return vec!["d3d12.dll not loaded".into()];
        }
        let Some(create) = windows_sys::Win32::System::LibraryLoader::GetProcAddress(dll, c"D3D12CreateDevice".as_ptr().cast())
        else { return vec!["no D3D12CreateDevice".into()] };
        type Create = unsafe extern "system" fn(*mut c_void, u32, *const u128, *mut *mut c_void) -> i32;
        let create: Create = std::mem::transmute(create);
        // IID_ID3D12Device 189819f1-1db6-4b57-be54-1821339b85f7, as a GUID in memory.
        let iid: [u8; 16] = [0xf1, 0x19, 0x98, 0x18, 0xb6, 0x1d, 0x57, 0x4b, 0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7];
        let mut device = std::ptr::null_mut();
        let hr = create(std::ptr::null_mut(), 0xb000, iid.as_ptr().cast(), &mut device);
        if hr < 0 || device.is_null() {
            return vec![format!("D3D12CreateDevice {hr:#x}")];
        }
        let vtable = *(device as *const *const usize);
        // QueryInterface for the newest interface each slot needs; the slots past ID3D12Device are only read when the
        // runtime's device object supports that version (checked through QueryInterface).
        type Qi = unsafe extern "system" fn(*mut c_void, *const u8, *mut *mut c_void) -> i32;
        type Release = unsafe extern "system" fn(*mut c_void) -> u32;
        let qi: Qi = std::mem::transmute(*vtable.add(0));
        let release: Release = std::mem::transmute(*vtable.add(2));
        // The interface's own vtable (the runtime may give newer interfaces a different one), or None.
        let supports = |iid: [u8; 16]| -> Option<*const usize> {
            let mut out = std::ptr::null_mut();
            if qi(device, iid.as_ptr(), &mut out) < 0 || out.is_null() {
                return None;
            }
            let table = *(out as *const *const usize);
            release(out);
            Some(table)
        };
        // ID3D12Device4 e865df17-a9ee-46f9-a463-3098315aa2e5, Device8 9218e6bb-f944-4f7e-a75c-b1b2c7b701f3,
        // Device10 517f8718-aa66-49f9-b02b-a7ab89c06031.
        let dev4 = supports([0x17, 0xdf, 0x65, 0xe8, 0xee, 0xa9, 0xf9, 0x46, 0xa4, 0x63, 0x30, 0x98, 0x31, 0x5a, 0xa2, 0xe5]);
        let dev8 = supports([0xbb, 0xe6, 0x18, 0x92, 0x44, 0xf9, 0x7e, 0x4f, 0xa7, 0x5c, 0xb1, 0xb2, 0xc7, 0xb7, 0x01, 0xf3]);
        let dev10 = supports([0x18, 0x87, 0x7f, 0x51, 0x66, 0xaa, 0xf9, 0x49, 0xb0, 0x2b, 0xa7, 0xab, 0x89, 0xc0, 0x60, 0x31]);
        report.push(format!("D3D12 device interfaces: Device4 {}, Device8 {}, Device10 {}", dev4.is_some(),
                            dev8.is_some(), dev10.is_some()));
        let mut slots: Vec<(*const usize, usize, usize, usize, &str)> = vec![
            (vtable, 27, committed as *const () as usize, 0, "CreateCommittedResource"),
            (vtable, 29, placed as *const () as usize, 1, "CreatePlacedResource"),
            (vtable, 20, rtv as *const () as usize, 7, "CreateRenderTargetView"),
        ];
        if let Some(t) = dev4 { slots.push((t, 53, committed1 as *const () as usize, 2, "CreateCommittedResource1")); }
        if let Some(t) = dev8 {
            slots.push((t, 69, committed2 as *const () as usize, 3, "CreateCommittedResource2"));
            slots.push((t, 70, placed1 as *const () as usize, 4, "CreatePlacedResource1"));
        }
        if let Some(t) = dev10 {
            slots.push((t, 76, committed3 as *const () as usize, 5, "CreateCommittedResource3"));
            slots.push((t, 77, placed2 as *const () as usize, 6, "CreatePlacedResource2"));
        }
        for (table, index, detour, slot, name) in slots {
            let target = *table.add(index);
            match MhHook::new(target as *mut c_void, detour as *mut c_void) {
                Ok(hook) => {
                    TRAMPOLINES[slot].store(hook.trampoline() as usize, Ordering::SeqCst);
                    std::mem::forget(hook);
                    let status = MH_EnableHook(target as *mut c_void);
                    report.push(format!("{name} (slot {index}) hooked at {target:#x}: {status:?}"));
                }
                Err(e) => report.push(format!("{name} (slot {index}) at {target:#x}: {e:?}")),
            }
        }
        release(device);
    }
    report
}

/// The render targets created before gameplay, grouped (size, format, flags, kind, first caller) with counts.
fn summarise_startup_targets() {
    let records = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
    let mut groups: Vec<((u64, u32, u16, u32, u32, &str, usize), u32)> = Vec::new();
    for r in records.iter() {
        let key = (r.width, r.height, r.depth, r.format, r.flags, r.kind, r.callers.first().copied().unwrap_or(0));
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => g.1 += 1,
            None => groups.push((key, 1)),
        }
    }
    plog!("{} render-target/depth textures created before gameplay, {} kinds:", records.len(), groups.len());
    for ((w, h, d, f, fl, k, c), n) in groups.iter().take(300) {
        plog!("rt0 {n}x {k} {w}x{h} arr={d} fmt={f} flags={fl:#x} caller={c:#x}");
    }
}

/// Which of the recorded render targets `value` is (an ID3D12Resource pointer), for object dumps.
fn known_target(value: usize) -> Option<String> {
    if value == 0 {
        return None;
    }
    let records = RECORDS.lock().unwrap_or_else(|e| e.into_inner());
    records.iter().position(|r| r.resource == value).map(|i| {
        let r = &records[i];
        format!("RT#{i} {}x{} fmt={} t={:.0}s", r.width, r.height, r.format, r.at)
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Background scans: public byte patterns and RTTI class names
// ---------------------------------------------------------------------------------------------------------------

/// EROverlay's (soarqin/EROverlay src/util/uiscene_patterns.inc) 1.16 patterns for the GX draw queue and the final
/// UI composite; only whether and where they still match is logged.
const PATTERNS: &[(&str, &str)] = &[
    ("QUEUE_CALLBACK", "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 30 8B 5C 24 60 49 8B F1 49 8B E8 4C 8B F2 48 8B F9 83 FB FF 75 1B 8B 41 38 8B D8 81 E3 00 00 80 FF 81 C3 00 00 80 00 33 D8 81 E3 00 00 80 1F 33 D8 41 B9 04 00 00 00 89 5C 24 20 B2 02 45 8D 41 14 E8"),
    ("QUEUE_SORT", "48 63 51 30 44 0F B6 44 24 08 48 C1 E2 04 48 03 51 28 48 8B 49 28 E9"),
    ("QUEUE_SUBMIT", "48 85 D2 0F 84 9B 00 00 00 57 48 83 EC 50 48 C7 44 24 30 FE FF FF FF 48 89 5C 24 60 48 8B FA 48 8B D9 C7 44 24 20 01 00 00 00 45 33 C9 4C 8B C1 48 8D 15"),
    ("BEGIN_CALLBACK", "40 57 48 83 EC 40 48 C7 44 24 20 FE FF FF FF 48 89 5C 24 50 48 8B FA 48 8B D1 48 8D 4C 24 28 E8 ?? ?? ?? ?? 90 48 8B 5C 24 28 48 8B 8F 30 0F 00 00 48 85 C9 74 08"),
    ("SCENE_SETUP", "4C 8B DC 55 56 41 56 41 57 48 81 EC A8 00 00 00 41 0F 29 73 B8 0F 29 7C 24 70 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 44 24 60 48 8B EA 4D 89 63 D0 48 8B F1 4D 89 6B C8"),
    ("RTV_CREATE", "48 89 5C 24 10 48 89 74 24 18 55 57 41 56 48 8B EC 48 83 EC 40 49 8B D8 4C 8B F2 48 8B F9 E8 ?? ?? ?? ?? 0F 10 03 48 8D 55 20 48 8B F0 0F 11 47 08 44 8B 4B 10 44 89 4F 18 48 8B 48 48 E8"),
    ("UI_COMPOSITE", "48 8B C4 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 A8 FD FF FF 48 81 EC 20 03 00 00 48 C7 45 88 FE FF FF FF 48 89 58 18 0F 29 70 B8 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 00 02 00 00 48 8B DA 48 8B F9 4C 8B B1 28 01 00 00 41 80 BE 3C 01 00 00 00 75 0E"),
    ("NATIVE_ENQUEUE", "48 89 5C 24 10 57 48 83 EC 20 48 8B D9 48 C7 44 24 30 00 00 00 00 4C 8D 89 E0 0F 00 00 41 8B F8 48 63 89 08 07 00 00 4C 8B C2 48 8D 54 24 30 48 8B 8C CB D8 06 00 00 E8"),
];

/// Class-name fragments (lower case) that may belong to the mirror's scene.
const CLASS_WORDS: &[&str] = &[
    "charamake", "chrmake", "facegen", "offscr", "offscreen", "preview", "viewer", "thumbnail", "rendertarget",
    "rendertotexture", "menuworld", "mirror", "menumodel", "modelview", "menucam", "menuchr", "chrmenu",
    "menuscene", "gxdraw", "scenedraw", "menurend", "facemodel", "dummychr", "menuoffscr",
];

/// Sections of the image: (start, size, executable).
fn sections() -> Vec<(usize, usize, bool)> {
    let base = image().0;
    unsafe {
        let pe = base + *((base + 0x3C) as *const u32) as usize;
        let count = *((pe + 6) as *const u16) as usize;
        let optional_size = *((pe + 20) as *const u16) as usize;
        let table = pe + 24 + optional_size;
        (0..count)
            .map(|i| {
                let s = table + 40 * i;
                let characteristics = *((s + 36) as *const u32);
                (base + *((s + 12) as *const u32) as usize, *((s + 8) as *const u32) as usize,
                 characteristics & 0x2000_0000 != 0)
            })
            .collect()
    }
}

fn parse_pattern(p: &str) -> Vec<Option<u8>> {
    p.split_whitespace().map(|t| if t == "??" { None } else { u8::from_str_radix(t, 16).ok() }).collect()
}

fn scan_all(regions: &[(usize, usize)], pattern: &[Option<u8>]) -> Vec<usize> {
    let first = pattern[0].unwrap_or(0);
    let mut hits = Vec::new();
    for &(start, size) in regions {
        if size < pattern.len() {
            continue;
        }
        let bytes = unsafe { std::slice::from_raw_parts(start as *const u8, size) };
        for i in 0..=size - pattern.len() {
            if bytes[i] == first
                && bytes[i..i + pattern.len()].iter().zip(pattern).all(|(b, p)| p.map_or(true, |p| p == *b))
            {
                hits.push(start + i);
                if hits.len() >= 8 {
                    return hits;
                }
            }
        }
    }
    hits
}

/// Runs once, in its own thread: pattern matches and the RTTI classes named like a menu scene.
fn background_scans() {
    let started = Instant::now();
    let sections = sections();
    let code: Vec<(usize, usize)> = sections.iter().filter(|s| s.2)
        .flat_map(|s| game::readable_regions(s.0, s.1)).collect();
    let data: Vec<(usize, usize)> = sections.iter().filter(|s| !s.2)
        .flat_map(|s| game::readable_regions(s.0, s.1)).collect();
    for (name, pattern) in PATTERNS {
        let hits = scan_all(&code, &parse_pattern(pattern));
        plog!("sig {name}: {} match(es){}", hits.len(),
              if hits.is_empty() { String::new() } else { format!(" at rva {}", fmt_callers(&hits.iter().map(|h| rva(*h)).collect::<Vec<_>>())) });
    }
    // RTTI type descriptors: {vtable of type_info, spare, ".?AV...@@"}.
    let mut types: HashMap<u32, String> = HashMap::new();
    let mut total = 0usize;
    for &(start, size) in &data {
        let bytes = unsafe { std::slice::from_raw_parts(start as *const u8, size) };
        let mut i = 16;
        while i + 4 < size {
            if &bytes[i..i + 4] == b".?AV" || &bytes[i..i + 4] == b".?AU" {
                total += 1;
                let end = bytes[i..size.min(i + 256)].iter().position(|b| *b == 0).map(|e| i + e);
                if let Some(end) = end {
                    if let Ok(raw) = std::str::from_utf8(&bytes[i..end]) {
                        let lower = raw.to_ascii_lowercase();
                        if CLASS_WORDS.iter().any(|w| lower.contains(w)) {
                            types.insert(rva(start + i - 16) as u32, demangle(raw).unwrap_or_else(|| raw.to_string()));
                        }
                    }
                    i = end;
                }
            }
            i += 1;
        }
    }
    // Complete object locators (signature 1, type descriptor RVA at +12, own RVA at +20) of those types, then the
    // vtables that point at them (the locator sits just before the vtable).
    let mut locators: HashMap<usize, (String, u32)> = HashMap::new();
    for &(start, size) in &data {
        let bytes = unsafe { std::slice::from_raw_parts(start as *const u8, size) };
        let mut i = (4 - start % 4) % 4;
        while i + 24 <= size {
            let at = |o: usize| u32::from_le_bytes(bytes[i + o..i + o + 4].try_into().unwrap());
            if at(0) == 1 {
                if let Some(name) = types.get(&at(12)) {
                    if at(20) as usize == rva(start + i) {
                        locators.insert(start + i, (name.clone(), at(4)));
                    }
                }
            }
            i += 4;
        }
    }
    let mut vtables: Vec<(String, usize, u32)> = Vec::new();
    for &(start, size) in &data {
        let bytes = unsafe { std::slice::from_raw_parts(start as *const u8, size) };
        let mut i = (8 - start % 8) % 8;
        while i + 8 <= size {
            let v = usize::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
            if let Some((name, offset)) = locators.get(&v) {
                vtables.push((name.clone(), rva(start + i + 8), *offset));
            }
            i += 8;
        }
    }
    vtables.sort();
    let mut names: Vec<&String> = types.values().collect();
    names.sort();
    plog!("rtti: {total} type names, {} match the scene words, {} vtables ({:.1}s)", types.len(), vtables.len(),
          started.elapsed().as_secs_f32());
    for (name, vt, offset) in vtables.iter().take(250) {
        plog!("class {name} vtable rva {vt:#x}{}", if *offset != 0 { format!(" (base at +{offset:#x})") } else { String::new() });
    }
    let without: Vec<&&String> = names.iter().filter(|n| !vtables.iter().any(|v| &v.0 == **n)).take(120).collect();
    if !without.is_empty() {
        plog!("classes without a vtable found: {}", without.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// MenuOffscrRendParam
// ---------------------------------------------------------------------------------------------------------------

struct OffscrParam {
    holder_index: usize,
    /// (row id, row address)
    rows: Vec<(u32, usize)>,
}

const OFFSCR_NAME: &[u8] = b"MENU_OFFSCR_REND_PARAM_ST";

fn find_offscr_param() -> Option<OffscrParam> {
    use eldenring::cs::{ParamResCap, SoloParamRepository};
    use eldenring::fd4::FD4ParamResCap;
    use std::mem::offset_of;
    let repo = from_singleton::static_of::<SoloParamRepository>()?;
    let repo = read::<usize>(repo.as_ptr() as usize)?;
    let holders = repo + offset_of!(SoloParamRepository, solo_param_holders);
    const HOLDER: usize = 72; // res_cap_count u32 (+pad), then 8 res cap pointers
    let file_of = |index: usize| -> Option<usize> {
        let holder = holders + index * HOLDER;
        let res_cap = read::<usize>(holder + 8)?;
        let param = read::<usize>(res_cap + offset_of!(ParamResCap, param_res_cap))?;
        read::<usize>(param + offset_of!(FD4ParamResCap, data))
    };
    let is_offscr = |file: usize| -> bool {
        let Some(header) = read_bytes(file, 0x40) else { return false };
        let name = if header[0x2D] & 0x80 != 0 {
            let offset = u32::from_le_bytes(header[0x10..0x14].try_into().unwrap()) as usize;
            read_bytes(file + offset, OFFSCR_NAME.len())
        } else {
            Some(header[0x0C..0x0C + OFFSCR_NAME.len()].to_vec())
        };
        name.as_deref() == Some(OFFSCR_NAME)
    };
    let (holder_index, file) = std::iter::once(78).chain(0..194)
        .find_map(|i| file_of(i).filter(|f| is_offscr(*f)).map(|f| (i, f)))?;
    let header = read_bytes(file, 0x40)?;
    let rows = u16::from_le_bytes(header[0x0A..0x0C].try_into().unwrap()) as usize;
    let format = header[0x2D];
    let wide = format & 0x04 != 0;
    let extended = wide || (format & 0x01 != 0 && format & 0x02 != 0);
    let first = 0x30 + if extended { 0x10 } else { 0 };
    let entry = if wide { 24 } else { 12 };
    let table = read_bytes(file + first, rows.min(512) * entry)?;
    let rows = table.chunks_exact(entry).map(|d| {
        let id = u32::from_le_bytes(d[0..4].try_into().unwrap());
        let offset = if wide { u64::from_le_bytes(d[8..16].try_into().unwrap()) as usize }
                     else { u32::from_le_bytes(d[4..8].try_into().unwrap()) as usize };
        (id, file + offset)
    }).collect();
    Some(OffscrParam { holder_index, rows })
}

fn log_offscr_rows(param: &OffscrParam, why: &str) {
    plog!("MenuOffscrRendParam ({why}): holder {} (crate says 78), {} rows", param.holder_index, param.rows.len());
    for (id, at) in param.rows.iter().take(32) {
        let Some(b) = read_bytes(*at, 0x40) else { continue };
        let f = |i: usize| f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
        let u = |i: usize| u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
        plog!("offscr row {id} @{at:#x}: at=({:.2},{:.2},{:.2}) dist={:.2} rotX={:.1} rotY={:.1} fov={:.1} dist {:.2}..{:.2} rotX {:.0}..{:.0} gparam={} envTex={}",
              f(0), f(1), f(2), f(3), f(4), f(5), f(6), f(7), f(8), f(9), f(10), u(11), u(12));
    }
}

fn write_f32(address: usize, value: f32) -> bool {
    use windows_sys::Win32::System::Diagnostics::Debug::WriteProcessMemory;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut written = 0;
    unsafe {
        WriteProcessMemory(GetCurrentProcess(), address as _, (&value as *const f32).cast(), 4, &mut written) != 0
            && written == 4
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Per-frame probe
// ---------------------------------------------------------------------------------------------------------------

/// Singleton names (lower case fragments) whose pointer slots are compared when a menu opens or closes.
const SINGLETON_WORDS: &[&str] = &[
    "menu", "face", "make", "cam", "field", "rend", "graphic", "draw", "scene", "thumb", "offscr", "model", "view",
    "gx", "pad", "fe", "chr",
];

struct Snapshot {
    name: String,
    address: usize,
    bytes: Vec<u8>,
}

pub struct Probe {
    started: bool,
    last_tick: Instant,
    last_second: Instant,
    hud: Option<u8>,
    menu_open: bool,
    menu_since: Instant,
    sessions: u32,
    snapshots: Vec<Snapshot>,
    last_baseline: Instant,
    chr_baseline: Vec<(usize, u32)>,
    chr_logged: Vec<usize>,
    player_at_open: Option<(usize, [f32; 3])>,
    diffs_done: u8,
    param: Option<OffscrParam>,
    param_tries: u32,
    nudged: bool,
    dumped: Vec<usize>,
    cam_line: String,
    /// The most the screen faded during the current menu (loading screens fade; the mirror shouldn't).
    max_fade: f32,
    /// The player (ChrIns, position) when the menu opened.
    open_player: Option<(usize, [f32; 3])>,
}

impl Probe {
    pub fn new() -> Self {
        let now = Instant::now();
        Probe {
            started: false, last_tick: now, last_second: now, hud: None, menu_open: false, menu_since: now,
            sessions: 0, snapshots: Vec::new(), last_baseline: now, chr_baseline: Vec::new(), chr_logged: Vec::new(),
            player_at_open: None, diffs_done: 0, param: None, param_tries: 0, nudged: false, dumped: Vec::new(),
            cam_line: String::new(), max_fade: 0.0, open_player: None,
        }
    }

    /// Called every frame from the render loop; does its work four times a second at most.
    pub fn tick(&mut self, game: Option<&Game>, nudge: bool) {
        let now = Instant::now();
        if (now - self.last_tick).as_secs_f32() < 0.25 {
            return;
        }
        self.last_tick = now;
        let Some(game) = game else { return };
        let player = game.player_address();
        if !self.started {
            if player.is_none() || game::hud_state() != Some(3) {
                return;
            }
            self.start(game);
        }
        if self.param.is_none() && self.param_tries < 20 && (now - self.last_second).as_secs_f32() >= 1.0 {
            self.param_tries += 1;
            self.param = find_offscr_param();
            match &self.param {
                Some(p) => log_offscr_rows(p, "found"),
                None if self.param_tries == 20 => plog!("MenuOffscrRendParam not found"),
                None => {}
            }
        }
        let hud = game::hud_state();
        if hud != self.hud {
            plog!("hud_state {:?} -> {:?} fade={:.2} t={:.1}s", self.hud, hud, game::faded(), since_start());
            self.hud = hud;
        }
        let open = hud.is_some_and(|h| h != 3);
        if open && !self.menu_open {
            self.menu_open = true;
            self.menu_since = now;
            self.sessions += 1;
            self.diffs_done = 0;
            self.dumped.clear();
            self.max_fade = 0.0;
            RTV_LOGGED.store(0, Ordering::Relaxed);
            ARMED.store(true, Ordering::Relaxed);
            plog!("=== menu #{} OPEN (hud_state {:?}) t={:.1}s", self.sessions, hud, since_start());
            self.player_at_open = player.and_then(|p| Game::chr_physics(p).map(|pos| (p, pos)));
            self.open_player = self.player_at_open;
            plog!("player {:#x} at {:?}; {} characters loaded before", player.unwrap_or(0),
                  self.player_at_open.map(|p| p.1), self.chr_baseline.len());
            self.chr_logged = self.chr_baseline.iter().map(|c| c.0).collect();
            self.cam_line.clear();
            self.cameras(game, true);
            self.menu_man("open");
            self.diff_singletons("open");
            if let Some(p) = &self.param {
                log_offscr_rows(p, "menu open");
            }
        } else if !open && self.menu_open {
            self.menu_open = false;
            ARMED.store(false, Ordering::Relaxed);
            let seconds = (now - self.menu_since).as_secs_f32();
            let here = player.and_then(|p| Game::chr_physics(p).map(|q| (p, q)));
            let stayed = matches!((self.open_player, here), (Some((a, pa)), Some((b, pb))) if a == b && dist(pa, pb) < 1.0);
            plog!("=== menu #{} CLOSED after {seconds:.1}s, max fade {:.2}, player stayed put: {stayed}", self.sessions,
                  self.max_fade);
            self.menu_man("closed");
            self.diff_singletons("closed");
            self.characters(game, true);
            self.cameras(game, true);
            if nudge && !self.nudged && seconds > 5.0 && stayed && self.max_fade < 0.9 {
                self.nudged = true;
                self.nudge();
            }
            self.baseline(game);
        }
        if self.menu_open {
            self.max_fade = self.max_fade.max(game::faded());
            let elapsed = (now - self.menu_since).as_secs_f32();
            if elapsed > 10.0 {
                ARMED.store(false, Ordering::Relaxed);
            }
            if (self.diffs_done == 0 && elapsed >= 1.0) || (self.diffs_done == 1 && elapsed >= 4.0) {
                self.diffs_done += 1;
                let why = if self.diffs_done == 1 { "open +1s" } else { "open +4s" };
                self.menu_man(why);
                self.diff_singletons(why);
            }
            if (now - self.last_second).as_secs_f32() >= 1.0 {
                self.last_second = now;
                if elapsed < 120.0 {
                    self.cameras(game, false);
                    self.characters(game, false);
                }
            }
        } else if (now - self.last_baseline).as_secs_f32() >= 5.0 && !open && game::faded() < 0.05 {
            self.baseline(game);
        }
        if (now - self.last_second).as_secs_f32() >= 1.0 {
            self.last_second = now;
        }
    }

    fn start(&mut self, game: &Game) {
        self.started = true;
        GAMEPLAY.store(true, Ordering::Relaxed);
        plog!("scene probe started t={:.1}s, image base {:#x} size {:#x}", since_start(), image().0, image().1);
        summarise_startup_targets();
        let mut names: Vec<(&String, usize)> = from_singleton::map().iter()
            .map(|(k, v)| (k, v.as_ptr() as usize)).collect();
        names.sort();
        plog!("{} singletons; all names: {}", names.len(),
              names.iter().map(|n| n.0.as_str()).collect::<Vec<_>>().join(" "));
        std::thread::spawn(background_scans);
        self.baseline(game);
        let picked: Vec<String> = self.snapshots.iter()
            .map(|s| format!("{}@{:#x}({}B){}", s.name, s.address, s.bytes.len(),
                             rtti(s.address).map(|n| format!("={n}")).unwrap_or_default()))
            .collect();
        plog!("watching {} singletons: {}", picked.len(), picked.join(" "));
        self.cameras(game, true);
    }

    /// Snapshots of the watched singletons and the loaded characters, as they are in gameplay. A change in the
    /// number of c0000 (player-model) characters is logged even without a menu (in case the mirror doesn't hide the
    /// HUD).
    fn baseline(&mut self, game: &Game) {
        self.last_baseline = Instant::now();
        self.snapshots = snapshot_singletons();
        let chrs = game.loaded_characters();
        let humans = |list: &[(usize, u32)]| list.iter().filter(|c| c.1 == 0).map(|c| c.0).collect::<Vec<_>>();
        let (before, now) = (humans(&self.chr_baseline), humans(&chrs));
        if !self.chr_baseline.is_empty() && before != now {
            plog!("c0000 characters changed: {} -> {}: {}", before.len(), now.len(),
                  now.iter().map(|c| format!("{c:#x} {} phys={:?}", rtti(*c).unwrap_or_default(), Game::chr_physics(*c)))
                     .collect::<Vec<_>>().join("; "));
        }
        self.chr_baseline = chrs;
    }

    /// CSMenuMan's popup menu, its current top menu job and the window job, with class names; the top job is
    /// dumped one level deep.
    fn menu_man(&mut self, why: &str) {
        use eldenring::cs::CSMenuManImp;
        let Some(man) = from_singleton::static_of::<CSMenuManImp>().and_then(|m| read::<usize>(m.as_ptr() as usize))
        else { return };
        let popup = read::<usize>(man + POPUP_MENU).unwrap_or(0);
        let window = read::<usize>(man + POPUP_MENU + 8).unwrap_or(0);
        let top = if heap_like(popup) { read::<usize>(popup + TOP_MENU_JOB).unwrap_or(0) } else { 0 };
        let name = |p: usize| if heap_like(p) { rtti(p).unwrap_or_else(|| "?".into()) } else { "-".into() };
        plog!("[{why}] menuman {man:#x}: popup {popup:#x}({}) top_job {top:#x}({}) window_job {window:#x}({})",
              name(popup), name(top), name(window));
        for p in [top, window] {
            if heap_like(p) && !self.dumped.contains(&p) && self.dumped.len() < 16 {
                self.dumped.push(p);
                dump_object(p, &name(p), why);
            }
        }
    }

    /// Logs the pointer slots of the watched singletons that differ from the gameplay baseline, naming what they
    /// point to, and dumps newly referenced objects with interesting class names one level deep.
    fn diff_singletons(&mut self, why: &str) {
        let now = snapshot_singletons();
        let mut fresh: Vec<(usize, String)> = Vec::new();
        let mut changed = 0;
        for new in &now {
            let Some(old) = self.snapshots.iter().find(|s| s.name == new.name) else { continue };
            if old.address != new.address {
                plog!("[{why}] {} moved {:#x} -> {:#x}", new.name, old.address, new.address);
                continue;
            }
            let n = old.bytes.len().min(new.bytes.len()) / 8;
            let mut named = Vec::new();
            let mut plain = Vec::new();
            for i in 0..n {
                let a = usize::from_le_bytes(old.bytes[i * 8..i * 8 + 8].try_into().unwrap());
                let b = usize::from_le_bytes(new.bytes[i * 8..i * 8 + 8].try_into().unwrap());
                if a == b {
                    continue;
                }
                let (na, nb) = (heap_like(a).then(|| rtti(a)).flatten(), heap_like(b).then(|| rtti(b)).flatten());
                if na.is_some() || nb.is_some() {
                    named.push(format!("+{:#x} {a:#x}({}) -> {b:#x}({})", i * 8, na.as_deref().unwrap_or("-"),
                                       nb.as_deref().unwrap_or("-")));
                    if let Some(nb) = nb {
                        fresh.push((b, nb));
                    }
                } else if plain.len() < 12 {
                    plain.push(format!("+{:#x} {a:#x}->{b:#x}", i * 8));
                }
            }
            if !named.is_empty() || !plain.is_empty() {
                changed += 1;
                plog!("[{why}] {} @{:#x}: {}{}{}", new.name, new.address, named.join("; "),
                      if named.is_empty() || plain.is_empty() { "" } else { " | other: " }, plain.join(" "));
            }
        }
        plog!("[{why}] {changed} watched singletons changed");
        // One level deeper for the new objects (at most 16 per menu session).
        for (address, name) in fresh {
            if self.dumped.len() >= 16 || self.dumped.contains(&address) {
                continue;
            }
            self.dumped.push(address);
            dump_object(address, &name, why);
        }
    }

    /// One line per camera: CSCamera's four perspective cameras (and its copy mask) and FieldArea's GameRend
    /// cameras. Logged when anything about them changes, or every second when `always`... while a menu is up the
    /// positions change, so the line is logged each second then.
    fn cameras(&mut self, game: &Game, force: bool) {
        use eldenring::cs::CSCamera;
        use std::mem::offset_of;
        let player = game.player_address().and_then(Game::chr_physics);
        let mut parts = Vec::new();
        if let Some(cam) = from_singleton::static_of::<CSCamera>().and_then(|c| read::<usize>(c.as_ptr() as usize)) {
            let mask = read::<u32>(cam + offset_of!(CSCamera, camera_mask)).unwrap_or(0);
            parts.push(format!("CSCamera@{cam:#x} mask={mask:#x}"));
            for (i, field) in [offset_of!(CSCamera, pers_cam_1), offset_of!(CSCamera, pers_cam_2),
                               offset_of!(CSCamera, pers_cam_3), offset_of!(CSCamera, pers_cam_4)].iter().enumerate() {
                if let Some(p) = read::<usize>(cam + field) {
                    parts.push(format!("p{}={}", i + 1, describe_cam(p, player)));
                }
            }
        }
        if let Some(field) = from_singleton::map().get("FieldArea").and_then(|s| read::<usize>(s.as_ptr() as usize)) {
            if let Some(rend) = read::<usize>(field + 0x20).filter(|r| heap_like(*r)) {
                let free = read::<u8>(rend + 0xC8).unwrap_or(0xFF);
                parts.push(format!("| GameRend@{rend:#x}({}) free={free}", rtti(rend).unwrap_or_default()));
                // Every camera-like object GameRend points to in its first 0x400 bytes (TGA: +0x20 CSPersCam1,
                // +0xD0 CSDebugCam).
                if let Some(bytes) = read_bytes(rend, 0x400).or_else(|| read_bytes(rend, 0x100)) {
                    for (i, chunk) in bytes.chunks_exact(8).enumerate() {
                        let p = usize::from_le_bytes(chunk.try_into().unwrap());
                        if heap_like(p) && rtti(p).is_some_and(|n| n.contains("Cam")) {
                            parts.push(format!("+{:#x}={}", i * 8, describe_cam(p, player)));
                        }
                    }
                }
            }
        }
        if let Some(p) = player {
            parts.push(format!("| player=({:.2},{:.2},{:.2})", p[0], p[1], p[2]));
        }
        let line = parts.join(" ");
        // Without a menu only pointer/name changes matter: compare the line minus the numbers.
        let shape: String = line.split(' ').filter(|w| w.contains('@') || w.starts_with("mask")).collect();
        if force || self.menu_open || shape != self.cam_line {
            self.cam_line = shape;
            plog!("cam {line}");
        }
    }

    /// Characters added or removed since the menu opened, and where the player is.
    fn characters(&mut self, game: &Game, closing: bool) {
        let now = game.loaded_characters();
        let ids: Vec<usize> = now.iter().map(|c| c.0).collect();
        let added: Vec<&(usize, u32)> = now.iter().filter(|c| !self.chr_logged.contains(&c.0)).collect();
        let removed: Vec<usize> = self.chr_logged.iter().filter(|c| !ids.contains(c)).copied().collect();
        let player = game.player_address();
        let moved = match (self.player_at_open, player.and_then(|p| Game::chr_physics(p).map(|q| (p, q)))) {
            (Some((a, pa)), Some((b, pb))) => a != b || dist(pa, pb) > 0.05,
            _ => false,
        };
        if added.is_empty() && removed.is_empty() && !moved && !closing {
            return;
        }
        let describe = |(c, model): &(usize, u32)| {
            format!("{c:#x} c{model:04} {} phys={:?}", rtti(*c).unwrap_or_default(), Game::chr_physics(*c))
        };
        plog!("chr: {} loaded ({} added, {} removed since open); player {:#x} at {:?}{}", now.len(), added.len(),
              removed.len(), player.unwrap_or(0), player.and_then(Game::chr_physics),
              if moved { " (player changed/moved)" } else { "" });
        for c in added.iter().take(12) {
            plog!("chr+ {}", describe(c));
        }
        if !removed.is_empty() {
            plog!("chr- {}", removed.iter().take(24).map(|c| format!("{c:#x}")).collect::<Vec<_>>().join(" "));
        }
        self.chr_logged = ids;
        if let Some(p) = player {
            self.player_at_open = Game::chr_physics(p).map(|q| (p, q)).or(self.player_at_open);
        }
    }

    /// The opt-in test: row 0's camera distance +50%, from the next mirror visit on.
    fn nudge(&mut self) {
        let Some(param) = &self.param else {
            plog!("nudge: MenuOffscrRendParam not found, nothing changed");
            return;
        };
        let Some(&(_, row)) = param.rows.iter().find(|r| r.0 == 0) else {
            plog!("nudge: no row 0");
            return;
        };
        match read::<f32>(row + 12) {
            Some(d) if d.is_finite() && d > 0.0 && d < 1000.0 => {
                let ok = write_f32(row + 12, d * 1.5);
                plog!("NUDGE: row 0 camDist {d:.3} -> {:.3} ({}); open the mirror again", d * 1.5,
                      if ok { "written" } else { "write FAILED" });
                log_offscr_rows(param, "after nudge");
            }
            other => plog!("nudge: row 0 camDist unreadable or odd: {other:?}"),
        }
    }
}

/// CSMenuManImp.popup_menu (window_job follows it) and CSPopupMenu's current_top_menu_job (private in the crate).
const POPUP_MENU: usize = 0x80;
const TOP_MENU_JOB: usize = 0xB0;
const _: () = assert!(std::mem::offset_of!(eldenring::cs::CSMenuManImp, popup_menu) == POPUP_MENU);

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// "addr(Class) pos=(x,y,z) fov=deg d=metres-to-player aim=degrees-off-player".
fn describe_cam(p: usize, player: Option<[f32; 3]>) -> String {
    use eldenring::cs::CSCam;
    use std::mem::offset_of;
    let name = rtti(p).unwrap_or_else(|| "?".into());
    let (Some(m), Some(fov)) = (read::<[[f32; 4]; 4]>(p + offset_of!(CSCam, matrix)), read::<f32>(p + offset_of!(CSCam, fov)))
    else { return format!("{p:#x}({name}) unreadable") };
    let pos = [m[3][0], m[3][1], m[3][2]];
    if !pos.iter().chain(&[fov]).all(|v| v.is_finite() && v.abs() < 1.0e6) {
        return format!("{p:#x}({name}) bad");
    }
    let mut out = format!("{p:#x}({name}) pos=({:.2},{:.2},{:.2}) fov={:.1}", pos[0], pos[1], pos[2], fov.to_degrees());
    if let Some(pl) = player {
        let d = dist(pos, pl);
        let to = [pl[0] - pos[0], pl[1] - pos[1], pl[2] - pos[2]];
        let fwd = [m[2][0], m[2][1], m[2][2]];
        let fl = (fwd[0] * fwd[0] + fwd[1] * fwd[1] + fwd[2] * fwd[2]).sqrt();
        let cos = (to[0] * fwd[0] + to[1] * fwd[1] + to[2] * fwd[2]) / (d * fl).max(1e-6);
        out += &format!(" d={d:.2} aim={:.0}", cos.clamp(-1.0, 1.0).acos().to_degrees());
    }
    out
}

/// The watched singletons' first bytes (up to 4 KB each).
fn snapshot_singletons() -> Vec<Snapshot> {
    let mut out = Vec::new();
    for (name, static_ptr) in from_singleton::map().iter() {
        let lower = name.to_ascii_lowercase();
        if !SINGLETON_WORDS.iter().any(|w| lower.contains(w)) {
            continue;
        }
        let Some(address) = read::<usize>(static_ptr.as_ptr() as usize).filter(|a| heap_like(*a)) else { continue };
        let bytes = [0x1000, 0x800, 0x400, 0x200, 0x100].iter().find_map(|n| read_bytes(address, *n));
        if let Some(bytes) = bytes {
            out.push(Snapshot { name: name.clone(), address, bytes });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The named pointers in an object's first 0x400 bytes (and any recorded render-target resource it holds), one line.
fn dump_object(address: usize, name: &str, why: &str) {
    let Some(bytes) = read_bytes(address, 0x400).or_else(|| read_bytes(address, 0x100)).or_else(|| read_bytes(address, 0x40))
    else { return };
    let mut fields = Vec::new();
    for (i, chunk) in bytes.chunks_exact(8).enumerate() {
        let p = usize::from_le_bytes(chunk.try_into().unwrap());
        if i == 0 || !heap_like(p) {
            continue;
        }
        if let Some(rt) = known_target(p) {
            fields.push(format!("+{:#x}={p:#x}[{rt}]", i * 8));
        } else if let Some(n) = rtti(p) {
            fields.push(format!("+{:#x}={n}", i * 8));
        }
        if fields.len() >= 40 {
            break;
        }
    }
    plog!("[{why}] obj {address:#x} {name}: {}", fields.join(" "));
}
