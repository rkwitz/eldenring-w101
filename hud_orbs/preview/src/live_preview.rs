//! Stand-in for the DLL's live_preview.rs (the live mirror picture needs the game): the book reports its pictures.

pub fn note_box(_size: [u32; 2], _view: [f32; 2], _turn: f32) {}

pub fn note_portrait(_size: [u32; 2]) {}

/// The live picture never shows in the preview, so it is never cropped and never breathes.
pub fn image_strips(_name: &str) -> Option<Vec<([f32; 2], [f32; 4])>> {
    None
}

pub fn note_book(_keep: bool) {}
