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

/// Free space around the text, for the white box behind the text in
/// `print-safe` mode. A label
/// with a wider halo gets more space (see `Look::halo`).
const PAD: f64 = 0.3;
/// Extra cost of a label for each river that it crosses.
const RIVER_COST: f64 = 0.5;
/// The cost of each symbol of a generated place that a required label
/// hides.
const HIDE_COST: f64 = 1.0;
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
    /// `fort`, `hamlet`, `river`, `lake`, `ocean`, `island`, `state`,
    /// `region`, or `feature`.
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
    /// `collision`, `outside`, or `hidden` (a required label covers the
    /// symbol of the place).
    pub reason: &'static str,
    /// `true` for the labels of pins, capitals, and states with their
    /// capital in the view.
    pub required: bool,
}

/// The result of label placement.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Labels {
    pub placed: Vec<Placed>,
    pub unplaced: Vec<Unplaced>,
    /// The generated places (not pinned, not capitals) whose symbols a
    /// required label covers. Their symbols and labels are left out.
    pub hidden_places: Vec<u32>,
    /// The places whose label the config leaves out and whose symbol it
    /// leaves out too (`render.labels.unlabelled_symbols`).
    pub no_symbol: Vec<u32>,
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
    /// Font size, in mm.
    size: f64,
    spacing: f64,
    cost: f64,
    /// Lower tiers are tried first. A candidate of tier 1 is a second
    /// choice: the placer uses it only when no candidate of tier 0 has a
    /// free place.
    tier: u8,
    /// The river that the label is for. A label does not cost more for
    /// crossing its own river.
    river: Option<usize>,
}

struct Request {
    id: String,
    /// The place of a place label.
    place: Option<u32>,
    kind: Kind,
    group: &'static str,
    text: String,
    family: String,
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

/// The free space around a text of `size` with a halo that reaches `halo`
/// times the size out of the glyphs.
fn pad_of(size: f64, halo: f64) -> f64 {
    PAD + halo * size
}

/// The box of a text with its baseline at `y` and its anchor at `x`.
#[allow(clippy::too_many_arguments)]
fn text_box(x: f64, y: f64, anchor: &str, w: f64, f: &FontMetrics, size: f64, pad: f64) -> Obb {
    let x0 = match anchor {
        "end" => x - w,
        "middle" => x - w / 2.0,
        _ => x,
    };
    Obb::rect(
        x0 - pad,
        y - f.cap(size) - pad,
        x0 + w + pad,
        y + 0.6 * f.descent(size) + pad,
    )
}

/// Eight positions around a place symbol, best first.
fn point_candidates(
    at: [f64; 2],
    r: f64,
    text: &str,
    f: &FontMetrics,
    size: f64,
    pad: f64,
) -> Vec<Candidate> {
    let w = f.width(text, size, 0.0);
    let (ch, dd) = (f.cap(size), 0.6 * f.descent(size));
    // Clear of the symbol box at the side, above, and below.
    let side = r + pad + 0.2;
    let above = at[1] - r - 0.2 - pad - dd;
    let below = at[1] + r + 0.2 + pad + ch;
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
            boxes: vec![text_box(bx, by, anchor, w, f, size, pad)],
            geometry: Geometry::Point {
                x: bx,
                y: by,
                anchor,
                angle: 0,
            },
            size,
            spacing: 0.0,
            cost: 0.1 * i as f64,
            tier: 0,
            river: None,
        })
        .collect()
}

/// The cells of an area mask, in increasing order.
fn members_of(area: &[bool]) -> Vec<usize> {
    (0..area.len()).filter(|&c| area[c]).collect()
}

/// The depth of each cell of an area: 1 on its edge, more inside, 0 for
/// cells outside the area. `members` are the cells of the area, in
/// increasing order. The work is in proportion to the size of the area,
/// not to the size of the grid.
fn depths(world: &World, area: &[bool], members: &[usize]) -> Vec<u32> {
    let g = &world.grid;
    let mut depth = vec![0u32; g.len()];
    let mut queue = VecDeque::new();
    for &c in members {
        if g.border[c] || g.neighbors(c).any(|m| !area[m]) {
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
/// The sizes of an area label, as parts of its full size, largest first.
/// The smallest is the smallest size of a state label in the original
/// program (`MIN_FONT_SIZE` of `fit-state-label.ts`).
const AREA_STEPS: [f64; 5] = [1.0, 0.85, 0.7, 0.55, 0.4];
/// The part of the run of an area that the text of its label can use
/// (`PATH_USAGE` of `fit-state-label.ts`).
const FIT: f64 = 0.9;
/// The smallest font size that an area label gets when it becomes smaller
/// to fit, in points. A label with a smaller full size keeps its size.
pub const MIN_LABEL_PT: f64 = 5.0;
/// The most anchors of an area label.
const MAX_ANCHORS: usize = 40;

/// Candidates for the label of an area, on its deepest cells.
///
/// On each anchor, the label gets the largest size of `AREA_STEPS` that
/// fits in the run of the area, as the original program fits state labels.
/// Smaller sizes and tighter letters are candidates of tier 1, for a label
/// that finds no free place. `halo` is the reach of the halo, as a part of
/// the font size. `inside` tells if a world point is in the area.
///
/// An area with no grid cell on the map, for example a sea far from a
/// coast, gets its anchors on a lattice of points in `bounds`, a world
/// rectangle round the area.
#[allow(clippy::too_many_arguments)]
fn area_candidates(
    world: &World,
    page: &Page,
    inner: [f64; 4],
    area: &[bool],
    members: &[usize],
    inside: &dyn Fn([f64; 2]) -> bool,
    bounds: Option<[f64; 4]>,
    text: &str,
    f: &FontMetrics,
    size: f64,
    halo: f64,
    spread: (f64, f64),
) -> Vec<Candidate> {
    let g = &world.grid;
    let depth = depths(world, area, members);
    let mut max_depth = depth.iter().copied().max().unwrap_or(0).max(1);
    let in_area = |q: [f64; 2]| {
        let w = page.to_world(q);
        let [x0, y0, x1, y1] = page.view;
        w[0] >= x0 && w[0] <= x1 && w[1] >= y0 && w[1] <= y1 && inside(w)
    };
    let on_map =
        |q: [f64; 2]| q[0] >= inner[0] && q[0] <= inner[2] && q[1] >= inner[1] && q[1] <= inner[3];
    let mut cells: Vec<(u32, usize)> = members
        .iter()
        .copied()
        .filter(|&c| depth[c] > 0)
        .filter(|&c| on_map(page.to_page(g.points[c])))
        .map(|c| (depth[c], c))
        .collect();
    cells.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut cells: Vec<(u32, [f64; 2])> = cells
        .into_iter()
        .map(|(d, c)| (d, page.to_page(g.points[c])))
        .collect();
    if cells.is_empty()
        && let Some(b) = bounds
    {
        cells = lattice_spots(page, b, &|q| on_map(q) && in_area(q));
        max_depth = cells.first().map_or(1, |s| s.0.max(1));
    }
    let n = text.chars().count();
    let least = pt(MIN_LABEL_PT).min(size);
    let mut sizes: Vec<f64> = Vec::new();
    for k in AREA_STEPS {
        let s = (size * k).max(least);
        if sizes.last().is_none_or(|&l| l - s > 1e-9) {
            sizes.push(s);
        }
    }
    // Anchor cells: the deepest cells first, each at least `ANCHOR_GAP` from
    // the anchors before it, so the candidates spread over the area.
    let mut anchors: Vec<(u32, [f64; 2])> = Vec::new();
    for &(d, q) in &cells {
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
            // The largest size with tight letters in the run, or the
            // smallest size.
            let fit = sizes
                .iter()
                .position(|&s| f.width(text, s, spread.0 * s) <= FIT * run)
                .unwrap_or(sizes.len() - 1);
            for (step, &size) in sizes.iter().enumerate().skip(fit) {
                let tier = u8::from(step > fit);
                let pad = pad_of(size, halo);
                let w0 = f.width(text, size, 0.0);
                let (ch, dd) = (f.cap(size), 0.6 * f.descent(size));
                let spaced = if n > 1 {
                    ((0.7 * run - w0) / (n - 1) as f64).clamp(spread.0 * size, spread.1 * size)
                } else {
                    0.0
                };
                // Spaced letters first; tight letters as a second choice. In
                // tier 1, tight letters and letters without space.
                let spacings = if tier == 0 {
                    [(false, spaced), (true, spread.0 * size)]
                } else {
                    [(true, spread.0 * size), (true, 0.0)]
                };
                for (tight, spacing) in spacings {
                    if tight && tier == 0 && (n < 2 || spacing >= spaced) {
                        continue;
                    }
                    if tier == 1 && spacing == 0.0 && (n < 2 || spread.0 == 0.0) {
                        continue;
                    }
                    let w = w0 + spacing * n.saturating_sub(1) as f64;
                    // The middle of the run, moved so that the text stays in the map.
                    let mid = (ahead - back) / 2.0;
                    let mut centre = [q[0] + axis[0] * mid, q[1] + axis[1] * mid];
                    let half = w / 2.0 + pad;
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
                        h: [half, (ch + dd) / 2.0 + pad],
                    };
                    // The part of the label outside the area costs more.
                    let outside = box_points(&b).filter(|&p| !in_area(p)).count() as u32;
                    // Skip a candidate that is almost the same as one before it.
                    if out.iter().any(|o| match o.geometry {
                        Geometry::Point { x, y, angle, .. } => {
                            angle == turn
                                && (o.size - size).abs() < 1e-9
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
                        size,
                        spacing,
                        cost: 3.0 * f64::from(outside) / 15.0
                            + 0.5 * (1.0 - f64::from(d) / f64::from(max_depth))
                            + STRAIGHT_COST
                            + if turn == 0 { 0.0 } else { 0.4 }
                            + if tight { 0.3 } else { 0.0 }
                            + 2.0 * (1.0 - size / sizes[0]),
                        tier,
                        river: None,
                    });
                }
            }
        }
    }
    let in_map = |p: [f64; 2]| on_map(p) && in_area(p);
    out.extend(curve_candidates(
        &anchors, max_depth, &in_map, text, f, &sizes, halo, spread,
    ));
    out
}

/// The extra cost of a straight area label, so that a curve that fits as
/// well goes first, as in the original program.
const STRAIGHT_COST: f64 = 0.2;
/// The most anchors that get curved candidates.
const CURVE_ANCHORS: usize = 12;
/// The directions of the long axis of a curved label, in degrees from the
/// page x axis.
const CURVE_AXES: [f64; 11] = [
    0.0, -15.0, 15.0, -30.0, 30.0, -45.0, 45.0, -60.0, 60.0, -75.0, 75.0,
];
/// The number of points across the area that set the shape of a curve.
const CURVE_STATIONS: usize = 7;
/// The largest bend of a curve, in degrees: the most that its direction
/// changes from one end to the other.
const CURVE_BEND: f64 = 40.0;

/// The length of a line through `q` along the unit vector `axis`, in each
/// direction, while the line stays in the area. `in_area` takes a page
/// point.
fn reach(q: [f64; 2], axis: [f64; 2], in_area: &dyn Fn([f64; 2]) -> bool, limit: f64) -> f64 {
    let mut t = 0.0;
    while t + 0.5 <= limit {
        let next = t + 0.5;
        if !in_area([q[0] + axis[0] * next, q[1] + axis[1] * next]) {
            break;
        }
        t = next;
    }
    t
}

/// The parabola `o = c0 + c1 t + c2 t^2` nearest to the points `(t, o)`,
/// by least squares. `None` if the points do not set one.
fn parabola(pts: &[(f64, f64)]) -> Option<[f64; 3]> {
    // The sums of t^k and of o t^k.
    let mut st = [0.0f64; 5];
    let mut so = [0.0f64; 3];
    for &(t, o) in pts {
        let mut tk = 1.0;
        for (k, v) in st.iter_mut().enumerate() {
            *v += tk;
            if k < 3 {
                so[k] += o * tk;
            }
            tk *= t;
        }
    }
    let m = [
        [st[0], st[1], st[2]],
        [st[1], st[2], st[3]],
        [st[2], st[3], st[4]],
    ];
    let det = |a: [[f64; 3]; 3]| {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
    };
    let d = det(m);
    if d.abs() < 1e-12 {
        return None;
    }
    let mut out = [0.0; 3];
    for (k, v) in out.iter_mut().enumerate() {
        let mut a = m;
        for r in 0..3 {
            a[r][k] = so[r];
        }
        *v = det(a) / d;
    }
    Some(out)
}

/// The lengths along a line, from its first point.
fn lengths(pts: &[[f64; 2]]) -> Vec<f64> {
    let mut acc = vec![0.0];
    for w in pts.windows(2) {
        let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        acc.push(acc[acc.len() - 1] + d);
    }
    acc
}

/// The direction of each segment of a line, in degrees from the page x
/// axis, from -180 to 180.
fn headings(pts: &[[f64; 2]]) -> Vec<f64> {
    pts.windows(2)
        .map(|w| libm::atan2(w[1][1] - w[0][1], w[1][0] - w[0][0]).to_degrees())
        .collect()
}

/// The largest angle of a line to the page x axis, in degrees. A line
/// that reads from left to right has angles below 90.
pub fn max_tilt(pts: &[[f64; 2]]) -> f64 {
    headings(pts).iter().fold(0.0, |m, a| m.max(a.abs()))
}

/// Candidates that bend along the middle of an area (`fit-state-label.ts`
/// of the original program). Through each anchor, a line along one of
/// `CURVE_AXES` crosses the area. The middle of the area across that line,
/// at `CURVE_STATIONS` points, gives a smooth curve. The text follows the
/// curve, centred on it, with letters spaced to fill part of it. The
/// candidates are `Geometry::Path` with no river.
#[allow(clippy::too_many_arguments)]
fn curve_candidates(
    anchors: &[(u32, [f64; 2])],
    max_depth: u32,
    in_area: &dyn Fn([f64; 2]) -> bool,
    text: &str,
    f: &FontMetrics,
    sizes: &[f64],
    halo: f64,
    spread: (f64, f64),
) -> Vec<Candidate> {
    let n = text.chars().count();
    let mut out: Vec<Candidate> = Vec::new();
    for &(d, q) in anchors.iter().take(CURVE_ANCHORS) {
        for deg in CURVE_AXES {
            let a = deg.to_radians();
            let axis = [libm::cos(a), libm::sin(a)];
            let across = [-axis[1], axis[0]];
            let back = reach(q, [-axis[0], -axis[1]], in_area, 200.0);
            let ahead = reach(q, axis, in_area, 200.0);
            let run = back + ahead;
            if f.width(
                text,
                sizes[sizes.len() - 1],
                spread.0 * sizes[sizes.len() - 1],
            ) > FIT * run
            {
                continue;
            }
            // The middle of the area across the axis, at each station of the
            // inner 90 % of the run.
            let mut mids: Vec<[f64; 2]> = Vec::new();
            for i in 0..CURVE_STATIONS {
                let t = -back + run * (0.05 + 0.9 * i as f64 / (CURVE_STATIONS - 1) as f64);
                let c = [q[0] + axis[0] * t, q[1] + axis[1] * t];
                if !in_area(c) {
                    mids.clear();
                    break;
                }
                let up = reach(c, [-across[0], -across[1]], in_area, 60.0);
                let down = reach(c, across, in_area, 60.0);
                let m = (down - up) / 2.0;
                mids.push([c[0] + across[0] * m, c[1] + across[1] * m]);
            }
            if mids.len() < CURVE_STATIONS {
                continue;
            }
            // One smooth arc: the parabola nearest to the middles, in the
            // frame of the axis, with its bend limited to `CURVE_BEND`.
            let local: Vec<(f64, f64)> = mids
                .iter()
                .map(|m| {
                    let d = [m[0] - q[0], m[1] - q[1]];
                    (
                        d[0] * axis[0] + d[1] * axis[1],
                        d[0] * across[0] + d[1] * across[1],
                    )
                })
                .collect();
            let Some([c0, c1, mut c2]) = parabola(&local) else {
                continue;
            };
            let (t0, t1) = (local[0].0, local[local.len() - 1].0);
            let arc = |c2: f64| -> Vec<[f64; 2]> {
                (0..=12)
                    .map(|i| {
                        let t = t0 + (t1 - t0) * f64::from(i) / 12.0;
                        let o = c0 + c1 * t + c2 * t * t;
                        [
                            q[0] + axis[0] * t + across[0] * o,
                            q[1] + axis[1] * t + across[1] * o,
                        ]
                    })
                    .collect()
            };
            let bend_of = |pts: &[[f64; 2]]| {
                let heads = headings(pts);
                let (lo, hi) = heads
                    .iter()
                    .fold((f64::MAX, f64::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
                hi - lo
            };
            let mut pts = arc(c2);
            for _ in 0..8 {
                if bend_of(&pts) <= CURVE_BEND {
                    break;
                }
                c2 /= 2.0;
                pts = arc(c2);
            }
            let bend = bend_of(&pts);
            if pts[pts.len() - 1][0] < pts[0][0] {
                pts.reverse();
            }
            if bend > CURVE_BEND || max_tilt(&pts) >= 90.0 {
                continue;
            }
            let acc = lengths(&pts);
            let len = acc[acc.len() - 1];
            let fit = sizes
                .iter()
                .position(|&s| f.width(text, s, spread.0 * s) <= FIT * len)
                .unwrap_or(sizes.len() - 1);
            let size = sizes[fit];
            let pad = pad_of(size, halo);
            let w0 = f.width(text, size, 0.0);
            let spacing = if n > 1 {
                ((0.7 * len - w0) / (n - 1) as f64).clamp(spread.0 * size, spread.1 * size)
            } else {
                0.0
            };
            let w = w0 + spacing * n.saturating_sub(1) as f64;
            if w > len {
                continue;
            }
            // The boxes of the text, centred on the middle of the curve.
            let (ch, dd) = (f.cap(size), 0.6 * f.descent(size));
            let s0 = (len - w) / 2.0;
            let parts = ((w / (1.2 * size)).ceil() as usize).max(1);
            let boxes: Vec<Obb> = (0..parts)
                .map(|i| {
                    let p = point_at(&pts, &acc, s0 + w * i as f64 / parts as f64);
                    let r = point_at(&pts, &acc, s0 + w * (i + 1) as f64 / parts as f64);
                    let l = ((r[0] - p[0]).powi(2) + (r[1] - p[1]).powi(2))
                        .sqrt()
                        .max(1e-9);
                    Obb {
                        c: [(p[0] + r[0]) / 2.0, (p[1] + r[1]) / 2.0],
                        u: [(r[0] - p[0]) / l, (r[1] - p[1]) / l],
                        h: [l / 2.0 + pad, (ch + dd) / 2.0 + pad],
                    }
                })
                .collect();
            let total = boxes.len() * 15;
            let outside = boxes
                .iter()
                .flat_map(box_points)
                .filter(|&p| !in_area(p))
                .count();
            // Skip a curve that is almost the same as one before it.
            let mid = point_at(&pts, &acc, len / 2.0);
            if out.iter().any(|o| match &o.geometry {
                Geometry::Path { points, .. } => {
                    let oa = lengths(points);
                    let om = point_at(points, &oa, oa[oa.len() - 1] / 2.0);
                    (o.size - size).abs() < 1e-9
                        && (om[0] - mid[0]).abs() < 1.0
                        && (om[1] - mid[1]).abs() < 1.0
                }
                Geometry::Point { .. } => false,
            }) {
                continue;
            }
            out.push(Candidate {
                geometry: Geometry::Path {
                    points: pts,
                    // A negative offset puts the baseline below the curve, so
                    // that the glyphs are centred on it.
                    offset: -(ch - dd) / 2.0,
                },
                boxes,
                size,
                spacing,
                cost: 3.0 * outside as f64 / total as f64
                    + 0.5 * (1.0 - f64::from(d) / f64::from(max_depth))
                    + 0.2 * deg.abs() / 75.0
                    + 0.1 * bend / CURVE_BEND
                    + 2.0 * (1.0 - size / sizes[0]),
                tier: 0,
                river: None,
            });
        }
    }
    out
}

/// Fifteen points on a box: five along it on each of its two long sides
/// and on its middle line.
/// The last candidates of a sea or lake label, for when no other candidate
/// has a place: level labels at the deepest water cells of the area in the
/// view, from the full size down to 0.4 of it. A label that stays on water
/// is of tier 2. If none does, the smallest size inside the map is of tier
/// 3, so the label is still drawn in its sea.
#[allow(clippy::too_many_arguments)]
fn water_fallback(
    world: &World,
    page: &Page,
    inner: [f64; 4],
    area: &[bool],
    text: &str,
    f: &FontMetrics,
    size: f64,
    halo: f64,
) -> Vec<Candidate> {
    let g = &world.grid;
    let members = members_of(area);
    let depth = depths(world, area, &members);
    let mut cells: Vec<usize> = members
        .iter()
        .copied()
        .filter(|&c| {
            let q = page.to_page(g.points[c]);
            q[0] >= inner[0] && q[0] <= inner[2] && q[1] >= inner[1] && q[1] <= inner[3]
        })
        .collect();
    cells.sort_by(|&a, &b| depth[b].cmp(&depth[a]).then(a.cmp(&b)));
    let on_water = |b: &Obb| {
        box_points(b).all(|q| {
            let p = page.to_world(q);
            !world.features.is_land(g.find_cell(p[0], p[1]))
        })
    };
    let mut out = Vec::new();
    // The middle of the water on the row of a cell, between the shores to
    // the west and to the east, in page mm.
    let middle = |c: usize| -> [f64; 2] {
        let p = g.points[c];
        let step = g.step_x.max(1e-9) / 4.0;
        let wet = |x: f64| {
            let q = [x, p[1]];
            let k = g.find_cell(q[0], q[1]);
            area[k] && !world.features.is_land(k)
        };
        let reach = |dir: f64| {
            let mut x = p[0];
            for _ in 0..400 {
                if !wet(x + dir * step) {
                    break;
                }
                x += dir * step;
            }
            x
        };
        page.to_page([(reach(-1.0) + reach(1.0)) / 2.0, p[1]])
    };
    for (rank, &c) in cells.iter().take(12).enumerate() {
        let [x, y0] = middle(c);
        for (step, k) in AREA_STEPS.iter().enumerate() {
            let sz = size * k;
            let y = y0 + f.cap(sz) / 2.0;
            let b = text_box(
                x,
                y,
                "middle",
                f.width(text, sz, 0.0),
                f,
                sz,
                pad_of(sz, halo),
            );
            if !b.inside(inner) {
                continue;
            }
            let wet = on_water(&b);
            let last = step == AREA_STEPS.len() - 1;
            if !wet && !last {
                continue;
            }
            out.push(Candidate {
                boxes: vec![b],
                geometry: Geometry::Point {
                    x,
                    y,
                    anchor: "middle",
                    angle: 0,
                },
                size: sz,
                spacing: 0.0,
                // A label on the water competes with the other area
                // candidates by its size, on the same scale as their cost,
                // so that a larger label on open water wins over a smaller
                // one. A label over land is the last choice.
                cost: if wet {
                    2.0 * (1.0 - k) + 0.01 * rank as f64
                } else {
                    rank as f64 + step as f64
                },
                tier: if wet { 0 } else { 3 },
                river: None,
            });
            if wet {
                break;
            }
        }
    }
    out
}

/// The connected parts of an area: for each cell, the index of its part
/// (cells joined through neighbors in the area), or `usize::MAX` outside
/// the area.
fn connected_parts(g: &inkland_core::grid::Grid, area: &[bool]) -> Vec<usize> {
    let mut part = vec![usize::MAX; area.len()];
    let mut next = 0;
    for start in 0..area.len() {
        if !area[start] || part[start] != usize::MAX {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        part[start] = next;
        while let Some(c) = queue.pop_front() {
            for m in g.neighbors(c) {
                if area[m] && part[m] == usize::MAX {
                    part[m] = next;
                    queue.push_back(m);
                }
            }
        }
        next += 1;
    }
    part
}

fn box_points(b: &Obb) -> impl Iterator<Item = [f64; 2]> + '_ {
    (0..5).flat_map(move |i| {
        (0..3).map(move |j| {
            let s = -b.h[0] + 2.0 * b.h[0] * f64::from(i) / 4.0;
            let t = -b.h[1] + b.h[1] * f64::from(j);
            [
                b.c[0] + b.u[0] * s - b.u[1] * t,
                b.c[1] + b.u[1] * s + b.u[0] * t,
            ]
        })
    })
}

/// The points of a square lattice in the part of the world rectangle `b`
/// in the view, on the page, with their depth in the area, deepest first.
/// `keep` tells if a page point is in the area. The depth is 1 for a point
/// next to a point outside the area or on the edge of the lattice, and
/// more inside, as `depths` gives it for cells.
fn lattice_spots(
    page: &Page,
    b: [f64; 4],
    keep: &dyn Fn([f64; 2]) -> bool,
) -> Vec<(u32, [f64; 2])> {
    const STEPS: usize = 24;
    let [vx0, vy0, vx1, vy1] = page.view;
    let (x0, y0, x1, y1) = (b[0].max(vx0), b[1].max(vy0), b[2].min(vx1), b[3].min(vy1));
    if x1 <= x0 || y1 <= y0 {
        return Vec::new();
    }
    let step = (x1 - x0).max(y1 - y0) / STEPS as f64;
    let nx = ((x1 - x0) / step).floor() as usize + 1;
    let ny = ((y1 - y0) / step).floor() as usize + 1;
    // The lattice is centred in the rectangle.
    let ox = x0 + ((x1 - x0) - step * (nx - 1) as f64) / 2.0;
    let oy = y0 + ((y1 - y0) - step * (ny - 1) as f64) / 2.0;
    let at = |i: usize, j: usize| page.to_page([ox + step * i as f64, oy + step * j as f64]);
    let ins: Vec<bool> = (0..nx * ny).map(|k| keep(at(k % nx, k / nx))).collect();
    let mut depth = vec![0u32; nx * ny];
    let mut queue = VecDeque::new();
    let around = |k: usize| {
        let (i, j) = (k % nx, k / nx);
        [
            (i > 0).then(|| k - 1),
            (i + 1 < nx).then(|| k + 1),
            (j > 0).then(|| k - nx),
            (j + 1 < ny).then(|| k + nx),
        ]
    };
    for k in 0..nx * ny {
        if ins[k] && around(k).iter().any(|m| m.is_none_or(|m| !ins[m])) {
            depth[k] = 1;
            queue.push_back(k);
        }
    }
    while let Some(k) = queue.pop_front() {
        for m in around(k).into_iter().flatten() {
            if ins[m] && depth[m] == 0 {
                depth[m] = depth[k] + 1;
                queue.push_back(m);
            }
        }
    }
    let mut out: Vec<(u32, usize)> = (0..nx * ny)
        .filter(|&k| depth[k] > 0)
        .map(|k| (depth[k], k))
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    out.into_iter()
        .map(|(d, k)| (d, at(k % nx, k / nx)))
        .collect()
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
#[allow(clippy::too_many_arguments)]
fn river_candidates(
    line: &[[f64; 2]],
    inner: [f64; 4],
    river: usize,
    text: &str,
    f: &FontMetrics,
    size: f64,
    pad: f64,
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
                        h: [l / 2.0 + pad, (ch + dd) / 2.0 + pad],
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
                size,
                spacing,
                cost: middle + 2.0 * (1.0 - straight),
                tier: 0,
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

/// The boxes that labels must not touch: place symbols and extras. The
/// second list gives the place of each box that a required label can hide:
/// the symbol of a generated place that is not a capital. The places in
/// `skip` have no symbol.
fn fixed_obstacles(
    world: &World,
    config: &Config,
    variant: &Variant,
    page: &Page,
    skip: &[u32],
) -> (Vec<Obb>, Vec<Option<u32>>) {
    let symbols = symbol_points(world, page);
    let [x0, y0, x1, y1] = page.view;
    let mut out: Vec<Obb> = Vec::new();
    let mut hideable: Vec<Option<u32>> = Vec::new();
    for p in world
        .society
        .places
        .iter()
        .filter(|p| p.at[0] >= x0 && p.at[0] <= x1 && p.at[1] >= y0 && p.at[1] <= y1)
        .filter(|p| !skip.contains(&p.id))
    {
        let [x, y] = page.to_page(p.at);
        let r = symbol_radius(p.kind, p.capital);
        out.push(Obb::rect(x - r, y - r, x + r, y + r));
        hideable.push((p.pin.is_none() && !p.capital).then_some(p.id));
    }
    for [x, y] in feature_points(config, variant, page) {
        let r = FEATURE_RADIUS;
        out.push(Obb::rect(x - r, y - r, x + r, y + r));
        hideable.push(None);
    }
    for [a, b, c, d] in crate::extras::boxes(config, variant, page, &symbols) {
        out.push(Obb::rect(a, b, c, d));
        hideable.push(None);
    }
    (out, hideable)
}

/// The look of a label group: its font family, its letter case, and its
/// halo.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub family: String,
    /// `true` for text in capital letters.
    pub upper: bool,
    /// How far the halo reaches out of the glyphs, as a part of the font
    /// size. The placer keeps this space free around the text, in addition
    /// to the space for the halo of `print-safe` mode.
    pub halo: f64,
    /// The font size of the group, in mm. `None` gives the size of the
    /// config. The label options of an overlay can set another size.
    pub size: Option<f64>,
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
        halo: 0.0,
        size: None,
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
    // The size of a label: the size of its overlay options, else the size
    // of its look, else the size of the config.
    let size_of = |kind: &str, id: &str, look: &Look, default_pt: f64| {
        opts(kind, id)
            .and_then(|o| o.size_pt)
            .map(pt)
            .or(look.size)
            .unwrap_or_else(|| pt(default_pt))
    };
    let name_of = |kind: &str, id: &str, name: Option<&str>| -> Option<String> {
        overlay
            .and_then(|o| inkland_core::overlay::renamed(o, kind, id))
            .or(name)
            .filter(|n| !n.trim().is_empty())
            .map(str::to_owned)
    };
    let mut requests: Vec<Request> = Vec::new();
    let symbols = config.render.labels.unlabelled_symbols;
    let mut no_symbol: Vec<u32> = Vec::new();

    // Places.
    for p in &world.society.places {
        if p.name.trim().is_empty() || !in_view(p.at) {
            continue;
        }
        let key = p.pin.clone().unwrap_or_else(|| p.id.to_string());
        if !shown("place", &key) {
            if !symbols {
                no_symbol.push(p.id);
            }
            continue;
        }
        let group = place_group(p.kind, p.capital);
        let look = style(group);
        let size = size_of(
            "place",
            &key,
            &look,
            if p.capital || p.kind == PlaceKind::City {
                sizes.place_city
            } else if p.kind == PlaceKind::Village {
                sizes.place_village
            } else {
                sizes.place_town
            },
        );
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
            place: Some(p.id),
            candidates: point_candidates(
                page.to_page(p.at),
                symbol_radius(p.kind, p.capital),
                &text,
                f,
                size,
                pad_of(size, look.halo),
            ),
            id,
            kind: Kind::Place,
            group,
            text,
            family: look.family,
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
        // A pinned river with its source and its mouth outside the view only
        // goes through it and can show only a short part, so its label is
        // optional, as the label of a state with its capital outside the
        // view.
        let ends_in_view = r.points.first().is_some_and(|&p| in_view(p))
            || r.points.last().is_some_and(|&p| in_view(p));
        let (key, name, required) = match r.course {
            Some(k) => {
                let pin = &config.pins.rivers[k];
                if !shown("river", &pin.id) {
                    continue;
                }
                match name_of("river", &pin.id, pin.name.as_deref()) {
                    Some(n) => (pin.id.clone(), n, ends_in_view),
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
        let look = style("river");
        let size = size_of("river", &key, &look, sizes.river);
        let name = look.text(&name);
        let f = FontMetrics::of(&look.family);
        requests.push(Request {
            place: None,
            candidates: river_candidates(
                &river_lines[i],
                inner,
                i,
                &name,
                f,
                size,
                pad_of(size, look.halo),
                0.1 * size,
            ),
            id: format!("label-river-{key}"),
            kind: Kind::River,
            group: "river",
            text: name,
            family: look.family,
            required,
            rank: if required { 1 } else { 6 },
        });
    }

    // Named lakes, oceans, and islands, with optional labels, in the label
    // groups `lake`, `ocean`, and `island`. A continent holds the states,
    // so it has no label.
    // The cells of each feature, in increasing order.
    let mut feature_cells: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (c, &f) in world.features.of_cell.iter().enumerate() {
        feature_cells.entry(f).or_default().push(c);
    }
    let mut feature_mask = vec![false; world.grid.len()];
    for n in &world.names.features {
        let Some(name) = n.name.as_deref() else {
            continue;
        };
        let (group, prefix, default_pt, spread, rank) = match n.group {
            "freshwater" | "salt" | "frozen" | "dry" | "sinkhole" | "lava" => {
                ("lake", "lake", sizes.river, (0.05, 0.3), 6)
            }
            "ocean" | "sea" | "gulf" => ("ocean", "water", sizes.region, (0.1, 0.6), 6),
            "island" | "isle" | "lake_island" => {
                ("island", "island", sizes.place_town, (0.05, 0.3), 7)
            }
            _ => continue,
        };
        let g = &world.grid;
        let members = feature_cells.get(&n.id).map_or(&[][..], Vec::as_slice);
        if !members.iter().any(|&c| in_view(g.points[c])) {
            continue;
        }
        let look = style(group);
        let size = look.size.unwrap_or_else(|| pt(default_pt));
        let name = look.text(name);
        let f = FontMetrics::of(&look.family);
        // One mask serves all features: set the cells of this feature, and
        // clear them again after.
        for &c in members {
            feature_mask[c] = true;
        }
        let candidates = area_candidates(
            world,
            page,
            inner,
            &feature_mask,
            members,
            &|p: [f64; 2]| feature_mask[world.grid.find_cell(p[0], p[1])],
            None,
            &name,
            f,
            size,
            look.halo,
            spread,
        );
        for &c in members {
            feature_mask[c] = false;
        }
        requests.push(Request {
            place: None,
            candidates,
            id: format!("label-{prefix}-{}", n.id),
            kind: Kind::Region,
            group,
            text: name,
            family: look.family,
            required: false,
            rank,
        });
    }

    // States. The label options of a state have the key of its capital pin,
    // for example `state:solmere`.
    let g = &world.grid;
    let mut state_keys: BTreeMap<String, String> = BTreeMap::new();
    for st in &world.society.states {
        if st.name.trim().is_empty() {
            continue;
        }
        let capital = world.society.places.iter().find(|p| p.id == st.capital);
        let key = capital.and_then(|p| p.pin.clone());
        if let Some(k) = &key {
            if !shown("state", k) {
                continue;
            }
            state_keys.insert(st.id.to_string(), k.clone());
        }
        let area: Vec<bool> = (0..g.len())
            .map(|c| world.society.cell_state[c] == st.id && world.features.is_land(c))
            .collect();
        if !(0..g.len()).any(|c| area[c] && in_view(g.points[c])) {
            continue;
        }
        // The parts of the state to label: all of it, or, for a state with
        // seats on other land, the part of the capital and the part of each
        // seat. The label of a seat part has the key of its seat pin.
        let seat_places: Vec<&inkland_core::society::Place> = st
            .seats
            .iter()
            .filter_map(|&id| world.society.places.iter().find(|p| p.id == id))
            .collect();
        let mut parts: Vec<(String, Option<String>, Vec<bool>, bool)> = Vec::new();
        if seat_places.is_empty() {
            let in_view_capital = capital.is_some_and(|p| in_view(p.at));
            parts.push((st.id.to_string(), key.clone(), area, in_view_capital));
        } else {
            let part_of = connected_parts(g, &area);
            let main = capital.map(|p| part_of[p.cell]);
            let main_area: Vec<bool> = (0..g.len())
                .map(|c| area[c] && (main.is_none() || part_of[c] == main.unwrap_or(0)))
                .collect();
            parts.push((
                st.id.to_string(),
                key.clone(),
                main_area,
                capital.is_some_and(|p| in_view(p.at)),
            ));
            let mut done: Vec<usize> = main.into_iter().collect();
            for (k, seat) in seat_places.iter().enumerate() {
                let part = part_of[seat.cell];
                if part == usize::MAX || done.contains(&part) {
                    continue;
                }
                done.push(part);
                let seat_key = seat.pin.clone();
                if seat_key.as_deref().is_some_and(|k| !shown("state", k)) {
                    continue;
                }
                let id = format!("{}-{}", st.id, k + 1);
                if let Some(sk) = &seat_key {
                    state_keys.insert(id.clone(), sk.clone());
                }
                let part_area: Vec<bool> = (0..g.len()).map(|c| part_of[c] == part).collect();
                parts.push((id, seat_key, part_area, in_view(seat.at)));
            }
        }
        let look = style("state");
        let name = look.text(&st.name);
        let f = FontMetrics::of(&look.family);
        for (id, part_key, part_area, part_in_view) in parts {
            if !(0..g.len()).any(|c| part_area[c] && in_view(g.points[c])) {
                continue;
            }
            let size = match &part_key {
                Some(k) => size_of("state", k, &look, sizes.state),
                None => look.size.unwrap_or_else(|| pt(sizes.state)),
            };
            // A part whose capital or seat is outside the view can show only
            // a small piece in the view. Its label is optional.
            requests.push(Request {
                place: None,
                candidates: area_candidates(
                    world,
                    page,
                    inner,
                    &part_area,
                    &members_of(&part_area),
                    &|p: [f64; 2]| part_area[world.grid.find_cell(p[0], p[1])],
                    None,
                    &name,
                    f,
                    size,
                    look.halo,
                    (0.15, 0.8),
                ),
                id: format!("label-state-{id}"),
                kind: Kind::State,
                group: "state",
                text: name.clone(),
                family: look.family.clone(),
                required: part_in_view,
                rank: 2,
            });
        }
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
        // The label of a sea or a lake uses only water cells.
        let water = pin.kind.is_water();
        let wet = |c: usize| !water || !world.features.is_land(c);
        let area: Vec<bool> = (0..g.len())
            .map(|c| shape.contains(g.points[c]) && wet(c))
            .collect();
        let inside = |p: [f64; 2]| shape.contains(p) && wet(g.find_cell(p[0], p[1]));
        // The grid has no cells in deep water, so a sea far from a coast can
        // have no cell. The shape, not its cells, tells if it is in the view.
        let bounds = shape.bbox();
        if bounds[0] > vx1 || bounds[2] < vx0 || bounds[1] > vy1 || bounds[3] < vy0 {
            continue;
        }
        let look = style("region");
        let size = size_of("region", &pin.id, &look, sizes.region);
        let name = look.text(name);
        let f = FontMetrics::of(&look.family);
        let mut candidates = area_candidates(
            world,
            page,
            inner,
            &area,
            &members_of(&area),
            &inside,
            Some(bounds),
            &name,
            f,
            size,
            look.halo,
            (0.05, 0.5),
        );
        if water {
            candidates.retain(|cand| {
                cand.boxes.iter().all(|b| {
                    box_points(b).all(|q| {
                        let p = page.to_world(q);
                        !world.features.is_land(g.find_cell(p[0], p[1]))
                    })
                })
            });
            // Level labels on the deepest water of the area in the view:
            // on the water, they compete by size; over land, they are the
            // last choice.
            candidates.extend(water_fallback(
                world, page, inner, &area, &name, f, size, look.halo,
            ));
        }
        requests.push(Request {
            place: None,
            candidates,
            id: format!("label-region-{}", pin.id),
            kind: Kind::Region,
            group: "region",
            text: name,
            family: look.family,
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
            let look = style("feature");
            let size = size_of("feature", &feat.id, &look, sizes.place_town);
            let name = look.text(&name);
            let f = FontMetrics::of(&look.family);
            let candidates = if let Some(at) = feat.at {
                if !in_view(at) {
                    continue;
                }
                point_candidates(
                    page.to_page(at),
                    FEATURE_RADIUS,
                    &name,
                    f,
                    size,
                    pad_of(size, look.halo),
                )
            } else if let Some(line) = &feat.polyline {
                let line: Vec<[f64; 2]> = line.iter().map(|&p| page.to_page(p)).collect();
                let pad = pad_of(size, look.halo);
                river_candidates(&line, inner, usize::MAX, &name, f, size, pad, 0.1 * size)
            } else if let Some(poly) = &feat.polygon {
                let shape = Shape::Polygon(poly.clone());
                let area: Vec<bool> = (0..g.len()).map(|c| shape.contains(g.points[c])).collect();
                area_candidates(
                    world,
                    page,
                    inner,
                    &area,
                    &members_of(&area),
                    &|p: [f64; 2]| area[world.grid.find_cell(p[0], p[1])],
                    None,
                    &name,
                    f,
                    size,
                    look.halo,
                    (0.05, 0.5),
                )
            } else {
                continue;
            };
            requests.push(Request {
                place: None,
                candidates,
                id: format!("label-feature-{}", feat.id),
                kind: Kind::Feature,
                group: "feature",
                text: name,
                family: look.family,
                required: true,
                rank: 1,
            });
        }
    }

    // The kind and the key of the label options of a request.
    let key_of = |id: &str| -> Option<(String, String)> {
        let (kind, key) = id.strip_prefix("label-")?.split_once('-')?;
        let key = match kind {
            "state" => state_keys.get(key)?.as_str(),
            _ => key,
        };
        Some((kind.to_owned(), key.to_owned()))
    };
    let angle_of =
        |id: &str| key_of(id).and_then(|(kind, key)| opts(&kind, &key).and_then(|o| o.angle));

    // Keep only the turns and the curves that the config lets labels have.
    // A label with an `angle` in its options keeps only that turn.
    let max_angle = config.render.labels.max_angle;
    let curves = config.render.labels.curved;
    for r in &mut requests {
        if let Some(a) = angle_of(&r.id) {
            r.candidates.retain(|c| match &c.geometry {
                Geometry::Point { angle, .. } => angle.unsigned_abs() == u32::from(a),
                Geometry::Path { .. } => c.river.is_some(),
            });
            continue;
        }
        r.candidates.retain(|c| match &c.geometry {
            Geometry::Point { angle, .. } => f64::from(angle.unsigned_abs()) <= max_angle,
            // A curved area label has no river. A river label follows its
            // river.
            Geometry::Path { points, .. } => {
                c.river.is_some() || (curves && max_tilt(points) <= max_angle)
            }
        });
    }

    // Leave out the labels with a generated name, when the config asks.
    let mut out = Labels::default();
    if !config.render.labels.generated {
        let written = config.written_names();
        requests.retain(|r| {
            let keep = written.contains(&r.text.trim().to_lowercase());
            if !keep
                && !symbols
                && let Some(p) = r.place
            {
                no_symbol.push(p);
            }
            if !keep {
                out.unplaced.push(Unplaced {
                    id: r.id.clone(),
                    reason: "generated",
                    required: false,
                });
            }
            keep
        });
    }

    // A label with a position in its options has one candidate there, and
    // goes first.
    let mut fixed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for r in &mut requests {
        let Some((kind, key)) = r.id.strip_prefix("label-").and_then(|k| k.split_once('-')) else {
            continue;
        };
        let key = match kind {
            "state" => match state_keys.get(key) {
                Some(k) => k.as_str(),
                None => continue,
            },
            _ => key,
        };
        let Some(at) = opts(kind, key).and_then(|o| o.at) else {
            continue;
        };
        // The size of its options, else the size that the placer gives it
        // without `at`: the size of its best candidate (the lowest tier,
        // then the lowest cost), as an area label fits its area. An item
        // with no candidate in the view gets the size of its kind.
        let best = r
            .candidates
            .iter()
            .min_by(|a, b| a.tier.cmp(&b.tier).then(a.cost.total_cmp(&b.cost)));
        let size = match (opts(kind, key).and_then(|o| o.size_pt), best) {
            (Some(s), _) => pt(s),
            (None, Some(best)) => best.size,
            (None, None) => {
                let default_pt = match r.group {
                    "state" => sizes.state,
                    "region" | "ocean" => sizes.region,
                    "river" | "lake" => sizes.river,
                    "capital" | "city" => sizes.place_city,
                    "village" | "hamlet" => sizes.place_village,
                    _ => sizes.place_town,
                };
                size_of(kind, key, &style(r.group), default_pt)
            }
        };
        let f = FontMetrics::of(&r.family);
        let [x, y] = page.to_page(at);
        let w = f.width(&r.text, size, 0.0);
        let up = opts(kind, key).and_then(|o| o.angle) == Some(90);
        // The baseline lies half the cap height off the middle: below it
        // for a level label, right of it for a label that reads up.
        let (x, y, b, angle) = if up {
            let half = (f.cap(size) + 0.6 * f.descent(size)) / 2.0;
            let b = Obb {
                c: [x, y],
                u: [0.0, -1.0],
                h: [w / 2.0, half],
            };
            (x + f.cap(size) / 2.0, y, b, -90)
        } else {
            let y = y + f.cap(size) / 2.0;
            (x, y, text_box(x, y, "middle", w, f, size, 0.0), 0)
        };
        r.candidates = vec![Candidate {
            boxes: vec![b],
            geometry: Geometry::Point {
                x,
                y,
                anchor: "middle",
                angle,
            },
            size,
            spacing: 0.0,
            cost: 0.0,
            tier: 0,
            river: None,
        }];
        fixed.insert(r.id.clone());
    }
    requests.sort_by(|a, b| {
        (!fixed.contains(&a.id), a.rank, &a.id).cmp(&(!fixed.contains(&b.id), b.rank, &b.id))
    });

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

    no_symbol.sort_unstable();
    let (mut obstacles, mut hideable) = fixed_obstacles(world, config, variant, page, &no_symbol);
    out.no_symbol = no_symbol;
    let mut index = Index::default();
    for (i, o) in obstacles.iter().enumerate() {
        index.insert(i, o.bounds());
    }
    let mut gone = vec![false; obstacles.len()];

    for mut req in requests {
        if fixed.contains(&req.id) {
            // The author put it here: no collision check.
            let cand = req.candidates.remove(0);
            for b in &cand.boxes {
                index.insert(obstacles.len(), b.bounds());
                obstacles.push(*b);
                hideable.push(None);
                gone.push(false);
            }
            out.placed.push(Placed {
                id: req.id,
                kind: req.kind,
                group: req.group,
                text: req.text,
                family: req.family,
                size: cand.size,
                spacing: cand.spacing,
                geometry: cand.geometry,
                boxes: cand.boxes,
            });
            continue;
        }
        if req.place.is_some_and(|p| out.hidden_places.contains(&p)) {
            out.unplaced.push(Unplaced {
                id: req.id,
                reason: "hidden",
                required: req.required,
            });
            continue;
        }
        // A required label that finds no free place can cover the symbols
        // of generated places. Each covered symbol costs more.
        let passes: &[bool] = if req.required {
            &[false, true]
        } else {
            &[false]
        };
        let mut best: Option<(f64, usize, Vec<usize>)> = None;
        let mut any_inside = false;
        let tiers = req.candidates.iter().map(|c| c.tier).max().unwrap_or(0);
        'passes: for &hide in passes {
            for tier in 0..=tiers {
                for (k, cand) in req.candidates.iter().enumerate() {
                    if cand.tier != tier {
                        continue;
                    }
                    if !cand.boxes.iter().all(|b| b.inside(inner)) {
                        continue;
                    }
                    any_inside = true;
                    let mut covered: Vec<usize> = Vec::new();
                    let hit = cand.boxes.iter().any(|b| {
                        index.near(b.bounds()).into_iter().any(|i| {
                            if gone[i] || !obstacles[i].overlaps(b) {
                                return false;
                            }
                            if hide && hideable[i].is_some() {
                                if !covered.contains(&i) {
                                    covered.push(i);
                                }
                                return false;
                            }
                            true
                        })
                    });
                    if hit {
                        continue;
                    }
                    let mut crossed: Vec<usize> = Vec::new();
                    for b in &cand.boxes {
                        for s in seg_index.near(b.bounds()) {
                            let (river, p, q) = segments[s];
                            if Some(river) != cand.river
                                && !crossed.contains(&river)
                                && b.crosses(p, q)
                            {
                                crossed.push(river);
                            }
                        }
                    }
                    let cost = cand.cost
                        + RIVER_COST * crossed.len() as f64
                        + HIDE_COST * covered.len() as f64;
                    if best.as_ref().is_none_or(|(c, _, _)| cost < *c) {
                        best = Some((cost, k, covered));
                    }
                }
                if best.is_some() {
                    break 'passes;
                }
            }
        }
        match best {
            Some((_, k, covered)) => {
                for i in covered {
                    gone[i] = true;
                    if let Some(p) = hideable[i].take() {
                        out.hidden_places.push(p);
                    }
                }
                let cand = req.candidates.into_iter().nth(k).expect("candidate");
                for b in &cand.boxes {
                    index.insert(obstacles.len(), b.bounds());
                    obstacles.push(*b);
                    hideable.push(None);
                    gone.push(false);
                }
                out.placed.push(Placed {
                    id: req.id,
                    kind: req.kind,
                    group: req.group,
                    text: req.text,
                    family: req.family,
                    size: cand.size,
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
            halo: 0.0,
            size: None,
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
        let (fixed, hideable) = fixed_obstacles(w, c, &c.variants[0], &page, &labels.no_symbol);
        // The symbols that a required label hides are not drawn.
        let fixed: Vec<Obb> = fixed
            .into_iter()
            .zip(hideable)
            .filter(|(_, h)| h.is_none_or(|p| !labels.hidden_places.contains(&p)))
            .map(|(o, _)| o)
            .collect();
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
    fn a_label_with_a_position_goes_there_first() {
        let (w, c) = world(
            3,
            "pangea",
            json!({
                "overlays": { "today": { "labels": {
                    "region:hills": { "at": [60, 250] },
                    "place:ostrey": { "at": [205, 140] }
                } } },
                "variants": [{ "name": "full", "overlay": "today",
                               "page": { "w_mm": 167, "h_mm": 250, "margin_mm": 5 } }]
            }),
        );
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        let labels = place(&w, &c, &v, &page);
        for (id, at) in [
            ("label-region-hills", [60.0, 250.0]),
            ("label-place-ostrey", [205.0, 140.0]),
        ] {
            let l = labels.placed.iter().find(|l| l.id == id).unwrap();
            let Geometry::Point {
                x,
                y,
                anchor,
                angle,
            } = l.geometry
            else {
                panic!("{id} is not at a point");
            };
            let q = page.to_page(at);
            assert_eq!((anchor, angle), ("middle", 0));

            assert!((x - q[0]).abs() < 1e-9 && y > q[1], "{id}");
        }
        // The other labels keep clear of them.
        let fixed: Vec<&Placed> = labels
            .placed
            .iter()
            .filter(|l| l.id == "label-region-hills")
            .collect();
        for l in labels
            .placed
            .iter()
            .filter(|l| l.id != "label-region-hills" && l.id != "label-place-ostrey")
        {
            for b in &l.boxes {
                assert!(fixed[0].boxes.iter().all(|f| !f.overlaps(b)), "{}", l.id);
            }
        }
    }

    /// A world with a named sea in deep water, far from the land.
    fn deep_sea(extra: serde_json::Value) -> (World, Config) {
        let mut v = json!({
            "pins": {
                "points": [{ "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [200, 150] }],
                "regions": [
                    { "id": "land", "kind": "plains", "circle": { "at": [200, 150], "r": 60 } },
                    { "id": "cold", "kind": "sea", "name": "The Cold Sea",
                      "box": [10, 10, 130, 60] }
                ]
            }
        });
        for (k, val) in extra.as_object().unwrap() {
            v[k] = val.clone();
        }
        let (w, c) = world(5, "atoll", v);
        let shape = Shape::of(&c.pins.regions[1]).unwrap();
        assert!(
            !w.grid.points.iter().any(|&p| shape.contains(p)),
            "the sea has grid cells, so the test does not test the deep sea"
        );
        (w, c)
    }

    #[test]
    fn a_sea_with_no_grid_cell_gets_its_label() {
        let (w, c) = deep_sea(json!({}));
        let labels = check_no_overlap(&w, &c);
        let l = labels
            .placed
            .iter()
            .find(|l| l.id == "label-region-cold")
            .unwrap_or_else(|| panic!("no label; unplaced: {:?}", labels.unplaced));
        let page = Page::new(&c, &c.variants[0]);
        // The middle of the text: its point, or the middle of its curve.
        let mid = match &l.geometry {
            Geometry::Point { x, y, .. } => [*x, *y],
            Geometry::Path { points, .. } => {
                let acc = lengths(points);
                point_at(points, &acc, acc[acc.len() - 1] / 2.0)
            }
        };
        let at = page.to_world(mid);
        assert!(
            Shape::of(&c.pins.regions[1]).unwrap().contains(at),
            "{at:?}"
        );
    }

    #[test]
    fn a_sea_with_no_grid_cell_goes_to_its_position() {
        let (w, c) = deep_sea(json!({
            "overlays": { "today": { "labels": { "region:cold": { "at": [70, 40] } } } },
            "variants": [{ "name": "full", "overlay": "today",
                           "page": { "w_mm": 167, "h_mm": 250, "margin_mm": 5 } }]
        }));
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        let labels = place(&w, &c, &v, &page);
        let l = labels
            .placed
            .iter()
            .find(|l| l.id == "label-region-cold")
            .unwrap();
        let Geometry::Point { x, .. } = l.geometry else {
            panic!("the label is not at a point");
        };
        assert!((x - page.to_page([70.0, 40.0])[0]).abs() < 1e-9);
    }

    #[test]
    fn a_sea_label_that_cannot_fit_is_listed() {
        let big = inkland_core::config::FontSizes {
            region: 400.0,
            ..Default::default()
        };
        let (w, c) = deep_sea(json!({ "render": {
            "font_sizes_pt": serde_json::to_value(&big).unwrap()
        }}));
        let labels = check_no_overlap(&w, &c);
        let u = labels
            .unplaced
            .iter()
            .find(|u| u.id == "label-region-cold")
            .unwrap();
        assert!(u.required);
        assert_eq!(u.reason, "outside");
    }

    #[test]
    fn a_sea_label_at_the_frame_edge_stays_on_the_water() {
        let (w, c) = world(
            3,
            "pangea",
            json!({ "pins": {
                "points": [{ "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [200, 150] }],
                "regions": [
                    { "id": "land", "kind": "plains", "polygon": [[80, 0], [400, 0], [400, 300], [0, 300], [0, 30], [80, 30]] },
                    { "id": "west", "kind": "sea", "name": "The Western Ocean",
                      "box": [0, 0, 80, 30] }
                ]
            }}),
        );
        let page = Page::new(&c, &c.variants[0]);
        let labels = check_no_overlap(&w, &c);
        // In this corner, the land and the extras leave no free water: the
        // label goes in the list, not on the land.
        let Some(l) = labels.placed.iter().find(|l| l.id == "label-region-west") else {
            assert!(labels.unplaced.iter().any(|u| u.id == "label-region-west"));
            return;
        };
        for b in &l.boxes {
            for q in box_points(b) {
                let p = page.to_world(q);
                let cell = w.grid.find_cell(p[0], p[1]);
                assert!(!w.features.is_land(cell), "the label is on land at {p:?}");
            }
        }
    }

    #[test]
    fn a_state_label_takes_the_options_of_its_capital_pin() {
        let (w, c) = world(
            3,
            "pangea",
            json!({
                "pins": {
                    "points": [{ "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [200, 150],
                                 "capital_of": "Ostria" }]
                },
                "overlays": { "today": { "labels": {
                    "state:ostrey": { "at": [120, 60], "size_pt": 9 }
                } } },
                "variants": [{ "name": "full", "overlay": "today",
                               "page": { "w_mm": 167, "h_mm": 250, "margin_mm": 5 } }]
            }),
        );
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        let labels = place(&w, &c, &v, &page);
        let l = labels.placed.iter().find(|l| l.text == "Ostria").unwrap();
        assert_eq!(l.kind, Kind::State);
        assert!((l.size - pt(9.0)).abs() < 1e-9, "{}", l.size);
        let Geometry::Point { x, angle, .. } = l.geometry else {
            panic!("the label is not at a point");
        };
        assert_eq!(angle, 0);
        assert!((x - page.to_page([120.0, 60.0])[0]).abs() < 1e-9);

        // `hide` leaves the label out.
        let mut c = c;
        let o = c.overlays.get_mut("today").unwrap();
        o.labels.get_mut("state:ostrey").unwrap().hide = true;
        let labels = place(&w, &c, &v, &page);
        assert!(labels.placed.iter().all(|l| l.text != "Ostria"));
    }

    #[test]
    fn a_max_angle_of_zero_gives_level_labels() {
        let turned = |l: &Labels| {
            l.placed
                .iter()
                .filter(|p| matches!(p.geometry, Geometry::Point { angle, .. } if angle != 0))
                .count()
        };
        // A tall, narrow region gets a turned label by default in some
        // worlds. A curved label is a path, so keep the labels straight.
        let mut turned_by_default = 0;
        for seed in 1..=6 {
            let (w, mut c) = world(seed, "pangea", json!({}));
            let v = c.variants[0].clone();
            let page = Page::new(&c, &v);
            c.render.labels.curved = false;
            c.pins.regions[0].rect = Some([180.0, 40.0, 200.0, 260.0]);
            turned_by_default += turned(&place(&w, &c, &v, &page));
            c.render.labels.max_angle = 0.0;
            let level = place(&w, &c, &v, &page);
            assert_eq!(turned(&level), 0, "seed {seed}");
        }
        assert!(
            turned_by_default > 0,
            "no label turns, so the test tests nothing"
        );
    }

    #[test]
    fn a_place_with_a_left_out_label_can_lose_its_symbol() {
        // The capital with no name in the config has a generated name.
        let (w, mut c) = world(
            3,
            "pangea",
            json!({ "pins": {
                "points": [
                    { "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [200, 150] },
                    { "id": "seat", "kind": "town", "at": [120, 90], "capital_of": "Varn" }
                ]
            }}),
        );
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        let seat = w
            .society
            .places
            .iter()
            .find(|p| p.pin.as_deref() == Some("seat"))
            .unwrap()
            .id;
        c.render.labels.generated = false;
        let kept = place(&w, &c, &v, &page);
        assert!(kept.unplaced.iter().any(|u| u.id == "label-place-seat"));
        assert!(kept.no_symbol.is_empty());
        c.render.labels.unlabelled_symbols = false;
        let gone = check_no_overlap(&w, &c);
        assert!(gone.no_symbol.contains(&seat));
        let ostrey = w
            .society
            .places
            .iter()
            .find(|p| p.name == "Ostrey")
            .unwrap();
        assert!(!gone.no_symbol.contains(&ostrey.id));
        // The overlay option `hide` also takes the symbol away.
        c.render.labels.generated = true;
        c.overlays.insert(
            "today".into(),
            serde_json::from_value(json!({ "labels": { "place:ostrey": { "hide": true } } }))
                .unwrap(),
        );
        let mut v = v;
        v.overlay = Some("today".into());
        let hidden = place(&w, &c, &v, &page);
        assert!(hidden.no_symbol.contains(&ostrey.id));
        assert!(!hidden.no_symbol.contains(&seat));
    }

    /// The curved state and region labels of a set of labels.
    fn curved(l: &Labels) -> Vec<&Placed> {
        l.placed
            .iter()
            .filter(|p| matches!(p.kind, Kind::State | Kind::Region))
            .filter(|p| matches!(p.geometry, Geometry::Path { .. }))
            .collect()
    }

    #[test]
    fn area_labels_can_bend_along_their_area() {
        let (w, c) = world(3, "pangea", json!({}));
        let labels = check_no_overlap(&w, &c);
        let bent = curved(&labels);
        assert!(!bent.is_empty(), "no area label bends");
        for l in bent {
            let Geometry::Path { points, offset } = &l.geometry else {
                unreachable!()
            };
            // The text reads from left to right, centred on the curve.
            assert!(points[0][0] < points[points.len() - 1][0], "{}", l.id);
            assert!(max_tilt(points) < 90.0, "{}", l.id);
            assert!(*offset < 0.0, "{}", l.id);
            assert!(l.spacing > 0.0, "{}", l.id);
        }
    }

    #[test]
    fn curved_false_gives_straight_area_labels() {
        let (w, mut c) = world(3, "pangea", json!({}));
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        assert!(!curved(&place(&w, &c, &v, &page)).is_empty());
        c.render.labels.curved = false;
        let straight = place(&w, &c, &v, &page);
        assert!(curved(&straight).is_empty());
        assert!(straight.placed.iter().any(|l| l.kind == Kind::State));
    }

    #[test]
    fn max_angle_limits_the_tilt_of_a_curve() {
        let (w, mut c) = world(3, "pangea", json!({}));
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        c.render.labels.max_angle = 20.0;
        let some = place(&w, &c, &v, &page);
        for l in curved(&some) {
            let Geometry::Path { points, .. } = &l.geometry else {
                unreachable!()
            };
            assert!(max_tilt(points) <= 20.0, "{}", l.id);
        }
        c.render.labels.max_angle = 0.0;
        let level = place(&w, &c, &v, &page);
        assert!(curved(&level).is_empty());
    }

    #[test]
    fn a_sea_label_with_a_position_keeps_the_size_that_fits_its_sea() {
        // A long, narrow sea: without `at`, its label is smaller than the
        // full size of a region, to fit. With `at`, it keeps that size.
        let (w, c) = world(
            3,
            "pangea",
            json!({
                "world": { "cells": 2500, "template": "pangea", "fill_land": true },
                "pins": { "regions": [
                    { "id": "strait", "kind": "sea", "name": "The Sea of Serenity",
                      "box": [170, 0, 230, 300] }
                ] }
            }),
        );
        // Level, straight labels only, so the label must get smaller to fit.
        let mut c = c;
        c.render.labels.max_angle = 0.0;
        c.render.labels.curved = false;
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        let size_of_label = |c: &Config| {
            let v = &c.variants[0];
            place(&w, c, v, &page)
                .placed
                .iter()
                .find(|l| l.id == "label-region-strait")
                .map(|l| l.size)
                .unwrap()
        };
        let free = size_of_label(&c);
        assert!(
            free < pt(c.render.font_sizes_pt.region),
            "the label fits its sea"
        );
        let mut at = c.clone();
        at.overlays = serde_json::from_value(json!({ "o": { "labels": {
            "region:strait": { "at": [200, 150] }
        } } }))
        .unwrap();
        at.variants[0].overlay = Some("o".into());
        assert!((size_of_label(&at) - free).abs() < 1e-9);
    }

    #[test]
    fn a_sea_label_in_a_narrow_strait_is_still_placed() {
        let (w, mut c) = world(
            3,
            "pangea",
            json!({
                "world": { "cells": 2500, "template": "pangea", "fill_land": true },
                "pins": { "regions": [
                    { "id": "strait", "kind": "sea", "name": "The Sea of Serenity",
                      "box": [195, 0, 205, 300] }
                ] }
            }),
        );
        // Only level, straight labels: none fits on the water of the strait.
        c.render.labels.max_angle = 0.0;
        c.render.labels.curved = false;
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        let labels = place(&w, &c, &v, &page);
        let l = labels
            .placed
            .iter()
            .find(|l| l.id == "label-region-strait")
            .unwrap_or_else(|| panic!("unplaced: {:?}", labels.unplaced));
        let Geometry::Point { x, angle, .. } = l.geometry else {
            panic!("not a level label");
        };
        assert_eq!(angle, 0);
        // It stands in the strait.
        let at = page.to_world([x, 0.0]);
        assert!((190.0..=210.0).contains(&at[0]), "at x {}", at[0]);
    }

    #[test]
    fn a_label_on_open_water_competes_by_size_in_the_middle_of_the_water() {
        let (w, c) = world(
            3,
            "pangea",
            json!({
                "world": { "cells": 2500, "template": "pangea", "fill_land": true },
                "pins": { "regions": [
                    { "id": "strait", "kind": "sea", "name": "The Sea of Serenity",
                      "box": [170, 0, 230, 300] }
                ] }
            }),
        );
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        let [mx, my, mw, mh] = page.map_rect();
        let inner = [mx + 0.5, my + 0.5, mx + mw - 0.5, my + mh - 0.5];
        let g = &w.grid;
        let area: Vec<bool> = (0..g.len())
            .map(|k| {
                let p = g.points[k];
                (170.0..=230.0).contains(&p[0]) && !w.features.is_land(k)
            })
            .collect();
        let f = FontMetrics::of(&c.render.fonts.labels_sc);
        let full = pt(c.render.font_sizes_pt.region);
        let out = water_fallback(&w, &page, inner, &area, "The Sea of Serenity", f, full, 0.0);
        // The candidates on the water that are larger than the smallest
        // size of an area label.
        let wet: Vec<&Candidate> = out
            .iter()
            .filter(|o| o.tier == 0 && o.size > pt(MIN_LABEL_PT) + 1e-9)
            .collect();
        assert!(
            !wet.is_empty(),
            "{:?}",
            out.iter().map(|o| (o.tier, o.size)).collect::<Vec<_>>()
        );
        // A normal candidate at the smallest size costs at least this much,
        // so each of them wins over it.
        let smallest = 2.0 * (1.0 - pt(MIN_LABEL_PT) / full);
        for o in &wet {
            assert!(o.cost < smallest, "{} {smallest}", o.cost);
            let Geometry::Point { x, y, .. } = o.geometry else {
                panic!("not a straight label");
            };
            // In the middle of the water between the two shores.
            let at = page.to_world([x, y]);
            assert!((at[0] - 200.0).abs() < 8.0, "{at:?}");
        }
    }

    #[test]
    fn a_sea_label_in_a_strait_can_turn_with_its_options() {
        let (w, mut c) = world(
            3,
            "pangea",
            json!({
                "world": { "cells": 2500, "template": "pangea", "fill_land": true },
                "pins": { "regions": [
                    { "id": "strait", "kind": "sea", "name": "The Sea of Serenity",
                      "box": [190, 0, 210, 300] }
                ] },
                "overlays": { "book": { "labels": { "region:strait": { "angle": 90 } } } }
            }),
        );
        c.render.labels.max_angle = 0.0;
        c.render.labels.curved = false;
        let level = place(
            &w,
            &c,
            &c.variants[0].clone(),
            &Page::new(&c, &c.variants[0]),
        );
        c.variants[0].overlay = Some("book".to_owned());
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        let labels = place(&w, &c, &v, &page);
        let find = |ls: &Labels| {
            ls.placed
                .iter()
                .find(|l| l.id == "label-region-strait")
                .cloned()
                .unwrap_or_else(|| panic!("unplaced: {:?}", ls.unplaced))
        };
        let (l, flat) = (find(&labels), find(&level));
        let Geometry::Point { angle, x, .. } = l.geometry else {
            panic!("not a straight label");
        };
        assert_eq!(angle, -90);
        assert!(l.size > flat.size, "{} {}", l.size, flat.size);
        let at = page.to_world([x, 0.0]);
        assert!((185.0..=215.0).contains(&at[0]), "at x {}", at[0]);
        // The other labels stay level.
        assert!(
            labels
                .placed
                .iter()
                .filter(|o| o.id != l.id)
                .all(|o| match o.geometry {
                    Geometry::Point { angle, .. } => angle == 0,
                    Geometry::Path { .. } => true,
                })
        );
    }

    #[test]
    fn each_part_of_a_state_with_seats_gets_a_label() {
        // One state on two shores of a strait: the capital in the west, a
        // seat in the east.
        let (w, c) = world(
            3,
            "pangea",
            json!({
                "world": { "cells": 3000, "template": "pangea", "fill_land": true, "states": 1 },
                "pins": {
                    "points": [
                        { "id": "solmere", "kind": "city", "name": "Solmere", "at": [110, 150], "capital_of": "Lassiter" },
                        { "id": "east-lassiter", "kind": "town", "name": "Eastwatch", "at": [300, 150], "capital_of": "Lassiter" }
                    ],
                    "regions": [{ "id": "strait", "kind": "sea", "box": [190, 0, 215, 300] }]
                }
            }),
        );
        let st = &w.society.states[0];
        assert_eq!(st.seats.len(), 1, "one state with a seat");
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        let labels = place(&w, &c, &v, &page);
        let states: Vec<&Placed> = labels
            .placed
            .iter()
            .filter(|l| l.kind == Kind::State)
            .collect();
        let ids: Vec<&str> = states.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids.len(), 2, "{ids:?} {:?}", labels.unplaced);
        assert!(ids.contains(&format!("label-state-{}", st.id).as_str()));
        assert!(ids.contains(&format!("label-state-{}-1", st.id).as_str()));
        assert!(states.iter().all(|l| l.text == states[0].text));
        // One label on each shore.
        let x = |l: &Placed| page.to_world(l.boxes[0].c)[0];
        assert!(states.iter().any(|l| x(l) < 190.0) && states.iter().any(|l| x(l) > 215.0));
    }

    #[test]
    fn generated_names_can_be_left_out() {
        // The capital names the state, so its label is a written name too.
        let (w, mut c) = world(3, "pangea", json!({}));
        let v = c.variants[0].clone();
        let page = Page::new(&c, &v);
        let all = place(&w, &c, &v, &page);
        let written = c.written_names();
        let generated = |l: &Labels| {
            l.placed
                .iter()
                .filter(|p| !written.contains(&p.text.to_lowercase()))
                .count()
        };
        assert!(generated(&all) > 0, "the world shows generated names");
        c.render.labels.generated = false;
        let only = place(&w, &c, &v, &page);
        assert_eq!(generated(&only), 0);
        let ids: Vec<&str> = only.placed.iter().map(|l| l.id.as_str()).collect();
        assert!(ids.contains(&"label-place-ostrey"));
        assert!(ids.contains(&"label-region-hills"));
        assert!(
            only.unplaced
                .iter()
                .any(|u| u.reason == "generated" && !u.required)
        );
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
    fn an_area_label_gets_smaller_to_fit_its_area() {
        let big = inkland_core::config::FontSizes {
            region: 40.0,
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
        let l = labels
            .placed
            .iter()
            .find(|l| l.id == "label-region-hills")
            .expect("placed");
        assert!(l.size < pt(40.0) - 1e-9, "{}", l.size);
        assert!(l.size >= 0.4 * pt(40.0) - 1e-9, "{}", l.size);
    }

    #[test]
    fn smaller_sizes_and_tighter_letters_are_a_second_choice() {
        let (w, c) = world(3, "pangea", json!({}));
        let page = Page::new(&c, &c.variants[0]);
        let [mx, my, mw, mh] = page.map_rect();
        let inner = [mx + 0.5, my + 0.5, mx + mw - 0.5, my + mh - 0.5];
        let area: Vec<bool> = (0..w.grid.len()).map(|c| w.features.is_land(c)).collect();
        let f = FontMetrics::of("IM Fell English SC");
        let size = pt(10.0);
        let cands = area_candidates(
            &w,
            &page,
            inner,
            &area,
            &members_of(&area),
            &|p: [f64; 2]| area[w.grid.find_cell(p[0], p[1])],
            None,
            "Long Name",
            f,
            size,
            0.0,
            (0.15, 0.8),
        );
        let first: Vec<&Candidate> = cands.iter().filter(|c| c.tier == 0).collect();
        let second: Vec<&Candidate> = cands.iter().filter(|c| c.tier == 1).collect();
        assert!(!first.is_empty() && !second.is_empty());
        let largest = first.iter().map(|c| c.size).fold(0.0, f64::max);
        assert!((largest - size).abs() < 1e-9);
        assert!(second.iter().all(|c| c.size < size));
        assert!(second.iter().any(|c| c.spacing == 0.0));
        // No label is smaller than 0.4 of its size or than the least size.
        for c in &cands {
            assert!(
                c.size >= (0.4 * size).max(pt(MIN_LABEL_PT)) - 1e-9,
                "{}",
                c.size
            );
        }
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
    fn a_river_that_only_goes_through_the_view_has_an_optional_label() {
        let pins = json!({ "pins": {
            "points": [{ "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [200, 150] }],
            "regions": [
                { "id": "hills", "kind": "mountains", "box": [40, 100, 90, 200] },
                { "id": "sea", "kind": "sea", "box": [330, 0, 400, 300] }
            ],
            "rivers": [{ "id": "brann", "name": "Brann", "source": { "region": "hills" }, "mouth": { "region": "sea" } }]
        }});
        let (w, mut c) = world(5, "pangea", pins);
        let r = &w.rivers[w.rivers.iter().position(|r| r.course == Some(0)).unwrap()];
        let (a, b) = (r.points[0], r.points[r.points.len() - 1]);
        // A crop between the source and the mouth, with a part of the river.
        let mid = r.points[r.points.len() / 2];
        let crop = [mid[0] - 20.0, mid[1] - 15.0, mid[0] + 20.0, mid[1] + 15.0];
        let inside =
            |p: [f64; 2]| p[0] >= crop[0] && p[0] <= crop[2] && p[1] >= crop[1] && p[1] <= crop[3];
        assert!(!inside(a) && !inside(b), "the ends are outside the crop");
        c.variants[0].crop = Some(crop);
        // A label too large for any map, so that it is never placed.
        c.render.font_sizes_pt.river = 400.0;
        let required = |c: &Config| {
            let page = Page::new(c, &c.variants[0]);
            let labels = place(&w, c, &c.variants[0], &page);
            let u = labels.unplaced.iter().find(|u| u.id == "label-river-brann");
            u.expect("unplaced").required
        };
        assert!(!required(&c));
        // The full map has the source and the mouth: the label is required.
        c.variants[0].crop = None;
        assert!(required(&c));
    }

    #[test]
    fn a_large_generated_river_gets_an_optional_label() {
        // A long name does not fit along a short river, and a label can
        // collide with other labels. So the first world with few towns
        // that places a river label is the test world.
        let placed = (11..120).any(|seed| river_labels_of(seed) > 0);
        assert!(placed, "no river label");
    }

    fn river_labels_of(seed: u64) -> usize {
        let (w, c) = world(
            seed,
            "continents",
            json!({ "world": { "cells": 8000, "template": "continents", "towns": 20 } }),
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
    fn lakes_oceans_and_islands_get_optional_labels() {
        // Whether a label has room depends on the world. So the test looks
        // at worlds until it has seen a placed label of each kind, and
        // checks the rules in each world on the way.
        let mut kinds = std::collections::BTreeSet::new();
        for seed in 1..=40 {
            let (w, c) = world(
                seed,
                "archipelago",
                json!({ "world": { "cells": 4000, "template": "archipelago" } }),
            );
            let labels = check_no_overlap(&w, &c);
            for n in &w.names.features {
                let prefix = match n.group {
                    "freshwater" | "salt" | "frozen" | "dry" | "sinkhole" | "lava" => "lake",
                    "ocean" | "sea" | "gulf" => "water",
                    "island" | "isle" | "lake_island" => "island",
                    _ => continue,
                };
                let id = format!("label-{prefix}-{}", n.id);
                let placed = labels.placed.iter().find(|l| l.id == id);
                let unplaced = labels.unplaced.iter().find(|u| u.id == id);
                // Each named feature in the view has a label or is in the list.
                assert!(
                    placed.is_some() || unplaced.is_some(),
                    "{seed} {id} {}",
                    n.group
                );
                assert!(unplaced.is_none_or(|u| !u.required), "{seed} {id}");
                if let Some(l) = placed {
                    assert_eq!(l.text, n.name.clone().unwrap());
                    kinds.insert(l.group);
                }
            }
            assert_eq!(labels.required_unplaced(), 0, "{seed}");
            if kinds.len() == 3 {
                break;
            }
        }
        assert_eq!(
            kinds.into_iter().collect::<Vec<_>>(),
            ["island", "lake", "ocean"],
            "a label of each kind has room in some world"
        );
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
