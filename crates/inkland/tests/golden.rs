//! Golden hashes: each fixture in `fixtures/golden` gives the same bytes on
//! each system and on each run.
//!
//! To accept new output on purpose, run
//! `INKLAND_BLESS=1 cargo test -p inkland --test golden`
//! and commit `tests/golden/hashes.json` with the change that made it.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The output with the package version in place of the build version, so
/// that the hashes do not change with each commit.
fn stable(text: &str) -> String {
    text.replace(
        &format!("\"{}\"", inkland::VERSION),
        &format!("\"{}\"", env!("CARGO_PKG_VERSION")),
    )
}

fn actual() -> BTreeMap<String, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/golden"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(files.len() >= 3, "too few golden fixtures");
    let mut hashes = BTreeMap::new();
    for file in files {
        let stem = file.file_stem().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&file).unwrap();
        let config = inkland::parse_config(&text, &[]).unwrap();
        let out = inkland::generate(&config, None).unwrap_or_else(|e| panic!("{stem}: {:?}", e[0]));
        hashes.insert(
            format!("{stem}/world.json"),
            sha(stable(&out.world_json).as_bytes()),
        );
        for v in &out.variants {
            hashes.insert(format!("{stem}/{}.svg", v.name), sha(v.svg.as_bytes()));
            let report = stable(&serde_json::to_string_pretty(&v.report).unwrap());
            hashes.insert(
                format!("{stem}/{}.report.json", v.name),
                sha(report.as_bytes()),
            );
        }
    }
    hashes
}

#[test]
fn outputs_match_the_golden_hashes() {
    let path = root().join("tests/golden/hashes.json");
    let actual = actual();
    if std::env::var_os("INKLAND_BLESS").is_some() {
        let text = serde_json::to_string_pretty(&actual).unwrap() + "\n";
        std::fs::write(&path, text).unwrap();
        return;
    }
    let text = std::fs::read_to_string(&path)
        .expect("tests/golden/hashes.json is missing; run with INKLAND_BLESS=1");
    let expected: BTreeMap<String, String> = serde_json::from_str(&text).unwrap();
    let mut diffs = Vec::new();
    for key in expected
        .keys()
        .chain(actual.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        if expected.get(key) != actual.get(key) {
            diffs.push(key.clone());
        }
    }
    assert!(diffs.is_empty(), "outputs that changed: {diffs:?}");
}
