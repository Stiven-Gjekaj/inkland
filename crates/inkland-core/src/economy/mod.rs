//! The economy of FMG (`model/economy`): goods and deposits, markets, the
//! production run of one day, taxes, and what the run gives.
//!
//! - `goods`: the catalogue and the deposits (`Goods.generate`). The
//!   deposits are placed before `rankCells`, as in FMG, and live in
//!   `Society::cells.good`.
//! - `demand`: what people need and how they spend.
//! - `finance`: the taxes, tariffs, and budget of a state (`State::economy`).
//! - `markets`: the markets, their cells, and the plaza of each center.
//! - `output`: rural output, deposits, and the multipliers of a good.
//! - `routes`: the travel cost over the route network.
//! - `deals`, `taxes`, `corruption`: the deal log, the tariffs and trade
//!   terms, and corruption by province.
//! - `trade`: prices, stock, quotes, carriage, rural spending, and the deal
//!   log of the run.
//!
//! `build` runs after the feature names, as `Markets.generate` does in the
//! FMG pipeline.

pub mod corruption;
pub mod deals;
pub mod demand;
pub mod finance;
pub mod goods;
pub mod goods_data;
pub mod markets;
pub mod output;
pub mod routes;
pub mod taxes;
pub mod trade;

use crate::config::Config;
use crate::features::Features;
use crate::grid::Grid;
use crate::rand::Rand;
use crate::roads::Road;
use crate::seed::{Layer, SeedTree};
use crate::society::Society;

/// A float key with a total order, for heaps.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Key(pub f64);
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// The stock and the price of a good in a market.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketGood {
    pub stock: f64,
    pub price: f64,
}

/// What the rural people of a market did in the run (`RuralDay`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RuralDay {
    pub earned: f64,
    pub spent: f64,
    /// Good id and units that they bought.
    pub bought: Vec<(u32, f64)>,
    /// Good id and units of their produce that nobody bought.
    pub kept: Vec<(u32, f64)>,
    /// What the center took on their produce and their purchases.
    pub margin: f64,
    /// State id, poll tax owed, and poll tax paid.
    pub poll_tax: Vec<(u32, f64, f64)>,
}

/// The economy of a world.
#[derive(Debug, Clone, PartialEq)]
pub struct Economy {
    /// The goods catalogue. The good with id `k` is at index `k - 1`.
    pub goods: Vec<goods::Good>,
    /// The markets. The market with id `k` is at index `k - 1`.
    pub markets: Vec<markets::Market>,
    /// The market of each cell, or 0 (FMG `cells.market`).
    pub cell_market: Vec<u32>,
}

impl Economy {
    /// The good with an id (`Goods.get`); `None` for 0 or an unknown id.
    pub fn good(&self, id: u32) -> Option<&goods::Good> {
        (id as usize).checked_sub(1).and_then(|k| self.goods.get(k))
    }

    /// The market with an id (`Markets.get`); `None` for 0.
    pub fn market(&self, id: u32) -> Option<&markets::Market> {
        (id as usize)
            .checked_sub(1)
            .and_then(|k| self.markets.get(k))
    }

    /// The market of a cell (`Markets.ofBurg` for the place on the cell).
    pub fn market_of_cell(&self, cell: usize) -> Option<&markets::Market> {
        self.market(self.cell_market.get(cell).copied().unwrap_or(0))
    }
}

/// What the economy stage reads.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub features: &'a Features,
    pub biomes: &'a [u8],
    pub roads: &'a [Road],
    pub sea_routes: &'a [Road],
    pub religions: &'a crate::religions::Religions,
}

/// The route links of each cell, by neighbor cell: `true` for a sea route
/// (FMG `cells.routes`). A later route on the same link wins.
pub fn route_links(roads: &[Road], sea: &[Road], n: usize) -> Vec<Vec<(usize, bool)>> {
    let mut links: Vec<std::collections::BTreeMap<usize, bool>> = vec![Default::default(); n];
    for r in roads.iter().chain(sea) {
        let water = r.group == "searoutes";
        for w in r.cells.windows(2) {
            if w[0] != w[1] {
                links[w[0]].insert(w[1], water);
                links[w[1]].insert(w[0], water);
            }
        }
    }
    links.into_iter().map(|l| l.into_iter().collect()).collect()
}

/// The places as markets read them.
pub(crate) fn burg_views(soc: &Society) -> Vec<markets::Burg> {
    soc.places
        .iter()
        .map(|p| markets::Burg {
            id: p.id,
            cell: p.cell,
            at: p.at,
            landmass: p.landmass,
            port: if p.port { p.water } else { 0 },
            state: p.state,
            population: p.population,
            capital: p.capital,
        })
        .collect()
}

/// Run the economy: markets (`Markets.generate`). The plaza of each place
/// is its market: a center has one.
pub fn build(input: &Input, soc: &mut Society) -> Economy {
    let Input {
        config,
        grid,
        features,
        roads,
        sea_routes,
        ..
    } = *input;
    let tree = SeedTree::new(config.seed);
    let n = grid.len();
    let links = route_links(roads, sea_routes, n);
    let burgs = burg_views(soc);
    let fee: Vec<f64> = std::iter::once(0.0)
        .chain(soc.states.iter().map(|s| s.economy.market_fee))
        .collect();

    // Markets: score, place, color, flood, recenter, and flood again.
    let mut rng = Rand::new(tree.part_rng(Layer::Economy, "markets"));
    let routes: Vec<usize> = links.iter().map(Vec::len).collect();
    let scored = markets::score(&burgs, &routes, &fee, &mut rng);
    let (w, h) = (config.frame.width, config.frame.height);
    let mut centers = markets::create(&burgs, &scored, &fee, w, h);
    let colors = crate::colors::distinct_colors(centers.len(), &mut rng);
    let states = &soc.states;
    let enemy = |a: u32, b: u32| {
        (a as usize)
            .checked_sub(1)
            .and_then(|k| states.get(k))
            .and_then(|s| s.diplomacy.get(b as usize))
            == Some(&crate::diplomacy::Relation::Enemy)
    };
    let flood = markets::Flood {
        grid,
        h: &soc.cells.h,
        feature: &features.of_cell,
        cell_state: &soc.cell_state,
        enemy: &enemy,
    };
    let first: Vec<&markets::Burg> = centers.iter().map(|&k| &burgs[k]).collect();
    let cell_market = markets::expand(&flood, &first);
    markets::recenter(&burgs, &scored, &cell_market, &mut centers);
    let second: Vec<&markets::Burg> = centers.iter().map(|&k| &burgs[k]).collect();
    let cell_market = markets::expand(&flood, &second);
    let markets: Vec<markets::Market> = centers
        .iter()
        .zip(colors)
        .enumerate()
        .map(|(k, (&c, color))| markets::Market {
            id: k as u32 + 1,
            center: burgs[c].id,
            color,
            fee_paid: 0.0,
            goods: Vec::new(),
            rural: None,
        })
        .collect();
    for m in &markets {
        soc.places[m.center as usize - 1].plaza = true;
    }

    Economy {
        goods: goods::catalogue(),
        markets,
        cell_market,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    fn world(seed: u64, template: &str) -> World {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": seed, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": template, "states": 6 }
        }))
        .unwrap();
        World::build(&c).unwrap()
    }

    #[test]
    fn each_place_is_in_a_market_and_only_centers_have_a_plaza() {
        for (seed, template) in [(1, "continents"), (2, "archipelago"), (3, "pangea")] {
            let w = world(seed, template);
            let e = &w.economy;
            assert!(!e.markets.is_empty());
            for (k, m) in e.markets.iter().enumerate() {
                assert_eq!(m.id as usize, k + 1);
                let center = &w.society.places[m.center as usize - 1];
                assert_eq!(
                    e.cell_market[center.cell], m.id,
                    "a center is in its market"
                );
            }
            for p in &w.society.places {
                assert_ne!(e.cell_market[p.cell], 0, "{} has no market", p.name);
                let center = e.markets.iter().any(|m| m.center == p.id);
                assert_eq!(p.plaza, center, "{}", p.name);
            }
        }
    }
}
