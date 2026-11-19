//! Biomes.
//!
//! Each land cell gets a biome from its moisture and its temperature. The
//! biome list and the moisture-temperature matrix come from Azgaar's Fantasy
//! Map Generator (MIT license, see THIRD_PARTY_NOTICES.md). Pinned regions
//! of kind `marsh`, `forest`, and `desert` force their biome.

use crate::config::RegionKind;
use crate::fit::FitRegion;
use crate::grid::Grid;

/// The biomes. The number of each biome is its id.
pub const NAMES: [&str; 13] = [
    "marine",
    "hot-desert",
    "cold-desert",
    "savanna",
    "grassland",
    "tropical-seasonal-forest",
    "temperate-deciduous-forest",
    "tropical-rainforest",
    "temperate-rainforest",
    "taiga",
    "tundra",
    "glacier",
    "wetland",
];

/// How well people can live in each biome, from 0 to 100.
pub const HABITABILITY: [f64; 13] = [
    0.0, 4.0, 10.0, 22.0, 30.0, 50.0, 100.0, 80.0, 90.0, 12.0, 4.0, 0.0, 12.0,
];

pub const MARINE: u8 = 0;
pub const HOT_DESERT: u8 = 1;
pub const COLD_DESERT: u8 = 2;
pub const TROPICAL_SEASONAL_FOREST: u8 = 5;
pub const TEMPERATE_DECIDUOUS_FOREST: u8 = 6;
pub const TAIGA: u8 = 9;
pub const TUNDRA: u8 = 10;
pub const GLACIER: u8 = 11;
pub const WETLAND: u8 = 12;

/// Rows: moisture bands from dry to wet. Columns: temperature bands from hot
/// (more than 19 C) to cold (less than -4 C).
const MATRIX: [[u8; 26]; 5] = [
    [
        1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 10,
    ],
    [
        3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 9, 9, 9, 9, 10, 10, 10,
    ],
    [
        5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 9, 9, 9, 9, 9, 10, 10, 10,
    ],
    [
        5, 6, 6, 6, 6, 6, 6, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 10, 10, 10,
    ],
    [
        7, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 10, 10,
    ],
];

fn by_climate(moisture: f64, temp: f64, height: f64, sea_level: f64, river: bool) -> u8 {
    if height < sea_level {
        return MARINE;
    }
    if temp < -5.0 {
        return GLACIER;
    }
    if temp >= 25.0 && !river && moisture < 8.0 {
        return HOT_DESERT;
    }
    let rel = height - sea_level;
    let wet = temp > -2.0
        && ((moisture > 40.0 && rel < 5.0) || (moisture > 24.0 && rel > 4.0 && rel < 40.0));
    if wet {
        return WETLAND;
    }
    let m = ((moisture / 5.0) as usize).min(4);
    let t = (20.0 - temp).clamp(0.0, 25.0) as usize;
    MATRIX[m][t]
}

pub(crate) fn forced(kind: RegionKind, temp: f64) -> Option<u8> {
    match kind {
        RegionKind::Marsh => Some(WETLAND),
        RegionKind::Forest if temp >= 20.0 => Some(TROPICAL_SEASONAL_FOREST),
        RegionKind::Forest if temp >= 0.0 => Some(TEMPERATE_DECIDUOUS_FOREST),
        RegionKind::Forest => Some(TAIGA),
        RegionKind::Desert if temp >= 15.0 => Some(HOT_DESERT),
        RegionKind::Desert => Some(COLD_DESERT),
        _ => None,
    }
}

/// The biome of each cell.
#[allow(clippy::too_many_arguments)]
pub fn biomes(
    grid: &Grid,
    heights: &[f64],
    sea_level: f64,
    temps: &[f64],
    prec: &[f64],
    river_cells: &[bool],
    flux: &[f64],
    regions: &[FitRegion],
) -> Vec<u8> {
    (0..grid.len())
        .map(|c| {
            if heights[c] < sea_level {
                return MARINE;
            }
            // A pinned region forces the biome of its land cells.
            if let Some(b) = regions
                .iter()
                .filter(|r| r.contains(grid.points[c]))
                .find_map(|r| forced(r.pin.kind, temps[c]))
            {
                return b;
            }
            let mut own = prec[c];
            if river_cells[c] {
                own += (flux[c] / 10.0).max(2.0);
            }
            let (sum, count) = grid
                .neighbors(c)
                .filter(|&m| heights[m] >= sea_level)
                .fold((own, 1.0), |(s, k), m| (s + prec[m], k + 1.0));
            let moisture = (4.0 + sum / count).round();
            by_climate(moisture, temps[c], heights[c], sea_level, river_cells[c])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_temperate_map_at_fifty_degrees_has_no_glacier_on_low_land() {
        let biomes_of = |preset: serde_json::Value| {
            let c: crate::config::Config = serde_json::from_value(serde_json::json!({
                "version": 1, "seed": 4, "frame": { "width": 400, "height": 300 },
                "world": { "cells": 4000, "template": "pangea",
                           "climate": { "latitude_range": [48, 54], "preset": preset } }
            }))
            .unwrap();
            let w = crate::world::World::build(&c).unwrap();
            (0..w.grid.len())
                .filter(|&k| (20.0..=50.0).contains(&w.heights[k]))
                .map(|k| (w.heights[k], w.biomes[k]))
                .collect::<Vec<_>>()
        };
        let cold = |v: &[(f64, u8)]| {
            v.iter()
                .filter(|(_, b)| matches!(*b, GLACIER | TAIGA | TUNDRA))
                .count()
        };
        let fmg = biomes_of(serde_json::Value::Null);
        assert!(cold(&fmg) > fmg.len() / 2, "FMG at 50 degrees is cold");
        let temperate = biomes_of(serde_json::json!("temperate"));
        assert!(temperate.iter().all(|(_, b)| *b != GLACIER));
        assert!(
            temperate
                .iter()
                .filter(|(h, _)| *h <= 35.0)
                .all(|(_, b)| *b != TAIGA && *b != TUNDRA),
            "no taiga or tundra on the plains"
        );
    }

    #[test]
    fn water_is_marine_and_cold_land_is_glacier() {
        assert_eq!(by_climate(10.0, 10.0, 10.0, 20.0, false), MARINE);
        assert_eq!(by_climate(10.0, -10.0, 30.0, 20.0, false), GLACIER);
    }

    #[test]
    fn hot_dry_land_is_hot_desert_unless_a_river_crosses_it() {
        assert_eq!(by_climate(5.0, 30.0, 30.0, 20.0, false), HOT_DESERT);
        assert_ne!(by_climate(5.0, 30.0, 30.0, 20.0, true), HOT_DESERT);
    }

    #[test]
    fn mild_moist_land_is_forest() {
        assert_eq!(
            by_climate(12.0, 12.0, 70.0, 20.0, false),
            TEMPERATE_DECIDUOUS_FOREST
        );
    }

    #[test]
    fn forced_kinds_choose_by_temperature() {
        assert_eq!(forced(RegionKind::Marsh, 5.0), Some(WETLAND));
        assert_eq!(
            forced(RegionKind::Forest, 25.0),
            Some(TROPICAL_SEASONAL_FOREST)
        );
        assert_eq!(forced(RegionKind::Forest, -5.0), Some(TAIGA));
        assert_eq!(forced(RegionKind::Desert, 5.0), Some(COLD_DESERT));
        assert_eq!(forced(RegionKind::Hills, 5.0), None);
    }

    #[test]
    fn each_matrix_value_is_a_biome() {
        assert!(MATRIX.iter().flatten().all(|&b| (b as usize) < NAMES.len()));
    }
}
