//! The world data that journeys read, in the form of the FMG `pack`.
//!
//! `Map::new` collects it from the world. A test can build a `Map` by
//! hand.

use crate::config::Config;
use crate::features::{FeatureKind, Features};
use crate::geonames::Names;
use crate::grid::Grid;
use crate::names::Chain;
use crate::rivers::{Mouth, River};
use crate::roads::Road;
use crate::society::Society;
use std::collections::BTreeMap;

use super::Point;

/// A place as journeys see it (FMG `Burg`).
#[derive(Debug, Clone, PartialEq)]
pub struct Burg {
    /// The place id.
    pub id: u32,
    pub cell: usize,
    /// The position of the place.
    pub at: [f64; 2],
    pub name: String,
    pub state: u32,
    pub culture: u32,
    pub capital: bool,
    pub port: bool,
    /// Population points.
    pub population: f64,
}

/// The cells, routes, and places that journeys read.
pub struct Map {
    /// The transports that a party can take.
    pub transports: Vec<super::Transport>,
    /// The size of the map, in world units.
    pub width: f64,
    pub height: f64,
    /// The site of each cell.
    pub p: Vec<[f64; 2]>,
    /// The neighbors of each cell.
    pub c: Vec<Vec<usize>>,
    /// The height on the FMG scale: land is 20 or more.
    pub h: Vec<f64>,
    /// The feature of each cell.
    pub f: Vec<u32>,
    pub biome: Vec<u8>,
    /// The distance of each cell to the coast (FMG `t`).
    pub t: Vec<i32>,
    /// The water cell next to a coastal land cell.
    pub haven: Vec<Option<usize>>,
    /// The river of each cell, or 0.
    pub r: Vec<u32>,
    /// `true` for a navigable river cell (`Rivers.isNavigable`).
    pub navigable: Vec<bool>,
    /// The temperature of each cell, in degrees C.
    pub temp: Vec<f64>,
    /// The neighbors along a river course from each cell.
    pub river_edges: Vec<Vec<usize>>,
    /// The neighbors that a route joins to each cell.
    pub connections: Vec<Vec<usize>>,
    /// The route links of each cell: neighbor to route index (FMG
    /// `cells.routes`). A later route overwrites an earlier one.
    pub links: Vec<BTreeMap<usize, usize>>,
    /// The points of each route: roads first, then sea routes.
    pub routes: Vec<Vec<Point>>,
    /// The burg id on each cell, or 0.
    pub burg: Vec<u32>,
    /// The burgs, in id order.
    pub burgs: Vec<Burg>,
    /// The name of each state, by state id. Entry 0 is the neutral lands.
    pub states: Vec<String>,
    /// The name of each feature, by feature id. Entry 0 is unused.
    pub feature_names: Vec<Option<String>>,
    /// `true` for a lake, by feature id.
    pub lakes: Vec<bool>,
    /// The name chain of each culture: chain 0 is the wildlands.
    pub chains: Vec<Chain>,
}

/// What `Map::new` reads from the world.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub features: &'a Features,
    pub biomes: &'a [u8],
    pub temperature: &'a [f64],
    pub rivers: &'a [River],
    pub names: &'a Names,
    pub society: &'a Society,
    pub roads: &'a [Road],
    pub sea_routes: &'a [Road],
    pub transports: &'a [super::Transport],
}

/// The name of the neutral lands in FMG (`states[0].name`).
pub const NEUTRALS: &str = "Neutrals";

impl Map {
    pub fn new(input: &Input) -> Map {
        let grid = input.grid;
        let soc = input.society;
        let cells = &soc.cells;
        let n = grid.len();

        let mut river_edges: Vec<Vec<usize>> = vec![Vec::new(); n];
        for r in input.rivers {
            let mut course = r.cells.clone();
            if let Mouth::Water(c) = r.mouth {
                course.push(c);
            }
            for w in course.windows(2) {
                river_edges[w[0]].push(w[1]);
                river_edges[w[1]].push(w[0]);
            }
        }

        // The routes of FMG `pack.routes`: roads and trails, then sea
        // routes. `buildLinks` lets a later route overwrite a link.
        let mut connections: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut links: Vec<BTreeMap<usize, usize>> = vec![BTreeMap::new(); n];
        let mut routes = Vec::new();
        for (k, road) in input.roads.iter().chain(input.sea_routes).enumerate() {
            for w in road.cells.windows(2) {
                let (a, b) = (w[0], w[1]);
                if a == b {
                    continue;
                }
                links[a].insert(b, k);
                links[b].insert(a, k);
                if !connections[a].contains(&b) {
                    connections[a].push(b);
                }
                if !connections[b].contains(&a) {
                    connections[b].push(a);
                }
            }
            routes.push(
                road.cells
                    .iter()
                    .zip(&road.points)
                    .map(|(&cell, &at)| Point { at, cell })
                    .collect(),
            );
        }

        let mut burg = vec![0u32; n];
        for p in &soc.places {
            burg[p.cell] = p.id;
        }
        let burgs = soc
            .places
            .iter()
            .map(|p| Burg {
                id: p.id,
                cell: p.cell,
                at: p.at,
                name: p.name.clone(),
                state: p.state,
                culture: p.culture,
                capital: p.capital,
                port: p.port,
                population: p.population,
            })
            .collect();
        let states = std::iter::once(NEUTRALS.to_string())
            .chain(soc.states.iter().map(|s| s.name.clone()))
            .collect();

        let features = input.features;
        let mut feature_names = vec![None; features.list.len() + 1];
        let mut lakes = vec![false; features.list.len() + 1];
        for f in &features.list {
            lakes[f.id as usize] = f.kind == FeatureKind::Lake;
        }
        for f in &input.names.features {
            if let Some(slot) = feature_names.get_mut(f.id as usize) {
                slot.clone_from(&f.name);
            }
        }
        let chains = std::iter::once(Chain::new(crate::cultures::WILDLANDS_BASE))
            .chain(soc.cultures.iter().map(|c| Chain::new(c.base)))
            .collect();

        Map {
            transports: input.transports.to_vec(),
            width: input.config.frame.width,
            height: input.config.frame.height,
            p: grid.points.clone(),
            c: (0..n).map(|c| grid.neighbors(c).collect()).collect(),
            h: cells.h.clone(),
            f: features.of_cell.clone(),
            biome: input.biomes.to_vec(),
            t: cells.t.clone(),
            haven: cells.haven.clone(),
            r: cells.r.clone(),
            navigable: (0..n).map(|c| cells.navigable(c)).collect(),
            temp: input.temperature.to_vec(),
            river_edges,
            connections,
            links,
            routes,
            burg,
            burgs,
            states,
            feature_names,
            lakes,
            chains,
        }
    }

    /// The number of cells.
    pub fn len(&self) -> usize {
        self.p.len()
    }

    /// `true` for a map without cells.
    pub fn is_empty(&self) -> bool {
        self.p.is_empty()
    }

    /// `true` for a land cell (`isLand`).
    pub fn is_land(&self, cell: usize) -> bool {
        self.h[cell] >= 20.0
    }

    /// The burg with this id.
    pub fn burg_by_id(&self, id: u32) -> Option<&Burg> {
        let k = (id as usize).checked_sub(1)?;
        match self.burgs.get(k) {
            Some(b) if b.id == id => Some(b),
            _ => self.burgs.iter().find(|b| b.id == id),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::journeys::Domain;
    use crate::world::World;
    use serde_json::json;

    pub(crate) fn world_map(seed: u64) -> (Config, World, Map) {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": seed, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 6000, "template": "archipelago" }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let m = Map::new(&Input {
            config: &c,
            grid: &w.grid,
            features: &w.features,
            biomes: &w.biomes,
            temperature: &w.temperature,
            rivers: &w.rivers,
            names: &w.names,
            society: &w.society,
            roads: &w.roads,
            sea_routes: &w.sea_routes,
            transports: &w.transports,
        });
        (c, w, m)
    }

    #[test]
    fn roads_and_sea_routes_give_valid_paths_on_a_world() {
        let (_, w, m) = world_map(5);
        assert_eq!(m.routes.len(), w.roads.len() + w.sea_routes.len());
        // A road joins its two ends; the land path follows its points.
        let road = w.roads.iter().find(|r| r.cells.len() > 3).unwrap();
        let (a, b) = (road.cells[0], *road.cells.last().unwrap());
        let r = m.find_path(a, b, Domain::Land, false);
        assert_eq!(r.error, None);
        assert!(m.is_valid_path(&r.points, Domain::Land));
        assert_eq!(r.points.first().map(|p| p.cell), Some(a));
        assert_eq!(r.points.last().map(|p| p.cell), Some(b));
        assert!(r.distance > 0.0);
        // Two ports of one sea route have a water path.
        let sea = w.sea_routes.first().unwrap();
        let (a, b) = (sea.cells[0], *sea.cells.last().unwrap());
        let r = m.find_path(a, b, Domain::Water, false);
        assert_eq!(r.error, None);
        assert!(m.is_valid_path(&r.points, Domain::Water));
        assert_eq!(r.points.last().map(|p| p.cell), Some(b));
        // Land paths do not cross between islands.
        let island = |c: usize| m.f[c];
        let other = (0..m.len())
            .find(|&c| m.is_land(c) && island(c) != island(a))
            .unwrap();
        if m.is_land(a) {
            let r = m.find_path(a, other, Domain::Land, false);
            assert_eq!(r.error, Some(crate::journeys::path::PathError::NoLandPath));
        }
    }
}
