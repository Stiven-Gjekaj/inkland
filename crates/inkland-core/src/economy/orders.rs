//! State orders (`state-orders.ts`): states spend their budget during the
//! run, so state demand pulls production.
//!
//! - The court and the army buy at the capital; administration and
//!   infrastructure at the capital of each province, by its people.
//! - After each round, each buyer spends its share of what the state has
//!   collected so far, less what officials pocket, once it has gathered
//!   `MIN_ORDER`. The last orders of the day spend the rest.
//! - A state pays no sales tax or tariff to itself.

use super::corruption::{Corruption, ProvinceCorruption, curb, tendency};
use super::deals::DealKind;
use super::demand::{Need, VARIETY, variety_shares};
use super::finance::Expense;
use super::production::Production;
use crate::rand::rn;
use crate::society::State;

/// The needs that each expense buys for, with the share of its money.
fn expense_needs(e: Expense) -> &'static [(Need, f64)] {
    match e {
        Expense::Court => &[(Need::Luxury, 1.0)],
        Expense::Administration => &[(Need::Utilities, 1.0)],
        Expense::Infrastructure => &[(Need::Construction, 1.0)],
        Expense::Military => &[(Need::Military, 0.6), (Need::Food, 0.4)],
    }
}
/// The money that a buyer gathers before it orders.
const MIN_ORDER: f64 = 1.0;
/// The spending a person a day that serves the people of the state fully:
/// infrastructure and court (`PROVISION_NEED`).
pub const PROVISION_NEED_INFRASTRUCTURE: f64 = 0.05;
pub const PROVISION_NEED_COURT: f64 = 0.08;

/// A place where an expense buys, with its share of the expense and the
/// corruption of its province before administration (`Buyer`).
#[derive(Debug, Clone)]
struct Buyer {
    b: usize,
    share: f64,
    corruption: f64,
    spent: f64,
}

/// The run of one state (`StateRun`).
#[derive(Debug, Clone)]
struct StateRun {
    state: usize,
    /// The money known at dawn: the poll tax its places owe and its market
    /// fees.
    dawn: f64,
    rural_poll_tax: f64,
    buyers: [Vec<Buyer>; 4],
}

/// The state orders of the run.
#[derive(Debug, Clone, Default)]
pub struct Orders {
    runs: Vec<StateRun>,
    /// The goods that cover each need.
    goods_for: Vec<Vec<usize>>,
}

/// The corruption of a province before administration, or of the state
/// without provinces.
fn tendency_of(state: &State, province: Option<&ProvinceCorruption>) -> f64 {
    province.map_or(state.economy.budget.corruption, |p| tendency(state, p))
}

impl Production<'_> {
    /// Start the orders, once the places owe their poll tax: what each state
    /// has at dawn and where it buys (`start`).
    pub fn start_orders(&mut self, corruption: &Corruption) {
        let w = &self.r.w;
        let n_states = w.states.len() + 1;
        let mut fees = vec![0.0; n_states];
        for (m, &fee) in self.r.fee_paid.iter().enumerate() {
            if fee != 0.0 {
                fees[w.market_state[m] as usize] += fee;
            }
        }
        let mut dawn = vec![0.0; n_states];
        for (b, p) in w.places.iter().enumerate() {
            let paid = self.r.poll_tax_paid[b];
            if paid != 0.0 {
                dawn[p.state as usize] += paid;
            }
        }
        let goods = w.goods;
        let goods_for: Vec<Vec<usize>> = Need::ALL
            .iter()
            .map(|n| {
                (0..goods.len())
                    .filter(|&g| goods[g].covers(*n) > 0.0)
                    .collect()
            })
            .collect();
        let mut runs = Vec::new();
        for (k, state) in w.states.iter().enumerate() {
            let capital = (state.capital as usize).checked_sub(1);
            let at_capital = capital.filter(|&c| w.market_of(c).is_some());
            let capital_cell = capital.map(|c| w.places[c].cell);
            let in_capital = || -> Vec<Buyer> {
                at_capital.map_or_else(Vec::new, |b| {
                    vec![Buyer {
                        b,
                        share: 1.0,
                        corruption: tendency_of(state, corruption.of_capital(capital_cell)),
                        spent: 0.0,
                    }]
                })
            };
            // By province; one whose capital has no market buys at the
            // capital of the state.
            let in_provinces = || -> Vec<Buyer> {
                corruption
                    .provinces(state.id)
                    .iter()
                    .filter_map(|p| {
                        let seat = (p.burg as usize)
                            .checked_sub(1)
                            .filter(|&b| w.market_of(b).is_some());
                        seat.or(at_capital).map(|b| Buyer {
                            b,
                            share: p.share,
                            corruption: tendency_of(state, Some(p)),
                            spent: 0.0,
                        })
                    })
                    .collect()
            };
            runs.push(StateRun {
                state: k,
                dawn: dawn[state.id as usize] + fees[state.id as usize],
                rural_poll_tax: 0.0,
                buyers: [in_capital(), in_provinces(), in_provinces(), in_capital()],
            });
        }
        self.orders = Orders { runs, goods_for };
    }

    /// Each buyer spends the share of its expense of the taxes so far; the
    /// last turn spends the rest (`place`).
    pub fn place_orders(&mut self, last: bool) {
        let rural = if last {
            self.r.settle_rural_poll_tax()
        } else {
            Vec::new()
        };
        let least = if last { 0.01 } else { MIN_ORDER };
        let mut runs = std::mem::take(&mut self.orders.runs);
        let states = self.r.w.states;
        for run in &mut runs {
            let state = &states[run.state];
            if last {
                run.rural_poll_tax = rural.get(state.id as usize).copied().unwrap_or(0.0);
            }
            let collected = run.dawn + run.rural_poll_tax + self.r.taxes_so_far(state.id);
            for e in Expense::ALL {
                let percent = state.economy.budget.of(e);
                for k in 0..run.buyers[e.index()].len() {
                    let buyer = &run.buyers[e.index()][k];
                    // Officials pocket it of the allowance of the buyer.
                    let corruption = curb(state, collected, buyer.corruption);
                    let money = collected * percent * buyer.share * (100.0 - corruption) / 10000.0
                        - buyer.spent;
                    if money < least {
                        continue;
                    }
                    let (b, id) = (buyer.b, state.id);
                    for &(need, part) in expense_needs(e) {
                        let spent = self.order(b, need, money * part, id, !last);
                        run.buyers[e.index()][k].spent += spent;
                    }
                }
            }
        }
        self.orders.runs = runs;
    }

    /// End the orders: each state keeps what it spent and collected
    /// (`end`). The spent and collected amounts by state index.
    pub fn end_orders(&mut self) -> Vec<([f64; 4], f64)> {
        let mut out = vec![([0.0; 4], 0.0); self.r.w.states.len()];
        for run in &self.orders.runs {
            let mut spent = [0.0; 4];
            for e in Expense::ALL {
                spent[e.index()] = rn(
                    run.buyers[e.index()].iter().fold(0.0, |t, b| t + b.spent),
                    2,
                );
            }
            let id = self.r.w.states[run.state].id;
            let collected = run.dawn + run.rural_poll_tax + self.r.taxes_so_far(id);
            out[run.state] = (spent, rn(collected, 2));
        }
        self.orders.runs.clear();
        out
    }

    /// Spend on the goods of the need at the market of the buyer or the
    /// cheapest other; what it spent (`order`).
    fn order(&mut self, b: usize, need: Need, money: f64, exempt: u32, signal: bool) -> f64 {
        let Some(market) = self.r.w.market_of(b) else {
            return 0.0;
        };
        let k = need.index();
        let goods = self.r.w.goods;
        let options: Vec<(usize, f64)> = self.orders.goods_for[k]
            .iter()
            .map(|&g| {
                (
                    g,
                    self.r
                        .customer_buy_price(market, self.r.row(market, g).price),
                )
            })
            .collect();
        let cost = |&(g, price): &(usize, f64)| price / goods[g].coverage[k];
        let shares = variety_shares(&options.iter().map(cost).collect::<Vec<_>>(), VARIETY);
        let mut spent = 0.0;
        // By good: where the buyer found it, kept for the rest of the order.
        let mut sources: Vec<Option<Option<usize>>> = vec![None; goods.len()];
        let mut buy = |p: &mut Self, g: usize, budget: f64| -> f64 {
            if budget < 0.01 {
                return 0.0;
            }
            let source = *sources[g].get_or_insert_with(|| {
                p.r.supplier(b, g, exempt, &mut |run| {
                    run.quote_source(b, g, exempt).map(|q| q.source)
                })
            });
            let Some(source) = source else {
                return 0.0;
            };
            if p.r.row(source, g).stock <= 0.01 {
                return 0.0;
            }
            p.r.buy(b, g, f64::INFINITY, DealKind::Order, source, exempt, budget)
                .map_or(0.0, |t| t.units * t.price)
        };
        for (j, &(g, price)) in options.iter().enumerate() {
            let spend = money * shares[j];
            let cost = buy(self, g, spend);
            if signal && cost < spend - 0.01 {
                self.r.signal_shortage(market, g, (spend - cost) / price);
            }
            spent += cost;
        }
        let mut sorted = options.clone();
        sorted.sort_by(|a, b| cost(a).total_cmp(&cost(b)));
        for &(g, _) in &sorted {
            if money - spent < 0.01 {
                break;
            }
            spent += buy(self, g, money - spent);
        }
        spent
    }
}

/// How fully the spending of the run on infrastructure or the court served
/// the people of the state, 0 to 1 (`getProvision`).
pub fn provision(state: &State, expense: Expense) -> f64 {
    let need = match expense {
        Expense::Infrastructure => PROVISION_NEED_INFRASTRUCTURE,
        Expense::Court => PROVISION_NEED_COURT,
        _ => return 0.0,
    } * (state.urban + state.rural);
    if need == 0.0 {
        0.0
    } else {
        (state.economy.spent_on(expense) / need).min(1.0)
    }
}

/// How fully the infrastructure spending of the run served each province of
/// a state, 0 to 1, by province id (`getInfrastructure`).
pub fn infrastructure(corruption: &Corruption, state: &State) -> Vec<(u32, f64)> {
    let provinces = corruption.provinces(state.id);
    let collected = state.economy.collected;
    let kept: Vec<f64> = provinces
        .iter()
        .map(|d| d.share * (1.0 - corruption.of_province(state, d, collected) / 100.0))
        .collect();
    let total = kept.iter().fold(0.0, |t, v| t + v);
    let spent = state.economy.spent_on(Expense::Infrastructure);
    provinces
        .iter()
        .zip(&kept)
        .map(|(d, &k)| {
            let need = PROVISION_NEED_INFRASTRUCTURE * d.people;
            let served = if need != 0.0 && total != 0.0 {
                (spent * k / total / need).min(1.0)
            } else {
                0.0
            };
            (d.id, served)
        })
        .collect()
}
