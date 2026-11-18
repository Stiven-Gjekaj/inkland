//! Roads.
//!
//! A road is a least-cost path of land cells. A step costs its distance,
//! more on slopes, in wetland, and into a river cell (a bridge or a ford).
//! A cell that a road already uses costs less, so roads share a path.
//!
//! A pinned road keeps a land corridor. Before the features are marked,
//! `keep_corridors` makes the water cells on the straight legs of the
//! road into land if no land path joins its stops, or if the shortest
//! land path is longer than the tolerance permits. A pinned water region
//! stays water.
//!
//! Pinned roads come first, in config order. A pinned road goes from its
//! start place through its `via` points to its end place. With a
//! `target_length`, a road that is too short gets a detour: a waypoint to
//! one side of a leg, at a distance that a search finds so that the length
//! is in the tolerance. The search tries the longest leg first, and on each
//! leg the middle first, then the quarter points. The points of a pinned road move in
//! their cells to cut the zigzag between cell sites, and the length is the
//! length of that line. A road that is too long cannot become
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
use std::collections::BinaryHeap;

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
    /// The id of a road that an overlay adds.
    pub overlay: Option<String>,
    pub kind: RoadKind,
    /// The route group of FMG: `roads`, `trails`, or `searoutes`.
    pub group: &'static str,
    /// The name of a generated route, from FMG `generateName`.
    pub name: Option<String>,
    /// The land feature of a road, or the water feature of a sea route.
    pub feature: u32,
    pub cells: Vec<usize>,
    pub points: Vec<[f64; 2]>,
    pub length: f64,
    pub stops: Vec<Stop>,
    pub crossings: Vec<Crossing>,
}

/// The roads of a world and the sea routes between its ports.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Routes {
    /// Pinned roads first, in config order, then the generated roads and
    /// trails.
    pub roads: Vec<Road>,
    /// The sea routes between ports, over water and navigable rivers.
    pub sea: Vec<Road>,
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
    pub temperature: &'a [f64],
    pub society: &'a Society,
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn line_length(points: &[[f64; 2]]) -> f64 {
    points.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// The points of a pinned road. Each point moves in its cell toward the
/// middle of its two neighbours, so that the line has no zigzag between
/// the cell sites. The ends and the cells of places do not move.
fn taut_points(input: &Input, cells: &[usize]) -> Vec<[f64; 2]> {
    taut_line(input.grid, cells, &|c| {
        input.society.places.iter().any(|p| p.cell == c)
    })
}

/// The taut line through the given cells. The ends and the cells where
/// `fixed` is `true` do not move.
fn taut_line(grid: &Grid, cells: &[usize], fixed: &dyn Fn(usize) -> bool) -> Vec<[f64; 2]> {
    let mut points: Vec<[f64; 2]> = cells.iter().map(|&c| grid.points[c]).collect();
    let fixed: Vec<bool> = cells
        .iter()
        .enumerate()
        .map(|(i, &c)| i == 0 || i + 1 == cells.len() || fixed(c))
        .collect();
    for _ in 0..TAUT_PASSES {
        for i in 1..cells.len().saturating_sub(1) {
            if fixed[i] {
                continue;
            }
            let p = points[i];
            let (a, b) = (points[i - 1], points[i + 1]);
            let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            // The point stays in its cell.
            for t in [1.0, 0.5, 0.25] {
                let q = [p[0] + (m[0] - p[0]) * t, p[1] + (m[1] - p[1]) * t];
                if grid.find_cell(q[0], q[1]) == cells[i] {
                    points[i] = q;
                    break;
                }
            }
        }
    }
    points
}

/// The length of a pinned road along its taut points.
fn path_length(input: &Input, cells: &[usize]) -> f64 {
    line_length(&taut_points(input, cells))
}

/// The passes of `taut_points`.
const TAUT_PASSES: usize = 8;

/// The cost factor of a step into a cell of an earlier leg of the same
/// road. A block can wall in a stop, so a reuse only costs more.
const REUSE_COST: f64 = 4.0;

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

    /// A path through the given cells in order.
    fn route_through(&self, stops: &[usize]) -> Option<Vec<usize>> {
        search_through(self.input.grid, stops, &|a, b| self.step(a, b))
    }
}

/// The cost of a step from a cell to a neighbour, or `None` if the step is
/// not possible.
type StepCost<'a> = &'a dyn Fn(usize, usize) -> Option<f64>;

/// The least-cost path from `from` to `to`, both included. A step into a
/// cell of `avoid` costs `REUSE_COST` times more.
fn search(
    grid: &Grid,
    from: usize,
    to: usize,
    avoid: &[bool],
    step: StepCost,
) -> Option<Vec<usize>> {
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
            let Some(mut s) = step(c, m) else {
                continue;
            };
            if avoid[m] && m != to {
                s *= REUSE_COST;
            }
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

/// A least-cost path through the given cells in order.
fn search_through(grid: &Grid, stops: &[usize], step: StepCost) -> Option<Vec<usize>> {
    let mut avoid = vec![false; grid.len()];
    let mut path = vec![stops[0]];
    for w in stops.windows(2) {
        let leg = search(grid, w[0], w[1], &avoid, step)?;
        for &c in &leg[1..] {
            path.push(c);
        }
        // A later leg goes back over this leg only where no other way
        // is near.
        for &c in &leg[..leg.len() - 1] {
            avoid[c] = true;
        }
    }
    Some(path)
}

/// Keep a land corridor for each pinned road, before the features are
/// marked. A pinned road needs a land path between its stops. With a
/// `target_length`, the shortest land path must also be short enough.
/// If the terrain does not give this, the water cells on the straight
/// legs between the stops become land at sea level. A cell where
/// `keep_water` is `true` stays water. `place_cells` holds the cell of
/// each pinned place, in config order.
pub fn keep_corridors(
    grid: &Grid,
    heights: &mut [f64],
    sea_level: f64,
    config: &Config,
    place_cells: &[usize],
    keep_water: &[bool],
) {
    let place_cell = |id: &str| -> Option<usize> {
        let i = config.pins.points.iter().position(|p| p.id == id)?;
        place_cells.get(i).copied()
    };
    for pin in &config.pins.roads {
        let mut stops = Vec::new();
        let mut known = true;
        let ids = std::iter::once(Waypoint::Place(pin.from.clone()))
            .chain(pin.via.iter().cloned())
            .chain(std::iter::once(Waypoint::Place(pin.to.clone())));
        for w in ids {
            match w {
                Waypoint::Place(id) => match place_cell(&id) {
                    Some(c) => stops.push(c),
                    None => known = false,
                },
                Waypoint::Point(p) => stops.push(grid.find_cell(p[0], p[1])),
            }
        }
        if !known {
            // The road stage gives the error.
            continue;
        }
        let land = |c: usize| heights[c] >= sea_level;
        let step = |a: usize, b: usize| land(b).then(|| dist(grid.points[a], grid.points[b]));
        let fixed = |c: usize| place_cells.contains(&c);
        let needed = match search_through(grid, &stops, &step) {
            None => true,
            Some(cells) => pin.target_length.is_some_and(|t| {
                line_length(&taut_line(grid, &cells, &fixed)) > t * (1.0 + pin.tolerance)
            }),
        };
        if !needed {
            continue;
        }
        for w in stops.windows(2) {
            let (a, b) = (grid.points[w[0]], grid.points[w[1]]);
            // Walk the leg in steps much shorter than a cell.
            let spacing = (grid.width * grid.height / grid.len() as f64).sqrt();
            let n = (dist(a, b) / (spacing / 8.0)).ceil().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f64 / n as f64;
                let c = grid.find_cell(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t);
                if heights[c] < sea_level && !keep_water[c] {
                    heights[c] = sea_level;
                }
            }
        }
    }
}

/// Route all roads.
pub fn build(input: &Input) -> Result<Routes, Error> {
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
        let cells = route_pinned(&router, input, format!("/pins/roads/{k}"), pin, &place_cell)?;
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
            true,
        ));
    }

    // The generated network of FMG: sea routes, roads, and trails.
    let sea = crate::routes::network(input, &router.river_of, &mut roads);
    Ok(Routes { roads, sea })
}

/// Route more roads on a finished world, for example the roads of an
/// overlay. They reuse the cells of the `existing` roads at a lower cost.
/// `path` is the JSON Pointer of the list of roads, for errors. Each road
/// gets its config id in `overlay`.
pub fn route_more(
    input: &Input,
    existing: &[Road],
    roads: &[RoadPin],
    path: &str,
) -> Result<Vec<Road>, Error> {
    let n = input.grid.len();
    let mut river_of = vec![0u32; n];
    for r in input.rivers {
        for &c in &r.cells {
            river_of[c] = r.id;
        }
    }
    let mut used = vec![false; n];
    for r in existing {
        for &c in &r.cells {
            used[c] = true;
        }
    }
    let mut router = Router {
        input,
        river_of,
        used,
        distance_only: false,
    };
    let place_cell = |id: &str| -> Option<usize> {
        input
            .society
            .places
            .iter()
            .find(|p| p.pin.as_deref() == Some(id))
            .map(|p| p.cell)
    };
    let mut out = Vec::new();
    for (k, pin) in roads.iter().enumerate() {
        let cells = route_pinned(&router, input, format!("{path}/{k}"), pin, &place_cell)?;
        for &c in &cells {
            router.used[c] = true;
        }
        let mut road = finish(
            input,
            &router.river_of,
            (existing.len() + out.len()) as u32 + 1,
            None,
            pin.kind,
            cells,
            true,
        );
        road.overlay = Some(pin.id.clone());
        out.push(road);
    }
    Ok(out)
}

fn route_pinned(
    router: &Router,
    input: &Input,
    path: String,
    pin: &RoadPin,
    place_cell: &dyn Fn(&str) -> Option<usize>,
) -> Result<Vec<usize>, Error> {
    let grid = input.grid;
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
    let len0 = path_length(input, &base);
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
    let len0 = path_length(input, &base);
    if len0 >= lo {
        // In the tolerance, or still too long: a road cannot be shorter
        // than its shortest path.
        return Ok(base);
    }

    // A detour: a waypoint at distance `d` to one side of a point of a
    // leg. The search tries the longest leg first, and on each leg the
    // middle first, then the quarter points.
    let mut legs: Vec<(usize, f64)> = stops
        .windows(2)
        .enumerate()
        .map(|(i, w)| (i, dist(grid.points[w[0]], grid.points[w[1]])))
        .collect();
    legs.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let (w, h) = (grid.width, grid.height);
    let detour = |leg: usize, at: f64, d: f64, side: f64| -> Option<(Vec<usize>, f64)> {
        let a = grid.points[stops[leg]];
        let b = grid.points[stops[leg + 1]];
        let len_ab = dist(a, b).max(1e-9);
        let normal = [-(b[1] - a[1]) / len_ab, (b[0] - a[0]) / len_ab];
        let base = [a[0] + (b[0] - a[0]) * at, a[1] + (b[1] - a[1]) * at];
        let p = [
            base[0] + normal[0] * d * side,
            base[1] + normal[1] * d * side,
        ];
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
        let l = path_length(input, &cells);
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
    let found = |best: &Option<(f64, Vec<usize>)>| best.as_ref().is_some_and(|(m, _)| *m == 0.0);
    'search: for &(leg, _) in &legs {
        for at in [0.5, 0.25, 0.75] {
            for side in [1.0, -1.0] {
                let mut prev: Option<(f64, f64)> = None;
                for i in 1..=steps {
                    let d = max_d * i as f64 / steps as f64;
                    let Some((cells, l)) = detour(leg, at, d, side) else {
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
                            let Some((cells, lm)) = detour(leg, at, xm, side) else {
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
                    if found(&best) {
                        break 'search;
                    }
                    if l > hi {
                        // A longer offset gives a longer road.
                        break;
                    }
                    prev = Some((d, l));
                }
            }
        }
    }
    Ok(best.map_or(base, |(_, cells)| cells))
}

/// Make a road from its cells: points, length, stops, and crossings.
pub(crate) fn finish(
    input: &Input,
    river_of: &[u32],
    id: u32,
    pin: Option<usize>,
    kind: RoadKind,
    cells: Vec<usize>,
    taut: bool,
) -> Road {
    let grid = input.grid;
    let points: Vec<[f64; 2]> = if taut {
        taut_points(input, &cells)
    } else {
        cells.iter().map(|&c| grid.points[c]).collect()
    };
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
        overlay: None,
        kind,
        group: match kind {
            RoadKind::Road => "roads",
            RoadKind::Trail => "trails",
        },
        name: None,
        feature: cells.first().map_or(0, |&c| input.features.of_cell[c]),
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

    /// A book layout: a road from a city through a village on the coast to
    /// a port.
    fn book(cells: u32) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 1717, "frame": { "width": 400, "height": 300 },
            "world": { "cells": cells, "template": "pangea", "culture_set": "high-fantasy",
                       "cultures": 5, "states": 4, "towns": 25,
                       "places": { "min_distance_from_pins": 10 } },
            "pins": {
                "points": [
                    { "id": "kingsholm", "kind": "city", "at": [150, 120], "capital_of": "The Kingdom" },
                    { "id": "harbour", "kind": "city", "at": [22, 180], "port": true, "capital_of": "The Free Port" },
                    { "id": "ford", "kind": "village", "at": [130, 200] },
                    { "id": "old-keep", "kind": "fortress", "at": [230, 100] },
                    { "id": "barrow", "kind": "ruin", "at": [300, 180] }
                ],
                "regions": [
                    { "id": "western-sea", "kind": "sea", "box": [0, 0, 20, 300] },
                    { "id": "grey-teeth", "kind": "mountains",
                      "polygon": [[200, 40], [270, 50], [260, 130], [210, 120]], "edge": 12 },
                    { "id": "wolds", "kind": "hills", "circle": { "at": [240, 220], "r": 30 },
                      "height": [35, 50], "edge": 8 },
                    { "id": "low-fen", "kind": "marsh",
                      "polygon": [[60, 230], [100, 225], [105, 260], [65, 265]] }
                ],
                "rivers": [
                    { "id": "dunn", "source": { "region": "grey-teeth" },
                      "through": [{ "place": "kingsholm" }], "mouth": [19, 110] },
                    { "id": "brann", "source": { "region": "wolds" },
                      "through": [{ "place": "ford" }], "mouth": { "region": "low-fen" } },
                    { "id": "corrin", "source": [120, 150], "mouth": { "place": "harbour" } },
                    { "id": "tey", "source": [110, 260], "mouth": { "place": "harbour" } }
                ],
                "roads": [
                    { "id": "kings-road", "from": "kingsholm", "to": "harbour", "via": ["ford"],
                      "target_length": 240, "tolerance": 0.05 }
                ]
            }
        }))
        .unwrap()
    }

    #[test]
    fn a_later_leg_can_go_back_over_an_earlier_leg() {
        // At 50000 cells the first leg walls in the village. Before, the
        // second leg could not leave it, and the build failed.
        let w = World::build(&book(50_000)).unwrap();
        let ids: Vec<&str> = pinned(&w, 0)
            .stops
            .iter()
            .map(|s| s.place.as_str())
            .collect();
        assert_eq!(ids, ["kingsholm", "ford", "harbour"]);
    }

    #[test]
    fn a_road_meets_its_target_at_each_cell_count() {
        for cells in [10_000, 20_000, 30_000, 50_000] {
            let w = World::build(&book(cells)).unwrap();
            let r = pinned(&w, 0);
            assert!(
                (r.length - 240.0).abs() <= 240.0 * 0.05,
                "{cells} cells: length {}",
                r.length
            );
            // The length is the length of the drawn line.
            let drawn: f64 = r
                .points
                .windows(2)
                .map(|p| ((p[0][0] - p[1][0]).powi(2) + (p[0][1] - p[1][1]).powi(2)).sqrt())
                .sum();
            assert!((drawn - r.length).abs() < 1e-9);
            assert_eq!(*r.points.first().unwrap(), [150.0, 120.0]);
            assert_eq!(*r.points.last().unwrap(), [22.0, 180.0]);
        }
    }

    /// Two halves of land with a strait between them, and a road from one
    /// half to the other.
    fn strait(regions: serde_json::Value, target: Option<f64>) -> Config {
        let mut road = json!({ "id": "r", "from": "ostrey", "to": "harrowgate" });
        if let Some(t) = target {
            road["target_length"] = json!(t);
            road["tolerance"] = json!(0.05);
        }
        serde_json::from_value(json!({
            "version": 1, "seed": 11, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000,
                       "template": { "steps": ["Add 35 all", "Strait 6 vertical 0 0"] } },
            "pins": {
                "points": [
                    { "id": "ostrey", "kind": "city", "at": [60, 150] },
                    { "id": "harrowgate", "kind": "town", "at": [340, 150] }
                ],
                "regions": regions,
                "roads": [road]
            }
        }))
        .unwrap()
    }

    #[test]
    fn a_road_keeps_a_land_corridor_over_water() {
        // Without the road, the strait cuts the land in two.
        let mut c = strait(json!([]), None);
        c.pins.roads.clear();
        let w = World::build(&c).unwrap();
        let (a, b) = (w.pin_cells[0], w.pin_cells[1]);
        assert_ne!(w.features.of_cell[a], w.features.of_cell[b]);

        for target in [None, Some(290.0)] {
            let w = World::build(&strait(json!([]), target)).unwrap();
            let r = pinned(&w, 0);
            let ids: Vec<&str> = r.stops.iter().map(|s| s.place.as_str()).collect();
            assert_eq!(ids, ["ostrey", "harrowgate"]);
            assert!(r.cells.iter().all(|&c| w.features.is_land(c)));
            if let Some(t) = target {
                assert!((r.length - t).abs() <= t * 0.05, "length {}", r.length);
            }
        }
    }

    #[test]
    fn a_road_does_not_cross_a_pinned_sea() {
        let regions = json!([{ "id": "sea", "kind": "sea", "box": [150, 0, 250, 300] }]);
        let e = World::build(&strait(regions, None)).unwrap_err();
        assert_eq!(e.code, crate::error::ErrorCode::RoadNoPath);
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
