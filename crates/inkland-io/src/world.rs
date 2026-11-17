//! `world.json`: the base world, in config units.
//!
//! The file holds the first grid (`base`: its lattice, sites, and
//! heights) and the second grid (`grid`), with the first-grid cell of each
//! cell. All other cell data and cell indices are for the second grid: the
//! heights, the features, the rivers, and the society. All coordinates have
//! 3 decimals. It also holds the part of the config that makes the world,
//! so other commands can make the same world again from the file.

use inkland_core::config::Config;
use inkland_core::num::round;
use inkland_core::world::World;
use serde_json::{Value, json};

/// The format name and version of `world.json`.
pub const FORMAT: &str = "inkland-world";
pub const FORMAT_VERSION: u32 = 2;

fn point(p: [f64; 2]) -> Value {
    json!([round(p[0], 3), round(p[1], 3)])
}

/// The JSON value of a world. `world_hash` is the hash of the part of the
/// config that makes the world, so the value does not change when only an
/// overlay, a variant, or a render option changes.
pub fn world_value(world: &World, config: &Config) -> Value {
    let g = &world.grid;
    let mut v = json!({
        "format": FORMAT,
        "format_version": FORMAT_VERSION,
        "tool": { "name": "inkland", "version": env!("CARGO_PKG_VERSION") },
        "world_hash": crate::config::world_hash(config),
        "config": crate::config::world_config_value(config),
        "seed": world.seed,
        "units": config.units.name,
        "frame": config.frame,
        "sea_level": world.sea_level,
        "coordinates": {
            "map_size": round(world.coordinates.map_size, 3),
            "latitude": round(world.coordinates.latitude, 3),
            "longitude": round(world.coordinates.longitude, 3),
            "lat_n": round(world.coordinates.lat_n, 3),
            "lat_s": round(world.coordinates.lat_s, 3),
            "lon_w": round(world.coordinates.lon_w, 3),
            "lon_e": round(world.coordinates.lon_e, 3),
        },
        "base": {
            "lattice": serde_json::to_value(world.base.grid.lattice.kind).expect("a lattice converts to JSON"),
            "cells_x": world.base.grid.cells_x,
            "cells_y": world.base.grid.cells_y,
            "points": world.base.grid.points.iter().map(|&p| point(p)).collect::<Vec<_>>(),
            "heights": world.base.heights.iter().map(|&h| h as u8).collect::<Vec<_>>(),
        },
        "grid": {
            "points": g.points.iter().map(|&p| point(p)).collect::<Vec<_>>(),
            "base_cell": world.base_cell,
        },
        "heights": world.heights.iter().map(|&h| h as u8).collect::<Vec<_>>(),
        "elevation": world.drainage.elevation.iter().map(|&e| round(e, 3)).collect::<Vec<_>>(),
        "temperature": world.temperature.iter().map(|&t| t as i32).collect::<Vec<_>>(),
        "precipitation": world.precipitation.iter().map(|&p| p as u32).collect::<Vec<_>>(),
        "flux": world.drainage.flux.iter().map(|&f| round(f, 3)).collect::<Vec<_>>(),
        "biomes": world.biomes.iter().map(|&b| inkland_core::biomes::NAMES[b as usize]).collect::<Vec<_>>(),
        "features": world.features.list.iter().zip(&world.names.features).map(|(f, n)| {
            let mut v = json!({
                "id": f.id,
                "kind": f.kind.name(),
                "group": n.group,
                "name": n.name,
                "cells": f.cells,
                "border": f.border,
                "area": round(f.area, 3),
                "outline": f.outline.iter().map(|&v| point(world.grid.vertices[v as usize])).collect::<Vec<_>>(),
            });
            if f.kind == inkland_core::features::FeatureKind::Lake {
                v["height"] = json!(round(f.height, 3));
                v["shoreline"] = json!(f.shoreline);
            }
            v
        }).collect::<Vec<_>>(),
        "religions": world.religions.list.iter().map(|r| json!({
            "id": r.id,
            "name": r.name,
            "kind": r.kind,
            "culture": r.culture,
            "center_place": r.center_place,
        })).collect::<Vec<_>>(),
        "cell_religion": world.religions.cell_religion,
        "rivers": world.rivers.iter().zip(&world.names.rivers).map(|(r, n)| river_value(r, n, config)).collect::<Vec<_>>(),
        "roads": world.roads.iter().map(|r| json!({
            "id": r.id,
            "pin": r.pin.map(|k| config.pins.roads[k].id.clone()),
            "kind": serde_json::to_value(r.kind).expect("a kind converts to JSON"),
            "length": round(r.length, 3),
            "cells": r.cells,
            "points": r.points.iter().map(|&p| point(p)).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "cell_culture": world.society.cell_culture,
        "cell_state": world.society.cell_state,
        "cell_province": world.society.cell_province,
        "cultures": world.society.cultures.iter().map(|c| json!({
            "id": c.id,
            "name": c.name,
            "name_base": inkland_core::names::data::NAME_BASES[c.base].name,
            "center": c.center,
        })).collect::<Vec<_>>(),
        "places": world.society.places.iter().map(|p| json!({
            "id": p.id,
            "pin": p.pin,
            "name": p.name,
            "kind": serde_json::to_value(p.kind).expect("a kind converts to JSON"),
            "at": point(p.at),
            "cell": p.cell,
            "port": p.port,
            "capital": p.capital,
            "culture": p.culture,
            "state": p.state,
            "province": p.province,
        })).collect::<Vec<_>>(),
        "states": world.society.states.iter().map(|st| json!({
            "id": st.id,
            "name": st.name,
            "capital": st.capital,
            "culture": st.culture,
        })).collect::<Vec<_>>(),
        "provinces": world.society.provinces.iter().map(|pr| json!({
            "id": pr.id,
            "name": pr.name,
            "state": pr.state,
            "seat": pr.seat,
        })).collect::<Vec<_>>(),
        "coastlines": world.coastlines.iter().map(|c| json!({
            "id": c.id,
            "land": c.land,
            "water": c.water,
            "closed": c.closed,
            "points": c.points.iter().map(|&p| point(p)).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    // Cell data of the coast (FMG `markupPack`).
    v["distance"] = json!(world.features.distance);
    v["haven"] = json!(
        world
            .features
            .haven
            .iter()
            .map(|&h| (h != inkland_core::features::NO_HAVEN).then_some(h))
            .collect::<Vec<_>>()
    );
    v["harbor"] = json!(world.features.harbor);
    v
}

/// The JSON value of one river.
fn river_value(
    r: &inkland_core::rivers::River,
    name: &inkland_core::geonames::RiverName,
    config: &Config,
) -> Value {
    use inkland_core::rivers::Mouth;
    let pin = r.course.map(|k| &config.pins.rivers[k]);
    let mouth = match r.mouth {
        Mouth::Water(c) => json!({ "kind": "water", "cell": c }),
        Mouth::Join(c) => json!({ "kind": "join", "cell": c }),
        Mouth::Sink => json!({ "kind": "sink" }),
        Mouth::Edge => json!({ "kind": "edge" }),
    };
    json!({
        "id": r.id,
        "pin": pin.map(|p| p.id.clone()),
        "name": name.name,
        "type": name.kind,
        "parent": r.parent,
        "basin": r.basin,
        "source_width": round(r.source_width, 3),
        "width_factor": round(r.width_factor, 3),
        "width": round(r.width, 3),
        "mouth": mouth,
        "flux": round(r.flux, 3),
        "length": round(r.length, 3),
        "cells": r.cells,
        "points": r.points.iter().map(|&p| point(p)).collect::<Vec<_>>(),
    })
}

/// Read the text of a `world.json`: the config in it, and the world that the
/// config makes. The config must agree with the `world_hash` of the file.
pub fn read_world(text: &str) -> Result<(Config, World), inkland_core::error::Error> {
    use inkland_core::error::{Error, ErrorCode};
    let bad = |msg: String| Error::new(ErrorCode::WorldParse, "", msg);
    let v: Value =
        serde_json::from_str(text).map_err(|e| bad(format!("the world is not valid JSON: {e}")))?;
    if v["format"] != FORMAT {
        return Err(bad(format!("the file is not a {FORMAT} file")));
    }
    if v["format_version"] != FORMAT_VERSION {
        return Err(bad(format!(
            "the format version {} is not {FORMAT_VERSION}",
            v["format_version"]
        ))
        .with("found", v["format_version"].clone()));
    }
    let config = crate::config::from_value(v["config"].clone()).map_err(|e| {
        let path = format!("/config{}", e.path);
        Error { path, ..e }
    })?;
    if v["world_hash"] != crate::config::world_hash(&config).as_str() {
        return Err(bad(
            "the config in the file does not agree with its world_hash".into(),
        ));
    }
    let world = World::build(&config)?;
    Ok((config, world))
}

/// The text of `world.json`: compact JSON and one newline at the end.
pub fn world_json(world: &World, config: &Config) -> String {
    let mut s = serde_json::to_string(&world_value(world, config))
        .expect("a world always converts to JSON");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 9, "frame": { "width": 100, "height": 80 },
            "world": { "cells": 1000, "template": "high-island" }
        }))
        .unwrap()
    }

    #[test]
    fn world_json_has_each_cell_and_rounded_points() {
        let c = config();
        let w = World::build(&c).unwrap();
        let v = world_value(&w, &c);
        assert_eq!(v["format"], FORMAT);
        assert_eq!(v["heights"].as_array().unwrap().len(), w.grid.len());
        assert_eq!(v["grid"]["points"].as_array().unwrap().len(), w.grid.len());
        let x = v["grid"]["points"][0][0].as_f64().unwrap();
        assert_eq!(x, round(x, 3));
    }

    #[test]
    fn world_json_is_the_same_text_for_the_same_config() {
        let c = config();
        let a = world_json(&World::build(&c).unwrap(), &c);
        let b = world_json(&World::build(&c).unwrap(), &c);
        assert_eq!(a, b);
        assert!(a.ends_with("}\n"));
    }

    #[test]
    fn read_world_gives_back_the_same_world() {
        let c = config();
        let text = world_json(&World::build(&c).unwrap(), &c);
        let (c2, w2) = read_world(&text).unwrap();
        assert_eq!(world_json(&w2, &c2), text);
    }

    #[test]
    fn read_world_refuses_a_changed_or_foreign_file() {
        use inkland_core::error::ErrorCode;
        let c = config();
        let text = world_json(&World::build(&c).unwrap(), &c);
        let changed = text.replacen("\"seed\":9", "\"seed\":10", 1);
        assert_ne!(changed, text);
        assert_eq!(
            read_world(&changed).unwrap_err().code,
            ErrorCode::WorldParse
        );
        assert_eq!(read_world("{}").unwrap_err().code, ErrorCode::WorldParse);
        assert_eq!(
            read_world("not json").unwrap_err().code,
            ErrorCode::WorldParse
        );
    }

    #[test]
    fn world_json_does_not_change_with_overlays_variants_or_render() {
        let c = config();
        let a = world_json(&World::build(&c).unwrap(), &c);
        let mut d = c.clone();
        d.overlays.insert("then".into(), Default::default());
        d.render.min_line_width_pt = 1.0;
        d.variants[0].crop = Some([0.0, 0.0, 50.0, 40.0]);
        d.font_dirs.push("fonts".into());
        assert_eq!(world_json(&World::build(&d).unwrap(), &d), a);
        let mut e = c.clone();
        e.seed += 1;
        assert_ne!(crate::config::world_hash(&e), crate::config::world_hash(&c));
    }
}
