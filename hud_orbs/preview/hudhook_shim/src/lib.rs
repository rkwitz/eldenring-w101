//! Stand-in for the parts of hudhook the spellbook uses, for the Linux preview.
pub use imgui;

pub trait RenderContext {
    fn load_texture(&mut self, data: &[u8], width: u32, height: u32) -> Result<imgui::TextureId, String>;
}
