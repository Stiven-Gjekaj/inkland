//! `world.json`: the base world, in config units.
//!
//! The file holds the cell sites, the heights, the features, and the
//! coastlines. All coordinates have 3 decimals.

use inkland_core::config::Config;
use inkland_core::num::round;
use inkland_core::world::World;
use serde_json::{Value, json};

/// The format name and version of `world.json`.
pub const FORMAT: &str = "inkland-world";
pub const FORMAT_VERSION: u32 = 1;

fn point(p: [f64; 2]) -> Value {
    json!([round(p[0], 3), round(p[1], 3)])
}

/// The JSON value of a world.
pub fn world_value(world: &World, config: &Config, config_hash: &str) -> Value {
    let g = &world.grid;
    json!({
        "format": FORMAT,
        "format_version": FORMAT_VERSION,
        "tool": { "name": "inkland", "version": env!("CARGO_PKG_VERSION") },
        "config_hash": config_hash,
        "seed": world.seed,
        "units": config.units.name,
        "frame": config.frame,
        "sea_level": world.sea_level,
        "grid": {
            "cells_x": g.cells_x,
            "cells_y": g.cells_y,
            "points": g.points.iter().map(|&p| point(p)).collect::<Vec<_>>(),
        },
        "heights": world.heights.iter().map(|&h| h as u8).collect::<Vec<_>>(),
        "elevation": world.drainage.elevation.iter().map(|&e| round(e, 3)).collect::<Vec<_>>(),
        "temperature": world.temperature.iter().map(|&t| t as i32).collect::<Vec<_>>(),
        "precipitation": world.precipitation.iter().map(|&p| p as u32).collect::<Vec<_>>(),
        "flux": world.drainage.flux.iter().map(|&f| round(f, 3)).collect::<Vec<_>>(),
        "biomes": world.biomes.iter().map(|&b| inkland_core::biomes::NAMES[b as usize]).collect::<Vec<_>>(),
        "features": world.features.list.iter().map(|f| json!({
            "id": f.id,
            "kind": f.kind.name(),
            "cells": f.cells,
            "border": f.border,
        })).collect::<Vec<_>>(),
        "rivers": world.rivers.iter().map(|r| river_value(r, config)).collect::<Vec<_>>(),
        "coastlines": world.coastlines.iter().map(|c| json!({
            "id": c.id,
            "land": c.land,
            "water": c.water,
            "closed": c.closed,
            "points": c.points.iter().map(|&p| point(p)).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// The JSON value of one river.
fn river_value(r: &inkland_core::rivers::River, config: &Config) -> Value {
    use inkland_core::rivers::Mouth;
    let pin = r.course.map(|k| &config.pins.rivers[k]);
    let mouth = match r.mouth {
        Mouth::Water(c) => json!({ "kind": "water", "cell": c }),
        Mouth::Join(c) => json!({ "kind": "join", "cell": c }),
        Mouth::Sink => json!({ "kind": "sink" }),
    };
    json!({
        "id": r.id,
        "pin": pin.map(|p| p.id.clone()),
        "name": pin.and_then(|p| p.name.clone()),
        "parent": r.parent,
        "mouth": mouth,
        "flux": round(r.flux, 3),
        "length": round(r.length, 3),
        "cells": r.cells,
        "points": r.points.iter().map(|&p| point(p)).collect::<Vec<_>>(),
    })
}

/// The text of `world.json`: compact JSON and one newline at the end.
pub fn world_json(world: &World, config: &Config, config_hash: &str) -> String {
    let mut s = serde_json::to_string(&world_value(world, config, config_hash))
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
        let v = world_value(&w, &c, "sha256:x");
        assert_eq!(v["format"], FORMAT);
        assert_eq!(v["heights"].as_array().unwrap().len(), w.grid.len());
        assert_eq!(v["grid"]["points"].as_array().unwrap().len(), w.grid.len());
        let x = v["grid"]["points"][0][0].as_f64().unwrap();
        assert_eq!(x, round(x, 3));
    }

    #[test]
    fn world_json_is_the_same_text_for_the_same_config() {
        let c = config();
        let a = world_json(&World::build(&c).unwrap(), &c, "h");
        let b = world_json(&World::build(&c).unwrap(), &c, "h");
        assert_eq!(a, b);
        assert!(a.ends_with("}\n"));
    }
}
