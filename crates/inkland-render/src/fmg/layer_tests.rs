//! Tests of each layer and each preset of `colour` mode: the layer is in
//! the SVG, the SVG is the same on each run, roxmltree reads it, and resvg
//! renders it.

use super::*;
use inkland_core::config::{RenderMode, StyleName};
use serde_json::json;
use std::sync::OnceLock;

fn base_config() -> Config {
    serde_json::from_value(json!({
        "version": 1, "seed": 5, "frame": { "width": 400, "height": 300 },
        "world": { "cells": 3000, "template": "pangea" },
        "pins": { "points": [
            { "id": "port-town", "kind": "city", "at": [200, 150], "port": true }
        ]},
        "render": { "mode": "colour" },
        "variants": [{ "name": "full", "page": { "w_mm": 167, "h_mm": 125 } }]
    }))
    .unwrap()
}

fn world() -> &'static World {
    static W: OnceLock<World> = OnceLock::new();
    W.get_or_init(|| World::build(&base_config()).unwrap())
}

fn render_with(layers: &[LayerName], style: StyleName) -> String {
    let mut c = base_config();
    c.render.layers = layers.to_vec();
    c.render.style = style;
    crate::svg::render(world(), &c, &c.variants[0]).svg
}

/// Parse the SVG with roxmltree and render it with resvg.
fn parse_and_draw(svg: &str) {
    roxmltree::Document::parse(svg).expect("roxmltree reads the SVG");
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .expect("resvg reads the SVG");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(120, 90).unwrap();
    let s = 120.0 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(s, s),
        &mut pixmap.as_mut(),
    );
    assert!(pixmap.data().iter().any(|&b| b != 0));
}

/// The id of the group of each layer, or `None` for a layer without data
/// in the test world.
fn group_id(layer: LayerName) -> Option<&'static str> {
    use LayerName::*;
    match layer {
        Ocean => Some("ocean"),
        Landmass => Some("landmass"),
        Heightmap => Some("terrs"),
        Lakes => Some("lakes"),
        Biomes => Some("biomes"),
        Cells => Some("cells"),
        Grid => Some("grid-overlay"),
        Coordinates => Some("coordinates"),
        Compass => Some("compass"),
        Rivers => Some("rivers"),
        Relief => Some("terrain"),
        Religions => Some("relig"),
        Cultures => Some("cults"),
        States => Some("regions"),
        Provinces => Some("provs"),
        Borders => Some("borders"),
        Routes => Some("routes"),
        Temperature => Some("temperature"),
        Coastline => Some("coastline"),
        Precipitation => Some("prec"),
        BurgIcons => Some("burg-icons"),
        Labels => Some("labels"),
        ScaleBar => Some("scale-bar"),
        Vignette => Some("vignette"),
        Legend => Some("legend"),
        Emblems => Some("emblems"),
        Texture | Zones | Ice | Population | Markers | Features => None,
        Coast | Regions | Roads | Places => None,
    }
}

#[test]
fn each_layer_is_drawn_and_reads_back() {
    for layer in ORDER {
        let svg = render_with(&[layer], StyleName::Default);
        match group_id(layer) {
            Some(id) => assert!(svg.contains(&format!("<g id=\"{id}\"")), "{layer:?}"),
            None => assert!(!svg.contains("<g id=\"zones\""), "{layer:?}"),
        }
        assert_eq!(svg, render_with(&[layer], StyleName::Default), "{layer:?}");
        parse_and_draw(&svg);
        let ids = crate::checks::check_svg(
            &svg,
            &crate::checks::CheckOptions {
                min_line_width_pt: 0.5,
                fonts_allowed: Vec::new(),
            },
        );
        for r in ids {
            if matches!(r.name, "deterministic_ids" | "all_text_is_text_elements") {
                assert!(r.pass, "{layer:?} {} {:?}", r.name, r.details);
            }
        }
    }
}

#[test]
fn the_layers_with_data_draw_their_elements() {
    let svg = render_with(
        &[LayerName::States, LayerName::Rivers, LayerName::Relief],
        StyleName::Default,
    );
    assert!(svg.contains("id=\"state-1\""));
    assert!(svg.contains("id=\"state-border-1\""));
    assert!(svg.contains("<path id=\"river-"));
    assert!(svg.contains("<use href=\"#relief-simple-"));
    assert!(svg.contains("<symbol id=\"relief-simple-"));
    let svg = render_with(&[LayerName::BurgIcons], StyleName::Default);
    assert!(svg.contains("<symbol id=\"burgs-atlas-"));
    assert!(svg.contains("<symbol id=\"ports-anchor\""));
}

#[test]
fn the_print_safe_names_stand_for_fmg_layers() {
    let a = render_with(&[LayerName::Coast, LayerName::Regions], StyleName::Default);
    let b = render_with(
        &[
            LayerName::Ocean,
            LayerName::Landmass,
            LayerName::Lakes,
            LayerName::Coastline,
            LayerName::States,
            LayerName::Borders,
        ],
        StyleName::Default,
    );
    assert_eq!(a, b);
}

#[test]
fn each_preset_draws_the_map() {
    use LayerName::*;
    // Relief icons are many; `each_relief_set_draws` tests them.
    let layers = [
        Coast, Rivers, Regions, Roads, Places, Labels, Heightmap, Biomes, Vignette, ScaleBar,
        Legend, Compass,
    ];
    let mut sizes = std::collections::BTreeSet::new();
    for style in StyleName::ALL {
        let svg = render_with(&layers, style);
        parse_and_draw(&svg);
        let fill = Style::preset(style)
            .attr(&["ocean", "base"], "fill")
            .unwrap();
        assert!(svg.contains(&format!("fill=\"{fill}\"")), "{style:?}");
        sizes.insert(svg.len());
    }
    // Each preset gives other output.
    assert_eq!(sizes.len(), StyleName::ALL.len());
}

#[test]
fn all_layers_give_the_same_bytes_on_each_run() {
    let all: Vec<LayerName> = ORDER.to_vec();
    for style in [StyleName::Default, StyleName::Ink] {
        assert_eq!(
            render_with(&all, style),
            render_with(&all, style),
            "{style:?}"
        );
    }
}

#[test]
fn colour_mode_uses_the_colours_of_the_style() {
    let mut c = base_config();
    c.render.style_overrides.insert(
        "landmass.attrs.fill".to_owned(),
        serde_json::Value::from("#123456"),
    );
    let svg = crate::svg::render(world(), &c, &c.variants[0]).svg;
    assert!(svg.contains("fill=\"#123456\""));
    assert!(svg.contains("mask=\"url(#land)\""));
    assert!(svg.contains("<mask id=\"land\""));
    assert_eq!(c.render.mode, RenderMode::Colour);
}

#[test]
fn layers_without_world_data_draw_from_given_data() {
    let c = base_config();
    let page = crate::page::Page::new(&c, &c.variants[0]);
    let labels = crate::labels::Labels::default();
    let ctx = Ctx::new(world(), &c, &c.variants[0], &page, &labels);
    let mut s = String::new();
    layers::areas::write_zones(
        &mut s,
        &ctx,
        &[data::Zone {
            id: 1,
            kind: "invasion".into(),
            color: "#ff0000".into(),
            cells: (0..200).collect(),
        }],
    );
    assert!(s.contains("id=\"zone-1\""));
    let mut s = String::new();
    layers::marks::write_ice(
        &mut s,
        &ctx,
        &[data::Ice {
            id: 2,
            glacier: true,
            points: vec![[10.0, 10.0], [20.0, 10.0], [15.0, 20.0]],
        }],
    );
    assert!(s.contains("id=\"ice-2\"") && s.contains("type=\"glacier\""));
    let mut s = String::new();
    layers::overlays::write_population(
        &mut s,
        &ctx,
        &data::Population {
            rural: vec![10.0; 5],
            urban: vec![([200.0, 150.0], 30.0)],
        },
    );
    assert_eq!(s.matches("<line").count(), 6);
    let mut s = String::new();
    layers::marks::write_emblems(
        &mut s,
        &ctx,
        &[data::Emblem {
            group: "state",
            id: 1,
            at: Some([200.0, 150.0]),
            center: 0,
            coa: serde_json::from_value(json!({ "t1": "gules", "shield": "round" })).unwrap(),
        }],
    );
    assert!(s.contains("id=\"emblem-state-1\""));
    assert!(s.contains("fill=\"#d7374a\""));
    let mut s = String::new();
    layers::marks::write_markers(
        &mut s,
        &ctx,
        &[data::Marker {
            id: 3,
            at: [100.0, 100.0],
            icon: "!".into(),
            size: 30.0,
            fill: "#ffffff".into(),
            stroke: "#000000".into(),
        }],
    );
    assert!(s.contains("id=\"marker-3\""));
    let doc = format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{s}</svg>");
    assert!(roxmltree::Document::parse(&doc).is_ok());
}

#[test]
fn each_relief_set_draws() {
    for set in super::icons::RELIEF_SETS {
        let mut c = base_config();
        c.render.layers = vec![LayerName::Relief];
        c.render.style_overrides.insert(
            "relief.options.set".to_owned(),
            serde_json::Value::from(set),
        );
        let svg = crate::svg::render(world(), &c, &c.variants[0]).svg;
        roxmltree::Document::parse(&svg).expect("roxmltree reads the SVG");
        assert!(
            svg.contains(&format!("<use href=\"#relief-{set}-")),
            "{set}"
        );
        // resvg reads each symbol of the set.
        let defs = &svg[svg.find("<defs>").unwrap()..svg.find("</defs>").unwrap() + 7];
        let uses: String = defs
            .match_indices("<symbol id=\"")
            .map(|(i, _)| {
                let id = &defs[i + 12..];
                let id = &id[..id.find('"').unwrap()];
                format!("<use href=\"#{id}\" width=\"10\" height=\"10\"/>")
            })
            .collect();
        parse_and_draw(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\">{defs}<g stroke=\"#000\">{uses}</g></svg>"
        ));
    }
}

#[test]
fn the_ice_layer_draws_the_ice_of_a_cold_world() {
    let mut c = base_config();
    c.world.climate.latitude_range = [70.0, 90.0];
    c.render.layers = vec![LayerName::Ice];
    let w = World::build(&c).unwrap();
    assert!(!w.ice.is_empty(), "a polar world has ice");
    let svg = crate::svg::render(&w, &c, &c.variants[0]).svg;
    assert!(svg.contains("<g id=\"ice\""));
    for i in &w.ice {
        assert!(
            svg.contains(&format!("id=\"ice-{}\"", i.id)),
            "ice-{}",
            i.id
        );
    }
    parse_and_draw(&svg);
}

#[test]
fn named_seas_get_labels_in_colour_mode_only() {
    let w = world();
    let named: Vec<u32> = w
        .names
        .features
        .iter()
        .filter(|n| matches!(n.group, "ocean" | "sea" | "gulf") && n.name.is_some())
        .map(|n| n.id)
        .collect();
    assert!(!named.is_empty(), "the test world has a named sea");
    let svg = render_with(&[LayerName::Labels], StyleName::Default);
    assert!(svg.contains("id=\"label-water-"));
    let mut c = base_config();
    c.render.mode = RenderMode::PrintSafe;
    let print = crate::svg::render(w, &c, &c.variants[0]).svg;
    assert!(!print.contains("label-water-"));
}

#[test]
fn emblems_follow_the_sizes_and_groups_of_fmg() {
    // 15 states: (1920 + 1080) / 40 = 75 pixels, divided by 1 + 15 / 100.
    assert_eq!(layers::marks::emblem_size("state", 15, 1.0), 65.0);
    assert_eq!(layers::marks::emblem_size("province", 115, 1.0), 27.0);
    assert_eq!(layers::marks::emblem_size("burg", 450, 1.0), 11.0);
    assert_eq!(layers::marks::emblem_size("state", 15, 2.0), 130.0);
    let soc = &world().society;
    let svg = render_with(&[LayerName::Emblems], StyleName::Default);
    // A full map shows the arms of the states; the smaller arms are hidden.
    for st in &soc.states {
        assert!(svg.contains(&format!("id=\"emblem-state-{}\"", st.id)));
    }
    assert!(!svg.contains("id=\"emblem-burg-"));
    let mut c = base_config();
    c.render.layers = vec![LayerName::Emblems];
    c.render.emblems.show_all = true;
    let all = crate::svg::render(world(), &c, &c.variants[0]).svg;
    for p in &soc.places {
        assert!(all.contains(&format!("id=\"emblem-burg-{}\"", p.id)));
    }
    for p in &soc.provinces {
        assert!(all.contains(&format!("id=\"emblem-province-{}\"", p.id)));
    }
    parse_and_draw(&all);
}

#[test]
fn the_pole_of_a_square_is_its_center() {
    let square = vec![vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]]];
    let [x, y] = geom::polylabel(&square, 1.0);
    assert!((x - 50.0).abs() < 1.0 && (y - 50.0).abs() < 1.0, "{x} {y}");
    // An L shape: the pole is in the wide part.
    let l = vec![vec![
        [0.0, 0.0],
        [100.0, 0.0],
        [100.0, 20.0],
        [20.0, 20.0],
        [20.0, 100.0],
        [0.0, 100.0],
    ]];
    let [x, y] = geom::polylabel(&l, 0.5);
    assert!(x < 20.0 || y < 20.0, "{x} {y}");
}
