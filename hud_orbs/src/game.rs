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

const PLAYER_GAME_DATA: usize = 0x8; // GameDataMan -> PlayerGameData: hp 0x10, max 0x14, fp 0x1C, max 0x20, sp 0x2C, max 0x30

pub struct Game {
    world_chr_man: usize,
    game_data_man: usize,
}

impl Game {
    /// Finds the game's globals, or None when a pattern is missing (another game version).
    pub fn find() -> Option<Game> {
        let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
        let sections = code_sections(base);
        Some(Game {
            world_chr_man: rip_target(&sections, WORLD_CHR_MAN)?,
            game_data_man: rip_target(&sections, GAME_DATA_MAN)?,
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
        Some(Sheet {
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
fn read_bytes(address: usize, len: usize) -> Option<Vec<u8>> {
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
fn read<T: Copy>(address: usize) -> Option<T> {
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
fn code_sections(base: usize) -> Vec<(usize, usize)> {
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
fn readable_regions(start: usize, size: usize) -> Vec<(usize, usize)> {
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

// The hand-written offsets above must agree with the eldenring crate's struct layouts.
const _: () = {
    use eldenring::cs::{CSChrDataModule, PlayerGameData};
    use std::mem::offset_of;
    assert!(offset_of!(CSChrDataModule, hp) == STAT_HP);
    assert!(offset_of!(CSChrDataModule, fp) == STAT_FP);
    assert!(offset_of!(CSChrDataModule, stamina) == STAT_SP);
    assert!(offset_of!(PlayerGameData, current_hp) == 0x10);
    assert!(offset_of!(PlayerGameData, current_fp) == 0x1C);
    assert!(offset_of!(PlayerGameData, current_stamina) == 0x2C);
};
