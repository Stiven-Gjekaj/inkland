//! Religions.
//!
//! Each culture has a folk religion on all of its land. `world.religions`
//! organized religions start at the best places: capitals first, then other
//! places by suitability, each in a culture that has no organized religion
//! yet when one is free. An organized religion spreads over land by least
//! cost, up to a range, and takes the cells that it reaches. The cost of a
//! step is its distance, more on high land. The range is the side of a
//! square with the land area divided by the number of organized religions.
//!
//! Religions have no layer in print-safe mode. `world.json` and `query`
//! give them.

use crate::config::Config;
use crate::features::Features;
use crate::grid::Grid;
use crate::names::Chain;
use crate::rand::Rand;
use crate::seed::{Layer, SeedTree};
use crate::society::Society;
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

/// One religion.
#[derive(Debug, Clone, PartialEq)]
pub struct Religion {
    /// 1, 2, 3, ...: folk religions first, in culture order.
    pub id: u32,
    pub name: String,
    /// `folk` or `organized`.
    pub kind: &'static str,
    /// The culture where the religion starts.
    pub culture: u32,
    /// The place where an organized religion starts.
    pub center_place: Option<u32>,
}

/// The religions of a world.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Religions {
    pub list: Vec<Religion>,
    /// The religion of each cell. 0 for water.
    pub cell_religion: Vec<u32>,
}

/// What the religion stage reads.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub heights: &'a [f64],
    pub sea_level: f64,
    pub features: &'a Features,
    pub society: &'a Society,
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

const FORMS: [&str; 5] = ["Church", "Faith", "Order", "Cult", "Way"];

/// Make the religions.
pub fn build(input: &Input) -> Religions {
    let g = input.grid;
    let soc = input.society;
    let n = g.len();
    let land = |c: usize| input.features.is_land(c);
    let tree = SeedTree::new(input.config.seed);
    let mut rng = Rand::new(tree.part_rng(Layer::Names, "religions"));
    let mut used: BTreeSet<String> = soc.places.iter().map(|p| p.name.clone()).collect();

    // Folk religions: one for each culture.
    let mut list: Vec<Religion> = soc
        .cultures
        .iter()
        .map(|c| Religion {
            id: c.id,
            name: format!("The Old Ways of {}", c.name),
            kind: "folk",
            culture: c.id,
            center_place: None,
        })
        .collect();
    let mut cell_religion: Vec<u32> = (0..n)
        .map(|c| if land(c) { soc.cell_culture[c] } else { 0 })
        .collect();

    // Centers of organized religions.
    let mut places: Vec<_> = soc.places.iter().filter(|p| land(p.cell)).collect();
    places.sort_by(|a, b| {
        b.capital
            .cmp(&a.capital)
            .then(soc.cells.s[b.cell].total_cmp(&soc.cells.s[a.cell]))
            .then(a.id.cmp(&b.id))
    });
    let count = input.config.world.religions as usize;
    let mut centers = Vec::new();
    let mut cultures_used = BTreeSet::new();
    for pass in 0..2 {
        for p in &places {
            if centers.len() == count {
                break;
            }
            let fresh = !cultures_used.contains(&p.culture);
            if (pass == 0 && !fresh) || centers.iter().any(|&(q, _)| q == p.id) {
                continue;
            }
            cultures_used.insert(p.culture);
            centers.push((p.id, p.cell));
        }
    }
    if centers.is_empty() {
        return Religions {
            list,
            cell_religion,
        };
    }

    let land_cells = (0..n).filter(|&c| land(c)).count();
    let cell_area = g.step_x * g.step_y;
    let range = ((land_cells as f64 * cell_area) / centers.len() as f64).sqrt();
    let chains: Vec<Chain> = soc.cultures.iter().map(|c| Chain::new(c.base)).collect();
    let mut best = vec![f64::INFINITY; n];
    let mut heap = BinaryHeap::new();
    for &(place, cell) in &centers {
        let culture = soc
            .places
            .iter()
            .find(|p| p.id == place)
            .map_or(1, |p| p.culture);
        let id = list.len() as u32 + 1;
        let deity = if chains.is_empty() {
            format!("Faith {id}")
        } else {
            let k = (culture.max(1) as usize - 1).min(chains.len() - 1);
            chains[k].name(&mut rng, &mut used)
        };
        let form = FORMS[rng.int(0, FORMS.len() as i64 - 1) as usize];
        let name = format!("The {form} of {deity}");
        used.insert(name.clone());
        list.push(Religion {
            id,
            name,
            kind: "organized",
            culture,
            center_place: Some(place),
        });
        best[cell] = 0.0;
        cell_religion[cell] = id;
        heap.push(Reverse((Key(0.0), cell, id)));
    }
    while let Some(Reverse((Key(d), c, r))) = heap.pop() {
        if d > best[c] || cell_religion[c] != r {
            continue;
        }
        for m in g.neighbors(c) {
            if !land(m) {
                continue;
            }
            let (a, b) = (g.points[c], g.points[m]);
            let step = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
            let high = ((input.heights[m] - input.sea_level) / 30.0).max(0.0);
            let next = d + step * (1.0 + high);
            if next <= range && next < best[m] {
                best[m] = next;
                cell_religion[m] = r;
                heap.push(Reverse((Key(next), m, r)));
            }
        }
    }
    Religions {
        list,
        cell_religion,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    fn world(religions: u32) -> World {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 12, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "continents", "religions": religions }
        }))
        .unwrap();
        World::build(&c).unwrap()
    }

    #[test]
    fn each_land_cell_with_a_culture_has_a_religion_and_water_has_none() {
        let w = world(3);
        let r = &w.religions;
        let folk = r.list.iter().filter(|x| x.kind == "folk").count();
        let organized = r.list.iter().filter(|x| x.kind == "organized").count();
        assert_eq!(folk, w.society.cultures.len());
        assert_eq!(organized, 3);
        for c in 0..w.grid.len() {
            // Wildland (culture 0) has no folk religion.
            if w.features.is_land(c) && w.society.cell_culture[c] != 0 {
                assert!(
                    r.list.iter().any(|x| x.id == r.cell_religion[c]),
                    "cell {c}"
                );
            } else if !w.features.is_land(c) {
                assert_eq!(r.cell_religion[c], 0);
            }
        }
        // Each organized religion holds its center.
        for x in r.list.iter().filter(|x| x.kind == "organized") {
            let p = w
                .society
                .places
                .iter()
                .find(|p| Some(p.id) == x.center_place)
                .unwrap();
            assert_eq!(r.cell_religion[p.cell], x.id);
        }
    }

    #[test]
    fn zero_organized_religions_leaves_folk_religions_only() {
        let w = world(0);
        assert!(w.religions.list.iter().all(|x| x.kind == "folk"));
    }
}
