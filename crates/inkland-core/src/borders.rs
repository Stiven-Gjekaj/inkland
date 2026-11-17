//! Lines between cells: borders of states and provinces, and the joining of
//! cell edges into lines.

use crate::world::World;

/// A polygon edge: the indices of its two vertices.
pub type Edge = (u32, u32);

/// Join edges that share a vertex into lines. The result is the same for
/// the same edges: lines start at the lowest free end, and closed loops at
/// their lowest vertex.
pub fn chain_edges(edges: &[(u32, u32)]) -> Vec<Vec<u32>> {
    use std::collections::BTreeMap;
    let mut adj: BTreeMap<u32, Vec<(u32, usize)>> = BTreeMap::new();
    for (i, &(a, b)) in edges.iter().enumerate() {
        adj.entry(a).or_default().push((b, i));
        adj.entry(b).or_default().push((a, i));
    }
    let mut used = vec![false; edges.len()];
    let mut lines = Vec::new();
    let walk = |start: u32, used: &mut Vec<bool>| -> Vec<u32> {
        let mut line = vec![start];
        let mut cur = start;
        while let Some(&(next, e)) = adj[&cur].iter().filter(|(_, e)| !used[*e]).min() {
            used[e] = true;
            line.push(next);
            cur = next;
            if adj[&cur].len() != 2 {
                break;
            }
        }
        line
    };
    // Open lines first: from each vertex that is not on exactly two edges.
    let ends: Vec<u32> = adj
        .iter()
        .filter(|(_, v)| v.len() != 2)
        .map(|(&k, _)| k)
        .collect();
    for v in ends {
        while adj[&v].iter().any(|(_, e)| !used[*e]) {
            lines.push(walk(v, &mut used));
        }
    }
    // Then closed loops.
    let all: Vec<u32> = adj.keys().copied().collect();
    for v in all {
        if adj[&v].iter().any(|(_, e)| !used[*e]) {
            lines.push(walk(v, &mut used));
        }
    }
    lines
}

/// The polygon edges between land cells whose owners differ by `key`.
pub fn owner_edges(world: &World, key: &dyn Fn(usize) -> (u32, u32)) -> Vec<(u32, u32)> {
    let g = &world.grid;
    let mut edges = Vec::new();
    for c in 0..g.len() {
        if !world.features.is_land(c) {
            continue;
        }
        let poly = &g.polygons[c];
        for (k, &other) in g.edge_cells[c].iter().enumerate() {
            if other == crate::grid::OUTSIDE {
                continue;
            }
            let o = other as usize;
            if o < c || !world.features.is_land(o) {
                continue;
            }
            let (a, b) = (key(c), key(o));
            // Different owners; a missing second owner (0) is not a border.
            if a != b && (a.0 != b.0 || (a.1 != 0 && b.1 != 0)) {
                edges.push((poly[k], poly[(k + 1) % poly.len()]));
            }
        }
    }
    edges
}

/// The polygon edges of the state borders and of the province borders. A
/// province border that is also a state border is only in the first list.
pub fn state_and_province_edges(world: &World) -> (Vec<Edge>, Vec<Edge>) {
    let soc = &world.society;
    let states = owner_edges(world, &|c| (soc.cell_state[c], 0));
    let provinces = owner_edges(world, &|c| (soc.cell_state[c], soc.cell_province[c]))
        .into_iter()
        .filter(|e| !states.contains(e))
        .collect();
    (states, provinces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_edges_joins_a_line_and_a_loop() {
        let edges = [(1, 2), (3, 2), (3, 4), (10, 11), (11, 12), (12, 10)];
        let lines = chain_edges(&edges);
        assert_eq!(lines, vec![vec![1, 2, 3, 4], vec![10, 11, 12, 10]]);
    }
}
