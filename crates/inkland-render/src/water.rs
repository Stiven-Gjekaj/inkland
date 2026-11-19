//! The water lines of `print-safe` mode: lines in the water that follow
//! the coast, as on an engraved map. Each line is further from the last
//! line, so the spacing grows out to sea.
//!
//! The lines come from a field of the distance to the land on the page.
//! A raster over the map holds the land, the exact Euclidean distance
//! transform of Felzenszwalb and Huttenlocher gives the distance of each
//! pixel to the land, and marching squares give the line at each level of
//! `LEVELS_MM`. Thus the lines of two islands that are near join as one
//! line, and a line never crosses another line.

use crate::page::Page;
use inkland_core::world::World;
use std::collections::BTreeMap;

/// The distance of each water line from the coast, in millimetres on the
/// page. The gaps between the lines grow.
pub const LEVELS_MM: [f64; 4] = [0.7, 1.5, 2.6, 4.0];

/// The smallest pixel of the raster, in millimetres.
pub const RESOLUTION_MM: f64 = 0.2;

/// The most pixels of the raster. A larger map gets larger pixels.
pub const MAX_PIXELS: f64 = 3.0e6;

/// The distance of a point with no land, squared, in pixels.
const FAR: f64 = 1.0e20;

/// One water line, in millimetres on the page. A closed line has its
/// first point again at its end.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The index of the level in `LEVELS_MM`.
    pub level: usize,
    pub points: Vec<[f64; 2]>,
    pub closed: bool,
}

/// The raster of the map.
struct Raster {
    /// The page position of pixel (0, 0), in millimetres.
    x0: f64,
    y0: f64,
    /// The size of a pixel, in millimetres.
    res: f64,
    nx: usize,
    ny: usize,
}

impl Raster {
    fn new(page: &Page) -> Self {
        let [mx, my, mw, mh] = page.map_rect();
        let res = RESOLUTION_MM.max(libm::sqrt(mw * mh / MAX_PIXELS));
        Self {
            x0: mx,
            y0: my,
            res,
            nx: libm::ceil(mw / res) as usize + 1,
            ny: libm::ceil(mh / res) as usize + 1,
        }
    }

    fn at(&self, i: usize, j: usize) -> [f64; 2] {
        [self.x0 + i as f64 * self.res, self.y0 + j as f64 * self.res]
    }
}

/// `true` for each pixel on land. A pixel in a cell of the first grid with
/// no land around it is water; else the cell of the second grid, which
/// follows the coast more closely, decides.
fn land(world: &World, page: &Page, r: &Raster) -> Vec<bool> {
    let base = &world.base;
    let open_sea: Vec<bool> = (0..base.grid.len())
        .map(|c| {
            !base.features.is_land(c) && base.grid.neighbors(c).all(|m| !base.features.is_land(m))
        })
        .collect();
    let mut out = vec![false; r.nx * r.ny];
    for j in 0..r.ny {
        for i in 0..r.nx {
            let w = page.to_world(r.at(i, j));
            let x = w[0].clamp(0.0, world.width);
            let y = w[1].clamp(0.0, world.height);
            if open_sea[base.grid.find_cell(x, y)] {
                continue;
            }
            out[j * r.nx + i] = world.features.is_land(world.grid.find_cell(x, y));
        }
    }
    out
}

/// The squared distance transform of one row or column, in place
/// (Felzenszwalb and Huttenlocher).
fn transform_1d(f: &mut [f64], v: &mut [usize], z: &mut [f64], d: &mut [f64]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut k = 0;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        let qf = q as f64;
        // The parabolas of `f` are finite, and `z[0]` is minus infinity, so
        // `k` does not go below 0.
        let cross = |p: usize| {
            let pf = p as f64;
            ((f[q] + qf * qf) - (f[p] + pf * pf)) / (2.0 * qf - 2.0 * pf)
        };
        let mut s = cross(v[k]);
        while s <= z[k] {
            k -= 1;
            s = cross(v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, dq) in d.iter_mut().enumerate().take(n) {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k] as f64;
        *dq = (q as f64 - p) * (q as f64 - p) + f[v[k]];
    }
    f.copy_from_slice(&d[..n]);
}

/// The distance of each pixel to the nearest land pixel, in millimetres.
fn distance(land: &[bool], r: &Raster) -> Vec<f64> {
    let (nx, ny) = (r.nx, r.ny);
    let mut g: Vec<f64> = land.iter().map(|&l| if l { 0.0 } else { FAR }).collect();
    let n = nx.max(ny);
    let (mut f, mut d) = (vec![0.0; n], vec![0.0; n]);
    let (mut v, mut z) = (vec![0usize; n], vec![0.0; n + 1]);
    for j in 0..ny {
        f[..nx].copy_from_slice(&g[j * nx..(j + 1) * nx]);
        transform_1d(&mut f[..nx], &mut v, &mut z, &mut d);
        g[j * nx..(j + 1) * nx].copy_from_slice(&f[..nx]);
    }
    for i in 0..nx {
        for j in 0..ny {
            f[j] = g[j * nx + i];
        }
        transform_1d(&mut f[..ny], &mut v, &mut z, &mut d);
        for j in 0..ny {
            g[j * nx + i] = f[j];
        }
    }
    g.iter().map(|&q| libm::sqrt(q) * r.res).collect()
}

/// The segments of the line where `value` is 0, with marching squares.
/// Each end of a segment is an edge of the raster: `2 * pixel` for the
/// edge to the right of the pixel, `2 * pixel + 1` for the edge below it.
/// `points` gets the point on each edge.
fn segments(
    value: &[f64],
    r: &Raster,
    points: &mut BTreeMap<usize, [f64; 2]>,
) -> Vec<(usize, usize)> {
    let nx = r.nx;
    let v = |i: usize, j: usize| value[j * nx + i];
    let mut out = Vec::new();
    for j in 0..r.ny.saturating_sub(1) {
        for i in 0..nx.saturating_sub(1) {
            let (a, b, c, d) = (v(i, j), v(i + 1, j), v(i + 1, j + 1), v(i, j + 1));
            let case = usize::from(a < 0.0)
                | usize::from(b < 0.0) << 1
                | usize::from(c < 0.0) << 2
                | usize::from(d < 0.0) << 3;
            if case == 0 || case == 15 {
                continue;
            }
            // The four edges of the square, with the point where the value
            // is 0 on each.
            let cut = |p: f64, q: f64| p / (p - q);
            let top = 2 * (j * nx + i);
            let right = 2 * (j * nx + i + 1) + 1;
            let bottom = 2 * ((j + 1) * nx + i);
            let left = 2 * (j * nx + i) + 1;
            let [x, y] = r.at(i, j);
            let s = r.res;
            let mut put = |e: usize| {
                points.entry(e).or_insert_with(|| {
                    if e == top {
                        [x + s * cut(a, b), y]
                    } else if e == right {
                        [x + s, y + s * cut(b, c)]
                    } else if e == bottom {
                        [x + s * cut(d, c), y + s]
                    } else {
                        [x, y + s * cut(a, d)]
                    }
                });
                e
            };
            let center = (a + b + c + d) / 4.0 < 0.0;
            let pairs: &[(usize, usize)] = match case {
                1 | 14 => &[(left, top)],
                2 | 13 => &[(top, right)],
                3 | 12 => &[(left, right)],
                4 | 11 => &[(right, bottom)],
                6 | 9 => &[(top, bottom)],
                7 | 8 => &[(left, bottom)],
                5 if center => &[(top, right), (bottom, left)],
                5 => &[(left, top), (right, bottom)],
                10 if center => &[(left, top), (right, bottom)],
                _ => &[(top, right), (bottom, left)],
            };
            for &(e1, e2) in pairs {
                out.push((put(e1), put(e2)));
            }
        }
    }
    out
}

/// Join the segments into lines. Each edge of the raster joins at most two
/// segments.
fn chain(
    segs: &[(usize, usize)],
    points: &BTreeMap<usize, [f64; 2]>,
) -> Vec<(Vec<[f64; 2]>, bool)> {
    let mut at: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (k, &(a, b)) in segs.iter().enumerate() {
        at.entry(a).or_default().push(k);
        at.entry(b).or_default().push(k);
    }
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    // Walk from the edge `from` on, away from the segment `k`.
    let walk = |used: &mut Vec<bool>, mut k: usize, mut from: usize| -> Vec<usize> {
        let mut edges = Vec::new();
        loop {
            let next = at[&from].iter().copied().find(|&m| m != k && !used[m]);
            let Some(m) = next else {
                break;
            };
            used[m] = true;
            let (a, b) = segs[m];
            from = if a == from { b } else { a };
            edges.push(from);
            k = m;
        }
        edges
    };
    for s in 0..segs.len() {
        if used[s] {
            continue;
        }
        used[s] = true;
        let (a, b) = segs[s];
        let forward = walk(&mut used, s, b);
        let closed = forward.last() == Some(&a);
        let mut edges = Vec::new();
        if !closed {
            let mut back = walk(&mut used, s, a);
            back.reverse();
            edges.extend(back);
        }
        edges.push(a);
        edges.push(b);
        edges.extend(forward);
        let pts: Vec<[f64; 2]> = edges.iter().map(|e| points[e]).collect();
        out.push((pts, closed));
    }
    out
}

/// The water lines of a map, in millimetres on the page, from the line
/// nearest to the coast out.
pub fn lines(world: &World, page: &Page) -> Vec<Line> {
    let r = Raster::new(page);
    let land = land(world, page, &r);
    if !land.iter().any(|&l| l) {
        return Vec::new();
    }
    let dist = distance(&land, &r);
    let mut out = Vec::new();
    for (level, &at) in LEVELS_MM.iter().enumerate() {
        let value: Vec<f64> = dist.iter().map(|&d| d - at).collect();
        let mut points = BTreeMap::new();
        let segs = segments(&value, &r, &mut points);
        for (pts, closed) in chain(&segs, &points) {
            let pts = crate::fmg::geom::simplify(&pts, r.res * 0.25);
            if pts.len() < 3 {
                continue;
            }
            out.push(Line {
                level,
                points: pts,
                closed,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster(n: usize) -> Raster {
        Raster {
            x0: 0.0,
            y0: 0.0,
            res: 1.0,
            nx: n,
            ny: n,
        }
    }

    #[test]
    fn the_distance_is_euclidean() {
        let r = raster(9);
        let mut land = vec![false; 81];
        land[4 * 9 + 4] = true;
        let d = distance(&land, &r);
        assert_eq!(d[4 * 9 + 4], 0.0);
        assert_eq!(d[4 * 9 + 7], 3.0);
        assert!((d[0] - 32f64.sqrt()).abs() < 1e-12);
        assert!((d[7 * 9 + 8] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn a_line_around_an_island_is_closed_and_round() {
        let r = raster(41);
        let mut land = vec![false; 41 * 41];
        land[20 * 41 + 20] = true;
        let d = distance(&land, &r);
        let value: Vec<f64> = d.iter().map(|&q| q - 10.0).collect();
        let mut points = BTreeMap::new();
        let segs = segments(&value, &r, &mut points);
        let lines = chain(&segs, &points);
        assert_eq!(lines.len(), 1);
        let (pts, closed) = &lines[0];
        assert!(*closed);
        assert_eq!(pts.first(), pts.last());
        for p in pts {
            let rad = ((p[0] - 20.0).powi(2) + (p[1] - 20.0).powi(2)).sqrt();
            assert!((rad - 10.0).abs() < 0.2, "{rad}");
        }
    }

    #[test]
    fn a_line_that_meets_the_edge_is_open() {
        let r = raster(30);
        let mut land = vec![false; 900];
        for j in 0..30 {
            land[j * 30] = true;
        }
        let d = distance(&land, &r);
        let value: Vec<f64> = d.iter().map(|&q| q - 5.5).collect();
        let mut points = BTreeMap::new();
        let lines = chain(&segments(&value, &r, &mut points), &points);
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].1);
        assert!(lines[0].0.iter().all(|p| (p[0] - 5.5).abs() < 1e-9));
        assert_eq!(lines[0].0.len(), 30);
    }

    #[test]
    fn the_gaps_between_the_lines_grow() {
        let gaps: Vec<f64> = LEVELS_MM.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(gaps.windows(2).all(|g| g[0] < g[1]));
    }
}
