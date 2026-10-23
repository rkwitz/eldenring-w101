//! Input for the spellbook. Elden Ring reads the controller through XInput (xinput1_4.dll) and the keyboard and
//! mouse through DirectInput 8, so both are hooked (MinHook, through hudhook):
//! - XInput: the View/Back button (ER's map button) opens the book instead of the map during normal play; while the
//!   book is open the game sees an idle controller and the book reads the real one; `press_map` replays a View
//!   press to the game so that its own map opens.
//! - DirectInput: while the book is open the game's keyboard and mouse devices read as idle.
//!
//! Keyboard keys for the book itself (B, Esc, arrows) come through imgui, which hudhook feeds from window messages.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI16, AtomicU16, AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use hudhook::mh::{MH_ApplyQueued, MhHook};
use windows_sys::core::GUID;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};

pub const DPAD_UP: u16 = 0x0001;
pub const DPAD_DOWN: u16 = 0x0002;
pub const DPAD_LEFT: u16 = 0x0004;
pub const DPAD_RIGHT: u16 = 0x0008;
pub const BACK: u16 = 0x0020;
pub const LB: u16 = 0x0100;
pub const RB: u16 = 0x0200;
pub const A: u16 = 0x1000;
pub const B: u16 = 0x2000;
pub const X: u16 = 0x4000;
pub const Y: u16 = 0x8000;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Gamepad {
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    left_x: i16,
    left_y: i16,
    right_x: i16,
    right_y: i16,
}

#[repr(C)]
struct XInputState {
    packet: u32,
    pad: Gamepad,
}

type GetState = unsafe extern "system" fn(u32, *mut XInputState) -> u32;
type GetDeviceState = unsafe extern "system" fn(*mut c_void, u32, *mut c_void) -> i32;
type GetDeviceData = unsafe extern "system" fn(*mut c_void, u32, *mut c_void, *mut u32, u32) -> i32;

/// The book is open: the game gets no controller, keyboard or mouse input.
static BOOK_OPEN: AtomicBool = AtomicBool::new(false);
/// Normal play (ER's HUD in its default state): View opens the book, so the game doesn't see it.
static GAMEPLAY: AtomicBool = AtomicBool::new(false);
/// Until this many milliseconds after START_TIME the game sees View held (opening its map).
static INJECT_BACK_UNTIL: AtomicU64 = AtomicU64::new(0);
/// Buttons that went down since the book last asked, and the buttons and right stick now.
static PRESSED: AtomicU16 = AtomicU16::new(0);
static HELD: AtomicU16 = AtomicU16::new(0);
static RIGHT_Y: AtomicI16 = AtomicI16::new(0);
static LEFT_Y: AtomicI16 = AtomicI16::new(0);
/// When a controller last answered, in milliseconds after START_TIME (0 = never).
static PAD_SEEN: AtomicU64 = AtomicU64::new(0);
static PREVIOUS: [AtomicU16; 4] = [AtomicU16::new(0), AtomicU16::new(0), AtomicU16::new(0), AtomicU16::new(0)];

static TRAMPOLINES: [AtomicUsize; 6] = [
    AtomicUsize::new(0), // XInputGetState
    AtomicUsize::new(0), // XInputGetStateEx (ordinal 100)
    AtomicUsize::new(0), // IDirectInputDevice8W::GetDeviceState
    AtomicUsize::new(0), // IDirectInputDevice8W::GetDeviceData
    AtomicUsize::new(0), // IDirectInputDevice8A::GetDeviceState
    AtomicUsize::new(0), // IDirectInputDevice8A::GetDeviceData
];
static START_TIME: OnceLock<Instant> = OnceLock::new();
static INSTALLED: OnceLock<()> = OnceLock::new();

fn now_ms() -> u64 {
    START_TIME.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

pub fn set_book_open(open: bool) {
    BOOK_OPEN.store(open, Ordering::SeqCst);
}

pub fn set_gameplay(gameplay: bool) {
    GAMEPLAY.store(gameplay, Ordering::SeqCst);
}

/// Controller buttons that went down since the last call.
pub fn take_pressed() -> u16 {
    PRESSED.swap(0, Ordering::SeqCst)
}

/// Controller buttons held now, and the left and right sticks' vertical axes (-1 to 1).
pub fn held() -> (u16, f32, f32) {
    (
        HELD.load(Ordering::SeqCst),
        LEFT_Y.load(Ordering::SeqCst) as f32 / 32767.0,
        RIGHT_Y.load(Ordering::SeqCst) as f32 / 32767.0,
    )
}

/// Whether a controller answered in the last few seconds.
pub fn pad_connected() -> bool {
    let seen = PAD_SEEN.load(Ordering::SeqCst);
    seen != 0 && now_ms() - seen < 3000
}

/// Opens Elden Ring's own map: a View press replayed to the game on the controller, or the map key (G) without one.
pub fn press_map() {
    if pad_connected() {
        INJECT_BACK_UNTIL.store(now_ms() + 150, Ordering::SeqCst);
    } else {
        press_key(0x22, false); // scan code of G
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(120));
            press_key(0x22, true);
        });
    }
}

fn press_key(scan: u16, up: bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
    };
    let flags = KEYEVENTF_SCANCODE | if up { KEYEVENTF_KEYUP } else { 0 };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    };
    unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) };
}

/// Installs the hooks once; returns what was hooked, for the log.
pub fn install() -> Vec<String> {
    let mut report = Vec::new();
    if INSTALLED.set(()).is_err() {
        return report;
    }
    now_ms();
    unsafe {
        let xinput = LoadLibraryW(wide("xinput1_4.dll").as_ptr());
        if xinput.is_null() {
            report.push("xinput1_4.dll not loaded".into());
        } else {
            let by_name = GetProcAddress(xinput, c"XInputGetState".as_ptr().cast());
            let by_ordinal = GetProcAddress(xinput, 100 as *const u8);
            report.push(hook(by_name.map(|f| f as usize), get_state as *const () as usize, 0, "XInputGetState"));
            report.push(hook(by_ordinal.map(|f| f as usize), get_state_ex as *const () as usize, 1, "XInputGetStateEx"));
        }
        for (wide_api, iid, slots, state_fn, data_fn) in [
            (true, 0xBF798031_483A_4DA2_AA99_5D64ED369700u128, (2, 3), di_state_w as *const () as usize, di_data_w as *const () as usize),
            (false, 0xBF798030_483A_4DA2_AA99_5D64ED369700u128, (4, 5), di_state_a as *const () as usize, di_data_a as *const () as usize),
        ] {
            let name = if wide_api { "W" } else { "A" };
            match device_vtable(iid) {
                Ok(vtable) => {
                    let state = *(vtable as *const usize).add(9);
                    let data = *(vtable as *const usize).add(10);
                    report.push(hook(Some(state), state_fn, slots.0, &format!("IDirectInputDevice8{name}::GetDeviceState")));
                    report.push(hook(Some(data), data_fn, slots.1, &format!("IDirectInputDevice8{name}::GetDeviceData")));
                }
                Err(e) => report.push(format!("DirectInput {name}: {e}")),
            }
        }
        if let Err(e) = MH_ApplyQueued().ok_context("MH_ApplyQueued") {
            report.push(format!("{e:?}"));
        }
    }
    report
}

unsafe fn hook(target: Option<usize>, detour: usize, slot: usize, name: &str) -> String {
    let Some(target) = target.filter(|t| *t != 0) else { return format!("{name}: not found") };
    match MhHook::new(target as *mut c_void, detour as *mut c_void) {
        Ok(hook) => {
            TRAMPOLINES[slot].store(hook.trampoline() as usize, Ordering::SeqCst);
            let result = hook.queue_enable();
            std::mem::forget(hook);
            match result {
                Ok(()) => format!("{name}: hooked at {target:#x}"),
                Err(e) => format!("{name}: {e:?}"),
            }
        }
        // The A and W devices can share one function; then the first hook covers both.
        Err(e) => format!("{name}: {e:?} at {target:#x}"),
    }
}

/// The vtable of a DirectInput 8 keyboard device made through the interface `iid` (W or A).
unsafe fn device_vtable(iid: u128) -> Result<usize, String> {
    type Create = unsafe extern "system" fn(*mut c_void, u32, *const GUID, *mut *mut c_void, *mut c_void) -> i32;
    type CreateDevice = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void, *mut c_void) -> i32;
    type Release = unsafe extern "system" fn(*mut c_void) -> u32;
    let dll = LoadLibraryW(wide("dinput8.dll").as_ptr());
    if dll.is_null() {
        return Err("dinput8.dll not loaded".into());
    }
    let create: Create = std::mem::transmute(
        GetProcAddress(dll, c"DirectInput8Create".as_ptr().cast()).ok_or("no DirectInput8Create")?,
    );
    let mut dinput = std::ptr::null_mut();
    let iid = GUID::from_u128(iid);
    let hr = create(GetModuleHandleW(std::ptr::null()) as *mut c_void, 0x0800, &iid, &mut dinput, std::ptr::null_mut());
    if hr < 0 || dinput.is_null() {
        return Err(format!("DirectInput8Create {hr:#x}"));
    }
    let vtable = *(dinput as *const *const usize);
    let create_device: CreateDevice = std::mem::transmute(*vtable.add(3));
    let release: Release = std::mem::transmute(*vtable.add(2));
    let keyboard = GUID::from_u128(0x6F1D2B61_D5A0_11CF_BFC7_444553540000);
    let mut device = std::ptr::null_mut();
    let hr = create_device(dinput, &keyboard, &mut device, std::ptr::null_mut());
    if hr < 0 || device.is_null() {
        release(dinput);
        return Err(format!("CreateDevice {hr:#x}"));
    }
    let device_vtable = *(device as *const usize);
    let device_release: Release = std::mem::transmute(*(device_vtable as *const usize).add(2));
    device_release(device);
    release(dinput);
    Ok(device_vtable)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

unsafe fn trampoline<T: Copy>(slot: usize) -> T {
    std::mem::transmute_copy(&TRAMPOLINES[slot].load(Ordering::SeqCst))
}

unsafe extern "system" fn get_state(user: u32, state: *mut XInputState) -> u32 {
    let result = trampoline::<GetState>(0)(user, state);
    filter_pad(user, result, state);
    result
}

unsafe extern "system" fn get_state_ex(user: u32, state: *mut XInputState) -> u32 {
    let result = trampoline::<GetState>(1)(user, state);
    filter_pad(user, result, state);
    result
}

unsafe fn filter_pad(user: u32, result: u32, state: *mut XInputState) {
    if result != 0 || state.is_null() {
        return;
    }
    let pad = &mut (*state).pad;
    let buttons = pad.buttons;
    if let Some(previous) = PREVIOUS.get(user as usize) {
        let before = previous.swap(buttons, Ordering::SeqCst);
        PRESSED.fetch_or(buttons & !before, Ordering::SeqCst);
    }
    HELD.store(buttons, Ordering::SeqCst);
    LEFT_Y.store(pad.left_y, Ordering::SeqCst);
    RIGHT_Y.store(pad.right_y, Ordering::SeqCst);
    PAD_SEEN.store(now_ms(), Ordering::SeqCst);
    if BOOK_OPEN.load(Ordering::SeqCst) {
        *pad = Gamepad::default();
        return;
    }
    if GAMEPLAY.load(Ordering::SeqCst) {
        pad.buttons &= !BACK;
    }
    if now_ms() < INJECT_BACK_UNTIL.load(Ordering::SeqCst) {
        pad.buttons |= BACK;
    }
}

unsafe fn device_state(slot: usize, this: *mut c_void, size: u32, data: *mut c_void) -> i32 {
    let result = trampoline::<GetDeviceState>(slot)(this, size, data);
    if BOOK_OPEN.load(Ordering::SeqCst) && !data.is_null() {
        std::ptr::write_bytes(data.cast::<u8>(), 0, size as usize);
    }
    result
}

unsafe fn device_data(slot: usize, this: *mut c_void, size: u32, data: *mut c_void, count: *mut u32, flags: u32) -> i32 {
    let result = trampoline::<GetDeviceData>(slot)(this, size, data, count, flags);
    if BOOK_OPEN.load(Ordering::SeqCst) && !count.is_null() {
        *count = 0;
    }
    result
}

unsafe extern "system" fn di_state_w(this: *mut c_void, size: u32, data: *mut c_void) -> i32 {
    device_state(2, this, size, data)
}

unsafe extern "system" fn di_data_w(this: *mut c_void, size: u32, data: *mut c_void, count: *mut u32, flags: u32) -> i32 {
    device_data(3, this, size, data, count, flags)
}

unsafe extern "system" fn di_state_a(this: *mut c_void, size: u32, data: *mut c_void) -> i32 {
    device_state(4, this, size, data)
}

unsafe extern "system" fn di_data_a(this: *mut c_void, size: u32, data: *mut c_void, count: *mut u32, flags: u32) -> i32 {
    device_data(5, this, size, data, count, flags)
}
