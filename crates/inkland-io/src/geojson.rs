//! GeoJSON export of a world.
//!
//! The coordinates are in config units of the frame, not WGS84. The output
//! says so in `crs` and in `inkland`. Properties give ids, names, kinds, and
//! lengths, so a text can quote real facts of the map. All coordinates have
//! 3 decimals.

use inkland_core::config::Config;
use inkland_core::error::{Error, ErrorCode};
use inkland_core::geom::Shape;
use inkland_core::num::round;
use inkland_core::world::World;
use serde_json::{Value, json};

/// The layers of the export, in output order.
pub const LAYERS: [&str; 15] = [
    "coast",
    "rivers",
    "regions",
    "sea_routes",
    "roads",
    "places",
    "markers",
    "borders",
    "zones",
    "markets",
    "trade",
    "regiments",
    "journeys",
    "measurers",
    "deposits",
];

/// The number of points of a circle region.
const CIRCLE_POINTS: usize = 64;

fn pt(p: [f64; 2]) -> Value {
    json!([round(p[0], 3), round(p[1], 3)])
}

fn line(points: &[[f64; 2]]) -> Value {
    Value::Array(points.iter().map(|&p| pt(p)).collect())
}

fn feature(geometry: Value, properties: Value) -> Value {
    json!({ "type": "Feature", "geometry": geometry, "properties": properties })
}

fn signed_area(ring: &[[f64; 2]]) -> f64 {
    ring.windows(2)
        .map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1])
        .sum::<f64>()
        / 2.0
}

fn contains(ring: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    for w in ring.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

/// The MultiPolygon of a set of cells: each outer loop with the holes in
/// it. The cells are on the left of each loop, so a hole turns the other
/// way.
fn cells_geometry(world: &World, inside: &dyn Fn(usize) -> bool) -> Option<Value> {
    let rings: Vec<Vec<[f64; 2]>> = inkland_core::features::outlines(&world.grid, inside)
        .iter()
        .map(|l| l.iter().map(|&v| world.grid.vertices[v as usize]).collect())
        .collect();
    let largest = rings
        .iter()
        .max_by(|a, b| signed_area(a).abs().total_cmp(&signed_area(b).abs()))?;
    let outer_sign = signed_area(largest).signum();
    let (outer, holes): (Vec<_>, Vec<_>) = rings
        .into_iter()
        .partition(|r| signed_area(r).signum() == outer_sign);
    let mut polygons: Vec<Vec<Vec<[f64; 2]>>> = outer.into_iter().map(|r| vec![r]).collect();
    for h in holes {
        if let Some(poly) = polygons.iter_mut().find(|poly| contains(&poly[0], h[0])) {
            poly.push(h);
        }
    }
    Some(json!({
        "type": "MultiPolygon",
        "coordinates": polygons
            .iter()
            .map(|poly| poly.iter().map(|r| line(r)).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
    }))
}

/// Parse a list of layers `a,b,c`. An empty list or `None` gives all layers.
pub fn parse_layers(text: Option<&str>) -> Result<Vec<&'static str>, Error> {
    let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
        return Ok(LAYERS.to_vec());
    };
    let mut out = Vec::new();
    for name in text.split(',').map(str::trim) {
        let Some(&layer) = LAYERS.iter().find(|&&l| l == name) else {
            return Err(Error::new(
                ErrorCode::CliUsage,
                "",
                format!("{name:?} is not a layer; use {}", LAYERS.join(", ")),
            ));
        };
        if !out.contains(&layer) {
            out.push(layer);
        }
    }
    out.sort_by_key(|l| LAYERS.iter().position(|x| x == l));
    Ok(out)
}

/// The outline of a region shape. A circle has `CIRCLE_POINTS` points.
fn outline(shape: &Shape) -> Vec<[f64; 2]> {
    let mut pts = match shape {
        Shape::Polygon(p) => p.clone(),
        Shape::Circle { at, r } => (0..CIRCLE_POINTS)
            .map(|i| {
                let a = 2.0 * std::f64::consts::PI * i as f64 / CIRCLE_POINTS as f64;
                [at[0] + r * libm::cos(a), at[1] + r * libm::sin(a)]
            })
            .collect(),
    };
    if pts.first() != pts.last()
        && let Some(&p) = pts.first()
    {
        pts.push(p);
    }
    pts
}

/// The GeoJSON FeatureCollection of the given layers of a world.
pub fn export(world: &World, config: &Config, layers: &[&str]) -> Value {
    let soc = &world.society;
    let mut features = Vec::new();
    let has = |l: &str| layers.contains(&l);

    if has("coast") {
        for c in &world.coastlines {
            features.push(feature(
                json!({ "type": "LineString", "coordinates": line(&c.points) }),
                json!({ "layer": "coast", "id": c.id, "closed": c.closed }),
            ));
        }
    }
    if has("rivers") {
        for (r, n) in world.rivers.iter().zip(&world.names.rivers) {
            let pin = r.course.map(|k| &config.pins.rivers[k]);
            features.push(feature(
                json!({ "type": "LineString", "coordinates": line(&r.points) }),
                json!({
                    "layer": "rivers",
                    "id": r.id,
                    "pin": pin.map(|p| p.id.clone()),
                    "name": n.name,
                    "type": n.kind,
                    "length": round(r.length, 3),
                    "flux": round(r.flux, 3),
                    "parent": r.parent,
                }),
            ));
        }
    }
    if has("regions") {
        for r in &config.pins.regions {
            let Some(shape) = Shape::of(r) else { continue };
            features.push(feature(
                json!({ "type": "Polygon", "coordinates": [line(&outline(&shape))] }),
                json!({
                    "layer": "regions",
                    "id": r.id,
                    "kind": r.kind,
                    "name": r.name,
                }),
            ));
        }
    }
    if has("sea_routes") {
        for r in &world.sea_routes {
            features.push(feature(
                json!({ "type": "LineString", "coordinates": line(&r.points) }),
                json!({
                    "layer": "sea_routes",
                    "id": r.id,
                    "group": r.group,
                    "name": r.name,
                    "length": round(r.length, 3),
                }),
            ));
        }
    }
    if has("roads") {
        for r in &world.roads {
            features.push(feature(
                json!({ "type": "LineString", "coordinates": line(&r.points) }),
                json!({
                    "layer": "roads",
                    "id": r.id,
                    "pin": r.pin.map(|k| config.pins.roads[k].id.clone()),
                    "kind": r.kind,
                    "group": r.group,
                    "name": r.name,
                    "length": round(r.length, 3),
                    "stops": r.stops.iter().map(|s| json!({
                        "place": s.place,
                        "at_length": round(s.at_length, 3),
                    })).collect::<Vec<_>>(),
                    "crossings": r.crossings.iter().map(|x| json!({
                        "river": x.river,
                        "river_pin": x.river_pin,
                        "at_length": round(x.at_length, 3),
                        "kind": x.kind,
                    })).collect::<Vec<_>>(),
                }),
            ));
        }
    }
    if has("places") {
        let state = |id: u32| {
            soc.states
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.name.clone())
        };
        for p in &soc.places {
            features.push(feature(
                json!({ "type": "Point", "coordinates": pt(p.at) }),
                json!({
                    "layer": "places",
                    "id": p.id,
                    "pin": p.pin,
                    "name": p.name,
                    "kind": p.kind,
                    "group": p.group,
                    "capital": p.capital,
                    "port": p.port,
                    "population": round(p.population, 3),
                    "citadel": p.citadel,
                    "plaza": p.plaza,
                    "walls": p.walls,
                    "shanty": p.shanty,
                    "temple": p.temple,
                    "state": state(p.state),
                    "coa": p.coa,
                }),
            ));
        }
    }
    if has("markers") {
        for m in &world.markers {
            features.push(feature(
                json!({ "type": "Point", "coordinates": pt(m.at) }),
                json!({
                    "layer": "markers",
                    "id": m.id,
                    "type": m.kind,
                    "icon": m.icon,
                    "name": m.name,
                    "legend": m.legend,
                }),
            ));
        }
    }
    if has("borders") {
        let (states, provinces) = inkland_core::borders::state_and_province_edges(world);
        for (kind, edges) in [("state", states), ("province", provinces)] {
            for (i, l) in inkland_core::borders::chain_edges(&edges)
                .iter()
                .enumerate()
            {
                let pts: Vec<[f64; 2]> =
                    l.iter().map(|&v| world.grid.vertices[v as usize]).collect();
                features.push(feature(
                    json!({ "type": "LineString", "coordinates": line(&pts) }),
                    json!({ "layer": "borders", "kind": kind, "id": i + 1 }),
                ));
            }
        }
    }

    if has("zones") {
        for z in &world.zones {
            let cells: std::collections::BTreeSet<usize> = z.cells.iter().copied().collect();
            if let Some(g) = cells_geometry(world, &|c| cells.contains(&c)) {
                features.push(feature(
                    g,
                    json!({ "layer": "zones", "id": z.id, "name": z.name, "type": z.kind,
                            "cells": z.cells.len(), "note": z.note }),
                ));
            }
        }
    }
    let eco = &world.economy;
    if has("markets") {
        for m in &eco.markets {
            if let Some(g) = cells_geometry(world, &|c| eco.cell_market[c] == m.id) {
                let center = world.society.places.get(m.center as usize - 1);
                features.push(feature(
                    g,
                    json!({ "layer": "markets", "id": m.id, "center": m.center,
                            "name": center.map(|p| p.name.clone()), "color": m.color }),
                ));
            }
        }
    }
    if has("trade") {
        for (i, l) in eco.traffic.iter().enumerate() {
            let pts = [world.grid.points[l.cells[0]], world.grid.points[l.cells[1]]];
            let units: f64 = l.goods.iter().map(|g| g.1).sum();
            features.push(feature(
                json!({ "type": "LineString", "coordinates": line(&pts) }),
                json!({ "layer": "trade", "id": i + 1, "cells": l.cells, "water": l.water,
                        "units": round(units, 3), "goods": l.goods.len() }),
            ));
        }
    }
    if has("regiments") {
        for st in &world.society.states {
            for r in &st.military {
                features.push(feature(
                    json!({ "type": "Point", "coordinates": pt(r.at) }),
                    json!({ "layer": "regiments", "state": r.state, "id": r.id,
                            "name": r.name, "naval": r.naval, "total": round(r.total, 3),
                            "icon": r.icon }),
                ));
            }
        }
    }
    if has("journeys") {
        for j in &world.journeys {
            for sg in &j.segments {
                let pts: Vec<[f64; 2]> = sg.points.iter().map(|p| p.at).collect();
                if pts.len() < 2 {
                    continue;
                }
                features.push(feature(
                    json!({ "type": "LineString", "coordinates": line(&pts) }),
                    json!({ "layer": "journeys", "journey": j.id, "id": sg.id,
                            "name": sg.name, "transport": sg.transport, "color": j.color,
                            "length": round(sg.distance, 3) }),
                ));
            }
        }
    }
    if has("measurers") {
        for (i, m) in world.measurers.iter().enumerate() {
            features.push(feature(
                json!({ "type": "LineString", "coordinates": line(&m.points) }),
                json!({ "layer": "measurers", "id": i + 1, "type": m.kind.name(),
                        "length": round(m.length(), 3) }),
            ));
        }
    }
    if has("deposits") {
        for (c, &g) in world.society.cells.good.iter().enumerate() {
            if g == 0 {
                continue;
            }
            let good = eco.goods.get(g as usize - 1);
            features.push(feature(
                json!({ "type": "Point", "coordinates": pt(world.grid.points[c]) }),
                json!({ "layer": "deposits", "cell": c, "good": g,
                        "name": good.map(|x| x.name) }),
            ));
        }
    }

    json!({
        "type": "FeatureCollection",
        "crs": { "type": "name", "properties": { "name": "inkland:local" } },
        "inkland": {
            "note": "Coordinates are in config units of the frame, not WGS84.",
            "units": config.units.name,
            "frame": config.frame,
            "world_hash": crate::config::world_hash(config),
            "layers": layers,
        },
        "features": features,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> (World, Config) {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 4, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea" },
            "pins": {
                "points": [
                    { "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [150, 150] },
                    { "id": "harrow", "kind": "town", "at": [250, 150] }
                ],
                "regions": [{ "id": "mere", "kind": "lake", "name": "The Mere",
                              "circle": { "at": [200, 220], "r": 15 } }],
                "roads": [{ "id": "kings-road", "from": "ostrey", "to": "harrow" }]
            }
        }))
        .unwrap();
        (World::build(&c).unwrap(), c)
    }

    #[test]
    fn export_has_each_layer_with_ids_and_rounded_points() {
        let (w, c) = world();
        let v = export(&w, &c, &LAYERS);
        assert_eq!(v["type"], "FeatureCollection");
        assert_eq!(v["crs"]["properties"]["name"], "inkland:local");
        let feats = v["features"].as_array().unwrap();
        for layer in LAYERS {
            assert!(
                feats.iter().any(|f| f["properties"]["layer"] == layer),
                "{layer}"
            );
        }
        let road = feats
            .iter()
            .find(|f| f["properties"]["pin"] == "kings-road")
            .unwrap();
        assert_eq!(road["geometry"]["type"], "LineString");
        assert!(road["properties"]["length"].as_f64().unwrap() >= 100.0);
        let mere = feats
            .iter()
            .find(|f| f["properties"]["id"] == "mere")
            .unwrap();
        let ring = mere["geometry"]["coordinates"][0].as_array().unwrap();
        assert_eq!(ring.len(), CIRCLE_POINTS + 1);
        assert_eq!(ring.first(), ring.last());
        let x = ring[3][0].as_f64().unwrap();
        assert_eq!(x, round(x, 3));
        let place = feats
            .iter()
            .find(|f| f["properties"]["pin"] == "ostrey")
            .unwrap();
        assert_eq!(place["geometry"]["coordinates"], json!([150.0, 150.0]));
        assert!(place["properties"]["coa"]["t1"].is_string());
    }

    #[test]
    fn export_keeps_only_the_asked_layers() {
        let (w, c) = world();
        let layers = parse_layers(Some("roads, coast")).unwrap();
        assert_eq!(layers, ["coast", "roads"]);
        let v = export(&w, &c, &layers);
        assert!(v["features"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["properties"]["layer"] == "coast" || f["properties"]["layer"] == "roads"));
        assert_eq!(
            parse_layers(Some("roads,lakes")).unwrap_err().code,
            ErrorCode::CliUsage
        );
        assert_eq!(parse_layers(None).unwrap(), LAYERS);
    }

    #[test]
    fn markets_and_zones_are_closed_multipolygons_of_their_cells() {
        let (w, c) = world();
        let v = export(&w, &c, &["markets", "zones"]);
        let feats = v["features"].as_array().unwrap();
        let markets = feats
            .iter()
            .filter(|f| f["properties"]["layer"] == "markets")
            .count();
        assert_eq!(markets, w.economy.markets.len());
        for f in feats {
            assert_eq!(f["geometry"]["type"], "MultiPolygon");
            for poly in f["geometry"]["coordinates"].as_array().unwrap() {
                for ring in poly.as_array().unwrap() {
                    let r = ring.as_array().unwrap();
                    assert!(r.len() >= 4);
                    assert_eq!(r.first(), r.last());
                }
            }
        }
    }

    #[test]
    fn a_hole_goes_into_its_outer_ring() {
        let outer = [
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0],
        ];
        let hole = [[2.0, 2.0], [2.0, 4.0], [4.0, 4.0], [4.0, 2.0], [2.0, 2.0]];
        assert!(signed_area(&outer) > 0.0 && signed_area(&hole) < 0.0);
        assert!(contains(&outer, hole[0]));
        assert!(!contains(&hole, [9.0, 9.0]));
    }
}
