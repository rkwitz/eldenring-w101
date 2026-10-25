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
