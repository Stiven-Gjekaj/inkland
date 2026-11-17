//! The style presets of `colour` mode.
//!
//! A preset is a style file of Azgaar's Fantasy Map Generator (MIT license,
//! see THIRD_PARTY_NOTICES.md), kept as data in `assets/fmg/styles`. A style
//! is a tree: each element (a layer) has `attrs` (SVG attributes),
//! `options` (values that the renderer reads), and `groups` (child elements
//! with the same parts).
//!
//! A preset that has no value for a path takes the value of the default
//! preset, as the original program does. `render.style_overrides` then sets
//! single values by their path.

use inkland_core::config::StyleName;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The text of the style file of a preset.
fn source(name: StyleName) -> &'static str {
    match name {
        StyleName::Default => include_str!("../../assets/fmg/styles/default.json"),
        StyleName::Ancient => include_str!("../../assets/fmg/styles/ancient.json"),
        StyleName::Atlas => include_str!("../../assets/fmg/styles/atlas.json"),
        StyleName::Cinderwood => include_str!("../../assets/fmg/styles/cinderwood.json"),
        StyleName::Clean => include_str!("../../assets/fmg/styles/clean.json"),
        StyleName::Cyberpunk => include_str!("../../assets/fmg/styles/cyberpunk.json"),
        StyleName::DarkSeas => include_str!("../../assets/fmg/styles/dark-seas.json"),
        StyleName::Frostbite => include_str!("../../assets/fmg/styles/frostbite.json"),
        StyleName::Gloom => include_str!("../../assets/fmg/styles/gloom.json"),
        StyleName::Ink => include_str!("../../assets/fmg/styles/ink.json"),
        StyleName::Light => include_str!("../../assets/fmg/styles/light.json"),
        StyleName::Monochrome => include_str!("../../assets/fmg/styles/monochrome.json"),
        StyleName::Night => include_str!("../../assets/fmg/styles/night.json"),
        StyleName::Pale => include_str!("../../assets/fmg/styles/pale.json"),
        StyleName::Watercolor => include_str!("../../assets/fmg/styles/watercolor.json"),
    }
}

/// A style tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    root: Value,
}

/// Put the values of `top` over the values of `base`. Objects merge key by
/// key; other values replace.
fn merge(base: &mut Value, top: &Value) {
    match (base, top) {
        (Value::Object(b), Value::Object(t)) => {
            for (k, v) in t {
                match b.get_mut(k) {
                    Some(slot) => merge(slot, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (slot, v) => *slot = v.clone(),
    }
}

impl Style {
    /// The style of a preset, with the default values where the preset has
    /// none.
    pub fn preset(name: StyleName) -> Style {
        let mut root: Value =
            serde_json::from_str(source(StyleName::Default)).expect("the default style is JSON");
        if name != StyleName::Default {
            let top: Value = serde_json::from_str(source(name)).expect("each style is JSON");
            merge(&mut root, &top);
        }
        Style { root }
    }

    /// The style of a preset with the overrides of a config. A path that
    /// does not exist yet is made.
    pub fn with_overrides(name: StyleName, overrides: &BTreeMap<String, Value>) -> Style {
        let mut style = Style::preset(name);
        for (path, value) in overrides {
            style.set(path, value.clone());
        }
        style
    }

    /// Set one value by its full path, for example
    /// `ocean.groups.base.attrs.fill`.
    pub fn set(&mut self, path: &str, value: Value) {
        let words: Vec<&str> = path.split('.').collect();
        let Some((last, parents)) = words.split_last() else {
            return;
        };
        let mut node = &mut self.root;
        for w in parents {
            if !node.is_object() {
                *node = Value::Object(Map::new());
            }
            node = node
                .as_object_mut()
                .expect("an object")
                .entry((*w).to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
        }
        if let Value::Object(map) = node {
            map.insert((*last).to_owned(), value);
        }
    }

    /// The node of an element or a group. `path` is the element and then the
    /// group names, for example `["ocean", "base"]`.
    pub fn node(&self, path: &[&str]) -> Option<&Value> {
        let (first, rest) = path.split_first()?;
        let mut node = self.root.get(*first)?;
        for g in rest {
            node = node.get("groups")?.get(*g)?;
        }
        Some(node)
    }

    /// The names of the groups of a node, in the order of their names.
    pub fn groups(&self, path: &[&str]) -> Vec<String> {
        self.node(path)
            .and_then(|n| n.get("groups"))
            .and_then(Value::as_object)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// The attributes of a node that have a value, as text, in the order of
    /// their names.
    pub fn attrs(&self, path: &[&str]) -> Vec<(String, String)> {
        let Some(Value::Object(map)) = self.node(path).and_then(|n| n.get("attrs")) else {
            return Vec::new();
        };
        map.iter()
            .filter_map(|(k, v)| text(v).map(|t| (k.clone(), t)))
            .collect()
    }

    /// One attribute of a node, as text.
    pub fn attr(&self, path: &[&str], name: &str) -> Option<String> {
        self.node(path)
            .and_then(|n| n.get("attrs"))
            .and_then(|a| a.get(name))
            .and_then(text)
    }

    /// One attribute of a node, as a number. A value such as `"10px"` gives
    /// 10.
    pub fn attr_f64(&self, path: &[&str], name: &str) -> Option<f64> {
        self.attr(path, name).and_then(|t| number(&t))
    }

    /// One option of a node. `name` can have more words, for example
    /// `bands.render`.
    pub fn option(&self, path: &[&str], name: &str) -> Option<&Value> {
        let mut v = self.node(path)?.get("options")?;
        for w in name.split('.') {
            v = v.get(w)?;
        }
        (!v.is_null()).then_some(v)
    }

    /// One option as a number.
    pub fn option_f64(&self, path: &[&str], name: &str) -> Option<f64> {
        match self.option(path, name)? {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => number(s),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// One option as text.
    pub fn option_str(&self, path: &[&str], name: &str) -> Option<String> {
        self.option(path, name).and_then(text)
    }

    /// One option as a boolean. A missing option is `false`.
    pub fn option_bool(&self, path: &[&str], name: &str) -> bool {
        match self.option(path, name) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_f64().is_some_and(|v| v != 0.0),
            _ => false,
        }
    }
}

/// A style value as attribute text. `null`, objects, and lists have none.
fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(match n.as_f64() {
            Some(f) => inkland_core::num::fixed(f, 4),
            None => n.to_string(),
        }),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The number at the start of a text, for example 10 in `"10px"`.
pub fn number(t: &str) -> Option<f64> {
    let t = t.trim();
    let end = t
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || (i == 0 && (c == '-' || c == '+'))))
        .map_or(t.len(), |(i, _)| i);
    t[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_preset_is_a_full_tree() {
        let base = Style::preset(StyleName::Default);
        for name in StyleName::ALL {
            let s = Style::preset(name);
            for element in [
                "ocean",
                "landmass",
                "labels",
                "rivers",
                "relief",
                "burgIcons",
            ] {
                assert!(s.node(&[element]).is_some(), "{name:?} {element}");
            }
            assert!(s.attr(&["ocean", "base"], "fill").is_some(), "{name:?}");
            assert!(s.option_str(&["relief"], "set").is_some(), "{name:?}");
            assert_eq!(
                s.groups(&["labels"]).is_empty(),
                base.groups(&["labels"]).is_empty()
            );
        }
    }

    #[test]
    fn presets_differ_from_the_default() {
        let base = Style::preset(StyleName::Default);
        for name in &StyleName::ALL[1..] {
            assert_ne!(&Style::preset(*name), &base, "{name:?}");
        }
    }

    #[test]
    fn an_override_sets_one_value() {
        let mut o = BTreeMap::new();
        o.insert(
            "ocean.groups.base.attrs.fill".to_owned(),
            Value::from("#123456"),
        );
        o.insert("relief.options.density".to_owned(), Value::from(0.9));
        o.insert("landmass.attrs.opacity".to_owned(), Value::Null);
        let s = Style::with_overrides(StyleName::Ancient, &o);
        assert_eq!(
            s.attr(&["ocean", "base"], "fill").as_deref(),
            Some("#123456")
        );
        assert_eq!(s.option_f64(&["relief"], "density"), Some(0.9));
        assert_eq!(s.attr(&["landmass"], "opacity"), None);
        // Other values keep the preset.
        let a = Style::preset(StyleName::Ancient);
        assert_eq!(s.attr(&["landmass"], "fill"), a.attr(&["landmass"], "fill"));
    }

    #[test]
    fn numbers_read_from_text() {
        assert_eq!(number("10px"), Some(10.0));
        assert_eq!(number("22%"), Some(22.0));
        assert_eq!(number("-0.5em"), Some(-0.5));
        assert_eq!(number("px"), None);
        assert_eq!(text(&Value::from(0.12)).as_deref(), Some("0.12"));
        assert_eq!(text(&Value::from(2)).as_deref(), Some("2"));
    }
}
