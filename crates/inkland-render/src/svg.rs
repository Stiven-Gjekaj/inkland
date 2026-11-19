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
            LayerName::Coast => &["depth-lines", "water-lines", "coast"],
            LayerName::Ocean => &["depth-lines", "water-lines"],
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
                "depth-lines" => write_depth_lines(&mut s, world, config, &page, min),
                "water-lines" => write_water_lines(&mut s, world, &page, min),
                "coast" => write_coast(&mut s, world, config, &page, coast_w),
                "relief" => write_relief(&mut s, world, config, &page, min, &labels),
                "rivers" => write_rivers(&mut s, world, config, variant, &page),
                "borders" => write_borders(&mut s, world, config, &page),
                "roads" => write_roads(&mut s, world, config, &page),
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
    // The halo is a white box behind each label at a point: the box that
    // the placer keeps free for the label. The text has no stroke, because
    // a PDF printer turns stroked text into a Type 3 font. A label on a
    // path gets no box, so that its own river stays whole.
    if halo {
        let _ = writeln!(s, "<g id=\"label-halos\" fill=\"{PAPER}\" stroke=\"none\">");
        for l in &labels.placed {
            if !matches!(l.geometry, Geometry::Point { .. }) {
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

/// Depth lines: thin dashed lines around the deep ocean, one for each depth
/// of `render.depth_lines`.
fn write_depth_lines(s: &mut String, world: &World, config: &Config, page: &Page, min: f64) {
    let depths = &config.render.depth_lines;
    if depths.is_empty() {
        return;
    }
    let width = 0.35f64.max(min);
    let _ = writeln!(
        s,
        "<g id=\"depth-lines\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{}\" stroke-dasharray=\"{} {}\" stroke-linejoin=\"round\">",
        n(line_mm(width)),
        n(line_mm(width * 4.0)),
        n(line_mm(width * 3.0))
    );
    for (k, &depth) in depths.iter().enumerate() {
        for (i, line) in crate::decor::depth_lines(world, depth).iter().enumerate() {
            let pts: Vec<[f64; 2]> = line
                .iter()
                .map(|&v| world.base.grid.vertices[v as usize])
                .collect();
            let closed = line.len() > 2 && line.first() == line.last();
            let smooth = crate::decor::chaikin(&pts, closed, 3);
            let d = path_data(page, &smooth, closed);
            if !d.is_empty() {
                let _ = writeln!(s, "<path id=\"depth-line-{}-{}\" d=\"{d}\"/>", k + 1, i + 1);
            }
        }
    }
    s.push_str("</g>\n");
}

/// Water lines: two thin lines in the water along each coast.
fn write_water_lines(s: &mut String, world: &World, page: &Page, min: f64) {
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
}

fn write_coast(s: &mut String, world: &World, config: &Config, page: &Page, coast_w: f64) {
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
/// `scale` is the factor of `decor::river_scale`; a factor above 1 can go
/// past `RIVER_MAX_PT` by the same factor.
pub fn river_width_pt(width_km: f64, mm_per_px: f64, min_pt: f64, scale: f64) -> f64 {
    let px = 3.0 * libm::pow(width_km.max(0.0), 1.0 / 1.8);
    let w = px * mm_per_px / crate::page::MM_PER_PT;
    (w.clamp(RIVER_MIN_PT, RIVER_MAX_PT) * scale)
        .clamp(RIVER_MIN_PT, RIVER_MAX_PT * scale.max(1.0))
        .max(min_pt)
}

fn write_rivers(s: &mut String, world: &World, config: &Config, variant: &Variant, page: &Page) {
    let mm_per_px = crate::fmg::geom::Px::new(page, config.frame.width, config.frame.height).mm;
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
            let d = river_curve(page, &r.points);
            let scale = crate::decor::river_scale(world, r, config.render.river_contrast);
            (!d.is_empty()).then(|| {
                (
                    id,
                    d,
                    line_mm(river_width_pt(r.width, mm_per_px, min, scale)),
                )
            })
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
        .filter(|p| !labels.hidden_places.contains(&p.id))
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
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(points.len());
    for &p in points {
        let q = page.to_page(p);
        if pts.last() != Some(&q) {
            pts.push(q);
        }
    }
    if pts.len() < 2 {
        return String::new();
    }
    crate::fmg::geom::catmull_rom(&pts, 0.1)
}

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
    fn depth_lines_go_around_the_deep_ocean_and_can_be_turned_off() {
        let (s, w) = svg();
        assert!(s.contains("<g id=\"depth-lines\""));
        assert!(s.contains("id=\"depth-line-1-1\""));
        assert!(s.contains("id=\"depth-line-2-1\""));
        // A deeper line goes around less ocean.
        let cells = |d: f64| {
            (0..w.base.grid.len())
                .filter(|&c| crate::decor::deep(&w, c, d))
                .count()
        };
        assert!(cells(12.0) < cells(6.0) && cells(12.0) > 0);
        let mut c = config();
        c.render.depth_lines = Vec::new();
        let none = render(&w, &c, &c.variants[0]).svg;
        assert!(!none.contains("depth-line"));
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
        assert!(river_width_pt(0.0, 0.1, 0.5, 1.0) >= 0.5);
        assert!(river_width_pt(1e9, 0.1, 0.5, 1.0) <= RIVER_MAX_PT);
        assert!(river_width_pt(4.0, 0.1, 0.5, 1.0) > river_width_pt(1.0, 0.1, 0.5, 1.0));
        // The scale makes a river wider or thinner, never below the minimum.
        assert!(river_width_pt(4.0, 0.1, 0.5, 2.0) > river_width_pt(4.0, 0.1, 0.5, 1.0));
        assert!(river_width_pt(1e9, 0.1, 0.5, 2.0) <= 2.0 * RIVER_MAX_PT);
        assert!(river_width_pt(4.0, 0.1, 0.5, 0.01) >= 0.5);
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
}
