//! The rows of the read-only overviews of the world: diplomacy, military,
//! markers, notes, zones, stability, goods, markets, the economy of a
//! state, journeys, and measurers. They follow the dialogs of FMG. They do
//! not change anything, because the config does not hold these data.

use crate::rows::{Row, row};
use inkland::World;
use inkland_core::rand::rn;
use inkland_core::society::State;

/// A number as FMG `si` gives it: `1.2K`, `34K`, `5.6M`.
pub fn si(n: f64) -> String {
    let text = |v: f64| format!("{v}");
    if n >= 1e9 {
        format!("{}B", text(rn(n / 1e9, 1)))
    } else if n >= 1e8 {
        format!("{}M", text(rn(n / 1e6, 0)))
    } else if n >= 1e6 {
        format!("{}M", text(rn(n / 1e6, 1)))
    } else if n >= 1e4 {
        format!("{}K", text(rn(n / 1e3, 0)))
    } else if n >= 1e3 {
        format!("{}K", text(rn(n / 1e3, 1)))
    } else {
        text(rn(n, 0))
    }
}

/// A number with at most `decimals` decimals, as FMG `rn` gives it.
pub fn num(v: f64, decimals: u32) -> String {
    let r = rn(v, decimals);
    // `-0` shows as `0`.
    format!("{}", if r == 0.0 { 0.0 } else { r })
}

/// The state with an id, if it is not the neutral lands.
fn state(world: &World, id: u32) -> Option<&State> {
    id.checked_sub(1)
        .and_then(|k| world.society.states.get(k as usize))
}

/// The names of the states, in id order: the entries of the state select
/// of the overviews. Entry `k` is the state with the id `k + 1`.
pub fn state_names(world: &World) -> Vec<String> {
    world
        .society
        .states
        .iter()
        .map(|s| s.name.clone())
        .collect()
}

/// The state id of an entry of the state select, if the world has it.
pub fn state_of_index(world: &World, index: i32) -> Option<u32> {
    let k = usize::try_from(index).ok()?;
    world.society.states.get(k).map(|s| s.id)
}

/// The Diplomacy Overview of FMG: the relation of each other state to the
/// selected state. The columns are the state, the relation, and its
/// description. The swatch has the FMG color of the relation.
pub fn relation_rows(world: &World, selected: u32) -> Vec<Row> {
    let Some(sel) = state(world, selected) else {
        return Vec::new();
    };
    world
        .society
        .states
        .iter()
        .filter(|s| s.id != selected)
        .map(|s| {
            let relation = s
                .diplomacy
                .get(selected as usize)
                .copied()
                .unwrap_or(inkland_core::diplomacy::Relation::None);
            // FMG shows the full name when it is short.
            let name = if s.full_name.chars().count() < 23 {
                s.full_name.clone()
            } else {
                s.name.clone()
            };
            row(
                s.id.to_string(),
                vec![
                    name,
                    relation.name().to_owned(),
                    relation.describe(&s.name, &sel.name),
                ],
                false,
            )
            .with_color(relation.color())
        })
        .collect()
}

/// The relations history of FMG: each record has a title line and its
/// lines, with an empty line between the records.
pub fn chronicle_text(world: &World) -> String {
    let records = &world.society.chronicle;
    if records.is_empty() {
        return "No historical records".to_owned();
    }
    records
        .iter()
        .map(|r| r.join("\n"))
        .collect::<Vec<String>>()
        .join("\n\n")
}

/// A share from 0 to 1 as a whole percent.
fn percent(v: f64) -> String {
    format!("{}%", num(v * 100.0, 0))
}

/// The header of the Military Overview: the state, each unit, and the
/// totals.
pub fn military_header() -> Vec<String> {
    let mut h = vec!["State".to_owned()];
    h.extend(
        inkland_core::military::UNITS
            .iter()
            .map(|u| crate::catalog::title_case(u.name)),
    );
    h.extend(
        ["Total", "Population", "Rate", "Alert", "Upkeep", "Power"]
            .iter()
            .map(|t| (*t).to_owned()),
    );
    h
}

/// The Military Overview of FMG: for each state, the troops of each unit,
/// the personnel with the crew, the people, the rate, the war alert, the
/// upkeep, and the power.
pub fn military_rows(world: &World) -> Vec<Row> {
    use inkland_core::military::{POPULATION_SCALE, UNITS, URBANIZATION_RATE, troops};
    let e = &world.economy;
    world
        .society
        .states
        .iter()
        .map(|s| {
            let forces: Vec<f64> = UNITS
                .iter()
                .map(|u| s.military.iter().fold(0.0, |t, r| t + r.of(u.name)))
                .collect();
            let points = s.rural + s.urban * URBANIZATION_RATE;
            let population = rn(points * POPULATION_SCALE, 0);
            let total = UNITS
                .iter()
                .zip(&forces)
                .fold(0.0, |t, (u, f)| t + f * u.crew);
            let rate = if points != 0.0 {
                total / points * 100.0
            } else {
                0.0
            };
            let mut cells = vec![s.name.clone()];
            cells.extend(forces.iter().map(|f| num(troops(*f), 0)));
            cells.push(si(troops(total)));
            cells.push(si(population));
            cells.push(format!("{}%", num(rate, 2)));
            cells.push(num(s.alert, 2));
            cells.push(percent(inkland_core::military::upkeep(s, e)));
            cells.push(si(troops(inkland_core::military::power(s, e))));
            row(s.id.to_string(), cells, false).with_color(s.color.clone())
        })
        .collect()
}

/// The header of the Regiments Overview: the name, each unit, the total,
/// and the power.
pub fn regiment_header() -> Vec<String> {
    let mut h = vec!["Regiment".to_owned()];
    h.extend(
        inkland_core::military::UNITS
            .iter()
            .map(|u| crate::catalog::title_case(u.name)),
    );
    h.push("Total".to_owned());
    h.push("Power".to_owned());
    h
}

/// The Regiments Overview of FMG for one state: the name, the troops of
/// each unit, the personnel without the crew, and the power.
pub fn regiment_rows(world: &World, selected: u32) -> Vec<Row> {
    use inkland_core::military::{UNITS, regiment_power, troops};
    let Some(s) = state(world, selected) else {
        return Vec::new();
    };
    s.military
        .iter()
        .map(|r| {
            let mut cells = vec![r.name.clone()];
            cells.extend(UNITS.iter().map(|u| num(troops(r.of(u.name)), 0)));
            cells.push(num(r.troops(), 0));
            cells.push(si(troops(regiment_power(r, s, &world.economy))));
            row(format!("{}-{}", s.id, r.id), cells, false)
        })
        .collect()
}

/// The Markers Overview of FMG: the name, the type, and the legend.
pub fn marker_rows(world: &World) -> Vec<Row> {
    world
        .markers
        .iter()
        .map(|m| {
            row(
                m.id.to_string(),
                vec![
                    m.name.clone().unwrap_or_default(),
                    m.kind.to_owned(),
                    m.legend.clone().unwrap_or_default(),
                ],
                false,
            )
        })
        .collect()
}

/// The legend of the marker with the row id `id`.
pub fn marker_text(world: &World, id: &str) -> String {
    world
        .markers
        .iter()
        .find(|m| m.id.to_string() == id)
        .and_then(|m| m.legend.clone())
        .unwrap_or_default()
}

/// The row id of a note: its type and its id.
fn note_id(n: &inkland_core::annotations::Note) -> String {
    format!("{}:{}", n.kind, n.id)
}

/// The Notes Editor of FMG: the list of the notes, with the type and the
/// name of the item.
pub fn note_rows(world: &World) -> Vec<Row> {
    inkland_core::annotations::notes(world)
        .iter()
        .map(|n| {
            row(
                note_id(n),
                vec![crate::catalog::title_case(n.kind), n.name.clone()],
                false,
            )
        })
        .collect()
}

/// The text of the note with the row id `id`: the name, then the note.
pub fn note_text(world: &World, id: &str) -> String {
    inkland_core::annotations::notes(world)
        .iter()
        .find(|n| note_id(n) == id)
        .map(|n| format!("{}\n\n{}", n.name, n.note))
        .unwrap_or_default()
}

/// The Zones Editor of FMG: the description, the type, the cells, the area
/// in square units, the people, and the fill. The fill is an FMG hatch
/// pattern, for example `hatch1`, so the row has no swatch.
pub fn zone_rows(world: &World, unit: &str) -> Vec<Row> {
    use inkland_core::military::{POPULATION_SCALE, URBANIZATION_RATE};
    let cells = &world.society.cells;
    let mut place_of = vec![None; cells.pop.len()];
    for p in &world.society.places {
        if let Some(slot) = place_of.get_mut(p.cell) {
            *slot = Some(p.population);
        }
    }
    world
        .zones
        .iter()
        .map(|z| {
            let at = |v: &Vec<f64>, c: usize| v.get(c).copied().unwrap_or(0.0);
            let area = z.cells.iter().fold(0.0, |t, c| t + at(&cells.area, *c));
            let rural = z.cells.iter().fold(0.0, |t, c| t + at(&cells.pop, *c));
            let urban = z.cells.iter().fold(0.0, |t, c| {
                t + place_of.get(*c).copied().flatten().unwrap_or(0.0)
            });
            let people = rn(
                rural * POPULATION_SCALE + urban * POPULATION_SCALE * URBANIZATION_RATE,
                0,
            );
            row(
                z.id.to_string(),
                vec![
                    z.name.clone(),
                    z.kind.to_owned(),
                    z.cells.len().to_string(),
                    format!("{} {unit}2", si(area)),
                    si(people),
                    z.color
                        .trim_start_matches("url(#")
                        .trim_end_matches(')')
                        .to_owned(),
                ],
                false,
            )
        })
        .collect()
}

/// A share with a sign, in whole percent: `+12`, `-3`.
fn signed(v: f64) -> String {
    let r = rn(v.abs() * 100.0, 0);
    format!("{}{}", if v >= 0.0 { "+" } else { "-" }, num(r, 0))
}

/// The Stability Overview of FMG for one state: the parts (name and signed
/// percent), the total, and the provinces, least stable first.
pub struct StabilityView {
    pub parts: Vec<Row>,
    pub total: String,
    pub provinces: Vec<Row>,
}

/// The FMG labels of the stability parts, in `Parts::NAMES` order.
const PART_LABELS: [&str; 8] = [
    "Base",
    "Prosperity",
    "Court",
    "Army",
    "Remoteness",
    "Minorities",
    "Corruption",
    "Tax burden",
];

/// The stability of a state, or `None` if it has none (no accounts or no
/// people).
pub fn stability_view(world: &World, selected: u32) -> Option<StabilityView> {
    let soc = &world.society;
    let s = state(world, selected)?;
    let st = world.stability.state(selected)?;
    let parts = PART_LABELS
        .iter()
        .zip(st.parts.values())
        .enumerate()
        .map(|(k, (label, v))| {
            let color = if v >= 0.0 { "#208040" } else { "#c02020" };
            row(k.to_string(), vec![(*label).to_owned(), signed(v)], false).with_color(color)
        })
        .collect();
    let infrastructure =
        inkland_core::economy::orders::infrastructure(&world.economy.corruption, s);
    let mut list: Vec<&inkland_core::stability::ProvinceStability> =
        world.stability.provinces(selected).iter().collect();
    // FMG shows the provinces only for two or more.
    if list.len() < 2 {
        list.clear();
    }
    list.sort_by(|a, b| {
        a.stability
            .total_cmp(&b.stability)
            .then(a.province.id.cmp(&b.province.id))
    });
    let provinces = list
        .iter()
        .map(|p| {
            let d = &p.province;
            let name =
                d.id.checked_sub(1)
                    .and_then(|k| soc.provinces.get(k as usize))
                    .map_or_else(|| "-".to_owned(), |x| x.name.clone());
            let corruption = world
                .economy
                .corruption
                .of_province(s, d, s.economy.collected);
            let served = infrastructure
                .iter()
                .find(|(id, _)| *id == d.id)
                .map_or(0.0, |x| x.1);
            row(
                d.id.to_string(),
                vec![
                    name,
                    percent(d.share),
                    percent(d.remoteness),
                    percent(d.diversity),
                    format!("{}%", num(corruption, 0)),
                    percent(served),
                    percent(p.stability),
                ],
                false,
            )
        })
        .collect();
    Some(StabilityView {
        parts,
        total: format!("Stability: {}", percent(st.stability)),
        provinces,
    })
}

/// The Goods Editor of FMG: the name, the type (`MFG` for a good with
/// recipes, `RAW` for a good with deposits), the unit, the units made a
/// day in cells and burgs, the units in stock in all markets, and the base
/// price. The swatch has the color of the good.
pub fn goods_rows(world: &World) -> Vec<Row> {
    let e = &world.economy;
    let n = e.goods.len();
    let mut made = vec![0.0; n];
    let mut add = |id: u32, units: f64| {
        if let Some(m) = id.checked_sub(1).and_then(|k| made.get_mut(k as usize)) {
            *m += units;
        }
    };
    for cell in &e.rural_output {
        for (id, units) in cell {
            add(*id, *units);
        }
    }
    for place in &e.places {
        for (id, units) in &place.production {
            add(*id, *units);
        }
    }
    let mut stock = vec![0.0; n];
    for m in &e.markets {
        for (k, g) in m.goods.iter().enumerate() {
            if g.stock > 0.0
                && let Some(s) = stock.get_mut(k)
            {
                *s += g.stock;
            }
        }
    }
    e.goods
        .iter()
        .enumerate()
        .map(|(k, g)| {
            let kind: Vec<&str> = [
                (!g.recipes.is_empty()).then_some("MFG"),
                g.distribution.is_some().then_some("RAW"),
            ]
            .into_iter()
            .flatten()
            .collect();
            row(
                g.id.to_string(),
                vec![
                    g.name.to_owned(),
                    kind.join(","),
                    g.unit.to_owned(),
                    num(made[k], 0),
                    num(stock[k], 0),
                    num(g.value, 2),
                ],
                false,
            )
            .with_color(g.color)
        })
        .collect()
}

/// The name of a state, or `Neutrals` for the id 0.
fn state_name(world: &World, id: u32) -> String {
    state(world, id).map_or_else(|| "Neutrals".to_owned(), |s| s.name.clone())
}

/// The Markets Overview of FMG: the market (its center), the owner state,
/// the cells, the burgs, the stock, the sales, the buys, and the value
/// (sales less buys, plus the stock at its price).
pub fn market_rows(world: &World) -> Vec<Row> {
    let e = &world.economy;
    let soc = &world.society;
    e.markets
        .iter()
        .map(|m| {
            let center = m
                .center
                .checked_sub(1)
                .and_then(|k| soc.places.get(k as usize));
            let name = center.map_or_else(|| format!("Market {}", m.id), |p| p.name.clone());
            let owner = center.map_or_else(String::new, |p| state_name(world, p.state));
            let cells = e.cell_market.iter().filter(|c| **c == m.id).count();
            let burgs = soc
                .places
                .iter()
                .filter(|p| e.cell_market.get(p.cell) == Some(&m.id))
                .count();
            let stock = m.goods.iter().fold(0.0, |t, g| t + g.stock);
            let stock_value = m.goods.iter().fold(0.0, |t, g| t + g.stock * g.price);
            let (mut sales, mut buys) = (0.0, 0.0);
            for d in e.deals.iter().filter(|d| d.market == m.id) {
                let income = d.market_income(m.fee_paid);
                if income > 0.0 {
                    sales += income;
                } else {
                    buys -= income;
                }
            }
            row(
                m.id.to_string(),
                vec![
                    name,
                    owner,
                    cells.to_string(),
                    burgs.to_string(),
                    num(stock, 2),
                    num(sales, 2),
                    num(buys, 2),
                    num(sales - buys + stock_value, 2),
                ],
                false,
            )
            .with_color(m.color.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkland::Config;
    use serde_json::json;

    /// A small world with four states. The tests build it here and do not
    /// read a config file.
    pub(crate) fn world() -> (Config, World) {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 7, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "continents", "states": 4, "towns": 12 }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        (c, w)
    }

    #[test]
    fn si_and_num_round_as_fmg_does() {
        assert_eq!(si(999.4), "999");
        assert_eq!(si(1234.0), "1.2K");
        assert_eq!(si(56_789.0), "57K");
        assert_eq!(si(2_345_678.0), "2.3M");
        assert_eq!(num(2.0, 2), "2");
        assert_eq!(num(-0.001, 2), "0");
        assert_eq!(num(1.005, 1), "1");
    }

    #[test]
    fn the_diplomacy_rows_give_the_relation_to_the_selected_state() {
        use inkland_core::diplomacy::Relation;
        let (_, w) = world();
        let names = state_names(&w);
        assert_eq!(names.len(), w.society.states.len());
        assert!(names.len() >= 2);
        let selected = state_of_index(&w, 0).unwrap();
        assert_eq!(state_of_index(&w, -1), None);
        assert_eq!(state_of_index(&w, names.len() as i32), None);
        let rows = relation_rows(&w, selected);
        assert_eq!(rows.len(), names.len() - 1, "all states but the selected");
        for r in &rows {
            let other = &w.society.states[r.id.parse::<usize>().unwrap() - 1];
            assert_ne!(other.id, selected);
            let relation = other.diplomacy[selected as usize];
            assert_eq!(r.cells[1], relation.name());
            assert_eq!(r.color.as_deref(), Some(relation.color()));
            assert_ne!(relation, Relation::None);
        }
        assert!(relation_rows(&w, 0).is_empty(), "the neutrals have no list");
        let text = chronicle_text(&w);
        assert!(!text.is_empty());
    }

    #[test]
    fn the_military_rows_sum_the_regiments_of_each_state() {
        use inkland_core::military::{UNITS, troops};
        let (_, w) = world();
        let header = military_header();
        let rows = military_rows(&w);
        assert_eq!(rows.len(), w.society.states.len());
        for (r, s) in rows.iter().zip(&w.society.states) {
            assert_eq!(r.cells.len(), header.len());
            assert_eq!(r.cells[0], s.name);
            assert_eq!(r.color.as_deref(), Some(s.color.as_str()));
            for (k, u) in UNITS.iter().enumerate() {
                let sum = s.military.iter().fold(0.0, |t, g| t + g.of(u.name));
                assert_eq!(r.cells[k + 1], num(troops(sum), 0));
            }
        }
        let armed = w
            .society
            .states
            .iter()
            .find(|s| !s.military.is_empty())
            .expect("a state has regiments");
        let regiments = regiment_rows(&w, armed.id);
        assert_eq!(regiments.len(), armed.military.len());
        assert_eq!(regiments[0].cells.len(), regiment_header().len());
        assert_eq!(regiments[0].cells[0], armed.military[0].name);
        assert!(regiment_rows(&w, 0).is_empty());
    }

    #[test]
    fn the_marker_rows_give_the_name_the_type_and_the_legend() {
        let (_, w) = world();
        let rows = marker_rows(&w);
        assert_eq!(rows.len(), w.markers.len());
        assert!(!rows.is_empty(), "the world has markers");
        for (r, m) in rows.iter().zip(&w.markers) {
            assert_eq!(r.cells[1], m.kind);
            assert_eq!(marker_text(&w, &r.id), m.legend.clone().unwrap_or_default());
        }
        assert_eq!(marker_text(&w, "no-such-marker"), "");
    }

    #[test]
    fn the_note_rows_list_the_notes_and_give_their_text() {
        let (_, w) = world();
        let notes = inkland_core::annotations::notes(&w);
        let rows = note_rows(&w);
        assert_eq!(rows.len(), notes.len());
        assert!(!rows.is_empty(), "the world has notes");
        for (r, n) in rows.iter().zip(&notes) {
            assert_eq!(r.id, format!("{}:{}", n.kind, n.id));
            assert_eq!(r.cells[1], n.name);
            assert!(note_text(&w, &r.id).ends_with(&n.note));
        }
        assert_eq!(note_text(&w, "zone:none"), "");
    }

    #[test]
    fn the_zone_rows_count_the_cells_of_each_zone() {
        let (_, w) = world();
        let rows = zone_rows(&w, "mi");
        assert_eq!(rows.len(), w.zones.len());
        assert!(!rows.is_empty(), "the world has zones");
        for (r, z) in rows.iter().zip(&w.zones) {
            assert_eq!(r.cells[0], z.name);
            assert_eq!(r.cells[1], z.kind);
            assert_eq!(r.cells[2], z.cells.len().to_string());
            assert!(r.cells[3].ends_with(" mi2"), "{}", r.cells[3]);
            assert!(z.color.contains(r.cells[5].as_str()), "{}", z.color);
            assert!(!r.cells[5].contains('('));
        }
    }

    #[test]
    fn the_stability_view_gives_the_parts_and_the_provinces() {
        let (_, w) = world();
        let s = w
            .society
            .states
            .iter()
            .find(|s| w.stability.state(s.id).is_some())
            .expect("a state has a stability");
        let st = w.stability.state(s.id).unwrap();
        let v = stability_view(&w, s.id).unwrap();
        assert_eq!(v.parts.len(), 8);
        assert_eq!(v.parts[0].cells[0], "Base");
        assert_eq!(v.parts[1].cells[1], signed(st.parts.prosperity));
        assert_eq!(v.total, format!("Stability: {}", percent(st.stability)));
        let n = w.stability.provinces(s.id).len();
        assert_eq!(v.provinces.len(), if n < 2 { 0 } else { n });
        // The least stable province is first.
        let values: Vec<f64> = v
            .provinces
            .iter()
            .map(|r| r.cells[6].trim_end_matches('%').parse().unwrap())
            .collect();
        assert!(values.windows(2).all(|p| p[0] <= p[1]), "{values:?}");
        assert!(stability_view(&w, 0).is_none());
        assert_eq!(signed(0.123), "+12");
        assert_eq!(signed(-0.03), "-3");
    }

    #[test]
    fn the_goods_rows_sum_the_output_and_the_stock() {
        let (_, w) = world();
        let e = &w.economy;
        let rows = goods_rows(&w);
        assert_eq!(rows.len(), e.goods.len());
        let made: f64 = rows
            .iter()
            .map(|r| r.cells[3].parse::<f64>().unwrap())
            .sum();
        assert!(made > 0.0, "the world makes goods");
        for (r, g) in rows.iter().zip(&e.goods) {
            assert_eq!(r.cells[0], g.name);
            assert_eq!(r.cells[5], num(g.value, 2));
            assert_eq!(r.cells[1].contains("MFG"), !g.recipes.is_empty());
            assert_eq!(r.cells[1].contains("RAW"), g.distribution.is_some());
        }
        // One good, counted here by hand.
        let k = 0;
        let id = e.goods[k].id;
        let mut units = 0.0;
        for c in &e.rural_output {
            units += c.iter().filter(|x| x.0 == id).map(|x| x.1).sum::<f64>();
        }
        for p in &e.places {
            units += p
                .production
                .iter()
                .filter(|x| x.0 == id)
                .map(|x| x.1)
                .sum::<f64>();
        }
        assert_eq!(rows[k].cells[3], num(units, 0));
    }

    #[test]
    fn the_market_rows_count_the_cells_and_sum_the_deals() {
        let (_, w) = world();
        let e = &w.economy;
        let rows = market_rows(&w);
        assert_eq!(rows.len(), e.markets.len());
        assert!(!rows.is_empty(), "the world has markets");
        let cells: usize = rows
            .iter()
            .map(|r| r.cells[2].parse::<usize>().unwrap())
            .sum();
        assert_eq!(cells, e.cell_market.iter().filter(|m| **m != 0).count());
        for (r, m) in rows.iter().zip(&e.markets) {
            let center = &w.society.places[m.center as usize - 1];
            assert_eq!(r.cells[0], center.name);
            assert_eq!(r.color.as_deref(), Some(m.color.as_str()));
            let sales: f64 = r.cells[5].parse().unwrap();
            let buys: f64 = r.cells[6].parse().unwrap();
            assert!(sales >= 0.0 && buys >= 0.0);
        }
    }
}
