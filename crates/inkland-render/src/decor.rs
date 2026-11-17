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
pub fn relief(world: &World, page: &Page, seed: u64) -> Vec<(Symbol, [f64; 2])> {
    let g = &world.grid;
    let mut rng = Rand::new(SeedTree::new(seed).part_rng(Layer::Decoration, "relief"));
    let mut order: Vec<(u64, usize)> = (0..g.len())
        .map(|c| ((rng.unit() * (1u64 << 52) as f64) as u64, c))
        .collect();
    order.sort();
    let [x0, y0, x1, y1] = page.view;
    let bucket = 4.0;
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
                        let gap = s.spacing().max(sym.spacing());
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
