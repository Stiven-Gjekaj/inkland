//! Label placement.
//!
//! Each label has a list of candidate positions. The placer takes the labels
//! in a fixed order and gives each one the candidate with the lowest cost
//! that does not touch a placed label, a place symbol, an extra, or the edge
//! of the map. A label without such a candidate is left out and goes in the
//! report. Placed labels never overlap.
//!
//! - Place labels try eight positions around the symbol.
//! - Area labels (states and named regions) go on the cells farthest from
//!   the edge of the area. The letters are spaced to fill part of the
//!   width of the area.
//! - River labels follow a straight part of the river, above the line.
//!
//! A label that crosses a river costs more, so labels keep off rivers where
//! they can. All sizes are in page millimetres.

use crate::metrics::FontMetrics;
use crate::page::{Page, pt};
use inkland_core::config::{Config, PlaceKind, Variant};
use inkland_core::geom::Shape;
use inkland_core::world::World;
use std::collections::{BTreeMap, VecDeque};

/// Free space around the text, for the halo.
const PAD: f64 = 0.3;
/// Width of the white halo behind each label.
pub const HALO_MM: f64 = 0.6;
/// Extra cost of a label for each river that it crosses.
const RIVER_COST: f64 = 0.5;
/// Size of the buckets of the spatial index.
const BUCKET: f64 = 5.0;

/// A rectangle that can turn: centre `c`, unit axis `u` along the text, and
/// half sizes `h` along `u` and across it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Obb {
    pub c: [f64; 2],
    pub u: [f64; 2],
    pub h: [f64; 2],
}

impl Obb {
    /// A rectangle with sides along the page axes.
    pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Obb {
        Obb {
            c: [(x0 + x1) / 2.0, (y0 + y1) / 2.0],
            u: [1.0, 0.0],
            h: [(x1 - x0).abs() / 2.0, (y1 - y0).abs() / 2.0],
        }
    }

    fn v(&self) -> [f64; 2] {
        [-self.u[1], self.u[0]]
    }

    pub fn corners(&self) -> [[f64; 2]; 4] {
        let [ux, uy] = [self.u[0] * self.h[0], self.u[1] * self.h[0]];
        let v = self.v();
        let [vx, vy] = [v[0] * self.h[1], v[1] * self.h[1]];
        let [cx, cy] = self.c;
        [
            [cx - ux - vx, cy - uy - vy],
            [cx + ux - vx, cy + uy - vy],
            [cx + ux + vx, cy + uy + vy],
            [cx - ux + vx, cy - uy + vy],
        ]
    }

    fn project(points: &[[f64; 2]], axis: [f64; 2]) -> (f64, f64) {
        points
            .iter()
            .map(|p| p[0] * axis[0] + p[1] * axis[1])
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                (a.min(x), b.max(x))
            })
    }

    /// `true` if the two rectangles share an area. Rectangles that only
    /// touch do not overlap.
    pub fn overlaps(&self, o: &Obb) -> bool {
        let (a, b) = (self.corners(), o.corners());
        [self.u, self.v(), o.u, o.v()].into_iter().all(|axis| {
            let (a0, a1) = Obb::project(&a, axis);
            let (b0, b1) = Obb::project(&b, axis);
            a1 > b0 + 1e-9 && b1 > a0 + 1e-9
        })
    }

    /// `true` if the segment from `p` to `q` goes through the rectangle.
    pub fn crosses(&self, p: [f64; 2], q: [f64; 2]) -> bool {
        let a = self.corners();
        let seg = [p, q];
        let d = [q[0] - p[0], q[1] - p[1]];
        let normal = [-d[1], d[0]];
        [self.u, self.v(), normal].into_iter().all(|axis| {
            let (a0, a1) = Obb::project(&a, axis);
            let (b0, b1) = Obb::project(&seg, axis);
            a1 >= b0 && b1 >= a0
        })
    }

    /// The bounding box `[x0, y0, x1, y1]`.
    pub fn bounds(&self) -> [f64; 4] {
        let c = self.corners();
        let (x0, x1) = Obb::project(&c, [1.0, 0.0]);
        let (y0, y1) = Obb::project(&c, [0.0, 1.0]);
        [x0, y0, x1, y1]
    }

    fn inside(&self, r: [f64; 4]) -> bool {
        let [x0, y0, x1, y1] = self.bounds();
        x0 >= r[0] && y0 >= r[1] && x1 <= r[2] && y1 <= r[3]
    }
}

/// Items in square buckets, for a fast search by area.
#[derive(Default)]
struct Index {
    buckets: BTreeMap<(i64, i64), Vec<usize>>,
}

impl Index {
    fn keys(b: [f64; 4]) -> impl Iterator<Item = (i64, i64)> {
        let k = |v: f64| (v / BUCKET).floor() as i64;
        let (x0, y0, x1, y1) = (k(b[0]), k(b[1]), k(b[2]), k(b[3]));
        (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
    }

    fn insert(&mut self, item: usize, b: [f64; 4]) {
        for key in Index::keys(b) {
            self.buckets.entry(key).or_default().push(item);
        }
    }

    /// The items near a box, each once, in increasing order.
    fn near(&self, b: [f64; 4]) -> Vec<usize> {
        let mut out: Vec<usize> = Index::keys(b)
            .filter_map(|k| self.buckets.get(&k))
            .flatten()
            .copied()
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// Where the text goes.
#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    /// Text at a point, with an SVG `text-anchor`.
    Point {
        x: f64,
        y: f64,
        anchor: &'static str,
        /// Turn of the text about `(x, y)`, in degrees: 0, or -90 for text
        /// that reads from bottom to top.
        angle: i32,
    },
    /// Text along a line, from its first point to its last. The text is in
    /// the middle of the line, `offset` above it.
    Path { points: Vec<[f64; 2]>, offset: f64 },
}

/// The kinds of labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Place,
    State,
    Region,
    River,
    Feature,
}

/// A placed label.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub id: String,
    pub kind: Kind,
    /// The style group of the label: `capital`, `city`, `town`, `village`,
    /// `fort`, `hamlet`, `river`, `lake`, `ocean`, `state`, `region`, or
    /// `feature`.
    pub group: &'static str,
    pub text: String,
    pub family: String,
    /// Font size, in mm.
    pub size: f64,
    /// Space between letters, in mm.
    pub spacing: f64,
    pub geometry: Geometry,
    pub boxes: Vec<Obb>,
}

/// A label that has no place.
#[derive(Debug, Clone, PartialEq)]
pub struct Unplaced {
    pub id: String,
    /// `collision` or `outside`.
    pub reason: &'static str,
    /// `true` for the labels of pins, capitals, and states.
    pub required: bool,
}

/// The result of label placement.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Labels {
    pub placed: Vec<Placed>,
    pub unplaced: Vec<Unplaced>,
}

impl Labels {
    /// `true` if a point is within `margin` of the box of a placed label.
    pub fn covers(&self, p: [f64; 2], margin: f64) -> bool {
        self.placed.iter().flat_map(|l| &l.boxes).any(|b| {
            let d = [p[0] - b.c[0], p[1] - b.c[1]];
            let along = (d[0] * b.u[0] + d[1] * b.u[1]).abs();
            let across = (-d[0] * b.u[1] + d[1] * b.u[0]).abs();
            along <= b.h[0] + margin && across <= b.h[1] + margin
        })
    }

    /// The number of required labels without a place.
    pub fn required_unplaced(&self) -> usize {
        self.unplaced.iter().filter(|u| u.required).count()
    }
}

struct Candidate {
    geometry: Geometry,
    boxes: Vec<Obb>,
    spacing: f64,
    cost: f64,
    /// The river that the label is for. A label does not cost more for
    /// crossing its own river.
    river: Option<usize>,
}

struct Request {
    id: String,
    kind: Kind,
    group: &'static str,
    text: String,
    family: String,
    size: f64,
    required: bool,
    /// Lower ranks are placed first.
    rank: u8,
    candidates: Vec<Candidate>,
}

/// The half size of the symbol of a place, in mm.
fn symbol_radius(kind: PlaceKind, capital: bool) -> f64 {
    if capital {
        return 1.45;
    }
    match kind {
        PlaceKind::City => 1.0,
        PlaceKind::Town => 0.75,
        PlaceKind::Village => 0.5,
        PlaceKind::Fortress | PlaceKind::Custom => 0.85,
        PlaceKind::Ruin => 0.7,
    }
}

/// The box of a text with its baseline at `y` and its anchor at `x`.
fn text_box(x: f64, y: f64, anchor: &str, w: f64, f: &FontMetrics, size: f64) -> Obb {
    let x0 = match anchor {
        "end" => x - w,
        "middle" => x - w / 2.0,
        _ => x,
    };
    Obb::rect(
        x0 - PAD,
        y - f.cap(size) - PAD,
        x0 + w + PAD,
        y + 0.6 * f.descent(size) + PAD,
    )
}

/// Eight positions around a place symbol, best first.
fn point_candidates(
    at: [f64; 2],
    r: f64,
    text: &str,
    f: &FontMetrics,
    size: f64,
) -> Vec<Candidate> {
    let w = f.width(text, size, 0.0);
    let (ch, dd) = (f.cap(size), 0.6 * f.descent(size));
    // Clear of the symbol box at the side, above, and below.
    let side = r + PAD + 0.2;
    let above = at[1] - r - 0.2 - PAD - dd;
    let below = at[1] + r + 0.2 + PAD + ch;
    let [x, y] = at;
    let half = 0.5 * side;
    let spots: [(f64, f64, &'static str); 8] = [
        (x + side, y + ch / 2.0, "start"),
        (x + half, above, "start"),
        (x + half, below, "start"),
        (x - side, y + ch / 2.0, "end"),
        (x - half, above, "end"),
        (x - half, below, "end"),
        (x, above, "middle"),
        (x, below, "middle"),
    ];
    spots
        .into_iter()
        .enumerate()
        .map(|(i, (bx, by, anchor))| Candidate {
            boxes: vec![text_box(bx, by, anchor, w, f, size)],
            geometry: Geometry::Point {
                x: bx,
                y: by,
                anchor,
                angle: 0,
            },
            spacing: 0.0,
            cost: 0.1 * i as f64,
            river: None,
        })
        .collect()
}

/// The depth of each cell of an area: 1 on its edge, more inside, 0 for
/// cells outside the area.
fn depths(world: &World, area: &[bool]) -> Vec<u32> {
    let g = &world.grid;
    let mut depth = vec![0u32; g.len()];
    let mut queue = VecDeque::new();
    for c in 0..g.len() {
        if area[c] && (g.border[c] || g.neighbors(c).any(|m| !area[m])) {
            depth[c] = 1;
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        for m in g.neighbors(c) {
            if area[m] && depth[m] == 0 {
                depth[m] = depth[c] + 1;
                queue.push_back(m);
            }
        }
    }
    depth
}

/// The least distance between two anchors of an area label, in mm.
const ANCHOR_GAP: f64 = 3.0;
/// The most anchors of an area label.
const MAX_ANCHORS: usize = 40;

/// Candidates for the label of an area, on its deepest cells.
#[allow(clippy::too_many_arguments)]
fn area_candidates(
    world: &World,
    page: &Page,
    inner: [f64; 4],
    area: &[bool],
    text: &str,
    f: &FontMetrics,
    size: f64,
    spread: (f64, f64),
) -> Vec<Candidate> {
    let g = &world.grid;
    let depth = depths(world, area);
    let max_depth = depth.iter().copied().max().unwrap_or(0).max(1);
    let in_area = |q: [f64; 2]| {
        let w = page.to_world(q);
        let [x0, y0, x1, y1] = page.view;
        w[0] >= x0 && w[0] <= x1 && w[1] >= y0 && w[1] <= y1 && area[g.find_cell(w[0], w[1])]
    };
    let mut cells: Vec<(u32, usize)> = (0..g.len())
        .filter(|&c| depth[c] > 0)
        .filter(|&c| {
            let q = page.to_page(g.points[c]);
            q[0] >= inner[0] && q[0] <= inner[2] && q[1] >= inner[1] && q[1] <= inner[3]
        })
        .map(|c| (depth[c], c))
        .collect();
    cells.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let n = text.chars().count();
    let w0 = f.width(text, size, 0.0);
    let (ch, dd) = (f.cap(size), 0.6 * f.descent(size));
    // Anchor cells: the deepest cells first, each at least `ANCHOR_GAP` from
    // the anchors before it, so the candidates spread over the area.
    let mut anchors: Vec<(u32, [f64; 2])> = Vec::new();
    for &(d, c) in &cells {
        let q = page.to_page(g.points[c]);
        if anchors
            .iter()
            .all(|(_, a)| (a[0] - q[0]).powi(2) + (a[1] - q[1]).powi(2) >= ANCHOR_GAP * ANCHOR_GAP)
        {
            anchors.push((d, q));
            if anchors.len() == MAX_ANCHORS {
                break;
            }
        }
    }
    let mut out: Vec<Candidate> = Vec::new();
    for &(d, q) in &anchors {
        // Text along the page x axis, or upright from bottom to top.
        for (turn, axis) in [(0, [1.0, 0.0]), (-90, [0.0, -1.0])] {
            // The length of the area along the text line through the cell.
            let walk = |sign: f64| {
                let mut t = 0.0;
                for _ in 0..400 {
                    let next = t + 0.5;
                    let p = [q[0] + sign * axis[0] * next, q[1] + sign * axis[1] * next];
                    let ok = p[0] >= inner[0]
                        && p[0] <= inner[2]
                        && p[1] >= inner[1]
                        && p[1] <= inner[3];
                    if !ok || !in_area(p) {
                        break;
                    }
                    t = next;
                }
                t
            };
            let (back, ahead) = (walk(-1.0), walk(1.0));
            let run = back + ahead;
            let spaced = if n > 1 {
                ((0.7 * run - w0) / (n - 1) as f64).clamp(spread.0 * size, spread.1 * size)
            } else {
                0.0
            };
            // Spaced letters first; tight letters as a second choice.
            for (tight, spacing) in [(false, spaced), (true, spread.0 * size)] {
                if tight && (n < 2 || spacing >= spaced) {
                    continue;
                }
                let w = w0 + spacing * n.saturating_sub(1) as f64;
                // The middle of the run, moved so that the text stays in the map.
                let mid = (ahead - back) / 2.0;
                let mut centre = [q[0] + axis[0] * mid, q[1] + axis[1] * mid];
                let half = w / 2.0 + PAD;
                let (lo, hi, k) = if turn == 0 {
                    (inner[0] + half, inner[2] - half, 0)
                } else {
                    (inner[1] + half, inner[3] - half, 1)
                };
                if lo > hi {
                    continue;
                }
                centre[k] = centre[k].clamp(lo, hi);
                // The baseline: glyphs from `cap` above it to `dd` below it,
                // centred on the line through the cell.
                let lift = (ch - dd) / 2.0;
                let (bx, by) = if turn == 0 {
                    (centre[0], centre[1] + lift)
                } else {
                    (centre[0] + lift, centre[1])
                };
                let b = Obb {
                    c: centre,
                    u: axis,
                    h: [half, (ch + dd) / 2.0 + PAD],
                };
                // The part of the label outside the area costs more.
                let mut outside = 0;
                for i in 0..5 {
                    for j in 0..3 {
                        let s = -b.h[0] + 2.0 * b.h[0] * f64::from(i) / 4.0;
                        let t = -b.h[1] + b.h[1] * f64::from(j);
                        let p = [
                            b.c[0] + b.u[0] * s - b.u[1] * t,
                            b.c[1] + b.u[1] * s + b.u[0] * t,
                        ];
                        if !in_area(p) {
                            outside += 1;
                        }
                    }
                }
                // Skip a candidate that is almost the same as one before it.
                if out.iter().any(|o| match o.geometry {
                    Geometry::Point { x, y, angle, .. } => {
                        angle == turn
                            && (o.spacing - spacing).abs() < 1e-9
                            && (x - bx).abs() < 1.0
                            && (y - by).abs() < 1.0
                    }
                    Geometry::Path { .. } => false,
                }) {
                    continue;
                }
                out.push(Candidate {
                    boxes: vec![b],
                    geometry: Geometry::Point {
                        x: bx,
                        y: by,
                        anchor: "middle",
                        angle: turn,
                    },
                    spacing,
                    cost: 3.0 * f64::from(outside) / 15.0
                        + 0.5 * (1.0 - f64::from(d) / f64::from(max_depth))
                        + if turn == 0 { 0.0 } else { 0.4 }
                        + if tight { 0.3 } else { 0.0 },
                    river: None,
                });
            }
        }
    }
    out
}

/// The point at length `s` along a line with cumulative lengths `acc`.
fn point_at(pts: &[[f64; 2]], acc: &[f64], s: f64) -> [f64; 2] {
    let i = acc.partition_point(|&a| a < s).clamp(1, pts.len() - 1);
    let (a, b) = (pts[i - 1], pts[i]);
    let seg = acc[i] - acc[i - 1];
    let t = if seg > 0.0 {
        ((s - acc[i - 1]) / seg).clamp(0.0, 1.0)
    } else {
        0.0
    };
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// The part of a line from length `s0` to `s1`.
fn sub_line(pts: &[[f64; 2]], acc: &[f64], s0: f64, s1: f64) -> Vec<[f64; 2]> {
    let mut out = vec![point_at(pts, acc, s0)];
    for (i, &a) in acc.iter().enumerate() {
        if a > s0 && a < s1 {
            out.push(pts[i]);
        }
    }
    out.push(point_at(pts, acc, s1));
    out
}

/// Candidates for a river label: straight parts of the river, inside the
/// map. The text sits above the line and reads from left to right.
fn river_candidates(
    line: &[[f64; 2]],
    inner: [f64; 4],
    river: usize,
    text: &str,
    f: &FontMetrics,
    size: f64,
    spacing: f64,
) -> Vec<Candidate> {
    if line.len() < 2 {
        return Vec::new();
    }
    let mut acc = vec![0.0];
    for w in line.windows(2) {
        let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        acc.push(acc[acc.len() - 1] + d);
    }
    let total = acc[acc.len() - 1];
    let len = 1.4 * f.width(text, size, spacing);
    let (ch, dd) = (f.cap(size), 0.6 * f.descent(size));
    let gap = 0.6;
    let mut out = Vec::new();
    let mut s = 0.0;
    while s + len <= total {
        let mut pts = sub_line(line, &acc, s, s + len);
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let chord = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let straight = chord / len;
        let inside = pts
            .iter()
            .all(|p| p[0] >= inner[0] && p[0] <= inner[2] && p[1] >= inner[1] && p[1] <= inner[3]);
        if straight >= 0.8 && inside {
            if b[0] < a[0] {
                pts.reverse();
            }
            let mut pacc = vec![0.0];
            for w in pts.windows(2) {
                let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
                pacc.push(pacc[pacc.len() - 1] + d);
            }
            let plen = pacc[pacc.len() - 1];
            let parts = ((plen / (1.2 * size)).ceil() as usize).max(1);
            let boxes = (0..parts)
                .map(|i| {
                    let p = point_at(&pts, &pacc, plen * i as f64 / parts as f64);
                    let q = point_at(&pts, &pacc, plen * (i + 1) as f64 / parts as f64);
                    let l = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2))
                        .sqrt()
                        .max(1e-9);
                    let u = [(q[0] - p[0]) / l, (q[1] - p[1]) / l];
                    // Above the line on the page, for text that reads to the right.
                    let up = [u[1], -u[0]];
                    let lift = gap + (ch - dd) / 2.0;
                    Obb {
                        c: [
                            (p[0] + q[0]) / 2.0 + up[0] * lift,
                            (p[1] + q[1]) / 2.0 + up[1] * lift,
                        ],
                        u,
                        h: [l / 2.0 + PAD, (ch + dd) / 2.0 + PAD],
                    }
                })
                .collect();
            let middle = (s + len / 2.0 - total / 2.0).abs() / total;
            out.push(Candidate {
                geometry: Geometry::Path {
                    points: pts,
                    offset: gap,
                },
                boxes,
                spacing,
                cost: middle + 2.0 * (1.0 - straight),
                river: Some(river),
            });
        }
        s += 1.5;
    }
    out
}

/// The places in the view, on the page, in mm.
pub fn symbol_points(world: &World, page: &Page) -> Vec<[f64; 2]> {
    let [x0, y0, x1, y1] = page.view;
    world
        .society
        .places
        .iter()
        .filter(|p| p.at[0] >= x0 && p.at[0] <= x1 && p.at[1] >= y0 && p.at[1] <= y1)
        .map(|p| page.to_page(p.at))
        .collect()
}

/// The half size of the symbol of a point feature, in mm.
pub const FEATURE_RADIUS: f64 = 0.8;

/// The overlay of a variant, if it has one.
pub fn overlay_of<'a>(
    config: &'a Config,
    variant: &Variant,
) -> Option<&'a inkland_core::config::Overlay> {
    variant
        .overlay
        .as_deref()
        .and_then(|n| config.overlays.get(n))
}

/// The point features of the overlay of a variant that are shown, in the
/// view, on the page, in mm.
pub fn feature_points(config: &Config, variant: &Variant, page: &Page) -> Vec<[f64; 2]> {
    let [x0, y0, x1, y1] = page.view;
    overlay_of(config, variant)
        .map(|o| {
            o.add
                .features
                .iter()
                .filter(|f| !inkland_core::overlay::removes(o, "feature", &f.id))
                .filter_map(|f| f.at)
                .filter(|p| p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1)
                .map(|p| page.to_page(p))
                .collect()
        })
        .unwrap_or_default()
}

/// The boxes that labels must not touch: place symbols and extras.
fn fixed_obstacles(world: &World, config: &Config, variant: &Variant, page: &Page) -> Vec<Obb> {
    let symbols = symbol_points(world, page);
    let [x0, y0, x1, y1] = page.view;
    let mut out: Vec<Obb> = world
        .society
        .places
        .iter()
        .filter(|p| p.at[0] >= x0 && p.at[0] <= x1 && p.at[1] >= y0 && p.at[1] <= y1)
        .map(|p| {
            let [x, y] = page.to_page(p.at);
            let r = symbol_radius(p.kind, p.capital);
            Obb::rect(x - r, y - r, x + r, y + r)
        })
        .collect();
    for [x, y] in feature_points(config, variant, page) {
        let r = FEATURE_RADIUS;
        out.push(Obb::rect(x - r, y - r, x + r, y + r));
    }
    for [a, b, c, d] in crate::extras::boxes(config, variant, page, &symbols) {
        out.push(Obb::rect(a, b, c, d));
    }
    out
}

/// The look of a label group: its font family and its letter case.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub family: String,
    /// `true` for text in capital letters.
    pub upper: bool,
}

impl Look {
    /// The text of a label in this look.
    pub fn text(&self, t: &str) -> String {
        if self.upper {
            t.to_uppercase()
        } else {
            t.to_owned()
        }
    }
}

/// The label group of a place.
pub fn place_group(kind: PlaceKind, capital: bool) -> &'static str {
    if capital {
        return "capital";
    }
    match kind {
        PlaceKind::City => "city",
        PlaceKind::Town | PlaceKind::Custom => "town",
        PlaceKind::Village => "village",
        PlaceKind::Fortress => "fort",
        PlaceKind::Ruin => "hamlet",
    }
}

/// Place the labels of a variant with the fonts of the config.
pub fn place(world: &World, config: &Config, variant: &Variant, page: &Page) -> Labels {
    let fonts = &config.render.fonts;
    let style = |group: &'static str| Look {
        family: if matches!(group, "state" | "region") {
            fonts.labels_sc.clone()
        } else {
            fonts.labels.clone()
        },
        upper: false,
    };
    place_with(world, config, variant, page, &style)
}

/// Place the labels of a variant. `style` gives the look of each label
/// group.
pub fn place_with(
    world: &World,
    config: &Config,
    variant: &Variant,
    page: &Page,
    style: &dyn Fn(&'static str) -> Look,
) -> Labels {
    let sizes = &config.render.font_sizes_pt;
    let [mx, my, mw, mh] = page.map_rect();
    let inner = [mx + 0.5, my + 0.5, mx + mw - 0.5, my + mh - 0.5];
    let [vx0, vy0, vx1, vy1] = page.view;
    let in_view = |p: [f64; 2]| p[0] >= vx0 && p[0] <= vx1 && p[1] >= vy0 && p[1] <= vy1;
    let overlay = overlay_of(config, variant);
    // The label options of an item, and whether it has a label.
    let opts = |kind: &str, id: &str| {
        overlay.and_then(|o| {
            o.labels
                .iter()
                .find(|(k, _)| inkland_core::overlay::split_key(k) == Some((kind, id)))
                .map(|(_, v)| v)
        })
    };
    let shown = |kind: &str, id: &str| {
        !overlay.is_some_and(|o| inkland_core::overlay::removes(o, kind, id))
            && !opts(kind, id).is_some_and(|o| o.hide)
    };
    let size_of = |kind: &str, id: &str, default_pt: f64| {
        pt(opts(kind, id).and_then(|o| o.size_pt).unwrap_or(default_pt))
    };
    let name_of = |kind: &str, id: &str, name: Option<&str>| -> Option<String> {
        overlay
            .and_then(|o| inkland_core::overlay::renamed(o, kind, id))
            .or(name)
            .filter(|n| !n.trim().is_empty())
            .map(str::to_owned)
    };
    let mut requests: Vec<Request> = Vec::new();

    // Places.
    for p in &world.society.places {
        if p.name.trim().is_empty() || !in_view(p.at) {
            continue;
        }
        let key = p.pin.clone().unwrap_or_else(|| p.id.to_string());
        if !shown("place", &key) {
            continue;
        }
        let size = size_of(
            "place",
            &key,
            if p.capital || p.kind == PlaceKind::City {
                sizes.place_city
            } else if p.kind == PlaceKind::Village {
                sizes.place_village
            } else {
                sizes.place_town
            },
        );
        let group = place_group(p.kind, p.capital);
        let look = style(group);
        let text = look.text(&p.name);
        let f = FontMetrics::of(&look.family);
        let id = match &p.pin {
            Some(pin) => format!("label-place-{pin}"),
            None => format!("label-place-{}", p.id),
        };
        let required = p.pin.is_some() || p.capital;
        let rank = match (required, p.kind) {
            (true, _) => 0,
            (false, PlaceKind::City) => 3,
            (false, PlaceKind::Village) => 5,
            _ => 4,
        };
        requests.push(Request {
            candidates: point_candidates(
                page.to_page(p.at),
                symbol_radius(p.kind, p.capital),
                &text,
                f,
                size,
            ),
            id,
            kind: Kind::Place,
            group,
            text,
            family: look.family,
            size,
            required,
            rank,
        });
    }

    // Pinned rivers with a name.
    let mut river_lines: Vec<Vec<[f64; 2]>> = Vec::new();
    for r in &world.rivers {
        river_lines.push(r.points.iter().map(|&p| page.to_page(p)).collect());
    }
    for (i, r) in world.rivers.iter().enumerate() {
        let generated = world.names.rivers.get(i);
        // A pinned river with a name has a required label. Another river of
        // the type `river` has an optional label with its generated name.
        let (key, name, required) = match r.course {
            Some(k) => {
                let pin = &config.pins.rivers[k];
                if !shown("river", &pin.id) {
                    continue;
                }
                match name_of("river", &pin.id, pin.name.as_deref()) {
                    Some(n) => (pin.id.clone(), n, true),
                    None => match generated {
                        Some(g) if !g.name.is_empty() => (pin.id.clone(), g.name.clone(), false),
                        _ => continue,
                    },
                }
            }
            None => match generated {
                Some(g) if g.kind == "river" && !g.name.is_empty() => {
                    (r.id.to_string(), g.name.clone(), false)
                }
                _ => continue,
            },
        };
        let size = size_of("river", &key, sizes.river);
        let look = style("river");
        let name = look.text(&name);
        let f = FontMetrics::of(&look.family);
        let spacing = 0.1 * size;
        requests.push(Request {
            candidates: river_candidates(&river_lines[i], inner, i, &name, f, size, spacing),
            id: format!("label-river-{key}"),
            kind: Kind::River,
            group: "river",
            text: name,
            family: look.family,
            size,
            required,
            rank: if required { 1 } else { 6 },
        });
    }

    // Named lakes, with optional labels.
    for n in &world.names.features {
        let Some(name) = n.name.as_deref() else {
            continue;
        };
        if !matches!(n.group, "freshwater" | "frozen") {
            continue;
        }
        let g = &world.grid;
        let area: Vec<bool> = world.features.of_cell.iter().map(|&f| f == n.id).collect();
        if !(0..g.len()).any(|c| area[c] && in_view(g.points[c])) {
            continue;
        }
        let size = pt(sizes.river);
        let look = style("lake");
        let name = look.text(name);
        let f = FontMetrics::of(&look.family);
        requests.push(Request {
            candidates: area_candidates(world, page, inner, &area, &name, f, size, (0.05, 0.3)),
            id: format!("label-lake-{}", n.id),
            kind: Kind::Region,
            group: "lake",
            text: name,
            family: look.family,
            size,
            required: false,
            rank: 6,
        });
    }

    // Named oceans, seas, and gulfs, with optional labels: only in colour
    // mode, as the custom labels of the original program.
    if config.render.mode == inkland_core::config::RenderMode::Colour {
        for n in &world.names.features {
            let Some(name) = n.name.as_deref() else {
                continue;
            };
            if !matches!(n.group, "ocean" | "sea" | "gulf") {
                continue;
            }
            let g = &world.grid;
            let area: Vec<bool> = world.features.of_cell.iter().map(|&f| f == n.id).collect();
            if !(0..g.len()).any(|c| area[c] && in_view(g.points[c])) {
                continue;
            }
            let size = pt(sizes.region);
            let look = style("ocean");
            let name = look.text(name);
            let f = FontMetrics::of(&look.family);
            requests.push(Request {
                candidates: area_candidates(world, page, inner, &area, &name, f, size, (0.1, 0.6)),
                id: format!("label-water-{}", n.id),
                kind: Kind::Region,
                group: "ocean",
                text: name,
                family: look.family,
                size,
                required: false,
                rank: 6,
            });
        }
    }

    // States.
    let g = &world.grid;
    for st in &world.society.states {
        if st.name.trim().is_empty() {
            continue;
        }
        let area: Vec<bool> = (0..g.len())
            .map(|c| world.society.cell_state[c] == st.id && world.features.is_land(c))
            .collect();
        if !(0..g.len()).any(|c| area[c] && in_view(g.points[c])) {
            continue;
        }
        let size = pt(sizes.state);
        let look = style("state");
        let name = look.text(&st.name);
        let f = FontMetrics::of(&look.family);
        requests.push(Request {
            candidates: area_candidates(world, page, inner, &area, &name, f, size, (0.15, 0.8)),
            id: format!("label-state-{}", st.id),
            kind: Kind::State,
            group: "state",
            text: name,
            family: look.family,
            size,
            required: true,
            rank: 2,
        });
    }

    // Pinned regions with a name.
    for pin in &config.pins.regions {
        if !shown("region", &pin.id) {
            continue;
        }
        let Some(name) = name_of("region", &pin.id, pin.name.as_deref()) else {
            continue;
        };
        let name = name.as_str();
        let Some(shape) = Shape::of(pin) else {
            continue;
        };
        let area: Vec<bool> = (0..g.len()).map(|c| shape.contains(g.points[c])).collect();
        if !(0..g.len()).any(|c| area[c] && in_view(g.points[c])) {
            continue;
        }
        let size = size_of("region", &pin.id, sizes.region);
        let look = style("region");
        let name = look.text(name);
        let f = FontMetrics::of(&look.family);
        requests.push(Request {
            candidates: area_candidates(world, page, inner, &area, &name, f, size, (0.05, 0.5)),
            id: format!("label-region-{}", pin.id),
            kind: Kind::Region,
            group: "region",
            text: name,
            family: look.family,
            size,
            required: true,
            rank: 2,
        });
    }

    // Named features of the overlay.
    if let Some(o) = overlay {
        for feat in &o.add.features {
            if !shown("feature", &feat.id) {
                continue;
            }
            let Some(name) = name_of("feature", &feat.id, feat.name.as_deref()) else {
                continue;
            };
            let size = size_of("feature", &feat.id, sizes.place_town);
            let look = style("feature");
            let name = look.text(&name);
            let f = FontMetrics::of(&look.family);
            let candidates = if let Some(at) = feat.at {
                if !in_view(at) {
                    continue;
                }
                point_candidates(page.to_page(at), FEATURE_RADIUS, &name, f, size)
            } else if let Some(line) = &feat.polyline {
                let line: Vec<[f64; 2]> = line.iter().map(|&p| page.to_page(p)).collect();
                river_candidates(&line, inner, usize::MAX, &name, f, size, 0.1 * size)
            } else if let Some(poly) = &feat.polygon {
                let shape = Shape::Polygon(poly.clone());
                let area: Vec<bool> = (0..g.len()).map(|c| shape.contains(g.points[c])).collect();
                area_candidates(world, page, inner, &area, &name, f, size, (0.05, 0.5))
            } else {
                continue;
            };
            requests.push(Request {
                candidates,
                id: format!("label-feature-{}", feat.id),
                kind: Kind::Feature,
                group: "feature",
                text: name,
                family: look.family,
                size,
                required: true,
                rank: 1,
            });
        }
    }

    requests.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.id.cmp(&b.id)));

    // River segments, for the cost of crossing a river.
    let mut segments: Vec<(usize, [f64; 2], [f64; 2])> = Vec::new();
    let mut seg_index = Index::default();
    for (i, line) in river_lines.iter().enumerate() {
        for w in line.windows(2) {
            let b = [
                w[0][0].min(w[1][0]),
                w[0][1].min(w[1][1]),
                w[0][0].max(w[1][0]),
                w[0][1].max(w[1][1]),
            ];
            seg_index.insert(segments.len(), b);
            segments.push((i, w[0], w[1]));
        }
    }

    let mut obstacles = fixed_obstacles(world, config, variant, page);
    let mut index = Index::default();
    for (i, o) in obstacles.iter().enumerate() {
        index.insert(i, o.bounds());
    }

    let mut out = Labels::default();
    for req in requests {
        let mut best: Option<(f64, usize)> = None;
        let mut any_inside = false;
        for (k, cand) in req.candidates.iter().enumerate() {
            if !cand.boxes.iter().all(|b| b.inside(inner)) {
                continue;
            }
            any_inside = true;
            let hit = cand.boxes.iter().any(|b| {
                index
                    .near(b.bounds())
                    .into_iter()
                    .any(|i| obstacles[i].overlaps(b))
            });
            if hit {
                continue;
            }
            let mut crossed: Vec<usize> = Vec::new();
            for b in &cand.boxes {
                for s in seg_index.near(b.bounds()) {
                    let (river, p, q) = segments[s];
                    if Some(river) != cand.river && !crossed.contains(&river) && b.crosses(p, q) {
                        crossed.push(river);
                    }
                }
            }
            let cost = cand.cost + RIVER_COST * crossed.len() as f64;
            if best.is_none_or(|(c, _)| cost < c) {
                best = Some((cost, k));
            }
        }
        match best {
            Some((_, k)) => {
                let cand = req.candidates.into_iter().nth(k).expect("candidate");
                for b in &cand.boxes {
                    index.insert(obstacles.len(), b.bounds());
                    obstacles.push(*b);
                }
                out.placed.push(Placed {
                    id: req.id,
                    kind: req.kind,
                    group: req.group,
                    text: req.text,
                    family: req.family,
                    size: req.size,
                    spacing: cand.spacing,
                    geometry: cand.geometry,
                    boxes: cand.boxes,
                });
            }
            None => out.unplaced.push(Unplaced {
                id: req.id,
                reason: if any_inside { "collision" } else { "outside" },
                required: req.required,
            }),
        }
    }
    out.placed.sort_by(|a, b| a.id.cmp(&b.id));
    out.unplaced.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangles_that_share_an_area_overlap() {
        let a = Obb::rect(0.0, 0.0, 2.0, 1.0);
        assert!(a.overlaps(&Obb::rect(1.0, 0.5, 3.0, 2.0)));
        assert!(!a.overlaps(&Obb::rect(2.0, 0.0, 3.0, 1.0)), "touch only");
        assert!(!a.overlaps(&Obb::rect(0.0, 1.5, 2.0, 2.0)));
    }

    #[test]
    fn a_turned_rectangle_overlaps_only_where_it_reaches() {
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let turned = Obb {
            c: [0.0, 0.0],
            u: [s, s],
            h: [2.0, 0.2],
        };
        // Its far corner is near (1.6, 1.6), so a box from (2.2, 2.2) is out of reach.
        assert!(turned.overlaps(&Obb::rect(1.0, 1.0, 1.6, 1.6)));
        assert!(!turned.overlaps(&Obb::rect(2.2, 2.2, 3.0, 3.0)));
        // The bounding box of the turned rectangle covers (1.3, -1.3), but
        // the rectangle does not.
        assert!(!turned.overlaps(&Obb::rect(1.1, -1.5, 1.5, -1.1)));
    }

    #[test]
    fn a_segment_crosses_a_rectangle_only_through_it() {
        let a = Obb::rect(0.0, 0.0, 2.0, 1.0);
        assert!(a.crosses([-1.0, 0.5], [3.0, 0.5]));
        assert!(!a.crosses([-1.0, 2.0], [3.0, 2.0]));
        assert!(!a.crosses([2.5, -1.0], [4.0, 3.0]));
    }

    use inkland_core::config::Config;
    use proptest::prelude::*;
    use serde_json::json;

    fn world(seed: u64, template: &str, extra: serde_json::Value) -> (World, Config) {
        let mut v = json!({
            "version": 1, "seed": seed, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 2500, "template": template },
            "pins": {
                "points": [{ "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [200, 150] }],
                "regions": [{ "id": "hills", "kind": "hills", "name": "The Grey Hills",
                              "box": [150, 100, 260, 200] }]
            },
            "render": { "extras": { "scale_bar": true, "compass": true, "cartouche": { "title": "The Realm" } } },
            "variants": [{ "name": "full", "page": { "w_mm": 167, "h_mm": 250, "margin_mm": 5 } }]
        });
        for (k, val) in extra.as_object().unwrap() {
            v[k] = val.clone();
        }
        let c: Config = serde_json::from_value(v).unwrap();
        (World::build(&c).unwrap(), c)
    }

    #[test]
    fn a_label_style_gives_the_family_and_the_case_of_each_group() {
        let (w, c) = world(4, "pangea", json!({}));
        let page = Page::new(&c, &c.variants[0]);
        let look = |group: &'static str| Look {
            family: format!("Family {group}"),
            upper: group == "city",
        };
        let labels = place_with(&w, &c, &c.variants[0], &page, &look);
        let ostrey = labels
            .placed
            .iter()
            .find(|l| l.id == "label-place-ostrey")
            .unwrap();
        assert_eq!(ostrey.group, "city");
        assert_eq!(ostrey.text, "OSTREY");
        assert_eq!(ostrey.family, "Family city");
        for l in &labels.placed {
            assert_eq!(l.family, format!("Family {}", l.group), "{}", l.id);
        }
        // The fonts of the config give the same places as before.
        let plain = place(&w, &c, &c.variants[0], &page);
        let state = plain.placed.iter().find(|l| l.group == "state").unwrap();
        assert_eq!(state.family, c.render.fonts.labels_sc);
    }

    fn check_no_overlap(w: &World, c: &Config) -> Labels {
        let page = Page::new(c, &c.variants[0]);
        let labels = place(w, c, &c.variants[0], &page);
        let [mx, my, mw, mh] = page.map_rect();
        let fixed = fixed_obstacles(w, c, &c.variants[0], &page);
        for (i, a) in labels.placed.iter().enumerate() {
            for b in &a.boxes {
                let [x0, y0, x1, y1] = b.bounds();
                assert!(
                    x0 >= mx && y0 >= my && x1 <= mx + mw && y1 <= my + mh,
                    "{} leaves the map",
                    a.id
                );
                for o in &fixed {
                    assert!(!b.overlaps(o), "{} covers a symbol or an extra", a.id);
                }
                for other in &labels.placed[i + 1..] {
                    for ob in &other.boxes {
                        assert!(!b.overlaps(ob), "{} overlaps {}", a.id, other.id);
                    }
                }
            }
        }
        labels
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(12))]
        #[test]
        fn placed_labels_never_overlap(
            seed in 0u64..1_000_000,
            template in prop::sample::select(vec!["pangea", "continents", "archipelago", "peninsula"]),
        ) {
            let (w, c) = world(seed, template, json!({}));
            let labels = check_no_overlap(&w, &c);
            prop_assert!(!labels.placed.is_empty());
        }
    }

    #[test]
    fn pinned_labels_and_states_are_placed() {
        let (w, c) = world(3, "pangea", json!({}));
        let labels = check_no_overlap(&w, &c);
        let ids: Vec<&str> = labels.placed.iter().map(|l| l.id.as_str()).collect();
        assert!(ids.contains(&"label-place-ostrey"));
        assert!(ids.contains(&"label-region-hills"));
        assert!(ids.iter().any(|i| i.starts_with("label-state-")));
        assert_eq!(labels.required_unplaced(), 0, "{:?}", labels.unplaced);
    }

    #[test]
    fn a_label_too_large_for_the_map_is_listed_and_not_drawn() {
        let big = inkland_core::config::FontSizes {
            region: 400.0,
            ..Default::default()
        };
        let (w, c) = world(
            3,
            "pangea",
            json!({ "render": {
                "font_sizes_pt": serde_json::to_value(&big).unwrap()
            }}),
        );
        let labels = check_no_overlap(&w, &c);
        let u = labels
            .unplaced
            .iter()
            .find(|u| u.id == "label-region-hills")
            .unwrap();
        assert!(u.required);
        assert_eq!(u.reason, "outside");
        assert!(labels.placed.iter().all(|l| l.id != "label-region-hills"));
    }

    #[test]
    fn a_river_label_follows_its_river_from_left_to_right() {
        let (w, c) = world(
            5,
            "pangea",
            json!({ "pins": {
                "points": [{ "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [200, 150] }],
                "regions": [
                    { "id": "hills", "kind": "mountains", "box": [40, 100, 90, 200] },
                    { "id": "sea", "kind": "sea", "box": [330, 0, 400, 300] }
                ],
                "rivers": [{ "id": "brann", "name": "Brann", "source": { "region": "hills" }, "mouth": { "region": "sea" } }]
            }}),
        );
        let labels = check_no_overlap(&w, &c);
        let l = labels
            .placed
            .iter()
            .find(|l| l.id == "label-river-brann")
            .expect("placed");
        match &l.geometry {
            Geometry::Path { points, .. } => {
                assert!(points.len() >= 2);
                assert!(points[points.len() - 1][0] > points[0][0]);
            }
            Geometry::Point { .. } => panic!("a river label follows a path"),
        }
    }

    #[test]
    fn a_large_generated_river_gets_an_optional_label() {
        // A label can collide with the names of a world, so some of a few
        // worlds must place a river label.
        let mut placed = 0;
        for seed in 11..16 {
            placed += river_labels_of(seed);
        }
        assert!(placed > 0, "no river label");
    }

    fn river_labels_of(seed: u64) -> usize {
        let (w, c) = world(
            seed,
            "continents",
            json!({ "world": { "cells": 8000, "template": "continents" } }),
        );
        let labels = check_no_overlap(&w, &c);
        let rivers: Vec<&Placed> = labels
            .placed
            .iter()
            .filter(|l| l.kind == Kind::River)
            .collect();
        for l in &rivers {
            let id: u32 = l.id.trim_start_matches("label-river-").parse().unwrap();
            let i = w.rivers.iter().position(|r| r.id == id).unwrap();
            assert_eq!(w.names.rivers[i].kind, "river");
            assert_eq!(l.text, w.names.rivers[i].name);
        }
        for u in &labels.unplaced {
            if u.id.starts_with("label-river-") || u.id.starts_with("label-lake-") {
                assert!(!u.required);
            }
        }
        rivers.len()
    }

    #[test]
    fn same_world_gives_the_same_labels() {
        let (w, c) = world(9, "continents", json!({}));
        let page = Page::new(&c, &c.variants[0]);
        assert_eq!(
            place(&w, &c, &c.variants[0], &page),
            place(&w, &c, &c.variants[0], &page)
        );
    }

    #[test]
    fn covers_is_true_only_near_a_placed_label() {
        let (w, c) = world(3, "pangea", json!({}));
        let page = Page::new(&c, &c.variants[0]);
        let labels = place(&w, &c, &c.variants[0], &page);
        let b = labels.placed[0].boxes[0];
        assert!(labels.covers(b.c, 0.0));
        let corner = b.corners()[0];
        assert!(labels.covers([corner[0] - 0.5, corner[1]], 1.0));
        assert!(!Labels::default().covers(b.c, 1.0));
    }

    #[test]
    fn sub_line_keeps_the_inner_points() {
        let pts = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]];
        let acc = [0.0, 10.0, 20.0];
        assert_eq!(
            sub_line(&pts, &acc, 5.0, 15.0),
            vec![[5.0, 0.0], [10.0, 0.0], [10.0, 5.0]]
        );
    }
}
