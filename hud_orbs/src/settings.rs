//! The overlay's settings, w101_orbs.ini next to the DLL.

use std::path::Path;

pub struct Settings {
    pub left: f32,
    pub bottom: f32,
    pub scale: f32,
    pub debug: bool,
    pub level: bool,
    /// The Wizard101 loading screen over all of Elden Ring's loading screens and the mod's door warps.
    pub loadscreen: bool,
    /// The loading screen's picture: a PNG or JPEG, relative to the DLL's folder (or absolute).
    pub loadscreen_image: String,
    /// The map page draws Elden Ring's map on the page from a copy of the screen, over the world as it was when
    /// the map opened (0: the page is cut out to show the map, and the rest of the screen is covered).
    pub bookmap: bool,
    /// Wizard101 nameplates over enemies and NPCs.
    pub nameplates: bool,
    /// Logs how the mirror's character preview scene works ("probe:" lines, probe.rs); off unless set to 1.
    pub scene_probe: bool,
    /// With the probe: after the first menu that stayed open over 5 s closes, MenuOffscrRendParam row 0's camera
    /// distance goes up 50% (in memory only), to see whether the param drives the mirror's camera.
    pub scene_probe_nudge: bool,
    /// The live character preview in the book (live_preview.rs, experimental); read when the game starts.
    pub live_preview: bool,
}

/// `key = value` lines: left and bottom (extra gap from the screen's bottom-left corner, in Wizard101 pixels; 0 is
/// exactly where Wizard101 puts the globes), scale (1 = Wizard101's size on a 768-pixel-tall screen) and debug (1
/// shows the raw numbers and logs every candidate stat value once a second) and level (0 hides the level number beside the experience bar).
pub fn load_settings(path: &Path) -> Settings {
    let mut settings = Settings { left: 0.0, bottom: 0.0, scale: 1.0, debug: false, level: true, loadscreen: true,
                                  loadscreen_image: LOADSCREEN_IMAGE.into(), bookmap: true, nameplates: true,
                                  scene_probe: false, scene_probe_nudge: false, live_preview: false };
    for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        match key.trim() {
            "left" => settings.left = value.parse().unwrap_or(settings.left),
            "bottom" => settings.bottom = value.parse().unwrap_or(settings.bottom),
            "scale" => settings.scale = value.parse().unwrap_or(settings.scale),
            "debug" => settings.debug = value == "1",
            "level" => settings.level = value != "0",
            "loadscreen" => settings.loadscreen = value != "0",
            "loadscreen_image" if !value.is_empty() => settings.loadscreen_image = value.to_string(),
            "bookmap" => settings.bookmap = value != "0",
            "nameplates" => settings.nameplates = value != "0",
            "scene_probe" => settings.scene_probe = value == "1",
            "scene_probe_nudge" => settings.scene_probe_nudge = value == "1",
            "live_preview" => settings.live_preview = value == "1",
            _ => {}
        }
    }
    settings
}

/// The default loading screen picture: Wizard City's (tools/w101_book_layout.py puts it in the book's art).
pub const LOADSCREEN_IMAGE: &str = "w101_orbs/book/art/GUI/LoadScreen_W01_WC_01.png";

/// Writes the settings back to w101_orbs.ini (the spellbook's Settings page changes them).
pub fn save_settings(path: &Path, s: &Settings) -> std::io::Result<()> {
    let text = format!(
        "# Wizard101 HUD for Elden Ring (w101_orbs.dll). The spellbook's Settings page rewrites this file.\n\
         # left / bottom: extra gap from the screen's bottom-left corner, in Wizard101 pixels\n\
         # scale: 1 = Wizard101's size on a 768-pixel-tall screen\n\
         # level: 0 hides the level number beside the experience bar\n\
         # loadscreen: 0 turns off the Wizard101 loading screen (over Elden Ring's loading screens and door warps)\n\
         # loadscreen_image: the loading screen's picture (PNG or JPEG), relative to this folder or absolute\n\
         # bookmap: 0 shows Elden Ring's map through a window in the map page instead of on it\n\
         # nameplates: 0 hides the Wizard101 nameplates over enemies and NPCs\n\
         # debug: 1 logs the raw values once a second\n\
         # live_preview: 1 shows the live character preview in the book (experimental; read when the game starts)\n\
         # scene_probe: 1 logs how the mirror's preview scene works (test builds); scene_probe_nudge: 1 also moves its camera\n\
         left = {}\nbottom = {}\nscale = {}\nlevel = {}\nloadscreen = {}\nloadscreen_image = {}\nbookmap = {}\n\
         nameplates = {}\ndebug = {}\nlive_preview = {}\n",
        s.left, s.bottom, s.scale, s.level as u8, s.loadscreen as u8, s.loadscreen_image, s.bookmap as u8,
        s.nameplates as u8, s.debug as u8, s.live_preview as u8
    ) + &if s.scene_probe || s.scene_probe_nudge {
        format!("scene_probe = {}\nscene_probe_nudge = {}\n", s.scene_probe as u8, s.scene_probe_nudge as u8)
    } else {
        String::new()
    };
    // Keys this file doesn't know (another part of the mod's, e.g. live_preview) are kept as they were.
    const KNOWN: [&str; 12] = ["left", "bottom", "scale", "level", "loadscreen", "loadscreen_image", "bookmap",
                               "nameplates", "debug", "scene_probe", "scene_probe_nudge", "live_preview"];
    let kept: String = std::fs::read_to_string(path).unwrap_or_default().lines()
        .filter(|l| l.split_once('=').is_some_and(|(k, _)| !KNOWN.contains(&k.trim())) && !l.trim_start().starts_with('#'))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(path, text + &kept)
}

