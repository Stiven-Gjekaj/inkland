//! Names and arms for generated areas (`world.renames`).
//!
//! An entry finds its area by a point, so the reference does not depend
//! on the generated ids: the state, culture, religion, or province of the
//! cell at the point. FMG renames an area in its editors; inkland keeps
//! the name in the config, so it comes back after each regeneration.

use crate::config::{AreaKind, Config, RenameEntry};
use crate::error::{Error, ErrorCode};
use crate::grid::Grid;
use crate::religions::Religions;
use crate::society::Society;

fn invalid(path: String, message: impl Into<String>) -> Error {
    Error::new(ErrorCode::RenameInvalid, path, message)
}

/// The errors of the form of an entry: one reference, a name that is not
/// empty, and a full name or arms only for a state or a province. The
/// point pin must exist.
pub fn check(config: &Config, i: usize, e: &RenameEntry) -> Vec<Error> {
    let path = format!("/world/renames/{i}");
    let mut out = Vec::new();
    if e.at.is_some() == e.pin.is_some() {
        out.push(invalid(
            path.clone(),
            "a rename needs exactly one of at and pin",
        ));
    }
    if let Some(id) = &e.pin
        && !config.pins.points.iter().any(|p| &p.id == id)
    {
        out.push(Error::new(
            ErrorCode::RefUnknown,
            format!("{path}/pin"),
            format!("no point pin has the id \"{id}\""),
        ));
    }
    if e.name.as_deref().is_some_and(|n| n.trim().is_empty()) {
        out.push(invalid(format!("{path}/name"), "a name cannot be empty"));
    }
    if matches!(e.kind, AreaKind::Culture | AreaKind::Religion) {
        for (key, given) in [
            ("full_name", e.full_name.is_some()),
            ("coa", e.coa.is_some()),
        ] {
            if given {
                out.push(invalid(
                    format!("{path}/{key}"),
                    format!("a {} has no {key}", e.kind.name()),
                ));
            }
        }
    }
    if let Some(c) = &e.color {
        if e.kind != AreaKind::State {
            out.push(invalid(
                format!("{path}/color"),
                format!("a {} has no color", e.kind.name()),
            ));
        } else if !is_hex_color(c) {
            out.push(invalid(
                format!("{path}/color"),
                format!("the color \"{c}\" is not #rrggbb"),
            ));
        }
    }
    out
}

/// `true` for a color `#rrggbb`. The case of the digits is free.
fn is_hex_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit())
}

/// The point of an entry.
fn point(config: &Config, e: &RenameEntry) -> Option<[f64; 2]> {
    match (&e.at, &e.pin) {
        (Some(p), None) => Some(*p),
        (None, Some(id)) => config
            .pins
            .points
            .iter()
            .find(|p| &p.id == id)
            .map(|p| p.at),
        _ => None,
    }
}

/// The new full name: the given one, else the old full name with the old
/// name changed to the new name.
fn full_name(old_full: &str, old: &str, new: &str, given: Option<&String>) -> String {
    match given {
        Some(f) => f.clone(),
        None if !old.is_empty() => old_full.replace(old, new),
        None => old_full.to_owned(),
    }
}

/// Apply the renames of the config, in order. A later entry for the same
/// area wins.
pub fn apply(
    config: &Config,
    grid: &Grid,
    soc: &mut Society,
    religions: &mut Religions,
) -> Result<(), Error> {
    for (i, e) in config.world.renames.iter().enumerate() {
        if let Some(err) = check(config, i, e).into_iter().next() {
            return Err(err);
        }
        let path = format!("/world/renames/{i}");
        let p = point(config, e).expect("check gives one reference");
        let cell = grid.find_cell(p[0], p[1]);
        let missing = || {
            invalid(
                path.clone(),
                format!("no {} holds the point ({}, {})", e.kind.name(), p[0], p[1]),
            )
        };
        match e.kind {
            AreaKind::State => {
                let id = soc.cell_state[cell];
                let s = soc
                    .states
                    .iter_mut()
                    .find(|s| s.id == id && id != 0)
                    .ok_or_else(missing)?;
                let new = e.name.clone().unwrap_or_else(|| s.name.clone());
                s.full_name = full_name(&s.full_name, &s.name, &new, e.full_name.as_ref());
                s.name = new;
                if let Some(coa) = &e.coa {
                    s.coa = Some(with_shield(coa, s.coa.as_ref()));
                }
                if let Some(c) = &e.color {
                    s.color = c.to_ascii_lowercase();
                }
            }
            AreaKind::Province => {
                let id = soc.cell_province[cell];
                let pr = soc
                    .provinces
                    .iter_mut()
                    .find(|x| x.id == id && id != 0)
                    .ok_or_else(missing)?;
                let new = e.name.clone().unwrap_or_else(|| pr.name.clone());
                pr.full_name = full_name(&pr.full_name, &pr.name, &new, e.full_name.as_ref());
                pr.name = new;
                if let Some(coa) = &e.coa {
                    pr.coa = Some(with_shield(coa, pr.coa.as_ref()));
                }
            }
            AreaKind::Culture => {
                let id = soc.cell_culture[cell];
                let c = soc
                    .cultures
                    .iter_mut()
                    .find(|c| c.id == id && id != 0)
                    .ok_or_else(missing)?;
                if let Some(n) = &e.name {
                    c.name = n.clone();
                }
            }
            AreaKind::Religion => {
                let id = religions.cell_religion[cell];
                let r = religions
                    .list
                    .iter_mut()
                    .find(|r| r.id == id && id != 0)
                    .ok_or_else(missing)?;
                if let Some(n) = &e.name {
                    r.name = n.clone();
                }
            }
        }
    }
    Ok(())
}

/// Given arms, with the generated shield when they give none.
fn with_shield(
    given: &crate::emblems::Coa,
    generated: Option<&crate::emblems::Coa>,
) -> crate::emblems::Coa {
    crate::emblems::Coa {
        shield: given
            .shield
            .clone()
            .or_else(|| generated.and_then(|g| g.shield.clone())),
        ..given.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;
    use serde_json::json;

    fn config(renames: serde_json::Value) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 5, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea", "renames": renames },
            "pins": { "points": [
                { "id": "home", "kind": "city", "at": [200, 150], "capital_of": "Realm" }
            ] }
        }))
        .unwrap()
    }

    #[test]
    fn each_kind_takes_its_new_name_and_keeps_it_on_a_new_build() {
        let base = World::build(&config(json!([]))).unwrap();
        let soc = &base.society;
        let cell = base.grid.find_cell(200.0, 150.0);
        let province = soc.cell_province[cell];
        assert!(province != 0, "the capital has a province");
        let c = config(json!([
            { "kind": "state", "pin": "home", "name": "Ostrey" },
            { "kind": "province", "at": [200, 150], "name": "Hearth",
              "coa": { "t1": "vert", "charges": [{ "charge": "lionRampant", "t": "or", "p": "e" }] } },
            { "kind": "culture", "at": [200, 150], "name": "Ostreyan" },
            { "kind": "religion", "at": [200, 150], "name": "The Old Way" }
        ]));
        let w = World::build(&c).unwrap();
        let s = &w.society;
        let state = s
            .states
            .iter()
            .find(|x| x.id == s.cell_state[cell])
            .unwrap();
        assert_eq!(state.name, "Ostrey");
        assert!(state.full_name.contains("Ostrey"), "{}", state.full_name);
        assert!(!state.full_name.contains("Realm"), "{}", state.full_name);
        let pr = s
            .provinces
            .iter()
            .find(|x| x.id == s.cell_province[cell])
            .unwrap();
        assert_eq!(pr.name, "Hearth");
        let coa = pr.coa.as_ref().unwrap();
        assert_eq!(coa.t1, "vert");
        let generated = soc.provinces.iter().find(|x| x.id == province).unwrap();
        assert_eq!(coa.shield, generated.coa.as_ref().unwrap().shield);
        let culture = s
            .cultures
            .iter()
            .find(|x| x.id == s.cell_culture[cell])
            .unwrap();
        assert_eq!(culture.name, "Ostreyan");
        let rel = w
            .religions
            .list
            .iter()
            .find(|r| r.id == w.religions.cell_religion[cell])
            .unwrap();
        assert_eq!(rel.name, "The Old Way");
        // The same config gives the same names.
        assert_eq!(World::build(&c).unwrap().society.states, w.society.states);
        // The other states keep their names.
        for (a, b) in soc.states.iter().zip(&s.states) {
            if a.id != state.id {
                assert_eq!(a.name, b.name);
            }
        }
    }

    #[test]
    fn a_state_color_replaces_the_generated_color() {
        let base = World::build(&config(json!([]))).unwrap();
        let cell = base.grid.find_cell(200.0, 150.0);
        let id = base.society.cell_state[cell];
        let old = &base
            .society
            .states
            .iter()
            .find(|s| s.id == id)
            .unwrap()
            .color;
        let new = if old == "#8da0cb" {
            "#ebb275"
        } else {
            "#8da0cb"
        };
        let upper = new.to_ascii_uppercase();
        let w = World::build(&config(json!([
            { "kind": "state", "pin": "home", "color": upper }
        ])))
        .unwrap();
        for (a, b) in base.society.states.iter().zip(&w.society.states) {
            if a.id == id {
                assert_eq!(b.color, new);
                assert_eq!(b.name, a.name, "a color alone keeps the name");
            } else {
                assert_eq!(a.color, b.color);
            }
        }
    }

    #[test]
    fn a_bad_entry_stops_the_build_with_its_path() {
        let cases = [
            (
                json!([{ "kind": "state", "name": "X" }]),
                "/world/renames/0",
            ),
            (
                json!([{ "kind": "state", "pin": "nowhere", "name": "X" }]),
                "/world/renames/0/pin",
            ),
            (
                json!([{ "kind": "culture", "at": [200, 150], "full_name": "X" }]),
                "/world/renames/0/full_name",
            ),
            (
                json!([{ "kind": "state", "at": [200, 150], "name": " " }]),
                "/world/renames/0/name",
            ),
            (
                json!([{ "kind": "state", "at": [1, 1], "name": "X" }]),
                "/world/renames/0",
            ),
            (
                json!([{ "kind": "state", "at": [200, 150], "color": "#12345" }]),
                "/world/renames/0/color",
            ),
            (
                json!([{ "kind": "state", "at": [200, 150], "color": "8da0cb" }]),
                "/world/renames/0/color",
            ),
            (
                json!([{ "kind": "state", "at": [200, 150], "color": "#8da0cg" }]),
                "/world/renames/0/color",
            ),
            (
                json!([{ "kind": "province", "at": [200, 150], "color": "#8da0cb" }]),
                "/world/renames/0/color",
            ),
        ];
        for (renames, path) in cases {
            let e = World::build(&config(renames)).unwrap_err();
            assert_eq!(e.path, path, "{}", e.message);
        }
    }
}
