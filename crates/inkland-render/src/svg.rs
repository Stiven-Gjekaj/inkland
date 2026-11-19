//! The SVG writer.
//!
//! The SVG is the real output. Its page size is in millimetres, and the
//! `viewBox` has the same numbers, so one SVG unit is one millimetre. All
//! coordinates have 3 decimals. Groups and elements have stable ids, and the
//! writer puts them in a fixed order.

use crate::hand::Hand;
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
    if config.render.mode == inkland_core::config::RenderMode::Colour {
        let labels = crate::fmg::place_labels(world, config, variant, &page);
        let svg = crate::fmg::render(world, config, variant, &page, &labels);
        return Rendered { svg, labels };
    }
    let labels = crate::labels::place(world, config, variant, &page);
    let min = config.render.min_line_width_pt;
    let coast_w = line_mm(COAST_WIDTH_PT.max(min));
    let neat_w = line_mm(NEATLINE_WIDTH_PT.max(min));
    let [mx, my, mw, mh] = page.map_rect();
    let hand = Hand::of(config);
    let hand = hand.as_ref();

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
    // A layer name of the original program draws the same as the name of
    // inkland for the same thing; each drawing is made once.
    let mut drawn: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for layer in &config.render.layers {
        use inkland_core::config::LayerName;
        let parts: &[&str] = match layer {
            LayerName::Coast => &["water-lines", "coast"],
            LayerName::Ocean => &["water-lines"],
            LayerName::Coastline => &["coast"],
            LayerName::Regions | LayerName::Borders => &["borders"],
            LayerName::Roads | LayerName::Routes => &["roads"],
            LayerName::Places | LayerName::BurgIcons => &["places"],
            LayerName::Relief => &["relief"],
            LayerName::Rivers => &["rivers"],
            LayerName::Features => &["features"],
            LayerName::Labels => &["labels"],
            LayerName::Cells => &["cells"],
            LayerName::Grid => &["grid"],
            LayerName::Coordinates => &["coordinates"],
            LayerName::Emblems => &["emblems"],
            _ => &[],
        };
        for part in parts {
            if !drawn.insert(part) {
                continue;
            }
            match *part {
                "water-lines" => write_water_lines(&mut s, world, &page, min, hand),
                "coast" => write_coast(&mut s, world, config, &page, coast_w, hand),
                "relief" => write_relief(&mut s, world, config, &page, min, &labels),
                "rivers" => write_rivers(&mut s, world, config, variant, &page, hand),
                "borders" => write_borders(&mut s, world, config, &page, hand),
                "roads" => write_roads(&mut s, world, config, &page, hand),
                "features" => write_features(&mut s, config, variant, &page),
                "places" => write_places(&mut s, world, config, &page, &labels),
                "labels" => write_labels(&mut s, &labels, config.render.labels.halo),
                "cells" => crate::print_layers::cells(&mut s, world, &page, min),
                "grid" => crate::print_layers::grid(&mut s, config, &page, min),
                "coordinates" => crate::print_layers::coordinates(&mut s, config, &page, min),
                "emblems" => crate::print_layers::emblems(&mut s, world, config, &page, min),
                _ => {}
            }
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
fn write_labels(s: &mut String, labels: &crate::labels::Labels, halo: bool) {
    use crate::extras::xml;
    use crate::labels::{Geometry, Kind};
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
    // The halo is a white box behind each label at a point: the box that
    // the placer keeps free for the label. The text has no stroke, because
    // a PDF printer turns stroked text into a Type 3 font. A label on a
    // path gets no box, so that its own river stays whole.
    if halo {
        let _ = writeln!(s, "<g id=\"label-halos\" fill=\"{PAPER}\" stroke=\"none\">");
        for l in &labels.placed {
            // A label along a river or a line gets no box, so that its line
            // stays whole. A curved area label gets boxes.
            if matches!(l.geometry, Geometry::Path { .. })
                && matches!(l.kind, Kind::River | Kind::Feature)
            {
                continue;
            }
            let mut d = String::new();
            for b in &l.boxes {
                for (i, p) in b.corners().iter().enumerate() {
                    d.push(if i == 0 { 'M' } else { 'L' });
                    let _ = write!(d, "{} {}", n(p[0]), n(p[1]));
                }
                d.push('Z');
            }
            let _ = writeln!(s, "<path id=\"halo-{}\" d=\"{d}\"/>", l.id);
        }
        s.push_str("</g>\n");
    }
    let _ = writeln!(s, "<g id=\"labels\" fill=\"{INK}\" stroke=\"none\">");
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

/// Water lines: thin lines in the water along each coast, further apart
/// out to sea (`water::lines`).
fn write_water_lines(s: &mut String, world: &World, page: &Page, min: f64, hand: Option<&Hand>) {
    let _ = writeln!(
        s,
        "<g id=\"water-lines\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{}\" stroke-linejoin=\"round\">",
        n(line_mm(0.5f64.max(min)))
    );
    for (i, line) in crate::water::lines(world, page).iter().enumerate() {
        let id = format!("water-line-{}", i + 1);
        let pts = match hand {
            Some(h) => h.wobble(&id, &line.points, line.closed),
            None => line.points.clone(),
        };
        let d = mm_path_data(&pts, line.closed);
        if !d.is_empty() {
            let _ = writeln!(s, "<path id=\"{id}\" d=\"{d}\"/>");
        }
    }
    s.push_str("</g>\n");
}

fn write_coast(
    s: &mut String,
    world: &World,
    config: &Config,
    page: &Page,
    coast_w: f64,
    hand: Option<&Hand>,
) {
    let _ = writeln!(
        s,
        "<g id=\"coast\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{}\" stroke-linejoin=\"round\" stroke-linecap=\"round\">",
        n(coast_w)
    );
    if config.render.coastline == Some(inkland_core::config::CoastlineDetail::Fractal) {
        // The fractal coastline of the original program: one closed path for
        // each island and each lake.
        let px = crate::fmg::geom::Px::new(page, config.frame.width, config.frame.height);
        let map = |p: [f64; 2]| page.to_page([p[0] / px.unit_px, p[1] / px.unit_px]);
        for o in
            crate::fmg::geom::outlines(world, px.unit_px, &crate::fmg::geom::CoastSettings::DEFAULT)
        {
            let d = crate::fmg::geom::coast_path(&o.shape, &map, 3);
            if !d.is_empty() {
                let _ = writeln!(s, "<path id=\"coast-feature-{}\" d=\"{d}\"/>", o.feature);
            }
        }
        s.push_str("</g>\n");
        return;
    }
    for line in &world.coastlines {
        let smooth = crate::decor::chaikin(&line.points, line.closed, 2);
        let key = format!("coast-{}", line.id);
        if let Some(h) = hand {
            // A hand draws the coast in pieces that are wider and thinner.
            let pts: Vec<[f64; 2]> = smooth.iter().map(|&p| page.to_page(p)).collect();
            let pieces = Hand::pieces(&h.wobble(&key, &pts, line.closed));
            let min = config.render.min_line_width_pt;
            let base = COAST_WIDTH_PT.max(min);
            let widths = h.coast_widths(&key, pieces.len(), base, min);
            let out: Vec<(String, f64)> = pieces
                .iter()
                .zip(widths)
                .map(|((p, _), w)| (mm_path_data(p, false), line_mm(w)))
                .filter(|(d, _)| !d.is_empty())
                .collect();
            write_pieces(s, &key, &out);
            continue;
        }
        let d = hand_path(page, hand, &key, &smooth, line.closed);
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
    let symbols: Vec<_> = crate::decor::relief(world, page, config.seed, &config.render.relief)
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

/// The line width of a river at its mouth, in points. `width_km` is the
/// FMG width of the river (`River::width`). The original program draws the
/// river `2 * 1.5 * width_km ^ (1 / 1.8)` map pixels wide (`getWidth` turned
/// back into `getOffset`); `mm_per_px` gives the millimetres of one map
/// pixel on the page (see `fmg::geom::Px`).
pub fn river_width_pt(width_km: f64, mm_per_px: f64, min_pt: f64) -> f64 {
    let px = 3.0 * libm::pow(width_km.max(0.0), 1.0 / 1.8);
    let w = px * mm_per_px / crate::page::MM_PER_PT;
    w.clamp(RIVER_MIN_PT, RIVER_MAX_PT).max(min_pt)
}

fn write_rivers(
    s: &mut String,
    world: &World,
    config: &Config,
    variant: &Variant,
    page: &Page,
    hand: Option<&Hand>,
) {
    let mm_per_px = crate::fmg::geom::Px::new(page, config.frame.width, config.frame.height).mm;
    let min = config.render.min_line_width_pt;
    let overlay = crate::labels::overlay_of(config, variant);
    let mut items: Vec<(String, Vec<(String, f64)>)> = world
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
            let mouth = river_width_pt(r.width, mm_per_px, min);
            let pieces = match hand {
                Some(h) => river_pieces(page, h, &id, &r.points, mouth, min),
                None => {
                    let d = river_curve(page, &r.points);
                    if d.is_empty() {
                        Vec::new()
                    } else {
                        vec![(d, line_mm(mouth))]
                    }
                }
            };
            (!pieces.is_empty()).then_some((id, pieces))
        })
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let _ = writeln!(
        s,
        "<g id=\"rivers\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-linejoin=\"round\" stroke-linecap=\"round\">"
    );
    for (id, pieces) in items {
        write_pieces(s, &id, &pieces);
    }
    s.push_str("</g>\n");
}

fn write_borders(s: &mut String, world: &World, config: &Config, page: &Page, hand: Option<&Hand>) {
    let min = config.render.min_line_width_pt;
    let line = |key: &str, verts: &[u32]| -> String {
        let pts: Vec<[f64; 2]> = verts
            .iter()
            .map(|&v| world.grid.vertices[v as usize])
            .collect();
        hand_path(page, hand, key, &pts, false)
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
        let id = format!("border-province-{}", i + 1);
        let d = line(&id, l);
        if !d.is_empty() {
            let dash = hand_dashes(hand, &id, &[0.3, 0.9]);
            let _ = writeln!(s, "<path id=\"{id}\" d=\"{d}\"{dash}/>");
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
        let id = format!("border-state-{}", i + 1);
        let d = line(&id, l);
        if !d.is_empty() {
            let dash = hand_dashes(hand, &id, &[2.0, 0.8, 0.4, 0.8]);
            let _ = writeln!(s, "<path id=\"{id}\" d=\"{d}\"{dash}/>");
        }
    }
    s.push_str("</g>\n</g>\n");
}

fn write_roads(s: &mut String, world: &World, config: &Config, page: &Page, hand: Option<&Hand>) {
    use inkland_core::config::RoadKind;
    use inkland_core::curves::road_line;
    use std::collections::BTreeSet;
    // Each road is a smooth curve with a seeded wander, as in colour mode.
    let seed = inkland_core::curves::wander_seed(config.seed);
    let min = config.render.min_line_width_pt;
    let style = |kind: RoadKind| match kind {
        RoadKind::Road => (n(line_mm(ROAD_PT.max(min))), [1.6, 0.5]),
        RoadKind::Trail => (n(line_mm(TRAIL_PT.max(min))), [0.5, 0.6]),
    };
    // The dash pattern of a road: exact, or with the changes of a hand.
    let dashes = |key: &str, base: [f64; 2]| -> String {
        match hand {
            Some(h) => dash_list(&h.dashes(key, &base)),
            None => dash_list(&base),
        }
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
        let key = format!("road-{id}");
        let d = hand_path(page, hand, &key, &road_line(seed, &r.points), false);
        if !d.is_empty() {
            items.push((key, d, r.kind));
        }
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));
    for (id, d, kind) in items {
        let (w, base) = style(kind);
        let dash = dashes(&id, base);
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
        let (w, base) = style(kind);
        let name = match kind {
            RoadKind::Road => "road-net",
            RoadKind::Trail => "trail-net",
        };
        for (i, line) in inkland_core::borders::chain_edges(&list).iter().enumerate() {
            let pts: Vec<[f64; 2]> = line
                .iter()
                .map(|&c| world.grid.points[c as usize])
                .collect();
            let id = format!("{name}-{}", i + 1);
            let d = hand_path(page, hand, &id, &road_line(seed, &pts), false);
            if !d.is_empty() {
                let dash = dashes(&id, base);
                let _ = writeln!(
                    s,
                    "<path id=\"{id}\" d=\"{d}\" stroke-width=\"{w}\" stroke-dasharray=\"{dash}\"/>"
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

fn write_places(
    s: &mut String,
    world: &World,
    config: &Config,
    page: &Page,
    labels: &crate::labels::Labels,
) {
    let w = n(line_mm(PLACE_LINE_PT.max(config.render.min_line_width_pt)));
    let [x0, y0, x1, y1] = page.view;
    let mut items: Vec<(String, String)> = world
        .society
        .places
        .iter()
        .filter(|p| p.at[0] >= x0 && p.at[0] <= x1 && p.at[1] >= y0 && p.at[1] <= y1)
        .filter(|p| !labels.hidden_places.contains(&p.id) && !labels.no_symbol.contains(&p.id))
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
/// The smooth curve of a river line on the page: a Catmull-Rom curve with
/// alpha 0.1, as FMG `getRiverPath` draws it. The curve passes through
/// each point of the meandered line.
fn river_curve(page: &Page, points: &[[f64; 2]]) -> String {
    let pts = river_page_points(page, points);
    if pts.len() < 2 {
        return String::new();
    }
    crate::fmg::geom::catmull_rom(&pts, 0.1)
}

/// The points of a river on the page, without repeats of one point.
fn river_page_points(page: &Page, points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(points.len());
    for &p in points {
        let q = page.to_page(p);
        if pts.last() != Some(&q) {
            pts.push(q);
        }
    }
    pts
}

/// A river that a hand draws: the curve of each piece of the wobbled
/// line, with its width in millimetres. The river becomes wider from its
/// source to its mouth, where it is `mouth_pt` wide.
fn river_pieces(
    page: &Page,
    hand: &Hand,
    key: &str,
    points: &[[f64; 2]],
    mouth_pt: f64,
    min_pt: f64,
) -> Vec<(String, f64)> {
    let pts = river_page_points(page, points);
    if pts.len() < 2 {
        return Vec::new();
    }
    Hand::pieces(&hand.wobble(key, &pts, false))
        .into_iter()
        .map(|(piece, t)| {
            let w = line_mm(Hand::river_width(mouth_pt, t, min_pt));
            (crate::fmg::geom::catmull_rom(&piece, 0.1), w)
        })
        .collect()
}

/// Write a line in one or more pieces, each with its stroke width in
/// millimetres. One piece is one path with the id. More pieces are a
/// group with the id, and each piece has the id and its number.
fn write_pieces(s: &mut String, id: &str, pieces: &[(String, f64)]) {
    match pieces {
        [] => {}
        [(d, w)] => {
            let _ = writeln!(
                s,
                "<path id=\"{id}\" d=\"{d}\" stroke-width=\"{}\"/>",
                n(*w)
            );
        }
        _ => {
            let _ = writeln!(s, "<g id=\"{id}\">");
            for (i, (d, w)) in pieces.iter().enumerate() {
                let _ = writeln!(
                    s,
                    "<path id=\"{id}-{}\" d=\"{d}\" stroke-width=\"{}\"/>",
                    i + 1,
                    n(*w)
                );
            }
            s.push_str("</g>\n");
        }
    }
}

fn path_data(page: &Page, points: &[[f64; 2]], closed: bool) -> String {
    let pts: Vec<[f64; 2]> = points.iter().map(|&p| page.to_page(p)).collect();
    mm_path_data(&pts, closed)
}

/// The path data of a line that a hand draws (`render.hand_drawn`), or of
/// the exact line. `key` gives the line its own wobble.
fn hand_path(
    page: &Page,
    hand: Option<&Hand>,
    key: &str,
    points: &[[f64; 2]],
    closed: bool,
) -> String {
    let pts: Vec<[f64; 2]> = points.iter().map(|&p| page.to_page(p)).collect();
    match hand {
        Some(h) => mm_path_data(&h.wobble(key, &pts, closed), closed),
        None => mm_path_data(&pts, closed),
    }
}

/// A dash pattern as the value of `stroke-dasharray`.
fn dash_list(values: &[f64]) -> String {
    values.iter().map(|&v| n(v)).collect::<Vec<_>>().join(" ")
}

/// The `stroke-dasharray` attribute of a line that a hand draws, with the
/// changes of the hand to the base pattern. Without a hand, the line uses
/// the pattern of its group, and the attribute is empty.
fn hand_dashes(hand: Option<&Hand>, key: &str, base: &[f64]) -> String {
    match hand {
        Some(h) => format!(" stroke-dasharray=\"{}\"", dash_list(&h.dashes(key, base))),
        None => String::new(),
    }
}

/// The path data of a line in millimetres on the page. A closed line has
/// its first point again at its end.
fn mm_path_data(points: &[[f64; 2]], closed: bool) -> String {
    let pts: &[[f64; 2]] = if closed && points.len() > 1 {
        &points[..points.len() - 1]
    } else {
        points
    };
    let mut d = String::new();
    let mut last = String::new();
    let mut count = 0;
    for &[x, y] in pts {
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
    fn a_river_curve_passes_through_its_points() {
        let c = config();
        let page = Page::new(&c, &c.variants[0]);
        let pts = [[10.0, 10.0], [10.0, 10.0], [30.0, 20.0], [50.0, 15.0]];
        let d = river_curve(&page, &pts);
        assert!(d.starts_with('M') && d.contains('C'), "{d}");
        assert_eq!(d.matches('C').count(), 2, "{d}");
        assert!(river_curve(&page, &pts[..2]).is_empty());
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
        assert!(river_width_pt(0.0, 0.1, 0.5) >= 0.5);
        assert!(river_width_pt(1e9, 0.1, 0.5) <= RIVER_MAX_PT);
        assert!(river_width_pt(4.0, 0.1, 0.5) > river_width_pt(1.0, 0.1, 0.5));
    }

    #[test]
    fn each_place_has_a_symbol_and_borders_are_drawn() {
        let mut c = config();
        c.world.template = serde_json::from_value(json!("pangea")).unwrap();
        c.world.cells = 5000;
        let w = World::build(&c).unwrap();
        let s = render(&w, &c, &c.variants[0]).svg;
        let page = Page::new(&c, &c.variants[0]);
        let hidden = crate::labels::place(&w, &c, &c.variants[0], &page).hidden_places;
        assert!(w.society.places.len() > 5);
        // A required label can hide the symbol of a generated place.
        for p in w.society.places.iter().filter(|p| !hidden.contains(&p.id)) {
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
    fn the_fractal_coastline_has_a_closed_path_for_each_island() {
        let mut c = config();
        c.render.coastline = Some(inkland_core::config::CoastlineDetail::Fractal);
        let w = World::build(&c).unwrap();
        let s = render(&w, &c, &c.variants[0]).svg;
        let islands = w.features.list.iter().filter(|f| f.kind.is_land()).count();
        assert!(islands > 0);
        assert!(s.matches("id=\"coast-feature-").count() >= islands);
        assert!(!s.contains("id=\"coast-1\""));
        // Fractal detail makes more points than the smooth coast.
        let smooth = render(&w, &config(), &c.variants[0]).svg;
        assert!(s.len() > smooth.len());
        assert_eq!(s, render(&w, &c, &c.variants[0]).svg);
        assert!(roxmltree::Document::parse(&s).is_ok());
    }

    #[test]
    fn same_world_gives_the_same_bytes() {
        assert_eq!(svg().0, svg().0);
    }

    #[test]
    fn a_road_is_drawn_as_the_curve_of_the_core() {
        let mut c = config();
        c.render.hand_drawn = false;
        let mut w = World::build(&c).unwrap();
        w.roads.push(inkland_core::roads::Road {
            id: 99,
            pin: None,
            overlay: Some("bend".into()),
            kind: inkland_core::config::RoadKind::Road,
            group: "roads",
            name: None,
            feature: 0,
            cells: vec![0, 1, 2],
            points: vec![[150.0, 150.0], [200.0, 120.0], [250.0, 150.0]],
            length: 0.0,
            stops: Vec::new(),
            crossings: Vec::new(),
        });
        let svg = render(&w, &c, &c.variants[0]).svg;
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let d = doc
            .descendants()
            .find(|e| e.attribute("id") == Some("road-bend"))
            .and_then(|e| e.attribute("d"))
            .expect("the road has a path");
        // One point on each sample of the curve: no corner at the middle.
        let points = d.matches(['M', 'L']).count();
        assert_eq!(points, 2 * inkland_core::curves::SAMPLES + 1);
    }

    /// The `d` of the element with this id.
    fn path_of(svg: &str, id: &str) -> String {
        let doc = roxmltree::Document::parse(svg).unwrap();
        doc.descendants()
            .find(|e| e.attribute("id") == Some(id))
            .and_then(|e| e.attribute("d"))
            .unwrap_or_else(|| panic!("no path {id}"))
            .to_string()
    }

    #[test]
    fn the_hand_drawn_look_moves_the_lines_and_can_be_turned_off() {
        let mut c = config();
        let w = World::build(&c).unwrap();
        let on = render(&w, &c, &c.variants[0]).svg;
        c.render.hand_drawn = false;
        let off = render(&w, &c, &c.variants[0]).svg;
        let id = format!("coast-{}", w.coastlines[0].id);
        let first = |svg: &str| {
            let piece = format!("{id}-1");
            if svg.contains(&format!("id=\"{piece}\"")) {
                path_of(svg, &piece)
            } else {
                path_of(svg, &id)
            }
        };
        assert_ne!(first(&on), first(&off));
        assert_ne!(path_of(&on, "water-line-1"), path_of(&off, "water-line-1"));
        // The wobble adds points between the points of the exact line.
        assert!(on.matches('L').count() > off.matches('L').count());
        // The coasts taper: the pieces of a coast have more than one width,
        // and no width is less than the minimum.
        let doc = roxmltree::Document::parse(&on).unwrap();
        let widths: std::collections::BTreeSet<String> = doc
            .descendants()
            .filter(|e| e.attribute("id").is_some_and(|i| i.starts_with("coast-")))
            .filter_map(|e| e.attribute("stroke-width"))
            .map(str::to_string)
            .collect();
        assert!(widths.len() > 2, "{widths:?}");
        let min_mm = line_mm(c.render.min_line_width_pt);
        for w in &widths {
            assert!(w.parse::<f64>().unwrap() >= min_mm - 0.001, "{w}");
        }
        // The dash patterns are less regular: each dashed line has its own.
        let patterns = |svg: &str| -> std::collections::BTreeSet<String> {
            let doc = roxmltree::Document::parse(svg).unwrap();
            doc.descendants()
                .filter_map(|e| e.attribute("stroke-dasharray"))
                .map(str::to_string)
                .collect()
        };
        assert!(patterns(&on).len() > patterns(&off).len() + 2);
        // Both pass the print checks.
        for svg in [&on, &off] {
            let opts = crate::checks::CheckOptions {
                min_line_width_pt: c.render.min_line_width_pt,
                fonts_allowed: c.render.fonts_allowed.clone(),
            };
            for r in crate::checks::check_svg(svg, &opts) {
                assert!(r.pass, "{}: {}", r.name, r.details);
            }
        }
    }

    #[test]
    fn a_hand_drawn_river_becomes_wider_to_its_mouth() {
        let c = config();
        let mut w = World::build(&c).unwrap();
        let k = (0..w.rivers.len())
            .max_by_key(|&k| w.rivers[k].points.len())
            .unwrap();
        w.rivers[k].width = 60.0;
        let id = format!("river-{}", w.rivers[k].id);
        let svg = render(&w, &c, &c.variants[0]).svg;
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let group = doc
            .descendants()
            .find(|e| e.attribute("id") == Some(id.as_str()))
            .unwrap();
        let widths: Vec<f64> = group
            .children()
            .filter_map(|e| e.attribute("stroke-width"))
            .map(|v| v.parse().unwrap())
            .collect();
        assert!(widths.len() > 1, "{id} has no pieces");
        assert!(widths.windows(2).all(|p| p[0] <= p[1]), "{widths:?}");
        assert!(widths[0] < *widths.last().unwrap());
        assert!(widths[0] >= line_mm(c.render.min_line_width_pt) - 0.001);
    }

    #[test]
    fn the_water_lines_follow_each_coast_at_each_level() {
        let c = config();
        let w = World::build(&c).unwrap();
        let page = Page::new(&c, &c.variants[0]);
        let lines = crate::water::lines(&w, &page);
        let levels: std::collections::BTreeSet<usize> = lines.iter().map(|l| l.level).collect();
        assert_eq!(levels.len(), crate::water::LEVELS_MM.len());
        // A closed line is a loop.
        for l in lines.iter().filter(|l| l.closed) {
            assert_eq!(l.points.first(), l.points.last());
        }
        let svg = render(&w, &c, &c.variants[0]).svg;
        assert_eq!(svg.matches("id=\"water-line-").count(), lines.len());
    }
}
