//! The CLI contract: JSON on stdout, stable error codes, exit codes, and a
//! dry run that writes nothing.

use assert_cmd::Command;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn inkland() -> Command {
    Command::cargo_bin("inkland").unwrap()
}

/// Run the tool. Return the exit code and the JSON on stdout.
fn run(args: &[&str]) -> (i32, Value) {
    let out = inkland().args(args).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {stdout}"));
    (out.status.code().unwrap(), json)
}

fn write_config(dir: &Path, config: &Value) -> PathBuf {
    let path = dir.join("map.json");
    std::fs::write(&path, serde_json::to_string(config).unwrap()).unwrap();
    path
}

fn minimal() -> Value {
    json!({ "version": 1, "seed": 3, "frame": { "width": 100, "height": 80 } })
}

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/configs");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    files
}

#[test]
fn schema_is_json_and_validates_each_fixture() {
    let (code, schema) = run(&["schema"]);
    assert_eq!(code, 0);
    let validator = jsonschema::validator_for(&schema).unwrap();
    for file in fixtures() {
        let config: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&config)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{}: {errors:?}", file.display());
    }
}

#[test]
fn schema_refuses_a_config_with_an_unknown_field() {
    let (_, schema) = run(&["schema"]);
    let validator = jsonschema::validator_for(&schema).unwrap();
    let mut bad = minimal();
    bad["colour"] = json!("red");
    assert!(!validator.is_valid(&bad));
}

#[test]
fn each_fixture_passes_the_dry_run() {
    for file in fixtures() {
        let (code, report) = run(&["generate", "--config", file.to_str().unwrap(), "--dry-run"]);
        assert_eq!(code, 0, "{}: {report}", file.display());
        assert_eq!(report["dry_run"], true);
        assert!(
            report["config_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
    }
}

#[test]
fn dry_run_writes_no_files() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path(), &minimal());
    let out_dir = dir.path().join("out");
    let preview = dir.path().join("preview.png");
    let (code, _) = run(&[
        "generate",
        "--config",
        config.to_str().unwrap(),
        "--out-dir",
        out_dir.to_str().unwrap(),
        "--preview",
        preview.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(code, 0);
    let names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["map.json".to_owned()]);
}

#[test]
fn flags_change_the_effective_config() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path(), &minimal());
    let (code, report) = run(&[
        "generate",
        "--config",
        config.to_str().unwrap(),
        "--dry-run",
        "--seed",
        "77",
        "--cells",
        "5000",
        "--set",
        "/units/name=league",
    ]);
    assert_eq!(code, 0);
    assert_eq!(report["seed"], 77);
    assert_eq!(report["effective_config"]["world"]["cells"], 5000);
    assert_eq!(report["units"], "league");
}

#[test]
fn invalid_json_is_exit_2_with_config_parse() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("map.json");
    std::fs::write(&path, "{ not json").unwrap();
    let (code, out) = run(&["generate", "--config", path.to_str().unwrap(), "--dry-run"]);
    assert_eq!(code, 2);
    assert_eq!(out["error"]["code"], "CONFIG_PARSE");
}

#[test]
fn schema_error_is_exit_2_with_a_pointer() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = minimal();
    c["world"] = json!({ "template": "flat-earth" });
    let config = write_config(dir.path(), &c);
    let (code, out) = run(&[
        "generate",
        "--config",
        config.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(code, 2);
    assert_eq!(out["error"]["code"], "CONFIG_SCHEMA");
    assert_eq!(out["error"]["path"], "/world/template");
}

#[test]
fn infeasible_pin_is_exit_3() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = minimal();
    c["pins"] = json!({
        "points": [{ "id": "a", "kind": "city", "at": [5, 40] }],
        "regions": [{ "id": "sea", "kind": "sea", "box": [0, 0, 10, 80] }]
    });
    let config = write_config(dir.path(), &c);
    let (code, out) = run(&[
        "generate",
        "--config",
        config.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(code, 3);
    assert_eq!(out["error"]["code"], "PIN_IN_FORCED_SEA");
    assert_eq!(out["error"]["path"], "/pins/points/0/at");
    assert_eq!(out["errors"].as_array().unwrap().len(), 1);
}

#[test]
fn config_error_wins_over_infeasible_pin() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = minimal();
    c["pins"] = json!({ "points": [
        { "id": "a", "kind": "city", "at": [500, 40] },
        { "id": "a", "kind": "city", "at": [5, 40] }
    ]});
    let config = write_config(dir.path(), &c);
    let (code, out) = run(&[
        "generate",
        "--config",
        config.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(code, 2);
    assert_eq!(out["errors"].as_array().unwrap().len(), 2);
}

#[test]
fn missing_config_file_is_exit_4() {
    let (code, out) = run(&["generate", "--config", "no-such-file.json", "--dry-run"]);
    assert_eq!(code, 4);
    assert_eq!(out["error"]["code"], "IO_READ");
}

#[test]
fn unknown_flag_is_exit_2_with_cli_usage() {
    let (code, out) = run(&["generate", "--colour"]);
    assert_eq!(code, 2);
    assert_eq!(out["error"]["code"], "CLI_USAGE");
}

#[test]
fn unknown_variant_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path(), &minimal());
    let (code, out) = run(&[
        "generate",
        "--config",
        config.to_str().unwrap(),
        "--dry-run",
        "--variant",
        "act-9",
    ]);
    assert_eq!(code, 2);
    assert_eq!(out["error"]["code"], "REF_UNKNOWN");
}

#[test]
fn version_is_json_with_the_notice() {
    let (code, out) = run(&["version", "--json"]);
    assert_eq!(code, 0);
    assert_eq!(out["name"], "inkland");
    assert_eq!(out["third_party"][0]["license"], "MIT");
}

#[test]
fn stderr_is_empty_without_verbose() {
    let dir = tempfile::tempdir().unwrap();
    let config = write_config(dir.path(), &minimal());
    let out = inkland()
        .args([
            "generate",
            "--config",
            config.to_str().unwrap(),
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(out.stderr.is_empty());
    let verbose = inkland()
        .args([
            "--verbose",
            "generate",
            "--config",
            config.to_str().unwrap(),
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(!verbose.stderr.is_empty());
}

#[test]
fn same_config_gives_the_same_bytes() {
    let file = &fixtures()[0];
    let a = inkland()
        .args(["generate", "--config", file.to_str().unwrap(), "--dry-run"])
        .output()
        .unwrap();
    let b = inkland()
        .args(["generate", "--config", file.to_str().unwrap(), "--dry-run"])
        .output()
        .unwrap();
    assert_eq!(a.stdout, b.stdout);
}
