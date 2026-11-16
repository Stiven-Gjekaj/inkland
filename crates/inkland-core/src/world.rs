//! The base world: terrain that all variants share.
//!
//! Generation runs the stages in a fixed order. Each stage gets the random
//! numbers of its own part of the seed tree.

use crate::config::{Config, Template};
use crate::features::{Coastline, Features, coastlines};
use crate::grid::Grid;
use crate::heightmap::{Heightmap, parse_steps, templates};
use crate::rand::Rand;
use crate::seed::{Layer, SeedTree};

/// The terrain of one config.
#[derive(Debug, Clone, PartialEq)]
pub struct World {
    pub seed: u64,
    pub width: f64,
    pub height: f64,
    pub sea_level: f64,
    pub grid: Grid,
    /// The height of each cell: a whole number from 0 to 100.
    pub heights: Vec<f64>,
    pub features: Features,
    pub coastlines: Vec<Coastline>,
}

/// The number of grid cells for a config: from `units.per_cell_hint` if it
/// is given, else `world.cells`.
pub fn cell_count(config: &Config) -> f64 {
    match config.units.per_cell_hint {
        Some(hint) if hint > 0.0 => {
            (config.frame.width * config.frame.height / (hint * hint)).round()
        }
        _ => f64::from(config.world.cells),
    }
}

/// The steps of the heightmap template of a config.
pub fn template_steps(config: &Config) -> Result<Vec<crate::heightmap::Step>, String> {
    match &config.world.template {
        Template::Named(name) => parse_steps(templates::steps(*name).lines()),
        Template::Steps { steps } => parse_steps(steps.iter().map(String::as_str)),
    }
}

impl World {
    /// Build the base world. The config must be valid.
    pub fn build(config: &Config) -> Result<World, String> {
        let tree = SeedTree::new(config.seed);
        let (w, h) = (config.frame.width, config.frame.height);
        let sea_level = f64::from(config.world.sea_level);

        let mut grid_rng = Rand::new(tree.part_rng(Layer::Terrain, "grid"));
        let grid = Grid::new(w, h, cell_count(config) as u32, &mut grid_rng);

        let steps = template_steps(config)?;
        let mut height_rng = Rand::new(tree.part_rng(Layer::Terrain, "heightmap"));
        let mut hm = Heightmap::new(&grid, sea_level);
        hm.run(&steps, &mut height_rng);
        let heights = hm.heights;

        let features = Features::mark(&grid, &heights, sea_level);
        let coastlines = coastlines(&grid, &features);
        Ok(World {
            seed: config.seed,
            width: w,
            height: h,
            sea_level,
            grid,
            heights,
            features,
            coastlines,
        })
    }

    /// The fraction of cells that are land.
    pub fn land_fraction(&self) -> f64 {
        let land = (0..self.grid.len())
            .filter(|&c| self.features.is_land(c))
            .count();
        land as f64 / self.grid.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    fn config(seed: u64, template: &str, cells: u32) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": seed,
            "frame": { "width": 400, "height": 300 },
            "world": { "cells": cells, "template": template }
        }))
        .unwrap()
    }

    const TEMPLATES: [&str; 14] = [
        "volcano",
        "high-island",
        "low-island",
        "continents",
        "archipelago",
        "atoll",
        "mediterranean",
        "peninsula",
        "pangea",
        "isthmus",
        "shattered",
        "taklamakan",
        "old-world",
        "fractious",
    ];

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]

        #[test]
        fn worlds_keep_their_invariants(seed in 0u64..1_000_000, t in 0usize..14) {
            let c = config(seed, TEMPLATES[t], 2000);
            let w = World::build(&c).unwrap();
            // Heights are whole numbers in range.
            for &h in &w.heights {
                prop_assert!((0.0..=100.0).contains(&h) && h.fract() == 0.0);
            }
            // Each cell is land exactly when its height is at or above sea level.
            for c in 0..w.grid.len() {
                prop_assert_eq!(w.features.is_land(c), w.heights[c] >= w.sea_level);
            }
            // A coastline is closed, or both of its ends are at the frame edge.
            let near_x = w.grid.step_x;
            let near_y = w.grid.step_y;
            let at_edge = |p: [f64; 2]| p[0] < near_x || p[0] > w.width - near_x
                || p[1] < near_y || p[1] > w.height - near_y;
            for line in &w.coastlines {
                prop_assert!(line.points.len() >= 2);
                if line.closed {
                    prop_assert_eq!(line.points.first(), line.points.last());
                } else {
                    prop_assert!(at_edge(line.points[0]), "start {:?}", line.points[0]);
                    prop_assert!(at_edge(*line.points.last().unwrap()));
                }
                prop_assert!(w.features.get(line.land).kind.is_land());
                prop_assert!(!w.features.get(line.water).kind.is_land());
            }
        }
    }

    #[test]
    fn same_config_gives_the_same_world() {
        let c = config(42, "continents", 3000);
        assert_eq!(World::build(&c).unwrap(), World::build(&c).unwrap());
    }

    #[test]
    fn a_different_seed_gives_a_different_world() {
        let a = World::build(&config(42, "continents", 3000)).unwrap();
        let b = World::build(&config(43, "continents", 3000)).unwrap();
        assert_ne!(a.heights, b.heights);
    }

    #[test]
    fn per_cell_hint_sets_the_cell_count() {
        let mut c = config(1, "continents", 3000);
        c.units.per_cell_hint = Some(2.0);
        assert_eq!(cell_count(&c), 30_000.0);
    }
}
