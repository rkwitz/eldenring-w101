//! Stand-in for the DLL's game.rs: a made-up character instead of the game's memory.
#![allow(dead_code)]

pub use crate::sheet::*;

pub struct Game;

/// Quest flags the preview pretends are on.
const FLAGS: &[u32] = &[10000800, 14000800, 105, 110, 1042366000, 1042366001, 1042366009, 1042366010, 1042366500 + 2, 1042366040, 1042366540,
                        1042366541, 1042366100];

pub fn event_flag(flag: u32) -> Option<bool> {
    Some(FLAGS.contains(&flag))
}

pub fn set_event_flag(_flag: u32, _on: bool) {}

impl Game {
    pub fn fake() -> Game {
        Game
    }

    pub fn position(&self) -> Option<([f32; 3], i32, i32)> {
        Some(([0.0; 3], 0, 0))
    }

    pub fn sheet(&self) -> Option<Sheet> {
        let goods = |id: u32| 0x4000_0000 | id;
        let mut items = vec![
            (1000003, 1), (2000000, 1), (3140005, 1), (1100000, 1), (33000000, 1), (34000000, 1),
            (50000000, 99), (52000000, 30),
            (0x1000_0000 | 40000, 1), (0x1000_0000 | 40100, 1), (0x1000_0000 | 40200, 1), (0x1000_0000 | 40300, 1),
            (0x1000_0000 | 290000, 1), (0x1000_0000 | 290100, 1),
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
            magic_slots: 4,
            weapon_slots: [0, 0],
            gender: 1,
            face: vec![0; 276],
            equipment: [3140005, 2000000, -1, 1000003, -1, -1, 50000000, -1, -1, -1, -1, -1, 1000000, 1000100, 1000200,
                        1000300, -1, 1000, 1010, 2000, -1, -1],
            spells: [4000, 4010, 4300, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
            items,
            key_items: vec![(goods(130), 1), (goods(8000), 1), (goods(8500), 1), (goods(9500), 1), (goods(9300), 1),
                            (goods(9340), 1)],
        })
    }
}
