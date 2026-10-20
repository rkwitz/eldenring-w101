//! Reads the player's HP and FP out of Elden Ring's memory.
//!
//! The game's globals are found by byte pattern (the same patterns community tools use), so small patches don't
//! break them. The live values come from the player character (WorldChrMan -> player -> stat module); the save's
//! copy in PlayerGameData is the fallback when those offsets look wrong. Without a player character there are none.

use std::mem::size_of;

use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{
    VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS,
};

const WORLD_CHR_MAN: &str = "48 8B 05 ?? ?? ?? ?? 48 85 C0 74 0F 48 39 88";
const GAME_DATA_MAN: &str = "48 8B 05 ?? ?? ?? ?? 48 85 C0 74 05 48 8B 40 58 C3 C3";

const PLAYER: usize = 0x1E508; // WorldChrMan -> main player ChrIns (checked against game version 2.7.1.0)
const MODULES: usize = 0x190; // ChrIns -> module bag; its first entry is the stat module
const STAT_HP: usize = 0x138; // hp, max hp, (base max hp), _, fp, max fp
const STAT_FP: usize = 0x148;
const PLAYER_GAME_DATA: usize = 0x8; // GameDataMan -> PlayerGameData: hp 0x10, max 0x14, fp 0x1C, max 0x20

#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    pub fp: i32,
    pub max_fp: i32,
}

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

    /// Current stats, or None while no character is loaded (title screen, loading screens): the player character
    /// has to exist in the world. Its own stat module gives the live values; the save's copy is the fallback if
    /// those offsets ever look wrong.
    pub fn stats(&self) -> Option<Stats> {
        let player = read::<usize>(self.world_chr_man).and_then(|wcm| read::<usize>(wcm + PLAYER))?;
        let live = read::<usize>(player + MODULES)
            .and_then(|modules| read::<usize>(modules))
            .and_then(|stat| {
                Some(Stats {
                    hp: read(stat + STAT_HP)?,
                    max_hp: read(stat + STAT_HP + 4)?,
                    fp: read(stat + STAT_FP)?,
                    max_fp: read(stat + STAT_FP + 4)?,
                })
            });
        let saved = read::<usize>(self.game_data_man)
            .and_then(|gdm| read::<usize>(gdm + PLAYER_GAME_DATA))
            .and_then(|pgd| {
                Some(Stats {
                    hp: read(pgd + 0x10)?,
                    max_hp: read(pgd + 0x14)?,
                    fp: read(pgd + 0x1C)?,
                    max_fp: read(pgd + 0x20)?,
                })
            });
        live.filter(plausible).or(saved.filter(plausible))
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

fn plausible(s: &Stats) -> bool {
    (1..100_000).contains(&s.max_hp) && (0..=s.max_hp).contains(&s.hp) && (0..100_000).contains(&s.max_fp)
        && (0..=s.max_fp.max(0)).contains(&s.fp)
}

/// Reads a value only if its memory is committed and readable, so a stale pointer can't crash the game.
fn read<T: Copy>(address: usize) -> Option<T> {
    if address < 0x10000 {
        return None;
    }
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let got = unsafe { VirtualQuery(address as _, &mut info, size_of::<MEMORY_BASIC_INFORMATION>()) };
    let end = info.BaseAddress as usize + info.RegionSize;
    if got == 0 || info.State != MEM_COMMIT || info.Protect & (PAGE_NOACCESS | PAGE_GUARD) != 0
        || address + size_of::<T>() > end
    {
        return None;
    }
    Some(unsafe { std::ptr::read_unaligned(address as *const T) })
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
