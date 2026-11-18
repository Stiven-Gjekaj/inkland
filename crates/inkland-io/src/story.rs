//! The JSON of the stages after the society: diplomacy, the economy, the
//! military, stability, zones, journeys, measurers, legends, and notes.
//!
//! `world.json`, `query`, and the GeoJSON export use these values. All
//! numbers have 3 decimals or fewer, and all lists are in id order.

use inkland_core::economy::finance::{Expense, StateFinance, Tariff, TariffRule};
use inkland_core::num::round;
use inkland_core::society::State;
use inkland_core::world::World;
use serde_json::{Value, json};

fn point(p: [f64; 2]) -> Value {
    json!([round(p[0], 3), round(p[1], 3)])
}

fn r3(v: f64) -> f64 {
    round(v, 3)
}

fn expenses(v: &[f64; 4]) -> Value {
    let mut m = serde_json::Map::new();
    for e in Expense::ALL {
        m.insert(e.name().to_owned(), json!(r3(v[e.index()])));
    }
    Value::Object(m)
}

fn tariff_value(t: &TariffRule) -> Value {
    json!({
        "states": t.states.as_ref().map_or_else(|| json!("all"), |s| json!(s)),
        "goods": t.goods.as_ref().map_or_else(|| json!("all"), |g| json!(g)),
        "rate": match t.rate {
            Tariff::Rate(r) => json!(r3(r)),
            Tariff::Embargo => json!("embargo"),
        },
    })
}

/// The finance of a state (FMG `state.economy`).
pub fn finance_value(f: &StateFinance) -> Value {
    let mut budget = expenses(&f.budget.expenses);
    budget["corruption"] = json!(r3(f.budget.corruption));
    json!({
        "sales_tax": r3(f.sales_tax),
        "poll_tax": r3(f.poll_tax),
        "market_fee": r3(f.market_fee),
        "budget": budget,
        "collected": r3(f.collected),
        "spent": expenses(&f.spent),
        "pocketed": r3(f.pocketed),
        "treasury": r3(f.treasury),
        "tariffs": f.tariffs.iter().map(tariff_value).collect::<Vec<_>>(),
    })
}

/// A regiment (FMG `state.military[..]`).
pub fn regiment_value(r: &inkland_core::military::Regiment) -> Value {
    let mut units = serde_json::Map::new();
    for (u, n) in &r.units {
        units.insert(u.clone(), json!(r3(*n)));
    }
    json!({
        "id": r.id,
        "state": r.state,
        "name": r.name,
        "cell": r.cell,
        "at": point(r.at),
        "base": point(r.base),
        "naval": r.naval,
        "units": units,
        "total": r3(r.total),
        "icon": r.icon,
        "note": r.note,
    })
}

fn parts_value(p: &inkland_core::stability::Parts) -> Value {
    json!({
        "base": r3(p.base),
        "prosperity": r3(p.prosperity),
        "court": r3(p.court),
        "army": r3(p.army),
        "remoteness": r3(p.remoteness),
        "diversity": r3(p.diversity),
        "corruption": r3(p.corruption),
        "burden": r3(p.burden),
    })
}

fn people_value(p: &inkland_core::economy::accounts::People) -> Value {
    json!({
        "people": r3(p.people),
        "urban_people": r3(p.urban_people),
        "rural": r3(p.rural),
        "manufacturing": r3(p.manufacturing),
        "trade": r3(p.trade),
        "product": r3(p.product),
        "wealth": r3(p.wealth),
        "prosperity": p.prosperity.map(r3),
    })
}

/// The new fields of a state: relations, campaigns, finance, alert,
/// regiments, stability, and accounts.
pub fn add_to_state(v: &mut Value, st: &State, world: &World) {
    let e = &world.economy;
    let acc = &e.accounts;
    v["diplomacy"] = json!(st.diplomacy.iter().map(|r| r.name()).collect::<Vec<_>>());
    v["campaigns"] = json!(
        st.campaigns
            .iter()
            .map(|c| json!({
                "name": c.name,
                "start": c.start,
                "end": c.end,
                "attacker": c.attacker,
                "defender": c.defender,
            }))
            .collect::<Vec<_>>()
    );
    v["alert"] = json!(r3(st.alert));
    v["economy"] = finance_value(&st.economy);
    v["military"] = json!(st.military.iter().map(regiment_value).collect::<Vec<_>>());
    v["stability"] = world.stability.state(st.id).map_or(
        Value::Null,
        |s| json!({ "stability": r3(s.stability), "parts": parts_value(&s.parts) }),
    );
    if let Some(a) = acc.states.get(st.id as usize) {
        let mut accounts = people_value(&a.people);
        accounts["taxes"] = json!(r3(a.taxes));
        accounts["economy_type"] = json!(inkland_core::economy::accounts::economy_type(
            &a.people,
            &acc.world.people
        ));
        accounts["known_for"] = json!(inkland_core::economy::accounts::known_for(
            &world.society,
            e,
            st.id
        ));
        v["accounts"] = accounts;
    }
}

/// A zone (FMG `pack.zones[..]`).
pub fn zone_value(z: &inkland_core::zones::Zone) -> Value {
    json!({
        "id": z.id,
        "name": z.name,
        "type": z.kind,
        "color": z.color,
        "cells": z.cells,
        "note": z.note,
    })
}

/// A journey (FMG `pack.journeys[..]`).
pub fn journey_value(j: &inkland_core::journeys::Journey) -> Value {
    json!({
        "id": j.id,
        "name": j.name,
        "type": j.kind,
        "color": j.color,
        "distance": r3(j.totals.distance),
        "hours": r3(j.totals.hours),
        "days": r3(j.totals.days),
        "segments": j.segments.iter().map(|s| json!({
            "id": s.id,
            "name": s.name,
            "from": s.from,
            "to": s.to,
            "transport": s.transport,
            "speed": r3(s.speed),
            "distance": r3(s.distance),
            "hours": r3(s.hours),
            "duration": s.duration.map(r3),
            "note": s.note,
            "cells": s.points.iter().map(|p| p.cell).collect::<Vec<_>>(),
            "points": s.points.iter().map(|p| point(p.at)).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// A measurer (FMG `pack.measurers[..]`).
pub fn measurer_value(m: &inkland_core::annotations::Measurer) -> Value {
    json!({
        "type": m.kind.name(),
        "length": r3(m.length()),
        "points": m.points.iter().map(|&p| point(p)).collect::<Vec<_>>(),
    })
}

/// The world-level data of the stages after the society.
pub fn add_to_world(v: &mut Value, world: &World) {
    let e = &world.economy;
    v["goods"] = json!(
        e.goods
            .iter()
            .map(|g| json!({
                "id": g.id,
                "name": g.name,
                "tags": g.tags,
                "icon": g.icon,
                "color": g.color,
                "value": g.value,
                "carriage": g.carriage,
                "unit": g.unit,
                "development": g.development,
                "raw": g.distribution.is_some(),
                "made": !g.recipes.is_empty(),
            }))
            .collect::<Vec<_>>()
    );
    v["cell_good"] = json!(world.society.cells.good);
    v["markets"] = json!(
        e.markets
            .iter()
            .map(|m| json!({
                "id": m.id,
                "center": m.center,
                "color": m.color,
                "fee_paid": r3(m.fee_paid),
                "goods": m.goods.iter().enumerate()
                    .filter(|(_, g)| g.stock != 0.0 || g.price != 0.0)
                    .map(|(k, g)| json!({ "good": k + 1, "stock": r3(g.stock), "price": r3(g.price) }))
                    .collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>()
    );
    v["cell_market"] = json!(e.cell_market);
    v["place_economy"] = json!(
        e.places
            .iter()
            .enumerate()
            .map(|(k, p)| json!({
                "place": k + 1,
                "production": p.production.iter().map(|&(g, u)| json!([g, r3(u)])).collect::<Vec<_>>(),
                "product": r3(p.product),
                "poll_tax_paid": r3(p.poll_tax_paid),
                "treasury": r3(p.treasury),
            }))
            .collect::<Vec<_>>()
    );
    // The deal log is the largest part, so each deal is a row of values in
    // the order of `fields`.
    v["deals"] = json!({
        "fields": ["burg", "market", "good", "kind", "units", "price", "tax", "tariff", "carriage"],
        "rows": e.deals.iter().map(|d| json!([
            d.burg, d.market, d.good, d.kind.name(), r3(d.units), r3(d.price),
            r3(d.tax), r3(d.tariff), r3(d.carriage),
        ])).collect::<Vec<_>>(),
    });
    v["cell_prosperity"] = json!(e.prosperity);
    v["economy_world"] = people_value(&e.accounts.world.people);
    v["zones"] = json!(world.zones.iter().map(zone_value).collect::<Vec<_>>());
    v["journeys"] = json!(world.journeys.iter().map(journey_value).collect::<Vec<_>>());
    v["measurers"] = json!(
        world
            .measurers
            .iter()
            .map(measurer_value)
            .collect::<Vec<_>>()
    );
    v["legends"] = json!(
        world
            .legends
            .iter()
            .map(|l| json!({
                "name": l.name,
                "items": l.items.iter().map(|i| json!({ "color": i.color, "text": i.text })).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>()
    );
    v["notes"] = json!(
        inkland_core::annotations::notes(world)
            .iter()
            .map(|n| json!({ "type": n.kind, "id": n.id, "name": n.name, "note": n.note }))
            .collect::<Vec<_>>()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkland_core::config::Config;

    fn world() -> (World, Config) {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 7, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea", "states": 8 }
        }))
        .unwrap();
        (World::build(&c).unwrap(), c)
    }

    #[test]
    fn world_json_has_the_economy_military_zones_and_story() {
        let (w, c) = world();
        let v = crate::world::world_value(&w, &c);
        let n = w.grid.len();
        for key in ["cell_good", "cell_market", "cell_prosperity"] {
            assert_eq!(v[key].as_array().unwrap().len(), n, "{key}");
        }
        assert_eq!(v["goods"].as_array().unwrap().len(), w.economy.goods.len());
        assert_eq!(
            v["markets"].as_array().unwrap().len(),
            w.economy.markets.len()
        );
        assert_eq!(
            v["deals"]["rows"].as_array().unwrap().len(),
            w.economy.deals.len()
        );
        assert_eq!(v["deals"]["fields"].as_array().unwrap().len(), 9);
        assert_eq!(v["zones"].as_array().unwrap().len(), w.zones.len());
        assert_eq!(v["journeys"].as_array().unwrap().len(), w.journeys.len());
        assert_eq!(v["measurers"][0]["type"], "Ruler");
        assert!(v["legends"].as_array().unwrap().is_empty());
        assert!(v["chronicle"].is_array());
        let states = v["states"].as_array().unwrap();
        for (st, s) in states.iter().zip(&w.society.states) {
            let rel = st["diplomacy"].as_array().unwrap();
            assert_eq!(rel.len(), w.society.states.len() + 1);
            assert_eq!(rel[s.id as usize], "x");
            assert!(st["economy"]["budget"]["military"].is_number());
            assert_eq!(st["military"].as_array().unwrap().len(), s.military.len());
            assert!(st["campaigns"].as_array().is_some_and(|c| !c.is_empty()));
        }
        let notes = v["notes"].as_array().unwrap();
        assert_eq!(notes.len(), inkland_core::annotations::notes(&w).len());
    }

    #[test]
    fn an_embargo_and_an_all_rule_have_their_fmg_words() {
        let t = TariffRule {
            states: None,
            goods: None,
            rate: Tariff::Embargo,
        };
        let v = tariff_value(&t);
        assert_eq!(v["states"], "all");
        assert_eq!(v["goods"], "all");
        assert_eq!(v["rate"], "embargo");
    }
}
