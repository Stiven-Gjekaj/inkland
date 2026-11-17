//! Overlays: what one variant adds to, removes from, or renames in the
//! world.
//!
//! An overlay works on a copy of the world. It changes only places, roads,
//! and names. The terrain, the rivers, and the coasts stay the same, so
//! `world.json` does not change when only an overlay changes.
//!
//! - `add.places`: a place at its exact position, in the cell under it. The
//!   terrain does not change, so the place can be on any terrain.
//! - `add.roads`: routed on the shared terrain. They reuse the roads of the
//!   world at a lower cost.
//! - `remove`: `place:<id>` and `road:<id>` leave the copy. Other kinds are
//!   for the renderer.
//! - `rename`: `place:<id>` changes the place name. Other kinds are for the
//!   renderer.

use crate::config::{Config, Overlay};
use crate::error::{Error, ErrorCode};
use crate::names::Chain;
use crate::rand::Rand;
use crate::seed::{Layer, SeedTree};
use crate::society::Place;
use crate::world::World;
use std::collections::BTreeSet;

/// The kind and the id of an item key `"<kind>:<id>"`.
pub fn split_key(key: &str) -> Option<(&str, &str)> {
    key.split_once(':')
}

/// `true` if the overlay removes the item.
pub fn removes(overlay: &Overlay, kind: &str, id: &str) -> bool {
    overlay
        .remove
        .iter()
        .any(|k| split_key(k) == Some((kind, id)))
}

/// The new name of an item, if the overlay gives one.
pub fn renamed<'a>(overlay: &'a Overlay, kind: &str, id: &str) -> Option<&'a str> {
    overlay
        .rename
        .iter()
        .find(|(k, _)| split_key(k) == Some((kind, id)))
        .map(|(_, v)| v.as_str())
}

/// The world as the overlay `name` shows it.
pub fn apply(world: &World, config: &Config, name: &str) -> Result<World, Error> {
    let overlay = config.overlays.get(name).ok_or_else(|| {
        Error::new(
            ErrorCode::RefUnknown,
            "/overlays",
            format!("there is no overlay {name:?}"),
        )
    })?;
    let mut w = world.clone();
    let soc = &mut w.society;

    // Places.
    soc.places.retain(|p| {
        !p.pin
            .as_deref()
            .is_some_and(|id| removes(overlay, "place", id))
    });
    let mut used: BTreeSet<String> = soc.places.iter().map(|p| p.name.clone()).collect();
    let mut rng =
        Rand::new(SeedTree::new(config.seed).part_rng(Layer::Names, &format!("overlay/{name}")));
    let first = world.society.places.iter().map(|p| p.id).max().unwrap_or(0) + 1;
    for (id, pin) in (first..).zip(&overlay.add.places) {
        let cell = w.grid.find_cell(pin.at[0], pin.at[1]);
        let culture = soc.cell_culture[cell];
        let name = match &pin.name {
            Some(n) => n.clone(),
            None => {
                let k = culture.max(1) as usize - 1;
                let base = soc.cultures.get(k).map_or(0, |c| c.base);
                Chain::new(base).name(&mut rng, &mut used)
            }
        };
        used.insert(name.clone());
        soc.places.push(Place {
            id,
            pin: Some(pin.id.clone()),
            name,
            kind: pin.kind,
            cell,
            at: pin.at,
            port: pin.port,
            capital: false,
            culture,
            state: soc.cell_state[cell],
            province: soc.cell_province[cell],
        });
    }
    for p in soc.places.iter_mut() {
        if let Some(n) = p
            .pin
            .as_deref()
            .and_then(|id| renamed(overlay, "place", id))
        {
            p.name = n.to_owned();
        }
    }

    // Roads.
    w.roads.retain(|r| {
        let id = r.pin.map(|k| config.pins.roads[k].id.as_str());
        !id.is_some_and(|id| removes(overlay, "road", id))
    });
    let added = crate::roads::route_more(
        &crate::roads::Input {
            config,
            grid: &w.grid,
            heights: &w.heights,
            features: &w.features,
            biomes: &w.biomes,
            rivers: &w.rivers,
            flux: &w.drainage.flux,
            river_threshold: crate::rivers::river_threshold(w.grid.len()),
            society: &w.society,
        },
        &w.roads,
        &overlay.add.roads,
        &format!("/overlays/{name}/add/roads"),
    )?;
    w.roads.extend(added);
    Ok(w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config() -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 8, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": { "steps": ["Add 35 all"] } },
            "pins": {
                "points": [
                    { "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [60, 150] },
                    { "id": "harrow", "kind": "town", "at": [340, 150] },
                    { "id": "mere", "kind": "village", "at": [200, 60] }
                ],
                "roads": [{ "id": "kings-road", "from": "ostrey", "to": "harrow" }]
            },
            "overlays": {
                "then": {
                    "add": {
                        "places": [
                            { "id": "old-keep", "kind": "ruin", "at": [200, 220] },
                            { "id": "camp", "kind": "custom", "name": "The Camp", "at": [120, 80] }
                        ],
                        "roads": [{ "id": "old-way", "from": "mere", "to": "old-keep", "kind": "trail" }]
                    },
                    "remove": ["place:harrow", "road:kings-road"],
                    "rename": { "place:ostrey": "Ostrey Old Town" }
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn an_overlay_changes_places_and_roads_and_not_the_terrain() {
        let c = config();
        let base = World::build(&c).unwrap();
        let w = apply(&base, &c, "then").unwrap();
        assert_eq!(w.heights, base.heights);
        assert_eq!(w.rivers, base.rivers);
        assert_eq!(w.coastlines, base.coastlines);
        let pin = |id: &str| {
            w.society
                .places
                .iter()
                .find(|p| p.pin.as_deref() == Some(id))
        };
        assert!(pin("harrow").is_none());
        assert_eq!(pin("ostrey").unwrap().name, "Ostrey Old Town");
        let keep = pin("old-keep").unwrap();
        assert_eq!(keep.at, [200.0, 220.0]);
        assert!(!keep.name.is_empty());
        assert_eq!(pin("camp").unwrap().name, "The Camp");
        assert!(w.roads.iter().all(|r| r.pin.is_none()));
        let way = w
            .roads
            .iter()
            .find(|r| r.overlay.as_deref() == Some("old-way"))
            .unwrap();
        assert_eq!(way.cells.first(), Some(&pin("mere").unwrap().cell));
        assert_eq!(way.cells.last(), Some(&keep.cell));
        // The base world keeps its places and roads.
        assert!(base.roads.iter().any(|r| r.pin == Some(0)));
        assert!(
            base.society
                .places
                .iter()
                .any(|p| p.pin.as_deref() == Some("harrow"))
        );
    }

    #[test]
    fn same_overlay_gives_the_same_world() {
        let c = config();
        let base = World::build(&c).unwrap();
        assert_eq!(
            apply(&base, &c, "then").unwrap(),
            apply(&base, &c, "then").unwrap()
        );
    }

    #[test]
    fn an_unknown_overlay_is_an_error() {
        let c = config();
        let base = World::build(&c).unwrap();
        assert_eq!(
            apply(&base, &c, "now").unwrap_err().code,
            ErrorCode::RefUnknown
        );
    }
}
