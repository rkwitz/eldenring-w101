//! Wizard101's NPC dialogue box and quest offers (round 17, F2/F3), in place of Elden Ring's generic dialog.
//!
//! The map's talk events (tools/emevd_quests.py) ask for a conversation by turning on its request flag
//! (1042362400 + k, w101_quests.dialogue_keys) and wait. The HUD shows that conversation in DialogWindow.gui
//! (Background_Dialog, the speaker's NpcPortraits picture on the left, the name in WizardFancy over the text, the
//! red NEXT / BACK / DONE buttons and the close X), and answers with DIALOG_OK (read to the end, or the quest
//! accepted) or DIALOG_CANCEL (closed early, or declined); the event goes on from there. Walking away makes the
//! event withdraw the request, which closes the box.
//!
//! A quest offer's last page shows the quest card (QuestInfoWindow.gui, as on the book's Quests page) in
//! DialogImage.gui's picture square over the box, and DialogWindow's own btnYes / btnNo pair as ACCEPT / DECLINE.
//!
//! The lines are read at runtime from `w101_orbs/book/dialogue.tsv` (tools/w101_book_dialogue.py, made on Ryan's PC
//! from his personal Wizard101 dialogue file; never in the repo).

use std::path::Path;
use std::time::Instant;

use crate::layout::{Layout, View};

/// The answers the talk events wait for (w101_quests.DIALOG_OK / DIALOG_CANCEL).
pub const DIALOG_OK: u32 = 1042362620;
pub const DIALOG_CANCEL: u32 = 1042362621;
/// The layouts (tools/w101_book_layout.py --pages=DialogWindow,DialogImage).
pub const WINDOW: &str = "DialogWindow";
pub const IMAGE: &str = "DialogImage";
/// The screen the layouts were made for: DialogWindow's wndDialogMain spans y 507..687 with DOCK_BOTTOM, i.e. 81
/// pixels over the bottom of Wizard101's 1024x768 screen; DialogImage's picture square is centred in it.
pub const SCREEN: [f32; 2] = [1024.0, 768.0];
/// Button labels: Wizard101's GUI.lang strings for the dialogue's page buttons (GUI_00000418 NEXT,
/// GUI_00000489 BACK, GUI_00000448 DONE) and for accepting (GUI ACCEPT / DECLINE).
const NEXT: &str = "NEXT";
const BACK: &str = "BACK";
const DONE: &str = "DONE";
const ACCEPT: &str = "ACCEPT";
const DECLINE: &str = "DECLINE";

/// One conversation of dialogue.tsv.
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub flag: u32,
    pub key: String,
    #[allow(dead_code)]
    pub speaker: String,
    pub name: String,
    pub portrait: String,
    /// Quest slug, or "-" for an idle line.
    pub quest: String,
    /// intro, goal<n>, offer, underway or idle.
    pub kind: String,
    pub pages: Vec<String>,
}

impl Request {
    pub fn offer(&self) -> bool {
        self.kind == "offer"
    }
}

/// Reads dialogue.tsv (R and P lines, tools/w101_book_dialogue.py).
pub fn load(path: &Path) -> Vec<Request> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut out: Vec<Request> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let flag = f.get(1).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        match f.first().copied() {
            Some("R") if f.len() >= 8 && flag != 0 => out.push(Request {
                flag,
                key: f[2].into(),
                speaker: f[3].into(),
                name: f[4].into(),
                portrait: f[5].into(),
                quest: f[6].into(),
                kind: f[7].into(),
                pages: Vec::new(),
            }),
            Some("P") if f.len() >= 3 => {
                if let Some(r) = out.iter_mut().rev().find(|r| r.flag == flag) {
                    r.pages.push(f[2].to_string());
                }
            }
            _ => {}
        }
    }
    out
}

/// A conversation on screen.
pub struct Conversation {
    /// Index into the requests.
    pub request: usize,
    /// Its pages (dialogue.tsv's, or for an offer without them the quest's own text).
    pub pages: Vec<String>,
    pub page: usize,
    pub offer: bool,
    pub opened: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answer {
    /// Read to the end, or the quest accepted.
    Ok,
    /// Closed early, or the quest declined.
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Press {
    Next,
    Back,
    Accept,
    Decline,
    Close,
}

impl Conversation {
    pub fn last(&self) -> bool {
        self.page + 1 >= self.pages.len()
    }

    /// Whether the quest card and ACCEPT / DECLINE are up (an offer's last page).
    pub fn choosing(&self) -> bool {
        self.offer && self.last()
    }

    /// Applies a press; Some(answer) ends the conversation.
    pub fn press(&mut self, press: Press) -> Option<Answer> {
        match press {
            Press::Next if self.choosing() => None,
            Press::Next if self.last() => Some(Answer::Ok),
            Press::Next => {
                self.page += 1;
                None
            }
            Press::Back => {
                self.page = self.page.saturating_sub(1);
                None
            }
            Press::Accept if self.choosing() => Some(Answer::Ok),
            Press::Accept => None,
            Press::Decline if self.choosing() => Some(Answer::Cancel),
            Press::Decline => None,
            Press::Close => Some(Answer::Cancel),
        }
    }

    /// DialogWindow's view for the current page: portrait, name, text and the buttons it shows.
    pub fn view(&self, window: &Layout, request: &Request) -> View {
        let mut v = View::default();
        if !request.portrait.is_empty() {
            if let Some(id) = window.find("imgPortrait") {
                v.node_art.insert(id, request.portrait.clone());
            }
        }
        v.set(window.find("txtTitle"), format!("<center>{}</center>", request.name));
        let text = self.pages.get(self.page).cloned().unwrap_or_default();
        if let Some(id) = window.find("txtMessage") {
            v.text.insert(id, text);
            if let Some(mut style) = window.nodes[id].text_style.clone() {
                // The node's VCENTER flag: the lines sit in the middle of the text area.
                style.middle = true;
                style.wrap = true;
                v.style.insert(id, style);
            }
        }
        let label = |v: &mut View, name: &str, text: &str| {
            if let Some(id) = window.find(name) {
                v.text.insert(id, format!("<center>{text}</center>"));
                if let Some(style) = window.nodes[id].text_style.clone() {
                    v.style.insert(id, style.centred());
                }
            }
        };
        // btnLeft has no use here; btnRight pages on; btnBack goes back from the second page on.
        v.show(window.find("btnLeft"), false);
        v.show(window.find("MarkButton"), false);
        v.show(window.find("btnBack"), self.page > 0);
        label(&mut v, "btnBack", BACK);
        let choosing = self.choosing();
        v.show(window.find("btnRight"), !choosing);
        label(&mut v, "btnRight", if self.last() { DONE } else { NEXT });
        v.show(window.find("btnYes"), choosing);
        v.show(window.find("btnNo"), choosing);
        label(&mut v, "btnYes", ACCEPT);
        label(&mut v, "btnNo", DECLINE);
        v
    }
}

/// Which press a click on DialogWindow's button `name` is.
pub fn button(name: &str) -> Option<Press> {
    Some(match name {
        "btnRight" => Press::Next,
        "btnBack" => Press::Back,
        "btnYes" => Press::Accept,
        "btnNo" => Press::Decline,
        "btnExit" => Press::Close,
        _ => return None,
    })
}

/// DialogWindow's place on a `screen` (pixels): the scale (screen pixels per layout pixel, Wizard101 scaling its GUI
/// with the screen's height) and the window's top-left corner, bottom-docked and centred as its flags say.
pub fn placement(window: &Layout, screen: [f32; 2]) -> (f32, [f32; 2]) {
    let s = screen[1] / SCREEN[1];
    let r = window.nodes.first().map(|n| n.rect).unwrap_or([0.0, 0.0, 800.0, 180.0]);
    let bottom_gap = SCREEN[1] - 687.0;
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    (s, [(screen[0] - w * s) / 2.0, screen[1] - (bottom_gap + h) * s])
}

/// DialogImage's picture square (imgSprite, 399 square, centred across the screen; its 588-high parent window
/// centred down it) in screen pixels.
pub fn picture_rect(image: Option<&Layout>, screen: [f32; 2]) -> [f32; 4] {
    let s = screen[1] / SCREEN[1];
    let (side, top) = match image.and_then(|l| l.find("imgSprite").map(|i| (l, i))) {
        Some((l, i)) => {
            let r = l.nodes[i].rect;
            let parent = l.nodes[i].parent.map(|p| l.nodes[p].rect).unwrap_or(r);
            let parent_top = (SCREEN[1] - (parent[3] - parent[1])) / 2.0;
            (r[2] - r[0], parent_top + (r[1] - parent[1]))
        }
        None => (399.0, 90.0),
    };
    let x = (screen[0] - side * s) / 2.0;
    [x, top * s, x + side * s, (top + side) * s]
}
