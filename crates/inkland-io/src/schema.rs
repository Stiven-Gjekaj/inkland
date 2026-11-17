//! The JSON Schema of the config.
//!
//! The schema comes from the Rust config types, so it cannot drift from the
//! code.

use inkland_core::config::Config;
use serde_json::Value;

/// The JSON Schema of the config, version 1.
pub fn config_schema() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(Config))
        .expect("a schema always converts to JSON");
    if let Value::Object(map) = &mut schema {
        map.insert(
            "$id".to_owned(),
            Value::from("https://github.com/Stiven-Gjekaj/inkland/schema/config-v1.json"),
        );
        map.insert("title".to_owned(), Value::from("inkland config, version 1"));
    }
    schema
}

/// The text of a schema description on one line, safe in a table cell.
fn one_line(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

/// The anchor of a type heading.
fn anchor(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// The type of a schema node, as Markdown.
fn type_of(node: &Value) -> String {
    if let Some(r) = node.get("$ref").and_then(Value::as_str) {
        let name = r.rsplit('/').next().unwrap_or(r);
        return format!("[`{name}`](#{})", anchor(name));
    }
    if let Some(list) = node
        .get("anyOf")
        .or_else(|| node.get("oneOf"))
        .and_then(Value::as_array)
    {
        let parts: Vec<String> = list
            .iter()
            .filter(|v| v.get("type") != Some(&Value::from("null")))
            .map(type_of)
            .collect();
        return parts.join(" or ");
    }
    if let Some(values) = node.get("enum").and_then(Value::as_array) {
        let parts: Vec<String> = values.iter().map(|v| format!("`{v}`")).collect();
        return parts.join(", ");
    }
    let kinds: Vec<&str> = match node.get("type") {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(ts)) => ts
            .iter()
            .filter_map(Value::as_str)
            .filter(|t| *t != "null")
            .collect(),
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for k in kinds {
        out.push(match k {
            "array" => {
                let item = node
                    .get("items")
                    .map_or_else(|| "value".to_owned(), type_of);
                match (node.get("minItems"), node.get("maxItems")) {
                    (Some(a), Some(b)) if a == b => format!("array of {a} {item}"),
                    _ => format!("array of {item}"),
                }
            }
            "object" => match node.get("additionalProperties") {
                Some(v @ Value::Object(_)) => format!("map of name to {}", type_of(v)),
                _ => "object".to_owned(),
            },
            other => other.to_owned(),
        });
    }
    if out.is_empty() {
        "value".to_owned()
    } else {
        out.join(" or ")
    }
}

/// The table of the properties of an object node.
fn property_table(node: &Value, out: &mut String) {
    let Some(props) = node.get("properties").and_then(Value::as_object) else {
        return;
    };
    let required: Vec<&str> = node
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    out.push_str("| Field | Type | Required | Default | Description |\n");
    out.push_str("|---|---|---|---|---|\n");
    for (name, p) in props {
        // An object default is in the table of its own type.
        let default = match p.get("default") {
            Some(Value::Object(_)) | None => String::new(),
            Some(d) if d.to_string().len() > 60 => "see the type".to_owned(),
            Some(d) => format!("`{d}`"),
        };
        let desc = p.get("description").and_then(Value::as_str).unwrap_or("");
        out.push_str(&format!(
            "| `{name}` | {} | {} | {default} | {} |\n",
            type_of(p),
            if required.contains(&name.as_str()) {
                "yes"
            } else {
                ""
            },
            one_line(desc)
        ));
    }
    out.push('\n');
}

/// The config reference in Markdown, made from the schema. `docs/CONFIG.md`
/// is this text, and a test keeps the two the same.
pub fn config_markdown() -> String {
    let schema = config_schema();
    let mut out = String::new();
    out.push_str("# Config reference\n\n");
    out.push_str("This file is made from the JSON Schema of the config. Do not edit it.\n");
    out.push_str("To make it again, run:\n\n");
    out.push_str(
        "```bash\ncargo run -p inkland-cli -- schema --markdown > docs/CONFIG.md\n```\n\n",
    );
    out.push_str("`inkland schema` prints the JSON Schema itself. All lengths are in\n");
    out.push_str("config units (`units.name`), unless a field says mm or points. Heights\n");
    out.push_str("are on the scale 0 to 100.\n\n");
    out.push_str("## Top level\n\n");
    if let Some(d) = schema.get("description").and_then(Value::as_str) {
        out.push_str(&format!("{}\n\n", one_line(d)));
    }
    property_table(&schema, &mut out);
    out.push_str("## Types\n\n");
    if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
        for (name, node) in defs {
            out.push_str(&format!("### {name}\n\n"));
            if let Some(d) = node.get("description").and_then(Value::as_str) {
                out.push_str(&format!("{}\n\n", one_line(d)));
            }
            if node.get("properties").is_some() {
                property_table(node, &mut out);
            } else if let Some(list) = node
                .get("anyOf")
                .or_else(|| node.get("oneOf"))
                .and_then(Value::as_array)
            {
                out.push_str("One of:\n\n");
                for v in list {
                    let desc = v.get("description").and_then(Value::as_str).map(one_line);
                    let shape = if v.get("properties").is_some() {
                        let keys: Vec<String> = v["properties"]
                            .as_object()
                            .map(|m| m.keys().map(|k| format!("`{k}`")).collect())
                            .unwrap_or_default();
                        format!("an object with {}", keys.join(", "))
                    } else if let Some(c) = v.get("const") {
                        format!("`{c}`")
                    } else {
                        type_of(v)
                    };
                    match desc {
                        Some(d) => out.push_str(&format!("- {shape}: {d}\n")),
                        None => out.push_str(&format!("- {shape}\n")),
                    }
                }
                out.push('\n');
            } else {
                out.push_str(&format!("Type: {}\n\n", type_of(node)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docs_config_md_is_the_text_from_the_schema() {
        let doc = include_str!("../../../docs/CONFIG.md");
        assert!(
            doc == config_markdown(),
            "docs/CONFIG.md is not the text from the schema; run `cargo run -p inkland-cli -- schema --markdown > docs/CONFIG.md`"
        );
    }

    /// Each property in the schema has a description, so a reader of the
    /// schema knows the unit of each field.
    #[test]
    fn each_property_has_a_description() {
        fn walk(v: &Value, path: &str, missing: &mut Vec<String>, seen: &mut usize) {
            match v {
                Value::Object(map) => {
                    if let Some(Value::Object(props)) = map.get("properties") {
                        for (name, prop) in props {
                            *seen += 1;
                            if prop.get("description").is_none() {
                                missing.push(format!("{path}/{name}"));
                            }
                        }
                    }
                    for (k, child) in map {
                        walk(child, &format!("{path}/{k}"), missing, seen);
                    }
                }
                Value::Array(items) => {
                    for (i, child) in items.iter().enumerate() {
                        walk(child, &format!("{path}/{i}"), missing, seen);
                    }
                }
                _ => {}
            }
        }
        let mut missing = Vec::new();
        let mut seen = 0;
        walk(&config_schema(), "", &mut missing, &mut seen);
        assert!(seen > 50, "the walk found only {seen} properties");
        assert!(
            missing.is_empty(),
            "properties without a description: {missing:?}"
        );
    }

    #[test]
    fn schema_is_the_same_on_each_call() {
        assert_eq!(
            serde_json::to_string(&config_schema()).unwrap(),
            serde_json::to_string(&config_schema()).unwrap()
        );
    }
}
