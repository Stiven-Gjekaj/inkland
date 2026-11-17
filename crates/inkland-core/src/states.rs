//! States (`states-generator.ts`): `createStates`, `expandStates`,
//! `normalize`, `findNeighbors`, and `assignColors`.
//!
//! - Each capital founds a state. A pinned capital gives its state the name
//!   in `capital_of`.
//! - States grow by least cost from their capitals with the costs of FMG:
//!   culture, population, biome, height, river, and coast, divided by the
//!   expansionism of the state. The growth stops at a total budget.
//! - `normalize` gives a lone cell to the state around it.
//! - inkland: each land cell must be in a state. A land cell that no state
//!   reaches takes the state of the nearest cell with a state.

use crate::cultures::{BIOME_COST, Culture, CultureType, Key, type_cost};
use crate::features::{FeatureKind, Features};
use crate::grid::Grid;
use crate::names::Chain;
use crate::population::Cells;
use crate::rand::{Rand, rn};
use crate::society::{NONE, Place, State};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, VecDeque};

/// What the state stage reads.
pub struct Input<'a> {
    pub config: &'a crate::config::Config,
    pub grid: &'a Grid,
    pub cells: &'a Cells,
    pub features: &'a Features,
    pub biomes: &'a [u8],
    pub cultures: &'a [Culture],
    pub cell_culture: &'a [u32],
    /// Chain 0 is the wildlands; chain `k` is culture `k`.
    pub chains: &'a [Chain],
}

/// One state for each capital (`createStates`).
pub fn create(
    input: &Input,
    places: &mut [Place],
    used: &mut BTreeSet<String>,
    rng: &mut Rand,
    nrng: &mut Rand,
) -> Vec<State> {
    let config = input.config;
    let mut states: Vec<State> = Vec::new();
    for p in places.iter_mut().filter(|p| p.capital) {
        let pinned = p
            .pin
            .as_ref()
            .and_then(|id| config.pins.points.iter().find(|q| &q.id == id))
            .and_then(|q| q.capital_of.clone());
        let expansionism = rn(rng.unit() * config.world.size_variety + 1.0, 1);
        let chain = &input.chains[p.culture as usize];
        let name = match pinned {
            Some(s) if !states.iter().any(|x| x.name == s) => s,
            Some(_) => {
                // A second pin with the same state name is not a capital.
                p.capital = false;
                continue;
            }
            None => {
                let short = p.name.chars().count() < 9 && p.cell % 5 == 0;
                let basename = if short {
                    p.name.clone()
                } else {
                    chain.get_short(nrng)
                };
                chain.state(nrng, &basename, used)
            }
        };
        used.insert(name.clone());
        let kind = input
            .cultures
            .get((p.culture as usize).wrapping_sub(1))
            .map_or(CultureType::Generic, |c| c.kind);
        states.push(State {
            id: states.len() as u32 + 1,
            name,
            capital: p.id,
            culture: p.culture,
            center: p.cell,
            kind,
            expansionism,
            ..State::default()
        });
    }
    states
}

/// Grow the states (`expandStates` and `normalize`). The result is the state
/// of each cell; water has state 0.
pub fn expand(input: &Input, states: &[State], places: &[Place]) -> Vec<u32> {
    let Input {
        config,
        grid,
        cells,
        features,
        biomes,
        cultures,
        cell_culture,
        ..
    } = *input;
    let n = grid.len();
    let mut state = vec![NONE; n];
    let mut cost = vec![0.0f64; n];
    let budget = n as f64 / 2.0 * config.world.growth_rate;
    let mut heap = BinaryHeap::new();
    let mut seq = 0u64;
    let mut is_center = vec![false; n];
    for s in states {
        state[s.center] = s.id;
        is_center[s.center] = true;
        let native = cultures
            .get((s.culture as usize).wrapping_sub(1))
            .map_or(biomes[s.center], |c| biomes[c.center]);
        heap.push(Reverse((Key(0.0, seq), s.center, s.id, native)));
        seq += 1;
        cost[s.center] = 1.0;
    }
    let is_lake = |c: usize| features.get(features.of_cell[c]).kind == FeatureKind::Lake;
    while let Some(Reverse((Key(p, _), e, s, native))) = heap.pop() {
        let st = &states[s as usize - 1];
        let kind = st.kind;
        for m in grid.neighbors(e) {
            if state[m] != NONE && is_center[m] {
                continue;
            }
            let h = cells.h[m];
            let culture_cost = if st.culture == cell_culture[m] {
                -9.0
            } else {
                100.0
            };
            let population_cost = if h < 20.0 {
                0.0
            } else if cells.s[m] != 0.0 {
                (20.0 - cells.s[m]).max(0.0)
            } else {
                5000.0
            };
            let b = biomes[m];
            let biome_cost = if native == b {
                10.0
            } else if kind == CultureType::Hunting {
                BIOME_COST[b as usize] * 2.0
            } else if kind == CultureType::Nomadic && b > 4 && b < 10 {
                BIOME_COST[b as usize] * 3.0
            } else {
                BIOME_COST[b as usize]
            };
            let height_cost = match kind {
                CultureType::Lake if is_lake(m) => 10.0,
                CultureType::Naval if h < 20.0 => 300.0,
                CultureType::Nomadic if h < 20.0 => 10000.0,
                _ if h < 20.0 => 1000.0,
                CultureType::Highland if h < 62.0 => 1100.0,
                CultureType::Highland => 0.0,
                _ if h >= 67.0 => 2200.0,
                _ if h >= 44.0 => 300.0,
                _ => 0.0,
            };
            let river_cost = if kind == CultureType::River {
                if cells.r[m] != 0 { 0.0 } else { 100.0 }
            } else if cells.r[m] == 0 {
                0.0
            } else {
                (cells.fl[m] / 10.0).clamp(20.0, 100.0)
            };
            let cell_cost = (culture_cost
                + population_cost
                + biome_cost
                + height_cost
                + river_cost
                + type_cost(cells.t[m], kind))
            .max(0.0);
            let total = p + 10.0 + cell_cost / st.expansionism;
            if total > budget {
                continue;
            }
            if cost[m] == 0.0 || total < cost[m] {
                if h >= 20.0 {
                    state[m] = s;
                }
                cost[m] = total;
                heap.push(Reverse((Key(total, seq), m, s, native)));
                seq += 1;
            }
        }
    }

    // normalize.
    let mut burg = vec![false; n];
    let mut capital_cell = vec![false; n];
    for p in places {
        burg[p.cell] = true;
        if p.capital {
            capital_cell[p.cell] = true;
        }
    }
    for i in 0..n {
        if !cells.land(i) || burg[i] {
            continue;
        }
        if grid.neighbors(i).any(|c| capital_cell[c]) {
            continue;
        }
        let neibs: Vec<usize> = grid.neighbors(i).filter(|&c| cells.land(c)).collect();
        let adversaries: Vec<usize> = neibs
            .iter()
            .copied()
            .filter(|&c| state[c] != state[i])
            .collect();
        if adversaries.len() < 2 {
            continue;
        }
        let buddies = neibs.iter().filter(|&&c| state[c] == state[i]).count();
        if buddies > 2 || adversaries.len() <= buddies {
            continue;
        }
        state[i] = state[adversaries[0]];
    }

    // inkland: each land cell gets a state. A cell that no state reached
    // takes the state of the nearest cell with a state, over land and water.
    if !states.is_empty() {
        let mut queue: VecDeque<usize> = (0..n)
            .filter(|&c| cells.land(c) && state[c] != NONE)
            .collect();
        let mut owner: Vec<u32> = (0..n)
            .map(|c| if cells.land(c) { state[c] } else { NONE })
            .collect();
        while let Some(c) = queue.pop_front() {
            for m in grid.neighbors(c) {
                if owner[m] == NONE {
                    owner[m] = owner[c];
                    queue.push_back(m);
                }
            }
        }
        for c in 0..n {
            state[c] = if cells.land(c) { owner[c] } else { NONE };
        }
    }
    state
}

/// The neighbor states of each state (`findNeighbors`).
pub fn neighbors(grid: &Grid, cells: &Cells, cell_state: &[u32], states: &mut [State]) {
    let mut sets: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); states.len() + 1];
    for i in 0..grid.len() {
        if !cells.land(i) {
            continue;
        }
        let s = cell_state[i];
        for c in grid.neighbors(i) {
            if cells.land(c) && cell_state[c] != s {
                sets[s as usize].insert(cell_state[c]);
            }
        }
    }
    for st in states.iter_mut() {
        st.neighbors = sets[st.id as usize].iter().copied().collect();
    }
}

/// Colors with a greedy coloring of the neighbor graph (`assignColors`).
pub fn colors(states: &mut [State], rng: &mut Rand) {
    let mut palette: Vec<String> = crate::colors::SET2.iter().map(|c| c.to_string()).collect();
    for k in 0..states.len() {
        let color = palette
            .iter()
            .find(|color| {
                states[k]
                    .neighbors
                    .iter()
                    .all(|&n| n == NONE || states[n as usize - 1].color != **color)
            })
            .cloned();
        states[k].color = color.unwrap_or_else(|| crate::colors::random_color(rng));
        palette.rotate_left(1);
    }
    for c in palette {
        let mut first = true;
        for st in states.iter_mut().filter(|s| s.color == c) {
            if first {
                first = false;
                continue;
            }
            st.color = crate::colors::mixed_color(&st.color, rng, 0.2, 0.3);
        }
    }
}

/// Area, population, and places of each state (`collectStatistics`).
pub fn statistics(cells: &Cells, cell_state: &[u32], places: &[Place], states: &mut [State]) {
    for st in states.iter_mut() {
        st.cells = 0;
        st.area = 0.0;
        st.burgs = 0;
        st.rural = 0.0;
        st.urban = 0.0;
    }
    let mut burg_at = vec![NONE; cells.h.len()];
    for p in places {
        burg_at[p.cell] = p.id;
    }
    for i in 0..cells.h.len() {
        if !cells.land(i) || cell_state[i] == NONE {
            continue;
        }
        let st = &mut states[cell_state[i] as usize - 1];
        st.cells += 1;
        st.area += cells.area[i];
        st.rural += cells.pop[i];
        if burg_at[i] != NONE {
            st.urban += places[burg_at[i] as usize - 1].population;
            st.burgs += 1;
        }
    }
}

const MONARCHY: [&str; 5] = ["Duchy", "Grand Duchy", "Principality", "Kingdom", "Empire"];
const GENERIC: [(&str, u32); 3] = [("Monarchy", 25), ("Republic", 2), ("Union", 1)];
const NAVAL: [(&str, u32); 3] = [("Monarchy", 25), ("Republic", 8), ("Union", 3)];
const REPUBLIC: [(&str, u32); 9] = [
    ("Republic", 75),
    ("Federation", 4),
    ("Trade Company", 4),
    ("Most Serene Republic", 2),
    ("Oligarchy", 2),
    ("Tetrarchy", 1),
    ("Triumvirate", 1),
    ("Diarchy", 1),
    ("Junta", 1),
];
const UNION: [(&str, u32); 8] = [
    ("Union", 3),
    ("League", 4),
    ("Confederation", 1),
    ("United Kingdom", 1),
    ("United Republic", 1),
    ("United Provinces", 2),
    ("Commonwealth", 1),
    ("Heptarchy", 1),
];
const THEOCRACY: [(&str, u32); 5] = [
    ("Theocracy", 15),
    ("Brotherhood", 1),
    ("Thearchy", 2),
    ("See", 1),
    ("Holy State", 1),
];
const ANARCHY: [(&str, u32); 4] = [
    ("Free Territory", 2),
    ("Council", 3),
    ("Commune", 1),
    ("Community", 1),
];
/// The forms that put the adjective before the form name.
const ADJECTIVE_FORMS: [&str; 17] = [
    "Empire",
    "Sultanate",
    "Khaganate",
    "Shogunate",
    "Caliphate",
    "Despotate",
    "Theocracy",
    "Oligarchy",
    "Union",
    "Confederation",
    "Trade Company",
    "League",
    "Tetrarchy",
    "Triumvirate",
    "Diarchy",
    "Horde",
    "Marches",
];

/// What `forms` reads.
pub struct FormInput<'a> {
    pub config: &'a crate::config::Config,
    pub cultures: &'a [Culture],
    pub places: &'a [Place],
    pub religions: &'a crate::religions::Religions,
    pub development: &'a [u8],
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// The form, form name, and full name of each state (`defineStateForms`).
/// FMG has a "Neutrals" state 0 with the area of the land without a state;
/// in inkland that area is 0. inkland has no diplomacy, so the vassal forms
/// (Marches, Dominion, Protectorate of a vassal) do not come.
pub fn forms(input: &FormInput, states: &mut [State], rng: &mut Rand) {
    if states.is_empty() {
        return;
    }
    let areas: Vec<f64> = std::iter::once(0.0)
        .chain(states.iter().map(|s| s.area))
        .collect();
    let median_area = median(areas);
    let mut sorted: Vec<f64> = states.iter().map(|s| s.area).collect();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let k = (libm::pow(states.len() as f64, 0.4).ceil() as i64 - 2).max(0) as usize;
    let empire_min = sorted[k.min(sorted.len() - 1)];
    for s in states.iter_mut() {
        let mut tier = ((s.area / median_area * 2.6).floor() as i64).clamp(0, 4) as usize;
        if median_area == 0.0 {
            tier = 4;
        }
        if tier == 4 && s.area < empire_min {
            tier = 3;
        }
        let religion = input.religions.cell_religion[s.center];
        let rel = input
            .religions
            .list
            .iter()
            .find(|r| r.id == religion && religion != NONE);
        let theocracy = rel.is_some_and(|r| r.expansion == "state")
            || (rng.p(0.1) && rel.is_some_and(|r| r.kind == "Organized" || r.kind == "Cult"));
        let anarchy = rng.p(0.01 - tier as f64 / 500.0);
        s.form = if theocracy {
            "Theocracy".to_owned()
        } else if anarchy {
            "Anarchy".to_owned()
        } else if s.kind == CultureType::Naval {
            rng.weighted(&NAVAL).to_owned()
        } else {
            rng.weighted(&GENERIC).to_owned()
        };
        let base = input
            .cultures
            .get((s.culture as usize).wrapping_sub(1))
            .map_or(crate::cultures::WILDLANDS_BASE, |c| c.base);
        let form_name = select_form(input, s, tier, base, rng);
        s.form_name = tribal_form(input, s, tier, form_name);
        s.full_name = full_name(s, rng);
    }
}

fn select_form(
    input: &FormInput,
    s: &mut State,
    tier: usize,
    base: usize,
    rng: &mut Rand,
) -> String {
    let name = |x: &str| x.to_owned();
    match s.form.as_str() {
        "Monarchy" => {
            let form = MONARCHY[tier];
            let empire_or_kingdom = form == "Empire" || form == "Kingdom";
            let duchy = form == "Grand Duchy" || form == "Duchy";
            let special = match base {
                31 if empire_or_kingdom => Some("Khanate"),
                16 if form == "Principality" => Some("Beylik"),
                5 if empire_or_kingdom => Some("Tsardom"),
                16 if empire_or_kingdom => Some("Khaganate"),
                12 if form == "Kingdom" || form == "Grand Duchy" => Some("Shogunate"),
                17 | 18 if form == "Empire" => Some("Caliphate"),
                18 if duchy => Some("Emirate"),
                7 if duchy => Some("Despotate"),
                31 if duchy => Some("Ulus"),
                16 if duchy => Some("Horde"),
                24 if duchy => Some("Satrapy"),
                _ => None,
            };
            name(special.unwrap_or(form))
        }
        "Republic" => {
            if tier < 2 && s.burgs == 1 {
                let capital = input.places.iter().find(|p| p.id == s.capital);
                let pinned = capital.is_some_and(|p| p.pin.is_some());
                if let Some(c) = capital
                    && crate::names::trim_vowels(&s.name, 3)
                        == crate::names::trim_vowels(&c.name, 3)
                {
                    // inkland: a pinned state keeps its name.
                    if !pinned {
                        s.name = c.name.clone();
                    }
                    return name("Free City");
                }
                if rng.p(0.3) {
                    return name("City-state");
                }
            }
            name(rng.weighted(&REPUBLIC))
        }
        "Union" => name(rng.weighted(&UNION)),
        "Anarchy" => name(rng.weighted(&ANARCHY)),
        _ => {
            if [0, 1, 2, 3, 4, 6, 8, 9, 13, 15, 20].contains(&base) {
                if rng.p(0.1) {
                    return format!("Divine {}", MONARCHY[tier]);
                }
                if tier < 2 && rng.p(0.5) {
                    return name("Diocese");
                }
                if tier < 2 && rng.p(0.5) {
                    return name("Bishopric");
                }
            }
            if rng.p(0.9) && [7, 5].contains(&base) {
                if tier < 2 {
                    return name("Eparchy");
                }
                if tier == 2 {
                    return name("Exarchate");
                }
                return name("Patriarchate");
            }
            if rng.p(0.9) && [21, 16].contains(&base) {
                return name("Imamah");
            }
            if tier > 2 && rng.p(0.8) && [18, 17, 28].contains(&base) {
                return name("Caliphate");
            }
            name(rng.weighted(&THEOCRACY))
        }
    }
}

/// A title for a people before the Bronze Age (`getTribalForm`).
fn tribal_form(input: &FormInput, s: &State, tier: usize, form_name: String) -> String {
    let development = input
        .places
        .iter()
        .find(|p| p.id == s.capital)
        .map_or(0, |p| input.development[p.cell]);
    if development == 0 || development >= crate::development::TRIBAL_DEVELOPMENT {
        return form_name;
    }
    let tribal = match s.form.as_str() {
        "Monarchy" if s.kind == CultureType::Nomadic => "Horde",
        "Monarchy" if tier < 2 => "Tribe",
        "Monarchy" if tier < 4 => "Chiefdom",
        "Monarchy" => "Paramount Chiefdom",
        "Republic" => "Tribal Council",
        "Union" => "Clans Union",
        "Theocracy" => "Shamanate",
        _ => return form_name,
    };
    tribal.to_owned()
}

/// `getFullName`.
fn full_name(s: &State, rng: &mut Rand) -> String {
    if s.form_name.is_empty() {
        return s.name.clone();
    }
    if s.name.is_empty() {
        return format!("The {}", s.form_name);
    }
    let adj = ADJECTIVE_FORMS.contains(&s.form_name.as_str()) && !s.name.contains([' ', '-']);
    if adj {
        format!("{} {}", crate::names::adjective(&s.name, rng), s.form_name)
    } else {
        format!("{} of {}", s.form_name, s.name)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    #[test]
    fn states_have_neighbors_colors_and_their_capital_cell() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 8, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "pangea", "states": 6 }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let s = &w.society;
        assert!(!s.states.is_empty());
        for st in &s.states {
            assert_eq!(s.cell_state[st.center], st.id);
            assert!(st.color.starts_with('#') && st.color.len() == 7);
            assert!(st.expansionism >= 1.0);
            for &nb in &st.neighbors {
                assert_ne!(nb, st.id);
            }
        }
        // On one landmass with more states, each state has a neighbor.
        if s.states.len() > 1 {
            assert!(s.states.iter().any(|st| !st.neighbors.is_empty()));
        }
    }
}
