//! Character areas (`pins.areas`): soft areas that steer the land.
//!
//! An area has a loose outline: fractal noise moves its edge. Inside, its
//! weight is its `strength`. Outside, the weight fades to 0 over its
//! `edge`. An area changes only land cells, so the coast stays where it
//! is. The region pins come after the areas, so a region rules over an
//! area. The noise comes from the seed tree.

use crate::config::{AreaPin, Character, Config, Point};
use crate::geom::{Shape, dist, segment_distance};
use crate::grid::Grid;
use crate::noise::Fractal;
use crate::seed::{Layer, SeedTree};

/// An area with its shape and its noise.
pub struct Area<'a> {
    pub pin: &'a AreaPin,
    shape: Shape,
    /// The bounding box, grown by the edge and the noise.
    reach: [f64; 4],
    edge: f64,
    strength: f64,
    /// How far the noise moves the outline, in units.
    loose: f64,
    outline: Fractal,
    /// The noise of the land inside.
    relief: Fractal,
}

impl<'a> Area<'a> {
    /// The areas of a config. `spacing` is the mean distance between two
    /// cells, which sets the size of the hills.
    pub fn all(config: &'a Config, spacing: f64) -> Vec<Area<'a>> {
        let tree = SeedTree::new(config.seed);
        config
            .pins
            .areas
            .iter()
            .filter_map(|pin| {
                let shape = Shape::from_fields(&pin.polygon, &pin.rect, &pin.circle)?;
                let [x0, y0, x1, y1] = shape.bbox();
                let size = ((x1 - x0).max(0.0) * (y1 - y0).max(0.0)).sqrt();
                let edge = pin.edge.unwrap_or(size / 5.0).max(0.0);
                let loose = size / 15.0;
                let grow = edge + loose;
                let part = |name: &str| {
                    Fractal::seed_of(tree.part_seed(Layer::Terrain, &format!("{name}/{}", pin.id)))
                };
                let hills = match pin.character {
                    Character::Rough => 5.0,
                    _ => 8.0,
                };
                Some(Area {
                    pin,
                    reach: [x0 - grow, y0 - grow, x1 + grow, y1 + grow],
                    edge,
                    strength: pin.strength.unwrap_or(1.0).clamp(0.0, 1.0),
                    loose,
                    outline: Fractal::new(part("area-outline"), 3, 4.0 * loose.max(1e-9)),
                    relief: Fractal::new(part("area-relief"), 4, hills * spacing),
                    shape,
                })
            })
            .collect()
    }

    /// The weight of the area at `p`, from 0 to its strength.
    pub fn weight(&self, p: Point) -> f64 {
        let [x0, y0, x1, y1] = self.reach;
        if p[0] < x0 || p[0] > x1 || p[1] < y0 || p[1] > y1 {
            return 0.0;
        }
        let d = match &self.shape {
            Shape::Circle { at, r } => (dist(*at, p) - r).abs(),
            Shape::Polygon(poly) => (0..poly.len())
                .map(|i| segment_distance(poly[i], poly[(i + 1) % poly.len()], p))
                .fold(f64::INFINITY, f64::min),
        };
        let signed = if self.shape.contains(p) { -d } else { d };
        let s = signed - self.loose * (2.0 * self.outline.at(p)).clamp(-1.0, 1.0);
        let w = if s <= 0.0 {
            1.0
        } else if self.edge > 0.0 && s < self.edge {
            let t = 1.0 - s / self.edge;
            t * t * (3.0 - 2.0 * t)
        } else {
            0.0
        };
        w * self.strength
    }

    /// The height above sea level that the character gives to land with
    /// the height `above` sea level at `p`.
    fn target(&self, p: Point, above: f64) -> f64 {
        let n = self.relief.at(p);
        match self.pin.character {
            // Sharp ridges along the lines where the noise is 0.
            Character::Rough => {
                let ridged = (1.0 - (2.0 * n).abs()).clamp(0.0, 1.0);
                1.3 * above + 10.0 + 28.0 * ridged * ridged
            }
            Character::Rolling => 6.0 + 10.0 * (0.5 + n).clamp(0.0, 1.0) + 0.3 * above,
            Character::Flat => 1.0 + 0.2 * above,
            Character::Wooded => above,
        }
    }
}

/// Steer the heights of the land cells by the areas. The result is a whole
/// number from 0 to 100, and land stays land.
pub fn apply_heights(grid: &Grid, heights: &mut [f64], areas: &[Area], sea_level: f64) {
    if areas.is_empty() {
        return;
    }
    for (c, h) in heights.iter_mut().enumerate() {
        if *h < sea_level {
            continue;
        }
        let p = grid.points[c];
        let mut above = *h - sea_level;
        for a in areas {
            let w = a.weight(p);
            if w > 0.0 {
                above += w * (a.target(p, above) - above);
            }
        }
        *h = (sea_level + above.max(0.0)).trunc().clamp(sea_level, 100.0);
    }
}

/// Make the land cells of the `wooded` areas forest, where the weight of
/// the area is at least a half. A cell in a region pin keeps its biome.
/// `temps` gives the temperature of each cell, which picks the forest.
pub fn apply_biomes(
    grid: &Grid,
    heights: &[f64],
    biomes: &mut [u8],
    temps: &[f64],
    areas: &[Area],
    regions: &[crate::fit::FitRegion],
    sea_level: f64,
) {
    let wooded: Vec<&Area> = areas
        .iter()
        .filter(|a| a.pin.character == Character::Wooded)
        .collect();
    if wooded.is_empty() {
        return;
    }
    for (c, b) in biomes.iter_mut().enumerate() {
        let p = grid.points[c];
        if heights[c] < sea_level || wooded.iter().all(|a| a.weight(p) < 0.5) {
            continue;
        }
        if !crate::fit::ruling_at(regions, p).is_empty() {
            continue;
        }
        if let Some(forest) = crate::biomes::forced(crate::config::RegionKind::Forest, temps[c]) {
            *b = forest;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config(areas: serde_json::Value) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 3, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 4000, "template": "pangea" },
            "pins": { "areas": areas }
        }))
        .unwrap()
    }

    #[test]
    fn the_weight_is_full_inside_and_fades_out_over_the_edge() {
        let c = config(json!([
            { "id": "a", "character": "rough", "box": [100, 50, 200, 150], "edge": 20 }
        ]));
        let areas = Area::all(&c, 3.9);
        let a = &areas[0];
        assert_eq!(a.weight([150.0, 100.0]), 1.0);
        let mid = a.weight([215.0, 100.0]);
        assert!(mid < 1.0, "{mid}");
        assert_eq!(a.weight([260.0, 100.0]), 0.0);
    }

    /// The mean height above sea level of the land cells in the middle of
    /// the frame.
    fn mean_land(areas: serde_json::Value) -> (f64, f64) {
        let w = crate::world::World::build(&config(areas)).unwrap();
        let hs: Vec<f64> = (0..w.base.grid.len())
            .filter(|&k| {
                let [x, y] = w.base.grid.points[k];
                (110.0..190.0).contains(&x)
                    && (60.0..140.0).contains(&y)
                    && w.base.heights[k] >= 20.0
            })
            .map(|k| w.base.heights[k] - 20.0)
            .collect();
        let mean = hs.iter().sum::<f64>() / hs.len() as f64;
        let var = hs.iter().map(|h| (h - mean).powi(2)).sum::<f64>() / hs.len() as f64;
        (mean, var.sqrt())
    }

    #[test]
    fn rough_raises_the_land_and_flat_lowers_it() {
        let shape = json!([100, 50, 200, 150]);
        let (base, _) = mean_land(json!([]));
        let (rough, rough_sd) =
            mean_land(json!([{ "id": "a", "character": "rough", "box": shape }]));
        let (flat, flat_sd) = mean_land(json!([{ "id": "a", "character": "flat", "box": shape }]));
        let (wooded, _) = mean_land(json!([{ "id": "a", "character": "wooded", "box": shape }]));
        assert!(rough > base + 5.0, "{rough} {base}");
        assert!(flat < base, "{flat} {base}");
        assert!(rough_sd > flat_sd);
        assert_eq!(wooded, base);
    }

    #[test]
    fn a_wooded_area_makes_forest() {
        let forest = |b: u8| (5..=9).contains(&b);
        let count = |areas: serde_json::Value| {
            let w = crate::world::World::build(&config(areas)).unwrap();
            (0..w.grid.len())
                .filter(|&k| {
                    let [x, y] = w.grid.points[k];
                    (110.0..190.0).contains(&x)
                        && (60.0..140.0).contains(&y)
                        && w.heights[k] >= 20.0
                })
                .map(|k| (forest(w.biomes[k]), 1))
                .fold((0, 0), |(f, n), (is, one)| (f + usize::from(is), n + one))
        };
        let (before, _) = count(json!([]));
        let (after, land) = count(json!([
            { "id": "greenwood", "character": "wooded", "box": [100, 50, 200, 150] }
        ]));
        assert!(after > before);
        assert_eq!(after, land, "each land cell inside is forest");
    }

    #[test]
    fn an_area_does_not_move_the_coast() {
        let a = crate::world::World::build(&config(json!([]))).unwrap();
        let b = crate::world::World::build(&config(json!([
            { "id": "a", "character": "flat", "box": [0, 0, 300, 200] }
        ])))
        .unwrap();
        let land = |w: &crate::world::World| {
            (0..w.base.grid.len())
                .map(|k| w.base.heights[k] >= 20.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(land(&a), land(&b));
    }
}
