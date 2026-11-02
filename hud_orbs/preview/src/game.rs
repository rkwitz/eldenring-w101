//! Stand-in for the DLL's game.rs: a made-up character instead of the game's memory.
#![allow(dead_code)]

pub use crate::sheet::*;

pub struct Game;

/// A giver's MSB position to put the preview camera in front of (11b/11c_markers_close).
pub static CLOSE: std::sync::Mutex<Option<[f32; 3]>> = std::sync::Mutex::new(None);

/// The camera turned this far (radians, clockwise from up the street): the quest helper shots (14_quest_helper*).
pub static TURN: std::sync::Mutex<f32> = std::sync::Mutex::new(0.0);

/// Quest flags the preview pretends are on.
const FLAGS: &[u32] = &[10000800, 14000800, 105, 110, 1042360300, 1042360301, 1042360309, 1042360310, 1042360500 + 2, 1042360340, 1042360540,
                        1042360541, 1042360400, 1042360311, 1042360409,
                        67000, 67010, 67210]; // and three cookbooks (crafting page)

pub fn event_flag(flag: u32) -> Option<bool> {
    Some(FLAGS.contains(&flag))
}

pub fn set_event_flag(_flag: u32, _on: bool) {}

static SPELLS: std::sync::Mutex<[i32; 14]> = std::sync::Mutex::new([4010, 4200, 4361, 4430, 4301, 6600, -1, -1, -1, -1, -1, -1, -1, -1]);

impl Game {
    /// A Lost Soul and Rattlebones up the street, a Dark Sprite nearer.
    pub fn enemies(&self, models: &[u32]) -> Vec<(u32, [f32; 3], f32)> {
        [(4180, [-23.0, 93.5, -100.0], 1.6), (4960, [-5.0, 93.5, -86.0], 0.0), (3450, [-17.5, 94.0, -106.0], 1.4),
         (3500, [-21.0, 93.5, -104.0], 1.7)]
            .into_iter().filter(|e| models.contains(&e.0)).collect()
    }

    /// The street NPCs the game "has loaded": all but Lady Oriel (who then takes the MSB-spot fallback), at their MSB
    /// spots (the preview's physics space is the tile's).
    pub fn entities(&self, models: &[u32], ids: &[u32]) -> Vec<(u32, [f32; 3])> {
        crate::markers::NPCS.iter().filter(|n| n.0 != "lady_oriel" && models.contains(&n.3) && ids.contains(&n.2))
            .map(|n| (n.2, n.4)).collect()
    }
    pub fn entity_render_bits(&self, _models: &[u32], _ids: &[u32]) -> Vec<(u32, u8, u8, u32)> {
        Vec::new()
    }

    pub fn set_spell(&self, index: usize, spell: i32) -> bool {
        SPELLS.lock().unwrap()[index] = spell;
        true
    }

    pub fn set_quick_item(&self, _item_id: u32, _on: bool) -> Result<(), &'static str> {
        Err("preview")
    }

    pub fn add_runes(&self, _runes: u32) -> Option<u32> {
        None
    }

    pub fn equip(&self, _slot: u32, _item_id: Option<u32>) -> Result<(), &'static str> {
        Ok(())
    }

    pub fn add_player_stat(&self, _fp: bool, _percent: f32) -> Option<(i32, i32)> {
        None
    }

    pub fn player_yaw(&self) -> Option<f32> {
        None
    }

    pub fn action_button(&self, _row: u32) -> Option<ActionButton> {
        None
    }

    pub fn set_action_button_text(&self, _row: u32, _text_id: i32) -> bool {
        false
    }

    pub fn fake() -> Game {
        Game
    }

    pub fn position(&self) -> Option<([f32; 3], i32, i32)> {
        // In front of Private Connelly, Olivia Dawnwillow further off (m60_42_36_00).
        Some((self.spot().1, 0x3C2A2400, 0x3C2A2400))
    }

    /// (camera, player): up the street, or (CLOSE) about talking distance (4 m) in front of a giver.
    fn spot(&self) -> ([f32; 3], [f32; 3]) {
        match CLOSE.lock().unwrap().as_ref() {
            Some(p) => ([p[0], p[1] + 1.6, p[2] - 5.5], [p[0], p[1], p[2] - 4.0]),
            None => ([-19.0, 95.3, -115.0], [-20.0, 93.5, -110.0]),
        }
    }

    /// A camera behind the player looking up the street at the quest givers (physics space = chunk space here).
    pub fn camera(&self) -> Option<crate::markers::Camera> {
        let turn = *TURN.lock().unwrap();
        let f = [if CLOSE.lock().unwrap().is_some() { 0.0f32 } else { 0.02 } + turn.sin(), 0.0, turn.cos()];
        let n = (f[0] * f[0] + f[2] * f[2]).sqrt();
        let f = [f[0] / n, 0.0, f[2] / n];
        let (c, player) = self.spot();
        let matrix = [[f[2], 0.0, -f[0], 0.0], [0.0, 1.0, 0.0, 0.0], [f[0], 0.0, f[2], 0.0], [c[0], c[1], c[2], 1.0]];
        Some(crate::markers::Camera { matrix, fov: 0.85, player })
    }

    pub fn sheet(&self) -> Option<Sheet> {
        let goods = |id: u32| 0x4000_0000 | id;
        let mut items = vec![
            (1000003, 1), (2000000, 1), (3140005, 1), (1100000, 1), (33000000, 1), (34000000, 1),
            (50000000, 99), (52000000, 30),
            (0x1000_0000 | 40000, 1), (0x1000_0000 | 40100, 1), (0x1000_0000 | 40200, 1), (0x1000_0000 | 40300, 1),
            (0x1000_0000 | 290000, 1), (0x1000_0000 | 290100, 1),
            (0x1000_0000 | 120000, 1), (0x1000_0000 | 170100, 1), (0x1000_0000 | 720200, 1), (0x1000_0000 | 180300, 1),
            (0x2000_0000 | 1000, 1), (0x2000_0000 | 1010, 1), (0x2000_0000 | 2000, 1),
            (0x8000_0000 | 10000, 1),
            (goods(1007), 9), (goods(1006), 1), (goods(1057), 2), (goods(2000), 5), (goods(3000), 2),
            (goods(4000), 1), (goods(4001), 1), (goods(4010), 1), (goods(4020), 1), (goods(4300), 1),
            (goods(4400), 1), (goods(4500), 1), (goods(6000), 1), (goods(6100), 1), (goods(6400), 1),
            (goods(6700), 1), (goods(7000), 1), (goods(7200), 1), (goods(6300), 1), (goods(7500), 1),
            (goods(200000), 1), (goods(201000), 1), (goods(240000 + 3), 1),
            (goods(15000), 12), (goods(15010), 4), (goods(20650), 7), (goods(20660), 2),
            (goods(20760), 6), (goods(20802), 3), (goods(20800), 2), (goods(20801), 1), (goods(15340), 9), // crafting
        ];
        // PREVIEW_SPELLS=4140,6210,...: these spells instead of the usual ones (to check particular cards).
        if let Ok(list) = std::env::var("PREVIEW_SPELLS") {
            items.retain(|(id, _)| id >> 28 != 4 || !(4000..8000).contains(&(id & 0x0FFF_FFFF)));
            items.extend(list.split(',').filter_map(|s| s.trim().parse::<u32>().ok()).map(|s| (goods(s), 1)));
        }
        items.extend((0..30).map(|i| (goods(2900 + i), 1)));
        Some(Sheet {
            name: "Ambrose".into(),
            level: 60,
            runes: 12000,
            rune_memory: 0,
            attributes: [30, 18, 20, 16, 14, 40, 12, 9],
            resistances: [180, 170, 120, 140],
            poise: 31.0,
            max_equip_load: 72.4,
            flasks: [12, 2],
            magic_slots: 8,
            talisman_slots: 3,
            weapon_slots: [0, 0],
            gender: 1,
            face: { let mut f = vec![0; 276]; f[4] = 5; f },
            equipment: [3140005, std::env::var("PREVIEW_WEAPON").ok().and_then(|w| w.parse().ok()).unwrap_or(17010000), -1, 1000003, -1, -1, 50000000, -1, -1, -1, -1, -1, 120000, 170100, 720200,
                        180300, -1, 1000, 1010, 2000, -1, -1],
            spells: *SPELLS.lock().unwrap(),
            items,
            key_items: vec![(goods(130), 1), (goods(8000), 1), (goods(8500), 1), (goods(9500), 1), (goods(9300), 1),
                            (goods(9340), 1)],
            quick_items: [goods(1007), goods(240000 + 3), 0, 0, 0, 0, 0, 0, 0, 0],
        })
    }
}

/// Elden Ring's options as the preview pretends they are (GameSettings bytes).
static OPTIONS: std::sync::Mutex<[u8; 0x30]> = std::sync::Mutex::new([
    5, 5, 5, 0, 7, 8, 8, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1,
    0, 1, 0, 0, 5, 5, 5, 0, 10, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1]);

impl Game {
    pub fn option(&self, offset: usize) -> Option<u8> {
        OPTIONS.lock().unwrap().get(offset).copied()
    }
    pub fn set_option(&self, offset: usize, value: u8) -> bool {
        OPTIONS.lock().unwrap().get_mut(offset).map(|v| *v = value).is_some()
    }
    pub fn player_address(&self) -> Option<usize> {
        Some(1)
    }
}

/// Ryan's GraphicsConfig.xml (2026-10-30) as CSWindowScreenConfig's fields.
static GRAPHICS: std::sync::Mutex<[u32; 25]> = std::sync::Mutex::new([
    2560, 1440, 2560, 1440, 2560, 1440, 2, 0, 1, 5, 3, 3, 4, 2, 2, 1, 3, 3, 3, 2, 2, 1, 0, 2, 2]);

pub fn graphics() -> Option<[u32; 25]> {
    Some(*GRAPHICS.lock().unwrap())
}
pub fn set_graphics(index: usize, value: u32) -> bool {
    GRAPHICS.lock().unwrap().get_mut(index).map(|v| *v = value).is_some()
}
pub fn quitting_out() -> bool {
    false
}

pub fn quit_to_title() -> bool {
    false
}
pub fn close_window() -> bool {
    false
}

#[derive(Clone, Copy, Debug)]
pub struct ActionButton {
    pub region: u8,
    pub radius: f32,
    pub angle: i32,
    pub height: f32,
    pub base: f32,
    pub text_id: i32,
}
