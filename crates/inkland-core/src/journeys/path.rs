//! The pathfinding of FMG journeys (`journeys-generator.ts` `findPath`).
//!
//! - Land: the route network first (a search over the route links that
//!   stays on land), else an A* search over the cells. A step costs its
//!   length, times the biome and height of the next cell, times 0.5 on a
//!   route cell (5 when the segment avoids roads).
//! - Water: the least-cost search of the sea routes (FMG
//!   `Routes.findWaterPath`). Ports leave and enter through their haven,
//!   navigable rivers are open along their course, cold seas are closed.
//! - Air and stay: a direct line.

use super::map::Map;
use super::{Domain, Point};
use crate::cultures::BIOME_COST;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

const ON_ROAD_DISCOUNT: f64 = 0.5;
const OFF_ROAD_PENALTY: f64 = 5.0;
const BASELINE_BIOME_COST: f64 = 50.0;
const HEIGHT_COST_THRESHOLD: f64 = 25.0;
const MIN_PASSABLE_SEA_TEMP: f64 = -4.0;
const RIVER_TYPE_MODIFIER: f64 = 1.5;

/// Why a path search fails (FMG `errorCode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathError {
    /// A water endpoint is landlocked.
    NoWater,
    /// A land endpoint is in water.
    NoLand,
    /// No sea path joins the endpoints.
    NoWaterPath,
    /// No land path joins the endpoints.
    NoLandPath,
}

/// The result of a path search (FMG `PathfindingResult`).
#[derive(Debug, Clone, PartialEq)]
pub struct PathResult {
    pub points: Vec<Point>,
    /// The length of the path, in world units.
    pub distance: f64,
    pub warning: Option<&'static str>,
    pub error: Option<PathError>,
}

impl PathResult {
    fn fail(error: PathError, warning: &'static str) -> PathResult {
        PathResult {
            points: Vec::new(),
            distance: 0.0,
            warning: Some(warning),
            error: Some(error),
        }
    }

    fn of(points: Vec<Point>) -> PathResult {
        let distance = path_length(&points);
        PathResult {
            points,
            distance,
            warning: None,
            error: None,
        }
    }
}

/// What a story journey needs from the pathfinding (FMG
/// `JourneyPathfinder`).
pub trait Pathfinder {
    fn find_path(&self, from: usize, to: usize, domain: Domain, avoid_roads: bool) -> PathResult;
    fn is_valid_path(&self, points: &[Point], domain: Domain) -> bool;
    fn path_length(&self, points: &[Point]) -> f64 {
        path_length(points)
    }
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
    libm::sqrt(dx * dx + dy * dy)
}

/// The length of a chain of points (`getPathLength`).
pub fn path_length(points: &[Point]) -> f64 {
    points.windows(2).map(|w| dist(w[0].at, w[1].at)).sum()
}

/// A cost in a heap: a total order, and the order of the push breaks a tie.
#[derive(PartialEq)]
struct Key(f64, u64);
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

impl Map {
    /// The site of a cell as a path point (`getPoint`).
    pub fn point(&self, cell: usize) -> Point {
        Point {
            at: self.p[cell],
            cell,
        }
    }

    /// `true` if a route goes through the cell (`hasRoute`).
    pub fn has_route(&self, cell: usize) -> bool {
        !self.links[cell].is_empty()
    }

    /// Land where a boat can put in: a coastal cell with a haven, or a
    /// navigable river cell (`isMoorage`).
    pub fn is_moorage(&self, cell: usize) -> bool {
        self.haven[cell].is_some() || self.navigable[cell]
    }

    /// `true` for land next to water (`isCoastalLand`).
    pub fn is_coastal_land(&self, cell: usize) -> bool {
        self.is_land(cell) && self.c[cell].iter().any(|&n| !self.is_land(n))
    }

    /// `true` if the cell can be an endpoint of the domain
    /// (`isValidEndpoint`).
    pub fn is_valid_endpoint(&self, cell: usize, domain: Domain) -> bool {
        match domain {
            Domain::Air | Domain::Stay => true,
            Domain::Land => self.is_land(cell),
            Domain::Water => !self.is_land(cell) || self.is_moorage(cell),
        }
    }

    /// `true` if a path of the domain can go through the cell. Water goes
    /// over land only on a navigable river (`isValidPathPoint`).
    pub fn is_valid_path_point(&self, cell: usize, domain: Domain) -> bool {
        match domain {
            Domain::Air | Domain::Stay => true,
            Domain::Land => self.is_land(cell),
            Domain::Water => !self.is_land(cell) || self.navigable[cell],
        }
    }

    /// `true` if each point of the path, except the endpoints, is valid
    /// for the domain (`isValidPath`).
    pub fn is_valid_path(&self, points: &[Point], domain: Domain) -> bool {
        let n = points.len();
        n < 3
            || points[1..n - 1]
                .iter()
                .all(|p| self.is_valid_path_point(p.cell, domain))
    }

    /// A path between two cells for a domain (`findPath`). A stay goes in
    /// a direct line, as air does.
    pub fn find_path(
        &self,
        from: usize,
        to: usize,
        domain: Domain,
        avoid_roads: bool,
    ) -> PathResult {
        if from == to {
            return PathResult::of(vec![self.point(from)]);
        }
        if domain == Domain::Land && (!self.is_land(from) || !self.is_land(to)) {
            return PathResult::fail(
                PathError::NoLand,
                "Land transport can only travel between land cells. At least one endpoint is in water.",
            );
        }
        if domain == Domain::Water
            && !(self.is_valid_endpoint(from, Domain::Water)
                && self.is_valid_endpoint(to, Domain::Water))
        {
            return PathResult::fail(
                PathError::NoWater,
                "Water transport needs a water cell, a coastal cell or a navigable river at both ends. At least one endpoint is landlocked.",
            );
        }
        match domain {
            Domain::Water => self.find_water_path(from, to),
            Domain::Land => self.find_land_path(from, to, avoid_roads),
            Domain::Air | Domain::Stay => PathResult::of(vec![self.point(from), self.point(to)]),
        }
    }

    fn find_land_path(&self, from: usize, to: usize, avoid_roads: bool) -> PathResult {
        if self.f[from] != self.f[to] {
            return PathResult::fail(
                PathError::NoLandPath,
                "No land route exists: the cells are on different landmasses.",
            );
        }
        if !avoid_roads && let Some(chain) = self.find_route_chain(from, to) {
            let points = self.collect_route_points(&chain);
            if self.is_valid_path(&points, Domain::Land) {
                return PathResult::of(points);
            }
        }
        let cost = |a: usize, b: usize| -> f64 {
            if !self.is_land(b) && b != to && b != from {
                return f64::INFINITY;
            }
            let c = dist(self.p[a], self.p[b]) * self.terrain_cost(b);
            if !self.has_route(b) {
                return c;
            }
            c * if avoid_roads {
                OFF_ROAD_PENALTY
            } else {
                ON_ROAD_DISCOUNT
            }
        };
        let Some(cells) = self.a_star(from, to, cost) else {
            return PathResult::fail(
                PathError::NoLandPath,
                "No land route found between these cells",
            );
        };
        let mut result = PathResult::of(cells.iter().map(|&c| self.point(c)).collect());
        if !avoid_roads {
            result.warning = Some("Segment leaves the road network");
        }
        result
    }

    /// The cost factor of the terrain of a cell: the biome cost over 50
    /// (at least 1), times 1 to 4 for the height (`getTerrainCost`).
    pub fn terrain_cost(&self, cell: usize) -> f64 {
        let biome_cost = BIOME_COST
            .get(usize::from(self.biome[cell]))
            .copied()
            .unwrap_or(BASELINE_BIOME_COST);
        let biome = (biome_cost / BASELINE_BIOME_COST).max(1.0);
        let height = 1.0 + (self.h[cell] - HEIGHT_COST_THRESHOLD).max(0.0) / HEIGHT_COST_THRESHOLD;
        biome * height
    }

    /// A* over the cells (`findPathAStar`). The heuristic is the straight
    /// distance times the road discount, the cheapest step factor.
    fn a_star(
        &self,
        start: usize,
        end: usize,
        cost: impl Fn(usize, usize) -> f64,
    ) -> Option<Vec<usize>> {
        let target = self.p[end];
        let heuristic = |c: usize| ON_ROAD_DISCOUNT * dist(self.p[c], target);
        let n = self.len();
        let mut from = vec![usize::MAX; n];
        let mut g = vec![f64::INFINITY; n];
        let mut closed = vec![false; n];
        let mut heap = BinaryHeap::new();
        let mut seq = 0u64;
        g[start] = 0.0;
        heap.push(Reverse((Key(heuristic(start), seq), start)));
        while let Some(Reverse((_, current))) = heap.pop() {
            if current == end {
                return Some(trace(&from, start, end));
            }
            if closed[current] {
                continue;
            }
            closed[current] = true;
            let current_g = g[current];
            for &next in &self.c[current] {
                if closed[next] {
                    continue;
                }
                let edge = cost(current, next);
                if edge == f64::INFINITY {
                    continue;
                }
                let tentative = current_g + edge;
                if tentative >= g[next] {
                    continue;
                }
                from[next] = current;
                g[next] = tentative;
                seq += 1;
                heap.push(Reverse((Key(tentative + heuristic(next), seq), next)));
            }
        }
        None
    }

    /// A breadth-first search over the route links for a chain of cells
    /// from `start` to `end` (`findRouteChain`). Cells between the ends
    /// must be land: the links hold the sea routes too.
    fn find_route_chain(&self, start: usize, end: usize) -> Option<Vec<usize>> {
        if self.links[start].is_empty() || self.links[end].is_empty() {
            return None;
        }
        let n = self.len();
        let mut from = vec![usize::MAX; n];
        let mut visited = vec![false; n];
        visited[start] = true;
        let mut queue = VecDeque::from([start]);
        while let Some(current) = queue.pop_front() {
            for &next in self.links[current].keys() {
                if visited[next] {
                    continue;
                }
                visited[next] = true;
                if next == end {
                    from[next] = current;
                    return Some(trace(&from, start, end));
                }
                if !self.is_land(next) {
                    continue;
                }
                from[next] = current;
                queue.push_back(next);
            }
        }
        None
    }

    /// The route geometry along a chain of cells, without a point twice
    /// in a row (`collectRoutePoints`).
    fn collect_route_points(&self, chain: &[usize]) -> Vec<Point> {
        let mut points: Vec<Point> = Vec::new();
        let mut push = |p: Point| {
            if points.last().is_none_or(|l| l.at != p.at) {
                points.push(p);
            }
        };
        for w in chain.windows(2) {
            let (a, b) = (w[0], w[1]);
            let route = self.links[a].get(&b).and_then(|&k| self.routes.get(k));
            let span = route.and_then(|r| {
                let i = r.iter().position(|p| p.cell == a)?;
                let j = r.iter().position(|p| p.cell == b)?;
                Some((r, i, j))
            });
            let Some((route, i, j)) = span else {
                push(self.point(a));
                push(self.point(b));
                continue;
            };
            if i <= j {
                route[i..=j].iter().for_each(|&p| push(p));
            } else {
                route[j..=i].iter().rev().for_each(|&p| push(p));
            }
        }
        if points.is_empty() {
            return vec![self.point(chain[0]), self.point(chain[chain.len() - 1])];
        }
        points
    }

    fn find_water_path(&self, from: usize, to: usize) -> PathResult {
        let Some(cells) = self.water_cells(from, to) else {
            return PathResult::fail(
                PathError::NoWaterPath,
                "No sea route found between these cells: they may be in different bodies of water.",
            );
        };
        // The anchors of the sea routes: the place position on a place
        // cell, else the cell site (`getWaterPoints`).
        let points = cells
            .iter()
            .map(|&cell| Point {
                at: self.anchor(cell),
                cell,
            })
            .collect();
        PathResult::of(points)
    }

    /// The point that a route goes through in a cell: the burg position on
    /// a burg cell, else the site (`getCellAnchor`).
    pub fn anchor(&self, cell: usize) -> [f64; 2] {
        match self.burg[cell] {
            0 => self.p[cell],
            id => self.burg_by_id(id).map_or(self.p[cell], |b| b.at),
        }
    }

    fn connected(&self, a: usize, b: usize) -> bool {
        self.connections[a].contains(&b)
    }

    fn river_edge(&self, a: usize, b: usize) -> bool {
        self.river_edges[a].contains(&b)
    }

    fn d2(&self, a: usize, b: usize) -> f64 {
        let (p, q) = (self.p[a], self.p[b]);
        (p[0] - q[0]) * (p[0] - q[0]) + (p[1] - q[1]) * (p[1] - q[1])
    }

    /// The cost of a water step (`Routes.getWaterPathCost`).
    fn water_cost(&self, current: usize, next: usize) -> f64 {
        let connection = if self.connected(current, next) {
            0.5
        } else {
            1.0
        };
        if self.is_land(next) {
            if !self.navigable[next] || !self.river_edge(current, next) {
                return f64::INFINITY;
            }
            return self.d2(current, next) * RIVER_TYPE_MODIFIER * connection;
        }
        if self.is_land(current) {
            if self.r[current] != 0 {
                if !self.river_edge(current, next) {
                    return f64::INFINITY;
                }
            } else if let Some(h) = self.haven[current]
                && h != next
            {
                return f64::INFINITY;
            }
        }
        if self.temp[next].trunc() < MIN_PASSABLE_SEA_TEMP {
            return f64::INFINITY;
        }
        let kind = match self.t[next] {
            -1 => 1.0,
            -2 => 1.8,
            -3 => 4.0,
            -4 => 6.0,
            _ => 8.0,
        };
        self.d2(current, next) * kind * connection
    }

    /// `true` if a water path may reach `exit` from `current`
    /// (`createWaterExitCheck`).
    fn water_exit(&self, exit: usize, next: usize, current: usize) -> bool {
        if next != exit {
            return false;
        }
        if self.river_edge(current, next) {
            return true;
        }
        if self.is_land(current) {
            return false;
        }
        self.haven[exit].is_none_or(|h| h == current)
    }

    /// The cells of the least-cost water path (`Routes.findWaterPath` with
    /// `findPath` of `path-utils.ts`).
    fn water_cells(&self, start: usize, exit: usize) -> Option<Vec<usize>> {
        if start == exit {
            return None;
        }
        let n = self.len();
        let mut cost = vec![f64::INFINITY; n];
        let mut from = vec![usize::MAX; n];
        let mut heap = BinaryHeap::new();
        let mut seq = 0u64;
        heap.push(Reverse((Key(0.0, seq), start)));
        while let Some(Reverse((Key(current_cost, _), current))) = heap.pop() {
            if current != start && current_cost > cost[current] {
                continue;
            }
            for &next in &self.c[current] {
                if self.water_exit(exit, next, current) {
                    from[next] = current;
                    return Some(trace(&from, start, next));
                }
                let step = self.water_cost(current, next);
                if step == f64::INFINITY {
                    continue;
                }
                let total = current_cost + step;
                if total >= cost[next] {
                    continue;
                }
                cost[next] = total;
                from[next] = current;
                seq += 1;
                heap.push(Reverse((Key(total, seq), next)));
            }
        }
        None
    }
}

/// Walk the predecessors back from `end` to `start`, and give the chain in
/// travel order (`tracePath`).
fn trace(from: &[usize], start: usize, end: usize) -> Vec<usize> {
    let mut chain = vec![end];
    let mut current = end;
    while current != start {
        current = from[current];
        chain.push(current);
    }
    chain.reverse();
    chain
}

impl Pathfinder for Map {
    fn find_path(&self, from: usize, to: usize, domain: Domain, avoid_roads: bool) -> PathResult {
        Map::find_path(self, from, to, domain, avoid_roads)
    }

    fn is_valid_path(&self, points: &[Point], domain: Domain) -> bool {
        Map::is_valid_path(self, points, domain)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A map of `n` cells with the given sites and neighbors. All cells
    /// are lowland grassland of one landmass, with no routes and no burgs.
    pub(crate) fn blank(p: Vec<[f64; 2]>, c: Vec<Vec<usize>>) -> Map {
        let n = p.len();
        Map {
            width: 1000.0,
            height: 1000.0,
            p,
            c,
            h: vec![25.0; n],
            f: vec![1; n],
            biome: vec![4; n],
            t: vec![1; n],
            haven: vec![None; n],
            r: vec![0; n],
            navigable: vec![false; n],
            temp: vec![20.0; n],
            river_edges: vec![Vec::new(); n],
            connections: vec![Vec::new(); n],
            links: vec![BTreeMap::new(); n],
            routes: Vec::new(),
            burg: vec![0; n],
            burgs: Vec::new(),
            states: vec![crate::journeys::map::NEUTRALS.to_string()],
            feature_names: vec![None; 2],
            lakes: vec![false; 2],
            chains: Vec::new(),
        }
    }

    /// Cells 0-1-2-3 in a row. Cell 2 is water, and a sea route joins
    /// 1-2-3.
    fn row() -> Map {
        let p = vec![[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0]];
        let c = vec![vec![1], vec![0, 2], vec![1, 3], vec![2]];
        let mut m = blank(p, c);
        m.h = vec![30.0, 30.0, 5.0, 30.0];
        m.f = vec![1, 1, 2, 3];
        m.links[1].insert(2, 0);
        m.links[2].insert(1, 0);
        m.links[2].insert(3, 0);
        m.links[3].insert(2, 0);
        m.routes.push(Vec::new());
        m
    }

    #[test]
    fn a_land_path_does_not_follow_a_sea_route_over_water() {
        let m = row();
        let r = m.find_path(1, 3, Domain::Land, false);
        assert!(r.points.iter().all(|p| m.is_land(p.cell)));
        assert_eq!(r.error, Some(PathError::NoLandPath));
        let r = m.find_path(0, 1, Domain::Land, false);
        assert_eq!(r.points.iter().map(|p| p.cell).collect::<Vec<_>>(), [0, 1]);
    }

    fn at(cell: usize) -> Point {
        Point {
            at: [cell as f64 * 10.0, 0.0],
            cell,
        }
    }

    #[test]
    fn valid_paths_skip_the_endpoints() {
        let mut m = row();
        m.f = vec![1; 4];
        assert!(!m.is_valid_path(&[at(0), at(2), at(3)], Domain::Land));
        assert!(m.is_valid_path(&[at(0), at(1), at(3)], Domain::Land));
        assert!(m.is_valid_path(&[at(0), at(2), at(3)], Domain::Water));
        assert!(!m.is_valid_path(&[at(2), at(1), at(2)], Domain::Water));
        m.navigable[1] = true;
        assert!(m.is_valid_path(&[at(2), at(1), at(2)], Domain::Water));
        assert!(m.is_valid_path(&[at(0), at(2), at(3)], Domain::Air));
        assert!(m.is_valid_path(&[at(0), at(2), at(3)], Domain::Stay));
        assert!(m.is_valid_path(&[at(0), at(3)], Domain::Land));
    }

    const GRID: usize = 9;

    fn cell_at(row: usize, col: usize) -> usize {
        row * GRID + col
    }

    /// A 9 x 9 land grid, 10 units apart, with four neighbors per cell.
    fn terrain() -> Map {
        let mut p = Vec::new();
        let mut c = Vec::new();
        for i in 0..GRID * GRID {
            let (row, col) = (i / GRID, i % GRID);
            p.push([col as f64 * 10.0, row as f64 * 10.0]);
            let mut n = Vec::new();
            if row > 0 {
                n.push(cell_at(row - 1, col));
            }
            if row < GRID - 1 {
                n.push(cell_at(row + 1, col));
            }
            if col > 0 {
                n.push(cell_at(row, col - 1));
            }
            if col < GRID - 1 {
                n.push(cell_at(row, col + 1));
            }
            c.push(n);
        }
        blank(p, c)
    }

    const START: usize = 4 * GRID;
    const END: usize = 4 * GRID + 8;

    #[test]
    fn a_path_goes_straight_over_even_lowland() {
        let m = terrain();
        let r = m.find_path(START, END, Domain::Land, true);
        assert_eq!(r.points.len(), 9);
        assert!(r.points.iter().all(|p| p.cell / GRID == 4));
        assert!((r.distance - 80.0).abs() < 1e-9);
    }

    #[test]
    fn a_path_goes_around_a_glacier_and_a_mountain() {
        for avoid in [true, false] {
            let mut m = terrain();
            for row in 3..=5 {
                for col in 3..=5 {
                    m.biome[cell_at(row, col)] = crate::biomes::GLACIER;
                }
            }
            let r = m.find_path(START, END, Domain::Land, avoid);
            let cells: Vec<usize> = r.points.iter().map(|p| p.cell).collect();
            assert!(cells.contains(&START) && cells.contains(&END));
            assert!(cells.iter().all(|&c| m.biome[c] != crate::biomes::GLACIER));
        }
        let mut m = terrain();
        for row in 3..=5 {
            for col in 3..=5 {
                m.h[cell_at(row, col)] = 100.0;
            }
        }
        let r = m.find_path(START, END, Domain::Land, true);
        assert!(r.points.iter().all(|p| m.h[p.cell] < 100.0));
    }

    #[test]
    fn a_path_crosses_a_glacier_when_it_must() {
        let mut m = terrain();
        for row in 0..GRID {
            m.biome[cell_at(row, 4)] = crate::biomes::GLACIER;
        }
        let r = m.find_path(START, END, Domain::Land, true);
        assert_eq!(r.error, None);
        assert!(
            r.points
                .iter()
                .any(|p| m.biome[p.cell] == crate::biomes::GLACIER)
        );
    }

    #[test]
    fn a_land_path_follows_a_road_with_its_points() {
        let mut m = terrain();
        // A road along row 4 with bent points.
        let cells: Vec<usize> = (0..GRID).map(|col| cell_at(4, col)).collect();
        let route: Vec<Point> = cells
            .iter()
            .map(|&cell| Point {
                at: [m.p[cell][0], m.p[cell][1] + 1.0],
                cell,
            })
            .collect();
        for w in cells.windows(2) {
            m.links[w[0]].insert(w[1], 0);
            m.links[w[1]].insert(w[0], 0);
        }
        m.routes.push(route.clone());
        let r = m.find_path(END, START, Domain::Land, false);
        assert_eq!(r.warning, None);
        let back: Vec<Point> = route.into_iter().rev().collect();
        assert_eq!(r.points, back);
        // Off-road, the path does not use the road points.
        let r = m.find_path(END, START, Domain::Land, true);
        assert!(r.points.iter().all(|p| p.at == m.p[p.cell]));
    }

    #[test]
    fn a_water_path_leaves_a_port_through_its_haven() {
        // Row 0 is land with a port at its middle; rows 1 and 2 are sea.
        let mut m = terrain();
        for c in 0..GRID * GRID {
            if c / GRID == 0 {
                m.h[c] = 30.0;
            } else {
                m.h[c] = 10.0;
                m.f[c] = 2;
                m.t[c] = -1;
            }
        }
        let (port, other) = (cell_at(0, 2), cell_at(0, 6));
        m.haven[port] = Some(cell_at(1, 2));
        m.haven[other] = Some(cell_at(1, 6));
        let r = m.find_path(port, other, Domain::Water, false);
        assert_eq!(r.error, None);
        let cells: Vec<usize> = r.points.iter().map(|p| p.cell).collect();
        assert_eq!(cells.first(), Some(&port));
        assert_eq!(cells[1], cell_at(1, 2));
        assert_eq!(cells[cells.len() - 2], cell_at(1, 6));
        assert!(m.is_valid_path(&r.points, Domain::Water));
        // An inland cell has no moorage.
        let r = m.find_path(cell_at(0, 0), other, Domain::Water, false);
        assert_eq!(r.error, Some(PathError::NoWater));
        // A cold sea is closed.
        m.temp = vec![-10.0; GRID * GRID];
        let r = m.find_path(port, other, Domain::Water, false);
        assert_eq!(r.error, Some(PathError::NoWaterPath));
    }

    #[test]
    fn air_goes_straight_and_one_cell_is_one_point() {
        let m = terrain();
        let r = m.find_path(0, 80, Domain::Air, false);
        assert_eq!(r.points.len(), 2);
        assert!((r.distance - libm::sqrt(2.0) * 80.0).abs() < 1e-9);
        let r = m.find_path(5, 5, Domain::Land, false);
        assert_eq!(r.points.len(), 1);
        assert_eq!(r.distance, 0.0);
    }
}
