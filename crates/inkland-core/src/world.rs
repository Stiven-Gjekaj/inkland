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
use crate::rivers::{Course, Drainage, Lake, River, river_threshold};
use crate::seed::{Layer, SeedTree};
use std::collections::BTreeMap;

/// The first grid (FMG `grid`): a lattice. The heightmap, the lakes of
/// the first grid, and the climate run on it.
#[derive(Debug, Clone, PartialEq)]
pub struct Base {
    pub grid: Grid,
    /// The height of each cell: a whole number from 0 to 100.
    pub heights: Vec<f64>,
    pub features: Features,
    /// The temperature of each cell, in degrees C, from its first-grid
    /// cell.
    pub temperature: Vec<f64>,
    /// The precipitation of each cell, from its first-grid cell.
    pub precipitation: Vec<f64>,
}

/// The terrain of one config.
#[derive(Debug, Clone, PartialEq)]
pub struct World {
    pub seed: u64,
    pub width: f64,
    pub height: f64,
    pub sea_level: f64,
    /// The first grid.
    pub base: Base,
    /// The second grid (FMG `pack`), finer at the coasts. All stages after
    /// the climate run on it, and all cell indices of the world are its
    /// cells.
    pub grid: Grid,
    /// The first-grid cell of each cell (FMG `pack.cells.g`).
    pub base_cell: Vec<usize>,
    /// The height of each cell: a whole number from 0 to 100.
    pub heights: Vec<f64>,
    pub features: Features,
    pub coastlines: Vec<Coastline>,
    /// The cell of each point pin, in config order. The site of the cell is
    /// the exact position of the pin.
    pub pin_cells: Vec<usize>,
    /// The size of the map as a part of the world, and its latitude and
    /// longitude box.
    pub coordinates: crate::coordinates::Coordinates,
    /// The temperature of each cell, in degrees C, from its first-grid
    /// cell.
    pub temperature: Vec<f64>,
    /// The precipitation of each cell, from its first-grid cell.
    pub precipitation: Vec<f64>,
    /// Where the water of each cell goes.
    pub drainage: Drainage,
    /// The course of each pinned river, in config order.
    pub courses: Vec<Course>,
    /// All rivers. A pinned river has `course: Some(index)`.
    pub rivers: Vec<River>,
    /// The water data of each lake, in feature order.
    pub lakes: Vec<Lake>,
    /// Glaciers and icebergs, from the first grid.
    pub ice: Vec<crate::ice::Ice>,
    /// The biome of each cell (see `biomes::NAMES`).
    pub biomes: Vec<u8>,
    /// Cultures, places, states, and provinces.
    pub society: crate::society::Society,
    /// Pinned roads first, in config order, then the generated roads and
    /// trails.
    pub roads: Vec<crate::roads::Road>,
    /// The sea routes between ports.
    pub sea_routes: Vec<crate::roads::Road>,
    /// Names and groups of rivers, islands, and lakes.
    pub names: crate::geonames::Names,
    /// Folk and organized religions.
    pub religions: crate::religions::Religions,
    /// Markers: points of interest, as data.
    pub markers: Vec<crate::markers::Marker>,
    /// Goods, markets, and the production run of one day.
    pub economy: crate::economy::Economy,
    /// The roster of military units (FMG `options.military`).
    pub military_units: Vec<crate::military::Unit>,
    /// The transports of the journeys (FMG `options.transports`).
    pub transports: Vec<crate::journeys::Transport>,
    /// The stability of the states and their provinces, after the
    /// military.
    pub stability: crate::stability::Stability,
    /// Zones: areas of cells with a story, after the markers.
    pub zones: Vec<crate::zones::Zone>,
    /// Journeys: the routes of parties between places, as data. The last
    /// stage.
    pub journeys: Vec<crate::journeys::Journey>,
    /// The default ruler (FMG `createDefaultRuler`).
    pub measurers: Vec<crate::annotations::Measurer>,
    /// Legend boxes. Generation adds none (FMG `Legends.initiate`).
    pub legends: Vec<crate::annotations::Legend>,
    /// The notes of the config (`world.notes`).
    pub user_notes: Vec<crate::config::NoteEntry>,
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
        Template::Raster { .. } => Ok(Vec::new()),
    }
}

impl World {
    /// Build the base world. The config must be valid. The error is an
    /// infeasible pin.
    pub fn build(config: &Config) -> Result<World, Error> {
        let tree = SeedTree::new(config.seed);
        let (w, h) = (config.frame.width, config.frame.height);
        // The tables of the config: the goods, the units, and the transports.
        let goods = crate::economy::goods::catalogue_from(&config.world.goods)?;
        let military_units = crate::military::units_from(&config.world.military_units)?;
        let transports = crate::journeys::transports::transports_from(&config.world.transports)?;
        let sea_level = f64::from(config.world.sea_level);
        let pins = &config.pins;

        // Exact sites: each place, then each point of each pinned river. A
        // position that comes twice is one site.
        let mut sites: Vec<[f64; 2]> = Vec::new();
        let mut site_index: BTreeMap<(u64, u64), usize> = BTreeMap::new();
        let mut add_site = |p: [f64; 2]| -> usize {
            *site_index
                .entry((p[0].to_bits(), p[1].to_bits()))
                .or_insert_with(|| {
                    sites.push(p);
                    sites.len() - 1
                })
        };
        let place_sites: Vec<usize> = pins.points.iter().map(|p| add_site(p.at)).collect();
        for p in crate::courses::river_points(config) {
            add_site(p);
        }
        let mut grid_rng = Rand::new(tree.part_rng(Layer::Terrain, "grid"));
        let (base_grid, base_site_cells) = Grid::generate(
            w,
            h,
            cell_count(config) as u32,
            config.world.lattice,
            config.world.jitter,
            &sites,
            &mut grid_rng,
        );
        let base_pin_cells: Vec<usize> = place_sites.iter().map(|&i| base_site_cells[i]).collect();

        let steps = template_steps(config)
            .map_err(|e| Error::new(ErrorCode::TemplateInvalid, "/world/template", e))?;
        let mut height_rng = Rand::new(tree.part_rng(Layer::Terrain, "heightmap"));
        let mut hm = Heightmap::new(&base_grid, sea_level);
        if let Template::Raster { raster } = &config.world.template {
            let size = u64::from(raster.cols) * u64::from(raster.rows);
            if raster.cols < 2 || raster.rows < 2 || raster.values.len() as u64 != size {
                return Err(Error::new(
                    ErrorCode::TemplateInvalid,
                    "/world/template/raster",
                    "the raster needs 2 or more columns and rows, and cols x rows values",
                ));
            }
            hm.apply_raster(raster);
        }
        hm.run(&steps, &mut height_rng);
        let mut base_heights = hm.heights;
        crate::heightmap::apply_brushes(&base_grid, &mut base_heights, &config.world.brushes);

        let regions = FitRegion::all(&pins.regions, sea_level);
        apply_regions(&base_grid, &mut base_heights, &regions, sea_level);
        let points: Vec<FitPoint> = pins
            .points
            .iter()
            .enumerate()
            .map(|(index, pin)| FitPoint {
                index,
                pin,
                cell: base_pin_cells[index],
            })
            .collect();
        apply_points(&base_grid, &mut base_heights, &points, &regions, sea_level)?;

        // Lakes in deep depressions, then lakes that break into the ocean
        // (FMG `addDeepDepressionLakes` and `openNearSeaLakes`).
        // Pins win: the exact sites and the land regions stay as they are.
        let mut protected = vec![false; base_grid.len()];
        for &c in &base_site_cells {
            protected[c] = true;
        }
        let mut keep_lake = vec![false; base_grid.len()];
        for c in 0..base_grid.len() {
            for r in regions.iter().filter(|r| r.contains(base_grid.points[c])) {
                if r.pin.kind.is_water() {
                    keep_lake[c] |= r.pin.kind == RegionKind::Lake;
                } else {
                    protected[c] = true;
                }
            }
        }
        // The heights of a raster (for example an imported map) hold their
        // lakes already.
        if !matches!(config.world.template, Template::Raster { .. }) {
            crate::lakes::add_depression_lakes(
                &base_grid,
                &mut base_heights,
                sea_level,
                config.world.lake_elevation_limit,
                &protected,
            );
            crate::lakes::open_near_sea_lakes(
                &base_grid,
                &mut base_heights,
                sea_level,
                &protected,
                &keep_lake,
            );
        }

        // The place of the map on the globe (FMG `Coordinates.generate`).
        let base_features = Features::mark(&base_grid, &base_heights, sea_level);
        let partial = base_features
            .list
            .iter()
            .any(|f| f.kind.is_land() && f.border);
        let mut coord_rng = Rand::new(tree.part_rng(Layer::Climate, "coordinates"));
        let coordinates = crate::coordinates::coordinates(config, partial, &mut coord_rng);
        let latitudes = coordinates.latitude_range();

        let base_temperature =
            crate::climate::temperatures(&base_grid, &base_heights, config, latitudes);
        let mut rain_rng = Rand::new(tree.part_rng(Layer::Climate, "rain"));
        let base_precipitation = crate::climate::precipitation(
            &base_grid,
            &base_heights,
            &base_temperature,
            config,
            latitudes,
            &mut rain_rng,
        );

        // Glaciers and icebergs (FMG `Ice.generate`).
        let mut ice_rng = Rand::new(tree.part_rng(Layer::Climate, "ice"));
        let ice = crate::ice::generate(
            &base_grid,
            &base_heights,
            sea_level,
            &base_features,
            &base_temperature,
            &mut ice_rng,
        );

        // The second grid (FMG `regraph`). Each exact site stays a site.
        let mut keep = vec![false; base_grid.len()];
        for &c in &base_site_cells {
            keep[c] = true;
        }
        let pack = crate::pack::regraph(&base_grid, &base_heights, sea_level, &keep);
        let base_grid_len = base_grid.len();
        let grid = pack.grid;
        let base_cell = pack.base_cell;
        let mut heights = pack.heights;
        let to_pack =
            |c: usize| pack.cell_of_base[c].expect("an exact site stays in the second grid");
        let pin_cells: Vec<usize> = base_pin_cells.iter().map(|&c| to_pack(c)).collect();
        let site_cells: BTreeMap<(u64, u64), usize> = site_index
            .into_iter()
            .map(|(k, i)| (k, to_pack(base_site_cells[i])))
            .collect();
        let temperature: Vec<f64> = base_cell.iter().map(|&g| base_temperature[g]).collect();
        let precipitation: Vec<f64> = base_cell.iter().map(|&g| base_precipitation[g]).collect();

        // The pins hold on the second grid too: a new cell in a region gets
        // a height in its range, and each port keeps water next to it.
        crate::fit::clamp_regions(&grid, &mut heights, &regions, sea_level);
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

        let plan = crate::courses::Plan {
            grid: &grid,
            config,
            regions: &regions,
            sea_level,
            place_cells: &pin_cells,
            site_cells: &site_cells,
        };
        let courses = crate::courses::plan_courses(&plan, &mut heights)?;

        // Each pinned road keeps a land path between its stops. The cells
        // of the water regions stay water.
        let keep_water: Vec<bool> = (0..grid.len())
            .map(|c| {
                regions
                    .iter()
                    .any(|r| r.pin.kind.is_water() && r.contains(grid.points[c]))
            })
            .collect();
        crate::roads::keep_corridors(
            &grid,
            &mut heights,
            sea_level,
            config,
            &pin_cells,
            &keep_water,
        );

        let features = Features::mark_pack(&grid, &heights, sea_level);
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

        // Water flow and rivers (FMG `Rivers.generate`).
        let flow = crate::rivers::generate(&crate::rivers::Input {
            grid: &grid,
            heights: &heights,
            sea_level,
            features: &features,
            precipitation: &precipitation,
            temperature: &temperature,
            courses: &courses,
            cells_desired: cell_count(config),
            lake_elevation_limit: f64::from(config.world.lake_elevation_limit),
        });
        let drainage = flow.drainage;
        let mut rivers = flow.rivers;
        let mut lakes = flow.lakes;
        if let Some(max) = config.world.river_limit {
            crate::rivers::limit(&mut rivers, &mut lakes, max as usize);
        }
        let mut heights = flow.heights;
        // Pins win: the new heights stay in the ranges of the regions.
        crate::fit::clamp_regions(&grid, &mut heights, &regions, sea_level);
        let mut features = features;
        features.set_lake_heights(&heights);
        let mut river_cells = vec![false; grid.len()];
        for r in &rivers {
            for &c in &r.cells {
                river_cells[c] = true;
            }
        }
        let biomes = crate::biomes::biomes(
            &grid,
            &heights,
            sea_level,
            &temperature,
            &precipitation,
            &river_cells,
            &drainage.flux,
            &regions,
        );
        let groups = crate::geonames::groups(&grid, &features, &lakes, base_grid_len, sea_level);
        let mut society = crate::society::build(&crate::society::Input {
            config,
            grid: &grid,
            heights: &heights,
            sea_level,
            features: &features,
            biomes: &biomes,
            temperature: &temperature,
            drainage: &drainage,
            rivers: &rivers,
            river_threshold: river_threshold(grid.len()),
            pin_cells: &pin_cells,
            lakes: &lakes,
            groups: &groups,
            base_cells: base_grid_len,
            goods: &goods,
        });
        let routes = crate::roads::build(&crate::roads::Input {
            config,
            grid: &grid,
            heights: &heights,
            features: &features,
            biomes: &biomes,
            rivers: &rivers,
            flux: &drainage.flux,
            river_threshold: river_threshold(grid.len()),
            temperature: &temperature,
            society: &society,
        })?;
        let (roads, sea_routes) = (routes.roads, routes.sea);
        let mut religions = crate::religions::build(&crate::religions::Input {
            config,
            grid: &grid,
            biomes: &biomes,
            society: &society,
            roads: &roads,
            sea_routes: &sea_routes,
        });
        crate::society::specify(
            &mut society,
            &crate::society::Later {
                config,
                grid: &grid,
                features: &features,
                biomes: &biomes,
                roads: &roads,
                sea_routes: &sea_routes,
                religions: &religions,
            },
        );
        // Coats of arms (FMG `Emblems.generate` in each stage).
        crate::emblems::assign(
            &mut society,
            &crate::emblems::Input {
                config,
                features: &features,
                biomes: &biomes,
            },
        );
        // The names and arms of the config (`world.renames`).
        crate::renames::apply(config, &grid, &mut society, &mut religions)?;
        let names = crate::geonames::build(&crate::geonames::Input {
            config,
            grid: &grid,
            features: &features,
            rivers: &rivers,
            lakes: &lakes,
            base_cells: base_grid_len,
            sea_level,
            society: &society,
        });
        // The finance of the config (`world.finance`), before the run.
        crate::economy::finance::apply_config(config, &grid, &goods, &mut society)?;
        let economy = crate::economy::build(
            &crate::economy::Input {
                config,
                grid: &grid,
                features: &features,
                biomes: &biomes,
                roads: &roads,
                sea_routes: &sea_routes,
                religions: &religions,
                goods: &goods,
            },
            &mut society,
        )?;
        // The military (FMG `Military.generate`), after the taxes.
        let era = crate::markers::era(config.seed);
        crate::military::generate(
            &crate::military::Input {
                config,
                grid: &grid,
                features: &features,
                biomes: &biomes,
                religions: &religions,
                economy: &economy,
                era: &era,
                units: &military_units,
            },
            &mut society,
        );
        crate::military::apply_config(config, &grid, &military_units, &mut society)?;
        // FMG works out the stability on demand; inkland keeps it.
        let stability = crate::stability::build(&society, &economy);
        let mut markers = crate::markers::generate(&crate::markers::Input {
            config,
            grid: &grid,
            features: &features,
            biomes: &biomes,
            temperature: &temperature,
            rivers: &rivers,
            names: &names,
            society: &society,
            religions: &religions,
            roads: &roads,
            sea_routes: &sea_routes,
            pin_cells: &pin_cells,
            economy: &economy,
        });
        // The zones (FMG `Zones.generate`), after the markers.
        let zones = crate::zones::generate(
            &crate::zones::Input {
                config,
                grid: &grid,
                features: &features,
                rivers: &rivers,
                names: &names,
                society: &society,
                religions: &religions,
                roads: &roads,
                sea_routes: &sea_routes,
                economy: &economy,
                stability: &stability,
            },
            &mut markers,
        );
        let mut zones = zones;
        crate::zones::apply_config(&mut zones, config, &grid)?;
        // The journeys run last, as in FMG (`Journeys.generate`). Later
        // stages go before them.
        let measurers = crate::annotations::measurers(
            config,
            crate::annotations::default_ruler(&grid, &features, society.cells.px),
        );
        let journeys = crate::journeys::generate(&crate::journeys::Input {
            config,
            grid: &grid,
            features: &features,
            biomes: &biomes,
            temperature: &temperature,
            rivers: &rivers,
            names: &names,
            society: &society,
            roads: &roads,
            sea_routes: &sea_routes,
            transports: &transports,
        })?;
        Ok(World {
            seed: config.seed,
            width: w,
            height: h,
            sea_level,
            base: Base {
                grid: base_grid,
                heights: base_heights,
                features: base_features,
                temperature: base_temperature,
                precipitation: base_precipitation,
            },
            grid,
            base_cell,
            heights,
            features,
            coastlines,
            pin_cells,
            coordinates,
            temperature,
            precipitation,
            drainage,
            courses,
            rivers,
            lakes,
            ice,
            biomes,
            society,
            roads,
            sea_routes,
            names,
            religions,
            markers,
            economy,
            military_units,
            transports,
            stability,
            zones,
            journeys,
            measurers,
            legends: crate::annotations::legends(config),
            user_notes: config.world.notes.clone(),
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
    fn the_tables_of_the_config_reach_the_world() {
        let mut c = config(42, "continents", 3000);
        c.world.goods = vec![crate::config::GoodEntry {
            name: "Wood".to_owned(),
            value: Some(9.0),
            ..Default::default()
        }];
        c.world.transports = vec![crate::config::TransportEntry {
            name: "Rowboat".to_owned(),
            remove: true,
            ..Default::default()
        }];
        let w = World::build(&c).unwrap();
        assert_eq!(
            w.economy.good(1).map(|g| (g.name.as_str(), g.value)),
            Some(("Wood", 9.0))
        );
        assert_eq!(w.transports.len(), 19);
        assert!(w.transports.iter().all(|t| t.name != "Rowboat"));
        // A table that cannot be used stops the build.
        c.world.transports[0].name = "Nothing".to_owned();
        let e = World::build(&c).unwrap_err();
        assert_eq!(e.code, crate::error::ErrorCode::RefUnknown);
    }

    #[test]
    fn a_different_seed_gives_a_different_world() {
        let a = World::build(&config(42, "continents", 3000)).unwrap();
        let b = World::build(&config(43, "continents", 3000)).unwrap();
        assert_ne!(a.heights, b.heights);
    }

    #[test]
    fn a_raster_template_gives_its_heights_and_pins_still_apply() {
        // Sea in the west column, land in the east columns.
        let (cols, rows) = (8u32, 6u32);
        let values: Vec<u8> = (0..rows * cols)
            .map(|i| if i % cols == 0 { 5 } else { 40 })
            .collect();
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 9,
            "frame": { "width": 400, "height": 300 },
            "world": { "cells": 2000, "template": { "raster": {
                "cols": cols, "rows": rows, "values": values } } },
            "pins": { "points": [{ "id": "port", "kind": "town", "at": [52.5, 150.0], "port": true }] }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        for (cell, &h) in w.heights.iter().enumerate() {
            assert_eq!(h.fract(), 0.0);
            let [x, _] = w.grid.points[cell];
            if x < 45.0 {
                assert!(h < w.sea_level, "cell {cell} at x {x} is land");
            } else if x > 70.0 {
                assert_eq!(h, 40.0, "cell {cell} at x {x}");
            }
        }
        let port = w.pin_cells[0];
        assert_eq!(w.grid.points[port], [52.5, 150.0]);
        assert!(w.features.is_land(port));
        assert!(w.grid.neighbors(port).any(|m| !w.features.is_land(m)));
        assert_eq!(World::build(&c).unwrap(), w);
    }

    #[test]
    fn a_raster_with_the_wrong_size_is_refused() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 9,
            "frame": { "width": 400, "height": 300 },
            "world": { "cells": 2000, "template": { "raster": {
                "cols": 3, "rows": 3, "values": [1, 2, 3] } } }
        }))
        .unwrap();
        let e = World::build(&c).unwrap_err();
        assert_eq!(e.code, ErrorCode::TemplateInvalid);
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

    /// The exact sites and the region ranges hold on each lattice of the
    /// first grid, also after the second grid.
    #[test]
    fn pins_hold_on_each_lattice() {
        let points = json!([
            { "id": "a", "kind": "town", "at": [120.5, 88.25] },
            { "id": "b", "kind": "city", "at": [250.0, 200.0] },
            { "id": "port", "kind": "city", "at": [21.0, 150.0], "port": true }
        ]);
        for lattice in ["square", "hex-pointy", "hex-flat", "diamond", "triangle"] {
            let mut c = config(5, "continents", points.clone());
            c.world.lattice = serde_json::from_value(json!(lattice)).unwrap();
            let w = World::build(&c).unwrap();
            for (i, p) in c.pins.points.iter().enumerate() {
                let cell = w.pin_cells[i];
                assert_eq!(w.grid.points[cell], p.at, "{lattice}: {} moved", p.id);
                assert_eq!(w.grid.find_cell(p.at[0], p.at[1]), cell, "{lattice}");
                assert!(w.features.is_land(cell), "{lattice}: {} is not land", p.id);
            }
            for r in &c.pins.regions {
                let shape = Shape::of(r).unwrap();
                let [lo, hi] = effective_range(r, w.sea_level);
                for cell in (0..w.grid.len()).filter(|&k| shape.contains(w.grid.points[k])) {
                    let h = w.heights[cell];
                    assert!(
                        h >= lo.ceil() && h <= hi.floor().max(lo.ceil()),
                        "{lattice}: {}",
                        r.id
                    );
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

#[cfg(test)]
mod river_tests {
    use super::*;
    use crate::config::Anchor;
    use crate::rivers::{Mouth, NONE};
    use proptest::prelude::*;
    use serde_json::json;

    fn book(seed: u64, template: &str) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": seed,
            "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": template },
            "pins": {
                "points": [
                    { "id": "kingsholm", "kind": "city", "at": [150, 120] },
                    { "id": "harbour", "kind": "city", "at": [22, 180], "port": true },
                    { "id": "ford", "kind": "village", "at": [130, 200] }
                ],
                "regions": [
                    { "id": "western-sea", "kind": "sea", "box": [0, 0, 20, 300] },
                    { "id": "grey-teeth", "kind": "mountains", "polygon": [[200, 40], [270, 50], [260, 130], [210, 120]] },
                    { "id": "wolds", "kind": "hills", "circle": { "at": [240, 220], "r": 30 }, "height": [35, 50] },
                    { "id": "low-fen", "kind": "marsh", "polygon": [[60, 230], [100, 225], [105, 260], [65, 265]] }
                ],
                "rivers": [
                    { "id": "dunn", "source": { "region": "grey-teeth" },
                      "through": [{ "place": "kingsholm" }], "mouth": [10, 120] },
                    { "id": "brann", "source": { "region": "wolds" },
                      "through": [{ "place": "ford" }], "mouth": { "region": "low-fen" } },
                    { "id": "corrin", "source": [120, 150], "mouth": { "place": "harbour" } },
                    { "id": "tey", "source": [110, 260], "mouth": { "place": "harbour" } }
                ]
            }
        }))
        .unwrap()
    }

    fn check(c: &Config, w: &World) -> Result<(), TestCaseError> {
        let d = &w.drainage;
        prop_assert_eq!(w.courses.len(), c.pins.rivers.len());
        for (k, pin) in c.pins.rivers.iter().enumerate() {
            let river = w.rivers.iter().find(|r| r.course == Some(k));
            prop_assert!(river.is_some(), "{} has no river", pin.id.clone());
            let river = river.unwrap();
            // Strictly downhill on the drainage elevation, to the end.
            for pair in river.cells.windows(2) {
                prop_assert!(
                    d.elevation[pair[1]] < d.elevation[pair[0]],
                    "{} goes up",
                    pin.id.clone()
                );
            }
            let last = *river.cells.last().unwrap();
            if d.receiver[last] != NONE {
                prop_assert!(d.elevation[d.receiver[last]] < d.elevation[last]);
            }
            // Each anchor cell is on the course, in order.
            let mut at = 0;
            for a in &pin.through {
                let cell = match a {
                    Anchor::Place(r) => {
                        w.pin_cells[c.pins.points.iter().position(|p| p.id == r.place).unwrap()]
                    }
                    Anchor::Point(p) => w.grid.find_cell(p[0], p[1]),
                    Anchor::Region(_) => continue,
                };
                let pos = river.cells[at..].iter().position(|&x| x == cell);
                prop_assert!(pos.is_some(), "{} misses a through point", pin.id.clone());
                at += pos.unwrap();
            }
            // Each course cell is land; no other river uses it.
            for &cell in &river.cells {
                prop_assert!(w.features.is_land(cell));
                for other in w.rivers.iter().filter(|o| o.id != river.id) {
                    prop_assert!(
                        !other.cells.contains(&cell),
                        "{} crosses river {}",
                        pin.id.clone(),
                        other.id
                    );
                }
            }
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]

        #[test]
        fn pinned_rivers_hold_when_the_world_builds(seed in 0u64..1_000_000, t in 0usize..3) {
            let c = book(seed, ["continents", "pangea", "old-world"][t]);
            match World::build(&c) {
                Ok(w) => check(&c, &w)?,
                // Some terrains cannot hold the pins. The error must name a pin.
                Err(e) => prop_assert!(e.path.starts_with("/pins"), "{e:?}"),
            }
        }
    }

    /// The book rivers build on each of 60 terrains and keep all of their
    /// rules. The property test above checks only the terrains that build.
    #[test]
    fn the_book_rivers_build_on_many_terrains() {
        let mut failed = Vec::new();
        for seed in 0..60u64 {
            let c = book(
                seed * 7919,
                ["continents", "pangea", "old-world"][(seed % 3) as usize],
            );
            match World::build(&c) {
                Ok(w) => {
                    if let Err(e) = check(&c, &w) {
                        failed.push(format!("seed {}: {e}", seed * 7919));
                    }
                }
                Err(e) => failed.push(format!("seed {}: {} {}", seed * 7919, e.path, e.message)),
            }
        }
        assert!(failed.is_empty(), "{failed:#?}");
    }

    #[test]
    fn the_book_rivers_build_on_a_land_template() {
        let c = book(7, "pangea");
        let w = World::build(&c).unwrap();
        check(&c, &w).unwrap();
        let by_id = |id: &str| {
            let k = c.pins.rivers.iter().position(|r| r.id == id).unwrap();
            w.rivers.iter().find(|r| r.course == Some(k)).unwrap()
        };
        // The Dunn ends at its mouth point in the sea.
        let dunn = by_id("dunn");
        assert_eq!(*dunn.points.last().unwrap(), [10.0, 120.0]);
        assert!(matches!(dunn.mouth, Mouth::Water(_)));
        // The Brann ends in the Low Fen and keeps its water there.
        let brann = by_id("brann");
        assert_eq!(brann.mouth, Mouth::Sink);
        // The Corrin reaches the sea at Harbour; the Tey joins it or reaches
        // the same water.
        let corrin = by_id("corrin");
        let harbour = w.pin_cells[1];
        assert_eq!(*corrin.cells.last().unwrap(), harbour);
        let tey = by_id("tey");
        match tey.mouth {
            Mouth::Join(j) => assert!(corrin.cells.contains(&j)),
            Mouth::Water(_) => assert_eq!(*tey.cells.last().unwrap(), harbour),
            Mouth::Sink | Mouth::Edge => panic!("the Tey does not reach water"),
        }
    }

    #[test]
    fn a_river_from_marsh_up_to_mountains_is_infeasible() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 3, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 2000, "template": { "steps": ["Add 30 all"] } },
            "pins": {
                "regions": [
                    { "id": "sea", "kind": "sea", "box": [0, 0, 20, 300] },
                    { "id": "fen", "kind": "marsh", "box": [40, 100, 80, 160] },
                    { "id": "peaks", "kind": "mountains", "box": [300, 100, 380, 200] }
                ],
                "rivers": [{ "id": "up", "source": { "region": "fen" }, "through": [[340, 150]], "mouth": [10, 150] }]
            }
        }))
        .unwrap();
        let e = World::build(&c).unwrap_err();
        assert_eq!(e.code, ErrorCode::RiverNotDownhill);
    }
}
