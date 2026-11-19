//! The `print-safe` drawings of the cells, grid, and coordinates layers:
//! black lines of at least the smallest line width, and real text.

use crate::extras::xml;
use crate::page::{Page, line_mm, pt};
use inkland_core::config::{Config, Origin};
use inkland_core::num::fixed;
use inkland_core::world::World;
use std::fmt::Write as _;

const INK: &str = "#000000";

fn n(v: f64) -> String {
    fixed(v, 3)
}

/// One line of latitude or longitude: its label text and its two ends in
/// world units.
#[derive(Debug, Clone, PartialEq)]
pub struct Graticule {
    pub latitude: bool,
    pub text: String,
    pub from: [f64; 2],
    pub to: [f64; 2],
}

/// The lines of latitude and longitude of the frame (`drawCoordinates` of
/// the original program). The latitudes come from `climate.latitude_range`.
/// The longitudes have the same degrees for each unit, with 0 in the
/// middle. The step is the step of the original program nearest to a tenth
/// of the longitude span.
pub fn graticule(config: &Config) -> Vec<Graticule> {
    let [lat_s, lat_n] = config.world.climate.latitude_range;
    let (w, h) = (config.frame.width, config.frame.height);
    let span = (lat_n - lat_s).abs().max(1e-9);
    let lon_span = w * span / h;
    let lon_w = -lon_span / 2.0;
    let steps = [0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0];
    let goal = lon_span / 10.0;
    let step = steps.iter().copied().fold(steps[0], |a, b| {
        if (b - goal).abs() < (a - goal).abs() {
            b
        } else {
            a
        }
    });
    let (lo, hi) = (lat_s.min(lat_n), lat_s.max(lat_n));
    // World y of a latitude: the north edge is y = 0 with a top-left origin.
    let y_of = |lat: f64| {
        let t = (hi - lat) / span;
        if config.frame.origin == Origin::TopLeft {
            t * h
        } else {
            (1.0 - t) * h
        }
    };
    let deg = |v: f64, pos: &str, neg: &str| {
        if v.fract() == 0.0 && v != 0.0 {
            if v < 0.0 {
                format!("{}°{neg}", fixed(-v, 2))
            } else {
                format!("{}°{pos}", fixed(v, 2))
            }
        } else {
            fixed(v, 2)
        }
    };
    let mut out = Vec::new();
    let mut lat = (lo / step).ceil() * step;
    while lat <= hi + 1e-9 {
        let y = y_of(lat);
        out.push(Graticule {
            latitude: true,
            text: deg(lat, "N", "S"),
            from: [0.0, y],
            to: [w, y],
        });
        lat += step;
    }
    let mut lon = (lon_w / step).ceil() * step;
    while lon <= lon_w + lon_span + 1e-9 {
        let x = (lon - lon_w) / lon_span * w;
        out.push(Graticule {
            latitude: false,
            text: deg(lon, "E", "W"),
            from: [x, 0.0],
            to: [x, h],
        });
        lon += step;
    }
    out
}

/// The outline of each cell, in one path.
pub fn cells(s: &mut String, world: &World, page: &Page, min: f64) {
    let g = &world.grid;
    let [vx0, vy0, vx1, vy1] = page.view;
    let m = g.step_x.max(g.step_y) * 2.0;
    let mut d = String::new();
    for c in 0..g.len() {
        let p = g.points[c];
        if p[0] < vx0 - m || p[0] > vx1 + m || p[1] < vy0 - m || p[1] > vy1 + m {
            continue;
        }
        for (i, &v) in g.polygons[c].iter().enumerate() {
            let q = page.to_page(g.vertices[v as usize]);
            let _ = write!(
                d,
                "{}{} {}",
                if i == 0 { 'M' } else { 'L' },
                n(q[0]),
                n(q[1])
            );
        }
        d.push('Z');
    }
    let _ = writeln!(
        s,
        "<g id=\"cells\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{}\" stroke-linejoin=\"round\"><path d=\"{d}\"/></g>",
        n(line_mm(min))
    );
}

/// A grid of pointed hexagons over the map, at the size of the grid of the
/// original program.
pub fn grid(s: &mut String, config: &Config, page: &Page, min: f64) {
    let px = crate::fmg::geom::Px::new(page, config.frame.width, config.frame.height);
    let k = px.mm;
    let [mx, my, mw, mh] = page.map_rect();
    let _ = writeln!(
        s,
        "<defs><pattern id=\"pattern-hex\" width=\"{}\" height=\"{}\" patternUnits=\"userSpaceOnUse\" patternTransform=\"translate({} {})\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{}\"><path d=\"M0 0L{} {}L{} 0M{} {}V{}M0 {}V{}L{} {}L{} {}V{}\"/></pattern></defs>",
        n(25.0 * k),
        n(43.4 * k),
        n(mx),
        n(my),
        n(line_mm(min)),
        n(12.5 * k),
        n(7.2 * k),
        n(25.0 * k),
        n(12.5 * k),
        n(21.7 * k),
        n(7.2 * k),
        n(43.4 * k),
        n(28.9 * k),
        n(12.5 * k),
        n(21.7 * k),
        n(25.0 * k),
        n(28.9 * k),
        n(43.4 * k)
    );
    let _ = writeln!(
        s,
        "<g id=\"grid-overlay\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"url(#pattern-hex)\" stroke=\"none\"/></g>",
        n(mx),
        n(my),
        n(mw),
        n(mh)
    );
}

/// Dashed lines of latitude and longitude, with labels inside the map edge.
pub fn coordinates(s: &mut String, config: &Config, page: &Page, min: f64) {
    let size = pt(config.render.font_sizes_pt.other);
    let [mx, my, _, _] = page.map_rect();
    let mut d = String::new();
    let mut labels = String::new();
    for g in graticule(config) {
        let a = page.to_page(g.from);
        let b = page.to_page(g.to);
        let _ = write!(d, "M{} {}L{} {}", n(a[0]), n(a[1]), n(b[0]), n(b[1]));
        let (x, y, anchor) = if g.latitude {
            (mx + 1.0, a[1] - 0.6, "start")
        } else {
            (a[0], my + size + 0.6, "middle")
        };
        let _ = writeln!(
            labels,
            "<text x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\">{}</text>",
            n(x),
            n(y),
            xml(&g.text)
        );
    }
    let _ = writeln!(
        s,
        "<g id=\"coordinates\"><g id=\"coordinate-grid\" clip-path=\"url(#clip-map)\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{}\" stroke-dasharray=\"1.5 1.5\"><path d=\"{d}\"/></g>\n<g id=\"coordinate-labels\" font-family=\"{}\" font-size=\"{}\" fill=\"{INK}\">\n{labels}</g>\n</g>",
        n(line_mm(min)),
        xml(&config.render.fonts.other),
        n(size)
    );
}

/// The coats of arms in black and white, with the hatching of Petra
/// Sancta, when `render.emblems.hatching` is set. The places, sizes, and
/// groups are those of `colour` mode (`fmg::layers::marks::layout`).
pub fn emblems(s: &mut String, world: &World, config: &Config, page: &Page, min: f64) {
    if !config.render.emblems.hatching {
        return;
    }
    let Some(list) = crate::fmg::data::emblems(world) else {
        return;
    };
    let px = crate::fmg::geom::Px::new(page, config.frame.width, config.frame.height);
    let placed = crate::fmg::layers::marks::layout(world, config, &px, &list, &|_| 1.0);
    let line = crate::page::line_mm(min);
    let _ = writeln!(s, "<g id=\"emblems\" clip-path=\"url(#clip-map)\">");
    for p in &placed {
        let e = p.emblem;
        let size = p.size * px.mm;
        let prefix = format!("emblem-{}-{}", e.group, e.id);
        let view = crate::emblem::draw(&e.coa, &prefix, &crate::emblem::Look::COLOUR).view;
        // The width of a line in the units of the shield box.
        let look = crate::emblem::Look {
            hatching: true,
            min_line: line * view[2] / size,
        };
        let d = crate::emblem::draw(&e.coa, &prefix, &look);
        let _ = writeln!(
            s,
            "<svg id=\"{prefix}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"{}\">{}</svg>",
            n(p.at[0] * px.mm - size / 2.0),
            n(p.at[1] * px.mm - size / 2.0),
            n(size),
            n(size),
            d.view_box(),
            d.body
        );
    }
    s.push_str("</g>\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config() -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 3, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 2000, "climate": { "latitude_range": [30, 55] } },
            "render": { "layers": ["coast", "cells", "grid", "coordinates"] }
        }))
        .unwrap()
    }

    #[test]
    fn the_graticule_spans_the_latitude_range() {
        let g = graticule(&config());
        let lats: Vec<&str> = g
            .iter()
            .filter(|l| l.latitude)
            .map(|l| l.text.as_str())
            .collect();
        // The longitude span is 400 * 25 / 300 degrees; a tenth of it is
        // nearest to the step 2.
        assert_eq!(lats.first(), Some(&"30°N"));
        assert_eq!(lats.last(), Some(&"54°N"));
        assert_eq!(lats.len(), 13);
        assert!(g.iter().any(|l| !l.latitude && l.text == "0"));
        // North is at the top of a top-left frame.
        let north = g.iter().find(|l| l.text == "54°N").unwrap();
        let south = g.iter().find(|l| l.text == "30°N").unwrap();
        assert!(north.from[1] < south.from[1]);
        assert_eq!(south.from[1], 300.0);
    }

    #[test]
    fn print_safe_layers_pass_the_print_checks() {
        let c = config();
        let w = World::build(&c).unwrap();
        let svg = crate::svg::render(&w, &c, &c.variants[0]).svg;
        for id in ["cells", "grid-overlay", "coordinates"] {
            assert!(svg.contains(&format!("<g id=\"{id}\"")), "{id}");
        }
        let opts = crate::checks::CheckOptions {
            min_line_width_pt: c.render.min_line_width_pt,
            fonts_allowed: c.render.fonts_allowed.clone(),
        };
        for r in crate::checks::check_svg(&svg, &opts) {
            assert!(r.pass, "{} {:?}", r.name, r.details);
        }
        assert_eq!(svg, crate::svg::render(&w, &c, &c.variants[0]).svg);
    }

    #[test]
    fn hatched_emblems_pass_the_print_checks() {
        use inkland_core::config::LayerName;
        let mut c = config();
        c.render.layers = vec![LayerName::Coast, LayerName::Emblems];
        let w = World::build(&c).unwrap();
        // Off by default: no emblems in print-safe mode.
        let svg = crate::svg::render(&w, &c, &c.variants[0]).svg;
        assert!(!svg.contains("<g id=\"emblems\""));
        c.render.emblems.hatching = true;
        c.render.emblems.show_all = true;
        let svg = crate::svg::render(&w, &c, &c.variants[0]).svg;
        for st in &w.society.states {
            assert!(svg.contains(&format!("id=\"emblem-state-{}\"", st.id)));
        }
        let opts = crate::checks::CheckOptions {
            min_line_width_pt: c.render.min_line_width_pt,
            fonts_allowed: c.render.fonts_allowed.clone(),
        };
        for r in crate::checks::check_svg(&svg, &opts) {
            assert!(r.pass, "{} {:?}", r.name, r.details);
        }
        assert_eq!(svg, crate::svg::render(&w, &c, &c.variants[0]).svg);
    }

    #[test]
    fn the_scale_bar_and_compass_layers_show_their_extras() {
        let mut c = config();
        c.render.layers = vec![
            inkland_core::config::LayerName::Coastline,
            inkland_core::config::LayerName::ScaleBar,
            inkland_core::config::LayerName::Compass,
        ];
        let w = World::build(&c).unwrap();
        let svg = crate::svg::render(&w, &c, &c.variants[0]).svg;
        assert!(svg.contains("<g id=\"scale-bar\""));
        // Paper lies behind the bar and its labels, before them.
        let g = &svg[svg.find("<g id=\"scale-bar\"").unwrap()..];
        let g = &g[..g.find("</g>").unwrap()];
        let back = g.find("<rect id=\"scale-bar-back\"").expect("a paper back");
        assert!(back < g.find("<text").unwrap());
        let num = |tag: &str, attr: &str| -> f64 {
            let t = &tag[tag.find(&format!(" {attr}=\"")).unwrap() + attr.len() + 3..];
            t[..t.find('"').unwrap()].parse().unwrap()
        };
        let rect = &g[back..];
        let (bx, by) = (num(rect, "x"), num(rect, "y"));
        let (bw, bh) = (num(rect, "width"), num(rect, "height"));
        for text in g.split("<text").skip(1) {
            let (tx, ty) = (num(text, "x"), num(text, "y"));
            assert!(tx > bx && tx < bx + bw && ty > by && ty < by + bh, "{text}");
        }
        assert!(svg.contains("<g id=\"compass\""));
        assert!(svg.contains("<g id=\"coast\""));
        assert!(!svg.contains("<g id=\"water-lines\""));
    }
}
