//! The cell data that the society stages of FMG read, and `rankCells`
//! (`population-generator.ts`).
//!
//! FMG works on its second graph (`pack`). inkland has one grid, so this
//! module makes the same cell data on that grid:
//!
//! - `h`: the height on the FMG scale, where 20 is sea level.
//! - `t`: the distance field of `markupPack`. 1 is a land cell next to
//!   water, 2 is a land cell next to such a cell, and so on. -1 is a water
//!   cell next to land, then -2 to -9.
//! - `haven`: the nearest water cell next to a coastal land cell. `harbor`:
//!   the number of water cells next to it.
//! - `fl` and `conf`: the water flux on the FMG scale, where a river forms
//!   at 30, and the flux of the smaller rivers at a confluence.
//! - `s` and `pop`: suitability and rural population of `rankCells`.

use crate::biomes::HABITABILITY;
use crate::features::{FeatureKind, Features};
use crate::grid::Grid;
use crate::rivers::{Drainage, River};

/// The flux at which FMG makes a river.
pub const FMG_RIVER_FLUX: f64 = 30.0;
/// The flux from which a river is navigable in FMG.
pub const NAVIGABLE_FLUX: f64 = 100.0;

/// Suitability bonus of a coastal cell, by what it is next to.
const ESTUARY: f64 = 15.0;
const OCEAN_COAST: f64 = 5.0;
const SAFE_HARBOR: f64 = 20.0;
const FRESHWATER: f64 = 30.0;
const FROZEN: f64 = 1.0;

/// What `Cells::new` reads.
pub struct Input<'a> {
    pub grid: &'a Grid,
    pub heights: &'a [f64],
    pub sea_level: f64,
    pub features: &'a Features,
    pub biomes: &'a [u8],
    pub temperature: &'a [f64],
    pub drainage: &'a Drainage,
    pub rivers: &'a [River],
    pub river_threshold: f64,
}

/// The cell data of the society stages.
#[derive(Debug, Clone, PartialEq)]
pub struct Cells {
    pub h: Vec<f64>,
    pub t: Vec<i32>,
    pub haven: Vec<Option<usize>>,
    pub harbor: Vec<u8>,
    /// The area of each cell, in square units.
    pub area: Vec<f64>,
    pub fl: Vec<f64>,
    pub conf: Vec<f64>,
    /// The river of each cell, or 0.
    pub r: Vec<u32>,
    /// Suitability.
    pub s: Vec<f64>,
    /// Rural population, in population points.
    pub pop: Vec<f64>,
    /// `true` for a cell of a frozen lake.
    pub frozen: Vec<bool>,
    /// `true` for a land cell on a small island ("isle").
    pub isle: Vec<bool>,
}

/// The polygon area of a cell.
pub fn cell_area(grid: &Grid, cell: usize) -> f64 {
    let poly = &grid.polygons[cell];
    let mut a = 0.0;
    for k in 0..poly.len() {
        let p = grid.vertices[poly[k] as usize];
        let q = grid.vertices[poly[(k + 1) % poly.len()] as usize];
        a += p[0] * q[1] - q[0] * p[1];
    }
    (a / 2.0).abs()
}

fn dist2(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])
}

/// The median of the values, as in d3.
fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

impl Cells {
    pub fn new(input: &Input) -> Cells {
        let Input {
            grid,
            heights,
            sea_level,
            features,
            biomes,
            temperature,
            drainage,
            rivers,
            river_threshold,
        } = *input;
        let n = grid.len();
        let land = |c: usize| features.is_land(c);
        let h: Vec<f64> = heights
            .iter()
            .map(|&x| (x - sea_level + 20.0).clamp(0.0, 100.0))
            .collect();

        // Distance field, haven, and harbor.
        let mut t = vec![0i32; n];
        let mut haven = vec![None; n];
        let mut harbor = vec![0u8; n];
        for c in 0..n {
            if !land(c) {
                continue;
            }
            let water: Vec<usize> = grid.neighbors(c).filter(|&m| !land(m)).collect();
            if water.is_empty() {
                continue;
            }
            t[c] = 1;
            for &m in &water {
                t[m] = -1;
            }
            let mut best = water[0];
            for &m in &water[1..] {
                if dist2(grid.points[c], grid.points[m]) < dist2(grid.points[c], grid.points[best])
                {
                    best = m;
                }
            }
            haven[c] = Some(best);
            harbor[c] = water.len().min(255) as u8;
        }
        for c in 0..n {
            if t[c] == 1 {
                for m in grid.neighbors(c) {
                    if land(m) && t[m] == 0 {
                        t[m] = 2;
                    }
                }
            }
        }
        markup(grid, &mut t, 3, 1, 128);
        markup(grid, &mut t, -2, -1, -10);

        let area: Vec<f64> = (0..n).map(|c| cell_area(grid, c)).collect();

        // Flux on the FMG scale.
        let scale = FMG_RIVER_FLUX / river_threshold.max(1e-9);
        let fl: Vec<f64> = drainage.flux.iter().map(|&f| f * scale).collect();
        let mut r = vec![0u32; n];
        for river in rivers {
            for &c in &river.cells {
                r[c] = river.id;
            }
        }
        let mut conf = vec![0.0; n];
        for c in 0..n {
            if r[c] == 0 {
                continue;
            }
            let mut inflow: Vec<f64> = grid
                .neighbors(c)
                .filter(|&m| r[m] != 0 && drainage.receiver[m] == c)
                .map(|m| fl[m])
                .collect();
            if inflow.len() >= 2 {
                inflow.sort_by(|a, b| b.total_cmp(a));
                conf[c] = inflow[1..].iter().sum();
            }
        }

        // Lakes: frozen when the mean temperature is below -5 C, as in the
        // feature groups of inkland. Isles: land features of less than 0.5 %
        // of the cells, or of less than 10 cells.
        let mut cells_of: Vec<Vec<usize>> = vec![Vec::new(); features.list.len() + 1];
        for (c, &f) in features.of_cell.iter().enumerate() {
            cells_of[f as usize].push(c);
        }
        let mut frozen = vec![false; n];
        let mut isle = vec![false; n];
        for f in &features.list {
            let cells = &cells_of[f.id as usize];
            match f.kind {
                FeatureKind::Lake => {
                    let mean = cells.iter().map(|&c| temperature[c]).sum::<f64>()
                        / cells.len().max(1) as f64;
                    if mean < -5.0 {
                        for &c in cells {
                            frozen[c] = true;
                        }
                    }
                }
                FeatureKind::Island => {
                    let share = f64::from(f.cells) / n.max(1) as f64;
                    if !(share >= 0.1 || (share >= 0.005 && f.cells >= 10)) {
                        for &c in cells {
                            isle[c] = true;
                        }
                    }
                }
                FeatureKind::Ocean => {}
            }
        }

        let mut cells = Cells {
            h,
            t,
            haven,
            harbor,
            area,
            fl,
            conf,
            r,
            s: vec![0.0; n],
            pop: vec![0.0; n],
            frozen,
            isle,
        };
        cells.rank(features, biomes);
        cells
    }

    /// `true` for a land cell.
    pub fn land(&self, c: usize) -> bool {
        self.h[c] >= 20.0
    }

    /// `true` for a cell of a navigable river (`Rivers.isNavigable`).
    pub fn navigable(&self, c: usize) -> bool {
        self.r[c] != 0 && self.fl[c] >= NAVIGABLE_FLUX
    }

    /// Suitability and rural population (`rankCells`).
    fn rank(&mut self, features: &Features, biomes: &[u8]) {
        let n = self.h.len();
        let mean_flux = median(self.fl.iter().copied().filter(|&f| f != 0.0).collect());
        let max_flux = self.fl.iter().copied().fold(0.0, f64::max)
            + self.conf.iter().copied().fold(0.0, f64::max);
        let mean_area = self.area.iter().sum::<f64>() / n.max(1) as f64;
        let lake = |c: usize| features.get(features.of_cell[c]).kind == FeatureKind::Lake;
        for i in 0..n {
            if !self.land(i) {
                continue;
            }
            let mut score = HABITABILITY[biomes[i] as usize];
            if score == 0.0 {
                continue;
            }
            if mean_flux != 0.0 {
                let v = (self.fl[i] + self.conf[i] - mean_flux) / (max_flux - mean_flux);
                score += v.clamp(0.0, 1.0) * 250.0;
            }
            score -= (self.h[i] - 50.0) / 5.0;
            if self.t[i] == 1 {
                if self.r[i] != 0 {
                    score += ESTUARY;
                }
                if let Some(hv) = self.haven[i] {
                    if lake(hv) {
                        score += if self.frozen[hv] { FROZEN } else { FRESHWATER };
                    } else {
                        score += OCEAN_COAST;
                        if self.harbor[i] == 1 {
                            score += SAFE_HARBOR;
                        }
                    }
                }
            }
            // FMG stores the value in an Int16Array: it drops the fraction.
            self.s[i] = (score / 5.0).trunc();
            self.pop[i] = if self.s[i] > 0.0 {
                self.s[i] * self.area[i] / mean_area
            } else {
                0.0
            };
        }
    }
}

/// The `markup` loop of FMG: from `start`, mark the unmarked neighbors of
/// the cells of the step before, until no cell changes or the limit.
fn markup(grid: &Grid, t: &mut [i32], start: i32, increment: i32, limit: i32) {
    let mut distance = start;
    loop {
        if distance == limit {
            break;
        }
        let prev = distance - increment;
        let mut marked = 0;
        for c in 0..t.len() {
            if t[c] != prev {
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
        distance += increment;
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    fn world() -> World {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 3, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "continents" }
        }))
        .unwrap();
        World::build(&c).unwrap()
    }

    #[test]
    fn the_distance_field_marks_coasts_and_depth() {
        let w = world();
        let cells = &w.society.cells;
        for c in 0..w.grid.len() {
            let land = w.features.is_land(c);
            let t = cells.t[c];
            assert_eq!(land, t > 0, "cell {c} t {t}");
            assert!(t >= -9);
            let water_next = w.grid.neighbors(c).any(|m| !w.features.is_land(m));
            if land {
                assert_eq!(t == 1, water_next);
                assert_eq!(cells.haven[c].is_some(), water_next);
                if let Some(hv) = cells.haven[c] {
                    assert!(!w.features.is_land(hv));
                }
            } else if w.grid.neighbors(c).any(|m| w.features.is_land(m)) {
                assert_eq!(t, -1);
            }
            // A step to a neighbor changes the distance by at most 1 in a
            // part of the field.
            for m in w.grid.neighbors(c) {
                if land && w.features.is_land(m) {
                    assert!((cells.t[m] - t).abs() <= 1);
                }
            }
        }
    }

    #[test]
    fn water_and_glaciers_have_no_people() {
        let w = world();
        let cells = &w.society.cells;
        for c in 0..w.grid.len() {
            if !w.features.is_land(c) || w.biomes[c] == crate::biomes::GLACIER {
                assert_eq!(cells.s[c], 0.0);
                assert_eq!(cells.pop[c], 0.0);
            }
            assert!(cells.pop[c] >= 0.0);
            assert_eq!(cells.s[c].fract(), 0.0);
        }
        assert!(cells.s.iter().any(|&s| s > 0.0));
    }
}
