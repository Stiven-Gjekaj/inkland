//! Ice: glaciers on cold land and icebergs on cold water.
//!
//! This follows Azgaar's Fantasy Map Generator (FMG,
//! `model/physical/ice-generator.ts`). It runs on the first grid:
//!
//! - A glacier is an outline around the land cells with a temperature of
//!   -8 C or less.
//! - Each ocean cell with a temperature of 0 C or less gets an iceberg with
//!   a chance of 1 in 5. The colder the water, the bigger the iceberg. An
//!   iceberg next to the coast is smaller. An iceberg is the polygon of its
//!   cell, scaled toward the site of the cell.
//!
//! The ice is data only. No layer draws it in this version.

use crate::features::{FeatureKind, Features, outlines};
use crate::grid::Grid;
use crate::rand::Rand;

/// The highest temperature of a glacier cell, in degrees C.
pub const GLACIER_MAX_TEMP: f64 = -8.0;
/// The highest temperature of an iceberg cell, in degrees C.
pub const ICEBERG_MAX_TEMP: f64 = 0.0;

/// The kind of a piece of ice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceKind {
    Glacier,
    Iceberg,
}

impl IceKind {
    pub fn name(self) -> &'static str {
        match self {
            IceKind::Glacier => "glacier",
            IceKind::Iceberg => "iceberg",
        }
    }
}

/// One glacier or iceberg.
#[derive(Debug, Clone, PartialEq)]
pub struct Ice {
    /// 1, 2, 3, ...: the glaciers first, then the icebergs.
    pub id: u32,
    pub kind: IceKind,
    /// The closed outline, in units. The first and the last point are the
    /// same.
    pub points: Vec<[f64; 2]>,
    /// The first-grid cell of an iceberg.
    pub cell: Option<usize>,
    /// The size of an iceberg: 0.1 to 1 of its cell.
    pub size: Option<f64>,
}

/// Round to `d` decimals, as FMG `rn`.
fn rn(v: f64, d: i32) -> f64 {
    let m = libm::pow(10.0, f64::from(d));
    (v * m).round() / m
}

/// Make the ice of the first grid.
pub fn generate(
    grid: &Grid,
    heights: &[f64],
    sea_level: f64,
    features: &Features,
    temperature: &[f64],
    rng: &mut Rand,
) -> Vec<Ice> {
    let mut ice = Vec::new();
    let clip = |p: [f64; 2]| [p[0].clamp(0.0, grid.width), p[1].clamp(0.0, grid.height)];

    // Glaciers.
    let cold_land = |c: usize| heights[c] >= sea_level && temperature[c] <= GLACIER_MAX_TEMP;
    for ring in outlines(grid, &cold_land) {
        if ring.len() < 4 {
            continue;
        }
        ice.push(Ice {
            id: ice.len() as u32 + 1,
            kind: IceKind::Glacier,
            points: ring
                .iter()
                .map(|&v| clip(grid.vertices[v as usize]))
                .collect(),
            cell: None,
            size: None,
        });
    }

    // Icebergs.
    let coldest = temperature.iter().copied().fold(f64::INFINITY, f64::min);
    let coast = crate::lakes::grid_distance(grid, heights, sea_level);
    for c in 0..grid.len() {
        let t = temperature[c];
        if heights[c] >= sea_level || t > ICEBERG_MAX_TEMP {
            continue;
        }
        if features.get(features.of_cell[c]).kind == FeatureKind::Lake {
            continue;
        }
        if rng.chance(0.8) {
            continue;
        }
        let factor = 0.8 + rng.unit() * 0.4;
        let normal = if coldest < 1.0 {
            ((t - coldest) / (1.0 - coldest)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut base = (1.0 - normal) * 0.8;
        if coast[c] == -1 {
            base /= 1.3;
        }
        let size = rn(base * factor, 2).clamp(0.1, 1.0);
        let [cx, cy] = grid.points[c];
        let mut points: Vec<[f64; 2]> = grid.polygons[c]
            .iter()
            .map(|&v| {
                let [x, y] = grid.vertices[v as usize];
                [cx + (x - cx) * size, cy + (y - cy) * size]
            })
            .collect();
        points.push(points[0]);
        ice.push(Ice {
            id: ice.len() as u32 + 1,
            kind: IceKind::Iceberg,
            points,
            cell: Some(c),
            size: Some(size),
        });
    }
    ice
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::{Layer, SeedTree};

    fn grid() -> Grid {
        Grid::new(
            400.0,
            300.0,
            3000,
            &mut Rand::new(SeedTree::new(8).part_rng(Layer::Terrain, "grid")),
        )
    }

    #[test]
    fn cold_land_has_glaciers_and_cold_sea_has_icebergs() {
        let g = grid();
        // Land in the middle, sea around it. The north third is very cold.
        let h: Vec<f64> = g
            .points
            .iter()
            .map(|p| {
                if (p[0] - 200.0).abs() < 100.0 {
                    40.0
                } else {
                    5.0
                }
            })
            .collect();
        let t: Vec<f64> = g
            .points
            .iter()
            .map(|p| if p[1] < 100.0 { -20.0 } else { 10.0 })
            .collect();
        let f = Features::mark(&g, &h, 20.0);
        let mut rng = Rand::new(SeedTree::new(8).part_rng(Layer::Climate, "ice"));
        let ice = generate(&g, &h, 20.0, &f, &t, &mut rng);
        let glaciers: Vec<&Ice> = ice.iter().filter(|i| i.kind == IceKind::Glacier).collect();
        let bergs: Vec<&Ice> = ice.iter().filter(|i| i.kind == IceKind::Iceberg).collect();
        assert_eq!(glaciers.len(), 1);
        let gl = glaciers[0];
        assert_eq!(gl.points.first(), gl.points.last());
        assert!(
            gl.points
                .iter()
                .all(|p| p[1] <= 110.0 && (90.0..=310.0).contains(&p[0]))
        );
        // About one in five cold sea cells has an iceberg.
        let cold_sea = (0..g.len()).filter(|&c| h[c] < 20.0 && t[c] <= 0.0).count();
        let share = bergs.len() as f64 / cold_sea as f64;
        assert!((0.1..0.3).contains(&share), "{share}");
        for b in &bergs {
            let c = b.cell.unwrap();
            assert!(h[c] < 20.0 && t[c] <= 0.0);
            let size = b.size.unwrap();
            assert!((0.1..=1.0).contains(&size));
            assert_eq!(b.points.len(), g.polygons[c].len() + 1);
        }
        // The ids run from 1.
        let ids: Vec<u32> = ice.iter().map(|i| i.id).collect();
        assert_eq!(ids, (1..=ice.len() as u32).collect::<Vec<_>>());
    }

    #[test]
    fn warm_worlds_have_no_ice() {
        let g = grid();
        let h = vec![40.0; g.len()];
        let t = vec![15.0; g.len()];
        let f = Features::mark(&g, &h, 20.0);
        let mut rng = Rand::new(SeedTree::new(8).part_rng(Layer::Climate, "ice"));
        assert!(generate(&g, &h, 20.0, &f, &t, &mut rng).is_empty());
    }
}
