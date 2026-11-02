//! Wizard101's music on Unicorn Way (round 20 M1-M5).
//!
//! Tracks (the classic Wizard City music, extracted from the Wizard101 install by tools/w101_music.py into
//! `w101_orbs/music/`):
//! - street: WizardCity4M ("Wizard City Theme 3"), what the Unicorn Way zone plays (its gamedata.bin names
//!   `Sound/WizardCity4M.mp3`); the classic copy is ClassicMode-WorldData.wad `Music/WizardCity4M.mp3`.
//! - battle: Combat1Test ("Wizard City Combat Theme"), the classic Wizard City combat track (ClassicMode-WorldData
//!   `Music/Combat1Test.mp3`); the updated Unicorn Way plays WC_HauntedWay_Combat.ogg instead.
//! - boss (round 21): WC_HauntedWay_Combat.ogg, the combat track Unicorn Way's boss rooms (T1/T2 interiors) name
//!   (Music-WorldData.wad `WC/`), falling back to the battle track; ini music_boss can name another file.
//!   The battle track comes from Root.wad `Sound/Combat1Test.mp3` when the classic copy was never streamed in.
//!
//! The music plays while the player is in the street's tile (questhelper::player_msb, which also covers the rooms
//! under the street). Combat: an alive Unicorn Way creature (nameplates::CREATURES) within FIGHT_RANGE while the
//! player's stamina drops (Ryan: "when your stamina starts being actually used"), the player loses HP, or such a
//! creature loses HP. Boss: Elden Ring's boss bar is up for one of our bosses
//! (GameDataMan, eldenring crate layout), or a boss creature is alive near the player in a room. After `calm`
//! seconds (ini music_calm, default 6) with none of that, the street theme comes back.
//!
//! M5: Elden Ring's own music is silenced through its Music Volume option (GameSettings.music_volume, the byte its
//! options menu writes) while ours plays, and set back when the player leaves the street, quits out, or ini
//! music = 0. The value it had is kept in w101_orbs.ini (music_er_volume) so a crash can't lose it.
//!
//! Round 22 S1: where the player is comes from the street NPCs the game has loaded (lib.rs passes
//! questhelper::player_msb_from_npcs, then the block-origin read), and a missing or bad read no longer stops the
//! music: the last good position holds for HOLD seconds (Ryan's 00:30-00:35 log: the block origin read
//! (1009394688, -1) after a grace warp and the music, and so the battle music, stayed off for five minutes).
//!
//! Round 22 S3/S4: the same thread mixes Wizard101's interface sounds over the music (`sfx`, files in
//! `w101_orbs/sfx/` from Root.wad / _Shared-WorldData.wad Sound/GUI, the .gui files' m_sSelectSound names) and
//! one voice line at a time (`voice`), which ducks the music while it plays.
//!
//! Output: one background thread decoding with symphonia and writing 16-bit stereo PCM through waveOut
//! (winmm), with crossfades between tracks and a fade when the game loses focus.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Once, OnceLock};
use std::time::{Duration, Instant};

use crate::game::{self, Game, Stats};

/// The ini keys this module reads (all optional).
/// music = 0 turns it off; music_volume 0-200 (percent, default 100); music_street / music_battle / music_boss: files relative to the
/// DLL's folder; music_calm: seconds of calm before the street theme returns; music_mute_er = 0 leaves Elden Ring's
/// music alone; music_background = 1 keeps playing while the game window is in the background.
const STREET: &str = "w101_orbs/music/WizardCity4M_classic.mp3";
const BATTLE: &str = "w101_orbs/music/Combat1Test_classic.mp3";
/// Round 21 M2: what Unicorn Way's boss rooms (T1/T2 interiors) name, streamed in once Ryan fought there.
const BOSS: &str = "w101_orbs/music/WC_HauntedWay_Combat.ogg";
/// Round 21 M2: once the battle music plays, an alive creature this close keeps it going (W101 keeps its combat
/// music until the fight ends; Elden Ring's enemies circle and back off between hits).
const STAY_RANGE: f32 = 15.0;

/// A creature this close (metres) starts the battle music by itself.
/// Stamina use, HP loss or a hit on a creature counts as a fight with a creature this close.
const FIGHT_RANGE: f32 = 25.0;
/// Boss creatures this close in a room under the street mean a boss fight.
const BOSS_RANGE: f32 = 35.0;
/// MSB height below which the player is in one of the rooms under the street (questhelper's ROOM_HEIGHT).
const ROOM_HEIGHT: f32 = 70.0;
/// The bosses' MSB entity IDs (tools/msb_add_boss.py BOSSES: Rattlebones, Lady Blackhope, Fear of Fire, Seraph).
const BOSS_IDS: std::ops::RangeInclusive<u32> = 1042360850..=1042360853;
/// Boss creature models (nameplates::CREATURES with Title::Boss).
const BOSS_MODELS: [u32; 3] = [4960, 3300, 4340];
/// Round 22 S1: how long the last good street position holds while the position can't be read.
const HOLD: Duration = Duration::from_secs(45);
/// Round 22 S3: interface sounds, relative to the DLL's folder.
const SFX_DIR: &str = "w101_orbs/sfx";
/// Music level while a voice line plays.
const DUCK: f32 = 0.45;
/// Output sample rate.
const RATE: u32 = 44100;
/// Crossfade between tracks and the focus fade, seconds.
const CROSSFADE: f32 = 1.5;
const FOCUS_FADE: f32 = 0.5;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Street,
    Battle,
    Boss,
}

/// What the audio thread should play (shared with it).
#[derive(Default)]
struct Want {
    track: Option<PathBuf>,
    volume: f32,
    paused: bool,
    /// Round 22 S3: one-shot sounds to start (taken by the audio thread).
    shots: Vec<PathBuf>,
    /// Round 22 S4: the voice line to play (None stops it) and a counter that changes with each new line.
    voice: Option<PathBuf>,
    voice_serial: u32,
}

static WANT: LazyLock<Arc<Mutex<Want>>> = LazyLock::new(Default::default);
static THREAD: Once = Once::new();
static FOLDER: OnceLock<PathBuf> = OnceLock::new();
static SFX_ON: AtomicBool = AtomicBool::new(true);
/// Interface sound gain (f32 bits), ini sfx_volume.
static SFX_GAIN: AtomicU32 = AtomicU32::new(0x3F80_0000);

fn start_thread() {
    THREAD.call_once(|| {
        let want = WANT.clone();
        if let Err(e) = std::thread::Builder::new().name("w101_music".into()).spawn(move || audio_thread(want)) {
            tracing::warn!("music: audio thread failed to start: {e}");
        }
    });
}

fn want() -> std::sync::MutexGuard<'static, Want> {
    WANT.lock().unwrap_or_else(|e| e.into_inner())
}

/// Round 22 S3: plays Wizard101's interface sound `name` (w101_orbs/sfx/<name>.wav) over the music.
pub fn sfx(name: &str) {
    let Some(folder) = FOLDER.get() else { return };
    if SFX_ON.load(Ordering::Relaxed) {
        play_file(folder.join(SFX_DIR).join(format!("{name}.wav")));
    }
}

/// Plays a sound file once over the music (the wisps' pick-up sound, interface sounds).
pub fn play_file(path: PathBuf) {
    if !path.is_file() {
        static MISSING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
        let mut missing = MISSING.lock().unwrap_or_else(|e| e.into_inner());
        if !missing.contains(&path) {
            tracing::warn!("sound: {} is missing", path.display());
            missing.push(path);
        }
        return;
    }
    want().shots.push(path);
    start_thread();
}

/// Round 22 S4: the voice line for dialogue `key` page `page` (0-based), from w101_orbs/book/voice.tsv
/// ("key<TAB>page<TAB>file under w101_orbs/voice/"), replacing the one playing; None (or no line) stops it.
pub fn voice(page: Option<(&str, usize)>) {
    static LINES: OnceLock<Vec<(String, usize, PathBuf)>> = OnceLock::new();
    let Some(folder) = FOLDER.get() else { return };
    let lines = LINES.get_or_init(|| {
        let text = std::fs::read_to_string(folder.join("w101_orbs/book/voice.tsv")).unwrap_or_default();
        let lines: Vec<_> = text.lines().filter(|l| !l.starts_with('#')).filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?.to_string(), f.next()?.trim().parse().ok()?,
                  folder.join("w101_orbs/voice").join(f.next()?.trim())))
        }).collect();
        tracing::info!("voice: {} dialogue voice lines in book/voice.tsv", lines.len());
        lines
    });
    let path = page.and_then(|(key, n)| lines.iter().find(|l| l.0 == key && l.1 == n)).map(|l| l.2.clone())
        .filter(|p| p.is_file());
    let mut w = want();
    if path.is_some() || w.voice.is_some() {
        w.voice = path;
        w.voice_serial = w.voice_serial.wrapping_add(1);
        drop(w);
        start_thread();
    }
}

pub struct Music {
    folder: PathBuf,
    ini: PathBuf,
    enabled: bool,
    volume: f32,
    tracks: [PathBuf; 3],
    calm: Duration,
    mute_er: bool,
    background: bool,
    mode: Option<Mode>,
    /// Round 22 S1: the last good street position and when it was seen.
    last_msb: Option<([f32; 3], Instant)>,
    holding_logged: bool,
    last_fight: Option<Instant>,
    last_boss: Option<Instant>,
    last_sp: Option<i32>,
    last_hp: Option<i32>,
    creature_hp: Vec<(usize, i32)>,
    /// Elden Ring's music volume we turned down (to restore), and whether it is down now.
    er_saved: Option<u8>,
    er_muted: bool,
    missing_logged: Vec<PathBuf>,
    /// Round 21 M1: Elden Ring's quit-out byte as last seen, and whether it rose while the player was here (a real
    /// quit-out). The byte also stays set after a load from the title screen, which kept the music off on the
    /// street until the first door warp cleared it (Ryan's log 23:45-23:50).
    quit_byte: bool,
    quit_latched: bool,
    /// The reasons the music is off, as last logged.
    why_off: String,
}

impl Music {
    pub fn new(folder: &Path) -> Music {
        let _ = FOLDER.set(folder.to_path_buf());
        let ini = folder.join("w101_orbs.ini");
        let mut m = Music {
            folder: folder.to_path_buf(),
            ini: ini.clone(),
            enabled: true,
            volume: 1.0,
            tracks: [STREET.into(), BATTLE.into(), BOSS.into()],
            calm: Duration::from_secs(6),
            mute_er: true,
            background: false,
            mode: None,
            last_msb: None,
            holding_logged: false,
            last_fight: None,
            last_boss: None,
            last_sp: None,
            last_hp: None,
            creature_hp: Vec::new(),
            er_saved: None,
            er_muted: false,
            missing_logged: Vec::new(),
            quit_byte: false,
            quit_latched: false,
            why_off: String::new(),
        };
        for line in std::fs::read_to_string(&ini).unwrap_or_default().lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let value = value.trim();
            match key.trim() {
                "music" => m.enabled = value != "0",
                "music_volume" => m.volume = value.parse::<f32>().map(|v| v.clamp(0.0, 200.0) / 100.0).unwrap_or(m.volume),
                "music_street" if !value.is_empty() => m.tracks[0] = value.into(),
                "music_battle" if !value.is_empty() => m.tracks[1] = value.into(),
                "music_boss" if !value.is_empty() => m.tracks[2] = value.into(),
                "music_calm" => m.calm = value.parse::<f32>().ok().filter(|v| (0.5..120.0).contains(v))
                    .map(Duration::from_secs_f32).unwrap_or(m.calm),
                "music_mute_er" => m.mute_er = value != "0",
                "music_background" => m.background = value == "1",
                "music_er_volume" => m.er_saved = value.parse::<u8>().ok().filter(|v| *v <= 10),
                "sfx" => SFX_ON.store(value != "0", Ordering::Relaxed),
                "sfx_volume" => if let Ok(v) = value.parse::<f32>() {
                    SFX_GAIN.store((v.clamp(0.0, 200.0) / 100.0).to_bits(), Ordering::Relaxed);
                },
                _ => {}
            }
        }
        tracing::info!("music: {} (volume {:.0}%, street {}, battle {}, boss {}, calm {:?}, mute Elden Ring's music {}, \
                        saved Elden Ring music volume {:?})", if m.enabled { "on" } else { "off" }, m.volume * 100.0,
                       m.tracks[0].display(), m.tracks[1].display(), m.tracks[2].display(), m.calm, m.mute_er, m.er_saved);
        m
    }

    /// Called every frame with what the HUD read this frame.
    /// `street` (round 22 S1): the player's MSB position on Unicorn Way's tile this frame, from the loaded street
    /// NPCs or the block origin (lib.rs), None when neither could be read.
    pub fn frame(&mut self, game: Option<&Game>, stats: Option<Stats>, position: Option<([f32; 3], i32, i32)>,
                 street: Option<[f32; 3]>, focused: bool) {
        let now = Instant::now();
        let quit = game.is_some() && game::quitting_out();
        if quit && !self.quit_byte && self.mode.is_some() {
            self.quit_latched = true;
            self.last_msb = None;
        }
        if position.is_none() {
            self.quit_latched = false;
        }
        self.quit_byte = quit;
        if game.is_none() {
            self.last_msb = None;
        }
        if let Some(p) = street {
            self.last_msb = Some((p, now));
            self.holding_logged = false;
        }
        // A missing or bad read (loading screens, a grace warp's garbage block origin) keeps the last good spot.
        let held = self.last_msb.filter(|(_, t)| now - *t < HOLD);
        if street.is_none() && !self.holding_logged {
            if let Some((_, t)) = held {
                self.holding_logged = true;
                tracing::info!("music: no street position (block {:?}); holding the last one from {:.1} s ago",
                               position.map(|p| (p.1, p.2)), (now - t).as_secs_f32());
            }
        }
        let msb = street.or(held.map(|h| h.0));
        let here = self.enabled && game.is_some() && msb.is_some() && !self.quit_latched;
        let why_off = if here {
            String::new()
        } else {
            format!("enabled {}, game {}, position {:?}, on the street tile {}, last street position {:?} s ago, \
                     quit-out byte {quit}, quit-out {}", self.enabled, game.is_some(), position.map(|p| (p.1, p.2)),
                    street.is_some(), self.last_msb.map(|(_, t)| (now - t).as_secs()), self.quit_latched)
        };
        if why_off != self.why_off {
            if !why_off.is_empty() {
                tracing::info!("music: not playing ({why_off})");
            }
            self.why_off = why_off;
        }
        self.er_volume(game, here);
        let (Some(g), Some(msb), true) = (game, msb, here) else {
            if self.mode.take().is_some() {
                tracing::info!("music: off (left the street, quit out or turned off)");
            }
            self.last_fight = None;
            self.last_boss = None;
            self.last_sp = None;
            self.last_hp = None;
            self.set(None, focused);
            return;
        };
        let (fight, boss) = self.signals(g, stats, msb);
        if let Some(why) = &fight {
            if self.last_fight.is_none_or(|t| now - t > self.calm) {
                tracing::info!("music: fight signal: {why}");
            }
            self.last_fight = Some(now);
        }
        if let Some(why) = &boss {
            if self.last_boss.is_none_or(|t| now - t > self.calm) {
                tracing::info!("music: boss signal: {why}");
            }
            self.last_boss = Some(now);
        }
        let recent = |t: Option<Instant>| t.is_some_and(|t| now - t < self.calm);
        let mode = if recent(self.last_boss) {
            Mode::Boss
        } else if recent(self.last_fight) {
            Mode::Battle
        } else {
            Mode::Street
        };
        if self.mode != Some(mode) {
            tracing::info!("music: {:?} -> {mode:?}", self.mode);
            self.mode = Some(mode);
        }
        let track = self.track(mode);
        self.set(track, focused);
    }

    /// Why this frame counts as a fight / a boss fight, if it does.
    fn signals(&mut self, g: &Game, stats: Option<Stats>, msb: [f32; 3]) -> (Option<String>, Option<String>) {
        let models = crate::nameplates::models();
        let creatures = g.combatants(&models);
        let player = g.player_address().and_then(Game::chr_physics);
        let dist = |p: [f32; 3]| player.map(|q| ((p[0] - q[0]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
            + ((p[1] - q[1]).abs() - 6.0).max(0.0)).unwrap_or(f32::MAX);
        let name = |model: u32| crate::nameplates::CREATURES.iter().find(|c| c.0 == model).map_or("?", |c| c.1);
        let nearest = creatures.iter().filter(|c| c.3 > 0).map(|c| (dist(c.2), c.1))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let mut fight = None;
        let near = nearest.filter(|n| n.0 < FIGHT_RANGE);
        if let Some((d, model)) = nearest.filter(|n| n.0 < STAY_RANGE && matches!(self.mode, Some(Mode::Battle | Mode::Boss))) {
            fight = Some(format!("{} still {d:.0} m away", name(model)));
        }
        // Round 23: being near a creature no longer starts a fight by itself (Ryan: only when Elden Ring is really in
        // battle, i.e. the stamina bar is used); it only keeps a fight that started going (STAY_RANGE above).
        if let Some(s) = stats {
            if let (Some(last), Some((d, model))) = (self.last_sp, near) {
                if s.sp < last && fight.is_none() {
                    fight = Some(format!("stamina {last} -> {} with {} {d:.0} m away", s.sp, name(model)));
                }
            }
            if let Some(last) = self.last_hp {
                if s.hp < last && s.hp > 0 && fight.is_none() && near.is_some() {
                    fight = Some(format!("player HP {last} -> {}", s.hp));
                }
            }
            self.last_sp = Some(s.sp);
            self.last_hp = Some(s.hp);
        }
        let mut hit = None;
        for c in &creatures {
            if let Some((_, last)) = self.creature_hp.iter().find(|h| h.0 == c.0) {
                if c.3 < *last && dist(c.2) < FIGHT_RANGE {
                    hit = Some(format!("{} HP {last} -> {}", name(c.1), c.3));
                }
            }
        }
        self.creature_hp = creatures.iter().map(|c| (c.0, c.3)).collect();
        fight = fight.or(hit);
        let mut boss = None;
        if let Some((active, entity)) = g.boss_fight() {
            if BOSS_IDS.contains(&entity) {
                boss = Some(format!("boss bar up for entity {entity} (fight active {active})"));
            }
        }
        if msb[1] < ROOM_HEIGHT {
            boss_probe(&creatures, player, msb);
        }
        if boss.is_none() && msb[1] < ROOM_HEIGHT {
            if let Some(c) = creatures.iter().find(|c| c.3 > 0 && BOSS_MODELS.contains(&c.1) && dist(c.2) < BOSS_RANGE) {
                boss = Some(format!("{} alive {:.0} m away in its room", name(c.1), dist(c.2)));
            }
        }
        (fight, boss)
    }

    /// The file for `mode`, falling back boss -> battle -> street when one isn't on disk.
    fn track(&mut self, mode: Mode) -> Option<PathBuf> {
        let order: &[usize] = match mode {
            Mode::Boss => &[2, 1, 0],
            Mode::Battle => &[1, 0],
            Mode::Street => &[0],
        };
        for &i in order {
            let path = self.folder.join(&self.tracks[i]);
            if path.is_file() {
                return Some(path);
            }
            if !self.missing_logged.contains(&path) {
                tracing::warn!("music: {} is missing (run tools/w101_music.py on the PC)", path.display());
                self.missing_logged.push(path);
            }
        }
        None
    }

    fn set(&mut self, track: Option<PathBuf>, focused: bool) {
        if track.is_some() {
            start_thread();
        }
        let mut w = want();
        w.track = track;
        // Round 23: Elden Ring's Music Volume setting (0-10, the one we keep while its music is silenced) scales ours.
        w.volume = self.volume * self.er_saved.filter(|_| self.er_muted).map_or(1.0, |v| v as f32 / 10.0);
        w.paused = !focused && !self.background;
    }

    /// (see the statics below for the Settings page's view of it)
    /// M5: Elden Ring's Music Volume to 0 while ours plays, back to what it was otherwise.
    fn er_volume(&mut self, game: Option<&Game>, here: bool) {
        use crate::options::opt::MUSIC;
        let Some(g) = game else { return };
        let Some(now) = g.option(MUSIC) else { return };
        if here && self.mute_er {
            // Round 23: the book's Settings page changed Music Volume while ER's music is silenced (options.rs).
            let pending = PENDING_SETTING.swap(NONE, Ordering::Relaxed);
            if pending != NONE && self.er_saved != Some(pending) {
                tracing::info!("music: Music Volume set to {pending} (our music follows it)");
                self.er_saved = Some(pending);
                self.write_ini_volume(self.er_saved);
            }
            // A non-zero value here is Ryan's (or the game's) setting: keep it to restore, then silence it.
            if now != 0 {
                self.er_saved = Some(now);
                self.write_ini_volume(self.er_saved);
                if g.set_option(MUSIC, 0) {
                    tracing::info!("music: Elden Ring's Music Volume {now} -> 0 (restored when you leave the street)");
                }
            }
            self.er_muted = true;
            SETTING.store(self.er_saved.unwrap_or(10), Ordering::Relaxed);
        } else if self.er_muted || self.er_saved.is_some() {
            if let Some(v) = self.er_saved.filter(|_| now == 0) {
                if g.set_option(MUSIC, v) {
                    tracing::info!("music: Elden Ring's Music Volume 0 -> {v} (restored)");
                }
            }
            self.er_muted = false;
            SETTING.store(NONE, Ordering::Relaxed);
            self.er_saved = None;
            self.write_ini_volume(None);
        }
    }

    /// Keeps the Elden Ring music volume to restore in w101_orbs.ini (music_er_volume), or removes it.
    fn write_ini_volume(&self, value: Option<u8>) {
        let text = std::fs::read_to_string(&self.ini).unwrap_or_default();
        let mut out: String = text.lines()
            .filter(|l| l.split_once('=').is_none_or(|(k, _)| k.trim() != "music_er_volume"))
            .map(|l| format!("{l}\n"))
            .collect();
        if let Some(v) = value {
            out += &format!("music_er_volume = {v}\n");
        }
        if out != text {
            if let Err(e) = std::fs::write(&self.ini, out) {
                tracing::warn!("music: can't write {}: {e}", self.ini.display());
            }
        }
    }
}

/// Round 25 B2/B3 (Rattlebones fell through the floor; he only hit near the walls): his place, height above the floor
/// and HP every 2 s while the player is in a room under the street with him loaded, so a test run shows where he goes.
/// MSB position = the player's MSB position + the physics offset between them; the room is msb_add_room's
/// AEG030_519_9000 (centre on the floor at RATTLEBONES_ROOM).
fn boss_probe(creatures: &[(usize, u32, [f32; 3], i32)], player: Option<[f32; 3]>, msb: [f32; 3]) {
    const RATTLEBONES_MODEL: u32 = 4960;
    const RATTLEBONES_ROOM: [f32; 3] = [-242.63, 34.0, 20.07];
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let (Some(p), Some(c)) = (player, creatures.iter().find(|c| c.1 == RATTLEBONES_MODEL)) else { return };
    let now = Instant::now();
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if last.is_some_and(|t| now - t < Duration::from_secs(2)) {
        return;
    }
    *last = Some(now);
    let at = [msb[0] + c.2[0] - p[0], msb[1] + c.2[1] - p[1], msb[2] + c.2[2] - p[2]];
    let r = |q: [f32; 3]| ((q[0] - RATTLEBONES_ROOM[0]).powi(2) + (q[2] - RATTLEBONES_ROOM[2]).powi(2)).sqrt();
    let apart = ((c.2[0] - p[0]).powi(2) + (c.2[2] - p[2]).powi(2)).sqrt();
    tracing::info!("boss probe: Rattlebones HP {} at MSB [{:.1}, {:.1}, {:.1}] ({:.1} m from the room centre, \
                    {:+.1} m above the floor); player {:.1} m from the centre, {apart:.1} m apart",
                   c.3, at[0], at[1], at[2], r(at), at[1] - RATTLEBONES_ROOM[1], r(msb));
}

// ---------------------------------------------------------------------------------------------------------------
// Decoding and output.

/// One track being played: its decoder and the frames decoded but not played yet.
struct Voice {
    path: PathBuf,
    source: Option<Source>,
    frames: Vec<[f32; 2]>,
    cursor: f64,
    gain: f32,
    target: f32,
    failed: bool,
    /// Music loops; sounds and voice lines play once.
    looping: bool,
    /// The file's sample rate.
    rate: u32,
}

struct Source {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track: u32,
    rate: u32,
}

fn open(path: &Path) -> Result<Source, String> {
    use symphonia::core::{codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream,
                          meta::MetadataOptions, probe::Hint};
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| e.to_string())?;
    let format = probed.format;
    let track = format.default_track().ok_or("no audio track")?;
    let rate = track.codec_params.sample_rate.unwrap_or(RATE);
    let decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;
    Ok(Source { track: track.id, format, decoder, rate })
}

impl Voice {
    fn new(path: PathBuf) -> Voice {
        Voice { path, source: None, frames: Vec::new(), cursor: 0.0, gain: 0.0, target: 1.0, failed: false, looping: true,
                rate: RATE }
    }

    fn once(path: PathBuf) -> Voice {
        Voice { gain: 1.0, looping: false, ..Voice::new(path) }
    }

    /// Decodes the next packet into `frames`; at the end of the file starts it over (the tracks loop).
    fn decode_more(&mut self) -> bool {
        use symphonia::core::audio::SampleBuffer;
        for _ in 0..64 {
            if self.source.is_none() {
                match open(&self.path) {
                    Ok(s) => self.source = Some(s),
                    Err(e) => {
                        tracing::warn!("music: can't play {}: {e}", self.path.display());
                        self.failed = true;
                        return false;
                    }
                }
            }
            let s = self.source.as_mut().unwrap();
            let packet = match s.format.next_packet() {
                Ok(p) => p,
                Err(_) => {
                    // End of the file (or a broken tail): loop, or end a sound.
                    self.source = None;
                    if !self.looping {
                        self.failed = true;
                        return false;
                    }
                    continue;
                }
            };
            if packet.track_id() != s.track {
                continue;
            }
            let Ok(decoded) = s.decoder.decode(&packet) else { continue };
            let spec = *decoded.spec();
            let channels = spec.channels.count().max(1);
            let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
            buf.copy_interleaved_ref(decoded);
            for f in buf.samples().chunks_exact(channels) {
                self.frames.push(if channels == 1 { [f[0], f[0]] } else { [f[0], f[1]] });
            }
            if !buf.samples().is_empty() {
                return true;
            }
        }
        false
    }

    /// The next output frame (linear resampling from the file's rate to RATE).
    fn next(&mut self) -> [f32; 2] {
        self.sample().unwrap_or([0.0; 2])
    }

    /// The next frame, None once a sound has ended (or can't be read).
    fn sample(&mut self) -> Option<[f32; 2]> {
        // A sound's source is gone at its end: keep its file's rate for the frames left.
        if let Some(s) = &self.source {
            self.rate = s.rate;
        }
        let step = self.rate as f64 / RATE as f64;
        while self.cursor as usize + 1 >= self.frames.len() {
            if self.failed || !self.decode_more() {
                return None;
            }
            if let Some(s) = &self.source {
                self.rate = s.rate;
            }
        }
        let i = self.cursor as usize;
        let t = (self.cursor - i as f64) as f32;
        let (a, b) = (self.frames[i], self.frames[i + 1]);
        self.cursor += step;
        // Drop what has been played now and then.
        if self.cursor > 8192.0 {
            let n = self.cursor as usize;
            self.frames.drain(..n);
            self.cursor -= n as f64;
        }
        Some([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])
    }
}

#[cfg(windows)]
fn audio_thread(want: Arc<Mutex<Want>>) {
    use windows_sys::Win32::Media::Audio::{waveOutOpen, waveOutPrepareHeader, waveOutWrite, HWAVEOUT,
                                           WAVEFORMATEX, WAVEHDR, WAVE_FORMAT_PCM, WAVE_MAPPER, WHDR_DONE};
    // Round 22 S3: short buffers so a click sounds with the click (4 x 512 frames, about 46 ms queued).
    const FRAMES: usize = 512;
    const BUFFERS: usize = 4;
    let format = WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_PCM as u16,
        nChannels: 2,
        nSamplesPerSec: RATE,
        nAvgBytesPerSec: RATE * 4,
        nBlockAlign: 4,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    let mut device: HWAVEOUT = std::ptr::null_mut();
    let result = unsafe { waveOutOpen(&mut device, WAVE_MAPPER, &format, 0, 0, 0) };
    if result != 0 {
        tracing::warn!("music: waveOutOpen failed ({result})");
        return;
    }
    tracing::info!("music: audio output open ({RATE} Hz stereo, waveOut)");
    let mut data: Vec<Box<[i16; FRAMES * 2]>> = (0..BUFFERS).map(|_| Box::new([0i16; FRAMES * 2])).collect();
    let mut headers: Vec<Box<WAVEHDR>> = (0..BUFFERS).map(|_| Box::new(unsafe { std::mem::zeroed::<WAVEHDR>() })).collect();
    for (h, d) in headers.iter_mut().zip(data.iter_mut()) {
        h.lpData = d.as_mut_ptr() as *mut u8;
        h.dwBufferLength = (FRAMES * 4) as u32;
        unsafe { waveOutPrepareHeader(device, &mut **h, size_of_header()) };
        h.dwFlags |= WHDR_DONE;
    }
    let mut voices: Vec<Voice> = Vec::new();
    let mut shots: Vec<Voice> = Vec::new();
    let mut line: Option<Voice> = None;
    let mut line_serial = 0u32;
    let mut duck = 1.0f32;
    let mut master = 0.0f32;
    loop {
        let Some(free) = headers.iter().position(|h| h.dwFlags & WHDR_DONE != 0) else {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        };
        let (track, volume, paused) = {
            let mut w = want.lock().unwrap_or_else(|e| e.into_inner());
            for path in w.shots.drain(..) {
                if shots.len() < 8 {
                    shots.push(Voice::once(path));
                }
            }
            if w.voice_serial != line_serial {
                line_serial = w.voice_serial;
                line = w.voice.clone().map(|p| {
                    tracing::info!("voice: playing {}", p.display());
                    Voice::once(p)
                });
            }
            (w.track.clone(), w.volume, w.paused)
        };
        let sfx_gain = f32::from_bits(SFX_GAIN.load(Ordering::Relaxed));
        // A new track fades in over the old one, which fades out.
        if voices.last().map(|v| &v.path) != track.as_ref() {
            for v in voices.iter_mut() {
                v.target = 0.0;
            }
            if let Some(path) = track {
                tracing::info!("music: playing {}", path.display());
                let mut v = Voice::new(path);
                if voices.is_empty() {
                    v.gain = 0.0;
                }
                voices.push(v);
            }
        }
        let master_target = if paused { 0.0 } else { volume };
        let fade = 1.0 / (CROSSFADE * RATE as f32);
        let focus_fade = 1.0 / (FOCUS_FADE * RATE as f32);
        let out = &mut data[free];
        for f in 0..FRAMES {
            master += (master_target - master).clamp(-focus_fade, focus_fade);
            let mut mix = [0.0f32; 2];
            // Paused and faded out: hold the tracks where they are.
            if !(paused && master <= 0.0) {
                for v in voices.iter_mut() {
                    v.gain += (v.target - v.gain).clamp(-fade, fade);
                    let s = v.next();
                    mix[0] += s[0] * v.gain;
                    mix[1] += s[1] * v.gain;
                }
            }
            duck += ((if line.is_some() { DUCK } else { 1.0 }) - duck).clamp(-focus_fade, focus_fade);
            let mut top = [0.0f32; 2];
            shots.retain_mut(|s| match s.sample() {
                Some(x) => {
                    top[0] += x[0] * sfx_gain;
                    top[1] += x[1] * sfx_gain;
                    true
                }
                None => false,
            });
            if let Some(v) = line.as_mut() {
                match v.sample() {
                    Some(x) => {
                        top[0] += x[0] * sfx_gain;
                        top[1] += x[1] * sfx_gain;
                    }
                    None => line = None,
                }
            }
            let l = mix[0] * master * duck + top[0];
            let r = mix[1] * master * duck + top[1];
            out[f * 2] = (l.clamp(-1.0, 1.0) * 32767.0) as i16;
            out[f * 2 + 1] = (r.clamp(-1.0, 1.0) * 32767.0) as i16;
        }
        voices.retain(|v| v.target > 0.0 || v.gain > 0.0);
        let h = &mut headers[free];
        h.dwFlags &= !WHDR_DONE;
        let r = unsafe { waveOutWrite(device, &mut **h, size_of_header()) };
        if r != 0 {
            tracing::warn!("music: waveOutWrite failed ({r})");
            h.dwFlags |= WHDR_DONE;
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(windows)]
fn size_of_header() -> u32 {
    std::mem::size_of::<windows_sys::Win32::Media::Audio::WAVEHDR>() as u32
}

#[cfg(not(windows))]
fn audio_thread(_want: Arc<Mutex<Want>>) {}

/// Round 23: while Elden Ring's Music Volume is held at 0 to silence its music, the Settings page shows and changes the
/// value it had (SETTING), and that value scales our music. NONE = not held.
const NONE: u8 = 255;
static SETTING: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(NONE);
static PENDING_SETTING: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(NONE);

/// options.rs: the Music Volume to show while ours plays (None: show Elden Ring's own value).
pub fn music_setting() -> Option<u8> {
    Some(SETTING.load(Ordering::Relaxed)).filter(|v| *v != NONE)
}

/// options.rs: OK on the Settings page with a new Music Volume; true when it was taken here (ours plays).
pub fn take_music_setting(value: u8) -> bool {
    if music_setting().is_none() {
        return false;
    }
    SETTING.store(value.min(10), Ordering::Relaxed);
    PENDING_SETTING.store(value.min(10), Ordering::Relaxed);
    true
}
