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

/// The closing test of milestone 13: the land and the sea of the world agree
/// with the map. Each cell is land when the map height at its site is at or
/// above sea level. Only the land that the places force can differ.
#[test]
fn land_and_sea_agree_with_the_map() {
    for file in maps() {
        let name = file.file_stem().unwrap().to_string_lossy().into_owned();
        let mut c = import(&file).config;
        for p in &mut c.pins.points {
            p.port = false;
        }
        let Template::Raster { raster } = c.world.template.clone() else {
            panic!("{name}: the template is not a raster");
        };
        let w = inkland_core::world::World::build(&c).unwrap();
        let sea_level = f64::from(c.world.sea_level);
        let (mut differ, mut forced) = (0usize, 0usize);
        // The raster gives the heights of the first grid.
        let g = &w.base.grid;
        let n = g.len();
        let mut near_place = vec![false; n];
        for &cell in &w.pin_cells {
            let cell = w.base_cell[cell];
            near_place[cell] = true;
            for m in g.neighbors(cell) {
                near_place[m] = true;
            }
        }
        for cell in 0..n {
            let [x, y] = g.points[cell];
            let map_land = f64::from(raster.sample(x, y, w.width, w.height)) >= sea_level;
            if map_land != w.base.features.is_land(cell) {
                if near_place[cell] {
                    forced += 1;
                } else {
                    differ += 1;
                }
            }
        }
        assert_eq!(
            differ, 0,
            "{name}: {differ} cells differ away from the places ({forced} near them)"
        );
    }
}
