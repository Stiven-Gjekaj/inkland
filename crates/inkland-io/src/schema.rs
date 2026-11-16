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

#[cfg(test)]
mod tests {
    use super::*;

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
