//! Renders the spellbook's pages to PNG files on Linux, with made-up character data, to check layouts without the
//! game: the real book.rs / pages.rs / layout.rs / font.rs code draws into imgui, and a small software rasterizer
//! turns imgui's draw lists into an image.
//!
//! Usage: cargo run --release -- <w101_orbs folder (with book/)> <out folder> [width height]
//! (W101_PARTS = a folder of decompressed Elden Ring parts files, for the character pictures.)

#![allow(dead_code)]

#[path = "../../src/book.rs"]
mod book;
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
#[path = "../../src/steam.rs"]
mod steam;
#[path = "../../src/model.rs"]
mod model;
#[path = "../../src/markers.rs"]
mod markers;
#[path = "../../src/nameplates.rs"]
mod nameplates;
#[path = "../../src/loadscreen.rs"]
mod loadscreen;
#[path = "../../src/teleport.rs"]
mod teleport;
mod input;
mod game;
mod live_preview;

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

/// Draws imgui's draw data over `canvas` (premultiplied-free alpha blending, like imgui's DX12 backend).
fn rasterize(draw_data: &imgui::DrawData, textures: &[RgbaImage], font: &RgbaImage, canvas: &mut RgbaImage) {
    let (cw, ch) = (canvas.width() as i32, canvas.height() as i32);
    for list in draw_data.draw_lists() {
        let vtx = list.vtx_buffer();
        let idx = list.idx_buffer();
        let mut additive = false;
        for cmd in list.commands() {
            if let DrawCmd::RawCallback { raw_cmd, .. } = cmd {
                match unsafe { (*raw_cmd).UserCallbackData } as usize {
                    hudhook::BLEND_ADDITIVE => additive = true,
                    hudhook::BLEND_NORMAL => additive = false,
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
                        let t = sample(texture, u, vv);
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
        ("3_pets", pages::Tab::Pets, 0, [0.0, 0.0], ""),
        ("3b_pets_ash", pages::Tab::Pets, 0, [0.0, 0.0], "pet:1"),
        ("4_deck", pages::Tab::Deck, 0, [0.565, 0.35], ""),
        ("4b_deck_fire", pages::Tab::Deck, 1, [0.0, 0.0], ""),
        ("4c_deck_astral", pages::Tab::Deck, 8, [0.0, 0.0], ""),
        ("5_quests", pages::Tab::Quests, 0, [0.0, 0.0], ""),
        ("6_map", pages::Tab::Map, 0, [0.0, 0.0], ""),
        ("6b_map_open", pages::Tab::Map, 0, [0.0, 0.0], "map"),
        ("7_crafting", pages::Tab::Crafting, 0, [0.0, 0.0], ""),
        ("7b_crafting_cookbook", pages::Tab::Crafting, 0, [0.0, 0.0], "cookbooks"),
        ("8_help", pages::Tab::Help, 1, [0.0, 0.0], ""),
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
        ("9j_settings_tip", pages::Tab::Settings, 0, [0.70, 0.58], ""),
        ("9k_settings_tab_tip", pages::Tab::Settings, 0, [0.4215, 0.193], "settings:3"),
        ("11_markers", pages::Tab::Character, 0, [0.0, 0.0], "markers"),
        ("10_dialogue", pages::Tab::Character, 0, [0.0, 0.0], "talk:Ceren Nightchant"),
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
    let mut load_screen = loadscreen::LoadScreen::default();
    book.textures.want([loadscreen::IMAGE.to_string()]);
    let settings_image = settings::load_settings(Path::new("/nonexistent")).loadscreen_image;
    load_screen.load(folder.parent().unwrap_or(&folder), &settings_image, &folder.join("book").join("page_flip.tsv"), &mut render);
    for (name, tab, sub, mouse, show) in shots {
        if only.is_some_and(|o| !name.contains(o)) {
            continue;
        }
        book.preview_show(tab, sub);
        if show == "map" {
            book.capture(&mut render, true); // the world as it was before the map opened
            book.preview_map();
        }
        if show == "markers" {
            book.preview_closed();
        }
        if show == "cookbooks" {
            book.preview_cookbooks();
        }
        let load = show.strip_prefix("load:");
        if load.is_some() || show.starts_with("poof") {
            book.preview_closed();
        }
        if let Some(tab) = show.strip_prefix("settings:") {
            book.preview_settings(tab.parse().unwrap());
        }
        if let Some(pet) = show.strip_prefix("pet:") {
            book.preview_pet(pet.parse().unwrap());
        }
        // A few frames: the first ones queue the art, which loads between frames.
        for frame in 0..4 {
            if let Some(npc) = show.strip_prefix("talk:") {
                if frame == 2 {
                    book.preview_talk(npc);
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
                position: Some(([0.0, 0.0, 0.0], 0, 0)),
                settings: &mut settings,
                settings_path: Path::new("/tmp/book_preview_settings.ini"),
            };
            book.frame(ui, &mut cx);
            if !show.starts_with("poof") {
                book.draw_markers(ui, &cx);
                book.draw_nameplates(ui, &cx);
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
