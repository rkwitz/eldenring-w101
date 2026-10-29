//! Wizard101 GUI layouts drawn with imgui: the `.lay` files tools/w101_book_layout.py makes from the game's .gui
//! files (one node per line, in drawing order, with page-pixel rectangles, art, frames and text styles).
//!
//! A page is drawn as Wizard101 would draw it with its default visibility; the spellbook changes what it shows
//! through a `View` (visibility, text and progress overrides, plus extra text boxes) instead of editing the layout.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use hudhook::imgui::{DrawListMut, ImColor32, TextureId};
use hudhook::RenderContext;

use crate::font::BitmapFont;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Centre,
    Right,
}

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub font: String,
    /// Glyphis font scale (font pixels to page pixels); 0 = fit the text to the box's height.
    pub scale: f32,
    pub colour: ImColor32,
    pub hover: Option<ImColor32>,
    pub shadow: bool,
    pub outline: bool,
    pub wrap: bool,
    /// Alignment when the text has no <center>/<right> markup.
    pub align: Align,
    /// Vertically centred in the box (otherwise from the top).
    pub middle: bool,
    /// The control's m_nKerningAdjust: extra pen advance per glyph, font pixels.
    pub tracking: f32,
}

impl TextStyle {
    pub fn new(font: &str, scale: f32, colour: ImColor32) -> TextStyle {
        TextStyle {
            font: font.to_string(),
            scale,
            colour,
            hover: None,
            shadow: false,
            outline: false,
            wrap: false,
            align: Align::Left,
            middle: true,
            tracking: 0.0,
        }
    }

    pub fn centred(mut self) -> Self {
        self.align = Align::Centre;
        self
    }

    pub fn right(mut self) -> Self {
        self.align = Align::Right;
        self
    }

    pub fn wrapped(mut self) -> Self {
        self.wrap = true;
        self.middle = false;
        self
    }

    pub fn outlined(mut self) -> Self {
        self.outline = true;
        self
    }

    pub fn shadowed(mut self) -> Self {
        self.shadow = true;
        self
    }
}

#[derive(Debug)]
pub struct Node {
    pub parent: Option<usize>,
    pub class: String,
    pub name: String,
    pub visible: bool,
    pub rect: [f32; 4],
    pub alpha: f32,
    pub img: Option<String>,
    pub uv: [f32; 4],
    pub tint: ImColor32,
    /// Button frames: normal, highlighted, depressed, greyed, checked.
    pub frames: [Option<[f32; 4]>; 5],
    pub grow: f32,
    pub back: Option<(String, [f32; 4])>,
    /// ControlWidget border pieces: tl, t, tr, l, r, bl, b, br, centre.
    pub border: [Option<String>; 9],
    pub bar: Option<String>,
    pub position: f32,
    pub vertical: bool,
    pub progress_uv: bool,
    pub text_style: Option<TextStyle>,
    pub text: Option<String>,
    pub label: Option<String>,
    pub card: Option<[f32; 2]>,
    pub gap: Option<[f32; 2]>,
}

impl Node {
    pub fn is_button(&self) -> bool {
        self.frames[0].is_some()
    }

    pub fn width(&self) -> f32 {
        self.rect[2] - self.rect[0]
    }

    pub fn height(&self) -> f32 {
        self.rect[3] - self.rect[1]
    }
}

pub struct Layout {
    pub nodes: Vec<Node>,
}

fn floats<const N: usize>(text: &str) -> Option<[f32; N]> {
    let values: Vec<f32> = text.split(',').filter_map(|v| v.trim().parse().ok()).collect();
    values.try_into().ok()
}

/// RRGGBBAA hex.
fn colour(text: &str) -> Option<ImColor32> {
    let v = u32::from_str_radix(text, 16).ok()?;
    Some(ImColor32::from_rgba((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8))
}

impl Layout {
    pub fn load(path: &Path) -> Result<Layout, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut nodes = Vec::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let fields: HashMap<&str, &str> = line.split('\t').filter_map(|kv| kv.split_once('=')).collect();
            let get = |key: &str| fields.get(key).copied();
            let parent = get("p").and_then(|p| p.parse::<i64>().ok()).filter(|p| *p >= 0).map(|p| p as usize);
            let style = get("st").unwrap_or("");
            let text_style = (get("txt").is_some() || get("label").is_some()).then(|| TextStyle {
                font: get("font").unwrap_or("").to_string(),
                scale: get("fs").and_then(|v| v.parse().ok()).unwrap_or(0.0),
                colour: get("col").and_then(colour).unwrap_or(ImColor32::BLACK),
                hover: get("hcol").and_then(colour),
                shadow: get("sh") == Some("1"),
                outline: get("ol") == Some("1"),
                wrap: get("ww") == Some("1"),
                align: Align::Left,
                middle: true,
                tracking: get("ka").and_then(|v| v.parse().ok()).unwrap_or(0.0),
            });
            let frame = |key: &str| get(key).and_then(floats::<4>);
            nodes.push(Node {
                parent,
                class: get("c").unwrap_or("").to_string(),
                name: get("n").unwrap_or("").to_string(),
                visible: get("v") == Some("1"),
                rect: get("r").and_then(floats::<4>).unwrap_or([0.0; 4]),
                alpha: get("a").and_then(|v| v.parse().ok()).unwrap_or(1.0),
                img: get("img").map(str::to_string),
                uv: get("uv").and_then(floats::<4>).unwrap_or([0.0, 0.0, 1.0, 1.0]),
                tint: get("tint").and_then(colour).unwrap_or(ImColor32::WHITE),
                frames: [frame("f0"), frame("f1"), frame("f2"), frame("f3"), frame("f4")],
                grow: get("grow").and_then(|v| v.parse().ok()).unwrap_or(1.0),
                back: get("back").map(|b| (b.to_string(), frame("buv").unwrap_or([0.0, 0.0, 1.0, 1.0]))),
                border: ["tl", "t", "tr", "l", "rt", "bl", "b", "br", "cm"].map(|k| get(k).map(str::to_string)),
                bar: get("bar").map(str::to_string),
                position: get("pos").and_then(|v| v.parse().ok()).unwrap_or(0.0),
                vertical: get("vert") == Some("1"),
                progress_uv: style.contains("PROGRESS_UV"),
                text_style,
                text: get("txt").map(|t| t.replace("\\n", "\n")),
                label: get("label").map(|t| t.replace("\\n", "\n")),
                card: get("card").and_then(floats::<2>),
                gap: get("gap").and_then(floats::<2>),
            });
        }
        if nodes.is_empty() {
            return Err(format!("{}: no nodes", path.display()));
        }
        Ok(Layout { nodes })
    }

    /// Every image the layout draws.
    pub fn images(&self) -> Vec<String> {
        let mut out = Vec::new();
        for n in &self.nodes {
            out.extend(n.img.iter().cloned());
            out.extend(n.back.iter().map(|b| b.0.clone()));
            out.extend(n.bar.iter().cloned());
            out.extend(n.border.iter().flatten().cloned());
        }
        out.sort();
        out.dedup();
        out
    }

    fn is_under(&self, mut id: usize, ancestor: usize) -> bool {
        while let Some(p) = self.nodes[id].parent {
            if p == ancestor {
                return true;
            }
            id = p;
        }
        false
    }

    /// The first node named `name` (in drawing order).
    pub fn find(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }

    /// The first node named `name` below `ancestor`.
    pub fn find_under(&self, ancestor: usize, name: &str) -> Option<usize> {
        (ancestor + 1..self.nodes.len()).find(|&i| self.nodes[i].name == name && self.is_under(i, ancestor))
    }

    /// A node by path of names, each below the one before ("StatsPageBG/DamageStats").
    pub fn path(&self, path: &str) -> Option<usize> {
        let mut parts = path.split('/');
        let mut at = self.find(parts.next()?)?;
        for part in parts {
            at = self.find_under(at, part)?;
        }
        Some(at)
    }

    /// The first text node whose default text contains `needle`.
    pub fn find_text(&self, needle: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.text.as_deref().is_some_and(|t| t.contains(needle)))
    }

    /// Direct children of `id`, in drawing order.
    pub fn children(&self, id: usize) -> Vec<usize> {
        (id + 1..self.nodes.len()).filter(|&i| self.nodes[i].parent == Some(id)).collect()
    }

    /// Direct children of `id`, left to right.
    pub fn columns(&self, id: usize) -> Vec<usize> {
        let mut c = self.children(id);
        c.sort_by(|a, b| self.nodes[*a].rect[0].total_cmp(&self.nodes[*b].rect[0]));
        c
    }
}

/// Changes to a layout's default look for one frame.
#[derive(Default)]
pub struct View {
    pub show: HashMap<usize, bool>,
    pub text: HashMap<usize, String>,
    pub style: HashMap<usize, TextStyle>,
    pub progress: HashMap<usize, f32>,
    pub checked: HashSet<usize>,
    /// Nodes drawn greyed out (Wizard101's disabled frame and alpha).
    pub greyed: HashSet<usize>,
    /// Nodes moved to another rectangle (the tab marker under the chosen tab).
    pub rect: HashMap<usize, [f32; 4]>,
    pub extra: Vec<([f32; 4], String, TextStyle)>,
    /// Images drawn over a node, right after its own (in the layout's painter's order).
    pub node_images: HashMap<usize, String>,
    /// Another picture for a node's own art (same frames).
    pub node_art: HashMap<usize, String>,
    /// Images drawn after the nodes, under the fills (backgrounds): (rect, image, uv).
    pub backs: Vec<([f32; 4], String, [f32; 4])>,
    /// Filled rectangles drawn after the nodes, under the images.
    pub fills: Vec<([f32; 4], ImColor32)>,
    /// Filled triangles and rectangles drawn over the images, under the text.
    pub over_tris: Vec<([[f32; 2]; 3], ImColor32)>,
    pub over_fills: Vec<([f32; 4], ImColor32)>,
    /// Nine-piece frames (tl, t, tr, l, r, bl, b, br, centre) drawn over the fills, under the images.
    pub frames: Vec<([f32; 4], [Option<String>; 9])>,
    /// Images drawn after the nodes: (rect, image, uv).
    pub images: Vec<([f32; 4], String, [f32; 4])>,
    /// Images drawn with the images, tinted (rect, image, uv, tint): a faded badge keeps its own shape.
    pub tinted: Vec<([f32; 4], String, [f32; 4], ImColor32)>,
    /// Windows drawn without their background image (their children still draw).
    pub no_back: HashSet<usize>,
    /// Nodes drawn whole over the canvas's hole (the frame around it).
    pub over_hole: HashSet<usize>,
    /// Hover texts: (rect, lines), the first line the title.
    pub tips: Vec<([f32; 4], Vec<String>)>,
    /// The page's row of category tabs along the top (for the LB / RB hints).
    pub tab_row: Option<[f32; 4]>,
    /// A cut-out in the book (page pixels) where the game shows through.
    pub hole: Option<[f32; 4]>,
    /// Where a picture of the game's screen is drawn on the page (the map page), page pixels.
    pub picture: Option<[f32; 4]>,
}

impl View {
    pub fn show(&mut self, id: Option<usize>, shown: bool) {
        if let Some(id) = id {
            self.show.insert(id, shown);
        }
    }

    pub fn set(&mut self, id: Option<usize>, text: impl Into<String>) {
        if let Some(id) = id {
            self.text.insert(id, text.into());
        }
    }

    pub fn add(&mut self, rect: [f32; 4], text: impl Into<String>, style: TextStyle) {
        self.extra.push((rect, text.into(), style));
    }

    pub fn image(&mut self, rect: [f32; 4], image: impl Into<String>) {
        self.images.push((rect, image.into(), [0.0, 0.0, 1.0, 1.0]));
    }

    pub fn tip(&mut self, rect: [f32; 4], lines: Vec<String>) {
        if !lines.is_empty() {
            self.tips.push((rect, lines));
        }
    }
}

/// Loaded art: image path (as in the layouts) -> texture and size in pixels.
#[derive(Default)]
pub struct Textures {
    map: HashMap<String, (TextureId, [f32; 2])>,
    failed: HashSet<String>,
    wanted: Vec<String>,
}

impl Textures {
    pub fn get(&self, name: &str) -> Option<(TextureId, [f32; 2])> {
        self.map.get(name).copied()
    }

    /// Queues images to load on the next `load_wanted` (render thread, before the frame).
    /// Registers a texture made elsewhere (the character pictures, "@model/...", and screen copies, "@screen/...").
    pub fn set(&mut self, name: &str, texture: TextureId, size: [f32; 2]) {
        self.map.insert(name.to_string(), (texture, size));
    }

    pub fn want(&mut self, names: impl IntoIterator<Item = String>) {
        for name in names {
            // Textures made elsewhere ("@model/...", "@screen/..."); "@er/..." is Elden Ring's art on disk.
            if name.starts_with('@') && !name.starts_with("@er/") {
                continue;
            }
            if !self.map.contains_key(&name) && !self.failed.contains(&name) && !self.wanted.contains(&name) {
                self.wanted.push(name);
            }
        }
    }

    pub fn pending(&self) -> bool {
        !self.wanted.is_empty()
    }

    pub fn load_wanted(&mut self, folder: &Path, render: &mut dyn RenderContext) {
        for name in std::mem::take(&mut self.wanted) {
            // "@er/..." is Elden Ring's art next to the Wizard101 art (book/er), the rest is book/art.
            let path = match name.strip_prefix("@er/") {
                Some(rest) => folder.parent().unwrap_or(folder).join("er").join(rest),
                None => folder.join(&name),
            };
            let loaded = image::open(&path)
                .map_err(|e| e.to_string())
                .and_then(|image| {
                    let image = image.into_rgba8();
                    let size = [image.width() as f32, image.height() as f32];
                    render
                        .load_texture(&image, image.width(), image.height())
                        .map(|t| (t, size))
                        .map_err(|e| format!("{e:?}"))
                });
            match loaded {
                Ok(t) => {
                    self.map.insert(name, t);
                }
                Err(e) => {
                    tracing::warn!("spellbook art {}: {e}", path.display());
                    self.failed.insert(name);
                }
            }
        }
    }
}

/// Where a layout is drawn: page pixels to screen pixels, the mouse in page pixels, and the art and fonts.
pub struct Canvas<'a, 'b> {
    pub draw: &'a DrawListMut<'b>,
    pub textures: &'a Textures,
    pub fonts: &'a HashMap<String, BitmapFont>,
    pub origin: [f32; 2],
    pub s: f32,
    pub mouse: [f32; 2],
    pub mouse_down: bool,
    /// Multiplies every alpha (fading the book in).
    pub alpha: f32,
    /// A cut-out (page pixels) that images leave empty, so the game shows through.
    pub hole: Cell<Option<[f32; 4]>>,
}

pub fn inside(p: [f32; 2], r: [f32; 4]) -> bool {
    p[0] >= r[0] && p[0] < r[2] && p[1] >= r[1] && p[1] < r[3]
}

fn fade(c: ImColor32, alpha: f32) -> ImColor32 {
    ImColor32::from_rgba(c.r, c.g, c.b, (c.a as f32 * alpha.clamp(0.0, 1.0)) as u8)
}

impl Canvas<'_, '_> {
    pub fn at(&self, x: f32, y: f32) -> [f32; 2] {
        [self.origin[0] + x * self.s, self.origin[1] + y * self.s]
    }

    pub fn image(&self, name: &str, r: [f32; 4], uv: [f32; 4], tint: ImColor32) {
        let Some((texture, _)) = self.textures.get(name) else { return };
        if r[2] <= r[0] || r[3] <= r[1] {
            return;
        }
        if let Some(h) = self.hole.get() {
            if h[0] < r[2] && h[2] > r[0] && h[1] < r[3] && h[3] > r[1] {
                // Draw the parts of the image around the hole: above, below, left, right.
                let h = [h[0].max(r[0]), h[1].max(r[1]), h[2].min(r[2]), h[3].min(r[3])];
                let part = |p: [f32; 4]| {
                    if p[2] - p[0] < 0.01 || p[3] - p[1] < 0.01 {
                        return;
                    }
                    let u = |x: f32| uv[0] + (uv[2] - uv[0]) * (x - r[0]) / (r[2] - r[0]);
                    let v = |y: f32| uv[1] + (uv[3] - uv[1]) * (y - r[1]) / (r[3] - r[1]);
                    self.draw
                        .add_image(texture, self.at(p[0], p[1]), self.at(p[2], p[3]))
                        .uv_min([u(p[0]), v(p[1])])
                        .uv_max([u(p[2]), v(p[3])])
                        .col(fade(tint, self.alpha))
                        .build();
                };
                part([r[0], r[1], r[2], h[1]]);
                part([r[0], h[3], r[2], r[3]]);
                part([r[0], h[1], h[0], h[3]]);
                part([h[2], h[1], r[2], h[3]]);
                return;
            }
        }
        self.draw
            .add_image(texture, self.at(r[0], r[1]), self.at(r[2], r[3]))
            .uv_min([uv[0], uv[1]])
            .uv_max([uv[2], uv[3]])
            .col(fade(tint, self.alpha))
            .build();
    }

    /// A ControlWidget's border: corners at their own size, edges stretched between them, centre repeated.
    pub fn border(&self, pieces: &[Option<String>; 9], r: [f32; 4], tint: ImColor32) {
        let size = |i: usize| pieces[i].as_deref().and_then(|p| self.textures.get(p)).map(|t| t.1).unwrap_or([0.0; 2]);
        let [x0, y0, x1, y1] = r;
        let (tl, tr, bl, br) = (size(0), size(2), size(5), size(7));
        let left = tl[0].max(bl[0]);
        let right = tr[0].max(br[0]);
        let top = tl[1].max(tr[1]);
        let bottom = bl[1].max(br[1]);
        let full = [0.0, 0.0, 1.0, 1.0];
        let rects = [
            [x0, y0, x0 + tl[0], y0 + tl[1]],
            [x0 + left, y0, x1 - right, y0 + top],
            [x1 - tr[0], y0, x1, y0 + tr[1]],
            [x0, y0 + top, x0 + left, y1 - bottom],
            [x1 - right, y0 + top, x1, y1 - bottom],
            [x0, y1 - bl[1], x0 + bl[0], y1],
            [x0 + left, y1 - bottom, x1 - right, y1],
            [x1 - br[0], y1 - br[1], x1, y1],
            [x0 + left, y0 + top, x1 - right, y1 - bottom],
        ];
        for (i, (piece, rect)) in pieces.iter().zip(rects).enumerate() {
            if let Some(p) = piece {
                // Pieces sample half a texel in from their edges, so the sampler's wrap doesn't bleed
                // the opposite edge in as a line.
                let uv = match (i, size(i)) {
                    (_, [w, h]) if w > 0.0 && h > 0.0 => {
                        let (du, dv) = (0.5 / w, 0.5 / h);
                        [du, dv, 1.0 - du, 1.0 - dv]
                    }
                    _ => full,
                };
                self.image(p, rect, uv, tint);
            }
        }
    }

    /// Text in a box with Wizard101's markup: <center> / <right>, "\n" line breaks; other tags are dropped.
    pub fn text(&self, raw: &str, r: [f32; 4], style: &TextStyle, hovered: bool) {
        let mut align = style.align;
        let lower = raw.to_ascii_lowercase();
        if lower.contains("<center>") {
            align = Align::Centre;
        } else if lower.contains("<right>") {
            align = Align::Right;
        }
        let mut plain = String::with_capacity(raw.len());
        let mut in_tag = false;
        for c in raw.chars() {
            match c {
                '<' => in_tag = true,
                '>' if in_tag => in_tag = false,
                _ if !in_tag => plain.push(c),
                _ => {}
            }
        }
        let plain = plain.replace("&amp;", "&");
        let font = self
            .fonts
            .get(&style.font)
            .or_else(|| self.fonts.get("WizardFancy"))
            .or_else(|| self.fonts.values().next());
        let Some(font) = font else { return };
        let t = style.tracking;
        let width = r[2] - r[0];
        let height = r[3] - r[1];
        let base = if style.scale > 0.0 { style.scale } else { (height * 0.8 / font.line_height).max(0.1) };
        let wrap_at = |scale: f32| -> Vec<String> {
            let max_w = width / scale;
            let mut lines = Vec::new();
            for paragraph in plain.split('\n') {
                let paragraph = paragraph.trim();
                if style.wrap {
                    let mut line = String::new();
                    for word in paragraph.split_whitespace() {
                        let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                        if font.width_tracked(&candidate, t) > max_w && !line.is_empty() {
                            lines.push(std::mem::take(&mut line));
                            line = word.to_string();
                        } else {
                            line = candidate;
                        }
                    }
                    lines.push(line);
                } else {
                    lines.push(paragraph.to_string());
                }
            }
            while lines.len() > 1 && lines.last().is_some_and(|l| l.is_empty()) {
                lines.pop();
            }
            lines
        };
        // Wrapped text: smaller (down to 60%) until it fits its box's height.
        let mut scale = base;
        let mut lines = wrap_at(scale);
        if style.wrap {
            while lines.len() as f32 * font.line_height * scale > height * 1.05 && scale > base * 0.6 {
                scale *= 0.93;
                lines = wrap_at(scale);
            }
        }
        // One-line text: shrink to fit (down to 50%), then cut with "..."; lines taller than the box shrink too.
        if !style.wrap {
            let widest = lines.iter().map(|l| font.width_tracked(l, t)).fold(0.0, f32::max) * scale;
            if widest > width && widest > 0.0 {
                scale *= (width / widest).max(0.5);
            }
            let tall = lines.len() as f32 * font.line_height * scale;
            if tall > height * 1.3 && tall > 0.0 {
                scale *= (height * 1.3 / tall).max(0.6);
            }
            let max_w = width / scale;
            for line in lines.iter_mut() {
                *line = font.fit(line, max_w.max(1.0), t);
            }
        }
        let line_h = font.line_height * scale;
        let total = line_h * lines.len() as f32;
        let mut y = if style.middle && total < height { r[1] + (height - total) / 2.0 } else { r[1] };
        let colour = if hovered { style.hover.unwrap_or(style.colour) } else { style.colour };
        let colour = fade(colour, self.alpha);
        let dark = fade(ImColor32::from_rgba(0, 0, 0, 200), self.alpha * colour.a as f32 / 255.0);
        for line in lines {
            // Wrapped text stops at the bottom of its box rather than running over what is below.
            if y + line_h > r[3] + line_h * 0.25 && style.wrap {
                break;
            }
            let w = font.width_tracked(&line, t) * scale;
            let x = match align {
                Align::Left => r[0],
                Align::Centre => r[0] + (width - w) / 2.0,
                Align::Right => r[2] - w,
            };
            let sc = scale * self.s;
            let at = self.at(x, y);
            let o = (1.2 * self.s).max(1.0);
            if style.outline {
                for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o)] {
                    font.draw_tracked(self.draw, &line, [at[0] + dx, at[1] + dy], sc, dark, t);
                }
            }
            if style.shadow {
                font.draw_tracked(self.draw, &line, [at[0] + o * 1.5, at[1] + o * 1.5], sc, dark, t);
            }
            font.draw_tracked(self.draw, &line, at, sc, colour, t);
            y += line_h;
        }
    }
}

/// What the mouse is over after drawing.
#[derive(Default, Clone, Copy)]
pub struct Hits {
    /// The topmost visible button under the mouse.
    pub button: Option<usize>,
}

/// Effective visibility of every node (own flag or override, and all parents shown).
pub fn shown(layout: &Layout, view: &View) -> Vec<bool> {
    let mut out = vec![false; layout.nodes.len()];
    for (i, n) in layout.nodes.iter().enumerate() {
        let own = view.show.get(&i).copied().unwrap_or(n.visible);
        out[i] = own && n.parent.is_none_or(|p| out[p]);
    }
    out
}

/// Draws the nodes `filter` accepts, in order; returns the button under the mouse.
pub fn draw(layout: &Layout, view: &View, canvas: &Canvas, filter: impl Fn(usize) -> bool) -> Hits {
    let visible = shown(layout, view);
    // The topmost button under the mouse wins: buttons are tested last to first.
    let hovered = (0..layout.nodes.len()).rev().find(|&i| {
        visible[i]
            && filter(i)
            && layout.nodes[i].is_button()
            && !view.greyed.contains(&i)
            && inside(canvas.mouse, view.rect.get(&i).copied().unwrap_or(layout.nodes[i].rect))
    });
    for (i, n) in layout.nodes.iter().enumerate() {
        if !visible[i] || !filter(i) {
            continue;
        }
        let greyed = view.greyed.contains(&i);
        let alpha = n.alpha * if greyed { 0.45 } else { 1.0 };
        let mut r = view.rect.get(&i).copied().unwrap_or(n.rect);
        let is_hovered = hovered == Some(i);
        if is_hovered && n.grow > 1.0 && !greyed {
            let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
            let (hw, hh) = ((r[2] - r[0]) / 2.0 * n.grow, (r[3] - r[1]) / 2.0 * n.grow);
            r = [cx - hw, cy - hh, cx + hw, cy + hh];
        }
        let tint = fade(n.tint, alpha);
        let hole = canvas.hole.get();
        if view.over_hole.contains(&i) {
            canvas.hole.set(None);
        }
        if let Some((back, uv)) = n.back.as_ref().filter(|_| !view.no_back.contains(&i)) {
            canvas.image(back, r, *uv, fade(ImColor32::WHITE, alpha));
        }
        if n.border.iter().any(Option::is_some) {
            canvas.border(&n.border, r, fade(ImColor32::WHITE, alpha));
        }
        // A rendered image goes under the node's own art (the portrait ring is drawn over the character).
        if let Some(img) = view.node_images.get(&i) {
            canvas.image(img, r, [0.0, 0.0, 1.0, 1.0], fade(ImColor32::WHITE, alpha));
        }
        if let Some(img) = view.node_art.get(&i).or(n.img.as_ref()) {
            let uv = if n.is_button() {
                let state = if greyed {
                    n.frames[3]
                } else if is_hovered && canvas.mouse_down {
                    n.frames[2]
                } else if is_hovered {
                    n.frames[1]
                } else if view.checked.contains(&i) {
                    n.frames[4].or(n.frames[1])
                } else {
                    None
                };
                state.or(n.frames[0]).unwrap_or(n.uv)
            } else {
                n.uv
            };
            match view.progress.get(&i).copied().filter(|_| n.progress_uv) {
                Some(f) => {
                    let f = f.clamp(0.0, 1.0);
                    let rr = [r[0], r[1], r[0] + (r[2] - r[0]) * f, r[3]];
                    canvas.image(img, rr, [uv[0], uv[1], uv[0] + (uv[2] - uv[0]) * f, uv[3]], tint);
                }
                None => canvas.image(img, r, uv, tint),
            }
        }
        if let Some(bar) = &n.bar {
            let f = view.progress.get(&i).copied().unwrap_or(n.position).clamp(0.0, 1.0);
            if f > 0.0 {
                if n.vertical {
                    canvas.image(bar, [r[0], r[3] - (r[3] - r[1]) * f, r[2], r[3]], [0.0, 1.0 - f, 1.0, 1.0], tint);
                } else {
                    canvas.image(bar, [r[0], r[1], r[0] + (r[2] - r[0]) * f, r[3]], [0.0, 0.0, f, 1.0], tint);
                }
            }
        }
        let style = view.style.get(&i).or(n.text_style.as_ref());
        if let Some(style) = style {
            let content = view.text.get(&i).map(String::as_str).or(n.text.as_deref());
            if let Some(content) = content.filter(|c| !c.is_empty()) {
                let mut style = style.clone();
                style.colour = fade(style.colour, alpha);
                canvas.text(content, r, &style, is_hovered);
            }
            if let Some(label) = n.label.as_deref().filter(|l| !l.is_empty()) {
                if !view.text.contains_key(&i) {
                    let mut style = style.clone().centred();
                    style.colour = fade(style.colour, alpha);
                    canvas.text(label, r, &style, is_hovered);
                }
            }
        }
        canvas.hole.set(hole);
    }
    Hits { button: hovered }
}

/// The extra text boxes and images of a view, drawn over the layout.
pub fn draw_extra(view: &View, canvas: &Canvas) {
    for (r, img, uv) in &view.backs {
        canvas.image(img, *r, *uv, ImColor32::WHITE);
    }
    for (r, colour) in &view.fills {
        canvas.draw.add_rect(canvas.at(r[0], r[1]), canvas.at(r[2], r[3]), fade(*colour, canvas.alpha))
            .filled(true).rounding(3.0 * canvas.s).build();
    }
    for (r, pieces) in &view.frames {
        canvas.border(pieces, *r, ImColor32::WHITE);
    }
    for (r, img, uv) in &view.images {
        canvas.image(img, *r, *uv, ImColor32::WHITE);
    }
    for (r, img, uv, tint) in &view.tinted {
        canvas.image(img, *r, *uv, *tint);
    }
    for (p, colour) in &view.over_tris {
        let [a, b, c] = p.map(|q| canvas.at(q[0], q[1]));
        canvas.draw.add_triangle(a, b, c, fade(*colour, canvas.alpha)).filled(true).build();
    }
    for (r, colour) in &view.over_fills {
        canvas.draw.add_rect(canvas.at(r[0], r[1]), canvas.at(r[2], r[3]), fade(*colour, canvas.alpha))
            .filled(true).build();
    }
    for (r, text, style) in &view.extra {
        canvas.text(text, *r, style, false);
    }
}
