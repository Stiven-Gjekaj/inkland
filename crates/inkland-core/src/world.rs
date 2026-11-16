//! The base world: terrain that all variants share.
//!
//! Generation runs the stages in a fixed order. Each stage gets the random
//! numbers of its own part of the seed tree.

use crate::config::{Config, RegionKind, Template};
use crate::error::{Error, ErrorCode};
use crate::features::{Coastline, FeatureKind, Features, coastlines};
use crate::fit::{FitPoint, FitRegion, apply_points, apply_regions};
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
    /// The cell of each point pin, in config order. The site of the cell is
    /// the exact position of the pin.
    pub pin_cells: Vec<usize>,
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
    /// Build the base world. The config must be valid. The error is an
    /// infeasible pin.
    pub fn build(config: &Config) -> Result<World, Error> {
        let tree = SeedTree::new(config.seed);
        let (w, h) = (config.frame.width, config.frame.height);
        let sea_level = f64::from(config.world.sea_level);
        let pins = &config.pins;

        let sites: Vec<[f64; 2]> = pins.points.iter().map(|p| p.at).collect();
        let mut grid_rng = Rand::new(tree.part_rng(Layer::Terrain, "grid"));
        let (grid, pin_cells) =
            Grid::with_sites(w, h, cell_count(config) as u32, &sites, &mut grid_rng);

        let steps = template_steps(config)
            .map_err(|e| Error::new(ErrorCode::TemplateInvalid, "/world/template", e))?;
        let mut height_rng = Rand::new(tree.part_rng(Layer::Terrain, "heightmap"));
        let mut hm = Heightmap::new(&grid, sea_level);
        hm.run(&steps, &mut height_rng);
        let mut heights = hm.heights;

        let regions = FitRegion::all(&pins.regions, sea_level);
        apply_regions(&grid, &mut heights, &regions, sea_level);
        let points: Vec<FitPoint> = pins
            .points
            .iter()
            .enumerate()
            .map(|(index, pin)| FitPoint {
                index,
                pin,
                cell: pin_cells[index],
            })
            .collect();
        apply_points(&grid, &mut heights, &points, &regions, sea_level)?;

        let features = Features::mark(&grid, &heights, sea_level);
        for r in &regions {
            if r.pin.kind != RegionKind::Sea || r.pin.landlocked {
                continue;
            }
            let lake = (0..grid.len())
                .filter(|&c| r.contains(grid.points[c]))
                .any(|c| features.get(features.of_cell[c]).kind != FeatureKind::Ocean);
            if lake {
                return Err(Error::new(
                    ErrorCode::SeaNotConnected,
                    format!("/pins/regions/{}", r.index),
                    format!(
                        "the sea {:?} does not connect to the ocean at a frame edge; move it to the edge or set landlocked: true",
                        r.pin.id
                    ),
                ));
            }
        }
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
            pin_cells,
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

#[cfg(test)]
mod pin_tests {
    use super::*;
    use crate::fit::effective_range;
    use crate::geom::Shape;
    use proptest::prelude::*;
    use serde_json::json;

    const TEMPLATES: [&str; 6] = [
        "continents",
        "archipelago",
        "high-island",
        "pangea",
        "shattered",
        "atoll",
    ];

    fn config(seed: u64, template: &str, points: serde_json::Value) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": seed,
            "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": template },
            "pins": {
                "points": points,
                "regions": [
                    { "id": "western-sea", "kind": "sea", "box": [0, 0, 20, 300] },
                    { "id": "teeth", "kind": "mountains", "box": [200, 40, 260, 140], "edge": 10 },
                    { "id": "wolds", "kind": "hills", "circle": { "at": [300, 220], "r": 30 }, "height": [35, 50] },
                    { "id": "low-fen", "kind": "marsh", "polygon": [[60, 170], [110, 160], [120, 210], [70, 220]] },
                    { "id": "mere", "kind": "lake", "box": [320, 40, 340, 60], "landlocked": true }
                ]
            }
        }))
        .unwrap()
    }

    fn place() -> impl Strategy<Value = (f64, f64)> {
        // Positions away from the forced water.
        (30.0..390.0f64, 5.0..295.0f64).prop_filter("not in the mere", |&(x, y)| {
            !((318.0..=342.0).contains(&x) && (38.0..=62.0).contains(&y))
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]

        #[test]
        fn pins_hold_on_any_terrain(
            seed in 0u64..1_000_000,
            t in 0usize..6,
            places in proptest::collection::vec(place(), 0..6),
            port_y in 10.0..290.0f64,
        ) {
            let mut points: Vec<serde_json::Value> = places
                .iter()
                .enumerate()
                .map(|(i, &(x, y))| json!({ "id": format!("p{i}"), "kind": "town", "at": [x, y] }))
                .collect();
            // A port on the coast of the western sea.
            points.push(json!({ "id": "port", "kind": "city", "at": [21.0, port_y], "port": true }));
            let c = config(seed, TEMPLATES[t], json!(points));
            let w = World::build(&c).map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
            let sea = w.sea_level;

            for r in &c.pins.regions {
                let shape = Shape::of(r).unwrap();
                let [lo, hi] = effective_range(r, sea);
                for cell in 0..w.grid.len() {
                    if !shape.contains(w.grid.points[cell]) {
                        continue;
                    }
                    let h = w.heights[cell];
                    prop_assert!(h >= lo.ceil() && h <= hi.floor().max(lo.ceil()),
                        "{} cell {cell} height {h} not in [{lo}, {hi}]", r.id);
                    if r.kind.is_water() {
                        prop_assert!(!w.features.is_land(cell), "{} cell {cell} is land", r.id);
                    }
                    if r.kind == RegionKind::Sea {
                        prop_assert_eq!(w.features.get(w.features.of_cell[cell]).kind, FeatureKind::Ocean);
                    }
                }
            }
            for (i, p) in c.pins.points.iter().enumerate() {
                let cell = w.pin_cells[i];
                prop_assert_eq!(w.grid.points[cell], p.at, "{} moved", p.id.clone());
                prop_assert!(w.features.is_land(cell), "{} is not on land", p.id.clone());
                if p.port {
                    prop_assert!(w.grid.neighbors(cell).any(|m| !w.features.is_land(m)),
                        "port {} has no water next to it", p.id.clone());
                }
            }
        }
    }

    #[test]
    fn a_sea_in_the_middle_of_land_is_refused_unless_landlocked() {
        let make = |landlocked: bool| -> Config {
            serde_json::from_value(json!({
                "version": 1, "seed": 4, "frame": { "width": 400, "height": 300 },
                "world": { "cells": 2000, "template": { "steps": ["Add 40 all"] } },
                "pins": { "regions": [
                    { "id": "inner-sea", "kind": "sea", "circle": { "at": [200, 150], "r": 40 }, "landlocked": landlocked }
                ]}
            }))
            .unwrap()
        };
        let e = World::build(&make(false)).unwrap_err();
        assert_eq!(e.code, ErrorCode::SeaNotConnected);
        assert_eq!(e.path, "/pins/regions/0");
        assert!(World::build(&make(true)).is_ok());
    }

    #[test]
    fn a_port_far_inland_is_refused() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 4, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 2000, "template": { "steps": ["Add 40 all"] } },
            "pins": { "points": [{ "id": "port", "kind": "city", "at": [200, 150], "port": true }] }
        }))
        .unwrap();
        let e = World::build(&c).unwrap_err();
        assert_eq!(e.code, ErrorCode::PortNotNearCoast);
        assert_eq!(e.path, "/pins/points/0");
    }

    #[test]
    fn a_port_near_the_coast_gets_a_channel() {
        // Land everywhere except a sea at the west edge. The port is three
        // cells from the sea, so a channel joins it to the sea.
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 4, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 2000, "template": { "steps": ["Add 40 all"] } },
            "pins": {
                "points": [{ "id": "port", "kind": "city", "at": [40, 150], "port": true }],
                "regions": [{ "id": "sea", "kind": "sea", "box": [0, 0, 20, 300] }]
            }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let cell = w.pin_cells[0];
        assert!(w.features.is_land(cell));
        let water = w
            .grid
            .neighbors(cell)
            .find(|&m| !w.features.is_land(m))
            .unwrap();
        assert_eq!(
            w.features.get(w.features.of_cell[water]).kind,
            FeatureKind::Ocean
        );
    }
}
