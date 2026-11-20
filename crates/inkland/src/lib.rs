//! The public library API of inkland.
//!
//! Each command of the `inkland` CLI has a function here, so Rust code can do
//! all that the CLI can do.

pub use inkland_core::config::Config;
pub use inkland_core::error::{Error, ErrorCode, ExitCode, Warning, WarningCode, exit_code_of};
pub use inkland_core::world::World;
pub use inkland_edit::{Command, History, PinList, Script};
pub use inkland_io::azgaar::{Import, ImportSummary};
pub use inkland_io::config::{Override, config_hash, parse_config};
pub use inkland_io::heights::load_template_image;
pub use inkland_io::report::DryRunReport;

use inkland_io::report::{Check, PinCounts, Tool};
use serde_json::{Value, json};

/// Read the text of a `world.json`: its config and the world that the
/// config makes.
/// The version of this build. See `inkland_io::VERSION`.
pub use inkland_io::VERSION;

pub fn read_world(text: &str) -> Result<(Config, inkland_core::world::World), Error> {
    inkland_io::world::read_world(text)
}

/// The facts about the point `at` (`x,y` in config units) of a world file.
pub fn query(world_text: &str, at: &str) -> Result<Value, Error> {
    let p = inkland_io::query::parse_point(at)?;
    let (config, world) = read_world(world_text)?;
    inkland_io::query::query(&world, &config, p)
}

/// The facts about the point `p` (config units) of a world in memory. The
/// desktop app uses this for its status bar.
pub fn query_at(world: &World, config: &Config, p: [f64; 2]) -> Result<Value, Error> {
    inkland_io::query::query(world, config, p)
}

/// The item whose coat of arms to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmblemOf {
    /// A state: its id, or its name.
    State(String),
    /// A province: its id, or its name.
    Province(String),
    /// A place: the id of its pin, its id, or its name.
    Place(String),
}

/// A drawn coat of arms.
#[derive(Debug, Clone, PartialEq)]
pub struct EmblemSvg {
    /// The SVG file.
    pub svg: String,
    /// The coat of arms, in the JSON shape of FMG.
    pub coa: inkland_core::emblems::Coa,
    /// `state-3`, `province-12`, or `burg-7`.
    pub id: String,
    pub name: String,
}

/// Draw the coat of arms of one state, province, or place of a world as an
/// SVG file of `size_mm` x `size_mm`. `hatching`: black and white with the
/// hatching of Petra Sancta, each line at least
/// `render.min_line_width_pt` of the config wide. The error is
/// `REF_UNKNOWN` when the world has no such item.
pub fn emblem(
    world: &World,
    config: &Config,
    of: &EmblemOf,
    hatching: bool,
    size_mm: f64,
) -> Result<EmblemSvg, Error> {
    let soc = &world.society;
    let matches =
        |key: &str, id: u32, names: &[&str]| key == id.to_string() || names.contains(&key);
    let found = match of {
        EmblemOf::State(k) => soc
            .states
            .iter()
            .find(|x| matches(k, x.id, &[&x.name, &x.full_name]))
            .map(|x| (format!("state-{}", x.id), x.name.clone(), x.coa.clone())),
        EmblemOf::Province(k) => soc
            .provinces
            .iter()
            .find(|x| matches(k, x.id, &[&x.name, &x.full_name]))
            .map(|x| (format!("province-{}", x.id), x.name.clone(), x.coa.clone())),
        EmblemOf::Place(k) => soc
            .places
            .iter()
            .find(|x| x.pin.as_deref() == Some(k.as_str()))
            .or_else(|| soc.places.iter().find(|x| matches(k, x.id, &[&x.name])))
            .map(|x| (format!("burg-{}", x.id), x.name.clone(), x.coa.clone())),
    };
    let Some((id, name, Some(coa))) = found else {
        let (what, key) = match of {
            EmblemOf::State(k) => ("state", k),
            EmblemOf::Province(k) => ("province", k),
            EmblemOf::Place(k) => ("place", k),
        };
        return Err(Error::new(
            ErrorCode::RefUnknown,
            "",
            format!("the world has no {what} {key:?}"),
        ));
    };
    let prefix = format!("emblem-{id}");
    let colour = inkland_render::emblem::Look::COLOUR;
    let look = if hatching {
        let view = inkland_render::emblem::draw(&coa, &prefix, &colour).view;
        let line = inkland_render::page::line_mm(config.render.min_line_width_pt);
        inkland_render::emblem::Look {
            hatching: true,
            min_line: line * view[2] / size_mm,
        }
    } else {
        colour
    };
    let svg = inkland_render::emblem::document(&coa, &prefix, &look, size_mm);
    Ok(EmblemSvg { svg, coa, id, name })
}

/// The GeoJSON export of a world file. `format` must be `geojson`. `layers`
/// is a list `a,b,c` of layers, or `None` for all layers.
pub fn export(world_text: &str, format: &str, layers: Option<&str>) -> Result<Value, Error> {
    if format != "geojson" {
        return Err(Error::new(
            ErrorCode::CliUsage,
            "",
            format!("{format:?} is not an export format; use geojson"),
        ));
    }
    let layers = inkland_io::geojson::parse_layers(layers)?;
    let (config, world) = read_world(world_text)?;
    Ok(inkland_io::geojson::export(&world, &config, &layers))
}

/// Apply an edit script to a config. The result is the new config and the
/// script that undoes the edit. The new config must pass `validate`.
pub fn edit_config(config: &Config, script_text: &str) -> Result<(Config, Script), Vec<Error>> {
    let script = Script::parse(script_text).map_err(|e| vec![e])?;
    apply_script(config, &script)
}

fn apply_script(config: &Config, script: &Script) -> Result<(Config, Script), Vec<Error>> {
    let (next, undo) = script.apply(config).map_err(|e| vec![e])?;
    let errors = validate(&next);
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok((next, undo))
}

/// The result of an edit of a world file.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldEdit {
    /// The text of the new `world.json`.
    pub world_json: String,
    /// The script that undoes the edit.
    pub undo: Script,
    /// The number of commands applied.
    pub applied: usize,
}

/// The errors of an edit script for a world file: one `EDIT_OUTSIDE_WORLD`
/// for each `set` or `unset` of a value that a `world.json` does not keep.
/// Such a script applies only to a config file.
pub fn world_script_errors(script: &Script) -> Vec<Error> {
    script
        .commands
        .iter()
        .enumerate()
        .filter_map(|(i, cmd)| {
            let path = match cmd {
                Command::Set { path, .. } | Command::Unset { path } => path,
                _ => return None,
            };
            if inkland_io::config::in_world_part(path) {
                return None;
            }
            Some(
                Error::new(
                    ErrorCode::EditOutsideWorld,
                    path.clone(),
                    format!(
                        "command {i} edits {path}, but world.json keeps only the part of the \
                         config that makes the world; apply the script to the config file"
                    ),
                )
                .with("command", i),
            )
        })
        .collect()
}

/// Apply an edit script to a world file: edit its config and make the world
/// again. A script that edits a value that the world file does not keep is
/// refused (see `world_script_errors`).
pub fn edit_world(world_text: &str, script_text: &str) -> Result<WorldEdit, Vec<Error>> {
    let (config, _) = read_world(world_text).map_err(|e| vec![e])?;
    let script = Script::parse(script_text).map_err(|e| vec![e])?;
    let errors = world_script_errors(&script);
    if !errors.is_empty() {
        return Err(errors);
    }
    let (next, undo) = apply_script(&config, &script)?;
    let world = inkland_core::world::World::build(&next).map_err(|e| vec![e])?;
    Ok(WorldEdit {
        world_json: inkland_io::world::world_json(&world, &next),
        applied: undo.commands.len(),
        undo,
    })
}

/// The JSON Schema of the config.
pub fn schema() -> Value {
    inkland_io::schema::config_schema()
}

/// The config reference in Markdown, made from the schema.
pub fn schema_markdown() -> String {
    inkland_io::schema::config_markdown()
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
    generate_with_world(config, only).map(|(_, out)| out)
}

/// Generate the maps of a config, as `generate` does, and also give the
/// world in memory. The desktop app queries this world.
pub fn generate_with_world(
    config: &Config,
    only: Option<&str>,
) -> Result<(World, Output), Vec<Error>> {
    let errors = validate(config);
    if !errors.is_empty() {
        return Err(errors);
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
    let mut variants = Vec::new();
    for v in selected {
        // A variant with an overlay renders a copy of the world with the
        // overlay applied. The shared world does not change.
        let shown = match &v.overlay {
            Some(name) => {
                Some(inkland_core::overlay::apply(&world, config, name).map_err(|e| vec![e])?)
            }
            None => None,
        };
        let w = shown.as_ref().unwrap_or(&world);
        let rendered = inkland_render::svg::render(w, config, v);
        let svg = rendered.svg;
        let mut report = inkland_io::report::variant_report(&world, config, &v.name, &hash);
        if let Some(name) = &v.overlay {
            let o = &config.overlays[name];
            let section = inkland_io::report::overlay_section(w, config, o, name);
            let bad = section["roads"].as_array().map_or(0, |r| {
                r.iter().filter(|x| x["within_tolerance"] == false).count()
            });
            report["overlay"] = section;
            if let Some(list) = report["checks"].as_array_mut()
                && o.add.roads.iter().any(|r| r.target_length.is_some())
            {
                list.push(json!({ "name": "overlay_road_length", "pass": bad == 0, "count": bad }));
            }
        }
        add_labels(&mut report, &rendered.labels, config);
        let relief = inkland_render::relief_report::summary(w, config, v);
        report["relief"] =
            json!({ "shown": relief.shown, "hidden": relief.hidden, "pins": relief.pins });
        // The print checks run on the written SVG text.
        let checks = check_svg(&svg, config);
        if let Some(list) = report["checks"].as_array_mut() {
            list.extend(checks);
        }
        variants.push(VariantOutput {
            name: v.name.clone(),
            svg,
            report,
        });
    }
    let output = Output {
        world_json: inkland_io::world::world_json(&world, config),
        variants,
    };
    Ok((world, output))
}

/// Put the result of label placement in a report: the `labels` section, the
/// `all_labels_placed` check, and a `LABEL_UNPLACED` warning for each
/// required label without a place. Optional labels, such as the labels of
/// generated towns, are in the list but do not fail the check.
fn add_labels(report: &mut Value, labels: &inkland_render::labels::Labels, config: &Config) {
    let missing = labels.required_unplaced();
    report["labels"] = serde_json::json!({
        "placed": labels.placed.len(),
        "unplaced": labels.unplaced.iter().map(|u| serde_json::json!({
            "id": u.id,
            "reason": u.reason,
            "required": u.required,
        })).collect::<Vec<_>>(),
    });
    // The river labels that stand level, as no part of the river is smooth
    // and long enough, or as the config gives them a position.
    let level: Vec<&str> = labels
        .placed
        .iter()
        .filter(|l| {
            l.kind == inkland_render::labels::Kind::River
                && matches!(l.geometry, inkland_render::labels::Geometry::Point { .. })
        })
        .map(|l| l.id.as_str())
        .collect();
    report["labels"]["level_rivers"] = serde_json::json!(level);
    // The path labels whose text does not fit its path.
    let misfits: Vec<Value> = labels
        .placed
        .iter()
        .filter_map(|l| {
            inkland_render::labels::path_fit_fault(l)
                .map(|why| serde_json::json!({ "id": l.id, "reason": why }))
        })
        .collect();
    if let Some(list) = report["checks"].as_array_mut() {
        list.push(serde_json::json!({
            "name": "path_labels_fit",
            "pass": misfits.is_empty(),
            "count": misfits.len(),
            "labels": misfits,
        }));
        list.push(serde_json::json!({
            "name": "all_labels_placed",
            "pass": missing == 0,
            "count": missing,
        }));
        // The labels that show a name that the config does not write.
        if config.render.labels.check_generated {
            let written = config.written_names();
            let ids: Vec<&str> = labels
                .placed
                .iter()
                .filter(|l| !written.contains(&l.text.trim().to_lowercase()))
                .map(|l| l.id.as_str())
                .collect();
            list.push(serde_json::json!({
                "name": "no_generated_labels",
                "pass": ids.is_empty(),
                "count": ids.len(),
                "ids": ids,
            }));
        }
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
    // The families of the config and the families that the SVG names: a
    // style of `colour` mode has its own fonts.
    let mut names: Vec<String> = config
        .render
        .fonts
        .families()
        .iter()
        .map(|(_, f)| (*f).to_owned())
        .collect();
    names.extend(inkland_render::png::families_in(svg));
    names.sort();
    names.dedup();
    let families: Vec<&str> = names.iter().map(String::as_str).collect();
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

/// The print checks that test black and white output. They do not apply
/// to `colour` mode, which has colours, effects, thin lines, and the fonts
/// of its style.
pub const PRINT_ONLY_CHECKS: [&str; 6] = [
    "no_opacity",
    "no_masks_filters_rgba",
    "black_and_white",
    "min_line_width",
    "no_text_stroke",
    "font_families_allowed",
];

/// Run the print checks on SVG text with the line width and the fonts of a
/// config. In `colour` mode, a print-only check gives `"applicable":
/// false` and passes.
pub fn check_svg(svg: &str, config: &Config) -> Vec<Value> {
    let opts = inkland_render::checks::CheckOptions {
        min_line_width_pt: config.render.min_line_width_pt,
        fonts_allowed: config.render.fonts_allowed.clone(),
    };
    let colour = config.render.mode == inkland_core::config::RenderMode::Colour;
    inkland_render::checks::check_svg(svg, &opts)
        .iter()
        .map(|c| {
            if colour && PRINT_ONLY_CHECKS.contains(&c.name) {
                json!({ "name": c.name, "pass": true, "applicable": false })
            } else {
                c.to_json()
            }
        })
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
        "version": inkland_io::VERSION,
        "config_version": inkland_core::config::CONFIG_VERSION,
        "license": "GPL-3.0-or-later",
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
    fn the_report_can_check_for_generated_names() {
        let check = |c: &Config| {
            let out = generate(c, None).unwrap();
            out.variants[0].report["checks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["name"] == "no_generated_labels")
                .cloned()
        };
        let mut c = config(json!({ "world": { "cells": 2000, "template": "pangea" } }));
        assert!(check(&c).is_none(), "the check is off by default");
        c.render.labels.check_generated = true;
        let shown = check(&c).unwrap();
        assert_eq!(
            shown["pass"], false,
            "generation names the state and the places"
        );
        assert!(shown["count"].as_u64().unwrap() > 0);
        c.render.labels.generated = false;
        let hidden = check(&c).unwrap();
        assert_eq!(hidden["pass"], true);
        assert_eq!(hidden["count"], 0);
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
    fn an_overlay_changes_its_variant_and_not_the_world() {
        let pins = json!({
            "points": [
                { "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [20, 40] },
                { "id": "harrow", "kind": "town", "name": "Harrow", "at": [80, 40] }
            ],
            "roads": [{ "id": "kings-road", "from": "ostrey", "to": "harrow" }]
        });
        // A flat land with no water: no depression lake, and two states, so
        // that the state labels have room.
        let world = json!({ "cells": 1500, "states": 2, "template": { "steps": ["Add 35 all"] },
                            "lake_elevation_limit": 80 });
        let base = config(json!({ "world": world, "pins": pins }));
        let c = config(json!({
            "world": world, "pins": pins,
            "overlays": { "then": {
                "add": {
                    "places": [{ "id": "old-keep", "kind": "ruin", "name": "Old Keep", "at": [50, 60] }],
                    "features": [
                        { "id": "field", "kind": "battle-site", "name": "Red Field", "at": [50, 20] },
                        { "id": "old-wall", "kind": "wall", "polyline": [[10, 70], [90, 70]] }
                    ],
                    "roads": [{ "id": "keep-way", "from": "ostrey", "to": "old-keep", "kind": "trail", "target_length": 40, "tolerance": 0.5 }]
                },
                "remove": ["road:kings-road"],
                "rename": { "place:harrow": "Harrowgate" },
                "labels": { "place:ostrey": { "size_pt": 11 } }
            }},
            "variants": [
                { "name": "today", "page": { "w_mm": 167, "h_mm": 120 } },
                { "name": "then", "overlay": "then", "page": { "w_mm": 167, "h_mm": 120 } }
            ]
        }));
        let a = generate(&base, None).unwrap();
        let out = generate(&c, None).unwrap();
        assert_eq!(
            out.world_json, a.world_json,
            "an overlay does not change world.json"
        );
        let today = &out.variants[0].svg;
        let then = &out.variants[1].svg;
        assert!(today.contains("id=\"road-kings-road\""));
        assert!(!then.contains("id=\"road-kings-road\""));
        assert!(then.contains("id=\"road-keep-way\""));
        assert!(then.contains("<g id=\"place-old-keep\">"));
        assert!(then.contains("id=\"feature-field\"") && then.contains("id=\"feature-old-wall\""));
        assert!(then.contains(">Harrowgate</text>") && !then.contains(">Harrow</text>"));
        assert!(then.contains(">Red Field</text>"));
        assert!(then.contains(&format!(
            "font-size=\"{}\"",
            inkland_core::num::fixed(11.0 * 25.4 / 72.0, 3)
        )));
        let r = &out.variants[1].report;
        assert_eq!(r["overlay"]["name"], "then");
        assert_eq!(r["overlay"]["roads"][0]["id"], "keep-way");
        assert_eq!(r["overlay"]["roads"][0]["found"], true);
        assert_eq!(r["overlay"]["places"][0]["name"], "Old Keep");
        assert!(out.variants[0].report.get("overlay").is_none());
        assert!(!out.variants[1].checks_failed(), "{}", r["checks"]);
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
    fn the_undo_script_of_an_edit_gives_back_the_same_world_bytes() {
        let c = config(json!({
            "world": { "cells": 1500, "template": "pangea" },
            "pins": { "points": [
                { "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [20, 40] },
                { "id": "harrow", "kind": "town", "at": [80, 40] }
            ]}
        }));
        let world = generate(&c, None).unwrap().world_json;
        let script = r#"{ "version": 1, "commands": [
            { "op": "move_pin", "id": "harrow", "at": [70, 30] },
            { "op": "brush", "at": [50, 40], "radius": 15, "delta": 25 },
            { "op": "rename", "list": "points", "id": "ostrey", "name": "Old Ostrey" },
            { "op": "add_pin", "list": "points", "pin": { "id": "mere", "kind": "village", "at": [50, 60] } },
            { "op": "set", "path": "/world/sea_level", "value": 22 }
        ]}"#;
        let e = edit_world(&world, script).unwrap();
        assert_eq!(e.applied, 5);
        assert_ne!(e.world_json, world);
        let undo = serde_json::to_string(&e.undo).unwrap();
        let back = edit_world(&e.world_json, &undo).unwrap();
        assert_eq!(back.world_json, world);
        // An edit that makes the config invalid is refused.
        let bad = r#"{ "version": 1, "commands": [{ "op": "move_pin", "id": "harrow", "at": [500, 30] }] }"#;
        assert_eq!(
            edit_world(&world, bad).unwrap_err()[0].code,
            ErrorCode::PinOutsideFrame
        );
    }

    #[test]
    fn an_edit_of_a_world_file_outside_its_world_part_is_refused() {
        let c = config(json!({
            "world": { "cells": 1000, "template": "pangea" },
            "variants": [
                { "name": "full", "page": { "w_mm": 100, "h_mm": 80 } },
                { "name": "west", "crop": [0, 0, 50, 80], "page": { "w_mm": 50, "h_mm": 80 } }
            ]
        }));
        let world = generate(&c, None).unwrap().world_json;
        let script = r#"{ "version": 1, "commands": [
            { "op": "set", "path": "/world/sea_level", "value": 22 },
            { "op": "set", "path": "/variants/1/crop", "value": [0, 0, 40, 80] },
            { "op": "unset", "path": "/render/extras" }
        ]}"#;
        let errors = edit_world(&world, script).unwrap_err();
        let found: Vec<_> = errors
            .iter()
            .map(|e| (e.code, e.path.as_str(), e.details["command"].clone()))
            .collect();
        assert_eq!(
            found,
            [
                (ErrorCode::EditOutsideWorld, "/variants/1/crop", json!(1)),
                (ErrorCode::EditOutsideWorld, "/render/extras", json!(2)),
            ]
        );
        assert_eq!(exit_code_of(&errors), ExitCode::InvalidConfig);
        // The same script applies to the config file.
        let (next, _) = edit_config(&c, script).unwrap();
        assert_eq!(next.variants[1].crop, Some([0.0, 0.0, 40.0, 80.0]));
    }

    #[test]
    fn a_crop_has_its_own_labels_and_scale_bar() {
        let c = config(json!({
            "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea" },
            "pins": { "points": [
                { "id": "west", "kind": "city", "name": "Westmarch", "at": [100, 150] },
                { "id": "east", "kind": "city", "name": "Eastmarch", "at": [300, 150] }
            ]},
            "render": { "extras": { "scale_bar": true } },
            "variants": [
                { "name": "full", "page": { "w_mm": 167, "h_mm": 120 } },
                { "name": "west", "crop": [40, 90, 200, 210], "page": { "w_mm": 167, "h_mm": 120 } }
            ]
        }));
        let out = generate(&c, None).unwrap();
        let (full, west) = (&out.variants[0].svg, &out.variants[1].svg);
        assert!(full.contains(">Eastmarch</text>") && full.contains(">Westmarch</text>"));
        assert!(west.contains(">Westmarch</text>"));
        assert!(
            !west.contains(">Eastmarch</text>"),
            "a place outside the crop has no label"
        );
        // The scale bar shows a shorter length on the larger scale of the crop.
        let last = |svg: &str| -> f64 {
            let i = svg.find("<g id=\"scale-bar\"").unwrap();
            let part = &svg[i..];
            let end = part.find(" miles</text>").unwrap();
            let start = part[..end].rfind('>').unwrap() + 1;
            part[start..end].parse().unwrap()
        };
        assert!(last(west) < last(full), "{} {}", last(west), last(full));
        assert!(
            !out.variants[1].checks_failed(),
            "{}",
            out.variants[1].report["checks"]
        );
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
    fn colour_mode_reports_the_print_only_checks_as_not_applicable() {
        let c = config(json!({ "world": { "cells": 1000 }, "render": { "mode": "colour" } }));
        let out = generate(&c, None).unwrap();
        let checks = out.variants[0].report["checks"].as_array().unwrap().clone();
        for name in PRINT_ONLY_CHECKS {
            let c = checks.iter().find(|c| c["name"] == name).unwrap();
            assert_eq!(c["applicable"], false, "{name}");
            assert_eq!(c["pass"], true, "{name}");
        }
        for name in ["all_text_is_text_elements", "deterministic_ids"] {
            let c = checks.iter().find(|c| c["name"] == name).unwrap();
            assert_eq!(c["pass"], true, "{name}");
            assert!(c.get("applicable").is_none(), "{name}");
        }
        // The same SVG fails the black and white check in print-safe mode.
        let print = check_svg(&out.variants[0].svg, &config(json!({})));
        let bw = print
            .iter()
            .find(|c| c["name"] == "black_and_white")
            .unwrap();
        assert_eq!(bw["pass"], false);
    }

    #[test]
    fn report_checks_that_the_path_labels_fit() {
        let c = config(json!({
            "world": { "cells": 2000, "template": "pangea" },
            "pins": { "rivers": [{ "id": "dunn", "name": "The River Dunn",
                                   "source": [70, 20], "mouth": [5, 60] }],
                      "regions": [{ "id": "sea", "kind": "sea", "box": [0, 0, 8, 80] }] }
        }));
        let out = generate(&c, None).unwrap();
        let r = &out.variants[0].report;
        let check = r["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["name"] == "path_labels_fit")
            .expect("the check");
        assert_eq!(check["pass"], true, "{check}");
        assert!(r["labels"]["level_rivers"].is_array());
    }

    #[test]
    fn report_compares_the_relief_of_two_ages() {
        let c = config(json!({
            "world": { "cells": 2000, "template": "pangea" },
            "pins": { "regions": [{ "id": "fen", "kind": "marsh", "box": [20, 20, 60, 60] }] },
            "variants": [
                { "name": "then", "page": { "w_mm": 100, "h_mm": 80 } },
                { "name": "now", "page": { "w_mm": 100, "h_mm": 80 },
                  "relief": { "hide_in": ["fen"] } }
            ]
        }));
        let out = generate(&c, None).unwrap();
        let total = |v: &Value| {
            v.as_object()
                .unwrap()
                .values()
                .map(|n| n.as_u64().unwrap())
                .sum::<u64>()
        };
        let then = &out.variants[0].report["relief"];
        let now = &out.variants[1].report["relief"];
        assert!(then["hidden"].as_object().unwrap().is_empty());
        let fen = total(&now["hidden"]["fen"]);
        assert!(fen > 0, "{now}");
        assert_eq!(total(&then["shown"]), total(&now["shown"]) + fen);
        assert_eq!(total(&then["pins"]["fen"]), fen);
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
    fn report_holds_an_island_over_a_sea_and_a_rough_sea_in_range() {
        let c = config(json!({
            "frame": { "width": 300, "height": 200 },
            "world": { "cells": 3000, "template": "pangea" },
            "pins": {
                "regions": [
                    { "id": "bay", "kind": "sea", "box": [0, 0, 120, 200], "roughness": 8 },
                    { "id": "isle", "kind": "hills", "height": [22, 28],
                      "circle": { "at": [50, 100], "r": 10 }, "priority": 1 }
                ]
            }
        }));
        let out = generate(&c, None).unwrap();
        let regions = &out.variants[0].report["pins"]["regions"];
        for r in regions.as_array().unwrap() {
            assert_eq!(r["in_range"], true, "{r}");
            assert!(r["cells"].as_u64().unwrap() > 0, "{r}");
        }
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
    fn the_world_of_generate_answers_a_query_as_the_world_file_does() {
        let c = config(json!({ "world": { "cells": 1000 } }));
        let (world, out) = generate_with_world(&c, None).unwrap();
        assert_eq!(out, generate(&c, None).unwrap());
        let a = query_at(&world, &c, [40.0, 30.0]).unwrap();
        let b = query(&out.world_json, "40,30").unwrap();
        assert_eq!(a, b);
        assert!(a["height"].is_number() && a["biome"].is_string());
        assert_eq!(
            query_at(&world, &c, [-1.0, 30.0]).unwrap_err().code,
            ErrorCode::ValueOutOfRange
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
