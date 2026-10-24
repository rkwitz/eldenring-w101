//! Wizard101's Glyphis bitmap fonts (Fonts/*.xml plus atlas pages, extracted by tools/w101_hud_art.py), drawn with
//! imgui images: WizardFancy for the HUD numbers and titles, WizardNormal for the spellbook's text.

use std::collections::HashMap;
use std::path::Path;

use hudhook::imgui::{DrawListMut, ImColor32, TextureId};
use hudhook::RenderContext;

/// A Glyphis bitmap font: glyph cells in atlas pages, with Windows ABC spacing.
pub struct BitmapFont {
    pages: Vec<(TextureId, [f32; 2])>,
    glyphs: HashMap<u32, Glyph>,
    kerning: HashMap<(u32, u32), f32>,
    /// Height of a line in font pixels.
    pub line_height: f32,
}

struct Glyph {
    page: usize,
    /// Left, top, right and bottom (exclusive; rows of cells share their edges) of the cell in its page, with a
    /// 1-pixel margin each side.
    cell: [f32; 4],
    a: f32,
    b: f32,
    c: f32,
}

impl BitmapFont {
    /// Loads `<name>.xml` and its pages (`<page>.png`) from `folder`.
    pub fn load(folder: &Path, name: &str, render: &mut dyn RenderContext) -> Result<BitmapFont, String> {
        let path = folder.join(format!("{name}.xml"));
        let xml = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut font = BitmapFont { pages: Vec::new(), glyphs: HashMap::new(), kerning: HashMap::new(), line_height: 0.0 };
        for tag in xml.split('<') {
            let number = |key: &str| attribute(tag, key).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
            if tag.starts_with("Page ") {
                let file = attribute(tag, "FileName").ok_or(format!("{name}.xml: page without FileName"))?;
                let png = folder.join(Path::new(file).with_extension("png"));
                let image = image::open(&png).map_err(|e| format!("{}: {e}", png.display()))?.into_rgba8();
                let texture = render.load_texture(&image, image.width(), image.height())
                    .map_err(|e| format!("{file}: {e:?}"))?;
                font.pages.push((texture, [image.width() as f32, image.height() as f32]));
            } else if tag.starts_with("Glyph ") {
                let cell = [number("Left"), number("Top"), number("Right"), number("Bottom")].map(|v| v as f32);
                let id = number("ID") as u32;
                if (32..127).contains(&id) {
                    font.line_height = font.line_height.max(cell[3] - cell[1]);
                }
                font.glyphs.insert(id, Glyph {
                    page: number("Page") as usize,
                    cell,
                    a: number("A") as f32,
                    b: number("B") as f32,
                    c: number("C") as f32,
                });
            } else if tag.starts_with("KerningPair ") {
                font.kerning.insert((number("First") as u32, number("Second") as u32), number("Amount") as f32);
            }
        }
        if font.glyphs.values().any(|g| g.page >= font.pages.len())
            || !('0'..='9').all(|d| font.glyphs.contains_key(&(d as u32)))
        {
            return Err(format!("{name}.xml: digits or pages missing"));
        }
        Ok(font)
    }

    /// Pen advance of each character (ABC width plus kerning with the next one, plus the control's
    /// m_nKerningAdjust `tracking`), in font pixels. Characters the font lacks are drawn as '?'.
    fn advances(&self, text: &str, tracking: f32) -> Vec<(u32, f32)> {
        let chars: Vec<u32> = text
            .chars()
            .map(|c| c as u32)
            .map(|c| if self.glyphs.contains_key(&c) { c } else { '?' as u32 })
            .filter(|c| self.glyphs.contains_key(c))
            .collect();
        chars.iter().enumerate().map(|(i, c)| {
            let g = &self.glyphs[c];
            let kern = chars.get(i + 1).and_then(|next| self.kerning.get(&(*c, *next))).copied().unwrap_or(0.0);
            (*c, g.a + g.b + g.c + kern + tracking)
        }).collect()
    }

    /// Width of `text` in font pixels.
    pub fn width(&self, text: &str) -> f32 {
        self.width_tracked(text, 0.0)
    }

    pub fn width_tracked(&self, text: &str, tracking: f32) -> f32 {
        self.advances(text, tracking).iter().map(|(_, a)| a).sum()
    }

    /// `text` centred on `centre`, `scale` screen pixels per font pixel.
    pub fn draw(&self, draw: &DrawListMut, text: &str, centre: [f32; 2], scale: f32, colour: ImColor32) {
        let width = self.width(text);
        let top_left = [centre[0] - width * scale / 2.0, centre[1] - self.line_height * scale / 2.0];
        self.draw_at(draw, text, top_left, scale, colour);
    }

    /// `text` with its line's top-left corner at `top_left`.
    pub fn draw_at(&self, draw: &DrawListMut, text: &str, top_left: [f32; 2], scale: f32, colour: ImColor32) {
        self.draw_tracked(draw, text, top_left, scale, colour, 0.0);
    }

    pub fn draw_tracked(&self, draw: &DrawListMut, text: &str, top_left: [f32; 2], scale: f32, colour: ImColor32,
                        tracking: f32) {
        let mut pen = top_left[0];
        let top = top_left[1];
        for (c, advance) in self.advances(text, tracking) {
            let g = &self.glyphs[&c];
            let (texture, [w, h]) = self.pages[g.page];
            let [left, cell_top, right, bottom] = g.cell;
            let x = pen + (g.a - 1.0) * scale;
            draw.add_image(texture, [x, top], [x + (right - left) * scale, top + (bottom - cell_top) * scale])
                .uv_min([left / w, cell_top / h])
                .uv_max([right / w, bottom / h])
                .col(colour)
                .build();
            pen += advance * scale;
        }
    }

    /// `text` shortened with "..." so that it is at most `max_width` font pixels wide.
    pub fn fit(&self, text: &str, max_width: f32, tracking: f32) -> String {
        if self.width_tracked(text, tracking) <= max_width {
            return text.to_string();
        }
        let mut chars: Vec<char> = text.chars().collect();
        while !chars.is_empty() {
            chars.pop();
            let candidate = format!("{}...", chars.iter().collect::<String>().trim_end());
            if self.width_tracked(&candidate, tracking) <= max_width {
                return candidate;
            }
        }
        String::new()
    }
}

/// The value of `key="..."` in an XML tag.
fn attribute<'a>(tag: &'a str, key: &str) -> Option<&'a str> {
    let start = tag.find(&format!(" {key}=\""))? + key.len() + 3;
    tag[start..].split('"').next()
}
