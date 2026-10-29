//! The Wizard101 loading screen, over every Elden Ring loading screen and the mod's warps.
//!
//! Two triggers: Elden Ring's own count of loading screens (CSNowLoadingHelper, `game::loading_screens`) going up,
//! which covers every loading screen the game shows (fast travel, the riding doors' map warps, death, loading a
//! save); and event flag `DOOR_FLAG`, which the door and map-page warp events turn on right before they warp (the
//! on-foot doors move the player without any loading screen of the game's). Either way the overlay covers the whole
//! screen with the loading picture (Wizard City's by default; `loadscreen_image` in w101_orbs.ini swaps it) and no
//! loading bar, over Elden Ring's own loading screen, bar and tips, until the game is back (at least `SHOW` seconds),
//! then turns the page away as Wizard101 does, and turns the flag back off.
//!
//! The picture fills the screen at any shape without being stretched (cover: the overflow is cut off). Wizard101's
//! older loading pictures (LoadScreen_W01_WC_01.dds and the other Wxx ones) are 4:3 paintings squeezed into a
//! 1024 x 1024 texture (round things come out round only at 4:3), so a square picture is taken as 4:3; Wizard101's
//! newer ones are 16:9 pictures (1920 x 1080, 3840 x 2160) and show whole on a 16:9 screen. The cut keeps the
//! picture's bottom (where the Wizard101 logo sits) rather than its very top.
//!
//! The page turn: Wizard101's PageFlip (WizardGraphicalClient GUI\PageFlip.cpp; WizardGUIManager plays
//! "StaticObjects/PageTransition/Page_Flip_Actor.kfm" and then sends "TransitionScreenRemoved") draws the loading
//! screen into a render target on both sides of a skinned page (Root.wad Page_Flip_Actor.nif: Page_FrontSide /
//! Page_BackSide, 160 x 120, 16 bones along the diagonal) and plays its "PageFlip" sequence (1.07 s): the page curls
//! up from the bottom-right corner and peels away to the top left, uncovering the world. tools/w101_page_flip.py
//! samples that animation into book\page_flip.tsv; this draws it, the screen's picture on the sheet. Without the
//! file the screen fades out instead.
//!
//! Round 9 only had the flag: Ryan's log of 2026-10-27 has no "showing the loading screen" line at all, so the flag
//! was never seen on, while the door's map warp still brought up Elden Ring's loading screen.

use std::path::Path;
use std::time::Instant;

use hudhook::imgui::{ImColor32, TextureId, Ui};
use hudhook::RenderContext;

use crate::layout::Textures;

/// Set by the warp events (m60_42_36_00.emevd doors, common.emevd map-page warps) just before a warp; cleared here.
pub const DOOR_FLAG: u32 = 1042362900;
/// The door events: flag on, 0.75 s, fade-out and warp, fade-in about 1 s after the flag. The screen holds
/// fully up this long from the flag, covering all of it.
const SHOW: f32 = 1.8;
/// The fade-out, when page_flip.tsv is missing.
const FADE: f32 = 0.35;
/// Wizard101's square loading pictures are 4:3 paintings.
const SQUARE_PICTURE_ASPECT: f32 = 4.0 / 3.0;
/// Where the cut falls when the picture is taller than the screen: 0 keeps the top, 1 the bottom.
const CROP_ANCHOR_Y: f32 = 0.85;
/// The camera's distance from the page in half screen heights (the page's lift toward the viewer grows it).
const CAMERA_DISTANCE: f32 = 4.0;
/// How much darker the back of the turning page is drawn, so the fold reads (Wizard101 draws both sides the same).
const BACK_SHADE: f32 = 0.82;
/// The longest the screen waits for the game to come back from a warp (its own fade or loading) before fading out.
const MAX_WAIT: f32 = 60.0;
/// The longest it waits without the game coming back to play at all (the title screen, if quitting there
/// counts as a loading screen).
const MAX_WAIT_NO_PLAYER: f32 = 30.0;
/// The game's loading screen count can go up a moment before it fades or drops the player, so the screen waits
/// to see the game busy before it fades; if it never does within this long, it fades anyway.
const NEVER_BUSY: f32 = 5.0;

/// Twice the signed area of a triangle's x, y.
fn winding(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])
}

/// Behind a missing picture.
const NIGHT: ImColor32 = ImColor32::from_rgb(18, 12, 40);

/// The part of a picture of `size` (pixels) that covers a screen of `screen` without stretching: (uv min, uv max).
/// A square picture is one of Wizard101's 4:3 paintings squeezed into a square texture.
fn cover(size: [f32; 2], screen: [f32; 2]) -> ([f32; 2], [f32; 2]) {
    let picture = if (size[0] - size[1]).abs() < 1.0 { SQUARE_PICTURE_ASPECT } else { size[0] / size[1].max(1.0) };
    let screen = screen[0] / screen[1].max(1.0);
    if picture > screen {
        // Wider than the screen: the sides are cut, evenly.
        let keep = screen / picture;
        ([(1.0 - keep) / 2.0, 0.0], [(1.0 + keep) / 2.0, 1.0])
    } else {
        // Taller: the top and bottom are cut, more off the top.
        let keep = picture / screen;
        let top = (1.0 - keep) * CROP_ANCHOR_Y;
        ([0.0, top], [1.0, top + keep])
    }
}

/// Wizard City's loading screen in the book's art, used when the ini's picture can't be loaded.
pub const IMAGE: &str = "GUI/LoadScreen_W01_WC_01.png";

#[derive(Default)]
pub struct LoadScreen {
    /// The picture from w101_orbs.ini's loadscreen_image, and its size.
    image: Option<(TextureId, [f32; 2])>,
    shown_at: Option<Instant>,
    /// When the fade-out began: after SHOW seconds, once the game has finished its warp.
    fading_at: Option<Instant>,
    /// Elden Ring's loading screen count last frame.
    last_count: Option<i32>,
    /// When the game last had a character in the world (stats), while the screen is up.
    player_seen: Option<Instant>,
    /// Whether the game has been busy (faded, loading, no player) since the screen came up.
    saw_busy: bool,
    /// Wizard101's page turn (book\page_flip.tsv), if it loaded.
    flip: Option<PageFlip>,
}

/// Wizard101's turning page (tools/w101_page_flip.py): texture coordinates and triangles of the sheet, and per frame
/// (30 a second) each vertex's position: x, y across and down the screen (-1..1 at rest), z toward the viewer.
struct PageFlip {
    seconds: f32,
    uv: Vec<[f32; 2]>,
    tris: Vec<[usize; 3]>,
    /// Per triangle, the sign of its winding on the screen while the sheet lies flat (its front showing).
    front: Vec<f32>,
    frames: Vec<Vec<[f32; 3]>>,
}

impl PageFlip {

    fn load(path: &Path) -> Option<PageFlip> {
        let text = std::fs::read_to_string(path).ok()?;
        let mut flip = PageFlip { seconds: 0.0, uv: vec![], tris: vec![], front: vec![], frames: vec![] };
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let num = |i: usize| f.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            match f[0] {
                "S" => flip.seconds = num(2),
                "V" => flip.uv.push([num(1), num(2)]),
                "T" => flip.tris.push([num(1) as usize, num(2) as usize, num(3) as usize]),
                "F" => flip.frames.push(Vec::with_capacity(flip.uv.len())),
                "P" => flip.frames.last_mut()?.push([num(1), num(2), num(3)]),
                _ => {}
            }
        }
        let n = flip.uv.len();
        let ok = n > 0 && flip.seconds > 0.0 && flip.frames.len() > 1 && flip.frames.iter().all(|f| f.len() == n)
            && flip.tris.iter().all(|t| t.iter().all(|&i| i < n));
        if !ok {
            return None;
        }
        let rest = &flip.frames[0];
        flip.front = flip.tris.iter().map(|&[a, b, c]| winding(rest[a], rest[b], rest[c]).signum()).collect();
        Some(flip)
    }

    /// The sheet `t` seconds into the turn, in screen pixels (with depth), between the two nearest frames.
    fn at(&self, t: f32, [w, h]: [f32; 2]) -> Vec<([f32; 2], f32)> {
        let last = self.frames.len() - 1;
        let x = (t / self.seconds).clamp(0.0, 1.0) * last as f32;
        let (i, k) = ((x as usize).min(last - 1), x.fract());
        let k = if x as usize >= last { 1.0 } else { k };
        self.frames[i].iter().zip(&self.frames[i + 1]).map(|(a, b)| {
            let p = [0, 1, 2].map(|c| a[c] + (b[c] - a[c]) * k);
            let s = CAMERA_DISTANCE / (CAMERA_DISTANCE - p[2]).max(0.5);
            ([w / 2.0 + p[0] * s * w / 2.0, h / 2.0 + p[1] * s * h / 2.0], p[2])
        }).collect()
    }
}

impl LoadScreen {
    /// Whether the screen is up and still holding (the door flag is left on until then).
    pub fn holding(&self) -> bool {
        self.shown_at.is_some() && self.fading_at.is_none()
    }

    /// Loads the picture `path` (relative to `folder`, or absolute; without it the book's Wizard City picture is
    /// used) and the page turn `flip` (book\page_flip.tsv).
    pub fn load(&mut self, folder: &Path, path: &str, flip: &Path, render: &mut dyn RenderContext) {
        self.flip = PageFlip::load(flip);
        match &self.flip {
            Some(f) => tracing::info!("loading screen: Wizard101's page turn, {} frames, {:.2} s", f.frames.len(), f.seconds),
            None => tracing::warn!("loading screen: no page turn ({}), fading out instead", flip.display()),
        }
        let path = folder.join(path);
        match image::open(&path) {
            Ok(image) => {
                let image = image.into_rgba8();
                match render.load_texture(&image, image.width(), image.height()) {
                    Ok(id) => self.image = Some((id, [image.width() as f32, image.height() as f32])),
                    Err(e) => tracing::warn!("loading screen picture {}: {e:?}", path.display()),
                }
            }
            Err(e) => tracing::warn!("loading screen picture {}: {e}", path.display()),
        }
    }

    /// Shows the loading screen from now (`why` goes to the log).
    pub fn show(&mut self, why: &str) {
        tracing::info!("loading screen: showing ({why})");
        self.shown_at = Some(Instant::now());
        self.saw_busy = false;
        self.fading_at = None;
        self.player_seen = Some(Instant::now());
    }

    /// Shows the screen as it is `shown` seconds after it came up, the page turn `turning` seconds in (the preview
    /// renderer).
    #[allow(dead_code)]
    pub fn preview_at(&mut self, shown: f32, turning: Option<f32>) {
        let now = Instant::now();
        self.shown_at = Some(now - std::time::Duration::from_secs_f32(shown));
        self.player_seen = Some(now);
        self.saw_busy = false;
        self.fading_at = turning.map(|t| now - std::time::Duration::from_secs_f32(t));
    }

    /// Whether Elden Ring has started one of its loading screens since the last call (`count`: its loading
    /// screen count now, None when it can't be read).
    pub fn game_loading_started(&mut self, count: Option<i32>) -> bool {
        let started = matches!((self.last_count, count), (Some(a), Some(b)) if a != b);
        if count.is_some() {
            self.last_count = count;
        }
        started
    }

    /// Draws the screen while it is up. `busy`: the game is still warping (faded out, loading, or no player yet);
    /// the screen stays fully up until it is done, so it covers the whole warp.
    pub fn draw(&mut self, ui: &Ui, textures: &Textures, busy: bool, player: bool) {
        let Some(at) = self.shown_at else { return };
        let t = at.elapsed().as_secs_f32();
        if player || self.player_seen.is_none() {
            self.player_seen = Some(Instant::now());
        }
        let no_player = self.player_seen.map_or(0.0, |p| p.elapsed().as_secs_f32());
        self.saw_busy |= busy;
        let back = !busy && (self.saw_busy || t >= NEVER_BUSY);
        if self.fading_at.is_none() && t >= SHOW && (back || t >= MAX_WAIT || no_player >= MAX_WAIT_NO_PLAYER) {
            tracing::info!("loading screen: {} after {t:.1} s", if self.flip.is_some() { "turning the page" } else { "fading out" });
            self.fading_at = Some(Instant::now());
            // The warp is over: clear the door flag (it stays on while the screen is up).
            crate::game::set_event_flag(DOOR_FLAG, false);
        }
        let [w, h] = ui.io().display_size;
        let draw = ui.get_foreground_draw_list();
        let picture = self.image.or_else(|| textures.get(IMAGE));
        let (uv0, uv1) = picture.map_or(([0.0, 0.0], [1.0, 1.0]), |(_, size)| cover(size, [w, h]));
        let ended = |at: Instant, seconds: f32| at.elapsed().as_secs_f32() >= seconds;
        match (self.fading_at, &self.flip) {
            (Some(at), Some(flip)) => {
                if ended(at, flip.seconds) {
                    self.shown_at = None;
                    return;
                }
                // Wizard101's page turn: the screen's picture on the curling sheet, the world behind it.
                let sheet = flip.at(at.elapsed().as_secs_f32(), [w, h]);
                let image_uv = |uv: [f32; 2]| [uv0[0] + uv[0] * (uv1[0] - uv0[0]), uv0[1] + uv[1] * (uv1[1] - uv0[1])];
                let mut order: Vec<(f32, usize)> =
                    flip.tris.iter().enumerate().map(|(i, t)| (t.iter().map(|&v| sheet[v].1).sum::<f32>(), i)).collect();
                order.sort_by(|a, b| a.0.total_cmp(&b.0));
                for (_, i) in order {
                    let [a, b, c] = flip.tris[i];
                    let (pa, pb, pc) = (sheet[a].0, sheet[b].0, sheet[c].0);
                    // Wound the other way round than when flat: the back of the page shows.
                    let back = winding([pa[0], pa[1], 0.0], [pb[0], pb[1], 0.0], [pc[0], pc[1], 0.0]) * flip.front[i] < 0.0;
                    let shade = if back { BACK_SHADE } else { 1.0 };
                    let col = ImColor32::from_rgb((255.0 * shade) as u8, (255.0 * shade) as u8, (255.0 * shade) as u8);
                    match picture {
                        Some((image, _)) => draw
                            .add_image_quad(image, pa, pb, pc, pc)
                            .uv(image_uv(flip.uv[a]), image_uv(flip.uv[b]), image_uv(flip.uv[c]), image_uv(flip.uv[c]))
                            .col(col)
                            .build(),
                        None => draw.add_triangle(pa, pb, pc, NIGHT).filled(true).build(),
                    }
                }
            }
            (fading, _) => {
                let alpha = fading.map_or(1.0, |f| 1.0 - f.elapsed().as_secs_f32() / FADE);
                if alpha <= 0.0 {
                    self.shown_at = None;
                    return;
                }
                let a = |c: ImColor32| ImColor32::from_rgba(c.r, c.g, c.b, (c.a as f32 * alpha) as u8);
                // The whole screen: Wizard101's night-sky purple (if the picture is missing), then the picture
                // covering it.
                draw.add_rect([0.0, 0.0], [w, h], a(NIGHT)).filled(true).build();
                if let Some((image, _)) = picture {
                    draw.add_image(image, [0.0, 0.0], [w, h]).uv_min(uv0).uv_max(uv1).col(a(ImColor32::WHITE)).build();
                }
            }
        }
    }
}
