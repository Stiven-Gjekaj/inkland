//! Extra costs for the growth of states, apart from the costs of FMG.
//!
//! - `world.border_noise`: fractal noise from the seed tree, so that the
//!   borders wander and do not follow straight lines.
//! - `world.border_terrain`: a cost to enter a river cell or a ridge cell,
//!   so that a state stops at a river or a ridge and the border follows it.

use crate::grid::Grid;
use crate::noise::Fractal;
use crate::seed::{Layer, SeedTree};

/// The largest extra cost of one cell at `border_noise: 1`.
pub const NOISE_COST: f64 = 60.0;

/// The extra cost of a river cell at `border_terrain: 1`.
pub const RIVER_COST: f64 = 120.0;

/// The extra cost of a ridge cell at `border_terrain: 1`, for each height
/// step above the mean of its neighbors.
pub const RIDGE_COST: f64 = 15.0;

/// The extra cost to enter each cell. `heights` gives the height of each
/// cell, and `river` the river of each cell (0: none).
pub fn extra(
    config: &crate::config::Config,
    grid: &Grid,
    heights: &[f64],
    river: &[u32],
) -> Vec<f64> {
    let mut out = vec![0.0; grid.len()];
    terrain(config, grid, heights, river, &mut out);
    noise(config, grid, &mut out);
    out
}

/// The cost of rivers and ridges.
fn terrain(
    config: &crate::config::Config,
    grid: &Grid,
    heights: &[f64],
    river: &[u32],
    out: &mut [f64],
) {
    let k = config.world.border_terrain;
    if !(k.is_finite() && k > 0.0) {
        return;
    }
    let sea = f64::from(config.world.sea_level);
    for (c, cost) in out.iter_mut().enumerate() {
        let h = heights[c];
        if h < sea {
            continue;
        }
        if river[c] != 0 {
            *cost += k * RIVER_COST;
        }
        // A ridge: a hill or mountain cell higher than its neighbors.
        if h >= sea + 15.0 {
            let (sum, count) = grid
                .neighbors(c)
                .fold((0.0, 0.0), |(s, n), m| (s + heights[m], n + 1.0));
            if count > 0.0 {
                let above = h - sum / count;
                if above > 0.0 {
                    *cost += k * RIDGE_COST * above.min(10.0);
                }
            }
        }
    }
}

/// The cost of the noise.
fn noise(config: &crate::config::Config, grid: &Grid, out: &mut [f64]) {
    let k = config.world.border_noise;
    if !(k.is_finite() && k > 0.0) || grid.is_empty() {
        return;
    }
    // The noise has features of about 6 cells.
    let spacing = (grid.width * grid.height / grid.len() as f64).sqrt();
    let seed =
        Fractal::seed_of(SeedTree::new(config.seed).part_seed(Layer::States, "border-noise"));
    let noise = Fractal::new(seed, 3, 6.0 * spacing);
    for (c, cost) in out.iter_mut().enumerate() {
        let v = (2.0 * noise.at(grid.points[c])).clamp(-1.0, 1.0);
        *cost += k * NOISE_COST * 0.5 * (1.0 + v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use serde_json::json;

    fn config(noise: f64, terrain: f64) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 7, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 4000, "template": "pangea", "states": 4,
                       "border_noise": noise, "border_terrain": terrain }
        }))
        .unwrap()
    }

    /// The world and the river of each cell.
    fn world(c: &Config) -> (crate::world::World, Vec<u32>) {
        let w = crate::world::World::build(c).unwrap();
        let mut river = vec![0; w.grid.len()];
        for r in &w.rivers {
            for &k in &r.cells {
                river[k] = r.id;
            }
        }
        (w, river)
    }

    #[test]
    fn border_noise_gives_extra_costs_in_its_range_and_none_at_zero() {
        let c = config(0.5, 0.0);
        let (w, river) = world(&c);
        let e = extra(&c, &w.grid, &w.heights, &river);
        assert!(e.iter().all(|&v| (0.0..=0.5 * NOISE_COST).contains(&v)));
        let (lo, hi) = e
            .iter()
            .fold((f64::INFINITY, 0.0f64), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(hi - lo > 0.3 * NOISE_COST, "{lo} {hi}");
        let none = config(0.0, 0.0);
        assert!(
            extra(&none, &w.grid, &w.heights, &river)
                .iter()
                .all(|&v| v == 0.0)
        );
    }

    #[test]
    fn border_noise_moves_the_borders() {
        let a = crate::world::World::build(&config(0.0, 0.0)).unwrap();
        let b = crate::world::World::build(&config(1.0, 0.0)).unwrap();
        assert_ne!(a.society.cell_state, b.society.cell_state);
    }

    #[test]
    fn border_terrain_costs_rivers_and_ridges_on_land() {
        let c = config(0.0, 1.0);
        let (w, river) = world(&c);
        let e = extra(&c, &w.grid, &w.heights, &river);
        let r = (0..w.grid.len())
            .find(|&k| river[k] != 0 && w.heights[k] >= 20.0)
            .unwrap();
        assert!(e[r] >= RIVER_COST);
        // A ridge cell without a river costs more too.
        assert!((0..w.grid.len()).any(|k| river[k] == 0 && e[k] > 0.0));
        // Water costs nothing more.
        assert!(
            (0..w.grid.len())
                .filter(|&k| w.heights[k] < 20.0)
                .all(|k| e[k] == 0.0)
        );
    }

    #[test]
    fn border_terrain_moves_the_borders() {
        let a = crate::world::World::build(&config(0.0, 0.0)).unwrap();
        let b = crate::world::World::build(&config(0.0, 1.0)).unwrap();
        assert_ne!(a.society.cell_state, b.society.cell_state);
    }
}
