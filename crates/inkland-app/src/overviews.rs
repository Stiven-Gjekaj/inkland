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
}
