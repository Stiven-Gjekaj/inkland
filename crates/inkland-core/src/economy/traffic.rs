//! Trade traffic (`trade-traffic.ts`): the units of each good that the
//! deals of the run carry over each route link, for the render.
//!
//! The goods of each deal follow the cheapest route between the place and
//! the center of its market, as their carriage was priced. Kept goods and
//! the own deals of a center travel nowhere.

use super::deals::{Deal, DealKind};
use super::routes::Graph;
use std::collections::BTreeMap;

/// The traffic of a route link: the units of each good carried over it
/// either way a day (`TrafficLink`).
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// The two cells, in the order of the first path over the link.
    pub cells: [usize; 2],
    /// Good id and units, in the order first carried.
    pub goods: Vec<(u32, f64)>,
    /// `true` for a link travelled by water.
    pub water: bool,
}

/// The goods that a place trades with a market: its cell, and good id and
/// units in the order first met.
type Carried = (usize, Vec<(u32, f64)>);

/// The traffic of all links that carried goods in the run, in the order
/// first used (`getLinks`). `centers` is the cell of the center of each
/// market; `burg_cell` the cell of each place.
pub fn links(deals: &[Deal], graph: &Graph, centers: &[usize], burg_cell: &[usize]) -> Vec<Link> {
    // By market: cells of places in the order first met, each with the
    // goods in the order first met.
    let mut by_market: Vec<Vec<Carried>> = vec![Vec::new(); centers.len()];
    for d in deals {
        if d.kind == DealKind::Kept || d.units == 0.0 {
            continue;
        }
        let cell = burg_cell[d.burg as usize - 1];
        let cells = &mut by_market[d.market as usize - 1];
        let k = match cells.iter().position(|(c, _)| *c == cell) {
            Some(k) => k,
            None => {
                cells.push((cell, Vec::new()));
                cells.len() - 1
            }
        };
        let goods = &mut cells[k].1;
        match goods.iter_mut().find(|(g, _)| *g == d.good) {
            Some(e) => e.1 += d.units.abs(),
            None => goods.push((d.good, d.units.abs())),
        }
    }
    let mut out: Vec<Link> = Vec::new();
    let mut index: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut search = graph.new_search();
    for (m, &center) in centers.iter().enumerate() {
        let cells = &by_market[m];
        if cells.is_empty() {
            continue;
        }
        graph.search(center, &mut search);
        for (cell, goods) in cells {
            if *cell == center {
                continue;
            }
            let (land, water) = (cell * 2, cell * 2 + 1);
            let mut state = if search.cost[land] <= search.cost[water] {
                land
            } else {
                water
            };
            if search.cost[state] == f64::INFINITY {
                continue;
            }
            loop {
                let from = search.prev_cell[state];
                let to = state >> 1;
                let key = (from.min(to), from.max(to));
                let k = *index.entry(key).or_insert_with(|| {
                    out.push(Link {
                        cells: [from, to],
                        goods: Vec::new(),
                        water: state & 1 == 1,
                    });
                    out.len() - 1
                });
                for &(g, units) in goods {
                    match out[k].goods.iter_mut().find(|(x, _)| *x == g) {
                        Some(e) => e.1 += units,
                        None => out[k].goods.push((g, units)),
                    }
                }
                match search.prev_state[state] {
                    Some(s) => state = s,
                    None => break,
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deal(burg: u32, market: u32, good: u32, kind: DealKind, units: f64) -> Deal {
        Deal {
            burg,
            market,
            good,
            kind,
            units,
            price: 1.0,
            tax: 0.0,
            tariff: 0.0,
            carriage: 0.0,
        }
    }

    /// Cells 0-1-2 by road, 2-3 by sea; the center of market 1 is on cell
    /// 0; place 1 on cell 0, place 2 on cell 2, place 3 on cell 3, and
    /// place 4 on cell 4, off the network.
    fn graph() -> Graph {
        let links = vec![
            vec![(1, false)],
            vec![(0, false), (2, false)],
            vec![(1, false), (3, true)],
            vec![(2, true)],
            vec![],
        ];
        let point = |c: usize| [c as f64, 0.0];
        Graph::new(&links, &point, vec![false, false, true, false, false], 1.0)
    }

    #[test]
    fn each_deal_adds_its_units_to_each_link_between_the_place_and_the_center() {
        let deals = vec![
            deal(2, 1, 5, DealKind::Sale, 3.0),
            deal(2, 1, 6, DealKind::Demand, 1.0),
            deal(3, 1, 5, DealKind::Input, 2.0),
            // Kept goods, the own deals of the center, and a place off the
            // network travel nowhere.
            deal(2, 1, 7, DealKind::Kept, 9.0),
            deal(1, 1, 5, DealKind::Sale, 9.0),
            deal(4, 1, 5, DealKind::Sale, 9.0),
        ];
        let links = links(&deals, &graph(), &[0], &[0, 2, 3, 4]);
        assert_eq!(links.len(), 3);
        let at = |a: usize, b: usize| {
            links
                .iter()
                .find(|l| (l.cells[0].min(l.cells[1]), l.cells[0].max(l.cells[1])) == (a, b))
                .unwrap()
        };
        assert_eq!(at(0, 1).goods, vec![(5, 5.0), (6, 1.0)]);
        assert_eq!(at(1, 2).goods, vec![(5, 5.0), (6, 1.0)]);
        assert_eq!(at(2, 3).goods, vec![(5, 2.0)]);
        assert!(at(2, 3).water && !at(0, 1).water);
    }
}
