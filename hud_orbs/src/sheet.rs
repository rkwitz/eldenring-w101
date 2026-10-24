//! What the overlay knows about the player, independent of how it is read from the game (game.rs).

#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    pub fp: i32,
    pub max_fp: i32,
    pub sp: i32,
    pub max_sp: i32,
    /// Rune level and runes held; None if PlayerGameData couldn't be read.
    pub level: Option<(u32, u32)>,
}

/// Runes needed to go from `level` to the next one (Elden Ring's level-up cost formula).
pub fn level_up_cost(level: u32) -> u32 {
    let x = level as f64 + 81.0;
    let extra = ((x - 92.0) * 0.02).max(0.0);
    ((extra + 0.1) * x * x + 1.0).floor() as u32
}

/// How far the runes held are toward the next level, 0 to 1 (full at the level cap).
pub fn level_progress(level: u32, runes: u32) -> f32 {
    if level >= 713 { 1.0 } else { (runes as f32 / level_up_cost(level) as f32).clamp(0.0, 1.0) }
}

/// What the spellbook shows about the player, read from PlayerGameData.
#[derive(Clone, Default)]
pub struct Sheet {
    pub name: String,
    pub level: u32,
    pub runes: u32,
    #[allow(dead_code)]
    pub rune_memory: u32,
    /// Vigor, mind, endurance, strength, dexterity, intelligence, faith, arcane.
    pub attributes: [u32; 8],
    /// Immunity (poison), robustness (bleed), focus (sleep), vitality (death).
    pub resistances: [u32; 4],
    pub poise: f32,
    pub max_equip_load: f32,
    /// Flasks of Crimson Tears and of Cerulean Tears.
    pub flasks: [u8; 2],
    pub magic_slots: u8,
    /// Param IDs by ChrAsm slot: weapons L1 R1 L2 R2 L3 R3, arrows/bolts 6-11, armour 12-15, talismans 17-20.
    pub equipment: [i32; 22],
    /// Which of the three weapons in each hand is held (0-2): left, right.
    pub weapon_slots: [u32; 2],
    /// 1 = body type A (the male model), 0 = B.
    pub gender: u8,
    /// FaceDataBuffer.buffer (the character creator's choices).
    pub face: Vec<u8>,
    /// Memorised spells (magic param IDs, -1 = empty).
    pub spells: [i32; 14],
    /// (item ID with category bits, quantity) of the normal and key item inventories.
    pub items: Vec<(u32, u32)>,
    pub key_items: Vec<(u32, u32)>,
}

