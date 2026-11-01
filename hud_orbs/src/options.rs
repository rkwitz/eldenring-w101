//! The spellbook's Settings page: Wizard101's Settings window (Settings.gui and its tab pages) filled with Elden
//! Ring's own options, its graphics settings and the HUD's settings (w101_orbs.ini).
//!
//! - Elden Ring's options (GameDataMan -> GameSettings, one byte each) are written in memory, the same values its
//!   options menu writes; the game keeps them and saves them with its user data.
//! - Graphics settings (CSWindow's persistent screen config, which Elden Ring writes to GraphicsConfig.xml and reads
//!   when it starts) are written in memory and to GraphicsConfig.xml (backed up once to
//!   GraphicsConfig.xml.before_book); Elden Ring applies them when it next starts.
//! - The HUD's settings are written to w101_orbs.ini.
//!
//! The table of every Wizard101 setting and what it became: /mnt/project-files/book_settings/settings_map.md.

use crate::game;

/// Offsets of Elden Ring's options in GameSettings (checked against the eldenring crate in game.rs).
pub mod opt {
    pub const CAMERA_SPEED: usize = 0x00;
    pub const RUMBLE: usize = 0x01;
    pub const BRIGHTNESS: usize = 0x02;
    pub const MUSIC: usize = 0x04;
    pub const SFX: usize = 0x05;
    pub const VOICE: usize = 0x06;
    pub const BLOOD: usize = 0x07;
    pub const SUBTITLES: usize = 0x08;
    pub const HUD: usize = 0x09;
    pub const INVERT_X: usize = 0x0A;
    pub const INVERT_Y: usize = 0x0B;
    pub const AUTO_LOCK: usize = 0x0C;
    pub const WALL_RECOVERY: usize = 0x0D;
    pub const RESET_Y: usize = 0x10;
    pub const CINEMATIC: usize = 0x11;
    pub const MANUAL_AIM: usize = 0x16;
    pub const AUTO_TARGET: usize = 0x17;
    pub const START_OFFLINE: usize = 0x18;
    pub const HDR_BRIGHTNESS: usize = 0x1C;
    pub const HDR_PEAK: usize = 0x1D;
    pub const HDR_CONTRAST: usize = 0x1E;
    pub const MASTER: usize = 0x20;
    pub const MARK_NEW: usize = 0x22;
    pub const RECENT_ITEMS: usize = 0x23;
    pub const TUTORIALS: usize = 0x2E;
    pub const AUTO_ROTATION: usize = 0x2F;
}

/// When a change shows.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum When {
    /// At once.
    Now,
    /// When Elden Ring next starts.
    Restart,
    /// When the HUD next loads (the DLL reads it at start).
    HudRestart,
}

impl When {
    pub fn text(self) -> &'static str {
        match self {
            When::Now => "Applied when you click OK",
            When::Restart => "Saved when you click OK;\nElden Ring uses it when it next starts",
            When::HudRestart => "Experimental; saved when you click OK,\nused when the game next starts",
        }
    }
}

/// The HUD's own settings (w101_orbs.ini).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hud {
    Size,
    Right,
    Up,
    Level,
    LoadScreen,
    BookMap,
    Nameplates,
    Debug,
    Probe,
    ProbeNudge,
    LivePreview,
}

/// A graphics setting: its field in CSWindowScreenConfig, GraphicsConfig.xml tag and choices (value, shown, xml).
pub struct Gfx {
    pub field: usize,
    pub tag: &'static str,
    pub choices: &'static [(u32, &'static str, &'static str)],
}

const LEVEL: &[(u32, &str, &str)] = &[(0, "Low", "LOW"), (1, "Medium", "MEDIUM"), (2, "High", "HIGH"), (3, "Maximum", "MAX")];
/// Off is written as DISABLE, as Ryan's GraphicsConfig.xml has for ray tracing (the same enum as the others).
const TOGGLE: &[(u32, &str, &str)] =
    &[(0, "Off", "DISABLE"), (1, "Low", "LOW"), (2, "Medium", "MEDIUM"), (3, "High", "HIGH"), (4, "Maximum", "MAX")];
const GRASS: &[(u32, &str, &str)] = &[(2, "Medium", "MEDIUM"), (3, "High", "HIGH"), (4, "Maximum", "MAX")];
const PRESET: &[(u32, &str, &str)] =
    &[(1, "Low", "LOW"), (2, "Medium", "MEDIUM"), (3, "High", "HIGH"), (4, "Maximum", "MAX"), (5, "Custom", "CUSTOM")];
const MODE: &[(u32, &str, &str)] = &[(0, "Windowed", "WINDOW"), (1, "Fullscreen", "FULLSCREEN"), (2, "Borderless", "BORDERLESS")];
const ON_OFF: &[(u32, &str, &str)] = &[(0, "Off", "OFF"), (1, "On", "ON")];

const fn gfx(field: usize, tag: &'static str, choices: &'static [(u32, &'static str, &'static str)]) -> Gfx {
    Gfx { field, tag, choices }
}

pub const SCREEN_MODE: Gfx = gfx(6, "ScreenMode", MODE);
const AUTO_DETECT: Gfx = gfx(7, "Auto-detectBestRenderingSettings", ON_OFF);
const QUALITY: Gfx = gfx(9, "QualitySetting", PRESET);
/// The single quality settings (field, tag, choices) after the preset, in GraphicsConfig.xml's order.
const QUALITIES: [Gfx; 15] = [
    gfx(10, "TextureQuality", LEVEL), gfx(11, "Antialiasing", TOGGLE), gfx(12, "SSAO", TOGGLE),
    gfx(13, "DepthOfField", TOGGLE), gfx(14, "MotionBlur", TOGGLE), gfx(15, "ShadowQuality", LEVEL),
    gfx(16, "LightingQuality", LEVEL), gfx(17, "EffectsQuality", LEVEL), gfx(18, "ReflectionQuality", LEVEL),
    gfx(19, "WaterSurfaceQuality", LEVEL), gfx(20, "ShadeQuality", LEVEL), gfx(21, "VolumetricEffectQuality", LEVEL),
    gfx(22, "RaytracingQuality", TOGGLE), gfx(23, "GIDataQuality", LEVEL), gfx(24, "GrassQuality", GRASS),
];
/// Resolution tags per screen mode (width, height fields and tags).
const RESOLUTION: [(usize, &str, &str); 3] = [
    (0, "Resolution-WindowScreenWidth", "Resolution-WindowScreenHeight"),
    (2, "Resolution-FullScreenWidth", "Resolution-FullScreenHeight"),
    (4, "Resolution-BorderlessScreenWidth", "Resolution-BorderlessScreenHeight"),
];
/// Window sizes offered (16:9, as Elden Ring's own list).
const SIZES: [(u32, u32); 7] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440), (3200, 1800), (3840, 2160), (1366, 768)];

pub enum Kind {
    /// An Elden Ring option byte: (offset, choices as (value, shown)).
    Opt(usize, &'static [(u8, &'static str)]),
    /// An Elden Ring option 0-10.
    Scale(usize),
    Gfx(&'static Gfx),
    Resolution,
    Hud(Hud),
}

pub struct Row {
    pub label: &'static str,
    pub kind: Kind,
    pub when: When,
    /// The Wizard101 option this row stands in for (shown in the row's tip).
    pub w101: &'static str,
}

const BOOL: &[(u8, &str)] = &[(0, "Off"), (1, "On")];
const AXIS: &[(u8, &str)] = &[(0, "Normal"), (1, "Inverted")];
const HUD_TYPE: &[(u8, &str)] = &[(0, "Off"), (1, "On"), (2, "Auto")];
const BLOOD: &[(u8, &str)] = &[(0, "Off"), (1, "On"), (2, "Mild")];
const LAUNCH: &[(u8, &str)] = &[(0, "Online"), (1, "Offline")];

const fn row(label: &'static str, kind: Kind, when: When, w101: &'static str) -> Row {
    Row { label, kind, when, w101 }
}

/// Settings.gui's tab buttons, left to right, with the page each shows: (button, title, rows). Wizard101 has at most
/// seven rows on a page.
pub const TABS: [(&str, &str, &[Row]); 10] = [
    ("VideoButton", "Video Options", &[
        row("Resolution", Kind::Resolution, When::Restart, "Resolution"),
        row("Screen Mode", Kind::Gfx(&SCREEN_MODE), When::Restart, "Fullscreen"),
        row("Quality Preset", Kind::Gfx(&QUALITY), When::Restart, "-"),
        row("Auto-detect Quality", Kind::Gfx(&AUTO_DETECT), When::Restart, "-"),
        row("Texture Quality", Kind::Gfx(&QUALITIES[0]), When::Restart, "Texture Detail"),
        row("Antialiasing", Kind::Gfx(&QUALITIES[1]), When::Restart, "Smoothing (Advanced Video)"),
        row("Brightness", Kind::Scale(opt::BRIGHTNESS), When::Now, "Brightness (Advanced Video)"),
    ]),
    ("AdvVideoButton", "Advanced Video", &[
        row("Shadows", Kind::Gfx(&QUALITIES[5]), When::Restart, "Shadows"),
        row("Lighting", Kind::Gfx(&QUALITIES[6]), When::Restart, "Bloom Active (closest)"),
        row("Effects", Kind::Gfx(&QUALITIES[7]), When::Restart, "Particles"),
        row("Reflections", Kind::Gfx(&QUALITIES[8]), When::Restart, "-"),
        row("Ambient Occlusion", Kind::Gfx(&QUALITIES[2]), When::Restart, "-"),
        row("Depth of Field", Kind::Gfx(&QUALITIES[3]), When::Restart, "-"),
        row("Motion Blur", Kind::Gfx(&QUALITIES[4]), When::Restart, "-"),
    ]),
    ("AudioButton", "Sound Options", &[
        row("Master Volume", Kind::Scale(opt::MASTER), When::Now, "Sound"),
        row("Music Volume", Kind::Scale(opt::MUSIC), When::Now, "Music Volume"),
        row("Sound Effects Volume", Kind::Scale(opt::SFX), When::Now, "Sound Effects Volume"),
        row("Voice Volume", Kind::Scale(opt::VOICE), When::Now, "Dialogue Volume"),
    ]),
    ("GameplayButton", "Gameplay", &[
        row("Nameplates", Kind::Hud(Hud::Nameplates), When::Now, "Show Name"),
        row("Tutorial Tips", Kind::Opt(opt::TUTORIALS, BOOL), When::Now, "Tutorial Tips"),
        row("HUD", Kind::Opt(opt::HUD, HUD_TYPE), When::Now, "Combat Display"),
        row("Subtitles", Kind::Opt(opt::SUBTITLES, BOOL), When::Now, "NPC Chatter"),
        row("New Item Marks", Kind::Opt(opt::MARK_NEW, BOOL), When::Now, "-"),
        row("Recent Items Tab", Kind::Opt(opt::RECENT_ITEMS, BOOL), When::Now, "-"),
    ]),
    ("AdvGameplayButton", "Camera", &[
        row("Camera Speed", Kind::Scale(opt::CAMERA_SPEED), When::Now, "-"),
        row("Camera Left/Right", Kind::Opt(opt::INVERT_X, AXIS), When::Now, "-"),
        row("Camera Up/Down", Kind::Opt(opt::INVERT_Y, AXIS), When::Now, "-"),
        row("Auto Lock-On", Kind::Opt(opt::AUTO_LOCK, BOOL), When::Now, "-"),
        row("Auto Target", Kind::Opt(opt::AUTO_TARGET, BOOL), When::Now, "-"),
        row("Camera Wall Recovery", Kind::Opt(opt::WALL_RECOVERY, BOOL), When::Now, "-"),
        row("Camera Auto Rotation", Kind::Opt(opt::AUTO_ROTATION, BOOL), When::Now, "-"),
    ]),
    ("MoreGameplayButton", "Controls", &[
        row("Camera Reset Up/Down", Kind::Opt(opt::RESET_Y, BOOL), When::Now, "-"),
        row("Cinematic Camera", Kind::Opt(opt::CINEMATIC, BOOL), When::Now, "-"),
        row("Controller Rumble", Kind::Scale(opt::RUMBLE), When::Now, "-"),
        row("Manual Aiming", Kind::Opt(opt::MANUAL_AIM, BOOL), When::Now, "-"),
        row("Blood", Kind::Opt(opt::BLOOD, BLOOD), When::Now, "-"),
        row("Launch Setting", Kind::Opt(opt::START_OFFLINE, LAUNCH), When::Restart, "-"),
    ]),
    ("AnotherGameplayButton", "Wizard101 HUD", &[
        row("Globe Size", Kind::Hud(Hud::Size), When::Now, "-"),
        row("Globes Right", Kind::Hud(Hud::Right), When::Now, "-"),
        row("Globes Up", Kind::Hud(Hud::Up), When::Now, "-"),
        row("Level Number", Kind::Hud(Hud::Level), When::Now, "-"),
        row("Loading Screens", Kind::Hud(Hud::LoadScreen), When::Now, "-"),
        row("Book Map", Kind::Hud(Hud::BookMap), When::Now, "-"),
        row("Live Preview", Kind::Hud(Hud::LivePreview), When::HudRestart, "-"),
    ]),
    ("PrivacyButton", "HDR Display", &[
        row("HDR Brightness", Kind::Scale(opt::HDR_BRIGHTNESS), When::Now, "-"),
        row("HDR Peak Brightness", Kind::Scale(opt::HDR_PEAK), When::Now, "-"),
        row("HDR Contrast", Kind::Scale(opt::HDR_CONTRAST), When::Now, "-"),
    ]),
    ("RealmsButton", "More Video", &[
        row("Water Surface", Kind::Gfx(&QUALITIES[9]), When::Restart, "-"),
        row("Shaders", Kind::Gfx(&QUALITIES[10]), When::Restart, "-"),
        row("Volumetric Effects", Kind::Gfx(&QUALITIES[11]), When::Restart, "-"),
        row("Global Illumination", Kind::Gfx(&QUALITIES[13]), When::Restart, "-"),
        row("Grass", Kind::Gfx(&QUALITIES[14]), When::Restart, "-"),
        row("Ray Tracing", Kind::Gfx(&QUALITIES[12]), When::Restart, "-"),
    ]),
    ("CreditsButton", "Testing", &[
        row("Debug Log", Kind::Hud(Hud::Debug), When::Now, "-"),
        row("Scene Probe", Kind::Hud(Hud::Probe), When::HudRestart, "-"),
        row("Probe Camera Nudge", Kind::Hud(Hud::ProbeNudge), When::HudRestart, "-"),
    ]),
];

/// Changes made on the page but not applied yet: like Wizard101's Settings window, they apply only when OK is
/// clicked; closing the book any other way drops them (Book::set_open).
#[derive(Default)]
pub struct Staged {
    /// Elden Ring option bytes: (offset, value).
    opts: Vec<(usize, u8)>,
    /// The whole graphics config as it will be written.
    gfx: Option<[u32; 25]>,
    /// The HUD's settings (bools as 0 / 1).
    hud: Vec<(Hud, f32)>,
}

impl Staged {
    pub fn is_empty(&self) -> bool {
        self.opts.is_empty() && self.gfx.is_none() && self.hud.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Staged::default();
    }

    fn opt(&self, game: Option<&game::Game>, offset: usize) -> Option<u8> {
        self.opts.iter().find(|o| o.0 == offset).map(|o| o.1).or_else(|| game?.option(offset))
    }

    fn set_opt(&mut self, offset: usize, value: u8) {
        self.opts.retain(|o| o.0 != offset);
        self.opts.push((offset, value));
    }

    fn hud(&self, s: &crate::Settings, h: Hud) -> f32 {
        self.hud.iter().find(|x| x.0 == h).map_or_else(|| hud_get(s, h), |x| x.1)
    }

    /// The graphics settings as the page shows them: the staged ones, else Elden Ring's.
    pub fn graphics(&self) -> Option<[u32; 25]> {
        self.gfx.or_else(graphics)
    }
}

fn hud_get(s: &crate::Settings, h: Hud) -> f32 {
    let b = |v: bool| v as u8 as f32;
    match h {
        Hud::Size => s.scale,
        Hud::Right => s.left,
        Hud::Up => s.bottom,
        Hud::Level => b(s.level),
        Hud::LoadScreen => b(s.loadscreen),
        Hud::BookMap => b(s.bookmap),
        Hud::Nameplates => b(s.nameplates),
        Hud::Debug => b(s.debug),
        Hud::Probe => b(s.scene_probe),
        Hud::ProbeNudge => b(s.scene_probe_nudge),
        Hud::LivePreview => b(s.live_preview),
    }
}

fn hud_put(s: &mut crate::Settings, h: Hud, v: f32) {
    let on = v > 0.5;
    match h {
        Hud::Size => s.scale = v,
        Hud::Right => s.left = v,
        Hud::Up => s.bottom = v,
        Hud::Level => s.level = on,
        Hud::LoadScreen => s.loadscreen = on,
        Hud::BookMap => s.bookmap = on,
        Hud::Nameplates => s.nameplates = on,
        Hud::Debug => s.debug = on,
        Hud::Probe => s.scene_probe = on,
        Hud::ProbeNudge => s.scene_probe_nudge = on,
        Hud::LivePreview => s.live_preview = on,
    }
}

fn hud_text(h: Hud, v: f32) -> String {
    match h {
        Hud::Size => format!("{v:.2}"),
        Hud::Right | Hud::Up => format!("{v:.0}"),
        Hud::BookMap => if v > 0.5 { "On Page" } else { "Behind" }.into(),
        _ => if v > 0.5 { "On" } else { "Off" }.into(),
    }
}

/// The row's shown value, staged changes included; None when it can't be read (no game, no GraphicsConfig.xml).
/// `gfx` is Staged::graphics(), read once per frame by the caller.
pub fn value(row: &Row, game: Option<&game::Game>, s: &crate::Settings, gfx: Option<&[u32; 25]>, staged: &Staged)
             -> Option<String> {
    match &row.kind {
        Kind::Opt(offset, choices) => {
            let v = staged.opt(game, *offset)?;
            Some(choices.iter().find(|c| c.0 == v).map_or(v.to_string(), |c| c.1.to_string()))
        }
        Kind::Scale(offset) => staged.opt(game, *offset).map(|v| v.to_string()),
        Kind::Gfx(g) => {
            let v = gfx?[g.field];
            Some(g.choices.iter().find(|c| c.0 == v).map_or(v.to_string(), |c| c.1.to_string()))
        }
        Kind::Resolution => {
            let g = gfx?;
            let mode = (g[SCREEN_MODE.field] as usize).min(2);
            let f = RESOLUTION[mode].0;
            Some(format!("{} x {}", g[f], g[f + 1]))
        }
        Kind::Hud(h) => Some(hud_text(*h, staged.hud(s, *h))),
    }
}

fn step_choice<T: Copy + PartialEq>(choices: &[T], now: T, step: i32) -> T {
    let i = choices.iter().position(|c| *c == now).unwrap_or(0) as i32;
    choices[(i + step).clamp(0, choices.len() as i32 - 1) as usize]
}

/// Changes the row by `step` (-1 / +1) in `staged` only; nothing is written until apply(). Returns the page's note.
pub fn change(row: &Row, step: i32, game: Option<&game::Game>, s: &crate::Settings, staged: &mut Staged)
              -> Result<String, String> {
    let shown = match &row.kind {
        Kind::Opt(offset, choices) => {
            let now = staged.opt(game, *offset).ok_or("Elden Ring's options can't be reached")?;
            let values: Vec<u8> = choices.iter().map(|c| c.0).collect();
            let new = step_choice(&values, now, step);
            staged.set_opt(*offset, new);
            choices.iter().find(|c| c.0 == new).map_or(String::new(), |c| c.1.to_string())
        }
        Kind::Scale(offset) => {
            let now = staged.opt(game, *offset).ok_or("Elden Ring's options can't be reached")?;
            let new = (now as i32 + step).clamp(0, 10) as u8;
            staged.set_opt(*offset, new);
            new.to_string()
        }
        Kind::Gfx(g) => {
            let mut now = staged.graphics().ok_or("Elden Ring's graphics settings can't be read")?;
            let values: Vec<u32> = g.choices.iter().map(|c| c.0).collect();
            let new = step_choice(&values, now[g.field], step);
            now[g.field] = new;
            if g.field == QUALITY.field && new != 5 {
                // A preset sets every quality as Elden Ring's presets do (ray tracing stays as it is).
                let level = new - 1; // Low 0 .. Maximum 3
                for q in &QUALITIES {
                    now[q.field] = match q.choices {
                        c if c.len() == LEVEL.len() => level,
                        c if c.len() == GRASS.len() => (level + 1).max(2),
                        _ if q.field == 22 => now[q.field],
                        _ => level + 1,
                    };
                }
                now[AUTO_DETECT.field] = 0;
            } else if QUALITIES.iter().any(|q| q.field == g.field) {
                now[QUALITY.field] = 5; // Custom, as Elden Ring's menu does
                now[AUTO_DETECT.field] = 0;
            }
            staged.gfx = Some(now);
            g.choices.iter().find(|c| c.0 == new).map_or(String::new(), |c| c.1.to_string())
        }
        Kind::Resolution => {
            let mut now = staged.graphics().ok_or("Elden Ring's graphics settings can't be read")?;
            let mode = (now[SCREEN_MODE.field] as usize).min(2);
            let f = RESOLUTION[mode].0;
            let mut sizes = SIZES.to_vec();
            sizes.sort();
            let new = step_choice(&sizes, (now[f], now[f + 1]), step);
            let new = if sizes.contains(&(now[f], now[f + 1])) { new } else { sizes[sizes.len() / 2] };
            (now[f], now[f + 1]) = new;
            staged.gfx = Some(now);
            format!("{} x {}", new.0, new.1)
        }
        Kind::Hud(h) => {
            let (now, st) = (staged.hud(s, *h), step as f32);
            let new = match h {
                Hud::Size => ((now + 0.05 * st) * 100.0).round().clamp(50.0, 200.0) / 100.0,
                Hud::Right | Hud::Up => (now + 5.0 * st).clamp(-200.0, 600.0),
                _ => if now > 0.5 { 0.0 } else { 1.0 },
            };
            staged.hud.retain(|x| x.0 != *h);
            staged.hud.push((*h, new));
            hud_text(*h, new)
        }
    };
    Ok(format!("{}: {shown} (OK applies it)", row.label))
}

/// Applies the staged changes (OK): Elden Ring's options in memory, the graphics settings to its persistent config
/// and GraphicsConfig.xml, the HUD's to w101_orbs.ini. Only what differs from now is written. Returns how many
/// settings changed, or what couldn't be applied (the rest still is). The staged changes are used up either way.
pub fn apply(staged: &mut Staged, game: Option<&game::Game>, s: &mut crate::Settings, settings_path: &std::path::Path)
             -> Result<usize, String> {
    let staged = std::mem::take(staged);
    let (mut changed, mut errors) = (0, Vec::new());
    for (offset, v) in staged.opts {
        match game {
            Some(g) if g.option(offset) == Some(v) => {}
            Some(g) if g.set_option(offset, v) => changed += 1,
            _ => errors.push("Elden Ring's options can't be written".to_string()),
        }
    }
    if let Some(gfx) = staged.gfx {
        if graphics() != Some(gfx) {
            match save_graphics(&gfx) {
                Ok(()) => changed += 1,
                Err(e) => errors.push(e),
            }
        }
    }
    let hud: Vec<_> = staged.hud.into_iter().filter(|(h, v)| hud_get(s, *h) != *v).collect();
    if !hud.is_empty() {
        for (h, v) in &hud {
            hud_put(s, *h, *v);
        }
        changed += hud.len();
        if let Err(e) = crate::save_settings(settings_path, s) {
            errors.push(format!("w101_orbs.ini not saved: {e}"));
        }
    }
    errors.dedup();
    if errors.is_empty() { Ok(changed) } else { Err(errors.join("; ")) }
}

/// The graphics settings: Elden Ring's (in memory) or, without them, GraphicsConfig.xml's.
pub fn graphics() -> Option<[u32; 25]> {
    game::graphics().or_else(|| read_config(&std::fs::read(config_path()?).ok()?))
}

/// %APPDATA%\EldenRing\GraphicsConfig.xml.
fn config_path() -> Option<std::path::PathBuf> {
    Some(std::path::Path::new(&std::env::var_os("APPDATA")?).join("EldenRing").join("GraphicsConfig.xml"))
}

/// How GraphicsConfig.xml is encoded, so it is written back the same way. Elden Ring writes UTF-16 little-endian
/// with no byte order mark (Ryan's file, 2026-11-01); the others are accepted in case a tool re-saved it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Encoding {
    Utf16Le { bom: bool },
    Utf16Be { bom: bool },
    Utf8 { bom: bool },
}

fn decode(bytes: &[u8]) -> Option<(String, Encoding)> {
    let utf16 = |b: &[u8], le: bool| -> Option<String> {
        let units: Vec<u16> = b.chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        String::from_utf16(&units).ok()
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => Some((utf16(rest, true)?, Encoding::Utf16Le { bom: true })),
        [0xFE, 0xFF, rest @ ..] => Some((utf16(rest, false)?, Encoding::Utf16Be { bom: true })),
        [0xEF, 0xBB, 0xBF, rest @ ..] => Some((String::from_utf8(rest.to_vec()).ok()?, Encoding::Utf8 { bom: true })),
        // No mark: an ASCII character then a zero byte is UTF-16LE; a zero byte first is UTF-16BE.
        [a, 0, ..] if *a != 0 => Some((utf16(bytes, true)?, Encoding::Utf16Le { bom: false })),
        [0, b, ..] if *b != 0 => Some((utf16(bytes, false)?, Encoding::Utf16Be { bom: false })),
        _ => Some((String::from_utf8(bytes.to_vec()).ok()?, Encoding::Utf8 { bom: false })),
    }
}

fn encode(xml: &str, enc: Encoding) -> Vec<u8> {
    let (mut out, bom): (Vec<u8>, &[u8]) = match enc {
        Encoding::Utf16Le { bom } => (Vec::new(), if bom { &[0xFF, 0xFE] } else { &[] }),
        Encoding::Utf16Be { bom } => (Vec::new(), if bom { &[0xFE, 0xFF] } else { &[] }),
        Encoding::Utf8 { bom } => (Vec::new(), if bom { &[0xEF, 0xBB, 0xBF] } else { &[] }),
    };
    out.extend_from_slice(bom);
    match enc {
        Encoding::Utf16Le { .. } => out.extend(xml.encode_utf16().flat_map(u16::to_le_bytes)),
        Encoding::Utf16Be { .. } => out.extend(xml.encode_utf16().flat_map(u16::to_be_bytes)),
        Encoding::Utf8 { .. } => out.extend_from_slice(xml.as_bytes()),
    }
    out
}

/// Where the text of element `tag` is: the byte range between `<tag ...>` and its `</tag>`. Tag names match
/// without regard to case; attributes and spacing inside the tags are allowed. None for a missing or self-closing
/// element.
fn tag_span(xml: &str, tag: &str) -> Option<std::ops::Range<usize>> {
    let lower = xml.to_ascii_lowercase(); // the same byte offsets
    let tag = tag.to_ascii_lowercase();
    let open = format!("<{tag}");
    let mut from = 0;
    while let Some(i) = lower[from..].find(&open) {
        let at = from + i + open.len();
        from = at;
        let next = lower[at..].chars().next()?;
        if next != '>' && !next.is_whitespace() && next != '/' {
            continue; // a longer name that starts the same
        }
        let close = at + lower[at..].find('>')?;
        if lower[..close].ends_with('/') {
            return None;
        }
        let start = close + 1;
        let end = start + lower[start..].find(&format!("</{tag}"))?;
        return Some(start..end);
    }
    None
}

fn tag_value<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    tag_span(xml, tag).map(|r| xml[r].trim())
}

/// Stand-ins for a field missing from the file (1920x1080 windowed, High), only so the rest of the file can still
/// be read; a missing field is written back only when it is changed.
const DEFAULTS: [u32; 25] = [1920, 1080, 1920, 1080, 1920, 1080, 0, 0, 1, 3, 2, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 0, 2, 3];

fn gfx_tags() -> impl Iterator<Item = &'static Gfx> {
    [&SCREEN_MODE, &AUTO_DETECT, &QUALITY].into_iter().chain(QUALITIES.iter())
}

/// GraphicsConfig.xml's settings in CSWindowScreenConfig's order. A tag that is missing or has a value it doesn't
/// know reads as DEFAULTS; None only if the file has none of the tags.
pub fn read_config(bytes: &[u8]) -> Option<[u32; 25]> {
    let (xml, _) = decode(bytes)?;
    let (mut out, mut found) = (DEFAULTS, false);
    for (f, w, h) in RESOLUTION {
        for (field, tag) in [(f, w), (f + 1, h)] {
            if let Some(v) = tag_value(&xml, tag).and_then(|v| v.parse().ok()) {
                (out[field], found) = (v, true);
            }
        }
    }
    for g in gfx_tags() {
        let Some(v) = tag_value(&xml, g.tag) else { continue };
        if let Some(c) = g.choices.iter().find(|c| c.2.eq_ignore_ascii_case(v)) {
            (out[g.field], found) = (c.0, true);
        }
    }
    found.then_some(out)
}

/// GraphicsConfig.xml with `values` put in. Only the tags whose value changes are rewritten; everything else
/// (encoding, byte order mark or none, line ends, spacing, tags it doesn't know) is kept byte for byte. A changed
/// setting whose tag is missing is added before the root's closing tag. None if the file can't be decoded or has
/// nowhere to add a missing tag.
pub fn write_config(bytes: &[u8], values: &[u32; 25]) -> Option<Vec<u8>> {
    let (mut xml, enc) = decode(bytes)?;
    let old = read_config(bytes).unwrap_or(DEFAULTS);
    let newline = if xml.contains("\r\n") { "\r\n" } else { "\n" };
    let mut put = |tag: &str, value: &str| -> Option<()> {
        if let Some(r) = tag_span(&xml, tag) {
            xml.replace_range(r, value);
        } else {
            let at = xml.trim_end().rfind("</")?;
            xml.insert_str(at, &format!("<{tag}>{value}</{tag}>{newline}"));
        }
        Some(())
    };
    for (f, w, h) in RESOLUTION {
        for (field, tag) in [(f, w), (f + 1, h)] {
            if values[field] != old[field] {
                put(tag, &values[field].to_string())?;
            }
        }
    }
    for g in gfx_tags() {
        let Some(c) = g.choices.iter().find(|c| c.0 == values[g.field]) else { continue };
        if values[g.field] != old[g.field] {
            put(g.tag, c.2)?;
        }
    }
    Some(encode(&xml, enc))
}

/// Writes the graphics settings into GraphicsConfig.xml (copied once to GraphicsConfig.xml.before_book; the new file
/// is written to GraphicsConfig.xml.new, then put in place) and into Elden Ring's persistent config, so its own save
/// writes the same.
fn save_graphics(values: &[u32; 25]) -> Result<(), String> {
    let path = config_path().ok_or("GraphicsConfig.xml not found (no APPDATA)")?;
    let old = std::fs::read(&path).map_err(|e| format!("GraphicsConfig.xml not read: {e}"))?;
    let new = write_config(&old, values).ok_or_else(|| {
        let head: Vec<String> = old.iter().take(16).map(|b| format!("{b:02X}")).collect();
        tracing::warn!("settings: GraphicsConfig.xml ({} bytes) not understood; it starts {}", old.len(), head.join(" "));
        "GraphicsConfig.xml couldn't be read; it was left as it is".to_string()
    })?;
    let backup = path.with_file_name("GraphicsConfig.xml.before_book");
    if !backup.exists() {
        std::fs::write(&backup, &old).map_err(|e| format!("GraphicsConfig.xml not backed up: {e}"))?;
    }
    if new != old {
        let temp = path.with_file_name("GraphicsConfig.xml.new");
        std::fs::write(&temp, &new).map_err(|e| format!("GraphicsConfig.xml not written: {e}"))?;
        std::fs::rename(&temp, &path).map_err(|e| format!("GraphicsConfig.xml not replaced: {e}"))?;
    }
    let in_memory = game::graphics().is_some();
    if in_memory {
        for (i, v) in values.iter().enumerate() {
            game::set_graphics(i, *v);
        }
    }
    tracing::info!("settings: graphics config written (in memory: {in_memory}): {values:?}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ryan's GraphicsConfig.xml as Elden Ring wrote it (2026-11-01): UTF-16LE, no byte order mark, CRLF.
    const RYAN: &str = "<?xml version=\"1.0\" encoding=\"UTF-16\" ?>\r\n<config><ScreenMode>BORDERLESS</ScreenMode>\r\n<Resolution-WindowScreenWidth>2560</Resolution-WindowScreenWidth>\r\n<Resolution-WindowScreenHeight>1440</Resolution-WindowScreenHeight>\r\n<Resolution-FullScreenWidth>2560</Resolution-FullScreenWidth>\r\n<Resolution-FullScreenHeight>1440</Resolution-FullScreenHeight>\r\n<Resolution-BorderlessScreenWidth>2560</Resolution-BorderlessScreenWidth>\r\n<Resolution-BorderlessScreenHeight>1440</Resolution-BorderlessScreenHeight>\r\n<Auto-detectBestRenderingSettings>OFF</Auto-detectBestRenderingSettings>\r\n<QualitySetting>CUSTOM</QualitySetting>\r\n<TextureQuality>MAX</TextureQuality>\r\n<Antialiasing>HIGH</Antialiasing>\r\n<SSAO>MAX</SSAO>\r\n<DepthOfField>MEDIUM</DepthOfField>\r\n<MotionBlur>MEDIUM</MotionBlur>\r\n<ShadowQuality>MEDIUM</ShadowQuality>\r\n<LightingQuality>MAX</LightingQuality>\r\n<EffectsQuality>MAX</EffectsQuality>\r\n<ReflectionQuality>MAX</ReflectionQuality>\r\n<WaterSurfaceQuality>HIGH</WaterSurfaceQuality>\r\n<ShadeQuality>HIGH</ShadeQuality>\r\n<VolumetricEffectQuality>MEDIUM</VolumetricEffectQuality>\r\n<RaytracingQuality>DISABLE</RaytracingQuality>\r\n<GIDataQuality>HIGH</GIDataQuality>\r\n<GrassQuality>MEDIUM</GrassQuality>\r\n</config>\r\n";

    fn utf16le(s: &str, bom: bool) -> Vec<u8> {
        encode(s, Encoding::Utf16Le { bom })
    }

    #[test]
    fn ryans_file_round_trip() {
        let bytes = utf16le(RYAN, false);
        assert_eq!(bytes.len(), 2432); // the size of Ryan's file
        let v = read_config(&bytes).unwrap();
        assert_eq!((v[4], v[5], v[6], v[10], v[22], v[24]), (2560, 1440, 2, 3, 0, 2));
        assert_eq!(write_config(&bytes, &v).unwrap(), bytes);
        let mut w = v;
        (w[4], w[5]) = (1920, 1080);
        w[6] = 0;
        let out = write_config(&bytes, &w).unwrap();
        assert_eq!(&out[..2], b"<\0", "no byte order mark added");
        assert_eq!(read_config(&out).unwrap(), w);
        let text = decode(&out).unwrap().0;
        assert_eq!(text, RYAN.replace("BorderlessScreenWidth>2560<", "BorderlessScreenWidth>1920<")
            .replace("BorderlessScreenHeight>1440<", "BorderlessScreenHeight>1080<")
            .replace(">BORDERLESS<", ">WINDOW<"));
    }

    #[test]
    fn other_encodings_kept() {
        for bytes in [utf16le(RYAN, true), encode(RYAN, Encoding::Utf16Be { bom: false }),
                      encode(RYAN, Encoding::Utf8 { bom: true }), RYAN.as_bytes().to_vec()] {
            let v = read_config(&bytes).unwrap();
            assert_eq!(v[6], 2);
            assert_eq!(write_config(&bytes, &v).unwrap(), bytes);
            let mut w = v;
            w[10] = 0;
            let out = write_config(&bytes, &w).unwrap();
            assert_eq!(out[..3], bytes[..3]);
            assert_eq!(read_config(&out).unwrap(), w);
        }
    }

    #[test]
    fn tolerant_layout() {
        // Attributes, spacing, lower case values, a tag it doesn't know, missing tags.
        let xml = "<?xml version=\"1.0\"?>\n<config>\n  <ScreenMode a=\"1\"> window </ScreenMode>\n  <FutureThing>7</FutureThing>\n  <TextureQuality>max</TextureQuality>\n  <Resolution-WindowScreenWidth>1600</Resolution-WindowScreenWidth>\n</config>\n";
        let bytes = utf16le(xml, false);
        let v = read_config(&bytes).unwrap();
        assert_eq!((v[0], v[6], v[10], v[1]), (1600, 0, 3, DEFAULTS[1]));
        assert_eq!(write_config(&bytes, &v).unwrap(), bytes, "nothing missing is added unless changed");
        let mut w = v;
        w[1] = 900;
        let out = decode(&write_config(&bytes, &w).unwrap()).unwrap().0;
        assert!(out.contains("<FutureThing>7</FutureThing>"));
        assert!(out.contains("<ScreenMode a=\"1\"> window </ScreenMode>"), "an unchanged tag is kept as it was");
        assert!(out.contains("<Resolution-WindowScreenHeight>900</Resolution-WindowScreenHeight>\n</config>"));
        assert_eq!(read_config(out.as_bytes()).unwrap()[1], 900);
        assert!(read_config(b"not xml").is_none());
    }
}
