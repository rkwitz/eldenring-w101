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
