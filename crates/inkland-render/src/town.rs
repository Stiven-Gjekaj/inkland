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
}

/// The four colours of the palette: paper, light, medium, and dark.
fn palette(colour: bool) -> [&'static str; 4] {
    if colour {
        ["#ccc5b8", "#99948a", "#67635c", "#1a1917"]
    } else {
        ["#ffffff", "#ffffff", "#000000", "#000000"]
    }
}

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

    // The walls, their gates, and their towers.
    let _ = writeln!(s, "<g id=\"town-walls\">");
    let mut wall = |id: &str, wl: &TownWall, large: bool| {
        let _ = writeln!(
            s,
            "<path id=\"{id}\" d=\"{}\" fill=\"none\" stroke=\"{dark}\" stroke-width=\"{}\"/>",
            path(&wl.shape, true),
            n(w(THICK_STROKE))
        );
        let m = wl.shape.len();
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
    s.push_str("</g>\n</g>\n</svg>\n");
    s
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
        })
        .unwrap()
    }

    fn style(colour: bool) -> TownStyle {
        TownStyle {
            colour,
            size_mm: 150.0,
            font: "IM Fell English".to_owned(),
            min_line_pt: 0.25,
        }
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
