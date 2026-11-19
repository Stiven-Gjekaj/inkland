//! The relief symbols of a variant for the report: how many of each kind
//! the map shows, and how many each shape in `hide_in` leaves out. Two
//! variants that share one terrain can then be compared: the map of each
//! age shows its own symbols and hides its own.

use crate::page::Page;
use inkland_core::config::{Config, ReliefSymbol, RenderMode, Variant};
use inkland_core::world::World;
use std::collections::BTreeMap;

/// The relief symbols of a variant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReliefSummary {
    /// The number of symbols of each kind that the map shows.
    pub shown: BTreeMap<String, usize>,
    /// For each id in `hide_in`, the number of symbols of each kind that
    /// it leaves out. A symbol in two shapes counts for the first.
    pub hidden: BTreeMap<String, BTreeMap<String, usize>>,
    /// For each region pin and area pin, the number of shown symbols of
    /// each kind inside it. A symbol in two shapes counts for both.
    pub pins: BTreeMap<String, BTreeMap<String, usize>>,
}

/// The relief symbols of a variant, before the labels clear their places.
/// In `print-safe` mode, the kinds are the symbol kinds of
/// `render.relief.hide`. In `colour` mode, they are the icon kinds of
/// Azgaar's Fantasy Map Generator, for example `mount` or `conifer`.
pub fn summary(world: &World, config: &Config, variant: &Variant) -> ReliefSummary {
    let shapes = config.relief_hidden_shapes(variant);
    // The symbols without `hide_in`, with their world points.
    let mut all = variant.clone();
    let mut patch = variant.relief.clone().unwrap_or_default();
    patch.hide_in = Some(Vec::new());
    all.relief = Some(patch);
    let page = Page::new(config, &all);
    let symbols: Vec<(String, [f64; 2])> = if config.render.mode == RenderMode::Colour {
        let labels = crate::labels::Labels::default();
        let ctx = crate::fmg::Ctx::new(world, config, &all, &page, &labels);
        let up = ctx.px.unit_px;
        crate::fmg::layers::marks::relief_icons(&ctx)
            .into_iter()
            .map(|i| {
                (
                    i.kind.to_owned(),
                    [(i.x + i.s / 2.0) / up, (i.y + i.s / 2.0) / up],
                )
            })
            .collect()
    } else {
        let opts = config.relief_of(&all);
        crate::decor::relief(
            world,
            &page,
            config.seed,
            &opts,
            &crate::decor::Peakless::of(world, config),
        )
        .into_iter()
        .map(|(sym, p, _)| (sym.name().to_owned(), page.to_world(p)))
        .collect()
    };
    // A kind in `only_in` stands only in its shapes.
    let only = config.relief_only_shapes(variant);
    let symbols: Vec<(String, [f64; 2])> = symbols
        .into_iter()
        .filter(|(kind, p)| {
            let k = if config.render.mode == RenderMode::Colour {
                crate::fmg::layers::marks::icon_kind(kind)
            } else {
                ReliefSymbol::from_name(kind)
            };
            k.and_then(|k| only.get(&k))
                .is_none_or(|shapes| shapes.iter().any(|a| a.contains(*p)))
        })
        .collect();
    let mut out = ReliefSummary::default();
    for id in config.relief_of(variant).hide_in.iter() {
        out.hidden.entry(id.clone()).or_default();
    }
    let pins: Vec<(String, inkland_core::geom::Shape)> = config
        .pins
        .regions
        .iter()
        .map(|r| r.id.clone())
        .chain(config.pins.areas.iter().map(|a| a.id.clone()))
        .filter_map(|id| config.pin_shape(&id).map(|s| (id, s)))
        .collect();
    for (id, _) in &pins {
        out.pins.entry(id.clone()).or_default();
    }
    for (kind, p) in symbols {
        let hider = shapes.iter().find(|(_, s)| s.contains(p));
        if hider.is_none() {
            for (id, _) in pins.iter().filter(|(_, s)| s.contains(p)) {
                *out.pins
                    .entry(id.clone())
                    .or_default()
                    .entry(kind.clone())
                    .or_default() += 1;
            }
        }
        match hider {
            Some((id, _)) => {
                *out.hidden
                    .entry(id.clone())
                    .or_default()
                    .entry(kind)
                    .or_default() += 1;
            }
            None => *out.shown.entry(kind).or_default() += 1,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config(mode: &str) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 5, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea" },
            "pins": {
                "regions": [{ "id": "fen", "kind": "marsh", "box": [100, 50, 300, 250] }],
                "areas": [{ "id": "wood", "character": "wooded", "box": [0, 0, 60, 60] }]
            },
            "render": { "mode": mode },
            "variants": [
                { "name": "then", "page": { "w_mm": 167, "h_mm": 125 } },
                { "name": "now", "page": { "w_mm": 167, "h_mm": 125 },
                  "relief": { "hide_in": ["fen", "wood"] } }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn only_in_keeps_a_kind_in_its_shapes() {
        for mode in ["print-safe", "colour"] {
            let mut c = config(mode);
            let w = World::build(&c).unwrap();
            let kind = if mode == "colour" {
                "deciduous"
            } else {
                "tree"
            };
            let before = summary(&w, &c, &c.variants[0]);
            let total = |s: &ReliefSummary| s.shown.get(kind).copied().unwrap_or(0);
            assert!(total(&before) > 0, "{mode}");
            c.render.relief.only_in.tree = vec!["wood".to_owned()];
            let after = summary(&w, &c, &c.variants[0]);
            // Only the trees in the wood (0 to 60, 0 to 60) stay.
            assert!(total(&after) < total(&before), "{mode}");
            assert_eq!(
                after.shown.get("mountain"),
                before.shown.get("mountain"),
                "{mode}"
            );
        }
    }

    #[test]
    fn each_age_shows_what_the_other_hides() {
        for mode in ["print-safe", "colour"] {
            let c = config(mode);
            let w = World::build(&c).unwrap();
            let then = summary(&w, &c, &c.variants[0]);
            let now = summary(&w, &c, &c.variants[1]);
            assert!(then.hidden.is_empty(), "{mode}");
            assert_eq!(
                now.hidden.keys().collect::<Vec<_>>(),
                vec!["fen", "wood"],
                "{mode}"
            );
            let count = |m: &BTreeMap<String, usize>| m.values().sum::<usize>();
            let hidden: usize = now.hidden.values().map(count).sum();
            assert!(now.hidden["fen"].values().sum::<usize>() > 0, "{mode}");
            // The two maps share one terrain, so the symbols add up.
            assert_eq!(count(&then.shown), count(&now.shown) + hidden, "{mode}");
            // Each pin has its counts; the hidden fen shows none now.
            assert!(count(&then.pins["fen"]) > 0, "{mode}");
            assert_eq!(count(&now.pins["fen"]), 0, "{mode}");
            assert_eq!(
                count(&then.pins["fen"]),
                count(&now.hidden["fen"]),
                "{mode}"
            );
        }
    }
}
