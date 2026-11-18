//! The economy of FMG (`model/economy`): goods and deposits, markets, the
//! production run of one day, taxes, and what the run gives.
//!
//! - `goods`: the catalogue and the deposits (`Goods.generate`). The
//!   deposits are placed before `rankCells`, as in FMG, and live in
//!   `Society::cells.good`.
//! - `demand`: what people need and how they spend.
//! - `finance`: the taxes, tariffs, and budget of a state (`State::economy`).
//! - `markets`: the markets, their cells, and the plaza of each center.
//! - `output`, `routes`, `trade`, `production`, `orders`, `taxes`,
//!   `corruption`, `deals`: the run of one day (`Production.produce`), then
//!   the collection of the taxes (`Taxes.collect`).
//! - `prosperity`, `accounts`, `traffic`: what the run gives. FMG works them
//!   out on demand; inkland works them out once in `build` and keeps them
//!   in `Economy`. The functions of `accounts` (`trade`, `budget`,
//!   `outputs`, `known_for`, `economy_type`) give the rest on demand.
//!
//! `build` runs after the feature names, as `Markets.generate` does in the
//! FMG pipeline.

pub mod accounts;
pub mod corruption;
pub mod deals;
pub mod demand;
pub mod finance;
pub mod goods;
pub mod goods_data;
pub mod markets;
pub mod orders;
pub mod output;
pub mod production;
pub mod prosperity;
pub mod routes;
pub mod taxes;
pub mod trade;
pub mod traffic;

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

/// What a place did in the run (FMG `burg.production`, `burg.product`,
/// `burg.pollTaxPaid`, `burg.treasury`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlaceEconomy {
    /// Good id and units that the place made, its deposit included.
    pub production: Vec<(u32, f64)>,
    /// Sales less the cost of the inputs.
    pub product: f64,
    pub poll_tax_paid: f64,
    /// The money at the end of the day.
    pub treasury: f64,
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
    /// What each place did in the run, by place index (id - 1).
    pub places: Vec<PlaceEconomy>,
    /// The deal log of the run (`pack.deals`).
    pub deals: Vec<deals::Deal>,
    /// The corruption of the provinces of each state.
    pub corruption: corruption::Corruption,
    /// A tenth of the map diagonal, in units (`getDecima`).
    pub decima: f64,
    /// The cell spacing of the first grid, in units (FMG `grid.spacing`).
    pub spacing: f64,
    /// The rural output of each cell a day: good id and units
    /// (`Output.getRural`).
    pub rural_output: Vec<Vec<(u32, f64)>>,
    /// The land cell whose people work each cell (`Output.getWorkingCell`).
    pub working_cell: Vec<usize>,
    /// The rural prosperity percent of each cell, 0 where no one lives
    /// (FMG `cells.prosperity`).
    pub prosperity: Vec<u8>,
    /// The rural people of each cell (`Prosperity.getRuralCell`).
    pub rural: Vec<Option<prosperity::RuralCell>>,
    /// How the people of each place live, by place index
    /// (`Prosperity.getBurg`).
    pub burg_living: Vec<Option<prosperity::Living>>,
    /// The accounts of the states, the provinces, and the world.
    pub accounts: accounts::Accounts,
    /// The units of each good on each route link (`TradeTraffic`).
    pub traffic: Vec<traffic::Link>,
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
    /// The goods catalogue.
    pub goods: &'a [goods::Good],
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

/// Run the economy: markets (`Markets.generate`), the run of one day
/// (`Production.produce`), and the taxes (`Taxes.collect`). The plaza of
/// each place is its market: a center has one.
pub fn build(input: &Input, soc: &mut Society) -> Result<Economy, crate::error::Error> {
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
    // The market changes of the config (`world.markets`).
    let mut colors = colors;
    let mut config_rng = Rand::new(tree.part_rng(Layer::Economy, "market-config"));
    markets::apply_config(config, &burgs, &mut centers, &mut colors, &mut config_rng)?;
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

    let mut economy = Economy {
        goods: input.goods.to_vec(),
        markets,
        cell_market,
        places: vec![PlaceEconomy::default(); soc.places.len()],
        deals: Vec::new(),
        corruption: corruption::Corruption::default(),
        decima: libm::hypot(w, h) / 10.0,
        spacing: libm::sqrt(w * h / crate::world::cell_count(config)),
        rural_output: Vec::new(),
        working_cell: Vec::new(),
        prosperity: Vec::new(),
        rural: Vec::new(),
        burg_living: Vec::new(),
        accounts: accounts::Accounts::default(),
        traffic: Vec::new(),
    };
    produce(input, soc, &links, &mut economy);
    // The rural prosperity of the cells, as `produce` of FMG ends: before
    // the collection of the taxes.
    let mut cell_burg = vec![0u32; n];
    for p in &soc.places {
        cell_burg[p.cell] = p.id;
    }
    {
        let e = &mut economy;
        let land = output::Land {
            grid,
            cells: &soc.cells,
            biomes: input.biomes,
            development: &soc.development,
            cell_culture: &soc.cell_culture,
            cultures: &soc.cultures,
            places: &soc.places,
            cell_burg: &cell_burg,
            cell_market: &e.cell_market,
            cell_state: &soc.cell_state,
        };
        let infrastructure = prosperity::infrastructure(soc, &e.corruption);
        e.rural = prosperity::rural(
            &e.goods,
            &land,
            &e.markets,
            &e.rural_output,
            &infrastructure,
        );
        e.prosperity = prosperity::cells(&e.rural);
        e.working_cell = (0..n).map(|c| land.working_cell(c)).collect();
    }
    collect(soc, &economy);
    // FMG reads these on demand after the run.
    let infrastructure = prosperity::infrastructure(soc, &economy.corruption);
    economy.burg_living = prosperity::burgs(
        &economy.goods,
        soc,
        &economy.deals,
        &economy.cell_market,
        &infrastructure,
    );
    economy.accounts = accounts::build(&accounts::Input {
        soc,
        economy: &economy,
        burg_living: &economy.burg_living,
        rural: &economy.rural,
        output: &economy.rural_output,
        working: &economy.working_cell,
    });
    let graph = graph(grid, soc, &links, economy.spacing);
    let centers: Vec<usize> = economy
        .markets
        .iter()
        .map(|m| soc.places[m.center as usize - 1].cell)
        .collect();
    let burg_cell: Vec<usize> = soc.places.iter().map(|p| p.cell).collect();
    economy.traffic = traffic::links(&economy.deals, &graph, &centers, &burg_cell);
    Ok(economy)
}

/// The cost of travel from the center of each market to each place, by
/// market then place index, as FMG keeps it in a `Float32Array`
/// (`TradeRoutes.measure`).
pub fn measure(
    graph: &routes::Graph,
    centers: &[usize],
    places: &[crate::society::Place],
) -> Vec<Vec<f32>> {
    let mut search = graph.new_search();
    centers
        .iter()
        .map(|&c| {
            let center = places[c].cell;
            graph.search(center, &mut search);
            places
                .iter()
                .map(|p| {
                    if p.cell == center {
                        0.0
                    } else {
                        search.cost_to(p.cell) as f32
                    }
                })
                .collect()
        })
        .collect()
}

/// The route graph of the economy (`TradeRoutes.buildGraph`).
pub fn graph(
    grid: &Grid,
    soc: &Society,
    links: &[Vec<(usize, bool)>],
    spacing: f64,
) -> routes::Graph {
    let n = grid.len();
    let mut cell_burg = vec![0u32; n];
    let mut port = vec![false; n];
    for p in &soc.places {
        cell_burg[p.cell] = p.id;
        if p.port {
            port[p.cell] = true;
        }
    }
    let point = |c: usize| match cell_burg[c] {
        0 => grid.points[c],
        id => soc.places[id as usize - 1].at,
    };
    routes::Graph::new(links, &point, port, spacing)
}

/// The run of one day (`Production.produce`).
fn produce(input: &Input, soc: &mut Society, links: &[Vec<(usize, bool)>], e: &mut Economy) {
    let Input {
        grid,
        features,
        biomes,
        religions,
        ..
    } = *input;
    let n = grid.len();
    let mut cell_burg = vec![0u32; n];
    for p in &soc.places {
        cell_burg[p.cell] = p.id;
    }
    e.corruption = corruption::Corruption::new(&corruption::Input {
        states: &soc.states,
        provinces: &soc.provinces,
        places: &soc.places,
        pop: &soc.cells.pop,
        cell_state: &soc.cell_state,
        cell_province: &soc.cell_province,
        cell_culture: &soc.cell_culture,
        cell_religion: &religions.cell_religion,
        cell_burg: &cell_burg,
        feature: &features.of_cell,
        decima: e.decima,
    });
    let centers: Vec<usize> = e.markets.iter().map(|m| m.center as usize - 1).collect();
    let market_state: Vec<u32> = centers.iter().map(|&c| soc.places[c].state).collect();
    let graph = graph(grid, soc, links, e.spacing);
    let route_costs = measure(&graph, &centers, &soc.places);
    let world = trade::World {
        goods: &e.goods,
        states: &soc.states,
        places: &soc.places,
        centers: centers.clone(),
        market_state,
        cell_market: &e.cell_market,
        land: output::Land {
            grid,
            cells: &soc.cells,
            biomes,
            development: &soc.development,
            cell_culture: &soc.cell_culture,
            cultures: &soc.cultures,
            places: &soc.places,
            cell_burg: &cell_burg,
            cell_market: &e.cell_market,
            cell_state: &soc.cell_state,
        },
        terms: taxes::TermTable::new(&soc.states, e.goods.len()),
        decima: e.decima,
        route_costs,
    };
    let mut r = trade::Run::new(world);
    // Licenses first: the margin rate follows them.
    let fees: Vec<f64> = centers
        .iter()
        .map(|&c| taxes::market_fee(&soc.states, soc.places[c].state))
        .collect();
    r.set_fees(fees.clone());
    r.bill_licenses();
    let rural = r.w.land.rural(&e.goods);
    r.collect_rural(&rural);
    e.rural_output = rural;
    r.initialize_prices();

    let markets = e.markets.len();
    let mut p = production::Production {
        ix: production::Index::new(&e.goods, markets),
        r,
        orders: orders::Orders::default(),
    };
    // The turn order: the root of the population times the development.
    let mut sorted: Vec<usize> = (0..soc.places.len()).collect();
    let weight = |b: usize| {
        let place = &soc.places[b];
        libm::sqrt(place.population) * f64::from(soc.development[place.cell])
    };
    sorted.sort_by(|&a, &b| weight(b).total_cmp(&weight(a)));
    let mut days: Vec<production::BurgDay> = Vec::new();
    for &b in &sorted {
        let place = &soc.places[b];
        let market = p.r.w.market_of(b);
        let paid = if market.is_some() {
            crate::rand::rn(
                taxes::poll_tax_due(&soc.states, place.state, place.population),
                2,
            )
        } else {
            0.0
        };
        p.r.poll_tax_paid[b] = paid;
        p.r.treasury[b] = 0.0 - paid;
        if let Some(m) = market {
            days.push(p.new_day(b, m));
        }
    }
    p.start_orders(&e.corruption);
    for d in &days {
        p.ix.market_workforce[d.m] += d.workers;
        p.ix.world_workforce += d.workers;
    }

    // A few workers a turn, so the first in line cannot drain a market
    // alone.
    let mut working: Vec<usize> = (0..days.len()).collect();
    while !working.is_empty() {
        let mut worked = false;
        let mut next = Vec::new();
        for &i in &working {
            let d = &mut days[i];
            let before = d.workers_used;
            let has_work = p.run_worker_loop(d, production::WORKERS_PER_TURN);
            worked |= d.workers_used > before;
            // The inputs of the chain under way wait for its next step.
            let inputs = if has_work {
                Some(p.chain_inputs(d.active))
            } else {
                None
            };
            let done = !has_work && d.workers_used >= d.workers - 0.001;
            p.sell_inventory(d, &|g| !inputs.as_ref().is_some_and(|i| i[g]));
            // On its last turn, what one outlet refused goes to the next.
            if done {
                for _ in 0..production::MAX_CLEARING_PASSES {
                    if !p.sell_inventory(d, &|_| true) {
                        break;
                    }
                }
            }
            let share = if has_work && d.workers != 0.0 {
                d.workers_used / d.workers
            } else {
                1.0
            };
            p.fill_demand(d, share);
            p.spend_on_wants(d);
            d.settled = p.r.treasury[d.b];
            if !done {
                next.push(i);
            }
        }
        working = next;
        for m in 0..markets {
            p.r.spend_rural(m);
        }
        p.place_orders(false);
        if !worked {
            break;
        }
    }
    p.r.settle_licenses();
    // Money after the last turn of a place, as the margin of a center, is
    // spent too.
    for d in &mut days {
        if p.r.treasury[d.b] <= d.settled + 0.01 {
            continue;
        }
        p.fill_demand(d, 1.0);
        p.spend_on_wants(d);
    }
    p.place_orders(true);
    let rural_days = p.r.end();
    let finance = p.end_orders();

    // Store the run.
    let r = &p.r;
    e.deals = r.deals.iter().copied().filter(|d| d.units > 0.0).collect();
    let g = e.goods.len();
    for (k, m) in e.markets.iter_mut().enumerate() {
        m.fee_paid = fees[k];
        m.goods = r.rows[k * g..(k + 1) * g].to_vec();
        m.rural = rural_days[k].clone();
    }
    for &b in &sorted {
        let t = crate::rand::rn(r.treasury[b], 2);
        e.places[b].treasury = if t == 0.0 { 0.0 } else { t };
        e.places[b].poll_tax_paid = r.poll_tax_paid[b];
    }
    for d in &days {
        let pe = &mut e.places[d.b];
        pe.product = crate::rand::rn(d.revenue - d.ingredient_costs, 2);
        pe.production = d
            .produced
            .iter()
            .enumerate()
            .filter(|&(_, &u)| u != 0.0)
            .map(|(k, &u)| (k as u32 + 1, crate::rand::rn(u, 2)))
            .collect();
    }
    drop(p);
    for (s, (spent, collected)) in soc.states.iter_mut().zip(finance) {
        s.economy.spent = spent;
        s.economy.collected = collected;
    }
}

/// Add up the taxes of the deals, the poll tax, and the fees into what each
/// state collected, what officials pocketed, and the treasury
/// (`Taxes.collect`).
fn collect(soc: &mut Society, e: &Economy) {
    let n = soc.states.len() + 1;
    let state_of_market = |m: u32| soc.places[e.markets[m as usize - 1].center as usize - 1].state;
    let (mut sales, mut tariffs, mut poll, mut fees) =
        (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    for d in &e.deals {
        let burg = soc.places[d.burg as usize - 1].state;
        let market = state_of_market(d.market);
        let (tax_state, tariff_state) = if d.kind == deals::DealKind::Sale {
            (burg, market)
        } else {
            (market, burg)
        };
        if d.tax != 0.0 {
            sales[tax_state as usize] += d.tax;
        }
        if d.tariff != 0.0 {
            tariffs[tariff_state as usize] += d.tariff;
        }
    }
    for (b, pe) in e.places.iter().enumerate() {
        if pe.poll_tax_paid != 0.0 {
            poll[soc.places[b].state as usize] += pe.poll_tax_paid;
        }
    }
    for m in &e.markets {
        if let Some(rural) = &m.rural {
            for &(s, _, paid) in &rural.poll_tax {
                poll[s as usize] += paid;
            }
        }
        if m.fee_paid != 0.0 {
            fees[state_of_market(m.id) as usize] += m.fee_paid;
        }
    }
    let capital_cells: Vec<Option<usize>> = soc
        .states
        .iter()
        .map(|s| {
            (s.capital as usize)
                .checked_sub(1)
                .map(|c| soc.places[c].cell)
        })
        .collect();
    for (k, s) in soc.states.iter_mut().enumerate() {
        let i = s.id as usize;
        let collected = crate::rand::rn(sales[i] + tariffs[i] + poll[i] + fees[i] + 0.0, 2);
        let expenses = s.economy.budget.total();
        let corruption = e.corruption.of_state(s, capital_cells[k], collected);
        let pocketed = crate::rand::rn(collected.max(0.0) * expenses * corruption / 10000.0, 2);
        let spent = s.economy.spent.iter().fold(0.0, |t, v| t + v);
        s.economy.collected = collected;
        s.economy.pocketed = pocketed;
        s.economy.treasury = crate::rand::rn(collected - pocketed - spent, 2);
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

    /// The money of a place at the end of the day comes from its deals:
    /// sales less their taxes, less purchases, and for a market center the
    /// margin on each deal at its market and on its rural trade, less the
    /// license fee; first the poll tax. Order deals are the state's. FMG
    /// says: within a cent or two.
    #[test]
    fn the_treasury_of_each_place_is_its_deals() {
        use super::deals::DealKind;
        for (seed, template) in [(4, "continents"), (5, "archipelago")] {
            let w = world(seed, template);
            let e = &w.economy;
            let n = w.society.places.len();
            let mut money = vec![0.0; n];
            for (b, pe) in e.places.iter().enumerate() {
                money[b] -= pe.poll_tax_paid;
            }
            for d in &e.deals {
                let b = d.burg as usize - 1;
                let m = &e.markets[d.market as usize - 1];
                let value = d.units * d.price;
                match d.kind {
                    DealKind::Sale => money[b] += value - d.tax - d.tariff,
                    DealKind::Input | DealKind::Demand => money[b] -= value,
                    DealKind::Kept | DealKind::Order => {}
                }
                let c = m.center as usize - 1;
                money[c] += d.margin(m.fee_paid);
            }
            for m in &e.markets {
                let c = m.center as usize - 1;
                money[c] += m.rural.as_ref().map_or(0.0, |r| r.margin) - m.fee_paid;
            }
            for b in 0..n {
                let diff = (money[b] - e.places[b].treasury).abs();
                assert!(
                    diff <= 0.03,
                    "{} ({template}): deals give {} but the treasury is {}",
                    w.society.places[b].name,
                    money[b],
                    e.places[b].treasury
                );
            }
        }
    }

    #[test]
    fn the_run_trades_within_its_rules() {
        use super::deals::DealKind;
        let w = world(6, "continents");
        let e = &w.economy;
        let soc = &w.society;
        assert!(e.deals.len() > 100);
        let table = super::taxes::TermTable::new(&soc.states, e.goods.len());
        for d in &e.deals {
            assert!(d.units > 0.0);
            let burg = soc.places[d.burg as usize - 1].state;
            let market = soc.places[e.markets[d.market as usize - 1].center as usize - 1].state;
            // No trade under an embargo.
            let (exporter, importer) = if d.kind == DealKind::Sale {
                (burg, market)
            } else {
                (market, burg)
            };
            if d.kind != DealKind::Kept {
                assert!(table.costs(0, exporter, importer, d.good, 1.0).is_some());
            }
            // Only an import pays a tariff.
            if d.tariff > 0.0 {
                assert!(super::taxes::is_foreign(burg, market));
            }
            // A state order pays no tariff to the state, and no sales tax at
            // a market of the state.
            if d.kind == DealKind::Order {
                assert_eq!(d.tariff, 0.0);
                if market == burg {
                    assert_eq!(d.tax, 0.0);
                }
            }
        }
        // Prices stay within 0.02 to 5 times the value of each good.
        for m in &e.markets {
            for (g, row) in m.goods.iter().enumerate() {
                let v = e.goods[g].value;
                assert!(row.price >= crate::rand::rn(v * 0.02, 2) - 1e-9);
                assert!(row.price <= v * 5.0 + 1e-9);
                assert!(row.stock >= 0.0);
            }
        }
        // Each state keeps what it collected less what officials pocketed
        // and what it spent.
        for s in &soc.states {
            let f = &s.economy;
            let spent: f64 = f.spent.iter().sum();
            assert!((f.treasury - (f.collected - f.pocketed - spent)).abs() < 0.011);
            assert!(f.pocketed >= 0.0);
        }
        assert!(soc.states.iter().any(|s| s.economy.collected > 0.0));
        // Places make goods, and most end the day with money.
        let profit = e.places.iter().filter(|p| p.treasury > 0.0).count();
        assert!(
            profit * 2 > e.places.len(),
            "{profit} of {}",
            e.places.len()
        );
        assert!(e.places.iter().any(|p| !p.production.is_empty()));
    }

    /// The provinces of a state sum to its accounts: people, product, and
    /// taxes. inkland gives each land cell a province.
    #[test]
    fn the_provinces_of_a_state_sum_to_its_accounts() {
        let w = world(7, "continents");
        let soc = &w.society;
        let a = &w.economy.accounts;
        assert_eq!(a.states.len(), soc.states.len() + 1);
        let close = |x: f64, y: f64| (x - y).abs() <= 1e-6 * x.abs().max(1.0);
        for s in &soc.states {
            let st = &a.states[s.id as usize];
            let p = &st.people;
            assert!(close(p.product, p.rural + p.manufacturing + p.trade));
            let mine: Vec<&super::accounts::ProvinceAccounts> = soc
                .provinces
                .iter()
                .filter(|x| x.state == s.id)
                .map(|x| &a.provinces[x.id as usize - 1])
                .collect();
            let sum = |f: &dyn Fn(&super::accounts::ProvinceAccounts) -> f64| {
                mine.iter().map(|x| f(x)).sum::<f64>()
            };
            assert!(
                close(sum(&|x| x.people.people), p.people),
                "people of {}",
                s.name
            );
            assert!(
                close(sum(&|x| x.people.product), p.product),
                "product of {}",
                s.name
            );
            let taxes = sum(&|x| {
                x.taxes.sales_tax + x.taxes.tariffs + x.taxes.poll_tax + x.taxes.market_fees
            });
            assert!(
                (taxes - st.taxes).abs() < 1e-6 * st.taxes.max(1.0),
                "taxes of {}: {taxes} {}",
                s.name,
                st.taxes
            );
            if let Some(pr) = p.prosperity {
                assert!((0.0..=1.0).contains(&pr));
            }
        }
        let people: f64 = a.states.iter().map(|x| x.people.people).sum();
        assert!(close(people, a.world.people.people));
        // A state consumes what its people and orders bought.
        assert!(a.consumption.iter().flatten().all(|&u| u >= 0.0));
        assert!(a.consumption.iter().flatten().any(|&u| u > 0.0));
        // Each state has an economy type.
        assert!(soc.states.iter().any(|s| {
            super::accounts::economy_type(&a.states[s.id as usize].people, &a.world.people)
                .is_some()
        }));
    }

    #[test]
    fn rural_people_live_where_they_are() {
        let w = world(8, "archipelago");
        let e = &w.economy;
        let cells = &w.society.cells;
        for c in 0..w.grid.len() {
            let peopled = cells.pop[c] > 0.0 && e.cell_market[c] != 0;
            assert_eq!(e.prosperity[c] > 0, peopled, "cell {c}");
            assert!(e.prosperity[c] <= 100);
            assert_eq!(e.rural[c].is_some(), peopled);
        }
        for (b, p) in w.society.places.iter().enumerate() {
            if let Some(l) = &e.burg_living[b] {
                assert!((0.0..=1.0).contains(&l.prosperity), "{}", p.name);
            }
        }
    }

    #[test]
    fn goods_travel_the_routes() {
        let w = world(8, "archipelago");
        let e = &w.economy;
        assert!(!e.traffic.is_empty());
        let links = super::route_links(&w.roads, &w.sea_routes, w.grid.len());
        for l in &e.traffic {
            let [a, b] = l.cells;
            assert!(
                links[a]
                    .iter()
                    .any(|&(m, water)| m == b && water == l.water),
                "{a}-{b}"
            );
            assert!(l.goods.iter().all(|&(_, u)| u > 0.0));
        }
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
