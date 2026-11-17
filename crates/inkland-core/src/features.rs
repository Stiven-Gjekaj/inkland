//! Features and coastlines.
//!
//! A feature is a connected group of land cells (an island) or of water
//! cells. Water that touches the frame edge is ocean. Other water is a lake.
//!
//! A coastline is the line between a land feature and a water feature. It
//! follows the edges of the cell polygons.

use crate::grid::{Grid, OUTSIDE};
use std::collections::VecDeque;

/// The kind of a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FeatureKind {
    Ocean,
    Lake,
    Island,
}

impl FeatureKind {
    pub fn name(self) -> &'static str {
        match self {
            FeatureKind::Ocean => "ocean",
            FeatureKind::Lake => "lake",
            FeatureKind::Island => "island",
        }
    }

    pub fn is_land(self) -> bool {
        self == FeatureKind::Island
    }
}

/// One feature.
#[derive(Debug, Clone, PartialEq)]
pub struct Feature {
    /// 1, 2, 3, ... in the order of the first cell of each feature.
    pub id: u32,
    pub kind: FeatureKind,
    /// The lowest cell index in the feature.
    pub first_cell: u32,
    /// Number of cells.
    pub cells: u32,
    /// `true` if a cell of the feature touches the frame edge.
    pub border: bool,
    /// The area of the cells, in square units. Only after `mark_pack`.
    pub area: f64,
    /// The outer outline: a closed loop of vertex indices, first and last
    /// the same. Empty for an ocean, and before `mark_pack`.
    pub outline: Vec<u32>,
    /// The land cells next to a lake, in cell order. Empty for other
    /// features.
    pub shoreline: Vec<usize>,
    /// The surface height of a lake: 0.1 below its lowest shore cell, as in
    /// FMG `Lakes.getHeight`. 0 for other features.
    pub height: f64,
}

/// No cell: a land cell with no water next to it has no haven.
pub const NO_HAVEN: u32 = u32::MAX;

/// All features of a world.
#[derive(Debug, Clone, PartialEq)]
pub struct Features {
    /// The feature id of each cell.
    pub of_cell: Vec<u32>,
    pub list: Vec<Feature>,
    /// The distance of each cell to the coast (FMG `cells.t`): 1 for land
    /// next to water, 2 for land next to that, then 3, 4, ...; -1 for water
    /// next to land, then -2 to -9. Empty before `mark_pack`.
    pub distance: Vec<i8>,
    /// The nearest water cell next to each coastal land cell (FMG
    /// `cells.haven`), else `NO_HAVEN`. Empty before `mark_pack`.
    pub haven: Vec<u32>,
    /// The number of water cells next to each land cell (FMG
    /// `cells.harbor`). Empty before `mark_pack`.
    pub harbor: Vec<u8>,
}

impl Features {
    /// Group the cells into features. Land is height `sea_level` or more.
    pub fn mark(grid: &Grid, heights: &[f64], sea_level: f64) -> Features {
        let n = grid.len();
        let land = |c: usize| heights[c] >= sea_level;
        let mut of_cell = vec![0u32; n];
        let mut list = Vec::new();
        for first in 0..n {
            if of_cell[first] != 0 {
                continue;
            }
            let id = list.len() as u32 + 1;
            let is_land = land(first);
            let mut count = 0u32;
            let mut border = false;
            of_cell[first] = id;
            let mut queue = VecDeque::from([first]);
            while let Some(c) = queue.pop_front() {
                count += 1;
                border |= grid.border[c];
                for m in grid.neighbors(c) {
                    if of_cell[m] == 0 && land(m) == is_land {
                        of_cell[m] = id;
                        queue.push_back(m);
                    }
                }
            }
            let kind = match (is_land, border) {
                (true, _) => FeatureKind::Island,
                (false, true) => FeatureKind::Ocean,
                (false, false) => FeatureKind::Lake,
            };
            list.push(Feature {
                id,
                kind,
                first_cell: first as u32,
                cells: count,
                border,
                area: 0.0,
                outline: Vec::new(),
                shoreline: Vec::new(),
                height: 0.0,
            });
        }
        Features {
            of_cell,
            list,
            distance: Vec::new(),
            haven: Vec::new(),
            harbor: Vec::new(),
        }
    }

    /// Group the cells of the second grid into features, and add the data
    /// of FMG `markupPack`: the distance field, the haven and the harbor of
    /// each coastal cell, and the area, the outline, the shoreline, and the
    /// lake height of each feature.
    pub fn mark_pack(grid: &Grid, heights: &[f64], sea_level: f64) -> Features {
        let mut f = Features::mark(grid, heights, sea_level);
        let n = grid.len();
        let land = |c: usize| heights[c] >= sea_level;
        let mut t = vec![0i8; n];
        let mut haven = vec![NO_HAVEN; n];
        let mut harbor = vec![0u8; n];
        for c in 0..n {
            if !land(c) {
                continue;
            }
            let p = grid.points[c];
            let mut best: Option<(f64, usize)> = None;
            for m in grid.neighbors(c).filter(|&m| !land(m)) {
                t[c] = 1;
                t[m] = -1;
                harbor[c] = harbor[c].saturating_add(1);
                let q = grid.points[m];
                let d = (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, m));
                }
            }
            if let Some((_, m)) = best {
                haven[c] = m as u32;
            }
        }
        for c in 0..n {
            if land(c) && t[c] == 0 && grid.neighbors(c).any(|m| t[m] == 1) {
                t[c] = 2;
            }
        }
        crate::lakes::mark_rings(grid, &mut t, 3, 1, i8::MAX);
        crate::lakes::mark_rings(grid, &mut t, -2, -1, -10);

        let mut area = vec![0.0; f.list.len()];
        for c in 0..n {
            area[f.of_cell[c] as usize - 1] += grid.area(c);
        }
        let mut shore: Vec<Vec<usize>> = vec![Vec::new(); f.list.len()];
        for c in 0..n {
            if land(c) {
                for m in grid.neighbors(c) {
                    let k = f.of_cell[m] as usize - 1;
                    if f.list[k].kind == FeatureKind::Lake && shore[k].last() != Some(&c) {
                        shore[k].push(c);
                    }
                }
            }
        }
        for (k, feature) in f.list.iter_mut().enumerate() {
            feature.area = area[k];
            if feature.kind != FeatureKind::Ocean {
                let id = feature.id;
                feature.outline = outlines(grid, &|c| f.of_cell[c] == id)
                    .into_iter()
                    .max_by(|a, b| {
                        loop_area(grid, a)
                            .total_cmp(&loop_area(grid, b))
                            .then(b.len().cmp(&a.len()))
                    })
                    .unwrap_or_default();
            }
            if feature.kind == FeatureKind::Lake {
                let mut cells = std::mem::take(&mut shore[k]);
                cells.sort_unstable();
                cells.dedup();
                let lowest = cells
                    .iter()
                    .map(|&c| heights[c])
                    .fold(f64::INFINITY, f64::min);
                let lowest = if lowest.is_finite() {
                    lowest
                } else {
                    sea_level
                };
                feature.height = ((lowest - 0.1) * 100.0).round() / 100.0;
                feature.shoreline = cells;
            }
        }
        f.distance = t;
        f.haven = haven;
        f.harbor = harbor;
        f
    }

    pub fn get(&self, id: u32) -> &Feature {
        &self.list[id as usize - 1]
    }

    pub fn is_land(&self, cell: usize) -> bool {
        self.get(self.of_cell[cell]).kind.is_land()
    }
}

/// The outlines of a set of cells: closed loops of vertex indices, first
/// and last the same. An edge next to the outside of the frame is a part
/// of the outline. The cells of the set are on the left of each loop.
pub fn outlines(grid: &Grid, inside: &dyn Fn(usize) -> bool) -> Vec<Vec<u32>> {
    // Each Voronoi vertex is on three cells, so it has at most one outline
    // edge out.
    let mut out: Vec<u32> = vec![u32::MAX; grid.vertices.len()];
    let mut starts = Vec::new();
    for c in 0..grid.len() {
        if !inside(c) {
            continue;
        }
        let poly = &grid.polygons[c];
        let k = poly.len();
        for (i, &other) in grid.edge_cells[c].iter().enumerate() {
            if other != OUTSIDE && inside(other as usize) {
                continue;
            }
            out[poly[i] as usize] = poly[(i + 1) % k];
            starts.push(poly[i]);
        }
    }
    starts.sort_unstable();
    let mut used = vec![false; out.len()];
    let mut loops = Vec::new();
    for start in starts {
        if used[start as usize] {
            continue;
        }
        let mut ring = vec![start];
        let mut v = start;
        loop {
            used[v as usize] = true;
            v = out[v as usize];
            ring.push(v);
            if v == start || v == u32::MAX || used[v as usize] {
                break;
            }
        }
        if *ring.last().expect("a ring has a start") == start {
            loops.push(ring);
        }
    }
    loops
}

/// The area that a closed loop of vertices encloses, in square units.
pub fn loop_area(grid: &Grid, ring: &[u32]) -> f64 {
    let mut sum = 0.0;
    for w in ring.windows(2) {
        let a = grid.vertices[w[0] as usize];
        let b = grid.vertices[w[1] as usize];
        sum += a[0] * b[1] - b[0] * a[1];
    }
    (sum / 2.0).abs()
}

/// A coastline: a line between one land feature and one water feature.
#[derive(Debug, Clone, PartialEq)]
pub struct Coastline {
    /// 1, 2, 3, ... in the order of the lowest vertex of each line.
    pub id: u32,
    /// The land feature on one side.
    pub land: u32,
    /// The water feature on the other side.
    pub water: u32,
    /// The points of the line, in units. The land is on the same side of
    /// each line.
    pub points: Vec<[f64; 2]>,
    /// `true` if the line is a closed loop. An open line starts and ends at
    /// the frame edge.
    pub closed: bool,
}

/// Find all coastlines.
pub fn coastlines(grid: &Grid, features: &Features) -> Vec<Coastline> {
    // Each edge of a land cell next to water or to the outside is a boundary
    // edge. Each Voronoi vertex is on three cells, so it has at most one
    // boundary edge out.
    #[derive(Clone, Copy)]
    struct Out {
        to: u32,
        land: u32,
        /// The water feature, or 0 for an edge next to the outside.
        water: u32,
    }
    let mut out: Vec<Option<Out>> = vec![None; grid.vertices.len()];
    for c in 0..grid.len() {
        if !features.is_land(c) {
            continue;
        }
        let poly = &grid.polygons[c];
        let k = poly.len();
        for (i, &other) in grid.edge_cells[c].iter().enumerate() {
            let water = if other == OUTSIDE {
                0
            } else if features.is_land(other as usize) {
                continue;
            } else {
                features.of_cell[other as usize]
            };
            let from = poly[i] as usize;
            debug_assert!(
                out[from].is_none(),
                "vertex {from} has two boundary edges out"
            );
            out[from] = Some(Out {
                to: poly[(i + 1) % k],
                land: features.of_cell[c],
                water,
            });
        }
    }

    let mut used = vec![false; out.len()];
    let mut lines = Vec::new();
    for start in 0..out.len() {
        if used[start] || out[start].is_none() {
            continue;
        }
        // Walk the loop once and collect its edges.
        let mut edges: Vec<(u32, Out)> = Vec::new();
        let mut v = start;
        while !used[v] {
            used[v] = true;
            let e = out[v].expect("a loop has no dead end");
            edges.push((v as u32, e));
            v = e.to as usize;
        }
        let point = |v: u32| grid.vertices[v as usize];
        let outside_at = edges.iter().position(|(_, e)| e.water == 0);
        match outside_at {
            None => {
                let mut pts: Vec<[f64; 2]> = edges.iter().map(|&(v, _)| point(v)).collect();
                pts.push(pts[0]);
                lines.push(Coastline {
                    id: 0,
                    land: edges[0].1.land,
                    water: edges[0].1.water,
                    points: pts,
                    closed: true,
                });
            }
            Some(at) => {
                // Start after an outside edge, then cut at each outside edge.
                edges.rotate_left(at + 1);
                let mut cur: Vec<[f64; 2]> = Vec::new();
                let mut meta = (0, 0);
                for &(v, e) in &edges {
                    if e.water == 0 {
                        if !cur.is_empty() {
                            cur.push(point(v));
                            lines.push(Coastline {
                                id: 0,
                                land: meta.0,
                                water: meta.1,
                                points: std::mem::take(&mut cur),
                                closed: false,
                            });
                        }
                        continue;
                    }
                    if cur.is_empty() {
                        meta = (e.land, e.water);
                    }
                    cur.push(point(v));
                }
                if !cur.is_empty() {
                    let (v, _) = edges[0];
                    cur.push(point(v));
                    lines.push(Coastline {
                        id: 0,
                        land: meta.0,
                        water: meta.1,
                        points: cur,
                        closed: false,
                    });
                }
            }
        }
    }
    for (i, line) in lines.iter_mut().enumerate() {
        line.id = i as u32 + 1;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rand::Rand;
    use crate::seed::{Layer, SeedTree};

    fn grid() -> Grid {
        let mut rng = Rand::new(SeedTree::new(5).part_rng(Layer::Terrain, "grid"));
        Grid::new(400.0, 300.0, 3000, &mut rng)
    }

    /// Land inside a circle of radius `r` around `(cx, cy)`.
    fn disc(g: &Grid, cx: f64, cy: f64, r: f64) -> Vec<f64> {
        g.points
            .iter()
            .map(|p| {
                let d = ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt();
                if d < r { 50.0 } else { 0.0 }
            })
            .collect()
    }

    #[test]
    fn one_island_in_the_ocean() {
        let g = grid();
        let h = disc(&g, 200.0, 150.0, 60.0);
        let f = Features::mark(&g, &h, 20.0);
        let kinds: Vec<FeatureKind> = f.list.iter().map(|x| x.kind).collect();
        assert_eq!(kinds, vec![FeatureKind::Ocean, FeatureKind::Island]);
        let lines = coastlines(&g, &f);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].closed);
        assert_eq!(lines[0].points.first(), lines[0].points.last());
    }

    #[test]
    fn a_lake_inside_an_island_is_a_lake_with_its_own_shore() {
        let g = grid();
        let mut h = disc(&g, 200.0, 150.0, 100.0);
        for (c, p) in g.points.iter().enumerate() {
            if ((p[0] - 200.0).powi(2) + (p[1] - 150.0).powi(2)).sqrt() < 25.0 {
                h[c] = 5.0;
            }
        }
        let f = Features::mark(&g, &h, 20.0);
        let mut kinds: Vec<FeatureKind> = f.list.iter().map(|x| x.kind).collect();
        kinds.sort();
        assert_eq!(
            kinds,
            vec![FeatureKind::Ocean, FeatureKind::Lake, FeatureKind::Island]
        );
        let lines = coastlines(&g, &f);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.closed));
        let waters: Vec<FeatureKind> = lines.iter().map(|l| f.get(l.water).kind).collect();
        assert!(waters.contains(&FeatureKind::Lake) && waters.contains(&FeatureKind::Ocean));
    }

    #[test]
    fn land_on_the_frame_edge_gives_an_open_line() {
        let g = grid();
        // Land on the left half of the map.
        let h: Vec<f64> = g
            .points
            .iter()
            .map(|p| if p[0] < 200.0 { 50.0 } else { 0.0 })
            .collect();
        let f = Features::mark(&g, &h, 20.0);
        let lines = coastlines(&g, &f);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert!(!line.closed);
        // The line goes from the top edge to the bottom edge, near x = 200.
        let ys: Vec<f64> = [line.points[0][1], line.points[line.points.len() - 1][1]].to_vec();
        assert!(
            ys.iter().any(|&y| y < 10.0) && ys.iter().any(|&y| y > 290.0),
            "{ys:?}"
        );
        assert!(line.points.iter().all(|p| (p[0] - 200.0).abs() < 15.0));
    }

    #[test]
    fn the_pack_markup_gives_havens_harbors_and_distances() {
        let g = grid();
        let mut h = disc(&g, 200.0, 150.0, 100.0);
        for (c, p) in g.points.iter().enumerate() {
            if ((p[0] - 200.0).powi(2) + (p[1] - 150.0).powi(2)).sqrt() < 25.0 {
                h[c] = 5.0;
            }
        }
        let f = Features::mark_pack(&g, &h, 20.0);
        let d2 = |a: usize, b: usize| {
            (g.points[a][0] - g.points[b][0]).powi(2) + (g.points[a][1] - g.points[b][1]).powi(2)
        };
        for c in 0..g.len() {
            let water: Vec<usize> = g.neighbors(c).filter(|&m| h[m] < 20.0).collect();
            if h[c] >= 20.0 {
                assert_eq!(f.harbor[c] as usize, water.len());
                if water.is_empty() {
                    assert_eq!(f.haven[c], NO_HAVEN);
                    assert!(f.distance[c] >= 2, "{}", f.distance[c]);
                    let lower = g.neighbors(c).any(|m| f.distance[m] == f.distance[c] - 1);
                    assert!(lower, "land cell {c} at distance {}", f.distance[c]);
                } else {
                    assert_eq!(f.distance[c], 1);
                    let haven = f.haven[c] as usize;
                    assert!(water.contains(&haven));
                    assert!(water.iter().all(|&m| d2(c, haven) <= d2(c, m)));
                }
            } else {
                assert!(f.distance[c] <= 0);
                assert_eq!(f.haven[c], NO_HAVEN);
            }
        }
        let lake = f.list.iter().find(|x| x.kind == FeatureKind::Lake).unwrap();
        assert!(!lake.shoreline.is_empty());
        for &c in &lake.shoreline {
            assert!(h[c] >= 20.0);
            assert!(g.neighbors(c).any(|m| f.of_cell[m] == lake.id));
        }
        // The lake is 0.1 below its lowest shore cell.
        assert_eq!(lake.height, 49.9);
        // The outline of a feature is closed and encloses about its area.
        for x in f.list.iter().filter(|x| x.kind != FeatureKind::Ocean) {
            assert_eq!(x.outline.first(), x.outline.last());
            let a = loop_area(&g, &x.outline);
            if x.kind == FeatureKind::Lake {
                assert!((a - x.area).abs() < 1e-6 * x.area, "{a} {}", x.area);
            } else {
                // The island outline also holds the lake inside it.
                assert!(a > x.area, "{a} {}", x.area);
            }
        }
        let total: f64 = f.list.iter().map(|x| x.area).sum();
        assert!(total > 400.0 * 300.0);
    }

    #[test]
    fn no_land_gives_no_coastline() {
        let g = grid();
        let f = Features::mark(&g, &vec![0.0; g.len()], 20.0);
        assert_eq!(f.list.len(), 1);
        assert!(coastlines(&g, &f).is_empty());
    }
}
