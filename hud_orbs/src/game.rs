//! Reads the player's HP and FP out of Elden Ring's memory.
//!
//! The game's globals are found by byte pattern (the same patterns community tools use), so small patches don't
//! break them. The live values come from the player character (WorldChrMan -> player -> stat module); the save's
//! copy in PlayerGameData is the fallback when those offsets look wrong. Without a player character there are none.
//! Level and runes held come from PlayerGameData (offsets from the eldenring crate's struct).

use std::mem::size_of;

use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Memory::{
    VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS,
};

const WORLD_CHR_MAN: &str = "48 8B 05 ?? ?? ?? ?? 48 85 C0 74 0F 48 39 88";
const GAME_DATA_MAN: &str = "48 8B 05 ?? ?? ?? ?? 48 85 C0 74 05 48 8B 40 58 C3 C3";

const PLAYER: usize = 0x1E508; // WorldChrMan -> main player ChrIns (checked against game version 2.7.1.0)
const MODULES: usize = 0x190; // ChrIns -> module bag; its first entry is the stat module
const STAT_HP: usize = 0x138; // hp, max hp, (base max hp), _, fp, max fp
const STAT_FP: usize = 0x148;
const STAT_SP: usize = 0x154; // stamina, max stamina (CSChrDataModule in the eldenring crate)
pub use crate::sheet::*;

/// Size of a WorldChrMan characters-by-distance entry (ChrInsDistanceEntry).
const DISTANCE_ENTRY: usize = 16;
const PLAYER_GAME_DATA: usize = 0x8; // GameDataMan -> PlayerGameData: hp 0x10, max 0x14, fp 0x1C, max 0x20, sp 0x2C, max 0x30

/// Elden Ring's own "change equipment" function (EquipGameData, ChrAsm slot 0-21, &item ID, inventory index, 1, 1,
/// 0), the one its Equipment menu calls; pattern and arguments from The Grand Archives' cheat table (equipItem),
/// pattern match minus 0x17 is the function start.
const EQUIP_GEAR: &str = "?? 8B F1 ?? 8B D8 ?? 63 EA ?? 8B F9";
const EQUIP_GEAR_BACK: usize = 0x17;
/// PlayerGameData -> EquipGameData (the cheat table's offset; const-asserted against the crate below).
const EQUIP_GAME_DATA: usize = 0x2B0;

pub struct Game {
    world_chr_man: usize,
    game_data_man: usize,
    /// The equip function, if its pattern was found (exactly once).
    equip_gear: Option<usize>,
    /// The loaded characters (ChrIns, model), re-read every half second, and the models last logged.
    characters: std::sync::Mutex<(Option<std::time::Instant>, Vec<(usize, u32)>)>,
    models_logged: std::sync::Mutex<Vec<u32>>,
}

impl Game {
    /// Finds the game's globals, or None when a pattern is missing (another game version).
    pub fn find() -> Option<Game> {
        let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
        let sections = code_sections(base);
        Some(Game {
            world_chr_man: rip_target(&sections, WORLD_CHR_MAN)?,
            game_data_man: rip_target(&sections, GAME_DATA_MAN)?,
            equip_gear: scan_unique(&sections, EQUIP_GEAR).map(|at| at - EQUIP_GEAR_BACK),
            characters: Default::default(),
            models_logged: Default::default(),
        })
    }

    /// The player's position in global map-chunk coordinates (metres) with the map block the player is in and the
    /// block the chunk coordinates are relative to (`block_origin`: when it changes, the coordinates jump although
    /// the player didn't move); None without a player character.
    pub fn position(&self) -> Option<([f32; 3], i32, i32)> {
        use eldenring::cs::ChrIns;
        use std::mem::offset_of;
        let player = self.player()?;
        let [x, y, z, _] = read::<[f32; 4]>(player + offset_of!(ChrIns, chunk_position))?;
        let block = read::<i32>(player + offset_of!(ChrIns, block_id))?;
        let origin = read::<i32>(player + offset_of!(ChrIns, block_origin))?;
        [x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1.0e6).then_some(([x, y, z], block, origin))
    }

    /// The main player's ChrIns (scene probe).
    pub fn player_address(&self) -> Option<usize> {
        self.player()
    }

    /// Every loaded character (ChrIns, model), read now (scene probe).
    pub fn loaded_characters(&self) -> Vec<(usize, u32)> {
        self.characters()
    }

    /// A character's physics position (the camera's space), or None.
    pub fn chr_physics(chr: usize) -> Option<[f32; 3]> {
        use eldenring::cs::{CSChrPhysicsModule, ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let modules = read::<usize>(chr + offset_of!(ChrIns, modules))?;
        let physics = read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics))?;
        let [x, y, z, _] = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, position))?;
        [x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1.0e6).then_some([x, y, z])
    }

    fn player(&self) -> Option<usize> {
        read::<usize>(self.world_chr_man).and_then(|wcm| read::<usize>(wcm + PLAYER))
    }

    /// The game camera (CSCamera's first perspective camera: rows right, up, forward, position; vertical fov in
    /// radians) and the player's physics position, in the same space as the camera; None when they can't be read.
    pub fn camera(&self) -> Option<crate::markers::Camera> {
        use eldenring::cs::{CSCam, CSCamera, CSChrPhysicsModule, ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let player = self.player()?;
        let modules = read::<usize>(player + offset_of!(ChrIns, modules))?;
        let physics = read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics))?;
        let [px, py, pz, _] = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, position))?;
        let camera = from_singleton::static_of::<CSCamera>()?;
        let camera = read::<usize>(camera.as_ptr() as usize)?;
        let cam = read::<usize>(camera + offset_of!(CSCamera, pers_cam_1))?;
        let matrix = read::<[[f32; 4]; 4]>(cam + offset_of!(CSCam, matrix))?;
        let fov = read::<f32>(cam + offset_of!(CSCam, fov))?;
        let ok = matrix.iter().flatten().chain(&[px, py, pz]).all(|v| v.is_finite() && v.abs() < 1.0e6);
        (ok && (0.1..3.0).contains(&fov)).then_some(crate::markers::Camera { matrix, fov, player: [px, py, pz] })
    }

    /// The characters near the player whose model (character_id) is in `models` and that are alive: (model,
    /// physics position in the camera's space, hit capsule height in metres). Every character the game has loaded is
    /// looked at: the map's characters (WorldChrMan's open-field set and its per-map sets) and the by-distance list
    /// (which only holds some of them: round 8 found only the Lost Souls in it).
    pub fn enemies(&self, models: &[u32]) -> Vec<(u32, [f32; 3], f32)> {
        use eldenring::cs::{CSChrDataModule, CSChrPhysicsModule, ChrIns, ChrInsModuleContainer};
        use std::mem::offset_of;
        let mut out = Vec::new();
        let list = {
            let mut cache = self.characters.lock().unwrap_or_else(|e| e.into_inner());
            if cache.0.is_none_or(|t| t.elapsed() > std::time::Duration::from_millis(500)) {
                *cache = (Some(std::time::Instant::now()), self.characters());
            }
            cache.1.clone()
        };
        for (chr, model) in list {
            if !models.contains(&model) {
                continue;
            }
            // Still the same character (the list is half a second old).
            if read::<u32>(chr + offset_of!(ChrIns, character_id)) != Some(model) {
                continue;
            }
            let Some(modules) = read::<usize>(chr + offset_of!(ChrIns, modules)) else { continue };
            let hp = read::<usize>(modules + offset_of!(ChrInsModuleContainer, data))
                .and_then(|data| read::<i32>(data + offset_of!(CSChrDataModule, hp)));
            if hp.is_none_or(|hp| hp <= 0) {
                continue;
            }
            let Some(physics) = read::<usize>(modules + offset_of!(ChrInsModuleContainer, physics)) else { continue };
            let Some([x, y, z, _]) = read::<[f32; 4]>(physics + offset_of!(CSChrPhysicsModule, position)) else { continue };
            let height = read::<f32>(physics + offset_of!(CSChrPhysicsModule, hit_height)).unwrap_or(0.0);
            if [x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1.0e6) {
                out.push((model, [x, y, z], if height.is_finite() && (0.2..30.0).contains(&height) { height } else { 0.0 }));
            }
        }
        out
    }

    /// Every loaded character and its model: (ChrIns address, character_id).
    fn characters(&self) -> Vec<(usize, u32)> {
        use eldenring::cs::{ChrIns, ChrSet, ChrSetEntry, OpenFieldChrSet, WorldChrMan};
        use std::mem::{offset_of, size_of};
        let mut chrs: Vec<usize> = Vec::new();
        let Some(wcm) = read::<usize>(self.world_chr_man) else { return vec![] };
        // The by-distance list (DLVector: allocator, first, last, end).
        let list = wcm + offset_of!(WorldChrMan, chr_inses_by_distance);
        if let (Some(first), Some(last)) = (read::<usize>(list + 8), read::<usize>(list + 16)) {
            if last >= first && first >= 0x10000 {
                let count = ((last - first) / DISTANCE_ENTRY).min(2048);
                if let Some(entries) = read_bytes(first, count * DISTANCE_ENTRY) {
                    chrs.extend(entries.chunks_exact(DISTANCE_ENTRY).map(|e| usize::from_le_bytes(e[..8].try_into().unwrap())));
                }
            }
        }
        // A ChrSet's entries: capacity, then an array of (ChrIns pointer, status bytes).
        let entry = size_of::<ChrSetEntry<ChrIns>>();
        let mut set = |at: usize| {
            let (Some(capacity), Some(entries)) = (read::<u32>(at + offset_of!(ChrSet<ChrIns>, capacity)),
                                                   read::<usize>(at + offset_of!(ChrSet<ChrIns>, entries))) else { return };
            let n = (capacity as usize).min(4096);
            if n == 0 {
                return;
            }
            if let Some(bytes) = read_bytes(entries, n * entry) {
                chrs.extend(bytes.chunks_exact(entry).map(|e| usize::from_le_bytes(e[..8].try_into().unwrap())));
            }
        };
        set(wcm + offset_of!(WorldChrMan, open_field_chr_set) + offset_of!(OpenFieldChrSet, base));
        let sets = wcm + offset_of!(WorldChrMan, chr_sets);
        for i in 0..196 {
            if let Some(p) = read::<usize>(sets + 8 * i).filter(|p| *p >= 0x10000) {
                set(p);
            }
        }
        // Only real characters: 8-aligned, with an RTTI class named *Ins (CS::PlayerIns, CS::EnemyIns, ...). The
        // lists also hold table slots and other objects, whose "character_id" was garbage (scene probe run 1).
        chrs.retain(|c| *c >= 0x10000 && *c % 8 == 0);
        chrs.retain(|c| crate::probe::rtti(*c).is_some_and(|n| n.ends_with("Ins")));
        chrs.sort_unstable();
        chrs.dedup();
        let out: Vec<(usize, u32)> = chrs.into_iter()
            .filter_map(|c| read::<u32>(c + offset_of!(ChrIns, character_id)).map(|m| (c, m)))
            .collect();
        let mut models: Vec<u32> = out.iter().map(|c| c.1).collect();
        models.sort_unstable();
        models.dedup();
        let mut seen = self.models_logged.lock().unwrap_or_else(|e| e.into_inner());
        if *seen != models && models.len() < 400 {
            tracing::info!("characters: {} loaded, models {:?}", out.len(), models);
            *seen = models;
        }
        out
    }

    /// The player's character sheet for the spellbook, from PlayerGameData; None without a character.
    pub fn sheet(&self) -> Option<Sheet> {
        use eldenring::cs::{EquipInventoryDataListEntry, EquipMagicData, PlayerGameData};
        use std::mem::{offset_of, size_of};
        let pgd = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA))?;
        let u32_at = |offset: usize| read::<u32>(pgd + offset).unwrap_or(0);
        // character_name ([u16; 17]) is private in the crate; gender follows it directly.
        let name_at = pgd + offset_of!(PlayerGameData, gender) - 34;
        let name: Vec<u16> = (0..17).map_while(|i| read::<u16>(name_at + 2 * i).filter(|c| *c != 0)).collect();
        let level = u32_at(offset_of!(PlayerGameData, level));
        if !(1..=713).contains(&level) {
            return None;
        }
        let equipment = read::<[i32; 22]>(pgd + offset_of!(PlayerGameData, equipment.chr_asm.equipment_param_ids))
            .unwrap_or([-1; 22]);
        let spells = read::<usize>(pgd + offset_of!(PlayerGameData, equipment.equip_magic_data))
            .and_then(|magic| read::<[[i32; 2]; 14]>(magic + offset_of!(EquipMagicData, entries)))
            .map(|entries| entries.map(|[id, _charges]| id))
            .unwrap_or([-1; 14]);
        let list = |head: usize, len: usize| -> Vec<(u32, u32)> {
            let (Some(head), Some(len)) = (read::<usize>(pgd + head), read::<u32>(pgd + len)) else { return vec![] };
            let size = size_of::<EquipInventoryDataListEntry>();
            let bytes = read_bytes(head, size * (len as usize).min(4096)).unwrap_or_default();
            let id_at = offset_of!(EquipInventoryDataListEntry, item_id);
            let quantity_at = offset_of!(EquipInventoryDataListEntry, quantity);
            bytes
                .chunks_exact(size)
                .map(|e| {
                    let field = |at: usize| u32::from_le_bytes(e[at..at + 4].try_into().unwrap());
                    (field(id_at), field(quantity_at))
                })
                .filter(|(id, n)| *id != u32::MAX && *id != 0 && *n > 0)
                .fold(Vec::<(u32, u32)>::new(), |mut out, (id, n)| {
                    // Several copies of a weapon or armour piece are separate entries: one row each, summed.
                    match out.iter_mut().find(|(seen, _)| *seen == id) {
                        Some(entry) => entry.1 += n,
                        None => out.push((id, n)),
                    }
                    out
                })
        };
        let items = list(
            offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data.normal_items_head),
            offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data.normal_items_len),
        );
        let key_items = list(
            offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data.key_items_head),
            offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data.key_items_len),
        );
        let quick_items = self.quick_slots(pgd).map(|slots| slots.map(|(_, _, id)| id)).unwrap_or([0; 10]);
        Some(Sheet {
            quick_items,
            name: String::from_utf16_lossy(&name),
            level,
            runes: u32_at(offset_of!(PlayerGameData, rune_count)),
            rune_memory: u32_at(offset_of!(PlayerGameData, rune_memory)),
            attributes: [
                offset_of!(PlayerGameData, vigor),
                offset_of!(PlayerGameData, mind),
                offset_of!(PlayerGameData, endurance),
                offset_of!(PlayerGameData, strength),
                offset_of!(PlayerGameData, dexterity),
                offset_of!(PlayerGameData, intelligence),
                offset_of!(PlayerGameData, faith),
                offset_of!(PlayerGameData, arcane),
            ]
            .map(u32_at),
            resistances: [
                offset_of!(PlayerGameData, poison_resist),
                offset_of!(PlayerGameData, bleed_resist),
                offset_of!(PlayerGameData, sleep_resist),
                offset_of!(PlayerGameData, death_resist),
            ]
            .map(u32_at),
            poise: read::<f32>(pgd + offset_of!(PlayerGameData, poise)).unwrap_or(0.0),
            max_equip_load: read::<f32>(pgd + offset_of!(PlayerGameData, max_equip_load)).unwrap_or(0.0),
            flasks: [
                read::<u8>(pgd + offset_of!(PlayerGameData, max_hp_flask)).unwrap_or(0),
                read::<u8>(pgd + offset_of!(PlayerGameData, max_fp_flask)).unwrap_or(0),
            ],
            magic_slots: read::<u8>(pgd + offset_of!(PlayerGameData, unlocked_magic_slots)).unwrap_or(0),
            talisman_slots: read::<u8>(pgd + offset_of!(PlayerGameData, unlocked_talisman_slots)).unwrap_or(0),
            weapon_slots: read::<[u32; 2]>(pgd + offset_of!(PlayerGameData, equipment.chr_asm.equipment.selected_slots))
                .unwrap_or([0; 2]),
            gender: read::<u8>(pgd + offset_of!(PlayerGameData, gender)).unwrap_or(1),
            face: read_bytes(pgd + offset_of!(PlayerGameData, face_data.face_data_buffer.buffer), 276).unwrap_or_default(),
            equipment,
            spells,
            items,
            key_items,
        })
    }

    /// Puts `spell` (a magic param ID, -1 = none) in memory slot `index` (EquipMagicData's entries), as Elden
    /// Ring's own Memorize Spell menu does at a grace; false when the slot can't be reached.
    pub fn set_spell(&self, index: usize, spell: i32) -> bool {
        use eldenring::cs::{EquipMagicData, EquipMagicItem, PlayerGameData};
        use std::mem::offset_of;
        if index >= 14 {
            return false;
        }
        let Some(pgd) = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA)) else {
            return false;
        };
        let Some(magic) = read::<usize>(pgd + offset_of!(PlayerGameData, equipment.equip_magic_data)) else { return false };
        let at = magic + offset_of!(EquipMagicData, entries) + index * size_of::<EquipMagicItem>();
        if read::<i32>(at).is_none() {
            return false;
        }
        unsafe { (at as *mut i32).write_volatile(spell) };
        // The spell selected for casting must be a memorised one: the first, if it was this slot's.
        let selected = magic + offset_of!(EquipMagicData, selected_slot);
        if spell < 0 && read::<i32>(selected) == Some(index as i32) {
            let entries = read::<[[i32; 2]; 14]>(magic + offset_of!(EquipMagicData, entries)).unwrap_or([[-1, 0]; 14]);
            let first = entries.iter().position(|e| e[0] >= 0).map_or(0, |i| i as i32);
            unsafe { (selected as *mut i32).write_volatile(first) };
        }
        true
    }

    /// Equips the inventory item `item_id` (with its category bits) in ChrAsm `slot` (weapons 0-5 L1 R1 L2 R2 L3 R3,
    /// arrows 6/8, bolts 7/9, armour 12-15, talismans 17-20) through Elden Ring's own equip function, as its
    /// Equipment menu does; `item_id` None empties a talisman slot. Err says why not.
    pub fn equip(&self, slot: u32, item_id: Option<u32>) -> Result<(), &'static str> {
        use eldenring::cs::{EquipInventoryDataListEntry, InventoryItemsData, PlayerGameData};
        use std::mem::offset_of;
        let function = self.equip_gear.ok_or("Elden Ring's equip function wasn't found (game version?)")?;
        if slot > 21 {
            return Err("not an equipment slot");
        }
        let pgd = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA))
            .ok_or("no character")?;
        let equip_game_data = pgd + EQUIP_GAME_DATA;
        let items = pgd + offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data);
        let (id, index) = match item_id {
            None => (u32::MAX, -1i32),
            Some(id) => {
                let head = read::<usize>(items + offset_of!(InventoryItemsData, normal_items_head)).ok_or("no inventory")?;
                let len = read::<u32>(items + offset_of!(InventoryItemsData, normal_items_len)).unwrap_or(0) as usize;
                let key_capacity = read::<u32>(items + offset_of!(InventoryItemsData, key_items_capacity)).unwrap_or(0);
                let size = size_of::<EquipInventoryDataListEntry>();
                let bytes = read_bytes(head, size * len.min(4096)).ok_or("no inventory")?;
                let at = offset_of!(EquipInventoryDataListEntry, item_id);
                let i = bytes.chunks_exact(size)
                    .position(|e| u32::from_le_bytes(e[at..at + 4].try_into().unwrap()) == id)
                    .ok_or("not in the inventory")?;
                // The game numbers key items first, then the normal items.
                (id, (i as u32 + key_capacity) as i32)
            }
        };
        let item = id;
        type Equip = unsafe extern "C" fn(usize, i32, *const u32, i32, i32, i32, i32) -> u64;
        let equip: Equip = unsafe { std::mem::transmute::<usize, Equip>(function) };
        unsafe { equip(equip_game_data, slot as i32, &item, index, 1, 1, 0) };
        Ok(())
    }

    /// The normal inventory's entries (gaitem handle, item ID) and the key item list's capacity: the game numbers
    /// the key items first, so normal entry i is inventory index capacity + i.
    fn normal_inventory(&self, pgd: usize) -> Option<(Vec<(u32, u32)>, u32)> {
        use eldenring::cs::{EquipInventoryDataListEntry, InventoryItemsData, PlayerGameData};
        use std::mem::offset_of;
        let items = pgd + offset_of!(PlayerGameData, equipment.equip_inventory_data.items_data);
        let head = read::<usize>(items + offset_of!(InventoryItemsData, normal_items_head))?;
        let len = read::<u32>(items + offset_of!(InventoryItemsData, normal_items_len)).unwrap_or(0) as usize;
        let key_capacity = read::<u32>(items + offset_of!(InventoryItemsData, key_items_capacity)).unwrap_or(0);
        let size = size_of::<EquipInventoryDataListEntry>();
        let bytes = read_bytes(head, size * len.min(4096))?;
        let (h, at) = (offset_of!(EquipInventoryDataListEntry, gaitem_handle), offset_of!(EquipInventoryDataListEntry, item_id));
        let field = |e: &[u8], o: usize| u32::from_le_bytes(e[o..o + 4].try_into().unwrap());
        Some((bytes.chunks_exact(size).map(|e| (field(e, h), field(e, at))).collect(), key_capacity))
    }

    /// EquipItemData's ten quick item slots: (gaitem handle, inventory index, item ID or 0).
    fn quick_slots(&self, pgd: usize) -> Option<[(u32, i32, u32); 10]> {
        use eldenring::cs::PlayerGameData;
        use std::mem::offset_of;
        let raw = read::<[(u32, i32); 10]>(pgd + offset_of!(PlayerGameData, equipment.equip_item_data.quick_slots))?;
        let (normal, key_capacity) = self.normal_inventory(pgd)?;
        Some(raw.map(|(handle, index)| {
            let id = (index >= key_capacity as i32)
                .then(|| normal.get((index - key_capacity as i32) as usize).map(|e| e.1))
                .flatten()
                .filter(|id| *id != u32::MAX)
                .unwrap_or(0);
            (handle, index, id)
        }))
    }

    /// Puts the inventory item `item_id` (a spirit ash: Elden Ring uses them from the quick items) in the first
    /// empty quick item slot (`on`), or empties the slots that hold it, by writing EquipItemData's slot entries
    /// (gaitem handle and inventory index) as the Equipment menu leaves them.
    pub fn set_quick_item(&self, item_id: u32, on: bool) -> Result<(), &'static str> {
        use eldenring::cs::PlayerGameData;
        use std::mem::offset_of;
        let pgd = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA))
            .ok_or("no character")?;
        let slots = self.quick_slots(pgd).ok_or("the quick items can't be read")?;
        let at = pgd + offset_of!(PlayerGameData, equipment.equip_item_data.quick_slots);
        let empty = slots.iter().find(|s| s.1 < 0).map(|s| (s.0, s.1)).unwrap_or((0, -1));
        tracing::info!("quick items before: {slots:?}");
        let write = |slot: usize, (handle, index): (u32, i32)| unsafe {
            ((at + slot * 8) as *mut u32).write_volatile(handle);
            ((at + slot * 8 + 4) as *mut i32).write_volatile(index);
        };
        if !on {
            slots.iter().enumerate().filter(|(_, s)| s.2 == item_id).for_each(|(i, _)| write(i, empty));
            return Ok(());
        }
        if slots.iter().any(|s| s.2 == item_id) {
            return Ok(());
        }
        let (normal, key_capacity) = self.normal_inventory(pgd).ok_or("no inventory")?;
        let i = normal.iter().position(|e| e.1 == item_id).ok_or("not in the inventory")?;
        let free = slots.iter().position(|s| s.1 < 0).ok_or("the quick item slots are full")?;
        write(free, (normal[i].0, (i as u32 + key_capacity) as i32));
        Ok(())
    }

    /// Every candidate value, for `stats` and for the debug log.
    pub fn probe(&self) -> Probe {
        let player = read::<usize>(self.world_chr_man).and_then(|wcm| read::<usize>(wcm + PLAYER));
        let stat = player.and_then(|p| read::<usize>(p + MODULES)).and_then(|modules| read::<usize>(modules));
        let live = stat.and_then(|stat| {
            Some(Stats {
                hp: read(stat + STAT_HP)?,
                max_hp: read(stat + STAT_HP + 4)?,
                fp: read(stat + STAT_FP)?,
                max_fp: read(stat + STAT_FP + 4)?,
                sp: read(stat + STAT_SP)?,
                max_sp: read(stat + STAT_SP + 4)?,
                level: None,
            })
        });
        let pgd = read::<usize>(self.game_data_man).and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA));
        let saved = pgd.and_then(|pgd| {
            Some(Stats {
                hp: read(pgd + 0x10)?,
                max_hp: read(pgd + 0x14)?,
                fp: read(pgd + 0x1C)?,
                max_fp: read(pgd + 0x20)?,
                sp: read(pgd + 0x2C)?,
                max_sp: read(pgd + 0x30)?,
                level: None,
            })
        });
        let level = pgd.and_then(|pgd| {
            use eldenring::cs::PlayerGameData;
            let level = read::<u32>(pgd + std::mem::offset_of!(PlayerGameData, level))?;
            let runes = read::<u32>(pgd + std::mem::offset_of!(PlayerGameData, rune_count))?;
            (1..=713).contains(&level).then_some((level, runes))
        });
        Probe { player, stat, live, saved, hud: hud_values(), level }
    }
}

/// The candidates `Game::probe` found.
#[derive(Debug)]
pub struct Probe {
    pub player: Option<usize>,
    /// The stat module's address; only shown in the debug log.
    #[allow(dead_code)]
    pub stat: Option<usize>,
    pub live: Option<Stats>,
    pub saved: Option<Stats>,
    /// (hp, fp, max fp, stamina, max stamina) from the HUD's FrontEndViewValues.
    pub hud: Option<[i32; 5]>,
    pub level: Option<(u32, u32)>,
}

impl Probe {
    /// Current stats, or None while no character is loaded (title screen, loading screens): the player character
    /// has to exist in the world. Its own stat module gives the live values; the save's copy is the fallback if
    /// those offsets ever look wrong. Stamina comes from the values Elden Ring's own HUD shows (CSFeMan's
    /// FrontEndViewValues), else the stat module, never from the save's copy, which isn't updated while you run.
    pub fn stats(&self) -> Option<Stats> {
        self.player?;
        let live = self.live.filter(plausible);
        let mut stats = live.or(self.saved.filter(plausible))?;
        // Stamina: what Elden Ring's own (hidden) bar shows, which is live by definition; then the stat module.
        match self.hud {
            Some([_, _, _, sp, max_sp]) if (1..100_000).contains(&max_sp) && (0..=max_sp).contains(&sp) => {
                (stats.sp, stats.max_sp) = (sp, max_sp);
            }
            _ if live.is_none() => stats.sp = stats.max_sp,
            _ => {}
        }
        Some(Stats { level: self.level, ..stats })
    }
}

/// What Elden Ring's own HUD is showing: CSFeMan copies hp, fp and stamina into FrontEndViewValues every frame.
fn hud_values() -> Option<[i32; 5]> {
    use eldenring::cs::{CSFeManImp, FrontEndViewValues};
    use std::mem::offset_of;
    let fe_man = from_singleton::static_of::<CSFeManImp>()?;
    let values = read::<usize>(fe_man.as_ptr() as usize)? + offset_of!(CSFeManImp, frontend_values);
    Some([
        read(values + offset_of!(FrontEndViewValues, player_hp))?,
        read(values + offset_of!(FrontEndViewValues, fp))?,
        read(values + offset_of!(FrontEndViewValues, fp_max))?,
        read(values + offset_of!(FrontEndViewValues, stamina))?,
        read(values + offset_of!(FrontEndViewValues, stamina_max))?,
    ])
}

/// An event flag (CSEventFlagMan), or None when the flag manager isn't there.
pub fn event_flag(flag: u32) -> Option<bool> {
    let man = from_singleton::static_of::<eldenring::cs::CSEventFlagMan>()?;
    let man = unsafe { (*man.as_ptr()).as_ref()? };
    Some(man.virtual_memory_flag.get_flag(flag))
}

/// Whether the event flag manager holds the 1000-flag group `group` (flag / 1000) at all: flags in a group it
/// doesn't hold read as off and can't be set, by the game's EMEVD or by us.
pub fn flag_group_exists(group: u32) -> Option<bool> {
    let man = from_singleton::static_of::<eldenring::cs::CSEventFlagMan>()?;
    let man = unsafe { (*man.as_ptr()).as_ref()? };
    Some(man.virtual_memory_flag.flag_block_descriptors.find(&group).is_some())
}

/// Elden Ring's count of loading screens shown (CSNowLoadingHelper, by name from the game's reflection data): it
/// goes up by one as each of its loading screens starts (fast travel, map warps, death, loading a save, ...).
pub fn loading_screens() -> Option<i32> {
    let helper = from_singleton::static_of::<eldenring::cs::CSNowLoadingHelper>()?;
    let helper = read::<usize>(helper.as_ptr() as usize)?;
    read::<i32>(helper + std::mem::offset_of!(eldenring::cs::CSNowLoadingHelper, menu_load_counter))
}

/// Sets or clears an event flag.
pub fn set_event_flag(flag: u32, on: bool) {
    let Some(man) = from_singleton::static_of::<eldenring::cs::CSEventFlagMan>() else { return };
    if let Some(man) = unsafe { (*man.as_ptr()).as_mut() } {
        man.virtual_memory_flag.set_flag(flag, on);
    }
}

/// Whether Elden Ring is showing its own HP/FP bars right now. The front-end manager (CSFeMan, found by name through
/// the game's reflection data, so patches don't move it) switches its HUD state away from "default" while a menu,
/// the map or a loading screen is up. If CSFeMan can't be found at all, the globes stay on.
pub fn hud_shown() -> bool {
    const DEFAULT: u8 = 3; // CSFeManHudState::Default
    let Some(fe_man) = from_singleton::static_of::<eldenring::cs::CSFeManImp>() else { return true };
    read::<usize>(fe_man.as_ptr() as usize)
        .and_then(|fe| read::<u8>(fe + std::mem::offset_of!(eldenring::cs::CSFeManImp, hud_state)))
        == Some(DEFAULT)
}

/// CSFeMan's raw HUD state (3 = default, the HUD is up), for the scene probe.
pub fn hud_state() -> Option<u8> {
    let fe_man = from_singleton::static_of::<eldenring::cs::CSFeManImp>()?;
    read::<usize>(fe_man.as_ptr() as usize)
        .and_then(|fe| read::<u8>(fe + std::mem::offset_of!(eldenring::cs::CSFeManImp, hud_state)))
}

/// How far the game has faded the screen out (0 = clear, 1 = covered), the most opaque of its fade plates. Loading
/// screens and cutscene transitions fade out first, so the globes go with them.
pub fn faded() -> f32 {
    let Some(fade) = from_singleton::static_of::<eldenring::cs::CSFade>() else { return 0.0 };
    let Some(fade) = read::<usize>(fade.as_ptr() as usize) else { return 0.0 };
    let plates = fade + std::mem::offset_of!(eldenring::cs::CSFade, fade_plates);
    let alpha = std::mem::offset_of!(eldenring::cs::CSFD4FadePlate, current_color) + 12;
    (0..9)
        .filter_map(|i| read::<usize>(plates + 8 * i).and_then(|plate| read::<f32>(plate + alpha)))
        .filter(|a| a.is_finite())
        .fold(0.0, f32::max)
        .clamp(0.0, 1.0)
}

fn plausible(s: &Stats) -> bool {
    (1..100_000).contains(&s.max_hp) && (0..=s.max_hp).contains(&s.hp) && (0..100_000).contains(&s.max_fp)
        && (0..=s.max_fp.max(0)).contains(&s.fp) && (0..100_000).contains(&s.max_sp)
        && (0..=s.max_sp.max(0)).contains(&s.sp)
}

/// `len` bytes at `address`, or None if any of them can't be read.
pub(crate) fn read_bytes(address: usize, len: usize) -> Option<Vec<u8>> {
    if address < 0x10000 {
        return None;
    }
    let mut buffer = vec![0u8; len];
    let mut got = 0;
    let ok = unsafe {
        ReadProcessMemory(GetCurrentProcess(), address as _, buffer.as_mut_ptr().cast(), len, &mut got)
    };
    (ok != 0 && got == len).then_some(buffer)
}

/// Reads a value without crashing on a stale pointer: ReadProcessMemory on our own process fails cleanly when the
/// memory isn't readable. (VirtualQuery did the same job but walks the page tables of the whole region, which in
/// Elden Ring's multi-gigabyte heaps took several milliseconds per call and dropped the game to a few fps.)
pub(crate) fn read<T: Copy>(address: usize) -> Option<T> {
    if address < 0x10000 {
        return None;
    }
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut got = 0;
    let ok = unsafe {
        ReadProcessMemory(GetCurrentProcess(), address as _, value.as_mut_ptr().cast(), size_of::<T>(), &mut got)
    };
    (ok != 0 && got == size_of::<T>()).then(|| unsafe { value.assume_init() })
}

/// (start, length) of each executable section of the module at `base`.
pub(crate) fn code_sections(base: usize) -> Vec<(usize, usize)> {
    unsafe {
        let pe = base + *((base + 0x3C) as *const u32) as usize;
        let count = *((pe + 6) as *const u16) as usize;
        let optional_size = *((pe + 20) as *const u16) as usize;
        let table = pe + 24 + optional_size;
        (0..count)
            .filter_map(|i| {
                let section = table + 40 * i;
                let characteristics = *((section + 36) as *const u32);
                let size = *((section + 8) as *const u32) as usize;
                let rva = *((section + 12) as *const u32) as usize;
                (characteristics & 0x2000_0000 != 0).then_some((base + rva, size))
            })
            .collect()
    }
}

/// The readable parts of [start, start + size), split where the memory protection changes.
pub(crate) fn readable_regions(start: usize, size: usize) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut at = start;
    while at < start + size {
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(at as _, &mut info, size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 {
            break;
        }
        let end = (info.BaseAddress as usize + info.RegionSize).min(start + size);
        if info.State == MEM_COMMIT && info.Protect & (PAGE_NOACCESS | PAGE_GUARD) == 0 {
            regions.push((at, end - at));
        }
        at = end;
    }
    regions
}

/// Address of the only match of `pattern` in the code sections; None when there is none or more than one.
fn scan_unique(sections: &[(usize, usize)], pattern: &str) -> Option<usize> {
    let pattern: Vec<Option<u8>> = pattern
        .split_whitespace()
        .map(|t| if t == "??" { None } else { u8::from_str_radix(t, 16).ok() })
        .collect();
    let mut found = None;
    for (start, size) in sections.iter().flat_map(|&(start, size)| readable_regions(start, size)) {
        let bytes = unsafe { std::slice::from_raw_parts(start as *const u8, size) };
        for (i, w) in bytes.windows(pattern.len()).enumerate() {
            if w.iter().zip(&pattern).all(|(b, p)| p.map_or(true, |p| p == *b)) {
                if found.is_some() {
                    return None;
                }
                found = Some(start + i);
            }
        }
    }
    found
}

/// Address that the first match of a `mov reg, [rip+disp32]` pattern loads from.
fn rip_target(sections: &[(usize, usize)], pattern: &str) -> Option<usize> {
    let pattern: Vec<Option<u8>> = pattern
        .split_whitespace()
        .map(|t| if t == "??" { None } else { u8::from_str_radix(t, 16).ok() })
        .collect();
    for (start, size) in sections.iter().flat_map(|&(start, size)| readable_regions(start, size)) {
        let bytes = unsafe { std::slice::from_raw_parts(start as *const u8, size) };
        let hit = bytes
            .windows(pattern.len())
            .position(|w| w.iter().zip(&pattern).all(|(b, p)| p.map_or(true, |p| p == *b)));
        if let Some(i) = hit {
            let disp = i32::from_le_bytes(bytes[i + 3..i + 7].try_into().unwrap());
            return Some((start + i + 7).wrapping_add_signed(disp as isize));
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------------------------
// The spellbook's Settings page (options.rs): Elden Ring's own options, graphics settings and quit.

/// GameDataMan -> GameSettings (Elden Ring's options: camera, sound, display; saved with the game's user data).
const GAME_SETTINGS: usize = 0x58;

impl Game {
    fn game_settings(&self) -> Option<usize> {
        let gdm = read::<usize>(self.game_data_man)?;
        read::<usize>(gdm + GAME_SETTINGS).filter(|p| *p != 0)
    }

    /// One byte of Elden Ring's GameSettings (options.rs has the offsets).
    pub fn option(&self, offset: usize) -> Option<u8> {
        read::<u8>(self.game_settings()? + offset)
    }

    /// Writes one byte of GameSettings: the same value Elden Ring's own options menu writes. The game saves it
    /// with its user data on its next save.
    pub fn set_option(&self, offset: usize, value: u8) -> bool {
        let Some(at) = self.game_settings().map(|g| g + offset) else { return false };
        if read::<u8>(at).is_none() {
            return false;
        }
        unsafe { (at as *mut u8).write_volatile(value) };
        true
    }
}

/// CSWindow's persistent screen config (what Elden Ring writes to GraphicsConfig.xml), as its 25 u32 fields.
fn window_config() -> Option<usize> {
    use eldenring::cs::CSWindowImp;
    let window = from_singleton::static_of::<CSWindowImp>()?;
    let window = read::<usize>(window.as_ptr() as usize).filter(|p| *p != 0)?;
    Some(window + std::mem::offset_of!(CSWindowImp, persistent_window_config))
}

/// The graphics settings Elden Ring will save (CSWindowScreenConfig's fields in order), or None.
pub fn graphics() -> Option<[u32; 25]> {
    read::<[u32; 25]>(window_config()?)
}

/// Sets one field of the persistent screen config (applied by Elden Ring when it next starts, or saved by it).
pub fn set_graphics(index: usize, value: u32) -> bool {
    let Some(at) = window_config().map(|c| c + index * 4).filter(|_| index < 25) else { return false };
    if read::<u32>(at).is_none() {
        return false;
    }
    unsafe { (at as *mut u32).write_volatile(value) };
    true
}

/// Quit to the title screen the way Elden Ring's system menu does (it saves first): the menu data's quit-out
/// byte, [[CSMenuMan] + 0x8] + 0x5D (the chain the Elden Ring practice tool's "quitout" uses).
pub fn quit_to_title() -> bool {
    let Some(menu) = from_singleton::static_of::<eldenring::cs::CSMenuManImp>() else { return false };
    let Some(menu) = read::<usize>(menu.as_ptr() as usize).filter(|p| *p != 0) else { return false };
    let Some(data) = read::<usize>(menu + 0x8).filter(|p| *p != 0) else { return false };
    let at = data + 0x5D;
    if read::<u8>(at).is_none() {
        return false;
    }
    unsafe { (at as *mut u8).write_volatile(1) };
    true
}

/// Asks Elden Ring's window to close (WM_CLOSE: the game's own exit, as Alt+F4). Used from the title screen only.
pub fn close_window() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let Some(window) = from_singleton::static_of::<eldenring::cs::CSWindowImp>() else { return false };
    let Some(window) = read::<usize>(window.as_ptr() as usize).filter(|p| *p != 0) else { return false };
    let Some(hwnd) = read::<isize>(window + std::mem::offset_of!(eldenring::cs::CSWindowImp, window_handle))
        .filter(|h| *h != 0) else { return false };
    unsafe { PostMessageW(hwnd as _, WM_CLOSE, 0, 0) != 0 }
}

// The hand-written offsets above must agree with the eldenring crate's struct layouts.
const _: () = {
    use eldenring::cs::{CSChrDataModule, ChrInsDistanceEntry, PlayerGameData, WorldChrMan};
    use std::mem::{offset_of, size_of};
    assert!(offset_of!(WorldChrMan, main_player) == PLAYER);
    assert!(size_of::<ChrInsDistanceEntry>() == DISTANCE_ENTRY);
    assert!(size_of::<eldenring::DLVector<ChrInsDistanceEntry>>() == 32);
    assert!(offset_of!(CSChrDataModule, hp) == STAT_HP);
    assert!(offset_of!(CSChrDataModule, fp) == STAT_FP);
    assert!(offset_of!(CSChrDataModule, stamina) == STAT_SP);
    assert!(offset_of!(PlayerGameData, current_hp) == 0x10);
    assert!(offset_of!(PlayerGameData, equipment) == EQUIP_GAME_DATA);
    assert!(offset_of!(PlayerGameData, current_fp) == 0x1C);
    assert!(offset_of!(PlayerGameData, current_stamina) == 0x2C);
    {
        use eldenring::cs::{CSWindowScreenConfig, GameDataMan, GameSettings};
        use crate::options::opt;
        assert!(offset_of!(GameDataMan, game_settings) == GAME_SETTINGS);
        assert!(size_of::<CSWindowScreenConfig>() == 25 * 4);
        assert!(offset_of!(CSWindowScreenConfig, grass_quality) == 24 * 4);
        assert!(offset_of!(GameSettings, camera_speed) == opt::CAMERA_SPEED);
        assert!(offset_of!(GameSettings, controller_rumble_strength) == opt::RUMBLE);
        assert!(offset_of!(GameSettings, brightness) == opt::BRIGHTNESS);
        assert!(offset_of!(GameSettings, music_volume) == opt::MUSIC);
        assert!(offset_of!(GameSettings, sfx_volume) == opt::SFX);
        assert!(offset_of!(GameSettings, voice_volume) == opt::VOICE);
        assert!(offset_of!(GameSettings, display_blood) == opt::BLOOD);
        assert!(offset_of!(GameSettings, show_subtitles) == opt::SUBTITLES);
        assert!(offset_of!(GameSettings, hud_type) == opt::HUD);
        assert!(offset_of!(GameSettings, reverse_camera_xaxis) == opt::INVERT_X);
        assert!(offset_of!(GameSettings, reverse_camera_yaxis) == opt::INVERT_Y);
        assert!(offset_of!(GameSettings, auto_lock_on) == opt::AUTO_LOCK);
        assert!(offset_of!(GameSettings, camera_auto_wall_recovery) == opt::WALL_RECOVERY);
        assert!(offset_of!(GameSettings, reset_camera_yaxis) == opt::RESET_Y);
        assert!(offset_of!(GameSettings, cinematic_effects) == opt::CINEMATIC);
        assert!(offset_of!(GameSettings, manual_attack_aiming) == opt::MANUAL_AIM);
        assert!(offset_of!(GameSettings, auto_target) == opt::AUTO_TARGET);
        assert!(offset_of!(GameSettings, start_offline) == opt::START_OFFLINE);
        assert!(offset_of!(GameSettings, hdr_brightness) == opt::HDR_BRIGHTNESS);
        assert!(offset_of!(GameSettings, hdr_max_brightness) == opt::HDR_PEAK);
        assert!(offset_of!(GameSettings, hdr_contrast) == opt::HDR_CONTRAST);
        assert!(offset_of!(GameSettings, master_volume) == opt::MASTER);
        assert!(offset_of!(GameSettings, mark_new_items) == opt::MARK_NEW);
        assert!(offset_of!(GameSettings, show_recent_items) == opt::RECENT_ITEMS);
        assert!(offset_of!(GameSettings, show_tutorials) == opt::TUTORIALS);
        assert!(offset_of!(GameSettings, camera_auto_rotation) == opt::AUTO_ROTATION);
    }
};
