//! Each map of Azgaar's Fantasy Map Generator in `fixtures/azgaar` imports
//! to a valid config, and the config generates. The maps are stored with
//! gzip, which the import reads.

use inkland_core::config::Template;
use std::path::{Path, PathBuf};

fn maps() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/azgaar");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".map.gz"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "fixtures/azgaar has no map files");
    files
}

fn import(file: &Path) -> inkland::Import {
    let bytes = std::fs::read(file).unwrap();
    inkland::import_map(&bytes).unwrap_or_else(|e| panic!("{}: {e:?}", file.display()))
}

#[test]
fn each_map_imports_to_a_valid_config() {
    for file in maps() {
        let name = file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .trim_end_matches(".map.gz")
            .to_owned();
        let i = import(&file);
        let c = &i.config;
        // The fixture files have the version as their name.
        assert_eq!(i.summary.version, name);
        let Template::Raster { raster } = &c.world.template else {
            panic!("{name}: the template is not a raster");
        };
        assert_eq!(
            raster.values.len(),
            (raster.cols * raster.rows) as usize,
            "{name}"
        );
        assert!(c.pins.points.len() > 100, "{name}: too few places");
        assert_eq!(c.pins.points.len(), i.summary.places, "{name}");
        assert!(c.pins.regions.is_empty() && c.pins.rivers.is_empty());
        let errors = inkland::validate(c);
        assert!(
            errors.is_empty(),
            "{name}: {:?}",
            &errors[..errors.len().min(3)]
        );
        // The same bytes give the same config.
        assert_eq!(import(&file).config, *c, "{name}");
    }
}

#[test]
fn each_map_generates() {
    for file in maps() {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let c = import(&file).config;
        let out = inkland::generate(&c, None)
            .unwrap_or_else(|e| panic!("{name}: {:?}", &e[..e.len().min(3)]));
        assert_eq!(out.variants.len(), 1, "{name}");
        // Each place of the map is in the report, exactly where it was.
        let points = out.variants[0].report["pins"]["points"].as_array().unwrap();
        assert_eq!(points.len(), c.pins.points.len(), "{name}");
        assert!(
            points
                .iter()
                .all(|p| p["error"] == 0.0 && p["land"] == true),
            "{name}"
        );
    }
}
