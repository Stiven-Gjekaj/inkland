//! The second grid (FMG `pack`): finer at the coasts, without deep water.
//!
//! This follows Azgaar's Fantasy Map Generator (FMG,
//! `model/graph/pack-generator.ts`). The second grid keeps the sites of the
//! land cells and of the coastal water cells of the first grid. It keeps
//! three of each four cells of the second ring of water, and no deeper
//! water. Between two coastal cells of the same kind that are far apart,
//! it adds a site at the middle. The new cells get the height of the first
//! cell of the pair. Then it makes the Voronoi cells again.
//!
//! Pins win: each exact site (a pinned place or a pinned river point) stays
//! a site of the second grid, also in deep water.

use crate::features::{FeatureKind, Features};
use crate::grid::Grid;
use crate::lakes::grid_distance;
use std::collections::BTreeSet;

/// The second grid and the data that it takes from the first grid.
pub struct Pack {
    pub grid: Grid,
    /// The first-grid cell of each cell (FMG `pack.cells.g`).
    pub base_cell: Vec<usize>,
    /// The height of each cell.
    pub heights: Vec<f64>,
    /// The cell of each first-grid cell that keeps its site, or `None`.
    pub cell_of_base: Vec<Option<usize>>,
}

/// Make the second grid. `keep` marks the first-grid cells that must stay.
pub fn regraph(base: &Grid, heights: &[f64], sea_level: f64, keep: &[bool]) -> Pack {
    let n = base.len();
    let t = grid_distance(base, heights, sea_level);
    let features = Features::mark(base, heights, sea_level);
    let lake = |c: usize| features.get(features.of_cell[c]).kind == FeatureKind::Lake;
    // Coastal pairs farther apart than this get a site between them. The
    // more regular the lattice, the fewer pairs get one.
    let split =
        base.lattice.neighbor_distance * (1.0 + (crate::grid::JITTER - base.lattice.jitter));
    let split2 = split * split;

    let mut points: Vec<[f64; 2]> = Vec::with_capacity(n);
    let mut base_cell = Vec::with_capacity(n);
    let mut pack_heights = Vec::with_capacity(n);
    let mut cell_of_base = vec![None; n];
    let mut new_points: Vec<(usize, [f64; 2])> = Vec::new();
    for i in 0..n {
        let (h, kind) = (heights[i], t[i]);
        if !keep[i] {
            if h < sea_level && kind != -1 && kind != -2 {
                continue;
            }
            if kind == -2 && (i % 4 == 0 || lake(i)) {
                continue;
            }
        }
        cell_of_base[i] = Some(points.len());
        points.push(base.points[i]);
        base_cell.push(i);
        pack_heights.push(h);
        if (kind == 1 || kind == -1) && !base.border[i] {
            let [x, y] = base.points[i];
            for e in base.neighbors(i) {
                if i > e || t[e] != kind {
                    continue;
                }
                let [ex, ey] = base.points[e];
                let d2 = (y - ey) * (y - ey) + (x - ex) * (x - ex);
                if d2 < split2 {
                    continue;
                }
                new_points.push((i, [(x + ex) / 2.0, (y + ey) / 2.0]));
            }
        }
    }
    // FMG adds each new site right after its first cell. The order of the
    // sites does not change the cells, so the new sites go at the end, and
    // the kept cells keep the order of the first grid. A new site at the
    // position of a kept site is left out.
    let taken: BTreeSet<(u64, u64)> = points
        .iter()
        .map(|p| (p[0].to_bits(), p[1].to_bits()))
        .collect();
    for (i, p) in new_points {
        if taken.contains(&(p[0].to_bits(), p[1].to_bits())) {
            continue;
        }
        points.push(p);
        base_cell.push(i);
        pack_heights.push(heights[i]);
    }
    let grid = base.packed(points);
    Pack {
        grid,
        base_cell,
        heights: pack_heights,
        cell_of_base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Lattice;
    use crate::rand::Rand;
    use crate::seed::{Layer, SeedTree};

    fn base(kind: Lattice) -> Grid {
        let mut rng = Rand::new(SeedTree::new(4).part_rng(Layer::Terrain, "grid"));
        Grid::generate(400.0, 300.0, 4000, kind, 0.9, &[], &mut rng).0
    }

    fn island(g: &Grid) -> Vec<f64> {
        g.points
            .iter()
            .map(|p| {
                let d = ((p[0] - 200.0).powi(2) + (p[1] - 150.0).powi(2)).sqrt();
                if d < 80.0 { 40.0 } else { 5.0 }
            })
            .collect()
    }

    #[test]
    fn land_and_coasts_stay_and_deep_water_goes() {
        for kind in [Lattice::Square, Lattice::HexPointy] {
            let g = base(kind);
            let h = island(&g);
            let keep = vec![false; g.len()];
            let p = regraph(&g, &h, 20.0, &keep);
            let t = grid_distance(&g, &h, 20.0);
            for c in 0..g.len() {
                let kept = p.cell_of_base[c];
                if h[c] >= 20.0 || t[c] == -1 {
                    let k = kept.expect("land and coast stay");
                    assert_eq!(p.grid.points[k], g.points[c]);
                    assert_eq!(p.heights[k], h[c]);
                    assert_eq!(p.base_cell[k], c);
                }
                if t[c] < -2 {
                    assert!(kept.is_none(), "deep water cell {c} stays");
                }
            }
            // The second grid has more cells at the coast than the first.
            let coast = |grid: &Grid, hh: &[f64]| {
                (0..grid.len())
                    .filter(|&c| hh[c] >= 20.0 && grid.neighbors(c).any(|m| hh[m] < 20.0))
                    .count()
            };
            assert!(coast(&p.grid, &p.heights) > coast(&g, &h), "{kind:?}");
            assert!(p.grid.len() < g.len());
            for c in 0..p.grid.len() {
                for m in p.grid.neighbors(c) {
                    assert!(p.grid.neighbors(m).any(|k| k == c));
                }
            }
        }
    }

    #[test]
    fn a_kept_site_in_deep_water_stays() {
        let g = base(Lattice::Square);
        let h = island(&g);
        let deep = g.find_cell(10.0, 10.0);
        let mut keep = vec![false; g.len()];
        keep[deep] = true;
        let p = regraph(&g, &h, 20.0, &keep);
        let k = p.cell_of_base[deep].unwrap();
        assert_eq!(p.grid.points[k], g.points[deep]);
        assert_eq!(p.grid.find_cell(g.points[deep][0], g.points[deep][1]), k);
    }
}
