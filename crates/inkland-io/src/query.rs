//! `inkland query`: facts about one point of a world.
//!
//! The answer has the cell under the point, its height, biome, climate,
//! feature, owners, the pinned regions that contain the point, and the
//! nearest place, river, and road. Distances are in config units.

use inkland_core::config::{Config, Origin};
use inkland_core::error::{Error, ErrorCode};
use inkland_core::geom::Shape;
use inkland_core::num::round;
use inkland_core::world::World;
use serde_json::{Value, json};

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// The nearest point of a line to `p`: the distance and the index of the
/// segment.
fn nearest_on_line(line: &[[f64; 2]], p: [f64; 2]) -> Option<(f64, usize)> {
    let mut best: Option<(f64, usize)> = None;
    for (i, w) in line.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let ab = [b[0] - a[0], b[1] - a[1]];
        let len2 = ab[0] * ab[0] + ab[1] * ab[1];
        let t = if len2 > 0.0 {
            (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let d = dist(p, [a[0] + ab[0] * t, a[1] + ab[1] * t]);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, i));
        }
    }
    best
}

/// The side of `p` as one looks from `a` to `b`: `left`, `right`, or `on`.
/// `y_up` is `true` when y increases to the north.
fn side_of(a: [f64; 2], b: [f64; 2], p: [f64; 2], y_up: bool) -> &'static str {
    let cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    if cross == 0.0 {
        "on"
    } else if (cross > 0.0) == y_up {
        "left"
    } else {
        "right"
    }
}

/// Parse a point `x,y`.
pub fn parse_point(text: &str) -> Result<[f64; 2], Error> {
    let bad = || {
        Error::new(
            ErrorCode::CliUsage,
            "",
            format!("{text:?} is not a point; give x,y in config units"),
        )
    };
    let (x, y) = text.split_once(',').ok_or_else(bad)?;
    let x: f64 = x.trim().parse().map_err(|_| bad())?;
    let y: f64 = y.trim().parse().map_err(|_| bad())?;
    if !x.is_finite() || !y.is_finite() {
        return Err(bad());
    }
    Ok([x, y])
}

/// The facts about the point `p` of a world.
pub fn query(world: &World, config: &Config, p: [f64; 2]) -> Result<Value, Error> {
    if p[0] < 0.0 || p[1] < 0.0 || p[0] > world.width || p[1] > world.height {
        return Err(Error::new(
            ErrorCode::ValueOutOfRange,
            "",
            format!(
                "the point ({}, {}) is outside the frame {} x {}",
                p[0], p[1], world.width, world.height
            ),
        ));
    }
    let g = &world.grid;
    let soc = &world.society;
    let c = g.find_cell(p[0], p[1]);
    let land = world.features.is_land(c);
    let feature = world
        .features
        .list
        .iter()
        .find(|f| f.id == world.features.of_cell[c]);
    let owner = |id: u32, names: &dyn Fn(u32) -> Option<String>| -> Value {
        if id == 0 {
            Value::Null
        } else {
            json!({ "id": id, "name": names(id) })
        }
    };
    let regions: Vec<&str> = config
        .pins
        .regions
        .iter()
        .filter(|r| Shape::of(r).is_some_and(|s| s.contains(p)))
        .map(|r| r.id.as_str())
        .collect();

    let nearest_place = soc
        .places
        .iter()
        .map(|pl| (dist(pl.at, p), pl))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)))
        .map(|(d, pl)| {
            json!({
                "id": pl.pin.clone().map_or_else(|| Value::from(pl.id), Value::from),
                "name": pl.name,
                "kind": pl.kind,
                "capital": pl.capital,
                "distance": round(d, 3),
            })
        });

    // Left and right of the river, as one looks downstream.
    let y_up = config.frame.origin == Origin::BottomLeft;
    let nearest_river = world
        .rivers
        .iter()
        .filter_map(|r| nearest_on_line(&r.points, p).map(|(d, i)| (d, i, r)))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.2.id.cmp(&b.2.id)))
        .map(|(d, i, r)| {
            let side = if d == 0.0 {
                "on"
            } else {
                side_of(r.points[i], r.points[i + 1], p, y_up)
            };
            let pin = r.course.map(|k| &config.pins.rivers[k]);
            json!({
                "id": r.id,
                "pin": pin.map(|x| x.id.clone()),
                "name": pin.and_then(|x| x.name.clone()),
                "distance": round(d, 3),
                "side": side,
                "length": round(r.length, 3),
            })
        });

    let nearest_road = world
        .roads
        .iter()
        .filter_map(|r| nearest_on_line(&r.points, p).map(|(d, _)| (d, r)))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.id.cmp(&b.1.id)))
        .map(|(d, r)| {
            json!({
                "id": r.id,
                "pin": r.pin.map(|k| config.pins.roads[k].id.clone()),
                "kind": r.kind,
                "distance": round(d, 3),
                "length": round(r.length, 3),
            })
        });

    let culture_name = |id: u32| {
        soc.cultures
            .iter()
            .find(|x| x.id == id)
            .map(|x| x.name.clone())
    };
    let state_name = |id: u32| {
        soc.states
            .iter()
            .find(|x| x.id == id)
            .map(|x| x.name.clone())
    };
    let province_name = |id: u32| {
        soc.provinces
            .iter()
            .find(|x| x.id == id)
            .map(|x| x.name.clone())
    };
    Ok(json!({
        "at": [p[0], p[1]],
        "units": config.units.name,
        "cell": { "id": c, "site": [round(g.points[c][0], 3), round(g.points[c][1], 3)] },
        "land": land,
        "height": world.heights[c],
        "sea_level": world.sea_level,
        "height_above_sea": world.heights[c] - world.sea_level,
        "feature": feature.map(|f| json!({ "id": f.id, "kind": f.kind.name() })),
        "biome": inkland_core::biomes::NAMES[world.biomes[c] as usize],
        "temperature_c": round(world.temperature[c], 3),
        "precipitation": round(world.precipitation[c], 3),
        "regions": regions,
        "culture": owner(soc.cell_culture[c], &culture_name),
        "state": owner(soc.cell_state[c], &state_name),
        "province": owner(soc.cell_province[c], &province_name),
        "nearest_place": nearest_place,
        "nearest_river": nearest_river,
        "nearest_road": nearest_road,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> (World, Config) {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 4, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea" },
            "pins": {
                "points": [{ "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [200, 150] }],
                "regions": [{ "id": "hills", "kind": "hills", "box": [150, 100, 260, 200] }]
            }
        }))
        .unwrap();
        (World::build(&c).unwrap(), c)
    }

    #[test]
    fn a_point_on_a_pinned_place_names_the_place_and_its_owners() {
        let (w, c) = world();
        let q = query(&w, &c, [200.0, 150.0]).unwrap();
        assert_eq!(q["land"], true);
        assert_eq!(q["nearest_place"]["id"], "ostrey");
        assert_eq!(q["nearest_place"]["distance"], 0.0);
        assert_eq!(q["regions"], json!(["hills"]));
        assert!(q["state"]["name"].is_string());
        assert!(q["culture"]["id"].as_u64().unwrap() > 0);
        assert!(q["nearest_river"]["side"].is_string());
        assert!(q["biome"].is_string());
    }

    #[test]
    fn a_point_outside_the_frame_is_an_error() {
        let (w, c) = world();
        assert_eq!(
            query(&w, &c, [401.0, 1.0]).unwrap_err().code,
            ErrorCode::ValueOutOfRange
        );
    }

    #[test]
    fn side_is_left_or_right_as_one_looks_downstream() {
        // A river that flows to +x.
        let (a, b) = ([0.0, 0.0], [10.0, 0.0]);
        // Top-left frame: +y is south, so (5, 3) is on the right.
        assert_eq!(side_of(a, b, [5.0, 3.0], false), "right");
        assert_eq!(side_of(a, b, [5.0, -3.0], false), "left");
        // Bottom-left frame: +y is north, so (5, 3) is on the left.
        assert_eq!(side_of(a, b, [5.0, 3.0], true), "left");
        assert_eq!(side_of(a, b, [5.0, 0.0], true), "on");
    }

    #[test]
    fn parse_point_reads_x_and_y() {
        assert_eq!(parse_point("120.5, 88").unwrap(), [120.5, 88.0]);
        assert!(parse_point("120").is_err());
        assert!(parse_point("a,b").is_err());
    }
}
