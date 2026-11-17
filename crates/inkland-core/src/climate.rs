//! Climate: the temperature and the precipitation of each cell.
//!
//! Temperature comes from the latitude and the height. Precipitation comes
//! from winds that cross the map along the lattice rows and columns: a wind
//! takes up water over the sea and drops it on land, more on slopes that go
//! up. The default values and the latitude table come from Azgaar's Fantasy
//! Map Generator (MIT license, see THIRD_PARTY_NOTICES.md).

use crate::config::{Config, Origin};
use crate::grid::Grid;
use crate::rand::Rand;

/// Sea-level temperature at the equator and at the poles, in degrees C.
pub const T_EQUATOR: f64 = 27.0;
pub const T_NORTH_POLE: f64 = -30.0;
pub const T_SOUTH_POLE: f64 = -15.0;

/// The tropics: no cooling from latitude except a small gradient.
const TROPIC_NORTH: f64 = 16.0;
const TROPIC_SOUTH: f64 = -20.0;
const TROPICAL_GRADIENT: f64 = 0.15;

/// Height in metres is `(h - 18) ^ HEIGHT_EXPONENT` for land.
pub const HEIGHT_EXPONENT: f64 = 2.0;

/// Default winds, one for each 30 degree band from north to south: the
/// direction to which the wind blows, in degrees clockwise from north.
pub const DEFAULT_WINDS: [f64; 6] = [225.0, 45.0, 225.0, 315.0, 135.0, 315.0];

/// Rain factor for each 5 degree band of latitude, from 0 to 90 degrees.
const LATITUDE_FACTOR: [f64; 18] = [
    4.0, 2.0, 2.0, 2.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 2.0, 2.0, 1.0, 1.0, 1.0, 0.5,
];

/// A wind does not pass a cell higher than this.
const MAX_PASSABLE_HEIGHT: f64 = 85.0;

/// The latitude of a position, in degrees, for the latitude range of the
/// config.
pub fn latitude(config: &Config, y: f64) -> f64 {
    latitude_in(config.world.climate.latitude_range, config, y)
}

/// The latitude of a position, in degrees, for a latitude range
/// `[south, north]`.
pub fn latitude_in(range: [f64; 2], config: &Config, y: f64) -> f64 {
    let [south, north] = range;
    let t = y / config.frame.height;
    match config.frame.origin {
        Origin::TopLeft => north - (north - south) * t,
        Origin::BottomLeft => south + (north - south) * t,
    }
}

fn sea_level_temperature(lat: f64) -> f64 {
    let t_north_tropic = T_EQUATOR - TROPIC_NORTH * TROPICAL_GRADIENT;
    let t_south_tropic = T_EQUATOR + TROPIC_SOUTH * TROPICAL_GRADIENT;
    if (TROPIC_SOUTH..=TROPIC_NORTH).contains(&lat) {
        T_EQUATOR - lat.abs() * TROPICAL_GRADIENT
    } else if lat > 0.0 {
        let g = (t_north_tropic - T_NORTH_POLE) / (90.0 - TROPIC_NORTH);
        t_north_tropic - (lat - TROPIC_NORTH) * g
    } else {
        let g = (t_south_tropic - T_SOUTH_POLE) / (90.0 + TROPIC_SOUTH);
        t_south_tropic + (lat - TROPIC_SOUTH) * g
    }
}

/// The height of a cell in metres above sea level. Water is 0.
pub fn metres(h: f64, sea_level: f64) -> f64 {
    if h < sea_level {
        0.0
    } else {
        libm::pow((h - 18.0).max(0.0), HEIGHT_EXPONENT)
    }
}

/// The temperature of each cell, in whole degrees C, for a latitude range
/// `[south, north]`.
pub fn temperatures(grid: &Grid, heights: &[f64], config: &Config, range: [f64; 2]) -> Vec<f64> {
    let sea = f64::from(config.world.sea_level);
    (0..grid.len())
        .map(|c| {
            let base = sea_level_temperature(latitude_in(range, config, grid.points[c][1]));
            let drop = metres(heights[c], sea) / 1000.0 * 6.5;
            (base - drop).round()
        })
        .collect()
}

/// The precipitation of each cell, for a latitude range `[south, north]`.
/// The mean over land is about 10 to 30 for the default winds.
pub fn precipitation(
    grid: &Grid,
    heights: &[f64],
    temps: &[f64],
    config: &Config,
    range: [f64; 2],
    rng: &mut Rand,
) -> Vec<f64> {
    let latitude = |_: &Config, y: f64| latitude_in(range, config, y);
    let n = grid.len();
    let (cx, cy) = (grid.cells_x as usize, grid.cells_y as usize);
    let sea = f64::from(config.world.sea_level);
    let winds = config.world.climate.wind.unwrap_or(DEFAULT_WINDS);
    let modifier = libm::pow(n as f64 / 10000.0, 0.25);
    let mut prec = vec![0.0; n];

    let lat_factor = |lat: f64| {
        let band = ((lat.abs() - 1.0).max(0.0) / 5.0) as usize;
        LATITUDE_FACTOR[band.min(LATITUDE_FACTOR.len() - 1)]
    };
    let tier = |lat: f64| (((89.0 - lat).abs() / 30.0) as usize).min(5);

    // One pass of a wind: from `first`, `steps` cells, `next` apart.
    let mut pass =
        |first: usize, next: isize, steps: usize, max: f64, step_len: f64, prec: &mut Vec<f64>| {
            let mut humidity = max - heights[first];
            if humidity <= 0.0 {
                return;
            }
            let mut cur = first as isize;
            for _ in 0..steps.saturating_sub(1) {
                let c = cur as usize;
                let nx = (cur + next) as usize;
                cur += next;
                if temps[c] < -5.0 {
                    continue;
                }
                if heights[c] < sea {
                    if heights[nx] >= sea {
                        prec[nx] += (humidity / rng.uniform(10.0, 20.0)).max(1.0);
                    } else {
                        humidity = (humidity + 5.0 * modifier * step_len).min(max);
                        prec[c] += 5.0 * modifier;
                    }
                    continue;
                }
                let passable = heights[nx] <= MAX_PASSABLE_HEIGHT;
                let fall = if passable {
                    let normal = (humidity / (10.0 * modifier)).max(1.0) * step_len;
                    let rise = (heights[nx] - heights[c]).max(0.0);
                    let m = (heights[nx] / 70.0) * (heights[nx] / 70.0);
                    (normal + rise * m).clamp(step_len.min(humidity), humidity.max(step_len))
                } else {
                    humidity
                };
                prec[c] += fall;
                let evaporation = if fall > 1.5 * step_len { step_len } else { 0.0 };
                humidity = if passable {
                    (humidity - fall + evaporation).clamp(0.0, max)
                } else {
                    0.0
                };
            }
        };

    let mut northerly = 0.0;
    let mut southerly = 0.0;
    for row in 0..cy {
        let first = row * cx;
        let lat = latitude(config, grid.points[first][1]);
        let angle = winds[tier(lat)];
        let max = 120.0 * modifier * lat_factor(lat);
        if angle > 40.0 && angle < 140.0 {
            pass(first, 1, cx, max, 1.0, &mut prec);
        }
        if angle > 220.0 && angle < 320.0 {
            pass(first + cx - 1, -1, cx, max, 1.0, &mut prec);
        }
        if angle > 100.0 && angle < 260.0 {
            northerly += 1.0;
        }
        if !(80.0..=280.0).contains(&angle) {
            southerly += 1.0;
        }
    }
    let total = northerly + southerly;
    if total > 0.0 {
        let top = latitude(config, grid.points[0][1]);
        let bottom = latitude(config, grid.points[n - 1][1]);
        if northerly > 0.0 {
            let max = northerly / total * 60.0 * modifier * lat_factor(top.max(bottom));
            for col in 0..cx {
                let (first, next) = if top >= bottom {
                    (col, cx as isize)
                } else {
                    (n - cx + col, -(cx as isize))
                };
                pass(first, next, cy, max, 1.0, &mut prec);
            }
        }
        if southerly > 0.0 {
            let max = southerly / total * 60.0 * modifier * lat_factor(top.min(bottom));
            for col in 0..cx {
                let (first, next) = if top >= bottom {
                    (n - cx + col, -(cx as isize))
                } else {
                    (col, cx as isize)
                };
                pass(first, next, cy, max, 1.0, &mut prec);
            }
        }
    }
    prec.iter().map(|&p| p.min(255.0).round()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::{Layer, SeedTree};
    use serde_json::json;

    fn config(lat: [f64; 2], origin: &str) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 1,
            "frame": { "origin": origin, "y_axis": if origin == "top-left" { "south" } else { "north" },
                       "width": 400, "height": 300 },
            "world": { "cells": 2000, "climate": { "latitude_range": lat } }
        }))
        .unwrap()
    }

    fn grid() -> Grid {
        Grid::new(
            400.0,
            300.0,
            2000,
            &mut Rand::new(SeedTree::new(1).part_rng(Layer::Terrain, "grid")),
        )
    }

    #[test]
    fn latitude_follows_the_frame_origin() {
        let tl = config([30.0, 55.0], "top-left");
        assert_eq!(latitude(&tl, 0.0), 55.0);
        assert_eq!(latitude(&tl, 300.0), 30.0);
        let bl = config([30.0, 55.0], "bottom-left");
        assert_eq!(latitude(&bl, 0.0), 30.0);
        assert_eq!(latitude(&bl, 300.0), 55.0);
    }

    #[test]
    fn it_is_colder_in_the_north_and_higher_up() {
        let c = config([30.0, 55.0], "top-left");
        let g = grid();
        let flat = vec![25.0; g.len()];
        let t = temperatures(&g, &flat, &c, c.world.climate.latitude_range);
        let north = g.slot(200.0, 2.0);
        let south = g.slot(200.0, 298.0);
        assert!(t[north] < t[south]);
        let mut high = flat.clone();
        high[south] = 90.0;
        assert!(
            temperatures(&g, &high, &c, c.world.climate.latitude_range)[south] < t[south] - 20.0
        );
    }

    #[test]
    fn rain_falls_on_land_and_more_on_rising_slopes() {
        let c = config([35.0, 50.0], "top-left");
        let g = grid();
        // Sea in the west half, land in the east half, a ridge near the coast.
        let h: Vec<f64> = g
            .points
            .iter()
            .map(|p| {
                if p[0] < 200.0 {
                    5.0
                } else if (p[0] - 235.0).abs() < 12.0 {
                    70.0
                } else {
                    30.0
                }
            })
            .collect();
        let range = c.world.climate.latitude_range;
        let t = temperatures(&g, &h, &c, range);
        let p = precipitation(
            &g,
            &h,
            &t,
            &c,
            range,
            &mut Rand::new(SeedTree::new(1).part_rng(Layer::Climate, "rain")),
        );
        let land: f64 = (0..g.len()).filter(|&i| h[i] >= 20.0).map(|i| p[i]).sum();
        assert!(land > 0.0);
        // The windward foot of the ridge gets the most rain of its row.
        let row = g.slot(0.0, 150.0);
        let cells: Vec<usize> = (row..row + g.cells_x as usize).collect();
        let wettest = *cells
            .iter()
            .max_by(|&&a, &&b| p[a].total_cmp(&p[b]).then(b.cmp(&a)))
            .unwrap();
        let ridge_start = *cells.iter().find(|&&i| h[i] > 40.0).unwrap();
        assert!(
            wettest < ridge_start && ridge_start - wettest <= 2,
            "{wettest} {ridge_start}"
        );
        let near_ridge = p[wettest];
        let behind_ridge = p[g.slot(300.0, 150.0)];
        assert!(near_ridge > behind_ridge, "{near_ridge} {behind_ridge}");
    }

    #[test]
    fn metres_grow_with_height() {
        assert_eq!(metres(10.0, 20.0), 0.0);
        assert!(metres(100.0, 20.0) > metres(50.0, 20.0));
    }
}
