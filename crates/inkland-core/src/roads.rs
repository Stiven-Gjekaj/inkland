//! Roads.
//!
//! A road is a least-cost path of land cells. A step costs its distance,
//! more on slopes, in wetland, and into a river cell (a bridge or a ford).
//! A cell that a road already uses costs less, so roads share a path.
//!
//! Pinned roads come first, in config order. A pinned road goes from its
//! start place through its `via` points to its end place. With a
//! `target_length`, a road that is too short gets a detour: a waypoint to
//! one side of its longest leg, at a distance that a search finds so that
//! the length is in the tolerance. A road that is too long cannot become
//! shorter; the report gives its real length, and its check fails.
//!
//! Then a network of generated roads joins the capitals of each landmass
//! (the shortest tree), and trails join each town to the nearest place on
//! the network.

use crate::config::{Config, RoadKind, RoadPin, Waypoint};
use crate::error::{Error, ErrorCode};
use crate::features::Features;
use crate::grid::Grid;
use crate::rivers::River;
use crate::society::Society;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

/// A place on a road.
#[derive(Debug, Clone, PartialEq)]
pub struct Stop {
    /// The config id of a pinned place.
    pub place: String,
    /// The length along the road from its start, in units.
    pub at_length: f64,
}

/// A river crossing.
#[derive(Debug, Clone, PartialEq)]
pub struct Crossing {
    /// The river id in the world.
    pub river: u32,
    /// The config id of a pinned river.
    pub river_pin: Option<String>,
    pub at_length: f64,
    /// `bridge` or `ford`.
    pub kind: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Road {
    /// 1, 2, 3, ... Pinned roads first.
    pub id: u32,
    /// The index of the pinned road in the config.
    pub pin: Option<usize>,
    pub kind: RoadKind,
    pub cells: Vec<usize>,
    pub points: Vec<[f64; 2]>,
    pub length: f64,
    pub stops: Vec<Stop>,
    pub crossings: Vec<Crossing>,
}

struct Key(f64);
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// What the road stage reads from the world.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub heights: &'a [f64],
    pub features: &'a Features,
    pub biomes: &'a [u8],
    pub rivers: &'a [River],
    pub flux: &'a [f64],
    pub river_threshold: f64,
    pub society: &'a Society,
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn path_length(grid: &Grid, cells: &[usize]) -> f64 {
    cells
        .windows(2)
        .map(|w| dist(grid.points[w[0]], grid.points[w[1]]))
        .sum()
}

struct Router<'a> {
    input: &'a Input<'a>,
    river_of: Vec<u32>,
    /// Cells that a road uses, with the cost factor of a reuse.
    used: Vec<bool>,
    /// `true`: a step costs only its distance. A pinned road that is too
    /// long with the terrain costs uses this to become as short as it can.
    distance_only: bool,
}

impl Router<'_> {
    fn step(&self, a: usize, b: usize) -> Option<f64> {
        let i = self.input;
        if !i.features.is_land(b) {
            return None;
        }
        let d = dist(i.grid.points[a], i.grid.points[b]);
        if self.distance_only {
            return Some(d);
        }
        let slope = (i.heights[b] - i.heights[a]).abs() / 3.0;
        let wet = if i.biomes[b] == crate::biomes::WETLAND {
            2.0
        } else {
            0.0
        };
        let river = if self.river_of[b] != 0 && self.river_of[b] != self.river_of[a] {
            4.0
        } else {
            0.0
        };
        let mut c = d * (1.0 + slope + wet + river);
        if self.used[b] {
            c *= 0.6;
        }
        Some(c)
    }

    /// The least-cost path from `from` to `to`, both included. Cells in
    /// `avoid` are not used.
    fn route(&self, from: usize, to: usize, avoid: &[bool]) -> Option<Vec<usize>> {
        let grid = self.input.grid;
        let n = grid.len();
        let goal = grid.points[to];
        let h = |c: usize| dist(grid.points[c], goal);
        let mut cost = vec![f64::INFINITY; n];
        let mut prev = vec![usize::MAX; n];
        let mut heap = BinaryHeap::new();
        cost[from] = 0.0;
        heap.push(Reverse((Key(h(from)), from)));
        while let Some(Reverse((Key(f), c))) = heap.pop() {
            if c == to {
                let mut path = vec![c];
                let mut cur = c;
                while cur != from {
                    cur = prev[cur];
                    path.push(cur);
                }
                path.reverse();
                return Some(path);
            }
            if f - h(c) > cost[c] + 1e-9 {
                continue;
            }
            for m in grid.neighbors(c) {
                if avoid[m] && m != to {
                    continue;
                }
                let Some(s) = self.step(c, m) else { continue };
                let next = cost[c] + s;
                if next < cost[m] {
                    cost[m] = next;
                    prev[m] = c;
                    heap.push(Reverse((Key(next + h(m)), m)));
                }
            }
        }
        None
    }

    /// A path through the given cells in order.
    fn route_through(&self, stops: &[usize]) -> Option<Vec<usize>> {
        let n = self.input.grid.len();
        let mut avoid = vec![false; n];
        let mut path = vec![stops[0]];
        for w in stops.windows(2) {
            let leg = self.route(w[0], w[1], &avoid)?;
            for &c in &leg[1..] {
                path.push(c);
            }
            // A later leg does not go back over this leg.
            for &c in &leg[..leg.len() - 1] {
                avoid[c] = true;
            }
        }
        Some(path)
    }
}

/// Route all roads.
pub fn build(input: &Input) -> Result<Vec<Road>, Error> {
    let grid = input.grid;
    let n = grid.len();
    let mut river_of = vec![0u32; n];
    for r in input.rivers {
        for &c in &r.cells {
            river_of[c] = r.id;
        }
    }
    let mut router = Router {
        input,
        river_of,
        used: vec![false; n],
        distance_only: false,
    };
    let mut roads: Vec<Road> = Vec::new();

    let place_cell = |id: &str| -> Option<usize> {
        input
            .society
            .places
            .iter()
            .find(|p| p.pin.as_deref() == Some(id))
            .map(|p| p.cell)
    };

    for (k, pin) in input.config.pins.roads.iter().enumerate() {
        let cells = route_pinned(&router, input, k, pin, &place_cell)?;
        for &c in &cells {
            router.used[c] = true;
        }
        roads.push(finish(
            input,
            &router.river_of,
            roads.len() as u32 + 1,
            Some(k),
            pin.kind,
            cells,
        ));
    }

    // The generated network: a shortest tree over the capitals of each
    // landmass, then a trail from each town.
    let soc = input.society;
    let landmass = |c: usize| input.features.of_cell[c];
    let capitals: Vec<usize> = soc
        .places
        .iter()
        .filter(|p| p.capital)
        .map(|p| p.cell)
        .collect();
    let mut groups: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for &c in &capitals {
        groups.entry(landmass(c)).or_default().push(c);
    }
    for cells in groups.values() {
        for (a, b) in shortest_tree(grid, cells) {
            if let Some(path) = router.route(a, b, &vec![false; n]) {
                for &c in &path {
                    router.used[c] = true;
                }
                roads.push(finish(
                    input,
                    &router.river_of,
                    roads.len() as u32 + 1,
                    None,
                    RoadKind::Road,
                    path,
                ));
            }
        }
    }
    let mut on_network: Vec<usize> = soc
        .places
        .iter()
        .filter(|p| router.used[p.cell])
        .map(|p| p.cell)
        .collect();
    let towns: Vec<(usize, [f64; 2])> = soc
        .places
        .iter()
        .filter(|p| p.kind == crate::config::PlaceKind::Town)
        .map(|p| (p.cell, p.at))
        .collect();
    for (cell, at) in towns {
        if router.used[cell] {
            continue;
        }
        let target = on_network
            .iter()
            .copied()
            .filter(|&c| landmass(c) == landmass(cell))
            .min_by(|&a, &b| {
                dist(grid.points[a], at)
                    .total_cmp(&dist(grid.points[b], at))
                    .then(a.cmp(&b))
            });
        let Some(t) = target else { continue };
        if let Some(path) = router.route(cell, t, &vec![false; n]) {
            for &c in &path {
                router.used[c] = true;
            }
            on_network.push(cell);
            roads.push(finish(
                input,
                &router.river_of,
                roads.len() as u32 + 1,
                None,
                RoadKind::Trail,
                path,
            ));
        }
    }
    Ok(roads)
}

/// The edges of a shortest tree (Kruskal) over points, by distance.
fn shortest_tree(grid: &Grid, cells: &[usize]) -> Vec<(usize, usize)> {
    let mut edges: Vec<(f64, usize, usize)> = Vec::new();
    for (i, &a) in cells.iter().enumerate() {
        for &b in &cells[i + 1..] {
            edges.push((dist(grid.points[a], grid.points[b]), a, b));
        }
    }
    edges.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
    let mut parent: BTreeMap<usize, usize> = cells.iter().map(|&c| (c, c)).collect();
    fn root(parent: &mut BTreeMap<usize, usize>, c: usize) -> usize {
        let mut r = c;
        while parent[&r] != r {
            r = parent[&r];
        }
        parent.insert(c, r);
        r
    }
    let mut out = Vec::new();
    for (_, a, b) in edges {
        let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
        if ra != rb {
            parent.insert(ra, rb);
            out.push((a, b));
        }
    }
    out
}

fn route_pinned(
    router: &Router,
    input: &Input,
    k: usize,
    pin: &RoadPin,
    place_cell: &dyn Fn(&str) -> Option<usize>,
) -> Result<Vec<usize>, Error> {
    let grid = input.grid;
    let path = format!("/pins/roads/{k}");
    let no_path = || {
        Error::new(
            ErrorCode::RoadNoPath,
            path.clone(),
            format!("no land path joins the points of the road {:?}", pin.id),
        )
    };
    let mut stops = vec![place_cell(&pin.from).ok_or_else(no_path)?];
    for w in &pin.via {
        stops.push(match w {
            Waypoint::Place(id) => place_cell(id).ok_or_else(no_path)?,
            Waypoint::Point(p) => grid.find_cell(p[0], p[1]),
        });
    }
    stops.push(place_cell(&pin.to).ok_or_else(no_path)?);
    let base = router.route_through(&stops).ok_or_else(no_path)?;
    let Some(target) = pin.target_length else {
        return Ok(base);
    };
    let lo = target * (1.0 - pin.tolerance);
    let hi = target * (1.0 + pin.tolerance);
    let len0 = path_length(grid, &base);
    if (lo..=hi).contains(&len0) {
        return Ok(base);
    }
    // Too long with the terrain costs: use the shortest path by distance,
    // and make it longer from there if it is now too short.
    let short_router;
    let router = if len0 > hi {
        short_router = Router {
            input: router.input,
            river_of: router.river_of.clone(),
            used: router.used.clone(),
            distance_only: true,
        };
        &short_router
    } else {
        router
    };
    let base = if len0 > hi {
        router.route_through(&stops).ok_or_else(no_path)?
    } else {
        base
    };
    let len0 = path_length(grid, &base);
    if len0 >= lo {
        // In the tolerance, or still too long: a road cannot be shorter
        // than its shortest path.
        return Ok(base);
    }

    // A detour on the longest leg: a waypoint at distance `d` to one side
    // of the middle of the leg.
    let (leg, _) = stops
        .windows(2)
        .enumerate()
        .map(|(i, w)| (i, dist(grid.points[w[0]], grid.points[w[1]])))
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .expect("a road has a leg");
    let a = grid.points[stops[leg]];
    let b = grid.points[stops[leg + 1]];
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let len_ab = dist(a, b).max(1e-9);
    let normal = [-(b[1] - a[1]) / len_ab, (b[0] - a[0]) / len_ab];
    let (w, h) = (grid.width, grid.height);
    let detour = |d: f64, side: f64| -> Option<(Vec<usize>, f64)> {
        let p = [mid[0] + normal[0] * d * side, mid[1] + normal[1] * d * side];
        if p[0] < 0.0 || p[1] < 0.0 || p[0] > w || p[1] > h {
            return None;
        }
        let c = grid.find_cell(p[0], p[1]);
        if !input.features.is_land(c) || stops.contains(&c) {
            return None;
        }
        let mut s = stops.clone();
        s.insert(leg + 1, c);
        let cells = router.route_through(&s)?;
        let l = path_length(grid, &cells);
        Some((cells, l))
    };
    // Scan the offset on each side, then halve the step where the length
    // passes the target.
    let max_d = target;
    let steps = 40;
    let mut best: Option<(f64, Vec<usize>)> = None;
    let consider = |best: &mut Option<(f64, Vec<usize>)>, cells: Vec<usize>, l: f64| {
        let miss = if l < lo {
            lo - l
        } else if l > hi {
            l - hi
        } else {
            0.0
        };
        if best.as_ref().is_none_or(|(m, _)| miss < *m) {
            *best = Some((miss, cells));
        }
    };
    for side in [1.0, -1.0] {
        let mut prev: Option<(f64, f64)> = None;
        for i in 1..=steps {
            let d = max_d * i as f64 / steps as f64;
            let Some((cells, l)) = detour(d, side) else {
                continue;
            };
            consider(&mut best, cells, l);
            if let Some((pd, pl)) = prev
                && pl < lo
                && l > hi
            {
                // Halve between pd and d.
                let (mut x0, mut x1) = (pd, d);
                for _ in 0..24 {
                    let xm = (x0 + x1) / 2.0;
                    let Some((cells, lm)) = detour(xm, side) else {
                        break;
                    };
                    consider(&mut best, cells, lm);
                    if lm < lo {
                        x0 = xm;
                    } else if lm > hi {
                        x1 = xm;
                    } else {
                        break;
                    }
                }
            }
            if best.as_ref().is_some_and(|(m, _)| *m == 0.0) {
                break;
            }
            prev = Some((d, l));
        }
        if best.as_ref().is_some_and(|(m, _)| *m == 0.0) {
            break;
        }
    }
    Ok(best.map_or(base, |(_, cells)| cells))
}

/// Make a road from its cells: points, length, stops, and crossings.
fn finish(
    input: &Input,
    river_of: &[u32],
    id: u32,
    pin: Option<usize>,
    kind: RoadKind,
    cells: Vec<usize>,
) -> Road {
    let grid = input.grid;
    let points: Vec<[f64; 2]> = cells.iter().map(|&c| grid.points[c]).collect();
    let mut along = vec![0.0; cells.len()];
    for i in 1..cells.len() {
        along[i] = along[i - 1] + dist(points[i - 1], points[i]);
    }
    let length = along.last().copied().unwrap_or(0.0);

    // Stops: each pinned place on the road, in order along it.
    let mut stops: Vec<Stop> = Vec::new();
    for (i, &c) in cells.iter().enumerate() {
        for p in input.society.places.iter().filter(|p| p.cell == c) {
            if let Some(id) = &p.pin
                && !stops.iter().any(|s| &s.place == id)
            {
                stops.push(Stop {
                    place: id.clone(),
                    at_length: along[i],
                });
            }
        }
    }

    // Crossings: a run of cells of one river, where the road comes from one
    // side of the river and leaves on the other side.
    let mut crossings = Vec::new();
    let mut i = 0;
    while i < cells.len() {
        let r = river_of[cells[i]];
        if r == 0 {
            i += 1;
            continue;
        }
        let start = i;
        while i < cells.len() && river_of[cells[i]] == r {
            i += 1;
        }
        let end = i - 1;
        if start == 0 || i >= cells.len() {
            continue;
        }
        let river = input
            .rivers
            .iter()
            .find(|x| x.id == r)
            .expect("a river cell has a river");
        let mid = (start + end) / 2;
        let k = river
            .cells
            .iter()
            .position(|&c| c == cells[mid])
            .unwrap_or(0);
        let p = river.points[k];
        let q = river.points[(k + 1).min(river.points.len() - 1)];
        let o = if k + 1 < river.points.len() {
            p
        } else {
            river.points[k.saturating_sub(1)]
        };
        let dir = if k + 1 < river.points.len() {
            [q[0] - p[0], q[1] - p[1]]
        } else {
            [p[0] - o[0], p[1] - o[1]]
        };
        let side = |x: [f64; 2]| (dir[0] * (x[1] - p[1]) - dir[1] * (x[0] - p[0])).signum();
        let before = points[start - 1];
        let after = points[i];
        if side(before) * side(after) < 0.0 {
            let big = input.flux[cells[mid]] >= 3.0 * input.river_threshold;
            let kind = match kind {
                RoadKind::Road => "bridge",
                RoadKind::Trail if big => "bridge",
                RoadKind::Trail => "ford",
            };
            crossings.push(Crossing {
                river: r,
                river_pin: river.course.map(|c| input.config.pins.rivers[c].id.clone()),
                at_length: along[mid],
                kind,
            });
        }
    }

    Road {
        id,
        pin,
        kind,
        cells,
        points,
        length,
        stops,
        crossings,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    fn config(roads: serde_json::Value, rivers: serde_json::Value) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 11, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": { "steps": ["Add 35 all", "Hill 6 10-20 10-90 10-90", "Smooth 2"] } },
            "pins": {
                "points": [
                    { "id": "ostrey", "kind": "city", "at": [60, 150] },
                    { "id": "mid", "kind": "village", "at": [200, 100] },
                    { "id": "harrowgate", "kind": "town", "at": [340, 150] }
                ],
                "regions": [{ "id": "hills", "kind": "hills", "circle": { "at": [200, 220], "r": 30 } }],
                "rivers": rivers,
                "roads": roads
            }
        }))
        .unwrap()
    }

    fn pinned(w: &World, k: usize) -> &super::Road {
        w.roads.iter().find(|r| r.pin == Some(k)).unwrap()
    }

    #[test]
    fn a_road_joins_its_places_through_its_via_points() {
        let c = config(
            json!([{ "id": "r", "from": "ostrey", "to": "harrowgate", "via": ["mid"] }]),
            json!([]),
        );
        let w = World::build(&c).unwrap();
        let r = pinned(&w, 0);
        assert_eq!(*r.points.first().unwrap(), [60.0, 150.0]);
        assert_eq!(*r.points.last().unwrap(), [340.0, 150.0]);
        let ids: Vec<&str> = r.stops.iter().map(|s| s.place.as_str()).collect();
        assert_eq!(ids, ["ostrey", "mid", "harrowgate"]);
        assert!(r.stops.windows(2).all(|s| s[0].at_length < s[1].at_length));
        assert!((r.stops[2].at_length - r.length).abs() < 1e-9);
    }

    #[test]
    fn a_road_reaches_its_target_length_in_the_tolerance() {
        for target in [320.0, 380.0, 450.0] {
            let c = config(
                json!([{ "id": "r", "from": "ostrey", "to": "harrowgate",
                                     "target_length": target, "tolerance": 0.05 }]),
                json!([]),
            );
            let w = World::build(&c).unwrap();
            let r = pinned(&w, 0);
            assert!(
                (r.length - target).abs() <= target * 0.05,
                "target {target}, length {}",
                r.length
            );
        }
    }

    #[test]
    fn a_road_reports_where_it_crosses_a_pinned_river() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 11, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": { "steps": ["Add 35 all"] } },
            "pins": {
                "points": [
                    { "id": "ostrey", "kind": "city", "at": [60, 150] },
                    { "id": "harrowgate", "kind": "town", "at": [340, 150] }
                ],
                "regions": [{ "id": "south-sea", "kind": "sea", "box": [0, 280, 400, 300] }],
                "rivers": [{ "id": "brann", "source": [200, 20], "mouth": [200, 290] }],
                "roads": [{ "id": "r", "from": "ostrey", "to": "harrowgate" }]
            }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let r = pinned(&w, 0);
        let brann: Vec<_> = r
            .crossings
            .iter()
            .filter(|x| x.river_pin.as_deref() == Some("brann"))
            .collect();
        assert_eq!(brann.len(), 1, "{:?}", r.crossings);
        assert_eq!(brann[0].kind, "bridge");
        assert!(brann[0].at_length > 0.0 && brann[0].at_length < r.length);
    }

    #[test]
    fn capitals_are_joined_by_generated_roads() {
        let c = config(json!([]), json!([]));
        let w = World::build(&c).unwrap();
        assert!(w.roads.iter().any(|r| r.pin.is_none()));
        assert!(
            w.roads
                .iter()
                .all(|r| r.cells.iter().all(|&c| w.features.is_land(c)))
        );
    }
}
