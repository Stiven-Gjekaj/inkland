//! The edit actions of the Submap tool, of the Variants and overlays
//! editor, and of the Labels editor. Each action is one or two
//! `inkland::Command`s.

use crate::model::{Editor, round2};
use crate::options::{number, number_text};
use crate::rows::Row;
use inkland::Config;
use serde_json::{Value, json};

/// Escape a key for a JSON Pointer.
fn esc(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// A crop rectangle from two corners, rounded, or `None` if it is empty.
pub fn crop_from_drag(a: [f64; 2], b: [f64; 2]) -> Option<[f64; 4]> {
    let r = [
        round2(a[0].min(b[0])),
        round2(a[1].min(b[1])),
        round2(a[0].max(b[0])),
        round2(a[1].max(b[1])),
    ];
    (r[2] > r[0] && r[3] > r[1]).then_some(r)
}

/// The label options of an item in an overlay.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LabelSetting {
    pub hide: bool,
    pub size_pt: Option<f64>,
}

/// The items that can have label options: `<kind>:<id>` and a name.
pub fn label_items(c: &Config) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for p in &c.pins.points {
        out.push((
            format!("place:{}", p.id),
            p.name.clone().unwrap_or_else(|| p.id.clone()),
        ));
    }
    for r in &c.pins.regions {
        out.push((
            format!("region:{}", r.id),
            r.name.clone().unwrap_or_else(|| r.id.clone()),
        ));
    }
    for r in &c.pins.rivers {
        out.push((
            format!("river:{}", r.id),
            r.name.clone().unwrap_or_else(|| r.id.clone()),
        ));
    }
    out
}

/// The label option of an item for the variant `v`.
pub fn label_setting(c: &Config, v: usize, key: &str) -> LabelSetting {
    c.variants
        .get(v)
        .and_then(|v| v.overlay.as_ref())
        .and_then(|o| c.overlays.get(o))
        .and_then(|o| o.labels.get(key))
        .map(|l| LabelSetting {
            hide: l.hide,
            size_pt: l.size_pt,
        })
        .unwrap_or_default()
}

/// The rows of the Labels editor: name, kind, shown or hidden, size.
pub fn label_rows(c: &Config, v: usize) -> Vec<Row> {
    label_items(c)
        .into_iter()
        .map(|(key, name)| {
            let s = label_setting(c, v, &key);
            let kind = key.split(':').next().unwrap_or("").to_owned();
            Row {
                cells: vec![
                    name,
                    kind,
                    if s.hide { "hidden" } else { "shown" }.to_owned(),
                    s.size_pt.map_or("default".to_owned(), number_text),
                ],
                id: key,
                pinned: true,
            }
        })
        .collect()
}

/// The rows of the variants: name, overlay, page, crop.
pub fn variant_rows(c: &Config) -> Vec<Row> {
    c.variants
        .iter()
        .enumerate()
        .map(|(i, v)| Row {
            id: i.to_string(),
            cells: vec![
                v.name.clone(),
                v.overlay.clone().unwrap_or_else(|| "none".to_owned()),
                format!(
                    "{} x {} mm",
                    number_text(v.page.w_mm),
                    number_text(v.page.h_mm)
                ),
                v.crop.map_or("full frame".to_owned(), |r| {
                    format!(
                        "{}, {} to {}, {}",
                        number_text(r[0]),
                        number_text(r[1]),
                        number_text(r[2]),
                        number_text(r[3])
                    )
                }),
            ],
            pinned: true,
        })
        .collect()
}

/// The rows of the overlays: name, the number of added items, removed
/// items, renamed items, and label options.
pub fn overlay_rows(c: &Config) -> Vec<Row> {
    c.overlays
        .iter()
        .map(|(name, o)| Row {
            id: name.clone(),
            cells: vec![
                name.clone(),
                (o.add.places.len() + o.add.features.len() + o.add.roads.len()).to_string(),
                o.remove.len().to_string(),
                o.rename.len().to_string(),
                o.labels.len().to_string(),
            ],
            pinned: true,
        })
        .collect()
}

impl Editor {
    fn variant_name_free(&self, base: &str) -> String {
        let used = |n: &str| self.config().variants.iter().any(|v| v.name == n);
        if !used(base) {
            return base.to_owned();
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|n| !used(n))
            .expect("a free name exists")
    }

    /// Set or remove the crop of the variant `v`.
    pub fn set_crop(&mut self, v: usize, crop: Option<[f64; 4]>) -> Result<(), String> {
        if v >= self.config().variants.len() {
            return Err(format!("there is no variant {v}"));
        }
        let path = format!("/variants/{v}/crop");
        match crop {
            Some(r) => self.set(&path, json!(r.map(number))),
            None => self.unset(&path),
        }
    }

    /// Add a variant: a copy of the variant `from` with a new name. With a
    /// crop, the page keeps its width and gets the shape of the crop. It
    /// is one edit of `/variants`. The result is the index of the variant.
    pub fn add_variant(&mut self, from: usize, crop: Option<[f64; 4]>) -> Result<usize, String> {
        let c = self.config();
        let base = c
            .variants
            .get(from)
            .ok_or_else(|| format!("there is no variant {from}"))?;
        let mut v = serde_json::to_value(base).map_err(|e| e.to_string())?;
        let name = self.variant_name_free(if crop.is_some() { "submap" } else { "variant" });
        v["name"] = json!(name);
        if let Some(r) = crop {
            v["crop"] = json!(r.map(number));
            let w = base.page.w_mm - 2.0 * base.page.margin_mm;
            let h = w * (r[3] - r[1]) / (r[2] - r[0]);
            v["page"]["h_mm"] = number((h + 2.0 * base.page.margin_mm).round());
        }
        let mut all = serde_json::to_value(&c.variants).map_err(|e| e.to_string())?;
        let list = all.as_array_mut().expect("variants is an array");
        list.push(v);
        let index = list.len() - 1;
        self.set("/variants", all)?;
        Ok(index)
    }

    /// Remove the variant `v`. The last variant cannot go.
    pub fn remove_variant(&mut self, v: usize) -> Result<(), String> {
        let c = self.config();
        if c.variants.len() < 2 {
            return Err("a config needs one variant".to_owned());
        }
        if v >= c.variants.len() {
            return Err(format!("there is no variant {v}"));
        }
        let mut all = serde_json::to_value(&c.variants).map_err(|e| e.to_string())?;
        all.as_array_mut().expect("variants is an array").remove(v);
        self.set("/variants", all)?;
        if self.variant >= self.config().variants.len() {
            self.variant = self.config().variants.len() - 1;
        }
        Ok(())
    }

    /// Set a field of the variant `v` from text: `name`, `w_mm`, `h_mm`,
    /// or `margin_mm`.
    pub fn set_variant_field(&mut self, v: usize, field: &str, text: &str) -> Result<(), String> {
        let text = text.trim();
        let (path, value) = match field {
            "name" => (format!("/variants/{v}/name"), json!(text)),
            "w_mm" | "h_mm" | "margin_mm" => {
                let n: f64 = text
                    .parse()
                    .map_err(|_| format!("{text:?} is not a number"))?;
                (format!("/variants/{v}/page/{field}"), number(n))
            }
            _ => return Err(format!("there is no variant field {field:?}")),
        };
        self.set(&path, value)
    }

    /// Show an overlay in the variant `v`, or no overlay.
    pub fn set_variant_overlay(&mut self, v: usize, overlay: Option<&str>) -> Result<(), String> {
        let path = format!("/variants/{v}/overlay");
        match overlay {
            Some(o) => self.set(&path, json!(o)),
            None => self.unset(&path),
        }
    }

    /// Add an empty overlay.
    pub fn add_overlay(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if self.config().overlays.contains_key(name) {
            return Err(format!("there is already an overlay {name:?}"));
        }
        self.set(&format!("/overlays/{}", esc(name)), json!({}))
    }

    /// Remove an overlay. No variant can show it.
    pub fn remove_overlay(&mut self, name: &str) -> Result<(), String> {
        self.unset(&format!("/overlays/{}", esc(name)))
    }

    /// The overlay of the variant `v`. If the variant has none, add an
    /// overlay with the name of the variant and show it in the variant.
    fn ensure_overlay(&mut self, v: usize) -> Result<String, String> {
        let c = self.config();
        let variant = c
            .variants
            .get(v)
            .ok_or_else(|| format!("there is no variant {v}"))?;
        if let Some(o) = &variant.overlay {
            return Ok(o.clone());
        }
        let name = variant.name.clone();
        if !c.overlays.contains_key(&name) {
            self.add_overlay(&name)?;
        }
        self.set_variant_overlay(v, Some(&name))?;
        Ok(name)
    }

    /// Set the label options of an item (`<kind>:<id>`) in the overlay of
    /// the variant `v`. The default options remove the entry.
    pub fn set_label(&mut self, v: usize, key: &str, setting: LabelSetting) -> Result<(), String> {
        if label_setting(self.config(), v, key) == setting {
            return Ok(());
        }
        let overlay = self.ensure_overlay(v)?;
        let path = format!("/overlays/{}/labels/{}", esc(&overlay), esc(key));
        if setting == LabelSetting::default() {
            return self.unset(&path);
        }
        let mut value = json!({});
        if setting.hide {
            value["hide"] = Value::Bool(true);
        }
        if let Some(s) = setting.size_pt {
            value["size_pt"] = number(s);
        }
        self.set(&path, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> Editor {
        Editor::new(
            serde_json::from_value(json!({
                "version": 1, "seed": 11,
                "frame": { "width": 200, "height": 150 },
                "world": { "cells": 1500, "template": "pangea" },
                "pins": {
                    "points": [{ "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [50, 70] }],
                    "regions": [{ "id": "hills", "kind": "hills", "box": [80, 90, 120, 130] }]
                },
                "variants": [{ "name": "full", "page": { "w_mm": 160, "h_mm": 120, "margin_mm": 5 } }]
            }))
            .unwrap(),
        )
    }

    #[test]
    fn the_submap_tool_sets_a_crop_or_adds_a_variant() {
        let mut e = editor();
        let crop = crop_from_drag([120.0, 100.0], [20.0, 50.0]).unwrap();
        assert_eq!(crop, [20.0, 50.0, 120.0, 100.0]);
        assert_eq!(crop_from_drag([1.0, 1.0], [1.0, 5.0]), None);
        e.set_crop(0, Some(crop)).unwrap();
        assert_eq!(e.config().variants[0].crop, Some(crop));
        e.set_crop(0, None).unwrap();
        assert_eq!(e.config().variants[0].crop, None);
        let i = e.add_variant(0, Some(crop)).unwrap();
        let v = &e.config().variants[i];
        assert_eq!(v.name, "submap");
        // The page keeps its width: 150 mm inside the margins, and the
        // crop is 2:1, so 75 mm and the margins.
        assert_eq!((v.page.w_mm, v.page.h_mm), (160.0, 85.0));
        assert_eq!(e.add_variant(0, None).unwrap(), 2);
        assert_eq!(e.config().variants[2].name, "variant");
        e.remove_variant(1).unwrap();
        assert_eq!(e.config().variants.len(), 2);
        while e.undo().unwrap() {}
        assert_eq!(e.config(), e.original());
        assert!(e.remove_variant(0).is_err(), "the last variant stays");
    }

    #[test]
    fn the_variants_editor_edits_pages_and_overlays() {
        let mut e = editor();
        e.set_variant_field(0, "w_mm", "170").unwrap();
        e.set_variant_field(0, "name", "main").unwrap();
        assert!(e.set_variant_field(0, "name", "Not Valid").is_err());
        assert!(e.set_variant_field(0, "h_mm", "tall").is_err());
        e.add_overlay("war").unwrap();
        assert!(e.add_overlay("war").is_err());
        e.set_variant_overlay(0, Some("war")).unwrap();
        assert!(e.remove_overlay("war").is_err(), "the variant shows it");
        e.set_variant_overlay(0, None).unwrap();
        e.remove_overlay("war").unwrap();
        assert!(e.set_variant_overlay(0, Some("nothing")).is_err());
        assert_eq!(variant_rows(e.config())[0].cells[0], "main");
        while e.undo().unwrap() {}
        assert_eq!(e.config(), e.original());
    }

    #[test]
    fn a_label_option_adds_the_overlay_of_the_variant_when_needed() {
        let mut e = editor();
        let keys: Vec<String> = label_items(e.config())
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(keys, vec!["place:ostrey", "region:hills"]);
        let hide = LabelSetting {
            hide: true,
            size_pt: None,
        };
        e.set_label(0, "place:ostrey", hide).unwrap();
        // Three edits: the overlay, the variant, and the label.
        assert_eq!(e.undo_len(), 3);
        assert_eq!(e.config().variants[0].overlay.as_deref(), Some("full"));
        assert_eq!(label_setting(e.config(), 0, "place:ostrey"), hide);
        let big = LabelSetting {
            hide: false,
            size_pt: Some(11.0),
        };
        e.set_label(0, "region:hills", big).unwrap();
        assert_eq!(e.undo_len(), 4, "the overlay is there now");
        assert_eq!(label_rows(e.config(), 0)[1].cells[3], "11");
        e.set_label(0, "place:ostrey", LabelSetting::default())
            .unwrap();
        assert!(
            !e.config().overlays["full"]
                .labels
                .contains_key("place:ostrey")
        );
        assert_eq!(overlay_rows(e.config())[0].cells[4], "1");
        while e.undo().unwrap() {}
        assert_eq!(e.config(), e.original());
    }
}
