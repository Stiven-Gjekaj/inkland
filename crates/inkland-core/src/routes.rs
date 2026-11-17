//! The generated route network of FMG (`routes-generator.ts`,
//! `utils/path-utils.ts` `findPath`).
//!
//! - For each landmass, an Urquhart graph of its places: the Delaunay
//!   triangulation without the longest edge of each triangle.
//! - Sea routes join the ports of each water body, roads join the capitals
//!   of each landmass, and trails join all places of each landmass, in this
//!   order. Each edge of a graph is a least-cost path (Dijkstra).
//! - Land cost: squared distance, times a habitability factor (1 to 1.1), a
//!   height factor (2 to 3), 0.5 on a step that a route already uses, and 3
//!   into a cell without a place. Uninhabitable cells are closed.
//! - Water cost: squared distance, times a factor from the distance to the
//!   coast. A port leaves and enters through its haven cell. Navigable
//!   rivers are open along their course. Cold seas are closed.
//! - A path splits where it meets a step that a route already uses. Parts
//!   that follow each other merge into one route.
//! - inkland: pinned roads come first and count as routes that exist.

use crate::config::RoadKind;
use crate::grid::Grid;
use crate::names::adjective;
use crate::population::Cells;
use crate::rand::Rand;
use crate::rivers::Mouth;
use crate::roads::{Input, Road, finish};
use crate::seed::{Layer, SeedTree};
use crate::society::NONE;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

const SHARP_ANGLE: f64 = 135.0;
const VERY_SHARP_ANGLE: f64 = 115.0;
const MIN_PASSABLE_SEA_TEMP: f64 = -4.0;
const RIVER_TYPE_MODIFIER: f64 = 1.5;

const PREFIXES: [&str; 104] = [
    "King",
    "Queen",
    "Military",
    "Old",
    "New",
    "Ancient",
    "Royal",
    "Imperial",
    "Great",
    "Grand",
    "High",
    "Silver",
    "Dragon",
    "Shadow",
    "Star",
    "Mystic",
    "Whisper",
    "Eagle",
    "Golden",
    "Crystal",
    "Enchanted",
    "Frost",
    "Moon",
    "Sun",
    "Thunder",
    "Phoenix",
    "Sapphire",
    "Celestial",
    "Wandering",
    "Echo",
    "Twilight",
    "Crimson",
    "Serpent",
    "Iron",
    "Forest",
    "Flower",
    "Whispering",
    "Eternal",
    "Frozen",
    "Rain",
    "Luminous",
    "Stardust",
    "Arcane",
    "Glimmering",
    "Jade",
    "Ember",
    "Azure",
    "Gilded",
    "Divine",
    "Shadowed",
    "Cursed",
    "Moonlit",
    "Sable",
    "Everlasting",
    "Amber",
    "Nightshade",
    "Wraith",
    "Scarlet",
    "Platinum",
    "Whirlwind",
    "Obsidian",
    "Ethereal",
    "Ghost",
    "Spike",
    "Dusk",
    "Raven",
    "Spectral",
    "Burning",
    "Verdant",
    "Copper",
    "Velvet",
    "Falcon",
    "Enigma",
    "Glowing",
    "Silvered",
    "Molten",
    "Radiant",
    "Astral",
    "Wild",
    "Flame",
    "Amethyst",
    "Aurora",
    "Shadowy",
    "Solar",
    "Lunar",
    "Whisperwind",
    "Fading",
    "Titan",
    "Dawn",
    "Crystalline",
    "Jeweled",
    "Sylvan",
    "Twisted",
    "Ebon",
    "Thorn",
    "Cerulean",
    "Halcyon",
    "Infernal",
    "Storm",
    "Eldritch",
    "Sapphire",
    "Crimson",
    "Tranquil",
    "Paved",
];
const DESCRIPTORS: [&str; 16] = [
    "Great",
    "Shrouded",
    "Sacred",
    "Fabled",
    "Frosty",
    "Winding",
    "Echoing",
    "Serpentine",
    "Breezy",
    "Misty",
    "Rustic",
    "Silent",
    "Cobbled",
    "Cracked",
    "Shaky",
    "Obscure",
];

fn models(group: &str) -> &'static [(&'static str, u32)] {
    match group {
        "trails" => &[
            ("burg_suffix", 8),
            ("prefix_suffix", 1),
            ("the_descriptor_burg_suffix", 1),
        ],
        "searoutes" => &[
            ("burg_suffix", 4),
            ("prefix_suffix", 2),
            ("the_descriptor_prefix_suffix", 1),
        ],
        _ => &[
            ("burg_suffix", 3),
            ("prefix_suffix", 6),
            ("the_descriptor_prefix_suffix", 2),
            ("the_descriptor_burg_suffix", 1),
        ],
    }
}

fn suffixes(group: &str) -> &'static [(&'static str, u32)] {
    match group {
        "trails" => &[("trail", 4), ("path", 1), ("track", 1), ("pass", 1)],
        "searoutes" => &[("route", 5), ("lane", 2), ("passage", 1), ("water way", 1)],
        _ => &[("road", 7), ("route", 3), ("way", 2), ("highway", 1)],
    }
}

/// The edges of the Urquhart graph of points (`calculateUrquhartEdges`).
pub fn urquhart(points: &[[f64; 2]]) -> Vec<(usize, usize)> {
    if points.len() < 2 {
        return Vec::new();
    }
    if points.len() == 2 {
        return vec![(0, 1)];
    }
    let pts: Vec<delaunator::Point> = points
        .iter()
        .map(|p| delaunator::Point { x: p[0], y: p[1] })
        .collect();
    let t = delaunator::triangulate(&pts);
    let n = t.triangles.len();
    let score = |a: usize, b: usize| {
        let (p, q) = (points[a], points[b]);
        (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)
    };
    let pair = |e: usize| {
        let h = t.halfedges[e];
        if h == delaunator::EMPTY { e } else { e.max(h) }
    };
    let mut removed = vec![false; n];
    for e in (0..n).step_by(3) {
        let (p0, p1, p2) = (t.triangles[e], t.triangles[e + 1], t.triangles[e + 2]);
        let (p01, p12, p20) = (score(p0, p1), score(p1, p2), score(p2, p0));
        let k = if p20 > p01 && p20 > p12 {
            pair(e + 2)
        } else if p12 > p01 && p12 > p20 {
            pair(e + 1)
        } else {
            pair(e)
        };
        removed[k] = true;
    }
    let mut edges = Vec::new();
    for e in 0..n {
        let h = t.halfedges[e];
        if (h == delaunator::EMPTY || e > h) && !removed[e] {
            let t0 = t.triangles[e];
            let t1 = t.triangles[if e % 3 == 2 { e - 2 } else { e + 1 }];
            edges.push((t0, t1));
        }
    }
    edges
}

struct Key(f64, u64);
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
        self.0.total_cmp(&other.0).then(self.1.cmp(&other.1))
    }
}

struct Net<'a> {
    grid: &'a Grid,
    cells: &'a Cells,
    habitability: Vec<f64>,
    temperature: &'a [f64],
    /// The place on each cell, or 0.
    burg: Vec<u32>,
    /// The neighbors that a route already joins to each cell.
    conn: Vec<Vec<u32>>,
    /// The neighbors along a river course from each cell.
    river_edges: Vec<Vec<u32>>,
    // Search buffers.
    cost: Vec<f64>,
    from: Vec<usize>,
    touched: Vec<usize>,
}

impl Net<'_> {
    fn connected(&self, a: usize, b: usize) -> bool {
        self.conn[a].contains(&(b as u32))
    }

    fn connect(&mut self, segment: &[usize]) {
        for w in segment.windows(2) {
            let (a, b) = (w[0], w[1]);
            if !self.connected(a, b) {
                self.conn[a].push(b as u32);
            }
            if !self.connected(b, a) {
                self.conn[b].push(a as u32);
            }
        }
    }

    fn d2(&self, a: usize, b: usize) -> f64 {
        let (p, q) = (self.grid.points[a], self.grid.points[b]);
        let px = self.cells.px;
        ((p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1])) * px * px
    }

    /// `getLandPathCost`.
    fn land_cost(&self, current: usize, next: usize) -> f64 {
        if !self.cells.land(next) {
            return f64::INFINITY;
        }
        let habitability = self.habitability[next];
        if habitability == 0.0 {
            return f64::INFINITY;
        }
        let habitability_mod = 1.0 + (100.0 - habitability).max(0.0) / 1000.0;
        let height_mod = 1.0 + (self.cells.h[next] - 25.0).max(25.0) / 25.0;
        let connection_mod = if self.connected(current, next) {
            0.5
        } else {
            1.0
        };
        let burg_mod = if self.burg[next] != NONE { 1.0 } else { 3.0 };
        self.d2(current, next) * habitability_mod * height_mod * connection_mod * burg_mod
    }

    /// `getWaterPathCost`.
    fn water_cost(&self, current: usize, next: usize) -> f64 {
        let cells = self.cells;
        let connection_mod = if self.connected(current, next) {
            0.5
        } else {
            1.0
        };
        let river_edge = self.river_edges[current].contains(&(next as u32));
        if cells.land(next) {
            if !cells.navigable(next) || !river_edge {
                return f64::INFINITY;
            }
            return self.d2(current, next) * RIVER_TYPE_MODIFIER * connection_mod;
        }
        if cells.land(current) {
            if cells.r[current] != 0 {
                if !river_edge {
                    return f64::INFINITY;
                }
            } else if let Some(h) = cells.haven[current]
                && h != next
            {
                return f64::INFINITY;
            }
        }
        if self.temperature[next].trunc() < MIN_PASSABLE_SEA_TEMP {
            return f64::INFINITY;
        }
        let type_mod = match cells.t[next] {
            -1 => 1.0,
            -2 => 1.8,
            -3 => 4.0,
            -4 => 6.0,
            _ => 8.0,
        };
        self.d2(current, next) * type_mod * connection_mod
    }

    /// `true` if a water route may reach `exit` from `current`
    /// (`createWaterExitCheck`).
    fn water_exit(&self, exit: usize, next: usize, current: usize) -> bool {
        if next != exit {
            return false;
        }
        if self.river_edges[current].contains(&(next as u32)) {
            return true;
        }
        if self.cells.land(current) {
            return false;
        }
        self.cells.haven[exit].is_none_or(|h| h == current)
    }

    /// The least-cost path from `start` to `exit` (`findPath`).
    fn find_path(&mut self, start: usize, exit: usize, water: bool) -> Option<Vec<usize>> {
        if start == exit {
            return None;
        }
        for &c in &self.touched {
            self.cost[c] = f64::INFINITY;
            self.from[c] = usize::MAX;
        }
        self.touched.clear();
        let mut heap = BinaryHeap::new();
        let mut seq = 0u64;
        heap.push(Reverse((Key(0.0, seq), start)));
        while let Some(Reverse((Key(current_cost, _), current))) = heap.pop() {
            if current != start && current_cost > self.cost[current] {
                continue;
            }
            for next in self.grid.neighbors(current) {
                let is_exit = if water {
                    self.water_exit(exit, next, current)
                } else {
                    next == exit
                };
                if is_exit {
                    let mut path = vec![next, current];
                    let mut c = current;
                    while c != start {
                        c = self.from[c];
                        path.push(c);
                    }
                    path.reverse();
                    return Some(path);
                }
                let step = if water {
                    self.water_cost(current, next)
                } else {
                    self.land_cost(current, next)
                };
                if step == f64::INFINITY {
                    continue;
                }
                let total = current_cost + step;
                if total >= self.cost[next] {
                    continue;
                }
                if self.cost[next] == f64::INFINITY {
                    self.touched.push(next);
                }
                self.cost[next] = total;
                self.from[next] = current;
                seq += 1;
                heap.push(Reverse((Key(total, seq), next)));
            }
        }
        None
    }

    /// Split a path where it meets a step that a route already uses
    /// (`getRouteSegments`).
    fn segments(&self, path: &[usize]) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        let mut segment: Vec<usize> = Vec::new();
        for i in 0..path.len() {
            let c = path[i];
            let joined = path
                .get(i + 1)
                .is_some_and(|&next| self.connected(c, next) || self.connected(next, c));
            if joined {
                if !segment.is_empty() {
                    segment.push(c);
                    out.push(std::mem::take(&mut segment));
                }
                continue;
            }
            segment.push(c);
        }
        if segment.len() > 1 {
            out.push(segment);
        }
        out
    }
}

/// Join the parts where the last cell of one is the first cell of a later
/// one (`mergeRoutes`).
fn merge(mut routes: Vec<(u32, Vec<usize>, bool)>) -> Vec<(u32, Vec<usize>)> {
    loop {
        let mut merged = 0;
        for i in 0..routes.len() {
            if routes[i].2 {
                continue;
            }
            for j in i + 1..routes.len() {
                if routes[j].2 {
                    continue;
                }
                if routes[j].1.first() == routes[i].1.last() {
                    merged += 1;
                    let tail = routes[j].1[1..].to_vec();
                    routes[i].1.extend(tail);
                    routes[j].2 = true;
                }
            }
        }
        if merged <= 1 {
            break;
        }
    }
    routes
        .into_iter()
        .filter(|r| !r.2)
        .map(|(f, c, _)| (f, c))
        .collect()
}

/// The generated routes (`generate`). Generated roads and trails go to
/// `roads`; the result is the sea routes.
pub fn network(input: &Input, river_of: &[u32], roads: &mut Vec<Road>) -> Vec<Road> {
    let grid = input.grid;
    let soc = input.society;
    let cells = &soc.cells;
    let n = grid.len();
    let mut burg = vec![NONE; n];
    for p in &soc.places {
        burg[p.cell] = p.id;
    }
    let mut river_edges: Vec<Vec<u32>> = vec![Vec::new(); n];
    for r in input.rivers {
        let mut course = r.cells.clone();
        if let Mouth::Water(c) = r.mouth {
            course.push(c);
        }
        for w in course.windows(2) {
            river_edges[w[0]].push(w[1] as u32);
            river_edges[w[1]].push(w[0] as u32);
        }
    }
    let mut net = Net {
        grid,
        cells,
        habitability: input
            .biomes
            .iter()
            .map(|&b| crate::biomes::HABITABILITY[b as usize])
            .collect(),
        temperature: input.temperature,
        burg,
        conn: vec![Vec::new(); n],
        river_edges,
        cost: vec![f64::INFINITY; n],
        from: vec![usize::MAX; n],
        touched: Vec::new(),
    };
    for r in roads.iter() {
        net.connect(&r.cells);
    }

    // Places by landmass, capitals by landmass, ports by water body.
    let mut by_land: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut capitals: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut ports: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (k, p) in soc.places.iter().enumerate() {
        by_land.entry(p.landmass).or_default().push(k);
        if p.capital {
            capitals.entry(p.landmass).or_default().push(k);
        }
        if p.port && p.water != NONE {
            ports.entry(p.water).or_default().push(k);
        }
    }
    let group = |net: &mut Net, sets: &BTreeMap<u32, Vec<usize>>, water: bool| {
        let mut out: Vec<(u32, Vec<usize>, bool)> = Vec::new();
        for (&feature, list) in sets {
            let points: Vec<[f64; 2]> = list.iter().map(|&k| soc.places[k].at).collect();
            for (a, b) in urquhart(&points) {
                let start = soc.places[list[a]].cell;
                let exit = soc.places[list[b]].cell;
                let Some(path) = net.find_path(start, exit, water) else {
                    continue;
                };
                for segment in net.segments(&path) {
                    net.connect(&segment);
                    out.push((feature, segment, false));
                }
            }
        }
        merge(out)
    };
    let sea_routes = group(&mut net, &ports, true);
    let main_roads = group(&mut net, &capitals, false);
    let trails = group(&mut net, &by_land, false);

    // Points: the place position on a place cell, the site elsewhere; sharp
    // angles of roads and trails are cut (`getPoints`).
    let mut anchors: Vec<[f64; 2]> = grid.points.clone();
    for p in &soc.places {
        anchors[p.cell] = p.at;
    }
    let mut rng = Rand::new(SeedTree::new(input.config.seed).part_rng(Layer::Roads, "names"));
    let mut sea = Vec::new();
    for (kind, group_name, list) in [
        (RoadKind::Road, "roads", main_roads),
        (RoadKind::Trail, "trails", trails),
        (RoadKind::Road, "searoutes", sea_routes),
    ] {
        for (feature, route_cells) in list {
            let points = if group_name == "searoutes" {
                route_cells.iter().map(|&c| anchors[c]).collect()
            } else {
                smooth(grid, &net.burg, &route_cells, &mut anchors)
            };
            let id = if group_name == "searoutes" {
                sea.len() as u32 + 1
            } else {
                roads.len() as u32 + 1
            };
            let mut road = finish(input, river_of, id, None, kind, route_cells.clone());
            road.group = group_name;
            road.feature = feature;
            road.name = route_name(group_name, &route_cells, soc, &net.burg, &mut rng);
            road.length = points
                .windows(2)
                .map(|w: &[[f64; 2]]| {
                    ((w[0][0] - w[1][0]).powi(2) + (w[0][1] - w[1][1]).powi(2)).sqrt()
                })
                .sum();
            road.points = points;
            if group_name == "searoutes" {
                road.crossings.clear();
                sea.push(road);
            } else {
                roads.push(road);
            }
        }
    }
    sea
}

/// Route points with sharp angles cut. A moved point is kept for the other
/// routes through the cell.
fn smooth(grid: &Grid, burg: &[u32], route: &[usize], anchors: &mut [[f64; 2]]) -> Vec<[f64; 2]> {
    let mut data: Vec<[f64; 2]> = route.iter().map(|&c| anchors[c]).collect();
    for i in 1..route.len().saturating_sub(1) {
        let cell = route[i];
        if burg[cell] != NONE {
            continue;
        }
        let ([px, py], [cx, cy], [nx, ny]) = (data[i - 1], data[i], data[i + 1]);
        let (ax, ay, bx, by) = (px - cx, py - cy, nx - cx, ny - cy);
        let angle = (libm::atan2(ax * by - ay * bx, ax * bx + ay * by) * 180.0
            / std::f64::consts::PI)
            .abs();
        if angle < SHARP_ANGLE {
            let (mx, my) = ((px + nx) / 2.0, (py + ny) / 2.0);
            let (x, y) = if angle < VERY_SHARP_ANGLE {
                ((cx + mx * 2.0) / 3.0, (cy + my * 2.0) / 3.0)
            } else {
                ((cx + mx) / 2.0, (cy + my) / 2.0)
            };
            if grid.find_cell(x, y) == cell {
                data[i] = [x, y];
                anchors[cell] = [x, y];
            }
        }
    }
    data
}

/// A route name (`generateName`). A route of fewer than 4 points has none.
fn route_name(
    group: &str,
    route: &[usize],
    soc: &crate::society::Society,
    burg: &[u32],
    rng: &mut Rand,
) -> Option<String> {
    if route.len() < 4 {
        return None;
    }
    let model = rng.weighted(models(group));
    let suffix = rng.weighted(suffixes(group));
    let mut priority = vec![route[route.len() - 1], route[0]];
    priority.extend(route[1..route.len() - 1].iter().rev());
    let place = priority
        .iter()
        .find(|&&c| burg[c] != NONE)
        .map(|&c| &soc.places[burg[c] as usize - 1].name);
    if let Some(name) = place {
        let adj = adjective(name, rng);
        if model == "burg_suffix" {
            return Some(format!("{adj} {suffix}"));
        }
        if model == "the_descriptor_burg_suffix" {
            let d = rng.pick(&DESCRIPTORS);
            return Some(format!("The {d} {adj} {suffix}"));
        }
    }
    if model == "the_descriptor_prefix_suffix" {
        let d = rng.pick(&DESCRIPTORS);
        let p = rng.pick(&PREFIXES);
        return Some(format!("The {d} {p} {suffix}"));
    }
    Some(format!("{} {suffix}", rng.pick(&PREFIXES)))
}

/// How well a cell is joined to the routes (`getConnectivityRate`): 0 for
/// no route, else 0.8 and more for each route step from the cell.
pub fn connectivity(roads: &[Road], sea: &[Road], n: usize) -> Vec<f64> {
    let mut links: Vec<BTreeMap<usize, &'static str>> = vec![BTreeMap::new(); n];
    for r in roads.iter().chain(sea) {
        for w in r.cells.windows(2) {
            if w[0] != w[1] {
                links[w[0]].insert(w[1], r.group);
                links[w[1]].insert(w[0], r.group);
            }
        }
    }
    links
        .iter()
        .map(|l| {
            if l.is_empty() {
                return 0.0;
            }
            l.values().fold(0.8, |acc, g| {
                acc + match *g {
                    "roads" | "searoutes" => 0.2,
                    _ => 0.1,
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urquhart_drops_the_longest_edge_of_each_triangle() {
        // A long thin triangle: the long edge 0-2 goes.
        let e = urquhart(&[[0.0, 0.0], [5.0, 1.0], [10.0, 0.0]]);
        assert_eq!(e.len(), 2);
        assert!(!e.contains(&(0, 2)) && !e.contains(&(2, 0)));
        assert_eq!(urquhart(&[[0.0, 0.0], [1.0, 1.0]]), vec![(0, 1)]);
        assert!(urquhart(&[[0.0, 0.0]]).is_empty());
    }

    #[test]
    fn parts_that_follow_each_other_merge() {
        let r = merge(vec![
            (1, vec![1, 2, 3], false),
            (1, vec![3, 4], false),
            (1, vec![7, 8], false),
        ]);
        assert_eq!(r, vec![(1, vec![1, 2, 3, 4]), (1, vec![7, 8])]);
    }

    #[test]
    fn roads_stay_on_land_and_sea_routes_join_ports() {
        let c: crate::config::Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 4, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "archipelago" }
        }))
        .unwrap();
        let w = crate::world::World::build(&c).unwrap();
        let made: Vec<&Road> = w.roads.iter().filter(|r| r.pin.is_none()).collect();
        assert!(made.iter().any(|r| r.group == "roads"));
        assert!(made.iter().any(|r| r.group == "trails"));
        for r in &made {
            assert!(r.cells.iter().all(|&c| w.features.is_land(c)), "{}", r.id);
            for pair in r.cells.windows(2) {
                assert!(w.grid.neighbors(pair[0]).any(|m| m == pair[1]));
            }
        }
        assert!(!w.sea_routes.is_empty());
        for r in &w.sea_routes {
            assert_eq!(r.group, "searoutes");
            // A sea route goes over water and navigable rivers. Its end
            // cells are port cells.
            let cells = &w.society.cells;
            let inner = &r.cells[1..r.cells.len() - 1];
            assert!(
                inner.iter().all(|&c| !cells.land(c) || cells.navigable(c)),
                "{}",
                r.id
            );
        }
        let ports: Vec<usize> = w
            .society
            .places
            .iter()
            .filter(|p| p.port)
            .map(|p| p.cell)
            .collect();
        let ends = w
            .sea_routes
            .iter()
            .flat_map(|r| [r.cells[0], r.cells[r.cells.len() - 1]]);
        let at_port = ends.filter(|c| ports.contains(c)).count();
        assert!(at_port > 0);
    }
}
