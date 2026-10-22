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
const PLAYER_GAME_DATA: usize = 0x8; // GameDataMan -> PlayerGameData: hp 0x10, max 0x14, fp 0x1C, max 0x20, sp 0x2C, max 0x30

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
