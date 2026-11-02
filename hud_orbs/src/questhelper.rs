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
/// Round 22 (H2): Wizard101's solid arrow model (tools/w101_quest_arrow.py), under the book folder.
pub const MESH: &str = "Quest/WC_QuestArrow.tsv";
pub const SCREEN_H: f32 = 768.0;
/// Round 22 (H1, Ryan: "the quest description needs to be moved way down, right above the level bar"): the gap
/// (layout pixels) between txtGoalName's bottom and the experience bar's top.
const BAR_GAP: f32 = 4.0;
/// The tile of the street (m60_42_36_00) and how far from its corner the helper still shows (metres).
const TILE: [i32; 2] = [42, 36];
const TILE_RANGE: f32 = 600.0;
/// Below this MSB height the player is in one of the rooms under the street (the street is at about 94 m).
const ROOM_HEIGHT: f32 = 70.0;
/// Quest_Arrow_JT.nif's quad (the fallback when WC_QuestArrow.tsv is missing): half width and half length (units).
const HALF_W: f32 = 21.23;
const HALF_L: f32 = 35.0;
/// Round 22 (H2): the little camera ArrowWindow looks at the arrow with: pitched ELEVATION down onto it from
/// DISTANCE units away, FOCAL layout pixels per unit at depth 1 (side on, the 62-unit arrow is about 110 layout
/// pixels long, most of ArrowWindow's 150).
const ELEVATION: f32 = 0.70;
const DISTANCE: f32 = 170.0;
const FOCAL: f32 = 300.0;
/// The light (camera space: right, up, toward the arrow; from the upper left in front) and the ink outline's width
/// (layout pixels), as Wizard101 draws its toon-shaded models with a black edge.
const LIGHT: [f32; 3] = [-0.38, 0.80, -0.46];
const OUTLINE: f32 = 1.3;
/// The sign that culls back faces with the outward winding `ArrowMesh::load` sets (checked in the preview: the
/// arrow's top, not its underside, shows).
const SIDE: f32 = 1.0;

// tools/w101_quest_targets.py m60_42_36_00.msb.dcx (msb 17b1dd61): (quest, goal index from 0, MSB points, the
// room's door on the street for goals inside a room).
const TARGETS: &[(&str, usize, &[[f32; 3]], Option<[f32; 3]>)] = &[
    ("unicorn_way", 0, &[[5.003, 94.183, -44.078]], None),
    ("ghost_hunters", 0, &[[-106.838, 93.988, -41.934], [-125.000, 93.999, -83.660], [-143.009, 93.999, -43.909], [-153.779, 92.499, -65.799], [-112.926, 93.999, -41.266], [-122.484, 93.999, -47.028], [-118.689, 93.999, -55.466], [-110.309, 93.999, -47.553], [-121.378, 93.999, -93.675], [-114.960, 93.999, -90.218], [-120.229, 93.999, -79.554], [-125.000, 93.999, -71.722], [-146.090, 93.999, -31.099], [-151.042, 93.999, -18.986], [-156.601, 93.999, -21.262], [-152.333, 93.999, -29.959], [-144.529, 92.499, -69.751], [-140.999, 92.520, -68.820], [-141.858, 92.499, -60.844], [-149.830, 92.499, -53.881]], None),
    ("ghost_hunters", 1, &[[5.003, 94.183, -44.078]], None),
    ("ghost_hunters", 2, &[[-84.838, 94.488, -18.448]], None),
    ("skeleton_crew", 0, &[[-174.706, 93.865, -7.295], [-224.319, 93.999, -14.996], [-233.068, 93.999, -38.080], [-211.351, 92.499, -48.532], [-191.699, 93.999, -14.760], [-200.160, 93.999, -19.839], [-197.825, 93.999, -26.461], [-188.622, 93.999, -20.708], [-226.537, 93.999, -2.599], [-231.498, 93.999, 5.621], [-237.135, 93.999, 0.639], [-233.131, 93.999, -5.810], [-246.934, 93.997, -42.301], [-253.552, 93.997, -46.452], [-250.329, 93.997, -54.021], [-242.953, 93.997, -48.753], [-206.137, 92.499, -46.495], [-202.871, 92.527, -37.287], [-214.245, 92.499, -37.220], [-220.472, 92.499, -43.283]], None),
    ("skeleton_crew", 1, &[[-84.838, 94.488, -18.448]], None),
    ("monsters_and_mazes", 0, &[[-336.103, 94.299, 105.505]], None),
    ("monsters_and_mazes", 1, &[[-336.103, 93.499, 105.505]], None),
    ("finding_out_why", 0, &[[-284.496, 93.000, -57.482], [-268.677, 93.000, -48.272], [-325.141, 93.999, -26.388], [-332.026, 93.999, -13.127]], None),
    ("finding_out_why", 1, &[[-278.594, 93.997, -44.062], [-289.062, 92.999, 3.035], [-281.735, 93.997, -30.283], [-284.642, 93.999, -20.763], [-290.663, 93.999, -22.944], [-287.072, 93.999, -32.296], [-303.581, 92.992, -1.351], [-320.420, 93.999, -12.637], [-316.935, 93.999, -18.455], [-299.868, 92.999, -9.293]], None),
    ("finding_out_why", 2, &[[-336.103, 94.299, 105.505]], None),
    ("finding_out_why", 3, &[[-336.103, 93.499, 105.505]], None),
    ("who_could_it_be_now", 0, &[[-84.838, 94.488, -18.448]], None),
    ("sinister_skeleton", 0, &[[-242.629, 37.999, 20.072]], Some([-240.829, 94.349, 16.343])),
    ("sinister_skeleton", 1, &[[-242.425, 33.999, 19.671]], Some([-240.829, 94.349, 16.343])),
    ("sinister_skeleton", 2, &[[-84.838, 94.999, -18.448]], None),
    ("sinister_skeleton", 3, &[[-84.838, 94.488, -18.448]], None),
    // Round 23 (Q3): the side quests' targets are gone with the quests (w101_quests.REMOVED_QUESTS).
];

/// Round 22 (H3): the hedge maze garden is not under the street like the rooms but beside it, behind a door that
/// teleports (m60_42_36_00.msb: W101_Garden_out 1042362807 on the street, W101_Garden_in 1042362805 inside; Lady
/// Oriel 1042360823 in its middle). A point within GARDEN_RADIUS of GARDEN_CENTRE (the middle of the arrival point
/// and Oriel; the street's walk graph comes no nearer than 75 m) is in the garden.
const GARDEN_CENTRE: [f32; 2] = [-327.62, 88.90];
const GARDEN_RADIUS: f32 = 40.0;
const GARDEN_DOOR: [f32; 3] = [-356.827, 95.679, -34.576];
const GARDEN_INSIDE: [f32; 3] = [-320.138, 95.679, 74.260];

pub fn in_garden(p: [f32; 3]) -> bool {
    dist([p[0], p[2]], GARDEN_CENTRE) < GARDEN_RADIUS
}

/// Below ROOM_HEIGHT the player is in one of the rooms under the street.
pub fn in_room(p: [f32; 3]) -> bool {
    p[1] < ROOM_HEIGHT
}

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

/// Round 22 S1: the player's MSB position from the loaded street NPCs (player_msb_from_npcs), when it is on the tile.
pub fn player_msb_now(game: &crate::game::Game) -> Option<[f32; 3]> {
    let camera = game.camera()?;
    let entities = game.entities(&crate::markers::MODELS, &crate::markers::entity_ids());
    player_msb_from_npcs(&camera, &entities).filter(|p| p[0].abs() < TILE_RANGE && p[2].abs() < TILE_RANGE)
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
    let goal = points.iter().min_by(|a, b| d2(a).total_cmp(&d2(b))).copied()?;
    // Round 22 (H3, Ryan: "it points to where the hedge maze is on the map, not where the door is"): a goal in the
    // garden from the street points at the garden's door on the street; a street goal from inside the garden at
    // the garden's own door (where its arrival point is).
    Some(match (in_garden(goal), in_garden(player)) {
        (true, false) => GARDEN_DOOR,
        (false, true) => GARDEN_INSIDE,
        _ => goal,
    })
}

/// The goal's bearing from the camera's view direction (radians, clockwise on the ground, 0 = straight ahead).
pub fn bearing(camera: &Camera, player: [f32; 3], goal: [f32; 3]) -> f32 {
    let (dx, dz) = (goal[0] - player[0], goal[2] - player[2]);
    let m = &camera.matrix;
    let ahead = dx * m[2][0] + dz * m[2][2];
    let right = dx * m[0][0] + dz * m[0][2];
    right.atan2(ahead)
}

/// The helper's scale (screen pixels per layout pixel) and its top-left corner on a `screen`. Round 22 (H1): docked
/// to the bottom: txtGoalName's bottom sits BAR_GAP over `bar_top` (the experience bar's top, screen pixels;
/// talk::xp_bar_top); the arrow (ArrowWindow) is right over the goal's name as in QuestHelperHud.gui.
pub fn placement(layout: &Layout, screen: [f32; 2], bar_top: f32) -> (f32, [f32; 2]) {
    let s = screen[1] / SCREEN_H;
    let w = layout.nodes.first().map(|n| n.rect[2] - n.rect[0]).unwrap_or(600.0);
    let name_bottom = layout.find("txtGoalName").map(|i| layout.nodes[i].rect[3]).unwrap_or(150.0);
    (s, [(screen[0] - w * s) / 2.0, bar_top - (name_bottom + BAR_GAP) * s])
}

/// Round 22 (H2): the point the arrow turns about and the steps are centred on (layout pixels): txtDistance's
/// middle, which QuestHelperHud.gui centres across like ArrowWindow. Round 21 moved the arrow's outline middle onto
/// the window's middle every frame, so its pivot (and the number on it) wandered as it turned.
pub fn pivot(layout: &Layout) -> [f32; 2] {
    let r = layout.find("txtDistance").or_else(|| layout.find("ArrowWindow")).map(|i| layout.nodes[i].rect)
        .unwrap_or([110.0, 73.0, 489.0, 97.0]);
    [(r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0]
}

/// One point of the arrow model (units: forward, up, right on the ground) turned by `bearing`, in the little
/// camera's space (right, up, depth).
fn view_space(forward: f32, up: f32, right: f32, bearing: f32) -> [f32; 3] {
    let (sin, cos) = bearing.sin_cos();
    let (r, f) = (right * cos + forward * sin, -right * sin + forward * cos);
    let (se, ce) = ELEVATION.sin_cos();
    [r, up * ce + f * se, DISTANCE + f * ce - up * se]
}

fn to_layout(p: [f32; 3], pivot: [f32; 2]) -> [f32; 2] {
    [pivot[0] + p[0] / p[2] * FOCAL, pivot[1] - p[1] / p[2] * FOCAL]
}

/// The flat arrow quad's corners in layout pixels (tail left, tail right, tip right, tip left: the texture's (0,0),
/// (1,0), (1,1), (0,1)), turned by `bearing` about the pivot: the fallback without WC_QuestArrow.tsv.
pub fn arrow_quad(layout: &Layout, bearing: f32) -> [[f32; 2]; 4] {
    let c = pivot(layout);
    let corner = |x: f32, y: f32| to_layout(view_space(y, 0.0, x, bearing), c);
    [corner(-HALF_W, -HALF_L), corner(HALF_W, -HALF_L), corner(HALF_W, HALF_L), corner(-HALF_W, HALF_L)]
}

/// Round 22 (H2, Ryan: "it looks 2D; make sure it looks like it does in Wizard101"): Wizard101's solid quest arrow,
/// _Shared-WorldData.wad Character/Arrow/WC_QuestArrow.nif (tools/w101_quest_arrow.py -> book/Quest/
/// WC_QuestArrow.tsv): a bevelled arrow 62 units long (tip at +x), 36 wide (z), 11 thick (y), 104 triangles, its UVs
/// on FX_Quest_Arrow_JT (Quest/Quest_Arrow.png).
#[derive(Default)]
pub struct ArrowMesh {
    /// x, y, z, u, v
    vertices: Vec<[f32; 5]>,
    triangles: Vec<[usize; 3]>,
    /// Which side of a face (by its winding) faces the camera: -1 or 1 (see `faces`).
    side: f32,
}

impl ArrowMesh {
    pub fn load(path: &Path) -> ArrowMesh {
        let mut mesh = ArrowMesh::default();
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let num = |k: usize| f.get(k).and_then(|v| v.parse::<f32>().ok());
            match f.first().copied() {
                Some("V") => {
                    if let (Some(x), Some(y), Some(z), Some(u), Some(v)) = (num(1), num(2), num(3), num(4), num(5)) {
                        mesh.vertices.push([x, y, z, u, v]);
                    }
                }
                Some("T") => {
                    if let (Some(a), Some(b), Some(c)) = (num(1), num(2), num(3)) {
                        mesh.triangles.push([a as usize, b as usize, c as usize]);
                    }
                }
                _ => {}
            }
        }
        let n = mesh.vertices.len();
        mesh.triangles.retain(|t| t.iter().all(|&i| i < n));
        // Outward faces: the strip's winding turned so the enclosed volume comes out positive.
        let volume: f32 = mesh.triangles.iter().map(|t| {
            let [a, b, c] = t.map(|i| [mesh.vertices[i][0], mesh.vertices[i][1], mesh.vertices[i][2]]);
            a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
        }).sum();
        if volume < 0.0 {
            for t in &mut mesh.triangles {
                t.swap(1, 2);
            }
        }
        mesh.side = SIDE;
        tracing::info!("quest helper: arrow model {} vertices, {} triangles from {}", n, mesh.triangles.len(),
                       path.display());
        mesh
    }

    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// The faces the little camera sees, far ones first: (layout corners, uvs, shade 0..1).
    pub fn faces(&self, layout: &Layout, bearing: f32) -> Vec<([[f32; 2]; 3], [[f32; 2]; 3], f32)> {
        let c = pivot(layout);
        let at: Vec<[f32; 3]> = self.vertices.iter().map(|v| view_space(v[0], v[1], v[2], bearing)).collect();
        let light = {
            let l = (LIGHT[0] * LIGHT[0] + LIGHT[1] * LIGHT[1] + LIGHT[2] * LIGHT[2]).sqrt();
            LIGHT.map(|v| v / l)
        };
        let mut out: Vec<(f32, [[f32; 2]; 3], [[f32; 2]; 3], f32)> = Vec::new();
        for t in &self.triangles {
            let [a, b, cc] = t.map(|i| at[i]);
            let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]]);
            // The view space is left-handed (x right, y up, z away): flip the cross product to keep it outward.
            let n = [-(u[1] * v[2] - u[2] * v[1]), -(u[2] * v[0] - u[0] * v[2]), -(u[0] * v[1] - u[1] * v[0])];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len < 1e-6 {
                continue;
            }
            let centre = [(a[0] + b[0] + cc[0]) / 3.0, (a[1] + b[1] + cc[1]) / 3.0, (a[2] + b[2] + cc[2]) / 3.0];
            let facing = n[0] * centre[0] + n[1] * centre[1] + n[2] * centre[2];
            if facing * self.side >= 0.0 {
                continue; // facing away
            }
            // Turned toward the camera for the light.
            let n = n.map(|x| x / len * -facing.signum());
            let lit = n[0] * light[0] + n[1] * light[1] + n[2] * light[2];
            let shade = 0.55 + 0.45 * lit.max(0.0);
            let uv = t.map(|i| [self.vertices[i][3], self.vertices[i][4]]);
            out.push((centre[2], [a, b, cc].map(|p| to_layout(p, c)), uv, shade));
        }
        out.sort_by(|x, y| y.0.total_cmp(&x.0));
        out.into_iter().map(|f| (f.1, f.2, f.3)).collect()
    }
}

/// Round 20 (Q8): metres per step, and how far from the walk graph a point still joins it.
const STEP: f32 = 0.75;
const NAV_REACH: f32 = 12.0;

/// Wizard101's walk graph of the street (quest_nav.tsv: "N x z" nodes in MSB metres, "E a b" edges).
#[derive(Default)]
pub struct Nav {
    nodes: Vec<[f32; 2]>,
    edges: Vec<Vec<(usize, f32)>>,
    /// Round 22 (H6): the street seen from above on a CELL grid: 0 = outside (walls, buildings, the hedges: off the
    /// walk graph and joined to the edge of the map), 1 = open (on the graph, or a hole in it such as a fountain or a
    /// lamp post's base, which hides nothing as tall as a marker).
    grid: Vec<u8>,
    grid_origin: [f32; 2],
    grid_size: [usize; 2],
}

/// Round 22 (H6): grid cell (metres); a point within OPEN_REACH of a walk node is open (the nodes are 4.8 m apart,
/// so every point between four of them is within 3.4 m).
const CELL: f32 = 1.2;
const OPEN_REACH: f32 = 3.6;

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
        nav.build_grid();
        tracing::info!("quest helper: walk graph {} nodes from {} (sight grid {}x{})", nav.nodes.len(), path.display(),
                       nav.grid_size[0], nav.grid_size[1]);
        nav
    }

    fn build_grid(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        let margin = 10.0;
        let lo = self.nodes.iter().fold([f32::MAX; 2], |m, n| [m[0].min(n[0]), m[1].min(n[1])]);
        let hi = self.nodes.iter().fold([f32::MIN; 2], |m, n| [m[0].max(n[0]), m[1].max(n[1])]);
        self.grid_origin = [lo[0] - margin, lo[1] - margin];
        let size = [((hi[0] - lo[0] + 2.0 * margin) / CELL) as usize + 1, ((hi[1] - lo[1] + 2.0 * margin) / CELL) as usize + 1];
        self.grid_size = size;
        // 2 = near a node, 0 = outside (filled from the border), what stays 1 is a hole inside the street.
        let mut grid = vec![1u8; size[0] * size[1]];
        let r = (OPEN_REACH / CELL).ceil() as i64;
        for n in &self.nodes {
            let (cx, cz) = (((n[0] - self.grid_origin[0]) / CELL) as i64, ((n[1] - self.grid_origin[1]) / CELL) as i64);
            for z in cz - r..=cz + r {
                for x in cx - r..=cx + r {
                    if x < 0 || z < 0 || x >= size[0] as i64 || z >= size[1] as i64 {
                        continue;
                    }
                    let centre = [self.grid_origin[0] + (x as f32 + 0.5) * CELL, self.grid_origin[1] + (z as f32 + 0.5) * CELL];
                    if dist(centre, *n) <= OPEN_REACH {
                        grid[z as usize * size[0] + x as usize] = 2;
                    }
                }
            }
        }
        let mut stack: Vec<usize> = Vec::new();
        for x in 0..size[0] {
            stack.extend([x, (size[1] - 1) * size[0] + x]);
        }
        for z in 0..size[1] {
            stack.extend([z * size[0], z * size[0] + size[0] - 1]);
        }
        while let Some(i) = stack.pop() {
            if grid[i] != 1 {
                continue;
            }
            grid[i] = 0;
            let (x, z) = (i % size[0], i / size[0]);
            if x > 0 { stack.push(i - 1); }
            if x + 1 < size[0] { stack.push(i + 1); }
            if z > 0 { stack.push(i - size[0]); }
            if z + 1 < size[1] { stack.push(i + size[0]); }
        }
        self.grid = grid;
    }

    /// Whether a point is open street (None off the grid or without a walk graph).
    fn open(&self, p: [f32; 2]) -> Option<bool> {
        let (x, z) = ((p[0] - self.grid_origin[0]) / CELL, (p[1] - self.grid_origin[1]) / CELL);
        if self.grid.is_empty() || x < 0.0 || z < 0.0 || x >= self.grid_size[0] as f32 || z >= self.grid_size[1] as f32 {
            return None;
        }
        Some(self.grid[z as usize * self.grid_size[0] + x as usize] != 0)
    }

    /// Round 22 (H6, Ryan: "I shouldn't be able to see the question mark and exclamation points through walls"):
    /// whether the street lets the player at `a` see `b` (MSB x, z): false when the straight line crosses outside the
    /// walk graph (a wall, a house, the hedges). Elden Ring has no public ray cast for this game version, so the
    /// street's own walk area stands in for its walls. The first stretch while the player stands off the graph
    /// (steps, a doorway) and the last END metres before the NPC (who may stand at the street's edge) don't count.
    pub fn sees(&self, a: [f32; 2], b: [f32; 2]) -> bool {
        const STEP: f32 = 0.8;
        const END: f32 = 2.0;
        let length = dist(a, b);
        let n = (length / STEP).ceil().max(1.0) as usize;
        let mut started = false;
        for i in 1..n {
            let t = i as f32 / n as f32;
            if length * (1.0 - t) < END {
                break;
            }
            let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            match self.open(p) {
                None => return true,
                Some(true) => started = true,
                Some(false) if started => return false,
                Some(false) => {}
            }
        }
        true
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

/// Draws the arrow (screen pixels: `origin` and scale `s` from placement): Wizard101's solid arrow model when
/// loaded (an ink outline, then its faces lit from the upper left), else the flat quad.
pub fn draw_arrow(draw: &DrawListMut, textures: &Textures, layout: &Layout, mesh: &ArrowMesh, bearing: f32,
                  origin: [f32; 2], s: f32, alpha: f32) {
    let Some((texture, _)) = textures.get(ARROW) else { return };
    let screen = |p: [f32; 2]| [origin[0] + p[0] * s, origin[1] + p[1] * s];
    let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
    if mesh.is_empty() {
        let q = arrow_quad(layout, bearing).map(screen);
        draw.add_image_quad(texture, q[0], q[1], q[2], q[3])
            .uv([0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0])
            .col(ImColor32::from_rgba(255, 255, 255, a))
            .build();
        return;
    }
    let faces = mesh.faces(layout, bearing);
    let ink = ImColor32::from_rgba(0, 0, 0, a);
    let o = OUTLINE * s;
    for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o), (-0.7 * o, -0.7 * o), (0.7 * o, -0.7 * o),
                     (-0.7 * o, 0.7 * o), (0.7 * o, 0.7 * o)] {
        for (p, _, _) in &faces {
            let [p0, p1, p2] = p.map(|q| { let q = screen(q); [q[0] + dx, q[1] + dy] });
            draw.add_triangle(p0, p1, p2, ink).filled(true).build();
        }
    }
    for (p, uv, shade) in &faces {
        let [p0, p1, p2] = p.map(screen);
        let k = (shade * 255.0) as u8;
        draw.add_image_quad(texture, p0, p1, p2, p2)
            .uv(uv[0], uv[1], uv[2], uv[2])
            .col(ImColor32::from_rgba(k, k, k, a))
            .build();
    }
}
