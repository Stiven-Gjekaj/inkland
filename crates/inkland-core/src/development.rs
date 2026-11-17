//! Development (`development-generator.ts`): a level from 0 to 100 for each
//! land cell with people.
//!
//! Each land cell starts with the level of its culture and its culture
//! type, plus the development of its biome, plus 1 on a river and 1 on the
//! coast. Two passes average each cell with its land neighbors. Value noise
//! in two octaves adds up to 9 points. A place gets at least 5.

use crate::cultures::{Culture, CultureType, DEFAULT_DEVELOPMENT};
use crate::grid::Grid;
use crate::population::Cells;

/// What a biome adds to the development of its people.
pub const BIOME_DEVELOPMENT: [f64; 13] = [
    0.0, -4.0, -5.0, -1.0, 0.0, -1.0, 2.0, -3.0, 0.0, -3.0, -5.0, -8.0, -2.0,
];
/// The development of the wildlands.
pub const WILDLANDS_DEVELOPMENT: f64 = 3.0;
/// The level below which a people lives before the Bronze Age.
pub const TRIBAL_DEVELOPMENT: u8 = 12;

const SMOOTHING: usize = 2;
const NOISE: f64 = 9.0;
const WAVES: f64 = 3.0;
const MIN_BURG: f64 = 5.0;

/// The eras of FMG and the level at which each begins.
pub const ERAS: [(u8, &str); 21] = [
    (1, "Paleolithic"),
    (3, "Mesolithic"),
    (6, "Neolithic"),
    (9, "Copper Age"),
    (12, "Bronze Age"),
    (15, "Late Bronze Age"),
    (18, "Iron Age"),
    (21, "Classical Age"),
    (24, "Late Antiquity"),
    (27, "Early Middle Ages"),
    (30, "High Middle Ages"),
    (33, "Commercial Revolution"),
    (36, "Late Middle Ages"),
    (40, "Renaissance"),
    (45, "Age of Discovery"),
    (50, "Scientific Revolution"),
    (60, "Enlightenment"),
    (70, "Industrial Revolution"),
    (80, "Machine Age"),
    (90, "Space Age"),
    (100, "Post-human"),
];

/// The era of a level (`getEra`).
pub fn era(level: u8) -> &'static str {
    let mut name = ERAS[0].1;
    for &(at, n) in &ERAS {
        if level >= at {
            name = n;
        }
    }
    name
}

fn type_development(kind: CultureType) -> f64 {
    match kind {
        CultureType::Generic => 0.0,
        CultureType::River => 2.0,
        CultureType::Naval => 3.0,
        CultureType::Lake => 1.0,
        CultureType::Highland => -2.0,
        CultureType::Nomadic => -3.0,
        CultureType::Hunting => -4.0,
    }
}

/// FNV-1a of a text, as `hashString` of FMG.
fn hash_string(text: &str) -> u32 {
    let mut hash: u32 = 2_166_136_261;
    for unit in text.encode_utf16() {
        hash = (hash ^ u32::from(unit)).wrapping_mul(16_777_619);
    }
    hash
}

/// The development of each cell (`generate`). `burg` marks the cells with a
/// place.
pub fn generate(
    grid: &Grid,
    cells: &Cells,
    biomes: &[u8],
    cultures: &[Culture],
    cell_culture: &[u32],
    burg: &[bool],
    seed: u64,
) -> Vec<u8> {
    let n = grid.len();
    let level = |culture: u32| -> f64 {
        match culture {
            0 => WILDLANDS_DEVELOPMENT,
            k => cultures
                .get(k as usize - 1)
                .map_or(WILDLANDS_DEVELOPMENT, |c| {
                    f64::from(if c.development == 0 {
                        DEFAULT_DEVELOPMENT
                    } else {
                        c.development
                    }) + type_development(c.kind)
                }),
        }
    };
    // FMG keeps the field in a Float32Array.
    let mut field = vec![0f32; n];
    for c in 0..n {
        if !cells.land(c) {
            continue;
        }
        let water =
            f64::from(u8::from(cells.r[c] != 0)) + f64::from(u8::from(cells.harbor[c] != 0));
        field[c] = (level(cell_culture[c]) + BIOME_DEVELOPMENT[biomes[c] as usize] + water) as f32;
    }
    for _ in 0..SMOOTHING {
        let mut next = vec![0f32; n];
        for c in 0..n {
            if !cells.land(c) {
                continue;
            }
            let mut sum = f64::from(field[c]);
            let mut count = 1.0;
            for m in grid.neighbors(c) {
                if cells.land(m) {
                    sum += f64::from(field[m]);
                    count += 1.0;
                }
            }
            next[c] = (sum / count) as f32;
        }
        field = next;
    }
    let noise = Noise::new(grid, seed);
    let mut out = vec![0u8; n];
    for c in 0..n {
        if !cells.land(c) || (cells.pop[c] == 0.0 && !burg[c]) {
            continue;
        }
        let least = if burg[c] { MIN_BURG } else { 1.0 };
        let v = crate::rand::js_round(f64::from(field[c]) + noise.at(grid.points[c]));
        out[c] = v.min(100.0).max(least) as u8;
    }
    out
}

/// Value noise in two octaves (`getNoise`).
struct Noise {
    min: [f64; 2],
    size: f64,
    seed: u32,
}

impl Noise {
    fn new(grid: &Grid, seed: u64) -> Noise {
        let (mut min, mut max) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in &grid.points {
            for k in 0..2 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        let size = (max[0] - min[0]).max(max[1] - min[1]);
        Noise {
            min,
            size: if size == 0.0 { 1.0 } else { size },
            seed: hash_string(&seed.to_string()),
        }
    }

    fn lattice(&self, x: i64, y: i64) -> f64 {
        let mut h =
            self.seed ^ (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
        h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
        f64::from(h ^ (h >> 16)) / 4_294_967_295.0 * 2.0 - 1.0
    }

    fn value(&self, x: f64, y: f64) -> f64 {
        let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
        let (ix, iy) = (x.floor(), y.floor());
        let (fx, fy) = (smooth(x - ix), smooth(y - iy));
        let (ix, iy) = (ix as i64, iy as i64);
        let top = self.lattice(ix, iy) + (self.lattice(ix + 1, iy) - self.lattice(ix, iy)) * fx;
        let bottom = self.lattice(ix, iy + 1)
            + (self.lattice(ix + 1, iy + 1) - self.lattice(ix, iy + 1)) * fx;
        top + (bottom - top) * fy
    }

    fn at(&self, p: [f64; 2]) -> f64 {
        let x = (p[0] - self.min[0]) / self.size * WAVES;
        let y = (p[1] - self.min[1]) / self.size * WAVES;
        NOISE * (0.67 * self.value(x, y) + 0.33 * self.value(x * 2.0 + 17.0, y * 2.0 + 17.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eras_and_hash_follow_fmg() {
        assert_eq!(era(0), "Paleolithic");
        assert_eq!(era(33), "Commercial Revolution");
        assert_eq!(era(100), "Post-human");
        assert_eq!(hash_string("a"), 0xe40c_292c);
    }

    #[test]
    fn people_have_development_and_empty_land_has_none() {
        let c: crate::config::Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 2, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "continents" }
        }))
        .unwrap();
        let w = crate::world::World::build(&c).unwrap();
        let s = &w.society;
        for cell in 0..w.grid.len() {
            let d = s.development[cell];
            let peopled = s.cells.land(cell) && s.cells.pop[cell] > 0.0;
            if !s.cells.land(cell) {
                assert_eq!(d, 0);
            } else if peopled {
                assert!((1..=100).contains(&d));
            }
        }
        for p in &s.places {
            assert!(s.development[p.cell] >= 5, "{}", p.name);
        }
    }
}
