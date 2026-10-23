//! The overlay's settings, w101_orbs.ini next to the DLL.

use std::path::Path;

pub struct Settings {
    pub left: f32,
    pub bottom: f32,
    pub scale: f32,
    pub debug: bool,
    pub level: bool,
    /// Wizard101 loading screens on the mod's door warps.
    pub loadscreen: bool,
}

/// `key = value` lines: left and bottom (extra gap from the screen's bottom-left corner, in Wizard101 pixels; 0 is
/// exactly where Wizard101 puts the globes), scale (1 = Wizard101's size on a 768-pixel-tall screen) and debug (1
/// shows the raw numbers and logs every candidate stat value once a second) and level (0 hides the level number beside the experience bar).
pub fn load_settings(path: &Path) -> Settings {
    let mut settings = Settings { left: 0.0, bottom: 0.0, scale: 1.0, debug: false, level: true, loadscreen: true };
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
            _ => {}
        }
    }
    settings
}

/// Writes the settings back to w101_orbs.ini (the spellbook's Settings page changes them).
pub fn save_settings(path: &Path, s: &Settings) -> std::io::Result<()> {
    let text = format!(
        "# Wizard101 HUD for Elden Ring (w101_orbs.dll). The spellbook's Settings page rewrites this file.\n\
         # left / bottom: extra gap from the screen's bottom-left corner, in Wizard101 pixels\n\
         # scale: 1 = Wizard101's size on a 768-pixel-tall screen\n\
         # level: 0 hides the level number beside the experience bar\n\
         # loadscreen: 0 turns off the Wizard101 loading screen on door warps\n\
         # debug: 1 logs the raw values once a second\n\
         left = {}\nbottom = {}\nscale = {}\nlevel = {}\nloadscreen = {}\ndebug = {}\n",
        s.left, s.bottom, s.scale, s.level as u8, s.loadscreen as u8, s.debug as u8
    );
    std::fs::write(path, text)
}

