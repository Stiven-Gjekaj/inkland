//! The SVG writer.
//!
//! The SVG is the real output. Its page size is in millimetres, and the
//! `viewBox` has the same numbers, so one SVG unit is one millimetre. All
//! coordinates have 3 decimals. Groups and elements have stable ids, and the
//! writer puts them in a fixed order.

use crate::page::{Page, line_mm};
use inkland_core::config::{Config, Variant};
use inkland_core::num::fixed;
use inkland_core::world::World;
use std::fmt::Write as _;

/// Line widths, in points. Milestone 7 moves these into the style config.
pub const COAST_WIDTH_PT: f64 = 0.75;
pub const NEATLINE_WIDTH_PT: f64 = 1.0;
pub const STATE_BORDER_PT: f64 = 0.8;
pub const PROVINCE_BORDER_PT: f64 = 0.5;
pub const PLACE_LINE_PT: f64 = 0.5;
pub const ROAD_PT: f64 = 0.7;
pub const TRAIL_PT: f64 = 0.5;
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
    let coast_w = line_mm(COAST_WIDTH_PT.max(min));
    let neat_w = line_mm(NEATLINE_WIDTH_PT.max(min));
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
    for layer in &config.render.layers {
        use inkland_core::config::LayerName;
        match layer {
            LayerName::Coast => write_coast(&mut s, world, &page, coast_w, min),
            LayerName::Relief => write_relief(&mut s, world, config, &page, min),
            LayerName::Rivers => write_rivers(&mut s, world, config, &page),
            LayerName::Regions => write_borders(&mut s, world, config, &page),
            LayerName::Roads => write_roads(&mut s, world, config, &page),
            LayerName::Places => write_places(&mut s, world, config, &page),
            LayerName::Labels => {}
        }
    }
    crate::extras::write(&mut s, config, variant, &page);
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

fn write_coast(s: &mut String, world: &World, page: &Page, coast_w: f64, min: f64) {
    // Water lines: two thin lines in the water along each coast.
    let _ = writeln!(
        s,
        "<g id=\"water-lines\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{}\" stroke-linejoin=\"round\">",
        n(line_mm(0.5f64.max(min)))
    );
    for (i, line) in crate::decor::water_lines(world).iter().enumerate() {
        let pts: Vec<[f64; 2]> = line
            .iter()
            .map(|&v| world.grid.vertices[v as usize])
            .collect();
        let closed = line.len() > 2 && line.first() == line.last();
        let smooth = crate::decor::chaikin(&pts, closed, 2);
        let d = path_data(page, &smooth, closed);
        if !d.is_empty() {
            let _ = writeln!(s, "<path id=\"water-line-{}\" d=\"{d}\"/>", i + 1);
        }
    }
    s.push_str("</g>\n");
    let _ = writeln!(
        s,
        "<g id=\"coast\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{}\" stroke-linejoin=\"round\" stroke-linecap=\"round\">",
        n(coast_w)
    );
    for line in &world.coastlines {
        let smooth = crate::decor::chaikin(&line.points, line.closed, 2);
        let d = path_data(page, &smooth, line.closed);
        if d.is_empty() {
            continue;
        }
        let _ = writeln!(s, "<path id=\"coast-{}\" d=\"{d}\"/>", line.id);
    }
    s.push_str("</g>\n");
}

fn write_relief(s: &mut String, world: &World, config: &Config, page: &Page, min: f64) {
    use crate::decor::Symbol;
    let symbols = crate::decor::relief(world, page, config.seed);
    let _ = writeln!(
        s,
        "<g id=\"relief\" clip-path=\"url(#clip-map)\" stroke=\"{INK}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">",
        n(line_mm(0.5f64.max(min)))
    );
    for (sym, [x, y]) in symbols {
        let fill = match sym {
            Symbol::Mountain | Symbol::Tree => PAPER,
            Symbol::Dune => INK,
            Symbol::Hill | Symbol::Marsh => "none",
        };
        let stroke = if sym == Symbol::Dune {
            " stroke=\"none\""
        } else {
            ""
        };
        let _ = writeln!(
            s,
            "<path d=\"{}\" fill=\"{fill}\"{stroke}/>",
            sym.path(x, y)
        );
    }
    s.push_str("</g>\n");
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
            (!d.is_empty()).then(|| (id, d, line_mm(river_width_pt(r.flux, threshold, min))))
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

/// Join edges that share a vertex into lines. The result is the same for
/// the same edges: lines start at the lowest free end, and closed loops at
/// their lowest vertex.
pub fn chain_edges(edges: &[(u32, u32)]) -> Vec<Vec<u32>> {
    use std::collections::BTreeMap;
    let mut adj: BTreeMap<u32, Vec<(u32, usize)>> = BTreeMap::new();
    for (i, &(a, b)) in edges.iter().enumerate() {
        adj.entry(a).or_default().push((b, i));
        adj.entry(b).or_default().push((a, i));
    }
    let mut used = vec![false; edges.len()];
    let mut lines = Vec::new();
    let walk = |start: u32, used: &mut Vec<bool>| -> Vec<u32> {
        let mut line = vec![start];
        let mut cur = start;
        while let Some(&(next, e)) = adj[&cur].iter().filter(|(_, e)| !used[*e]).min() {
            used[e] = true;
            line.push(next);
            cur = next;
            if adj[&cur].len() != 2 {
                break;
            }
        }
        line
    };
    // Open lines first: from each vertex that is not on exactly two edges.
    let ends: Vec<u32> = adj
        .iter()
        .filter(|(_, v)| v.len() != 2)
        .map(|(&k, _)| k)
        .collect();
    for v in ends {
        while adj[&v].iter().any(|(_, e)| !used[*e]) {
            lines.push(walk(v, &mut used));
        }
    }
    // Then closed loops.
    let all: Vec<u32> = adj.keys().copied().collect();
    for v in all {
        if adj[&v].iter().any(|(_, e)| !used[*e]) {
            lines.push(walk(v, &mut used));
        }
    }
    lines
}

/// The polygon edges between land cells whose owners differ by `key`.
fn owner_edges(world: &World, key: &dyn Fn(usize) -> (u32, u32)) -> Vec<(u32, u32)> {
    let g = &world.grid;
    let mut edges = Vec::new();
    for c in 0..g.len() {
        if !world.features.is_land(c) {
            continue;
        }
        let poly = &g.polygons[c];
        for (k, &other) in g.edge_cells[c].iter().enumerate() {
            if other == inkland_core::grid::OUTSIDE {
                continue;
            }
            let o = other as usize;
            if o < c || !world.features.is_land(o) {
                continue;
            }
            let (a, b) = (key(c), key(o));
            // Different owners; a missing second owner (0) is not a border.
            if a != b && (a.0 != b.0 || (a.1 != 0 && b.1 != 0)) {
                edges.push((poly[k], poly[(k + 1) % poly.len()]));
            }
        }
    }
    edges
}

fn write_borders(s: &mut String, world: &World, config: &Config, page: &Page) {
    let soc = &world.society;
    let min = config.render.min_line_width_pt;
    let line = |verts: &[u32]| -> String {
        let pts: Vec<[f64; 2]> = verts
            .iter()
            .map(|&v| world.grid.vertices[v as usize])
            .collect();
        path_data(page, &pts, false)
    };
    let state_edges = owner_edges(world, &|c| (soc.cell_state[c], 0));
    let province_edges: Vec<(u32, u32)> =
        owner_edges(world, &|c| (soc.cell_state[c], soc.cell_province[c]))
            .into_iter()
            .filter(|e| !state_edges.contains(e))
            .collect();
    let _ = writeln!(
        s,
        "<g id=\"regions\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">"
    );
    let _ = writeln!(
        s,
        "<g id=\"borders-provinces\" stroke-width=\"{}\" stroke-dasharray=\"0.3 0.9\">",
        n(line_mm(PROVINCE_BORDER_PT.max(min)))
    );
    for (i, l) in chain_edges(&province_edges).iter().enumerate() {
        let d = line(l);
        if !d.is_empty() {
            let _ = writeln!(s, "<path id=\"border-province-{}\" d=\"{d}\"/>", i + 1);
        }
    }
    s.push_str("</g>\n");
    let _ = writeln!(
        s,
        "<g id=\"borders-states\" stroke-width=\"{}\" stroke-dasharray=\"2 0.8 0.4 0.8\">",
        n(line_mm(STATE_BORDER_PT.max(min)))
    );
    for (i, l) in chain_edges(&state_edges).iter().enumerate() {
        let d = line(l);
        if !d.is_empty() {
            let _ = writeln!(s, "<path id=\"border-state-{}\" d=\"{d}\"/>", i + 1);
        }
    }
    s.push_str("</g>\n</g>\n");
}

fn write_roads(s: &mut String, world: &World, config: &Config, page: &Page) {
    use inkland_core::config::RoadKind;
    use std::collections::BTreeSet;
    let min = config.render.min_line_width_pt;
    let style = |kind: RoadKind| match kind {
        RoadKind::Road => (n(line_mm(ROAD_PT.max(min))), "1.6 0.5"),
        RoadKind::Trail => (n(line_mm(TRAIL_PT.max(min))), "0.5 0.6"),
    };
    let _ = writeln!(
        s,
        "<g id=\"roads\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">"
    );
    // Pinned roads, each with its config id.
    let mut pinned_edges: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut items: Vec<(String, String, RoadKind)> = Vec::new();
    for r in world.roads.iter() {
        let Some(k) = r.pin else { continue };
        for w in r.cells.windows(2) {
            pinned_edges.insert((w[0].min(w[1]), w[0].max(w[1])));
        }
        let d = path_data(page, &r.points, false);
        if !d.is_empty() {
            items.push((format!("road-{}", config.pins.roads[k].id), d, r.kind));
        }
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));
    for (id, d, kind) in items {
        let (w, dash) = style(kind);
        let _ = writeln!(
            s,
            "<path id=\"{id}\" d=\"{d}\" stroke-width=\"{w}\" stroke-dasharray=\"{dash}\"/>"
        );
    }
    // The generated network: shared edges are drawn once.
    for kind in [RoadKind::Road, RoadKind::Trail] {
        let mut edges: BTreeSet<(usize, usize)> = BTreeSet::new();
        for r in world
            .roads
            .iter()
            .filter(|r| r.pin.is_none() && r.kind == kind)
        {
            for w in r.cells.windows(2) {
                let e = (w[0].min(w[1]), w[0].max(w[1]));
                if !pinned_edges.contains(&e) {
                    edges.insert(e);
                }
            }
        }
        if kind == RoadKind::Trail {
            // A trail edge that a road uses is a road.
            for r in world
                .roads
                .iter()
                .filter(|r| r.pin.is_none() && r.kind == RoadKind::Road)
            {
                for w in r.cells.windows(2) {
                    edges.remove(&(w[0].min(w[1]), w[0].max(w[1])));
                }
            }
        }
        let list: Vec<(u32, u32)> = edges.iter().map(|&(a, b)| (a as u32, b as u32)).collect();
        let (w, dash) = style(kind);
        let name = match kind {
            RoadKind::Road => "road-net",
            RoadKind::Trail => "trail-net",
        };
        for (i, line) in chain_edges(&list).iter().enumerate() {
            let pts: Vec<[f64; 2]> = line
                .iter()
                .map(|&c| world.grid.points[c as usize])
                .collect();
            let d = path_data(page, &pts, false);
            if !d.is_empty() {
                let _ = writeln!(
                    s,
                    "<path id=\"{name}-{}\" d=\"{d}\" stroke-width=\"{w}\" stroke-dasharray=\"{dash}\"/>",
                    i + 1
                );
            }
        }
    }
    s.push_str("</g>\n");
}

/// The symbol of a place: a circle for a city or a town, a dot for a village,
/// a square for a fortress, a cross for a ruin, a triangle for a custom
/// place. A capital has a ring around its symbol.
fn place_symbol(
    kind: inkland_core::config::PlaceKind,
    capital: bool,
    x: f64,
    y: f64,
    w: &str,
) -> String {
    use inkland_core::config::PlaceKind::*;
    let (x, y) = (n(x), n(y));
    let mut out = match kind {
        City => format!(
            "<circle cx=\"{x}\" cy=\"{y}\" r=\"0.9\" fill=\"{PAPER}\" stroke-width=\"{w}\"/><circle cx=\"{x}\" cy=\"{y}\" r=\"0.35\" fill=\"{INK}\" stroke=\"none\"/>"
        ),
        Town => format!(
            "<circle cx=\"{x}\" cy=\"{y}\" r=\"0.65\" fill=\"{PAPER}\" stroke-width=\"{w}\"/>"
        ),
        Village => {
            format!("<circle cx=\"{x}\" cy=\"{y}\" r=\"0.4\" fill=\"{INK}\" stroke=\"none\"/>")
        }
        Fortress => format!(
            "<rect x=\"{}\" y=\"{}\" width=\"1.4\" height=\"1.4\" fill=\"{PAPER}\" stroke-width=\"{w}\" transform=\"translate(-0.7 -0.7)\"/>",
            x, y
        ),
        Ruin => format!(
            "<path d=\"M{x} {y}m-0.6 -0.6l1.2 1.2m0 -1.2l-1.2 1.2\" fill=\"none\" stroke-width=\"{w}\"/>"
        ),
        Custom => format!(
            "<path d=\"M{x} {y}m0 -0.8l0.7 1.2h-1.4z\" fill=\"{PAPER}\" stroke-width=\"{w}\"/>"
        ),
    };
    if capital {
        out.push_str(&format!(
            "<circle cx=\"{x}\" cy=\"{y}\" r=\"1.35\" fill=\"none\" stroke-width=\"{w}\"/>"
        ));
    }
    out
}

fn write_places(s: &mut String, world: &World, config: &Config, page: &Page) {
    let w = n(line_mm(PLACE_LINE_PT.max(config.render.min_line_width_pt)));
    let [x0, y0, x1, y1] = page.view;
    let mut items: Vec<(String, String)> = world
        .society
        .places
        .iter()
        .filter(|p| p.at[0] >= x0 && p.at[0] <= x1 && p.at[1] >= y0 && p.at[1] <= y1)
        .map(|p| {
            let id = match &p.pin {
                Some(pin) => format!("place-{pin}"),
                None => format!("place-{}", p.id),
            };
            let [x, y] = page.to_page(p.at);
            (id, place_symbol(p.kind, p.capital, x, y, &w))
        })
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let _ = writeln!(s, "<g id=\"places\" stroke=\"{INK}\">");
    for (id, sym) in items {
        let _ = writeln!(s, "<g id=\"{id}\">{sym}</g>");
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
        assert!(s.contains(&format!("stroke-width=\"{}\"", n(line_mm(2.0)))));
        assert!(!s.contains(&format!("stroke-width=\"{}\"", n(line_mm(COAST_WIDTH_PT)))));
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
    fn chain_edges_joins_a_line_and_a_loop() {
        let edges = [(1, 2), (3, 2), (3, 4), (10, 11), (11, 12), (12, 10)];
        let lines = chain_edges(&edges);
        assert_eq!(lines, vec![vec![1, 2, 3, 4], vec![10, 11, 12, 10]]);
    }

    #[test]
    fn each_place_has_a_symbol_and_borders_are_drawn() {
        let mut c = config();
        c.world.template = serde_json::from_value(json!("pangea")).unwrap();
        c.world.cells = 5000;
        let w = World::build(&c).unwrap();
        let s = render(&w, &c, &c.variants[0]);
        assert!(w.society.places.len() > 5);
        for p in &w.society.places {
            assert!(
                s.contains(&format!("<g id=\"place-{}\">", p.id)),
                "place-{}",
                p.id
            );
        }
        assert!(s.contains("<g id=\"borders-states\""));
        assert!(s.contains("id=\"border-state-1\""));
    }

    #[test]
    fn same_world_gives_the_same_bytes() {
        assert_eq!(svg().0, svg().0);
    }
}
