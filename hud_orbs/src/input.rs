//! Input for the spellbook. Elden Ring reads the controller through XInput (xinput1_4.dll) and the keyboard and
//! mouse through DirectInput 8, so both are hooked (MinHook, through hudhook):
//! - XInput: the View/Back button (ER's map button) opens the book instead of the map during normal play; while the
//!   book is open the game sees an idle controller and the book reads the real one; `press_map` replays a View
//!   press to the game so that its own map opens.
//! - DirectInput: while the book is open the game's keyboard and mouse devices read as idle. The mouse's movement
//!   and buttons are kept for the book's own cursor (Elden Ring keeps the system cursor locked), and
//!   SetCursorPos / ClipCursor are hooked so that the game doesn't pull the cursor back while the book is open.
//! - Turning the character in the book: while `set_camera_pass` is on, the right stick's X axis (and mouse movement
//!   asked for with `turn_camera`) still reach the game, which turns its camera around the player.
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
/// When a controller last answered, in milliseconds after START_TIME (0 = never).
static PAD_SEEN: AtomicU64 = AtomicU64::new(0);
static PREVIOUS: [AtomicU16; 4] = [AtomicU16::new(0), AtomicU16::new(0), AtomicU16::new(0), AtomicU16::new(0)];

/// The game's camera input passes through while the book is open (turning the character).
static CAMERA_PASS: AtomicBool = AtomicBool::new(false);
/// Camera turn asked for by the book: right stick X for the controller, mouse X counts for the mouse.
static TURN_STICK: AtomicI16 = AtomicI16::new(0);
static TURN_MOUSE: AtomicI32 = AtomicI32::new(0);
/// DirectInput mouse movement since the book last asked, the wheel, and the buttons held (bit 0 left, 1 right).
static MOUSE_DX: AtomicI32 = AtomicI32::new(0);
static MOUSE_DY: AtomicI32 = AtomicI32::new(0);
static MOUSE_WHEEL: AtomicI32 = AtomicI32::new(0);
static MOUSE_BUTTONS: AtomicU8 = AtomicU8::new(0);
static MOUSE_SEEN: AtomicU64 = AtomicU64::new(0);
/// DirectInput devices seen in GetDeviceData: (device, is a mouse).
static DEVICES: Mutex<Vec<(usize, bool)>> = Mutex::new(Vec::new());

static TRAMPOLINES: [AtomicUsize; 8] = [
    AtomicUsize::new(0), // XInputGetState
    AtomicUsize::new(0), // XInputGetStateEx (ordinal 100)
    AtomicUsize::new(0), // IDirectInputDevice8W::GetDeviceState
    AtomicUsize::new(0), // IDirectInputDevice8W::GetDeviceData
    AtomicUsize::new(0), // IDirectInputDevice8A::GetDeviceState
    AtomicUsize::new(0), // IDirectInputDevice8A::GetDeviceData
    AtomicUsize::new(0), // SetCursorPos
    AtomicUsize::new(0), // ClipCursor
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

/// Lets the game's camera input through while the book is open (the character view).
pub fn set_camera_pass(on: bool) {
    CAMERA_PASS.store(on, Ordering::SeqCst);
    if !on {
        TURN_STICK.store(0, Ordering::SeqCst);
        TURN_MOUSE.store(0, Ordering::SeqCst);
    }
}

/// Turns the game camera while camera input passes: `stick` -1..1 on the right stick (held until changed), plus
/// `mouse` counts of mouse movement once.
pub fn turn_camera(stick: f32, mouse: i32) {
    TURN_STICK.store((stick.clamp(-1.0, 1.0) * 32767.0) as i16, Ordering::SeqCst);
    TURN_MOUSE.fetch_add(mouse, Ordering::SeqCst);
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
        let user32 = LoadLibraryW(wide("user32.dll").as_ptr());
        if !user32.is_null() {
            let set = GetProcAddress(user32, c"SetCursorPos".as_ptr().cast());
            let clip = GetProcAddress(user32, c"ClipCursor".as_ptr().cast());
            report.push(hook(set.map(|f| f as usize), set_cursor_pos as *const () as usize, 6, "SetCursorPos"));
            report.push(hook(clip.map(|f| f as usize), clip_cursor as *const () as usize, 7, "ClipCursor"));
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
    RIGHT_Y.store(pad.right_y, Ordering::SeqCst);
    PAD_SEEN.store(now_ms(), Ordering::SeqCst);
    if BOOK_OPEN.load(Ordering::SeqCst) {
        let right_x = pad.right_x;
        *pad = Gamepad::default();
        if CAMERA_PASS.load(Ordering::SeqCst) {
            let turn = TURN_STICK.load(Ordering::SeqCst);
            pad.right_x = if turn != 0 { turn } else { right_x };
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
        std::ptr::write_bytes(data.cast::<u8>(), 0, size as usize);
        if mouse && CAMERA_PASS.load(Ordering::SeqCst) {
            *data.cast::<i32>() = TURN_MOUSE.swap(0, Ordering::SeqCst);
        }
    }
    result
}

/// Whether a DirectInput device is a mouse (GetCapabilities' device type), remembered per device.
unsafe fn is_mouse(this: *mut c_void) -> bool {
    let Ok(mut devices) = DEVICES.lock() else { return false };
    if let Some((_, mouse)) = devices.iter().find(|(d, _)| *d == this as usize) {
        return *mouse;
    }
    type GetCapabilities = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
    let vtable = *(this as *const *const usize);
    let get: GetCapabilities = std::mem::transmute(*vtable.add(3));
    let mut caps = [0u32; 11];
    caps[0] = 44; // sizeof(DIDEVCAPS)
    let mouse = get(this, caps.as_mut_ptr()) >= 0 && caps[2] & 0xFF == 0x12; // DI8DEVTYPE_MOUSE
    if devices.len() < 64 {
        devices.push((this as usize, mouse));
    }
    mouse
}

/// Buffered data: DIDEVICEOBJECTDATA entries of `size` bytes (offset, data, time, sequence, app data); for a mouse
/// offset 0/4/8 = X/Y/wheel movement, 12/13 = left/right button (0x80 = down).
unsafe fn device_data(slot: usize, this: *mut c_void, size: u32, data: *mut c_void, count: *mut u32, flags: u32) -> i32 {
    let capacity = if count.is_null() { 0 } else { *count };
    let result = trampoline::<GetDeviceData>(slot)(this, size, data, count, flags);
    if result < 0 || count.is_null() {
        return result;
    }
    if !data.is_null() && size >= 16 && *count > 0 && is_mouse(this) {
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
        *count = 0;
        // Turning the camera: one X movement entry, if the game reads its mouse buffered.
        if CAMERA_PASS.load(Ordering::SeqCst) && !data.is_null() && size >= 16 && capacity > 0 && flags == 0
            && is_mouse(this)
        {
            let turn = TURN_MOUSE.swap(0, Ordering::SeqCst);
            if turn != 0 {
                std::ptr::write_bytes(data.cast::<u8>(), 0, size as usize);
                *data.cast::<i32>().add(1) = turn;
                *count = 1;
            }
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
