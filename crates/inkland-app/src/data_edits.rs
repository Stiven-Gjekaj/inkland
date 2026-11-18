//! The edit forms of the data dialogs: zones, markets, regiments,
//! journeys, and notes. Each action of a dialog is one `insert` or
//! `remove` command on a list of the world part of the config, so undo
//! gives back the config exactly.

use inkland::Command;
use inkland_core::config::Config;
use inkland_core::world::World;
use serde_json::{Value, json};

/// The command that inserts `value` at the end of the world list `key`.
fn append(config: &Config, key: &str, value: Value) -> Command {
    let len = serde_json::to_value(config)
        .ok()
        .and_then(|v| v["world"][key].as_array().map(Vec::len))
        .unwrap_or(0);
    Command::Insert {
        path: format!("/world/{key}/{len}"),
        value,
    }
}

/// The command of an action `op` on the item `id` of a dialog list, with
/// the text of its field.
pub fn command(
    config: &Config,
    world: &World,
    list: &str,
    id: &str,
    op: &str,
    text: &str,
) -> Result<Command, String> {
    let soc = &world.society;
    let missing = || format!("{list} has no item {id:?}");
    match (list, op) {
        ("zones", "remove" | "name") => {
            let z = world
                .zones
                .iter()
                .find(|z| z.id.to_string() == id)
                .ok_or_else(missing)?;
            let cell = *z.cells.first().ok_or_else(missing)?;
            let at = world.grid.points[cell];
            let at = [crate::model::round2(at[0]), crate::model::round2(at[1])];
            let value = if op == "remove" {
                json!({ "at": at, "remove": true })
            } else {
                json!({ "at": at, "name": text.trim() })
            };
            Ok(append(config, "zones", value))
        }
        ("markets", "remove") => {
            let m = world
                .economy
                .markets
                .iter()
                .find(|m| m.id.to_string() == id)
                .ok_or_else(missing)?;
            let at = soc
                .places
                .get(m.center as usize - 1)
                .ok_or_else(missing)?
                .at;
            Ok(append(
                config,
                "markets",
                json!({ "at": at, "remove": true }),
            ))
        }
        ("regiments", "remove" | "split") => {
            let (state, regiment) = id.split_once('-').ok_or_else(missing)?;
            let r = soc
                .states
                .iter()
                .filter(|s| s.id.to_string() == state)
                .flat_map(|s| &s.military)
                .find(|r| r.id.to_string() == regiment)
                .ok_or_else(missing)?;
            Ok(append(config, "regiments", json!({ "at": r.at, op: true })))
        }
        ("journeys", "remove") => {
            let id: u32 = id.parse().map_err(|_| missing())?;
            Ok(append(
                config,
                "journeys",
                json!({ "id": id, "remove": true }),
            ))
        }
        ("notes", "add") => {
            let name = text.trim();
            if name.is_empty() {
                return Err("a note needs a title".to_owned());
            }
            Ok(append(config, "notes", json!({ "name": name, "note": "" })))
        }
        ("notes", "remove") => {
            let k: usize = id
                .strip_prefix("custom:")
                .and_then(|k| k.parse().ok())
                .ok_or_else(|| "only a note of the config can go".to_owned())?;
            if k == 0 || k > config.world.notes.len() {
                return Err(missing());
            }
            Ok(Command::Remove {
                path: format!("/world/notes/{}", k - 1),
            })
        }
        _ => Err(format!("{list} has no action {op:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Editor;

    fn config() -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 7, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "continents", "states": 6 }
        }))
        .unwrap()
    }

    #[test]
    fn each_action_is_one_undoable_edit_that_changes_the_world() {
        let c = config();
        let w = World::build(&c).unwrap();
        let mut e = Editor::new(c.clone());
        let zone = w.zones[0].id.to_string();
        let market = w.economy.markets[0].id.to_string();
        let armed = w
            .society
            .states
            .iter()
            .find(|s| !s.military.is_empty())
            .unwrap();
        let regiment = format!("{}-{}", armed.id, armed.military[0].id);
        let actions = [
            ("zones", zone.as_str(), "name", "The Red Plague"),
            ("markets", market.as_str(), "remove", ""),
            ("regiments", regiment.as_str(), "split", ""),
            ("notes", "", "add", "The Old Road"),
        ];
        for (list, id, op, text) in actions {
            let cmd = command(e.config(), &w, list, id, op, text).unwrap();
            e.run(cmd).unwrap();
        }
        let w2 = World::build(e.config()).unwrap();
        assert_eq!(w2.zones[0].name, "The Red Plague");
        assert_eq!(w2.economy.markets.len(), w.economy.markets.len() - 1);
        let s = w2.society.states.iter().find(|s| s.id == armed.id).unwrap();
        assert_eq!(s.military.len(), armed.military.len() + 1);
        assert_eq!(w2.user_notes[0].name, "The Old Road");
        // The new note can go again.
        let cmd = command(e.config(), &w2, "notes", "custom:1", "remove", "").unwrap();
        e.run(cmd).unwrap();
        assert!(e.config().world.notes.is_empty());
        for _ in 0..5 {
            e.undo().unwrap();
        }
        assert_eq!(e.config(), &c);
        assert!(command(&c, &w, "notes", "zone:1", "remove", "").is_err());
        assert!(command(&c, &w, "rivers", "1", "remove", "").is_err());
    }
}
