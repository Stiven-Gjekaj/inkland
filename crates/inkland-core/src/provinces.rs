//! Provinces (`provinces-generator.ts`, `generate`).
//!
//! - The places of each state, capital first, then by population with a
//!   random factor. `provinces_ratio` percent of them, at least 2, become
//!   seats. A province takes the name of its seat or a new name, and a form
//!   name of the form of its state.
//! - The provinces grow inside their state: a step costs 10 on low land,
//!   30 on hills, 100 on mountains and water, up to a random budget.
//! - A cell between two other provinces joins the one with more cells
//!   around it.
//! - The land that no province reaches gets "wild" provinces: islands,
//!   colonies ("New ..."), territories, and more.
//! - inkland: a state with fewer than 2 places gets one province at its
//!   capital, so that each land cell is in a province.

use crate::config::Config;
use crate::features::Features;
use crate::grid::Grid;
use crate::names::Chain;
use crate::population::Cells;
use crate::rand::Rand;
use crate::society::{NONE, Place, Province, State};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

fn forms(form: &str) -> Vec<(&'static str, u32)> {
    match form {
        "Monarchy" => vec![
            ("County", 22),
            ("Earldom", 6),
            ("Shire", 2),
            ("Landgrave", 2),
            ("Margrave", 2),
            ("Barony", 2),
            ("Captaincy", 1),
            ("Seneschalty", 1),
        ],
        "Republic" => vec![
            ("Province", 6),
            ("Department", 2),
            ("Governorate", 2),
            ("District", 1),
            ("Canton", 1),
            ("Prefecture", 1),
        ],
        "Theocracy" => vec![("Parish", 3), ("Deanery", 1)],
        "Union" => vec![
            ("Province", 1),
            ("State", 1),
            ("Canton", 1),
            ("Republic", 1),
            ("County", 1),
            ("Council", 1),
        ],
        "Anarchy" => vec![
            ("Council", 1),
            ("Commune", 1),
            ("Community", 1),
            ("Tribe", 1),
        ],
        _ => wild(),
    }
}

fn wild() -> Vec<(&'static str, u32)> {
    vec![
        ("Territory", 10),
        ("Land", 5),
        ("Region", 2),
        ("Tribe", 1),
        ("Clan", 1),
        ("Dependency", 1),
        ("Area", 1),
    ]
}

/// What the province stage reads.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub cells: &'a Cells,
    pub features: &'a Features,
    pub cell_state: &'a [u32],
    pub cell_culture: &'a [u32],
    /// Chain 0 is the wildlands; chain `k` is culture `k`.
    pub chains: &'a [Chain],
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

/// Make the provinces. The result is the provinces and the province of each
/// cell.
pub fn generate(
    input: &Input,
    states: &[State],
    places: &[Place],
    rng: &mut Rand,
) -> (Vec<Province>, Vec<u32>) {
    let Input {
        config,
        grid,
        cells,
        features,
        cell_state,
        cell_culture,
        chains,
    } = *input;
    let n = grid.len();
    let mut ids = vec![NONE; n];
    let mut provinces: Vec<Province> = Vec::new();
    let mut burg_at = vec![NONE; n];
    for p in places {
        burg_at[p.cell] = p.id;
    }
    let ratio = f64::from(config.world.provinces_ratio);
    let max_growth = if ratio >= 100.0 {
        1000.0
    } else {
        rng.gauss(20.0, 5.0, 5.0, 100.0, 0) * libm::sqrt(ratio.max(1.0))
    };
    let state_name = |culture: u32, rng: &mut Rand| {
        let chain = &chains[culture as usize];
        let short = chain.get_short(rng);
        crate::names::state_name(&short, chain.base_index(), rng)
    };
    let mut of_state: BTreeMap<u32, Vec<u32>> = BTreeMap::new();

    for s in states {
        let mut list: Vec<(&Place, f64)> = places
            .iter()
            .filter(|p| p.state == s.id)
            .map(|p| (p, p.population * rng.gauss(1.0, 0.2, 0.5, 1.5, 3)))
            .collect();
        list.sort_by(|a, b| b.0.capital.cmp(&a.0.capital).then(b.1.total_cmp(&a.1)));
        let list: Vec<&Place> = list.into_iter().map(|x| x.0).collect();
        let count = if list.len() < 2 {
            // inkland: one province for a state with fewer than 2 places.
            list.len().min(1)
        } else {
            ((list.len() as f64 * ratio / 100.0).ceil() as usize).max(2)
        };
        let mut form = forms(&s.form);
        for &burg in list.iter().take(count) {
            let c = burg.culture;
            let by_burg = rng.p(0.5);
            let name = if by_burg {
                burg.name.clone()
            } else {
                state_name(c, rng)
            };
            let form_name = rng.weighted(&form);
            if let Some(f) = form.iter_mut().find(|f| f.0 == form_name) {
                f.1 += 10;
            }
            let id = provinces.len() as u32 + 1;
            of_state.entry(s.id).or_default().push(id);
            provinces.push(Province {
                id,
                full_name: format!("{name} {form_name}"),
                name,
                state: s.id,
                seat: burg.id,
                center: burg.cell,
                form_name: form_name.to_owned(),
                color: crate::colors::mixed_color(&s.color, rng, 0.2, 0.3),
            });
        }
    }

    // Expand the provinces.
    let mut cost = vec![0.0f64; n];
    let mut heap = BinaryHeap::new();
    let mut seq = 0u64;
    for p in &provinces {
        ids[p.center] = p.id;
        heap.push(Reverse((Key(0.0, seq), p.center, p.id, p.state)));
        seq += 1;
        cost[p.center] = 1.0;
    }
    while let Some(Reverse((Key(pc, _), e, province, state))) = heap.pop() {
        for m in grid.neighbors(e) {
            let land = cells.land(m);
            if !land && cells.t[m] == 0 {
                continue;
            }
            if land && cell_state[m] != state {
                continue;
            }
            let h = cells.h[m];
            let elevation = if h >= 70.0 {
                100.0
            } else if h >= 50.0 {
                30.0
            } else if h >= 20.0 {
                10.0
            } else {
                100.0
            };
            let total = pc + elevation;
            if total > max_growth {
                continue;
            }
            if cost[m] == 0.0 || total < cost[m] {
                if land {
                    ids[m] = province;
                }
                cost[m] = total;
                seq += 1;
                heap.push(Reverse((Key(total, seq), m, province, state)));
            }
        }
    }

    // Justify the shapes a bit.
    for i in 0..n {
        if !cells.land(i) || burg_at[i] != NONE {
            continue;
        }
        let neibs: Vec<u32> = grid
            .neighbors(i)
            .filter(|&c| cell_state[c] == cell_state[i])
            .map(|c| ids[c])
            .collect();
        let adversaries: Vec<u32> = neibs.iter().copied().filter(|&c| c != ids[i]).collect();
        if adversaries.len() < 2 {
            continue;
        }
        let buddies = neibs.iter().filter(|&&c| c == ids[i]).count();
        if buddies > 2 {
            continue;
        }
        let competitors: Vec<usize> = adversaries
            .iter()
            .map(|p| adversaries.iter().filter(|&v| v == p).count())
            .collect();
        let max = competitors.iter().copied().max().unwrap_or(0);
        if buddies >= max {
            continue;
        }
        let k = competitors.iter().position(|&x| x == max).unwrap_or(0);
        ids[i] = adversaries[k];
    }

    // Wild provinces for the land that no province reaches.
    for s in states {
        let Some(core) = of_state.get(&s.id).cloned() else {
            continue;
        };
        let mut pool: Vec<String> = std::iter::once(s.name.clone())
            .chain(core.iter().map(|&p| provinces[p as usize - 1].name.clone()))
            .filter(|n| !n.is_empty() && !n.to_lowercase().contains("new"))
            .collect();
        loop {
            let rest: Vec<usize> = (0..n)
                .filter(|&i| cells.land(i) && cell_state[i] == s.id && ids[i] == NONE)
                .collect();
            if rest.is_empty() {
                break;
            }
            let id = provinces.len() as u32 + 1;
            let burg_cell = rest.iter().copied().find(|&i| burg_at[i] != NONE);
            let center = burg_cell.unwrap_or(rest[0]);
            let burg = burg_cell.map_or(NONE, |c| burg_at[c]);
            ids[center] = id;
            let mut cost = vec![0.0f64; n];
            cost[center] = 1.0;
            let mut heap = BinaryHeap::new();
            let mut seq = 0u64;
            heap.push(Reverse((Key(0.0, seq), center)));
            while let Some(Reverse((Key(pc, _), e))) = heap.pop() {
                for m in grid.neighbors(e) {
                    if ids[m] != NONE {
                        continue;
                    }
                    let land = cells.land(m);
                    if cell_state[m] != NONE && cell_state[m] != s.id {
                        continue;
                    }
                    let ter = if land {
                        if cell_state[m] == s.id { 3.0 } else { 20.0 }
                    } else if cells.t[m] != 0 {
                        10.0
                    } else {
                        30.0
                    };
                    let total = pc + ter;
                    if total > max_growth {
                        continue;
                    }
                    if cost[m] == 0.0 || total < cost[m] {
                        if land && cell_state[m] == s.id {
                            ids[m] = id;
                        }
                        cost[m] = total;
                        seq += 1;
                        heap.push(Reverse((Key(total, seq), m)));
                    }
                }
            }

            let c = cell_culture[center];
            let f = features.of_cell[center];
            let prov_cells: Vec<usize> = rest.iter().copied().filter(|&i| ids[i] == id).collect();
            let single_isle = prov_cells.len() as u32 == features.get(f).cells
                && prov_cells.iter().all(|&i| features.of_cell[i] == f);
            let isles = !single_isle && prov_cells.iter().all(|&i| cells.isle[i]);
            let colony = !single_isle
                && !isles
                && rng.p(0.5)
                && !passable(grid, features, cells, cell_state, s.center, center);
            let mut colony_name = None;
            if colony && rng.p(0.8) && !pool.is_empty() {
                let k = rng.rand(0, pool.len() as i64 - 1) as usize;
                colony_name = Some(format!("New {}", pool.remove(k)));
            }
            let name = match colony_name {
                Some(n) => n,
                None if burg_cell.is_some() && rng.p(0.5) => places[burg as usize - 1].name.clone(),
                None => state_name(c, rng),
            };
            let form_name = if single_isle {
                "Island"
            } else if isles {
                "Islands"
            } else if colony {
                "Colony"
            } else {
                rng.weighted(&wild())
            };
            provinces.push(Province {
                id,
                full_name: format!("{name} {form_name}"),
                name,
                state: s.id,
                seat: burg,
                center,
                form_name: form_name.to_owned(),
                color: crate::colors::mixed_color(&s.color, rng, 0.2, 0.3),
            });
        }
    }
    for (i, id) in ids.iter_mut().enumerate() {
        if !cells.land(i) {
            *id = NONE;
        }
    }
    (provinces, ids)
}

/// `true` if a land path inside the state joins two cells (`isPassable`).
fn passable(
    grid: &Grid,
    features: &Features,
    cells: &Cells,
    cell_state: &[u32],
    from: usize,
    to: usize,
) -> bool {
    if features.of_cell[from] != features.of_cell[to] {
        return false;
    }
    let state = cell_state[from];
    let mut used = vec![false; grid.len()];
    let mut stack = vec![from];
    while let Some(c) = stack.pop() {
        if c == to {
            return true;
        }
        for m in grid.neighbors(c) {
            if used[m] || !cells.land(m) || cell_state[m] != state {
                continue;
            }
            used[m] = true;
            stack.push(m);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    #[test]
    fn states_get_forms_places_get_groups_and_provinces_cover_states() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 12, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 5000, "template": "continents", "states": 5 }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let s = &w.society;
        for st in &s.states {
            assert!(
                ["Monarchy", "Republic", "Union", "Theocracy", "Anarchy"]
                    .contains(&st.form.as_str()),
                "{}",
                st.form
            );
            assert!(!st.form_name.is_empty() && !st.full_name.is_empty());
            assert!(st.cells > 0 && st.area > 0.0);
        }
        let total: u32 = s.states.iter().map(|x| x.burgs).sum();
        assert_eq!(total as usize, s.places.len());
        for p in &s.places {
            assert!(p.population > 0.0);
            assert!(!p.group.is_empty());
            if p.capital {
                assert!(p.citadel && p.walls);
            }
        }
        for pr in &s.provinces {
            assert!(pr.full_name.starts_with(&pr.name));
            assert!(s.cell_province.contains(&pr.id) || pr.seat == 0);
        }
    }
}
