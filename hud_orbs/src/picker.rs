//! Round 25 (H2, Ryan: "Multiple quests from one NPC: W101 shows a quest-picker list on a parchment"): Wizard101's
//! NPC services window, GUI/NPCServices.gui (its rows GUI/NPCServicesOptionWindow.gui), shown when one NPC's talk
//! events ask for more than one conversation at once.
//!
//! NPCServices.gui (tools/w101_book_layout.py --pages=NPCServices,NPCServicesOptionWindow): a 384 x 382 parchment
//! picture GUI/Backgrounds/Background_Pre.dds (the stars drawing), HCENTER, 22 down; its optionsLayout (32, 32 .. 369,
//! 348 in it) stacks NPCServicesOptionWindow rows (307 x 49: OptionImage 50 x 50 at 2,0, the quest's state icon;
//! OptionButton GUI/Buttons/Button_Red02.dds at 54,9 .. 309,41 with the quest's name in WizardFancy); and its
//! wndDialogMain is DialogWindow's box with txtMessage GUI_00000706 "What can I do for you?" and the Exit button
//! GUI_00000708 "Cancel" (Button_Red01, yellow label). Icons (WizardGraphicalClient's NPCServices strings):
//! GUI/QuestButtons/Art_Quest_Start ("!", a quest to take), Art_Quest_Complete ("?", a goal or turn-in at this NPC),
//! Art_Quest_Unfinished (grey "?", in progress). Measured on Ryan's screenshot ref/2_w101_multi_quest_picker.png: the
//! parchment's top 31 layout pixels down (PARCHMENT_TOP), the rows 52 apart (ROW_PITCH), the labels about 0.44 of
//! WizardFancy (LABEL_SCALE), yellow, white under the cursor.
//!
//! Here the dialogue box is the same DialogWindow the conversations use (talk.rs: placed over the experience bar),
//! its btnRight labelled Cancel. Picking a row opens that conversation; the others stay asked for and come back
//! (picker again while two or more are left). Cancel (or B / Esc) answers all of them "cancel".

use crate::layout::{Layout, View};

pub const PARCHMENT: &str = "GUI/Backgrounds/Background_Pre.png";
const BAR: &str = "GUI/Buttons/Button_Red02.png";
pub const ICON_OFFER: &str = "GUI/QuestButtons/Art_Quest_Start.png";
pub const ICON_TALK: &str = "GUI/QuestButtons/Art_Quest_Complete.png";
pub const ICON_UNDERWAY: &str = "GUI/QuestButtons/Art_Quest_Unfinished.png";
/// GUI.lang GUI_00000706 and GUI_00000708.
pub const MESSAGE: &str = "What can I do for you?";
pub const CANCEL: &str = "Cancel";

/// Layout pixels (768-high screen).
const PARCHMENT_SIZE: [f32; 2] = [384.0, 382.0];
const PARCHMENT_TOP: f32 = 31.0;
const ROWS_AT: [f32; 2] = [32.0, 32.0];
const ROW_PITCH: f32 = 52.0;
const ICON_RECT: [f32; 4] = [2.0, 0.0, 52.0, 50.0];
const BAR_RECT: [f32; 4] = [54.0, 9.0, 309.0, 41.0];
const LABEL_SCALE: f32 = 0.44;
const MAX_ROWS: usize = 6;
const YELLOW: [u8; 3] = [255, 255, 0];

/// One row: the request it opens, the icon and the quest's name.
#[derive(Clone, Debug)]
pub struct Row {
    pub request: usize,
    pub icon: &'static str,
    pub label: String,
}

/// The picker on screen: whose (name, portrait), its rows, the row picked with the controller / keys.
pub struct Picker {
    pub speaker: String,
    pub name: String,
    pub portrait: String,
    pub rows: Vec<Row>,
    pub focus: usize,
    pub opened: std::time::Instant,
}

/// The icon for a conversation kind (dialogue.tsv R lines: offer, goal<n>, done, underway, ...).
pub fn icon(kind: &str) -> &'static str {
    match kind {
        "offer" => ICON_OFFER,
        "underway" | "accepted" => ICON_UNDERWAY,
        _ => ICON_TALK,
    }
}

/// The parchment's rectangle and each row's (icon, bar) rectangles on a `screen` (pixels).
pub fn rects(screen: [f32; 2], rows: usize) -> ([f32; 4], Vec<([f32; 4], [f32; 4])>) {
    let s = screen[1] / 768.0;
    let x0 = (screen[0] - PARCHMENT_SIZE[0] * s) / 2.0;
    let y0 = PARCHMENT_TOP * s;
    let parchment = [x0, y0, x0 + PARCHMENT_SIZE[0] * s, y0 + PARCHMENT_SIZE[1] * s];
    let at = |r: [f32; 4], i: usize| {
        let (ox, oy) = (x0 + ROWS_AT[0] * s, y0 + (ROWS_AT[1] + ROW_PITCH * i as f32) * s);
        [ox + r[0] * s, oy + r[1] * s, ox + r[2] * s, oy + r[3] * s]
    };
    (parchment, (0..rows.min(MAX_ROWS)).map(|i| (at(ICON_RECT, i), at(BAR_RECT, i))).collect())
}

/// The art the picker draws (besides DialogWindow's).
pub fn art() -> Vec<String> {
    [PARCHMENT, BAR, ICON_OFFER, ICON_TALK, ICON_UNDERWAY].map(String::from).to_vec()
}

/// Draws the parchment and rows; returns the row under the mouse.
pub fn draw(draw: &hudhook::imgui::DrawListMut, textures: &crate::layout::Textures,
            fonts: &std::collections::HashMap<String, crate::font::BitmapFont>, screen: [f32; 2], picker: &Picker,
            mouse: [f32; 2], alpha: f32) -> Option<usize> {
    use hudhook::imgui::ImColor32;
    let s = screen[1] / 768.0;
    let (parchment, rows) = rects(screen, picker.rows.len());
    let white = [1.0, 1.0, 1.0, alpha];
    if let Some((t, _)) = textures.get(PARCHMENT) {
        draw.add_image(t, [parchment[0], parchment[1]], [parchment[2], parchment[3]]).col(white).build();
    }
    let inside = |r: [f32; 4]| mouse[0] >= r[0] && mouse[0] < r[2] && mouse[1] >= r[1] && mouse[1] < r[3];
    let mut hover = None;
    for (i, (row, (icon, bar))) in picker.rows.iter().zip(&rows).enumerate() {
        let lit = inside(*bar) || inside(*icon);
        if lit {
            hover = Some(i);
        }
        let on = lit || (hover.is_none() && i == picker.focus && mouse[0] < 0.0);
        if let Some((t, _)) = textures.get(row.icon) {
            draw.add_image(t, [icon[0], icon[1]], [icon[2], icon[3]]).col(white).build();
        }
        // Button_Red02's four frames (normal, highlighted, pressed, greyed) one under another.
        if let Some((t, _)) = textures.get(BAR) {
            let v = if on { 0.25 } else { 0.0 };
            draw.add_image(t, [bar[0], bar[1]], [bar[2], bar[3]]).uv_min([0.0, v]).uv_max([1.0, v + 0.25]).col(white)
                .build();
        }
        if let Some(font) = fonts.get("WizardFancy") {
            let k = LABEL_SCALE * s;
            let w = font.width(&row.label) * k;
            let c = if on { [255, 255, 255] } else { YELLOW };
            let colour = ImColor32::from_rgba(c[0], c[1], c[2], (255.0 * alpha) as u8);
            font.draw_at(draw, &row.label, [(bar[0] + bar[2] - w) / 2.0,
                         (bar[1] + bar[3]) / 2.0 - font.line_height * k / 2.0], k, colour);
        }
    }
    hover
}

/// DialogWindow's view for the picker: the NPC's portrait and name, "What can I do for you?", btnRight = Cancel.
pub fn dialog_view(window: &Layout, picker: &Picker) -> View {
    let mut v = View::default();
    if let Some(id) = window.find("imgPortrait") {
        v.node_art.insert(id, picker.portrait.clone());
    }
    v.set(window.find("txtTitle"), format!("<center>{}</center>", picker.name));
    if let Some(id) = window.find("txtMessage") {
        v.text.insert(id, MESSAGE.to_string());
        if let Some(mut style) = window.nodes[id].text_style.clone() {
            style.middle = true;
            style.wrap = true;
            v.style.insert(id, style);
        }
    }
    for name in ["btnLeft", "MarkButton", "btnBack", "btnYes", "btnNo"] {
        v.show(window.find(name), false);
    }
    if let Some(id) = window.find("btnRight") {
        v.text.insert(id, format!("<center>{CANCEL}</center>"));
        if let Some(style) = window.nodes[id].text_style.clone() {
            v.style.insert(id, style.centred());
        }
    }
    v
}
