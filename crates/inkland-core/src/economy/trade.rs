//! Trading (`trade.ts`): prices, stock, quotes, carriage, rural spending,
//! and the deal log.
//!
//! A market holds no money, only the stock and the price of each good. The
//! price is a pure function of the demand and the supply of the day:
//! the list price x ((depth + demand) / (depth + supply))^`PRICE_CURVE`. A
//! trade settles at the mean price over the change that it makes, so a big
//! lot pays its own impact.
//!
//! Inside this module a market, a place, and a good go by index (id - 1).

use super::deals::{Deal, DealKind, Transaction, margin_of};
use super::demand::{Need, SAVINGS_RATE, TARGET_FACTORS, VARIETY, WANT_LIMITS, WANT_SHARES};
use super::goods::Good;
use super::markets::margin_rate;
use super::taxes::{TermTable, is_foreign};
use super::{MarketGood, RuralDay};
use crate::rand::rn;
use crate::society::{Place, State};
use std::collections::BTreeMap;

const PRICE_FLOOR_FACTOR: f64 = 0.02;
const PRICE_CEILING_FACTOR: f64 = 5.0;
/// The power of the ratio of demand to supply in a price.
pub const PRICE_CURVE: f64 = 2.0;
/// The expected sales beyond needs, as a multiple of them.
const WANTS_ESTIMATE: f64 = 0.93;
/// How far below its value an ingredient opens where its trades need less
/// than a unit a day.
const OPENING_ELASTICITY: f64 = 0.21;
/// The map diagonal in decimas: the most carriage, paid with no route to
/// the own market.
const MAX_DECIMAS: f64 = 10.0;
/// A buyer stays at its own market while it stocks this much.
pub const MIN_LOCAL_STOCK: f64 = 1.0;
/// Carriage from the market of another state: borders, tolls, longer hauls.
pub const IMPORT_CARRIAGE_FACTOR: f64 = 2.0;
/// Less is no stock to buy from.
const MIN_STOCK: f64 = 0.01;

/// The day of a market: list prices, depth, demand and supply so far, the
/// rural share of the stock, and the money of the rural people
/// (`MarketDay`).
#[derive(Debug, Clone)]
pub struct MarketDay {
    pub list_price: Vec<f64>,
    pub depth: Vec<f64>,
    pub demand: Vec<f64>,
    pub supply: Vec<f64>,
    pub rural: Vec<f64>,
    pub rural_people: f64,
    pub cash: f64,
    pub income: f64,
    pub spent: f64,
    pub margin: f64,
    pub covered: [f64; 5],
    pub bought: Vec<f64>,
    /// By the state of the cells of the rural people.
    pub poll_tax_due: Vec<f64>,
    pub poll_tax_share: Option<f64>,
}

/// The decimas and the carriage factors from each market to a place, kept
/// for the run (`Carriage`).
#[derive(Debug, Clone)]
pub struct Carriage {
    pub decimas: Vec<f64>,
    pub factors: Vec<f64>,
    pub own: Option<usize>,
    /// Decimas to the nearest market that is not its own.
    pub nearest_other: f64,
    /// Markets by distance to the place, filled on first use.
    pub nearest: Option<Vec<usize>>,
}

/// A good in each market, with the orders that the market scans walk
/// (`GoodBook`).
#[derive(Debug, Clone)]
pub struct Book {
    pub stocked: Vec<bool>,
    pub stocked_count: usize,
    /// What the market pays for a unit, before carriage and tariff.
    pub sell: Vec<f64>,
    /// What it asks for a unit, before taxes and carriage.
    pub buy: Vec<f64>,
    /// Markets, the best paying first.
    pub by_sell: Vec<usize>,
    /// Markets, the cheapest first.
    pub by_buy: Vec<usize>,
    pub sell_ranks: Vec<usize>,
    pub buy_ranks: Vec<usize>,
    /// Changes with each change of a market row of the good.
    pub revision: u64,
}

/// The market from which a good reaches a place cheapest (`SourceQuote`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceQuote {
    pub source: usize,
    pub stock: f64,
    pub buy_price: f64,
    pub carriage: f64,
}

/// The market that pays most at the door of a place (`Outlet`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outlet {
    pub market: usize,
    pub price: f64,
    pub carriage: f64,
}

/// The prices of a market at the door of a place (`quoteMarket`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quote {
    pub stock: f64,
    pub buy_price: f64,
    pub sell_price: f64,
    pub carriage: f64,
}

/// The world that the run reads.
pub struct World<'a> {
    pub goods: &'a [Good],
    pub states: &'a [State],
    pub places: &'a [Place],
    /// The place index of the center of each market.
    pub centers: Vec<usize>,
    /// The state of each market: the state of its center.
    pub market_state: Vec<u32>,
    /// The market of each cell, or 0.
    pub cell_market: &'a [u32],
    pub land: super::output::Land<'a>,
    pub terms: TermTable,
    /// A tenth of the map diagonal.
    pub decima: f64,
    /// The route cost from the center of each market to each place, as FMG
    /// keeps it in a `Float32Array`.
    pub route_costs: Vec<Vec<f32>>,
}

impl World<'_> {
    /// The market index of a place, if any (`Markets.ofBurg`).
    pub fn market_of(&self, b: usize) -> Option<usize> {
        (self.cell_market[self.places[b].cell] as usize).checked_sub(1)
    }

    pub fn state_of(&self, b: usize) -> u32 {
        self.places[b].state
    }
}

/// The trade state of the run.
pub struct Run<'a> {
    pub w: World<'a>,
    /// The stock and the price of each good in each market, by market x
    /// goods + good.
    pub rows: Vec<MarketGood>,
    pub fee_paid: Vec<f64>,
    pub margin_rate: Vec<f64>,
    pub days: Vec<MarketDay>,
    /// The sales tax and tariffs that each state collected so far.
    pub taxes: Vec<f64>,
    carriage: Vec<Option<Carriage>>,
    pub books: Vec<Book>,
    /// What of its license each market center has still to pay.
    license_due: Vec<f64>,
    pub treasury: Vec<f64>,
    pub poll_tax_paid: Vec<f64>,
    pub deals: Vec<Deal>,
    deal_ids: BTreeMap<u64, usize>,
}

/// The units that the budget pays for, rounded down to the hundredth so the
/// cost stays within it.
pub(crate) fn affordable(budget: f64, price: f64) -> f64 {
    ((budget / price) * 100.0 + 1e-6).floor() / 100.0
}

/// The mean over t from 0 to x of (1 + t)^power.
fn average_growth(x: f64, power: f64) -> f64 {
    let raised = power + 1.0;
    if raised != 0.0 {
        libm::expm1(raised * libm::log1p(x)) / (raised * x)
    } else {
        libm::log1p(x) / x
    }
}

/// The identity of a deal as one number.
fn deal_key(burg: u32, market: u32, good: u32, kind: DealKind) -> u64 {
    (u64::from(burg) << 35) + (u64::from(market) << 19) + (u64::from(good) << 3) + kind as u64
}

/// Move a market to its place after its price moved, step by step: a trade
/// moves prices little (`reorder`).
fn reorder(order: &mut [usize], ranks: &mut [usize], prices: &[f64], at: usize, descending: bool) {
    let sign = if descending { -1.0 } else { 1.0 };
    let key = prices[at] * sign;
    let mut i = ranks[at];
    while i > 0 && prices[order[i - 1]] * sign > key {
        order[i] = order[i - 1];
        ranks[order[i]] = i;
        i -= 1;
    }
    while i + 1 < order.len() && prices[order[i + 1]] * sign < key {
        order[i] = order[i + 1];
        ranks[order[i]] = i;
        i += 1;
    }
    order[i] = at;
    ranks[at] = i;
}

impl<'a> Run<'a> {
    pub fn new(w: World<'a>) -> Run<'a> {
        let (m, b) = (w.centers.len(), w.places.len());
        let rows = (0..m)
            .flat_map(|_| {
                w.goods.iter().map(|good| MarketGood {
                    stock: 0.0,
                    price: good.value,
                })
            })
            .collect();
        let states = w.states.len() + 1;
        Run {
            rows,
            fee_paid: vec![0.0; m],
            margin_rate: vec![margin_rate(0.0); m],
            days: Vec::new(),
            taxes: vec![0.0; states],
            carriage: vec![None; b],
            books: Vec::new(),
            license_due: vec![0.0; m],
            treasury: vec![0.0; b],
            poll_tax_paid: vec![0.0; b],
            deals: Vec::new(),
            deal_ids: BTreeMap::new(),
            w,
        }
    }

    pub fn goods(&self) -> usize {
        self.w.goods.len()
    }

    pub fn markets(&self) -> usize {
        self.w.centers.len()
    }

    pub fn row(&self, m: usize, g: usize) -> MarketGood {
        self.rows[m * self.goods() + g]
    }

    fn row_mut(&mut self, m: usize, g: usize) -> &mut MarketGood {
        let n = self.goods();
        &mut self.rows[m * n + g]
    }

    /// Sets the license fee of each market and its margin rate.
    pub fn set_fees(&mut self, fees: Vec<f64>) {
        self.margin_rate = fees.iter().map(|&f| margin_rate(f)).collect();
        self.fee_paid = fees;
    }

    /// Bills each market center its license for the day (`billLicenses`).
    pub fn bill_licenses(&mut self) {
        self.license_due = self.fee_paid.clone();
    }

    /// The center pays what the margin has not covered (`settleLicenses`).
    pub fn settle_licenses(&mut self) {
        for m in 0..self.markets() {
            let due = self.license_due[m];
            if due != 0.0 {
                let c = self.w.centers[m];
                self.treasury[c] -= due;
            }
        }
        self.license_due.fill(0.0);
    }

    pub fn customer_buy_price(&self, m: usize, mid: f64) -> f64 {
        rn(mid * (1.0 + self.margin_rate[m]), 2)
    }

    pub fn customer_sell_price(&self, m: usize, mid: f64) -> f64 {
        rn(mid * (1.0 - self.margin_rate[m]), 2)
    }

    /// Every cell delivers its rural output to its market
    /// (`collectRuralProduction`).
    pub fn collect_rural(&mut self, output: &[Vec<(u32, f64)>]) {
        let (g, s) = (self.goods(), self.w.states.len() + 1);
        self.days = (0..self.markets())
            .map(|_| MarketDay {
                list_price: vec![0.0; g],
                depth: vec![0.0; g],
                demand: vec![0.0; g],
                supply: vec![0.0; g],
                rural: vec![0.0; g],
                rural_people: 0.0,
                cash: 0.0,
                income: 0.0,
                spent: 0.0,
                margin: 0.0,
                covered: [0.0; 5],
                bought: vec![0.0; g],
                poll_tax_due: vec![0.0; s],
                poll_tax_share: None,
            })
            .collect();
        self.taxes.fill(0.0);
        let land = &self.w.land;
        for (cell, produced) in output.iter().enumerate() {
            let Some(m) = (land.cell_market[cell] as usize).checked_sub(1) else {
                continue;
            };
            let people = land.cells.pop[cell];
            let state = land.cell_state[cell];
            let due = super::taxes::poll_tax_due(self.w.states, state, people);
            let day = &mut self.days[m];
            day.rural_people += people;
            day.poll_tax_due[state as usize] += due;
            for &(id, amount) in produced {
                let k = id as usize - 1;
                let row = &mut self.rows[m * g + k];
                row.stock = rn(row.stock + amount, 2);
                self.days[m].rural[k] += amount;
            }
        }
    }

    /// Units a person buys of each good a day, plus what the trades that
    /// make its products need (`collectDemand`).
    fn collect_demand(&self) -> Vec<f64> {
        let goods = self.w.goods;
        let n = goods.len();
        let mut consumer = vec![0.0; n];
        for need in Need::ALL {
            let k = need.index();
            let covering: Vec<usize> = (0..n).filter(|&g| goods[g].coverage[k] > 0.0).collect();
            let costs: Vec<f64> = covering
                .iter()
                .map(|&g| goods[g].value / goods[g].coverage[k])
                .collect();
            let shares = super::demand::variety_shares(&costs, VARIETY);
            for (j, &g) in covering.iter().enumerate() {
                consumer[g] += TARGET_FACTORS[k] * shares[j] / goods[g].coverage[k];
            }
        }
        // The goods that use each good, with the amount for each recipe.
        let mut users: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for (g, good) in goods.iter().enumerate() {
            for recipe in &good.recipes {
                for &(id, amount) in recipe {
                    users[id as usize - 1].push((g, amount / good.recipes.len() as f64));
                }
            }
        }
        fn resolve(
            g: usize,
            consumer: &[f64],
            users: &[Vec<(usize, f64)>],
            demand: &mut [Option<f64>],
            path: &mut [bool],
        ) -> f64 {
            if let Some(d) = demand[g] {
                return d;
            }
            if path[g] {
                return 0.0;
            }
            path[g] = true;
            let mut units = consumer[g] * (1.0 + WANTS_ESTIMATE);
            for &(output, amount) in &users[g] {
                units += amount * resolve(output, consumer, users, demand, path);
            }
            path[g] = false;
            demand[g] = Some(units);
            units
        }
        let mut demand = vec![None; n];
        for g in 0..n {
            let mut path = vec![false; n];
            resolve(g, &consumer, &users, &mut demand, &mut path);
        }
        demand.into_iter().map(|d| d.unwrap_or(0.0)).collect()
    }

    /// The opening prices and depth of each market
    /// (`initializeMarketPrices`).
    pub fn initialize_prices(&mut self) {
        let factors = self.collect_demand();
        let mut population = vec![0.0; self.markets()];
        for (b, p) in self.w.places.iter().enumerate() {
            if let Some(m) = self.w.market_of(b)
                && p.population != 0.0
            {
                population[m] += p.population;
            }
        }
        let goods = self.w.goods;
        for m in 0..self.markets() {
            let people = population[m] + self.days[m].rural_people;
            for (g, good) in goods.iter().enumerate() {
                let expected = people * factors[g];
                let day = &mut self.days[m];
                day.depth[g] = expected.max(1.0);
                day.supply[g] = day.rural[g];
                day.list_price[g] = if good.covers_needs() {
                    good.value
                } else {
                    good.value * libm::pow(expected.min(1.0), OPENING_ELASTICITY)
                };
                let price = self.balance_price(m, g);
                self.row_mut(m, g).price = price;
            }
        }
        self.reindex();
    }

    /// Index the books of all goods again from the rows.
    pub fn reindex(&mut self) {
        self.books = (0..self.goods()).map(|g| self.build_book(g)).collect();
    }

    fn build_book(&self, g: usize) -> Book {
        let m = self.markets();
        let sell: Vec<f64> = (0..m)
            .map(|k| self.customer_sell_price(k, self.row(k, g).price))
            .collect();
        let buy: Vec<f64> = (0..m)
            .map(|k| self.customer_buy_price(k, self.row(k, g).price))
            .collect();
        let mut by_sell: Vec<usize> = (0..m).collect();
        by_sell.sort_by(|&a, &b| sell[b].total_cmp(&sell[a]));
        let mut by_buy: Vec<usize> = (0..m).collect();
        by_buy.sort_by(|&a, &b| buy[a].total_cmp(&buy[b]));
        let mut sell_ranks = vec![0; m];
        let mut buy_ranks = vec![0; m];
        for (rank, &at) in by_sell.iter().enumerate() {
            sell_ranks[at] = rank;
        }
        for (rank, &at) in by_buy.iter().enumerate() {
            buy_ranks[at] = rank;
        }
        let stocked: Vec<bool> = (0..m).map(|k| self.row(k, g).stock > MIN_STOCK).collect();
        let stocked_count = stocked.iter().filter(|&&s| s).count();
        Book {
            stocked,
            stocked_count,
            sell,
            buy,
            by_sell,
            by_buy,
            sell_ranks,
            buy_ranks,
            revision: 0,
        }
    }

    /// Changes with the market rows of the good, so a quote can tell that it
    /// is stale (`getRevision`).
    pub fn revision(&self, g: usize) -> u64 {
        self.books[g].revision
    }

    /// A price change that the rounded prices do not show changes no quote
    /// (`touchPrice`).
    fn touch_price(&mut self, m: usize, g: usize) {
        let price = self.row(m, g).price;
        let sell = self.customer_sell_price(m, price);
        let buy = self.customer_buy_price(m, price);
        let book = &self.books[g];
        if sell == book.sell[m] && buy == book.buy[m] {
            return;
        }
        self.touch(m, g);
    }

    fn touch(&mut self, m: usize, g: usize) {
        let row = self.row(m, g);
        let sell = self.customer_sell_price(m, row.price);
        let buy = self.customer_buy_price(m, row.price);
        let book = &mut self.books[g];
        book.revision += 1;
        let stocked = row.stock > MIN_STOCK;
        if stocked != book.stocked[m] {
            if stocked {
                book.stocked_count += 1;
            } else {
                book.stocked_count -= 1;
            }
        }
        book.stocked[m] = stocked;
        book.sell[m] = sell;
        book.buy[m] = buy;
        reorder(&mut book.by_sell, &mut book.sell_ranks, &book.sell, m, true);
        reorder(&mut book.by_buy, &mut book.buy_ranks, &book.buy, m, false);
    }

    fn balance_price(&self, m: usize, g: usize) -> f64 {
        let day = &self.days[m];
        let good = &self.w.goods[g];
        let depth = day.depth[g];
        let ratio = (depth + day.demand[g]) / (depth + day.supply[g]);
        (day.list_price[g] * libm::pow(ratio, PRICE_CURVE)).clamp(
            good.value * PRICE_FLOOR_FACTOR,
            good.value * PRICE_CEILING_FACTOR,
        )
    }

    /// Demand with no stock that changes hands: buyers turned away, or a
    /// share of an import (`addDemand`).
    fn add_demand(&mut self, m: usize, g: usize, units: f64) {
        if self.days[m].depth[g] == 0.0 {
            return;
        }
        self.days[m].demand[g] += units;
        let price = self.balance_price(m, g);
        self.row_mut(m, g).price = price;
        self.touch_price(m, g);
    }

    /// A trade: the stock that it leaves, and the demand or supply that it
    /// adds (`trade`).
    fn trade(&mut self, m: usize, g: usize, stock: f64, demand: f64, supply: f64) {
        self.row_mut(m, g).stock = stock;
        if self.days[m].depth[g] != 0.0 {
            self.days[m].demand[g] += demand;
            self.days[m].supply[g] += supply;
            let price = self.balance_price(m, g);
            self.row_mut(m, g).price = price;
        }
        self.touch(m, g);
    }

    /// Nearer markets share the demand of an import, the nearest most, so it
    /// counts once (`addNearerDemand`).
    fn add_nearer_demand(&mut self, b: usize, source: usize, g: usize, units: f64) {
        self.ensure_carriage(b);
        let c = self.carriage[b].as_mut().expect("carriage");
        let far = c.decimas[source];
        if far == 0.0 || far.is_nan() {
            return;
        }
        if c.nearest.is_none() {
            let mut nearest: Vec<usize> = (0..c.decimas.len()).collect();
            let d = &c.decimas;
            nearest.sort_by(|&x, &y| d[x].total_cmp(&d[y]));
            c.nearest = Some(nearest);
        }
        let c = self.carriage[b].as_ref().expect("carriage");
        let nearest = c.nearest.as_ref().expect("nearest");
        let mut total = 0.0;
        for &k in nearest {
            if c.decimas[k] >= far {
                break;
            }
            total += 1.0 - c.decimas[k] / far;
        }
        let shares: Vec<(usize, f64)> = nearest
            .iter()
            .map(|&k| {
                let share = if c.decimas[k] < far {
                    units * (1.0 - c.decimas[k] / far) / total
                } else {
                    0.0
                };
                (k, share)
            })
            .take_while(|&(_, share)| share >= 0.01)
            .collect();
        for (k, share) in shares {
            self.add_demand(k, g, share);
        }
    }

    /// The mean price over a lot as it moves the price; units are negative
    /// on a sale (`getLotPrice`).
    pub fn lot_price(&self, m: usize, g: usize, units: f64) -> f64 {
        self.lot_price_from(self.row(m, g).price, m, g, units)
    }

    fn lot_price_from(&self, price: f64, m: usize, g: usize, units: f64) -> f64 {
        let day = &self.days[m];
        let depth = day.depth[g];
        if depth == 0.0 || units.abs() < 1e-9 {
            return price;
        }
        let growth = if units > 0.0 {
            average_growth(units / (depth + day.demand[g]), PRICE_CURVE)
        } else {
            average_growth(-units / (depth + day.supply[g]), -PRICE_CURVE)
        };
        let good = &self.w.goods[g];
        (price * growth).clamp(
            good.value * PRICE_FLOOR_FACTOR,
            good.value * PRICE_CEILING_FACTOR,
        )
    }

    /// The units a market expects to sell a day; unlimited outside a run,
    /// where no depth is set (`getDepth`).
    pub fn depth(&self, m: usize, g: usize) -> f64 {
        match self.days[m].depth[g] {
            0.0 => f64::INFINITY,
            d => d,
        }
    }

    pub fn signal_shortage(&mut self, m: usize, g: usize, units: f64) {
        if units > 0.01 {
            self.add_demand(m, g, units);
        }
    }

    /// Take units at the price of the lot; the rural share pays the rural
    /// people, less the margin of the center (`takeStock`).
    fn take_stock(&mut self, m: usize, g: usize, units: f64, price: f64) {
        let stock = self.row(m, g).stock;
        if stock > 0.0 {
            let rural = self.days[m].rural[g];
            let from_rural = rural.min(units * rural / stock);
            if from_rural > 0.0 {
                let earned = from_rural * self.customer_sell_price(m, price);
                let margin = from_rural * price * self.margin_rate[m];
                let day = &mut self.days[m];
                day.rural[g] -= from_rural;
                day.cash += earned;
                day.income += earned;
                self.pay_margin_amount(m, margin);
                self.days[m].margin += margin;
            }
        }
        self.trade(m, g, rn((stock - units).max(0.0), 2), units, 0.0);
    }

    /// The room of the rural people for more of a good of a need.
    fn rural_room(&self, m: usize, k: usize, g: usize) -> f64 {
        let day = &self.days[m];
        (day.rural_people * TARGET_FACTORS[k] * WANT_LIMITS[k] - day.covered[k])
            / self.w.goods[g].coverage[k]
    }

    /// A purchase of the rural people; the cost that it takes.
    fn rural_purchase(&mut self, m: usize, k: usize, g: usize, spend: f64) -> f64 {
        let wanted = self.row(m, g).stock.min(self.rural_room(m, k, g));
        let at_open = wanted.min(spend / self.customer_buy_price(m, self.row(m, g).price));
        // The smaller lot moves the price less, so it stays within the
        // spend.
        let lot = self.lot_price(m, g, at_open);
        let units = at_open.min(spend / self.customer_buy_price(m, lot));
        if spend <= 0.01 || units < 0.01 {
            return 0.0;
        }
        let lot = self.lot_price(m, g, units);
        let cost = units * self.customer_buy_price(m, lot);
        let coverage = self.w.goods[g].coverage;
        let day = &mut self.days[m];
        day.cash -= cost;
        day.spent += cost;
        day.bought[g] += units;
        for (j, &share) in coverage.iter().enumerate() {
            if share > 0.0 {
                day.covered[j] += units * share;
            }
        }
        let margin = units * lot * self.margin_rate[m];
        self.pay_margin_amount(m, margin);
        self.days[m].margin += margin;
        self.take_stock(m, g, units, lot);
        cost
    }

    /// The rural people spend their earnings, less savings and poll tax, at
    /// their market up to their wants (`spendRural`).
    pub fn spend_rural(&mut self, m: usize) {
        let day = &self.days[m];
        let poll_tax = day.poll_tax_due.iter().fold(0.0, |t, d| t + d);
        let budget = day
            .cash
            .min(((1.0 - SAVINGS_RATE) * day.income).min(day.income - poll_tax) - day.spent);
        if budget <= 0.01 {
            return;
        }
        let goods = self.w.goods;
        for need in Need::ALL {
            let k = need.index();
            let mut money = budget * WANT_SHARES[k];
            let offers: Vec<(usize, f64)> = (0..goods.len())
                .filter(|&g| goods[g].coverage[k] > 0.0)
                .map(|g| (g, self.customer_buy_price(m, self.row(m, g).price)))
                .collect();
            // Goods out of stock turn the buyers away, which is demand too.
            let costs: Vec<f64> = offers
                .iter()
                .map(|&(g, price)| price / goods[g].coverage[k])
                .collect();
            let shares = super::demand::variety_shares(&costs, VARIETY);
            let total = money;
            for (j, &(g, price)) in offers.iter().enumerate() {
                let spend = money.min(total * shares[j]);
                if self.row(m, g).stock > 0.01 {
                    money -= self.rural_purchase(m, k, g, spend);
                } else {
                    let units = (spend / price).min(self.rural_room(m, k, g));
                    self.signal_shortage(m, g, units);
                }
            }
            let mut stocked: Vec<(usize, f64)> = offers
                .iter()
                .copied()
                .filter(|&(g, _)| self.row(m, g).stock > 0.01)
                .collect();
            stocked.sort_by(|a, b| {
                (a.1 / goods[a.0].coverage[k]).total_cmp(&(b.1 / goods[b.0].coverage[k]))
            });
            for (g, _) in stocked {
                if money <= 0.01 {
                    break;
                }
                money -= self.rural_purchase(m, k, g, money);
            }
        }
    }

    /// The share of their poll tax that rural people can pay from what they
    /// kept (`getPollTaxShare`).
    fn poll_tax_share(day: &MarketDay) -> f64 {
        let due = day.poll_tax_due.iter().fold(0.0, |t, d| t + d);
        if due > 0.0 {
            (day.cash.max(0.0) / due).min(1.0)
        } else {
            0.0
        }
    }

    /// The rural poll tax, paid before the last orders so the states can
    /// spend it; by state (`settleRuralPollTax`).
    pub fn settle_rural_poll_tax(&mut self) -> Vec<f64> {
        let mut paid = vec![0.0; self.w.states.len() + 1];
        for day in &mut self.days {
            let share = Self::poll_tax_share(day);
            day.poll_tax_share = Some(share);
            for (s, &owed) in day.poll_tax_due.iter().enumerate() {
                if owed != 0.0 {
                    paid[s] += owed * share;
                }
            }
        }
        paid
    }

    /// The decimas and carriage factors from each market to a place
    /// (`getCarriage`), kept for the run.
    pub fn ensure_carriage(&mut self, b: usize) {
        if self.carriage[b].is_some() {
            return;
        }
        let own = self.w.market_of(b);
        let state = self.w.state_of(b);
        let n = self.markets();
        let mut decimas = vec![f64::INFINITY; n];
        let mut factors = vec![1.0; n];
        let mut nearest_other = f64::INFINITY;
        for m in 0..n {
            let distance = self.decimas(m, b);
            let factor = if own.is_some_and(|o| o != m) && is_foreign(self.w.market_state[m], state)
            {
                IMPORT_CARRIAGE_FACTOR
            } else {
                1.0
            };
            decimas[m] = distance;
            factors[m] = factor;
            if Some(m) != own && distance < nearest_other {
                nearest_other = distance;
            }
        }
        self.carriage[b] = Some(Carriage {
            decimas,
            factors,
            own,
            nearest_other,
            nearest: None,
        });
    }

    pub fn carriage_of(&mut self, b: usize) -> &Carriage {
        self.ensure_carriage(b);
        self.carriage[b].as_ref().expect("carriage")
    }

    /// The decimas between the center of the market and the place; infinite
    /// with no route to another market (`getDecimas`).
    pub fn decimas(&self, m: usize, b: usize) -> f64 {
        if b == self.w.centers[m] {
            return 0.0;
        }
        let distance = f64::from(self.w.route_costs[m][b]);
        if distance == f64::INFINITY && self.w.market_of(b) != Some(m) {
            return f64::INFINITY;
        }
        (distance / self.w.decima).min(MAX_DECIMAS)
    }

    /// The carriage of a unit, dearer from another state; infinite with no
    /// route to another market (`getTransport`).
    pub fn transport(&mut self, m: usize, b: usize, g: usize) -> f64 {
        let carriage = self.w.goods[g].carriage;
        let c = self.carriage_of(b);
        let distance = c.decimas[m];
        if distance == f64::INFINITY {
            f64::INFINITY
        } else {
            distance * carriage * c.factors[m]
        }
    }

    /// The tariff for each unit that a market of another state levies on
    /// goods from the place; `None` under an embargo (`getSaleTariff`).
    fn sale_tariff(&self, b: usize, m: usize, g: usize, price: f64, exempt: u32) -> Option<f64> {
        self.w
            .terms
            .costs(
                exempt,
                self.w.state_of(b),
                self.w.market_state[m],
                g as u32 + 1,
                price,
            )
            .map(|c| c.1)
    }

    /// The costs for the place to buy from the market; `None` under an
    /// embargo (`getPurchaseCosts`).
    fn purchase_costs(
        &self,
        m: usize,
        b: usize,
        g: usize,
        price: f64,
        exempt: u32,
    ) -> Option<(f64, f64)> {
        self.w.terms.costs(
            exempt,
            self.w.market_state[m],
            self.w.state_of(b),
            g as u32 + 1,
            price,
        )
    }

    fn price_at_door(with_margin: f64, tax: f64, tariff: f64, carriage: f64) -> f64 {
        rn(with_margin + tax + tariff + carriage, 2)
    }

    /// The best market to sell at, after carriage and the tariff of the
    /// importer; none under an embargo (`getOutlet`).
    pub fn outlet(&mut self, b: usize, g: usize, exempt: u32) -> Option<Outlet> {
        self.ensure_carriage(b);
        let good_carriage = self.w.goods[g].carriage;
        let c = self.carriage[b].as_ref().expect("carriage");
        let book = &self.books[g];
        let own = c.own;
        let least = c.nearest_other * good_carriage;
        let mut best: Option<Outlet> = None;
        let mut best_at = usize::MAX;
        let mut rank = 0;
        let mut first = own.is_some();
        loop {
            let at = if first {
                first = false;
                own.expect("own")
            } else {
                if rank >= book.by_sell.len() {
                    break;
                }
                let at = book.by_sell[rank];
                rank += 1;
                if Some(at) == own {
                    continue;
                }
                if let Some(bst) = best
                    && book.sell[at] - least < bst.price
                {
                    break;
                }
                at
            };
            let distance = c.decimas[at];
            if distance == f64::INFINITY {
                continue;
            }
            let carriage = distance * good_carriage * c.factors[at];
            let before = book.sell[at] - carriage;
            if let Some(bst) = best
                && before < bst.price
            {
                continue;
            }
            let price = self.row(at, g).price;
            let Some(tariff) = self.sale_tariff(b, at, g, price, exempt) else {
                continue;
            };
            let p = before - tariff;
            let better = match best {
                None => true,
                Some(bst) => p > bst.price || (p == bst.price && at < best_at),
            };
            if better {
                best = Some(Outlet {
                    market: at,
                    price: p,
                    carriage,
                });
                best_at = at;
            }
        }
        best
    }

    /// The prices at the door of the place: buying adds taxes and carriage,
    /// selling pays tariff and carriage; 0 under an embargo (`quoteMarket`).
    pub fn quote_market(&mut self, m: usize, g: usize, b: usize, exempt: u32) -> Quote {
        let row = self.row(m, g);
        let carriage = self.transport(m, b, g);
        let Some((tax, tariff)) = self.purchase_costs(m, b, g, row.price, exempt) else {
            return Quote {
                stock: 0.0,
                buy_price: 0.0,
                sell_price: 0.0,
                carriage,
            };
        };
        Quote {
            stock: row.stock,
            buy_price: Self::price_at_door(
                self.customer_buy_price(m, row.price),
                tax,
                tariff,
                carriage,
            ),
            sell_price: self.quote_sale(m, g, b, exempt).0,
            carriage,
        }
    }

    /// What a unit fetches at the door of the place and the carriage in it;
    /// nothing under an embargo (`quoteSale`).
    pub fn quote_sale(&mut self, m: usize, g: usize, b: usize, exempt: u32) -> (f64, f64) {
        let carriage = self.transport(m, b, g);
        let price = self.row(m, g).price;
        let Some(tariff) = self.sale_tariff(b, m, g, price, exempt) else {
            return (0.0, carriage);
        };
        let sell = rn(
            (self.customer_sell_price(m, price) - tariff - carriage).max(0.0),
            2,
        );
        (sell, carriage)
    }

    /// A door price as the price of a lot, units negative on a sale: the
    /// part of the market moves, carriage stays (`atLot`).
    pub fn at_lot(&self, door: f64, carriage: f64, m: usize, g: usize, units: f64) -> f64 {
        let price = self.row(m, g).price;
        if price == 0.0 {
            return door;
        }
        let factor = self.lot_price_from(price, m, g, units) / price;
        let fixed = if units < 0.0 { -carriage } else { carriage };
        (door - fixed) * factor + fixed
    }

    /// Buy from a market, paying margin, taxes, and carriage (`buy`).
    #[allow(clippy::too_many_arguments)]
    pub fn buy(
        &mut self,
        b: usize,
        g: usize,
        units: f64,
        kind: DealKind,
        source: usize,
        exempt: u32,
        budget: f64,
    ) -> Option<Transaction> {
        let market = self.w.market_of(b)?;
        let carriage = self.transport(source, b, g);
        if carriage == f64::INFINITY {
            return None;
        }
        let quote = |run: &Run, lot: f64| {
            run.purchase_costs(source, b, g, lot, exempt)
                .map(|(tax, tariff)| {
                    let price = Self::price_at_door(
                        run.customer_buy_price(source, lot),
                        tax,
                        tariff,
                        carriage,
                    );
                    (tax, tariff, price)
                })
        };
        let row = self.row(source, g);
        let opening = quote(self, row.price)?;
        let at_open = units.min(row.stock).min(affordable(budget, opening.2));
        let lot = self.lot_price(source, g, at_open);
        let at_lot = quote(self, lot).expect("no embargo").2;
        let actual = rn(at_open.min(affordable(budget, at_lot)), 2);
        if actual < 0.01 {
            return None;
        }
        let lot = self.lot_price(source, g, actual);
        let (tax, tariff, price) = quote(self, lot).expect("no embargo");
        let trade = Transaction {
            units: actual,
            price,
            tax: rn(tax * actual, 2),
            tariff: rn(tariff * actual, 2),
            carriage: rn(carriage, 2),
        };
        self.record_deal(b, source, g, kind, &trade);
        self.pay_margin(source, kind, &trade);
        self.take_stock(source, g, actual, lot);
        if source != market {
            self.add_nearer_demand(b, source, g, actual);
        }
        Some(trade)
    }

    /// The own market while it stocks enough, else the cheapest, else what
    /// the own market has left; `None` if none or under an embargo
    /// (`getSupplier`).
    pub fn supplier(
        &mut self,
        b: usize,
        g: usize,
        exempt: u32,
        cheapest: &mut dyn FnMut(&mut Run<'a>) -> Option<usize>,
    ) -> Option<usize> {
        let own = self.w.market_of(b)?;
        let open = self.purchase_costs(own, b, g, 0.0, exempt).is_some();
        let local = if open { self.row(own, g).stock } else { 0.0 };
        if local >= MIN_LOCAL_STOCK {
            return Some(own);
        }
        cheapest(self).or(if local > 0.01 { Some(own) } else { None })
    }

    /// The units sold to its own market that it can take back, at what each
    /// earned; rural stock is not its own (`quoteKeep`).
    pub fn quote_keep(&self, b: usize, g: usize) -> Option<(f64, f64)> {
        let m = self.w.market_of(b)?;
        let sale = self.find_deal(b, m, g, DealKind::Sale)?;
        let sale = self.deals[sale];
        let rural = self.days[m].rural[g];
        let units = sale.units.min(self.row(m, g).stock - rural);
        if units < 0.01 {
            return None;
        }
        Some((units, sale.price - (sale.tax + sale.tariff) / sale.units))
    }

    /// Take back own sales to cover demand, as if never sold: the sale and
    /// its taxes shrink (`keep`).
    pub fn keep(&mut self, b: usize, g: usize, units: f64, budget: f64) -> Option<Transaction> {
        let m = self.w.market_of(b)?;
        let (q_units, q_price) = self.quote_keep(b, g)?;
        let kept = rn(units.min(q_units).min(affordable(budget, q_price)), 2);
        if kept < 0.01 {
            return None;
        }
        let at = self.find_deal(b, m, g, DealKind::Sale).expect("sale");
        let sale = self.deals[at];
        let back = Transaction {
            units: -kept,
            price: sale.price,
            tax: 0.0,
            tariff: 0.0,
            carriage: sale.carriage,
        };
        self.pay_margin(m, DealKind::Sale, &back);
        let left = 1.0 - kept / sale.units;
        let tax = rn(sale.tax * left, 2);
        let tariff = rn(sale.tariff * left, 2);
        self.credit_taxes(&sale, tax - sale.tax, tariff - sale.tariff);
        let d = &mut self.deals[at];
        d.tax = tax;
        d.tariff = tariff;
        d.units = rn(d.units - kept, 2);
        self.record_deal(
            b,
            m,
            g,
            DealKind::Kept,
            &Transaction {
                units: kept,
                ..Transaction::default()
            },
        );
        let stock = self.row(m, g).stock;
        self.trade(m, g, rn((stock - kept).max(0.0), 2), 0.0, -kept);
        Some(Transaction {
            units: kept,
            price: rn(q_price, 2),
            ..Transaction::default()
        })
    }

    /// The units that the market takes before the next fetches under a cent
    /// at the door of the place (`getSellable`).
    fn sellable(&self, m: usize, g: usize, carriage: f64, tariff_rate: f64) -> f64 {
        let kept = 1.0 - self.margin_rate[m] - tariff_rate;
        let price = self.row(m, g).price;
        let least = if kept > 0.0 {
            (carriage + 0.01) / kept
        } else {
            f64::INFINITY
        };
        if price <= least {
            return 0.0;
        }
        let depth = self.days[m].depth[g];
        if depth == 0.0 || least <= self.w.goods[g].value * PRICE_FLOOR_FACTOR {
            return f64::INFINITY;
        }
        (depth + self.days[m].supply[g]) * (libm::pow(price / least, 1.0 / PRICE_CURVE) - 1.0)
    }

    /// Sell to a market the units that fetch something; `None` if none or
    /// under an embargo (`sell`).
    pub fn sell(
        &mut self,
        b: usize,
        g: usize,
        units: f64,
        tax_rate: f64,
        m: usize,
        exempt: u32,
    ) -> Option<Transaction> {
        if units <= 0.0 {
            return None;
        }
        let tariff_rate = self.sale_tariff(b, m, g, 1.0, exempt)?;
        let transport = self.transport(m, b, g);
        let sold = rn(units.min(self.sellable(m, g, transport, tariff_rate)), 2);
        if sold < 0.01 {
            return None;
        }
        let lot = self.lot_price(m, g, -sold);
        let price = rn(self.customer_sell_price(m, lot) - transport, 2);
        let tax = rn(sold * price * tax_rate, 2);
        let trade = Transaction {
            units: sold,
            price,
            tax,
            tariff: rn(sold * tariff_rate * lot, 2),
            carriage: rn(transport, 2),
        };
        self.record_deal(b, m, g, DealKind::Sale, &trade);
        self.pay_margin(m, DealKind::Sale, &trade);
        let stock = self.row(m, g).stock;
        self.trade(m, g, rn(stock + sold, 2), 0.0, sold);
        Some(trade)
    }

    pub fn record_deal(&mut self, b: usize, m: usize, g: usize, kind: DealKind, t: &Transaction) {
        let (burg, market, good) = (b as u32 + 1, m as u32 + 1, g as u32 + 1);
        match self.find_deal(b, m, g, kind) {
            None => {
                let deal = Deal {
                    burg,
                    market,
                    good,
                    kind,
                    units: t.units,
                    price: t.price,
                    tax: t.tax,
                    tariff: t.tariff,
                    carriage: t.carriage,
                };
                self.deal_ids
                    .insert(deal_key(burg, market, good, kind), self.deals.len());
                self.deals.push(deal);
                self.credit_taxes(&deal, t.tax, t.tariff);
            }
            Some(at) => {
                let deal = self.deals[at];
                self.credit_taxes(&deal, t.tax, t.tariff);
                let d = &mut self.deals[at];
                let units = d.units + t.units;
                d.price = (d.units * d.price + t.units * t.price) / units;
                d.carriage = (d.units * d.carriage + t.units * t.carriage) / units;
                d.units = units;
                d.tax += t.tax;
                d.tariff += t.tariff;
            }
        }
    }

    /// The states that the sales tax and the tariff of a deal go to: the
    /// seller and the importer (`getTaxStates`).
    pub fn tax_states(&self, deal: &Deal) -> (u32, u32) {
        let burg = self.w.state_of(deal.burg as usize - 1);
        let market = self.w.market_state[deal.market as usize - 1];
        if deal.kind == DealKind::Sale {
            (burg, market)
        } else {
            (market, burg)
        }
    }

    /// The taxes of the states so far in the run (`creditTaxes`).
    fn credit_taxes(&mut self, deal: &Deal, tax: f64, tariff: f64) {
        if tax == 0.0 && tariff == 0.0 {
            return;
        }
        let (tax_state, tariff_state) = self.tax_states(deal);
        if tax != 0.0 {
            self.taxes[tax_state as usize] += tax;
        }
        if tariff != 0.0 {
            self.taxes[tariff_state as usize] += tariff;
        }
    }

    pub fn taxes_so_far(&self, state: u32) -> f64 {
        self.taxes[state as usize]
    }

    fn find_deal(&self, b: usize, m: usize, g: usize, kind: DealKind) -> Option<usize> {
        self.deal_ids
            .get(&deal_key(b as u32 + 1, m as u32 + 1, g as u32 + 1, kind))
            .copied()
    }

    /// Paid as the trade happens, so the center can spend it the same day
    /// (`payMargin`).
    fn pay_margin(&mut self, m: usize, kind: DealKind, t: &Transaction) {
        let amount = margin_of(self.fee_paid[m], kind, t);
        self.pay_margin_amount(m, amount);
    }

    /// To the center of the market, first to the license that it still owes
    /// (`payMarginAmount`).
    fn pay_margin_amount(&mut self, m: usize, amount: f64) {
        let c = self.w.centers[m];
        let due = self.license_due[m];
        let to_license = if amount > 0.0 { due.min(amount) } else { 0.0 };
        if to_license != 0.0 {
            self.license_due[m] = due - to_license;
        }
        self.treasury[c] += amount - to_license;
    }

    /// The market from which a good reaches the place cheapest, with its
    /// stock and the buy price of the place (`quoteSource`).
    pub fn quote_source(&mut self, b: usize, g: usize, exempt: u32) -> Option<SourceQuote> {
        let market = self.w.market_of(b)?;
        self.ensure_carriage(b);
        let good_carriage = self.w.goods[g].carriage;
        let c = self.carriage[b].as_ref().expect("carriage");
        let book = &self.books[g];
        let least = c.nearest_other * good_carriage;
        let mut best: Option<SourceQuote> = None;
        let mut best_at = usize::MAX;
        let mut unseen = book.stocked_count;
        for &at in &book.by_buy {
            if unseen == 0 {
                break;
            }
            let with_margin = book.buy[at];
            if let Some(bst) = best
                && with_margin + least > bst.buy_price + 0.006
            {
                break;
            }
            if !book.stocked[at] {
                continue;
            }
            unseen -= 1;
            if at == market {
                continue;
            }
            let distance = c.decimas[at];
            if distance == f64::INFINITY {
                continue;
            }
            let carriage = distance * good_carriage * c.factors[at];
            if let Some(bst) = best
                && with_margin + carriage > bst.buy_price + 0.006
            {
                continue;
            }
            let row = self.row(at, g);
            let Some((tax, tariff)) = self.purchase_costs(at, b, g, row.price, exempt) else {
                continue;
            };
            let buy_price = Self::price_at_door(with_margin, tax, tariff, carriage);
            let better = match best {
                None => true,
                Some(bst) => {
                    buy_price < bst.buy_price || (buy_price == bst.buy_price && at < best_at)
                }
            };
            if better {
                best = Some(SourceQuote {
                    source: at,
                    stock: row.stock,
                    buy_price,
                    carriage,
                });
                best_at = at;
            }
        }
        best
    }

    /// End the run: round prices and deals; each market keeps its rural day
    /// and the poll tax its people paid (`endRun`).
    pub fn end(&mut self) -> Vec<Option<RuralDay>> {
        for d in &mut self.deals {
            d.units = rn(d.units, 2);
            d.price = rn(d.price, 4);
            d.tax = rn(d.tax, 2);
            d.tariff = rn(d.tariff, 2);
            d.carriage = rn(d.carriage, 4);
        }
        for row in &mut self.rows {
            row.price = rn(row.price, 2);
        }
        let g = self.goods();
        (0..self.markets())
            .map(|m| {
                let day = &self.days[m];
                let bought = day
                    .bought
                    .iter()
                    .enumerate()
                    .filter(|&(_, &u)| u != 0.0)
                    .map(|(k, &u)| (k as u32 + 1, rn(u, 2)))
                    .collect();
                // Each purchase took the rural share of the stock, so what
                // is left of it is theirs.
                let kept = day
                    .rural
                    .iter()
                    .enumerate()
                    .filter_map(|(k, &u)| {
                        let left = u.min(self.rows[m * g + k].stock);
                        (left >= 0.01).then(|| (k as u32 + 1, rn(left, 2)))
                    })
                    .collect();
                let share = day
                    .poll_tax_share
                    .unwrap_or_else(|| Self::poll_tax_share(day));
                let poll_tax = day
                    .poll_tax_due
                    .iter()
                    .enumerate()
                    .filter(|&(_, &o)| o != 0.0)
                    .map(|(s, &o)| (s as u32, rn(o, 2), rn(o * share, 2)))
                    .collect();
                Some(RuralDay {
                    earned: rn(day.income, 2),
                    spent: rn(day.spent, 2),
                    bought,
                    kept,
                    margin: rn(day.margin, 2),
                    poll_tax,
                })
            })
            .collect()
    }
}

#[cfg(test)]
pub mod kit {
    //! A small world for the tests of the run: a few cells, places, and
    //! states, with routes and markets given by hand.
    use super::*;
    use crate::economy::goods::Good;
    use crate::grid::Grid;
    use crate::population::Cells;
    use crate::rand::Rand;
    use crate::seed::{Layer, SeedTree};

    /// A good of value 10 and carriage 1 that covers food.
    pub fn wheat() -> Good {
        let mut g = crate::economy::goods::catalogue()[8].clone();
        g.id = 1;
        g.name = "Wheat";
        g.value = 10.0;
        g.carriage = 1.0;
        g.coverage = [1.0, 0.0, 0.0, 0.0, 0.0];
        g.culture_type = Vec::new();
        g.biome_output = Vec::new();
        g.recipes = Vec::new();
        g.development = 0;
        g
    }

    pub struct Kit {
        pub grid: Grid,
        pub cells: Cells,
        pub goods: Vec<Good>,
        pub states: Vec<State>,
        pub places: Vec<Place>,
        pub cell_market: Vec<u32>,
        pub cell_burg: Vec<u32>,
        pub cell_state: Vec<u32>,
        pub zeros: Vec<u32>,
        pub biomes: Vec<u8>,
        pub development: Vec<u8>,
    }

    impl Kit {
        /// Places with their state, each on cell `index + 1`; cell `k` is in
        /// market `markets[k]`.
        pub fn new(
            goods: Vec<Good>,
            states: Vec<State>,
            place_states: &[u32],
            markets: &[u32],
        ) -> Kit {
            let mut rng = Rand::new(SeedTree::new(1).part_rng(Layer::Economy, "kit"));
            let grid = Grid::new(1000.0, 800.0, 30, &mut rng);
            let n = grid.len();
            let cells = Cells {
                h: vec![50.0; n],
                t: vec![2; n],
                haven: vec![None; n],
                harbor: vec![0; n],
                area: vec![1.0; n],
                px: 1.0,
                fl: vec![0.0; n],
                conf: vec![0.0; n],
                r: vec![0; n],
                s: vec![0.0; n],
                pop: vec![0.0; n],
                good: vec![0; n],
                group: vec![""; n],
                frozen: vec![false; n],
                isle: vec![false; n],
            };
            let places: Vec<Place> = place_states
                .iter()
                .enumerate()
                .map(|(k, &s)| {
                    let mut p = Place::new(
                        k as u32 + 1,
                        format!("P{}", k + 1),
                        crate::config::PlaceKind::Town,
                        k + 1,
                        [100.0 * (k + 1) as f64, 100.0],
                    );
                    p.state = s;
                    p.population = 100.0;
                    p
                })
                .collect();
            let mut cell_market = vec![0; n];
            cell_market[..markets.len()].copy_from_slice(markets);
            let mut cell_burg = vec![0; n];
            let mut cell_state = vec![0; n];
            for p in &places {
                cell_burg[p.cell] = p.id;
                cell_state[p.cell] = p.state;
            }
            Kit {
                grid,
                cells,
                goods,
                states,
                places,
                cell_market,
                cell_burg,
                cell_state,
                zeros: vec![0; n],
                biomes: vec![6; n],
                development: vec![20; n],
            }
        }

        /// A run with the markets centered at these place indices, and the
        /// route cost from each center to each place. The days have no
        /// depth: prices stay, as outside a run of FMG.
        pub fn run(&self, centers: &[usize], route_costs: Vec<Vec<f32>>) -> Run<'_> {
            let world = World {
                goods: &self.goods,
                states: &self.states,
                places: &self.places,
                centers: centers.to_vec(),
                market_state: centers.iter().map(|&c| self.places[c].state).collect(),
                cell_market: &self.cell_market,
                land: crate::economy::output::Land {
                    grid: &self.grid,
                    cells: &self.cells,
                    biomes: &self.biomes,
                    development: &self.development,
                    cell_culture: &self.zeros,
                    cultures: &[],
                    places: &self.places,
                    cell_burg: &self.cell_burg,
                    cell_market: &self.cell_market,
                    cell_state: &self.cell_state,
                },
                terms: TermTable::new(&self.states, self.goods.len()),
                decima: libm::hypot(1000.0, 800.0) / 10.0,
                route_costs,
            };
            let mut r = Run::new(world);
            r.collect_rural(&vec![Vec::new(); self.grid.len()]);
            r.reindex();
            r
        }
    }

    /// A state with a sales tax and tariff rules.
    pub fn state(
        id: u32,
        sales_tax: f64,
        tariffs: Vec<crate::economy::finance::TariffRule>,
    ) -> State {
        let mut s = State {
            id,
            form: "Monarchy".into(),
            ..State::default()
        };
        s.economy.sales_tax = sales_tax;
        s.economy.tariffs = tariffs;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::kit::{Kit, state, wheat};
    use super::*;
    use crate::economy::finance::{Tariff, TariffRule};

    const MARGIN: f64 = 0.04;

    fn set(r: &mut Run, m: usize, stock: f64, price: f64) {
        *r.row_mut(m, 0) = MarketGood { stock, price };
        r.reindex();
    }

    /// One market centered at place 0, which also trades there.
    fn one(states: Vec<State>, place_states: &[u32]) -> Kit {
        Kit::new(vec![wheat()], states, place_states, &[0, 1, 1, 1, 1])
    }

    #[test]
    fn a_purchase_floors_its_units_and_keeps_within_the_budget() {
        let k = one(vec![], &[0]);
        let mut r = k.run(&[0], vec![vec![0.0]]);
        set(&mut r, 0, 100.0, 10.0);
        let t = r.buy(0, 0, 5.0, DealKind::Demand, 0, 0, 15.0).unwrap();
        let price = rn(10.0 * (1.0 + MARGIN), 2);
        assert_eq!(t.price, price);
        assert_eq!(t.units, (15.0 / price * 100.0).floor() / 100.0);
        assert!(t.units * t.price <= 15.0);
        assert!((r.row(0, 0).stock - (100.0 - t.units)).abs() < 1e-9);
        // Too little money for a hundredth of a unit buys nothing.
        assert!(r.buy(0, 0, 5.0, DealKind::Demand, 0, 0, 0.05).is_none());
    }

    #[test]
    fn the_center_takes_the_margin_as_each_trade_happens_and_gives_it_back_on_goods_kept() {
        let k = one(vec![], &[0]);
        let mut r = k.run(&[0], vec![vec![0.0]]);
        set(&mut r, 0, 100.0, 10.0);
        r.buy(0, 0, 2.0, DealKind::Demand, 0, 0, f64::INFINITY);
        assert!((r.treasury[0] - 2.0 * 10.0 * MARGIN).abs() < 1e-9);
        r.sell(0, 0, 4.0, 0.0, 0, 0);
        r.keep(0, 0, 1.0, f64::INFINITY);
        let margins: f64 = r.deals.iter().map(|d| d.margin(0.0)).sum();
        assert!((r.treasury[0] - margins).abs() < 0.05);
    }

    #[test]
    fn the_license_comes_out_of_the_margin_and_the_rest_at_the_end_of_the_day() {
        let k = one(vec![], &[0]);
        let mut r = k.run(&[0], vec![vec![0.0]]);
        set(&mut r, 0, 100.0, 10.0);
        r.set_fees(vec![1.0]);
        let rate = r.margin_rate[0];
        let margin = |units: f64| units * 10.0 * rate;
        r.bill_licenses();
        r.buy(0, 0, 2.0, DealKind::Demand, 0, 0, f64::INFINITY);
        assert_eq!(r.treasury[0], 0.0);
        r.buy(0, 0, 2.0, DealKind::Demand, 0, 0, f64::INFINITY);
        assert!((r.treasury[0] - (margin(4.0) - 1.0)).abs() < 1e-9);
        r.settle_licenses();
        assert!((r.treasury[0] - (margin(4.0) - 1.0)).abs() < 1e-9);
        // A quiet market.
        r.set_fees(vec![5.0]);
        let rate = r.margin_rate[0];
        r.treasury[0] = 0.0;
        r.bill_licenses();
        r.buy(0, 0, 2.0, DealKind::Demand, 0, 0, f64::INFINITY);
        assert_eq!(r.treasury[0], 0.0);
        r.settle_licenses();
        assert!((r.treasury[0] - (2.0 * 10.0 * rate - 5.0)).abs() < 1e-9);
    }

    #[test]
    fn a_purchase_pays_the_sales_tax_of_the_state_of_the_market() {
        let k = one(vec![state(1, 0.2, vec![])], &[1]);
        let mut r = k.run(&[0], vec![vec![0.0]]);
        set(&mut r, 0, 100.0, 10.0);
        let t = r
            .buy(0, 0, 2.0, DealKind::Demand, 0, 0, f64::INFINITY)
            .unwrap();
        assert!((t.price - (10.0 * (1.0 + MARGIN) + 2.0)).abs() < 0.01);
        assert!((t.tax - 4.0).abs() < 0.01);
        assert_eq!(r.tax_states(&r.deals[0]).0, 1);
        assert!((r.taxes_so_far(1) - 4.0).abs() < 1e-9);
    }

    /// Market 0 exports from place 0 of state 1; place 1 of state 2 lives in
    /// market 1, centered at itself, 100 units from the exporter.
    fn import(states: Vec<State>) -> Kit {
        Kit::new(vec![wheat()], states, &[1, 2], &[0, 1, 2])
    }

    fn import_routes(cost: f32) -> Vec<Vec<f32>> {
        vec![vec![0.0, cost], vec![f32::INFINITY, 0.0]]
    }

    fn carriage(cost: f64) -> f64 {
        cost / (libm::hypot(1000.0, 800.0) / 10.0)
    }

    #[test]
    fn an_import_pays_price_margin_and_carriage_dearer_from_another_state() {
        let k = import(vec![state(1, 0.0, vec![]), state(2, 0.0, vec![])]);
        let mut r = k.run(&[0, 1], import_routes(100.0));
        set(&mut r, 0, 100.0, 5.0);
        set(&mut r, 1, 0.0, 20.0);
        let t = r
            .buy(1, 0, 10.0, DealKind::Input, 0, 0, f64::INFINITY)
            .unwrap();
        assert_eq!(t.units, 10.0);
        let transport = carriage(100.0) * IMPORT_CARRIAGE_FACTOR;
        assert!((t.price - (rn(5.0 * (1.0 + MARGIN), 2) + transport)).abs() < 0.01);
        assert_eq!(r.deals.len(), 1);
        assert_eq!((r.deals[0].burg, r.deals[0].market), (2, 1));
        assert_eq!(r.row(0, 0).stock, 90.0);
        // The budget caps the units at the price at the door.
        let t = r.buy(1, 0, 10.0, DealKind::Input, 0, 0, 13.0).unwrap();
        assert!(t.units * t.price <= 13.0);
        // From the own market there is no import carriage.
        assert_eq!(r.transport(1, 1, 0), 0.0);
        assert!((r.transport(0, 1, 0) - transport).abs() < 1e-9);
    }

    #[test]
    fn the_cheapest_source_wins_after_carriage_and_none_without_a_route() {
        let k = Kit::new(
            vec![wheat()],
            vec![state(1, 0.0, vec![]), state(2, 0.0, vec![])],
            &[1, 2, 1],
            &[0, 1, 2, 3],
        );
        let routes = vec![
            vec![0.0, 100.0, 0.0],
            vec![f32::INFINITY, 0.0, 0.0],
            vec![0.0, 300.0, 0.0],
        ];
        let mut r = k.run(&[0, 1, 2], routes);
        set(&mut r, 0, 100.0, 5.0);
        set(&mut r, 1, 0.0, 20.0);
        set(&mut r, 2, 50.0, 4.5);
        // The far market is cheaper, but its carriage costs more than the gap.
        assert_eq!(r.quote_source(1, 0, 0).unwrap().source, 0);
        set(&mut r, 2, 50.0, 0.1);
        assert_eq!(r.quote_source(1, 0, 0).unwrap().source, 2);

        let k = import(vec![state(1, 0.0, vec![]), state(2, 0.0, vec![])]);
        let mut r = k.run(&[0, 1], import_routes(f32::INFINITY));
        set(&mut r, 0, 100.0, 5.0);
        assert!(r.quote_source(1, 0, 0).is_none());
        assert!(
            r.buy(1, 0, 10.0, DealKind::Input, 0, 0, f64::INFINITY)
                .is_none()
        );
    }

    #[test]
    fn carriage_stops_at_ten_decimas_for_the_own_market() {
        let k = Kit::new(
            vec![wheat()],
            vec![state(1, 0.0, vec![]), state(2, 0.0, vec![])],
            &[1, 2],
            &[0, 1, 1],
        );
        let r = k.run(&[0], vec![vec![0.0, 1e6]]);
        let mut r = r;
        assert_eq!(r.transport(0, 1, 0), 10.0);
        let mut r2 = k.run(&[0], vec![vec![0.0, f32::INFINITY]]);
        assert_eq!(r2.transport(0, 1, 0), 10.0);
        r.ensure_carriage(1);
    }

    #[test]
    fn an_embargo_keeps_a_place_out_of_its_own_home_market() {
        let embargo = TariffRule {
            states: Some(vec![1]),
            goods: None,
            rate: Tariff::Embargo,
        };
        let k = Kit::new(
            vec![wheat()],
            vec![state(1, 0.0, vec![]), state(2, 0.0, vec![embargo])],
            &[1, 2],
            &[0, 1, 1],
        );
        let mut r = k.run(&[0], vec![vec![0.0, 100.0]]);
        set(&mut r, 0, 100.0, 5.0);
        assert!(r.supplier(1, 0, 0, &mut |_| None).is_none());
        assert!(
            r.buy(1, 0, 10.0, DealKind::Input, 0, 0, f64::INFINITY)
                .is_none()
        );
        assert!(r.sell(1, 0, 10.0, 0.0, 0, 0).is_none());
        assert_eq!(r.quote_sale(0, 0, 1, 0).0, 0.0);
    }

    #[test]
    fn a_place_pays_the_tariff_of_its_own_state_at_a_home_market_of_another_state() {
        let tariff = TariffRule {
            states: None,
            goods: None,
            rate: Tariff::Rate(0.1),
        };
        let k = Kit::new(
            vec![wheat()],
            vec![state(1, 0.0, vec![]), state(2, 0.0, vec![tariff])],
            &[1, 2],
            &[0, 1, 1],
        );
        let mut r = k.run(&[0], vec![vec![0.0, 100.0]]);
        set(&mut r, 0, 100.0, 5.0);
        let t = r
            .buy(1, 0, 10.0, DealKind::Input, 0, 0, f64::INFINITY)
            .unwrap();
        assert!((t.tariff - 10.0 * 0.1 * 5.0).abs() < 0.01);
        assert_eq!(r.tax_states(&r.deals[0]).1, 2);
    }

    #[test]
    fn a_price_follows_demand_against_supply_in_any_order_of_trades() {
        let k = one(vec![], &[0, 0]);
        let prices = |order: &[bool]| {
            let mut r = k.run(&[0], vec![vec![0.0, 0.0]]);
            r.days[0].depth[0] = 10.0;
            r.days[0].list_price[0] = 10.0;
            set(&mut r, 0, 100.0, 10.0);
            for &buying in order {
                if buying {
                    r.buy(1, 0, 5.0, DealKind::Demand, 0, 0, f64::INFINITY);
                } else {
                    r.sell(1, 0, 5.0, 0.0, 0, 0);
                }
            }
            r.row(0, 0).price
        };
        let a = prices(&[true, false, true]);
        let b = prices(&[false, true, true]);
        assert!((a - b).abs() < 1e-9, "{a} {b}");
        // Two units of demand more than supply: (10 + 5) / 10, squared.
        let k2 = one(vec![], &[0, 0]);
        let mut r = k2.run(&[0], vec![vec![0.0, 0.0]]);
        r.days[0].depth[0] = 10.0;
        r.days[0].list_price[0] = 10.0;
        set(&mut r, 0, 100.0, 10.0);
        r.buy(1, 0, 5.0, DealKind::Demand, 0, 0, f64::INFINITY);
        assert!((r.row(0, 0).price - 10.0 * 1.5 * 1.5).abs() < 1e-9);
    }

    #[test]
    fn a_lot_pays_the_mean_price_over_the_move_that_it_makes() {
        let k = one(vec![], &[0]);
        let mut r = k.run(&[0], vec![vec![0.0]]);
        r.days[0].depth[0] = 10.0;
        r.days[0].list_price[0] = 10.0;
        set(&mut r, 0, 100.0, 10.0);
        // The mean of 10 x (1 + t)^2 for t from 0 to 1 is 10 x 7 / 3.
        assert!((r.lot_price(0, 0, 10.0) - 70.0 / 3.0).abs() < 1e-9);
        // A sale: the mean of 10 x (1 + t)^-2 is 10 x 1 / 2.
        assert!((r.lot_price(0, 0, -10.0) - 5.0).abs() < 1e-9);
        assert_eq!(r.lot_price(0, 0, 0.0), 10.0);
    }

    #[test]
    fn a_sale_brings_only_the_units_that_fetch_something() {
        let k = one(vec![], &[0, 0]);
        let mut r = k.run(&[0], vec![vec![0.0, 400.0]]);
        r.days[0].depth[0] = 10.0;
        r.days[0].list_price[0] = 1.0;
        set(&mut r, 0, 0.0, 1.0);
        // Carriage: 400 units of route are about 3.1 decimas; the price is
        // 1, so no unit pays its way to the market.
        assert!(r.sell(1, 0, 50.0, 0.0, 0, 0).is_none());
        // At a price of 10 a glut sells until the next unit fetches under a
        // cent: (depth + supply) x ((price / least)^(1/2) - 1) units.
        let mut r = k.run(&[0], vec![vec![0.0, 400.0]]);
        r.days[0].depth[0] = 10.0;
        r.days[0].list_price[0] = 10.0;
        set(&mut r, 0, 0.0, 10.0);
        let least = (carriage(400.0) + 0.01) / (1.0 - MARGIN);
        let sellable = 10.0 * (libm::sqrt(10.0 / least) - 1.0);
        let t = r.sell(1, 0, 50.0, 0.0, 0, 0).unwrap();
        assert_eq!(t.units, rn(sellable, 2));
        assert!(t.price >= 0.0);
    }

    #[test]
    fn a_place_keeps_back_only_its_own_goods_and_never_the_rural_produce() {
        let k = one(vec![], &[0, 0]);
        let mut r = k.run(&[0], vec![vec![0.0, 0.0]]);
        set(&mut r, 0, 10.0, 10.0);
        r.days[0].rural[0] = 10.0;
        assert!(r.quote_keep(1, 0).is_none());
        r.sell(1, 0, 4.0, 0.0, 0, 0);
        let (units, _) = r.quote_keep(1, 0).unwrap();
        assert_eq!(units, 4.0);
        let kept = r.keep(1, 0, 10.0, f64::INFINITY).unwrap();
        assert_eq!(kept.units, 4.0);
        assert!(r.quote_keep(1, 0).is_none());
        assert_eq!(r.row(0, 0).stock, 10.0);
    }

    #[test]
    fn the_countryside_earns_for_its_produce_less_the_margin_of_the_center() {
        let k = one(vec![], &[0, 0]);
        let mut r = k.run(&[0], vec![vec![0.0, 0.0]]);
        set(&mut r, 0, 10.0, 10.0);
        r.days[0].rural[0] = 10.0;
        let t = r
            .buy(1, 0, 5.0, DealKind::Demand, 0, 0, f64::INFINITY)
            .unwrap();
        assert_eq!(t.units, 5.0);
        let day = &r.days[0];
        assert!((day.rural[0] - 5.0).abs() < 1e-9);
        assert!((day.income - 5.0 * rn(10.0 * (1.0 - MARGIN), 2)).abs() < 1e-9);
        assert!((day.margin - 5.0 * 10.0 * MARGIN).abs() < 1e-9);
    }

    #[test]
    fn rural_people_keep_back_their_poll_tax_and_pay_it_as_far_as_their_money_goes() {
        let k = one(vec![state(1, 0.0, vec![])], &[1, 1]);
        let mut r = k.run(&[0], vec![vec![0.0, 0.0]]);
        r.days[0].poll_tax_due[1] = 10.0;
        r.days[0].cash = 4.0;
        let paid = r.settle_rural_poll_tax();
        assert!((paid[1] - 4.0).abs() < 1e-9);
        let days = r.end();
        let rural = days[0].as_ref().unwrap();
        assert_eq!(rural.poll_tax, vec![(1, 10.0, 4.0)]);
    }

    #[test]
    fn an_import_counts_as_demand_once_in_the_markets_nearer_the_buyer() {
        // Place 1 lives in market 1 (its own center); market 0 is far and
        // sells; market 2 lies between.
        let k = Kit::new(vec![wheat()], vec![], &[0, 0, 0], &[0, 1, 2, 3]);
        let routes = vec![
            vec![0.0, 800.0, 0.0],
            vec![f32::INFINITY, 0.0, 0.0],
            vec![0.0, 400.0, 0.0],
        ];
        let mut r = k.run(&[0, 1, 2], routes);
        for day in r.days.iter_mut() {
            day.depth[0] = 100.0;
            day.list_price[0] = 10.0;
        }
        set(&mut r, 0, 100.0, 1.0);
        set(&mut r, 1, 0.0, 10.0);
        set(&mut r, 2, 0.0, 10.0);
        r.buy(1, 0, 10.0, DealKind::Demand, 0, 0, f64::INFINITY)
            .unwrap();
        // The own market is nearest and takes the most; the market between
        // takes less; the source takes none.
        let (own, between) = (r.days[1].demand[0], r.days[2].demand[0]);
        assert!(own > between && between > 0.0, "{own} {between}");
        assert!((own + between - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_deposit_yields_the_people_of_its_cell_up_to_10_scaled_to_the_map() {
        let mut k = one(vec![], &[0]);
        let n = k.grid.len() as f64;
        k.cells.pop[3] = 4.0;
        k.cells.pop[4] = 40.0;
        let r = k.run(&[0], vec![vec![0.0]]);
        let g = &k.goods[0];
        let scale = n / 5000.0;
        let rich = crate::economy::output::DEPOSIT_RICHNESS;
        assert!((r.w.land.deposit(g, 3) - 4.0 * rich * scale).abs() < 1e-9);
        assert!((r.w.land.deposit(g, 4) - 10.0 * rich * scale).abs() < 1e-9);
        // Beyond the technology of the cell, nothing.
        let mut dear = g.clone();
        dear.development = 90;
        assert_eq!(r.w.land.deposit(&dear, 3), 0.0);
    }
}
