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

/// The terrain of the cells of the second grid, for the relief symbols of
/// both render modes (`render.relief.terrain`).
#[derive(Debug, Clone, PartialEq)]
pub struct ReliefTerrain {
    /// A ridge cell: a cell in the highest 15 percent of the land, at least
    /// `RIDGE_HEIGHT` above sea level, higher than all of its neighbors but
    /// two and higher than their mean, so it is on a crest line.
    pub ridge: Vec<bool>,
    /// A foot cell: a land cell next to a ridge cell, not itself a ridge.
    pub foot: Vec<bool>,
    /// How high a land cell is, 0 at 15 above sea level to 1 at 55 above.
    pub crest: Vec<f64>,
    /// Fractal noise from -1 to 1 with features of about 7 cells, so that
    /// forests grow in groups.
    pub clump: Vec<f64>,
}

/// Where the relief shows no ridge peaks: the `hills` regions and the
/// regions with `peaks: false`, and the `rolling` and `flat` areas and the
/// areas with `peaks: false`, with their soft edges.
pub struct Peakless<'a> {
    shapes: Vec<inkland_core::geom::Shape>,
    areas: Vec<inkland_core::character::Area<'a>>,
}

impl<'a> Peakless<'a> {
    /// The places without peaks of a config.
    pub fn of(world: &World, config: &'a inkland_core::config::Config) -> Self {
        let n = world.base.grid.len().max(1) as f64;
        let spacing = (world.width * world.height / n).sqrt();
        Peakless {
            shapes: config.peakless_shapes(),
            areas: inkland_core::character::Area::all(config, spacing)
                .into_iter()
                .filter(|a| !inkland_core::config::Config::area_has_peaks(a.pin))
                .collect(),
        }
    }

    /// No place without peaks.
    pub fn none() -> Self {
        Peakless {
            shapes: Vec::new(),
            areas: Vec::new(),
        }
    }

    /// `true` if no ridge peak stands at `p`.
    pub fn contains(&self, p: [f64; 2]) -> bool {
        self.shapes.iter().any(|s| s.contains(p)) || self.areas.iter().any(|a| a.weight(p) > 0.0)
    }
}

/// The height above sea level, from which a crest cell is a ridge.
pub const RIDGE_HEIGHT: f64 = 15.0;

/// The terrain of a world for the relief symbols. The noise of the groups
/// comes from the `decoration` layer of the seed tree.
///
/// No ridge stands in `peakless`, for example in a `hills` region.
pub fn relief_terrain(world: &World, peakless: &Peakless) -> ReliefTerrain {
    let g = &world.grid;
    let n = g.len();
    let sea = world.sea_level;
    let land = |c: usize| world.features.is_land(c);
    // The height of the highest 15 percent of the land.
    let mut land_heights: Vec<f64> = (0..n)
        .filter(|&c| land(c))
        .map(|c| world.heights[c])
        .collect();
    land_heights.sort_by(f64::total_cmp);
    let top = land_heights
        .get(land_heights.len() * 85 / 100)
        .copied()
        .unwrap_or(f64::INFINITY);
    let least = top.max(sea + RIDGE_HEIGHT);
    let mut ridge = vec![false; n];
    for (c, r) in ridge.iter_mut().enumerate() {
        let h = world.heights[c];
        if !land(c) || h < least {
            continue;
        }
        if peakless.contains(g.points[c]) {
            continue;
        }
        let (mut count, mut lower, mut sum) = (0, 0, 0.0);
        for m in g.neighbors(c) {
            count += 1;
            sum += world.heights[m];
            if world.heights[m] < h {
                lower += 1;
            }
        }
        *r = count > 0 && lower + 2 >= count && h >= sum / f64::from(count) + 1.0;
    }
    let foot: Vec<bool> = (0..n)
        .map(|c| land(c) && !ridge[c] && g.neighbors(c).any(|m| ridge[m]))
        .collect();
    let crest: Vec<f64> = (0..n)
        .map(|c| ((world.heights[c] - sea - RIDGE_HEIGHT) / 40.0).clamp(0.0, 1.0))
        .collect();
    let spacing = (world.width * world.height / n.max(1) as f64).sqrt();
    let seed = inkland_core::noise::Fractal::seed_of(
        SeedTree::new(world.seed).part_seed(Layer::Decoration, "relief-clumps"),
    );
    let noise = inkland_core::noise::Fractal::new(seed, 3, 7.0 * spacing);
    let clump = (0..n)
        .map(|c| (2.0 * noise.at(g.points[c])).clamp(-1.0, 1.0))
        .collect();
    ReliefTerrain {
        ridge,
        foot,
        crest,
        clump,
    }
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
    /// A field with furrows, on open farmland near the places.
    Field,
}

impl Symbol {
    /// The kind of the symbol in the relief options.
    pub fn kind(self) -> inkland_core::config::ReliefSymbol {
        use inkland_core::config::ReliefSymbol;
        match self {
            Symbol::Mountain => ReliefSymbol::Mountain,
            Symbol::Hill => ReliefSymbol::Hill,
            Symbol::Tree => ReliefSymbol::Tree,
            Symbol::Marsh => ReliefSymbol::Marsh,
            Symbol::Dune => ReliefSymbol::Dune,
            Symbol::Field => ReliefSymbol::Field,
        }
    }

    /// The name of the kind, as in `render.relief.hide`.
    pub fn name(self) -> &'static str {
        self.kind().name()
    }

    /// The least distance between two symbols of this kind, in mm.
    fn spacing(self) -> f64 {
        match self {
            Symbol::Mountain => 3.6,
            Symbol::Hill => 2.8,
            Symbol::Tree => 2.0,
            Symbol::Marsh => 2.6,
            Symbol::Dune => 1.6,
            Symbol::Field => 3.2,
        }
    }

    /// Path data of the symbol at `(x, y)`, in mm.
    pub fn path(self, x: f64, y: f64) -> String {
        self.path_scaled(x, y, 1.0)
    }

    /// Path data of the symbol at `(x, y)`, in mm, `k` times its size. A
    /// marsh and a dune keep their size.
    pub fn path_scaled(self, x: f64, y: f64, k: f64) -> String {
        let f = |v: f64| inkland_core::num::fixed(v, 3);
        match self {
            // A peak, with a hatch line on its east side.
            Symbol::Mountain => format!(
                "M{} {}L{} {}L{} {}M{} {}L{} {}",
                f(x - 1.6 * k),
                f(y + 0.9 * k),
                f(x),
                f(y - 1.1 * k),
                f(x + 1.6 * k),
                f(y + 0.9 * k),
                f(x + 0.25 * k),
                f(y - 0.6 * k),
                f(x + 0.85 * k),
                f(y + 0.7 * k)
            ),
            Symbol::Hill => format!(
                "M{} {}Q{} {} {} {}",
                f(x - 1.1 * k),
                f(y + 0.4 * k),
                f(x),
                f(y - 0.9 * k),
                f(x + 1.1 * k),
                f(y + 0.4 * k)
            ),
            Symbol::Tree => format!(
                "M{} {}a{} {} 0 1 1 0.001 0M{} {}v{}",
                f(x - 0.45 * k),
                f(y - 0.3 * k),
                f(0.45 * k),
                f(0.45 * k),
                f(x),
                f(y + 0.15 * k),
                f(0.7 * k)
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
            // A field: a low, slanted outline with two furrows in it.
            Symbol::Field => format!(
                "M{} {}L{} {}L{} {}L{} {}ZM{} {}L{} {}M{} {}L{} {}",
                f(x - 1.2 * k),
                f(y + 0.5 * k),
                f(x - 0.8 * k),
                f(y - 0.5 * k),
                f(x + 1.2 * k),
                f(y - 0.5 * k),
                f(x + 0.8 * k),
                f(y + 0.5 * k),
                f(x - 0.7 * k),
                f(y + 0.17 * k),
                f(x + 0.85 * k),
                f(y + 0.17 * k),
                f(x - 0.55 * k),
                f(y - 0.17 * k),
                f(x + 1.0 * k),
                f(y - 0.17 * k)
            ),
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

/// A relief candidate: the symbol, its cell, its point (`None`: the cell
/// site with jitter), its size factor, and its spacing factor.
type Candidate = (Symbol, usize, Option<[f64; 2]>, f64, f64);

/// Place relief symbols: cells in a random order from the seed tree, each
/// at least the spacing of its kind from the symbols before it. Each symbol
/// has a size factor. With `terrain`, the ridges come first: mountains in
/// chains along them, then hills at their feet, then the other symbols.
pub fn relief(
    world: &World,
    page: &Page,
    seed: u64,
    opts: &inkland_core::config::ReliefOptions,
    peakless: &Peakless,
) -> Vec<(Symbol, [f64; 2], f64)> {
    if opts.density <= 0.0 {
        return Vec::new();
    }
    // A density d keeps about d of the symbols: the spacing grows by
    // 1 / sqrt(d), so the area of each symbol grows by 1 / d.
    let scale = 1.0 / libm::sqrt(opts.density.min(1.0));
    let hidden = |sym: Symbol| opts.hide.contains(&sym.kind());
    let g = &world.grid;
    let mut rng = Rand::new(SeedTree::new(seed).part_rng(Layer::Decoration, "relief"));
    let mut order: Vec<(u64, usize)> = (0..g.len())
        .map(|c| ((rng.unit() * (1u64 << 52) as f64) as u64, c))
        .collect();
    order.sort();
    let terrain = opts.terrain.then(|| relief_terrain(world, peakless));
    // Each candidate: the symbol, the world point, the size, and the
    // spacing factor. `None` for the point: the cell site with jitter.
    let mut candidates: Vec<Candidate> = Vec::new();
    if let Some(t) = &terrain {
        // The ridges, highest first: a peak at each ridge cell and one
        // between each two ridge cells, so that the peaks overlap.
        let mut ridges: Vec<usize> = (0..g.len()).filter(|&c| t.ridge[c]).collect();
        ridges.sort_by(|&a, &b| {
            world.heights[b]
                .total_cmp(&world.heights[a])
                .then(a.cmp(&b))
        });
        for &c in &ridges {
            let k = 0.8 + 0.6 * t.crest[c];
            candidates.push((Symbol::Mountain, c, Some(g.points[c]), k, 0.5));
            for m in g.neighbors(c).filter(|&m| t.ridge[m] && m > c) {
                let (a, b) = (g.points[c], g.points[m]);
                let km = 0.8 + 0.3 * (t.crest[c] + t.crest[m]);
                candidates.push((
                    Symbol::Mountain,
                    c,
                    Some([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]),
                    km,
                    0.5,
                ));
            }
        }
        for &(_, c) in &order {
            if t.foot[c] {
                candidates.push((Symbol::Hill, c, None, 0.8 + 0.4 * rng.unit(), 0.9));
            }
        }
    }
    for &(_, c) in &order {
        if !world.features.is_land(c) {
            continue;
        }
        if let Some(t) = &terrain {
            if t.ridge[c] || t.foot[c] {
                continue;
            }
            let Some(sym) = symbol_of(world, c) else {
                continue;
            };
            // Trees grow in groups, and closer in the highland.
            if sym == Symbol::Tree && t.clump[c] < -0.15 {
                continue;
            }
            let high = ((world.heights[c] - world.sea_level) / 30.0).clamp(0.0, 1.0);
            let gap = if sym == Symbol::Tree {
                1.1 - 0.35 * high
            } else {
                1.0
            };
            candidates.push((sym, c, None, 0.85 + 0.3 * rng.unit(), gap));
        } else if let Some(sym) = symbol_of(world, c) {
            candidates.push((sym, c, None, 1.0, 1.0));
        }
    }
    // Fields: open grassland and savanna in the lowland near a place, where
    // no other symbol stands. They come last, so the other symbols keep
    // their places.
    let near = 6.0 * g.step_x.max(g.step_y);
    let places: Vec<[f64; 2]> = world.society.places.iter().map(|p| p.at).collect();
    for &(_, c) in &order {
        let open = matches!(world.biomes[c], 3 | 4);
        let low = world.heights[c] - world.sea_level < 22.0;
        if !(open && low && world.features.is_land(c) && symbol_of(world, c).is_none()) {
            continue;
        }
        let p = g.points[c];
        if places
            .iter()
            .any(|q| (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) <= near * near)
        {
            // As the other symbols: a size that changes a little only with
            // the terrain placement.
            let k = if terrain.is_some() {
                0.85 + 0.3 * rng.unit()
            } else {
                1.0
            };
            candidates.push((Symbol::Field, c, None, k, 1.0));
        }
    }
    let [x0, y0, x1, y1] = page.view;
    let bucket = 4.0 * scale;
    let mut grid: Buckets = BTreeMap::new();
    let mut out = Vec::new();
    for (sym, c, at, k, gap_factor) in candidates {
        if !world.features.is_land(c) || hidden(sym) {
            continue;
        }
        let p = at.unwrap_or(g.points[c]);
        if p[0] < x0 || p[0] > x1 || p[1] < y0 || p[1] > y1 {
            continue;
        }
        // Jitter inside the cell, so that the symbols do not form rows.
        let w = if at.is_some() {
            p
        } else {
            let j = (g.step_x.min(g.step_y)) * 0.3;
            [p[0] + rng.uniform(-j, j), p[1] + rng.uniform(-j, j)]
        };
        let q = page.to_page(w);
        let key = (
            (q[0] / bucket).floor() as i64,
            (q[1] / bucket).floor() as i64,
        );
        let near = (-1..=1)
            .flat_map(|dx| (-1..=1).map(move |dy| (key.0 + dx, key.1 + dy)))
            .any(|kk| {
                grid.get(&kk).is_some_and(|v| {
                    v.iter().any(|(s, r)| {
                        let gap = s.spacing().max(sym.spacing()) * scale * gap_factor;
                        (r[0] - q[0]).powi(2) + (r[1] - q[1]).powi(2) < gap * gap
                    })
                })
            });
        if near {
            continue;
        }
        grid.entry(key).or_default().push((sym, q));
        out.push((sym, q, k));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_lie_on_open_lowland_near_the_places() {
        use inkland_core::config::{Config, ReliefOptions, ReliefSymbol};
        let c: Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 6, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 6000, "template": "pangea" },
            "variants": [{ "name": "m", "page": { "w_mm": 300, "h_mm": 200 } }]
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let page = Page::new(&c, &c.variants[0]);
        let g = &w.grid;
        let near = 6.0 * g.step_x.max(g.step_y);
        let fields: Vec<[f64; 2]> = relief(
            &w,
            &page,
            c.seed,
            &ReliefOptions::default(),
            &Peakless::none(),
        )
        .into_iter()
        .filter(|s| s.0 == Symbol::Field)
        .map(|s| page.to_world(s.1))
        .collect();
        assert!(fields.len() > 5, "{} fields", fields.len());
        for p in &fields {
            // The jitter moves a symbol up to a third of a cell, so a field
            // at the coast can stand in a water cell next to its land cell.
            let cell = g.find_cell(p[0], p[1]);
            assert!(w.features.is_land(cell) || g.neighbors(cell).any(|m| w.features.is_land(m)));
            let reach = near + g.step_x.max(g.step_y);
            assert!(
                w.society
                    .places
                    .iter()
                    .any(|q| (q.at[0] - p[0]).powi(2) + (q.at[1] - p[1]).powi(2) <= reach * reach),
                "a field far from the places at {p:?}"
            );
        }
        let none = relief(
            &w,
            &page,
            c.seed,
            &ReliefOptions {
                hide: vec![ReliefSymbol::Field],
                ..Default::default()
            },
            &Peakless::none(),
        );
        assert!(none.iter().all(|s| s.0 != Symbol::Field));
    }

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
        let run = |o: ReliefOptions| relief(&w, &page, c.seed, &o, &Peakless::none());
        let count =
            |v: &[(Symbol, [f64; 2], f64)], s: Symbol| v.iter().filter(|x| x.0 == s).count();
        let full = run(ReliefOptions {
            terrain: false,
            ..Default::default()
        });
        assert!(count(&full, Symbol::Tree) > 50);
        let quarter = run(ReliefOptions {
            density: 0.25,
            terrain: false,
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
    fn terrain_relief_puts_mountains_in_chains_and_trees_in_groups() {
        use inkland_core::config::{Config, ReliefOptions};
        let c: Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 6, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 6000, "template": "high-island" },
            "variants": [{ "name": "m", "page": { "w_mm": 300, "h_mm": 200 } }]
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let page = Page::new(&c, &c.variants[0]);
        let fmg = relief(
            &w,
            &page,
            c.seed,
            &ReliefOptions {
                terrain: false,
                ..Default::default()
            },
            &Peakless::none(),
        );
        let new = relief(
            &w,
            &page,
            c.seed,
            &ReliefOptions::default(),
            &Peakless::none(),
        );
        let mountains = |v: &[(Symbol, [f64; 2], f64)]| {
            v.iter()
                .filter(|x| x.0 == Symbol::Mountain)
                .map(|x| x.1)
                .collect::<Vec<_>>()
        };
        let (a, b) = (mountains(&fmg), mountains(&new));
        assert!(b.len() > a.len(), "more mountains: {} {}", a.len(), b.len());
        // In a chain, peaks overlap: closer than the old spacing of 3.6 mm.
        let close = b.iter().enumerate().any(|(i, p)| {
            b[i + 1..]
                .iter()
                .any(|q| (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) < 3.0 * 3.0)
        });
        assert!(close);
        // The sizes change.
        let sizes: Vec<f64> = new.iter().map(|x| x.2).collect();
        let (lo, hi) = sizes
            .iter()
            .fold((9.0f64, 0.0f64), |(l, h), &k| (l.min(k), h.max(k)));
        assert!(hi - lo > 0.2);
        assert!(fmg.iter().all(|x| x.2 == 1.0));
    }

    #[test]
    fn a_hills_region_shows_hills_and_no_ridge_peaks() {
        use inkland_core::config::{Config, ReliefOptions};
        // Low land, and rough hills are the highest land of the map.
        let count = |peaks: Option<bool>| {
            let mut v = serde_json::json!({
                "version": 1, "seed": 6, "frame": { "width": 300, "height": 200 },
                "world": { "cells": 4000, "template": { "steps": ["Add 25 all"] } },
                "pins": { "regions": [
                    { "id": "sea", "kind": "sea", "box": [0, 0, 20, 200] },
                    { "id": "wolds", "kind": "hills", "box": [120, 60, 220, 140], "height": [42, 52] }
                ],
                "areas": [{ "id": "rough", "character": "rough", "box": [120, 60, 220, 140] }] }
            });
            if let Some(p) = peaks {
                v["pins"]["regions"][1]["peaks"] = serde_json::json!(p);
            }
            let c: Config = serde_json::from_value(v).unwrap();
            let w = World::build(&c).unwrap();
            let page = Page::new(&c, &c.variants[0]);
            let symbols = relief(
                &w,
                &page,
                c.seed,
                &ReliefOptions::default(),
                &Peakless::of(&w, &c),
            );
            let inside = |p: [f64; 2]| {
                let q = page.to_world(p);
                (125.0..=215.0).contains(&q[0]) && (65.0..=135.0).contains(&q[1])
            };
            let of = |k: Symbol| symbols.iter().filter(|s| s.0 == k && inside(s.1)).count();
            (of(Symbol::Mountain), of(Symbol::Hill))
        };
        let (mountains, hills) = count(None);
        assert_eq!(mountains, 0);
        assert!(hills > 10, "{hills}");
        let (mountains, _) = count(Some(true));
        assert!(mountains > 0, "peaks: true gives the ridge peaks");
    }

    #[test]
    fn a_rolling_area_has_no_ridge_peaks_unless_it_asks() {
        use inkland_core::config::Config;
        let ridges = |peaks: Option<bool>| {
            let mut v = serde_json::json!({
                "version": 1, "seed": 6, "frame": { "width": 300, "height": 200 },
                "world": { "cells": 4000, "template": "high-island" },
                "pins": { "areas": [
                    { "id": "downs", "character": "rolling", "box": [60, 40, 240, 160] }
                ] }
            });
            if let Some(p) = peaks {
                v["pins"]["areas"][0]["peaks"] = serde_json::json!(p);
            }
            let c: Config = serde_json::from_value(v).unwrap();
            let w = World::build(&c).unwrap();
            let t = relief_terrain(&w, &Peakless::of(&w, &c));
            (0..w.grid.len())
                .filter(|&k| {
                    let [x, y] = w.grid.points[k];
                    t.ridge[k] && (70.0..=230.0).contains(&x) && (50.0..=150.0).contains(&y)
                })
                .count()
        };
        assert_eq!(ridges(None), 0);
        assert!(ridges(Some(true)) > 0);
    }

    #[test]
    fn relief_terrain_finds_ridges_with_feet_and_clumps() {
        use inkland_core::config::Config;
        let c: Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 6, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 4000, "template": "high-island" }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let t = relief_terrain(&w, &Peakless::none());
        let ridges: Vec<usize> = (0..w.grid.len()).filter(|&k| t.ridge[k]).collect();
        assert!(ridges.len() > 5);
        for &k in &ridges {
            assert!(w.heights[k] >= w.sea_level + RIDGE_HEIGHT);
            assert!(!t.foot[k]);
        }
        // Each foot cell touches a ridge.
        assert!(
            (0..w.grid.len())
                .filter(|&k| t.foot[k])
                .all(|k| w.grid.neighbors(k).any(|m| t.ridge[m]))
        );
        assert!(t.crest.iter().all(|v| (0.0..=1.0).contains(v)));
        let (lo, hi) = t
            .clump
            .iter()
            .fold((1.0f64, -1.0f64), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(lo < -0.3 && hi > 0.3);
        assert_eq!(relief_terrain(&w, &Peakless::none()), t);
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
