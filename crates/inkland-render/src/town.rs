//! The SVG of a town map.
//!
//! The drawing follows `com.watabou.towngenerator.mapping.CityMap` of
//! TownGeneratorOS by Watabou (GPL-3.0,
//! <https://github.com/watabou/TownGeneratorOS>): the roads, the buildings
//! of each ward, then the walls with their gates and towers. `colour` uses
//! the default palette of the original. `print-safe` is black and white:
//! white buildings with black outlines.

use crate::page::line_mm;
use inkland_core::town::{Kind, P, Town, TownWall};
use std::fmt::Write as _;

/// A normal stroke of the original, in town units.
const NORMAL_STROKE: f64 = 0.3;
/// A thick stroke of the original: the walls.
const THICK_STROKE: f64 = 1.8;
/// The width of a main street, in town units.
const MAIN_STREET: f64 = 2.0;
/// The height of the title band above the map, in mm.
const TITLE_MM: f64 = 12.0;
/// The part of the town radius that the map shows around the centre.
const VIEW: f64 = 1.25;

/// The look of a town map.
#[derive(Debug, Clone, PartialEq)]
pub struct TownStyle {
    /// `true`: the colours of the original. `false`: black and white.
    pub colour: bool,
    /// The width of the map, in mm. The page is this wide, and the title
    /// band is above the map.
    pub size_mm: f64,
    /// The font family of the title.
    pub font: String,
    /// The least width of a line, in points.
    pub min_line_pt: f64,
    /// The name of the distance unit of the world, for example `mile`. A
    /// scale bar in feet goes with an imperial unit, and one in metres goes
    /// with all other units.
    pub unit: String,
}

/// inkland: the length of a town unit, in metres. A main street is then
/// 6 m wide.
const METRES_PER_UNIT: f64 = 3.0;

/// inkland: the unit of the scale bar of a town map for a world unit: its
/// name and its length in metres.
fn bar_unit(world_unit: &str) -> (&'static str, f64) {
    let u = world_unit.to_lowercase();
    if ["mile", "league", "foot", "feet", "yard", "furlong"]
        .iter()
        .any(|w| u.contains(w))
    {
        ("feet", 0.3048)
    } else {
        ("metres", 1.0)
    }
}

/// The four colours of the palette: paper, light, medium, and dark.
fn palette(colour: bool) -> [&'static str; 4] {
    if colour {
        ["#ccc5b8", "#99948a", "#67635c", "#1a1917"]
    } else {
        ["#ffffff", "#ffffff", "#000000", "#000000"]
    }
}

/// inkland: the colour of the water. Black and white has white water with
/// a black shore.
fn water_colour(colour: bool) -> &'static str {
    if colour { "#9fadab" } else { "#ffffff" }
}

/// inkland: the width of a river, in town units.
const RIVER: f64 = 4.0;

fn n(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let r = if r == 0.0 { 0.0 } else { r };
    format!("{r}")
}

/// The SVG of a town map with its name in the title.
pub fn town_svg(town: &Town, name: &str, style: &TownStyle) -> String {
    let [paper, light, medium, dark] = palette(style.colour);
    let size = style.size_mm;
    let r = (town.radius * VIEW).max(1.0);
    let k = size / (2.0 * r);
    let top = TITLE_MM;
    let to = |p: P| -> [f64; 2] { [(p[0] + r) * k, (p[1] + r) * k + top] };
    let min = line_mm(style.min_line_pt);
    let w = |units: f64| (units * k).max(min);
    let path = |pts: &[P], close: bool| -> String {
        let mut d = String::new();
        for (i, p) in pts.iter().enumerate() {
            let [x, y] = to(*p);
            let _ = write!(d, "{}{} {}", if i == 0 { "M" } else { "L" }, n(x), n(y));
        }
        if close {
            d.push('Z');
        }
        d
    };
    let mut s = String::new();
    let height = size + top;
    let _ = writeln!(
        s,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">",
        w = n(size),
        h = n(height)
    );
    let _ = writeln!(
        s,
        "<defs><clipPath id=\"town-clip\"><rect x=\"0\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath></defs>",
        n(top),
        n(size),
        n(size)
    );
    let _ = writeln!(
        s,
        "<rect id=\"town-paper\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{paper}\"/>",
        n(size),
        n(height)
    );
    let _ = writeln!(
        s,
        "<text id=\"town-title\" x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"6\" text-anchor=\"middle\" fill=\"{dark}\">{}</text>",
        n(size / 2.0),
        n(top - 3.5),
        xml(&style.font),
        xml(name)
    );
    let _ = writeln!(s, "<g id=\"town-map\" clip-path=\"url(#town-clip)\">");

    // inkland: the sea, then the river: a dark bank line on each side.
    let water = water_colour(style.colour);
    if !town.water.is_empty() {
        let d: String = town.water.iter().map(|p| path(p, true)).collect();
        let _ = writeln!(
            s,
            "<path id=\"town-sea\" d=\"{d}\" fill=\"{water}\" stroke=\"{water}\" stroke-width=\"{}\"/>",
            n(w(NORMAL_STROKE))
        );
    }
    if let Some(river) = &town.river {
        let d = path(river, false);
        let _ = writeln!(
            s,
            "<g id=\"town-river\" fill=\"none\" stroke-linejoin=\"round\" stroke-linecap=\"round\"><path d=\"{d}\" stroke=\"{dark}\" stroke-width=\"{}\"/><path d=\"{d}\" stroke=\"{water}\" stroke-width=\"{}\"/></g>",
            n(w(RIVER + NORMAL_STROKE * 2.0)),
            n(w(RIVER))
        );
    }

    // inkland: the fields of the farms, under the roads.
    if !town.fields.is_empty() {
        let d: String = town.fields.iter().map(|f| path(f, true)).collect();
        let fill = if style.colour { "#c2bba9" } else { "none" };
        let _ = writeln!(
            s,
            "<path id=\"town-fields\" d=\"{d}\" fill=\"{fill}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"/>",
            if style.colour { light } else { dark },
            n(w(NORMAL_STROKE * 0.5))
        );
    }

    // The roads: a dark line with a paper line inside.
    let _ = writeln!(
        s,
        "<g id=\"town-roads\" fill=\"none\" stroke-linejoin=\"round\">"
    );
    for road in &town.roads {
        let d = path(road, false);
        let _ = writeln!(
            s,
            "<path d=\"{d}\" stroke=\"{medium}\" stroke-width=\"{}\"/>",
            n(w(MAIN_STREET + NORMAL_STROKE))
        );
        let _ = writeln!(
            s,
            "<path d=\"{d}\" stroke=\"{paper}\" stroke-width=\"{}\"/>",
            n(w(MAIN_STREET - NORMAL_STROKE))
        );
    }
    s.push_str("</g>\n");

    // The wards.
    let _ = writeln!(s, "<g id=\"town-wards\" stroke-linejoin=\"miter\">");
    for (i, p) in town.patches.iter().enumerate() {
        if p.buildings.is_empty() {
            continue;
        }
        let d: String = p.buildings.iter().map(|b| path(b, true)).collect();
        let class = p.kind.label().to_ascii_lowercase();
        match p.kind {
            Kind::Castle | Kind::Cathedral => {
                // A thick outline under a fill, as `drawBuilding`.
                let t = if p.kind == Kind::Castle { 2.0 } else { 1.0 };
                let _ = writeln!(
                    s,
                    "<g id=\"town-ward-{i}\" class=\"{class}\"><path d=\"{d}\" fill=\"none\" stroke=\"{dark}\" stroke-width=\"{}\"/><path d=\"{d}\" fill=\"{light}\" stroke=\"none\"/></g>",
                    n(w(NORMAL_STROKE * t * 2.0))
                );
            }
            Kind::Park => {
                let (fill, stroke) = if style.colour {
                    (medium, medium)
                } else {
                    ("none", dark)
                };
                let _ = writeln!(
                    s,
                    "<path id=\"town-ward-{i}\" class=\"{class}\" d=\"{d}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{}\"/>",
                    n(w(NORMAL_STROKE))
                );
            }
            _ => {
                let _ = writeln!(
                    s,
                    "<path id=\"town-ward-{i}\" class=\"{class}\" d=\"{d}\" fill=\"{light}\" stroke=\"{dark}\" stroke-width=\"{}\"/>",
                    n(w(NORMAL_STROKE))
                );
            }
        }
    }
    s.push_str("</g>\n");

    // inkland: the piers and the bridges.
    if !town.piers.is_empty() {
        let d: String = town.piers.iter().map(|p| path(p, true)).collect();
        let _ = writeln!(
            s,
            "<path id=\"town-piers\" d=\"{d}\" fill=\"{light}\" stroke=\"{dark}\" stroke-width=\"{}\"/>",
            n(w(NORMAL_STROKE))
        );
    }
    if !town.bridges.is_empty() {
        let _ = writeln!(s, "<g id=\"town-bridges\">");
        for (i, b) in town.bridges.iter().enumerate() {
            // A deck across the river, along the street.
            let along = [
                b.dir[0] * (RIVER / 2.0 + 1.5),
                b.dir[1] * (RIVER / 2.0 + 1.5),
            ];
            let side = [-b.dir[1] * MAIN_STREET * 0.6, b.dir[0] * MAIN_STREET * 0.6];
            let deck = [
                [b.at[0] - along[0] - side[0], b.at[1] - along[1] - side[1]],
                [b.at[0] + along[0] - side[0], b.at[1] + along[1] - side[1]],
                [b.at[0] + along[0] + side[0], b.at[1] + along[1] + side[1]],
                [b.at[0] - along[0] + side[0], b.at[1] - along[1] + side[1]],
            ];
            let _ = writeln!(
                s,
                "<path id=\"town-bridge-{i}\" d=\"{}\" fill=\"{paper}\" stroke=\"{dark}\" stroke-width=\"{}\"/>",
                path(&deck, true),
                n(w(NORMAL_STROKE))
            );
        }
        s.push_str("</g>\n");
    }

    // The walls, their gates, and their towers.
    let _ = writeln!(s, "<g id=\"town-walls\">");
    let mut wall = |id: &str, wl: &TownWall, large: bool| {
        // The parts of the wall that stand: a shore has none.
        let m = wl.shape.len();
        let mut d = String::new();
        for i in 0..m {
            if wl.segments.get(i).copied().unwrap_or(true) {
                d.push_str(&path(&[wl.shape[i], wl.shape[(i + 1) % m]], false));
            }
        }
        let _ = writeln!(
            s,
            "<path id=\"{id}\" d=\"{d}\" fill=\"none\" stroke=\"{dark}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>",
            n(w(THICK_STROKE))
        );
        for (g, gate) in wl.gates.iter().enumerate() {
            let Some(i) = wl.shape.iter().position(|v| v == gate) else {
                continue;
            };
            let (a, b) = (wl.shape[(i + 1) % m], wl.shape[(i + m - 1) % m]);
            let dir = [a[0] - b[0], a[1] - b[1]];
            let l = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt().max(1e-9);
            let f = THICK_STROKE * 1.5 / l;
            let (p, q) = (
                [gate[0] - dir[0] * f, gate[1] - dir[1] * f],
                [gate[0] + dir[0] * f, gate[1] + dir[1] * f],
            );
            let _ = writeln!(
                s,
                "<path id=\"{id}-gate-{g}\" d=\"{}\" stroke=\"{dark}\" stroke-width=\"{}\" stroke-linecap=\"butt\"/>",
                path(&[p, q], false),
                n(w(THICK_STROKE * 2.0))
            );
        }
        let tr = THICK_STROKE * if large { 1.5 } else { 1.0 };
        for t in &wl.towers {
            let [x, y] = to(*t);
            let _ = writeln!(
                s,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{dark}\"/>",
                n(x),
                n(y),
                n(tr * k)
            );
        }
    };
    if let Some(wl) = &town.wall {
        wall("town-wall", wl, false);
    }
    if let Some(wl) = &town.castle {
        wall("town-castle-wall", wl, true);
    }
    s.push_str("</g>\n");
    district_labels(&mut s, town, style, &to, paper, dark);
    s.push_str("</g>\n");
    scale_and_compass(&mut s, style, k, paper, dark);
    s.push_str("</svg>\n");
    s
}

/// inkland: a scale bar at the bottom left of the map and a compass at the
/// bottom right, each on a paper box. `k` is mm for each town unit.
fn scale_and_compass(s: &mut String, style: &TownStyle, k: f64, paper: &str, dark: &str) {
    let size = style.size_mm;
    let font = (size / 62.0).clamp(2.0, 3.5);
    let line = line_mm(style.min_line_pt).max(0.15);
    let (name, metres) = bar_unit(&style.unit);
    // mm on the page for one unit of the bar.
    let mm = k * metres / METRES_PER_UNIT;
    let length = crate::extras::nice_length(0.25 * size / mm);
    let bar = length * mm;
    let bottom = TITLE_MM + size;
    let (x, y) = (5.0, bottom - 5.0);
    let _ = writeln!(s, "<g id=\"town-scale-bar\">");
    let _ = writeln!(
        s,
        "<rect id=\"town-scale-bar-back\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{paper}\"/>",
        n(x - 2.0),
        n(y - font - 2.0),
        n(bar + 4.0 + font * 3.0),
        n(font + 5.0)
    );
    for i in 0..4 {
        let fill = if i % 2 == 0 { dark } else { paper };
        let _ = writeln!(
            s,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"1\" fill=\"{fill}\" stroke=\"{dark}\" stroke-width=\"{}\"/>",
            n(x + bar * f64::from(i) / 4.0),
            n(y),
            n(bar / 4.0),
            n(line)
        );
    }
    let num = |v: f64| inkland_core::num::fixed(v, 3);
    for (v, at, last) in [
        (0.0, x, false),
        (length / 2.0, x + bar / 2.0, false),
        (length, x + bar, true),
    ] {
        let text = if last {
            format!("{} {name}", num(v))
        } else {
            num(v)
        };
        let _ = writeln!(
            s,
            "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" text-anchor=\"{}\" fill=\"{dark}\">{}</text>",
            n(at),
            n(y - 0.8),
            xml(&style.font),
            n(font),
            if last { "start" } else { "middle" },
            xml(&text)
        );
    }
    s.push_str("</g>\n");
    // The compass: a north arrow with an N above it.
    let (cx, cy) = (size - 8.0, bottom - 9.0);
    let _ = writeln!(s, "<g id=\"town-compass\">");
    let _ = writeln!(
        s,
        "<circle id=\"town-compass-back\" cx=\"{}\" cy=\"{}\" r=\"6\" fill=\"{paper}\"/>",
        n(cx),
        n(cy - 0.5)
    );
    let _ = writeln!(
        s,
        "<path d=\"M{} {}L{} {}L{} {}Z\" fill=\"{dark}\" stroke=\"{dark}\" stroke-width=\"{}\" stroke-linejoin=\"round\"/>",
        n(cx),
        n(cy - 3.0),
        n(cx + 1.4),
        n(cy + 3.0),
        n(cx),
        n(cy + 1.8),
        n(line)
    );
    let _ = writeln!(
        s,
        "<path d=\"M{} {}L{} {}L{} {}Z\" fill=\"{paper}\" stroke=\"{dark}\" stroke-width=\"{}\" stroke-linejoin=\"round\"/>",
        n(cx),
        n(cy - 3.0),
        n(cx - 1.4),
        n(cy + 3.0),
        n(cx),
        n(cy + 1.8),
        n(line)
    );
    let _ = writeln!(
        s,
        "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" text-anchor=\"middle\" fill=\"{dark}\">N</text>",
        n(cx),
        n(cy - 3.6),
        xml(&style.font),
        n(font)
    );
    s.push_str("</g>\n");
}

/// inkland: the names of the districts, in capitals on a paper box.
/// The larger districts come first. A label that would cover a label
/// before it tries the other patches of its district, and is left out if
/// no patch gives it space.
fn district_labels(
    s: &mut String,
    town: &Town,
    style: &TownStyle,
    to: &dyn Fn(P) -> [f64; 2],
    paper: &str,
    dark: &str,
) {
    if town.districts.is_empty() {
        return;
    }
    let size = (style.size_mm / 62.0).clamp(2.0, 3.5);
    let spacing = size * 0.12;
    let metrics = crate::metrics::FontMetrics::of(&style.font);
    let cap = metrics.cap(size);
    let pad = size * 0.18;
    let (lo, hi) = (TITLE_MM, TITLE_MM + style.size_mm);
    let mut order: Vec<usize> = (0..town.districts.len()).collect();
    order.sort_by(|&a, &b| {
        town.districts[b]
            .area
            .total_cmp(&town.districts[a].area)
            .then(a.cmp(&b))
    });
    let mut boxes: Vec<[f64; 4]> = Vec::new();
    let mut texts = String::new();
    for i in order {
        let d = &town.districts[i];
        let text = d.name.to_uppercase();
        let width = metrics.width(&text, size, spacing) + 2.0 * pad;
        let mut spots = vec![d.label];
        spots.extend(
            d.patches
                .iter()
                .map(|&p| inkland_core::town::poly::centroid(&town.patches[p].shape)),
        );
        let fits = |b: &[f64; 4]| {
            b[0] >= 0.0
                && b[2] <= style.size_mm
                && b[1] >= lo
                && b[3] <= hi
                && !boxes
                    .iter()
                    .any(|o| b[0] < o[2] && o[0] < b[2] && b[1] < o[3] && o[1] < b[3])
        };
        let found = spots
            .into_iter()
            .map(|p| {
                let [x, y] = to(p);
                let h = cap / 2.0 + pad;
                ([x, y], [x - width / 2.0, y - h, x + width / 2.0, y + h])
            })
            .find(|(_, b)| fits(b));
        let Some(([x, y], b)) = found else {
            continue;
        };
        boxes.push(b);
        let _ = writeln!(
            texts,
            "<text id=\"town-district-{i}\" x=\"{}\" y=\"{}\">{}</text>",
            n(x),
            n(y + cap / 2.0),
            xml(&text)
        );
    }
    // The paper box behind each label keeps it clear of the buildings.
    let _ = writeln!(s, "<g id=\"town-district-boxes\" fill=\"{paper}\">");
    for (i, b) in boxes.iter().enumerate() {
        let _ = writeln!(
            s,
            "<rect id=\"town-district-box-{i}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"/>",
            n(b[0]),
            n(b[1]),
            n(b[2] - b[0]),
            n(b[3] - b[1]),
            n(pad)
        );
    }
    s.push_str("</g>\n");
    let _ = writeln!(
        s,
        "<g id=\"town-districts\" font-family=\"{}\" font-size=\"{}\" letter-spacing=\"{}\" text-anchor=\"middle\" fill=\"{dark}\">",
        xml(&style.font),
        n(size),
        n(spacing)
    );
    s.push_str(&texts);
    s.push_str("</g>\n");
}

fn xml(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkland_core::town::{TownOptions, generate};

    fn town() -> Town {
        generate(&TownOptions {
            patches: 15,
            seed: 4242,
            plaza: Some(true),
            citadel: Some(true),
            walls: Some(true),
            lots: false,
            coast: None,
            river: None,
            names: 1,
            uphill: None,
            roads: Vec::new(),
        })
        .unwrap()
    }

    fn style(colour: bool) -> TownStyle {
        TownStyle {
            colour,
            size_mm: 150.0,
            font: "IM Fell English".to_owned(),
            min_line_pt: 0.25,
            unit: "mile".to_owned(),
        }
    }

    #[test]
    fn the_scale_bar_uses_feet_for_miles_and_metres_else() {
        assert_eq!(bar_unit("mile").0, "feet");
        assert_eq!(bar_unit("League").0, "feet");
        assert_eq!(bar_unit("km").0, "metres");
        let svg = town_svg(&town(), "Kingsholm", &style(true));
        assert!(svg.contains("id=\"town-scale-bar\""));
        assert!(svg.contains(" feet</text>"));
        assert!(svg.contains("id=\"town-compass\""));
        assert!(svg.contains("id=\"town-fields\""));
    }

    #[test]
    fn a_print_safe_town_map_passes_the_print_checks() {
        let svg = town_svg(&town(), "Kingsholm", &style(false));
        let opts = crate::checks::CheckOptions {
            min_line_width_pt: 0.25,
            fonts_allowed: vec!["IM Fell English".to_owned()],
        };
        for r in crate::checks::check_svg(&svg, &opts) {
            assert!(r.pass, "{} {:?}", r.name, r.details);
        }
        assert!(svg.contains(">Kingsholm</text>"));
        assert!(svg.contains("id=\"town-wall\""));
        assert!(svg.contains("id=\"town-castle-wall\""));
        assert!(svg.contains("class=\"market\""));
    }

    #[test]
    fn a_colour_town_map_uses_the_palette_of_the_original() {
        let svg = town_svg(&town(), "Kingsholm", &style(true));
        assert!(svg.contains("fill=\"#ccc5b8\""));
        assert!(svg.contains("fill=\"#99948a\""));
        roxmltree::Document::parse(&svg).expect("valid XML");
        // The same town and style give the same bytes.
        assert_eq!(svg, town_svg(&town(), "Kingsholm", &style(true)));
    }
}
