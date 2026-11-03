//! w101_orbs (round 26, O1): the game's own scene depth, for hiding the overlay's world tags (name plates, quest
//! markers, wisps) behind whatever the game drew in front of them.
//!
//! How the scene depth buffer is found, by watching the game's D3D12 calls at runtime (no game code is read):
//! `ID3D12GraphicsCommandList::ResourceBarrier` is hooked. Every transition OUT of `DEPTH_WRITE` is the end of a
//! pass that wrote a depth buffer. Its resource is a candidate when it is a single-sample 2D depth-stencil texture
//! the size of the back buffer (or the same shape and at least half of it). Over each window of `WINDOW` frames the
//! candidate whose depth writes ended most often (exact back buffer size first) becomes the scene depth; every
//! choice is logged with the whole candidate table. The app checks the choice against the player's own distance
//! (depth under the player's chest = the camera's distance to the player) and rejects it when it never agrees
//! (`reject_current`), and the next candidate is tried.
//!
//! How it gets into the overlay: right after the game's barrier that ends the depth writes, on the game's own
//! command list (so the copy is ordered with the game's work and taken before anything later in the frame can
//! reuse that memory), a copy of the depth plane is recorded into our own texture (same size, the depth format's
//! typeless twin, kept in COPY_DEST between uses). The overlay renderer moves it to PIXEL_SHADER_RESOURCE while it
//! draws and samples it in the pixel shader: a draw between `tag(z)` and `DEPTH_OFF` callbacks has every pixel
//! whose scene depth is nearer than `z` metres discarded. Reversed or normal Z and the near/far planes come from
//! the app (`set_projection`, CSCam's planes and the player check).

use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use parking_lot::Mutex;
use tracing::{error, info};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

pub(crate) type ResourceBarrierFn =
    unsafe extern "system" fn(this: *mut c_void, count: u32, barriers: *const D3D12_RESOURCE_BARRIER);

/// The original ResourceBarrier (the hook's trampoline).
pub(crate) static BARRIER_TRAMPOLINE: AtomicUsize = AtomicUsize::new(0);
/// The back buffer's size (width << 32 | height), set by the renderer each frame.
pub(crate) static BACK_BUFFER: AtomicU64 = AtomicU64::new(0);
/// Frames rendered (Present calls the overlay drew in).
pub(crate) static FRAME: AtomicU64 = AtomicU64::new(0);
static BARRIER_CALLS: AtomicU64 = AtomicU64::new(0);
static DEPTH_ENDS: AtomicU64 = AtomicU64::new(0);
static COPIES: AtomicU64 = AtomicU64::new(0);
static LAST_COPY_FRAME: AtomicU64 = AtomicU64::new(0);
static NEAR: AtomicU32 = AtomicU32::new(0);
static FAR: AtomicU32 = AtomicU32::new(0);
static REVERSED: AtomicBool = AtomicBool::new(true);
static WANTED: AtomicBool = AtomicBool::new(true);

const DEPTH_WRITE: u32 = 0x10;
const COPY_SOURCE: u32 = 0x800;
/// Frames per choosing window.
const WINDOW: u64 = 120;

/// The callback user data that turns the depth test on for what follows, with its reference depth in the low 32
/// bits (see [`tag`]).
pub const DEPTH_TAG_HI: usize = 0x5710_D0E9;
/// The callback user data that turns the depth test off again.
pub const DEPTH_OFF: usize = 0x5710_D0FF;

/// User data for [`crate::blend_callback`]: what follows is hidden wherever the game's scene is nearer than `z`
/// metres from the camera (along its forward axis). Ends at [`DEPTH_OFF`] or the end of the draw list.
pub fn tag(z: f32) -> usize {
    (DEPTH_TAG_HI << 32) | z.to_bits() as usize
}

/// The camera's near and far planes (metres; far <= near = an infinite far plane) and whether the depth buffer is
/// reversed (1 at the near plane, 0 far away).
pub fn set_projection(near: f32, far: f32, reversed: bool) {
    NEAR.store(near.to_bits(), Ordering::Relaxed);
    FAR.store(far.to_bits(), Ordering::Relaxed);
    REVERSED.store(reversed, Ordering::Relaxed);
}

/// Turns the capture on or off (ini).
pub fn set_wanted(on: bool) {
    WANTED.store(on, Ordering::Relaxed);
}

pub(crate) fn projection() -> (f32, f32, bool) {
    (
        f32::from_bits(NEAR.load(Ordering::Relaxed)),
        f32::from_bits(FAR.load(Ordering::Relaxed)),
        REVERSED.load(Ordering::Relaxed),
    )
}

/// Metres from the camera of a depth buffer value, for the projection set by [`set_projection`].
pub fn linear(d: f32) -> f32 {
    let (n, f, reversed) = projection();
    linear_with(d, n, f, reversed)
}

/// The same with the projection given.
pub fn linear_with(d: f32, n: f32, f: f32, reversed: bool) -> f32 {
    let infinite = f <= n;
    match (reversed, infinite) {
        (true, true) => n / d.max(1.0e-9),
        (true, false) => n * f / (d * (f - n) + n),
        (false, true) => n / (1.0 - d).max(1.0e-9),
        (false, false) => n * f / (f - d * (f - n)).max(1.0e-9),
    }
}

/// What the capture is doing, for the app's log.
#[derive(Clone, Debug, Default)]
pub struct Info {
    /// A copy was made in the last few frames (the test can be used).
    pub fresh: bool,
    /// The game's resource chosen as the scene depth (0 = none yet).
    pub resource: usize,
    /// Its size and DXGI format.
    pub width: u64,
    /// Its height.
    pub height: u32,
    /// Its DXGI format.
    pub format: u32,
    /// How many times it was chosen (a new one each time the game makes a new buffer or one is rejected).
    pub generation: u32,
    /// Copies made so far.
    pub copies: u64,
    /// Frames rendered so far.
    pub frames: u64,
    /// ResourceBarrier calls seen, and transitions out of DEPTH_WRITE among them.
    pub barrier_calls: u64,
    /// Transitions out of DEPTH_WRITE seen.
    pub depth_ends: u64,
    /// Candidates seen.
    pub candidates: usize,
    /// The state the game leaves it in after its depth writes end.
    pub after_state: u32,
}

struct Candidate {
    resource: usize,
    width: u64,
    height: u32,
    format: u32,
    /// Whether it is a usable depth buffer of the back buffer's shape (false: seen and ruled out).
    usable: bool,
    exact: bool,
    ends: u32,
    last_ends: u32,
    rejected: bool,
}

#[derive(Default)]
struct State {
    candidates: Vec<Candidate>,
    /// The chosen game resource (a reference held) and our copy of it.
    main: Option<ID3D12Resource>,
    main_raw: usize,
    copy: Option<ID3D12Resource>,
    copy_format: u32,
    srv_format: u32,
    width: u64,
    height: u32,
    format: u32,
    generation: u32,
    after_state: u32,
    /// The candidate to adopt the next time its barrier is seen (it is alive then).
    pending: usize,
    window_start: u64,
    /// Old copies, kept alive (game command lists recorded with them may not have run yet).
    retired: Vec<ID3D12Resource>,
    logged_none: bool,
    /// The frame the main buffer's depth ends are counted for, the count so far, and the frame last copied in.
    /// The game ends depth writes on the main buffer many times a frame; copying every time cost about 40 fps
    /// at 2560x1440 (round 27 log), so one copy a frame is made, at about the frame's last end.
    frame_of_ends: u64,
    frame_ends: u64,
    prev_frame_ends: u64,
    copied_frame: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// The copy for the renderer: (texture, SRV format, width, height, generation), when one exists.
pub(crate) fn current_copy() -> Option<(ID3D12Resource, DXGI_FORMAT, u64, u32, u32)> {
    let s = STATE.lock();
    let s = s.as_ref()?;
    s.copy.as_ref().map(|c| (c.clone(), DXGI_FORMAT(s.srv_format as i32), s.width, s.height, s.generation))
}

/// Whether a copy was made within the last `frames` frames.
pub(crate) fn fresh(frames: u64) -> bool {
    let last = LAST_COPY_FRAME.load(Ordering::Relaxed);
    last > 0 && FRAME.load(Ordering::Relaxed).saturating_sub(last) <= frames
}

/// What the capture is doing.
pub fn info() -> Info {
    let s = STATE.lock();
    let mut out = Info {
        fresh: fresh(3),
        copies: COPIES.load(Ordering::Relaxed),
        frames: FRAME.load(Ordering::Relaxed),
        barrier_calls: BARRIER_CALLS.load(Ordering::Relaxed),
        depth_ends: DEPTH_ENDS.load(Ordering::Relaxed),
        ..Default::default()
    };
    if let Some(s) = s.as_ref() {
        out.resource = s.main_raw;
        out.width = s.width;
        out.height = s.height;
        out.format = s.format;
        out.generation = s.generation;
        out.candidates = s.candidates.iter().filter(|c| c.usable).count();
        out.after_state = s.after_state;
    }
    out
}

/// The app found the chosen buffer is not the scene depth (it never agreed with the player's distance): it is
/// dropped and never chosen again; the next window chooses another.
pub fn reject_current() {
    let mut s = STATE.lock();
    let Some(s) = s.as_mut() else { return };
    let raw = s.main_raw;
    if let Some(c) = s.candidates.iter_mut().find(|c| c.resource == raw) {
        c.rejected = true;
    }
    info!("depth: buffer {raw:#x} rejected");
    drop_main(s);
}

fn drop_main(s: &mut State) {
    if let Some(c) = s.copy.take() {
        s.retired.push(c);
    }
    if let Some(m) = s.main.take() {
        s.retired.push(m);
    }
    s.main_raw = 0;
}

/// The renderer's frame tick: the choosing window.
pub(crate) fn tick() {
    let frame = FRAME.fetch_add(1, Ordering::Relaxed) + 1;
    let mut guard = STATE.lock();
    let s = guard.get_or_insert_with(State::default);
    if frame < s.window_start + WINDOW {
        return;
    }
    s.window_start = frame;
    for c in s.candidates.iter_mut() {
        c.last_ends = c.ends;
        c.ends = 0;
    }
    let main_alive = s.main_raw != 0 && s.candidates.iter().any(|c| c.resource == s.main_raw && c.last_ends > 0);
    if main_alive {
        return;
    }
    // Exact back buffer size first, then the most depth writes ended in the window, then the largest.
    let best = s
        .candidates
        .iter()
        .filter(|c| c.usable && !c.rejected && c.last_ends > 0)
        .max_by_key(|c| (c.exact, c.last_ends, c.width * c.height as u64))
        .map(|c| c.resource);
    match best {
        Some(raw) if raw != s.main_raw => {
            s.pending = raw;
        },
        None if !s.logged_none && frame > 10 * WINDOW => {
            s.logged_none = true;
            info!(
                "depth: no scene depth buffer found after {frame} frames ({} ResourceBarrier calls, {} ended depth \
                 writes, candidates {})",
                BARRIER_CALLS.load(Ordering::Relaxed),
                DEPTH_ENDS.load(Ordering::Relaxed),
                table(s)
            );
        },
        _ => {},
    }
}

fn table(s: &State) -> String {
    let rows: Vec<String> = s
        .candidates
        .iter()
        .filter(|c| c.usable)
        .map(|c| {
            format!(
                "{:#x} {}x{} fmt {} ends {}/{WINDOW} frames{}{}",
                c.resource,
                c.width,
                c.height,
                c.format,
                c.last_ends,
                if c.exact { "" } else { " (not back buffer size)" },
                if c.rejected { " rejected" } else { "" }
            )
        })
        .collect();
    format!("[{}]", rows.join("; "))
}

/// The typeless format our copy is made in and the format it is read as, for a depth format.
fn copy_formats(format: DXGI_FORMAT) -> Option<(DXGI_FORMAT, DXGI_FORMAT)> {
    Some(match format {
        DXGI_FORMAT_D32_FLOAT | DXGI_FORMAT_R32_TYPELESS | DXGI_FORMAT_R32_FLOAT => {
            (DXGI_FORMAT_R32_TYPELESS, DXGI_FORMAT_R32_FLOAT)
        },
        DXGI_FORMAT_D32_FLOAT_S8X24_UINT | DXGI_FORMAT_R32G8X24_TYPELESS => {
            (DXGI_FORMAT_R32G8X24_TYPELESS, DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS)
        },
        DXGI_FORMAT_D24_UNORM_S8_UINT | DXGI_FORMAT_R24G8_TYPELESS => {
            (DXGI_FORMAT_R24G8_TYPELESS, DXGI_FORMAT_R24_UNORM_X8_TYPELESS)
        },
        DXGI_FORMAT_D16_UNORM | DXGI_FORMAT_R16_TYPELESS => (DXGI_FORMAT_R16_TYPELESS, DXGI_FORMAT_R16_UNORM),
        _ => return None,
    })
}

/// The hook: the game's barriers, then (after a depth pass of the scene depth ends) our copy.
pub(crate) unsafe extern "system" fn resource_barrier_impl(
    this: *mut c_void,
    count: u32,
    barriers: *const D3D12_RESOURCE_BARRIER,
) {
    let trampoline = BARRIER_TRAMPOLINE.load(Ordering::Relaxed);
    if trampoline == 0 {
        return;
    }
    let original: ResourceBarrierFn = std::mem::transmute(trampoline);
    original(this, count, barriers);
    BARRIER_CALLS.fetch_add(1, Ordering::Relaxed);
    if barriers.is_null() || count == 0 || this.is_null() || !WANTED.load(Ordering::Relaxed) {
        return;
    }
    let list = std::slice::from_raw_parts(barriers, count as usize);
    for b in list {
        if b.Type != D3D12_RESOURCE_BARRIER_TYPE_TRANSITION
            || b.Flags.0 & D3D12_RESOURCE_BARRIER_FLAG_BEGIN_ONLY.0 != 0
        {
            continue;
        }
        let t = &b.Anonymous.Transition;
        let (before, after) = (t.StateBefore.0 as u32, t.StateAfter.0 as u32);
        if before & DEPTH_WRITE == 0 || after & DEPTH_WRITE != 0 {
            continue;
        }
        let raw: usize = std::mem::transmute_copy(&t.pResource);
        if raw == 0 {
            continue;
        }
        DEPTH_ENDS.fetch_add(1, Ordering::Relaxed);
        depth_write_ended(this, raw, t.Subresource, after, original);
    }
}

unsafe fn depth_write_ended(list: *mut c_void, raw: usize, subresource: u32, after: u32, original: ResourceBarrierFn) {
    let job = {
        let mut guard = STATE.lock();
        let s = guard.get_or_insert_with(State::default);
        let k = match s.candidates.iter().position(|c| c.resource == raw) {
            Some(k) => k,
            None => {
                s.candidates.push(describe(raw));
                if s.candidates.len() > 64 {
                    // Released buffers' addresses are reused; keep the table small.
                    let main = s.main_raw;
                    s.candidates.retain(|c| c.usable || c.resource == main);
                }
                s.candidates.iter().position(|c| c.resource == raw).unwrap_or(0)
            },
        };
        if s.candidates.get(k).is_some_and(|c| c.resource == raw) {
            s.candidates[k].ends += 1;
        }
        if s.pending == raw {
            s.pending = 0;
            adopt(s, raw);
        }
        if s.main_raw == raw && (subresource == D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES || subresource == 0) {
            s.after_state = after;
            let frame = FRAME.load(Ordering::Relaxed);
            if s.frame_of_ends != frame {
                s.frame_of_ends = frame;
                s.prev_frame_ends = s.frame_ends;
                s.frame_ends = 0;
            }
            let index = s.frame_ends;
            s.frame_ends += 1;
            // Copy once, at the end that was last in the previous frame (a frame with fewer ends keeps the
            // older copy, and the next frame adjusts).
            if index + 1 >= s.prev_frame_ends.max(1) && s.copied_frame != frame {
                s.copy.clone().zip(s.main.clone()).map(|j| (j, frame))
            } else {
                None
            }
        } else {
            None
        }
    };
    if let Some(((copy, source), frame)) = job {
        if record_copy(list, &copy, &source, after, original) {
            if let Some(s) = STATE.lock().as_mut() {
                s.copied_frame = frame;
            }
        }
    }
}

/// A resource whose depth writes ended: its description (alive now, inside the barrier call).
unsafe fn describe(raw: usize) -> Candidate {
    let ptr = raw as *mut c_void;
    let mut c = Candidate {
        resource: raw,
        width: 0,
        height: 0,
        format: 0,
        usable: false,
        exact: false,
        ends: 0,
        last_ends: 0,
        rejected: false,
    };
    let Some(resource) = ID3D12Resource::from_raw_borrowed(&ptr) else { return c };
    let desc = resource.GetDesc();
    c.width = desc.Width;
    c.height = desc.Height;
    c.format = desc.Format.0 as u32;
    let bb = BACK_BUFFER.load(Ordering::Relaxed);
    let (bw, bh) = ((bb >> 32) as f64, (bb & 0xffff_ffff) as f64);
    let (w, h) = (desc.Width as f64, desc.Height as f64);
    let shaped = bw > 0.0 && bh > 0.0 && (w / h - bw / bh).abs() < 0.01 && w >= bw * 0.5 && w <= bw * 1.01;
    c.exact = desc.Width as f64 == bw && desc.Height as f64 == bh;
    c.usable = desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D
        && desc.SampleDesc.Count == 1
        && desc.DepthOrArraySize == 1
        && desc.Flags.0 & D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL.0 != 0
        && desc.Flags.0 & D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE.0 == 0
        && copy_formats(desc.Format).is_some()
        && shaped;
    c
}

/// Makes `raw` the scene depth: a reference to it and a copy texture for it.
unsafe fn adopt(s: &mut State, raw: usize) {
    let ptr = raw as *mut c_void;
    let Some(resource) = ID3D12Resource::from_raw_borrowed(&ptr) else { return };
    let resource = resource.clone();
    let desc = resource.GetDesc();
    let Some((copy_format, srv_format)) = copy_formats(desc.Format) else { return };
    let device: windows::core::Result<ID3D12Device> = crate::util::try_out_ptr(|v| resource.GetDevice(v));
    let Ok(device) = device else { return };
    let mut copy: Option<ID3D12Resource> = None;
    let made = device.CreateCommittedResource(
        &D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
            MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
            CreationNodeMask: 0,
            VisibleNodeMask: 0,
        },
        D3D12_HEAP_FLAG_NONE,
        &D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: desc.Width,
            Height: desc.Height,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: copy_format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
        },
        D3D12_RESOURCE_STATE_COPY_DEST,
        None,
        &mut copy,
    );
    let Some(copy) = made.as_ref().ok().and(copy) else {
        error!("depth: could not make a copy texture for {raw:#x}: {made:?}");
        if let Some(c) = s.candidates.iter_mut().find(|c| c.resource == raw) {
            c.rejected = true;
        }
        return;
    };
    let _ = copy.SetName(windows::core::w!("w101_orbs scene depth copy"));
    drop_main(s);
    s.main = Some(resource);
    s.main_raw = raw;
    s.copy = Some(copy);
    s.copy_format = copy_format.0 as u32;
    s.srv_format = srv_format.0 as u32;
    s.width = desc.Width;
    s.height = desc.Height;
    s.format = desc.Format.0 as u32;
    s.generation += 1;
    info!(
        "depth: chose {raw:#x} ({}x{}, DXGI format {}, flags {:#x}) as the scene depth (choice {}); candidates {}",
        desc.Width,
        desc.Height,
        desc.Format.0,
        desc.Flags.0,
        s.generation,
        table(s)
    );
}

fn barrier(resource: &ID3D12Resource, before: u32, after: u32) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: ManuallyDrop::new(Some(resource.clone())),
                Subresource: 0,
                StateBefore: D3D12_RESOURCE_STATES(before as i32),
                StateAfter: D3D12_RESOURCE_STATES(after as i32),
            }),
        },
    }
}

/// Records, on the game's command list right after its barrier, the copy of the depth plane into ours.
unsafe fn record_copy(
    list: *mut c_void,
    copy: &ID3D12Resource,
    source: &ID3D12Resource,
    after: u32,
    original: ResourceBarrierFn,
) -> bool {
    let Some(list_ref) = ID3D12GraphicsCommandList::from_raw_borrowed(&list) else { return false };
    // Copies and these transitions belong on a direct (graphics) list.
    if list_ref.GetType() != D3D12_COMMAND_LIST_TYPE_DIRECT {
        return false;
    }
    let move_source = after & COPY_SOURCE == 0;
    if move_source {
        let b = barrier(source, after, COPY_SOURCE);
        original(list, 1, &b);
        crate::util::drop_barrier(b);
    }
    let dst = D3D12_TEXTURE_COPY_LOCATION {
        pResource: ManuallyDrop::new(Some(copy.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
    };
    let src = D3D12_TEXTURE_COPY_LOCATION {
        pResource: ManuallyDrop::new(Some(source.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 },
    };
    list_ref.CopyTextureRegion(&dst, 0, 0, 0, &src, None);
    let _ = ManuallyDrop::into_inner(dst.pResource);
    let _ = ManuallyDrop::into_inner(src.pResource);
    if move_source {
        let b = barrier(source, COPY_SOURCE, after);
        original(list, 1, &b);
        crate::util::drop_barrier(b);
    }
    COPIES.fetch_add(1, Ordering::Relaxed);
    LAST_COPY_FRAME.store(FRAME.load(Ordering::Relaxed).max(1), Ordering::Relaxed);
    true
}

/// Points (screen pixels) to read the scene depth at, asked by the app; the renderer copies the depth to the CPU
/// on a frame it can and answers some frames later ([`take_probe`]).
static PROBE_REQUEST: Mutex<Option<(u64, Vec<[f32; 2]>)>> = Mutex::new(None);
static PROBE_RESULT: Mutex<Option<Probe>> = Mutex::new(None);

/// Scene depth values read back at the points of a [`probe`] request.
#[derive(Clone, Debug)]
pub struct Probe {
    /// The request's id.
    pub id: u64,
    /// The points (screen pixels).
    pub points: Vec<[f32; 2]>,
    /// The raw depth buffer values there (NaN outside the buffer).
    pub depth: Vec<f32>,
}

/// Asks for the scene depth at `points` (screen pixels); replaces an older request not taken yet. `id` comes back
/// with the answer.
pub fn probe(id: u64, points: Vec<[f32; 2]>) {
    *PROBE_REQUEST.lock() = Some((id, points));
}

/// The answer to the last [`probe`] taken by the renderer, once.
pub fn take_probe() -> Option<Probe> {
    PROBE_RESULT.lock().take()
}

pub(crate) fn take_probe_request() -> Option<(u64, Vec<[f32; 2]>)> {
    PROBE_REQUEST.lock().take()
}

pub(crate) fn put_probe(p: Probe) {
    *PROBE_RESULT.lock() = Some(p);
}
