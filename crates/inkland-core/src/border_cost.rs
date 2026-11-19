//! Extra costs for the growth of states, apart from the costs of FMG.
//!
//! - `world.border_noise`: fractal noise from the seed tree, so that the
//!   borders wander and do not follow straight lines.

use crate::grid::Grid;
use crate::noise::Fractal;
use crate::seed::{Layer, SeedTree};

/// The largest extra cost of one cell at `border_noise: 1`.
pub const NOISE_COST: f64 = 60.0;

/// The extra cost to enter each cell.
pub fn extra(config: &crate::config::Config, grid: &Grid) -> Vec<f64> {
    let k = config.world.border_noise;
    let mut out = vec![0.0; grid.len()];
    if !(k.is_finite() && k > 0.0) || grid.is_empty() {
        return out;
    }
    // The noise has features of about 6 cells.
    let spacing = (grid.width * grid.height / grid.len() as f64).sqrt();
    let seed =
        Fractal::seed_of(SeedTree::new(config.seed).part_seed(Layer::States, "border-noise"));
    let noise = Fractal::new(seed, 3, 6.0 * spacing);
    for (c, cost) in out.iter_mut().enumerate() {
        let v = (2.0 * noise.at(grid.points[c])).clamp(-1.0, 1.0);
        *cost = k * NOISE_COST * 0.5 * (1.0 + v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use serde_json::json;

    fn config(noise: f64) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 7, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 4000, "template": "pangea", "states": 4,
                       "border_noise": noise }
        }))
        .unwrap()
    }

    #[test]
    fn border_noise_gives_extra_costs_in_its_range_and_none_at_zero() {
        let c = config(0.5);
        let w = crate::world::World::build(&c).unwrap();
        let e = extra(&c, &w.grid);
        assert!(e.iter().all(|&v| (0.0..=0.5 * NOISE_COST).contains(&v)));
        let (lo, hi) = e
            .iter()
            .fold((f64::INFINITY, 0.0f64), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(hi - lo > 0.3 * NOISE_COST, "{lo} {hi}");
        assert!(extra(&config(0.0), &w.grid).iter().all(|&v| v == 0.0));
    }

    #[test]
    fn border_noise_moves_the_borders() {
        let a = crate::world::World::build(&config(0.0)).unwrap();
        let b = crate::world::World::build(&config(1.0)).unwrap();
        assert_ne!(a.society.cell_state, b.society.cell_state);
    }
}
