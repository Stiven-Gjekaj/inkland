//! Geometry for `colour` mode: the pixel frame, areas of cells, curves, and
//! the coastline of Azgaar's Fantasy Map Generator.
//!
//! The original program draws in "map pixels": a map of about 1920 x 1080
//! pixels. The styles give line widths and sizes in these pixels. inkland
//! puts the world in the same number of pixels, so the styles keep their
//! look. The SVG scales the pixels to millimetres on the page.

use crate::page::Page;
use inkland_core::grid::OUTSIDE;
use inkland_core::num::fixed;
use inkland_core::world::World;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The map size of the original program, in pixels.
pub const MAP_PX: [f64; 2] = [1920.0, 1080.0];

/// A number in pixel coordinates: 2 decimals.
pub fn n(v: f64) -> String {
    fixed(v, 2)
}

/// From world units to pixels on the page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Px {
    pub page: Page,
    /// Pixels in one world unit, for a map of `MAP_PX` area.
    pub unit_px: f64,
    /// Millimetres in one pixel on the page.
    pub mm: f64,
}

impl Px {
    pub fn new(page: &Page, width: f64, height: f64) -> Px {
        let unit_px = ((MAP_PX[0] * MAP_PX[1]) / (width * height)).sqrt();
        Px {
            page: *page,
            unit_px,
            mm: page.scale / unit_px,
        }
    }

    /// A world point in page pixels.
    pub fn p(&self, w: [f64; 2]) -> [f64; 2] {
        let q = self.page.to_page(w);
        [q[0] / self.mm, q[1] / self.mm]
    }

    /// A point in world pixels (world units times `unit_px`) in page
    /// pixels.
    pub fn from_world_px(&self, w: [f64; 2]) -> [f64; 2] {
        self.p([w[0] / self.unit_px, w[1] / self.unit_px])
    }

    /// A length in millimetres, in pixels.
    pub fn of_mm(&self, mm: f64) -> f64 {
        mm / self.mm
    }

    /// The map area in page pixels: `[x, y, width, height]`.
    pub fn map_rect(&self) -> [f64; 4] {
        let [x, y, w, h] = self.page.map_rect();
        [x / self.mm, y / self.mm, w / self.mm, h / self.mm]
    }
}

/// A directed edge between two Voronoi vertices, with the cell on the other
/// side (`OUTSIDE` for the frame).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub from: u32,
    pub to: u32,
    pub other: u32,
}

/// The rings of the areas of each type. `type_of` gives the type of each
/// cell; 0 is no type. Each ring has the area on the same side, so a hole
/// turns the other way. The result is in the order of the types.
pub fn rings(world: &World, type_of: &dyn Fn(usize) -> u32) -> BTreeMap<u32, Vec<Vec<Edge>>> {
    rings_on(&world.grid, type_of)
}

/// The rings of the areas of each type on a grid: the second grid of the
/// world, or its first grid (`World::base`).
pub fn rings_on(
    g: &inkland_core::grid::Grid,
    type_of: &dyn Fn(usize) -> u32,
) -> BTreeMap<u32, Vec<Vec<Edge>>> {
    let mut by_type: BTreeMap<u32, Vec<Edge>> = BTreeMap::new();
    for c in 0..g.len() {
        let t = type_of(c);
        if t == 0 {
            continue;
        }
        let poly = &g.polygons[c];
        for (k, &o) in g.edge_cells[c].iter().enumerate() {
            if o != OUTSIDE && type_of(o as usize) == t {
                continue;
            }
            by_type.entry(t).or_default().push(Edge {
                from: poly[k],
                to: poly[(k + 1) % poly.len()],
                other: o,
            });
        }
    }
    by_type
        .into_iter()
        .map(|(t, edges)| (t, chain_rings(&edges)))
        .collect()
}

/// Join directed edges into closed rings. Rings start at their lowest
/// free edge, so the result is the same on each run.
pub fn chain_rings(edges: &[Edge]) -> Vec<Vec<Edge>> {
    let mut out_of: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, e) in edges.iter().enumerate() {
        out_of.entry(e.from).or_default().push(i);
    }
    let mut used = vec![false; edges.len()];
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by_key(|&i| (edges[i].from, edges[i].to));
    let mut rings = Vec::new();
    for start in order {
        if used[start] {
            continue;
        }
        let mut ring = Vec::new();
        let mut cur = start;
        loop {
            used[cur] = true;
            ring.push(edges[cur]);
            let next = out_of
                .get(&edges[cur].to)
                .and_then(|list| list.iter().copied().find(|&i| !used[i]));
            match next {
                Some(i) => cur = i,
                None => break,
            }
        }
        rings.push(ring);
    }
    rings
}

/// The points of a ring of edges, in world units.
pub fn ring_points(world: &World, ring: &[Edge]) -> Vec<[f64; 2]> {
    ring.iter()
        .map(|e| world.grid.vertices[e.from as usize])
        .collect()
}

/// The pole of inaccessibility of a polygon: the inside point that is
/// farthest from the edges (`polylabel` of Mapbox, which FMG uses). The
/// first ring is the outline and the others are holes. `precision` is in
/// the units of the points.
pub fn polylabel(polygon: &[Vec<[f64; 2]>], precision: f64) -> [f64; 2] {
    use std::cmp::Ordering;
    use std::collections::BinaryHeap;
    let Some(outer) = polygon.first().filter(|r| !r.is_empty()) else {
        return [0.0, 0.0];
    };
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in outer {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let size = precision.max(w.min(h));
    if size == precision {
        return [x0, y0];
    }
    #[derive(Clone, Copy)]
    struct Cell {
        x: f64,
        y: f64,
        h: f64,
        d: f64,
        max: f64,
        seq: u64,
    }
    impl PartialEq for Cell {
        fn eq(&self, o: &Self) -> bool {
            self.cmp(o) == Ordering::Equal
        }
    }
    impl Eq for Cell {}
    impl PartialOrd for Cell {
        fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for Cell {
        fn cmp(&self, o: &Self) -> Ordering {
            // The largest `max` first; on a tie, the first pushed.
            self.max.total_cmp(&o.max).then(o.seq.cmp(&self.seq))
        }
    }
    let dist = |x: f64, y: f64| -> f64 {
        let mut inside = false;
        let mut min = f64::INFINITY;
        for ring in polygon {
            let n = ring.len();
            let mut j = n.wrapping_sub(1);
            for i in 0..n {
                let (a, b) = (ring[i], ring[j]);
                if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0]
                {
                    inside = !inside;
                }
                min = min.min(seg_dist2(x, y, a, b));
                j = i;
            }
        }
        if min == 0.0 {
            0.0
        } else {
            (if inside { 1.0 } else { -1.0 }) * libm::sqrt(min)
        }
    };
    let mut seq = 0u64;
    let mut cell = |x: f64, y: f64, h: f64| {
        let d = dist(x, y);
        seq += 1;
        Cell {
            x,
            y,
            h,
            d,
            max: d + h * std::f64::consts::SQRT_2,
            seq,
        }
    };
    // The first guesses: the centroid, then the center of the box.
    let (mut area, mut cx, mut cy) = (0.0, 0.0, 0.0);
    let n = outer.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (outer[i], outer[j]);
        let f = a[0] * b[1] - b[0] * a[1];
        cx += (a[0] + b[0]) * f;
        cy += (a[1] + b[1]) * f;
        area += f * 3.0;
        j = i;
    }
    let mut best = if area == 0.0 {
        cell(outer[0][0], outer[0][1], 0.0)
    } else {
        cell(cx / area, cy / area, 0.0)
    };
    let bbox = cell(x0 + w / 2.0, y0 + h / 2.0, 0.0);
    if bbox.d > best.d {
        best = bbox;
    }
    let mut queue = BinaryHeap::new();
    let mut probe = |x: f64, y: f64, h: f64, best: &mut Cell, queue: &mut BinaryHeap<Cell>| {
        let c = cell(x, y, h);
        if c.max > best.d + precision {
            queue.push(c);
        }
        if c.d > best.d {
            *best = c;
        }
    };
    let half = size / 2.0;
    let mut x = x0;
    while x < x1 {
        let mut y = y0;
        while y < y1 {
            probe(x + half, y + half, half, &mut best, &mut queue);
            y += size;
        }
        x += size;
    }
    while let Some(c) = queue.pop() {
        if c.max - best.d <= precision {
            break;
        }
        let h = c.h / 2.0;
        probe(c.x - h, c.y - h, h, &mut best, &mut queue);
        probe(c.x + h, c.y - h, h, &mut best, &mut queue);
        probe(c.x - h, c.y + h, h, &mut best, &mut queue);
        probe(c.x + h, c.y + h, h, &mut best, &mut queue);
    }
    [best.x, best.y]
}

/// The squared distance from a point to a segment.
fn seg_dist2(px: f64, py: f64, a: [f64; 2], b: [f64; 2]) -> f64 {
    let (mut x, mut y) = (a[0], a[1]);
    let (mut dx, mut dy) = (b[0] - x, b[1] - y);
    if dx != 0.0 || dy != 0.0 {
        let t = ((px - x) * dx + (py - y) * dy) / (dx * dx + dy * dy);
        if t > 1.0 {
            x = b[0];
            y = b[1];
        } else if t > 0.0 {
            x += dx * t;
            y += dy * t;
        }
    }
    dx = px - x;
    dy = py - y;
    dx * dx + dy * dy
}

/// Twice the signed area of a polygon.
pub fn area2(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let a = points[i];
            let b = points[(i + 1) % n];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum()
}

/// Path data of a closed polygon with straight lines.
pub fn polygon_path(points: &[[f64; 2]]) -> String {
    let mut d = String::new();
    for (i, p) in points.iter().enumerate() {
        d.push(if i == 0 { 'M' } else { 'L' });
        let _ = write!(d, "{},{}", n(p[0]), n(p[1]));
    }
    if !d.is_empty() {
        d.push('Z');
    }
    d
}

/// Path data of an open line with straight lines.
pub fn line_path(points: &[[f64; 2]]) -> String {
    let mut d = String::new();
    for (i, p) in points.iter().enumerate() {
        d.push(if i == 0 { 'M' } else { 'L' });
        let _ = write!(d, "{},{}", n(p[0]), n(p[1]));
    }
    d
}

/// The closed B-spline of d3 (`curveBasisClosed`).
pub fn basis_closed(points: &[[f64; 2]]) -> String {
    let mut d = String::new();
    let m = points.len();
    if m == 0 {
        return d;
    }
    if m == 1 {
        let _ = write!(d, "M{},{}Z", n(points[0][0]), n(points[0][1]));
        return d;
    }
    if m == 2 {
        let (a, b) = (points[0], points[1]);
        let _ = write!(
            d,
            "M{},{}L{},{}Z",
            n((a[0] + 2.0 * b[0]) / 3.0),
            n((a[1] + 2.0 * b[1]) / 3.0),
            n((b[0] + 2.0 * a[0]) / 3.0),
            n((b[1] + 2.0 * a[1]) / 3.0)
        );
        return d;
    }
    let p = |i: usize| points[i % m];
    let start = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        [
            (a[0] + 4.0 * b[0] + c[0]) / 6.0,
            (a[1] + 4.0 * b[1] + c[1]) / 6.0,
        ]
    };
    let s = start(p(0), p(1), p(2));
    let _ = write!(d, "M{},{}", n(s[0]), n(s[1]));
    // After the first three points, each point adds one curve. The last
    // three curves close the ring through the first three points again.
    for i in 3..m + 3 {
        let (x0, x1, x) = (p(i - 2), p(i - 1), p(i));
        let _ = write!(
            d,
            "C{},{} {},{} {},{}",
            n((2.0 * x0[0] + x1[0]) / 3.0),
            n((2.0 * x0[1] + x1[1]) / 3.0),
            n((x0[0] + 2.0 * x1[0]) / 3.0),
            n((x0[1] + 2.0 * x1[1]) / 3.0),
            n((x0[0] + 4.0 * x1[0] + x[0]) / 6.0),
            n((x0[1] + 4.0 * x1[1] + x[1]) / 6.0)
        );
    }
    d
}

/// The Catmull-Rom curve of d3 (`curveCatmullRom.alpha(alpha)`) through
/// an open line. The curve passes through each point.
pub fn catmull_rom(points: &[[f64; 2]], alpha: f64) -> String {
    let mut d = String::new();
    catmull_rom_into(&mut d, points, alpha, true);
    d
}

/// Write a Catmull-Rom curve. With `move_to` false, the first point is not
/// written; the curve goes on from the current point.
pub fn catmull_rom_into(d: &mut String, points: &[[f64; 2]], alpha: f64, move_to: bool) {
    const EPS: f64 = 1e-12;
    let m = points.len();
    if m == 0 {
        return;
    }
    if move_to {
        let _ = write!(d, "M{},{}", n(points[0][0]), n(points[0][1]));
    }
    if m == 1 {
        return;
    }
    if m == 2 {
        let _ = write!(d, "L{},{}", n(points[1][0]), n(points[1][1]));
        return;
    }
    let len_a = |a: [f64; 2], b: [f64; 2]| -> (f64, f64) {
        let l2 = (a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]);
        let l_2a = libm::pow(l2, alpha);
        (l_2a.sqrt(), l_2a)
    };
    // Segment k goes from points[k] to points[k + 1]; the end points repeat.
    for k in 0..m - 1 {
        let p0 = if k == 0 { points[0] } else { points[k - 1] };
        let p1 = points[k];
        let p2 = points[k + 1];
        let p3 = if k + 2 < m {
            points[k + 2]
        } else {
            points[m - 1]
        };
        let (l01_a, l01_2a) = if k == 0 { (0.0, 0.0) } else { len_a(p0, p1) };
        let (l12_a, l12_2a) = len_a(p1, p2);
        let (l23_a, l23_2a) = if k + 2 < m { len_a(p2, p3) } else { (0.0, 0.0) };
        let mut c1 = p1;
        if l01_a > EPS {
            let a = 2.0 * l01_2a + 3.0 * l01_a * l12_a + l12_2a;
            let nn = 3.0 * l01_a * (l01_a + l12_a);
            c1 = [
                (p1[0] * a - p0[0] * l12_2a + p2[0] * l01_2a) / nn,
                (p1[1] * a - p0[1] * l12_2a + p2[1] * l01_2a) / nn,
            ];
        }
        let mut c2 = p2;
        if l23_a > EPS {
            let b = 2.0 * l23_2a + 3.0 * l23_a * l12_a + l12_2a;
            let mm = 3.0 * l23_a * (l23_a + l12_a);
            c2 = [
                (p2[0] * b + p1[0] * l23_2a - p3[0] * l12_2a) / mm,
                (p2[1] * b + p1[1] * l23_2a - p3[1] * l12_2a) / mm,
            ];
        }
        let _ = write!(
            d,
            "C{},{} {},{} {},{}",
            n(c1[0]),
            n(c1[1]),
            n(c2[0]),
            n(c2[1]),
            n(p2[0]),
            n(p2[1])
        );
    }
}

/// Leave out points that are nearer than `tolerance` to the line, as
/// simplify-js does: first by distance to the point before, then with the
/// Douglas-Peucker method.
pub fn simplify(points: &[[f64; 2]], tolerance: f64) -> Vec<[f64; 2]> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let sq = tolerance * tolerance;
    let d2 = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
    // Radial distance.
    let mut radial = vec![points[0]];
    let mut prev = points[0];
    for &p in &points[1..] {
        if d2(p, prev) > sq {
            radial.push(p);
            prev = p;
        }
    }
    let last = points[points.len() - 1];
    if prev != last {
        radial.push(last);
    }
    // Douglas-Peucker.
    let seg_d2 = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (mut x, mut y) = (a[0], a[1]);
        let (dx, dy) = (b[0] - x, b[1] - y);
        if dx != 0.0 || dy != 0.0 {
            let t = ((p[0] - x) * dx + (p[1] - y) * dy) / (dx * dx + dy * dy);
            if t > 1.0 {
                x = b[0];
                y = b[1];
            } else if t > 0.0 {
                x += dx * t;
                y += dy * t;
            }
        }
        (p[0] - x).powi(2) + (p[1] - y).powi(2)
    };
    let n = radial.len();
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0usize, n - 1)];
    while let Some((first, lastk)) = stack.pop() {
        let mut max = sq;
        let mut index = 0;
        for i in first + 1..lastk {
            let dd = seg_d2(radial[i], radial[first], radial[lastk]);
            if dd > max {
                index = i;
                max = dd;
            }
        }
        if index > 0 {
            keep[index] = true;
            if index - first > 1 {
                stack.push((first, index));
            }
            if lastk - index > 1 {
                stack.push((index, lastk));
            }
        }
    }
    radial
        .into_iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(p))
        .collect()
}

/// The coastline settings of the original program (`DEFAULT_COASTLINE` in
/// `coastline-generator.ts`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoastSettings {
    pub enabled: bool,
    pub max_depth: u32,
    pub base_amplitude: f64,
    pub amplitude_decay: f64,
    pub min_edge: f64,
    pub smooth_threshold: f64,
    pub roughness_contrast: f64,
    pub roughness_scale: f64,
    pub lake_smooth_thresh_mult: f64,
}

impl CoastSettings {
    pub const DEFAULT: CoastSettings = CoastSettings {
        enabled: true,
        max_depth: 4,
        base_amplitude: 1.5,
        amplitude_decay: 0.9,
        min_edge: 1.0,
        smooth_threshold: 0.25,
        roughness_contrast: 1.5,
        roughness_scale: 60.0,
        lake_smooth_thresh_mult: 2.0,
    };

    /// The settings of a lake shore: calmer.
    pub fn lake(self) -> CoastSettings {
        CoastSettings {
            smooth_threshold: (self.smooth_threshold * self.lake_smooth_thresh_mult).min(1.0),
            ..self
        }
    }
}

const QUANTUM: f64 = 64.0;
const OCTAVE_WEIGHT: f64 = 0.35;
const FIELD_STRETCH: f64 = 1.9;
/// The points of an outline nearer than this, in pixels, are left out.
pub const SIMPLIFICATION_TOLERANCE: f64 = 0.3;

/// A value in [0, 1) for a list of whole numbers (murmur3-style mixing).
fn noise(seed: u32, values: &[i64]) -> f64 {
    let mut h = seed;
    for &v in values {
        h = (h ^ (v as i32 as u32)).wrapping_mul(0x5bd1_e995);
        h ^= h >> 13;
    }
    h = (h ^ (h >> 16)).wrapping_mul(0x85eb_ca6b);
    h = (h ^ (h >> 13)).wrapping_mul(0xc2b2_ae35);
    f64::from(h ^ (h >> 16)) / 4_294_967_296.0
}

fn field_at(seed: u32, x: f64, y: f64, scale: f64) -> f64 {
    let (fx, fy) = (x / scale, y / scale);
    let (ix, iy) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - ix, fy - iy);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let (ix, iy) = (ix as i64, iy as i64);
    let corner = |cx: i64, cy: i64| noise(seed, &[cx, cy]);
    let top = corner(ix, iy) * (1.0 - sx) + corner(ix + 1, iy) * sx;
    let bottom = corner(ix, iy + 1) * (1.0 - sx) + corner(ix + 1, iy + 1) * sx;
    top * (1.0 - sy) + bottom * sy
}

/// The roughness of the coast at a place, in [0, 1].
pub fn roughness_at(seed: u32, x: f64, y: f64, s: &CoastSettings) -> f64 {
    let scale = s.roughness_scale.max(1.0);
    let base = field_at(seed, x, y, scale);
    let detail = field_at(seed ^ 0x9e37_79b9, x, y, scale / 2.0);
    let combined = base * (1.0 - OCTAVE_WEIGHT) + detail * OCTAVE_WEIGHT;
    let spread = ((combined - 0.5) * FIELD_STRETCH + 0.5).clamp(0.0, 1.0);
    libm::pow(spread, s.roughness_contrast)
}

/// A shape with fractal detail: its points, and the index of each first
/// point in them.
#[derive(Debug, Clone, PartialEq)]
pub struct Fractal {
    pub points: Vec<[f64; 2]>,
    pub orig: Vec<usize>,
}

/// Displace a closed polygon into a coastline. The polygon is in world
/// pixels. `on_border` marks a point on the frame edge.
pub fn fractalize(
    points: &[[f64; 2]],
    seed: u32,
    s: &CoastSettings,
    on_border: &dyn Fn([f64; 2]) -> bool,
) -> Fractal {
    let n = points.len();
    let mut out = Vec::with_capacity(n * 4);
    let mut orig = Vec::with_capacity(n);
    if n < 3 || !s.enabled {
        return Fractal {
            points: points.to_vec(),
            orig: (0..n).collect(),
        };
    }
    for i in 0..n {
        orig.push(out.len());
        out.push(points[i]);
        let (a, b) = (points[i], points[(i + 1) % n]);
        if on_border(a) && on_border(b) {
            continue;
        }
        subdivide(a, b, s.max_depth, s.base_amplitude, seed, &mut out, s);
    }
    Fractal { points: out, orig }
}

fn subdivide(
    a: [f64; 2],
    b: [f64; 2],
    depth: u32,
    amplitude: f64,
    seed: u32,
    out: &mut Vec<[f64; 2]>,
    s: &CoastSettings,
) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt();
    if depth == 0 || len < s.min_edge || amplitude == 0.0 {
        return;
    }
    let (mx, my) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
    let rough = roughness_at(seed, mx, my, s);
    if rough < s.smooth_threshold {
        return;
    }
    let q = |v: f64| (v * QUANTUM).round() as i64;
    let seg = noise(
        seed,
        &[q(a[0]), q(a[1]), q(b[0]), q(b[1]), i64::from(depth)],
    );
    let disp = (seg - 0.5) * len.sqrt() * amplitude * rough;
    let mid = [mx + (-dy / len) * disp, my + (dx / len) * disp];
    let next = amplitude * s.amplitude_decay;
    subdivide(a, mid, depth - 1, next, seed, out, s);
    out.push(mid);
    subdivide(mid, b, depth - 1, next, seed, out, s);
}

/// The closed path of a shape, as the original program builds it: a
/// quadratic B-spline on smooth spans, a Catmull-Rom curve on spans with
/// detail. `map` moves each point to the output frame.
pub fn coast_path(shape: &Fractal, map: &dyn Fn([f64; 2]) -> [f64; 2], decimals: u32) -> String {
    let pts: Vec<[f64; 2]> = shape.points.iter().map(|&p| map(p)).collect();
    let nn = pts.len();
    let m = shape.orig.len();
    let mut d = String::new();
    if nn < 3 || m < 3 {
        return d;
    }
    let f = |v: f64| fixed(v, decimals);
    let smooth: Vec<bool> = (0..m)
        .map(|i| {
            let a = shape.orig[i];
            let b = shape.orig[(i + 1) % m];
            (if b > a { b - a } else { b + nn - a }) == 1
        })
        .collect();
    let p0 = pts[shape.orig[0]];
    let pl = pts[shape.orig[m - 1]];
    let mut at_mid = smooth[m - 1];
    let (sx, sy) = if at_mid {
        ((pl[0] + p0[0]) / 2.0, (pl[1] + p0[1]) / 2.0)
    } else {
        (p0[0], p0[1])
    };
    let _ = write!(d, "M{},{}", f(sx), f(sy));
    for i in 0..m {
        let ci = shape.orig[i];
        let ni = shape.orig[(i + 1) % m];
        let [cx, cy] = pts[ci];
        if smooth[i] {
            let [nx, ny] = pts[ni];
            let (mx, my) = ((cx + nx) / 2.0, (cy + ny) / 2.0);
            if at_mid {
                let _ = write!(d, "Q{},{} {},{}", f(cx), f(cy), f(mx), f(my));
            } else {
                let _ = write!(d, "L{},{}", f(mx), f(my));
            }
            at_mid = true;
        } else {
            if at_mid {
                let _ = write!(d, "L{},{}", f(cx), f(cy));
            }
            let end = if ni > ci { ni } else { ni + nn };
            for j in ci..end {
                let a = pts[j % nn];
                let b = pts[(j + 1) % nn];
                let prev = pts[(j + nn - 1) % nn];
                let next = pts[(j + 2) % nn];
                let _ = write!(
                    d,
                    "C{},{} {},{} {},{}",
                    f(a[0] + (b[0] - prev[0]) / 8.0),
                    f(a[1] + (b[1] - prev[1]) / 8.0),
                    f(b[0] - (next[0] - a[0]) / 8.0),
                    f(b[1] - (next[1] - a[1]) / 8.0),
                    f(b[0]),
                    f(b[1])
                );
            }
            at_mid = false;
        }
    }
    d.push('Z');
    d
}

/// The outline of a land or lake feature.
#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    /// The feature id.
    pub feature: u32,
    /// `true` for a lake, `false` for an island.
    pub lake: bool,
    /// For an island: `true` if no ocean or frame edge touches it, so it is
    /// in a lake.
    pub in_lake: bool,
    /// The number of water and land features around it from the ocean:
    /// 1 for an island in the ocean, 2 for a lake in it, and so on.
    pub depth: u32,
    /// The shape, in world pixels.
    pub shape: Fractal,
}

/// The outlines of all islands and lakes, with fractal detail when
/// `settings.enabled` is `true`. Each outline is the outer ring of its
/// feature, simplified as the original program does.
pub fn outlines(world: &World, unit_px: f64, settings: &CoastSettings) -> Vec<Outline> {
    use inkland_core::features::FeatureKind;
    use inkland_core::seed::{Layer, SeedTree};
    let feats = &world.features;
    let kind = |id: u32| feats.get(id).kind;
    let rings = rings(world, &|c| {
        let f = feats.of_cell[c];
        if kind(f) == FeatureKind::Ocean { 0 } else { f }
    });
    // Depth from the ocean, by a walk over the feature neighbours.
    let g = &world.grid;
    let mut next: BTreeMap<u32, std::collections::BTreeSet<u32>> = BTreeMap::new();
    for c in 0..g.len() {
        for o in g.neighbors(c) {
            let (a, b) = (feats.of_cell[c], feats.of_cell[o]);
            if a != b {
                next.entry(a).or_default().insert(b);
            }
        }
    }
    let mut depth: BTreeMap<u32, u32> = BTreeMap::new();
    let mut queue = std::collections::VecDeque::new();
    for f in &feats.list {
        if f.kind == FeatureKind::Ocean {
            depth.insert(f.id, 0);
            queue.push_back(f.id);
        }
    }
    while let Some(f) = queue.pop_front() {
        let d = depth[&f];
        for &o in next.get(&f).into_iter().flatten() {
            if let std::collections::btree_map::Entry::Vacant(e) = depth.entry(o) {
                e.insert(d + 1);
                queue.push_back(o);
            }
        }
    }
    let (w, h) = (world.width * unit_px, world.height * unit_px);
    let on_border = |p: [f64; 2]| p[0] <= 0.0 || p[1] <= 0.0 || p[0] >= w || p[1] >= h;
    let tree = SeedTree::new(world.seed);
    let mut out = Vec::new();
    for (f, list) in rings {
        // The outer ring is the ring with the largest area.
        let Some(ring) = list.iter().max_by(|a, b| {
            let (pa, pb) = (ring_points(world, a), ring_points(world, b));
            area2(&pa).abs().total_cmp(&area2(&pb).abs())
        }) else {
            continue;
        };
        let lake = kind(f) == FeatureKind::Lake;
        let in_lake = !lake
            && ring.iter().all(|e| {
                e.other != OUTSIDE && kind(feats.of_cell[e.other as usize]) == FeatureKind::Lake
            });
        let pts: Vec<[f64; 2]> = ring_points(world, ring)
            .into_iter()
            .map(|p| {
                [
                    p[0].clamp(0.0, world.width) * unit_px,
                    p[1].clamp(0.0, world.height) * unit_px,
                ]
            })
            .collect();
        let mut pts = simplify(&pts, SIMPLIFICATION_TOLERANCE);
        if pts.len() > 3 && pts.first() == pts.last() {
            pts.pop();
        }
        let s = if lake { settings.lake() } else { *settings };
        let mut rng =
            inkland_core::rand::Rand::new(tree.part_rng(Layer::Decoration, &format!("coast-{f}")));
        let seed = (rng.unit() * 4_294_967_296.0) as u32;
        out.push(Outline {
            feature: f,
            lake,
            in_lake,
            depth: depth.get(&f).copied().unwrap_or(1),
            shape: fractalize(&pts, seed, &s, &on_border),
        });
    }
    out.sort_by_key(|o| (o.depth, o.feature));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basis_closed_starts_and_ends_at_the_same_point() {
        let pts = [[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]];
        let d = basis_closed(&pts);
        assert!(d.starts_with("M5,1C"));
        assert!(d.ends_with(" 5,1"));
        assert_eq!(d.matches('C').count(), 4);
    }

    #[test]
    fn catmull_rom_passes_through_each_point() {
        let pts = [[0.0, 0.0], [10.0, 5.0], [20.0, 0.0], [30.0, 7.5]];
        let d = catmull_rom(&pts, 0.1);
        assert!(d.starts_with("M0,0C"));
        for p in &pts[1..] {
            assert!(d.contains(&format!(" {},{}", n(p[0]), n(p[1]))), "{d}");
        }
    }

    #[test]
    fn simplify_keeps_corners_and_drops_points_on_a_line() {
        let pts = [[0.0, 0.0], [1.0, 0.01], [2.0, 0.0], [2.0, 2.0]];
        let s = simplify(&pts, 0.3);
        assert_eq!(s, vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0]]);
    }

    #[test]
    fn fractal_detail_is_the_same_on_each_run_and_keeps_the_points() {
        let pts: Vec<[f64; 2]> = (0..12)
            .map(|i| {
                let a = f64::from(i) * std::f64::consts::TAU / 12.0;
                [500.0 + 80.0 * libm::cos(a), 400.0 + 80.0 * libm::sin(a)]
            })
            .collect();
        let s = CoastSettings::DEFAULT;
        let a = fractalize(&pts, 7, &s, &|_| false);
        let b = fractalize(&pts, 7, &s, &|_| false);
        assert_eq!(a, b);
        assert!(a.points.len() > pts.len());
        for (i, &k) in a.orig.iter().enumerate() {
            assert_eq!(a.points[k], pts[i]);
        }
        let off = CoastSettings {
            enabled: false,
            ..s
        };
        assert_eq!(fractalize(&pts, 7, &off, &|_| false).points, pts);
        let d = coast_path(&a, &|p| p, 2);
        assert!(d.starts_with('M') && d.ends_with('Z'));
    }

    #[test]
    fn noise_is_in_the_unit_range() {
        for i in 0..100 {
            let v = noise(i, &[i64::from(i), -3, 99]);
            assert!((0.0..1.0).contains(&v));
        }
        let s = CoastSettings::DEFAULT;
        for i in 0..50 {
            let r = roughness_at(3, f64::from(i) * 7.3, f64::from(i) * 3.1, &s);
            assert!((0.0..=1.0).contains(&r));
        }
    }
}
