//! Labels in `colour` mode: the places of the label placer of inkland, with
//! the label styles of the original program (`labels-renderer.ts`,
//! `label-groups.ts`, `label-markup.ts`).
//!
//! A label group gets its colours, its font, and its outline from the
//! style. The CSS `text-shadow` of a group becomes a halo of the shadow
//! colour behind the text, as the white halo of `print-safe` mode.

use super::super::Ctx;
use super::super::geom::n;
use super::super::icons::kebab;
use crate::extras::xml;
use crate::labels::Geometry;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The colour and the blur of the first shadow of a CSS `text-shadow`.
pub fn text_shadow(css: &str) -> Option<(String, f64)> {
    let decl = css
        .split(';')
        .find_map(|d| d.trim().strip_prefix("text-shadow:"))?;
    let first = decl.split(',').next()?.trim();
    let mut color = None;
    let mut lengths = Vec::new();
    for word in first.split_whitespace() {
        match super::super::style::number(word) {
            Some(v)
                if word.ends_with("px") || word.trim_start_matches('-') == "0" || word == "0" =>
            {
                lengths.push(v)
            }
            _ => color = Some(word.to_owned()),
        }
    }
    let blur = lengths.get(2).copied().unwrap_or(0.0);
    Some((color?, blur))
}

/// The outline of a label group of a style: the stroke width as a part of
/// the font size, and the colour of a halo that the CSS shadow gives.
/// `None` for a group without an outline and without a shadow.
pub fn outline(style: &crate::fmg::style::Style, group: &str) -> Option<(f64, Option<String>)> {
    let path = ["labels", group];
    let stroke_w = style.attr_f64(&path, "stroke-width").unwrap_or(0.0);
    if stroke_w > 0.0 {
        // The size of the group in the original program, in pixels.
        let fmg_size = style
            .attr_f64(&path, "font-size")
            .filter(|v| *v > 0.0)
            .unwrap_or(5.0);
        return Some((stroke_w / fmg_size, None));
    }
    let css = style.attr(&path, "style").unwrap_or_default();
    let (color, blur) = text_shadow(&css)?;
    Some((0.12 + 0.02 * blur.min(6.0), Some(color)))
}

/// How far the outline of a label group reaches out of the glyphs, as a
/// part of the font size.
pub fn halo(style: &crate::fmg::style::Style, group: &str) -> f64 {
    outline(style, group).map_or(0.0, |(w, _)| w / 2.0)
}

/// The label groups of the style that the placed labels use, in a fixed
/// order.
const GROUPS: [&str; 10] = [
    "state", "added", "province", "river", "capital", "city", "town", "village", "fort", "hamlet",
];

/// Labels.
pub fn labels(s: &mut String, ctx: &Ctx) {
    let k = ctx.px.mm;
    let placed = &ctx.labels.placed;
    // Text paths of river labels.
    let mut paths = String::new();
    for l in placed {
        if let Geometry::Path { points, .. } = &l.geometry {
            let mut d = String::new();
            for (i, p) in points.iter().enumerate() {
                let _ = write!(
                    d,
                    "{}{},{}",
                    if i == 0 { 'M' } else { 'L' },
                    n(p[0] / k),
                    n(p[1] / k)
                );
            }
            let _ = writeln!(paths, "<path id=\"path-{}\" d=\"{d}\"/>", l.id);
        }
    }
    if !paths.is_empty() {
        let _ = write!(s, "<defs>\n{paths}</defs>\n");
    }
    let mut by_group: BTreeMap<&str, Vec<&crate::labels::Placed>> = BTreeMap::new();
    for l in placed {
        by_group
            .entry(super::super::style_group(l.group))
            .or_default()
            .push(l);
    }
    // A label stroke is a halo under the glyphs (`#labels` in `index.css`).
    s.push_str("<g id=\"labels\" paint-order=\"stroke\" stroke-linejoin=\"round\">\n");
    for group in GROUPS {
        let Some(list) = by_group.get(group) else {
            continue;
        };
        let path = ["labels", group];
        let gid = kebab(group);
        // The outline of the style, wider with the text: each label gets
        // its own stroke width. Without an outline, the shadow of the style
        // becomes a halo of its colour.
        let outline = outline(&ctx.style, group);
        let stroke_w = match &outline {
            Some((_, None)) => 1.0,
            _ => 0.0,
        };
        let stroke = match &outline {
            Some((_, Some(color))) => format!(
                " stroke=\"{}\" stroke-opacity=\"0.85\" stroke-linejoin=\"round\" paint-order=\"stroke\"",
                xml(color)
            ),
            _ => String::new(),
        };
        let _ = writeln!(
            s,
            "<g id=\"labels-{gid}\"{}{stroke}>",
            ctx.attrs(
                &path,
                if stroke_w > 0.0 {
                    &["font-size", "style", "letter-spacing", "stroke-width"][..]
                } else {
                    &[
                        "font-size",
                        "style",
                        "letter-spacing",
                        "stroke-width",
                        "stroke",
                        "stroke-opacity",
                    ][..]
                }
            )
        );
        for l in list {
            let spacing = if l.spacing > 0.0 {
                format!(" letter-spacing=\"{}\"", n(l.spacing / k))
            } else {
                String::new()
            };
            let width = match &outline {
                Some((w, _)) => format!(" stroke-width=\"{}\"", n(w * l.size / k)),
                None => String::new(),
            };
            let font = format!("font-size=\"{}\"{spacing}{width}", n(l.size / k));
            match &l.geometry {
                Geometry::Point {
                    x,
                    y,
                    anchor,
                    angle,
                } => {
                    let (x, y) = (x / k, y / k);
                    let turn = if *angle != 0 {
                        format!(" transform=\"rotate({angle} {} {})\"", n(x), n(y))
                    } else {
                        String::new()
                    };
                    let _ = writeln!(
                        s,
                        "<text id=\"{}\" x=\"{}\" y=\"{}\" {font} text-anchor=\"{anchor}\"{turn}>{}</text>",
                        l.id,
                        n(x),
                        n(y),
                        xml(&l.text)
                    );
                }
                Geometry::Path { offset, .. } => {
                    let _ = writeln!(
                        s,
                        "<text id=\"{}\" dy=\"{}\" {font} text-anchor=\"middle\"><textPath href=\"#path-{}\" startOffset=\"50%\" text-anchor=\"middle\">{}</textPath></text>",
                        l.id,
                        n(-offset / k),
                        l.id,
                        xml(&l.text)
                    );
                }
            }
        }
        s.push_str("</g>\n");
    }
    s.push_str("</g>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_shadow_gives_the_colour_and_the_blur() {
        assert_eq!(
            text_shadow("text-shadow: white 0px 0px 4px; transform: translate(0em, -0.5em)"),
            Some(("white".to_owned(), 4.0))
        );
        assert_eq!(
            text_shadow("text-shadow: #d3c9ae 0px 0px 1px; text-transform: uppercase"),
            Some(("#d3c9ae".to_owned(), 1.0))
        );
        assert_eq!(
            text_shadow("text-shadow: white 0 0 4px;"),
            Some(("white".to_owned(), 4.0))
        );
        assert_eq!(text_shadow("transform: translate(0em, -0.45em)"), None);
    }
}
