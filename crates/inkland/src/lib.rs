//! The public library API of inkland.
//!
//! Each command of the `inkland` CLI has a function here, so Rust code can do
//! all that the CLI can do.

pub use inkland_core::config::Config;
pub use inkland_core::error::{Error, ErrorCode, ExitCode, Warning, WarningCode, exit_code_of};
pub use inkland_io::azgaar::{Import, ImportSummary};
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

impl VariantOutput {
    /// `true` if a check in the report failed.
    pub fn checks_failed(&self) -> bool {
        self.report["checks"]
            .as_array()
            .is_some_and(|c| c.iter().any(|x| x["pass"] == false))
    }
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
    let mut not_yet = Vec::new();
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
    let world = inkland_core::world::World::build(config).map_err(|e| vec![e])?;
    let hash = config_hash(config);
    Ok(Output {
        world_json: inkland_io::world::world_json(&world, config),
        variants: selected
            .into_iter()
            .map(|v| {
                let rendered = inkland_render::svg::render(&world, config, v);
                let svg = rendered.svg;
                let mut report = inkland_io::report::variant_report(&world, config, &v.name, &hash);
                add_labels(&mut report, &rendered.labels);
                // The print checks run on the written SVG text.
                let checks = check_svg(&svg, config);
                if let Some(list) = report["checks"].as_array_mut() {
                    list.extend(checks);
                }
                VariantOutput {
                    name: v.name.clone(),
                    svg,
                    report,
                }
            })
            .collect(),
    })
}

/// Put the result of label placement in a report: the `labels` section, the
/// `all_labels_placed` check, and a `LABEL_UNPLACED` warning for each
/// required label without a place. Optional labels, such as the labels of
/// generated towns, are in the list but do not fail the check.
fn add_labels(report: &mut Value, labels: &inkland_render::labels::Labels) {
    let missing = labels.required_unplaced();
    report["labels"] = serde_json::json!({
        "placed": labels.placed.len(),
        "unplaced": labels.unplaced.iter().map(|u| serde_json::json!({
            "id": u.id,
            "reason": u.reason,
            "required": u.required,
        })).collect::<Vec<_>>(),
    });
    if let Some(list) = report["checks"].as_array_mut() {
        list.push(serde_json::json!({
            "name": "all_labels_placed",
            "pass": missing == 0,
            "count": missing,
        }));
    }
    if let Some(list) = report["warnings"].as_array_mut() {
        for u in labels.unplaced.iter().filter(|u| u.required) {
            let w = Warning::new(
                WarningCode::LabelUnplaced,
                "/render",
                format!("the label {:?} has no free place ({})", u.id, u.reason),
            )
            .with("id", u.id.clone());
            list.push(serde_json::to_value(&w).unwrap_or(Value::Null));
        }
    }
}

/// A PNG preview of an SVG. The longer side is `longest_px` pixels.
#[cfg(feature = "png")]
pub fn preview_png(
    svg: &str,
    longest_px: u32,
    config: &Config,
) -> Result<(Vec<u8>, Vec<Warning>), String> {
    let dirs: Vec<&std::path::Path> = config.font_dirs.iter().map(std::path::Path::new).collect();
    let families: Vec<&str> = config
        .render
        .fonts
        .families()
        .iter()
        .map(|(_, f)| *f)
        .collect();
    let p = inkland_render::png::svg_to_png(svg, longest_px, &dirs, &families)?;
    let warnings = p
        .missing_fonts
        .into_iter()
        .map(|f| {
            Warning::new(
                WarningCode::FontNotFound,
                "/render/fonts",
                format!("the font family {f:?} is not on this system or in font_dirs; the preview uses a fallback font"),
            )
            .with("font", f)
        })
        .collect();
    Ok((p.png, warnings))
}

/// Run the print checks on SVG text with the line width and the fonts of a
/// config.
pub fn check_svg(svg: &str, config: &Config) -> Vec<Value> {
    let opts = inkland_render::checks::CheckOptions {
        min_line_width_pt: config.render.min_line_width_pt,
        fonts_allowed: config.render.fonts_allowed.clone(),
    };
    inkland_render::checks::check_svg(svg, &opts)
        .iter()
        .map(|c| c.to_json())
        .collect()
}

/// Run the print checks on SVG text with explicit options.
pub fn check_svg_with(svg: &str, min_line_width_pt: f64, fonts_allowed: Vec<String>) -> Vec<Value> {
    let opts = inkland_render::checks::CheckOptions {
        min_line_width_pt,
        fonts_allowed,
    };
    inkland_render::checks::check_svg(svg, &opts)
        .iter()
        .map(|c| c.to_json())
        .collect()
}

/// Make a config from a map file of Azgaar's Fantasy Map Generator (a
/// `.map` file), and give the facts of the import.
pub fn import_map(bytes: &[u8]) -> Result<Import, Error> {
    inkland_io::azgaar::read_map(bytes)
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
    fn generate_refuses_overlays_that_it_cannot_apply_yet() {
        let c = config(json!({ "overlays": { "then": { "remove": [] } } }));
        let e = generate(&c, None).unwrap_err();
        assert_eq!(e[0].code, ErrorCode::NotImplemented);
        assert_eq!(e[0].path, "/overlays");
    }

    #[test]
    fn report_gives_each_pinned_road_and_its_check() {
        let c = config(json!({
            "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": { "steps": ["Add 35 all"] } },
            "pins": {
                "points": [
                    { "id": "ostrey", "kind": "city", "at": [60, 150] },
                    { "id": "harrowgate", "kind": "town", "at": [340, 150] }
                ],
                "roads": [
                    { "id": "kings-road", "from": "ostrey", "to": "harrowgate", "target_length": 330 },
                    { "id": "short", "from": "ostrey", "to": "harrowgate", "target_length": 282, "tolerance": 0.001 }
                ]
            }
        }));
        let out = generate(&c, None).unwrap();
        let r = &out.variants[0].report;
        let road = &r["pins"]["roads"][0];
        assert_eq!(road["within_tolerance"], true);
        assert_eq!(road["stops"][0]["place"], "ostrey");
        assert_eq!(road["legs"][0]["length"], road["actual"]);
        // The second road cannot be as short as 282 +- 0.1 on a grid.
        assert_eq!(r["pins"]["roads"][1]["within_tolerance"], false);
        assert_eq!(r["checks"][0]["name"], "pinned_road_length");
        assert_eq!(r["checks"][0]["pass"], false);
        assert!(out.variants[0].checks_failed());
    }

    #[test]
    fn report_lists_the_labels_and_fails_for_a_required_label_without_a_place() {
        let pins = json!({ "regions": [
            { "id": "hills", "kind": "hills", "name": "The Hills", "box": [30, 20, 70, 60] }
        ]});
        let ok = generate(
            &config(json!({ "world": { "cells": 1500 }, "pins": pins })),
            None,
        )
        .unwrap();
        let r = &ok.variants[0].report;
        assert!(r["labels"]["placed"].as_u64().unwrap() > 0);
        let check = r["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "all_labels_placed")
            .unwrap();
        assert_eq!(check["pass"], true);
        assert!(
            ok.variants[0]
                .svg
                .contains("<text id=\"label-region-hills\"")
        );

        let big = config(json!({ "world": { "cells": 1500 }, "pins": pins,
            "render": { "font_sizes_pt": { "region": 400 } } }));
        let out = generate(&big, None).unwrap();
        let r = &out.variants[0].report;
        let unplaced = &r["labels"]["unplaced"];
        assert!(
            unplaced
                .as_array()
                .unwrap()
                .iter()
                .any(|u| u["id"] == "label-region-hills"
                    && u["reason"] == "outside"
                    && u["required"] == true)
        );
        assert!(
            r["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["code"] == "LABEL_UNPLACED")
        );
        assert!(out.variants[0].checks_failed());
        assert!(!out.variants[0].svg.contains("label-region-hills"));
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
    fn report_gives_each_pin_with_error_zero() {
        let c = config(json!({
            "world": { "cells": 1000 },
            "pins": {
                "points": [{ "id": "ostrey", "kind": "city", "at": [41.25, 33.5] }],
                "regions": [{ "id": "sea", "kind": "sea", "box": [0, 0, 10, 80] }]
            }
        }));
        let out = generate(&c, None).unwrap();
        let pins = &out.variants[0].report["pins"];
        assert_eq!(pins["points"][0]["id"], "ostrey");
        assert_eq!(pins["points"][0]["placed"], json!([41.25, 33.5]));
        assert_eq!(pins["points"][0]["error"], 0.0);
        assert_eq!(pins["points"][0]["land"], true);
        assert_eq!(pins["regions"][0]["in_range"], true);
        assert!(pins["regions"][0]["cells"].as_u64().unwrap() > 0);
    }

    #[test]
    fn report_gives_each_pinned_river() {
        let c = config(json!({
            "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea" },
            "pins": {
                "points": [{ "id": "ford", "kind": "village", "at": [150, 150] }],
                "regions": [
                    { "id": "sea", "kind": "sea", "box": [0, 0, 20, 300] },
                    { "id": "peaks", "kind": "mountains", "box": [260, 100, 320, 200] }
                ],
                "rivers": [{ "id": "brann", "name": "Brann", "source": { "region": "peaks" },
                             "through": [{ "place": "ford" }], "mouth": [10, 150] }]
            }
        }));
        let out = generate(&c, None).unwrap();
        let r = &out.variants[0].report["pins"]["rivers"][0];
        assert_eq!(r["id"], "brann");
        assert_eq!(r["through_ok"], true);
        assert_eq!(r["max_deviation"], 0.0);
        assert_eq!(r["downhill"], true);
        assert_eq!(r["mouth"]["at"], json!([10.0, 150.0]));
        assert!(r["length"].as_f64().unwrap() > 140.0);
        assert!(out.variants[0].svg.contains("id=\"river-brann\""));
    }

    #[test]
    fn report_gives_the_state_of_each_pinned_place() {
        let c = config(json!({
            "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea", "states": 3 },
            "pins": { "points": [
                { "id": "kingsholm", "kind": "city", "at": [150, 120], "name": "Kingsholm", "capital_of": "Kingdom of Holm" }
            ]}
        }));
        let out = generate(&c, None).unwrap();
        let r = &out.variants[0].report;
        let p = &r["pins"]["points"][0];
        assert_eq!(p["name"], "Kingsholm");
        assert_eq!(p["state"], "Kingdom of Holm");
        assert!(p["province"].is_string());
        assert!(p["culture"].is_string());
        let states = r["world"]["states"].as_array().unwrap();
        assert_eq!(states.len(), 3);
        assert!(
            states
                .iter()
                .any(|s| s["name"] == "Kingdom of Holm" && s["capital"] == "Kingsholm")
        );
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
