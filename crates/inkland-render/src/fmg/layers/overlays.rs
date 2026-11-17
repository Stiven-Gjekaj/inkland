//! Overlay layers: cells, grid, coordinates, temperature, precipitation,
//! and population (`draw-cells.ts`, `draw-grid.ts`,
//! `draw-coordinates.ts`, `draw-temperature.ts`,
//! `draw-precipitation.ts`, `draw-population.ts`).

use super::super::Ctx;
use super::super::color::spectral;
use super::super::geom::{self, basis_closed, n};
use inkland_core::grid::OUTSIDE;
use std::fmt::Write as _;

/// The outline of each cell, in one path (`drawCells`).
pub fn cells(s: &mut String, ctx: &Ctx) {
    let g = &ctx.world.grid;
    let [vx0, vy0, vx1, vy1] = ctx.px.page.view;
    let m = g.step_x.max(g.step_y) * 2.0;
    let mut d = String::new();
    for c in 0..g.len() {
        let p = g.points[c];
        if p[0] < vx0 - m || p[0] > vx1 + m || p[1] < vy0 - m || p[1] > vy1 + m {
            continue;
        }
        for (i, &v) in g.polygons[c].iter().enumerate() {
            let q = ctx.p(g.vertices[v as usize]);
            let _ = write!(
                d,
                "{}{},{}",
                if i == 0 { 'M' } else { 'L' },
                n(q[0]),
                n(q[1])
            );
        }
    }
    let _ = writeln!(
        s,
        "<g id=\"cells\" fill=\"none\"{}><path d=\"{d}\"/></g>",
        ctx.attrs(&["cells"], &[])
    );
}

/// The patterns of the grid layer (`gridPatterns` in `index.html`): size
/// and path.
fn pattern(kind: &str) -> (f64, f64, &'static str) {
    match kind {
        "square" => (25.0, 25.0, "M 25 0 L 0 0 0 25"),
        "flatHex" => (
            43.4,
            25.0,
            "M 43.4,0 36.2,12.5 43.4,25 M 21.7,12.5 H 36.2 Z M 0,0 H 14.5 L 21.7,12.5 14.5,25 H 0",
        ),
        "square45deg" => (
            35.355,
            35.355,
            "M 0 0 L 35.355 35.355 M 0 35.355 L 35.355 0",
        ),
        "squareTruncated" => (
            25.0,
            25.0,
            "M 8.33 25 L 0 16.66 V 8.33 L 8.33 0 16.66 0 25 8.33  M 16.66 25 L 25 16.66 L 25 8.33 M 8.33 25 L 16.66 25",
        ),
        "squareTetrakis" => (
            25.0,
            25.0,
            "M 25 0 L 0 0 0 25 M 0 0 L 25 25 M 0 25 L 25 0 M 12.5 0 L 12.5 25 M 0 12.5 L 25 12.5 M 0 25 L 25 25 L 25 0",
        ),
        "triangleHorizontal" => (
            41.76,
            72.33,
            "M 41.76 36.165 H 0 L 20.88 0 41.76 36.165 20.88 72.33 0 36.165 M 0 0 H 72.33 M 0 72.33 L 41.76 72.33",
        ),
        "triangleVertical" => (
            72.33,
            41.76,
            "M 36.165 0 L 0 20.88 36.165 41.76 72.33 20.88 36.165 0 V 41.76 M 0 0 V 72.33 M 72.33 0 L 72.33 41.76",
        ),
        "trihexagonal" => (
            25.0,
            43.4,
            "M 25 10.85 H 0 L 18.85 43.4 25 32.55 H 0 L 18.85 0 25 10.85",
        ),
        "rhombille" => (
            82.5,
            50.0,
            "M 13.8 50 L 0 25 13.8 0 H 41.2 L 27.5 25 41.2 50 55 25 41.2 0 68.8 0 82.5 25 68.8 50 M 0 25 H 27.5 M 55 25 H 82.5 M 13.8 50 H 41.2 L 68.8 50",
        ),
        _ => (
            25.0,
            43.4,
            "M 0,0 12.5,7.2 25,0 M 12.5,21.7 V 7.2 Z M 0,43.4 V 28.9 L 12.5,21.7 25,28.9 v 14.5",
        ),
    }
}

/// A grid of hexagons or squares over the map (`drawGrid`).
pub fn grid(s: &mut String, ctx: &Ctx) {
    let st = &ctx.style;
    let kind = st
        .option_str(&["grid"], "type")
        .unwrap_or_else(|| "pointyHex".into());
    let scale = st.option_f64(&["grid"], "scale").unwrap_or(1.0);
    let dx = st.option_f64(&["grid"], "dx").unwrap_or(0.0);
    let dy = st.option_f64(&["grid"], "dy").unwrap_or(0.0);
    let (w, h, d) = pattern(&kind);
    let a = |k: &str| st.attr(&["grid"], k);
    let dash = a("stroke-dasharray")
        .map(|v| format!(" stroke-dasharray=\"{v}\""))
        .unwrap_or_default();
    let cap = a("stroke-linecap")
        .map(|v| format!(" stroke-linecap=\"{v}\""))
        .unwrap_or_default();
    let [mx, my, _, _] = ctx.px.map_rect();
    ctx.def(
        "pattern-grid",
        format!(
            "<pattern id=\"pattern-grid\" width=\"{w}\" height=\"{h}\" patternUnits=\"userSpaceOnUse\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"{dash}{cap} patternTransform=\"translate({} {}) scale({}) translate({} {})\"><path d=\"{d}\"/></pattern>",
            a("stroke").unwrap_or_else(|| "#808080".into()),
            a("stroke-width").unwrap_or_else(|| "0.5".into()),
            n(mx),
            n(my),
            n(scale),
            n(dx),
            n(dy)
        ),
    );
    let _ = writeln!(
        s,
        "<g id=\"grid-overlay\"{}><rect {} fill=\"url(#pattern-grid)\" stroke=\"none\"/></g>",
        ctx.attrs(
            &["grid"],
            &[
                "stroke",
                "stroke-width",
                "stroke-dasharray",
                "stroke-linecap"
            ]
        ),
        ctx.map_rect_attrs(0.0)
    );
}

/// Lines of latitude and longitude, with labels at the left and top edges
/// (`drawCoordinates`). See `print_layers::graticule`.
pub fn coordinates(s: &mut String, ctx: &Ctx) {
    let size = ctx
        .style
        .attr_f64(&["coordinates"], "font-size")
        .unwrap_or(12.0);
    let [mx, my, _, _] = ctx.px.map_rect();
    let mut d = String::new();
    let mut labels = String::new();
    for g in crate::print_layers::graticule(ctx.config) {
        let a = ctx.p(g.from);
        let b = ctx.p(g.to);
        let _ = write!(d, "M{},{}L{},{}", n(a[0]), n(a[1]), n(b[0]), n(b[1]));
        let (x, y) = if g.latitude {
            (mx + size + 2.0, a[1])
        } else {
            (a[0], my + size * 1.1)
        };
        let _ = writeln!(
            labels,
            "<text x=\"{}\" y=\"{}\">{}</text>",
            n(x),
            n(y),
            crate::extras::xml(&g.text)
        );
    }
    let _ = writeln!(
        s,
        "<g id=\"coordinates\"{}>\n<g id=\"coordinate-grid\" fill=\"none\"><path d=\"{d}\"/></g>\n<g id=\"coordinate-labels\" stroke=\"none\" fill=\"#3f3f3f\" text-anchor=\"middle\">\n{labels}</g>\n</g>",
        ctx.attrs(&["coordinates"], &[])
    );
}

/// Temperature: shapes of equal temperature, coloured by temperature
/// (`drawTemperature`).
pub fn temperature(s: &mut String, ctx: &Ctx) {
    let w = ctx.world;
    let temp = &w.temperature;
    let tmin = temp.iter().copied().fold(f64::INFINITY, f64::min).round();
    let tmax = temp
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .round();
    if !tmin.is_finite() || !tmax.is_finite() {
        return;
    }
    let step = ((tmax - tmin).abs() / 5.0).round().max(1.0);
    let scheme = |t: f64| spectral(1.0 - (t + 50.0) / 100.0);
    let _ = writeln!(
        s,
        "<g id=\"temperature\" fill-rule=\"evenodd\" text-anchor=\"middle\" dominant-baseline=\"central\"{}>",
        ctx.attrs(&["temperature"], &[])
    );
    let _ = writeln!(
        s,
        "<rect {} fill=\"{}\" stroke=\"none\"/>",
        ctx.map_rect_attrs(10.0),
        scheme(tmin).hex()
    );
    let mut labels = String::new();
    let mut t = tmin + step;
    while t < tmax {
        let rings = geom::rings(w, &|c| u32::from(temp[c].round() >= t));
        let mut d = String::new();
        for ring in rings.get(&1).into_iter().flatten() {
            let pts: Vec<[f64; 2]> = ring
                .iter()
                .enumerate()
                .filter(|(i, e)| i % 4 == 0 || e.other == OUTSIDE)
                .map(|(_, e)| ctx.p(w.grid.vertices[e.from as usize]))
                .collect();
            if pts.len() < 6 {
                continue;
            }
            d.push_str(&basis_closed(&pts));
            // A label at the top point of the shape.
            if let Some(top) = pts.iter().copied().min_by(|a, b| a[1].total_cmp(&b[1])) {
                let _ = writeln!(
                    labels,
                    "<text x=\"{}\" y=\"{}\">{}°C</text>",
                    n(top[0]),
                    n(top[1]),
                    inkland_core::num::fixed(t, 0)
                );
            }
        }
        if !d.is_empty() {
            let fill = scheme(t);
            let _ = writeln!(
                s,
                "<path d=\"{d}\" fill=\"{}\" stroke=\"{}\"/>",
                fill.hex(),
                fill.darker(0.2).hex()
            );
        }
        t += step;
    }
    let _ = writeln!(
        s,
        "<g id=\"temp-labels\" fill-opacity=\"1\" stroke=\"#ffffff\" stroke-width=\"2\" paint-order=\"stroke\">\n{labels}</g>"
    );
    s.push_str("</g>\n");
}

/// Precipitation: a circle on each land cell, larger for more rain
/// (`drawPrecipitation`).
pub fn precipitation(s: &mut String, ctx: &Ctx) {
    let w = ctx.world;
    let g = &w.grid;
    let modifier = libm::pow(g.len() as f64 / 10_000.0, 0.25);
    let [vx0, vy0, vx1, vy1] = ctx.px.page.view;
    let _ = writeln!(s, "<g id=\"prec\"{}>", ctx.attrs(&["precipitation"], &[]));
    for c in 0..g.len() {
        let p = g.points[c];
        if !w.features.is_land(c) || w.precipitation[c] <= 0.0 {
            continue;
        }
        if p[0] < vx0 || p[0] > vx1 || p[1] < vy0 || p[1] > vy1 {
            continue;
        }
        let r = inkland_core::num::round((w.precipitation[c] / 4.0).sqrt() / modifier, 2);
        let q = ctx.p(p);
        let _ = writeln!(
            s,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
            n(q[0]),
            n(q[1]),
            n(r)
        );
    }
    s.push_str("</g>\n");
}

/// Population: a bar on each cell and each place (`drawPopulation`). The
/// world has no population yet; see `data::population`.
pub fn population(s: &mut String, ctx: &Ctx) {
    if let Some(pop) = super::super::data::population(ctx.world) {
        write_population(s, ctx, &pop);
    }
}

/// Write population bars.
pub fn write_population(s: &mut String, ctx: &Ctx, pop: &super::super::data::Population) {
    let w = ctx.world;
    let modifier = ctx
        .style
        .option_f64(&["population"], "barHeight")
        .unwrap_or(1.0);
    let _ = writeln!(
        s,
        "<g id=\"population\" fill=\"none\"{}>",
        ctx.attrs(&["population"], &[])
    );
    let _ = writeln!(
        s,
        "<g id=\"rural\"{}>",
        ctx.attrs(&["population", "rural"], &[])
    );
    for (c, &v) in pop.rural.iter().enumerate() {
        if v <= 0.0 || c >= w.grid.len() {
            continue;
        }
        let q = ctx.p(w.grid.points[c]);
        let _ = writeln!(
            s,
            "<line x1=\"{x}\" y1=\"{}\" x2=\"{x}\" y2=\"{}\"/>",
            n(q[1]),
            n(q[1] - v / 5.0 * modifier),
            x = n(q[0])
        );
    }
    s.push_str("</g>\n");
    let _ = writeln!(
        s,
        "<g id=\"urban\"{}>",
        ctx.attrs(&["population", "urban"], &[])
    );
    for &(at, v) in &pop.urban {
        let q = ctx.p(at);
        let _ = writeln!(
            s,
            "<line x1=\"{x}\" y1=\"{}\" x2=\"{x}\" y2=\"{}\"/>",
            n(q[1]),
            n(q[1] - v / 5.0 * modifier),
            x = n(q[0])
        );
    }
    s.push_str("</g>\n</g>\n");
}
