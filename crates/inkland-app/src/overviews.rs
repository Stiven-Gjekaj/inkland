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
}
