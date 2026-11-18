//! The edit forms of the data dialogs: zones, markets, regiments,
//! journeys, and notes. Each action of a dialog is one `insert` or
//! `remove` command on a list of the world part of the config, so undo
//! gives back the config exactly.

use inkland::Command;
use inkland_core::config::Config;
use inkland_core::world::World;
use serde_json::{Value, json};
use std::collections::BTreeMap;

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
            let r = regiment(world, id).ok_or_else(missing)?;
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
        ("measurers", "remove") => {
            let k: usize = id.parse().map_err(|_| missing())?;
            if k >= world.measurers.len() {
                return Err(missing());
            }
            // The default ruler is the first measurer when the config keeps
            // it; the others are the measurers of the config.
            let default = config.world.default_ruler && k == 0;
            if default {
                Ok(Command::Set {
                    path: "/world/default_ruler".to_owned(),
                    value: json!(false),
                })
            } else {
                let i = k - usize::from(config.world.default_ruler);
                Ok(Command::Remove {
                    path: format!("/world/measurers/{i}"),
                })
            }
        }
        ("regiments", "units") => {
            let r = regiment(world, id).ok_or_else(missing)?;
            let units = parse_units(text)?;
            if units.is_empty() {
                return Err("give the troops as unit=people".to_owned());
            }
            Ok(append(
                config,
                "regiments",
                json!({ "at": r.at, "units": units }),
            ))
        }
        ("finance", "tariffs") => {
            let index: usize = id.parse().map_err(|_| missing())?;
            let (at, k) = finance_entry(config, world, index).ok_or_else(missing)?;
            let t = text.trim();
            let rate = if t.eq_ignore_ascii_case("embargo") {
                json!("embargo")
            } else {
                let v: f64 = t
                    .parse()
                    .map_err(|_| format!("{text:?} is not a number or embargo"))?;
                json!(v)
            };
            let tariffs = json!([{ "rate": rate }]);
            Ok(match k {
                Some(k) => Command::Set {
                    path: format!("/world/finance/{k}/tariffs"),
                    value: tariffs,
                },
                None => append(config, "finance", json!({ "at": at, "tariffs": tariffs })),
            })
        }
        ("finance", field @ ("sales_tax" | "poll_tax" | "market_fee" | "military")) => {
            let index: usize = id.parse().map_err(|_| missing())?;
            let v: f64 = text
                .trim()
                .parse()
                .map_err(|_| format!("{text:?} is not a number"))?;
            let (at, k) = finance_entry(config, world, index).ok_or_else(missing)?;
            match (k, field) {
                (Some(k), "military") if config.world.finance[k].budget.is_some() => {
                    Ok(Command::Set {
                        path: format!("/world/finance/{k}/budget/military"),
                        value: json!(v),
                    })
                }
                (Some(k), "military") => Ok(Command::Set {
                    path: format!("/world/finance/{k}/budget"),
                    value: json!({ "military": v }),
                }),
                (Some(k), f) => Ok(Command::Set {
                    path: format!("/world/finance/{k}/{f}"),
                    value: json!(v),
                }),
                (None, "military") => Ok(append(
                    config,
                    "finance",
                    json!({ "at": at, "budget": { "military": v } }),
                )),
                (None, f) => Ok(append(config, "finance", json!({ "at": at, f: v }))),
            }
        }
        _ => Err(format!("{list} has no action {op:?}")),
    }
}

/// The point at the center of the state `index`, and the finance entry of
/// the config at that point, if one exists.
fn finance_entry(
    config: &Config,
    world: &World,
    index: usize,
) -> Option<([f64; 2], Option<usize>)> {
    let state = world.society.states.get(index)?;
    let at = world.grid.points[state.center];
    let at = [crate::model::round2(at[0]), crate::model::round2(at[1])];
    let k = config
        .world
        .finance
        .iter()
        .position(|f| f.at == Some(at) && f.pin.is_none());
    Some((at, k))
}

/// The regiment of the dialog id `state-regiment`.
fn regiment<'a>(world: &'a World, id: &str) -> Option<&'a inkland_core::military::Regiment> {
    let (state, regiment) = id.split_once('-')?;
    world
        .society
        .states
        .iter()
        .filter(|s| s.id.to_string() == state)
        .flat_map(|s| &s.military)
        .find(|r| r.id.to_string() == regiment)
}

/// The troops of the text `Infantry=500, Archers=200`, by unit name.
fn parse_units(text: &str) -> Result<BTreeMap<String, f64>, String> {
    let mut units = BTreeMap::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (name, people) = part
            .split_once('=')
            .ok_or_else(|| format!("{part:?} is not unit=people"))?;
        let people: f64 = people
            .trim()
            .parse()
            .map_err(|_| format!("{people:?} is not a number"))?;
        units.insert(name.trim().to_owned(), people);
    }
    Ok(units)
}

/// A tool that adds or moves an item with points on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTool {
    /// A click adds a market at the nearest place.
    Market,
    /// A click moves the market of the dialog id to the nearest place.
    MoveMarket,
    /// A click adds a regiment of the state at the point.
    Regiment,
    /// A click moves the regiment of the dialog id.
    MoveRegiment,
    /// A click puts a legend box with its top left corner at the point.
    Legend,
    /// A line of points makes a journey through the stops.
    Journey,
    /// A line of points makes a measurer.
    Measurer,
    /// An outline makes a zone.
    Zone,
}

impl DrawTool {
    /// The tool of the id of a Tools tab button or a dialog button.
    pub fn from_id(id: &str) -> Option<DrawTool> {
        Some(match id {
            "add-market" => DrawTool::Market,
            "move-market" => DrawTool::MoveMarket,
            "add-regiment" => DrawTool::Regiment,
            "move-regiment" => DrawTool::MoveRegiment,
            "add-legend" => DrawTool::Legend,
            "add-journey" => DrawTool::Journey,
            "add-measurer" => DrawTool::Measurer,
            "add-zone" => DrawTool::Zone,
            _ => return None,
        })
    }

    /// The id of the tool.
    pub fn id(self) -> &'static str {
        match self {
            DrawTool::Market => "add-market",
            DrawTool::MoveMarket => "move-market",
            DrawTool::Regiment => "add-regiment",
            DrawTool::MoveRegiment => "move-regiment",
            DrawTool::Legend => "add-legend",
            DrawTool::Journey => "add-journey",
            DrawTool::Measurer => "add-measurer",
            DrawTool::Zone => "add-zone",
        }
    }

    /// The number of points that the tool needs. A tool of one point
    /// finishes on the click.
    pub fn min_points(self) -> usize {
        match self {
            DrawTool::Journey | DrawTool::Measurer => 2,
            DrawTool::Zone => 3,
            _ => 1,
        }
    }

    /// The text of the status bar while the tool is on.
    pub fn hint(self) -> &'static str {
        match self {
            DrawTool::Market => "click a burg to make it the center of a new market",
            DrawTool::MoveMarket => "click the burg that is the new center of the market",
            DrawTool::Regiment => "click a point in a state to add a regiment there",
            DrawTool::MoveRegiment => "click the new position of the regiment",
            DrawTool::Legend => "click the top left corner of the legend box",
            DrawTool::Journey => "click the stops, then double click or press Enter",
            DrawTool::Measurer => "click the points, then double click or press Enter",
            DrawTool::Zone => "click the outline, then double click or press Enter",
        }
    }
}

/// The command of a draw tool with the points that the user clicked. `id`
/// is the item to move, and `text` is the field of the tool: the name of a
/// zone or a journey, the troops of a regiment, the kind of a measurer, or
/// the title and items of a legend box.
pub fn draw(
    config: &Config,
    world: &World,
    tool: DrawTool,
    id: &str,
    points: &[[f64; 2]],
    text: &str,
) -> Result<Command, String> {
    let points: Vec<[f64; 2]> = points
        .iter()
        .map(|p| [crate::model::round2(p[0]), crate::model::round2(p[1])])
        .collect();
    if points.len() < tool.min_points() {
        return Err(format!(
            "{} needs {} points or more",
            tool.id(),
            tool.min_points()
        ));
    }
    let at = points[0];
    let text = text.trim();
    let missing = || format!("there is no item {id:?} to move");
    match tool {
        DrawTool::Market => Ok(append(config, "markets", json!({ "at": at }))),
        DrawTool::MoveMarket => {
            let m = world
                .economy
                .markets
                .iter()
                .find(|m| m.id.to_string() == id)
                .ok_or_else(missing)?;
            let from = world
                .society
                .places
                .get(m.center as usize - 1)
                .ok_or_else(missing)?
                .at;
            Ok(append(config, "markets", json!({ "at": from, "to": at })))
        }
        DrawTool::Regiment => {
            let mut units = parse_units(text)?;
            if units.is_empty() {
                let unit = world
                    .military_units
                    .first()
                    .ok_or("the world has no military units")?;
                units.insert(unit.name.clone(), 500.0);
            }
            Ok(append(
                config,
                "regiments",
                json!({ "at": at, "add": true, "units": units }),
            ))
        }
        DrawTool::MoveRegiment => {
            let r = regiment(world, id).ok_or_else(missing)?;
            Ok(append(config, "regiments", json!({ "at": r.at, "to": at })))
        }
        DrawTool::Legend => {
            // The text is `Title; #rrggbb text; #rrggbb text`.
            let mut parts = text.split(';').map(str::trim);
            let name = parts.next().filter(|n| !n.is_empty()).unwrap_or("Legend");
            let mut items = Vec::new();
            for part in parts.filter(|p| !p.is_empty()) {
                let (color, label) = part.split_once(' ').unwrap_or((part, ""));
                if !(color.len() == 7 && color.starts_with('#')) {
                    return Err(format!("{part:?} does not start with a color #rrggbb"));
                }
                items.push(json!({ "color": color, "text": label.trim() }));
            }
            Ok(append(
                config,
                "legends",
                json!({ "name": name, "items": items, "at": at }),
            ))
        }
        DrawTool::Journey => {
            let mut value = json!({ "stops": points });
            if !text.is_empty() {
                value["name"] = json!(text);
            }
            Ok(append(config, "journeys", value))
        }
        DrawTool::Measurer => {
            let kind = if text.is_empty() { "ruler" } else { text };
            let kinds = ["ruler", "opisometer", "route-opisometer", "planimeter"];
            if !kinds.contains(&kind) {
                return Err(format!("{kind:?} is not one of {}", kinds.join(", ")));
            }
            Ok(append(
                config,
                "measurers",
                json!({ "kind": kind, "points": points }),
            ))
        }
        DrawTool::Zone => {
            // The text is `Name` or `Name; Type`.
            let (name, kind) = text.split_once(';').unwrap_or((text, ""));
            let name = if name.trim().is_empty() {
                "New zone"
            } else {
                name.trim()
            };
            let kind = if kind.trim().is_empty() {
                "Custom"
            } else {
                kind.trim()
            };
            Ok(append(
                config,
                "zones",
                json!({ "polygon": points, "name": name, "kind": kind }),
            ))
        }
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

    #[test]
    fn the_finance_fields_and_the_measurers_are_undoable_edits() {
        let c = config();
        let w = World::build(&c).unwrap();
        let mut e = Editor::new(c.clone());
        for (field, text) in [("sales_tax", "0.25"), ("military", "60"), ("poll_tax", "0")] {
            let cmd = command(e.config(), &w, "finance", "0", field, text).unwrap();
            e.run(cmd).unwrap();
        }
        assert_eq!(e.config().world.finance.len(), 1, "one entry for the state");
        let w2 = World::build(e.config()).unwrap();
        let s = &w2.society.states[0];
        assert_eq!(s.economy.sales_tax, 0.25);
        assert_eq!(s.economy.poll_tax, 0.0);
        assert_eq!(
            s.economy
                .budget
                .of(inkland_core::economy::finance::Expense::Military),
            60.0
        );
        assert!(command(e.config(), &w2, "finance", "0", "sales_tax", "much").is_err());
        let cmd = command(e.config(), &w2, "measurers", "0", "remove", "").unwrap();
        e.run(cmd).unwrap();
        assert!(World::build(e.config()).unwrap().measurers.is_empty());
        for _ in 0..4 {
            e.undo().unwrap();
        }
        assert_eq!(e.config(), &c);
    }

    #[test]
    fn each_draw_tool_is_one_undoable_edit_that_adds_or_moves_an_item() {
        let c = config();
        let w = World::build(&c).unwrap();
        let mut e = Editor::new(c.clone());
        let soc = &w.society;
        let centers: Vec<u32> = w.economy.markets.iter().map(|m| m.center).collect();
        let free: Vec<[f64; 2]> = soc
            .places
            .iter()
            .enumerate()
            .filter(|(i, _)| !centers.contains(&(*i as u32 + 1)))
            .map(|(_, p)| p.at)
            .collect();
        let state = soc.states.iter().find(|s| s.id != 0).unwrap();
        let home = w.grid.points[state.center];
        let around = [
            [home[0] - 15.0, home[1] - 15.0],
            [home[0] + 15.0, home[1] - 15.0],
            [home[0], home[1] + 15.0],
        ];
        // Two burgs of one state, so a land path joins them.
        let stops: Vec<[f64; 2]> = soc
            .places
            .iter()
            .map(|p| p.at)
            .filter(|p| soc.cell_state[w.grid.find_cell(p[0], p[1])] == state.id)
            .take(2)
            .collect();
        assert_eq!(stops.len(), 2);
        let draws: [(DrawTool, &[[f64; 2]], &str); 6] = [
            (DrawTool::Market, &[free[0]], ""),
            (DrawTool::Regiment, &[home], ""),
            (
                DrawTool::Legend,
                &[[10.0, 10.0]],
                "Roads; #aa0000 Kings road; #0000aa Sea lane",
            ),
            (DrawTool::Journey, &stops, "The Long Walk"),
            (
                DrawTool::Measurer,
                &[[10.0, 10.0], [90.0, 40.0]],
                "opisometer",
            ),
            (DrawTool::Zone, &around, "The Red Plague; Disease"),
        ];
        for (tool, points, text) in draws {
            let cmd = draw(e.config(), &w, tool, "", points, text).unwrap();
            e.run(cmd).unwrap();
        }
        let w2 = World::build(e.config()).unwrap();
        let center_at = |w: &World, m: &inkland_core::economy::markets::Market| {
            w.society.places[m.center as usize - 1].at
        };
        assert!(
            w2.economy
                .markets
                .iter()
                .any(|m| center_at(&w2, m) == free[0])
        );
        // Other edits change the generated regiments, so check the new one
        // in a world with only that edit.
        let mut alone = Editor::new(c.clone());
        alone
            .run(draw(&c, &w, DrawTool::Regiment, "", &[home], "").unwrap())
            .unwrap();
        let count = |w: &World| {
            w.society
                .states
                .iter()
                .find(|s| s.id == state.id)
                .map_or(0, |s| s.military.len())
        };
        let w1 = World::build(alone.config()).unwrap();
        assert_eq!(count(&w1), count(&w) + 1);
        let legend = w2.legends.last().unwrap();
        assert_eq!(legend.name, "Roads");
        assert_eq!(legend.items.len(), 2);
        assert!(w2.journeys.iter().any(|j| j.name == "The Long Walk"));
        assert_eq!(w2.measurers.len(), w.measurers.len() + 1);
        let zone = w2
            .zones
            .iter()
            .find(|z| z.name == "The Red Plague")
            .unwrap();
        assert_eq!(zone.kind, "Disease");

        // Move a market and a regiment.
        let market = w2.economy.markets[0].id.to_string();
        let to = *free.last().unwrap();
        let cmd = draw(e.config(), &w2, DrawTool::MoveMarket, &market, &[to], "").unwrap();
        e.run(cmd).unwrap();
        let armed = w2
            .society
            .states
            .iter()
            .find(|s| !s.military.is_empty())
            .unwrap();
        let r = &armed.military[0];
        let id = format!("{}-{}", armed.id, r.id);
        let to = [r.at[0] + 5.0, r.at[1] + 5.0];
        let cmd = draw(e.config(), &w2, DrawTool::MoveRegiment, &id, &[to], "").unwrap();
        e.run(cmd).unwrap();
        let w3 = World::build(e.config()).unwrap();
        let moved = w3
            .society
            .states
            .iter()
            .find(|s| s.id == armed.id)
            .and_then(|s| s.military.iter().find(|m| m.id == r.id))
            .unwrap();
        assert_ne!(moved.at, r.at);
        assert_ne!(w3.economy.markets, w2.economy.markets);

        for _ in 0..8 {
            e.undo().unwrap();
        }
        assert_eq!(e.config(), &c);
        // A line tool needs its points, and the fields must parse.
        assert!(draw(&c, &w, DrawTool::Zone, "", &around[..2], "").is_err());
        assert!(draw(&c, &w, DrawTool::Measurer, "", &around, "compass").is_err());
        assert!(draw(&c, &w, DrawTool::Legend, "", &[home], "Key; red Forest").is_err());
        assert!(draw(&c, &w, DrawTool::Regiment, "", &[home], "Infantry").is_err());
        assert!(draw(&c, &w, DrawTool::MoveMarket, "999", &[home], "").is_err());
        for t in ["add-market", "move-regiment", "add-zone"] {
            assert_eq!(DrawTool::from_id(t).unwrap().id(), t);
        }
    }

    #[test]
    fn the_tariffs_and_the_troops_of_a_regiment_are_undoable_edits() {
        let c = config();
        let w = World::build(&c).unwrap();
        let mut e = Editor::new(c.clone());
        let cmd = command(e.config(), &w, "finance", "1", "tariffs", "embargo").unwrap();
        e.run(cmd).unwrap();
        let cmd = command(e.config(), &w, "finance", "1", "tariffs", "0.3").unwrap();
        e.run(cmd).unwrap();
        assert_eq!(
            e.config().world.finance.len(),
            1,
            "the second edit sets the same entry"
        );
        let tariffs = e.config().world.finance[0].tariffs.as_ref().unwrap();
        assert_eq!(tariffs[0].rate, inkland_core::config::TariffRate::Rate(0.3));
        World::build(e.config()).unwrap();
        assert!(command(e.config(), &w, "finance", "1", "tariffs", "lots").is_err());

        let armed = w
            .society
            .states
            .iter()
            .find(|s| !s.military.is_empty())
            .unwrap();
        let r = &armed.military[0];
        let unit = &w.military_units[0].name;
        let id = format!("{}-{}", armed.id, r.id);
        let cmd = command(
            e.config(),
            &w,
            "regiments",
            &id,
            "units",
            &format!("{unit}=1234"),
        )
        .unwrap();
        e.run(cmd).unwrap();
        let w2 = World::build(e.config()).unwrap();
        let r2 = w2
            .society
            .states
            .iter()
            .find(|s| s.id == armed.id)
            .and_then(|s| s.military.iter().find(|m| m.id == r.id))
            .unwrap();
        assert_ne!(r2.units, r.units);
        assert!(command(e.config(), &w, "regiments", &id, "units", "").is_err());
        for _ in 0..3 {
            e.undo().unwrap();
        }
        assert_eq!(e.config(), &c);
    }
}
