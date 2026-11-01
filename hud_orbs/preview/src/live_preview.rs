//! Stand-in for the DLL's live_preview.rs (the live mirror picture needs the game): the book reports its pictures.
//! With PREVIEW_LIVE_MIRROR=<png> (a probe dump of Elden Ring's mirror picture, e.g. second_scene_research/run2)
//! main.rs puts that picture in the book's character pictures, cropped and keyed as the DLL does (round 18 A3).

use std::sync::Mutex;

/// The square picture covers this much (centre height, view height in metres), as the DLL's MIRROR_FRAME.
const MIRROR_FRAME: [f32; 2] = [0.9, 1.92];
const PORTRAIT_VIEW: [f32; 2] = [1.66, 0.62];

static BOX: Mutex<Option<([u32; 2], [f32; 2])>> = Mutex::new(None);
static FACE: Mutex<Option<[u32; 2]>> = Mutex::new(None);

pub fn mirror() -> bool {
    std::env::var_os("PREVIEW_LIVE_MIRROR").is_some()
}

pub fn note_box(size: [u32; 2], view: [f32; 2], _turn: f32) {
    *BOX.lock().unwrap() = Some((size, view));
}

pub fn note_portrait(size: [u32; 2]) {
    *FACE.lock().unwrap() = Some(size);
}

pub fn auto_spin() -> bool {
    true
}

pub fn walk_under_mirror() -> bool {
    false
}

fn crop(frame: [f32; 2], view: [f32; 2], size: [u32; 2]) -> [f32; 4] {
    let h = (view[1] / frame[1]).clamp(0.05, 1.0);
    let w = (h * size[0] as f32 / size[1].max(1) as f32).min(1.0);
    let v0 = (0.5 - (view[0] + view[1] / 2.0 - frame[0]) / frame[1]).clamp(0.0, 1.0 - h);
    let u0 = 0.5 - w / 2.0;
    [u0, v0, u0 + w, v0 + h]
}

/// The live picture only shows with PREVIEW_LIVE_MIRROR; then cropped like the DLL's.
pub fn image_strips(name: &str) -> Option<Vec<([f32; 2], [f32; 4])>> {
    if !mirror() {
        return None;
    }
    let uv = match name {
        "@model/0" => {
            let (size, view) = (*BOX.lock().unwrap())?;
            crop(MIRROR_FRAME, view, size)
        }
        "@model/1" => crop(MIRROR_FRAME, PORTRAIT_VIEW, (*FACE.lock().unwrap())?),
        _ => return None,
    };
    Some(vec![([0.0, 1.0], uv)])
}

/// As the DLL's: the live picture is drawn with its flat background keyed out.
pub fn key_blend(on: bool) {
    use hudhook::imgui::sys;
    let mode = if on { hudhook::BLEND_KEY } else { hudhook::BLEND_NORMAL };
    unsafe { sys::ImDrawList_AddCallback(sys::igGetForegroundDrawList(), Some(hudhook::blend_callback), mode as *mut _) };
}

pub fn note_book(_keep: bool) {}
