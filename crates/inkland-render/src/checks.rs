//! Print checks on a written SVG.
//!
//! The checks parse the SVG text again, so they test what will be printed,
//! not the state of the program. `inkland check` runs them on any SVG.

use crate::page::MM_PER_PT;
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

/// The value of an inherited property: on the node or on an ancestor.
fn inherited<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<String> {
    node.ancestors()
        .filter(|n| n.is_element())
        .find_map(|n| prop(n, name))
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

    // min_line_width: each stroked shape, with the inherited stroke width.
    // The SVG default width is 1 user unit; one user unit is 1 mm.
    let mut thin = Vec::new();
    let mut min_found = f64::INFINITY;
    for &n in &elements {
        if !is_shape(n.tag_name().name()) {
            continue;
        }
        let stroke = inherited(n, "stroke").unwrap_or_else(|| "none".to_owned());
        if stroke.trim() == "none" {
            continue;
        }
        let width = inherited(n, "stroke-width")
            .and_then(|w| w.trim().trim_end_matches("px").parse::<f64>().ok())
            .unwrap_or(1.0);
        let width_pt = width / MM_PER_PT;
        min_found = min_found.min(width_pt);
        if width_pt + 1e-6 < opts.min_line_width_pt {
            thin.push(id_of(n));
        }
    }

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
        let svg = crate::svg::render(&w, &c, &c.variants[0]);
        let results = check_svg(&svg, &opts());
        let bad: Vec<_> = results.iter().filter(|r| !r.pass).collect();
        assert!(bad.is_empty(), "{bad:?}");
        let min = results.iter().find(|r| r.name == "min_line_width").unwrap();
        assert!(min.details["min_found_pt"].as_f64().unwrap() >= 0.5);
    }

    #[test]
    fn a_broken_svg_fails_each_check() {
        assert_eq!(failed("<svg").len(), NAMES.len());
    }
}
