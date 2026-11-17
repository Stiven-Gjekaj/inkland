//! The options of the Options tab and of the Style tab. Each option is a
//! key, a JSON Pointer of the config, and a kind of value. The app sends
//! the text of a field, and `Editor::set_option` makes one `set` or `unset`
//! command from it.

use crate::catalog;
use crate::model::Editor;
use inkland::Config;
use serde_json::{Value, json};

/// The kind of value of an option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A whole number that is 0 or more.
    Count,
    /// A number.
    Number,
    /// A text.
    Text,
    /// `true` or `false`.
    Bool,
    /// `auto` or a whole number.
    Towns,
    /// A number, or an empty text that removes the value. The value is an
    /// object with this one field.
    OptionalIn(&'static str),
    /// A text, or an empty text that removes the value. The value is an
    /// object with this one field.
    OptionalTextIn(&'static str),
}

/// The options: the key that the app uses, the JSON Pointer, and the kind.
pub const OPTIONS: &[(&str, &str, Kind)] = &[
    ("seed", "/seed", Kind::Count),
    ("width", "/frame/width", Kind::Number),
    ("height", "/frame/height", Kind::Number),
    ("cells", "/world/cells", Kind::Count),
    ("template", "/world/template", Kind::Text),
    ("cultures", "/world/cultures", Kind::Count),
    ("culture-set", "/world/culture_set", Kind::Text),
    ("states", "/world/states", Kind::Count),
    ("towns", "/world/towns", Kind::Towns),
    ("religions", "/world/religions", Kind::Count),
    ("sea-level", "/world/sea_level", Kind::Count),
    ("lat-south", "/world/climate/latitude_range/0", Kind::Number),
    ("lat-north", "/world/climate/latitude_range/1", Kind::Number),
    ("units", "/units/name", Kind::Text),
    ("mode", "/render/mode", Kind::Text),
    ("style", catalog::STYLE_PRESET_PATH, Kind::Text),
    ("font-title", "/render/fonts/title", Kind::Text),
    ("font-labels", "/render/fonts/labels", Kind::Text),
    ("font-labels-sc", "/render/fonts/labels_sc", Kind::Text),
    ("font-other", "/render/fonts/other", Kind::Text),
    ("size-title", "/render/font_sizes_pt/title", Kind::Number),
    ("size-state", "/render/font_sizes_pt/state", Kind::Number),
    ("size-region", "/render/font_sizes_pt/region", Kind::Number),
    (
        "size-city",
        "/render/font_sizes_pt/place_city",
        Kind::Number,
    ),
    (
        "size-town",
        "/render/font_sizes_pt/place_town",
        Kind::Number,
    ),
    (
        "size-village",
        "/render/font_sizes_pt/place_village",
        Kind::Number,
    ),
    ("size-river", "/render/font_sizes_pt/river", Kind::Number),
    ("size-other", "/render/font_sizes_pt/other", Kind::Number),
    ("min-line", "/render/min_line_width_pt", Kind::Number),
    ("scale-bar", "/render/extras/scale_bar", Kind::Bool),
    ("compass", "/render/extras/compass", Kind::Bool),
    ("grid-step", "/render/extras/grid", Kind::OptionalIn("step")),
    (
        "title",
        "/render/extras/cartouche",
        Kind::OptionalTextIn("title"),
    ),
];

fn option(key: &str) -> Result<(&'static str, Kind), String> {
    OPTIONS
        .iter()
        .find(|(k, _, _)| *k == key)
        .map(|(_, p, kind)| (*p, *kind))
        .ok_or_else(|| format!("there is no option {key:?}"))
}

/// A number as JSON: a whole number without a decimal point.
pub fn number(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        json!(v as i64)
    } else {
        json!(v)
    }
}

/// A number as short text: `400`, `12.5`.
pub fn number_text(v: f64) -> String {
    let t = format!("{:.3}", v);
    t.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// An object with one field.
fn obj(field: &str, v: Value) -> Value {
    let mut m = serde_json::Map::new();
    m.insert(field.to_owned(), v);
    Value::Object(m)
}

fn parse_number(key: &str, text: &str) -> Result<f64, String> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("{key}: {text:?} is not a number"))
}

fn parse_count(key: &str, text: &str) -> Result<u64, String> {
    text.trim()
        .parse::<u64>()
        .map_err(|_| format!("{key}: {text:?} is not a whole number"))
}

/// The text of an option for its field.
pub fn option_text(c: &Config, key: &str) -> String {
    let Ok((path, kind)) = option(key) else {
        return String::new();
    };
    let v = catalog::value_at(c, path);
    match (kind, v) {
        (_, None) => String::new(),
        (Kind::OptionalIn(field) | Kind::OptionalTextIn(field), Some(v)) => text_of(&v[field]),
        (_, Some(v)) => text_of(&v),
    }
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(t) => t.clone(),
        Value::Number(n) => n.as_f64().map(number_text).unwrap_or_default(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

impl Editor {
    /// Set an option from the text of its field. Each change is one edit,
    /// so undo and the edit script have it. A text that is the same as the
    /// current value makes no edit.
    pub fn set_option(&mut self, key: &str, text: &str) -> Result<(), String> {
        let (path, kind) = option(key)?;
        let text = text.trim();
        let value = match kind {
            Kind::Count => json!(parse_count(key, text)?),
            Kind::Number => number(parse_number(key, text)?),
            Kind::Text => {
                if text.is_empty() {
                    return Err(format!("{key}: the value cannot be empty"));
                }
                json!(text)
            }
            Kind::Bool => match text {
                "true" => json!(true),
                "false" => json!(false),
                _ => return Err(format!("{key}: use true or false")),
            },
            Kind::Towns => {
                if text == "auto" || text.is_empty() {
                    json!("auto")
                } else {
                    json!(parse_count(key, text)?)
                }
            }
            Kind::OptionalIn(field) => {
                if text.is_empty() {
                    return self.unset(path);
                }
                obj(field, number(parse_number(key, text)?))
            }
            Kind::OptionalTextIn(field) => {
                if text.is_empty() {
                    return self.unset(path);
                }
                obj(field, json!(text))
            }
        };
        self.set(path, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> Editor {
        Editor::new(
            serde_json::from_value(json!({
                "version": 1, "seed": 5,
                "frame": { "width": 200, "height": 150 },
                "world": { "cells": 1500, "template": "pangea" },
                "pins": { "points": [{ "id": "ostrey", "kind": "city", "at": [150, 100] }] }
            }))
            .unwrap(),
        )
    }

    #[test]
    fn each_option_key_has_a_valid_pointer_and_gives_text() {
        let e = editor();
        for (key, path, kind) in OPTIONS {
            assert!(path.starts_with('/'), "{key}");
            let text = option_text(e.config(), key);
            // The style preset and the optional values can be missing.
            if *path != catalog::STYLE_PRESET_PATH
                && !matches!(
                    kind,
                    Kind::OptionalIn(_) | Kind::OptionalTextIn(_) | Kind::Bool
                )
            {
                assert!(!text.is_empty(), "{key} has no text");
            }
        }
        assert_eq!(option_text(e.config(), "width"), "200");
        assert_eq!(option_text(e.config(), "towns"), "auto");
        assert_eq!(option_text(e.config(), "lat-north"), "55");
    }

    #[test]
    fn options_are_edits_that_undo() {
        let mut e = editor();
        let steps: &[(&str, &str)] = &[
            ("seed", "42"),
            ("cells", "2000"),
            ("template", "archipelago"),
            ("cultures", "4"),
            ("culture-set", "oriental"),
            ("states", "3"),
            ("towns", "12"),
            ("religions", "2"),
            ("sea-level", "25"),
            ("lat-south", "-10"),
            ("units", "league"),
            ("width", "220"),
            ("size-city", "9.5"),
            ("min-line", "0.6"),
            ("font-title", "Inter"),
            ("scale-bar", "true"),
            ("grid-step", "25"),
            ("title", "The Realm"),
        ];
        for (key, text) in steps {
            let before = e.config().clone();
            e.set_option(key, text)
                .unwrap_or_else(|err| panic!("{key}: {err}"));
            assert_ne!(e.config(), &before, "{key} changes the config");
            assert_eq!(&option_text(e.config(), key), text, "{key}");
        }
        assert_eq!(e.config().seed, 42);
        assert_eq!(e.config().world.sea_level, 25);
        assert_eq!(e.config().render.extras.grid.as_ref().unwrap().step, 25.0);
        // The same text again makes no edit.
        let n = e.script_json();
        e.set_option("seed", "42").unwrap();
        assert_eq!(e.script_json(), n);
        // An empty text removes an optional value.
        e.set_option("title", "").unwrap();
        assert!(e.config().render.extras.cartouche.is_none());
        e.set_option("towns", "auto").unwrap();
        while e.undo().unwrap() {}
        assert_eq!(e.config(), e.original());
    }

    #[test]
    fn a_bad_option_is_refused_and_changes_nothing() {
        let mut e = editor();
        let before = e.config().clone();
        assert!(e.set_option("seed", "-1").is_err());
        assert!(e.set_option("cells", "many").is_err());
        assert!(e.set_option("template", "moon").is_err());
        assert!(e.set_option("sea-level", "150").is_err());
        assert!(e.set_option("culture-set", "").is_err());
        assert!(e.set_option("nothing", "1").is_err());
        // The pin at x 150 is outside a frame of width 100.
        assert!(e.set_option("width", "100").is_err());
        assert!(e.set_option("mode", "sepia").is_err());
        assert_eq!(e.config(), &before);
        assert!(!e.can_undo());
    }
}
