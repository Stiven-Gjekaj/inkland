//! The SVG writer.
//!
//! The SVG is the real output. Its page size is in millimetres, and the
//! `viewBox` has the same numbers, so one SVG unit is one millimetre. All
//! coordinates have 3 decimals. Groups and elements have stable ids, and the
//! writer puts them in a fixed order.

use crate::page::{Page, pt};
use inkland_core::config::{Config, Variant};
use inkland_core::num::fixed;
use inkland_core::world::World;
use std::fmt::Write as _;

/// Line widths, in points. Milestone 7 moves these into the style config.
pub const COAST_WIDTH_PT: f64 = 0.75;
pub const NEATLINE_WIDTH_PT: f64 = 1.0;
pub const RIVER_MIN_PT: f64 = 0.5;
pub const RIVER_STEP_PT: f64 = 0.35;
pub const RIVER_MAX_PT: f64 = 2.0;

/// Black and white only, so the file prints the same on each printer.
const INK: &str = "#000000";
const PAPER: &str = "#ffffff";

fn n(v: f64) -> String {
    fixed(v, 3)
}

/// Write the SVG of one variant.
pub fn render(world: &World, config: &Config, variant: &Variant) -> String {
    let page = Page::new(config, variant);
    let min = config.render.min_line_width_pt;
    let coast_w = pt(COAST_WIDTH_PT.max(min));
    let neat_w = pt(NEATLINE_WIDTH_PT.max(min));
    let [mx, my, mw, mh] = page.map_rect();

    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">",
        w = n(page.w_mm),
        h = n(page.h_mm)
    );
    let _ = writeln!(
        s,
        "<defs><clipPath id=\"clip-map\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath></defs>",
        n(mx),
        n(my),
        n(mw),
        n(mh)
    );
    let _ = writeln!(
        s,
        "<rect id=\"paper\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{PAPER}\"/>",
        n(page.w_mm),
        n(page.h_mm)
    );
    let _ = writeln!(
        s,
        "<g id=\"coast\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{}\" stroke-linejoin=\"round\" stroke-linecap=\"round\">",
        n(coast_w)
    );
    for line in &world.coastlines {
        let d = path_data(&page, &line.points, line.closed);
        if d.is_empty() {
            continue;
        }
        let _ = writeln!(s, "<path id=\"coast-{}\" d=\"{d}\"/>", line.id);
    }
    s.push_str("</g>\n");
    write_rivers(&mut s, world, config, &page);
    let _ = writeln!(
        s,
        "<rect id=\"neatline\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{}\"/>",
        n(mx),
        n(my),
        n(mw),
        n(mh),
        n(neat_w)
    );
    s.push_str("</svg>\n");
    s
}

/// The line width of a river, in points: wider for more water.
pub fn river_width_pt(flux: f64, threshold: f64, min_pt: f64) -> f64 {
    let w = RIVER_MIN_PT + RIVER_STEP_PT * libm::log2(1.0 + flux / threshold);
    w.clamp(RIVER_MIN_PT, RIVER_MAX_PT).max(min_pt)
}

fn write_rivers(s: &mut String, world: &World, config: &Config, page: &Page) {
    let threshold = inkland_core::rivers::river_threshold(world.grid.len());
    let min = config.render.min_line_width_pt;
    let mut items: Vec<(String, String, f64)> = world
        .rivers
        .iter()
        .filter_map(|r| {
            let id = match r.course {
                Some(k) => format!("river-{}", config.pins.rivers[k].id),
                None => format!("river-{}", r.id),
            };
            let d = path_data(page, &r.points, false);
            (!d.is_empty()).then(|| (id, d, pt(river_width_pt(r.flux, threshold, min))))
        })
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let _ = writeln!(
        s,
        "<g id=\"rivers\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-linejoin=\"round\" stroke-linecap=\"round\">"
    );
    for (id, d, w) in items {
        let _ = writeln!(s, "<path id=\"{id}\" d=\"{d}\" stroke-width=\"{}\"/>", n(w));
    }
    s.push_str("</g>\n");
}

/// Path data in page millimetres. Points that round to the same text as the
/// point before them are left out.
fn path_data(page: &Page, points: &[[f64; 2]], closed: bool) -> String {
    let pts: &[[f64; 2]] = if closed && points.len() > 1 {
        &points[..points.len() - 1]
    } else {
        points
    };
    let mut d = String::new();
    let mut last = String::new();
    let mut count = 0;
    for &p in pts {
        let [x, y] = page.to_page(p);
        let xy = format!("{} {}", n(x), n(y));
        if xy == last {
            continue;
        }
        d.push(if count == 0 { 'M' } else { 'L' });
        d.push_str(&xy);
        last = xy;
        count += 1;
    }
    if count < 2 {
        return String::new();
    }
    if closed {
        d.push('Z');
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config() -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 11, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 2000, "template": "archipelago" },
            "variants": [{ "name": "full", "page": { "w_mm": 167, "h_mm": 250 } }]
        }))
        .unwrap()
    }

    fn svg() -> (String, World) {
        let c = config();
        let w = World::build(&c).unwrap();
        (render(&w, &c, &c.variants[0]), w)
    }

    #[test]
    fn page_size_is_in_millimetres_with_a_matching_view_box() {
        let (s, _) = svg();
        assert!(s.contains("width=\"167mm\" height=\"250mm\" viewBox=\"0 0 167 250\""));
    }

    #[test]
    fn each_coastline_has_a_path_with_a_stable_id() {
        let (s, w) = svg();
        assert!(!w.coastlines.is_empty());
        for line in &w.coastlines {
            assert!(
                s.contains(&format!("id=\"coast-{}\"", line.id)),
                "coast-{}",
                line.id
            );
        }
        assert!(s.contains("<g id=\"coast\""));
    }

    #[test]
    fn output_has_no_grey_and_no_effects() {
        let (s, _) = svg();
        for bad in [
            "opacity",
            "rgba",
            "hsla",
            "<filter",
            "<mask",
            "Gradient",
            "mix-blend",
        ] {
            assert!(!s.contains(bad), "{bad}");
        }
    }

    #[test]
    fn line_widths_are_at_least_the_minimum() {
        let mut c = config();
        c.render.min_line_width_pt = 2.0;
        let w = World::build(&c).unwrap();
        let s = render(&w, &c, &c.variants[0]);
        assert!(s.contains(&format!("stroke-width=\"{}\"", n(pt(2.0)))));
        assert!(!s.contains(&format!("stroke-width=\"{}\"", n(pt(COAST_WIDTH_PT)))));
    }

    #[test]
    fn each_river_has_a_path_and_a_width_in_range() {
        let mut c = config();
        c.world.template = serde_json::from_value(json!("pangea")).unwrap();
        c.world.cells = 5000;
        let w = World::build(&c).unwrap();
        let s = render(&w, &c, &c.variants[0]);
        assert!(!w.rivers.is_empty());
        assert!(s.contains("<g id=\"rivers\""));
        for r in &w.rivers {
            assert!(
                s.contains(&format!("id=\"river-{}\"", r.id)),
                "river-{}",
                r.id
            );
        }
        assert!(river_width_pt(0.0, 50.0, 0.5) >= 0.5);
        assert!(river_width_pt(1e9, 50.0, 0.5) <= RIVER_MAX_PT);
        assert!(river_width_pt(400.0, 50.0, 0.5) > river_width_pt(60.0, 50.0, 0.5));
    }

    #[test]
    fn same_world_gives_the_same_bytes() {
        assert_eq!(svg().0, svg().0);
    }
}
