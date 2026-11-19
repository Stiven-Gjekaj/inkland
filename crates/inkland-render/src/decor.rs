//! Print-safe decoration: smooth coasts, water lines, and relief symbols.
//!
//! Shading uses lines and dots only, never grey: water lines follow the
//! coast in the water, and symbols mark mountains, hills, forest, wetland,
//! and desert. The symbols come from the `decoration` layer of the seed
//! tree, so they are the same on each run.

use crate::page::Page;
use inkland_core::biomes;
use inkland_core::grid::OUTSIDE;
use inkland_core::rand::Rand;
use inkland_core::seed::{Layer, SeedTree};
use inkland_core::world::World;
use std::collections::{BTreeMap, VecDeque};

/// Cut the corners of a line: each segment gives two points at a quarter
/// and three quarters of it. An open line keeps its ends.
pub fn chaikin(points: &[[f64; 2]], closed: bool, rounds: usize) -> Vec<[f64; 2]> {
    let mut pts = points.to_vec();
    if closed && pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    for _ in 0..rounds {
        if pts.len() < 3 {
            break;
        }
        let n = pts.len();
        let mut out = Vec::with_capacity(2 * n + 2);
        if !closed {
            out.push(pts[0]);
        }
        let segs = if closed { n } else { n - 1 };
        for i in 0..segs {
            let a = pts[i];
            let b = pts[(i + 1) % n];
            out.push([0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]]);
            out.push([0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]);
        }
        if !closed {
            out.push(pts[n - 1]);
        }
        pts = out;
    }
    if closed && !pts.is_empty() {
        pts.push(pts[0]);
    }
    pts
}

/// The width factor of a river for `render.river_contrast`: with the
/// contrast `k`, a river with the share `r` of the water of the largest
/// river gets `(1 + k) * r ^ (0.6 k)`. The largest river is `1 + k` times
/// as wide as before, and a stream with a hundredth of its water is about
/// a seventh as wide (at `k = 1`). `k = 0` gives 1.
pub fn river_scale(world: &World, river: &inkland_core::rivers::River, contrast: f64) -> f64 {
    if !(contrast.is_finite() && contrast > 0.0) {
        return 1.0;
    }
    let max = world.rivers.iter().map(|r| r.flux).fold(0.0f64, f64::max);
    if max <= 0.0 {
        return 1.0;
    }
    let share = (river.flux / max).clamp(1e-6, 1.0);
    (1.0 + contrast) * libm::pow(share, 0.6 * contrast)
}

/// `true` for a cell of the first grid in the ocean at least `depth` below
/// sea level, on the height scale.
pub fn deep(world: &World, c: usize, depth: f64) -> bool {
    let b = &world.base;
    !b.features.is_land(c)
        && b.features.get(b.features.of_cell[c]).kind == inkland_core::features::FeatureKind::Ocean
        && b.heights[c] <= world.sea_level - depth
}

/// The depth contours of the ocean: the lines around the ocean cells at
/// least `depth` below sea level, on the height scale. Each line is a list
/// of vertex indices of the first grid. A line ends at the frame, so it is
/// open there; else it is closed (its last vertex is its first).
pub fn depth_lines(world: &World, depth: f64) -> Vec<Vec<u32>> {
    let g = &world.base.grid;
    let rings = crate::fmg::geom::rings_on(g, &|c| u32::from(deep(world, c, depth)));
    let mut out = Vec::new();
    for ring in rings.get(&1).into_iter().flatten() {
        // Start after an edge on the frame, so that each piece is whole.
        let start = ring
            .iter()
            .position(|e| e.other == OUTSIDE)
            .map_or(0, |i| (i + 1) % ring.len());
        let closed = ring.iter().all(|e| e.other != OUTSIDE);
        let mut line: Vec<u32> = Vec::new();
        for k in 0..ring.len() {
            let e = ring[(start + k) % ring.len()];
            if e.other == OUTSIDE {
                if line.len() > 1 {
                    out.push(std::mem::take(&mut line));
                }
                line.clear();
                continue;
            }
            if line.is_empty() {
                line.push(e.from);
            }
            line.push(e.to);
        }
        if line.len() > 1 {
            if closed && line.first() != line.last() {
                let first = line[0];
                line.push(first);
            }
            out.push(line);
        }
    }
    out
}

/// The water lines: lines between water cells at distance `k` and `k + 1`
/// from land, for `k` in 1 and 2. Each line is a list of vertex indices of
/// the first grid, which keeps the deep water (as the ocean bands of FMG).
pub fn water_lines(world: &World) -> Vec<Vec<u32>> {
    let g = &world.base.grid;
    let n = g.len();
    let mut d = vec![u32::MAX; n];
    let mut queue = VecDeque::new();
    for (c, dc) in d.iter_mut().enumerate() {
        if world.base.features.is_land(c) {
            *dc = 0;
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        for m in g.neighbors(c) {
            if d[m] == u32::MAX {
                d[m] = d[c] + 1;
                queue.push_back(m);
            }
        }
    }
    let mut edges = Vec::new();
    for c in 0..n {
        if !(1..=2).contains(&d[c]) {
            continue;
        }
        let poly = &g.polygons[c];
        for (k, &o) in g.edge_cells[c].iter().enumerate() {
            if o == OUTSIDE {
                continue;
            }
            if d[o as usize] == d[c] + 1 {
                edges.push((poly[k], poly[(k + 1) % poly.len()]));
            }
        }
    }
    inkland_core::borders::chain_edges(&edges)
}

/// Placed symbols in square buckets of the page, for a fast distance test.
type Buckets = BTreeMap<(i64, i64), Vec<(Symbol, [f64; 2])>>;

/// A relief symbol: its kind and its position on the page, in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Symbol {
    Mountain,
    Hill,
    Tree,
    Marsh,
    Dune,
}

impl Symbol {
    /// The least distance between two symbols of this kind, in mm.
    fn spacing(self) -> f64 {
        match self {
            Symbol::Mountain => 3.6,
            Symbol::Hill => 2.8,
            Symbol::Tree => 2.0,
            Symbol::Marsh => 2.6,
            Symbol::Dune => 1.6,
        }
    }

    /// Path data of the symbol at `(x, y)`, in mm.
    pub fn path(self, x: f64, y: f64) -> String {
        let f = |v: f64| inkland_core::num::fixed(v, 3);
        match self {
            // A peak, with a hatch line on its east side.
            Symbol::Mountain => format!(
                "M{} {}L{} {}L{} {}M{} {}L{} {}",
                f(x - 1.6),
                f(y + 0.9),
                f(x),
                f(y - 1.1),
                f(x + 1.6),
                f(y + 0.9),
                f(x + 0.25),
                f(y - 0.6),
                f(x + 0.85),
                f(y + 0.7)
            ),
            Symbol::Hill => format!(
                "M{} {}Q{} {} {} {}",
                f(x - 1.1),
                f(y + 0.4),
                f(x),
                f(y - 0.9),
                f(x + 1.1),
                f(y + 0.4)
            ),
            Symbol::Tree => format!(
                "M{} {}a0.45 0.45 0 1 1 0.001 0M{} {}v0.7",
                f(x - 0.45),
                f(y - 0.3),
                f(x),
                f(y + 0.15)
            ),
            Symbol::Marsh => format!(
                "M{} {}h1.4M{} {}h0.9M{} {}h1.4",
                f(x - 0.7),
                f(y - 0.4),
                f(x - 0.45),
                f(y),
                f(x - 0.7),
                f(y + 0.4)
            ),
            Symbol::Dune => format!("M{} {}a0.12 0.12 0 1 1 0.001 0", f(x - 0.12), f(y)),
        }
    }
}

/// The relief symbol for a land cell, if any.
fn symbol_of(world: &World, c: usize) -> Option<Symbol> {
    let h = world.heights[c] - world.sea_level;
    if h >= 45.0 {
        return Some(Symbol::Mountain);
    }
    if h >= 22.0 {
        return Some(Symbol::Hill);
    }
    match world.biomes[c] {
        biomes::WETLAND => Some(Symbol::Marsh),
        biomes::HOT_DESERT | biomes::COLD_DESERT => Some(Symbol::Dune),
        5..=9 => Some(Symbol::Tree),
        _ => None,
    }
}

/// Place relief symbols: cells in a random order from the seed tree, each
/// at least the spacing of its kind from the symbols before it.
pub fn relief(
    world: &World,
    page: &Page,
    seed: u64,
    opts: &inkland_core::config::ReliefOptions,
) -> Vec<(Symbol, [f64; 2])> {
    use inkland_core::config::ReliefSymbol;
    if opts.density <= 0.0 {
        return Vec::new();
    }
    // A density d keeps about d of the symbols: the spacing grows by
    // 1 / sqrt(d), so the area of each symbol grows by 1 / d.
    let scale = 1.0 / libm::sqrt(opts.density.min(1.0));
    let hidden = |sym: Symbol| {
        let kind = match sym {
            Symbol::Mountain => ReliefSymbol::Mountain,
            Symbol::Hill => ReliefSymbol::Hill,
            Symbol::Tree => ReliefSymbol::Tree,
            Symbol::Marsh => ReliefSymbol::Marsh,
            Symbol::Dune => ReliefSymbol::Dune,
        };
        opts.hide.contains(&kind)
    };
    let g = &world.grid;
    let mut rng = Rand::new(SeedTree::new(seed).part_rng(Layer::Decoration, "relief"));
    let mut order: Vec<(u64, usize)> = (0..g.len())
        .map(|c| ((rng.unit() * (1u64 << 52) as f64) as u64, c))
        .collect();
    order.sort();
    let [x0, y0, x1, y1] = page.view;
    let bucket = 4.0 * scale;
    let mut grid: Buckets = BTreeMap::new();
    let mut out = Vec::new();
    for (_, c) in order {
        if !world.features.is_land(c) {
            continue;
        }
        let p = g.points[c];
        if p[0] < x0 || p[0] > x1 || p[1] < y0 || p[1] > y1 {
            continue;
        }
        let Some(sym) = symbol_of(world, c) else {
            continue;
        };
        if hidden(sym) {
            continue;
        }
        // Jitter inside the cell, so that the symbols do not form rows.
        let j = (g.step_x.min(g.step_y)) * 0.3;
        let w = [p[0] + rng.uniform(-j, j), p[1] + rng.uniform(-j, j)];
        let q = page.to_page(w);
        let key = (
            (q[0] / bucket).floor() as i64,
            (q[1] / bucket).floor() as i64,
        );
        let near = (-1..=1)
            .flat_map(|dx| (-1..=1).map(move |dy| (key.0 + dx, key.1 + dy)))
            .any(|k| {
                grid.get(&k).is_some_and(|v| {
                    v.iter().any(|(s, r)| {
                        let gap = s.spacing().max(sym.spacing()) * scale;
                        (r[0] - q[0]).powi(2) + (r[1] - q[1]).powi(2) < gap * gap
                    })
                })
            });
        if near {
            continue;
        }
        grid.entry(key).or_default().push((sym, q));
        out.push((sym, q));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_relief_options_thin_out_and_hide_the_symbols() {
        use inkland_core::config::{Config, ReliefOptions, ReliefSymbol};
        let c: Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 6, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 6000, "template": "pangea" },
            "variants": [{ "name": "m", "page": { "w_mm": 300, "h_mm": 200 } }]
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let page = Page::new(&c, &c.variants[0]);
        let run = |o: ReliefOptions| relief(&w, &page, c.seed, &o);
        let count = |v: &[(Symbol, [f64; 2])], s: Symbol| v.iter().filter(|x| x.0 == s).count();
        let full = run(ReliefOptions::default());
        assert!(count(&full, Symbol::Tree) > 50);
        let quarter = run(ReliefOptions {
            density: 0.25,
            ..Default::default()
        });
        let (a, b) = (full.len() as f64, quarter.len() as f64);
        assert!(b > 0.1 * a && b < 0.45 * a, "{b} of {a}");
        let no_trees = run(ReliefOptions {
            hide: vec![ReliefSymbol::Tree],
            ..Default::default()
        });
        assert_eq!(count(&no_trees, Symbol::Tree), 0);
        assert!(count(&no_trees, Symbol::Mountain) + count(&no_trees, Symbol::Hill) > 0);
        assert!(
            run(ReliefOptions {
                density: 0.0,
                ..Default::default()
            })
            .is_empty()
        );
    }

    #[test]
    fn river_contrast_widens_the_main_river_and_thins_the_streams() {
        use inkland_core::config::Config;
        let c: Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 6, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 4000, "template": "pangea" }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        assert!(w.rivers.len() > 3);
        let main = w
            .rivers
            .iter()
            .max_by(|a, b| a.flux.total_cmp(&b.flux))
            .unwrap();
        let small = w
            .rivers
            .iter()
            .min_by(|a, b| a.flux.total_cmp(&b.flux))
            .unwrap();
        assert_eq!(river_scale(&w, main, 1.0), 2.0);
        assert!(river_scale(&w, small, 1.0) < 1.0);
        assert!(river_scale(&w, small, 2.0) < river_scale(&w, small, 1.0));
        assert_eq!(river_scale(&w, small, 0.0), 1.0);
    }

    #[test]
    fn chaikin_keeps_the_ends_of_an_open_line() {
        let pts = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]];
        let s = chaikin(&pts, false, 2);
        assert_eq!(s.first(), Some(&[0.0, 0.0]));
        assert_eq!(s.last(), Some(&[10.0, 10.0]));
        assert!(s.len() > pts.len());
    }

    #[test]
    fn chaikin_closes_a_closed_line() {
        let pts = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 0.0]];
        let s = chaikin(&pts, true, 1);
        assert_eq!(s.first(), s.last());
        assert_eq!(s.len(), 7);
    }
}
