//! Stand-in for the parts of hudhook the spellbook uses, for the Linux preview.
pub use imgui;

pub trait RenderContext {
    fn load_texture(&mut self, data: &[u8], width: u32, height: u32) -> Result<imgui::TextureId, String>;
    fn replace_texture(&mut self, texture_id: imgui::TextureId, data: &[u8], width: u32, height: u32)
        -> Result<(), String>;
    /// As the vendored hudhook's: a copy of the game's frame.
    fn capture_screen(&mut self, _slot: usize) -> Option<imgui::TextureId> {
        None
    }
}

/// As the vendored hudhook's: a draw-list callback whose user data switches to additive / normal blending (the
/// preview's rasterizer recognises it).
pub unsafe extern "C" fn blend_callback(_list: *const imgui::sys::ImDrawList, _cmd: *const imgui::sys::ImDrawCmd) {}
pub const BLEND_ADDITIVE: usize = 0x5710_1AD0;
pub const BLEND_NORMAL: usize = 0x5710_1A10;
pub const BLEND_KEY: usize = 0x5710_1AE7;

/// As the vendored hudhook's depth module (round 26 O1): the preview has no game depth.
pub mod depth {
    pub const DEPTH_TAG_HI: usize = 0x5710_D0E9;
    pub const DEPTH_OFF: usize = 0x5710_D0FF;
    pub fn tag(z: f32) -> usize {
        (DEPTH_TAG_HI << 32) | z.to_bits() as usize
    }
    pub fn set_projection(_near: f32, _far: f32, _reversed: bool) {}
    pub fn set_wanted(_on: bool) {}
    pub fn linear_with(d: f32, n: f32, _f: f32, _reversed: bool) -> f32 {
        n / d.max(1.0e-9)
    }
    #[derive(Clone, Debug, Default)]
    pub struct Info {
        pub fresh: bool,
        pub resource: usize,
        pub width: u64,
        pub height: u32,
        pub format: u32,
        pub generation: u32,
        pub copies: u64,
        pub frames: u64,
        pub barrier_calls: u64,
        pub depth_ends: u64,
        pub candidates: usize,
        pub after_state: u32,
    }
    pub fn info() -> Info {
        Info::default()
    }
    pub struct Probe {
        pub id: u64,
        pub points: Vec<[f32; 2]>,
        pub depth: Vec<f32>,
    }
    pub fn probe(_id: u64, _points: Vec<[f32; 2]>) {}
    pub fn take_probe() -> Option<Probe> {
        None
    }
    pub fn reject_current() {}
}
