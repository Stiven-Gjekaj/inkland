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
}

/// All features of a world.
#[derive(Debug, Clone, PartialEq)]
pub struct Features {
    /// The feature id of each cell.
    pub of_cell: Vec<u32>,
    pub list: Vec<Feature>,
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
            });
        }
        Features { of_cell, list }
    }

    pub fn get(&self, id: u32) -> &Feature {
        &self.list[id as usize - 1]
    }

    pub fn is_land(&self, cell: usize) -> bool {
        self.get(self.of_cell[cell]).kind.is_land()
    }
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
    fn no_land_gives_no_coastline() {
        let g = grid();
        let f = Features::mark(&g, &vec![0.0; g.len()], 20.0);
        assert_eq!(f.list.len(), 1);
        assert!(coastlines(&g, &f).is_empty());
    }
}
