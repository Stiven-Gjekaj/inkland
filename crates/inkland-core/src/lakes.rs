//! Lakes of the first grid: depression lakes and near-sea lakes.
//!
//! These stages follow Azgaar's Fantasy Map Generator (FMG,
//! `model/graph/grid-generator.ts`, `addDeepDepressionLakes` and
//! `openNearSeaLakes`):
//!
//! - A land low point becomes a lake when its water cannot reach other
//!   water through cells lower than its height plus the lake elevation
//!   limit. The lake is the low point and its neighbors of the same height.
//! - A lake next to a low land cell (at most 2 above sea level) that touches
//!   the ocean breaks through: the land cell becomes water, and the lake
//!   becomes a part of the ocean.
//!
//! FMG uses the fixed sea level 20: water is 19, and the limit of a break is
//! 22. inkland uses `sea_level - 1` and `sea_level + 2`.
//!
//! Pins win: a protected cell (a pinned place, a pinned river point, or a
//! cell of a land region) never becomes a lake or breaks, and a lake with a
//! cell of a pinned lake region stays a lake.

use crate::features::{FeatureKind, Features};
use crate::grid::Grid;

/// The default lake elevation limit of FMG.
pub const DEFAULT_ELEVATION_LIMIT: u8 = 20;

/// With this limit, FMG makes no depression lakes.
pub const NO_LAKES_LIMIT: u8 = 80;

/// The distance field of FMG `markupGrid`: 1 for land next to water, -1 for
/// water next to land, -2 to -9 for deeper water, and 0 for other land.
pub fn grid_distance(grid: &Grid, heights: &[f64], sea_level: f64) -> Vec<i8> {
    let n = grid.len();
    let land = |c: usize| heights[c] >= sea_level;
    let mut t = vec![0i8; n];
    for c in 0..n {
        if !land(c) {
            continue;
        }
        for m in grid.neighbors(c) {
            if !land(m) {
                t[c] = 1;
                t[m] = -1;
            }
        }
    }
    mark_rings(grid, &mut t, -2, -1, -10);
    t
}

/// Mark rings of cells with no mark: from `start`, each ring `step` from the
/// ring before, until no cell gets a mark or the ring reaches `limit`.
pub fn mark_rings(grid: &Grid, t: &mut [i8], start: i8, step: i8, limit: i8) {
    let mut distance = start;
    loop {
        if distance == limit {
            break;
        }
        let before = distance - step;
        let mut marked = 0;
        for c in 0..grid.len() {
            if t[c] != before {
                continue;
            }
            for m in grid.neighbors(c) {
                if t[m] == 0 {
                    t[m] = distance;
                    marked += 1;
                }
            }
        }
        if marked == 0 {
            break;
        }
        match distance.checked_add(step) {
            Some(d) => distance = d,
            None => break,
        }
    }
}

/// Make the deep depressions of the land into lakes. Return the number of
/// lakes.
pub fn add_depression_lakes(
    grid: &Grid,
    heights: &mut [f64],
    sea_level: f64,
    elevation_limit: u8,
    protected: &[bool],
) -> usize {
    if elevation_limit >= NO_LAKES_LIMIT {
        return 0;
    }
    let n = grid.len();
    let limit = f64::from(elevation_limit);
    let mut lakes = 0;
    let mut checked = vec![0u32; n];
    let mut visit = 0u32;
    for i in 0..n {
        if grid.border[i] || heights[i] < sea_level || protected[i] {
            continue;
        }
        let lowest = grid
            .neighbors(i)
            .map(|m| heights[m])
            .fold(f64::INFINITY, f64::min);
        if heights[i] > lowest {
            continue;
        }
        // Can the water of the low point pour into water?
        let threshold = heights[i] + limit;
        visit += 1;
        checked[i] = visit;
        let mut stack = vec![i];
        let mut deep = true;
        'search: while let Some(q) = stack.pop() {
            for m in grid.neighbors(q) {
                if checked[m] == visit || heights[m] >= threshold {
                    continue;
                }
                if heights[m] < sea_level {
                    deep = false;
                    break 'search;
                }
                checked[m] = visit;
                stack.push(m);
            }
        }
        if !deep {
            continue;
        }
        let h = heights[i];
        let cells: Vec<usize> = std::iter::once(i)
            .chain(
                grid.neighbors(i)
                    .filter(|&m| heights[m] == h && !protected[m]),
            )
            .collect();
        for c in cells {
            heights[c] = sea_level - 1.0;
        }
        lakes += 1;
    }
    lakes
}

/// Open the lakes that a low shore cell separates from the ocean. Return
/// the number of opened lakes.
pub fn open_near_sea_lakes(
    grid: &Grid,
    heights: &mut [f64],
    sea_level: f64,
    protected: &[bool],
    keep_lake: &[bool],
) -> usize {
    let features = Features::mark(grid, heights, sea_level);
    if !features.list.iter().any(|f| f.kind == FeatureKind::Lake) {
        return 0;
    }
    let mut t = grid_distance(grid, heights, sea_level);
    let mut kind: Vec<FeatureKind> = features.list.iter().map(|f| f.kind).collect();
    let mut of_cell = features.of_cell.clone();
    // A lake with a cell of a pinned lake region stays a lake.
    let mut keep = vec![false; kind.len()];
    for c in 0..grid.len() {
        if keep_lake[c] {
            keep[of_cell[c] as usize - 1] = true;
        }
    }
    let limit = sea_level + 2.0;
    let mut opened = 0;
    for i in 0..grid.len() {
        let lake = of_cell[i];
        if kind[lake as usize - 1] != FeatureKind::Lake || keep[lake as usize - 1] {
            continue;
        }
        let neighbors: Vec<usize> = grid.neighbors(i).collect();
        'shore: for c in neighbors {
            if t[c] != 1 || heights[c] > limit || protected[c] {
                continue;
            }
            for m in grid.neighbors(c) {
                let ocean = of_cell[m];
                if kind[ocean as usize - 1] != FeatureKind::Ocean {
                    continue;
                }
                heights[c] = sea_level - 1.0;
                t[c] = -1;
                of_cell[c] = ocean;
                for k in grid.neighbors(c) {
                    if heights[k] >= sea_level {
                        t[k] = 1;
                    }
                }
                for f in of_cell.iter_mut() {
                    if *f == lake {
                        *f = ocean;
                    }
                }
                kind[lake as usize - 1] = FeatureKind::Ocean;
                opened += 1;
                break 'shore;
            }
        }
    }
    opened
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rand::Rand;
    use crate::seed::{Layer, SeedTree};

    fn grid() -> Grid {
        Grid::new(
            400.0,
            300.0,
            3000,
            &mut Rand::new(SeedTree::new(3).part_rng(Layer::Terrain, "grid")),
        )
    }

    /// Land at 45 in a disc, sea outside, and a pit of the given depth in
    /// the middle.
    fn island_with_pit(g: &Grid, pit: f64) -> (Vec<f64>, usize) {
        let centre = g.find_cell(200.0, 150.0);
        let h = g
            .points
            .iter()
            .enumerate()
            .map(|(c, p)| {
                let d = ((p[0] - 200.0).powi(2) + (p[1] - 150.0).powi(2)).sqrt();
                if c == centre {
                    pit
                } else if d < 100.0 {
                    45.0
                } else {
                    5.0
                }
            })
            .collect();
        (h, centre)
    }

    #[test]
    fn the_distance_field_marks_coasts_and_deep_water() {
        let g = grid();
        let (h, _) = island_with_pit(&g, 40.0);
        let t = grid_distance(&g, &h, 20.0);
        for c in 0..g.len() {
            let land = h[c] >= 20.0;
            let water_next = g.neighbors(c).any(|m| h[m] < 20.0);
            let land_next = g.neighbors(c).any(|m| h[m] >= 20.0);
            if land {
                assert_eq!(t[c], if water_next { 1 } else { 0 });
            } else if land_next {
                assert_eq!(t[c], -1);
            } else {
                assert!((-9..=-2).contains(&t[c]) || t[c] == 0, "{}", t[c]);
            }
        }
        assert!(t.contains(&-5));
    }

    #[test]
    fn a_deep_pit_becomes_a_lake_and_a_shallow_one_does_not() {
        let g = grid();
        let none = vec![false; g.len()];
        // The rim is 45: water must rise 24 to leave a pit at 21, so the
        // pit is deep with the default limit of 20.
        let (mut h, centre) = island_with_pit(&g, 21.0);
        assert_eq!(add_depression_lakes(&g, &mut h, 20.0, 20, &none), 1);
        assert_eq!(h[centre], 19.0);
        // With a limit of 30 the water can pour over the rim.
        let (mut h, centre) = island_with_pit(&g, 21.0);
        assert_eq!(add_depression_lakes(&g, &mut h, 20.0, 30, &none), 0);
        assert_eq!(h[centre], 21.0);
        // The limit 80 makes no lakes.
        let (mut h, _) = island_with_pit(&g, 21.0);
        assert_eq!(add_depression_lakes(&g, &mut h, 20.0, 80, &none), 0);
    }

    #[test]
    fn a_protected_cell_does_not_become_a_lake() {
        let g = grid();
        let (mut h, centre) = island_with_pit(&g, 21.0);
        let mut protected = vec![false; g.len()];
        protected[centre] = true;
        assert_eq!(add_depression_lakes(&g, &mut h, 20.0, 20, &protected), 0);
        assert_eq!(h[centre], 21.0);
    }

    /// Sea in the west, land in the east, a lake in the land, and a wall
    /// of cells with the given height between the lake and the sea.
    fn lake_by_the_sea(g: &Grid, wall: f64) -> (Vec<f64>, usize) {
        let lake = |p: [f64; 2]| p[0] >= 150.0 && p[0] < 175.0 && (p[1] - 150.0).abs() < 15.0;
        let mut h: Vec<f64> = g
            .points
            .iter()
            .map(|&p| {
                if p[0] < 150.0 {
                    5.0
                } else if lake(p) {
                    10.0
                } else {
                    40.0
                }
            })
            .collect();
        let mut wall_cell = usize::MAX;
        for c in 0..g.len() {
            if h[c] == 10.0 && g.neighbors(c).any(|m| h[m] == 5.0) {
                h[c] = wall;
                wall_cell = wall_cell.min(c);
            }
        }
        (h, wall_cell)
    }

    #[test]
    fn a_lake_behind_a_low_shore_joins_the_ocean() {
        let g = grid();
        let none = vec![false; g.len()];
        let (mut h, wall) = lake_by_the_sea(&g, 22.0);
        assert!(
            wall != usize::MAX,
            "the test terrain has a shore between lake and sea"
        );
        assert_eq!(open_near_sea_lakes(&g, &mut h, 20.0, &none, &none), 1);
        let f = Features::mark(&g, &h, 20.0);
        assert!(!f.list.iter().any(|x| x.kind == FeatureKind::Lake));
        // A high shore keeps the lake.
        let (mut h, _) = lake_by_the_sea(&g, 23.0);
        assert_eq!(open_near_sea_lakes(&g, &mut h, 20.0, &none, &none), 0);
        // A pinned lake stays a lake.
        let (mut h, _) = lake_by_the_sea(&g, 22.0);
        let keep: Vec<bool> = h.iter().map(|&x| x == 10.0).collect();
        assert_eq!(open_near_sea_lakes(&g, &mut h, 20.0, &none, &keep), 0);
    }
}
