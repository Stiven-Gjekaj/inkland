//! Edit commands for inkland.
//!
//! Each edit is a serializable `Command` on the config. The world comes from
//! the config, so an edit of the config is an edit of the world, and the
//! same commands give the same world on each machine. The CLI and the
//! desktop app use the same commands.
//!
//! Applying a command gives the new config and the inverse command. The
//! inverse gives back the exact config from before, so undo is "apply the
//! inverse". `History` keeps the undo and redo stacks. A `Script` is a list
//! of commands in a JSON file.

use inkland_core::config::{Config, Point, RenameEntry};
use inkland_core::error::{Error, ErrorCode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The lists of pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PinList {
    Points,
    Regions,
    Rivers,
    Roads,
}

impl PinList {
    fn key(self) -> &'static str {
        match self {
            PinList::Points => "points",
            PinList::Regions => "regions",
            PinList::Rivers => "rivers",
            PinList::Roads => "roads",
        }
    }
}

/// One edit of a config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Set the value at a JSON Pointer, for example `/world/sea_level`. An
    /// array index must exist. An object key can be new.
    Set { path: String, value: Value },
    /// Remove an object key at a JSON Pointer, so the default applies.
    Unset { path: String },
    /// Add a pin to a list, at `index` or at the end.
    AddPin {
        list: PinList,
        pin: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    /// Remove the pin with this id from a list.
    RemovePin { list: PinList, id: String },
    /// Move a point pin.
    MovePin { id: String, at: Point },
    /// Change the name of a pin. No name: the name is generated.
    Rename {
        list: PinList,
        id: String,
        #[serde(default)]
        name: Option<String>,
    },
    /// Add a heightmap brush stroke, at `index` or at the end.
    Brush {
        at: Point,
        radius: f64,
        delta: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    /// Remove the brush stroke at `index`.
    RemoveBrush { index: usize },
    /// Give a generated state, culture, religion, or province a name or
    /// arms (`world.renames`). Without `index`, the entry replaces the
    /// entry with the same kind and reference, or goes at the end. With
    /// `index`, it goes in at that index.
    NameArea {
        rename: Box<RenameEntry>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    /// Remove the rename at `index`.
    RemoveRename { index: usize },
}

/// A list of commands in a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Script {
    /// The script format version. Only 1.
    pub version: u32,
    pub commands: Vec<Command>,
}

fn invalid(path: &str, msg: impl Into<String>) -> Error {
    Error::new(ErrorCode::ConfigSetInvalid, path, msg)
}

fn unescape(t: &str) -> String {
    t.replace("~1", "/").replace("~0", "~")
}

/// The parent of a pointer target and its last token.
fn parent_mut<'a>(root: &'a mut Value, path: &str) -> Result<(&'a mut Value, String), Error> {
    if !path.starts_with('/') {
        return Err(invalid(path, "a path is a JSON Pointer that starts with /"));
    }
    let tokens: Vec<String> = path.split('/').skip(1).map(unescape).collect();
    let (last, parents) = tokens.split_last().expect("a pointer has a token");
    let mut cur = root;
    for t in parents {
        cur = match cur {
            Value::Object(map) => map
                .get_mut(t)
                .ok_or_else(|| invalid(path, format!("{t:?} does not exist")))?,
            Value::Array(items) => {
                let i: usize = t
                    .parse()
                    .map_err(|_| invalid(path, "an array index is not a number"))?;
                items
                    .get_mut(i)
                    .ok_or_else(|| invalid(path, "the array index is too large"))?
            }
            _ => {
                return Err(invalid(
                    path,
                    "a value on the way is not an object or an array",
                ));
            }
        };
    }
    Ok((cur, last.clone()))
}

fn to_value(config: &Config) -> Value {
    serde_json::to_value(config).expect("a config always converts to JSON")
}

fn to_config(v: Value) -> Result<Config, Error> {
    serde_json::from_value(v).map_err(|e| {
        Error::new(
            ErrorCode::ConfigSchema,
            "",
            format!("the edit makes a config that is not valid: {e}"),
        )
    })
}

/// The list of pins of a kind in a config value.
fn list_mut(v: &mut Value, list: PinList) -> &mut Vec<Value> {
    let pins = v
        .as_object_mut()
        .expect("a config is an object")
        .entry("pins")
        .or_insert_with(|| Value::Object(Default::default()));
    pins.as_object_mut()
        .expect("pins is an object")
        .entry(list.key())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .expect("a pin list is an array")
}

fn find(items: &[Value], list: PinList, id: &str) -> Result<usize, Error> {
    items.iter().position(|p| p["id"] == id).ok_or_else(|| {
        Error::new(
            ErrorCode::RefUnknown,
            format!("/pins/{}", list.key()),
            format!("there is no pin {id:?} in {}", list.key()),
        )
    })
}

/// A list of the world part of a config value, for example `brushes`.
fn world_list_mut<'a>(v: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
    v.as_object_mut()
        .expect("a config is an object")
        .entry("world")
        .or_insert_with(|| Value::Object(Default::default()))
        .as_object_mut()
        .expect("world is an object")
        .entry(key)
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .expect("a world list is an array")
}

fn brushes_mut(v: &mut Value) -> &mut Vec<Value> {
    world_list_mut(v, "brushes")
}

/// Remove an empty list of the world part, so that a config without items
/// keeps the same JSON as before.
fn tidy(v: &mut Value, key: &str) {
    if let Some(world) = v.get_mut("world").and_then(Value::as_object_mut)
        && world
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    {
        world.remove(key);
    }
}

fn tidy_brushes(v: &mut Value) {
    tidy(v, "brushes");
}

/// `true` if two renames name the same area: the same kind and reference.
fn same_area(a: &RenameEntry, b: &RenameEntry) -> bool {
    a.kind == b.kind && a.at == b.at && a.pin == b.pin
}

impl Command {
    /// Apply the command to a config. The result is the new config and the
    /// command that undoes this one.
    pub fn apply(&self, config: &Config) -> Result<(Config, Command), Error> {
        let mut v = to_value(config);
        let inverse = match self {
            Command::Set { path, value } => {
                let (parent, last) = parent_mut(&mut v, path)?;
                match parent {
                    Value::Object(map) => {
                        let old = map.insert(last, value.clone());
                        match old {
                            Some(old) => Command::Set {
                                path: path.clone(),
                                value: old,
                            },
                            None => Command::Unset { path: path.clone() },
                        }
                    }
                    Value::Array(items) => {
                        let i: usize = last
                            .parse()
                            .map_err(|_| invalid(path, "an array index is not a number"))?;
                        let slot = items
                            .get_mut(i)
                            .ok_or_else(|| invalid(path, "the array index is too large"))?;
                        let old = std::mem::replace(slot, value.clone());
                        Command::Set {
                            path: path.clone(),
                            value: old,
                        }
                    }
                    _ => return Err(invalid(path, "the parent is not an object or an array")),
                }
            }
            Command::Unset { path } => {
                let (parent, last) = parent_mut(&mut v, path)?;
                let Value::Object(map) = parent else {
                    return Err(invalid(
                        path,
                        "unset removes an object key; use remove_pin for a pin",
                    ));
                };
                let old = map
                    .remove(&last)
                    .ok_or_else(|| invalid(path, format!("{last:?} does not exist")))?;
                Command::Set {
                    path: path.clone(),
                    value: old,
                }
            }
            Command::AddPin { list, pin, index } => {
                let id = pin["id"].as_str().ok_or_else(|| {
                    Error::new(
                        ErrorCode::ConfigSchema,
                        format!("/pins/{}", list.key()),
                        "a pin needs a string id",
                    )
                })?;
                let items = list_mut(&mut v, *list);
                if items.iter().any(|p| p["id"] == id) {
                    return Err(Error::new(
                        ErrorCode::IdDuplicate,
                        format!("/pins/{}", list.key()),
                        format!("there is already a pin {id:?}"),
                    ));
                }
                let at = index.unwrap_or(items.len()).min(items.len());
                items.insert(at, pin.clone());
                Command::RemovePin {
                    list: *list,
                    id: id.to_owned(),
                }
            }
            Command::RemovePin { list, id } => {
                let items = list_mut(&mut v, *list);
                let i = find(items, *list, id)?;
                let old = items.remove(i);
                Command::AddPin {
                    list: *list,
                    pin: old,
                    index: Some(i),
                }
            }
            Command::MovePin { id, at } => {
                let items = list_mut(&mut v, PinList::Points);
                let i = find(items, PinList::Points, id)?;
                let old: Point = serde_json::from_value(items[i]["at"].clone())
                    .map_err(|_| invalid("/pins/points", "the pin has no position"))?;
                items[i]["at"] = serde_json::json!(at);
                Command::MovePin {
                    id: id.clone(),
                    at: old,
                }
            }
            Command::Rename { list, id, name } => {
                let items = list_mut(&mut v, *list);
                let i = find(items, *list, id)?;
                let obj = items[i].as_object_mut().expect("a pin is an object");
                let old = match name {
                    Some(n) => obj.insert("name".into(), Value::from(n.clone())),
                    None => obj.remove("name"),
                };
                Command::Rename {
                    list: *list,
                    id: id.clone(),
                    name: old.and_then(|o| o.as_str().map(str::to_owned)),
                }
            }
            Command::Brush {
                at,
                radius,
                delta,
                index,
            } => {
                let items = brushes_mut(&mut v);
                let i = index.unwrap_or(items.len()).min(items.len());
                items.insert(
                    i,
                    serde_json::json!({ "at": at, "radius": radius, "delta": delta }),
                );
                Command::RemoveBrush { index: i }
            }
            Command::RemoveBrush { index } => {
                let items = brushes_mut(&mut v);
                if *index >= items.len() {
                    return Err(invalid(
                        "/world/brushes",
                        format!("there is no brush {index}"),
                    ));
                }
                let old = items.remove(*index);
                tidy_brushes(&mut v);
                let b: inkland_core::config::Brush = serde_json::from_value(old)
                    .map_err(|_| invalid("/world/brushes", "the brush is not valid"))?;
                Command::Brush {
                    at: b.at,
                    radius: b.radius,
                    delta: b.delta,
                    index: Some(*index),
                }
            }
            Command::NameArea { rename, index } => {
                let entry = serde_json::to_value(rename).expect("a rename converts to JSON");
                let items = world_list_mut(&mut v, "renames");
                let at = match index {
                    Some(_) => None,
                    None => items.iter().position(|old| {
                        serde_json::from_value::<RenameEntry>(old.clone())
                            .is_ok_and(|old| same_area(&old, rename))
                    }),
                };
                match (at, index) {
                    (Some(k), _) => {
                        let old = std::mem::replace(&mut items[k], entry);
                        let old: RenameEntry = serde_json::from_value(old)
                            .map_err(|_| invalid("/world/renames", "a rename is not valid"))?;
                        Command::NameArea {
                            rename: Box::new(old),
                            index: None,
                        }
                    }
                    (None, i) => {
                        let k = i.unwrap_or(items.len()).min(items.len());
                        items.insert(k, entry);
                        Command::RemoveRename { index: k }
                    }
                }
            }
            Command::RemoveRename { index } => {
                let items = world_list_mut(&mut v, "renames");
                if *index >= items.len() {
                    tidy(&mut v, "renames");
                    return Err(invalid(
                        "/world/renames",
                        format!("there is no rename {index}"),
                    ));
                }
                let old = items.remove(*index);
                tidy(&mut v, "renames");
                let old: RenameEntry = serde_json::from_value(old)
                    .map_err(|_| invalid("/world/renames", "a rename is not valid"))?;
                Command::NameArea {
                    rename: Box::new(old),
                    index: Some(*index),
                }
            }
        };
        Ok((to_config(v)?, inverse))
    }
}

impl Script {
    /// Read a script from JSON text.
    pub fn parse(text: &str) -> Result<Script, Error> {
        let script: Script = serde_json::from_str(text).map_err(|e| {
            Error::new(
                ErrorCode::EditScriptInvalid,
                "",
                format!("the edit script is not valid: {e}"),
            )
            .with("line", e.line())
            .with("column", e.column())
        })?;
        if script.version != 1 {
            return Err(Error::new(
                ErrorCode::EditScriptInvalid,
                "/version",
                format!("the script version {} is not 1", script.version),
            ));
        }
        Ok(script)
    }

    /// Apply each command in order. The result is the new config and the
    /// script that undoes all of the commands.
    pub fn apply(&self, config: &Config) -> Result<(Config, Script), Error> {
        let mut cur = config.clone();
        let mut undo = Vec::new();
        for (i, cmd) in self.commands.iter().enumerate() {
            let (next, inv) = cmd.apply(&cur).map_err(|e| {
                let path = format!("/commands/{i}{}", e.path);
                Error { path, ..e }
            })?;
            cur = next;
            undo.push(inv);
        }
        undo.reverse();
        Ok((
            cur,
            Script {
                version: 1,
                commands: undo,
            },
        ))
    }
}

/// Undo and redo stacks of applied commands.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct History {
    /// Pairs of a command and its inverse, oldest first.
    done: Vec<(Command, Command)>,
    /// Commands that undo removed, last undone last.
    undone: Vec<Command>,
}

impl History {
    /// Apply a command and keep it for undo. A new command clears redo.
    pub fn apply(&mut self, config: &Config, cmd: Command) -> Result<Config, Error> {
        let (next, inverse) = cmd.apply(config)?;
        self.done.push((cmd, inverse));
        self.undone.clear();
        Ok(next)
    }

    /// Undo the last command. `None` if there is nothing to undo.
    pub fn undo(&mut self, config: &Config) -> Option<Result<Config, Error>> {
        let (cmd, inverse) = self.done.pop()?;
        Some(inverse.apply(config).map(|(next, _)| {
            self.undone.push(cmd);
            next
        }))
    }

    /// Redo the last undone command. `None` if there is nothing to redo.
    pub fn redo(&mut self, config: &Config) -> Option<Result<Config, Error>> {
        let cmd = self.undone.pop()?;
        Some(cmd.apply(config).map(|(next, inverse)| {
            self.done.push((cmd, inverse));
            next
        }))
    }

    /// The applied commands, oldest first: a script that redoes the history
    /// from the first config.
    pub fn script(&self) -> Script {
        Script {
            version: 1,
            commands: self.done.iter().map(|(c, _)| c.clone()).collect(),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkland_core::config::AreaKind;
    use proptest::prelude::*;
    use serde_json::json;

    fn config() -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 3, "frame": { "width": 100, "height": 80 },
            "pins": {
                "points": [
                    { "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [20, 40] },
                    { "id": "harrow", "kind": "town", "at": [80, 40] }
                ],
                "regions": [{ "id": "hills", "kind": "hills", "box": [30, 20, 60, 50] }]
            }
        }))
        .unwrap()
    }

    fn undo_gives_back(c: &Config, cmd: &Command) {
        let (next, inverse) = cmd.apply(c).unwrap();
        let (back, _) = inverse.apply(&next).unwrap();
        assert_eq!(&back, c, "{cmd:?}");
        assert_eq!(to_value(&back), to_value(c));
    }

    #[test]
    fn a_rename_replaces_the_same_area_and_undo_gives_back_the_config() {
        let c = config();
        let state = |name: &str| RenameEntry {
            kind: AreaKind::State,
            pin: Some("ostrey".into()),
            name: Some(name.into()),
            ..RenameEntry::default()
        };
        let add = Command::NameArea {
            rename: Box::new(state("Ostrey")),
            index: None,
        };
        undo_gives_back(&c, &add);
        let (one, inv) = add.apply(&c).unwrap();
        assert_eq!(inv, Command::RemoveRename { index: 0 });
        assert_eq!(one.world.renames, vec![state("Ostrey")]);
        // The same area again: the entry changes in place.
        let change = Command::NameArea {
            rename: Box::new(state("Ostreya")),
            index: None,
        };
        undo_gives_back(&one, &change);
        let (two, _) = change.apply(&one).unwrap();
        assert_eq!(two.world.renames, vec![state("Ostreya")]);
        // A remove gives back the entry at its index.
        let remove = Command::RemoveRename { index: 0 };
        undo_gives_back(&two, &remove);
        let (none, _) = remove.apply(&two).unwrap();
        assert!(none.world.renames.is_empty());
        assert!(to_value(&none)["world"].get("renames").is_none());
        assert!(Command::RemoveRename { index: 0 }.apply(&none).is_err());
    }

    #[test]
    fn each_command_has_an_exact_inverse() {
        let c = config();
        for cmd in [
            Command::Set {
                path: "/seed".into(),
                value: json!(9),
            },
            Command::Set {
                path: "/world/sea_level".into(),
                value: json!(30),
            },
            Command::Set {
                path: "/pins/points/1/name".into(),
                value: json!("Harrowgate"),
            },
            Command::Unset {
                path: "/pins/points/0/name".into(),
            },
            Command::AddPin {
                list: PinList::Points,
                pin: json!({ "id": "mere", "kind": "village", "at": [50, 70] }),
                index: None,
            },
            Command::AddPin {
                list: PinList::Points,
                pin: json!({ "id": "first", "kind": "village", "at": [5, 5] }),
                index: Some(0),
            },
            Command::RemovePin {
                list: PinList::Points,
                id: "ostrey".into(),
            },
            Command::RemovePin {
                list: PinList::Regions,
                id: "hills".into(),
            },
            Command::MovePin {
                id: "harrow".into(),
                at: [70.0, 30.0],
            },
            Command::Rename {
                list: PinList::Points,
                id: "ostrey".into(),
                name: Some("Old Ostrey".into()),
            },
            Command::Rename {
                list: PinList::Points,
                id: "ostrey".into(),
                name: None,
            },
            Command::Rename {
                list: PinList::Points,
                id: "harrow".into(),
                name: Some("Harrow".into()),
            },
            Command::Brush {
                at: [50.0, 40.0],
                radius: 10.0,
                delta: 20.0,
                index: None,
            },
        ] {
            undo_gives_back(&c, &cmd);
        }
    }

    #[test]
    fn a_bad_command_is_an_error_and_changes_nothing() {
        let c = config();
        let e = Command::RemovePin {
            list: PinList::Points,
            id: "nowhere".into(),
        }
        .apply(&c)
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::RefUnknown);
        let e = Command::AddPin {
            list: PinList::Points,
            pin: json!({ "id": "ostrey", "kind": "city", "at": [1, 1] }),
            index: None,
        }
        .apply(&c)
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::IdDuplicate);
        let e = Command::Set {
            path: "/world/sea_level".into(),
            value: json!("high"),
        }
        .apply(&c)
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::ConfigSchema);
        let e = Command::Set {
            path: "/nowhere/x".into(),
            value: json!(1),
        }
        .apply(&c)
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::ConfigSetInvalid);
        let e = Command::RemoveBrush { index: 0 }.apply(&c).unwrap_err();
        assert_eq!(e.code, ErrorCode::ConfigSetInvalid);
    }

    #[test]
    fn a_script_gives_its_undo_script() {
        let c = config();
        let script = Script::parse(
            r#"{ "version": 1, "commands": [
                { "op": "move_pin", "id": "ostrey", "at": [25, 45] },
                { "op": "brush", "at": [50, 40], "radius": 12, "delta": -15 },
                { "op": "rename", "list": "points", "id": "harrow", "name": "Harrowgate" }
            ]}"#,
        )
        .unwrap();
        let (next, undo) = script.apply(&c).unwrap();
        assert_eq!(next.pins.points[0].at, [25.0, 45.0]);
        assert_eq!(next.world.brushes.len(), 1);
        let (back, _) = undo.apply(&next).unwrap();
        assert_eq!(back, c);
        let bad = Script::parse(r#"{ "version": 1, "commands": [{ "op": "fly" }] }"#);
        assert_eq!(bad.unwrap_err().code, ErrorCode::EditScriptInvalid);
        let e = Script::parse(r#"{ "version": 1, "commands": [{ "op": "remove_pin", "list": "points", "id": "x" }] }"#)
            .unwrap()
            .apply(&c)
            .unwrap_err();
        assert_eq!(e.path, "/commands/0/pins/points");
    }

    #[test]
    fn history_undoes_and_redoes() {
        let c0 = config();
        let mut h = History::default();
        let c1 = h
            .apply(
                &c0,
                Command::MovePin {
                    id: "ostrey".into(),
                    at: [10.0, 10.0],
                },
            )
            .unwrap();
        let c2 = h
            .apply(
                &c1,
                Command::Set {
                    path: "/seed".into(),
                    value: json!(77),
                },
            )
            .unwrap();
        assert_eq!(h.script().commands.len(), 2);
        let u1 = h.undo(&c2).unwrap().unwrap();
        assert_eq!(u1, c1);
        let u0 = h.undo(&u1).unwrap().unwrap();
        assert_eq!(u0, c0);
        assert!(h.undo(&u0).is_none());
        let r1 = h.redo(&u0).unwrap().unwrap();
        assert_eq!(r1, c1);
        assert!(h.can_redo() && h.can_undo());
    }

    fn command() -> impl Strategy<Value = Command> {
        let ids = prop::sample::select(vec!["ostrey", "harrow", "new-a", "new-b"]);
        prop_oneof![
            (0u64..1000).prop_map(|s| Command::Set {
                path: "/seed".into(),
                value: json!(s)
            }),
            (1u8..99).prop_map(|s| Command::Set {
                path: "/world/sea_level".into(),
                value: json!(s)
            }),
            (ids.clone(), 0.0f64..100.0, 0.0f64..80.0).prop_map(|(id, x, y)| Command::AddPin {
                list: PinList::Points,
                pin: json!({ "id": id, "kind": "village", "at": [x, y] }),
                index: None,
            }),
            ids.clone().prop_map(|id| Command::RemovePin {
                list: PinList::Points,
                id: id.into()
            }),
            (ids.clone(), 0.0f64..100.0, 0.0f64..80.0).prop_map(|(id, x, y)| Command::MovePin {
                id: id.into(),
                at: [x, y]
            }),
            (ids, prop::option::of("[A-Z][a-z]{2,8}")).prop_map(|(id, name)| Command::Rename {
                list: PinList::Points,
                id: id.into(),
                name,
            }),
            (0.0f64..100.0, 1.0f64..30.0, -50.0f64..50.0).prop_map(|(x, r, d)| Command::Brush {
                at: [x, 40.0],
                radius: r,
                delta: d,
                index: None
            }),
            (0usize..3).prop_map(|index| Command::RemoveBrush { index }),
            (
                0u8..4,
                prop::option::of("[A-Z][a-z]{2,8}"),
                prop::option::of(0usize..3)
            )
                .prop_map(|(k, name, index)| Command::NameArea {
                    rename: Box::new(RenameEntry {
                        kind: [
                            AreaKind::State,
                            AreaKind::Culture,
                            AreaKind::Religion,
                            AreaKind::Province
                        ][usize::from(k)],
                        pin: Some(["ostrey", "harrow"][usize::from(k) % 2].into()),
                        name,
                        ..RenameEntry::default()
                    }),
                    index,
                }),
            (0usize..3).prop_map(|index| Command::RemoveRename { index }),
        ]
    }

    proptest! {
        #[test]
        fn undo_of_any_command_sequence_gives_back_the_config(cmds in prop::collection::vec(command(), 1..12)) {
            let c0 = config();
            let mut h = History::default();
            let mut states = vec![c0.clone()];
            for cmd in cmds {
                let cur = states.last().unwrap().clone();
                if let Ok(next) = h.apply(&cur, cmd) {
                    states.push(next);
                }
            }
            let mut cur = states.pop().unwrap();
            while let Some(r) = h.undo(&cur) {
                cur = r.unwrap();
                prop_assert_eq!(&cur, states.last().unwrap());
                states.pop();
            }
            prop_assert_eq!(cur, c0);
        }
    }
}
