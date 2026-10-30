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
            When::Now => "Changes Elden Ring right away",
            When::Restart => "Saved; Elden Ring uses it when it next starts",
            When::HudRestart => "Saved; used when the game next starts",
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
    ("GameplayButton", "Gameplay 1", &[
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

/// The row's shown value now; None when it can't be read (no game, no GraphicsConfig.xml).
pub fn value(row: &Row, game: Option<&game::Game>, s: &crate::Settings, gfx: Option<&[u32; 25]>) -> Option<String> {
    let on = |b: bool| if b { "On" } else { "Off" }.to_string();
    match &row.kind {
        Kind::Opt(offset, choices) => {
            let v = game?.option(*offset)?;
            Some(choices.iter().find(|c| c.0 == v).map_or(v.to_string(), |c| c.1.to_string()))
        }
        Kind::Scale(offset) => game?.option(*offset).map(|v| v.to_string()),
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
        Kind::Hud(h) => Some(match h {
            Hud::Size => format!("{:.2}", s.scale),
            Hud::Right => format!("{:.0}", s.left),
            Hud::Up => format!("{:.0}", s.bottom),
            Hud::Level => on(s.level),
            Hud::LoadScreen => on(s.loadscreen),
            Hud::BookMap => if s.bookmap { "Map On Page" } else { "Map Behind" }.into(),
            Hud::Nameplates => on(s.nameplates),
            Hud::Debug => on(s.debug),
            Hud::Probe => on(s.scene_probe),
            Hud::ProbeNudge => on(s.scene_probe_nudge),
        }),
    }
}

fn step_choice<T: Copy + PartialEq>(choices: &[T], now: T, step: i32) -> T {
    let i = choices.iter().position(|c| *c == now).unwrap_or(0) as i32;
    choices[(i + step).clamp(0, choices.len() as i32 - 1) as usize]
}

/// Changes the row by `step` (-1 / +1). Returns what happened, for the page's note.
pub fn change(row: &Row, step: i32, game: Option<&game::Game>, s: &mut crate::Settings, settings_path: &std::path::Path)
              -> Result<String, String> {
    match &row.kind {
        Kind::Opt(offset, choices) => {
            let game = game.ok_or("Elden Ring's options can't be reached")?;
            let now = game.option(*offset).ok_or("Elden Ring's options can't be reached")?;
            let values: Vec<u8> = choices.iter().map(|c| c.0).collect();
            let new = step_choice(&values, now, step);
            game.set_option(*offset, new).then_some(()).ok_or("Elden Ring's options can't be written")?;
            Ok(format!("{}: {}", row.label, choices.iter().find(|c| c.0 == new).map_or("", |c| c.1)))
        }
        Kind::Scale(offset) => {
            let game = game.ok_or("Elden Ring's options can't be reached")?;
            let now = game.option(*offset).ok_or("Elden Ring's options can't be reached")?;
            let new = (now as i32 + step).clamp(0, 10) as u8;
            game.set_option(*offset, new).then_some(()).ok_or("Elden Ring's options can't be written")?;
            Ok(format!("{}: {new}", row.label))
        }
        Kind::Gfx(g) => {
            let mut now = graphics().ok_or("Elden Ring's graphics settings can't be read")?;
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
            save_graphics(&now)?;
            Ok(format!("{}: {} (when Elden Ring next starts)", row.label,
                       g.choices.iter().find(|c| c.0 == new).map_or("", |c| c.1)))
        }
        Kind::Resolution => {
            let mut now = graphics().ok_or("Elden Ring's graphics settings can't be read")?;
            let mode = (now[SCREEN_MODE.field] as usize).min(2);
            let f = RESOLUTION[mode].0;
            let mut sizes = SIZES.to_vec();
            sizes.sort();
            let new = step_choice(&sizes, (now[f], now[f + 1]), step);
            let new = if sizes.contains(&(now[f], now[f + 1])) { new } else { sizes[sizes.len() / 2] };
            (now[f], now[f + 1]) = new;
            save_graphics(&now)?;
            Ok(format!("{} window: {} x {} (when Elden Ring next starts)", MODE[mode].1, new.0, new.1))
        }
        Kind::Hud(h) => {
            let st = step as f32;
            match h {
                Hud::Size => s.scale = ((s.scale + 0.05 * st) * 100.0).round().clamp(50.0, 200.0) / 100.0,
                Hud::Right => s.left = (s.left + 5.0 * st).clamp(-200.0, 600.0),
                Hud::Up => s.bottom = (s.bottom + 5.0 * st).clamp(-200.0, 600.0),
                Hud::Level => s.level = !s.level,
                Hud::LoadScreen => s.loadscreen = !s.loadscreen,
                Hud::BookMap => s.bookmap = !s.bookmap,
                Hud::Nameplates => s.nameplates = !s.nameplates,
                Hud::Debug => s.debug = !s.debug,
                Hud::Probe => s.scene_probe = !s.scene_probe,
                Hud::ProbeNudge => s.scene_probe_nudge = !s.scene_probe_nudge,
            }
            crate::save_settings(settings_path, s).map_err(|e| format!("w101_orbs.ini not saved: {e}"))?;
            Ok(format!("{}: {}", row.label, value(row, None, s, None).unwrap_or_default()))
        }
    }
}

/// The graphics settings: Elden Ring's (in memory) or, without them, GraphicsConfig.xml's.
pub fn graphics() -> Option<[u32; 25]> {
    game::graphics().or_else(|| read_config(&std::fs::read(config_path()?).ok()?))
}

/// %APPDATA%\EldenRing\GraphicsConfig.xml.
fn config_path() -> Option<std::path::PathBuf> {
    Some(std::path::Path::new(&std::env::var_os("APPDATA")?).join("EldenRing").join("GraphicsConfig.xml"))
}

fn decode(bytes: &[u8]) -> Option<String> {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16(&units).ok()
    } else {
        String::from_utf8(bytes.to_vec()).ok()
    }
}

fn tag_value<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find("</")?;
    Some(xml[start..end].trim())
}

/// GraphicsConfig.xml's settings in CSWindowScreenConfig's order (fps target, not in the file, as 60).
pub fn read_config(bytes: &[u8]) -> Option<[u32; 25]> {
    let xml = decode(bytes)?;
    let mut out = [0u32; 25];
    out[8] = 1;
    for (f, w, h) in RESOLUTION {
        out[f] = tag_value(&xml, w)?.parse().ok()?;
        out[f + 1] = tag_value(&xml, h)?.parse().ok()?;
    }
    for g in [&SCREEN_MODE, &AUTO_DETECT, &QUALITY].into_iter().chain(QUALITIES.iter()) {
        let v = tag_value(&xml, g.tag)?;
        out[g.field] = g.choices.iter().find(|c| c.2 == v)?.0;
    }
    Some(out)
}

/// GraphicsConfig.xml with `values` put in (each tag Elden Ring wrote is replaced; the rest is kept).
pub fn write_config(bytes: &[u8], values: &[u32; 25]) -> Option<Vec<u8>> {
    let mut xml = decode(bytes)?;
    let mut put = |tag: &str, value: String| -> Option<()> {
        let open = format!("<{tag}>");
        let start = xml.find(&open)? + open.len();
        let end = start + xml[start..].find("</")?;
        xml.replace_range(start..end, &value);
        Some(())
    };
    for (f, w, h) in RESOLUTION {
        put(w, values[f].to_string())?;
        put(h, values[f + 1].to_string())?;
    }
    for g in [&SCREEN_MODE, &AUTO_DETECT, &QUALITY].into_iter().chain(QUALITIES.iter()) {
        put(g.tag, g.choices.iter().find(|c| c.0 == values[g.field])?.2.to_string())?;
    }
    let mut out = vec![0xFF, 0xFE];
    out.extend(xml.encode_utf16().flat_map(|u| u.to_le_bytes()));
    Some(out)
}

/// Writes the graphics settings into Elden Ring's persistent config (so its own save writes the same) and into
/// GraphicsConfig.xml, which is first copied to GraphicsConfig.xml.before_book (once).
fn save_graphics(values: &[u32; 25]) -> Result<(), String> {
    let in_memory = game::graphics().is_some();
    if in_memory {
        for (i, v) in values.iter().enumerate() {
            game::set_graphics(i, *v);
        }
    }
    let path = config_path().ok_or("GraphicsConfig.xml not found")?;
    let old = std::fs::read(&path).map_err(|e| format!("GraphicsConfig.xml not read: {e}"))?;
    let backup = path.with_file_name("GraphicsConfig.xml.before_book");
    if !backup.exists() {
        std::fs::write(&backup, &old).map_err(|e| format!("GraphicsConfig.xml not backed up: {e}"))?;
    }
    let new = write_config(&old, values).ok_or("GraphicsConfig.xml has an unexpected layout")?;
    std::fs::write(&path, new).map_err(|e| format!("GraphicsConfig.xml not written: {e}"))?;
    tracing::info!("settings: graphics config written (in memory: {in_memory}): {values:?}");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn graphics_config_round_trip() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-16\" ?>\r\n<config><ScreenMode>BORDERLESS</ScreenMode>\r\n<Resolution-WindowScreenWidth>2560</Resolution-WindowScreenWidth>\r\n<Resolution-WindowScreenHeight>1440</Resolution-WindowScreenHeight>\r\n<Resolution-FullScreenWidth>2560</Resolution-FullScreenWidth>\r\n<Resolution-FullScreenHeight>1440</Resolution-FullScreenHeight>\r\n<Resolution-BorderlessScreenWidth>2560</Resolution-BorderlessScreenWidth>\r\n<Resolution-BorderlessScreenHeight>1440</Resolution-BorderlessScreenHeight>\r\n<Auto-detectBestRenderingSettings>OFF</Auto-detectBestRenderingSettings>\r\n<QualitySetting>CUSTOM</QualitySetting>\r\n<TextureQuality>MAX</TextureQuality>\r\n<Antialiasing>HIGH</Antialiasing>\r\n<SSAO>MAX</SSAO>\r\n<DepthOfField>MEDIUM</DepthOfField>\r\n<MotionBlur>MEDIUM</MotionBlur>\r\n<ShadowQuality>MEDIUM</ShadowQuality>\r\n<LightingQuality>MAX</LightingQuality>\r\n<EffectsQuality>MAX</EffectsQuality>\r\n<ReflectionQuality>MAX</ReflectionQuality>\r\n<WaterSurfaceQuality>HIGH</WaterSurfaceQuality>\r\n<ShadeQuality>HIGH</ShadeQuality>\r\n<VolumetricEffectQuality>MEDIUM</VolumetricEffectQuality>\r\n<RaytracingQuality>DISABLE</RaytracingQuality>\r\n<GIDataQuality>HIGH</GIDataQuality>\r\n<GrassQuality>MEDIUM</GrassQuality>\r\n</config>\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(xml.encode_utf16().flat_map(|u| u.to_le_bytes()));
        let v = super::read_config(&bytes).unwrap();
        assert_eq!(v[6], 2);
        assert_eq!(v[10], 3);
        assert_eq!(v[22], 0);
        assert_eq!(super::write_config(&bytes, &v).unwrap(), bytes);
        let mut w = v;
        w[6] = 0;
        w[0] = 1920;
        let out = super::write_config(&bytes, &w).unwrap();
        assert_eq!(super::read_config(&out).unwrap(), w);
    }
}
