//! Town maps: the plan of a town with its wards, buildings, streets, and
//! walls.
//!
//! This module is a port of TownGeneratorOS by Watabou
//! (<https://github.com/watabou/TownGeneratorOS>), the open version of the
//! Medieval Fantasy City Generator. TownGeneratorOS is licensed under the
//! GNU General Public License, version 3. This port is part of inkland and
//! is licensed under the GNU General Public License, version 3 or any later
//! version. THIRD_PARTY_NOTICES.md keeps the notice of the original.
//!
//! The town is in its own units: the centre of the town is at the origin,
//! and a main street is 2 units wide.

pub mod model;
pub mod poly;
pub mod random;
pub mod voronoi;
pub mod wards;

pub use poly::{P, Poly};
pub use wards::Kind;

use crate::error::{Error, ErrorCode};

/// The options of a town.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TownOptions {
    /// The number of patches of the town: 6 for a small town, 10 for a
    /// large town, 15 for a small city, 24 for a large city, and 40 for a
    /// metropolis.
    pub patches: usize,
    /// The seed of the town.
    pub seed: i64,
    /// A plaza with a market in the middle. `None`: a random choice.
    pub plaza: Option<bool>,
    /// A castle. `None`: a random choice.
    pub citadel: Option<bool>,
    /// A wall around the town. `None`: a random choice.
    pub walls: Option<bool>,
}

/// A patch of a town: a ward and its buildings.
#[derive(Debug, Clone, PartialEq)]
pub struct TownPatch {
    pub kind: Kind,
    pub shape: Poly,
    pub buildings: Vec<Poly>,
    pub within_city: bool,
    pub within_walls: bool,
}

/// A wall of a town.
#[derive(Debug, Clone, PartialEq)]
pub struct TownWall {
    pub shape: Poly,
    pub gates: Vec<P>,
    pub towers: Vec<P>,
}

/// The plan of a town.
#[derive(Debug, Clone, PartialEq)]
pub struct Town {
    pub patches: Vec<TownPatch>,
    pub wall: Option<TownWall>,
    pub castle: Option<TownWall>,
    /// The streets inside the town and the roads out of it, joined, with
    /// no segment twice.
    pub arteries: Vec<Poly>,
    /// The roads out of the town from its gates.
    pub roads: Vec<Poly>,
    /// The distance of the farthest vertex of the town from its centre.
    pub radius: f64,
}

/// Make the plan of a town.
pub fn generate(opts: &TownOptions) -> Result<Town, Error> {
    let m = model::Model::new(
        opts.patches,
        opts.seed,
        opts.plaza,
        opts.citadel,
        opts.walls,
    )
    .map_err(|e| {
        Error::new(
            ErrorCode::Internal,
            "",
            format!("the town generator could not build a town: {}", e.0),
        )
    })?;
    let wall_of = |w: &model::CurtainWall| TownWall {
        shape: m.pts(&w.shape),
        gates: m.pts(&w.gates),
        towers: m.pts(&w.towers),
    };
    let patches = m
        .patches
        .iter()
        .map(|&p| {
            let s = &m.store[p];
            let ward = s.ward.as_ref();
            TownPatch {
                kind: ward.map_or(Kind::Empty, |w| w.kind),
                shape: m.pts(&s.shape),
                buildings: ward.map(|w| w.geometry.clone()).unwrap_or_default(),
                within_city: s.within_city,
                within_walls: s.within_walls,
            }
        })
        .collect();
    let castle = m
        .citadel
        .and_then(|c| m.store[c].ward.as_ref())
        .and_then(|w| w.wall.as_ref())
        .map(wall_of);
    Ok(Town {
        patches,
        wall: m.wall().map(wall_of),
        castle,
        arteries: m.arteries.iter().map(|a| m.pts(a)).collect(),
        roads: m.roads.iter().map(|a| m.pts(a)).collect(),
        radius: m.city_radius,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn town(seed: i64, patches: usize, walls: Option<bool>, citadel: Option<bool>) -> Town {
        generate(&TownOptions {
            patches,
            seed,
            plaza: Some(true),
            citadel,
            walls,
        })
        .unwrap()
    }

    #[test]
    fn a_town_has_wards_with_buildings_streets_and_a_wall() {
        let t = town(4242, 15, Some(true), Some(true));
        let inside: Vec<&TownPatch> = t.patches.iter().filter(|p| p.within_city).collect();
        assert!(inside.len() >= 15, "{}", inside.len());
        let buildings: usize = inside.iter().map(|p| p.buildings.len()).sum();
        assert!(buildings > 100, "{buildings}");
        assert!(inside.iter().any(|p| p.kind == Kind::Market));
        assert!(inside.iter().any(|p| p.kind == Kind::Castle));
        let wall = t.wall.as_ref().expect("a wall");
        assert!(!wall.gates.is_empty() && !wall.towers.is_empty());
        assert!(t.castle.is_some());
        assert!(!t.arteries.is_empty());
        assert!(t.radius > 0.0);
        // Each building is a polygon with an area and finite points.
        for p in &t.patches {
            for b in &p.buildings {
                assert!(b.len() >= 3);
                assert!(b.iter().all(|v| v[0].is_finite() && v[1].is_finite()));
            }
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_town_and_another_seed_another() {
        let a = town(7, 10, None, None);
        assert_eq!(a, town(7, 10, None, None));
        assert_ne!(a, town(8, 10, None, None));
    }

    #[test]
    fn many_seeds_and_sizes_build_a_town() {
        for seed in 1..40 {
            for &n in &[6, 10, 15, 24, 40] {
                let t = generate(&TownOptions {
                    patches: n,
                    seed,
                    plaza: None,
                    citadel: None,
                    walls: None,
                })
                .unwrap_or_else(|e| panic!("seed {seed}, {n} patches: {e:?}"));
                assert!(t.patches.iter().filter(|p| p.within_city).count() >= n);
            }
        }
    }

    #[test]
    fn an_open_town_has_no_wall() {
        let t = town(99, 10, Some(false), Some(false));
        assert!(t.wall.is_none());
        assert!(t.castle.is_none());
        assert!(!t.patches.iter().any(|p| p.kind == Kind::Castle));
    }
}
