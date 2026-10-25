//! The player's Steam achievements, read from the Steam client through the game's own steam_api64.dll (Elden Ring
//! unlocks its achievements through ISteamUserStats; this only reads them). Steam's achievement list is matched to
//! the book's by display name, so the Steam API names don't need to be known.

/// Per name in `names` (the book's achievement titles): unlocked or not, as Steam has it; None for one Steam
/// doesn't list. None in all when Steam isn't reachable or hasn't sent the player's stats yet.
#[cfg(windows)]
pub fn achievements(names: &[&str]) -> Option<Vec<Option<bool>>> {
    use std::ffi::{c_char, c_void, CStr, CString};
    use std::sync::OnceLock;
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

    type Accessor = unsafe extern "C" fn() -> *mut c_void;
    type Count = unsafe extern "C" fn(*mut c_void) -> u32;
    type NameAt = unsafe extern "C" fn(*mut c_void, u32) -> *const c_char;
    type Attribute = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *const c_char;
    type Get = unsafe extern "C" fn(*mut c_void, *const c_char, *mut bool) -> bool;

    struct Api {
        stats: *mut c_void,
        count: Count,
        name_at: NameAt,
        attribute: Attribute,
        get: Get,
    }
    unsafe impl Send for Api {}
    unsafe impl Sync for Api {}

    static API: OnceLock<Option<Api>> = OnceLock::new();
    let api = API.get_or_init(|| unsafe {
        let module = GetModuleHandleA(c"steam_api64.dll".as_ptr() as *const u8);
        if module.is_null() {
            tracing::info!("steam: steam_api64.dll is not loaded; achievements come from event flags");
            return None;
        }
        let proc = |name: &CStr| GetProcAddress(module, name.as_ptr() as *const u8);
        // The ISteamUserStats accessor: versioned in newer SDKs, plain in older ones.
        let accessor = [c"SteamAPI_SteamUserStats_v013", c"SteamAPI_SteamUserStats_v012", c"SteamAPI_SteamUserStats_v011",
                        c"SteamUserStats"]
            .iter()
            .find_map(|n| proc(n).map(|f| (n.to_string_lossy().into_owned(), f)));
        let (Some((accessor_name, accessor)), Some(count), Some(name_at), Some(attribute), Some(get)) = (
            accessor,
            proc(c"SteamAPI_ISteamUserStats_GetNumAchievements"),
            proc(c"SteamAPI_ISteamUserStats_GetAchievementName"),
            proc(c"SteamAPI_ISteamUserStats_GetAchievementDisplayAttribute"),
            proc(c"SteamAPI_ISteamUserStats_GetAchievement"),
        ) else {
            tracing::info!("steam: the ISteamUserStats functions were not found; achievements come from event flags");
            return None;
        };
        let accessor: Accessor = std::mem::transmute(accessor);
        let stats = accessor();
        if stats.is_null() {
            tracing::info!("steam: no ISteamUserStats ({accessor_name})");
            return None;
        }
        tracing::info!("steam: ISteamUserStats from {accessor_name}");
        Some(Api {
            stats,
            count: std::mem::transmute::<_, Count>(count),
            name_at: std::mem::transmute::<_, NameAt>(name_at),
            attribute: std::mem::transmute::<_, Attribute>(attribute),
            get: std::mem::transmute::<_, Get>(get),
        })
    });
    let api = api.as_ref()?;
    // Steam's list: (API name, display name), read once it has entries.
    static LIST: OnceLock<Vec<(CString, String)>> = OnceLock::new();
    let list = match LIST.get() {
        Some(l) => l,
        None => unsafe {
            let n = (api.count)(api.stats);
            if n == 0 {
                return None; // stats not received yet
            }
            let mut out = Vec::new();
            for i in 0..n.min(512) {
                let api_name = (api.name_at)(api.stats, i);
                if api_name.is_null() {
                    continue;
                }
                let api_name = CStr::from_ptr(api_name).to_owned();
                let shown = (api.attribute)(api.stats, api_name.as_ptr(), c"name".as_ptr());
                let shown = if shown.is_null() { String::new() } else { CStr::from_ptr(shown).to_string_lossy().into_owned() };
                out.push((api_name, shown));
            }
            tracing::info!("steam: {} achievements: {}", out.len(),
                           out.iter().map(|(a, s)| format!("{}={s}", a.to_string_lossy())).collect::<Vec<_>>().join(", "));
            LIST.get_or_init(|| out)
        },
    };
    let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    let mut any = false;
    let out = names
        .iter()
        .map(|name| {
            let (api_name, _) = list.iter().find(|(_, shown)| key(shown) == key(name))?;
            let mut unlocked = false;
            let ok = unsafe { (api.get)(api.stats, api_name.as_ptr(), &mut unlocked) };
            any |= ok;
            ok.then_some(unlocked)
        })
        .collect();
    any.then_some(out)
}

#[cfg(not(windows))]
pub fn achievements(_names: &[&str]) -> Option<Vec<Option<bool>>> {
    None
}
