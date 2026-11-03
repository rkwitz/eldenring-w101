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
//! DialogImage.gui's picture square over the box, and DialogWindow's own btnYes / btnNo pair ("yes" / "no").
//!
//! Round 18 (quests F1/F3): a conversation may change speaker from page to page (Gamma's tips after Ambrose, Olivia
//! cutting in on Ceren), each page with its own name and portrait; any conversation can end in a quest's card (the R
//! line's card column), as Wizard101's givers close one quest and offer the next in one talk; the card shows the
//! goal's picture (QuestInfoWindow's imgGoalObjective1); and the buttons take mouse clicks, the cursor freed from
//! Elden Ring's clip while the box is up (lib.rs).
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
/// GUI_00000489 BACK, GUI_00000448 DONE) and DialogWindow.gui's own btnYes / btnNo labels (round 18:
/// GUI_00000378 "yes", GUI_00000379 "no"; round 17 had ACCEPT / DECLINE, which the client doesn't use here).
const NEXT: &str = "NEXT";
const BACK: &str = "BACK";
const DONE: &str = "DONE";
/// Round 28 (Q2, Ryan: "Decline (left) and Accept (right), not Yes/No"): Wizard101's GUI.lang ACCEPT / DECLINE
/// strings (keys ACCEPT and DECLINE; GUI_00000378/379 "yes"/"no" are DialogWindow.gui's own labels, which Ryan
/// doesn't want). DialogWindow.gui has btnYes left (448..576) and btnNo right (616..744); the pair is swapped so
/// Decline is on the left and Accept on the right.
const YES: &str = "Accept";
const NO: &str = "Decline";

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
    #[allow(dead_code)]
    pub quest: String,
    /// intro, after, goal<n>, offer, accepted, underway or idle.
    #[allow(dead_code)]
    pub kind: String,
    /// Round 18: the quest whose card and yes / no end the conversation ("" for none): an offer, the intro, or a
    /// talk goal that hands out the next quest (Connelly closing Unicorn Way and offering Ghost Hunters).
    pub card: String,
    pub pages: Vec<String>,
    /// Round 18: each page's speaker when it isn't the conversation's own (Gamma's tips, Olivia cutting in on
    /// Ceren): (name, portrait), empty strings for the request's own.
    pub speakers: Vec<(String, String)>,
}

impl Request {
    pub fn offer(&self) -> bool {
        !self.card.is_empty()
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
                // Round 17 files have no card column: their offers end in their own quest's card.
                card: match f.get(8).copied() {
                    Some("-") | Some("") => String::new(),
                    Some(card) => card.into(),
                    None if f[7] == "offer" => f[6].into(),
                    None => String::new(),
                },
                pages: Vec::new(),
                speakers: Vec::new(),
            }),
            Some("P") if f.len() >= 3 => {
                if let Some(r) = out.iter_mut().rev().find(|r| r.flag == flag) {
                    r.pages.push(f[2].to_string());
                    let field = |i: usize| f.get(i).map(|v| v.to_string()).unwrap_or_default();
                    r.speakers.push((field(3), field(4)));
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

    /// Whether the quest card and yes / no are up (an offer's last page).
    pub fn choosing(&self) -> bool {
        self.offer && self.last()
    }

    /// Round 22 S3: Wizard101's sound for a press (_Shared-WorldData GUI/DialogWindow.gui m_sSelectSound: btnRight
    /// HudSelect, btnBack CancelButtonClick, btnExit HudCancel; a quest taken or turned down plays
    /// Sound/GUI/ui_quest_accepted / ui_quest_decline).
    pub fn press_sound(&self, press: Press) -> Option<&'static str> {
        match press {
            Press::Next if self.choosing() => None,
            Press::Next => Some("HudSelect"),
            Press::Back => Some("CancelButtonClick"),
            Press::Accept if self.choosing() => Some(if self.offer { "ui_quest_accepted" } else { "HudSelect" }),
            Press::Decline if self.choosing() => Some(if self.offer { "ui_quest_decline" } else { "HudSelect" }),
            Press::Accept | Press::Decline => None,
            Press::Close => Some("HudCancel"),
        }
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
        let (name, portrait) = self.speaker(request);
        if !portrait.is_empty() {
            if let Some(id) = window.find("imgPortrait") {
                v.node_art.insert(id, portrait.to_string());
            }
        }
        v.set(window.find("txtTitle"), format!("<center>{name}</center>"));
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
        label(&mut v, "btnYes", YES);
        label(&mut v, "btnNo", NO);
        if let (Some(yes), Some(no)) = (window.find("btnYes"), window.find("btnNo")) {
            v.rect.insert(yes, window.nodes[no].rect);
            v.rect.insert(no, window.nodes[yes].rect);
        }
        // Round 28 (Q3): no red X on an offer. Wizard101's own dialogue box with a choice (Ryan's screenshot
        // round25/ref/2_w101_multi_quest_picker.png: "What can I do for you?" with CANCEL) shows no close X at the
        // bottom right; the offer is closed by Decline (B / Esc).
        if choosing {
            v.show(window.find("btnExit"), false);
        }
        v
    }

    /// Round 27 (A3, Ryan: "Diego's name in the dialogue box is not centred"): DialogWindow's txtTitle (169..730) and
    /// txtMessage (185..768) leave room for the 180-wide portrait on the left, so a speaker without one (Diego and Ugo
    /// had no ER portrait yet: the log's "NpcPortraits/ER/diego.png: cannot find the file") had his name centred
    /// 48 layout px right of the box's middle over an empty space. With no portrait loaded the name is centred on the
    /// whole title scroll (the box's 0..802 less the scroll ends: 40..762) and the text starts at the same 40.
    /// With a portrait, Wizard101's own layout is kept.
    pub fn without_portrait(window: &Layout, v: &mut View) {
        for (name, left, right) in [("txtTitle", 40.0, 762.0), ("txtMessage", 40.0, 768.0)] {
            if let Some(id) = window.find(name) {
                let r = window.nodes[id].rect;
                v.rect.insert(id, [left, r[1], right, r[3]]);
            }
        }
    }

    /// The current page's speaker, (name, portrait): the request's own unless the page names another.
    pub fn speaker<'a>(&self, request: &'a Request) -> (&'a str, &'a str) {
        match request.speakers.get(self.page) {
            Some((name, portrait)) if !name.is_empty() => {
                (name.as_str(), if portrait.is_empty() { request.portrait.as_str() } else { portrait.as_str() })
            }
            _ => (request.name.as_str(), request.portrait.as_str()),
        }
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

/// The experience bar's top on a `screen` (pixels), as lib.rs draws it (XP_BAR 733 of HUDWindow's 768, scaled by
/// the screen's height and w101_orbs.ini `scale`).
pub fn xp_bar_top(screen: [f32; 2], hud_scale: f32) -> f32 {
    const XP_BAR_TOP: f32 = 733.0;
    screen[1] - (SCREEN[1] - XP_BAR_TOP) * hud_scale * screen[1] / SCREEN[1]
}

/// DialogWindow's place on a `screen` (pixels): the scale (screen pixels per layout pixel, Wizard101 scaling its GUI
/// with the screen's height) and the window's top-left corner, centred across. Round 22 (H4, Ryan: "move the
/// dialogue down a little bit; put it right above the level bar"): its bottom BAR_GAP layout pixels over the
/// experience bar's top (`bar_top`, xp_bar_top), not at DialogWindow.gui's 687 of 768 (81 pixels up).
pub fn placement(window: &Layout, screen: [f32; 2], bar_top: f32) -> (f32, [f32; 2]) {
    const BAR_GAP: f32 = 4.0;
    let s = screen[1] / SCREEN[1];
    let r = window.nodes.first().map(|n| n.rect).unwrap_or([0.0, 0.0, 800.0, 180.0]);
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    (s, [(screen[0] - w * s) / 2.0, bar_top - (BAR_GAP + h) * s])
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
