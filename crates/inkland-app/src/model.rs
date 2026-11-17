//! The view-model of the app: the config, the edit history, and the edit
//! actions of the tools.
//!
//! Each edit is an `inkland::Command` on the config, so undo, redo, and the
//! edit script come from `inkland::History`. This module does not use Slint,
//! so the tests can run without a window.

use crate::catalog::{self, Target};
use inkland::{Command, Config, Error, History, PinList};
use serde_json::{Value, json};

/// An error as one line of text: the code and the message.
pub fn error_text(e: &Error) -> String {
    format!("{}: {}", e.code.as_str(), e.message)
}

/// A point pin, as the pin editor shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct PinInfo {
    pub id: String,
    pub name: Option<String>,
    pub kind: String,
    pub at: [f64; 2],
}

/// The state of the editor: the config, its history, and the selection.
#[derive(Debug, Clone)]
pub struct Editor {
    /// The config when the file was opened. The edit script starts here.
    original: Config,
    config: Config,
    history: History,
    /// The index of the shown variant.
    pub variant: usize,
    /// The id of the selected point pin.
    pub selected: Option<String>,
}

/// Round a coordinate to 2 decimals, so that the config stays easy to read.
pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

impl Editor {
    pub fn new(config: Config) -> Editor {
        Editor {
            original: config.clone(),
            config,
            history: History::default(),
            variant: 0,
            selected: None,
        }
    }

    /// The current config.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The config when the file was opened.
    pub fn original(&self) -> &Config {
        &self.original
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// The name of the shown variant.
    pub fn variant_name(&self) -> String {
        self.config
            .variants
            .get(self.variant)
            .map(|v| v.name.clone())
            .unwrap_or_default()
    }

    /// Apply a command. The command is kept only if the new config passes
    /// `inkland::validate`. Else the config does not change and the result
    /// is the first error.
    pub fn run(&mut self, cmd: Command) -> Result<(), String> {
        let (next, _) = cmd.apply(&self.config).map_err(|e| error_text(&e))?;
        if let Some(e) = inkland::validate(&next).first() {
            return Err(error_text(e));
        }
        self.config = self
            .history
            .apply(&self.config, cmd)
            .map_err(|e| error_text(&e))?;
        Ok(())
    }

    /// The point pins of the config, in config order.
    pub fn points(&self) -> Vec<PinInfo> {
        self.config
            .pins
            .points
            .iter()
            .map(|p| PinInfo {
                id: p.id.clone(),
                name: p.name.clone(),
                kind: serde_json::to_value(p.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                at: p.at,
            })
            .collect()
    }

    /// The number of edits in the history.
    pub fn undo_len(&self) -> usize {
        self.history.script().commands.len()
    }

    /// A pin id that no pin of the config has: `pin-1`, `pin-2`, and so on
    /// for the prefix `pin`.
    pub(crate) fn free_id(&self, prefix: &str) -> String {
        let pins = &self.config.pins;
        let used = |id: &str| {
            pins.points.iter().any(|p| p.id == id)
                || pins.regions.iter().any(|p| p.id == id)
                || pins.rivers.iter().any(|p| p.id == id)
                || pins.roads.iter().any(|p| p.id == id)
        };
        (1..)
            .map(|n| format!("{prefix}-{n}"))
            .find(|id| !used(id))
            .expect("a free id exists")
    }

    /// Add a point pin of the kind `kind` (for example `village`) at `at`.
    /// The new pin is selected. The result is its id.
    pub fn add_pin(&mut self, at: [f64; 2], kind: &str) -> Result<String, String> {
        let id = self.free_id("pin");
        self.run(Command::AddPin {
            list: PinList::Points,
            pin: json!({ "id": id, "kind": kind, "at": [round2(at[0]), round2(at[1])] }),
            index: None,
        })?;
        self.selected = Some(id.clone());
        Ok(id)
    }

    /// Move a point pin to `at`.
    pub fn move_pin(&mut self, id: &str, at: [f64; 2]) -> Result<(), String> {
        self.run(Command::MovePin {
            id: id.to_owned(),
            at: [round2(at[0]), round2(at[1])],
        })
    }

    /// Remove a point pin.
    pub fn remove_pin(&mut self, id: &str) -> Result<(), String> {
        self.run(Command::RemovePin {
            list: PinList::Points,
            id: id.to_owned(),
        })?;
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
        Ok(())
    }

    /// Give a point pin a name. An empty name removes the name, so that the
    /// name is generated.
    pub fn rename_pin(&mut self, id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        self.run(Command::Rename {
            list: PinList::Points,
            id: id.to_owned(),
            name: (!name.is_empty()).then(|| name.to_owned()),
        })
    }

    /// Add a heightmap brush stroke at `at`.
    pub fn brush(&mut self, at: [f64; 2], radius: f64, delta: f64) -> Result<(), String> {
        self.run(Command::Brush {
            at: [round2(at[0]), round2(at[1])],
            radius: round2(radius),
            delta: round2(delta),
            index: None,
        })
    }

    /// Set the value at a JSON Pointer of the config. Nothing happens if
    /// the value is already there, so that the history gets no empty edit.
    pub fn set(&mut self, path: &str, value: Value) -> Result<(), String> {
        if catalog::value_at(&self.config, path).as_ref() == Some(&value) {
            return Ok(());
        }
        self.run(Command::Set {
            path: path.to_owned(),
            value,
        })
    }

    /// Remove the value at a JSON Pointer, so that the default applies.
    /// Nothing happens if there is no value.
    pub fn unset(&mut self, path: &str) -> Result<(), String> {
        if catalog::value_at(&self.config, path).is_none() {
            return Ok(());
        }
        self.run(Command::Unset {
            path: path.to_owned(),
        })
    }

    /// Show or hide the layer with the FMG id `id` (see
    /// `catalog::LAYERS`). A map layer is in `/render/layers`. A layer that
    /// comes back goes to its place in the draw order. The scale bar, the
    /// wind rose, and the grid are in `/render/extras`.
    pub fn set_layer(&mut self, id: &str, on: bool) -> Result<(), String> {
        let def = catalog::layer_def(id).ok_or_else(|| format!("there is no layer {id:?}"))?;
        if !catalog::available(def) {
            return Err(format!("the renderer has no layer {:?}", def.label));
        }
        if catalog::is_on(&self.config, def) == on {
            return Ok(());
        }
        match def.target {
            Target::Layer(name) => {
                let mut layers = catalog::config_layers(&self.config);
                if on {
                    let k = catalog::order_of(name);
                    let at = layers
                        .iter()
                        .position(|l| catalog::order_of(l) > k)
                        .unwrap_or(layers.len());
                    layers.insert(at, name.to_owned());
                } else {
                    layers.retain(|l| l != name);
                }
                self.set("/render/layers", json!(layers))
            }
            Target::Extra("grid") if on => {
                let step = catalog::grid_step(&self.config);
                self.set("/render/extras/grid", json!({ "step": step }))
            }
            Target::Extra("grid") => self.unset("/render/extras/grid"),
            Target::Extra(field) => self.set(&format!("/render/extras/{field}"), json!(on)),
            Target::None => Err(format!("the renderer has no layer {:?}", def.label)),
        }
    }

    /// Show the layers of an FMG layer preset (see `catalog::PRESETS`). It
    /// is one edit of `/render`.
    pub fn apply_preset(&mut self, id: &str) -> Result<(), String> {
        let render = catalog::render_with_preset(&self.config, id)
            .ok_or_else(|| format!("there is no layer preset {id:?}"))?;
        self.set("/render", render)
    }

    /// Undo the last edit. `false` if there is nothing to undo.
    pub fn undo(&mut self) -> Result<bool, String> {
        match self.history.undo(&self.config) {
            None => Ok(false),
            Some(r) => {
                self.config = r.map_err(|e| error_text(&e))?;
                self.after_history();
                Ok(true)
            }
        }
    }

    /// Redo the last undone edit. `false` if there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool, String> {
        match self.history.redo(&self.config) {
            None => Ok(false),
            Some(r) => {
                self.config = r.map_err(|e| error_text(&e))?;
                self.after_history();
                Ok(true)
            }
        }
    }

    /// Keep the selection and the variant valid after undo or redo.
    fn after_history(&mut self) {
        if let Some(id) = &self.selected
            && !self.config.pins.points.iter().any(|p| &p.id == id)
        {
            self.selected = None;
        }
        if self.variant >= self.config.variants.len() {
            self.variant = 0;
        }
    }

    /// The edit script of the history: the commands that change the opened
    /// config into the current config. `inkland edit` replays it.
    pub fn script_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(&self.history.script())
            .expect("a script converts to JSON");
        text.push('\n');
        text
    }

    /// The text of the current config file.
    pub fn config_json(&self) -> String {
        let mut text =
            serde_json::to_string_pretty(&self.config).expect("a config converts to JSON");
        text.push('\n');
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pins::Shape;
    use inkland_core::config::{Anchor, RegionRef};

    /// A small world with two places, a road between them, and a region.
    fn config() -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 11,
            "frame": { "width": 200, "height": 150 },
            "world": { "cells": 1500, "template": "pangea", "states": 2, "towns": 3 },
            "pins": {
                "points": [
                    { "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [50, 70] },
                    { "id": "harrow", "kind": "town", "at": [140, 80] },
                    { "id": "mill", "kind": "village", "at": [100, 40] }
                ],
                "regions": [{ "id": "hills", "kind": "hills", "box": [80, 90, 120, 130] }],
                "roads": [{ "id": "kings-road", "from": "ostrey", "to": "harrow" }]
            },
            "variants": [
                { "name": "full", "page": { "w_mm": 167, "h_mm": 125 } },
                { "name": "west", "crop": [0, 0, 100, 150], "page": { "w_mm": 100, "h_mm": 150 } }
            ]
        }))
        .unwrap()
    }

    /// One app action: what one tool does for one click, drag, or button.
    type Action = Box<dyn Fn(&mut Editor) -> Result<(), String>>;

    /// The app actions of the tests, in the order of a session.
    fn actions() -> Vec<(&'static str, Action)> {
        vec![
            (
                "add pin",
                Box::new(|e| e.add_pin([60.123, 100.456], "village").map(|_| ())),
            ),
            (
                "move pin",
                Box::new(|e| e.move_pin("harrow", [150.0, 70.0])),
            ),
            (
                "rename pin",
                Box::new(|e| e.rename_pin("pin-1", "Little Mere")),
            ),
            (
                "brush raise",
                Box::new(|e| e.brush([100.0, 75.0], 20.0, 15.0)),
            ),
            (
                "brush lower",
                Box::new(|e| e.brush([30.0, 30.0], 12.5, -10.0)),
            ),
            ("remove pin", Box::new(|e| e.remove_pin("mill"))),
            ("clear name", Box::new(|e| e.rename_pin("ostrey", " "))),
            ("hide layer", Box::new(|e| e.set_layer("relief", false))),
            ("show layer", Box::new(|e| e.set_layer("relief", true))),
            ("hide labels", Box::new(|e| e.set_layer("labels", false))),
            ("layer preset", Box::new(|e| e.apply_preset("political"))),
            ("option", Box::new(|e| e.set_option("states", "3"))),
            ("style option", Box::new(|e| e.set_option("size-city", "9"))),
            (
                "add region",
                Box::new(|e| {
                    e.add_region("forest", &Shape::Circle([40.0, 40.0], 15.0))
                        .map(|_| ())
                }),
            ),
            (
                "region height",
                Box::new(|e| e.set_region_height("hills", Some([40.0, 55.0]))),
            ),
            (
                "add river",
                Box::new(|e| {
                    e.add_river(&[
                        Anchor::Region(RegionRef {
                            region: "hills".into(),
                        }),
                        Anchor::Point([1.0, 149.0]),
                    ])
                    .map(|_| ())
                }),
            ),
            (
                "add road",
                Box::new(|e| e.add_road("pin-1", "harrow", &[], "trail").map(|_| ())),
            ),
            (
                "road length",
                Box::new(|e| e.set_road_target("kings-road", Some(110.0))),
            ),
            (
                "pin a place",
                Box::new(|e| {
                    e.pin_place([170.0, 30.0], "village", "Northend")
                        .map(|_| ())
                }),
            ),
            ("remove brush", Box::new(|e| e.remove_brush(0))),
        ]
    }

    #[test]
    fn app_edits_saved_as_a_script_give_the_same_world_when_the_cli_replays_them() {
        let start = config();
        let mut e = Editor::new(start.clone());
        for (name, act) in actions() {
            act(&mut e).unwrap_or_else(|err| panic!("{name}: {err}"));
        }
        // An undo and a redo do not change the script.
        assert!(e.undo().unwrap());
        assert!(e.redo().unwrap());
        assert_eq!(e.history.script().commands.len(), actions().len());

        let script = e.script_json();
        // The config file: `inkland edit --config`.
        let (replayed, _) = inkland::edit_config(&start, &script).unwrap();
        assert_eq!(&replayed, e.config());
        assert_eq!(
            serde_json::to_string_pretty(&replayed).unwrap() + "\n",
            e.config_json(),
            "the saved config has the same bytes"
        );
        // The world file: `inkland edit --world`.
        let first = inkland::generate(&start, None).unwrap();
        let edited = inkland::edit_world(&first.world_json, &script).unwrap();
        let app = inkland::generate(e.config(), None).unwrap();
        assert_ne!(
            app.world_json, first.world_json,
            "the edits change the world"
        );
        assert_eq!(edited.world_json, app.world_json);
        // The app shows the same SVG as the CLI makes from the replayed
        // config.
        let cli = inkland::generate(&replayed, None).unwrap();
        assert_eq!(cli.variants, app.variants);
    }

    #[test]
    fn undo_of_each_action_gives_back_the_previous_config() {
        let mut e = Editor::new(config());
        for (name, act) in actions() {
            let before = e.config().clone();
            act(&mut e).unwrap_or_else(|err| panic!("{name}: {err}"));
            let after = e.config().clone();
            assert_ne!(after, before, "{name} changes the config");
            assert!(e.undo().unwrap(), "{name}");
            assert_eq!(e.config(), &before, "undo of {name}");
            assert!(e.redo().unwrap(), "{name}");
            assert_eq!(e.config(), &after, "redo of {name}");
        }
        // Undo of all actions gives back the opened config.
        while e.undo().unwrap() {}
        assert_eq!(e.config(), e.original());
        assert!(!e.can_undo() && e.can_redo());
    }

    #[test]
    fn an_edit_that_makes_the_config_invalid_is_refused() {
        let mut e = Editor::new(config());
        let before = e.config().clone();
        let err = e.move_pin("harrow", [500.0, 10.0]).unwrap_err();
        assert!(err.starts_with("PIN_OUTSIDE_FRAME"), "{err}");
        // The road needs its end place.
        assert!(e.remove_pin("harrow").is_err());
        assert!(e.rename_pin("nobody", "X").is_err());
        assert_eq!(e.config(), &before);
        assert!(!e.can_undo());
    }

    #[test]
    fn new_pins_get_free_ids_and_the_selection_follows_the_pins() {
        let mut e = Editor::new(config());
        assert_eq!(e.add_pin([10.0, 10.0], "village").unwrap(), "pin-1");
        assert_eq!(e.add_pin([20.0, 10.0], "ruin").unwrap(), "pin-2");
        assert_eq!(e.selected.as_deref(), Some("pin-2"));
        let p = e.points().into_iter().find(|p| p.id == "pin-2").unwrap();
        assert_eq!((p.kind.as_str(), p.at), ("ruin", [20.0, 10.0]));
        e.undo().unwrap();
        assert_eq!(e.selected, None, "the pin of the selection is gone");
        e.remove_pin("pin-1").unwrap();
        assert_eq!(e.add_pin([10.0, 10.0], "town").unwrap(), "pin-1");
    }

    #[test]
    fn a_layer_that_comes_back_takes_its_place_in_the_order() {
        let mut e = Editor::new(config());
        let all = e.config().render.layers.clone();
        e.set_layer("rivers", false).unwrap();
        assert!(!catalog::config_layers(e.config()).contains(&"rivers".to_owned()));
        e.set_layer("rivers", false).unwrap();
        assert_eq!(e.history.script().commands.len(), 1, "no change, no edit");
        e.set_layer("rivers", true).unwrap();
        assert_eq!(e.config().render.layers, all);
    }

    #[test]
    fn a_layer_preset_is_one_edit() {
        let mut e = Editor::new(config());
        e.apply_preset("landmass").unwrap();
        assert_eq!(e.history.script().commands.len(), 1);
        assert_eq!(catalog::current_preset(e.config()), Some("landmass"));
        assert!(!catalog::config_layers(e.config()).contains(&"labels".to_owned()));
        e.apply_preset("landmass").unwrap();
        assert_eq!(e.history.script().commands.len(), 1, "no change, no edit");
        assert!(e.apply_preset("nothing").is_err());
        e.undo().unwrap();
        assert_eq!(e.config(), e.original());
    }

    #[test]
    fn the_scale_bar_and_the_grid_are_extras() {
        let mut e = Editor::new(config());
        e.set_layer("scaleBar", true).unwrap();
        assert_eq!(e.config().render.extras.scale_bar, Some(true));
        e.set_layer("grid", true).unwrap();
        assert!(e.config().render.extras.grid.is_some());
        e.set_layer("grid", false).unwrap();
        assert!(e.config().render.extras.grid.is_none());
        assert!(
            e.set_layer("rulers", true).is_err(),
            "no such renderer layer"
        );
        assert!(e.set_layer("nothing", true).is_err());
        while e.undo().unwrap() {}
        assert_eq!(e.config(), e.original());
    }
}
