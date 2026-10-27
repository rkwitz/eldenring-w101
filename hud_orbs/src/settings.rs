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
    /// The loading screen's picture: a PNG or JPEG, relative to the DLL's folder (or absolute).
    pub loadscreen_image: String,
    /// The map page draws Elden Ring's map on the page from a copy of the screen, over the world as it was when
    /// the map opened (0: the page is cut out to show the map, and the rest of the screen is covered).
    pub bookmap: bool,
}

/// `key = value` lines: left and bottom (extra gap from the screen's bottom-left corner, in Wizard101 pixels; 0 is
/// exactly where Wizard101 puts the globes), scale (1 = Wizard101's size on a 768-pixel-tall screen) and debug (1
/// shows the raw numbers and logs every candidate stat value once a second) and level (0 hides the level number beside the experience bar).
pub fn load_settings(path: &Path) -> Settings {
    let mut settings = Settings { left: 0.0, bottom: 0.0, scale: 1.0, debug: false, level: true, loadscreen: true,
                                  loadscreen_image: LOADSCREEN_IMAGE.into(), bookmap: true };
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
         # loadscreen: 0 turns off the Wizard101 loading screen on door warps\n\
         # loadscreen_image: the loading screen's picture (PNG or JPEG), relative to this folder or absolute\n\
         # bookmap: 0 shows Elden Ring's map through a window in the map page instead of on it\n\
         # debug: 1 logs the raw values once a second\n\
         left = {}\nbottom = {}\nscale = {}\nlevel = {}\nloadscreen = {}\nloadscreen_image = {}\nbookmap = {}\n\
         debug = {}\n",
        s.left, s.bottom, s.scale, s.level as u8, s.loadscreen as u8, s.loadscreen_image, s.bookmap as u8,
        s.debug as u8
    );
    std::fs::write(path, text)
}

