//! Read a config: parse the JSON, apply the flags, check the schema.

use inkland_core::config::{CONFIG_VERSION, Config};
use inkland_core::error::{Error, ErrorCode};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A change to one value of the config, from a flag such as `--seed` or
/// `--set /world/cells=20000`.
#[derive(Debug, Clone, PartialEq)]
pub struct Override {
    /// JSON Pointer to the value, for example `/world/cells`.
    pub pointer: String,
    /// The new value.
    pub value: Value,
}

impl Override {
    /// Read the text of a `--set` flag: `POINTER=JSON`. A value that is not
    /// valid JSON is a string.
    pub fn parse(text: &str) -> Result<Override, Error> {
        let Some((pointer, raw)) = text.split_once('=') else {
            return Err(Error::new(
                ErrorCode::ConfigSetInvalid,
                "",
                format!("--set needs POINTER=JSON, found {text:?}"),
            ));
        };
        if !pointer.starts_with('/') {
            return Err(Error::new(
                ErrorCode::ConfigSetInvalid,
                "",
                format!("the pointer must start with '/', found {pointer:?}"),
            ));
        }
        let value = serde_json::from_str(raw).unwrap_or_else(|_| Value::from(raw));
        Ok(Override {
            pointer: pointer.to_owned(),
            value,
        })
    }
}

/// Parse a config from JSON text and apply the overrides in order.
///
/// The result is the effective config. The errors have stable codes and
/// JSON Pointer paths.
pub fn parse_config(text: &str, overrides: &[Override]) -> Result<Config, Error> {
    let mut value: Value = serde_json::from_str(text).map_err(|e| {
        Error::new(
            ErrorCode::ConfigParse,
            "",
            format!("the config is not valid JSON: {e}"),
        )
        .with("line", e.line())
        .with("column", e.column())
    })?;
    for o in overrides {
        set_pointer(&mut value, o)?;
    }
    from_value(value)
}

/// Check the schema of a JSON value and make a config from it.
pub fn from_value(value: Value) -> Result<Config, Error> {
    if !value.is_object() {
        return Err(Error::new(
            ErrorCode::ConfigSchema,
            "",
            "the config must be a JSON object",
        ));
    }
    match value.get("version") {
        Some(v) if v.as_u64() == Some(u64::from(CONFIG_VERSION)) => {}
        Some(v) => {
            return Err(Error::new(
                ErrorCode::ConfigVersion,
                "/version",
                format!("this tool reads config version {CONFIG_VERSION}, found {v}"),
            )
            .with("supported", CONFIG_VERSION));
        }
        None => {
            return Err(Error::new(
                ErrorCode::ConfigSchema,
                "",
                "the config has no \"version\" field",
            ));
        }
    }
    serde_path_to_error::deserialize(value).map_err(|e| {
        let path = pointer_of(e.path());
        Error::new(ErrorCode::ConfigSchema, path, e.inner().to_string())
    })
}

/// The hash of the effective config: `sha256:` and 64 hex digits.
pub fn config_hash(config: &Config) -> String {
    let bytes = serde_json::to_vec(config).expect("a config always converts to JSON");
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

/// The hash of the part of a config that makes the world: the config
/// without its overlays, variants, render options, and font directories. A
/// change to one of these, or to their defaults, does not change it.
pub fn world_hash(config: &Config) -> String {
    let mut v = serde_json::to_value(config).expect("a config always converts to JSON");
    if let Some(map) = v.as_object_mut() {
        for key in ["overlays", "variants", "render", "font_dirs"] {
            map.remove(key);
        }
    }
    let bytes = serde_json::to_vec(&v).expect("a value always converts to JSON");
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

fn pointer_of(path: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;
    let mut out = String::new();
    for seg in path.iter() {
        match seg {
            Segment::Seq { index } => out.push_str(&format!("/{index}")),
            Segment::Map { key } => out.push_str(&format!("/{}", escape(key))),
            Segment::Enum { .. } | Segment::Unknown => {}
        }
    }
    out
}

fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

/// Set the value at a JSON Pointer. Missing objects on the way are made.
fn set_pointer(root: &mut Value, o: &Override) -> Result<(), Error> {
    let bad = |why: &str| {
        Error::new(
            ErrorCode::ConfigSetInvalid,
            o.pointer.clone(),
            format!("cannot set {}: {why}", o.pointer),
        )
    };
    let tokens: Vec<String> = o.pointer.split('/').skip(1).map(unescape).collect();
    let Some((last, parents)) = tokens.split_last() else {
        return Err(bad("the pointer is empty"));
    };
    let mut cur = root;
    for t in parents {
        cur = match cur {
            Value::Object(map) => map
                .entry(t.clone())
                .or_insert_with(|| Value::Object(Default::default())),
            Value::Array(items) => {
                let i: usize = t
                    .parse()
                    .map_err(|_| bad("an array index is not a number"))?;
                items
                    .get_mut(i)
                    .ok_or_else(|| bad("the array index is too large"))?
            }
            _ => return Err(bad("a value on the way is not an object or an array")),
        };
    }
    match cur {
        Value::Object(map) => {
            map.insert(last.clone(), o.value.clone());
        }
        Value::Array(items) if last == "-" => items.push(o.value.clone()),
        Value::Array(items) => {
            let i: usize = last
                .parse()
                .map_err(|_| bad("an array index is not a number"))?;
            let slot = items
                .get_mut(i)
                .ok_or_else(|| bad("the array index is too large"))?;
            *slot = o.value.clone();
        }
        _ => return Err(bad("the parent is not an object or an array")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str =
        r#"{ "version": 1, "seed": 7, "frame": { "width": 400, "height": 300 } }"#;

    #[test]
    fn invalid_json_gives_config_parse_with_a_line() {
        let e = parse_config("{ \"version\": 1,\n  oops }", &[]).unwrap_err();
        assert_eq!(e.code, ErrorCode::ConfigParse);
        assert_eq!(e.details["line"], 2);
    }

    #[test]
    fn wrong_version_gives_config_version() {
        let e =
            parse_config(&MINIMAL.replace("\"version\": 1", "\"version\": 2"), &[]).unwrap_err();
        assert_eq!(e.code, ErrorCode::ConfigVersion);
        assert_eq!(e.path, "/version");
    }

    #[test]
    fn schema_error_gives_the_pointer_of_the_bad_value() {
        let text = r#"{ "version": 1, "seed": 7, "frame": { "width": 400, "height": 300 },
            "pins": { "points": [
              { "id": "a", "kind": "city", "at": [1, 2] },
              { "id": "b", "kind": "castle", "at": [1, 2] } ] } }"#;
        let e = parse_config(text, &[]).unwrap_err();
        assert_eq!(e.code, ErrorCode::ConfigSchema);
        assert_eq!(e.path, "/pins/points/1/kind");
    }

    #[test]
    fn overrides_replace_and_create_values() {
        let o = [
            Override::parse("/seed=99").unwrap(),
            Override::parse("/world/cells=20000").unwrap(),
            Override::parse("/units/name=league").unwrap(),
        ];
        let c = parse_config(MINIMAL, &o).unwrap();
        assert_eq!(c.seed, 99);
        assert_eq!(c.world.cells, 20000);
        assert_eq!(c.units.name, "league");
    }

    #[test]
    fn override_without_equals_sign_is_refused() {
        assert_eq!(
            Override::parse("/seed").unwrap_err().code,
            ErrorCode::ConfigSetInvalid
        );
    }

    #[test]
    fn override_through_a_number_is_refused() {
        let o = [Override::parse("/seed/x=1").unwrap()];
        assert_eq!(
            parse_config(MINIMAL, &o).unwrap_err().code,
            ErrorCode::ConfigSetInvalid
        );
    }

    #[test]
    fn hash_changes_with_the_config_and_only_then() {
        let a = parse_config(MINIMAL, &[]).unwrap();
        let other_layout =
            "{\"frame\":{\"height\":300,\"width\":400},\n \"seed\":7, \"version\":1}";
        assert_ne!(other_layout, MINIMAL);
        let b = parse_config(other_layout, &[]).unwrap();
        let c = parse_config(MINIMAL, &[Override::parse("/seed=8").unwrap()]).unwrap();
        assert_eq!(config_hash(&a), config_hash(&b));
        assert_ne!(config_hash(&a), config_hash(&c));
        assert!(config_hash(&a).starts_with("sha256:"));
        assert_eq!(config_hash(&a).len(), "sha256:".len() + 64);
    }
}
