//! The stability of the states and their provinces
//! (`model/political/stability.ts`).
//!
//! FMG works it out on demand from the last economy run and the military.
//! inkland works it out once, after the military, and keeps it in
//! `World::stability`: the zones and the overview read it.

use crate::economy::Economy;
use crate::economy::corruption::ProvinceCorruption;
use crate::economy::finance::Expense;
use crate::society::{NONE, Society};
use std::collections::BTreeMap;

const BASE: f64 = 0.5;
/// Added at `PROSPERITY_TOP` prosperity or more.
const PROSPERITY: f64 = 0.5;
const PROSPERITY_TOP: f64 = 0.5;
/// Added by a fully provided court and clergy.
const COURT: f64 = 0.25;
/// An army of `ARMY_SHARE` of the people or more: added when fully kept,
/// taken when not kept at all.
const ARMY: f64 = 0.15;
const ARMY_SHARE: f64 = 0.005;
/// Taken from a fully remote province.
const REMOTENESS: f64 = 0.15;
/// Taken from a province of strangers only.
const DIVERSITY: f64 = 0.2;
/// Taken at `CORRUPTION_TOP` percent or more.
const CORRUPTION: f64 = 0.2;
const CORRUPTION_TOP: f64 = 40.0;
/// Taken when the taxes are `BURDEN_TOP` of the product or more.
const BURDEN: f64 = 0.15;
const BURDEN_TOP: f64 = 0.5;

/// The parts of stability (`STABILITY_PARTS`), in FMG order.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Parts {
    pub base: f64,
    pub prosperity: f64,
    pub court: f64,
    pub army: f64,
    pub remoteness: f64,
    pub diversity: f64,
    pub corruption: f64,
    pub burden: f64,
}

impl Parts {
    /// The FMG names of the parts, in order.
    pub const NAMES: [&'static str; 8] = [
        "base",
        "prosperity",
        "court",
        "army",
        "remoteness",
        "diversity",
        "corruption",
        "burden",
    ];

    /// The parts in FMG order.
    pub fn values(&self) -> [f64; 8] {
        [
            self.base,
            self.prosperity,
            self.court,
            self.army,
            self.remoteness,
            self.diversity,
            self.corruption,
            self.burden,
        ]
    }

    /// The sum of the parts, from 0 to 1.
    pub fn stability(&self) -> f64 {
        self.values().iter().fold(0.0, |s, v| s + v).clamp(0.0, 1.0)
    }
}

/// The stability of a province, or of the cells of a state outside any
/// province (`ProvinceStability`).
#[derive(Debug, Clone, PartialEq)]
pub struct ProvinceStability {
    /// The corruption data of the province: its id (0 outside any
    /// province), share, remoteness, and diversity.
    pub province: ProvinceCorruption,
    /// From 0 (falling apart) to 1 (unshakable).
    pub stability: f64,
    pub parts: Parts,
}

/// The stability of a state (`StateStability`).
#[derive(Debug, Clone, PartialEq)]
pub struct StateStability {
    pub stability: f64,
    /// The parts of its provinces, weighed by their share of the people.
    pub parts: Parts,
}

/// The stability of all states.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stability {
    /// By state id; entry 0 (the neutral lands) is `None`, as is a state
    /// without accounts or provinces with people.
    pub states: Vec<Option<StateStability>>,
    /// The provinces of each state, by state id, in the order of
    /// `Corruption::provinces`.
    pub provinces: Vec<Vec<ProvinceStability>>,
}

impl Stability {
    /// The stability of a state (`get`).
    pub fn state(&self, id: u32) -> Option<&StateStability> {
        self.states.get(id as usize).and_then(Option::as_ref)
    }

    /// The provinces of a state (`getProvinces`).
    pub fn provinces(&self, state: u32) -> &[ProvinceStability] {
        self.provinces
            .get(state as usize)
            .map_or(&[], Vec::as_slice)
    }

    /// The stability of a province by id (`getProvince`).
    pub fn province(&self, soc: &Society, id: u32) -> Option<&ProvinceStability> {
        if id == NONE {
            return None;
        }
        let province = soc.provinces.get(id as usize - 1)?;
        self.provinces(province.state)
            .iter()
            .find(|d| d.province.id == id)
    }
}

/// The prosperity of the people of each province, urban and rural, by
/// their numbers (`index`). The key is the state id and the index in its
/// province list.
fn province_prosperity(soc: &Society, e: &Economy) -> BTreeMap<(u32, usize), f64> {
    let mut sums: BTreeMap<(u32, usize), (f64, f64)> = BTreeMap::new();
    let mut add = |cell: usize, people: f64, prosperity: Option<f64>| {
        let Some(key) = e.corruption.of_cell.get(cell).copied().flatten() else {
            return;
        };
        let Some(prosperity) = prosperity else {
            return;
        };
        if people == 0.0 {
            return;
        }
        let sum = sums.entry(key).or_insert((0.0, 0.0));
        sum.0 += people;
        sum.1 += people * prosperity;
    };
    for (k, p) in soc.places.iter().enumerate() {
        let living = e.burg_living.get(k).and_then(Option::as_ref);
        add(p.cell, p.population, living.map(|l| l.prosperity));
    }
    for (c, &pop) in soc.cells.pop.iter().enumerate() {
        let rural = e.rural.get(c).and_then(Option::as_ref);
        add(c, pop, rural.map(|r| r.living.prosperity));
    }
    sums.into_iter()
        .map(|(key, (people, total))| (key, total / people))
        .collect()
}

/// Work out the stability of all states.
pub fn build(soc: &Society, e: &Economy) -> Stability {
    let by_province = province_prosperity(soc, e);
    let mut out = Stability {
        states: vec![None; soc.states.len() + 1],
        provinces: vec![Vec::new(); soc.states.len() + 1],
    };
    for state in &soc.states {
        let id = state.id as usize;
        let Some(accounts) = e.accounts.states.get(id) else {
            continue;
        };
        let Some(state_prosperity) = accounts.people.prosperity else {
            continue;
        };
        let points = state.rural + state.urban * crate::military::URBANIZATION_RATE;
        let troops = state.military.iter().fold(0.0, |s, r| s + r.total);
        let army = if points != 0.0 {
            (troops / points / ARMY_SHARE).min(1.0)
        } else {
            0.0
        };
        let product = accounts.people.product;
        let burden = if product > 0.0 {
            accounts.taxes / product / BURDEN_TOP
        } else {
            0.0
        };
        let court = crate::economy::orders::provision(state, Expense::Court);
        let upkeep = crate::military::upkeep(state, e);
        let provinces: Vec<ProvinceStability> = e
            .corruption
            .provinces(state.id)
            .iter()
            .enumerate()
            .map(|(k, province)| {
                let prosperity = by_province
                    .get(&(state.id, k))
                    .copied()
                    .unwrap_or(state_prosperity);
                let corruption = e
                    .corruption
                    .of_province(state, province, state.economy.collected);
                let parts = Parts {
                    base: BASE,
                    prosperity: PROSPERITY * (prosperity / PROSPERITY_TOP).min(1.0),
                    court: COURT * court,
                    army: ARMY * army * (2.0 * upkeep - 1.0),
                    remoteness: -REMOTENESS * province.remoteness,
                    diversity: -DIVERSITY * province.diversity,
                    corruption: -CORRUPTION * (corruption / CORRUPTION_TOP).min(1.0),
                    burden: -BURDEN * burden.min(1.0),
                };
                ProvinceStability {
                    province: province.clone(),
                    stability: parts.stability(),
                    parts,
                }
            })
            .collect();
        if !provinces.is_empty() {
            let weigh = |f: fn(&Parts) -> f64| {
                provinces
                    .iter()
                    .fold(0.0, |s, d| s + d.province.share * f(&d.parts))
            };
            let parts = Parts {
                base: weigh(|p| p.base),
                prosperity: weigh(|p| p.prosperity),
                court: weigh(|p| p.court),
                army: weigh(|p| p.army),
                remoteness: weigh(|p| p.remoteness),
                diversity: weigh(|p| p.diversity),
                corruption: weigh(|p| p.corruption),
                burden: weigh(|p| p.burden),
            };
            out.states[id] = Some(StateStability {
                stability: parts.stability(),
                parts,
            });
        }
        out.provinces[id] = provinces;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_sum_to_a_stability_from_0_to_1() {
        let mut p = Parts {
            base: BASE,
            prosperity: 0.5,
            court: 0.25,
            army: 0.15,
            ..Parts::default()
        };
        assert_eq!(p.stability(), 1.0);
        p.prosperity = 0.0;
        p.court = 0.0;
        p.army = -0.15;
        p.corruption = -0.2;
        p.burden = -0.15;
        assert!((p.stability() - 0.0).abs() < 1e-12);
        p.base = 0.9;
        assert!((p.stability() - 0.4).abs() < 1e-12);
    }

    #[test]
    fn each_state_with_accounts_gets_a_stability() {
        let c: crate::config::Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 5, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "continents", "states": 6 }
        }))
        .unwrap();
        let w = crate::world::World::build(&c).unwrap();
        let st = &w.stability;
        assert!(st.state(0).is_none());
        let mut any = false;
        for s in &w.society.states {
            let Some(d) = st.state(s.id) else { continue };
            any = true;
            assert!((0.0..=1.0).contains(&d.stability));
            assert!((d.parts.base - BASE).abs() < 1e-9, "{}", d.parts.base);
            let provinces = st.provinces(s.id);
            assert!(!provinces.is_empty());
            for p in provinces {
                assert!((0.0..=1.0).contains(&p.stability));
                assert!(p.parts.remoteness <= 0.0 && p.parts.diversity <= 0.0);
                assert!(p.parts.corruption <= 0.0 && p.parts.burden <= 0.0);
                if p.province.id != NONE {
                    let found = st.province(&w.society, p.province.id).unwrap();
                    assert_eq!(found, p);
                }
            }
        }
        assert!(any);
    }
}
