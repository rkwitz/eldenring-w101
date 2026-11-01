//! Stand-in for the DLL's input.rs (the hooks need Windows): same API, no input.
#![allow(dead_code)]

pub const DPAD_UP: u16 = 0x0001;
pub const DPAD_DOWN: u16 = 0x0002;
pub const DPAD_LEFT: u16 = 0x0004;
pub const DPAD_RIGHT: u16 = 0x0008;
pub const BACK: u16 = 0x0020;
pub const LB: u16 = 0x0100;
pub const RB: u16 = 0x0200;
pub const LT: u16 = 0x0400;
pub const RT: u16 = 0x0800;
pub const A: u16 = 0x1000;
pub const B: u16 = 0x2000;
pub const X: u16 = 0x4000;
pub const Y: u16 = 0x8000;

pub fn set_book_open(_open: bool) {}
pub fn set_gameplay(_gameplay: bool) {}
pub fn set_move_through(_on: bool) {}
pub fn take_pressed() -> u16 { 0 }
pub fn held() -> (u16, f32, f32) { (0, 0.0, 0.0) }
pub fn pad_connected() -> bool { std::env::var("PREVIEW_PAD").is_ok() }
pub fn press_map() {}
pub fn right_x() -> f32 { 0.0 }
pub fn sticks() -> [f32; 4] { [0.0; 4] }
pub fn take_mouse() -> (i32, i32, i32, u8, bool) { (0, 0, 0, 0, false) }
pub fn end_map_press() {}
