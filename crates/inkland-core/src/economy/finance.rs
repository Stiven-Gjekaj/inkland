//! The finance of a state (`StateFinance`, `TariffRule`, and `StateBudget`
//! of `states-generator.ts`, and `data/expenses.ts`).
//!
//! `states::forms` sets the rates, the fee, the tariffs, and the budget.
//! The production run sets what the state collected, spent, and pocketed.

/// What a state spends its taxes on (`EXPENSES`), in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Expense {
    Court,
    Administration,
    Infrastructure,
    Military,
}

impl Expense {
    /// All expenses in FMG order.
    pub const ALL: [Expense; 4] = [
        Expense::Court,
        Expense::Administration,
        Expense::Infrastructure,
        Expense::Military,
    ];

    /// The index of the expense in `ALL`.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The FMG name of the expense.
    pub fn name(self) -> &'static str {
        match self {
            Expense::Court => "court",
            Expense::Administration => "administration",
            Expense::Infrastructure => "infrastructure",
            Expense::Military => "military",
        }
    }
}

/// The rate of a tariff rule: a share of the price, or an embargo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tariff {
    Rate(f64),
    Embargo,
}

/// An import rule of a state (`TariffRule`). `None` means all states or all
/// goods.
#[derive(Debug, Clone, PartialEq)]
pub struct TariffRule {
    pub states: Option<Vec<u32>>,
    pub goods: Option<Vec<u32>>,
    pub rate: Tariff,
}

/// The budget of a state (`StateBudget`): each expense in whole percents of
/// the taxes collected, and the percent of each expense that officials
/// pocket. The expenses may sum past 100.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Budget {
    /// The expenses in `Expense` order.
    pub expenses: [f64; 4],
    pub corruption: f64,
}

impl Budget {
    /// The percent of one expense.
    pub fn of(&self, e: Expense) -> f64 {
        self.expenses[e.index()]
    }

    /// The sum of the expenses, in FMG order.
    pub fn total(&self) -> f64 {
        self.expenses.iter().fold(0.0, |s, v| s + v)
    }
}

/// The taxes, fee, tariffs, budget, and treasury of a state
/// (`StateFinance`). All zero for a state before its form and for the
/// neutral lands. inkland has no user adjustments, so they are not here:
/// FMG adds them as zero lines on a new map.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StateFinance {
    /// The sales tax rate, a share of the price.
    pub sales_tax: f64,
    /// The poll tax a day for each population point.
    pub poll_tax: f64,
    /// The license fee a day from each market centered in the state.
    pub market_fee: f64,
    pub budget: Budget,
    /// All taxes collected in the run.
    pub collected: f64,
    /// What each expense bought in the run, in `Expense` order.
    pub spent: [f64; 4],
    /// What officials pocketed in the run.
    pub pocketed: f64,
    pub treasury: f64,
    pub tariffs: Vec<TariffRule>,
}

impl StateFinance {
    /// What the state spent on one expense.
    pub fn spent_on(&self, e: Expense) -> f64 {
        self.spent[e.index()]
    }
}
/// The errors of the form of a finance entry: one reference, a known pin,
/// rates in range, and known goods.
pub fn check(
    config: &crate::config::Config,
    goods: &[crate::economy::goods::Good],
    i: usize,
    e: &crate::config::FinanceEntry,
) -> Vec<crate::error::Error> {
    use crate::error::{Error, ErrorCode};
    let path = format!("/world/finance/{i}");
    let mut out = Vec::new();
    if e.at.is_some() == e.pin.is_some() {
        out.push(Error::new(
            ErrorCode::FinanceInvalid,
            path.clone(),
            "a finance entry needs exactly one of at and pin",
        ));
    }
    let pins = |ids: &[String], at: &str, out: &mut Vec<Error>| {
        for id in ids {
            if !config.pins.points.iter().any(|p| &p.id == id) {
                out.push(Error::new(
                    ErrorCode::RefUnknown,
                    at.to_owned(),
                    format!("no point pin has the id \"{id}\""),
                ));
            }
        }
    };
    if let Some(id) = &e.pin {
        pins(std::slice::from_ref(id), &format!("{path}/pin"), &mut out);
    }
    let mut range = |field: String, v: Option<f64>, ok: fn(f64) -> bool, rule: &str| {
        if let Some(v) = v
            && !(v.is_finite() && ok(v))
        {
            out.push(Error::new(
                ErrorCode::ValueOutOfRange,
                field,
                format!("the value must be {rule}, found {v}"),
            ));
        }
    };
    range(
        format!("{path}/sales_tax"),
        e.sales_tax,
        |v| (0.0..=1.0).contains(&v),
        "0 to 1",
    );
    range(
        format!("{path}/poll_tax"),
        e.poll_tax,
        |v| v >= 0.0,
        "0 or more",
    );
    range(
        format!("{path}/market_fee"),
        e.market_fee,
        |v| v >= 0.0,
        "0 or more",
    );
    if let Some(b) = &e.budget {
        for (name, v) in [
            ("court", b.court),
            ("administration", b.administration),
            ("infrastructure", b.infrastructure),
            ("military", b.military),
        ] {
            range(
                format!("{path}/budget/{name}"),
                v,
                |v| v >= 0.0,
                "0 or more",
            );
        }
        range(
            format!("{path}/budget/corruption"),
            b.corruption,
            |v| (0.0..=100.0).contains(&v),
            "0 to 100",
        );
    }
    for (k, t) in e.tariffs.iter().flatten().enumerate() {
        let at = format!("{path}/tariffs/{k}");
        match &t.rate {
            crate::config::TariffRate::Rate(v) if !(v.is_finite() && *v >= 0.0) => {
                out.push(Error::new(
                    ErrorCode::ValueOutOfRange,
                    format!("{at}/rate"),
                    format!("a tariff rate must be 0 or more, found {v}"),
                ))
            }
            crate::config::TariffRate::Word(w) if w != "embargo" => out.push(Error::new(
                ErrorCode::FinanceInvalid,
                format!("{at}/rate"),
                format!("a tariff rate is a number or embargo, found \"{w}\""),
            )),
            _ => {}
        }
        if let Some(ids) = &t.states {
            pins(ids, &format!("{at}/states"), &mut out);
        }
        for name in t.goods.iter().flatten() {
            if !goods.iter().any(|g| &g.name == name) {
                out.push(Error::new(
                    ErrorCode::RefUnknown,
                    format!("{at}/goods"),
                    format!("no good has the name \"{name}\""),
                ));
            }
        }
    }
    out
}

/// Apply the finance entries of the config to the states, in order.
pub fn apply_config(
    config: &crate::config::Config,
    grid: &crate::grid::Grid,
    goods: &[crate::economy::goods::Good],
    soc: &mut crate::society::Society,
) -> Result<(), crate::error::Error> {
    use crate::error::{Error, ErrorCode};
    let state_of =
        |soc: &crate::society::Society, p: [f64; 2]| soc.cell_state[grid.find_cell(p[0], p[1])];
    let pin_at = |id: &str| config.pins.points.iter().find(|p| p.id == id).map(|p| p.at);
    for (i, e) in config.world.finance.iter().enumerate() {
        if let Some(err) = check(config, goods, i, e).into_iter().next() {
            return Err(err);
        }
        let path = format!("/world/finance/{i}");
        let p =
            e.at.or_else(|| e.pin.as_deref().and_then(pin_at))
                .expect("check gives one reference");
        let id = state_of(soc, p);
        let Some(k) = soc.states.iter().position(|s| s.id == id && id != 0) else {
            return Err(Error::new(
                ErrorCode::FinanceInvalid,
                path,
                format!("no state holds the point ({}, {})", p[0], p[1]),
            ));
        };
        let tariffs = e.tariffs.as_ref().map(|list| {
            list.iter()
                .map(|t| TariffRule {
                    states: t.states.as_ref().map(|ids| {
                        ids.iter()
                            .filter_map(|id| pin_at(id))
                            .map(|p| state_of(soc, p))
                            .collect()
                    }),
                    goods: t.goods.as_ref().map(|names| {
                        names
                            .iter()
                            .filter_map(|n| goods.iter().find(|g| &g.name == n).map(|g| g.id))
                            .collect()
                    }),
                    rate: match t.rate {
                        crate::config::TariffRate::Rate(v) => Tariff::Rate(v),
                        crate::config::TariffRate::Word(_) => Tariff::Embargo,
                    },
                })
                .collect::<Vec<_>>()
        });
        let f = &mut soc.states[k].economy;
        if let Some(v) = e.sales_tax {
            f.sales_tax = v;
        }
        if let Some(v) = e.poll_tax {
            f.poll_tax = v;
        }
        if let Some(v) = e.market_fee {
            f.market_fee = v;
        }
        if let Some(b) = &e.budget {
            for (expense, v) in [
                (Expense::Court, b.court),
                (Expense::Administration, b.administration),
                (Expense::Infrastructure, b.infrastructure),
                (Expense::Military, b.military),
            ] {
                if let Some(v) = v {
                    f.budget.expenses[expense.index()] = v;
                }
            }
            if let Some(v) = b.corruption {
                f.budget.corruption = v;
            }
        }
        if let Some(t) = tariffs {
            f.tariffs = t;
        }
    }
    Ok(())
}

#[cfg(test)]
mod config_tests {
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    fn build(finance: serde_json::Value) -> Result<World, crate::error::Error> {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 5, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea", "finance": finance },
            "pins": { "points": [
                { "id": "home", "kind": "city", "at": [200, 150], "capital_of": "Realm" }
            ] }
        }))
        .unwrap();
        World::build(&c)
    }

    #[test]
    fn a_finance_entry_sets_the_rates_budget_and_tariffs_of_its_state() {
        let base = build(json!([])).unwrap();
        let w = build(json!([{
            "pin": "home", "sales_tax": 0.3, "poll_tax": 0.0,
            "budget": { "military": 80, "corruption": 0 },
            "tariffs": [{ "goods": ["Wood"], "rate": "embargo" }, { "rate": 0.2 }]
        }]))
        .unwrap();
        let realm = |w: &World| {
            let cell = w.grid.find_cell(200.0, 150.0);
            w.society
                .states
                .iter()
                .find(|s| s.id == w.society.cell_state[cell])
                .unwrap()
                .clone()
        };
        let (old, new) = (realm(&base), realm(&w));
        assert_eq!(new.economy.sales_tax, 0.3);
        assert_eq!(new.economy.poll_tax, 0.0);
        assert_eq!(new.economy.budget.of(super::Expense::Military), 80.0);
        assert_eq!(new.economy.budget.corruption, 0.0);
        assert_eq!(
            new.economy.budget.of(super::Expense::Court),
            old.economy.budget.of(super::Expense::Court)
        );
        assert_eq!(new.economy.tariffs.len(), 2);
        assert_eq!(new.economy.tariffs[0].goods, Some(vec![1]));
        assert_eq!(new.economy.tariffs[0].rate, super::Tariff::Embargo);
        // The run uses the new rates.
        assert_ne!(new.economy.collected, old.economy.collected);
        for (finance, path) in [
            (json!([{ "sales_tax": 0.3 }]), "/world/finance/0"),
            (
                json!([{ "at": [1, 1], "sales_tax": 0.3 }]),
                "/world/finance/0",
            ),
            (
                json!([{ "pin": "home", "sales_tax": 3 }]),
                "/world/finance/0/sales_tax",
            ),
            (
                json!([{ "pin": "home", "tariffs": [{ "rate": "free" }] }]),
                "/world/finance/0/tariffs/0/rate",
            ),
            (
                json!([{ "pin": "home", "tariffs": [{ "goods": ["Nothing"], "rate": 1 }] }]),
                "/world/finance/0/tariffs/0/goods",
            ),
        ] {
            let e = build(finance).unwrap_err();
            assert_eq!(e.path, path, "{}", e.message);
        }
    }
}
