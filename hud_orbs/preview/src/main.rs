//! Renders the spellbook's pages to PNG files on Linux, with made-up character data, to check layouts without the
//! game: the real book.rs / pages.rs / layout.rs / font.rs code draws into imgui, and a small software rasterizer
//! turns imgui's draw lists into an image.
//!
//! Usage: cargo run --release -- <w101_orbs folder (with book/)> <out folder> [width height]
//! (W101_PARTS = a folder of decompressed Elden Ring parts files, for the character pictures.)

#![allow(dead_code)]

#[path = "../../src/book.rs"]
mod book;
#[path = "../../src/crafting.rs"]
mod crafting;
#[path = "../../src/font.rs"]
mod font;
#[path = "../../src/layout.rs"]
mod layout;
#[path = "../../src/names.rs"]
mod names;
#[path = "../../src/pages.rs"]
mod pages;
#[path = "../../src/options.rs"]
mod options;
#[path = "../../src/settings.rs"]
mod settings;
#[path = "../../src/sheet.rs"]
mod sheet;
#[path = "../../src/itemdata.rs"]
mod itemdata;
#[path = "../../src/quests.rs"]
mod quests;
#[path = "../../src/talk.rs"]
mod talk;
#[path = "../../src/questhelper.rs"]
mod questhelper;
#[path = "../../src/steam.rs"]
mod steam;
#[path = "../../src/model.rs"]
mod model;
#[path = "../../src/markers.rs"]
mod markers;
#[path = "../../src/nameplates.rs"]
mod nameplates;
#[path = "../../src/occlusion.rs"]
mod occlusion;
#[path = "../../src/loadscreen.rs"]
mod loadscreen;
#[path = "../../src/teleport.rs"]
mod teleport;
#[path = "../../src/wisps.rs"]
mod wisps;
#[path = "../../src/fairies.rs"]
mod fairies;
#[path = "../../src/goalpopup.rs"]
mod goalpopup;
#[path = "../../src/floaters.rs"]
mod floaters;
#[path = "../../src/picker.rs"]
mod picker;
#[path = "../../src/prompt.rs"]
mod prompt;
#[path = "../../src/shop.rs"]
mod shop;
mod input;
mod game;
mod live_preview;
/// Round 22 S3/S4: the DLL's sounds (music.rs) are silent in the preview.
mod music {
    pub fn sfx(_name: &str) {}
    pub fn voice(_page: Option<(&str, usize)>) {}
    pub fn music_setting() -> Option<u8> { None }
    pub fn take_music_setting(_value: u8) -> bool { false }
    pub fn play_file(_path: std::path::PathBuf) {}
}

pub(crate) use settings::{save_settings, Settings};

use std::path::{Path, PathBuf};

use image::{Rgba, RgbaImage};
use imgui::{DrawCmd, DrawCmdParams, TextureId};

struct Render {
    textures: Vec<RgbaImage>,
    captures: [Option<TextureId>; 2],
}

impl hudhook::RenderContext for Render {
    fn load_texture(&mut self, data: &[u8], width: u32, height: u32) -> Result<TextureId, String> {
        let image = RgbaImage::from_raw(width, height, data.to_vec()).ok_or("bad texture size")?;
        self.textures.push(image);
        Ok(TextureId::new(self.textures.len()))
    }

    fn replace_texture(&mut self, texture_id: TextureId, data: &[u8], width: u32, height: u32) -> Result<(), String> {
        let image = RgbaImage::from_raw(width, height, data.to_vec()).ok_or("bad texture size")?;
        self.textures[texture_id.id() - 1] = image;
        Ok(())
    }

    /// Stand-ins for the game's frame: a sky-and-hills "world" (slot 0) and a parchment "map" with a grid (slot 1).
    fn capture_screen(&mut self, slot: usize) -> Option<TextureId> {
        if let Some(id) = self.captures[slot.min(1)] {
            return Some(id);
        }
        if slot == 1 {
            // A stand-in map screen (PREVIEW_MAP_SCREEN: a full-screen picture, e.g. with Elden Ring's hint bar).
            if let Some(i) = std::env::var_os("PREVIEW_MAP_SCREEN").and_then(|p| image::open(p).ok()) {
                let i = i.to_rgba8();
                let (w, h) = (i.width(), i.height());
                let id = self.load_texture(i.as_raw(), w, h).ok()?;
                self.captures[1] = Some(id);
                return Some(id);
            }
        }
        let (w, h) = (480u32, 270u32);
        let image = RgbaImage::from_fn(w, h, |x, y| {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            if slot == 0 {
                let hill = 0.62 + 0.06 * (fx * 9.0).sin();
                if fy < hill { image::Rgba([(90.0 + 80.0 * fy) as u8, (120.0 + 70.0 * fy) as u8, 200, 255]) }
                else { image::Rgba([60, (110.0 - 40.0 * fy) as u8, 50, 255]) }
            } else if x % 40 < 2 || y % 40 < 2 {
                image::Rgba([120, 95, 60, 255])
            } else if (fx - 0.5).abs() < 0.01 && (fy - 0.5).abs() < 0.02 {
                image::Rgba([200, 30, 30, 255])
            } else {
                image::Rgba([(196.0 - 30.0 * fy) as u8, (176.0 - 20.0 * fx) as u8, 128, 255])
            }
        });
        self.textures.push(image);
        let id = TextureId::new(self.textures.len());
        self.captures[slot.min(1)] = Some(id);
        Some(id)
    }
}

fn sample(t: &RgbaImage, u: f32, v: f32) -> [f32; 4] {
    // Bilinear, wrapping (the game's sampler wraps).
    let (w, h) = (t.width() as f32, t.height() as f32);
    let x = u * w - 0.5;
    let y = v * h - 0.5;
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let px = |xi: f32, yi: f32| {
        let xi = (xi as i64).rem_euclid(t.width() as i64) as u32;
        let yi = (yi as i64).rem_euclid(t.height() as i64) as u32;
        let p = t.get_pixel(xi, yi).0;
        [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32]
    };
    let (a, b, c, d) = (px(x0, y0), px(x0 + 1.0, y0), px(x0, y0 + 1.0), px(x0 + 1.0, y0 + 1.0));
    let mut out = [0.0; 4];
    for i in 0..4 {
        out[i] = (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy;
    }
    out
}

/// The DX12 renderer's key pixel shader (vendor hudhook dx12.rs PS_KEY), for the live mirror picture.
fn key_out(texture: &RgbaImage, t: [f32; 4]) -> [f32; 4] {
    let fixed = [23.0, 20.0, 14.0];
    let corner = sample(texture, 0.004, 0.004);
    let near = (0..3).map(|i| (corner[i] - fixed[i]).abs()).fold(0.0, f32::max) < 12.0;
    let key = if near { [corner[0], corner[1], corner[2]] } else { fixed };
    let d = (0..3).map(|i| (t[i] - key[i]).abs()).fold(0.0, f32::max);
    let a = ((d - 1.0) / 2.0).clamp(0.0, 1.0);
    let mut out = [0.0; 4];
    for i in 0..3 {
        out[i] = ((t[i] - key[i] * (1.0 - a)) / a.max(0.05)).clamp(0.0, 255.0);
    }
    out[3] = t[3] * a;
    out
}

/// Draws imgui's draw data over `canvas` (premultiplied-free alpha blending, like imgui's DX12 backend).
fn rasterize(draw_data: &imgui::DrawData, textures: &[RgbaImage], font: &RgbaImage, canvas: &mut RgbaImage) {
    let (cw, ch) = (canvas.width() as i32, canvas.height() as i32);
    for list in draw_data.draw_lists() {
        let vtx = list.vtx_buffer();
        let idx = list.idx_buffer();
        let mut additive = false;
        let mut keyed = false;
        for cmd in list.commands() {
            if let DrawCmd::RawCallback { raw_cmd, .. } = cmd {
                match unsafe { (*raw_cmd).UserCallbackData } as usize {
                    hudhook::BLEND_ADDITIVE => (additive, keyed) = (true, false),
                    hudhook::BLEND_NORMAL => (additive, keyed) = (false, false),
                    hudhook::BLEND_KEY => (additive, keyed) = (false, true),
                    _ => {}
                }
                continue;
            }
            let DrawCmd::Elements { count, cmd_params: DrawCmdParams { clip_rect, texture_id, vtx_offset, idx_offset } } =
                cmd
            else {
                continue;
            };
            let texture = match texture_id.id() {
                0 => font,
                usize::MAX => font,
                i => &textures[i - 1],
            };
            for tri in idx[idx_offset..idx_offset + count].chunks(3) {
                let v = [vtx[vtx_offset + tri[0] as usize], vtx[vtx_offset + tri[1] as usize], vtx[vtx_offset + tri[2] as usize]];
                let minx = v.iter().map(|p| p.pos[0]).fold(f32::MAX, f32::min).max(clip_rect[0]).max(0.0) as i32;
                let maxx = v.iter().map(|p| p.pos[0]).fold(f32::MIN, f32::max).min(clip_rect[2]).min(cw as f32) as i32;
                let miny = v.iter().map(|p| p.pos[1]).fold(f32::MAX, f32::min).max(clip_rect[1]).max(0.0) as i32;
                let maxy = v.iter().map(|p| p.pos[1]).fold(f32::MIN, f32::max).min(clip_rect[3]).min(ch as f32) as i32;
                let area = (v[1].pos[0] - v[0].pos[0]) * (v[2].pos[1] - v[0].pos[1])
                    - (v[2].pos[0] - v[0].pos[0]) * (v[1].pos[1] - v[0].pos[1]);
                if area.abs() < 1e-6 {
                    continue;
                }
                for y in miny..=maxy.min(ch - 1) {
                    for x in minx..=maxx.min(cw - 1) {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((v[1].pos[0] - px) * (v[2].pos[1] - py) - (v[2].pos[0] - px) * (v[1].pos[1] - py)) / area;
                        let w1 = ((v[2].pos[0] - px) * (v[0].pos[1] - py) - (v[0].pos[0] - px) * (v[2].pos[1] - py)) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                            continue;
                        }
                        let u = w0 * v[0].uv[0] + w1 * v[1].uv[0] + w2 * v[2].uv[0];
                        let vv = w0 * v[0].uv[1] + w1 * v[1].uv[1] + w2 * v[2].uv[1];
                        let mut col = [0.0f32; 4];
                        for i in 0..4 {
                            col[i] = (w0 * v[0].col[i] as f32 + w1 * v[1].col[i] as f32 + w2 * v[2].col[i] as f32) / 255.0;
                        }
                        let mut t = sample(texture, u, vv);
                        if keyed {
                            t = key_out(texture, t);
                        }
                        let a = t[3] / 255.0 * col[3];
                        if a <= 0.0 {
                            continue;
                        }
                        let dst = canvas.get_pixel_mut(x as u32, y as u32);
                        for i in 0..3 {
                            let s = t[i] * col[i];
                            let keep = if additive { 1.0 } else { 1.0 - a };
                            dst.0[i] = (s * a + dst.0[i] as f32 * keep).round().clamp(0.0, 255.0) as u8;
                        }
                        dst.0[3] = 255;
                    }
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let folder = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    let (width, height) = if args.len() > 4 { (args[3].parse().unwrap(), args[4].parse().unwrap()) } else { (1920.0, 1080.0) };
    let only: Option<&str> = args.get(5).map(String::as_str);
    std::fs::create_dir_all(&out).unwrap();

    let mut ctx = imgui::Context::create();
    ctx.set_ini_filename(None);
    ctx.io_mut().display_size = [width, height];
    let font = {
        let atlas = ctx.fonts();
        let tex = atlas.build_rgba32_texture();
        RgbaImage::from_raw(tex.width, tex.height, tex.data.to_vec()).unwrap()
    };
    ctx.fonts().tex_id = TextureId::new(usize::MAX);
    let mut render = Render { textures: Vec::new(), captures: [None, None] };
    let mut book = book::Book::new();
    book.load(&folder.join("book"), &mut render);
    let backdrop = image::open(folder.join("preview_backdrop.png")).ok().map(|i| i.to_rgba8());
    let mut settings = settings::load_settings(Path::new("/nonexistent"));
    let game = game::Game::fake();
    let stats = sheet::Stats { hp: 1210, max_hp: 1450, fp: 88, max_fp: 140, sp: 120, max_sp: 132, level: Some((60, 12000)) };

    // (name, tab, sub-page, mouse as a fraction of the screen (0,0 = none), what to show: "" book, "map", "talk:<npc>")
    let shots: Vec<(&str, pages::Tab, usize, [f32; 2], &str)> = vec![
        ("1_character", pages::Tab::Character, 0, [0.0, 0.0], ""),
        ("1b_character_equipment", pages::Tab::Character, 1, [0.0, 0.0], ""),
        ("1c_character_flasks_tip", pages::Tab::Character, 0, [0.445, 0.83], ""),
        ("2_backpack", pages::Tab::Backpack, 0, [0.609, 0.282], ""),
        ("2b_backpack_weapons", pages::Tab::Backpack, 4, [0.609, 0.282], ""),
        ("2c_backpack_armour", pages::Tab::Backpack, 1, [0.74, 0.30], ""),
        ("2d_backpack_talismans", pages::Tab::Backpack, 6, [0.74, 0.30], ""),
        ("2e_backpack_arrows", pages::Tab::Backpack, 7, [0.74, 0.30], ""),
        // Round 21 (book B3): a stack's hover (the count is only in the tooltip, not on the row).
        ("2h_backpack_count_tip", pages::Tab::Backpack, 0, [0.62, 0.34], ""),
        // Round 16 (book): the Equipment Sets page, a set's hover, a bookmark's hover, a stat's hover.
        ("2f_backpack_gear_sets", pages::Tab::Backpack, 0, [0.0, 0.0], "gear"),
        ("2g_backpack_gear_tip", pages::Tab::Backpack, 0, [0.70, 0.335], "gear"),
        ("1e_bookmark_tip", pages::Tab::Character, 0, [0.765, 0.235], ""),
        ("1f_stat_tip", pages::Tab::Character, 0, [0.515, 0.42], ""),
        ("3_pets", pages::Tab::Pets, 0, [0.0, 0.0], ""),
        ("3b_pets_ash", pages::Tab::Pets, 0, [0.0, 0.0], "pet:1"),
        ("4_deck", pages::Tab::Deck, 0, [0.565, 0.35], ""),
        ("4b_deck_fire", pages::Tab::Deck, 1, [0.0, 0.0], ""),
        ("4c_deck_astral", pages::Tab::Deck, 8, [0.0, 0.0], ""),
        ("5_quests", pages::Tab::Quests, 0, [0.0, 0.0], ""),
        ("6_map", pages::Tab::Map, 0, [0.0, 0.0], ""),
        // Wizard101's maps: on Unicorn Way (the player near the Ceren Nightchant / Olivia corner, the cursor on the
        // grace), and at the Roundtable Hold.
        ("6c_map_street", pages::Tab::Map, 0, [0.344, 0.586], "place:street"),
        ("6d_map_roundtable", pages::Tab::Map, 0, [0.5, 0.5], "place:roundtable"),
        ("33map1_maze", pages::Tab::Map, 0, [0.99, 0.99], "place:maze"),
        ("33map1_rattlebones_door", pages::Tab::Map, 0, [0.99, 0.99], "place:rbdoor"),
        ("33map1_blackhope_door", pages::Tab::Map, 0, [0.99, 0.99], "place:bhdoor"),
        ("33map1_diego", pages::Tab::Map, 0, [0.99, 0.99], "place:diego"),
        ("6b_map_open", pages::Tab::Map, 0, [0.0, 0.0], "map"),
        // Round 16 (map): the bosses' plates, the magnifier's plain tooltip on both maps.
        ("6e_map_rattlebones", pages::Tab::Map, 0, [0.6044, 0.4967], "place:street"),
        ("6f_map_blackhope", pages::Tab::Map, 0, [0.3737, 0.4225], "place:street"),
        ("6g_map_zoom_tip", pages::Tab::Map, 0, [0.2773, 0.8306], "place:street"),
        ("6h_map_open_zoom_tip", pages::Tab::Map, 0, [0.2773, 0.8306], "map"),
        ("6i_map_home_tip", pages::Tab::Map, 0, [0.716, 0.8306], "place:street"),
        // Round 26 (W6, W8): the home button's tip in the Roundtable Hold says Unicorn Way; red boss pictures; with no
        // boss quest assigned ("noboss" in the name: the game's case) no boss shows, Rattlebones hovered.
        ("26a_map_roundtable_home_tip", pages::Tab::Map, 0, [0.6705, 0.8306], "place:roundtable"),
        ("26b_map_noboss_rattlebones_spot", pages::Tab::Map, 0, [0.6044, 0.4967], "place:street"),
        ("26c_map_rattlebones_red", pages::Tab::Map, 0, [0.6044, 0.4967], "place:street"),
        ("7_crafting", pages::Tab::Crafting, 0, [0.0, 0.0], ""),
        ("7g_crafting_tab_tip", pages::Tab::Crafting, 0, [0.318, 0.198], ""),
        ("7b_crafting_materials", pages::Tab::Crafting, 0, [0.0, 0.0], "materials"),
        ("7c_crafting_pots_tip", pages::Tab::Crafting, 1, [0.62, 0.41], "recipe:3"),
        ("7d_crafting_food_tip", pages::Tab::Crafting, 4, [0.314, 0.66], "recipe:44"),
        ("7e_crafting_arrows", pages::Tab::Crafting, 3, [0.0, 0.0], "recipe:91"),
        ("7f_crafting_made", pages::Tab::Crafting, 1, [0.0, 0.0], "made:1"),
        ("34cr1_made_1.4s", pages::Tab::Crafting, 1, [0.0, 0.0], "made:1@1.4"),
        ("34cr1_stamp_1.85s", pages::Tab::Crafting, 1, [0.0, 0.0], "made:1@1.85"),
        ("34cr1_stamp_2.2s", pages::Tab::Crafting, 1, [0.0, 0.0], "made:1@2.2"),
        // Round 16: the materials tabs' plain tooltip (ToolTip.gui; 7g: the recipe tabs').
        ("7h_crafting_materials_tab_tip", pages::Tab::Crafting, 1, [0.309, 0.203], "materials"),
        ("7i_crafting_materials_card_hover", pages::Tab::Crafting, 0, [0.30, 0.40], "materials"),
        ("8_help", pages::Tab::Help, 1, [0.0, 0.0], ""),
        // Round 15 (book): badge hover, a memory slot's card, the help topics' added spreads.
        ("1d_badge_tip", pages::Tab::Character, 1, [0.63, 0.33], ""),
        ("4d_deck_slot_card", pages::Tab::Deck, 0, [0.26, 0.336], ""),
        ("8b_help_questing_2", pages::Tab::Help, 0, [0.0, 0.0], "page:1"),
        ("8c_help_questing_3", pages::Tab::Help, 0, [0.0, 0.0], "page:2"),
        ("8d_help_combat_2", pages::Tab::Help, 1, [0.0, 0.0], "page:1"),
        ("8e_help_combat_3", pages::Tab::Help, 1, [0.0, 0.0], "page:2"),
        ("8f_help_interface_2", pages::Tab::Help, 2, [0.0, 0.0], "page:1"),
        ("8g_help_character_2", pages::Tab::Help, 3, [0.0, 0.0], "page:1"),
        ("8h_help_crafting", pages::Tab::Help, 4, [0.0, 0.0], ""),
        ("8i_help_crafting_2", pages::Tab::Help, 4, [0.0, 0.0], "page:1"),
        ("8j_help_pets", pages::Tab::Help, 5, [0.0, 0.0], ""),
        ("8k_help_pets_2", pages::Tab::Help, 5, [0.0, 0.0], "page:1"),
        ("9_settings", pages::Tab::Settings, 0, [0.0, 0.0], ""),
        ("9a_settings_advvideo", pages::Tab::Settings, 0, [0.0, 0.0], "settings:1"),
        ("9b_settings_audio", pages::Tab::Settings, 0, [0.0, 0.0], "settings:2"),
        ("9c_settings_gameplay1", pages::Tab::Settings, 0, [0.0, 0.0], "settings:3"),
        ("9d_settings_camera", pages::Tab::Settings, 0, [0.0, 0.0], "settings:4"),
        ("9e_settings_gameplay3", pages::Tab::Settings, 0, [0.0, 0.0], "settings:5"),
        ("9f_settings_hud", pages::Tab::Settings, 0, [0.0, 0.0], "settings:6"),
        ("9g_settings_display", pages::Tab::Settings, 0, [0.0, 0.0], "settings:7"),
        ("9h_settings_morevideo", pages::Tab::Settings, 0, [0.0, 0.0], "settings:8"),
        ("9i_settings_testing", pages::Tab::Settings, 0, [0.0, 0.0], "settings:9"),
        ("9j_settings_row_hover", pages::Tab::Settings, 0, [0.70, 0.58], ""),
        ("9k_settings_tab_tip", pages::Tab::Settings, 0, [0.4215, 0.193], "settings:3"),
        ("11_markers", pages::Tab::Character, 0, [0.0, 0.0], "markers"),
        ("11b_markers_close_connelly", pages::Tab::Character, 0, [0.0, 0.0], "markers:5.0029,94.1828,-44.0778"),
        ("11c_markers_close_ceren", pages::Tab::Character, 0, [0.0, 0.0], "markers:-84.8379,94.4878,-18.4484"),
        ("11d_markers_close_olivia", pages::Tab::Character, 0, [0.0, 0.0], "markers:-65.8655,92.8248,-7.9762"),
        // Round 18 (G1/G2): Lady Oriel (not "loaded": her MSB spot) and Private O'Ryan up close.
        ("11e_markers_close_oriel", pages::Tab::Character, 0, [0.0, 0.0], "markers:-335.1056,93.4988,103.5473"),
        ("11f_markers_close_oryan", pages::Tab::Character, 0, [0.0, 0.0], "markers:-160.8270,93.4878,-54.0237"),
        // Round 18 (G3): the Elden Ring faces in the portrait (Lady Oriel = Melina, Olivia, O'Ryan).
        ("10h_dialogue_oriel", pages::Tab::Character, 0, [0.0, 0.0], "talk:lady_oriel|monsters_and_mazes|goal2:0"),
        ("10i_dialogue_olivia", pages::Tab::Character, 0, [0.0, 0.0], "talk:olivia_dawnwillow|idle:0"),
        ("10j_dialogue_oryan", pages::Tab::Character, 0, [0.0, 0.0], "talk:private_oryan|saving_private_oryan|goal2:0"),
        ("10_dialogue", pages::Tab::Character, 0, [0.0, 0.0], "talk:ceren_nightchant|ghost_hunters|goal3:0"),
        ("10b_dialogue_page2", pages::Tab::Character, 0, [0.62, 0.86], "talk:ceren_nightchant|ghost_hunters|goal3:1"),
        ("10c_dialogue_last", pages::Tab::Character, 0, [0.0, 0.0], "talk:private_connelly|idle:0"),
        ("10d_offer", pages::Tab::Character, 0, [0.0, 0.0], "talk:olivia_dawnwillow|pesky_pirates|offer:0"),
        ("10e_offer_accept", pages::Tab::Character, 0, [0.0, 0.0], "talk:olivia_dawnwillow|pesky_pirates|offer:1"),
        ("10f_offer_oryan", pages::Tab::Character, 0, [0.0, 0.0], "talk:private_connelly|saving_private_oryan|offer:1"),
        ("10g_intro_gamma", pages::Tab::Character, 0, [0.0, 0.0], "talk:intro|unicorn_way:0"),
        // Round 18 (quests F1/F3/F4): Ambrose's offer, Gamma's tip, a goal that offers the next quest (its card with
        // the goal's picture), Olivia cutting in, the mouse on yes, and the Quest Helper arrow.
        ("15a_intro_ambrose", pages::Tab::Character, 0, [0.0, 0.0], "talk:intro|unicorn_way:0"),
        ("15b_intro_offer_card", pages::Tab::Character, 0, [0.0, 0.0], "talk:intro|unicorn_way:2"),
        ("15c_intro_after_gamma", pages::Tab::Character, 0, [0.0, 0.0], "talk:intro|unicorn_way|after:1"),
        ("15d_connelly_goal1_page1", pages::Tab::Character, 0, [0.0, 0.0], "talk:private_connelly|unicorn_way|goal1:0"),
        ("15e_connelly_goal1_offer_ghost_hunters", pages::Tab::Character, 0, [0.0, 0.0], "talk:private_connelly|unicorn_way|goal1:5"),
        ("15f_ceren_olivia_cuts_in", pages::Tab::Character, 0, [0.0, 0.0], "talk:ceren_nightchant|ghost_hunters|goal3:4"),
        ("15g_ceren_offer_skeleton_crew_mouse_yes", pages::Tab::Character, 0, [0.582, 0.873], "talk:ceren_nightchant|ghost_hunters|goal3:5"),
        ("15h_oriel_offer_finding_out_why", pages::Tab::Character, 0, [0.0, 0.0], "talk:lady_oriel|monsters_and_mazes|goal2:5"),
        ("15i_accepted_gamma", pages::Tab::Character, 0, [0.0, 0.0], "talk:private_connelly|ghost_hunters|accepted:0"),
        // Round 20 (quests Q2-Q8): Connelly's first talk (Unicorn Way's conversation) ending in the Ghost Hunters
        // card (Spirit Jellyfish picture, 0/2 over its bottom, the rune reward, yes / no); the Quests page with the
        // tracked card (glow, tracker arrow); the Quest Helper with its steps.
        ("20a_connelly_first_talk", pages::Tab::Character, 0, [0.0, 0.0], "talk:private_connelly|unicorn_way|goal1:0"),
        ("20b_ghost_hunters_card_yes_no", pages::Tab::Character, 0, [0.0, 0.0], "talk:private_connelly|unicorn_way|goal1:5"),
        ("20c_skeleton_crew_card", pages::Tab::Character, 0, [0.0, 0.0], "talk:ceren_nightchant|ghost_hunters|goal3:5"),
        ("20d_finding_out_why_card", pages::Tab::Character, 0, [0.0, 0.0], "talk:lady_oriel|monsters_and_mazes|goal2:5"),
        ("20e_quests_page_tracked", pages::Tab::Quests, 0, [0.0, 0.0], ""),
        ("20f_quest_helper_steps", pages::Tab::Character, 0, [0.0, 0.0], "turn:0"),
        ("20g_quest_helper_steps_turned", pages::Tab::Character, 0, [0.0, 0.0], "turn:-70"),
        // Round 21 (quests Q3-Q6): goal picture on Wizard101's red enemy disc, centred with its count; the rune on
        // Wizard101's gold reward symbol; the selected quest's paper lit (no border); the Quest Helper.
        ("21a_ghost_hunters_card", pages::Tab::Character, 0, [0.0, 0.0], "talk:private_connelly|unicorn_way|goal1:5"),
        ("21b_skeleton_crew_card", pages::Tab::Character, 0, [0.0, 0.0], "talk:ceren_nightchant|ghost_hunters|goal3:5"),
        ("21c_monsters_and_mazes_card", pages::Tab::Character, 0, [0.0, 0.0], "talk:ceren_nightchant|skeleton_crew|goal2:5"),
        ("21d_quests_page_selected_paper", pages::Tab::Quests, 0, [0.0, 0.0], ""),
        ("21e_quest_helper", pages::Tab::Character, 0, [0.0, 0.0], "turn:0"),
        // Round 22 (hud H1-H8): helper over the experience bar with Wizard101's solid arrow (ahead, left, right,
        // behind), the dialogue over the bar, the Monsters and Mazes card's picture, the quest page's pulse
        // (PREVIEW_PULSE_T=0 dim, =1.2 bright), the markers.
        ("22a_helper_ahead", pages::Tab::Character, 0, [0.0, 0.0], "turn:0"),
        ("22b_helper_left", pages::Tab::Character, 0, [0.0, 0.0], "turn:70"),
        ("22c_helper_right", pages::Tab::Character, 0, [0.0, 0.0], "turn:-70"),
        ("22d_helper_behind", pages::Tab::Character, 0, [0.0, 0.0], "turn:160"),
        ("22e_dialogue_over_bar", pages::Tab::Character, 0, [0.0, 0.0], "talk:ceren_nightchant|ghost_hunters|goal3:0"),
        ("22f_monsters_and_mazes_card", pages::Tab::Character, 0, [0.0, 0.0], "talk:ceren_nightchant|monsters_and_mazes|offer:9"),
        ("22g_quests_page_pulse", pages::Tab::Quests, 0, [0.0, 0.0], ""),
        ("22h_markers", pages::Tab::Character, 0, [0.0, 0.0], "markers"),
        // Round 23 (hud H4, H6): a wisp up close (fuller), and the pickup spiral round the player at these seconds.
        ("23f_wisp_idle", pages::Tab::Character, 0, [0.0, 0.0], "wisp:"),
        // Round 30 (FA1): the hedge maze fairies from tools/blender_fairy_sheet.py, as ref 6 frames them.
        ("30_fairies_0.0", pages::Tab::Character, 0, [0.0, 0.0], "fairy:0.0"),
        ("30_fairies_0.7", pages::Tab::Character, 0, [0.0, 0.0], "fairy:0.7"),
        // Round 23 (hud H7): Wizard101's quest popups (WizardPopupText.gui, Quest.lang), shown and fading.
        // Round 25 (H1): the stacked yellow popups as in Ryan's ref/3_w101_stacked_popups.png and ref/1.
        ("25a_popups_stacked", pages::Tab::Character, 0, [0.0, 0.0], "popup:5.1|Quest Complete: Who could it be now?;;1.0|New Quest: Sinister Skeleton;;0.5|New Goal: Search for Rattlebones (The Archives)"),
        ("25b_popup_quest_complete", pages::Tab::Character, 0, [0.0, 0.0], "popup:1.0|Quest Complete: Rattlebones Report"),
        ("25c_popup_ref_text", pages::Tab::Character, 0, [0.0, 0.0], "popup:5.0|Quest Complete: Rattlebones Report;;1.0|New Quest: Panic in Three Streets!;;1.0|New Quest: Enrollment"),
        // Round 25 (H2): Wizard101's quest picker (NPCServices.gui) for an NPC asked for two conversations at once.
        ("25d_picker_ceren", pages::Tab::Character, 0, [0.0, 0.0], "picker:0|ceren_nightchant|skeleton_crew|offer,ceren_nightchant|monsters_and_mazes|offer"),
        ("25e_picker_ceren_three_focus2", pages::Tab::Character, 0, [0.0, 0.0], "picker:1|ceren_nightchant|who_could_it_be_now|goal1,ceren_nightchant|sinister_skeleton|offer,ceren_nightchant|skeleton_crew|underway"),
        // Round 25 (H3): Wizard101's interact prompt (keyboard E, controller A, the cage's Examine) with the helper over it.
        ("25f_prompt_talk_keyboard", pages::Tab::Character, 0, [0.0, 0.0], "prompt:ceren_nightchant|0"),
        ("25g_prompt_talk_pad", pages::Tab::Character, 0, [0.0, 0.0], "prompt:lady_oriel|1"),
        ("25h_prompt_examine_cage", pages::Tab::Character, 0, [0.0, 0.0], "prompt:cage|0"),
        ("25i_prompt_connelly", pages::Tab::Character, 0, [0.0, 0.0], "prompt:private_connelly|0"),
        // Round 26 (N5/N6): own glyphs: the key cap without the black outline, Xbox face buttons, a shoulder button.
        ("26a_prompt_key_e", pages::Tab::Character, 0, [0.0, 0.0], "prompt:ceren_nightchant|0"),
        ("26b_prompt_pad_y", pages::Tab::Character, 0, [0.0, 0.0], "prompt:private_connelly|Y"),
        // Round 29 (P1): the name's size and place against Ryan's Wizard101 screenshot (round25/ref/4_w101_press_to_talk.png).
        ("29p1_prompt_cage", pages::Tab::Character, 0, [0.0, 0.0], "prompt:cage|0"),
        ("26c_prompt_pad_a", pages::Tab::Character, 0, [0.0, 0.0], "prompt:diego|A"),
        ("26d_prompt_pad_rb", pages::Tab::Character, 0, [0.0, 0.0], "prompt:ugo_kalahad|RB"),
        // Round 27 (B2): Ugo's shop: Buy (Hats, Wands with a row's tooltip), Sell (Wands, Robes).
        ("27p_shop_buy_hats", pages::Tab::Character, 0, [0.0, 0.0], "shop:buy|0|0"),
        ("27q_shop_buy_wands_tip", pages::Tab::Character, 0, [0.60, 0.42], "shop:buy|3|2"),
        ("27r_shop_sell_wands", pages::Tab::Character, 0, [0.0, 0.0], "shop:sell|3|0"),
        ("27s_shop_sell_robes", pages::Tab::Character, 0, [0.0, 0.0], "shop:sell|1|0"),
        // Round 28 (S1): Ugo's purse a fixed size and gap over his plate's top, 5.5, 12 and 25 m away.
        // Round 29 (M1): the purse at its world size: 3 m, 5 m, 12 m, 25 m from Ugo.
        ("29m1a_ugo_purse_3m", pages::Tab::Character, 0, [0.0, 0.0], "markers:-29.7745,94.5018,-59.8978"),
        ("28e_ugo_purse_5m", pages::Tab::Character, 0, [0.0, 0.0], "markers:-29.7745,94.5018,-61.8978"),
        ("28f_ugo_purse_12m", pages::Tab::Character, 0, [0.0, 0.0], "markers:-29.7745,94.5018,-68.3978"),
        ("28g_ugo_purse_25m", pages::Tab::Character, 0, [0.0, 0.0], "markers:-29.7745,94.5018,-81.3978"),
        // Round 28 (B1-B9): Elden Ring names, centred Buy rows, the Robes tab's hover bubble, a Sell row's tooltip.
        // Round 30 (SH1, SH2): ref 4 (Ryan's Wizard101 shop after buying a wand): SOLD in the picture box, BUY on its plate.
        ("30sh_shop_buy_wand_sold", pages::Tab::Character, 0, [0.0, 0.0], "shop:buy|3|0|sold"),
        ("30sh_shop_buy_wand", pages::Tab::Character, 0, [0.0, 0.0], "shop:buy|3|0"),
        ("28a_shop_buy_hats", pages::Tab::Character, 0, [0.0, 0.0], "shop:buy|0|0"),
        ("28b_shop_buy_robes_tab_hover", pages::Tab::Character, 0, [0.30, 0.245], "shop:buy|1|0"),
        ("28c_shop_buy_wands_tip", pages::Tab::Character, 0, [0.60, 0.42], "shop:buy|3|2"),
        ("28d_shop_sell_wands_tip", pages::Tab::Character, 0, [0.62, 0.36], "shop:sell|3|0"),
        // Round 29 (Q3): Finding Out Why's two open goals, one per card page, flipped by the card's red arrows.
        ("29q3a_quests_goal_page1", pages::Tab::Quests, 0, [0.0, 0.0], "goalpage:0"),
        ("29q3b_quests_goal_page2", pages::Tab::Quests, 0, [0.0, 0.0], "goalpage:1"),
        ("23l_popup_goal_update", pages::Tab::Character, 0, [0.0, 0.0], "popup:0.5|Goal Update: Defeat Lost Souls 1 of 2"),
        ("23m_popup_goal_complete", pages::Tab::Character, 0, [0.0, 0.0], "popup:1.0|Goal Complete: Defeat Lost Souls"),
        ("23n_popup_quest_complete_fading", pages::Tab::Character, 0, [0.0, 0.0], "popup:3.5|Quest Complete: Ghost Hunters"),
        // Round 30 (FT1-FT5): Wizard101's floating numbers (refs in /mnt/project-files/round30/ref), and the rise and fade.
        ("30a_float_heal", pages::Tab::Character, 0, [0.0, 0.0], "float:heal:110:p:0.2"),
        ("30b_float_damage_dealt", pages::Tab::Character, 0, [0.0, 0.0], "float:damage:105:e:0.2"),
        ("30c_float_damage_taken", pages::Tab::Character, 0, [0.0, 0.0], "float:damage:42:p:0.2"),
        ("30d_float_runes", pages::Tab::Character, 0, [0.0, 0.0], "float:runes:25:p:0.2"),
        ("30e_float_victory", pages::Tab::Character, 0, [0.0, 0.0], "float:victory:0:p:0.2;runes:1500:p:0.2"),
        ("30f_float_mana", pages::Tab::Character, 0, [0.0, 0.0], "float:mana:24:p:0.2"),
        ("30g_float_heal_1.0s", pages::Tab::Character, 0, [0.0, 0.0], "float:heal:110:p:1.0"),
        ("30h_float_heal_1.6s", pages::Tab::Character, 0, [0.0, 0.0], "float:heal:110:p:1.6"),
        // Round 31 (FT6): from each one's head top, rising faster.
        ("31ft6a_damage_sprite_0s", pages::Tab::Character, 0, [0.0, 0.0], "float:damage:105:e:0.0"),
        ("31ft6b_damage_lost_soul_0s", pages::Tab::Character, 0, [0.0, 0.0], "float:damage:105:e4180:0.0"),
        ("31ft6c_damage_sprite_1s", pages::Tab::Character, 0, [0.0, 0.0], "float:damage:105:e:1.0"),
        ("31ft6d_heal_0s", pages::Tab::Character, 0, [0.0, 0.0], "float:heal:110:p:0.0"),
        // Round 32 (U7): VICTORY! fixed at the top centre (0.2 s, and 2.6 s fading); U6: an item's name.
        ("32u7_victory_0.2s", pages::Tab::Character, 0, [0.0, 0.0], "float:victory:0:p:0.2;runes:1500:p:0.2"),
        ("32u7_victory_2.6s", pages::Tab::Character, 0, [0.0, 0.0], "float:victory:0:p:2.6"),
        ("32u6_item_0.3s", pages::Tab::Character, 0, [0.0, 0.0], "float:item:2900:p:0.3"),
        ("31ft6e_heal_1s", pages::Tab::Character, 0, [0.0, 0.0], "float:heal:110:p:1.0"),
        ("23g_wisp_pickup_0.3", pages::Tab::Character, 0, [0.0, 0.0], "wisp:0.3"),
        ("23h_wisp_pickup_0.8", pages::Tab::Character, 0, [0.0, 0.0], "wisp:0.8"),
        ("23i_wisp_pickup_1.4", pages::Tab::Character, 0, [0.0, 0.0], "wisp:1.4"),
        ("23j_wisp_pickup_1.8", pages::Tab::Character, 0, [0.0, 0.0], "wisp:1.8"),
        // Round 23b: the classic quest text (longest page, a card page, Oriel's narration page, an idle line).
        ("23b_classic_ceren_rattlebones", pages::Tab::Character, 0, [0.0, 0.0], "talk:ceren_nightchant|sinister_skeleton|goal4:0"),
        ("23b_classic_oriel_offer_card", pages::Tab::Character, 0, [0.0, 0.0], "talk:lady_oriel|finding_out_why|offer:3"),
        ("23b_classic_oriel_narration", pages::Tab::Character, 0, [0.0, 0.0], "talk:lady_oriel|finding_out_why|goal4:3"),
        ("23b_classic_oriel_idle", pages::Tab::Character, 0, [0.0, 0.0], "talk:lady_oriel|idle:0"),
        ("23b_classic_connelly_offer", pages::Tab::Character, 0, [0.0, 0.0], "talk:private_connelly|unicorn_way|goal1:0"),
        ("14a_quest_helper_ahead", pages::Tab::Character, 0, [0.0, 0.0], "turn:0"),
        ("14b_quest_helper_turned_left", pages::Tab::Character, 0, [0.0, 0.0], "turn:-70"),
        ("14c_quest_helper_behind", pages::Tab::Character, 0, [0.0, 0.0], "turn:150"),
        // The loading screen, then Wizard101's page turn that many seconds in.
        ("12_loadscreen", pages::Tab::Character, 0, [0.0, 0.0], "load:"),
        ("12b_flip_0.35", pages::Tab::Character, 0, [0.0, 0.0], "load:0.35"),
        ("12c_flip_0.55", pages::Tab::Character, 0, [0.0, 0.0], "load:0.55"),
        ("12d_flip_0.70", pages::Tab::Character, 0, [0.0, 0.0], "load:0.70"),
        ("12e_flip_0.85", pages::Tab::Character, 0, [0.0, 0.0], "load:0.85"),
        ("12f_flip_0.95", pages::Tab::Character, 0, [0.0, 0.0], "load:0.95"),
    ];
    // Wizard101's teleport poof (teleport.rs) at these times, on feet at the origin (compare with
    // tools/w101_poof_reference.py's frames of the .nif itself).
    let poof_times = ["0.05", "0.12", "0.20", "0.30", "0.40", "0.55", "0.75", "1.00", "1.40"];
    let mut shots = shots;
    let names: Vec<String> = poof_times.iter().map(|t| format!("13_poof_{t}")).collect();
    let shows: Vec<String> = poof_times.iter().map(|t| format!("poof:{t}")).collect();
    for (n, sh) in names.iter().zip(&shows) {
        shots.push((Box::leak(n.clone().into_boxed_str()), pages::Tab::Character, 0, [0.0, 0.0], Box::leak(sh.clone().into_boxed_str())));
    }
    // Round 24: PREVIEW_SHOW=<name>=<show> adds one shot (e.g. a camera turn for the quest arrow).
    if let Some((n, sh)) = std::env::var("PREVIEW_SHOW").ok().and_then(|v| v.split_once('=').map(|(a, b)| (a.to_string(), b.to_string()))) {
        shots.push((Box::leak(n.into_boxed_str()), pages::Tab::Character, 0, [0.0, 0.0], Box::leak(sh.into_boxed_str())));
    }
    let mut load_screen = loadscreen::LoadScreen::default();
    book.textures.want([loadscreen::IMAGE.to_string()]);
    let settings_image = settings::load_settings(Path::new("/nonexistent")).loadscreen_image;
    load_screen.load(folder.parent().unwrap_or(&folder), &settings_image, &folder.join("book").join("page_flip.tsv"), &mut render);
    // Round 18 A3: Elden Ring's mirror picture (a probe dump) in the character pictures, as the DLL shows it.
    let mirror = std::env::var_os("PREVIEW_LIVE_MIRROR").and_then(|p| image::open(p).ok()).map(|i| {
        let i = i.to_rgba8();
        let (w, h) = (i.width(), i.height());
        (hudhook::RenderContext::load_texture(&mut render, i.as_raw(), w, h).unwrap(), [w as f32, h as f32])
    });
    for (name, tab, sub, mouse, show) in shots {
        if only.is_some_and(|o| !name.contains(o)) {
            continue;
        }
        book.preview_show(tab, sub);
        book.map_bosses_all = !name.contains("noboss");
        if show == "map" {
            book.capture(&mut render, true); // the world as it was before the map opened
            book.preview_map();
        }
        *game::CLOSE.lock().unwrap() = None;
        *game::TURN.lock().unwrap() = show.strip_prefix("turn:").and_then(|t| t.parse::<f32>().ok()).unwrap_or(0.0).to_radians();
        if show.starts_with("turn:") || show.starts_with("popup:") {
            book.preview_closed();
        }
        // Round 25 (H2): "picker:<focus>|key,key"; (H3): "prompt:<npc slug or cage>|<pad 0/1>".
        if let Some((focus, keys)) = show.strip_prefix("picker:").and_then(|p| p.split_once('|')) {
            book.preview_closed();
            book.preview_picker(&keys.split(',').collect::<Vec<_>>(), focus.parse().unwrap_or(0));
        }
        // Round 27 (B2): "shop:<buy|sell>|<category 0-3>|<row>": Ugo's shop window.
        if let Some(rest) = show.strip_prefix("shop:") {
            book.preview_closed();
            let f: Vec<&str> = rest.split('|').collect();
            let num = |i: usize| f.get(i).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
            book.preview_shop(f.first() == Some(&"sell"), num(1), num(2), game.sheet());
            if f.get(3) == Some(&"sold") {
                book.preview_shop_sold();
            }
        }
        if let Some((who, pad)) = show.strip_prefix("prompt:").and_then(|p| p.split_once('|')) {
            book.preview_closed();
            let target = match markers::NPCS.iter().find(|n| n.0 == who) {
                Some(n) => prompt::Target { name: n.1.into(), portrait: format!("{}/{}.png", markers::ER_PORTRAITS, n.0), examine: false },
                None => prompt::Target { name: "Bone Cage".into(), portrait: "GUI/QuestButtons/Art_HD_WC_BoneCage.png".into(), examine: true },
            };
            // Round 26 (N5/N6): "<pad 0/1>" or the pad button's label (Y, A, LB ...).
            if pad != "0" && pad != "1" {
                book.prompt.set_pad_button(pad);
            }
            book.preview_prompt(Some(target), pad != "0");
        } else {
            book.preview_prompt(None, false);
        }
        // Round 25 (H1): "popup:<age>|<text>;;<age>|<text>..." oldest first.
        if !show.starts_with("popup:") {
            book.popups.preview(&[]);
        }
        if let Some(list) = show.strip_prefix("popup:") {
            let lines: Vec<(&str, f32)> = list.split(";;").filter_map(|p| p.split_once('|'))
                .map(|(age, text)| (text, age.parse().unwrap_or(0.0))).collect();
            book.popups.preview(&lines);
        }
        // Round 30 (FT1-FT5): "float:<kind>:<amount>:<p|e>:<age>;..." over the player (p) or the Dark Sprite (e).
        if let Some(list) = show.strip_prefix("float:") {
            book.preview_closed();
            // Elden Ring's third-person camera: 3.6 m behind the player and over the head, facing the Dark Sprite
            // (-17.5, 94, -106) 3.5 m ahead.
            // Shots with nothing over the enemy look the other way, down the street, so only the player's are seen.
            let player = [-18.5f32, 94.0, -109.5];
            let back = if list.contains(":e") { 1.0 } else { -1.0 };
            *game::VIEW.lock().unwrap() = Some(([player[0] - 0.9 * back, player[1] + 2.0, player[2] - 3.8 * back],
                                                [player[0] + 0.3 * back, player[1] + 1.5, player[2] + 1.5 * back], player));
            // Round 31 (FT6): over an enemy's head top (nameplates::head): "e" the Dark Sprite, "e<model>" another.
            let over = |model: u32| game.enemies(&[model]).first().and_then(|e| nameplates::head(e.0, e.1, e.2, e.3));
            let items: Vec<_> = list.split(';').filter_map(|f| {
                let v: Vec<&str> = f.split(':').collect();
                let n: i32 = v.get(1)?.parse().ok()?;
                let kind = match *v.first()? {
                    "heal" => floaters::Kind::Heal(n),
                    "mana" => floaters::Kind::Mana(n),
                    "damage" => floaters::Kind::Damage(n),
                    "runes" => floaters::Kind::Runes(n as u32),
                    // Round 32 (U6): an item gained (named below with the preview's own sample name).
                    "item" => floaters::Kind::Item(n as u32, 1),
                    _ => floaters::Kind::Victory,
                };
                let on = v.get(2)?.strip_prefix('e').map(|m| over(m.parse().unwrap_or(3450))).unwrap_or(None);
                Some((kind, on, v.get(3)?.parse().ok()?))
            }).collect();
            for (kind, _, _) in &items {
                if let floaters::Kind::Item(id, _) = kind {
                    book.floaters.name_item(*id, "Golden Rune [1]");
                }
            }
            book.preview_floaters(&items);
        }
        if let Some(at) = show.strip_prefix("markers") {
            book.preview_closed();
            let p: Vec<f32> = at.trim_start_matches(':').split(',').filter_map(|v| v.parse().ok()).collect();
            if p.len() == 3 {
                *game::CLOSE.lock().unwrap() = Some([p[0], p[1], p[2]]);
            }
        }
        if show == "gear" {
            book.preview_gear(&game);
        }
        if show == "materials" {
            book.preview_materials();
        }
        if let Some(r) = show.strip_prefix("recipe:") {
            book.preview_recipe(r.parse().unwrap(), None);
        }
        if let Some(r) = show.strip_prefix("made:") {
            // made:<recipe>[@<seconds since the craft>] (round 34 CR1: the panel's fade and the stamp)
            let (r, ago) = r.split_once('@').map_or((r, 0.0), |(r, a)| (r, a.parse().unwrap()));
            book.preview_recipe(r.parse().unwrap(), Some(ago));
        }
        let load = show.strip_prefix("load:");
        if load.is_some() || show.starts_with("poof") || show.starts_with("wisp:") || show.starts_with("fairy:") {
            book.preview_closed();
        }
        if let Some(tab) = show.strip_prefix("settings:") {
            book.preview_settings(tab.parse().unwrap());
        }
        if let Some(page) = show.strip_prefix("page:") {
            book.preview_page(page.parse().unwrap());
        }
        if let Some(pet) = show.strip_prefix("pet:") {
            book.preview_pet(pet.parse().unwrap());
        }
        // A few frames: the first ones queue the art, which loads between frames.
        for frame in 0..4 {
            // Round 29 (Q3): once the first frame has read the quests in progress.
            if let Some(page) = show.strip_prefix("goalpage:").filter(|_| frame > 0) {
                book.preview_goal_page(page.parse().unwrap());
            }
            if let Some(talk) = show.strip_prefix("talk:") {
                if frame == 2 {
                    let (key, page) = talk.rsplit_once(':').unwrap_or((talk, "0"));
                    book.preview_talk(key, page.parse().unwrap_or(0));
                }
            }
            book.load_art(&mut render);
            book.capture(&mut render, true);
            ctx.io_mut().mouse_pos = if mouse == [0.0, 0.0] { [width - 2.0, 2.0] } else { [mouse[0] * width, mouse[1] * height] };
            ctx.io_mut().delta_time = 0.2;
            let ui = ctx.new_frame();
            let mut cx = book::Context {
                game: Some(&game),
                stats: Some(stats),
                gameplay: show != "map",
                hud: show != "map",
                faded: 0.0,
                position: match show {
                    // m60_42_36_00 (block 0x3C2A2400, chunk = tile-local) near the street's middle; m11_10.
                    "place:street" => Some(([-80.0, 94.0, -30.0], 0x3C2A_2400, 0x3C2A_2400)),
                    "place:roundtable" => Some(([0.0, 0.0, 0.0], 0x0B0A_0000, 0x0B0A_0000)),
                    _ => Some(([0.0, 0.0, 0.0], 0, 0)),
                },
                street: match show {
                    "place:maze" => Some([-327.6, 94.0, 88.9]),
                    "place:rbdoor" => Some([-242.68, 95.0, 20.25]),
                    "place:bhdoor" => Some([-110.92, 95.0, -106.58]),
                    "place:diego" => Some([5.0, 94.2, -44.08]),
                    _ => None,
                },
                settings: &mut settings,
                settings_path: Path::new("/tmp/book_preview_settings.ini"),
            };
            if let Some((id, size)) = mirror.filter(|_| frame >= 2) {
                book.textures.set(pages::MODEL_FULL, id, size);
                book.textures.set(pages::MODEL_PORTRAIT, id, size);
            }
            book.frame(ui, &mut cx);
            if !show.starts_with("poof") {
                book.draw_markers(ui, &cx);
                book.draw_quest_helper(ui, &cx);
                book.draw_goal_popup(ui, &cx);
                book.draw_nameplates(ui, &cx);
                book.draw_floaters(ui, &cx);
            }
            // Round 22 (H1, H4): the experience bar where lib.rs draws it (its frame art copied to
            // book/art/preview_hud/Art_Bar_Exp.png in the preview's data), to show the helper and dialogue over it.
            if show.starts_with("turn:") || show.starts_with("talk:") || show.starts_with("picker:") || show.starts_with("prompt:") {
                book.textures.want(["preview_hud/Art_Bar_Exp.png".to_string()]);
                if let Some((t, _)) = book.textures.get("preview_hud/Art_Bar_Exp.png") {
                    let s = height / 768.0;
                    let (w, h) = (504.0 * s, 19.0 * s);
                    let top = talk::xp_bar_top([width, height], 1.0);
                    ui.get_foreground_draw_list().add_image(t, [(width - w) / 2.0, top], [(width + w) / 2.0, top + h]).build();
                }
            }
            if let Some(t) = show.strip_prefix("wisp:") {
                // Round 23 (H4, H6): the poof's camera at (0, 1.55, -3.4) looking at (0, 0.95, 0); the player's feet at
                // the origin; the wisp drifting 0.9 m to the side.
                let (pos, look) = ([0.0f32, 1.55, -3.4], [0.0f32, 0.95, 0.0]);
                let f = [look[0] - pos[0], look[1] - pos[1], look[2] - pos[2]];
                let l = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt();
                let f = [f[0] / l, f[1] / l, f[2] / l];
                let r = [f[2], 0.0, -f[0]];
                let rl = (r[0] * r[0] + r[2] * r[2]).sqrt();
                let r = [r[0] / rl, 0.0, r[2] / rl];
                let u = [f[1] * r[2] - f[2] * r[1], f[2] * r[0] - f[0] * r[2], f[0] * r[1] - f[1] * r[0]];
                let camera = markers::Camera { matrix: [[r[0], r[1], r[2], 0.0], [u[0], u[1], u[2], 0.0], [f[0], f[1], f[2], 0.0],
                                                        [pos[0], pos[1], pos[2], 1.0]], fov: 48f32.to_radians(), player: [0.0; 3] };
                wisps::preview(ui, &mut book.textures, &camera, [0.0; 3], [0.9, 0.9, 0.3], t.parse::<f32>().ok(), 2.0);
            }
            if let Some(t) = show.strip_prefix("fairy:").and_then(|t| t.parse::<f32>().ok()) {
                // Round 30 (FA1): the camera 1.6 m up looking level along +z; fairies 3-9 m ahead flying
                // left, right, towards and away (ref 6 shows them about this far off, round the arena).
                let camera = markers::Camera { matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0],
                                                        [0.0, 1.6, 0.0, 1.0]], fov: 48f32.to_radians(), player: [0.0; 3] };
                let looks = [([-2.6, 1.2, 4.0], [1.0, 0.0], t), ([2.4, 0.9, 5.5], [0.0, -1.0], t + 0.4),
                             ([-1.5, 1.5, 8.0], [0.0, 1.0], t + 1.1), ([3.0, 1.6, 9.0], [-1.0, 0.0], t + 1.7),
                             ([0.6, 0.8, 3.2], [0.7071, -0.7071], t + 0.9)];
                fairies::preview(ui, &mut book.textures, &camera, &looks);
            }
            if let Some(t) = show.strip_prefix("poof:").and_then(|t| t.parse::<f32>().ok()) {
                // The camera of tools/w101_poof_reference.py: at (0, 1.55, -3.4)
                // looking at (0, 0.95, 0), 48 degrees.
                let (pos, look) = ([0.0f32, 1.55, -3.4], [0.0f32, 0.95, 0.0]);
                let f = [look[0] - pos[0], look[1] - pos[1], look[2] - pos[2]];
                let l = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt();
                let f = [f[0] / l, f[1] / l, f[2] / l];
                let r = [f[2], 0.0, -f[0]];
                let rl = (r[0] * r[0] + r[2] * r[2]).sqrt();
                let r = [r[0] / rl, 0.0, r[2] / rl];
                let u = [f[1] * r[2] - f[2] * r[1], f[2] * r[0] - f[0] * r[2], f[0] * r[1] - f[1] * r[0]];
                let camera = markers::Camera { matrix: [[r[0], r[1], r[2], 0.0], [u[0], u[1], u[2], 0.0], [f[0], f[1], f[2], 0.0],
                                                        [pos[0], pos[1], pos[2], 1.0]], fov: 48f32.to_radians(), player: [0.0; 3] };
                let tsv = folder.join("book").join("art").join("teleport").join("fx_disappear.tsv");
                teleport::preview(ui, &mut book.textures, Some(&tsv), &camera, [0.0; 3], t);
            }
            if let Some(t) = load {
                load_screen.preview_at(1.0, t.parse().ok());
                load_screen.draw(ui, &book.textures, true, true);
            }
            if frame == 1 {
                book.preview_wait_models();
            }
            let draw_data = ctx.render();
            if frame == 3 {
                let mut canvas = backdrop.clone().map(|b| image::imageops::resize(&b, width as u32, height as u32,
                    image::imageops::FilterType::Triangle)).unwrap_or_else(|| RgbaImage::from_pixel(width as u32, height as u32, Rgba([70, 90, 80, 255])));
                rasterize(draw_data, &render.textures, &font, &mut canvas);
                canvas.save(out.join(format!("{name}.png"))).unwrap();
                println!("{name}");
            }
        }
    }
}
