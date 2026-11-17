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

/// The SVG of one variant and its labels.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub svg: String,
    pub labels: crate::labels::Labels,
}

/// Write the SVG of one variant.
pub fn render(world: &World, config: &Config, variant: &Variant) -> Rendered {
    let page = Page::new(config, variant);
    let labels = crate::labels::place(world, config, variant, &page);
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
            LayerName::Relief => write_relief(&mut s, world, config, &page, min, &labels),
            LayerName::Rivers => write_rivers(&mut s, world, config, variant, &page),
            LayerName::Regions => write_borders(&mut s, world, config, &page),
            LayerName::Roads => write_roads(&mut s, world, config, &page),
            LayerName::Features => write_features(&mut s, config, variant, &page),
            LayerName::Places => write_places(&mut s, world, config, &page),
            LayerName::Labels => write_labels(&mut s, &labels),
        }
    }
    crate::extras::write(
        &mut s,
        config,
        variant,
        &page,
        &crate::labels::symbol_points(world, &page),
    );
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
    Rendered { svg: s, labels }
}

/// Write the labels: real `<text>` with a white halo behind it. A river label
/// follows a path in `<defs>`.
fn write_labels(s: &mut String, labels: &crate::labels::Labels) {
    use crate::extras::xml;
    use crate::labels::Geometry;
    let paths: Vec<_> = labels
        .placed
        .iter()
        .filter_map(|l| match &l.geometry {
            Geometry::Path { points, .. } => Some((&l.id, points)),
            Geometry::Point { .. } => None,
        })
        .collect();
    if !paths.is_empty() {
        s.push_str("<defs>\n");
        for (id, points) in paths {
            let mut d = String::new();
            for (i, p) in points.iter().enumerate() {
                d.push(if i == 0 { 'M' } else { 'L' });
                let _ = write!(d, "{} {}", n(p[0]), n(p[1]));
            }
            let _ = writeln!(s, "<path id=\"path-{id}\" d=\"{d}\"/>");
        }
        s.push_str("</defs>\n");
    }
    let _ = writeln!(
        s,
        "<g id=\"labels\" fill=\"{INK}\" stroke=\"{PAPER}\" stroke-width=\"{}\" stroke-linejoin=\"round\" paint-order=\"stroke\">",
        n(crate::labels::HALO_MM)
    );
    for l in &labels.placed {
        let spacing = if l.spacing > 0.0 {
            format!(" letter-spacing=\"{}\"", n(l.spacing))
        } else {
            String::new()
        };
        let font = format!(
            "font-family=\"{}\" font-size=\"{}\"{spacing}",
            xml(&l.family),
            n(l.size)
        );
        match &l.geometry {
            Geometry::Point {
                x,
                y,
                anchor,
                angle,
            } => {
                let turn = if *angle != 0 {
                    format!(" transform=\"rotate({angle} {} {})\"", n(*x), n(*y))
                } else {
                    String::new()
                };
                let _ = writeln!(
                    s,
                    "<text id=\"{}\" x=\"{}\" y=\"{}\" {font} text-anchor=\"{anchor}\"{turn}>{}</text>",
                    l.id,
                    n(*x),
                    n(*y),
                    xml(&l.text)
                );
            }
            Geometry::Path { offset, .. } => {
                let _ = writeln!(
                    s,
                    "<text id=\"{}\" dy=\"{}\" {font} text-anchor=\"middle\"><textPath href=\"#path-{}\" startOffset=\"50%\" text-anchor=\"middle\">{}</textPath></text>",
                    l.id,
                    n(-offset),
                    l.id,
                    xml(&l.text)
                );
            }
        }
    }
    s.push_str("</g>\n");
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
            .map(|&v| world.base.grid.vertices[v as usize])
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

fn write_relief(
    s: &mut String,
    world: &World,
    config: &Config,
    page: &Page,
    min: f64,
    labels: &crate::labels::Labels,
) {
    use crate::decor::Symbol;
    // A symbol under a label is left out, so that the text stays clear.
    let symbols: Vec<_> = crate::decor::relief(world, page, config.seed)
        .into_iter()
        .filter(|(_, p)| !labels.covers(*p, 1.0))
        .collect();
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

fn write_rivers(s: &mut String, world: &World, config: &Config, variant: &Variant, page: &Page) {
    let threshold = inkland_core::rivers::river_threshold(world.grid.len());
    let min = config.render.min_line_width_pt;
    let overlay = crate::labels::overlay_of(config, variant);
    let mut items: Vec<(String, String, f64)> = world
        .rivers
        .iter()
        .filter(|r| {
            // An overlay can hide a pinned river. The river stays in the world.
            !r.course.is_some_and(|k| {
                overlay.is_some_and(|o| {
                    inkland_core::overlay::removes(o, "river", &config.pins.rivers[k].id)
                })
            })
        })
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

fn write_borders(s: &mut String, world: &World, config: &Config, page: &Page) {
    let min = config.render.min_line_width_pt;
    let line = |verts: &[u32]| -> String {
        let pts: Vec<[f64; 2]> = verts
            .iter()
            .map(|&v| world.grid.vertices[v as usize])
            .collect();
        path_data(page, &pts, false)
    };
    let (state_edges, province_edges) = inkland_core::borders::state_and_province_edges(world);
    let _ = writeln!(
        s,
        "<g id=\"regions\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">"
    );
    let _ = writeln!(
        s,
        "<g id=\"borders-provinces\" stroke-width=\"{}\" stroke-dasharray=\"0.3 0.9\">",
        n(line_mm(PROVINCE_BORDER_PT.max(min)))
    );
    for (i, l) in inkland_core::borders::chain_edges(&province_edges)
        .iter()
        .enumerate()
    {
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
    for (i, l) in inkland_core::borders::chain_edges(&state_edges)
        .iter()
        .enumerate()
    {
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
    // Pinned roads and overlay roads, each with its config id.
    for r in world.roads.iter() {
        let id = match (r.pin, &r.overlay) {
            (Some(k), _) => config.pins.roads[k].id.clone(),
            (None, Some(id)) => id.clone(),
            (None, None) => continue,
        };
        for w in r.cells.windows(2) {
            pinned_edges.insert((w[0].min(w[1]), w[0].max(w[1])));
        }
        let d = path_data(page, &r.points, false);
        if !d.is_empty() {
            items.push((format!("road-{id}"), d, r.kind));
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
            .filter(|r| r.pin.is_none() && r.overlay.is_none() && r.kind == kind)
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
                .filter(|r| r.pin.is_none() && r.overlay.is_none() && r.kind == RoadKind::Road)
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
        for (i, line) in inkland_core::borders::chain_edges(&list).iter().enumerate() {
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

/// Write the features of the overlay of a variant: a symbol at a point, a
/// line, or an outline. A ruin is a cross, a battle site is two crossed
/// blades, and other point features are a small triangle. A border is a
/// dash-dot line, a wall is a thick line, and other lines are dotted.
fn write_features(s: &mut String, config: &Config, variant: &Variant, page: &Page) {
    use inkland_core::config::FeatureKind;
    let Some(overlay) = crate::labels::overlay_of(config, variant) else {
        return;
    };
    let mut feats: Vec<_> = overlay
        .add
        .features
        .iter()
        .filter(|f| !inkland_core::overlay::removes(overlay, "feature", &f.id))
        .collect();
    if feats.is_empty() {
        return;
    }
    feats.sort_by(|a, b| a.id.cmp(&b.id));
    let min = config.render.min_line_width_pt;
    let _ = writeln!(
        s,
        "<g id=\"features\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">"
    );
    for f in feats {
        let (width_pt, dash) = match f.kind {
            FeatureKind::Border => (0.6, " stroke-dasharray=\"2 0.8 0.4 0.8\""),
            FeatureKind::Wall => (1.0, ""),
            _ => (0.5, " stroke-dasharray=\"0.4 0.8\""),
        };
        let w = n(line_mm(f64::max(width_pt, min)));
        let id = format!("feature-{}", f.id);
        if let Some(at) = f.at {
            let [x, y] = page.to_page(at);
            let (x, y) = (n(x), n(y));
            let d = match f.kind {
                FeatureKind::Ruin => format!("M{x} {y}m-0.6 -0.6l1.2 1.2m0 -1.2l-1.2 1.2"),
                FeatureKind::BattleSite => format!(
                    "M{x} {y}m-0.7 -0.7l1.4 1.4m0 -1.4l-1.4 1.4m0.2 -0.6l0.4 0.4m0.8 0l0.4 -0.4"
                ),
                _ => format!("M{x} {y}m0 -0.7l0.6 1.1h-1.2z"),
            };
            let _ = writeln!(s, "<path id=\"{id}\" d=\"{d}\" stroke-width=\"{w}\"/>");
        } else if let Some(line) = &f.polyline {
            let d = path_data(page, line, false);
            if !d.is_empty() {
                let _ = writeln!(
                    s,
                    "<path id=\"{id}\" d=\"{d}\" stroke-width=\"{w}\"{dash}/>"
                );
            }
        } else if let Some(poly) = &f.polygon {
            let mut pts = poly.clone();
            if pts.first() != pts.last()
                && let Some(&p) = pts.first()
            {
                pts.push(p);
            }
            let d = path_data(page, &pts, true);
            if !d.is_empty() {
                let _ = writeln!(
                    s,
                    "<path id=\"{id}\" d=\"{d}\" stroke-width=\"{w}\"{dash}/>"
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
        (render(&w, &c, &c.variants[0]).svg, w)
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
        let s = render(&w, &c, &c.variants[0]).svg;
        assert!(s.contains(&format!("stroke-width=\"{}\"", n(line_mm(2.0)))));
        assert!(!s.contains(&format!("stroke-width=\"{}\"", n(line_mm(COAST_WIDTH_PT)))));
    }

    #[test]
    fn each_river_has_a_path_and_a_width_in_range() {
        let mut c = config();
        c.world.template = serde_json::from_value(json!("pangea")).unwrap();
        c.world.cells = 5000;
        let w = World::build(&c).unwrap();
        let s = render(&w, &c, &c.variants[0]).svg;
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
    fn each_place_has_a_symbol_and_borders_are_drawn() {
        let mut c = config();
        c.world.template = serde_json::from_value(json!("pangea")).unwrap();
        c.world.cells = 5000;
        let w = World::build(&c).unwrap();
        let s = render(&w, &c, &c.variants[0]).svg;
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
