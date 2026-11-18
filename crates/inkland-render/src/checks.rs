//! Print checks on a written SVG.
//!
//! The checks parse the SVG text again, so they test what will be printed,
//! not the state of the program. `inkland check` runs them on any SVG.

use crate::page::MM_PER_PT;
use crate::transform::Affine;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Options of the checks.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckOptions {
    /// The smallest permitted line width, in points.
    pub min_line_width_pt: f64,
    /// The permitted font families. Empty: any family.
    pub fonts_allowed: Vec<String>,
}

/// The result of one check.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    pub name: &'static str,
    pub pass: bool,
    /// More data, for example the ids of the elements that fail.
    pub details: Value,
}

impl CheckResult {
    pub fn to_json(&self) -> Value {
        let mut v = json!({ "name": self.name, "pass": self.pass });
        if let (Value::Object(map), Value::Object(d)) = (&mut v, &self.details) {
            for (k, x) in d {
                map.insert(k.clone(), x.clone());
            }
        }
        v
    }
}

/// The names of the checks, in the order that they run.
pub const NAMES: [&str; 7] = [
    "no_opacity",
    "no_masks_filters_rgba",
    "black_and_white",
    "min_line_width",
    "all_text_is_text_elements",
    "font_families_allowed",
    "deterministic_ids",
];

/// The style properties of an element: its attributes and the parts of its
/// `style` attribute.
fn props<'a>(node: roxmltree::Node<'a, 'a>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = node
        .attributes()
        .map(|a| (a.name().to_owned(), a.value().to_owned()))
        .collect();
    if let Some(style) = node.attribute("style") {
        for part in style.split(';') {
            if let Some((k, v)) = part.split_once(':') {
                out.push((k.trim().to_owned(), v.trim().to_owned()));
            }
        }
    }
    out
}

fn prop<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<String> {
    props(node)
        .into_iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v)
}

fn id_of(node: roxmltree::Node) -> String {
    node.attribute("id")
        .map_or_else(|| format!("<{}>", node.tag_name().name()), str::to_owned)
}

fn is_shape(name: &str) -> bool {
    matches!(
        name,
        "path" | "line" | "polyline" | "polygon" | "rect" | "circle" | "ellipse" | "text"
    )
}

/// The scale from the user units of the root `<svg>` to millimetres. A
/// root with an absolute width and a view box maps the view box to that
/// width. Any other root has one user unit to the millimetre.
fn root_transform(root: roxmltree::Node) -> Affine {
    let mm = |v: &str| -> Option<f64> {
        let v = v.trim();
        let units = [
            ("mm", 1.0),
            ("cm", 10.0),
            ("in", 25.4),
            ("pt", MM_PER_PT),
            ("pc", 12.0 * MM_PER_PT),
        ];
        units.iter().find_map(|(u, f)| {
            v.strip_suffix(u)
                .and_then(|n| n.trim().parse::<f64>().ok())
                .map(|n| n * f)
        })
    };
    let (Some(w), Some(h)) = (
        root.attribute("width").and_then(mm),
        root.attribute("height").and_then(mm),
    ) else {
        return Affine::IDENTITY;
    };
    view_box_transform(root, w, h).unwrap_or(Affine::IDENTITY)
}

/// The transform that fits the `viewBox` of an element into a box `w` by
/// `h`, as `xMidYMid meet` does. `None` if the element has no view box.
fn view_box_transform(node: roxmltree::Node, w: f64, h: f64) -> Option<Affine> {
    let v: Vec<f64> = node
        .attribute("viewBox")?
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    let [vx, vy, vw, vh] = v[..] else {
        return None;
    };
    if vw <= 0.0 || vh <= 0.0 {
        return None;
    }
    let s = (w / vw).min(h / vh);
    Some(
        Affine::translate((w - vw * s) / 2.0 - vx * s, (h - vh * s) / 2.0 - vy * s)
            .then(&Affine::scale(s, s)),
    )
}

fn number(node: roxmltree::Node, name: &str) -> Option<f64> {
    node.attribute(name)
        .and_then(|v| v.trim().trim_end_matches("px").parse().ok())
}

/// The id that a `url(#id)` or `#id` names.
fn reference(v: &str) -> Option<&str> {
    let v = v.trim();
    v.strip_prefix("url(#")
        .and_then(|r| r.strip_suffix(')'))
        .or_else(|| v.strip_prefix('#'))
}

/// The elements that the walk does not draw where they stand.
fn is_not_drawn(name: &str) -> bool {
    matches!(
        name,
        "defs"
            | "clipPath"
            | "mask"
            | "pattern"
            | "symbol"
            | "marker"
            | "linearGradient"
            | "radialGradient"
            | "filter"
            | "metadata"
            | "style"
            | "title"
            | "desc"
    )
}

/// The state of the `min_line_width` check.
struct Lines<'d, 'i> {
    doc: &'d roxmltree::Document<'i>,
    /// Each element with an id.
    ids: std::collections::BTreeMap<&'d str, roxmltree::Node<'d, 'i>>,
    min_pt: f64,
    thin: Vec<String>,
    min_found: f64,
    /// The patterns already walked, with the thin factor of their use.
    patterns_seen: BTreeSet<(String, u64)>,
}

impl<'d, 'i> Lines<'d, 'i> {
    /// Walk an element that is drawn. `m` is the transform of its parent
    /// to millimetres, and `inherited` the properties of its ancestors in
    /// the drawn tree, the nearest last.
    fn walk(
        &mut self,
        node: roxmltree::Node<'d, 'i>,
        m: &Affine,
        inherited: &[(String, String)],
        depth: usize,
    ) {
        if depth > 64 {
            return;
        }
        let name = node.tag_name().name();
        let mut local = node
            .attribute("transform")
            .map_or(Affine::IDENTITY, Affine::parse);
        if name == "use" {
            let at = Affine::translate(
                number(node, "x").unwrap_or(0.0),
                number(node, "y").unwrap_or(0.0),
            );
            local = local.then(&at);
        }
        if name == "svg" && node != self.doc.root_element() {
            let at = Affine::translate(
                number(node, "x").unwrap_or(0.0),
                number(node, "y").unwrap_or(0.0),
            );
            local = at;
            if let (Some(w), Some(h)) = (number(node, "width"), number(node, "height"))
                && let Some(v) = view_box_transform(node, w, h)
            {
                local = local.then(&v);
            }
        }
        let m = m.then(&local);
        let mut props = inherited.to_vec();
        props.extend(self::props(node));
        let get = |key: &str| {
            props
                .iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
        };

        if is_shape(name) {
            let stroke = get("stroke").unwrap_or_else(|| "none".to_owned());
            if stroke.trim() != "none" {
                let width = get("stroke-width")
                    .and_then(|w| w.trim().trim_end_matches("px").parse::<f64>().ok())
                    .unwrap_or(1.0);
                let width_pt = width * m.thin_factor() / MM_PER_PT;
                self.min_found = self.min_found.min(width_pt);
                if width_pt + 1e-6 < self.min_pt {
                    let id = match (
                        node.attribute("id"),
                        node.ancestors().find_map(|a| a.attribute("id")),
                    ) {
                        (None, Some(near)) => format!("<{name}> in {near}"),
                        _ => id_of(node),
                    };
                    if !self.thin.contains(&id) {
                        self.thin.push(id);
                    }
                }
            }
            for key in ["fill", "stroke"] {
                if let Some(id) = get(key).as_deref().and_then(reference) {
                    self.pattern(id, &m, depth);
                }
            }
        }
        if name == "use" {
            let target = node
                .attribute("href")
                .or_else(|| node.attribute(("http://www.w3.org/1999/xlink", "href")))
                .and_then(reference)
                .and_then(|id| self.find(id));
            if let Some(t) = target {
                if t.tag_name().name() == "symbol" {
                    for c in t.children().filter(|c| c.is_element()) {
                        self.walk(c, &m, &props, depth + 1);
                    }
                } else {
                    self.walk(t, &m, &props, depth + 1);
                }
            }
            return;
        }
        for c in node.children().filter(|c| c.is_element()) {
            if !is_not_drawn(c.tag_name().name()) {
                self.walk(c, &m, &props, depth + 1);
            }
        }
    }

    fn find(&self, id: &str) -> Option<roxmltree::Node<'d, 'i>> {
        self.ids.get(id).copied()
    }

    /// Walk the tiles of a pattern that paints a shape with transform `m`.
    /// A tile inherits from the ancestors of the pattern in the file.
    fn pattern(&mut self, id: &str, m: &Affine, depth: usize) {
        let Some(p) = self.find(id).filter(|p| p.tag_name().name() == "pattern") else {
            return;
        };
        let key = (id.to_owned(), (m.thin_factor() * 1e6).round() as u64);
        if !self.patterns_seen.insert(key) {
            return;
        }
        let mut m = m.then(
            &p.attribute("patternTransform")
                .map_or(Affine::IDENTITY, Affine::parse),
        );
        if let (Some(w), Some(h)) = (number(p, "width"), number(p, "height"))
            && let Some(v) = view_box_transform(p, w, h)
        {
            m = m.then(&v);
        }
        let mut chain: Vec<_> = p.ancestors().filter(|n| n.is_element()).collect();
        chain.reverse();
        let props: Vec<(String, String)> = chain.into_iter().flat_map(self::props).collect();
        for c in p.children().filter(|c| c.is_element()) {
            if !is_not_drawn(c.tag_name().name()) {
                self.walk(c, &m, &props, depth + 1);
            }
        }
    }
}

/// Run all checks on SVG text.
pub fn check_svg(svg: &str, opts: &CheckOptions) -> Vec<CheckResult> {
    let doc = match roxmltree::Document::parse(svg) {
        Ok(d) => d,
        Err(e) => {
            return NAMES
                .iter()
                .map(|&name| CheckResult {
                    name,
                    pass: false,
                    details: json!({ "error": format!("the SVG cannot be read: {e}") }),
                })
                .collect();
        }
    };
    let elements: Vec<roxmltree::Node> = doc.descendants().filter(|n| n.is_element()).collect();

    // no_opacity
    let mut opacity = Vec::new();
    for &n in &elements {
        for (k, v) in props(n) {
            if matches!(k.as_str(), "opacity" | "fill-opacity" | "stroke-opacity")
                && v.trim() != "1"
            {
                opacity.push(id_of(n));
            }
        }
    }

    // no_masks_filters_rgba
    let mut effects = Vec::new();
    for &n in &elements {
        let tag = n.tag_name().name();
        if matches!(
            tag,
            "mask" | "filter" | "linearGradient" | "radialGradient" | "feGaussianBlur"
        ) {
            effects.push(id_of(n));
            continue;
        }
        for (k, v) in props(n) {
            let lv = v.to_ascii_lowercase();
            if matches!(k.as_str(), "mask" | "filter" | "mix-blend-mode")
                || lv.contains("rgba(")
                || lv.contains("hsla(")
            {
                effects.push(id_of(n));
            }
        }
    }

    // black_and_white
    let mut colours = Vec::new();
    for &n in &elements {
        for (k, v) in props(n) {
            if !matches!(k.as_str(), "fill" | "stroke" | "color" | "stop-color") {
                continue;
            }
            let lv = v.trim().to_ascii_lowercase();
            let ok = matches!(
                lv.as_str(),
                "none"
                    | "#000"
                    | "#000000"
                    | "black"
                    | "#fff"
                    | "#ffffff"
                    | "white"
                    | "currentcolor"
                    | "inherit"
            ) || lv.starts_with("url(");
            if !ok {
                colours.push(format!("{} {k}={v}", id_of(n)));
            }
        }
    }

    // min_line_width: each stroked shape that is drawn, with the inherited
    // stroke width and all transforms applied. The walk follows `<use>`
    // to its target and a pattern fill to its tiles.
    let mut lines = Lines {
        doc: &doc,
        ids: elements
            .iter()
            .filter_map(|n| n.attribute("id").map(|id| (id, *n)))
            .collect(),
        min_pt: opts.min_line_width_pt,
        thin: Vec::new(),
        min_found: f64::INFINITY,
        patterns_seen: BTreeSet::new(),
    };
    let root = doc.root_element();
    lines.walk(root, &root_transform(root), &Vec::new(), 0);
    let Lines {
        thin, min_found, ..
    } = lines;

    // all_text_is_text_elements: labels are <text>, and no font is embedded.
    let mut outlined = Vec::new();
    for &n in &elements {
        let tag = n.tag_name().name();
        let id = n.attribute("id").unwrap_or("");
        if id.starts_with("label-") && !matches!(tag, "text" | "g") {
            outlined.push(id.to_owned());
        }
        if matches!(tag, "font" | "font-face" | "glyph") {
            outlined.push(id_of(n));
        }
        if tag == "style" && n.text().is_some_and(|t| t.contains("@font-face")) {
            outlined.push("<style> @font-face".to_owned());
        }
    }

    // font_families_allowed
    let allowed: BTreeSet<&str> = opts.fonts_allowed.iter().map(String::as_str).collect();
    let mut fonts = Vec::new();
    for &n in &elements {
        if let Some(f) = prop(n, "font-family") {
            for family in f.split(',') {
                let family = family.trim().trim_matches(|c| c == '"' || c == '\'');
                if !allowed.is_empty() && !allowed.contains(family) {
                    fonts.push(format!("{} {family}", id_of(n)));
                }
            }
        }
    }

    // deterministic_ids
    let mut seen = BTreeSet::new();
    let mut bad_ids = Vec::new();
    for &n in &elements {
        if let Some(id) = n.attribute("id") {
            let ok = !id.is_empty()
                && id.split('-').all(|w| {
                    !w.is_empty()
                        && w.bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                });
            if !ok || !seen.insert(id.to_owned()) {
                bad_ids.push(id.to_owned());
            }
        }
    }

    let list = |v: Vec<String>| json!({ "elements": v.into_iter().take(20).collect::<Vec<_>>() });
    vec![
        CheckResult {
            name: "no_opacity",
            pass: opacity.is_empty(),
            details: list(opacity),
        },
        CheckResult {
            name: "no_masks_filters_rgba",
            pass: effects.is_empty(),
            details: list(effects),
        },
        CheckResult {
            name: "black_and_white",
            pass: colours.is_empty(),
            details: list(colours),
        },
        CheckResult {
            name: "min_line_width",
            pass: thin.is_empty(),
            details: json!({
                "min_found_pt": if min_found.is_finite() { inkland_core::num::round(min_found, 3).into() } else { Value::Null },
                "elements": thin.into_iter().take(20).collect::<Vec<_>>(),
            }),
        },
        CheckResult {
            name: "all_text_is_text_elements",
            pass: outlined.is_empty(),
            details: list(outlined),
        },
        CheckResult {
            name: "font_families_allowed",
            pass: fonts.is_empty(),
            details: list(fonts),
        },
        CheckResult {
            name: "deterministic_ids",
            pass: bad_ids.is_empty(),
            details: list(bad_ids),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> CheckOptions {
        CheckOptions {
            min_line_width_pt: 0.5,
            fonts_allowed: vec![
                "Caladea".into(),
                "Inter".into(),
                "IM Fell English".into(),
                "IM Fell English SC".into(),
            ],
        }
    }

    fn failed(svg: &str) -> Vec<&'static str> {
        check_svg(svg, &opts())
            .into_iter()
            .filter(|c| !c.pass)
            .map(|c| c.name)
            .collect()
    }

    macro_rules! bad {
        ($file:literal) => {
            include_str!(concat!("../../../fixtures/svg-bad/", $file))
        };
    }

    #[test]
    fn each_bad_svg_fails_its_check_and_only_that_check() {
        assert_eq!(failed(bad!("opacity.svg")), ["no_opacity"]);
        assert_eq!(failed(bad!("filter.svg")), ["no_masks_filters_rgba"]);
        assert_eq!(failed(bad!("grey.svg")), ["black_and_white"]);
        assert_eq!(failed(bad!("thin-line.svg")), ["min_line_width"]);
        assert_eq!(
            failed(bad!("outlined-text.svg")),
            ["all_text_is_text_elements"]
        );
        assert_eq!(failed(bad!("bad-font.svg")), ["font_families_allowed"]);
        assert_eq!(failed(bad!("duplicate-id.svg")), ["deterministic_ids"]);
        assert_eq!(
            failed(bad!("rgba.svg")),
            ["no_masks_filters_rgba", "black_and_white"]
        );
    }

    #[test]
    fn a_generated_map_passes_each_check() {
        use inkland_core::config::Config;
        use inkland_core::world::World;
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 4, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea" },
            "render": { "extras": { "scale_bar": true, "compass": true, "cartouche": { "title": "A & B" }, "grid": { "step": 50 } } }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let svg = crate::svg::render(&w, &c, &c.variants[0]).svg;
        let results = check_svg(&svg, &opts());
        let bad: Vec<_> = results.iter().filter(|r| !r.pass).collect();
        assert!(bad.is_empty(), "{bad:?}");
        let min = results.iter().find(|r| r.name == "min_line_width").unwrap();
        assert!(min.details["min_found_pt"].as_f64().unwrap() >= 0.5);
    }

    #[test]
    fn a_transform_makes_a_line_thinner() {
        let svg = |inner: &str| {
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10mm\" height=\"10mm\" viewBox=\"0 0 10 10\">{inner}</svg>"
            )
        };
        // 0.5 mm wide: thick enough, with no transform.
        let line = "<path id=\"a\" d=\"M0 0L5 5\" stroke=\"#000\" stroke-width=\"0.5\"/>";
        assert!(failed(&svg(line)).is_empty());
        // The same line in a group at a tenth of the size.
        let scaled = format!("<g transform=\"matrix(0.1,0,0,0.1,2,2)\">{line}</g>");
        assert_eq!(failed(&svg(&scaled)), ["min_line_width"]);
        // A scale in one direction only is enough.
        let squashed = format!("<g transform=\"scale(4 0.1)\">{line}</g>");
        assert_eq!(failed(&svg(&squashed)), ["min_line_width"]);
        // A use of a small definition.
        let used = format!(
            "<defs><g id=\"d\" transform=\"scale(0.1)\">{line}</g></defs><use href=\"#d\"/>"
        );
        assert_eq!(failed(&svg(&used)), ["min_line_width"]);
        // A definition that nothing uses is not drawn.
        let unused = format!("<defs><g id=\"d\" transform=\"scale(0.1)\">{line}</g></defs>");
        assert!(failed(&svg(&unused)).is_empty());
        // A nested view box at a tenth of the size.
        let nested = format!(
            "<svg x=\"0\" y=\"0\" width=\"1\" height=\"1\" viewBox=\"0 0 10 10\">{line}</svg>"
        );
        assert_eq!(failed(&svg(&nested)), ["min_line_width"]);
        // A root view box at a tenth of the width in millimetres.
        let small_root = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1mm\" height=\"1mm\" viewBox=\"0 0 10 10\"><path id=\"a\" d=\"M0 0L5 5\" stroke=\"#000\" stroke-width=\"0.5\"/></svg>";
        assert_eq!(failed(small_root), ["min_line_width"]);
        // The tile of a pattern that fills a small shape.
        let tile = format!(
            "<defs><pattern id=\"p\" width=\"4\" height=\"4\" patternUnits=\"userSpaceOnUse\">{line}</pattern></defs><g transform=\"scale(0.1)\"><rect id=\"r\" width=\"5\" height=\"5\" fill=\"url(#p)\" stroke=\"none\"/></g>"
        );
        assert_eq!(failed(&svg(&tile)), ["min_line_width"]);
    }

    #[test]
    fn a_broken_svg_fails_each_check() {
        assert_eq!(failed("<svg").len(), NAMES.len());
    }
}
