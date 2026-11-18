//! Prosperity (`prosperity.ts`): how well people live, from what they
//! consume.
//!
//! - Needs met (0 to 1): the share of each need covered. It multiplies the
//!   rest, so a people short of food is never prosperous.
//! - Comfort: the consumption at base prices, in days of bare needs.
//! - Variety: the effective number of goods consumed.
//! - Living = needs met x (`NEEDS_SHARE` + `COMFORT_SHARE` x comfort score +
//!   `VARIETY_SHARE` x variety score); prosperity takes
//!   `INFRASTRUCTURE_SHARE` from the infrastructure that serves them.
//! - Rural people's needs come from their fields. A market gives its rural
//!   earnings and purchases to its cells by the value of their produce.

use super::corruption::Corruption;
use super::deals::{Deal, DealKind};
use super::demand::{Need, TARGET_FACTORS};
use super::goods::Good;
use crate::society::Society;

const NEEDS_SHARE: f64 = 0.2;
const COMFORT_SHARE: f64 = 0.4;
const VARIETY_SHARE: f64 = 0.4;
/// Consuming this many times the bare needs is full comfort.
const COMFORT_TOP: f64 = 15.0;
/// Consuming this many goods in even measure is full variety.
const VARIETY_TOP: f64 = 8.0;
/// The plenty of a village: rural people are measured against village
/// wants.
const RURAL_COMFORT_TOP: f64 = 6.0;
const RURAL_VARIETY_TOP: f64 = 8.0;
/// The share of prosperity that the public works of the state give.
const INFRASTRUCTURE_SHARE: f64 = 0.15;

/// The names of the ten prosperity levels (`PROSPERITY_LEVELS`).
pub const LEVELS: [&str; 10] = [
    "Desolate",
    "Destitute",
    "Poor",
    "Modest",
    "Comfortable",
    "Thriving",
    "Affluent",
    "Opulent",
    "Splendid",
    "Peerless",
];

/// How a people lives (`Living`): needs, infrastructure, and prosperity
/// from 0 to 1; comfort in days of bare needs; variety in effective goods.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Living {
    pub needs: f64,
    pub comfort: f64,
    pub variety: f64,
    pub infrastructure: f64,
    pub prosperity: f64,
}

/// The rural people of a cell (`RuralCell`): their share of the rural
/// earnings and purchases of the market, and the units they live on.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RuralCell {
    pub people: f64,
    pub earned: f64,
    /// Good id and units bought.
    pub bought: Vec<(u32, f64)>,
    /// Good id and units lived on: bought, and unsold produce that meets a
    /// need.
    pub goods: Vec<(u32, f64)>,
    pub living: Living,
}

/// The index of a prosperity level (`getLevel`).
pub fn level(prosperity: f64) -> usize {
    ((prosperity * 10.0).floor().max(0.0) as usize).min(LEVELS.len() - 1)
}

/// What a consumption measures (`Consumption`).
struct Consumption<'a> {
    people: f64,
    /// Units by good index.
    goods: &'a [f64],
    needs_cost: f64,
    infrastructure: f64,
    needs_met: Option<f64>,
    comfort_top: f64,
    variety_top: f64,
}

/// Each need counts by its size (`getNeedsMet`).
fn needs_met(people: f64, covered: &[f64; 5]) -> f64 {
    let total = TARGET_FACTORS.iter().fold(0.0, |s, f| s + f);
    Need::ALL.iter().fold(0.0, |sum, n| {
        let k = n.index();
        let target = people * TARGET_FACTORS[k];
        sum + (TARGET_FACTORS[k] / total) * (covered[k] / target).min(1.0)
    })
}

/// How a consumption lives (`measure`).
fn measure(goods: &[Good], c: &Consumption) -> Living {
    let bare = c.people * c.needs_cost;
    let mut covered = [0.0; 5];
    // Rural fields count as one good worth the bare needs.
    let mut values: Vec<f64> = if c.needs_met.is_some() {
        vec![bare]
    } else {
        Vec::new()
    };
    for (g, &units) in c.goods.iter().enumerate() {
        if units == 0.0 || units.is_nan() {
            continue;
        }
        values.push(units * goods[g].value);
        if c.needs_met.is_some() {
            continue;
        }
        for (k, cv) in covered.iter_mut().enumerate() {
            *cv += units * goods[g].coverage[k];
        }
    }
    let needs = c.needs_met.unwrap_or_else(|| needs_met(c.people, &covered));
    let total = values.iter().fold(0.0, |s, v| s + v);
    let comfort = if bare != 0.0 { total / bare } else { 0.0 };
    let entropy = values.iter().fold(0.0, |s, &v| {
        if v > 0.0 {
            s - (v / total) * libm::log(v / total)
        } else {
            s
        }
    });
    let variety = if total != 0.0 {
        libm::exp(entropy)
    } else {
        0.0
    };
    let comfort_score = if comfort > 1.0 {
        (libm::log(comfort) / libm::log(c.comfort_top)).min(1.0)
    } else {
        0.0
    };
    let variety_score = if variety > 1.0 {
        ((variety - 1.0) / (c.variety_top - 1.0)).min(1.0)
    } else {
        0.0
    };
    let living =
        needs * (NEEDS_SHARE + COMFORT_SHARE * comfort_score + VARIETY_SHARE * variety_score);
    let prosperity =
        (1.0 - INFRASTRUCTURE_SHARE) * living + INFRASTRUCTURE_SHARE * c.infrastructure;
    Living {
        needs,
        comfort,
        variety,
        infrastructure: c.infrastructure,
        prosperity,
    }
}

/// The infrastructure provision of the province of each cell
/// (`getInfrastructure`).
pub fn infrastructure(soc: &Society, corruption: &Corruption) -> Vec<f64> {
    let mut served: Vec<Vec<(u32, f64)>> = vec![Vec::new(); soc.states.len() + 1];
    for s in &soc.states {
        served[s.id as usize] = super::orders::infrastructure(corruption, s);
    }
    (0..soc.cell_state.len())
        .map(|c| {
            corruption.of_cell[c].map_or(0.0, |(s, k)| {
                served[s as usize].get(k).map_or(0.0, |&(_, v)| v)
            })
        })
        .collect()
}

/// The units of each good that each place bought or kept for its people,
/// by place index and good index (`getConsumption` of `Prosperity`).
pub fn consumed(deals: &[Deal], places: usize, goods: usize) -> Vec<Vec<f64>> {
    let mut out = vec![Vec::new(); places];
    for d in deals {
        if d.kind != DealKind::Demand && d.kind != DealKind::Kept {
            continue;
        }
        let list: &mut Vec<f64> = &mut out[d.burg as usize - 1];
        if list.is_empty() {
            list.resize(goods, 0.0);
        }
        list[d.good as usize - 1] += d.units;
    }
    out
}

/// How the people of each place live; `None` for a place with no people or
/// no market (`getBurg`).
pub fn burgs(
    goods: &[Good],
    soc: &Society,
    deals: &[Deal],
    cell_market: &[u32],
    infrastructure: &[f64],
) -> Vec<Option<Living>> {
    let needs_cost = super::demand::needs_cost(goods);
    let consumed = consumed(deals, soc.places.len(), goods.len());
    soc.places
        .iter()
        .enumerate()
        .map(|(b, p)| {
            if p.population == 0.0 || cell_market[p.cell] == 0 {
                return None;
            }
            Some(measure(
                goods,
                &Consumption {
                    people: p.population,
                    goods: &consumed[b],
                    needs_cost,
                    infrastructure: infrastructure[p.cell],
                    needs_met: None,
                    comfort_top: COMFORT_TOP,
                    variety_top: VARIETY_TOP,
                },
            ))
        })
        .collect()
}

/// The output of each cell with the water deposits that its people work
/// from the shore of the same market (`getWorked`).
pub fn worked(land: &super::output::Land, output: &[Vec<(u32, f64)>]) -> Vec<Vec<(u32, f64)>> {
    let mut worked = output.to_vec();
    for cell in 0..output.len() {
        if land.cells.h[cell] >= 20.0 || land.cell_market[cell] == 0 {
            continue;
        }
        let shore = land.working_cell(cell);
        if shore == cell
            || land.cells.pop[shore] == 0.0
            || land.cell_market[shore] != land.cell_market[cell]
        {
            continue;
        }
        let mut merged = worked[shore].clone();
        for &(g, units) in &output[cell] {
            match merged.iter_mut().find(|(x, _)| *x == g) {
                Some(e) => e.1 += units,
                None => merged.push((g, units)),
            }
        }
        merged.sort_by_key(|&(g, _)| g);
        worked[shore] = merged;
    }
    worked
}

/// The rural people of each cell: their earnings, purchases, and living
/// (`indexRural`).
pub fn rural(
    goods: &[Good],
    land: &super::output::Land,
    markets: &[super::markets::Market],
    output: &[Vec<(u32, f64)>],
    infrastructure: &[f64],
) -> Vec<Option<RuralCell>> {
    let n = output.len();
    let g_count = goods.len();
    let needs_cost = super::demand::needs_cost(goods);
    let worked = worked(land, output);
    // The cells of each market: cell, people, and the value of the produce.
    let mut by_market: Vec<Vec<(usize, f64, f64)>> = vec![Vec::new(); markets.len()];
    for cell in 0..n {
        let people = land.cells.pop[cell];
        let m = land.cell_market[cell];
        if people == 0.0 || m == 0 {
            continue;
        }
        let produce = worked[cell].iter().fold(0.0, |s, &(g, units)| {
            s + units * goods[g as usize - 1].value
        });
        by_market[m as usize - 1].push((cell, people, produce));
    }
    let mut out: Vec<Option<RuralCell>> = vec![None; n];
    for (k, market) in markets.iter().enumerate() {
        let cells = &by_market[k];
        let Some(rural) = &market.rural else {
            continue;
        };
        if cells.is_empty() {
            continue;
        }
        let total_produce = cells.iter().fold(0.0, |s, c| s + c.2);
        let total_people = cells.iter().fold(0.0, |s, c| s + c.1);
        // Unsold produce stays with those who grew it; only what meets a
        // need is lived on.
        let mut grown = vec![0.0; g_count];
        for &(cell, _, _) in cells {
            for &(g, units) in &worked[cell] {
                grown[g as usize - 1] += units;
            }
        }
        let kept: Vec<(u32, f64)> = rural
            .kept
            .iter()
            .copied()
            .filter(|&(g, _)| grown[g as usize - 1] != 0.0 && goods[g as usize - 1].covers_needs())
            .collect();
        for &(cell, people, produce) in cells {
            let share = if total_produce != 0.0 {
                produce / total_produce
            } else {
                people / total_people
            };
            let mut dense = vec![0.0; g_count];
            let mut bought = Vec::with_capacity(rural.bought.len());
            for &(g, units) in &rural.bought {
                dense[g as usize - 1] = units * share;
                bought.push((g, units * share));
            }
            for &(g, units) in &kept {
                let here = worked[cell]
                    .iter()
                    .find(|(x, _)| *x == g)
                    .map_or(0.0, |&(_, u)| u);
                dense[g as usize - 1] += units * here / grown[g as usize - 1];
            }
            let living = measure(
                goods,
                &Consumption {
                    people,
                    goods: &dense,
                    needs_cost,
                    infrastructure: infrastructure[cell],
                    needs_met: Some(1.0),
                    comfort_top: RURAL_COMFORT_TOP,
                    variety_top: RURAL_VARIETY_TOP,
                },
            );
            let goods_list = dense
                .iter()
                .enumerate()
                .filter(|&(_, &u)| u != 0.0)
                .map(|(g, &u)| (g as u32 + 1, u))
                .collect();
            out[cell] = Some(RuralCell {
                people,
                earned: rural.earned * share,
                bought,
                goods: goods_list,
                living,
            });
        }
    }
    out
}

/// The rural prosperity percent of each cell, 0 where no one lives
/// (`getCellsProsperity`, `cells.prosperity`).
pub fn cells(rural: &[Option<RuralCell>]) -> Vec<u8> {
    rural
        .iter()
        .map(|r| {
            r.as_ref().map_or(0, |r| {
                crate::rand::js_round(r.living.prosperity * 100.0).clamp(1.0, 255.0) as u8
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn goods() -> Vec<Good> {
        super::super::goods::catalogue()
    }

    fn living(goods: &[Good], units: &[f64]) -> Living {
        let needs_cost = super::super::demand::needs_cost(goods);
        measure(
            goods,
            &Consumption {
                people: 10.0,
                goods: units,
                needs_cost,
                infrastructure: 0.0,
                needs_met: None,
                comfort_top: COMFORT_TOP,
                variety_top: VARIETY_TOP,
            },
        )
    }

    #[test]
    fn nothing_meets_no_need() {
        let g = goods();
        let l = living(&g, &vec![0.0; g.len()]);
        assert_eq!(l.needs, 0.0);
        assert_eq!(l.prosperity, 0.0);
    }

    /// The cheapest coverage of each need, enough for 10 people.
    fn bare(g: &[Good]) -> Vec<f64> {
        let mut units = vec![0.0; g.len()];
        for n in Need::ALL {
            let k = n.index();
            let (best, _) = g
                .iter()
                .enumerate()
                .filter(|(_, x)| x.coverage[k] > 0.0)
                .map(|(i, x)| (i, x.value / x.coverage[k]))
                .fold((0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a });
            units[best] += 10.0 * TARGET_FACTORS[k] / g[best].coverage[k];
        }
        units
    }

    #[test]
    fn bare_needs_the_cheapest_way_meet_every_need() {
        let g = goods();
        let l = living(&g, &bare(&g));
        assert!((l.needs - 1.0).abs() < 1e-9);
        assert!((l.comfort - 1.0).abs() < 1e-9);
        assert!(l.prosperity > 0.0);
    }

    #[test]
    fn more_and_finer_goods_live_better() {
        let g = goods();
        let base = living(&g, &bare(&g));
        let mut more = bare(&g);
        // A little of five more goods: Wine, Silk, Spices, Tea, and Jewelry.
        for k in [12, 25, 26, 38, 62] {
            more[k] += 0.05;
        }
        let richer = living(&g, &more);
        assert!(richer.comfort > base.comfort);
        assert!(richer.variety > base.variety);
        assert!(richer.prosperity > base.prosperity);
    }

    #[test]
    fn a_level_has_a_name() {
        assert_eq!(LEVELS[level(0.0)], "Desolate");
        assert_eq!(LEVELS[level(0.47)], "Comfortable");
        assert_eq!(LEVELS[level(1.0)], "Peerless");
    }
}
