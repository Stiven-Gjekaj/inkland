//! The report that `generate` writes as `report.json` and prints.

use inkland_core::config::Config;
use inkland_core::error::Warning;
use serde::Serialize;

/// The name and the version of the tool.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tool {
    pub name: &'static str,
    pub version: &'static str,
}

impl Tool {
    pub fn current() -> Self {
        Tool {
            name: "inkland",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

/// The result of one check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub pass: bool,
}

/// The counts of the pins in the config.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PinCounts {
    pub points: usize,
    pub regions: usize,
    pub rivers: usize,
    pub roads: usize,
}

/// The report of `generate --dry-run`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DryRunReport {
    pub tool: Tool,
    pub config_hash: String,
    pub seed: u64,
    pub units: String,
    pub dry_run: bool,
    pub variants: Vec<String>,
    pub pins: PinCounts,
    pub checks: Vec<Check>,
    pub warnings: Vec<Warning>,
    /// The config after the flags are applied.
    pub effective_config: Config,
}

/// The report of one generated variant.
pub fn variant_report(
    world: &inkland_core::world::World,
    config: &Config,
    variant: &str,
    config_hash: &str,
) -> serde_json::Value {
    use inkland_core::features::FeatureKind;
    use inkland_core::num::round;
    let count = |k: FeatureKind| world.features.list.iter().filter(|f| f.kind == k).count();
    serde_json::json!({
        "tool": Tool::current(),
        "config_hash": config_hash,
        "seed": config.seed,
        "variant": variant,
        "units": config.units.name,
        "dry_run": false,
        "world": {
            "cells": world.grid.len(),
            "land_fraction": round(world.land_fraction(), 3),
            "features": {
                "ocean": count(FeatureKind::Ocean),
                "lake": count(FeatureKind::Lake),
                "island": count(FeatureKind::Island),
            },
            "coastlines": world.coastlines.len(),
            "rivers": world.rivers.len(),
            "cultures": world.society.cultures.len(),
            "states": world.society.states.iter().map(|st| serde_json::json!({
                "id": st.id,
                "name": st.name,
                "capital": world.society.places[st.capital as usize - 1].name,
                "cells": world.society.cell_state.iter().filter(|&&x| x == st.id).count(),
            })).collect::<Vec<_>>(),
            "provinces": world.society.provinces.len(),
            "places": world.society.places.len(),
        },
        "pins": {
            "points": point_pins(world, config),
            "regions": region_pins(world, config),
            "rivers": river_pins(world, config),
            "roads": [],
        },
        "labels": { "placed": 0, "unplaced": [] },
        "checks": [],
        "warnings": [],
    })
}

/// The report of each point pin: where it was asked for, where it is, and
/// what its cell is. `error` is the distance between the two, in units.
fn point_pins(world: &inkland_core::world::World, config: &Config) -> Vec<serde_json::Value> {
    use inkland_core::num::round;
    config
        .pins
        .points
        .iter()
        .zip(&world.pin_cells)
        .map(|(pin, &cell)| {
            let placed = world.grid.points[cell];
            let error = ((placed[0] - pin.at[0]).powi(2) + (placed[1] - pin.at[1]).powi(2)).sqrt();
            let soc = &world.society;
            let place = soc
                .places
                .iter()
                .find(|p| p.pin.as_deref() == Some(pin.id.as_str()));
            let name_of = |list: Option<String>| {
                list.map_or(serde_json::Value::Null, serde_json::Value::from)
            };
            let state = place
                .and_then(|p| soc.states.iter().find(|s| s.id == p.state))
                .map(|s| s.name.clone());
            let province = place
                .and_then(|p| soc.provinces.iter().find(|s| s.id == p.province))
                .map(|s| s.name.clone());
            let culture = place
                .and_then(|p| soc.cultures.iter().find(|s| s.id == p.culture))
                .map(|s| s.name.clone());
            let mut v = serde_json::json!({
                "name": place.map(|p| p.name.clone()),
                "state": name_of(state),
                "province": name_of(province),
                "culture": name_of(culture),
                "id": pin.id,
                "requested": [round(pin.at[0], 3), round(pin.at[1], 3)],
                "placed": [round(placed[0], 3), round(placed[1], 3)],
                "error": round(error, 3),
                "height": world.heights[cell],
                "land": world.features.is_land(cell),
            });
            if pin.port {
                v["port"] = serde_json::Value::from(
                    world
                        .grid
                        .neighbors(cell)
                        .any(|m| !world.features.is_land(m)),
                );
            }
            v
        })
        .collect()
}

/// The report of each region pin: its cells and their height range.
fn region_pins(world: &inkland_core::world::World, config: &Config) -> Vec<serde_json::Value> {
    use inkland_core::fit::effective_range;
    use inkland_core::geom::Shape;
    config
        .pins
        .regions
        .iter()
        .map(|r| {
            let shape = Shape::of(r);
            let cells: Vec<usize> = (0..world.grid.len())
                .filter(|&c| {
                    shape
                        .as_ref()
                        .is_some_and(|s| s.contains(world.grid.points[c]))
                })
                .collect();
            let [lo, hi] = effective_range(r, world.sea_level);
            let min = cells
                .iter()
                .map(|&c| world.heights[c])
                .fold(f64::INFINITY, f64::min);
            let max = cells
                .iter()
                .map(|&c| world.heights[c])
                .fold(f64::NEG_INFINITY, f64::max);
            let in_range = cells.iter().all(|&c| {
                let h = world.heights[c];
                h >= lo.ceil() && h <= hi.floor().max(lo.ceil())
            });
            serde_json::json!({
                "id": r.id,
                "kind": serde_json::to_value(r.kind).expect("a kind converts to JSON"),
                "cells": cells.len(),
                "range": [lo, hi],
                "min_height": if cells.is_empty() { serde_json::Value::Null } else { min.into() },
                "max_height": if cells.is_empty() { serde_json::Value::Null } else { max.into() },
                "in_range": in_range,
            })
        })
        .collect()
}

/// The report of each pinned river: its anchors, its course, and its
/// length in units.
fn river_pins(world: &inkland_core::world::World, config: &Config) -> Vec<serde_json::Value> {
    use inkland_core::config::Anchor;
    use inkland_core::geom::Shape;
    use inkland_core::num::round;
    use inkland_core::rivers::{Mouth, NONE};
    let place_at = |id: &str| config.pins.points.iter().find(|p| p.id == id).map(|p| p.at);
    config
        .pins
        .rivers
        .iter()
        .enumerate()
        .map(|(k, pin)| {
            let Some(river) = world.rivers.iter().find(|r| r.course == Some(k)) else {
                return serde_json::json!({ "id": pin.id, "found": false });
            };
            // The distance from each point or place anchor to the nearest
            // point of the line, and whether the course enters each region.
            let mut max_dev: f64 = 0.0;
            let mut through_ok = true;
            for a in &pin.through {
                match a {
                    Anchor::Point(p) => max_dev = max_dev.max(nearest(&river.points, *p)),
                    Anchor::Place(r) => {
                        if let Some(p) = place_at(&r.place) {
                            max_dev = max_dev.max(nearest(&river.points, p));
                        }
                    }
                    Anchor::Region(r) => {
                        let shape = config
                            .pins
                            .regions
                            .iter()
                            .find(|g| g.id == r.region)
                            .and_then(Shape::of);
                        let enters = shape.is_some_and(|s| {
                            river
                                .cells
                                .iter()
                                .any(|&c| s.contains(world.grid.points[c]))
                        });
                        through_ok &= enters;
                    }
                }
            }
            through_ok &= max_dev == 0.0;
            let e = &world.drainage.elevation;
            let mut downhill = river.cells.windows(2).all(|w| e[w[1]] < e[w[0]]);
            let last = *river.cells.last().expect("a river has cells");
            let r = world.drainage.receiver[last];
            if r != NONE {
                downhill &= e[r] < e[last];
            }
            let mouth = match river.mouth {
                Mouth::Water(_) => "water",
                Mouth::Join(_) => "join",
                Mouth::Sink => "sink",
            };
            let end = river.points[river.points.len() - 1];
            serde_json::json!({
                "id": pin.id,
                "found": true,
                "source": [round(river.points[0][0], 3), round(river.points[0][1], 3)],
                "mouth": { "kind": mouth, "at": [round(end[0], 3), round(end[1], 3)] },
                "through_ok": through_ok,
                "max_deviation": round(max_dev, 3),
                "downhill": downhill,
                "length": round(river.length, 3),
                "cells": river.cells.len(),
            })
        })
        .collect()
}

fn nearest(points: &[[f64; 2]], p: [f64; 2]) -> f64 {
    points
        .iter()
        .map(|q| ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt())
        .fold(f64::INFINITY, f64::min)
}
