//! The cell data that the society stages of FMG read, and `rankCells`
//! (`population-generator.ts`).
//!
//! The cells are the cells of the second grid (`pack`). This module
//! collects the cell data that the society stages read:
//!
//! - `h`: the height on the FMG scale, where 20 is sea level.
//! - `t`, `haven`, `harbor`: the coast markup of `markupPack`
//!   (`Features::distance`, `haven`, `harbor`).
//! - `group`: the group of the feature of each cell (`defineGroups`), for
//!   example `isle` or the lake subtype `salt`.
//! - `fl` and `conf`: the water flux, where a river forms at 30, and the
//!   flux of the smaller rivers at a confluence.
//! - `good`: the deposit of each cell (`Goods.generate`), placed before
//!   `rankCells`.
//! - `s` and `pop`: suitability and rural population of `rankCells`, with
//!   the bonus of the goods around a cell.

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
/// The bonus of the coast of each lake subtype.
fn lake_score(group: &str) -> f64 {
    match group {
        "freshwater" => 30.0,
        "salt" => 10.0,
        "frozen" => 1.0,
        "dry" | "sinkhole" => -5.0,
        "lava" => -30.0,
        _ => 0.0,
    }
}

/// What `Cells::markup` reads.
pub struct Input<'a> {
    pub grid: &'a Grid,
    pub heights: &'a [f64],
    pub sea_level: f64,
    pub features: &'a Features,
    pub drainage: &'a Drainage,
    pub rivers: &'a [River],
    pub river_threshold: f64,
    /// The group of each feature (`geonames::groups`).
    pub groups: &'a [&'static str],
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
    /// FMG pixels for each unit. FMG costs and limits use pixels of a map of
    /// about 1280 x 800; the frame maps to that area.
    pub px: f64,
    pub fl: Vec<f64>,
    pub conf: Vec<f64>,
    /// The river of each cell, or 0.
    pub r: Vec<u32>,
    /// Suitability.
    pub s: Vec<f64>,
    /// Rural population, in population points.
    pub pop: Vec<f64>,
    /// The deposit of each cell: a good id, or 0 (FMG `cells.good`).
    pub good: Vec<u16>,
    /// The group of the feature of each cell.
    pub group: Vec<&'static str>,
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
    /// The cell data before `rankCells`: no deposits, suitability, or
    /// population yet.
    pub fn markup(input: &Input) -> Cells {
        let Input {
            grid,
            heights,
            sea_level,
            features,
            drainage,
            rivers,
            river_threshold,
            groups,
        } = *input;
        let n = grid.len();
        let h: Vec<f64> = heights
            .iter()
            .map(|&x| (x - sea_level + 20.0).clamp(0.0, 100.0))
            .collect();

        // The coast markup of the second grid.
        let t: Vec<i32> = features.distance.iter().map(|&d| i32::from(d)).collect();
        let haven: Vec<Option<usize>> = features
            .haven
            .iter()
            .map(|&h| (h != crate::features::NO_HAVEN).then_some(h as usize))
            .collect();
        let harbor = features.harbor.clone();

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
        // The confluence flux of the river stage (FMG `cells.conf`).
        let conf: Vec<f64> = drainage.confluence.iter().map(|&f| f * scale).collect();

        let group: Vec<&'static str> = features
            .of_cell
            .iter()
            .map(|&f| groups.get(f as usize - 1).copied().unwrap_or(""))
            .collect();
        let frozen: Vec<bool> = group.iter().map(|&g| g == "frozen").collect();
        let isle: Vec<bool> = group.iter().map(|&g| g == "isle").collect();

        Cells {
            h,
            t,
            haven,
            harbor,
            area,
            px: libm::sqrt(1280.0 * 800.0 / (grid.width * grid.height).max(1e-9)),
            fl,
            conf,
            r,
            s: vec![0.0; n],
            pop: vec![0.0; n],
            good: vec![0; n],
            group,
            frozen,
            isle,
        }
    }

    /// `true` for a land cell.
    pub fn land(&self, c: usize) -> bool {
        self.h[c] >= 20.0
    }

    /// The area of a cell in FMG square pixels.
    pub fn area_px(&self, c: usize) -> f64 {
        self.area[c] * self.px * self.px
    }

    /// `true` for a cell of a navigable river (`Rivers.isNavigable`).
    pub fn navigable(&self, c: usize) -> bool {
        self.r[c] != 0 && self.fl[c] >= NAVIGABLE_FLUX
    }

    /// Suitability and rural population (`rankCells`). `good_value` is the
    /// value of each good by id; the deposits in `good` add a bonus.
    pub fn rank(&mut self, grid: &Grid, features: &Features, biomes: &[u8], good_value: &[f64]) {
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
                        score += lake_score(self.group[hv]);
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
            // The bonus for goods around: the value of the deposit of the
            // cell plus 10, and the mean value of the deposits next to it.
            let value = |c: usize| {
                let g = self.good[c] as usize;
                if g == 0 {
                    0.0
                } else {
                    good_value.get(g).copied().unwrap_or(0.0)
                }
            };
            if self.good[i] != 0 || grid.neighbors(i).any(|c| self.good[c] != 0) {
                let own = value(i);
                let (sum, count) = grid
                    .neighbors(i)
                    .fold((0.0, 0usize), |(s, k), c| (s + value(c), k + 1));
                let around = if count == 0 { 0.0 } else { sum / count as f64 };
                let bonus = if own != 0.0 { own + 10.0 } else { 0.0 } + around;
                self.s[i] = (self.s[i] + bonus).trunc();
            }
            self.pop[i] = if self.s[i] > 0.0 {
                self.s[i] * self.area[i] / mean_area
            } else {
                0.0
            };
        }
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

    #[test]
    fn the_rank_reads_the_confluence_flux_of_the_river_stage() {
        let w = world();
        assert_eq!(w.society.cells.conf, w.drainage.confluence);
    }
}
