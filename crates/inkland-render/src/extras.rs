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
/// render value.
pub fn merged(config: &Config, variant: &Variant) -> Extras {
    let r = &config.render.extras;
    let v = &variant.extras;
    Extras {
        scale_bar: v.scale_bar.or(r.scale_bar),
        compass: v.compass.or(r.compass),
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

/// Write the extras of a variant into the SVG.
pub fn write(s: &mut String, config: &Config, variant: &Variant, page: &Page) {
    let e = merged(config, variant);
    let min = config.render.min_line_width_pt;
    let line = n(line_mm(0.5f64.max(min)));
    let font = xml(&config.render.fonts.other);
    let title_font = xml(&config.render.fonts.title);
    let [mx, my, mw, mh] = page.map_rect();
    let small = pt(7.0);

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
            n(pt(6.0))
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
        let (x, y) = (mx + 4.0, my + mh - 5.0);
        let _ = writeln!(
            s,
            "<g id=\"scale-bar\" stroke=\"{INK}\" stroke-width=\"{line}\">"
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
        let (cx, cy) = (mx + mw - 7.0, my + 9.0);
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
            n(pt(8.0))
        );
        s.push_str("</g>\n");
    }

    if let Some(c) = &e.cartouche {
        let size = pt(12.0);
        let w = (c.title.chars().count() as f64 * size * 0.55 + 6.0).min(mw - 8.0);
        let (x, y) = (mx + 4.0, my + 4.0);
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
