//! Travel over the route network (`trade-routes.ts`): the cost of carriage
//! and the paths that goods follow.
//!
//! The search is a Dijkstra with a land state and a water state for each
//! cell. A sea link costs a fifth of its length. A change between land and
//! water costs more away from a port.

use super::Key;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// The cost of a sea link for each unit of length; land costs 1.
const WATER_COST: f64 = 0.2;
/// The cost to load between land and water at a port, in cell spacings.
const SWITCH_SPACINGS: f64 = 4.0;
/// The cost to load anywhere else that a road meets a river route.
const LANDING_SPACINGS: f64 = 40.0;

/// The route network as flat arrays: the links of each cell from
/// `offsets[cell]`, with their cost and mode (`RouteGraph`).
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub offsets: Vec<usize>,
    pub targets: Vec<usize>,
    pub costs: Vec<f64>,
    pub water: Vec<bool>,
    /// `true` for the cell of a port.
    pub port: Vec<bool>,
    pub switch_cost: f64,
    pub landing_cost: f64,
}

/// A search from one cell. The states are `cell * 2`, plus 1 when the
/// state arrives by water.
#[derive(Debug, Clone, PartialEq)]
pub struct Search {
    pub cost: Vec<f64>,
    /// The cell before each state, or `usize::MAX`.
    pub prev_cell: Vec<usize>,
    /// The state before each state, or `None` at the start.
    pub prev_state: Vec<Option<usize>>,
}

impl Graph {
    /// The graph of the links (`buildGraph`). `point` gives the position of
    /// a cell: the place on it, else the cell site. `spacing` is the cell
    /// spacing of the first grid.
    pub fn new(
        links: &[Vec<(usize, bool)>],
        point: &dyn Fn(usize) -> [f64; 2],
        port: Vec<bool>,
        spacing: f64,
    ) -> Graph {
        let n = links.len();
        let mut offsets = Vec::with_capacity(n + 1);
        let (mut targets, mut costs, mut water) = (Vec::new(), Vec::new(), Vec::new());
        for (cell, list) in links.iter().enumerate() {
            offsets.push(targets.len());
            if list.is_empty() {
                continue;
            }
            let [x1, y1] = point(cell);
            for &(next, w) in list {
                let [x2, y2] = point(next);
                targets.push(next);
                costs.push(libm::hypot(x2 - x1, y2 - y1) * if w { WATER_COST } else { 1.0 });
                water.push(w);
            }
        }
        offsets.push(targets.len());
        Graph {
            offsets,
            targets,
            costs,
            water,
            port,
            switch_cost: SWITCH_SPACINGS * spacing,
            landing_cost: LANDING_SPACINGS * spacing,
        }
    }

    /// The cheapest travel from a cell to each state (`search`).
    pub fn search(&self, start: usize, search: &mut Search) {
        search.cost.fill(f64::INFINITY);
        search.cost[start * 2] = 0.0;
        search.cost[start * 2 + 1] = 0.0;
        let mut queue: BinaryHeap<Reverse<(Key, u64, usize)>> = BinaryHeap::new();
        let mut seq = 0u64;
        let mut visit = |search: &mut Search,
                         queue: &mut BinaryHeap<Reverse<(Key, u64, usize)>>,
                         from: usize,
                         from_state: Option<usize>,
                         link: usize,
                         base: f64| {
            let water = self.water[link];
            let switching = from_state.is_some_and(|s| water != (s & 1 == 1));
            let extra = if switching {
                if self.port[from] {
                    self.switch_cost
                } else {
                    self.landing_cost
                }
            } else {
                0.0
            };
            let total = base + self.costs[link] + extra;
            let state = self.targets[link] * 2 + usize::from(water);
            if total >= search.cost[state] {
                return;
            }
            search.cost[state] = total;
            search.prev_cell[state] = from;
            search.prev_state[state] = from_state;
            queue.push(Reverse((Key(total), seq, state)));
            seq += 1;
        };
        for link in self.offsets[start]..self.offsets[start + 1] {
            visit(search, &mut queue, start, None, link, 0.0);
        }
        while let Some(Reverse((Key(cost), _, state))) = queue.pop() {
            if cost > search.cost[state] {
                continue;
            }
            let cell = state >> 1;
            for link in self.offsets[cell]..self.offsets[cell + 1] {
                visit(search, &mut queue, cell, Some(state), link, cost);
            }
        }
    }

    /// A new search for this graph (`createSearch`).
    pub fn new_search(&self) -> Search {
        let states = (self.offsets.len() - 1) * 2;
        Search {
            cost: vec![f64::INFINITY; states],
            prev_cell: vec![usize::MAX; states],
            prev_state: vec![None; states],
        }
    }
}

impl Search {
    /// The cost to reach a cell by land or by water.
    pub fn cost_to(&self, cell: usize) -> f64 {
        self.cost[cell * 2].min(self.cost[cell * 2 + 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cells 0-1-2 on a line, one unit apart; 2-3 by sea.
    fn line(port: bool) -> Graph {
        let links = vec![
            vec![(1, false)],
            vec![(0, false), (2, false)],
            vec![(1, false), (3, true)],
            vec![(2, true)],
        ];
        let point = |c: usize| [c as f64, 0.0];
        Graph::new(&links, &point, vec![false, false, port, false], 1.0)
    }

    #[test]
    fn land_costs_its_length_and_sea_a_fifth_plus_loading() {
        let g = line(true);
        let mut s = g.new_search();
        g.search(0, &mut s);
        assert_eq!(s.cost_to(2), 2.0);
        // A port: one sea unit at 0.2 and the loading of 4 spacings.
        assert!((s.cost_to(3) - (2.0 + 0.2 + 4.0)).abs() < 1e-12);
        let g = line(false);
        g.search(0, &mut s);
        assert!((s.cost_to(3) - (2.0 + 0.2 + 40.0)).abs() < 1e-12);
    }

    #[test]
    fn a_cell_off_the_network_has_no_path() {
        let links = vec![vec![(1, false)], vec![(0, false)], vec![]];
        let point = |c: usize| [c as f64, 0.0];
        let g = Graph::new(&links, &point, vec![false; 3], 1.0);
        let mut s = g.new_search();
        g.search(0, &mut s);
        assert_eq!(s.cost_to(2), f64::INFINITY);
        assert_eq!(s.prev_cell[2], 0);
    }
}
