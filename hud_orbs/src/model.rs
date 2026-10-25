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
    /// (centre height, view height) in metres, instead of the framing's own (the backpack's zoom).
    pub view: Option<[f32; 2]>,
}

impl Framing {
    /// (centre height, view height) in metres.
    pub fn view(self, helm: bool) -> [f32; 2] {
        match self {
            Framing::Full => [0.9, 1.92],
            // A hat reaches well above the head.
            Framing::Portrait if helm => [1.66, 0.62],
            Framing::Portrait => [1.56, 0.44],
        }
    }
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
    let mut cache = Cache { folder: parts, files: HashMap::new(), posed: None };
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
    /// Halved copies, down to 8 texels (cache-friendly sampling of far-away or small triangles).
    mips: Vec<Texture>,
}

fn halve(px: &[[u8; 4]], w: usize, h: usize) -> (Vec<[u8; 4]>, usize, usize) {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let mut next = vec![[0u8; 4]; nw * nh];
    for y in 0..nh {
        for x in 0..nw {
            let mut s = [0u32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let p = px[(2 * y + dy).min(h - 1) * w + (2 * x + dx).min(w - 1)];
                for i in 0..4 {
                    s[i] += p[i] as u32;
                }
            }
            next[y * nw + x] = s.map(|v| (v / 4) as u8);
        }
    }
    (next, nw, nh)
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
        (px, w, h) = halve(&px, w, h);
    }
    Some(with_mips(px, w, h))
}

fn with_mips(px: Vec<[u8; 4]>, w: usize, h: usize) -> Texture {
    let mut mips = Vec::new();
    let (mut level, mut lw, mut lh) = (px.clone(), w, h);
    while lw >= 16 && lh >= 16 {
        (level, lw, lh) = halve(&level, lw, lh);
        mips.push(Texture { w: lw, h: lh, px: level.clone(), mips: vec![] });
    }
    Texture { w, h, px, mips }
}

/// ER's eye albedo is only the white; the shader paints the iris and pupil from the `_3m` mask (green = iris and
/// pupil, blue = not pupil, red = iris fibres). Baked here with a hazel iris.
fn eye(white: &Texture, mask: &Texture) -> Texture {
    const IRIS: [f32; 3] = [92.0, 64.0, 38.0];
    let px = (0..white.w * white.h)
        .map(|i| {
            let (x, y) = (i % white.w, i / white.w);
            let m = mask.px[(y * mask.h / white.h).min(mask.h - 1) * mask.w + (x * mask.w / white.w).min(mask.w - 1)];
            let [g, b, r] = [m[1], m[2], m[0]].map(|v| v as f32 / 255.0);
            let (iris, pupil) = (g * b, g * (1.0 - b));
            let w = white.px[i];
            let c: [u8; 3] = std::array::from_fn(|k| {
                let iris_c = IRIS[k] * (0.75 + 0.6 * r);
                let v = w[k] as f32 * (1.0 - iris - pupil) + iris_c * iris + 8.0 * pupil;
                v.clamp(0.0, 255.0) as u8
            });
            [c[0], c[1], c[2], 255]
        })
        .collect();
    with_mips(px, white.w, white.h)
}

impl Texture {
    /// The level for `lod` (0 = full size).
    fn level(&self, lod: usize) -> &Texture {
        if lod == 0 { self } else { self.mips.get(lod - 1).unwrap_or_else(|| self.mips.last().unwrap_or(self)) }
    }

    #[inline]
    fn texel(&self, xi: i64, yi: i64) -> [u8; 4] {
        let xi = if self.w.is_power_of_two() { (xi & (self.w as i64 - 1)) as usize } else { xi.rem_euclid(self.w as i64) as usize };
        let yi = if self.h.is_power_of_two() { (yi & (self.h as i64 - 1)) as usize } else { yi.rem_euclid(self.h as i64) as usize };
        self.px[yi * self.w + xi]
    }

    /// Bilinear, wrapping; sRGB 0-255.
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = u * self.w as f32 - 0.5;
        let y = v * self.h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (xi, yi) = (x0 as i64, y0 as i64);
        let (a, b, c, d) = (self.texel(xi, yi), self.texel(xi + 1, yi), self.texel(xi, yi + 1), self.texel(xi + 1, yi + 1));
        [0, 1, 2, 3].map(|i| {
            (a[i] as f32 * (1.0 - fx) + b[i] as f32 * fx) * (1.0 - fy) + (c[i] as f32 * (1.0 - fx) + d[i] as f32 * fx) * fy
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
    /// The posed triangles of the last look drawn: turning redraws the same look.
    posed: Option<(Look, Arc<Vec<Batch>>)>,
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
            let mut masks: HashMap<String, Texture> = HashMap::new();
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
                        } else if t.contains("eye_3m") {
                            if let Some(m) = dds(dds_bytes) {
                                masks.insert(t.replace("eye_3m", "eye_a"), m);
                            }
                        }
                    }
                }
            }
            for (name, mask) in masks {
                if let Some(white) = part.textures.get(&name).cloned() {
                    part.textures.insert(name, Arc::new(eye(&white, &mask)));
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

/// A rotation about an axis (row-vector convention, as `apply`): turns vectors from `cross(axis, ...)` sense,
/// i.e. a positive angle turns `d` toward `n` for the axis `cross(d, n)`.
fn rot_axis(axis: [f32; 3], a: f32) -> M3 {
    let [x, y, z] = norm(axis);
    let (s, c) = a.sin_cos();
    let k = 1.0 - c;
    // The column-vector Rodrigues matrix, transposed for row vectors.
    [
        [c + x * x * k, y * x * k + z * s, z * x * k - y * s],
        [x * y * k - z * s, c + y * y * k, z * y * k + x * s],
        [x * z * k + y * s, y * z * k - x * s, c + z * z * k],
    ]
}

/// A standing pose for Elden Ring's player skeleton, as joint rotations in the bind pose's space (the bind pose is
/// an A-pose: arms 45 degrees down and out, palms down). Each rotation turns its bone and everything under it about
/// the bone's own joint. Arms hang at the sides, a little forward; the elbows bend, the right one more, so the right
/// hand holds the weapon in front of the thigh; the right fingers close around the grip, the left ones relax; the
/// feet stand a little apart.
fn pose_rotation(name: &str, world: &(M3, [f32; 3])) -> Option<M3> {
    let side = |n: &str| if n.starts_with("L_") { 1.0 } else { -1.0 };
    let along = world.0[0];
    match name {
        "L_UpperArm" | "R_UpperArm" => Some(mul(&rot_z(-0.62 * side(name)), &rot_x(0.10))),
        // The deltoid helpers follow the upper arm halfway.
        "L_Shoulder" | "R_Shoulder" => Some(mul(&rot_z(-0.31 * side(name)), &rot_x(0.05))),
        "L_Forearm" => Some(rot_axis(cross(along, [0.0, 0.0, -1.0]), 0.35)),
        "R_Forearm" => Some(rot_axis(cross(along, [0.0, 0.0, -1.0]), 0.6)),
        "L_Thigh" | "R_Thigh" => Some(rot_z(0.05 * side(name))),
        _ => {
            // Fingers (not the thumbs, Finger0*): curl toward the palm, about the axis the fingers spread along.
            let finger = name.strip_prefix("L_Finger").or_else(|| name.strip_prefix("R_Finger"))?;
            if finger.starts_with('0') {
                return None;
            }
            let right = name.starts_with("R_");
            let amount = match (right, finger.len()) {
                (true, 1) => 1.0,
                (true, _) => 0.8,
                (false, _) => 0.25,
            };
            Some(rot_axis([0.0, 0.0, if right { 1.0 } else { -1.0 }], amount))
        }
    }
}

/// The posed body skeleton: per bone of the body FLVER, the transform from its bind position to its posed one
/// (`p * rotation + offset`).
struct Pose {
    names: HashMap<String, usize>,
    bind: Vec<(M3, [f32; 3])>,
    delta: Vec<(M3, [f32; 3])>,
    /// The right hand's grip point after posing.
    grip: [f32; 3],
}

const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

impl Pose {
    fn new(body: Option<&Flver>) -> Pose {
        let Some(f) = body else {
            return Pose { names: HashMap::new(), bind: vec![], delta: vec![], grip: [-0.25, 0.95, -0.15] };
        };
        let bind = bind_pose(&f.bones);
        let mut delta: Vec<Option<(M3, [f32; 3])>> = vec![None; f.bones.len()];
        fn get(i: usize, f: &Flver, bind: &[(M3, [f32; 3])], delta: &mut Vec<Option<(M3, [f32; 3])>>, depth: usize)
               -> (M3, [f32; 3]) {
            if let Some(d) = delta[i] {
                return d;
            }
            let b = &f.bones[i];
            let pivot = bind[i].1;
            let q = pose_rotation(&b.name, &bind[i]).unwrap_or(IDENTITY);
            // Turn about the joint, then follow the parent.
            let mut r = q;
            let mut t = sub(pivot, apply(pivot, &q));
            if b.parent >= 0 && (b.parent as usize) < f.bones.len() && depth < 64 {
                let (pr, pt) = get(b.parent as usize, f, bind, delta, depth + 1);
                r = mul(&r, &pr);
                t = add(apply(t, &pr), pt);
            }
            delta[i] = Some((r, t));
            (r, t)
        }
        let delta: Vec<(M3, [f32; 3])> = (0..f.bones.len()).map(|i| get(i, f, &bind, &mut delta, 0)).collect();
        let names: HashMap<String, usize> = f.bones.iter().enumerate().map(|(i, b)| (b.name.clone(), i)).collect();
        let mut grip = [-0.25, 0.95, -0.15];
        if let Some(&h) = names.get("R_Hand") {
            // R_Weapon sits 9 cm along the hand.
            let g = add(bind[h].1, scale(bind[h].0[0], 0.09));
            grip = add(apply(g, &delta[h].0), delta[h].1);
        }
        Pose { names, bind, delta, grip }
    }

    /// For each bone of a parts FLVER, the body bone that moves it: the one of the same name, else that of its
    /// nearest named ancestor, else the deforming body bone nearest its bind position. (Armour files carry partial
    /// skeletons; the gauntlets' forearm bones have no parents.)
    fn map(&self, f: &Flver) -> Vec<Option<usize>> {
        if self.delta.is_empty() {
            return vec![None; f.bones.len()];
        }
        let own = bind_pose(&f.bones);
        (0..f.bones.len())
            .map(|i| {
                let mut j = i as i32;
                for _ in 0..64 {
                    if j < 0 || j as usize >= f.bones.len() {
                        break;
                    }
                    if let Some(&k) = self.names.get(&f.bones[j as usize].name) {
                        return Some(k);
                    }
                    j = f.bones[j as usize].parent;
                }
                let p = own[i].1;
                self.names
                    .iter()
                    .filter(|(n, _)| !n.starts_with("Ctrl") && !n.starts_with("FC_") && !n.contains("Finger"))
                    .map(|(_, &k)| {
                        let d = sub(self.bind[k].1, p);
                        (dot(d, d), k)
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, k)| k)
            })
            .collect()
    }

    /// A skinned vertex: the weighted blend of its bones' bind-to-posed transforms.
    fn vertex(&self, p: [f32; 3], n: [f32; 3], bones: Option<&[u16; 4]>, weights: Option<&[f32; 4]>,
              map: &[Option<usize>]) -> ([f32; 3], [f32; 3]) {
        let (Some(b), Some(w)) = (bones, weights) else { return (p, n) };
        let total: f32 = w.iter().sum();
        let mut op = [0.0; 3];
        let mut on = [0.0; 3];
        let mut used = 0.0;
        for k in 0..4 {
            let wk = if total > 0.0 { w[k] / total } else if k == 0 { 1.0 } else { 0.0 };
            if wk <= 0.0 {
                continue;
            }
            let Some(Some(bone)) = map.get(b[k] as usize) else { continue };
            let (r, t) = &self.delta[*bone];
            op = add(op, scale(add(apply(p, r), *t), wk));
            on = add(on, scale(apply(n, r), wk));
            used += wk;
        }
        if used < 1e-4 {
            return (p, n);
        }
        // Weights on bones that could not be found stay put.
        (add(op, scale(p, 1.0 - used)), add(on, scale(n, 1.0 - used)))
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
    let mut stems: Vec<(String, bool)> = vec![(format!("fc_{g}_0000"), false), ("fg_a_0100".into(), false), ("fg_a_1500".into(), false)];
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
    let mut out = Vec::new();
    // The eyes' posed bounds (they come before the armour).
    let mut eyes: Option<([f32; 3], [f32; 3])> = None;
    for (stem, part, weapon) in &parts {
        let is_face = stem.starts_with("fg_");
        let is_hair = stem.starts_with("hr_");
        // A weapon's main model only (the others are sheaths and such).
        let flvers = if *weapon { &part.flvers[..part.flvers.len().min(1)] } else { &part.flvers[..] };
        for f in flvers {
            let map = pose.map(f);
            // Held weapons are shrunk to a wand's size (at most WEAPON_LENGTH from end to end), like W101's.
            let fit = if *weapon {
                let (lo, hi) = f.meshes.iter().flat_map(|m| m.pos.iter()).fold((0.0f32, 0.0f32), |(lo, hi), p| {
                    (lo.min(p[1]), hi.max(p[1]))
                });
                (WEAPON_LENGTH / (hi - lo).max(1e-3)).min(1.0)
            } else {
                1.0
            };
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
                        weapon_vertex(scale(m.pos[i], fit), m.nrm[i], pose.grip)
                    } else {
                        pose.vertex(m.pos[i], m.nrm[i], m.bones.get(i), m.weights.get(i), &map)
                    };
                    pos.push(p);
                    nrm.push(n);
                }
                let mut tris = m.tris.clone();
                if stem.starts_with("fg_a_15") {
                    for p in &pos {
                        let b = eyes.get_or_insert((*p, *p));
                        b.0 = [0, 1, 2].map(|i| b.0[i].min(p[i]));
                        b.1 = [0, 1, 2].map(|i| b.1[i].max(p[i]));
                    }
                } else if let (true, Some((lo, hi))) = (stem.starts_with("hd_"), eyes) {
                    // Masks and blindfolds over the eyes are left off: the book's wizard looks out at you.
                    tris.retain(|t| {
                        let c = [0, 1, 2].map(|i| t.iter().map(|&v| pos[v as usize][i]).sum::<f32>() / 3.0);
                        !(c[0] > lo[0] - 0.02 && c[0] < hi[0] + 0.02 && c[1] > lo[1] - 0.03 && c[1] < hi[1] + 0.03
                            && c[2] > lo[2] - 0.08 && c[2] < hi[2])
                    });
                }
                let own = stem.to_ascii_lowercase();
                let texture = if is_hair { None } else { texture_for(&m.material, &own, &textures) };
                out.push(Batch {
                    pos,
                    nrm,
                    uv: m.uv.clone(),
                    tris,
                    texture,
                    colour: if is_hair { look.hair_colour } else { [0.55, 0.52, 0.5] },
                    alpha_test: true,
                });
            }
        }
    }
    out
}

/// Longest held weapon, metres (a W101 wand is about half the wizard's height).
const WEAPON_LENGTH: f32 = 0.9;

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

/// Lighting and the sRGB <-> linear tables.
struct Shade {
    light: [f32; 3],
    fill: [f32; 3],
    to_linear: [f32; 256],
    to_srgb: Vec<f32>,
}

const SRGB_STEPS: usize = 4096;

impl Shade {
    fn new(light: [f32; 3], fill: [f32; 3]) -> Shade {
        Shade {
            light,
            fill,
            to_linear: std::array::from_fn(|i| srgb_to_linear(i as f32 / 255.0)),
            to_srgb: (0..=SRGB_STEPS).map(|i| linear_to_srgb(i as f32 / SRGB_STEPS as f32)).collect(),
        }
    }

    fn srgb(&self, linear: f32) -> f32 {
        self.to_srgb[((linear * SRGB_STEPS as f32) as usize).min(SRGB_STEPS)]
    }
}

/// Draws one batch's triangles into the rows `y0..y1` (`colour` and `depth` hold just those rows).
#[allow(clippy::too_many_arguments)]
fn raster(b: &Batch, screen: &[[f32; 3]], normals: &[[f32; 3]], sw: usize, y0: usize, y1: usize,
          colour: &mut [[f32; 4]], depth: &mut [f32], shade: &Shade) {
    let flat = [b.colour[0], b.colour[1], b.colour[2]].map(srgb_to_linear);
    for t in &b.tris {
        let [a, bb, c] = t.map(|i| i as usize);
        let (Some(&p0), Some(&p1), Some(&p2)) = (screen.get(a), screen.get(bb), screen.get(c)) else { continue };
        let miny = p0[1].min(p1[1]).min(p2[1]).floor().max(y0 as f32) as usize;
        let maxy = (p0[1].max(p1[1]).max(p2[1]).ceil() as i64).min(y1 as i64 - 1);
        if maxy < miny as i64 {
            continue;
        }
        let area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
        if area.abs() < 1e-6 {
            continue;
        }
        let minx = p0[0].min(p1[0]).min(p2[0]).floor().max(0.0) as usize;
        let maxx = (p0[0].max(p1[0]).max(p2[0]).ceil() as i64).min(sw as i64 - 1);
        if maxx < minx as i64 {
            continue;
        }
        let inv = 1.0 / area;
        // The mip level whose texels are about a pixel on screen.
        let tex = b.texture.as_ref().map(|t| {
            let (ua, ub, uc) = (b.uv[a], b.uv[bb], b.uv[c]);
            let uv_area = ((ub[0] - ua[0]) * (uc[1] - ua[1]) - (uc[0] - ua[0]) * (ub[1] - ua[1])).abs()
                * (t.w * t.h) as f32;
            let lod = (0.5 * (uv_area / area.abs()).max(1.0).log2()) as usize;
            t.level(lod)
        });
        // Edge functions step linearly along x.
        let (d0x, d1x) = (-(p2[1] - p1[1]) * inv, -(p0[1] - p2[1]) * inv);
        for y in miny..=maxy as usize {
            let py = y as f32 + 0.5;
            let px = minx as f32 + 0.5;
            let mut w0 = ((p1[0] - px) * (p2[1] - py) - (p2[0] - px) * (p1[1] - py)) * inv;
            let mut w1 = ((p2[0] - px) * (p0[1] - py) - (p0[0] - px) * (p2[1] - py)) * inv;
            let row = (y - y0) * sw;
            for x in minx..=maxx as usize {
                let (u0, u1) = (w0, w1);
                w0 += d0x;
                w1 += d1x;
                let u2 = 1.0 - u0 - u1;
                if u0 < 0.0 || u1 < 0.0 || u2 < 0.0 {
                    continue;
                }
                let z = u0 * p0[2] + u1 * p1[2] + u2 * p2[2];
                let at = row + x;
                if z >= depth[at] {
                    continue;
                }
                let albedo = match tex {
                    Some(tex) => {
                        let u = u0 * b.uv[a][0] + u1 * b.uv[bb][0] + u2 * b.uv[c][0];
                        let v = u0 * b.uv[a][1] + u1 * b.uv[bb][1] + u2 * b.uv[c][1];
                        let s = tex.sample(u, v);
                        if b.alpha_test && s[3] < 127.5 {
                            continue;
                        }
                        [0, 1, 2].map(|i| shade.to_linear[s[i] as usize & 255])
                    }
                    None => flat,
                };
                depth[at] = z;
                let mut n = norm(add(add(scale(normals[a], u0), scale(normals[bb], u1)), scale(normals[c], u2)));
                if n[2] > 0.0 {
                    n = scale(n, -1.0); // the back of a single-sided cloth faces the camera
                }
                let lit = 0.5 + 0.85 * dot(n, shade.light).max(0.0) + 0.3 * dot(n, shade.fill).max(0.0);
                colour[at] = [shade.srgb(albedo[0] * lit), shade.srgb(albedo[1] * lit), shade.srgb(albedo[2] * lit), 1.0];
            }
        }
    }
}

const PORTRAIT_PITCH: f32 = 0.2;

/// Draws the request: 2x supersampled, then averaged down; transparent background.
fn draw(cache: &mut Cache, r: &Request) -> Frame {
    let [w, h] = [r.size[0].max(8) as usize, r.size[1].max(8) as usize];
    // 2x supersampled; while turning, drawn at 70% and stretched, for speed.
    let ss = if r.fast { 1 } else { 2 };
    let (sw, sh) = if r.fast { (w, h) } else { (w * 2, h * 2) };
    let mut look = r.look.clone();
    if r.framing == Framing::Portrait {
        // The portrait shows the head as worn (hat on), without the weapon.
        look.weapon = None;
    }
    let batches = match &cache.posed {
        Some((l, b)) if *l == look => b.clone(),
        _ => {
            // Outer layers first (weapon, armour, then hair, face, body): what they cover fails the depth test
            // before it is shaded.
            let b = Arc::new(batches(cache, &look).into_iter().rev().collect::<Vec<_>>());
            cache.posed = Some((look.clone(), b.clone()));
            b
        }
    };
    // View: turn by yaw around the vertical axis through the body; camera looks along +Z (the character faces
    // -Z), x to the right, y up.
    let helm = look.armour.iter().any(|a| a.starts_with("hd_"));
    let [cy, height] = r.view.unwrap_or_else(|| r.framing.view(helm));
    let centre = [0.0, cy];
    let k = sh as f32 / height;
    // The portrait looks up a little, under a hat's brim.
    let pitch = if r.framing == Framing::Portrait && helm { PORTRAIT_PITCH } else { 0.0 };
    let turn = mul(&rot_y(r.yaw), &rot_x(pitch));
    let to_screen = |p: [f32; 3]| -> [f32; 3] {
        // Turned about the vertical axis through the feet and tilted about the view's centre.
        let q = apply([p[0], p[1] - centre[1], p[2]], &turn);
        [sw as f32 / 2.0 + (q[0] - centre[0]) * k, sh as f32 / 2.0 - q[1] * k, q[2]]
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
    // Project once, then rasterize in horizontal bands, one thread each.
    let projected: Vec<(Vec<[f32; 3]>, Vec<[f32; 3]>)> = batches
        .iter()
        .map(|b| (b.pos.iter().map(|p| to_screen(*p)).collect(), b.nrm.iter().map(|n| norm(apply(*n, &turn))).collect()))
        .collect();
    // Half the cores: the game keeps the rest.
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get() / 2).clamp(2, 6);
    let rows = sh.div_ceil(threads);
    let shade = Shade::new(light, fill);
    std::thread::scope(|scope| {
        for (band, (colour, depth)) in colour.chunks_mut(rows * sw).zip(depth.chunks_mut(rows * sw)).enumerate() {
            let (batches, projected, shade) = (&batches, &projected, &shade);
            scope.spawn(move || {
                let y0 = band * rows;
                let y1 = (y0 + rows).min(sh);
                for (b, (screen, normals)) in batches.iter().zip(projected) {
                    raster(b, screen, normals, sw, y0, y1, colour, depth, shade);
                }
            });
        }
    });
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let mut s = [0.0f32; 4];
            for dy in 0..ss {
                for dx in 0..ss {
                    let (cx, cy) = if r.fast {
                        ((x * sw / w).min(sw - 1), (y * sh / h).min(sh - 1))
                    } else {
                        (x * ss + dx, y * ss + dy)
                    };
                    let c = colour[cy * sw + cx];
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
                // Inside the ring of Art_Character_Cover02 (its hole is 0.375 of the width).
                let radius = w.min(h) as f32 * 0.37;
                a *= (radius - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            }
            rgba[o + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    Frame { rgba, size: [w as u32, h as u32] }
}
