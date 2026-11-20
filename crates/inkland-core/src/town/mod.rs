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

pub mod districts;
pub mod model;
pub mod poly;
pub mod random;
pub mod voronoi;
pub mod wards;

pub use districts::District;
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
    /// inkland: narrow lots along the streets, with yards in the blocks.
    /// `false`: the buildings of the original generator.
    pub lots: bool,
    /// inkland: the direction of the sea from the centre of the town, in
    /// radians (0 east, a quarter turn south), or `None` for no sea.
    pub coast: Option<f64>,
    /// inkland: the direction in which a river flows through the town, in
    /// radians, or `None` for no river.
    pub river: Option<f64>,
    /// inkland: the index of the name base for the names of the
    /// districts.
    pub names: usize,
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
    /// For each edge of the shape, `true` if the wall stands there. A wall
    /// has no part along the shore.
    pub segments: Vec<bool>,
    pub gates: Vec<P>,
    pub towers: Vec<P>,
}

/// A bridge: its middle and the direction of the street over it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bridge {
    pub at: P,
    pub dir: P,
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
    /// inkland: the sea patches.
    pub water: Vec<Poly>,
    /// inkland: the river, from upstream.
    pub river: Option<Poly>,
    /// inkland: the bridges of the streets over the river.
    pub bridges: Vec<Bridge>,
    /// inkland: the piers into the sea.
    pub piers: Vec<Poly>,
    /// inkland: the districts, with their names.
    pub districts: Vec<District>,
}

/// The options of the town map of a place: the number of patches from its
/// population, as the size of the city generator of FMG, at most 40; a
/// seed from the map seed and the place id; and the plaza, the castle, and
/// the walls of the place.
pub fn options_of(world: &crate::world::World, place: &crate::society::Place) -> TownOptions {
    use crate::military::POPULATION_SCALE;
    let size = libm::ceil(
        2.13 * libm::pow(
            place.population * POPULATION_SCALE / crate::watabou::URBAN_DENSITY,
            0.385,
        ),
    );
    let patches = size.clamp(6.0, 40.0) as usize;
    // As the seed of the town links: the map seed, then the id with 4
    // digits, in the range of the generator.
    let n: u128 = 2_147_483_647;
    let seed = ((u128::from(world.seed) % n) * 10_000 + u128::from(place.id)) % n;
    TownOptions {
        patches,
        seed: seed as i64,
        plaza: Some(place.plaza),
        citadel: Some(place.citadel),
        walls: Some(place.walls),
        lots: true,
        coast: coast_of(world, place),
        river: river_of(world, place),
        names: world
            .society
            .cultures
            .iter()
            .find(|c| c.id == place.culture)
            .map_or(0, |c| c.base),
    }
}

/// inkland: the direction of the sea from a port, from its cell to the
/// water cell next to it.
fn coast_of(world: &crate::world::World, place: &crate::society::Place) -> Option<f64> {
    let haven = world.society.cells.haven[place.cell]?;
    if !place.port {
        return None;
    }
    let (a, b) = (world.grid.points[place.cell], world.grid.points[haven]);
    Some(libm::atan2(b[1] - a[1], b[0] - a[0]))
}

/// inkland: the direction of the river through the cell of a place, from
/// the cell before it to the cell after it in the course of the river.
fn river_of(world: &crate::world::World, place: &crate::society::Place) -> Option<f64> {
    let c = place.cell;
    let r = world.rivers.iter().find(|r| r.cells.contains(&c))?;
    let i = r.cells.iter().position(|&x| x == c)?;
    let before = r.cells[i.saturating_sub(1)];
    let after = r.cells.get(i + 1).copied().unwrap_or(c);
    let (a, b) = (world.grid.points[before], world.grid.points[after]);
    if a == b {
        return None;
    }
    Some(libm::atan2(b[1] - a[1], b[0] - a[0]))
}

/// Make the plan of a town.
pub fn generate(opts: &TownOptions) -> Result<Town, Error> {
    let m = model::Model::new(
        opts.patches,
        opts.seed,
        opts.plaza,
        opts.citadel,
        opts.walls,
        opts.lots,
        (opts.coast, opts.river),
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
        segments: w.segments.clone(),
        gates: m.pts(&w.gates),
        towers: m.pts(&w.towers),
    };
    let patches = m
        .patches
        .iter()
        .filter(|&&p| !m.store[p].water)
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
    let mut town = Town {
        patches,
        wall: m.wall().map(wall_of),
        castle,
        arteries: m.arteries.iter().map(|a| m.pts(a)).collect(),
        roads: m.roads.iter().map(|a| m.pts(a)).collect(),
        radius: m.city_radius,
        water: m
            .patches
            .iter()
            .filter(|&&p| m.store[p].water)
            .map(|&p| m.shape_pts(p))
            .collect(),
        river: (!m.river.is_empty()).then(|| m.pts(&m.river)),
        bridges: bridges(&m),
        piers: m.piers.clone(),
        districts: Vec::new(),
    };
    town.districts = districts::districts(&town, opts.names, opts.seed);
    Ok(town)
}

/// The bridges: each vertex of the river that a street or a road goes
/// through.
fn bridges(m: &model::Model) -> Vec<Bridge> {
    let mut out: Vec<Bridge> = Vec::new();
    for a in m.streets.iter().chain(&m.roads) {
        let n = a.len();
        for i in 0..n {
            // The river vertex, also at the end of a street that reaches it.
            if n >= 2 && m.river.contains(&a[i]) {
                let (p, q) = (a[i.saturating_sub(1)], a[(i + 1).min(n - 1)]);
                let d = poly::sub(m.v[q], m.v[p]);
                let l = poly::len(d).max(1e-9);
                let at = m.v[a[i]];
                if !out.iter().any(|b| b.at == at) {
                    out.push(Bridge {
                        at,
                        dir: poly::scale(d, 1.0 / l),
                    });
                }
            }
        }
    }
    out
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
            lots: false,
            coast: None,
            river: None,
            names: 1,
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
                    lots: false,
                    coast: None,
                    river: None,
                    names: 1,
                })
                .unwrap_or_else(|e| panic!("seed {seed}, {n} patches: {e:?}"));
                assert!(t.patches.iter().filter(|p| p.within_city).count() >= n);
            }
        }
    }

    #[test]
    fn a_place_gives_the_options_of_its_town() {
        let c: crate::config::Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 42, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "continents", "states": 4, "towns": 30 }
        }))
        .unwrap();
        let w = crate::world::World::build(&c).unwrap();
        let mut sizes = Vec::new();
        for p in &w.society.places {
            let o = options_of(&w, p);
            assert!((6..=40).contains(&o.patches));
            assert_eq!(o.walls, Some(p.walls));
            assert_eq!(o.citadel, Some(p.citadel));
            sizes.push(o.patches);
        }
        // A larger place gets a larger town.
        let big = w
            .society
            .places
            .iter()
            .max_by(|a, b| a.population.total_cmp(&b.population))
            .unwrap();
        let small = w
            .society
            .places
            .iter()
            .min_by(|a, b| a.population.total_cmp(&b.population))
            .unwrap();
        assert!(options_of(&w, big).patches > options_of(&w, small).patches);
        // Each town of the world builds.
        for p in w.society.places.iter().take(10) {
            generate(&options_of(&w, p)).unwrap();
        }
    }

    #[test]
    fn lots_give_more_and_smaller_buildings() {
        let make = |lots: bool| {
            generate(&TownOptions {
                patches: 15,
                seed: 4242,
                plaza: Some(true),
                citadel: Some(true),
                walls: Some(true),
                lots,
                coast: None,
                river: None,
                names: 1,
            })
            .unwrap()
        };
        let count = |t: &Town| t.patches.iter().map(|p| p.buildings.len()).sum::<usize>();
        let (classic, lots) = (make(false), make(true));
        assert!(
            count(&lots) > count(&classic) * 5 / 4,
            "{} {}",
            count(&lots),
            count(&classic)
        );
        assert_eq!(make(true), lots);
    }

    fn water_town(coast: Option<f64>, river: Option<f64>) -> Town {
        generate(&TownOptions {
            patches: 24,
            seed: 17170015,
            plaza: Some(true),
            citadel: Some(false),
            walls: Some(true),
            lots: true,
            coast,
            river,
            names: 1,
        })
        .unwrap()
    }

    #[test]
    fn each_district_has_a_name_of_its_own_and_a_label_inside() {
        let t = water_town(Some(0.0), Some(1.5));
        assert!(t.districts.len() >= 4, "{}", t.districts.len());
        let mut names = std::collections::BTreeSet::new();
        let mut seen = std::collections::BTreeSet::new();
        for d in &t.districts {
            assert!(!d.name.is_empty());
            assert!(names.insert(d.name.clone()), "{} twice", d.name);
            assert!(
                d.patches
                    .iter()
                    .any(|&i| districts::contains(&t.patches[i].shape, d.label)),
                "the label of {} is outside it",
                d.name
            );
            for &i in &d.patches {
                assert!(seen.insert(i), "a patch in two districts");
                assert!(!matches!(t.patches[i].kind, Kind::Market | Kind::Castle));
            }
        }
        // The sea gives the district on the shore a name of the water.
        assert!(
            t.districts
                .iter()
                .any(|d| ["Harbour", "Docks", "Quayside", "Wharf", "Quay"]
                    .iter()
                    .any(|w| d.name.contains(w))),
            "{:?}",
            t.districts.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        // The same options give the same names.
        assert_eq!(t.districts, water_town(Some(0.0), Some(1.5)).districts);
    }

    #[test]
    fn a_port_has_sea_on_its_side_piers_and_no_wall_on_the_shore() {
        // The sea to the east: +x.
        let t = water_town(Some(0.0), None);
        assert!(!t.water.is_empty());
        assert!(!t.piers.is_empty());
        for w in &t.water {
            assert!(poly::centroid(w)[0] > 0.0, "the sea is east");
        }
        let wall = t.wall.as_ref().unwrap();
        assert!(wall.segments.iter().any(|s| !s), "no wall on the shore");
        // No building stands in the sea.
        for p in &t.patches {
            for b in &p.buildings {
                let c = poly::center(b);
                assert!(!t.water.iter().any(|w| inside(w, c)), "{c:?}");
            }
        }
    }

    #[test]
    fn a_river_crosses_the_town_with_bridges_and_no_building_on_it() {
        let t = water_town(None, Some(0.0));
        let river = t.river.as_ref().expect("a river");
        // It flows from west to east, across the town.
        assert!(river[0][0] < 0.0 && river[river.len() - 1][0] > 0.0);
        assert!(!t.bridges.is_empty());
        // No building is on the line of the river.
        for p in &t.patches {
            for b in &p.buildings {
                for w in river.windows(2) {
                    let mid = poly::interpolate(w[0], w[1], 0.5);
                    assert!(!inside(b, mid), "a building on the river");
                }
            }
        }
    }

    /// `true` if the point is inside the polygon.
    fn inside(poly: &[P], p: P) -> bool {
        let n = poly.len();
        let mut c = false;
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + n - 1) % n]);
            if (a[1] > p[1]) != (b[1] > p[1])
                && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
            {
                c = !c;
            }
        }
        c
    }

    #[test]
    fn an_open_town_has_no_wall() {
        let t = town(99, 10, Some(false), Some(false));
        assert!(t.wall.is_none());
        assert!(t.castle.is_none());
        assert!(!t.patches.iter().any(|p| p.kind == Kind::Castle));
    }
}
