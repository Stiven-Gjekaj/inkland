//! Extras: scale bar, compass, title cartouche, and grid.
//!
//! The scale bar uses the scale of the page of the variant, so a crop has a
//! scale bar for its own scale. All text is real `<text>`, with the font
//! family of the config.

use crate::page::{Page, line_mm, pt};
use inkland_core::config::{Config, Extras, Variant};
use inkland_core::num::fixed;
use std::fmt::Write as _;

const INK: &str = "#000000";
const PAPER: &str = "#ffffff";

fn n(v: f64) -> String {
    fixed(v, 3)
}

/// Escape text for XML.
pub fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The extras of a variant: the variant value where it has one, else the
/// render value. The layers `scale-bar` and `compass` turn their extras on.
pub fn merged(config: &Config, variant: &Variant) -> Extras {
    use inkland_core::config::LayerName;
    let r = &config.render.extras;
    let v = &variant.extras;
    // The layers `scale-bar` and `compass` also show them.
    let layer = |l: LayerName| config.render.layers.contains(&l).then_some(true);
    Extras {
        scale_bar: layer(LayerName::ScaleBar).or(v.scale_bar.or(r.scale_bar)),
        compass: layer(LayerName::Compass).or(v.compass.or(r.compass)),
        cartouche: v.cartouche.clone().or_else(|| r.cartouche.clone()),
        grid: v.grid.clone().or_else(|| r.grid.clone()),
    }
}

/// The largest "nice" length (1, 2, or 5 times a power of ten) that is not
/// more than `max`.
pub fn nice_length(max: f64) -> f64 {
    if max <= 0.0 {
        return 0.0;
    }
    let mut p = 1.0;
    while p * 10.0 <= max {
        p *= 10.0;
    }
    while p > max {
        p /= 10.0;
    }
    for m in [5.0, 2.0, 1.0] {
        if m * p <= max {
            return m * p;
        }
    }
    p
}

/// The box `[x0, y0, x1, y1]` of a scale bar with its left end at `x` and
/// its top edge at `y`.
fn scale_bar_box(config: &Config, page: &Page, [x, y]: [f64; 2]) -> [f64; 4] {
    let small = pt(config.render.font_sizes_pt.other);
    let [_, _, mw, _] = page.map_rect();
    let bar = nice_length(0.25 * mw / page.scale) * page.scale;
    let f = crate::metrics::FontMetrics::of(&config.render.fonts.other);
    let last = f.width(&format!("0000 {}s", config.units.name), small, 0.0);
    [
        x - 2.0,
        y - 1.0 - small,
        x + bar + last / 2.0 + 1.0,
        y + 1.6,
    ]
}

/// The left end of the scale bar, on its top edge. The scale bar goes in the
/// bottom-left corner, or in the bottom-right corner if fewer place symbols
/// are there. `symbols` are the place symbols on the page, in mm.
fn scale_bar_origin(config: &Config, page: &Page, symbols: &[[f64; 2]]) -> [f64; 2] {
    let [mx, my, mw, mh] = page.map_rect();
    let left = [mx + 4.0, my + mh - 5.0];
    let [x0, _, x1, _] = scale_bar_box(config, page, left);
    let right = [mx + mw - 4.0 - (x1 - x0) + 2.0, my + mh - 5.0];
    let count = |o: [f64; 2]| {
        let [a, b, c, d] = scale_bar_box(config, page, o);
        symbols
            .iter()
            .filter(|p| p[0] >= a - 2.0 && p[0] <= c + 2.0 && p[1] >= b - 2.0 && p[1] <= d + 2.0)
            .count()
    };
    if count(right) < count(left) {
        right
    } else {
        left
    }
}

/// The centre of the compass.
fn compass_centre(page: &Page) -> [f64; 2] {
    let [mx, my, mw, _] = page.map_rect();
    [mx + mw - 7.0, my + 9.0]
}

/// The cartouche: `[x, y, width, height]`.
fn cartouche_rect(config: &Config, title: &str, page: &Page) -> [f64; 4] {
    let [mx, my, mw, _] = page.map_rect();
    let size = pt(config.render.font_sizes_pt.title);
    let f = crate::metrics::FontMetrics::of(&config.render.fonts.title);
    let w = (f.width(title, size, 0.0) + 6.0).min(mw - 8.0);
    [mx + 4.0, my + 4.0, w, size * 1.8]
}

/// The boxes `[x0, y0, x1, y1]` of the extras of a variant that are on the
/// map, in mm. Labels keep off them. The grid is not in the list.
pub fn boxes(
    config: &Config,
    variant: &Variant,
    page: &Page,
    symbols: &[[f64; 2]],
) -> Vec<[f64; 4]> {
    let e = merged(config, variant);
    let small = pt(config.render.font_sizes_pt.other);
    let mut out = Vec::new();
    if config.render.mode == inkland_core::config::RenderMode::Colour {
        // The scale bar and the compass of the original program.
        out.extend(crate::fmg::layers::boxes(config, variant, page));
        if let Some(c) = &e.cartouche {
            let [x, y, w, h] = cartouche_rect(config, &c.title, page);
            out.push([x - 0.5, y - 0.5, x + w + 0.5, y + h + 0.5]);
        }
        return out;
    }
    if e.scale_bar == Some(true) {
        out.push(scale_bar_box(
            config,
            page,
            scale_bar_origin(config, page, symbols),
        ));
    }
    if e.compass == Some(true) {
        let [cx, cy] = compass_centre(page);
        out.push([cx - 2.5, cy - 6.5 - small, cx + 2.5, cy + 2.5]);
    }
    if let Some(c) = &e.cartouche {
        let [x, y, w, h] = cartouche_rect(config, &c.title, page);
        out.push([x - 0.5, y - 0.5, x + w + 0.5, y + h + 0.5]);
    }
    out
}

/// The parts of the extras to write.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parts {
    pub scale_bar: bool,
    pub compass: bool,
    pub cartouche: bool,
    pub grid: bool,
}

/// Write the extras of a variant into the SVG. `symbols` are the place
/// symbols on the page, in mm.
pub fn write(
    s: &mut String,
    config: &Config,
    variant: &Variant,
    page: &Page,
    symbols: &[[f64; 2]],
) {
    let all = Parts {
        scale_bar: true,
        compass: true,
        cartouche: true,
        grid: true,
    };
    write_only(s, config, variant, page, symbols, all);
}

/// Write the given parts of the extras of a variant into the SVG.
pub fn write_only(
    s: &mut String,
    config: &Config,
    variant: &Variant,
    page: &Page,
    symbols: &[[f64; 2]],
    parts: Parts,
) {
    let mut e = merged(config, variant);
    if !parts.scale_bar {
        e.scale_bar = None;
    }
    if !parts.compass {
        e.compass = None;
    }
    if !parts.cartouche {
        e.cartouche = None;
    }
    if !parts.grid {
        e.grid = None;
    }
    let min = config.render.min_line_width_pt;
    let line = n(line_mm(0.5f64.max(min)));
    let font = xml(&config.render.fonts.other);
    let title_font = xml(&config.render.fonts.title);
    let [mx, my, mw, mh] = page.map_rect();
    let sizes = &config.render.font_sizes_pt;
    let small = pt(sizes.other);

    if let Some(grid) = &e.grid
        && grid.step > 0.0
    {
        let [x0, y0, x1, y1] = page.view;
        let _ = writeln!(
            s,
            "<g id=\"grid\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"{line}\" stroke-dasharray=\"0.4 1.2\" clip-path=\"url(#clip-map)\">"
        );
        let mut k = (x0 / grid.step).ceil();
        while k * grid.step <= x1 {
            let x = page.to_page([k * grid.step, y0])[0];
            let _ = writeln!(s, "<path d=\"M{} {}V{}\"/>", n(x), n(my), n(my + mh));
            k += 1.0;
        }
        let mut k = (y0 / grid.step).ceil();
        while k * grid.step <= y1 {
            let y = page.to_page([x0, k * grid.step])[1];
            let _ = writeln!(s, "<path d=\"M{} {}H{}\"/>", n(mx), n(y), n(mx + mw));
            k += 1.0;
        }
        s.push_str("</g>\n");
        let _ = writeln!(
            s,
            "<g id=\"grid-labels\" font-family=\"{font}\" font-size=\"{}\" fill=\"{INK}\">",
            n(small)
        );
        let mut k = (x0 / grid.step).ceil();
        while k * grid.step <= x1 {
            let x = page.to_page([k * grid.step, y0])[0];
            let _ = writeln!(
                s,
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
                n(x),
                n(my - 0.8),
                xml(&fixed(k * grid.step, 3))
            );
            k += 1.0;
        }
        let mut k = (y0 / grid.step).ceil();
        while k * grid.step <= y1 {
            let y = page.to_page([x0, k * grid.step])[1];
            let _ = writeln!(
                s,
                "<text x=\"{}\" y=\"{}\" text-anchor=\"end\">{}</text>",
                n(mx - 0.8),
                n(y + 0.7),
                xml(&fixed(k * grid.step, 3))
            );
            k += 1.0;
        }
        s.push_str("</g>\n");
    }

    if e.scale_bar == Some(true) {
        // At most a quarter of the map width.
        let length = nice_length(0.25 * mw / page.scale);
        let bar = length * page.scale;
        let [x, y] = scale_bar_origin(config, page, symbols);
        let _ = writeln!(
            s,
            "<g id=\"scale-bar\" stroke=\"{INK}\" stroke-width=\"{line}\">"
        );
        // Paper behind the bar and its labels, so that no map symbol or
        // line goes through them.
        let [bx0, by0, bx1, by1] = scale_bar_box(config, page, [x, y]);
        let _ = writeln!(
            s,
            "<rect id=\"scale-bar-back\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{PAPER}\" stroke=\"none\"/>",
            n(bx0),
            n(by0),
            n(bx1 - bx0),
            n(by1 - by0)
        );
        for i in 0..4 {
            let fill = if i % 2 == 0 { INK } else { PAPER };
            let _ = writeln!(
                s,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"1.2\" fill=\"{fill}\"/>",
                n(x + bar * f64::from(i) / 4.0),
                n(y),
                n(bar / 4.0)
            );
        }
        let unit = xml(&config.units.name);
        let label = |v: f64, x: f64, s: &mut String, last: bool| {
            let text = if last {
                format!("{} {unit}s", fixed(v, 3))
            } else {
                fixed(v, 3)
            };
            let _ = writeln!(
                s,
                "<text x=\"{}\" y=\"{}\" font-family=\"{font}\" font-size=\"{}\" text-anchor=\"middle\" fill=\"{INK}\" stroke=\"none\">{}</text>",
                n(x),
                n(y - 0.8),
                n(small),
                xml(&text)
            );
        };
        label(0.0, x, s, false);
        label(length / 2.0, x + bar / 2.0, s, false);
        label(length, x + bar, s, true);
        s.push_str("</g>\n");
    }

    if e.compass == Some(true) {
        let [cx, cy] = compass_centre(page);
        let _ = writeln!(
            s,
            "<g id=\"compass\" stroke=\"{INK}\" stroke-width=\"{line}\">"
        );
        let _ = writeln!(
            s,
            "<path d=\"M{} {}L{} {}L{} {}Z\" fill=\"{INK}\"/>",
            n(cx),
            n(cy - 5.0),
            n(cx + 1.6),
            n(cy + 2.0),
            n(cx),
            n(cy)
        );
        let _ = writeln!(
            s,
            "<path d=\"M{} {}L{} {}L{} {}Z\" fill=\"{PAPER}\"/>",
            n(cx),
            n(cy - 5.0),
            n(cx - 1.6),
            n(cy + 2.0),
            n(cx),
            n(cy)
        );
        let _ = writeln!(
            s,
            "<text x=\"{}\" y=\"{}\" font-family=\"{font}\" font-size=\"{}\" text-anchor=\"middle\" fill=\"{INK}\" stroke=\"none\">N</text>",
            n(cx),
            n(cy - 6.0),
            n(small)
        );
        s.push_str("</g>\n");
    }

    if let Some(c) = &e.cartouche {
        let size = pt(sizes.title);
        let [x, y, w, _] = cartouche_rect(config, &c.title, page);
        let _ = writeln!(s, "<g id=\"cartouche\">");
        let _ = writeln!(
            s,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{PAPER}\" stroke=\"{INK}\" stroke-width=\"{}\"/>",
            n(x),
            n(y),
            n(w),
            n(size * 1.8),
            n(line_mm(1.0f64.max(min)))
        );
        let _ = writeln!(
            s,
            "<text x=\"{}\" y=\"{}\" font-family=\"{title_font}\" font-size=\"{}\" text-anchor=\"middle\" fill=\"{INK}\">{}</text>",
            n(x + w / 2.0),
            n(y + size * 1.25),
            n(size),
            xml(&c.title)
        );
        s.push_str("</g>\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_lengths() {
        assert_eq!(nice_length(73.0), 50.0);
        assert_eq!(nice_length(20.0), 20.0);
        assert_eq!(nice_length(19.9), 10.0);
        assert_eq!(nice_length(0.7), 0.5);
        assert_eq!(nice_length(3000.0), 2000.0);
    }

    #[test]
    fn xml_escapes_markup() {
        assert_eq!(xml("A & <B>"), "A &amp; &lt;B&gt;");
    }
}
