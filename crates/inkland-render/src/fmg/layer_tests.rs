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
        Zones => Some("zones"),
        Goods => Some("goods"),
        Markets => Some("markets"),
        Trade => Some("trade-animation"),
        Military => Some("armies"),
        Journeys => Some("journeys"),
        Rulers => Some("ruler"),
        Markers => Some("markers"),
        Texture | Ice | Population | Features => None,
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
fn named_seas_get_labels_in_both_modes() {
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
    // An ocean label has the style of the custom labels.
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let text = doc
        .descendants()
        .find(|n| {
            n.attribute("id")
                .is_some_and(|i| i.starts_with("label-water-"))
        })
        .unwrap();
    assert_eq!(text.parent().unwrap().attribute("id"), Some("labels-added"));
    let mut c = base_config();
    c.render.mode = RenderMode::PrintSafe;
    let print = crate::svg::render(w, &c, &c.variants[0]).svg;
    assert!(print.contains("id=\"label-water-"));
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

/// Each placed label box, with the halo of its style, is inside the map
/// area, in both modes and with each style preset.
#[test]
fn each_label_box_with_its_halo_stays_inside_the_map() {
    let mut runs: Vec<(RenderMode, StyleName)> = vec![(RenderMode::PrintSafe, StyleName::Default)];
    runs.extend(StyleName::ALL.iter().map(|&s| (RenderMode::Colour, s)));
    for (mode, style) in runs {
        let mut c = base_config();
        c.render.mode = mode;
        c.render.style = style;
        let page = Page::new(&c, &c.variants[0]);
        let labels = if mode == RenderMode::Colour {
            place_labels(world(), &c, &c.variants[0], &page)
        } else {
            crate::labels::place(world(), &c, &c.variants[0], &page)
        };
        assert!(!labels.placed.is_empty(), "{style:?}");
        let [mx, my, mw, mh] = page.map_rect();
        let s = Style::with_overrides(style, &c.render.style_overrides);
        for l in &labels.placed {
            // The box keeps the halo of the style free around the glyphs.
            let reach = if mode == RenderMode::Colour {
                layers::text::halo(&s, style_group(l.group)) * l.size
            } else {
                0.0
            };
            let f = crate::metrics::FontMetrics::of(&l.family);
            for b in &l.boxes {
                let [x0, y0, x1, y1] = b.bounds();
                assert!(
                    x0 >= mx && y0 >= my && x1 <= mx + mw && y1 <= my + mh,
                    "{style:?} {mode:?}: {} leaves the map",
                    l.id
                );
                assert!(
                    2.0 * b.h[1] >= f.cap(l.size) + 0.6 * f.descent(l.size) + 2.0 * reach - 1e-9,
                    "{style:?}: {} has no room for its halo",
                    l.id
                );
            }
        }
    }
}

#[test]
fn a_label_outline_is_a_part_of_the_font_size() {
    let s = Style::preset(StyleName::Atlas);
    // A shadow of 3 px blur gives a stroke of 0.18 of the size.
    let css = s.attr(&["labels", "city"], "style").unwrap();
    assert!(css.contains("white 0px 0px 3px"), "{css}");
    let (w, color) = layers::text::outline(&s, "city").unwrap();
    assert!((w - 0.18).abs() < 1e-9);
    assert_eq!(color.as_deref(), Some("white"));
    assert!((layers::text::halo(&s, "city") - 0.09).abs() < 1e-9);
    // An outline of the style is a part of the size of the group.
    let mut o = Style::preset(StyleName::Atlas);
    o.set("labels.groups.city.attrs.stroke-width", json!(0.6));
    let (w, color) = layers::text::outline(&o, "city").unwrap();
    assert!((w - 0.1).abs() < 1e-9, "{w}");
    assert_eq!(color, None);
    // Each label in the SVG has the stroke width of its own size.
    let svg = render_with(&[LayerName::Labels], StyleName::Atlas);
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let texts: Vec<_> = doc
        .descendants()
        .filter(|n| {
            n.has_tag_name("text")
                && n.attribute("id")
                    .is_some_and(|i| i.starts_with("label-place-"))
        })
        .collect();
    assert!(!texts.is_empty());
    for t in texts {
        let size: f64 = t.attribute("font-size").unwrap().parse().unwrap();
        let width: f64 = t.attribute("stroke-width").unwrap().parse().unwrap();
        assert!((width - 0.18 * size).abs() < 0.01, "{size} {width}");
    }
}

#[test]
fn label_sizes_follow_the_ratios_of_the_style() {
    let c = base_config();
    let city = crate::page::pt(c.render.font_sizes_pt.place_city);
    let least = crate::page::pt(crate::labels::MIN_LABEL_PT);
    for &name in &StyleName::ALL {
        let s = Style::preset(name);
        let pct = |g: &str| s.attr_f64(&["labels", g], "font-size").unwrap();
        // The capitals get the size of the cities of the config.
        assert!(
            (label_size(&s, &c, "capital") - city).abs() < 1e-9,
            "{name:?}"
        );
        for g in [
            "state", "city", "town", "village", "river", "added", "province",
        ] {
            let want = (city * pct(g) / pct("capital")).max(least);
            assert!((label_size(&s, &c, g) - want).abs() < 1e-9, "{name:?} {g}");
        }
    }
    // Atlas: the states are 20/7 of the capitals, the rivers get the least size.
    let s = Style::preset(StyleName::Atlas);
    assert!((label_size(&s, &c, "state") - city * 20.0 / 7.0).abs() < 1e-9);
    assert!((label_size(&s, &c, "river") - least).abs() < 1e-9);
}

#[test]
fn placed_colour_labels_have_the_size_of_their_style_group() {
    let mut c = base_config();
    c.render.style = StyleName::Atlas;
    let page = Page::new(&c, &c.variants[0]);
    let labels = place_labels(world(), &c, &c.variants[0], &page);
    let s = Style::preset(StyleName::Atlas);
    let mut seen = 0;
    for l in &labels.placed {
        let full = label_size(&s, &c, style_group(l.group));
        if matches!(
            l.kind,
            crate::labels::Kind::State | crate::labels::Kind::Region
        ) {
            // An area label can be smaller to fit its area.
            assert!(
                l.size <= full + 1e-9 && l.size >= 0.4 * full - 1e-9,
                "{}",
                l.id
            );
        } else {
            assert!((l.size - full).abs() < 1e-9, "{} {} {}", l.id, l.size, full);
            seen += 1;
        }
    }
    assert!(seen > 0);
}

/// Render the layers of a world in `colour` mode.
fn render_world(w: &World, layers: &[LayerName]) -> String {
    let mut c = base_config();
    c.render.layers = layers.to_vec();
    crate::svg::render(w, &c, &c.variants[0]).svg
}

/// The test world with a change.
fn world_with(change: impl FnOnce(&mut World)) -> World {
    let mut w = world().clone();
    change(&mut w);
    w
}

#[test]
fn zones_draw_with_the_hatches_of_fmg() {
    let svg = render_with(&[LayerName::Zones], StyleName::Default);
    let w = world();
    assert!(!w.zones.is_empty(), "the test world has zones");
    for z in &w.zones {
        assert!(
            svg.contains(&format!("<path id=\"zone-{}\" d=\"M", z.id)),
            "{}",
            z.id
        );
        let id = z.color.trim_start_matches("url(#").trim_end_matches(')');
        assert!(svg.contains(&format!("fill=\"{}\"", z.color)));
        assert!(svg.contains(&format!("<pattern id=\"{id}\"")), "{id}");
    }
    // The style of FMG: opacity 0.6, no stroke width.
    assert!(svg.contains("<g id=\"zones\" opacity=\"0.6\""));
    parse_and_draw(&svg);
    // No zones: no group and no pattern.
    let empty = render_world(&world_with(|w| w.zones.clear()), &[LayerName::Zones]);
    assert!(!empty.contains("<g id=\"zones\"") && !empty.contains("<pattern id=\"hatch"));
}

#[test]
fn goods_draw_the_cells_the_places_and_the_deposits() {
    let svg = render_with(&[LayerName::Goods], StyleName::Default);
    for part in ["goods-cells", "goods-traffic", "goods-burgs", "goods-icons"] {
        assert!(svg.contains(&format!("<g id=\"{part}\"")), "{part}");
    }
    assert!(svg.contains("<polygon id=\"goods-cell-"));
    assert!(svg.contains("<g id=\"goods-burg-"));
    assert!(svg.contains("<g id=\"goods-icon-"));
    // Each deposit icon has its symbol, with the lines of FMG: 0.1 x 0.4 x
    // 100 / 2.5.
    assert!(svg.contains("<symbol id=\"goods-"));
    assert!(svg.contains("stroke=\"#41414f\" stroke-width=\"1.6\"/></g>"));
    // The traffic of the goods layer is off in the default style.
    assert!(svg.contains("<g id=\"goods-traffic\" fill=\"none\" opacity=\"0.7\""));
    assert!(!svg.contains("data-cells="));
    parse_and_draw(&svg);

    let mut c = base_config();
    c.render.layers = vec![LayerName::Goods];
    c.render.style_overrides.insert(
        "goods.groups.goodsTraffic.options.render".to_owned(),
        json!(true),
    );
    let svg = crate::svg::render(world(), &c, &c.variants[0]).svg;
    assert!(svg.contains("<path id=\"goods-link-"));

    let empty = world_with(|w| w.economy.goods.clear());
    assert!(!render_world(&empty, &[LayerName::Goods]).contains("<g id=\"goods\""));
}

#[test]
fn markets_draw_their_areas_and_pins() {
    let svg = render_with(&[LayerName::Markets], StyleName::Default);
    let w = world();
    assert!(svg.contains("<g id=\"markets\" fill-opacity=\"0.03\""));
    for m in &w.economy.markets {
        assert!(
            svg.contains(&format!("<g id=\"market-{}\">", m.id)),
            "{}",
            m.id
        );
        assert!(svg.contains(&format!("<clipPath id=\"market-clip-{}\">", m.id)));
    }
    // A pin with the scales glyph of the style, radius 5 and stroke 5 / 8.
    assert!(svg.contains("stroke-width=\"0.63\"/><use href=\"#glyph-2696-fe0f\""));
    assert!(svg.contains("<symbol id=\"glyph-2696-fe0f\""));
    parse_and_draw(&svg);
    let empty = world_with(|w| w.economy.markets.clear());
    assert!(!render_world(&empty, &[LayerName::Markets]).contains("<g id=\"markets\""));
}

#[test]
fn trade_draws_the_traffic_of_the_links() {
    let svg = render_with(&[LayerName::Trade], StyleName::Default);
    assert!(
        svg.contains("<g id=\"trade-traffic\" fill=\"none\" opacity=\"0.7\" stroke=\"#808080\"")
    );
    let count = svg.matches("<path id=\"trade-link-").count();
    assert!(count > 0 && count <= world().economy.traffic.len());
    // The width is at most the size of the style, and at least 0.15.
    for part in svg.split("stroke-width=\"").skip(1) {
        let v: f64 = part[..part.find('"').unwrap()].parse().unwrap();
        assert!((0.15..=3.0).contains(&v), "{v}");
    }
    parse_and_draw(&svg);
    // The goods layer and the trade layer give each line its own id.
    let mut c = base_config();
    c.render.layers = vec![LayerName::Goods, LayerName::Trade];
    c.render.style_overrides.insert(
        "goods.groups.goodsTraffic.options.render".to_owned(),
        json!(true),
    );
    let both = crate::svg::render(world(), &c, &c.variants[0]).svg;
    assert_eq!(both.matches("<path id=\"goods-link-").count(), count);
    let empty = world_with(|w| w.economy.traffic.clear());
    assert!(!render_world(&empty, &[LayerName::Trade]).contains("<g id=\"trade-animation\""));
}

#[test]
fn military_draws_a_box_for_each_regiment() {
    let svg = render_with(&[LayerName::Military], StyleName::Default);
    assert!(svg.contains("<g id=\"armies\" font-size=\"6\" fill-opacity=\"1\" stroke=\"#000\""));
    let soc = &world().society;
    let palette = layers::Palette::of(world());
    let mut n = 0;
    for st in soc.states.iter().filter(|s| !s.military.is_empty()) {
        let fill = &palette.states[&st.id];
        assert!(svg.contains(&format!("<g id=\"army-{}\" fill=\"{fill}\"", st.id)));
        for r in &st.military {
            let id = format!("<g id=\"regiment-{}-{}\"", st.id, r.id);
            assert!(svg.contains(&id), "{id}");
            let box_w = if r.naval { 12 } else { 18 };
            let at = svg.find(&id).unwrap();
            assert!(svg[at..].contains(&format!("width=\"{box_w}\" height=\"6\"/>")));
            n += 1;
        }
    }
    assert!(n > 0, "the test world has regiments");
    // Each regiment icon is a glyph symbol in a white fill.
    assert!(svg.contains("<symbol id=\"glyph-"));
    assert!(svg.contains("fill=\"#ffffff\"/></g>"));
    parse_and_draw(&svg);
    let empty = world_with(|w| {
        for st in &mut w.society.states {
            st.military.clear();
        }
    });
    assert!(!render_world(&empty, &[LayerName::Military]).contains("<g id=\"armies\""));
}

#[test]
fn journeys_draw_their_segments_with_markers() {
    let svg = render_with(&[LayerName::Journeys], StyleName::Default);
    let w = world();
    assert!(!w.journeys.is_empty(), "the test world has a journey");
    assert!(svg.contains("<g id=\"journeys\" fill=\"none\" marker-start=\"url(#journey-start)\""));
    assert!(
        svg.contains("<marker id=\"journey-start\"")
            && svg.contains("<marker id=\"journey-arrow\"")
    );
    for j in &w.journeys {
        assert!(svg.contains(&format!("<g id=\"journey-{}\">", j.id)));
        for g in j.segments.iter().filter(|g| g.points.len() > 1) {
            let id = format!("<path id=\"segment-{}-{}\" d=\"M", j.id, g.id);
            assert!(svg.contains(&id), "{id}");
            assert!(svg.contains(&format!("stroke=\"{}\"/>", j.color)));
        }
    }
    parse_and_draw(&svg);
    let empty = world_with(|w| w.journeys.clear());
    let svg = render_world(&empty, &[LayerName::Journeys]);
    assert!(!svg.contains("<g id=\"journeys\"") && !svg.contains("journey-arrow"));
}

#[test]
fn rulers_draw_the_default_ruler_with_its_length() {
    let svg = render_with(&[LayerName::Rulers], StyleName::Default);
    let w = world();
    assert_eq!(w.measurers.len(), 1, "the default ruler");
    assert!(svg.contains("<g id=\"ruler\" fill=\"none\""));
    assert!(svg.contains("<g id=\"ruler-0\"><polyline points=\""));
    // A white line under the dashed grey line, as FMG draws it.
    assert!(svg.contains("stroke=\"currentColor\" stroke-width=\"2\" stroke-dasharray=\"none\"/>"));
    assert!(svg.contains("stroke-width=\"2\" stroke-dasharray=\"10\"/>"));
    assert_eq!(svg.matches("<circle r=\"1em\"").count(), 2);
    let len = inkland_core::num::fixed(w.measurers[0].length().round(), 0);
    assert!(svg.contains(&format!(">{len} miles</text>")), "{len}");
    parse_and_draw(&svg);
    // The style can hide the text.
    let mut c = base_config();
    c.render.layers = vec![LayerName::Rulers];
    c.render
        .style_overrides
        .insert("rulers.attrs.data-labels".to_owned(), json!(false));
    assert!(
        !crate::svg::render(world(), &c, &c.variants[0])
            .svg
            .contains("</text>")
    );
    let empty = world_with(|w| w.measurers.clear());
    assert!(!render_world(&empty, &[LayerName::Rulers]).contains("<g id=\"ruler\""));
}

#[test]
fn the_other_measurers_draw_their_shapes() {
    use inkland_core::annotations::{Measurer, MeasurerKind};
    let pts = vec![[100.0, 100.0], [200.0, 100.0], [150.0, 200.0]];
    let w = world_with(|w| {
        w.measurers = [MeasurerKind::Opisometer, MeasurerKind::Planimeter]
            .into_iter()
            .map(|kind| Measurer {
                kind,
                points: pts.clone(),
            })
            .collect();
    });
    let svg = render_world(&w, &[LayerName::Rulers]);
    assert!(svg.contains("<g id=\"opisometer-0\"><path d=\"M"));
    assert!(svg.contains("<g id=\"planimeter-1\"><path d=\"M"));
    // The area of the triangle: 100 x 100 / 2 square miles.
    assert!(svg.contains(">5K square miles</text>"));
    parse_and_draw(&svg);
}

#[test]
fn markers_show_the_economy_kinds_with_the_icons_of_fmg() {
    let w = world();
    let list = data::markers(w).unwrap();
    for (m, d) in w.markers.iter().zip(&list) {
        assert_ne!(d.icon, "\u{2753}", "{} has an FMG icon", m.kind);
    }
    let svg = render_with(&[LayerName::Markers], StyleName::Default);
    for (kind, icon) in [
        ("mines", "\u{26CF}\u{FE0F}"),
        ("guild-halls", "\u{1F3DB}\u{FE0F}"),
        ("merchant-houses", "\u{2696}\u{FE0F}"),
        ("customs-houses", "\u{1F6C3}"),
    ] {
        let i = w.markers.iter().position(|m| m.kind == kind).expect(kind);
        assert_eq!(list[i].icon, icon, "{kind}");
        let id = format!("<svg id=\"marker-{}\"", w.markers[i].id);
        let at = svg.find(&id).expect(kind);
        let end = at + svg[at..].find("</svg>").unwrap();
        assert!(svg[at..end].contains(&format!(">{icon}</text>")), "{kind}");
    }
}
