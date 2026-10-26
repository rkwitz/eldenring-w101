//! Stand-in for the DLL's game.rs: a made-up character instead of the game's memory.
#![allow(dead_code)]

pub use crate::sheet::*;

pub struct Game;

/// Quest flags the preview pretends are on.
const FLAGS: &[u32] = &[10000800, 14000800, 105, 110, 1042366000, 1042366001, 1042366009, 1042366010, 1042366500 + 2, 1042366040, 1042366540,
                        1042366541, 1042366100, 1042366011, 1042366109];

pub fn event_flag(flag: u32) -> Option<bool> {
    Some(FLAGS.contains(&flag))
}

pub fn set_event_flag(_flag: u32, _on: bool) {}

static SPELLS: std::sync::Mutex<[i32; 14]> = std::sync::Mutex::new([4010, 4200, 4361, 4430, 4301, 6600, -1, -1, -1, -1, -1, -1, -1, -1]);

impl Game {
    /// A Lost Soul and Rattlebones up the street, a Dark Sprite nearer.
    pub fn enemies(&self, models: &[u32]) -> Vec<(u32, [f32; 3])> {
        [(4180, [-23.0, 93.5, -100.0]), (4960, [-5.0, 93.5, -86.0]), (3450, [-17.5, 94.0, -106.0])]
            .into_iter().filter(|e| models.contains(&e.0)).collect()
    }

    pub fn set_spell(&self, index: usize, spell: i32) -> bool {
        SPELLS.lock().unwrap()[index] = spell;
        true
    }

    pub fn equip(&self, _slot: u32, _item_id: Option<u32>) -> Result<(), &'static str> {
        Ok(())
    }

    pub fn fake() -> Game {
        Game
    }

    pub fn position(&self) -> Option<([f32; 3], i32, i32)> {
        // In front of Private Connelly, Olivia Dawnwillow further off (m60_42_36_00).
        Some(([-20.0, 93.5, -110.0], 0x3C2A2400, 0x3C2A2400))
    }

    /// A camera behind the player looking up the street at the quest givers (physics space = chunk space here).
    pub fn camera(&self) -> Option<crate::markers::Camera> {
        let f = [0.02f32, 0.0, 1.0];
        let n = (f[0] * f[0] + f[2] * f[2]).sqrt();
        let f = [f[0] / n, 0.0, f[2] / n];
        let matrix = [[f[2], 0.0, -f[0], 0.0], [0.0, 1.0, 0.0, 0.0], [f[0], 0.0, f[2], 0.0], [-19.0, 95.3, -115.0, 1.0]];
        Some(crate::markers::Camera { matrix, fov: 0.85, player: [-20.0, 93.5, -110.0] })
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
        ];
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
            face: vec![0; 276],
            equipment: [3140005, 2000000, -1, 1000003, -1, -1, 50000000, -1, -1, -1, -1, -1, 120000, 170100, 720200,
                        180300, -1, 1000, 1010, 2000, -1, -1],
            spells: *SPELLS.lock().unwrap(),
            items,
            key_items: vec![(goods(130), 1), (goods(8000), 1), (goods(8500), 1), (goods(9500), 1), (goods(9300), 1),
                            (goods(9340), 1)],
        })
    }
}
