//! The player character as a picture, for the spellbook's Backpack (Wizard101's 3D character window) and the
//! Character page's portrait: Elden Ring's own armour, body, face, hair and weapon models, read from the game's
//! `parts` folder (FLVER meshes and their albedo textures), posed standing with the arms down and the weapon in
//! hand, and drawn by a small software rasterizer on a worker thread into an RGBA image that the book shows as a
//! texture. Nothing is taken from the live character, so riding Torrent or swinging in the world doesn't matter.
//!
//! Files: `parts\<hd|bd|am|lg>_<m|f>_<model>_l.partsbnd.dcx` (the low-detail copies: the same meshes with smaller
//! textures), `fc_<m|f>_0000_l` (the bare body), `fg_a_0100_l` (the face), `hr_a_<hair>_l`, `wp_a_<model>_l`. DCX
//! KRAK is decompressed with the game's own oo2core_6_win64.dll; the preview reads already-decompressed files.
//! Meshes named `#NN#...` are hidden while any worn armour has EquipParamProtector.invisibleFlag_SexVerNN set
//! (er_item_stats.tsv `hide=`), which is how the game hides the body under armour and the hair under a hood.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

/// What to draw.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Look {
    pub male: bool,
    /// Armour parts files without the size suffix, e.g. "bd_m_1520".
    pub armour: Vec<String>,
    /// invisibleFlag bits of the worn armour.
    pub hide: u128,
    /// The right-hand weapon's model (wp_a_<model>), if any.
    pub weapon: Option<u32>,
    pub hair: Option<u32>,
    /// Hair colour (sRGB 0-1).
    pub hair_colour: [f32; 3],
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Framing {
    /// Head to feet.
    Full,
    /// Head and shoulders, for the portrait circle.
    Portrait,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Request {
    pub look: Look,
    pub framing: Framing,
    /// Turn around the vertical axis, radians (0 = facing the viewer).
    pub yaw: f32,
    pub size: [u32; 2],
    /// Drawn without supersampling (while turning).
    pub fast: bool,
}

pub struct Frame {
    pub rgba: Vec<u8>,
    pub size: [u32; 2],
}

#[derive(Default)]
struct Shared {
    want: [Option<Request>; 2],
    done: [Option<Frame>; 2],
    /// What the worker last drew (or is drawing) per slot.
    drawn: [Option<Request>; 2],
    busy: bool,
}

/// The worker thread and its two image slots (0 = Backpack, 1 = portrait).
pub struct Renderer {
    parts: PathBuf,
    shared: Arc<(Mutex<Shared>, Condvar)>,
    started: bool,
}

impl Renderer {
    pub fn new(parts: PathBuf) -> Renderer {
        Renderer { parts, shared: Arc::new((Mutex::new(Shared::default()), Condvar::new())), started: false }
    }

    /// Asks for `request` in `slot`; nothing happens if that is what was last asked for.
    pub fn request(&mut self, slot: usize, request: Request) {
        if !self.started {
            self.started = true;
            let shared = self.shared.clone();
            let parts = self.parts.clone();
            let spawned = std::thread::Builder::new().name("w101 model".into()).spawn(move || worker(parts, shared));
            if let Err(e) = spawned {
                tracing::warn!("model renderer thread: {e}");
            }
        }
        let (lock, wake) = &*self.shared;
        let Ok(mut s) = lock.lock() else { return };
        if s.want[slot].as_ref() != Some(&request) {
            s.want[slot] = Some(request);
            wake.notify_one();
        }
    }

    /// Waits (up to `timeout`) until every request is drawn.
    #[allow(dead_code)]
    pub fn wait_idle(&self, timeout: std::time::Duration) {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if let Ok(s) = self.shared.0.lock() {
                if !s.busy && (0..2).all(|i| s.want[i] == s.drawn[i]) {
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// A newly finished image for `slot`.
    pub fn take(&self, slot: usize) -> Option<Frame> {
        self.shared.0.lock().ok()?.done[slot].take()
    }
}

fn worker(parts: PathBuf, shared: Arc<(Mutex<Shared>, Condvar)>) {
    let mut cache = Cache { folder: parts, files: HashMap::new() };
    let (lock, wake) = &*shared;
    loop {
        let (slot, request) = {
            let Ok(mut s) = lock.lock() else { return };
            loop {
                let next = (0..2).find(|i| s.want[*i].is_some() && s.want[*i] != s.drawn[*i]);
                if let Some(i) = next {
                    let r = s.want[i].clone().expect("checked");
                    s.drawn[i] = Some(r.clone());
                    s.busy = true;
                    break (i, r);
                }
                s = match wake.wait(s) {
                    Ok(s) => s,
                    Err(_) => return,
                };
            }
        };
        let started = std::time::Instant::now();
        let frame = draw(&mut cache, &request);
        if started.elapsed().as_millis() > 200 {
            tracing::info!("model slot {slot}: {:?} {}x{} in {} ms", request.framing, request.size[0], request.size[1],
                           started.elapsed().as_millis());
        }
        if let Ok(mut s) = lock.lock() {
            s.done[slot] = Some(frame);
            s.busy = false;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Files: DCX, BND4, TPF, DDS, FLVER.

#[cfg(windows)]
fn oodle(src: &[u8], size: usize) -> Option<Vec<u8>> {
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
    type Decompress = unsafe extern "system" fn(*const u8, isize, *mut u8, isize, i32, i32, i32, usize, isize, usize,
                                                usize, usize, isize, i32) -> isize;
    static FUNCTION: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
    let f = (*FUNCTION.get_or_init(|| unsafe {
        let module = LoadLibraryA(c"oo2core_6_win64.dll".as_ptr() as *const u8);
        if module.is_null() {
            tracing::warn!("model: oo2core_6_win64.dll not found");
            return None;
        }
        GetProcAddress(module, c"OodleLZ_Decompress".as_ptr() as *const u8).map(|p| p as usize)
    }))?;
    let decompress: Decompress = unsafe { std::mem::transmute(f) };
    let mut out = vec![0u8; size];
    let n = unsafe {
        decompress(src.as_ptr(), src.len() as isize, out.as_mut_ptr(), size as isize, 1, 0, 0, 0, 0, 0, 0, 0, 0, 3)
    };
    (n as usize == size).then_some(out)
}

#[cfg(not(windows))]
fn oodle(_src: &[u8], _size: usize) -> Option<Vec<u8>> {
    None
}

/// The contents of a DCX file (or the bytes themselves if they aren't DCX).
fn dcx(data: Vec<u8>) -> Option<Vec<u8>> {
    if data.get(..4) != Some(b"DCX\0") {
        return Some(data);
    }
    let be = |at: usize| u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
    let (size, packed) = (be(0x1C), be(0x20));
    if &data[0x28..0x2C] != b"KRAK" || data.len() < 0x4C + packed {
        return None;
    }
    oodle(&data[0x4C..0x4C + packed], size)
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    b.get(at..at + 2).map_or(0, |s| u16::from_le_bytes(s.try_into().unwrap()))
}
fn i16_at(b: &[u8], at: usize) -> i16 {
    u16_at(b, at) as i16
}
fn u32_at(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4).map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}
fn i32_at(b: &[u8], at: usize) -> i32 {
    u32_at(b, at) as i32
}
fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(b, at))
}
fn u64_at(b: &[u8], at: usize) -> u64 {
    b.get(at..at + 8).map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
}

fn utf16_at(b: &[u8], at: usize) -> String {
    let mut chars = Vec::new();
    let mut i = at;
    while i + 1 < b.len() {
        let c = u16_at(b, i);
        if c == 0 {
            break;
        }
        chars.push(c);
        i += 2;
    }
    String::from_utf16_lossy(&chars)
}

fn ascii_at(b: &[u8], at: usize) -> String {
    let end = b[at.min(b.len())..].iter().position(|c| *c == 0).map_or(b.len(), |p| at + p);
    String::from_utf8_lossy(&b[at.min(end)..end]).into_owned()
}

/// (name, bytes) of each file in a BND4.
fn bnd4(b: &[u8]) -> Vec<(String, &[u8])> {
    if b.get(..4) != Some(b"BND4") {
        return vec![];
    }
    let count = i32_at(b, 0x0C).max(0) as usize;
    let header = u64_at(b, 0x20) as usize;
    let unicode = b.get(0x30).copied().unwrap_or(0) != 0;
    let mut out = Vec::new();
    for k in 0..count {
        let o = 0x40 + k * header;
        let size = u64_at(b, o + 8) as usize;
        let at = if header >= 0x24 { o + 24 } else { o + 16 };
        let (offset, name_at) = (u32_at(b, at) as usize, u32_at(b, at + 8) as usize);
        let name = if unicode { utf16_at(b, name_at) } else { ascii_at(b, name_at) };
        if let Some(data) = b.get(offset..offset + size) {
            out.push((name, data));
        }
    }
    out
}

/// (name, DDS bytes) of each texture in a TPF.
fn tpf(b: &[u8]) -> Vec<(String, &[u8])> {
    if b.get(..4) != Some(b"TPF\0") {
        return vec![];
    }
    let count = i32_at(b, 8).max(0) as usize;
    (0..count)
        .filter_map(|i| {
            let o = 0x10 + 20 * i;
            let (offset, size) = (u32_at(b, o) as usize, u32_at(b, o + 4) as usize);
            let name = utf16_at(b, u32_at(b, o + 12) as usize);
            Some((name, b.get(offset..offset + size)?))
        })
        .collect()
}

/// A texture, at most MAX_TEXTURE on a side.
pub struct Texture {
    w: usize,
    h: usize,
    /// sRGB colour and alpha.
    px: Vec<[u8; 4]>,
}

const MAX_TEXTURE: usize = 1024;

/// The top mip of a BC1 / BC3 / BC7 DDS, halved until it fits MAX_TEXTURE.
fn dds(b: &[u8]) -> Option<Texture> {
    if b.get(..4) != Some(b"DDS ") {
        return None;
    }
    let (h, w) = (u32_at(b, 12) as usize, u32_at(b, 16) as usize);
    let fourcc = b.get(84..88)?;
    let (format, data_at) = if fourcc == b"DX10" { (u32_at(b, 128), 148) } else { (0, 128) };
    let kind = match (fourcc, format) {
        (b"DXT1", _) | (_, 71 | 72) => 1,
        (b"DXT5", _) | (_, 77 | 78) => 3,
        (_, 98 | 99) => 7,
        _ => return None,
    };
    let blocks = w.div_ceil(4) * h.div_ceil(4) * if kind == 1 { 8 } else { 16 };
    let data = b.get(data_at..data_at + blocks)?;
    let mut out = vec![0u32; w * h];
    let ok = match kind {
        1 => texture2ddecoder::decode_bc1(data, w, h, &mut out),
        3 => texture2ddecoder::decode_bc3(data, w, h, &mut out),
        _ => texture2ddecoder::decode_bc7(data, w, h, &mut out),
    };
    ok.ok()?;
    // 0xAARRGGBB
    let mut px: Vec<[u8; 4]> = out
        .iter()
        .map(|c| {
            let [b, g, r, a] = c.to_le_bytes();
            [r, g, b, a]
        })
        .collect();
    let (mut w, mut h) = (w, h);
    while (w > MAX_TEXTURE || h > MAX_TEXTURE) && w >= 2 && h >= 2 {
        let (nw, nh) = (w / 2, h / 2);
        let mut next = vec![[0u8; 4]; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let mut s = [0u32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = px[(2 * y + dy) * w + 2 * x + dx];
                    for i in 0..4 {
                        s[i] += p[i] as u32;
                    }
                }
                next[y * nw + x] = s.map(|v| (v / 4) as u8);
            }
        }
        (px, w, h) = (next, nw, nh);
    }
    Some(Texture { w, h, px })
}

impl Texture {
    /// Bilinear, wrapping; sRGB 0-1.
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = (u - u.floor()) * self.w as f32 - 0.5;
        let y = (v - v.floor()) * self.h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let at = |xi: f32, yi: f32| {
            let xi = (xi as i64).rem_euclid(self.w as i64) as usize;
            let yi = (yi as i64).rem_euclid(self.h as i64) as usize;
            self.px[yi * self.w + xi]
        };
        let (a, b, c, d) = (at(x0, y0), at(x0 + 1.0, y0), at(x0, y0 + 1.0), at(x0 + 1.0, y0 + 1.0));
        [0, 1, 2, 3].map(|i| {
            ((a[i] as f32 * (1.0 - fx) + b[i] as f32 * fx) * (1.0 - fy) + (c[i] as f32 * (1.0 - fx) + d[i] as f32 * fx) * fy)
                / 255.0
        })
    }
}

struct Bone {
    name: String,
    parent: i32,
    translation: [f32; 3],
    rotation: [f32; 3],
    scale: [f32; 3],
}

struct Mesh {
    /// The `#NN#` group of its name.
    group: Option<u8>,
    name: String,
    material: String,
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    /// Bone indices into the FLVER's bones and their weights.
    bones: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
    tris: Vec<[u32; 3]>,
}

struct Flver {
    bones: Vec<Bone>,
    meshes: Vec<Mesh>,
}

fn flver(b: &[u8]) -> Option<Flver> {
    if b.get(..6) != Some(b"FLVER\0") {
        return None;
    }
    let data_at = u32_at(b, 0x0C) as usize;
    let (dummies, materials, bone_count, mesh_count, buffer_count) =
        (i32_at(b, 0x14) as usize, i32_at(b, 0x18) as usize, i32_at(b, 0x1C) as usize, i32_at(b, 0x20) as usize,
         i32_at(b, 0x24) as usize);
    let unicode = b.get(0x49).copied().unwrap_or(1) != 0;
    let header_index = b.get(0x48).copied().unwrap_or(16) as u32;
    let (face_sets, layout_count) = (i32_at(b, 0x50) as usize, i32_at(b, 0x54) as usize);
    let string = |at: usize| if unicode { utf16_at(b, at) } else { ascii_at(b, at) };
    let mut at = 0x80 + dummies * 0x40;
    let material_names: Vec<(String, String)> = (0..materials)
        .map(|i| (string(u32_at(b, at + i * 0x20) as usize), string(u32_at(b, at + i * 0x20 + 4) as usize)))
        .collect();
    at += materials * 0x20;
    let f3 = |o: usize| [f32_at(b, o), f32_at(b, o + 4), f32_at(b, o + 8)];
    let bones: Vec<Bone> = (0..bone_count)
        .map(|i| {
            let o = at + i * 0x80;
            Bone {
                translation: f3(o),
                name: string(u32_at(b, o + 0x0C) as usize),
                rotation: f3(o + 0x10),
                parent: i16_at(b, o + 0x1C) as i32,
                scale: f3(o + 0x20),
            }
        })
        .collect();
    at += bone_count * 0x80;
    let mesh_at = at;
    at += mesh_count * 0x30;
    let face_set_at = at;
    at += face_sets * 0x20;
    let buffer_at = at;
    at += buffer_count * 0x20;
    // Layouts: [(type, semantic)]
    let layouts: Vec<Vec<(u32, u32)>> = (0..layout_count)
        .map(|i| {
            let o = at + i * 0x10;
            let (n, members) = (i32_at(b, o) as usize, u32_at(b, o + 12) as usize);
            (0..n.min(64)).map(|k| (u32_at(b, members + k * 0x14 + 8), u32_at(b, members + k * 0x14 + 12))).collect()
        })
        .collect();
    let mut meshes = Vec::new();
    for m in 0..mesh_count {
        let o = mesh_at + m * 0x30;
        let material = i32_at(b, o + 4).max(0) as usize;
        let bone_list_count = i32_at(b, o + 0x14) as usize;
        let bone_list_at = u32_at(b, o + 0x1C) as usize;
        let local_bones: Vec<u16> = (0..bone_list_count.min(1024)).map(|k| i32_at(b, bone_list_at + 4 * k) as u16).collect();
        let (fs_count, fs_list) = (i32_at(b, o + 0x20) as usize, u32_at(b, o + 0x24) as usize);
        let (vb_count, vb_list) = (i32_at(b, o + 0x28) as usize, u32_at(b, o + 0x2C) as usize);
        let (name, mtd) = material_names.get(material).cloned().unwrap_or_default();
        let mut mesh = Mesh {
            group: name.strip_prefix('#').and_then(|r| r.split('#').next()).and_then(|n| n.parse().ok()),
            name,
            material: mtd,
            pos: vec![],
            nrm: vec![],
            uv: vec![],
            bones: vec![],
            weights: vec![],
            tris: vec![],
        };
        for k in 0..vb_count.min(8) {
            let vb = buffer_at + i32_at(b, vb_list + 4 * k) as usize * 0x20;
            let layout = i32_at(b, vb + 4) as usize;
            let (stride, count, offset) = (u32_at(b, vb + 8) as usize, u32_at(b, vb + 12) as usize,
                                           u32_at(b, vb + 0x1C) as usize);
            let Some(members) = layouts.get(layout) else { continue };
            let base = data_at + offset;
            if b.len() < base + stride * count {
                continue;
            }
            let mut field = 0usize;
            for &(kind, semantic) in members {
                let size = match kind {
                    0x00 | 0x10 | 0x11 | 0x12 | 0x13 | 0x15 | 0x2F | 0xF0 => 4,
                    0x01 | 0x16 | 0x18 | 0x1A | 0x2E => 8,
                    0x02 => 12,
                    0x03 => 16,
                    _ => 4,
                };
                let v = |i: usize| base + i * stride + field;
                match (semantic, kind) {
                    (0, 0x02 | 0x03) if mesh.pos.is_empty() => mesh.pos = (0..count).map(|i| f3(v(i))).collect(),
                    (3, _) if mesh.nrm.is_empty() => {
                        mesh.nrm = (0..count)
                            .map(|i| match kind {
                                0x02 | 0x03 => f3(v(i)),
                                0x1A => [0, 2, 4].map(|c| i16_at(b, v(i) + c) as f32 / 32767.0),
                                _ => [0, 1, 2].map(|c| (b[v(i) + c] as f32 - 127.0) / 127.0),
                            })
                            .collect()
                    }
                    (5, 0x15 | 0x16) if mesh.uv.is_empty() => {
                        mesh.uv = (0..count).map(|i| [0, 2].map(|c| i16_at(b, v(i) + c) as f32 / 2048.0)).collect()
                    }
                    (5, 0x01 | 0x03) if mesh.uv.is_empty() => {
                        mesh.uv = (0..count).map(|i| [f32_at(b, v(i)), f32_at(b, v(i) + 4)]).collect()
                    }
                    (2, _) if mesh.bones.is_empty() => {
                        mesh.bones = (0..count)
                            .map(|i| {
                                let raw = match kind {
                                    0x18 => [0, 2, 4, 6].map(|c| u16_at(b, v(i) + c)),
                                    _ => [0, 1, 2, 3].map(|c| b[v(i) + c] as u16),
                                };
                                if local_bones.is_empty() {
                                    raw
                                } else {
                                    raw.map(|r| local_bones.get(r as usize).copied().unwrap_or(0))
                                }
                            })
                            .collect()
                    }
                    (1, 0x1A) if mesh.weights.is_empty() => {
                        mesh.weights =
                            (0..count).map(|i| [0, 2, 4, 6].map(|c| i16_at(b, v(i) + c) as f32 / 32767.0)).collect()
                    }
                    (1, 0x10 | 0x13) if mesh.weights.is_empty() => {
                        mesh.weights = (0..count).map(|i| [0, 1, 2, 3].map(|c| b[v(i) + c] as f32 / 255.0)).collect()
                    }
                    _ => {}
                }
                field += size;
            }
        }
        // The main face set (no LOD / motion-blur flags).
        for k in 0..fs_count.min(16) {
            let fs = face_set_at + i32_at(b, fs_list + 4 * k) as usize * 0x20;
            if u32_at(b, fs) != 0 {
                continue;
            }
            let strip = b.get(fs + 4).copied().unwrap_or(0) != 0;
            let (count, offset) = (i32_at(b, fs + 0x08).max(0) as usize, u32_at(b, fs + 0x0C) as usize);
            let size = match u32_at(b, fs + 0x18) {
                0 => header_index,
                s => s,
            };
            let at = data_at + offset;
            if count > 4_000_000 || b.len() < at + count * (size as usize / 8) {
                break;
            }
            let idx: Vec<u32> = if size == 16 {
                (0..count).map(|i| u16_at(b, at + 2 * i) as u32).collect()
            } else {
                (0..count).map(|i| u32_at(b, at + 4 * i)).collect()
            };
            if strip {
                for i in 0..idx.len().saturating_sub(2) {
                    let (a, bb, c) = (idx[i], idx[i + 1], idx[i + 2]);
                    if a == bb || bb == c || a == c || [a, bb, c].contains(&0xFFFF) {
                        continue;
                    }
                    mesh.tris.push(if i % 2 == 0 { [a, bb, c] } else { [bb, a, c] });
                }
            } else {
                mesh.tris = idx.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
            }
            break;
        }
        let n = mesh.pos.len() as u32;
        mesh.tris.retain(|t| t.iter().all(|i| *i < n));
        if mesh.nrm.len() != mesh.pos.len() {
            mesh.nrm = vec![[0.0, 0.0, -1.0]; mesh.pos.len()];
        }
        if mesh.uv.len() != mesh.pos.len() {
            mesh.uv = vec![[0.0, 0.0]; mesh.pos.len()];
        }
        meshes.push(mesh);
    }
    Some(Flver { bones, meshes })
}

// ---------------------------------------------------------------------------------------------------------------
// Parts.

/// A loaded parts file: its FLVERs (the first is the main one) and textures.
struct Part {
    flvers: Vec<Flver>,
    textures: HashMap<String, Arc<Texture>>,
}

struct Cache {
    folder: PathBuf,
    files: HashMap<String, Option<Arc<Part>>>,
}

impl Cache {
    /// `stem` like "bd_m_1520" (the "_l" copy is read).
    fn part(&mut self, stem: &str) -> Option<Arc<Part>> {
        if let Some(p) = self.files.get(stem) {
            return p.clone();
        }
        if self.files.len() >= 32 {
            // Each part holds its textures; start over rather than grow without end as equipment changes.
            self.files.clear();
        }
        let path = self.folder.join(format!("{stem}_l.partsbnd.dcx"));
        let loaded = std::fs::read(&path).ok().and_then(dcx).map(|raw| {
            let mut part = Part { flvers: vec![], textures: HashMap::new() };
            for (name, data) in bnd4(&raw) {
                let lower = name.to_ascii_lowercase();
                if lower.ends_with(".flver") {
                    part.flvers.extend(flver(data));
                } else if lower.ends_with(".tpf") {
                    for (tex, dds_bytes) in tpf(data) {
                        // Only albedo is used.
                        let t = tex.to_ascii_lowercase();
                        if t.ends_with("_a") || t.ends_with("_a_l") {
                            if let Some(t) = dds(dds_bytes) {
                                part.textures.insert(tex.to_ascii_lowercase(), Arc::new(t));
                            }
                        }
                    }
                }
            }
            Arc::new(part)
        });
        if loaded.is_none() {
            tracing::info!("model: {} not loaded", path.display());
        }
        self.files.insert(stem.to_string(), loaded.clone());
        loaded
    }

    fn exists(&self, stem: &str) -> bool {
        self.files.get(stem).is_some_and(|p| p.is_some()) || self.folder.join(format!("{stem}_l.partsbnd.dcx")).exists()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Posing and drawing.

type M3 = [[f32; 3]; 3];

fn mul(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

/// Row vector times matrix.
fn apply(v: [f32; 3], m: &M3) -> [f32; 3] {
    [0, 1, 2].map(|j| v[0] * m[0][j] + v[1] * m[1][j] + v[2] * m[2][j])
}

fn rot_x(a: f32) -> M3 {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]]
}
fn rot_y(a: f32) -> M3 {
    let (s, c) = a.sin_cos();
    [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]]
}
fn rot_z(a: f32) -> M3 {
    let (s, c) = a.sin_cos();
    [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 1e-8 { scale(a, 1.0 / l) } else { a }
}

/// Each bone's bind-pose world rotation and position (FLVER bones: scale, then rotation X, Z, Y, then
/// translation, then the parent's).
fn bind_pose(bones: &[Bone]) -> Vec<(M3, [f32; 3])> {
    let mut out: Vec<Option<(M3, [f32; 3])>> = vec![None; bones.len()];
    fn get(i: usize, bones: &[Bone], out: &mut Vec<Option<(M3, [f32; 3])>>, depth: usize) -> (M3, [f32; 3]) {
        if let Some(w) = out[i] {
            return w;
        }
        let b = &bones[i];
        let s = [[b.scale[0], 0.0, 0.0], [0.0, b.scale[1], 0.0], [0.0, 0.0, b.scale[2]]];
        let r = mul(&mul(&mul(&s, &rot_x(b.rotation[0])), &rot_z(b.rotation[2])), &rot_y(b.rotation[1]));
        let w = if b.parent >= 0 && (b.parent as usize) < bones.len() && depth < 64 {
            let (pr, pt) = get(b.parent as usize, bones, out, depth + 1);
            (mul(&r, &pr), add(apply(b.translation, &pr), pt))
        } else {
            (r, b.translation)
        };
        out[i] = Some(w);
        w
    }
    (0..bones.len()).map(|i| get(i, bones, &mut out, 0)).collect()
}

/// How much each bone follows the left and right arm (1 under L_/R_UpperArm, half for shoulder armour bones).
fn arm_share(bones: &[Bone]) -> Vec<[f32; 2]> {
    (0..bones.len())
        .map(|i| {
            let mut j = i as i32;
            for _ in 0..64 {
                if j < 0 || j as usize >= bones.len() {
                    break;
                }
                match bones[j as usize].name.as_str() {
                    "L_UpperArm" => return [1.0, 0.0],
                    "R_UpperArm" => return [0.0, 1.0],
                    _ => {}
                }
                j = bones[j as usize].parent;
            }
            let n = bones[i].name.as_str();
            if n.starts_with("L_Shoulder") {
                [0.5, 0.0]
            } else if n.starts_with("R_Shoulder") {
                [0.0, 0.5]
            } else {
                [0.0, 0.0]
            }
        })
        .collect()
}

/// How far the arms come down from the bind pose (which holds them 45 degrees out), radians.
const ARM_DOWN: f32 = 0.58;

struct Pose {
    /// Shoulder pivots and rotations of the left and right arm.
    pivot: [[f32; 3]; 2],
    rot: [M3; 2],
    /// The right hand's grip point after posing.
    grip: [f32; 3],
}

impl Pose {
    fn new(body: Option<&Flver>) -> Pose {
        let mut pivot = [[0.172, 1.391, 0.034], [-0.172, 1.391, 0.034]];
        let mut grip = [-0.598, 0.965, 0.016];
        if let Some(f) = body {
            let world = bind_pose(&f.bones);
            let find = |n: &str| f.bones.iter().position(|b| b.name == n);
            if let (Some(l), Some(r)) = (find("L_UpperArm"), find("R_UpperArm")) {
                pivot = [world[l].1, world[r].1];
            }
            if let Some(h) = find("R_Hand") {
                // R_Weapon sits 9 cm along the hand.
                grip = add(world[h].1, scale(world[h].0[0], 0.09));
            }
        }
        // Left arm (+x) turns clockwise seen from the front, the right arm the other way.
        let rot = [rot_z(-ARM_DOWN), rot_z(ARM_DOWN)];
        let grip = add(apply(sub(grip, pivot[1]), &rot[1]), pivot[1]);
        Pose { pivot, rot, grip }
    }

    fn vertex(&self, p: [f32; 3], n: [f32; 3], share: [f32; 2]) -> ([f32; 3], [f32; 3]) {
        let keep = (1.0 - share[0] - share[1]).max(0.0);
        let mut op = scale(p, keep);
        let mut on = scale(n, keep);
        for s in 0..2 {
            if share[s] > 0.0 {
                op = add(op, scale(add(apply(sub(p, self.pivot[s]), &self.rot[s]), self.pivot[s]), share[s]));
                on = add(on, scale(apply(n, &self.rot[s]), share[s]));
            }
        }
        (op, on)
    }
}

/// A triangle soup ready to draw: positions, normals, uvs, and per mesh its texture or colour.
struct Batch {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    tris: Vec<[u32; 3]>,
    texture: Option<Arc<Texture>>,
    colour: [f32; 3],
    alpha_test: bool,
}

/// The albedo for a mesh. Elden Ring names the textures in its material files (not read here), so: the textures
/// of the material's "P[NAME]" (else the part's own) whose name contains a word of the material's name
/// ("P[LG_M_1010]_Fabric" -> "LG_M_1010_Fabric_a_l"), else the biggest of them (the part's main atlas).
fn texture_for(material: &str, own: &str, textures: &HashMap<String, Arc<Texture>>) -> Option<Arc<Texture>> {
    let material = material.to_ascii_lowercase();
    let (key, rest) = match material.split_once("p[").and_then(|(_, r)| r.split_once(']')) {
        Some((k, r)) => (k.to_string(), r.trim_end_matches(".matxml").trim_start_matches('_').to_string()),
        None => (own.to_string(), material.clone()),
    };
    for k in [key.as_str(), own] {
        let candidates: Vec<(&String, &Arc<Texture>)> =
            textures.iter().filter(|(n, _)| n.starts_with(k) && (n.ends_with("_a") || n.ends_with("_a_l"))).collect();
        if candidates.is_empty() {
            continue;
        }
        let named = candidates.iter().find(|(n, _)| {
            let middle = n[k.len()..].trim_end_matches("_l").trim_end_matches("_a").trim_matches('_');
            !middle.is_empty() && (rest.contains(middle) || middle.contains(rest.as_str()) && !rest.is_empty())
        });
        if let Some((_, t)) = named {
            return Some((*t).clone());
        }
        return candidates.iter().max_by_key(|(n, t)| (t.w * t.h, std::cmp::Reverse(n.len()))).map(|(_, t)| (*t).clone());
    }
    None
}

fn batches(cache: &mut Cache, look: &Look) -> Vec<Batch> {
    let g = if look.male { "m" } else { "f" };
    let gendered = |cache: &Cache, stem: &str| -> String {
        // Female characters use the _f model where there is one.
        if !look.male {
            let f = stem.replacen("_m_", "_f_", 1);
            if cache.exists(&f) {
                return f;
            }
        }
        stem.to_string()
    };
    let mut stems: Vec<(String, bool)> = vec![(format!("fc_{g}_0000"), false), ("fg_a_0100".into(), false)];
    if let Some(h) = look.hair {
        stems.push((format!("hr_a_{h:04}"), false));
    }
    for a in &look.armour {
        let s = gendered(cache, a);
        stems.push((s, false));
    }
    if let Some(w) = look.weapon {
        stems.push((format!("wp_a_{w:04}"), true));
    }
    let parts: Vec<(String, Arc<Part>, bool)> =
        stems.into_iter().filter_map(|(s, weapon)| cache.part(&s).map(|p| (s, p, weapon))).collect();
    let mut textures: HashMap<String, Arc<Texture>> = HashMap::new();
    for (_, p, _) in &parts {
        for (k, t) in &p.textures {
            textures.entry(k.clone()).or_insert_with(|| t.clone());
        }
    }
    let body = parts.first().filter(|(s, _, _)| s.starts_with("fc_")).and_then(|(_, p, _)| p.flvers.first());
    let pose = Pose::new(body);
    // The armour files' skeletons are partial (the gauntlets' forearm bones have no parents), so which bones follow
    // the arms comes from the body's full skeleton, by name.
    let by_name: HashMap<&str, [f32; 2]> = body
        .map(|b| b.bones.iter().map(|x| x.name.as_str()).zip(arm_share(&b.bones)).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    for (stem, part, weapon) in &parts {
        let is_face = stem.starts_with("fg_");
        let is_hair = stem.starts_with("hr_");
        // A weapon's main model only (the others are sheaths and such).
        let flvers = if *weapon { &part.flvers[..part.flvers.len().min(1)] } else { &part.flvers[..] };
        for f in flvers {
            let share: Vec<[f32; 2]> = arm_share(&f.bones)
                .into_iter()
                .zip(&f.bones)
                .map(|(own, b)| by_name.get(b.name.as_str()).copied().unwrap_or(own))
                .collect();
            for m in &f.meshes {
                if m.tris.is_empty() || m.group.is_some_and(|g| g < 128 && look.hide & (1u128 << g) != 0) {
                    continue;
                }
                let mat = m.material.to_ascii_lowercase();
                // The face file carries a copy of the body; only its own face meshes are used. Shadow / AO cards
                // and alpha-blended hairlines are left out.
                if (is_face && !mat.contains("fg_a")) || mat.contains("eyeao") || mat.contains("_alpha")
                    || mat.contains("shadow") || m.name.to_ascii_lowercase().contains("hairy_plane")
                {
                    continue;
                }
                let mut pos = Vec::with_capacity(m.pos.len());
                let mut nrm = Vec::with_capacity(m.pos.len());
                for i in 0..m.pos.len() {
                    let (p, n) = if *weapon {
                        weapon_vertex(m.pos[i], m.nrm[i], pose.grip)
                    } else {
                        let mut s = [0.0f32; 2];
                        if let (Some(b), Some(w)) = (m.bones.get(i), m.weights.get(i)) {
                            let total: f32 = w.iter().sum();
                            for k in 0..4 {
                                if let Some(bs) = share.get(b[k] as usize) {
                                    let wk = if total > 0.0 { w[k] / total } else if k == 0 { 1.0 } else { 0.0 };
                                    s[0] += bs[0] * wk;
                                    s[1] += bs[1] * wk;
                                }
                            }
                        }
                        pose.vertex(m.pos[i], m.nrm[i], s)
                    };
                    pos.push(p);
                    nrm.push(n);
                }
                let own = stem.to_ascii_lowercase();
                let texture = if is_hair { None } else { texture_for(&m.material, &own, &textures) };
                out.push(Batch {
                    pos,
                    nrm,
                    uv: m.uv.clone(),
                    tris: m.tris.clone(),
                    texture,
                    colour: if is_hair { look.hair_colour } else { [0.55, 0.52, 0.5] },
                    alpha_test: true,
                });
            }
        }
    }
    out
}

/// A weapon vertex: the model hangs from the grip (its origin), blade (-Y) down and a little forward and out.
fn weapon_vertex(p: [f32; 3], n: [f32; 3], grip: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let down = norm([-0.12, -1.0, -0.3]);
    let y = scale(down, -1.0);
    let x = norm(cross(y, [0.0, 0.0, 1.0]));
    let z = cross(x, y);
    let m: M3 = [x, y, z];
    (add(apply(p, &m), grip), apply(n, &m))
}

fn srgb_to_linear(c: f32) -> f32 {
    c.powf(2.2)
}
fn linear_to_srgb(c: f32) -> f32 {
    c.max(0.0).powf(1.0 / 2.2)
}

/// Draws the request: 2x supersampled, then averaged down; transparent background.
fn draw(cache: &mut Cache, r: &Request) -> Frame {
    let [w, h] = [r.size[0].max(8) as usize, r.size[1].max(8) as usize];
    let ss = if r.fast { 1 } else { 2 };
    let (sw, sh) = (w * ss, h * ss);
    let mut look = r.look.clone();
    if r.framing == Framing::Portrait {
        // The portrait shows the face: no helm.
        look.armour.retain(|a| !a.starts_with("hd_"));
        look.hide = 0;
        look.weapon = None;
    }
    let batches = batches(cache, &look);
    // View: turn by yaw around the vertical axis through the body; camera looks along +Z (the character faces
    // -Z), x to the right, y up.
    let (centre, height) = match r.framing {
        Framing::Full => ([0.0, 0.9], 1.92),
        Framing::Portrait => ([0.0, 1.55], 0.5),
    };
    let k = sh as f32 / height;
    let turn = rot_y(r.yaw);
    let to_screen = |p: [f32; 3]| -> [f32; 3] {
        let q = apply(p, &turn);
        [sw as f32 / 2.0 + (q[0] - centre[0]) * k, sh as f32 / 2.0 - (q[1] - centre[1]) * k, q[2]]
    };
    let light = norm([-0.45, 0.65, -0.6]);
    let fill = norm([0.6, 0.2, -0.4]);
    let mut colour = vec![[0.0f32; 4]; sw * sh];
    let mut depth = vec![f32::INFINITY; sw * sh];
    if r.framing == Framing::Full {
        // A soft shadow on the ground.
        let (cx, cy) = (sw as f32 / 2.0, to_screen([0.0, 0.0, 0.0])[1]);
        let (rx, ry) = (0.42 * k, 0.07 * k);
        for y in (cy - ry).max(0.0) as usize..((cy + ry) as usize).min(sh) {
            for x in (cx - rx).max(0.0) as usize..((cx + rx) as usize).min(sw) {
                let d = ((x as f32 - cx) / rx).powi(2) + ((y as f32 - cy) / ry).powi(2);
                if d < 1.0 {
                    colour[y * sw + x] = [0.0, 0.0, 0.0, 0.45 * (1.0 - d)];
                }
            }
        }
    }
    for b in &batches {
        let screen: Vec<[f32; 3]> = b.pos.iter().map(|p| to_screen(*p)).collect();
        let normals: Vec<[f32; 3]> = b.nrm.iter().map(|n| norm(apply(*n, &turn))).collect();
        for t in &b.tris {
            let [a, bb, c] = t.map(|i| i as usize);
            let (p0, p1, p2) = (screen[a], screen[bb], screen[c]);
            let area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
            if area.abs() < 1e-6 {
                continue;
            }
            let minx = p0[0].min(p1[0]).min(p2[0]).floor().max(0.0) as usize;
            let maxx = (p0[0].max(p1[0]).max(p2[0]).ceil() as i64).min(sw as i64 - 1);
            let miny = p0[1].min(p1[1]).min(p2[1]).floor().max(0.0) as usize;
            let maxy = (p0[1].max(p1[1]).max(p2[1]).ceil() as i64).min(sh as i64 - 1);
            if maxx < 0 || maxy < 0 {
                continue;
            }
            for y in miny..=maxy as usize {
                let py = y as f32 + 0.5;
                for x in minx..=maxx as usize {
                    let px = x as f32 + 0.5;
                    let w0 = ((p1[0] - px) * (p2[1] - py) - (p2[0] - px) * (p1[1] - py)) / area;
                    let w1 = ((p2[0] - px) * (p0[1] - py) - (p0[0] - px) * (p2[1] - py)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * p0[2] + w1 * p1[2] + w2 * p2[2];
                    let at = y * sw + x;
                    if z >= depth[at] {
                        continue;
                    }
                    let mut albedo = [b.colour[0], b.colour[1], b.colour[2], 1.0];
                    if let Some(tex) = &b.texture {
                        let u = w0 * b.uv[a][0] + w1 * b.uv[bb][0] + w2 * b.uv[c][0];
                        let v = w0 * b.uv[a][1] + w1 * b.uv[bb][1] + w2 * b.uv[c][1];
                        albedo = tex.sample(u, v);
                    }
                    if b.alpha_test && albedo[3] < 0.5 {
                        continue;
                    }
                    depth[at] = z;
                    let mut n = norm(add(add(scale(normals[a], w0), scale(normals[bb], w1)), scale(normals[c], w2)));
                    if n[2] > 0.0 {
                        n = scale(n, -1.0); // the back of a single-sided cloth faces the camera
                    }
                    let lit = 0.5 + 0.85 * dot(n, light).max(0.0) + 0.3 * dot(n, fill).max(0.0);
                    colour[at] = [
                        linear_to_srgb(srgb_to_linear(albedo[0]) * lit),
                        linear_to_srgb(srgb_to_linear(albedo[1]) * lit),
                        linear_to_srgb(srgb_to_linear(albedo[2]) * lit),
                        1.0,
                    ];
                }
            }
        }
    }
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let mut s = [0.0f32; 4];
            for dy in 0..ss {
                for dx in 0..ss {
                    let c = colour[(y * ss + dy) * sw + x * ss + dx];
                    // Premultiplied while averaging.
                    s[0] += c[0] * c[3];
                    s[1] += c[1] * c[3];
                    s[2] += c[2] * c[3];
                    s[3] += c[3];
                }
            }
            let n = (ss * ss) as f32;
            let a = s[3] / n;
            let o = (y * w + x) * 4;
            if a > 0.0 {
                for i in 0..3 {
                    rgba[o + i] = (s[i] / s[3] * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
            let mut a = a;
            if r.framing == Framing::Portrait {
                // Round, for the portrait circle (soft edge).
                let (dx, dy) = (x as f32 + 0.5 - w as f32 / 2.0, y as f32 + 0.5 - h as f32 / 2.0);
                let radius = w.min(h) as f32 * 0.42;
                a *= (radius - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            }
            rgba[o + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    Frame { rgba, size: [w as u32, h as u32] }
}
