//! Measure the time of world generation and of one colour render.
//!
//! Usage: `cargo run --release -p inkland --example timing -- [CELLS...]`.
//!
//! For each cell count, the example makes 6 worlds with the seeds 1 to 6
//! and 6 FMG templates, and prints the time of each world and the median.
//! "generate" is `World::build`: all the stages. "render" is the SVG of
//! one colour variant in the `default` style. The times compare with the
//! "Generation Pipeline" and "Layers Rendering" measures of FMG.

use inkland_core::config::Config;
use inkland_core::world::World;
use serde_json::json;
use std::time::Instant;

const TEMPLATES: [&str; 6] = [
    "continents",
    "high-island",
    "low-island",
    "archipelago",
    "mediterranean",
    "pangea",
];

fn config(seed: u64, cells: u64, template: &str) -> Config {
    serde_json::from_value(json!({
        "version": 1,
        "seed": seed,
        "frame": { "width": 1024, "height": 768 },
        "world": { "cells": cells, "template": template },
        "render": { "mode": "colour", "style": "default" },
        "variants": [{ "name": "map", "page": { "w_mm": 250, "h_mm": 187, "margin_mm": 5 } }]
    }))
    .expect("the timing config is valid")
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn main() {
    let mut counts: Vec<u64> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    if counts.is_empty() {
        counts = vec![10_000, 20_000, 50_000, 100_000];
    }
    // One world before the timing, so that the first time does not count
    // the first use of the memory and the files.
    let _ = World::build(&config(0, 2_000, "continents"));
    for cells in counts {
        let (mut gen_ms, mut draw_ms) = (Vec::new(), Vec::new());
        for (k, template) in TEMPLATES.iter().enumerate() {
            let c = config(k as u64 + 1, cells, template);
            let t = Instant::now();
            let world = match World::build(&c) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("{cells} cells, {template}: {e:?}");
                    continue;
                }
            };
            let g = t.elapsed().as_secs_f64() * 1000.0;
            let t = Instant::now();
            let svg = inkland_render::svg::render(&world, &c, &c.variants[0]).svg;
            let d = t.elapsed().as_secs_f64() * 1000.0;
            println!(
                "{cells:>7} cells  {template:<14} grid {:>6}  pack {:>6}  generate {g:>7.0} ms  render {d:>5.0} ms  ({} kB)",
                world.base.grid.len(),
                world.grid.len(),
                svg.len() / 1000
            );
            gen_ms.push(g);
            draw_ms.push(d);
        }
        println!(
            "{cells:>7} cells  median: generate {:.0} ms, render {:.0} ms",
            median(gen_ms),
            median(draw_ms)
        );
    }
}
