//! Input for the spellbook. Elden Ring reads the controller through XInput (xinput1_4.dll) and the keyboard and
//! mouse through DirectInput 8, so both are hooked (MinHook, through hudhook):
//! - XInput: the View/Back button (ER's map button) opens the book instead of the map during normal play; while the
//!   book is open the game sees an idle controller and the book reads the real one; `press_map` replays a View
//!   press to the game so that its own map opens.
//! - DirectInput and raw input: while the book is open the game's keyboard and mouse read as idle (Elden Ring turns
//!   its camera from raw mouse input, WM_INPUT / GetRawInputData / GetRawInputBuffer, so those are hooked too). The
//!   mouse's movement and buttons are kept for the book's own cursor (Elden Ring keeps the system cursor locked),
//!   and SetCursorPos / ClipCursor are hooked so that the game doesn't pull the cursor back while the book is open.
//! - Walking with the book open (round 16 M1, `set_move_through`): in normal play, off the map page, the W, A, S and D
//!   keys (DirectInput keyboard state and buffered data) and the controller's left stick still reach the game; the
//!   mouse, the right stick and every other key stay with the book, so the camera doesn't move.
//!
//! Keyboard keys for the book itself (B, Esc, arrows) come through imgui, which hudhook feeds from window messages.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI16, AtomicI32, AtomicU16, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
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
/// The triggers, pressed past half way (XInput reports them as axes; these bits are free in `wButtons`).
pub const LT: u16 = 0x0400;
pub const RT: u16 = 0x0800;

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
/// The left stick's horizontal axis (the map page's cursor).
static LEFT_X: AtomicI16 = AtomicI16::new(0);
/// When a controller last answered, in milliseconds after START_TIME (0 = never).
static PAD_SEEN: AtomicU64 = AtomicU64::new(0);
static PREVIOUS: [AtomicU16; 4] = [AtomicU16::new(0), AtomicU16::new(0), AtomicU16::new(0), AtomicU16::new(0)];

/// The right stick's horizontal axis (-1 to 1), for turning the character preview.
static RIGHT_X: AtomicI16 = AtomicI16::new(0);
/// Raw input packets already counted (hudhook and the game can both read the same one).
static RAW_SEEN: Mutex<[usize; 8]> = Mutex::new([0; 8]);
/// DirectInput mouse movement since the book last asked, the wheel, and the buttons held (bit 0 left, 1 right).
static MOUSE_DX: AtomicI32 = AtomicI32::new(0);
static MOUSE_DY: AtomicI32 = AtomicI32::new(0);
static MOUSE_WHEEL: AtomicI32 = AtomicI32::new(0);
static MOUSE_BUTTONS: AtomicU8 = AtomicU8::new(0);
static MOUSE_SEEN: AtomicU64 = AtomicU64::new(0);
/// DirectInput devices seen in GetDeviceData: (device, DI8DEVTYPE: 0x12 mouse, 0x13 keyboard).
static DEVICES: Mutex<Vec<(usize, u8)>> = Mutex::new(Vec::new());

static TRAMPOLINES: [AtomicUsize; 10] = [
    AtomicUsize::new(0), // XInputGetState
    AtomicUsize::new(0), // XInputGetStateEx (ordinal 100)
    AtomicUsize::new(0), // IDirectInputDevice8W::GetDeviceState
    AtomicUsize::new(0), // IDirectInputDevice8W::GetDeviceData
    AtomicUsize::new(0), // IDirectInputDevice8A::GetDeviceState
    AtomicUsize::new(0), // IDirectInputDevice8A::GetDeviceData
    AtomicUsize::new(0), // SetCursorPos
    AtomicUsize::new(0), // ClipCursor
    AtomicUsize::new(0), // GetRawInputData
    AtomicUsize::new(0), // GetRawInputBuffer
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

/// live_preview.rs: while the book shows Elden Ring's mirror scene, the controller's right stick still reaches the
/// game (its CharaMake menu turns the model with it); every other input stays with the book.
static PASS_RIGHT_STICK: AtomicBool = AtomicBool::new(false);

pub fn pass_right_stick(on: bool) {
    PASS_RIGHT_STICK.store(on, Ordering::SeqCst);
}

/// live_preview.rs: a right stick x (-32767..32767, 0 = none) given to the game while the right stick passes and
/// the real stick is at rest: the book's turning of the live picture (its slow spin, a mouse drag) becomes Elden
/// Ring's CharaMake turning its model by the stick. With `fake_pad`, an idle controller is made up for player 0
/// while no real one is connected, so that this also works without one.
static INJECT_RIGHT_X: AtomicI16 = AtomicI16::new(0);
static FAKE_PAD: AtomicBool = AtomicBool::new(false);
static FAKE_PACKET: AtomicU64 = AtomicU64::new(0);

pub fn inject_right_x(x: f32, fake_pad: bool) {
    INJECT_RIGHT_X.store((x.clamp(-1.0, 1.0) * 32767.0) as i16, Ordering::SeqCst);
    FAKE_PAD.store(fake_pad, Ordering::SeqCst);
}

/// Real right-stick values inside this dead zone count as at rest (the injected turning applies).
const STICK_REST: u16 = 7000;

/// M1: while the book is open in normal play the character still walks: the W, A, S and D keys and the left stick
/// reach the game; the camera stays still (the mouse and the right stick stay with the book). book.rs turns it off
/// on the map page (both sticks move its cursor), in NPC dialogue and while Elden Ring's own menu is up.
static MOVE_THROUGH: AtomicBool = AtomicBool::new(false);

pub fn set_move_through(on: bool) {
    MOVE_THROUGH.store(on, Ordering::SeqCst);
}

/// DirectInput keyboard offsets (DIK_ codes) of W, A, S and D.
const MOVE_KEYS: [usize; 4] = [0x11, 0x1E, 0x1F, 0x20];

fn move_through() -> bool {
    MOVE_THROUGH.load(Ordering::SeqCst) && BOOK_OPEN.load(Ordering::SeqCst)
}

/// Forgets every half-finished input after the game window lost or regained focus (alt-tab): buttons whose release
/// went to another window would otherwise stay held (no click is ever seen again: the book looked frozen), raw input
/// packet handles get reused, and DirectInput devices can be made anew at old addresses.
pub fn reset_after_focus_change() {
    MOUSE_BUTTONS.store(0, Ordering::SeqCst);
    MOUSE_DX.store(0, Ordering::SeqCst);
    MOUSE_DY.store(0, Ordering::SeqCst);
    MOUSE_WHEEL.store(0, Ordering::SeqCst);
    PRESSED.store(0, Ordering::SeqCst);
    HELD.store(0, Ordering::SeqCst);
    for p in &PREVIOUS {
        p.store(0, Ordering::SeqCst);
    }
    INJECT_BACK_UNTIL.store(0, Ordering::SeqCst);
    if let Ok(mut seen) = RAW_SEEN.lock() {
        *seen = [0; 8];
    }
    if let Ok(mut devices) = DEVICES.lock() {
        devices.clear();
    }
}

/// Whether the foreground window belongs to this process (the game has focus).
pub fn game_focused() -> bool {
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, &mut pid);
        pid == GetCurrentProcessId()
    }
}

/// DirectInput mouse movement and wheel since the last call, the buttons held now (bit 0 left, bit 1 right), and
/// whether the game reads a DirectInput mouse at all (recently).
pub fn take_mouse() -> (i32, i32, i32, u8, bool) {
    let seen = MOUSE_SEEN.load(Ordering::SeqCst);
    (
        MOUSE_DX.swap(0, Ordering::SeqCst),
        MOUSE_DY.swap(0, Ordering::SeqCst),
        MOUSE_WHEEL.swap(0, Ordering::SeqCst),
        MOUSE_BUTTONS.load(Ordering::SeqCst),
        seen != 0 && now_ms() - seen < 1000,
    )
}

/// Controller buttons that went down since the last call.
pub fn take_pressed() -> u16 {
    PRESSED.swap(0, Ordering::SeqCst)
}

/// The right stick's horizontal axis now (-1 to 1).
pub fn right_x() -> f32 {
    RIGHT_X.load(Ordering::SeqCst) as f32 / 32767.0
}

/// Controller buttons held now, and the left and right sticks' vertical axes (-1 to 1).
pub fn held() -> (u16, f32, f32) {
    (
        HELD.load(Ordering::SeqCst),
        LEFT_Y.load(Ordering::SeqCst) as f32 / 32767.0,
        RIGHT_Y.load(Ordering::SeqCst) as f32 / 32767.0,
    )
}

/// Both sticks now, -1 to 1: left x, left y, right x, right y (y up).
pub fn sticks() -> [f32; 4] {
    [&LEFT_X, &LEFT_Y, &RIGHT_X, &RIGHT_Y].map(|a| a.load(Ordering::SeqCst) as f32 / 32767.0)
}

/// Whether a controller answered in the last few seconds.
pub fn pad_connected() -> bool {
    let seen = PAD_SEEN.load(Ordering::SeqCst);
    seen != 0 && now_ms() - seen < 3000
}

/// How long a replayed View press is held: a few of the game's frames, short enough that the press that closes
/// the map is over before gameplay comes back (a View still held then opens the map again: the book flickered).
const MAP_PRESS_MS: u64 = 80;

/// Ends a replayed View press now (the map has opened or closed).
pub fn end_map_press() {
    INJECT_BACK_UNTIL.store(0, Ordering::SeqCst);
}

/// Opens Elden Ring's own map: a View press replayed to the game on the controller, or the map key (G) without one.
pub fn press_map() {
    if pad_connected() {
        INJECT_BACK_UNTIL.store(now_ms() + MAP_PRESS_MS, Ordering::SeqCst);
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
        let user32 = LoadLibraryW(wide("user32.dll").as_ptr());
        if !user32.is_null() {
            let set = GetProcAddress(user32, c"SetCursorPos".as_ptr().cast());
            let clip = GetProcAddress(user32, c"ClipCursor".as_ptr().cast());
            report.push(hook(set.map(|f| f as usize), set_cursor_pos as *const () as usize, 6, "SetCursorPos"));
            report.push(hook(clip.map(|f| f as usize), clip_cursor as *const () as usize, 7, "ClipCursor"));
            let data = GetProcAddress(user32, c"GetRawInputData".as_ptr().cast());
            let buffer = GetProcAddress(user32, c"GetRawInputBuffer".as_ptr().cast());
            report.push(hook(data.map(|f| f as usize), raw_input_data as *const () as usize, 8, "GetRawInputData"));
            report.push(hook(buffer.map(|f| f as usize), raw_input_buffer as *const () as usize, 9, "GetRawInputBuffer"));
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
    let result = fake_pad(user, trampoline::<GetState>(0)(user, state), state);
    filter_pad(user, result, state);
    result
}

unsafe extern "system" fn get_state_ex(user: u32, state: *mut XInputState) -> u32 {
    let result = fake_pad(user, trampoline::<GetState>(1)(user, state), state);
    filter_pad(user, result, state);
    result
}

/// No controller on player 0 while the live picture is turned by an injected stick (and the ini allows it): an
/// idle one answers instead, whose right stick `filter_pad` then sets.
unsafe fn fake_pad(user: u32, result: u32, state: *mut XInputState) -> u32 {
    if result == 0 || user != 0 || state.is_null() || !FAKE_PAD.load(Ordering::SeqCst)
        || INJECT_RIGHT_X.load(Ordering::SeqCst) == 0 || !PASS_RIGHT_STICK.load(Ordering::SeqCst)
        || !BOOK_OPEN.load(Ordering::SeqCst) {
        return result;
    }
    (*state).packet = FAKE_PACKET.fetch_add(1, Ordering::SeqCst) as u32;
    (*state).pad = Gamepad::default();
    FAKED.store(true, Ordering::SeqCst);
    0
}

/// The state filter_pad sees this call is the made-up controller (it doesn't count as a connected one).
static FAKED: AtomicBool = AtomicBool::new(false);

unsafe fn filter_pad(user: u32, result: u32, state: *mut XInputState) {
    if result != 0 || state.is_null() {
        return;
    }
    let pad = &mut (*state).pad;
    let mut buttons = pad.buttons & !(LT | RT);
    if pad.left_trigger > 128 {
        buttons |= LT;
    }
    if pad.right_trigger > 128 {
        buttons |= RT;
    }
    if let Some(previous) = PREVIOUS.get(user as usize) {
        let before = previous.swap(buttons, Ordering::SeqCst);
        PRESSED.fetch_or(buttons & !before, Ordering::SeqCst);
    }
    HELD.store(buttons, Ordering::SeqCst);
    LEFT_Y.store(pad.left_y, Ordering::SeqCst);
    LEFT_X.store(pad.left_x, Ordering::SeqCst);
    RIGHT_Y.store(pad.right_y, Ordering::SeqCst);
    RIGHT_X.store(pad.right_x, Ordering::SeqCst);
    if !FAKED.swap(false, Ordering::SeqCst) {
        PAD_SEEN.store(now_ms(), Ordering::SeqCst);
    }
    if BOOK_OPEN.load(Ordering::SeqCst) {
        let (right_x, right_y, left_x, left_y) = (pad.right_x, pad.right_y, pad.left_x, pad.left_y);
        *pad = Gamepad::default();
        if PASS_RIGHT_STICK.load(Ordering::SeqCst) {
            let injected = INJECT_RIGHT_X.load(Ordering::SeqCst);
            let at_rest = right_x.unsigned_abs() < STICK_REST && right_y.unsigned_abs() < STICK_REST;
            (pad.right_x, pad.right_y) = if at_rest && injected != 0 { (injected, 0) } else { (right_x, right_y) };
        }
        if move_through() {
            (pad.left_x, pad.left_y) = (left_x, left_y);
        }
        return;
    }
    if GAMEPLAY.load(Ordering::SeqCst) {
        pad.buttons &= !BACK;
    }
    if now_ms() < INJECT_BACK_UNTIL.load(Ordering::SeqCst) {
        pad.buttons |= BACK;
    }
}

/// DIMOUSESTATE / DIMOUSESTATE2: lX, lY, lZ, then 4 / 8 button bytes (0x80 = down).
unsafe fn device_state(slot: usize, this: *mut c_void, size: u32, data: *mut c_void) -> i32 {
    let result = trampoline::<GetDeviceState>(slot)(this, size, data);
    if data.is_null() || result < 0 {
        return result;
    }
    let mouse = size == 16 || size == 20;
    if mouse {
        let state = data.cast::<i32>();
        MOUSE_DX.fetch_add(*state, Ordering::SeqCst);
        MOUSE_DY.fetch_add(*state.add(1), Ordering::SeqCst);
        MOUSE_WHEEL.fetch_add(*state.add(2), Ordering::SeqCst);
        let buttons = data.cast::<u8>().add(12);
        MOUSE_BUTTONS.store(((*buttons & 0x80 != 0) as u8) | (((*buttons.add(1) & 0x80 != 0) as u8) << 1), Ordering::SeqCst);
        MOUSE_SEEN.store(now_ms(), Ordering::SeqCst);
    }
    if BOOK_OPEN.load(Ordering::SeqCst) {
        // A keyboard's state (256 key bytes) keeps W, A, S and D while the character may walk (M1).
        let keep = (size == 256 && move_through()).then(|| MOVE_KEYS.map(|k| *data.cast::<u8>().add(k)));
        std::ptr::write_bytes(data.cast::<u8>(), 0, size as usize);
        if let Some(keys) = keep {
            for (k, v) in MOVE_KEYS.iter().zip(keys) {
                *data.cast::<u8>().add(*k) = v;
            }
        }
    }
    result
}

/// Whether a DirectInput device is a mouse (GetCapabilities' device type), remembered per device.
unsafe fn is_mouse(this: *mut c_void) -> bool {
    device_type(this) == 0x12 // DI8DEVTYPE_MOUSE
}

/// A DirectInput device's type (GetCapabilities' dwDevType & 0xFF: 0x12 mouse, 0x13 keyboard), remembered per device.
unsafe fn device_type(this: *mut c_void) -> u8 {
    let Ok(mut devices) = DEVICES.lock() else { return 0 };
    if let Some((_, kind)) = devices.iter().find(|(d, _)| *d == this as usize) {
        return *kind;
    }
    type GetCapabilities = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
    let vtable = *(this as *const *const usize);
    let get: GetCapabilities = std::mem::transmute(*vtable.add(3));
    let mut caps = [0u32; 11];
    caps[0] = 44; // sizeof(DIDEVCAPS)
    let kind = if get(this, caps.as_mut_ptr()) >= 0 { (caps[2] & 0xFF) as u8 } else { 0 };
    if devices.len() < 64 {
        devices.push((this as usize, kind));
    }
    kind
}

/// Buffered data: DIDEVICEOBJECTDATA entries of `size` bytes (offset, data, time, sequence, app data); for a mouse
/// offset 0/4/8 = X/Y/wheel movement, 12/13 = left/right button (0x80 = down).
unsafe fn device_data(slot: usize, this: *mut c_void, size: u32, data: *mut c_void, count: *mut u32, flags: u32) -> i32 {
    let result = trampoline::<GetDeviceData>(slot)(this, size, data, count, flags);
    if result < 0 || count.is_null() {
        return result;
    }
    let mouse = !data.is_null() && size >= 16 && *count > 0 && is_mouse(this);
    if mouse {
        MOUSE_SEEN.store(now_ms(), Ordering::SeqCst);
        for i in 0..*count as usize {
            let entry = data.cast::<u8>().add(i * size as usize).cast::<u32>();
            let (offset, value) = (*entry, *entry.add(1));
            match offset {
                0 => { MOUSE_DX.fetch_add(value as i32, Ordering::SeqCst); }
                4 => { MOUSE_DY.fetch_add(value as i32, Ordering::SeqCst); }
                8 => { MOUSE_WHEEL.fetch_add(value as i32, Ordering::SeqCst); }
                12 | 13 => {
                    let bit = 1u8 << (offset - 12);
                    if value & 0x80 != 0 {
                        MOUSE_BUTTONS.fetch_or(bit, Ordering::SeqCst);
                    } else {
                        MOUSE_BUTTONS.fetch_and(!bit, Ordering::SeqCst);
                    }
                }
                _ => {}
            }
        }
    }
    if BOOK_OPEN.load(Ordering::SeqCst) {
        if !data.is_null() && size >= 16 && *count > 0 && move_through() && device_type(this) == 0x13 {
            // A keyboard's buffered keys: only W, A, S and D stay (M1), packed to the front.
            let mut kept = 0usize;
            for i in 0..*count as usize {
                let entry = data.cast::<u8>().add(i * size as usize);
                if MOVE_KEYS.contains(&(*entry.cast::<u32>() as usize)) {
                    if kept != i {
                        std::ptr::copy(entry, data.cast::<u8>().add(kept * size as usize), size as usize);
                    }
                    kept += 1;
                }
            }
            *count = kept as u32;
        } else {
            *count = 0;
        }
    }
    result
}

unsafe extern "system" fn set_cursor_pos(x: i32, y: i32) -> i32 {
    if BOOK_OPEN.load(Ordering::SeqCst) {
        return 1;
    }
    trampoline::<unsafe extern "system" fn(i32, i32) -> i32>(6)(x, y)
}

unsafe extern "system" fn clip_cursor(rect: *const c_void) -> i32 {
    let rect = if BOOK_OPEN.load(Ordering::SeqCst) { std::ptr::null() } else { rect };
    trampoline::<unsafe extern "system" fn(*const c_void) -> i32>(7)(rect)
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

/// RAWINPUT on 64-bit Windows: a 24-byte header (type, size, device, wParam), then for a mouse RAWMOUSE: flags (u16),
/// pad, button flags (u16), button data (u16), raw buttons (u32), last X (i32), last Y (i32), extra (u32).
const RIM_TYPEMOUSE: u32 = 0;
const RAW_HEADER: usize = 24;

/// Takes a raw mouse packet's movement, wheel and buttons for the book's cursor (once per packet), and blanks it for
/// the game while the book is open.
unsafe fn raw_mouse(packet: *mut u8, size: usize, key: usize) {
    if packet.is_null() || size < RAW_HEADER + 24 || *(packet as *const u32) != RIM_TYPEMOUSE {
        return;
    }
    let mouse = packet.add(RAW_HEADER);
    let flags = *(mouse as *const u16);
    let buttons = *(mouse.add(4) as *const u16);
    let wheel = *(mouse.add(6) as *const i16);
    let (dx, dy) = (*(mouse.add(12) as *const i32), *(mouse.add(16) as *const i32));
    let new = match RAW_SEEN.lock() {
        Ok(mut seen) if key != 0 => {
            if seen.contains(&key) {
                false
            } else {
                seen.rotate_right(1);
                seen[0] = key;
                true
            }
        }
        _ => true,
    };
    if new {
        MOUSE_SEEN.store(now_ms(), Ordering::SeqCst);
        if flags & 1 == 0 {
            // MOUSE_MOVE_RELATIVE
            MOUSE_DX.fetch_add(dx, Ordering::SeqCst);
            MOUSE_DY.fetch_add(dy, Ordering::SeqCst);
        }
        if buttons & 0x0400 != 0 {
            MOUSE_WHEEL.fetch_add(wheel as i32, Ordering::SeqCst);
        }
        for (down, up, bit) in [(0x0001u16, 0x0002u16, 1u8), (0x0004, 0x0008, 2)] {
            if buttons & down != 0 {
                MOUSE_BUTTONS.fetch_or(bit, Ordering::SeqCst);
            }
            if buttons & up != 0 {
                MOUSE_BUTTONS.fetch_and(!bit, Ordering::SeqCst);
            }
        }
    }
    if BOOK_OPEN.load(Ordering::SeqCst) {
        // No movement, buttons or wheel for the game.
        std::ptr::write_bytes(mouse.add(2), 0, 22);
    }
}

unsafe extern "system" fn raw_input_data(input: *mut c_void, command: u32, data: *mut c_void, size: *mut u32,
                                         header: u32) -> u32 {
    type Fn = unsafe extern "system" fn(*mut c_void, u32, *mut c_void, *mut u32, u32) -> u32;
    let result = trampoline::<Fn>(8)(input, command, data, size, header);
    // RID_INPUT with a buffer: the whole packet was copied.
    if command == 0x1000_0003 && !data.is_null() && result != u32::MAX && result as usize >= RAW_HEADER {
        raw_mouse(data.cast(), result as usize, input as usize);
    }
    result
}

unsafe extern "system" fn raw_input_buffer(data: *mut c_void, size: *mut u32, header: u32) -> u32 {
    type Fn = unsafe extern "system" fn(*mut c_void, *mut u32, u32) -> u32;
    let count = trampoline::<Fn>(9)(data, size, header);
    if data.is_null() || count == u32::MAX {
        return count;
    }
    let mut at = data.cast::<u8>();
    for _ in 0..count {
        let packet_size = *(at.add(4) as *const u32) as usize;
        raw_mouse(at, packet_size, 0);
        // NEXTRAWINPUTBLOCK: aligned to 8 bytes.
        at = at.add((packet_size + 7) & !7);
    }
    count
}
