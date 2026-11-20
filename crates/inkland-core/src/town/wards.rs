//! The wards of a town and their buildings.
//!
//! A port of the package `com.watabou.towngenerator.wards` of
//! TownGeneratorOS by Watabou (GPL-3.0,
//! <https://github.com/watabou/TownGeneratorOS>).

use super::model::{CurtainWall, Model, V, find_edge};
use super::poly::{self, P, Poly, dist};
use super::random::Random;

/// The width of a main street.
pub const MAIN_STREET: f64 = 2.0;
/// The width of a regular street.
pub const REGULAR_STREET: f64 = 1.0;
/// The width of an alley.
pub const ALLEY: f64 = 0.6;
/// inkland: the width of a river in a town.
pub const RIVER: f64 = 4.0;

/// The most levels of cuts of a block, so that a cut that does not split a
/// block cannot repeat for ever.
const MAX_DEPTH: usize = 40;

/// The kind of a ward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Countryside with no buildings.
    Empty,
    Craftsmen,
    Merchant,
    Slum,
    Patriciate,
    Administration,
    Military,
    Cathedral,
    Castle,
    Market,
    Gate,
    Park,
    Farm,
}

impl Kind {
    /// The name of the ward kind, as the label of the original.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Empty => "",
            Kind::Craftsmen => "Craftsmen",
            Kind::Merchant => "Merchant",
            Kind::Slum => "Slum",
            Kind::Patriciate => "Patriciate",
            Kind::Administration => "Administration",
            Kind::Military => "Military",
            Kind::Cathedral => "Temple",
            Kind::Castle => "Castle",
            Kind::Market => "Market",
            Kind::Gate => "Gate",
            Kind::Park => "Park",
            Kind::Farm => "Farm",
        }
    }
}

/// A ward: its kind, the parameters of its buildings, and its buildings.
#[derive(Debug, Clone)]
pub struct Ward {
    pub kind: Kind,
    /// The least area of a building, the chaos of the street grid, the
    /// chaos of the building sizes, and the part of empty lots.
    pub common: Option<(f64, f64, f64, f64)>,
    /// The wall of a castle.
    pub wall: Option<CurtainWall>,
    pub geometry: Vec<Poly>,
}

impl Ward {
    pub fn new(kind: Kind, wall: Option<CurtainWall>) -> Ward {
        Ward {
            kind,
            common: None,
            wall,
            geometry: Vec::new(),
        }
    }
}

/// A ward of buildings in blocks, with its parameters from the random
/// numbers, as the constructor of each kind of the original.
pub fn common(kind: Kind, rng: &mut Random) -> Ward {
    let (min_sq, grid_chaos, size_chaos, empty) = match kind {
        Kind::Craftsmen => (
            10.0 + 80.0 * rng.float() * rng.float(),
            0.5 + rng.float() * 0.2,
            0.6,
            0.04,
        ),
        Kind::Merchant => (
            50.0 + 60.0 * rng.float() * rng.float(),
            0.5 + rng.float() * 0.3,
            0.7,
            0.15,
        ),
        Kind::Slum => (
            10.0 + 30.0 * rng.float() * rng.float(),
            0.6 + rng.float() * 0.4,
            0.8,
            0.03,
        ),
        Kind::Patriciate => (
            80.0 + 30.0 * rng.float() * rng.float(),
            0.5 + rng.float() * 0.3,
            0.8,
            0.2,
        ),
        Kind::Administration => (
            80.0 + 30.0 * rng.float() * rng.float(),
            0.1 + rng.float() * 0.3,
            0.3,
            0.04,
        ),
        _ => (
            10.0 + 50.0 * rng.float() * rng.float(),
            0.5 + rng.float() * 0.3,
            0.7,
            0.04,
        ),
    };
    Ward {
        kind,
        common: Some((min_sq, grid_chaos, size_chaos, empty)),
        wall: None,
        geometry: Vec::new(),
    }
}

/// A ward of a kind, with the random numbers of its constructor.
pub fn create(kind: Kind, rng: &mut Random) -> Ward {
    match kind {
        Kind::Craftsmen
        | Kind::Merchant
        | Kind::Slum
        | Kind::Patriciate
        | Kind::Administration
        | Kind::Gate => common(kind, rng),
        _ => Ward::new(kind, None),
    }
}

/// `true` if the kind rates the places for it. A kind without a rate goes
/// to a random patch.
pub fn has_rate(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Merchant
            | Kind::Slum
            | Kind::Patriciate
            | Kind::Administration
            | Kind::Military
            | Kind::Cathedral
            | Kind::Market
    )
}

/// The distance of the original `Polygon.distance`: from the first vertex
/// of the shape only.
fn first_distance(model: &Model, patch: usize, p: P) -> f64 {
    dist(model.v[model.shape(patch)[0]], p)
}

/// The centre of the plaza, or the centre of the town.
fn hub(model: &Model) -> P {
    match model.plaza {
        Some(p) => poly::center(&model.shape_pts(p)),
        None => model.v[model.center],
    }
}

fn borders(model: &Model, a: usize, b: usize) -> bool {
    super::model::borders(model.shape(a), model.shape(b))
}

/// How good a patch is for a ward kind: less is better.
pub fn rate(model: &Model, kind: Kind, patch: usize) -> f64 {
    let area = || poly::square(&model.shape_pts(patch));
    match kind {
        Kind::Merchant => first_distance(model, patch, hub(model)),
        Kind::Slum => -first_distance(model, patch, hub(model)),
        Kind::Patriciate => {
            let mut rate = 0.0;
            for &p in &model.patches {
                if let Some(w) = &model.store[p].ward
                    && borders(model, p, patch)
                {
                    if w.kind == Kind::Park {
                        rate -= 1.0;
                    } else if w.kind == Kind::Slum {
                        rate += 1.0;
                    }
                }
            }
            rate
        }
        Kind::Administration => match model.plaza {
            Some(pl) => {
                if borders(model, patch, pl) {
                    0.0
                } else {
                    first_distance(model, patch, poly::center(&model.shape_pts(pl)))
                }
            }
            None => first_distance(model, patch, model.v[model.center]),
        },
        Kind::Military => {
            if model.citadel.is_some_and(|c| borders(model, c, patch)) {
                0.0
            } else if model.wall().is_some_and(|w| w.borders(model, patch)) {
                1.0
            } else if model.citadel.is_none() && model.wall().is_none() {
                0.0
            } else {
                f64::INFINITY
            }
        }
        Kind::Cathedral => {
            if model.plaza.is_some_and(|pl| borders(model, patch, pl)) {
                -1.0 / area()
            } else {
                first_distance(model, patch, hub(model)) * area()
            }
        }
        Kind::Market => {
            for &p in &model.inner {
                if model.store[p]
                    .ward
                    .as_ref()
                    .is_some_and(|w| w.kind == Kind::Market)
                    && borders(model, p, patch)
                {
                    return f64::INFINITY;
                }
            }
            match model.plaza {
                Some(pl) => area() / poly::square(&model.shape_pts(pl)),
                None => first_distance(model, patch, model.v[model.center]),
            }
        }
        _ => 0.0,
    }
}

/// `true` if an artery runs along the edge from `v0` to `v1`.
fn on_artery(model: &Model, v0: V, v1: V) -> bool {
    model
        .arteries
        .iter()
        .any(|s| s.contains(&v0) && s.contains(&v1))
}

/// The block of a patch: its shape less the half widths of the streets
/// around it.
fn city_block(model: &Model, patch: usize) -> Poly {
    let s = model.shape(patch).clone();
    let inner_patch = model.wall().is_none() || model.store[patch].within_walls;
    let n = s.len();
    let inset: Vec<f64> = (0..n)
        .map(|i| {
            let (v0, v1) = (s[i], s[(i + 1) % n]);
            if model.wall().is_some_and(|w| w.borders_by(patch, v0, v1)) {
                // Not too close to the wall.
                MAIN_STREET / 2.0
            } else if model.on_river(v0, v1) {
                // inkland: the river and a quay on each bank.
                RIVER / 2.0 + ALLEY
            } else if model.on_shore(v0, v1) {
                // inkland: a quay along the sea.
                MAIN_STREET / 2.0
            } else {
                let mut on_street = inner_patch
                    && model
                        .plaza
                        .is_some_and(|pl| find_edge(model.shape(pl), v1, v0).is_some());
                if !on_street {
                    on_street = on_artery(model, v0, v1);
                }
                (if on_street {
                    MAIN_STREET
                } else if inner_patch {
                    REGULAR_STREET
                } else {
                    ALLEY
                }) / 2.0
            }
        })
        .collect();
    let pts = model.pts(&s);
    if poly::is_convex(&pts) {
        poly::shrink(&pts, &inset)
    } else {
        poly::buffer(&pts, &inset)
    }
}

/// The buildings of a ward.
pub fn geometry(model: &mut Model, patch: usize, ward: &Ward) -> Vec<Poly> {
    match ward.kind {
        Kind::Empty => Vec::new(),
        Kind::Craftsmen
        | Kind::Merchant
        | Kind::Slum
        | Kind::Patriciate
        | Kind::Administration
        | Kind::Gate => {
            let (min_sq, grid, size, empty) = ward.common.unwrap_or((10.0, 0.5, 0.6, 0.04));
            let block = city_block(model, patch);
            let mut g = if model.lots && ward.kind != Kind::Slum {
                // inkland: street blocks with alleys, then narrow lots along
                // each street, with a yard in the middle.
                let blocks = create_alleys(
                    &mut model.rng,
                    &block,
                    min_sq * INSULA,
                    grid,
                    size * 0.5,
                    0.0,
                    true,
                    0,
                );
                let mut lots = Vec::new();
                for b in blocks {
                    lots.extend(frontage_lots(&mut model.rng, &b, min_sq, empty));
                }
                lots
            } else {
                create_alleys(&mut model.rng, &block, min_sq, grid, size, empty, true, 0)
            };
            if !model.is_enclosed(patch) {
                g = filter_outskirts(model, patch, g);
            }
            g
        }
        Kind::Military => {
            let block = city_block(model, patch);
            let min_sq = poly::square(&block).sqrt() * (1.0 + model.rng.float());
            let grid = 0.1 + model.rng.float() * 0.3;
            create_alleys(&mut model.rng, &block, min_sq, grid, 0.3, 0.25, true, 0)
        }
        // inkland: with lots, a temple with a cross plan stands in a close,
        // with a ring of buildings around it.
        Kind::Cathedral if model.lots => {
            let block = city_block(model, patch);
            let t = 2.0 + model.rng.float() * 3.0;
            let ring = poly::ring(&block, t);
            match cross_plan(&block, t + ALLEY * 2.0) {
                Some(c) => ring.into_iter().chain([c]).collect(),
                // A courtyard too narrow for the temple: the temple stands
                // alone in its yard.
                None => match cross_plan(&block, ALLEY * 3.0) {
                    Some(c) => vec![c],
                    None => ring,
                },
            }
        }
        Kind::Cathedral => {
            if model.rng.bool(0.4) {
                let block = city_block(model, patch);
                let t = 2.0 + model.rng.float() * 4.0;
                poly::ring(&block, t)
            } else {
                let block = city_block(model, patch);
                ortho_building(&mut model.rng, &block, 50.0, 0.8)
            }
        }
        Kind::Castle => {
            let block = poly::shrink_eq(&model.shape_pts(patch), MAIN_STREET * 2.0);
            let min = poly::square(&block).sqrt() * 4.0;
            ortho_building(&mut model.rng, &block, min, 0.6)
        }
        Kind::Market => market(model, patch),
        Kind::Park => {
            let block = city_block(model, patch);
            if poly::compactness(&block) >= 0.7 {
                poly::radial(&block, ALLEY)
            } else {
                poly::semi_radial(&block, ALLEY)
            }
        }
        Kind::Farm => {
            let shape = model.shape_pts(patch);
            let corner = shape[model.rng.index(shape.len())];
            let c = poly::centroid(&shape);
            let pos = poly::interpolate(corner, c, 0.3 + model.rng.float() * 0.4);
            let mut housing = poly::rect(4.0, 4.0);
            poly::rotate(&mut housing, model.rng.float() * std::f64::consts::PI);
            poly::offset(&mut housing, pos);
            ortho_building(&mut model.rng, &housing, 8.0, 0.5)
        }
    }
}

/// The statue or the fountain of a market.
fn market(model: &mut Model, patch: usize) -> Vec<Poly> {
    let shape = model.shape_pts(patch);
    let statue = model.rng.bool(0.6);
    let offset = statue || model.rng.bool(0.3);
    let (mut v0, mut v1) = ([0.0, 0.0], [0.0, 0.0]);
    if statue || offset {
        let mut length = -1.0;
        let n = shape.len();
        for i in 0..n {
            let (p0, p1) = (shape[i], shape[(i + 1) % n]);
            let l = dist(p0, p1);
            if l > length {
                length = l;
                v0 = p0;
                v1 = p1;
            }
        }
    }
    let mut object = if statue {
        let w = 1.0 + model.rng.float();
        let h = 1.0 + model.rng.float();
        let mut r = poly::rect(w, h);
        poly::rotate(&mut r, libm::atan2(v1[1] - v0[1], v1[0] - v0[0]));
        r
    } else {
        poly::circle(1.0 + model.rng.float())
    };
    let c = poly::centroid(&shape);
    if offset {
        let gravity = poly::interpolate(v0, v1, 0.5);
        let at = poly::interpolate(c, gravity, 0.2 + model.rng.float() * 0.4);
        poly::offset(&mut object, at);
    } else {
        poly::offset(&mut object, c);
    }
    vec![object]
}

/// Cut a block into buildings with alleys between them.
#[allow(clippy::too_many_arguments)]
pub fn create_alleys(
    rng: &mut Random,
    p: &[P],
    min_sq: f64,
    grid_chaos: f64,
    size_chaos: f64,
    empty_prob: f64,
    split: bool,
    depth: usize,
) -> Vec<Poly> {
    if p.len() < 3 {
        return Vec::new();
    }
    // Cut the longest edge.
    let n = p.len();
    let mut v = 0;
    let mut length = -1.0;
    for i in 0..n {
        let l = dist(p[i], p[(i + 1) % n]);
        if l > length {
            length = l;
            v = i;
        }
    }
    let spread = 0.8 * grid_chaos;
    let ratio = (1.0 - spread) / 2.0 + rng.float() * spread;
    // Keep the buildings rectangular, also in chaotic wards.
    let angle_spread = std::f64::consts::PI / 6.0
        * grid_chaos
        * if poly::square(p) < min_sq * 4.0 {
            0.0
        } else {
            1.0
        };
    let b = (rng.float() - 0.5) * angle_spread;
    let halves = poly::bisect(p, v, ratio, b, if split { ALLEY } else { 0.0 });
    let mut buildings = Vec::new();
    for half in halves {
        let limit = min_sq * libm::pow(2.0, 4.0 * size_chaos * (rng.float() - 0.5));
        if poly::square(&half) < limit || depth >= MAX_DEPTH {
            if !rng.bool(empty_prob) {
                buildings.push(half);
            }
        } else {
            let s = poly::square(&half) > min_sq / (rng.float() * rng.float());
            buildings.extend(create_alleys(
                rng,
                &half,
                min_sq,
                grid_chaos,
                size_chaos,
                empty_prob,
                s,
                depth + 1,
            ));
        }
    }
    buildings
}

/// inkland: the area of a street block with lots around it, as a multiple
/// of the least area of a building.
const INSULA: f64 = 6.0;

/// inkland: the lots of a street block. A strip one lot deep runs along each
/// edge, and cuts across it make narrow lots on the street. The middle of
/// the block is a yard, or sometimes one more building.
pub fn frontage_lots(rng: &mut Random, block: &[P], min_sq: f64, empty: f64) -> Vec<Poly> {
    let depth = min_sq.sqrt();
    let width = 0.6 * min_sq.sqrt();
    let area = poly::square(block).abs();
    if block.len() < 3 || area < depth * depth * 2.5 {
        return vec![block.to_vec()];
    }
    let mut out = Vec::new();
    let strips = poly::ring(block, depth * (0.8 + 0.4 * rng.float()));
    for strip in strips {
        split_lots(rng, &strip, width, empty, 0, &mut out);
    }
    // The yard: the rest of the block, with a building in some of them.
    if rng.bool(0.25) {
        let core = poly::shrink_eq(block, depth * 1.4);
        if core.len() >= 3 && poly::square(&core).abs() > min_sq {
            out.push(core);
        }
    }
    out
}

/// Cut a strip into lots across its longest edge, each about `width` wide.
fn split_lots(
    rng: &mut Random,
    strip: &[P],
    width: f64,
    empty: f64,
    depth: usize,
    out: &mut Vec<Poly>,
) {
    if strip.len() < 3 {
        return;
    }
    let i = longest_edge(strip);
    let l = poly::len(poly::vector(strip, i));
    if l < width * 1.4 || depth >= MAX_DEPTH {
        if poly::square(strip).abs() > 0.5 && !rng.bool(empty) {
            out.push(strip.to_vec());
        }
        return;
    }
    let ratio = 0.5 + (rng.float() - 0.5) * 0.3;
    for half in poly::bisect(strip, i, ratio, 0.0, 0.0) {
        if half.len() >= 3 && poly::square(&half).abs() < poly::square(strip).abs() - 1e-9 {
            split_lots(rng, &half, width, empty, depth + 1, out);
        } else {
            out.push(half);
        }
    }
}

/// The index of the vertex that starts the longest edge.
fn longest_edge(p: &[P]) -> usize {
    let idx: Vec<usize> = (0..p.len()).collect();
    poly::min_by(&idx, |&i| -poly::len(poly::vector(p, i)))
}

/// inkland: a temple with a cross plan in the middle of a block, along the
/// longest edge of the block, with `margin` of space on each side. `None`
/// if the block is too small.
pub fn cross_plan(block: &[P], margin: f64) -> Option<Poly> {
    let n = block.len();
    let longest = (0..n).max_by(|&a, &b| {
        poly::len(poly::vector(block, a)).total_cmp(&poly::len(poly::vector(block, b)))
    })?;
    let u = poly::norm(poly::vector(block, longest), 1.0);
    let v = poly::rotate90(u);
    let inner = poly::shrink_eq(block, margin);
    if inner.len() < 3 {
        return None;
    }
    let c = poly::centroid(&inner);
    // The largest cross that the extents of the block allow: its length
    // along `u` and its transept across.
    let span = |d: P| {
        let t: Vec<f64> = inner
            .iter()
            .map(|&p| poly::dot(poly::sub(p, c), d))
            .collect();
        t.iter().copied().fold(f64::INFINITY, |a, b| a.min(b.abs()))
    };
    let mut s = f64::min(span(u) * 0.9, span(v) / 0.55 * 0.9);
    for _ in 0..12 {
        let w = 0.32 * s;
        let (t0, t1, e, h) = (0.2 * s, 0.2 * s + w, 0.9 * s, 0.55 * s);
        let local = [
            [-s, -w / 2.0],
            [t0, -w / 2.0],
            [t0, -h],
            [t1, -h],
            [t1, -w / 2.0],
            [e, -w / 2.0],
            [e, w / 2.0],
            [t1, w / 2.0],
            [t1, h],
            [t0, h],
            [t0, w / 2.0],
            [-s, w / 2.0],
        ];
        let shape: Poly = local
            .iter()
            .map(|&[a, b]| poly::add(c, poly::add(poly::scale(u, a), poly::scale(v, b))))
            .collect();
        if shape.iter().all(|&p| super::districts::contains(&inner, p)) {
            return (s > 2.0).then_some(shape);
        }
        s *= 0.85;
    }
    None
}

/// Cut a block into buildings along two directions at right angles.
pub fn ortho_building(rng: &mut Random, p: &[P], min_block_sq: f64, fill: f64) -> Vec<Poly> {
    fn slice(
        rng: &mut Random,
        p: &[P],
        c1: P,
        c2: P,
        min: f64,
        fill: f64,
        depth: usize,
    ) -> Vec<Poly> {
        let v0i = longest_edge(p);
        let v0 = p[v0i];
        let v1 = poly::next(p, v0i);
        let v = poly::sub(v1, v0);
        let ratio = 0.4 + rng.float() * 0.2;
        let p1 = poly::interpolate(v0, v1, ratio);
        let c = if poly::dot(v, c1).abs() < poly::dot(v, c2).abs() {
            c1
        } else {
            c2
        };
        let halves = poly::cut(p, p1, poly::add(p1, c), 0.0);
        let mut buildings = Vec::new();
        for half in halves {
            let limit = min * libm::pow(2.0, rng.normal() * 2.0 - 1.0);
            if poly::square(&half) < limit || depth >= MAX_DEPTH {
                if rng.bool(fill) {
                    buildings.push(half);
                }
            } else {
                buildings.extend(slice(rng, &half, c1, c2, min, fill, depth + 1));
            }
        }
        buildings
    }
    if p.len() < 3 {
        return Vec::new();
    }
    if poly::square(p) < min_block_sq {
        return vec![p.to_vec()];
    }
    let c1 = poly::vector(p, longest_edge(p));
    let c2 = poly::rotate90(c1);
    for _ in 0..100 {
        let blocks = slice(rng, p, c1, c2, min_block_sq, fill, 0);
        if !blocks.is_empty() {
            return blocks;
        }
    }
    vec![p.to_vec()]
}

/// Leave out the buildings of an open ward that are far from the roads and
/// from the town.
fn filter_outskirts(model: &mut Model, patch: usize, geometry: Vec<Poly>) -> Vec<Poly> {
    let s = model.shape(patch).clone();
    let pts = model.pts(&s);
    let n = s.len();
    // Each populated edge: its point, its direction, and the largest
    // distance of a vertex from it.
    let mut edges: Vec<(f64, f64, f64, f64, f64)> = Vec::new();
    let add_edge = |edges: &mut Vec<(f64, f64, f64, f64, f64)>, i: usize, factor: f64| {
        let (v1, v2) = (s[i], s[(i + 1) % n]);
        let (a, b) = (model.v[v1], model.v[v2]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let d = s
            .iter()
            .map(|&v| {
                if v != v1 && v != v2 {
                    let q = model.v[v];
                    poly::distance2line(a[0], a[1], dx, dy, q[0], q[1]) * factor
                } else {
                    0.0
                }
            })
            .fold(f64::NEG_INFINITY, f64::max);
        edges.push((a[0], a[1], dx, dy, d));
    };
    for i in 0..n {
        let (v1, v2) = (s[i], s[(i + 1) % n]);
        if on_artery(model, v1, v2) {
            add_edge(&mut edges, i, 1.0);
        } else if let Some(nb) = model.get_neighbour(patch, v1)
            && model.store[nb].within_city
        {
            let f = if model.is_enclosed(nb) { 1.0 } else { 0.4 };
            add_edge(&mut edges, i, f);
        }
    }
    // The density at each vertex: 1 at a gate, random among town patches,
    // else 0.
    let density: Vec<f64> = s
        .iter()
        .map(|&v| {
            if model.gates.contains(&v) {
                1.0
            } else if model
                .patch_by_vertex(v)
                .iter()
                .all(|&p| model.store[p].within_city)
            {
                2.0 * model.rng.float()
            } else {
                0.0
            }
        })
        .collect();
    geometry
        .into_iter()
        .filter(|building| {
            let mut min_dist: f64 = 1.0;
            for &(x, y, dx, dy, d) in &edges {
                for v in building {
                    let dd = poly::distance2line(x, y, dx, dy, v[0], v[1]);
                    min_dist = min_dist.min(dd / d);
                }
            }
            let c = poly::center(building);
            let w = poly::interpolate_weights(&pts, c);
            let p: f64 = density.iter().zip(&w).map(|(a, b)| a * b).sum();
            min_dist /= p;
            model.rng.fuzzy(1.0) > min_dist
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cross_plan_fits_inside_its_block() {
        let block = poly::rect(60.0, 40.0);
        let c = cross_plan(&block, 3.0).expect("a temple");
        assert_eq!(c.len(), 12);
        for p in &c {
            assert!(p[0].abs() <= 27.0 && p[1].abs() <= 17.0, "{p:?}");
        }
        // Along the longest edge: longer in x than in y.
        let xs = c.iter().map(|p| p[0]);
        let ys = c.iter().map(|p| p[1]);
        let span = |it: &mut dyn Iterator<Item = f64>| {
            let v: Vec<f64> = it.collect();
            v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert!(span(&mut xs.into_iter()) > span(&mut ys.into_iter()));
        assert!(cross_plan(&poly::rect(6.0, 6.0), 3.0).is_none());
    }

    #[test]
    fn a_block_gets_narrow_lots_along_its_edges() {
        let block: Poly = vec![[0.0, 0.0], [40.0, 0.0], [40.0, 40.0], [0.0, 40.0]];
        let mut rng = Random::new(3);
        let lots = frontage_lots(&mut rng, &block, 25.0, 0.0);
        assert!(lots.len() >= 16, "{}", lots.len());
        // The lots along the edges cover the ring of the block, one lot deep.
        let along: Vec<&Poly> = lots
            .iter()
            .filter(|l| {
                l.iter()
                    .any(|v| v[0] == 0.0 || v[0] == 40.0 || v[1] == 0.0 || v[1] == 40.0)
            })
            .collect();
        assert!(along.len() >= 16);
        for l in &along {
            let a = poly::square(l).abs();
            assert!(a > 0.5 && a < 80.0, "{a}");
        }
        // A small block stays one building.
        let small: Poly = vec![[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]];
        assert_eq!(frontage_lots(&mut rng, &small, 25.0, 0.0), vec![small]);
    }
}
