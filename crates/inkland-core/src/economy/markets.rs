//! Markets (`markets-generator.ts`, `Markets.generate`).
//!
//! - Places score by population: capitals, ports, and crossroads score
//!   more, a dear license less, then random noise.
//! - The best place first, a place becomes a market center when no other
//!   center on its landmass is within its spacing, and none on another
//!   landmass within half of it. A landmass with places but no port gets a
//!   market of its own.
//! - Each market floods out from its center by cost. Another state, an
//!   enemy, another landmass, and water cost more. Water cells join too.
//! - Each market moves to the best place that it serves on the landmass of
//!   its center, and floods again. The centers get a plaza.

use crate::grid::Grid;
use crate::rand::Rand;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// The share of the spacing that a market on another landmass keeps clear.
const OTHER_LANDMASS_SPACING: f64 = 0.5;
/// The fee a day that halves the chance of a place to be a center and
/// doubles the area of each market (`MARKET_FEE_DETERRENCE`).
pub const MARKET_FEE_DETERRENCE: f64 = 35.0;
/// The margin of a market before its license fee raises it.
pub const MARKET_MARGIN: f64 = 0.035;
/// The highest margin, however dear the license.
const MAX_MARKET_MARGIN: f64 = 0.5;
/// The score that a place gains for each route beyond a road through it,
/// up to 4 more.
const CROSSROADS_BONUS: f64 = 0.25;
/// The spacing that a place needs, as a share of the base, from the
/// busiest land to the emptiest.
const SPACING_RANGE: [f64; 2] = [0.5, 2.5];
/// How far markets follow places: 1 gives each market about as many
/// places, 0 spaces them evenly.
const DENSITY_PULL: f64 = 0.6;

/// A market: a center place and the cells it serves.
#[derive(Debug, Clone, PartialEq)]
pub struct Market {
    /// 1, 2, 3, ...
    pub id: u32,
    /// The place id of the center.
    pub center: u32,
    /// A color as `#rrggbb`. Data only.
    pub color: String,
    /// The license fee that the center paid its state in the run.
    pub fee_paid: f64,
    /// The stock and the price of each good after the run, by good index
    /// (good id - 1).
    pub goods: Vec<super::MarketGood>,
    /// What its rural people did in the run.
    pub rural: Option<super::RuralDay>,
}

/// What markets read of a place.
#[derive(Debug, Clone, PartialEq)]
pub struct Burg {
    pub id: u32,
    pub cell: usize,
    pub at: [f64; 2],
    /// The land feature.
    pub landmass: u32,
    /// The water feature of a port, or 0.
    pub port: u32,
    pub state: u32,
    pub population: f64,
    pub capital: bool,
}

/// The margin rate of a market from the fee its center paid: whole
/// percents, at most `MAX_MARKET_MARGIN` (`getMarginRate`).
pub fn margin_rate(fee_paid: f64) -> f64 {
    let rate = crate::rand::js_round(
        MARKET_MARGIN * libm::sqrt(1.0 + fee_paid / MARKET_FEE_DETERRENCE) * 100.0,
    ) / 100.0;
    rate.min(MAX_MARKET_MARGIN)
}

/// The score of each place, best first (`scoreBurgs`). `routes` is the
/// number of route links at each cell; `fee` is the market fee of each
/// state by id. Each place draws one noise number in place order.
pub fn score(burgs: &[Burg], routes: &[usize], fee: &[f64], rng: &mut Rand) -> Vec<(usize, f64)> {
    let mut scored: Vec<(usize, f64)> = burgs
        .iter()
        .enumerate()
        .map(|(k, b)| {
            let mut s = b.population;
            if b.capital {
                s *= 2.5;
            }
            if b.port != 0 {
                s *= 1.2;
            }
            let roads = routes[b.cell] as f64;
            s *= 1.0 + CROSSROADS_BONUS * (roads - 2.0).clamp(0.0, 4.0);
            s /= fee_factor(b, fee);
            s *= rng.unit() * 2.0 + 0.5;
            (k, s)
        })
        .collect();
    // A stable sort: equal scores keep the place order.
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored
}

fn fee_factor(b: &Burg, fee: &[f64]) -> f64 {
    1.0 + fee.get(b.state as usize).copied().unwrap_or(0.0) / MARKET_FEE_DETERRENCE
}

/// How far each place must be from other centers to become one
/// (`getSpacing`), in the order of `burgs`. `count` is the FMG burg count,
/// which holds the empty burg 0.
pub fn spacing(
    burgs: &[&Burg],
    factors: &[f64],
    width: f64,
    height: f64,
    count: usize,
) -> Vec<f64> {
    let base = (width + height) * 2.5 / libm::pow(count as f64, 0.6);
    let neighbors: Vec<f64> = burgs
        .iter()
        .map(|a| {
            burgs
                .iter()
                .filter(|b| {
                    let (dx, dy) = (b.at[0] - a.at[0], b.at[1] - a.at[1]);
                    dx * dx + dy * dy < base * base
                })
                .count() as f64
        })
        .collect();
    let mean = |v: &[f64]| v.iter().fold(0.0, |s, x| s + x) / (v.len().max(1)) as f64;
    let mean_neighbors = mean(&neighbors);
    let mean_fee = mean(factors);
    let [lo, hi] = SPACING_RANGE;
    (0..burgs.len())
        .map(|k| {
            let area =
                libm::pow(mean_neighbors / neighbors[k], DENSITY_PULL) * (factors[k] / mean_fee);
            base * libm::sqrt(area).clamp(lo, hi)
        })
        .collect()
}

/// The centers of new markets, as indices of `burgs`, in market order
/// (`createMarkets`).
pub fn create(
    burgs: &[Burg],
    scored: &[(usize, f64)],
    fee: &[f64],
    width: f64,
    height: f64,
) -> Vec<usize> {
    let living: Vec<&Burg> = scored.iter().map(|&(k, _)| &burgs[k]).collect();
    let factors: Vec<f64> = living.iter().map(|b| fee_factor(b, fee)).collect();
    let space = spacing(&living, &factors, width, height, burgs.len() + 1);
    let mut centers: Vec<usize> = Vec::new();
    // A center blocks a place within the radius: the same landmass, or any.
    let near = |centers: &[usize], b: &Burg, r: f64, same: bool| {
        centers.iter().any(|&c| {
            let o = &burgs[c];
            if same && o.landmass != b.landmass {
                return false;
            }
            let (dx, dy) = (o.at[0] - b.at[0], o.at[1] - b.at[1]);
            dx * dx + dy * dy < r * r
        })
    };
    for (n, &(k, _)) in scored.iter().enumerate() {
        let b = &burgs[k];
        let r = space[n];
        if !near(&centers, b, r, true) && !near(&centers, b, r * OTHER_LANDMASS_SPACING, false) {
            centers.push(k);
        }
    }
    // A landmass with places but no port has no sea routes: it trades
    // within itself.
    let ports: Vec<u32> = living
        .iter()
        .filter(|b| b.port != 0)
        .map(|b| b.landmass)
        .collect();
    for &(k, _) in scored {
        let b = &burgs[k];
        if !ports.contains(&b.landmass) && !centers.iter().any(|&c| burgs[c].landmass == b.landmass)
        {
            centers.push(k);
        }
    }
    centers
}

/// What the flood of the markets reads of the world.
pub struct Flood<'a> {
    pub grid: &'a Grid,
    /// The FMG height of each cell.
    pub h: &'a [f64],
    /// The feature id of each cell.
    pub feature: &'a [u32],
    pub cell_state: &'a [u32],
    /// `true` if state `a` (index) is at war with state `b`: `enemy[a][b]`.
    pub enemy: &'a dyn Fn(u32, u32) -> bool,
}

/// The market of each cell: each market floods out from its center by
/// cost (`expandMarkets`). `centers` are the center places, in market order.
pub fn expand(flood: &Flood, centers: &[&Burg]) -> Vec<u32> {
    const MIN_COST: f64 = 1.0;
    const BASE_COST: f64 = 10.0;
    const DIFFERENT_STATE_COST: f64 = 200.0;
    const ENEMY_STATE_COST: f64 = 1000.0;
    const WATER_COST: f64 = 50.0;
    const WATER_COST_FOR_NON_PORTS: f64 = 50.0;
    const ISLAND_CHANGE_COST: f64 = 100.0;

    let Flood {
        grid,
        h,
        feature,
        cell_state,
        enemy,
    } = *flood;
    let n = grid.len();
    let mut cell_market = vec![0u32; n];
    let mut costs = vec![0.0f64; n];
    let mut native_owner = vec![false; n];
    let mut queue = BinaryHeap::new();
    let mut seq = 0u64;
    for (k, b) in centers.iter().enumerate() {
        cell_market[b.cell] = k as u32 + 1;
        costs[b.cell] = MIN_COST;
        queue.push(Reverse((super::Key(0.0), seq, b.cell, k)));
        seq += 1;
    }
    while let Some(Reverse((super::Key(priority), _, cell, k))) = queue.pop() {
        let b = centers[k];
        for m in grid.neighbors(cell) {
            let water = h[m] < 20.0;
            let native = cell_state[m] != 0 && b.state == cell_state[m];
            let mut cost = BASE_COST;
            if water {
                cost += WATER_COST;
                if b.port != feature[m] {
                    cost += WATER_COST_FOR_NON_PORTS;
                }
            } else {
                if feature[b.cell] != feature[m] {
                    cost += ISLAND_CHANGE_COST;
                }
                let other = cell_state[m];
                if other != 0 && !native {
                    cost += DIFFERENT_STATE_COST;
                    if enemy(b.state, other) && cell_state[cell] != other {
                        cost += ENEMY_STATE_COST;
                    }
                }
            }
            let total = priority + cost;
            let native_tie = total == costs[m] && native && !native_owner[m];
            if costs[m] == 0.0 || total < costs[m] || native_tie {
                costs[m] = total;
                queue.push(Reverse((super::Key(total), seq, m, k)));
                seq += 1;
                cell_market[m] = k as u32 + 1;
                native_owner[m] = native;
            }
        }
    }
    cell_market
}

/// Move each market to the best place that it serves on the landmass of its
/// center (`recenter`). `centers` holds the center of each market as an
/// index of `burgs`.
pub fn recenter(
    burgs: &[Burg],
    scored: &[(usize, f64)],
    cell_market: &[u32],
    centers: &mut [usize],
) {
    let mut best: Vec<Option<usize>> = vec![None; centers.len()];
    for &(k, _) in scored {
        let b = &burgs[k];
        let m = cell_market[b.cell] as usize;
        if m == 0 || best[m - 1].is_some() {
            continue;
        }
        if b.landmass == burgs[centers[m - 1]].landmass {
            best[m - 1] = Some(k);
        }
    }
    for (c, b) in centers.iter_mut().zip(best) {
        if let Some(k) = b {
            *c = k;
        }
    }
}

/// Apply the market changes of the config to the centers (burg indices)
/// and their colors, in order. A new market gets a new distinct color.
pub fn apply_config(
    config: &crate::config::Config,
    burgs: &[Burg],
    centers: &mut Vec<usize>,
    colors: &mut Vec<String>,
    rng: &mut crate::rand::Rand,
) -> Result<(), crate::error::Error> {
    use crate::error::{Error, ErrorCode};
    let nearest = |p: [f64; 2]| {
        (0..burgs.len()).min_by(|&a, &b| {
            let d = |k: usize| libm::hypot(burgs[k].at[0] - p[0], burgs[k].at[1] - p[1]);
            d(a).total_cmp(&d(b)).then(a.cmp(&b))
        })
    };
    for (i, e) in config.world.markets.iter().enumerate() {
        let path = format!("/world/markets/{i}");
        let bad = |m: String| Error::new(ErrorCode::MarketInvalid, path.clone(), m);
        if e.remove && e.to.is_some() {
            return Err(bad("a market entry cannot have both remove and to".into()));
        }
        let Some(b) = nearest(e.at) else {
            return Err(bad("the world has no place for a market".into()));
        };
        let at = centers.iter().position(|&c| c == b);
        match (e.remove, e.to, at) {
            (true, _, Some(k)) => {
                centers.remove(k);
                colors.remove(k);
            }
            (false, Some(to), Some(k)) => {
                let Some(t) = nearest(to) else {
                    return Err(bad("the world has no place for a market".into()));
                };
                if centers.contains(&t) && t != b {
                    return Err(bad("the new center has a market already".into()));
                }
                centers[k] = t;
            }
            (false, None, None) => {
                centers.push(b);
                let mut c = crate::colors::distinct_colors(colors.len() + 1, rng);
                colors.push(c.pop().unwrap_or_else(|| "#808080".to_owned()));
            }
            (false, None, Some(_)) => {
                return Err(bad(format!(
                    "the place {} is a market center already",
                    burgs[b].id
                )));
            }
            (_, _, None) => {
                return Err(bad(format!(
                    "the place {} is not a market center",
                    burgs[b].id
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn burg(id: u32, x: f64, landmass: u32, population: f64, port: u32) -> Burg {
        Burg {
            id,
            cell: id as usize,
            at: [x, 0.0],
            landmass,
            port,
            state: 0,
            population,
            capital: false,
        }
    }

    #[test]
    fn a_portless_landmass_gets_a_market_and_one_across_the_water_stands_closer() {
        let burgs = vec![
            burg(1, 0.0, 1, 100.0, 9),
            burg(2, 100.0, 1, 1.0, 0),
            burg(3, 300.0, 2, 1.0, 0),
            burg(4, 400.0, 3, 1.0, 9),
            burg(5, 1200.0, 4, 1.0, 9),
        ];
        // Scored by population: 1 first, then the others in place order.
        let scored: Vec<(usize, f64)> = vec![(0, 100.0), (1, 1.0), (2, 1.0), (3, 1.0), (4, 1.0)];
        let mut centers: Vec<u32> = create(&burgs, &scored, &[], 1000.0, 800.0)
            .iter()
            .map(|&k| burgs[k].id)
            .collect();
        centers.sort();
        assert_eq!(centers, vec![1, 3, 5]);
    }

    #[test]
    fn a_market_moves_to_the_best_place_on_the_landmass_of_its_center() {
        let burgs = vec![
            burg(1, 0.0, 1, 0.9, 0),
            burg(2, 0.0, 1, 23.6, 0),
            burg(3, 0.0, 2, 50.0, 0),
        ];
        let scored = vec![(2, 50.0), (1, 23.6), (0, 0.9)];
        let mut centers = vec![0];
        recenter(&burgs, &scored, &[0, 1, 1, 1], &mut centers);
        assert_eq!(burgs[centers[0]].id, 2);
    }

    #[test]
    fn markets_space_closer_where_places_crowd_and_wider_where_the_license_is_dear() {
        let burgs: Vec<Burg> = [0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 5000.0]
            .iter()
            .enumerate()
            .map(|(k, &x)| burg(k as u32 + 1, x, 1, 1.0, 0))
            .collect();
        let refs: Vec<&Burg> = burgs.iter().collect();
        let factors: Vec<f64> = burgs
            .iter()
            .map(|b| if b.id == 1 { 3.0 } else { 1.0 })
            .collect();
        let s = spacing(&refs, &factors, 1000.0, 800.0, burgs.len() + 1);
        assert!(s[6] > s[1] * 1.5);
        assert!(s[0] > s[1] * 1.5);
    }

    #[test]
    fn the_margin_grows_with_the_license() {
        assert_eq!(margin_rate(0.0), 0.04);
        assert_eq!(margin_rate(30.0), 0.05);
        assert_eq!(margin_rate(1e9), 0.5);
    }
}

#[cfg(test)]
mod config_tests {
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    fn build(markets: serde_json::Value) -> Result<World, crate::error::Error> {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 7, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "continents", "states": 6, "markets": markets }
        }))
        .unwrap();
        World::build(&c)
    }

    #[test]
    fn market_entries_add_move_and_remove_centers() {
        let base = build(json!([])).unwrap();
        let places = &base.society.places;
        let centers: Vec<u32> = base.economy.markets.iter().map(|m| m.center).collect();
        let free = places
            .iter()
            .find(|p| !centers.contains(&p.id))
            .expect("a place without a market");
        let first = &places[centers[0] as usize - 1];
        let added = build(json!([{ "at": free.at }])).unwrap();
        assert_eq!(added.economy.markets.len(), centers.len() + 1);
        assert!(added.economy.markets.iter().any(|m| m.center == free.id));
        assert!(added.society.places[free.id as usize - 1].plaza);
        let removed = build(json!([{ "at": first.at, "remove": true }])).unwrap();
        assert_eq!(removed.economy.markets.len(), centers.len() - 1);
        assert!(!removed.economy.markets.iter().any(|m| m.center == first.id));
        let moved = build(json!([{ "at": first.at, "to": free.at }])).unwrap();
        assert_eq!(moved.economy.markets.len(), centers.len());
        assert!(moved.economy.markets.iter().any(|m| m.center == free.id));
        assert!(!moved.economy.markets.iter().any(|m| m.center == first.id));
        for markets in [
            json!([{ "at": first.at }]),
            json!([{ "at": free.at, "remove": true }]),
        ] {
            let e = build(markets).unwrap_err();
            assert_eq!(
                (e.code, e.path.as_str()),
                (crate::error::ErrorCode::MarketInvalid, "/world/markets/0")
            );
        }
    }
}
