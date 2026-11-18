//! The run of one day (`production-generator.ts`, `Production.produce`).
//!
//! - Places take turns by the root of their population times the
//!   development of their cell, the most first.
//! - Round after round, each place puts up to `WORKERS_PER_TURN` workers to
//!   work and sells each finished good at once, sells what is left where it
//!   fetches most, buys the share of its needs that matches the work done,
//!   and spends what it can spare on wants.
//! - After each round the rural people spend their earnings and the states
//!   place their orders. The run ends after a round in which no place
//!   works.
//!
//! Inside this module a market, a place, and a good go by index (id - 1).

use super::deals::{DealKind, Transaction};
use super::demand::{
    NOVELTY, SAVINGS_RATE, VARIETY, WANT_LIMITS, WANT_SHARES, WEALTH_WANTS, demand_targets,
    needs_cost, variety_shares,
};
use super::trade::{Outlet, Run, SourceQuote};
use crate::rand::rn;
use std::rc::Rc;

/// The most base value that the deposit of a place yields a day.
pub const MAX_DEPOSIT_VALUE: f64 = 200.0;
/// The units a worker gets a day from the deposit of its place, before
/// multipliers.
pub const DEPOSIT_OUTPUT: f64 = 2.0;
const MAX_WORKERS: f64 = 300.0;
/// The townspeople who work in trades.
pub const WORKFORCE_SHARE: f64 = 0.32;
pub const WORKERS_PER_TURN: f64 = 5.0;
/// The outlets that a place tries for its leftovers on its last turn.
pub const MAX_CLEARING_PASSES: usize = 3;
/// A step sells at most this share of the sales of a day where it sells.
const LOT_DEPTH_SHARE: f64 = 0.5;
/// The fewest workers in a step.
const MIN_STEP: f64 = 0.5;
/// The fewest coins that a place spends on a good.
const MIN_PURCHASE: f64 = 1.0;
/// The same when all that it has for the need or want is less.
const MIN_SMALL_PURCHASE: f64 = 0.25;
pub const BURG_SPECIALIZATION: f64 = 1.89;
pub const MARKET_SPECIALIZATION: f64 = 0.6;

/// A recipe: the good that it makes and its ingredients by good index.
#[derive(Debug, Clone)]
pub struct Recipe {
    pub good: usize,
    pub ingredients: Vec<(usize, f64)>,
}

/// Quotes by good; an earlier quote holds while no market row of the good
/// has changed (`QuoteCache`).
#[derive(Debug, Clone)]
pub struct QuoteCache<T> {
    pub turn: u64,
    /// The value, its turn, and the revision of the book.
    pub quotes: Vec<Option<(T, u64, u64)>>,
}

impl<T: Copy> QuoteCache<T> {
    fn new(goods: usize) -> Self {
        QuoteCache {
            turn: 0,
            quotes: vec![None; goods],
        }
    }

    /// The quote of the turn; an earlier one stands while the rows of the
    /// good are unchanged (`cached`).
    fn get(&mut self, g: usize, revision: u64, quote: impl FnOnce() -> T) -> T {
        if let Some(entry) = self.quotes[g].as_mut()
            && (entry.1 == self.turn || entry.2 == revision)
        {
            entry.1 = self.turn;
            return entry.0;
        }
        let value = quote();
        self.quotes[g] = Some((value, self.turn, revision));
        value
    }
}

/// The day of a place in the run (`BurgDay`).
#[derive(Debug, Clone)]
pub struct BurgDay {
    pub b: usize,
    pub m: usize,
    pub workers: f64,
    pub inventory: Vec<f64>,
    /// By good, what its deposit yielded included.
    pub produced: Vec<f64>,
    /// The good of the cell of the place.
    pub deposit: Option<usize>,
    pub deposit_left: f64,
    /// What it can make: goods with recipes, and its deposit.
    pub goals: Vec<usize>,
    pub tax_rate: f64,
    pub ingredient_costs: f64,
    pub revenue: f64,
    pub workers_used: f64,
    /// On the needs and wants of the place.
    pub spent: f64,
    /// The treasury after the place last bought for its people.
    pub settled: f64,
    pub outlets: QuoteCache<Option<Outlet>>,
    pub covered: [f64; 5],
    pub consumed: Vec<f64>,
    pub workers_by_good: Vec<f64>,
    pub active: Option<usize>,
    pub sources: QuoteCache<Option<SourceQuote>>,
    pub modifiers: Vec<Option<f64>>,
}

/// What the run reads for each good, and the workers of the run
/// (`ProductionIndex`).
#[derive(Debug, Clone)]
pub struct Index {
    pub coverage: Vec<[f64; 5]>,
    /// The goods of each need with their coverage, the best first.
    pub demand_goods: Vec<Vec<(usize, f64)>>,
    pub recipes: Rc<Vec<Vec<Recipe>>>,
    pub productive: Vec<usize>,
    chain_inputs: Vec<Option<Rc<Vec<bool>>>>,
    pub market_workers: Vec<Vec<f64>>,
    pub market_workforce: Vec<f64>,
    pub world_workers: Vec<f64>,
    pub world_workforce: f64,
    pub needs_cost: f64,
}

/// What a step makes (`PlannedAction`).
#[derive(Debug, Clone)]
pub struct Action {
    pub good: usize,
    pub ingredients: Vec<(usize, f64)>,
    pub max_yield: f64,
}

/// A plan for a goal (`GoalActionPlan`).
#[derive(Debug, Clone)]
struct Plan {
    workers_needed: f64,
    market_cost: f64,
    projected_gain: f64,
    normalized_gain: f64,
    action: Action,
}

/// The decision of a step (`ProductionDecision`).
#[derive(Debug, Clone)]
struct Decision {
    action: Action,
    goal: usize,
    runner_up: f64,
}

/// The stock that a plan may still use (`PlanningStock`).
#[derive(Debug, Clone)]
struct Stock {
    inventory: Vec<f64>,
    market: Vec<Option<f64>>,
}

/// A quote with the market where the place buys (`MarketQuote`).
#[derive(Debug, Clone, Copy)]
struct MarketQuote {
    stock: f64,
    buy_price: f64,
    source: usize,
    source_carriage: f64,
}

/// A goal ranked for the planning (`RankedGoal`).
struct Ranked {
    good: usize,
    bound: f64,
    step: f64,
    sale_value: f64,
}

/// The production run: the trade state, the index, and the state orders.
pub struct Production<'a> {
    pub r: Run<'a>,
    pub ix: Index,
    pub orders: super::orders::Orders,
}

impl Index {
    pub fn new(goods: &[super::goods::Good], markets: usize) -> Index {
        let n = goods.len();
        let coverage: Vec<[f64; 5]> = goods.iter().map(|g| g.coverage).collect();
        let demand_goods: Vec<Vec<(usize, f64)>> = (0..5)
            .map(|k| {
                let mut list: Vec<(usize, f64)> = (0..n)
                    .filter(|&g| coverage[g][k] > 0.0)
                    .map(|g| (g, coverage[g][k]))
                    .collect();
                list.sort_by(|a, b| {
                    b.1.total_cmp(&a.1)
                        .then(goods[a.0].value.total_cmp(&goods[b.0].value))
                        .then(a.0.cmp(&b.0))
                });
                list
            })
            .collect();
        let mut recipes: Vec<Vec<Recipe>> = vec![Vec::new(); n];
        for (g, good) in goods.iter().enumerate() {
            for r in &good.recipes {
                if r.is_empty() {
                    continue;
                }
                recipes[g].push(Recipe {
                    good: g,
                    ingredients: r.iter().map(|&(id, a)| (id as usize - 1, a)).collect(),
                });
            }
        }
        let productive = (0..n).filter(|&g| !recipes[g].is_empty()).collect();
        Index {
            coverage,
            demand_goods,
            recipes: Rc::new(recipes),
            productive,
            chain_inputs: vec![None; n],
            market_workers: vec![vec![0.0; n]; markets],
            market_workforce: vec![0.0; markets],
            world_workers: vec![0.0; n],
            world_workforce: 0.0,
            needs_cost: needs_cost(goods),
        }
    }
}

/// The workers of a place (`getWorkers`).
pub fn workers(population: f64) -> f64 {
    rn(population * WORKFORCE_SHARE, 2).min(MAX_WORKERS)
}

impl Production<'_> {
    fn goods(&self) -> usize {
        self.r.goods()
    }

    /// A new day for a place in a market (`createBurgDay`).
    pub fn new_day(&self, b: usize, m: usize) -> BurgDay {
        let n = self.goods();
        let place = &self.r.w.places[b];
        let deposit = (self.r.w.land.cells.good[place.cell] as usize).checked_sub(1);
        let mut goals = self.ix.productive.clone();
        if let Some(d) = deposit
            && !goals.contains(&d)
        {
            goals.push(d);
        }
        let deposit_left = match deposit {
            Some(d) if self.r.w.goods[d].value != 0.0 => {
                let good = &self.r.w.goods[d];
                MAX_DEPOSIT_VALUE * good.deposit_yield / good.value
            }
            _ => f64::INFINITY,
        };
        BurgDay {
            b,
            m,
            workers: workers(place.population),
            inventory: vec![0.0; n],
            produced: vec![0.0; n],
            deposit,
            deposit_left,
            goals,
            tax_rate: super::taxes::sales_tax(self.r.w.states, place.state),
            ingredient_costs: 0.0,
            revenue: 0.0,
            workers_used: 0.0,
            spent: 0.0,
            settled: 0.0,
            outlets: QuoteCache::new(n),
            covered: [0.0; 5],
            consumed: vec![0.0; n],
            workers_by_good: vec![0.0; n],
            active: None,
            sources: QuoteCache::new(n),
            modifiers: vec![None; n],
        }
    }

    fn population(&self, d: &BurgDay) -> f64 {
        self.r.w.places[d.b].population
    }

    /// Spend up to `workers` of its remaining workers; false once none are
    /// left or nothing pays (`runWorkerLoop`).
    pub fn run_worker_loop(&mut self, d: &mut BurgDay, workers: f64) -> bool {
        let mut used = 0.0;
        d.sources.turn += 1;
        d.outlets.turn += 1;
        let mut runner_up = f64::INFINITY;
        while used < workers - 0.001 {
            let workers_left = d.workers - d.workers_used;
            let fraction = 1.0f64.min(workers_left).min(workers - used);
            if fraction <= 0.001 {
                return false;
            }
            let decision = match self.continue_goal(d, workers_left, fraction, runner_up) {
                Some(dec) => dec,
                None => {
                    let Some(dec) = self.decide(d, d.active, workers_left, fraction) else {
                        return false;
                    };
                    runner_up = dec.runner_up;
                    dec
                }
            };
            d.active = Some(decision.goal);
            let spent = self.execute(d, &decision, fraction);
            used += spent;
            d.workers_used += spent;
            // Sold at once, so the next worker sees the moved price.
            if decision.action.good == decision.goal {
                let goal = decision.goal;
                self.sell_inventory(d, &|g| g == goal);
            }
        }
        true
    }

    /// Make the decided good; the workers that it took
    /// (`executeManufacture`).
    fn execute(&mut self, d: &mut BurgDay, decision: &Decision, fraction: f64) -> f64 {
        let Action {
            good,
            ref ingredients,
            max_yield,
        } = decision.action;
        let modifier = self.burg_modifiers(d, good);
        let specialization = self.specialization(d, good);
        let output = modifier * specialization;
        let mut actual = fraction.min(max_yield);
        if ingredients.is_empty() {
            actual = actual.min(d.deposit_left / output);
        }
        let produced = rn(actual * output, 2);
        if produced == 0.0 {
            return fraction;
        }
        for &(ig, amount) in ingredients {
            let needed = actual * amount;
            let shortfall = needed - d.inventory[ig].min(needed);
            let from_market = (shortfall * 100.0 - 1e-6).ceil() / 100.0;
            if from_market <= 0.0 {
                continue;
            }
            let trade = self.acquire(d, ig, from_market, DealKind::Input, f64::INFINITY);
            if let Some(t) = trade {
                let cost = t.units * t.price;
                d.ingredient_costs += cost;
                self.r.treasury[d.b] -= cost;
                d.inventory[ig] += t.units;
            }
            // Short of an input, the step makes nothing: what it bought
            // waits in the inventory.
            if trade.is_none_or(|t| t.units < from_market) {
                return fraction;
            }
        }
        for &(ig, amount) in ingredients {
            let left = d.inventory[ig] - actual * amount;
            d.inventory[ig] = if left < 0.01 { 0.0 } else { left };
        }
        if ingredients.is_empty() {
            d.deposit_left -= produced;
        }
        d.inventory[good] += produced;
        d.produced[good] += produced;
        d.workers_by_good[good] += actual;
        self.ix.market_workers[d.m][good] += actual;
        self.ix.world_workers[good] += actual;
        actual
    }

    /// Buy at its own market while it stocks enough, else the cheapest
    /// other, else what its own has (`acquire`).
    pub fn acquire(
        &mut self,
        d: &mut BurgDay,
        g: usize,
        units: f64,
        kind: DealKind,
        budget: f64,
    ) -> Option<Transaction> {
        let market = self.supplier(d, g)?;
        if market != d.m {
            d.sources.quotes[g] = None;
        }
        d.outlets.quotes[g] = None;
        self.r.buy(d.b, g, units, kind, market, 0, budget)
    }

    /// The quote that `acquire` would buy at (`quote`).
    fn quote(&mut self, d: &mut BurgDay, g: usize) -> MarketQuote {
        let local = self.r.quote_market(d.m, g, d.b, 0);
        let supplier = self.supplier(d, g);
        if supplier.is_none_or(|s| s == d.m) {
            return MarketQuote {
                stock: if supplier.is_some() { local.stock } else { 0.0 },
                buy_price: local.buy_price,
                source: d.m,
                source_carriage: local.carriage,
            };
        }
        let source = self.source(d, g).expect("a source");
        MarketQuote {
            stock: source.stock,
            buy_price: source.buy_price,
            source: source.source,
            source_carriage: source.carriage,
        }
    }

    fn supplier(&mut self, d: &mut BurgDay, g: usize) -> Option<usize> {
        let b = d.b;
        let sources = &mut d.sources;
        self.r.supplier(b, g, 0, &mut |run| {
            let revision = run.revision(g);
            sources
                .get(g, revision, || run.quote_source(b, g, 0))
                .map(|q| q.source)
        })
    }

    /// The cheapest other market of the turn for the good (`getSource`).
    fn source(&mut self, d: &mut BurgDay, g: usize) -> Option<SourceQuote> {
        let revision = self.r.revision(g);
        let (b, r) = (d.b, &mut self.r);
        d.sources.get(g, revision, || r.quote_source(b, g, 0))
    }

    /// The market that pays most at the door of the place (`getOutlet`).
    fn outlet(&mut self, d: &mut BurgDay, g: usize) -> Option<Outlet> {
        let revision = self.r.revision(g);
        let (b, r) = (d.b, &mut self.r);
        d.outlets.get(g, revision, || r.outlet(b, g, 0))
    }

    /// The place sold only what it made, so only that can be kept
    /// (`quoteKeep`).
    fn quote_keep(&self, d: &BurgDay, g: usize) -> Option<(f64, f64)> {
        if d.produced[g] == 0.0 {
            return None;
        }
        self.r.quote_keep(d.b, g)
    }

    /// Sell the chosen goods of the inventory where each fetches most;
    /// `true` if any sold (`sellInventoryToMarket`).
    pub fn sell_inventory(&mut self, d: &mut BurgDay, sells: &dyn Fn(usize) -> bool) -> bool {
        let mut sold = false;
        let mut revenue = 0.0;
        for g in 0..d.inventory.len() {
            let units = d.inventory[g];
            if units <= 0.0 || !sells(g) {
                continue;
            }
            let market = self.outlet(d, g).map_or(d.m, |o| o.market);
            let Some(t) = self.r.sell(d.b, g, units, d.tax_rate, market, 0) else {
                continue;
            };
            sold = true;
            let left = units - t.units;
            d.inventory[g] = if left < 0.01 { 0.0 } else { left };
            d.outlets.quotes[g] = None;
            if market != d.m {
                d.sources.quotes[g] = None;
            }
            revenue += t.units * t.price - t.tax - t.tariff;
        }
        d.revenue += revenue;
        self.r.treasury[d.b] += revenue;
        sold
    }

    /// Every input down the chain of a goal (`getChainInputs`).
    pub fn chain_inputs(&mut self, goal: Option<usize>) -> Rc<Vec<bool>> {
        let n = self.goods();
        let Some(g) = goal else {
            return Rc::new(vec![false; n]);
        };
        if let Some(c) = &self.ix.chain_inputs[g] {
            return c.clone();
        }
        let mut inputs = vec![false; n];
        fn add(recipes: &[Vec<Recipe>], output: usize, inputs: &mut [bool]) {
            for r in &recipes[output] {
                for &(i, _) in &r.ingredients {
                    if inputs[i] {
                        continue;
                    }
                    inputs[i] = true;
                    add(recipes, i, inputs);
                }
            }
        }
        add(&self.ix.recipes, g, &mut inputs);
        let c = Rc::new(inputs);
        self.ix.chain_inputs[g] = Some(c.clone());
        c
    }

    /// Buy the share of its needs due so far, a poor place only the
    /// cheapest; the rest waits a turn (`fillDemand`).
    pub fn fill_demand(&mut self, d: &mut BurgDay, share: f64) {
        let targets = demand_targets(self.population(d)).map(|t| t * share);
        let poor = self.affluence(d) < share;
        if poor && self.r.treasury[d.b] <= 0.01 {
            return;
        }
        d.sources.turn += 1;
        for k in 0..5 {
            let shortage = (targets[k] - d.covered[k]).max(0.0);
            if shortage <= 0.001 {
                continue;
            }
            let mut sorted: Vec<(usize, f64, f64)> = Vec::new();
            let mut missing: Vec<(usize, f64, f64)> = Vec::new();
            let candidates = self.ix.demand_goods[k].clone();
            for &(g, weight) in &candidates {
                let own = self.quote_keep(d, g);
                let quote = self.quote(d, g);
                if own.is_none() && quote.stock <= 0.01 {
                    if poor {
                        continue;
                    }
                    let cost = self.r.quote_market(d.m, g, d.b, 0).buy_price / weight;
                    if cost > 0.0 {
                        missing.push((g, weight, cost));
                    }
                    continue;
                }
                let price = own.map_or(quote.buy_price, |o| o.1.min(quote.buy_price));
                sorted.push((g, weight, price / weight));
            }
            sorted.sort_by(|a, b| a.2.total_cmp(&b.2));
            let shortage_of = |d: &BurgDay| (targets[k] - d.covered[k]).max(0.0);
            let mut shortage = shortage;
            if !poor {
                let options: Vec<(usize, f64, f64)> =
                    sorted.iter().chain(missing.iter()).copied().collect();
                let wanted = shortage;
                let raw = self.shares(d, k, &options);
                let treasury = self.r.treasury[d.b];
                let n = sorted.len();
                let shares = lot_shares(&raw, &|j| j < n, &|j| wanted * options[j].2, treasury);
                for (j, &(g, weight, _)) in options.iter().enumerate() {
                    if j >= n {
                        self.r.signal_shortage(d.m, g, wanted * shares[j] / weight);
                    } else {
                        self.buy_need(d, g, weight, shortage.min(wanted * shares[j]));
                        shortage = shortage_of(d);
                    }
                }
            }
            for &(g, weight, _) in &sorted {
                if shortage <= 0.001 || self.r.treasury[d.b] <= 0.01 {
                    break;
                }
                self.buy_need(d, g, weight, shortage);
                shortage = shortage_of(d);
            }
        }
    }

    /// What the place sold to its own market is kept back first when that
    /// is cheaper than to buy it (`buy` of `fillDemand`).
    fn buy_need(&mut self, d: &mut BurgDay, g: usize, weight: f64, wanted: f64) {
        let budget = self.r.treasury[d.b];
        if budget <= 0.01 || wanted <= 0.001 {
            return;
        }
        let mut units = wanted / weight;
        if let Some(own) = self.quote_keep(d, g)
            && own.1 <= self.quote(d, g).buy_price
        {
            let kept = self.r.keep(d.b, g, units, budget);
            if let Some(k) = kept {
                d.revenue -= k.units * k.price;
                d.outlets.quotes[g] = None;
                units -= k.units;
                self.consume(d, g, &k);
            }
        }
        if units < 0.01 {
            return;
        }
        let treasury = self.r.treasury[d.b];
        if let Some(t) = self.acquire(d, g, units, DealKind::Demand, treasury) {
            self.consume(d, g, &t);
        }
    }

    /// The days of needs that a person earns (`getAffluence`).
    fn affluence(&self, d: &BurgDay) -> f64 {
        let income = self.r.treasury[d.b] + d.spent;
        let pop = self.population(d);
        let per_person = if pop != 0.0 { income / pop } else { 0.0 };
        if self.ix.needs_cost != 0.0 {
            per_person / self.ix.needs_cost
        } else {
            1.0
        }
    }

    /// The shares of the spending on a need by price per coverage; richer
    /// places pay more for goods that they lack (`getShares`).
    fn shares(&self, d: &BurgDay, k: usize, options: &[(usize, f64, f64)]) -> Vec<f64> {
        let bonus = NOVELTY * libm::log10(self.affluence(d).max(1.0));
        let covered = d.covered[k];
        let costs: Vec<f64> = options
            .iter()
            .map(|&(g, weight, cost)| {
                let had = if covered != 0.0 {
                    (d.consumed[g] * weight / covered).min(1.0)
                } else {
                    0.0
                };
                cost / (1.0 + bonus * (1.0 - had))
            })
            .collect();
        variety_shares(&costs, VARIETY)
    }

    /// Spend what the place can spare beyond its needs, up to 1 - savings
    /// of its income (`spendOnWants`).
    pub fn spend_on_wants(&mut self, d: &mut BurgDay) {
        let treasury = self.r.treasury[d.b];
        let income = treasury + d.spent;
        let budget = treasury.min((1.0 - SAVINGS_RATE) * income - d.spent);
        if budget <= 0.01 {
            return;
        }
        let targets = demand_targets(self.population(d));
        let affluence = self.affluence(d).max(1.0);
        let mut left = budget;
        for k in 0..5 {
            let mut money = if WEALTH_WANTS[k] {
                left
            } else {
                budget * WANT_SHARES[k]
            };
            let limit = WANT_LIMITS[k]
                * if WEALTH_WANTS[k] {
                    libm::sqrt(affluence)
                } else {
                    1.0
                };
            let cap = |d: &BurgDay| targets[k] * (1.0 + limit) - d.covered[k];
            if money <= 0.01 || cap(d) <= 0.001 {
                continue;
            }
            // (good, coverage, cost per coverage, stocked, price)
            let mut options: Vec<(usize, f64, f64, bool, f64)> = Vec::new();
            let candidates = self.ix.demand_goods[k].clone();
            for &(g, weight) in &candidates {
                let quote = self.quote(d, g);
                let stocked = quote.stock > 0.01;
                let price = if stocked {
                    quote.buy_price
                } else {
                    self.r.quote_market(d.m, g, d.b, 0).buy_price
                };
                if price > 0.0 {
                    options.push((g, weight, price / weight, stocked, price));
                }
            }
            if options.is_empty() {
                continue;
            }
            let total = money;
            let plain: Vec<(usize, f64, f64)> = options.iter().map(|o| (o.0, o.1, o.2)).collect();
            let raw = self.shares(d, k, &plain);
            let shares = lot_shares(&raw, &|j| options[j].3, &|_| total, total);
            for (j, &(g, weight, _, stocked, price)) in options.iter().enumerate() {
                if stocked {
                    let spend = money.min(total * shares[j]);
                    let cost = self.want_purchase(d, g, weight, price, spend, cap(d));
                    money -= cost;
                    left -= cost;
                } else {
                    let units = (total * shares[j] / price).min(cap(d) / weight);
                    self.r.signal_shortage(d.m, g, units);
                }
            }
            // What is left goes to the goods that the place favors most.
            let mut favored: Vec<(usize, f64, f64)> = options
                .iter()
                .enumerate()
                .filter(|&(j, o)| o.3 && shares[j] > 0.0)
                .map(|(j, o)| (o.0, o.1, shares[j]))
                .collect();
            favored.sort_by(|a, b| b.2.total_cmp(&a.2));
            for (g, weight, _) in favored {
                if money <= 0.01 || cap(d) <= 0.001 {
                    break;
                }
                let price = self.quote(d, g).buy_price;
                let cost = self.want_purchase(d, g, weight, price, money, cap(d));
                money -= cost;
                left -= cost;
            }
        }
    }

    /// A purchase for a want; the cost (`purchase` of `spendOnWants`).
    /// `cap` is the coverage that the place still wants.
    fn want_purchase(
        &mut self,
        d: &mut BurgDay,
        g: usize,
        weight: f64,
        price: f64,
        spend: f64,
        cap: f64,
    ) -> f64 {
        let units = (spend / price).min(cap / weight);
        if spend <= 0.01 || units < 0.01 {
            return 0.0;
        }
        match self.acquire(d, g, units, DealKind::Demand, spend) {
            Some(t) => self.consume(d, g, &t),
            None => 0.0,
        }
    }

    /// Pay for what the place bought for itself and count the needs that
    /// it covered; the cost (`consume`).
    fn consume(&mut self, d: &mut BurgDay, g: usize, t: &Transaction) -> f64 {
        let cost = t.units * t.price;
        self.r.treasury[d.b] -= cost;
        d.spent += cost;
        d.consumed[g] += t.units;
        let coverage = self.ix.coverage[g];
        for (k, c) in coverage.iter().enumerate() {
            d.covered[k] += t.units * c;
        }
        cost
    }

    fn worker_output(&mut self, d: &mut BurgDay, g: usize) -> f64 {
        self.burg_modifiers(d, g) * self.specialization(d, g)
    }

    fn working_deposit(d: &BurgDay, g: usize) -> bool {
        d.deposit == Some(g) && d.deposit_left >= 0.01
    }

    /// The modifiers of the good at the place, fixed for the run; a deposit
    /// gives more while it lasts (`getBurgModifiers`).
    fn burg_modifiers(&mut self, d: &mut BurgDay, g: usize) -> f64 {
        let base = match d.modifiers[g] {
            Some(v) => v,
            None => {
                let cell = self.r.w.places[d.b].cell;
                let v = self.r.w.land.modifiers(&self.r.w.goods[g], cell);
                d.modifiers[g] = Some(v);
                v
            }
        };
        base * if Self::working_deposit(d, g) {
            DEPOSIT_OUTPUT
        } else {
            1.0
        }
    }

    /// The division of labor (`getSpecialization`).
    fn specialization(&self, d: &BurgDay, g: usize) -> f64 {
        let own = if d.workers != 0.0 {
            d.workers_by_good[g] / d.workers
        } else {
            0.0
        };
        let workforce = self.ix.market_workforce[d.m];
        let market = if workforce != 0.0 {
            self.ix.market_workers[d.m][g] / workforce
        } else {
            0.0
        };
        let world = if self.ix.world_workforce != 0.0 {
            self.ix.world_workers[g] / self.ix.world_workforce
        } else {
            0.0
        };
        let excess = if world != 0.0 {
            (market / world - 1.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        1.0 + BURG_SPECIALIZATION * own.min(1.0) + MARKET_SPECIALIZATION * excess
    }

    /// The output value for each worker: the best price of the lot less the
    /// sales tax (`getSaleValue`).
    fn sale_value(&mut self, d: &mut BurgDay, g: usize, modifier: f64, workers: f64) -> f64 {
        let units = -modifier * workers;
        let outlet = self.outlet(d, g);
        let (sell, carriage) = self.r.quote_sale(d.m, g, d.b, 0);
        let mut price = self.r.at_lot(sell, carriage, d.m, g, units);
        // A lot never fetches more than a unit, so an outlet that pays no
        // more for a unit cannot win.
        if let Some(o) = outlet
            && o.market != d.m
            && o.price > price
        {
            price = price.max(self.r.at_lot(o.price, o.carriage, o.market, g, units));
        }
        price.max(0.0) * modifier * (1.0 - d.tax_rate)
    }

    /// The cost of the units at the door of the place (`getLotCost`).
    fn lot_cost(&self, q: &MarketQuote, g: usize, units: f64) -> f64 {
        if units <= 0.0 {
            return 0.0;
        }
        units
            * self
                .r
                .at_lot(q.buy_price, q.source_carriage, q.source, g, units)
    }

    fn quoted(
        &mut self,
        d: &mut BurgDay,
        quotes: &mut [Option<MarketQuote>],
        g: usize,
    ) -> MarketQuote {
        if let Some(q) = quotes[g] {
            return q;
        }
        let q = self.quote(d, g);
        quotes[g] = Some(q);
        q
    }

    /// The step that makes the recipe now from the stock at hand
    /// (`buildImmediateManufactureCandidate`): the action and its score and
    /// ingredient cost for each unit.
    #[allow(clippy::too_many_arguments)]
    fn immediate(
        &mut self,
        d: &mut BurgDay,
        recipe: &Recipe,
        units: f64,
        stock: Option<&Stock>,
        quotes: &mut [Option<MarketQuote>],
        sale_value: f64,
    ) -> Option<(Action, f64, f64)> {
        let mut max_yield = f64::INFINITY;
        if recipe.ingredients.is_empty() {
            max_yield = if d.deposit_left >= 0.01 {
                d.deposit_left / self.worker_output(d, recipe.good)
            } else {
                0.0
            };
        }
        for &(ig, amount) in &recipe.ingredients {
            let q = self.quoted(d, quotes, ig);
            let inventory = stock.map_or(d.inventory[ig], |s| s.inventory[ig]);
            let market = stock.and_then(|s| s.market[ig]).unwrap_or(q.stock);
            let total = inventory + market;
            if total < amount * units - 0.001 {
                return None;
            }
            max_yield = max_yield.min(total / amount);
        }
        if max_yield <= 0.0 {
            return None;
        }
        let actual = units.min(max_yield);
        let mut market_cost = 0.0;
        for &(ig, amount) in &recipe.ingredients {
            let q = self.quoted(d, quotes, ig);
            let inventory = stock.map_or(d.inventory[ig], |s| s.inventory[ig]);
            let needed = actual * amount;
            let from_market = (needed - inventory.min(needed)).max(0.0);
            market_cost += self.lot_cost(&q, ig, from_market);
        }
        let modifier = self.worker_output(d, recipe.good);
        let sell = if actual == units {
            sale_value
        } else {
            self.sale_value(d, recipe.good, modifier, actual)
        };
        let ingredient_cost = market_cost / actual;
        let score = sell - ingredient_cost;
        let action = Action {
            good: recipe.good,
            ingredients: recipe.ingredients.clone(),
            // A step that makes an input makes only what the chain lacks.
            max_yield: max_yield.min(units.max(0.01)),
        };
        Some((action, score, ingredient_cost))
    }

    /// Reserve the inputs of a recipe from the stock; what is missing and
    /// what the rest costs (`reserveRecipeInputs`).
    fn reserve(
        &mut self,
        d: &mut BurgDay,
        recipe: &Recipe,
        units: f64,
        stock: Option<&Stock>,
        quotes: &mut [Option<MarketQuote>],
    ) -> (Stock, Vec<(usize, f64)>, f64) {
        let n = self.goods();
        let mut reserved = Stock {
            inventory: stock.map_or_else(|| d.inventory.clone(), |s| s.inventory.clone()),
            market: stock.map_or_else(|| vec![None; n], |s| s.market.clone()),
        };
        let mut missing = Vec::new();
        let mut market_cost = 0.0;
        for &(ig, amount) in &recipe.ingredients {
            let q = self.quoted(d, quotes, ig);
            let inventory = reserved.inventory[ig];
            let market = reserved.market[ig].unwrap_or(q.stock);
            let from_inventory = (units * amount).min(inventory);
            let from_market = (units * amount - from_inventory).min(market);
            reserved.inventory[ig] = inventory - from_inventory;
            reserved.market[ig] = Some(market - from_market);
            market_cost += self.lot_cost(&q, ig, from_market);
            let remaining = units * amount - from_inventory - from_market;
            if remaining > 0.001 {
                missing.push((ig, remaining));
            }
        }
        (reserved, missing, market_cost)
    }

    /// The stock without the good of the deposit, to weigh working against
    /// buying; `None` if the recipe does not take it (`getRecipeStocks`).
    fn own_stock(
        &mut self,
        d: &mut BurgDay,
        recipe: &Recipe,
        stock: Option<&Stock>,
        quotes: &mut [Option<MarketQuote>],
    ) -> Option<Stock> {
        let dep = d.deposit?;
        if !recipe.ingredients.iter().any(|&(g, _)| g == dep) {
            return None;
        }
        let q = self.quoted(d, quotes, dep);
        if stock.and_then(|s| s.market[dep]).unwrap_or(q.stock) <= 0.001 {
            return None;
        }
        let n = self.goods();
        let mut own = Stock {
            inventory: stock.map_or_else(|| d.inventory.clone(), |s| s.inventory.clone()),
            market: stock.map_or_else(|| vec![None; n], |s| s.market.clone()),
        };
        own.market[dep] = Some(0.0);
        Some(own)
    }

    /// The best plan to make a good, with the inputs that it lacks made in
    /// steps of their own (`planGoodAction`). FMG also passes the step
    /// units, which only its debug log reads.
    #[allow(clippy::too_many_arguments)]
    fn plan(
        &mut self,
        d: &mut BurgDay,
        g: usize,
        target: f64,
        workers_left: f64,
        path: &mut [bool],
        stock: Option<&mut Stock>,
        quotes: &mut [Option<MarketQuote>],
        sale_value: Option<f64>,
    ) -> Option<Plan> {
        if workers_left <= 0.0 || target <= 0.0 || target > workers_left + 0.001 {
            return None;
        }
        if path[g] {
            return None;
        }
        // Working its deposit while that lasts, else the recipes of the
        // good (`getRecipes`).
        let all = self.ix.recipes.clone();
        let deposit;
        let recipes: &[Recipe] = if Self::working_deposit(d, g) {
            deposit = [Recipe {
                good: g,
                ingredients: Vec::new(),
            }];
            &deposit
        } else {
            &all[g]
        };
        if recipes.is_empty() {
            return None;
        }
        path[g] = true;
        let modifier = self.worker_output(d, g);
        let sell = match sale_value {
            Some(v) => v,
            None => self.sale_value(d, g, modifier, target),
        };
        let total_gain = sell * target;
        let has_stock = stock.is_some();
        let mut best: Option<Plan> = None;
        let mut best_stock: Option<Stock> = None;
        let better = |best: &Option<Plan>, gain: f64| {
            best.as_ref()
                .is_none_or(|b| gain > b.normalized_gain + 0.001)
        };
        for recipe in recipes {
            let base: Option<&Stock> = stock.as_deref();
            let own = self.own_stock(d, recipe, base, quotes);
            let variants: Vec<Option<&Stock>> = match &own {
                Some(o) => vec![base, Some(o)],
                None => vec![base],
            };
            for recipe_stock in variants {
                if let Some((action, score, ingredient_cost)) =
                    self.immediate(d, recipe, target, recipe_stock, quotes, sell)
                {
                    let plan = Plan {
                        workers_needed: target,
                        market_cost: ingredient_cost * target,
                        projected_gain: score * target,
                        normalized_gain: score,
                        action,
                    };
                    if better(&best, plan.normalized_gain) {
                        best = Some(plan);
                        best_stock = if has_stock {
                            Some(self.reserve(d, recipe, target, recipe_stock, quotes).0)
                        } else {
                            None
                        };
                    }
                    continue;
                }
                let mut workers_needed = target;
                let (mut reserved, missing, mut market_cost) =
                    self.reserve(d, recipe, target, recipe_stock, quotes);
                let mut feasible = true;
                let mut next: Option<Plan> = None;
                for (ig, amount) in missing {
                    let output = self.worker_output(d, ig);
                    let sub = self.plan(
                        d,
                        ig,
                        amount / output,
                        workers_left - workers_needed,
                        path,
                        Some(&mut reserved),
                        quotes,
                        None,
                    );
                    let Some(sub) = sub else {
                        feasible = false;
                        break;
                    };
                    workers_needed += sub.workers_needed;
                    market_cost += sub.market_cost;
                    if better(&next, sub.normalized_gain) {
                        next = Some(sub);
                    }
                }
                let Some(next) = next.filter(|_| feasible) else {
                    continue;
                };
                if workers_needed > workers_left + 0.001 {
                    continue;
                }
                let projected_gain = total_gain - market_cost;
                let normalized_gain = projected_gain / workers_needed;
                let plan = Plan {
                    workers_needed,
                    market_cost,
                    projected_gain,
                    normalized_gain,
                    action: next.action,
                };
                if better(&best, plan.normalized_gain) {
                    best = Some(plan);
                    best_stock = Some(reserved);
                }
            }
        }
        path[g] = false;
        if let (Some(s), Some(bs)) = (stock, best_stock) {
            *s = bs;
        }
        best
    }

    /// Plan the goals, the best paying first, and choose
    /// (`makeProductionDecision`).
    fn decide(
        &mut self,
        d: &mut BurgDay,
        active: Option<usize>,
        workers_left: f64,
        fraction: f64,
    ) -> Option<Decision> {
        let n = self.goods();
        let mut quotes: Vec<Option<MarketQuote>> = vec![None; n];
        let mut active_plan: Option<usize> = None;
        let mut plans: Vec<(usize, Plan)> = Vec::new();
        let mut unplanned = f64::NEG_INFINITY;
        let mut best = f64::NEG_INFINITY;
        for ranked in self.rank_goals(d, active, fraction) {
            // It and the rest cannot beat the choice.
            let beaten = unplanned > f64::NEG_INFINITY
                || (!plans.is_empty() && ranked.bound <= best + 0.001);
            if beaten {
                if unplanned == f64::NEG_INFINITY {
                    unplanned = ranked.bound;
                }
                break;
            }
            let mut path = vec![false; n];
            let plan = self.plan(
                d,
                ranked.good,
                ranked.step,
                workers_left,
                &mut path,
                None,
                &mut quotes,
                Some(ranked.sale_value),
            );
            let Some(plan) = plan.filter(|p| p.projected_gain > 0.0) else {
                continue;
            };
            if Some(ranked.good) == active {
                active_plan = Some(plans.len());
            }
            best = best.max(plan.normalized_gain);
            plans.push((ranked.good, plan));
        }
        let mut chosen: Option<usize> = None;
        for (k, (_, plan)) in plans.iter().enumerate() {
            if chosen.is_none_or(|c| plan.normalized_gain > plans[c].1.normalized_gain + 0.001) {
                chosen = Some(k);
            }
        }
        if let (Some(a), Some(c)) = (active_plan, chosen)
            && plans[a].1.normalized_gain >= plans[c].1.normalized_gain
        {
            chosen = Some(a);
        }
        let c = chosen?;
        let mut runner_up = unplanned;
        for (k, (_, plan)) in plans.iter().enumerate() {
            if k != c {
                runner_up = runner_up.max(plan.normalized_gain);
            }
        }
        let (goal, plan) = plans.swap_remove(c);
        Some(Decision {
            action: plan.action,
            goal,
            runner_up,
        })
    }

    /// Keep the goal while it beats the last runner-up (`continueGoal`).
    fn continue_goal(
        &mut self,
        d: &mut BurgDay,
        workers_left: f64,
        fraction: f64,
        runner_up: f64,
    ) -> Option<Decision> {
        let g = d.active?;
        if runner_up == f64::INFINITY {
            return None;
        }
        let modifier = self.worker_output(d, g);
        let step = self.step(d, g, modifier, fraction);
        let sale_value = self.sale_value(d, g, modifier, step);
        if sale_value <= runner_up + 0.001 {
            return None;
        }
        let n = self.goods();
        let mut path = vec![false; n];
        let mut quotes = vec![None; n];
        let plan = self.plan(
            d,
            g,
            step,
            workers_left,
            &mut path,
            None,
            &mut quotes,
            Some(sale_value),
        )?;
        if plan.projected_gain <= 0.0 || plan.normalized_gain <= runner_up + 0.001 {
            return None;
        }
        Some(Decision {
            action: plan.action,
            goal: g,
            runner_up,
        })
    }

    /// The goals that can pay, the active first, then the best paying
    /// (`rankGoals`).
    fn rank_goals(&mut self, d: &mut BurgDay, active: Option<usize>, fraction: f64) -> Vec<Ranked> {
        let mut goals = Vec::new();
        for g in d.goals.clone() {
            let modifier = self.worker_output(d, g);
            let step = self.step(d, g, modifier, fraction);
            let sale_value = self.sale_value(d, g, modifier, step);
            if Some(g) == active {
                goals.push(Ranked {
                    good: g,
                    bound: f64::INFINITY,
                    step,
                    sale_value,
                });
            } else if sale_value > 0.0 {
                goals.push(Ranked {
                    good: g,
                    bound: sale_value,
                    step,
                    sale_value,
                });
            }
        }
        goals.sort_by(|a, b| b.bound.total_cmp(&a.bound));
        goals
    }

    /// The workers to sell a share of the sales of a day (`getStep`).
    fn step(&mut self, d: &mut BurgDay, g: usize, modifier: f64, fraction: f64) -> f64 {
        let market = self.outlet(d, g).map_or(d.m, |o| o.market);
        let workers = LOT_DEPTH_SHARE * self.r.depth(market, g) / modifier;
        workers.min(fraction).max(MIN_STEP.min(fraction))
    }
}

/// Drop the buyable options with too small a lot; their money goes to the
/// rest (`getLotShares`).
fn lot_shares(
    shares: &[f64],
    buyable: &dyn Fn(usize) -> bool,
    cost: &dyn Fn(usize) -> f64,
    budget: f64,
) -> Vec<f64> {
    let mut kept: Vec<usize> = (0..shares.len())
        .filter(|&k| shares[k] > 0.0 && buyable(k))
        .collect();
    if kept.len() < 2 {
        return shares.to_vec();
    }
    let sum =
        |list: &[usize], value: &dyn Fn(usize) -> f64| list.iter().fold(0.0, |t, &k| t + value(k));
    let total = sum(&kept, &|k| shares[k]);
    let full = sum(&kept, &|k| shares[k] * cost(k));
    let affordable = if full > budget { budget / full } else { 1.0 };
    let least = if full.min(budget) < MIN_PURCHASE {
        MIN_SMALL_PURCHASE
    } else {
        MIN_PURCHASE
    };
    let lot = |k: usize, raised: f64| shares[k] * raised * cost(k) * affordable;
    loop {
        let raised = total / sum(&kept, &|k| shares[k]);
        let enough: Vec<usize> = kept
            .iter()
            .copied()
            .filter(|&k| lot(k, raised) >= least)
            .collect();
        if enough.len() == kept.len() {
            break;
        }
        if enough.is_empty() {
            // Too little for any lot: the largest.
            let mut best = kept[0];
            for &k in &kept {
                if lot(k, raised) > lot(best, raised) {
                    best = k;
                }
            }
            kept = vec![best];
            break;
        }
        kept = enough;
    }
    let raised = total / sum(&kept, &|k| shares[k]);
    (0..shares.len())
        .map(|k| {
            if buyable(k) {
                if kept.contains(&k) {
                    shares[k] * raised
                } else {
                    0.0
                }
            } else {
                shares[k]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::goods::Good;
    use crate::economy::trade::kit::Kit;

    const MARGIN: f64 = 0.04;

    /// A good with a value, carriage of a tenth of it, and recipes by id.
    fn good(id: u32, name: &'static str, value: f64, recipes: Vec<Vec<(u32, f64)>>) -> Good {
        let mut g = crate::economy::goods::catalogue()[0].clone();
        g.id = id;
        g.name = name;
        g.value = value;
        g.carriage = value / 10.0;
        g.recipes = recipes;
        g.coverage = [0.0; 5];
        g.culture_type = Vec::new();
        g.biome_output = Vec::new();
        g.development = 0;
        g
    }

    /// Oil, tool sets, and steel; an engine of 2 oil, 2 tools, and 10
    /// steel; a ship of an engine.
    fn catalogue() -> Vec<Good> {
        vec![
            good(1, "Oil", 1.0, vec![]),
            good(2, "Tool sets", 1.0, vec![]),
            good(3, "Steel", 1.0, vec![]),
            good(
                4,
                "Engine",
                100.0,
                vec![vec![(1, 2.0), (2, 2.0), (3, 10.0)]],
            ),
            good(5, "Ship", 10000.0, vec![vec![(4, 1.0)]]),
        ]
    }

    /// A run of one place at the center of one market, with every
    /// townsperson at work; `stock` of each good in the market.
    fn production<'a>(k: &'a Kit, stock: &[f64]) -> (Production<'a>, BurgDay) {
        let mut r = k.run(&[0], vec![vec![0.0]]);
        for (g, good) in k.goods.iter().enumerate() {
            let at = g;
            r.rows[at] = crate::economy::MarketGood {
                stock: stock[g],
                price: good.value,
            };
        }
        r.reindex();
        let p = Production {
            ix: Index::new(&k.goods, 1),
            r,
            orders: crate::economy::orders::Orders::default(),
        };
        let mut d = p.new_day(0, 0);
        d.workers = k.places[0].population;
        (p, d)
    }

    fn kit(goods: Vec<Good>, population: f64) -> Kit {
        let mut k = Kit::new(goods, vec![], &[0], &[0, 1]);
        k.places[0].population = population;
        k
    }

    const STOCK: [f64; 5] = [10000.0, 10000.0, 10000.0, 0.0, 0.0];

    /// Plan one unit of a good (by id) with a worker budget.
    fn plan(p: &mut Production, d: &mut BurgDay, id: usize, workers: f64) -> Option<Plan> {
        let n = p.goods();
        let mut path = vec![false; n];
        let mut quotes = vec![None; n];
        p.plan(d, id - 1, 1.0, workers, &mut path, None, &mut quotes, None)
    }

    #[test]
    fn an_engine_takes_one_step_and_a_ship_two() {
        let k = kit(catalogue(), 100.0);
        let (mut p, mut d) = production(&k, &STOCK);
        assert_eq!(plan(&mut p, &mut d, 4, 1.0).unwrap().workers_needed, 1.0);
        let ship = plan(&mut p, &mut d, 5, 2.0).unwrap();
        assert_eq!(ship.action.good, 3, "the first step makes the engine");
        assert_eq!(ship.workers_needed, 2.0);
        let gain = (10000.0 * (1.0 - MARGIN) - 14.0 * rn(1.0 + MARGIN, 2)) / 2.0;
        assert!((ship.normalized_gain - gain).abs() < 1e-6);
        assert_eq!(plan(&mut p, &mut d, 5, 100.0).unwrap().workers_needed, 2.0);
        // A chain whose work exceeds the budget is refused.
        assert!(plan(&mut p, &mut d, 5, 1.0).is_none());
    }

    #[test]
    fn two_workers_make_the_engine_then_the_ship_and_take_the_right_inputs() {
        let k = kit(catalogue(), 2.0);
        let (mut p, mut d) = production(&k, &STOCK);
        p.run_worker_loop(&mut d, 2.0);
        assert_eq!((d.produced[3], d.produced[4]), (1.0, 1.0));
        assert_eq!((d.inventory[3], d.inventory[4]), (0.0, 0.0));
        let sold =
            p.r.deals
                .iter()
                .find(|x| x.good == 5 && x.kind == DealKind::Sale)
                .unwrap();
        assert_eq!(sold.units, 1.0);
        let stock: Vec<f64> = (0..3).map(|g| p.r.row(0, g).stock).collect();
        assert_eq!(stock, vec![9998.0, 9998.0, 9990.0]);
    }

    #[test]
    fn the_inventory_goes_before_the_market() {
        let k = kit(catalogue(), 100.0);
        let (mut p, mut d) = production(&k, &STOCK);
        d.inventory = vec![2.0, 2.0, 10.0, 0.0, 0.0];
        let ship = plan(&mut p, &mut d, 5, 2.0).unwrap();
        assert_eq!(ship.workers_needed, 2.0);
        assert_eq!(ship.market_cost, 0.0);
        assert_eq!(d.inventory, vec![2.0, 2.0, 10.0, 0.0, 0.0]);
        assert_eq!(p.r.row(0, 0).stock, 10000.0);
    }

    #[test]
    fn a_stocked_engine_is_bought_not_made() {
        let k = kit(catalogue(), 100.0);
        let (mut p, mut d) = production(&k, &[10000.0, 10000.0, 10000.0, 1.0, 0.0]);
        let ship = plan(&mut p, &mut d, 5, 1.0).unwrap();
        assert_eq!(ship.action.good, 4);
        assert_eq!(ship.workers_needed, 1.0);
    }

    #[test]
    fn a_turn_spends_at_most_its_workers() {
        let k = kit(catalogue(), 12.0);
        let (mut p, mut d) = production(&k, &STOCK);
        assert!(p.run_worker_loop(&mut d, 5.0));
        assert!((d.workers_used - 5.0).abs() < 1e-9);
        assert!(p.run_worker_loop(&mut d, 5.0));
        assert!((d.workers_used - 10.0).abs() < 1e-9);
        assert!(!p.run_worker_loop(&mut d, 5.0));
        assert!((d.workers_used - 12.0).abs() < 1e-9);
    }

    #[test]
    fn missing_raw_inputs_stop_the_plan() {
        let k = kit(catalogue(), 100.0);
        let (mut p, mut d) = production(&k, &[1.0, 10000.0, 10000.0, 0.0, 0.0]);
        assert!(plan(&mut p, &mut d, 5, 100.0).is_none());
    }

    #[test]
    fn shared_inputs_are_reserved_across_the_parent_and_its_component() {
        let mut goods = catalogue();
        goods[4].recipes = vec![vec![(1, 1.0), (4, 1.0)]];
        let k = kit(goods.clone(), 100.0);
        let (mut p, mut d) = production(&k, &[2.0, 10000.0, 10000.0, 0.0, 0.0]);
        assert!(plan(&mut p, &mut d, 5, 100.0).is_none());
        let (mut p, mut d) = production(&k, &[3.0, 10000.0, 10000.0, 0.0, 0.0]);
        assert_eq!(plan(&mut p, &mut d, 5, 2.0).unwrap().workers_needed, 2.0);
    }

    #[test]
    fn a_cycle_needs_stock_or_another_recipe() {
        let mut goods = catalogue();
        goods[3].recipes = vec![vec![(5, 1.0)]];
        let k = kit(goods.clone(), 100.0);
        let (mut p, mut d) = production(&k, &STOCK);
        assert!(plan(&mut p, &mut d, 5, 100.0).is_none());
        // A stocked ship breaks the cycle.
        let (mut p, mut d) = production(&k, &[10000.0, 10000.0, 10000.0, 0.0, 1.0]);
        assert_eq!(plan(&mut p, &mut d, 5, 2.0).unwrap().workers_needed, 2.0);
        // So does a recipe that is not a cycle.
        goods[3].recipes.push(vec![(1, 2.0)]);
        let k = kit(goods, 100.0);
        let (mut p, mut d) = production(&k, &STOCK);
        assert_eq!(plan(&mut p, &mut d, 5, 2.0).unwrap().workers_needed, 2.0);
    }

    #[test]
    fn a_small_lot_gives_its_money_to_the_rest() {
        // Three goods with shares 0.6, 0.3, 0.1 of 5 coins: 0.5 coin for the
        // last is under the least lot, so the first two share its money.
        let shares = lot_shares(&[0.6, 0.3, 0.1], &|_| true, &|_| 5.0, 5.0);
        assert!((shares[0] - 0.6 / 0.9).abs() < 1e-12);
        assert!((shares[1] - 0.3 / 0.9).abs() < 1e-12);
        assert_eq!(shares[2], 0.0);
        // Too little money for any lot: the largest takes all.
        let shares = lot_shares(&[0.5, 0.5], &|_| true, &|_| 0.2, 0.2);
        assert_eq!(shares, vec![1.0, 0.0]);
        // An option that cannot be bought keeps its share.
        let shares = lot_shares(&[0.5, 0.4, 0.1], &|k| k != 2, &|_| 100.0, 100.0);
        assert_eq!(shares, vec![0.5, 0.4, 0.1]);
    }

    #[test]
    fn a_place_works_a_third_of_its_people() {
        assert_eq!(workers(10.0), 3.2);
        assert_eq!(workers(5000.0), MAX_WORKERS);
    }
}
