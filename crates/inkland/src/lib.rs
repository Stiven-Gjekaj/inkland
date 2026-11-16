//! The public library API of inkland.
//!
//! Each command of the `inkland` CLI has a function here, so Rust code can do
//! all that the CLI can do.

pub use inkland_core::config::Config;
pub use inkland_core::error::{Error, ErrorCode, ExitCode, Warning, WarningCode, exit_code_of};
pub use inkland_io::config::{Override, config_hash, parse_config};
pub use inkland_io::report::DryRunReport;

use inkland_io::report::{Check, PinCounts, Tool};
use serde_json::{Value, json};

/// The JSON Schema of the config.
pub fn schema() -> Value {
    inkland_io::schema::config_schema()
}

/// All config rules and pin feasibility checks that need no terrain.
pub fn validate(config: &Config) -> Vec<Error> {
    inkland_constraints::validate(config)
}

/// Check a config and give the dry-run report. Nothing is generated and
/// nothing is written.
pub fn dry_run(config: &Config) -> Result<DryRunReport, Vec<Error>> {
    let errors = validate(config);
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(DryRunReport {
        tool: Tool::current(),
        config_hash: config_hash(config),
        seed: config.seed,
        units: config.units.name.clone(),
        dry_run: true,
        variants: config.variants.iter().map(|v| v.name.clone()).collect(),
        pins: PinCounts {
            points: config.pins.points.len(),
            regions: config.pins.regions.len(),
            rivers: config.pins.rivers.len(),
            roads: config.pins.roads.len(),
        },
        checks: inkland_constraints::CHECKS
            .iter()
            .map(|&name| Check { name, pass: true })
            .collect(),
        warnings: Vec::new(),
        effective_config: config.clone(),
    })
}

/// The output of one variant.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantOutput {
    pub name: String,
    /// The text of `<name>.svg`.
    pub svg: String,
    /// The report of the variant.
    pub report: Value,
}

/// The output of `generate`.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    /// The text of `world.json`. All variants share it.
    pub world_json: String,
    pub variants: Vec<VariantOutput>,
}

/// Generate the maps of a config: all variants, or only the variant `only`.
pub fn generate(config: &Config, only: Option<&str>) -> Result<Output, Vec<Error>> {
    let errors = validate(config);
    if !errors.is_empty() {
        return Err(errors);
    }
    let later = |path: &str, what: &str, milestone: u32| {
        Error::new(
            ErrorCode::NotImplemented,
            path,
            format!("{what} come in milestone {milestone}; this version cannot apply them"),
        )
    };
    let p = &config.pins;
    let mut not_yet = Vec::new();
    if !(p.points.is_empty() && p.regions.is_empty()) {
        not_yet.push(later("/pins", "pinned points and regions", 3));
    }
    if !p.rivers.is_empty() {
        not_yet.push(later("/pins/rivers", "pinned rivers", 4));
    }
    if !p.roads.is_empty() {
        not_yet.push(later("/pins/roads", "pinned roads", 6));
    }
    if !config.overlays.is_empty() {
        not_yet.push(later("/overlays", "overlays", 9));
    }
    if !not_yet.is_empty() {
        return Err(not_yet);
    }
    let selected: Vec<&inkland_core::config::Variant> = config
        .variants
        .iter()
        .filter(|v| only.is_none_or(|name| v.name == name))
        .collect();
    if selected.is_empty() {
        return Err(vec![Error::new(
            ErrorCode::RefUnknown,
            "/variants",
            format!("there is no variant {:?}", only.unwrap_or_default()),
        )]);
    }
    let world = inkland_core::world::World::build(config)
        .map_err(|e| vec![Error::new(ErrorCode::Internal, "", e)])?;
    let hash = config_hash(config);
    Ok(Output {
        world_json: inkland_io::world::world_json(&world, config, &hash),
        variants: selected
            .into_iter()
            .map(|v| VariantOutput {
                name: v.name.clone(),
                svg: inkland_render::svg::render(&world, config, v),
                report: inkland_io::report::variant_report(&world, config, &v.name, &hash),
            })
            .collect(),
    })
}

/// A PNG preview of an SVG. The longer side is `longest_px` pixels.
#[cfg(feature = "png")]
pub fn preview_png(svg: &str, longest_px: u32) -> Result<Vec<u8>, String> {
    inkland_render::png::svg_to_png(svg, longest_px)
}

/// The version of the tool and the notices of third-party work.
pub fn version_info() -> Value {
    json!({
        "name": "inkland",
        "version": env!("CARGO_PKG_VERSION"),
        "config_version": inkland_core::config::CONFIG_VERSION,
        "license": "MIT OR Apache-2.0",
        "third_party": [{
            "name": "Azgaar's Fantasy Map Generator",
            "url": "https://github.com/Azgaar/Fantasy-Map-Generator",
            "license": "MIT",
            "copyright": "Copyright 2017-2024 Max Haniyeu (Azgaar)",
            "use": "inkland re-implements its algorithms"
        }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(extra: Value) -> Config {
        let mut v = json!({ "version": 1, "seed": 5, "frame": { "width": 100, "height": 80 } });
        for (k, val) in extra.as_object().unwrap() {
            v[k] = val.clone();
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn dry_run_of_a_valid_config_passes_each_check() {
        let report = dry_run(&config(json!({}))).unwrap();
        assert!(report.dry_run);
        assert_eq!(report.seed, 5);
        assert_eq!(report.checks.len(), inkland_constraints::CHECKS.len());
        assert!(report.checks.iter().all(|c| c.pass));
        assert_eq!(report.variants, vec!["default".to_owned()]);
    }

    #[test]
    fn dry_run_of_an_infeasible_config_gives_the_errors() {
        let c = config(
            json!({ "pins": { "points": [{ "id": "a", "kind": "town", "at": [500, 1] }] } }),
        );
        let errors = dry_run(&c).unwrap_err();
        assert_eq!(errors[0].code, ErrorCode::PinOutsideFrame);
        assert_eq!(exit_code_of(&errors), ExitCode::Infeasible);
    }

    #[test]
    fn generate_validates_first() {
        let c = config(
            json!({ "pins": { "points": [{ "id": "a", "kind": "town", "at": [500, 1] }] } }),
        );
        assert_eq!(
            generate(&c, None).unwrap_err()[0].code,
            ErrorCode::PinOutsideFrame
        );
    }

    #[test]
    fn generate_refuses_pins_that_it_cannot_apply_yet() {
        let c =
            config(json!({ "pins": { "points": [{ "id": "a", "kind": "town", "at": [5, 5] }] } }));
        let e = generate(&c, None).unwrap_err();
        assert_eq!(e[0].code, ErrorCode::NotImplemented);
        assert_eq!(e[0].path, "/pins");
    }

    #[test]
    fn generate_makes_one_svg_for_each_variant() {
        let c = config(json!({ "world": { "cells": 1000 }, "variants": [
            { "name": "full", "page": { "w_mm": 167, "h_mm": 250 } },
            { "name": "half", "crop": [0, 0, 50, 40], "page": { "w_mm": 167, "h_mm": 120 } }
        ]}));
        let out = generate(&c, None).unwrap();
        let names: Vec<&str> = out.variants.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["full", "half"]);
        assert!(out.variants[1].svg.contains("height=\"120mm\""));
        assert_eq!(out.variants[0].report["variant"], "full");
        let one = generate(&c, Some("half")).unwrap();
        assert_eq!(one.variants.len(), 1);
        assert_eq!(one.world_json, out.world_json);
    }

    #[test]
    fn version_names_the_original_program() {
        let v = version_info();
        assert_eq!(v["third_party"][0]["license"], "MIT");
        assert!(
            v["third_party"][0]["copyright"]
                .as_str()
                .unwrap()
                .contains("Azgaar")
        );
    }
}
