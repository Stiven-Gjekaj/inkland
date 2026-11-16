//! The report that `generate` writes as `report.json` and prints.

use inkland_core::config::Config;
use inkland_core::error::Warning;
use serde::Serialize;

/// The name and the version of the tool.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tool {
    pub name: &'static str,
    pub version: &'static str,
}

impl Tool {
    pub fn current() -> Self {
        Tool {
            name: "inkland",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

/// The result of one check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub pass: bool,
}

/// The counts of the pins in the config.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PinCounts {
    pub points: usize,
    pub regions: usize,
    pub rivers: usize,
    pub roads: usize,
}

/// The report of `generate --dry-run`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DryRunReport {
    pub tool: Tool,
    pub config_hash: String,
    pub seed: u64,
    pub units: String,
    pub dry_run: bool,
    pub variants: Vec<String>,
    pub pins: PinCounts,
    pub checks: Vec<Check>,
    pub warnings: Vec<Warning>,
    /// The config after the flags are applied.
    pub effective_config: Config,
}

/// The report of one generated variant.
pub fn variant_report(
    world: &inkland_core::world::World,
    config: &Config,
    variant: &str,
    config_hash: &str,
) -> serde_json::Value {
    use inkland_core::features::FeatureKind;
    use inkland_core::num::round;
    let count = |k: FeatureKind| world.features.list.iter().filter(|f| f.kind == k).count();
    serde_json::json!({
        "tool": Tool::current(),
        "config_hash": config_hash,
        "seed": config.seed,
        "variant": variant,
        "units": config.units.name,
        "dry_run": false,
        "world": {
            "cells": world.grid.len(),
            "land_fraction": round(world.land_fraction(), 3),
            "features": {
                "ocean": count(FeatureKind::Ocean),
                "lake": count(FeatureKind::Lake),
                "island": count(FeatureKind::Island),
            },
            "coastlines": world.coastlines.len(),
        },
        "pins": { "points": [], "regions": [], "rivers": [], "roads": [] },
        "labels": { "placed": 0, "unplaced": [] },
        "checks": [],
        "warnings": [],
    })
}
