//! Wizard101's Quest Helper (round 18, quests F4): the yellow arrow that points the way to the current quest goal,
//! with the goal's name under it, from QuestHelperHud.gui (Root.wad; tools/w101_book_layout.py
//! --pages=QuestHelperHud) and the arrow's own model, Character/Arrow/Quest_Arrow_JT.nif (tools/w101_quest_art.py:
//! its texture FX_Quest_Arrow_JT as Quest/Quest_Arrow.png).
//!
//! QuestHelperHud.gui: a 600 x 200 window at 318,425 of Wizard101's 1024 x 768 screen (HCENTER: centred across, low
//! in the screen's middle, over the bottom HUD), with ArrowWindow (a PortraitWindow, i.e. a little 3D view,
//! 225,39..375,149) in which the client draws the arrow model turned toward the goal, txtDistance (black,
//! WizardNormal 0.45) over it, and txtGoalName (yellow FFFF00 with a drop shadow, WizardNormal 0.4) under it. The nif
//! is one flat quad 42.46 x 70 units (the arrow lying on the ground, tip forward); here it is drawn as that quad seen
//! by a fixed camera above and behind it, turned by the goal's bearing from the camera's view direction, so it
//! leans into the screen when the goal is ahead and swings round as the camera turns.
//!
//! The goal is the first quest of the Quests page (main quests first, as Wizard101's helper follows the main
//! quest unless another is picked): its first current goal, or once ready its turn-in. Where each goal is
//! (TARGETS) comes from the map itself (tools/w101_quest_targets.py reading m60_42_36_00.msb): talk goals point at
//! the NPC, goto goals at their sphere, defeat goals at the nearest enemy of the kind, the Bone Cages at the nearest
//! cage, bosses at the boss; for a goal in a room (60 m under the street) the arrow points at the room's door
//! until the player is inside.
//!
//! Round 20 (Q8): txtDistance shows the steps to the goal. Wizard101's players' guide (wizard101.com
//! w101playersguide/questing3): the arrow shows "the approximate number of steps it will take your Wizard to reach
//! that destination". The walk is measured along Wizard101's own walk graph of Unicorn Way (zone.nav, 1079 nodes on
//! a 4.8 m grid, decoded by tools/w101_pathing.py, moved into this map's coordinates: w101_orbs/book/quest_nav.tsv,
//! Wizard101 data, made on the PC), from the node nearest the player to the one nearest the goal, plus the two ends;
//! where the graph doesn't reach (the hedge maze, the rooms) it is the straight distance. One step is STEP metres
//! (the client's own stride length isn't in its data files; a walking stride).

use std::collections::BinaryHeap;
use std::path::Path;

use hudhook::imgui::{DrawListMut, ImColor32};

use crate::layout::{Layout, Textures, View};
use crate::markers::Camera;
use crate::quests::Active;

pub const LAYOUT: &str = "QuestHelperHud";
pub const ARROW: &str = "Quest/Quest_Arrow.png";
/// QuestHelperHud's m_Window on Wizard101's 1024 x 768 screen (its top; HCENTER centres it across).
const TOP: f32 = 425.0;
const SCREEN_H: f32 = 768.0;
/// The tile of the street (m60_42_36_00) and how far from its corner the helper still shows (metres).
const TILE: [i32; 2] = [42, 36];
const TILE_RANGE: f32 = 600.0;
/// Below this MSB height the player is in one of the rooms under the street (the street is at about 94 m).
const ROOM_HEIGHT: f32 = 70.0;
/// Quest_Arrow_JT.nif's quad: half width and half length (units), the tip at +y (the texture's bottom edge).
const HALF_W: f32 = 21.23;
const HALF_L: f32 = 35.0;
/// The little camera over the arrow: back, up (units) and focal length (layout pixels per unit at depth 1).
const BACK: f32 = 100.0;
const UP: f32 = 120.0;
const FOCAL: f32 = 230.0;

// tools/w101_quest_targets.py m60_42_36_00.msb.dcx (msb 17b1dd61): (quest, goal index from 0, MSB points, the
// room's door on the street for goals inside a room).
const TARGETS: &[(&str, usize, &[[f32; 3]], Option<[f32; 3]>)] = &[
    ("unicorn_way", 0, &[[5.003, 94.183, -44.078]], None),
    ("ghost_hunters", 0, &[[-106.838, 93.988, -41.934], [-125.000, 93.999, -83.660], [-143.009, 93.999, -43.909], [-153.779, 92.499, -65.799], [-112.926, 93.999, -41.266], [-122.484, 93.999, -47.028], [-118.689, 93.999, -55.466], [-110.309, 93.999, -47.553], [-121.378, 93.999, -93.675], [-114.960, 93.999, -90.218], [-120.229, 93.999, -79.554], [-125.000, 93.999, -71.722], [-146.090, 93.999, -31.099], [-151.042, 93.999, -18.986], [-156.601, 93.999, -21.262], [-152.333, 93.999, -29.959], [-144.529, 92.499, -69.751], [-140.999, 92.520, -68.820], [-141.858, 92.499, -60.844], [-149.830, 92.499, -53.881]], None),
    ("ghost_hunters", 1, &[[5.003, 94.183, -44.078]], None),
    ("ghost_hunters", 2, &[[-84.838, 94.488, -18.448]], None),
    ("skeleton_crew", 0, &[[-174.706, 93.865, -7.295], [-224.319, 93.999, -14.996], [-233.068, 93.999, -38.080], [-211.351, 92.499, -48.532], [-191.699, 93.999, -14.760], [-200.160, 93.999, -19.839], [-197.825, 93.999, -26.461], [-188.622, 93.999, -20.708], [-226.537, 93.999, -2.599], [-231.498, 93.999, 5.621], [-237.135, 93.999, 0.639], [-233.131, 93.999, -5.810], [-246.934, 93.997, -42.301], [-253.552, 93.997, -46.452], [-250.329, 93.997, -54.021], [-242.953, 93.997, -48.753], [-206.137, 92.499, -46.495], [-202.871, 92.527, -37.287], [-214.245, 92.499, -37.220], [-220.472, 92.499, -43.283]], None),
    ("skeleton_crew", 1, &[[-84.838, 94.488, -18.448]], None),
    ("monsters_and_mazes", 0, &[[-335.106, 94.299, 103.547]], None),
    ("monsters_and_mazes", 1, &[[-335.106, 93.499, 103.547]], None),
    ("finding_out_why", 0, &[[-284.496, 93.000, -57.482], [-268.677, 93.000, -48.272], [-325.141, 93.999, -26.388], [-66.410, 93.171, -12.722]], None),
    ("finding_out_why", 1, &[[-278.594, 93.997, -44.062], [-289.062, 92.999, 3.035], [-281.735, 93.997, -30.283], [-284.642, 93.999, -20.763], [-290.663, 93.999, -22.944], [-287.072, 93.999, -32.296], [-303.581, 92.992, -1.351], [-320.420, 93.999, -12.637], [-316.935, 93.999, -18.455], [-299.868, 92.999, -9.293]], None),
    ("finding_out_why", 2, &[[-335.106, 94.299, 103.547]], None),
    ("finding_out_why", 3, &[[-335.106, 93.499, 103.547]], None),
    ("who_could_it_be_now", 0, &[[-84.838, 94.488, -18.448]], None),
    ("sinister_skeleton", 0, &[[-242.629, 37.999, 20.072]], Some([-240.829, 94.349, 16.343])),
    ("sinister_skeleton", 1, &[[-242.425, 33.999, 19.671]], Some([-240.829, 94.349, 16.343])),
    ("sinister_skeleton", 2, &[[-84.838, 94.999, -18.448]], None),
    ("sinister_skeleton", 3, &[[-84.838, 94.488, -18.448]], None),
    ("pesky_pirates", 0, &[[-174.706, 93.865, -7.295], [-224.319, 93.999, -14.996], [-233.068, 93.999, -38.080], [-211.351, 92.499, -48.532], [-191.699, 93.999, -14.760], [-200.160, 93.999, -19.839], [-197.825, 93.999, -26.461], [-188.622, 93.999, -20.708], [-226.537, 93.999, -2.599], [-231.498, 93.999, 5.621], [-237.135, 93.999, 0.639], [-233.131, 93.999, -5.810], [-246.934, 93.997, -42.301], [-253.552, 93.997, -46.452], [-250.329, 93.997, -54.021], [-242.953, 93.997, -48.753], [-206.137, 92.499, -46.495], [-202.871, 92.527, -37.287], [-214.245, 92.499, -37.220], [-220.472, 92.499, -43.283]], None),
    ("pesky_pirates", 1, &[[-65.865, 92.825, -7.976]], None),
    ("saving_private_oryan", 0, &[[-160.827, 94.999, -54.024]], None),
    ("saving_private_oryan", 1, &[[-160.827, 93.488, -54.024]], None),
    ("saving_private_oryan", 2, &[[-106.973, 34.999, -114.507]], Some([-112.061, 94.379, -104.406])),
    ("saving_private_oryan", 3, &[[-110.717, 33.999, -106.976]], Some([-112.061, 94.379, -104.406])),
    ("saving_private_oryan", 4, &[[-160.827, 93.488, -54.024]], None),
    ("saving_private_oryan", 5, &[[5.003, 94.183, -44.078]], None),
    ("fear_of_fire", 0, &[[-188.150, 34.999, -2.772]], Some([-188.461, 94.349, -2.847])),
    ("fear_of_fire", 1, &[[-182.461, 33.999, -13.764]], Some([-188.461, 94.349, -2.847])),
    ("fear_of_fire", 2, &[[-65.865, 92.825, -7.976]], None),
];

/// The player's MSB position in the street's tile, from Game::position (chunk position, block, block origin).
pub fn player_msb(position: ([f32; 3], i32, i32)) -> Option<[f32; 3]> {
    let (chunk, _, origin) = position;
    let o = crate::markers::tile(origin)?;
    let p = [chunk[0] + o[0] - TILE[0] as f32 * 256.0, chunk[1], chunk[2] + o[1] - TILE[1] as f32 * 256.0];
    (p[0].abs() < TILE_RANGE && p[2].abs() < TILE_RANGE).then_some(p)
}

/// Round 21 (Q6): the player's MSB position from the street NPCs the game has loaded (`entities` =
/// Game::entities): each one's MSB spot (markers::NPCS) plus the player's offset from it in the physics space,
/// averaged. Needs no block origin (Ryan's round 20 helper never showed; the block-origin path is the fallback).
pub fn player_msb_from_npcs(camera: &Camera, entities: &[(u32, [f32; 3])]) -> Option<[f32; 3]> {
    let p = camera.player;
    let (mut sum, mut n) = ([0.0f32; 3], 0.0f32);
    for (id, at) in entities {
        let Some(npc) = crate::markers::NPCS.iter().find(|n| n.2 == *id) else { continue };
        for k in 0..3 {
            sum[k] += npc.4[k] + p[k] - at[k];
        }
        n += 1.0;
    }
    (n > 0.0).then(|| sum.map(|v| v / n))
}

/// Where the arrow points for quest `active` with the player at `player` (MSB): the goal's nearest point, or the
/// room's door while the player is out on the street.
pub fn target(active: &Active, player: [f32; 3]) -> Option<[f32; 3]> {
    let (_, _, points, door) = TARGETS.iter().find(|t| t.0 == active.slug && t.1 == active.goal_index)?;
    if let Some(door) = door {
        if player[1] > ROOM_HEIGHT {
            return Some(*door);
        }
    }
    let d2 = |p: &[f32; 3]| (p[0] - player[0]).powi(2) + (p[2] - player[2]).powi(2);
    points.iter().min_by(|a, b| d2(a).total_cmp(&d2(b))).copied()
}

/// The goal's bearing from the camera's view direction (radians, clockwise on the ground, 0 = straight ahead).
pub fn bearing(camera: &Camera, player: [f32; 3], goal: [f32; 3]) -> f32 {
    let (dx, dz) = (goal[0] - player[0], goal[2] - player[2]);
    let m = &camera.matrix;
    let ahead = dx * m[2][0] + dz * m[2][2];
    let right = dx * m[0][0] + dz * m[0][2];
    right.atan2(ahead)
}

/// The helper's scale (screen pixels per layout pixel) and its top-left corner on a `screen`.
pub fn placement(layout: &Layout, screen: [f32; 2]) -> (f32, [f32; 2]) {
    let s = screen[1] / SCREEN_H;
    let w = layout.nodes.first().map(|n| n.rect[2] - n.rect[0]).unwrap_or(600.0);
    (s, [(screen[0] - w * s) / 2.0, TOP * s])
}

/// The arrow quad's corners in layout pixels (tail left, tail right, tip right, tip left: the texture's (0,0),
/// (1,0), (1,1), (0,1)), turned by `bearing`, in ArrowWindow's middle.
pub fn arrow_quad(layout: &Layout, bearing: f32) -> [[f32; 2]; 4] {
    let r = layout.find("ArrowWindow").map(|i| layout.nodes[i].rect).unwrap_or([225.0, 39.0, 375.0, 149.0]);
    let centre = [(r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0];
    let (sin, cos) = bearing.sin_cos();
    let length = (BACK * BACK + UP * UP).sqrt();
    let corner = |x: f32, y: f32| {
        // On the ground: right, ahead; then seen from (0, -BACK, UP) looking at the arrow's middle.
        let (gx, gy) = (x * cos + y * sin, -x * sin + y * cos);
        let depth = ((gy + BACK) * BACK + UP * UP) / length;
        let up = gy * UP / length;
        [centre[0] + gx / depth * FOCAL, centre[1] - up / depth * FOCAL]
    };
    let q = [corner(-HALF_W, -HALF_L), corner(HALF_W, -HALF_L), corner(HALF_W, HALF_L), corner(-HALF_W, HALF_L)];
    // Kept inside ArrowWindow whichever way it turns: its outline's middle on the window's middle.
    let (lo, hi) = q.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| {
        ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
    });
    let shift = [centre[0] - (lo[0] + hi[0]) / 2.0, centre[1] - (lo[1] + hi[1]) / 2.0];
    q.map(|p| [p[0] + shift[0], p[1] + shift[1]])
}

/// Round 20 (Q8): metres per step, and how far from the walk graph a point still joins it.
const STEP: f32 = 0.75;
const NAV_REACH: f32 = 12.0;

/// Wizard101's walk graph of the street (quest_nav.tsv: "N x z" nodes in MSB metres, "E a b" edges).
#[derive(Default)]
pub struct Nav {
    nodes: Vec<[f32; 2]>,
    edges: Vec<Vec<(usize, f32)>>,
}

impl Nav {
    pub fn load(path: &Path) -> Nav {
        let mut nav = Nav::default();
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match (f.first().copied(), f.get(1).and_then(|v| v.parse::<f32>().ok()), f.get(2).and_then(|v| v.parse::<f32>().ok())) {
                (Some("N"), Some(x), Some(z)) => nav.nodes.push([x, z]),
                (Some("E"), Some(a), Some(b)) => {
                    let (a, b) = (a as usize, b as usize);
                    if a < nav.nodes.len() && b < nav.nodes.len() {
                        nav.edges.resize(nav.nodes.len(), Vec::new());
                        let d = dist(nav.nodes[a], nav.nodes[b]);
                        nav.edges[a].push((b, d));
                        nav.edges[b].push((a, d));
                    }
                }
                _ => {}
            }
        }
        tracing::info!("quest helper: walk graph {} nodes from {}", nav.nodes.len(), path.display());
        nav
    }

    fn nearest(&self, p: [f32; 2]) -> Option<(usize, f32)> {
        self.nodes.iter().enumerate().map(|(i, n)| (i, dist(*n, p))).min_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, d)| *d <= NAV_REACH)
    }

    /// The walk from `a` to `b` along the graph (metres), or None when either end is off it or no path joins them.
    pub fn walk(&self, a: [f32; 2], b: [f32; 2]) -> Option<f32> {
        let ((start, da), (goal, db)) = (self.nearest(a)?, self.nearest(b)?);
        let mut best = vec![f32::INFINITY; self.nodes.len()];
        let mut heap = BinaryHeap::new();
        best[start] = 0.0;
        heap.push((std::cmp::Reverse((0.0f32).to_bits()), start));
        while let Some((std::cmp::Reverse(bits), i)) = heap.pop() {
            let d = f32::from_bits(bits);
            if i == goal {
                return Some(da + d + db);
            }
            if d > best[i] {
                continue;
            }
            for &(j, w) in self.edges.get(i).map(|e| e.as_slice()).unwrap_or(&[]) {
                if d + w < best[j] {
                    best[j] = d + w;
                    heap.push((std::cmp::Reverse((d + w).to_bits()), j));
                }
            }
        }
        None
    }
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Round 20 (Q8): the steps from the player to the goal (MSB positions): the walk along the graph, else straight.
pub fn steps(nav: &Nav, player: [f32; 3], goal: [f32; 3]) -> u32 {
    let (a, b) = ([player[0], player[2]], [goal[0], goal[2]]);
    let metres = nav.walk(a, b).unwrap_or_else(|| dist(a, b));
    (metres / STEP).round() as u32
}

/// QuestHelperHud's view: the goal's name in txtGoalName, the steps to it in txtDistance (round 20), no button.
pub fn view(layout: &Layout, active: &Active, steps: u32) -> View {
    let mut v = View::default();
    v.set(layout.find("txtGoalName"), format!("<center>{}</center>", active.goal));
    v.set(layout.find("txtDistance"), format!("<center>{steps}</center>"));
    v.show(layout.find("btnQuestHelper"), false);
    v
}

/// Draws the arrow (screen pixels: `origin` and scale `s` from placement).
pub fn draw_arrow(draw: &DrawListMut, textures: &Textures, layout: &Layout, bearing: f32, origin: [f32; 2], s: f32) {
    let Some((texture, _)) = textures.get(ARROW) else { return };
    let q = arrow_quad(layout, bearing).map(|p| [origin[0] + p[0] * s, origin[1] + p[1] * s]);
    draw.add_image_quad(texture, q[0], q[1], q[2], q[3])
        .uv([0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0])
        .col(ImColor32::WHITE)
        .build();
}
