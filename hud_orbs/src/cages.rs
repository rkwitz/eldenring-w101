//! Round 32 (G5, Ryan: "No sound when opening the Bone Cages any more (there was one)"). The cage events
//! (tools/emevd_quests.py) only play SFX 1060 (no sound of its own); the sound Ryan heard with each cage was the HUD's
//! "Goal Update: Open Bone Cages n of 4" popup (ui_quest_updated), which round 30 (SND2, Ryan: only Goal Complete
//! plays a sound) made silent. Wizard101's own cage sound: Root.wad ObjectData/WC/WC-FairyCage02..05.xml use
//! StateData/FairyCageStates.xml, whose Escape (FairyEscaped) state plays Sound-WorldData.wad
//! Sound/Ambient/envgen_chestwood_open.ogg (tools/w101_sfx.py copies it to w101_orbs\sfx). It is played when a cage's
//! flag (questhelper::CAGE_FLAG + c, set by the cage event as the cage opens) turns on during play.

pub const SOUND: &str = "envgen_chestwood_open.ogg";
const CAGES: u32 = 4;

#[derive(Default)]
pub struct Cages {
    seen: Option<[bool; CAGES as usize]>,
}

impl Cages {
    pub fn frame(&mut self, in_game: bool) {
        if !in_game {
            self.seen = None; // a load re-reads the saved state without playing anything
            return;
        }
        let mut now = [false; CAGES as usize];
        for (c, on) in now.iter_mut().enumerate() {
            match crate::game::event_flag(crate::questhelper::CAGE_FLAG + c as u32) {
                Some(v) => *on = v,
                None => return,
            }
        }
        if let Some(before) = self.seen {
            for c in 0..CAGES as usize {
                if now[c] && !before[c] {
                    tracing::info!("cages: Bone Cage {} opened, sound {SOUND}", c + 1);
                    crate::music::sfx(SOUND);
                }
            }
        }
        self.seen = Some(now);
    }
}

/// Round 32 (G6): the cage events' riding branch also takes this flag (+ c) as the Examine press
/// (tools/emevd_quests.py cage_ride_press); the DLL sets it while the rider is within prompt::RIDE_EXAMINE_REACH m
/// of shut cage c and presses the interact binding, and clears it PRESS_HOLD s later.
pub const CAGE_PRESS: u32 = 1042362923;
const PRESS_HOLD: f32 = 1.0;

#[derive(Default)]
pub struct RidePress {
    held_before: bool,
    on: Option<(u32, std::time::Instant)>,
}

impl RidePress {
    pub fn frame(&mut self, game: Option<&crate::game::Game>, binding: (u16, Option<u8>)) {
        if let Some((flag, since)) = self.on {
            if since.elapsed().as_secs_f32() > PRESS_HOLD {
                crate::game::set_event_flag(flag, false);
                self.on = None;
            }
        }
        let (held, _, _) = crate::input::held();
        let key = binding.1.is_some_and(|vk| crate::prompt::keys_down().get(vk as usize).copied().unwrap_or(false));
        let down = (binding.0 != 0 && held & binding.0 != 0) || key;
        let pressed = down && !self.held_before;
        self.held_before = down;
        let Some(game) = game else { return };
        if !pressed || self.on.is_some() || crate::game::event_flag(crate::prompt::RIDING_FLAG) != Some(true) {
            return;
        }
        let Some(me) = crate::questhelper::player_msb_now(game) else { return };
        let near = crate::questhelper::cages().iter().enumerate()
            .filter(|(c, _)| crate::game::event_flag(crate::questhelper::CAGE_FLAG + *c as u32) == Some(false))
            .map(|(c, p)| (c, (p[0] - me[0]).hypot(p[2] - me[2])))
            .filter(|(_, d)| *d < crate::prompt::RIDE_EXAMINE_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((c, d)) = near {
            tracing::info!("cages: Examine pressed on Torrent {d:.2} m from Bone Cage {}; passed to its event", c + 1);
            crate::game::set_event_flag(CAGE_PRESS + c as u32, true);
            self.on = Some((CAGE_PRESS + c as u32, std::time::Instant::now()));
        }
    }
}
