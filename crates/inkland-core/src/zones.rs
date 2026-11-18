//! Zones (`model/political/zones-generator.ts`): areas of cells with a
//! story, as invasions, rebels, diseases, and disasters.
//!
//! `generate` runs after the markers and before the journeys, as
//! `Zones.generate` does in the FMG pipeline. Each zone type has an
//! expected number; the count of a type is a normal draw around it. The
//! zones read the last economy run (trade, prosperity) and the stability.
//! An eruption reads the volcano markers and makes an active volcano an
//! erupting one.
//!
//! The color of a zone is the FMG fill: a hatch pattern, for example
//! `url(#hatch1)`.

use crate::economy::Economy;
use crate::economy::deals::DealKind;
use crate::features::{FeatureKind, Features};
use crate::geonames::Names;
use crate::grid::Grid;
use crate::markers::Marker;
use crate::names::{Chain, adjective};
use crate::rand::Rand;
use crate::religions::Religions;
use crate::rivers::River;
use crate::roads::Road;
use crate::seed::{Layer, SeedTree};
use crate::society::{NONE, Society};
use crate::stability::{Parts, Stability};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// A zone (`Zone`).
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    /// 0, 1, 2, ... (FMG `i`).
    pub id: u32,
    pub name: String,
    /// The zone type, for example `Invasion` or `Disease`.
    pub kind: &'static str,
    /// The cells, each one time.
    pub cells: Vec<usize>,
    /// The FMG fill: a hatch pattern, for example `url(#hatch1)`.
    pub color: &'static str,
    /// The legend text.
    pub note: String,
}

/// What the zones read.
pub struct Input<'a> {
    pub config: &'a crate::config::Config,
    pub grid: &'a Grid,
    pub features: &'a Features,
    pub rivers: &'a [River],
    pub names: &'a Names,
    pub society: &'a Society,
    pub religions: &'a Religions,
    pub roads: &'a [Road],
    pub sea_routes: &'a [Road],
    pub economy: &'a Economy,
    pub stability: &'a Stability,
}

/// The zone types and their expected numbers, in FMG order.
const TYPES: [(&str, f64); 12] = [
    ("invasion", 2.0),
    ("rebels", 1.5),
    ("proselytism", 1.6),
    ("crusade", 1.6),
    ("disease", 1.4),
    ("disaster", 1.0),
    ("eruption", 1.0),
    ("avalanche", 0.8),
    ("fault", 1.0),
    ("flood", 1.0),
    ("tsunami", 1.0),
    ("famine", 1.0),
];

const INVASIONS: &[(&str, u32)] = &[
    ("Invasion", 5),
    ("Occupation", 4),
    ("Conquest", 3),
    ("Incursion", 2),
    ("Intervention", 2),
    ("Assault", 1),
    ("Foray", 1),
    ("Intrusion", 1),
    ("Irruption", 1),
    ("Offensive", 1),
    ("Pillaging", 1),
    ("Plunder", 1),
    ("Raid", 1),
    ("Skirmishes", 1),
];
const TAX_REVOLTS: &[(&str, u32)] = &[
    ("Tax Revolt", 3),
    ("Tax Rebels", 2),
    ("Tithe Riots", 1),
    ("Peasant Revolt", 1),
];
const REBELS: &[(&str, u32)] = &[
    ("Rebels", 5),
    ("Insurrection", 2),
    ("Mutineers", 1),
    ("Insurgents", 1),
    ("Rebellion", 1),
    ("Renegades", 1),
    ("Revolters", 1),
    ("Revolutionaries", 1),
    ("Rioters", 1),
    ("Separatists", 1),
    ("Secessionists", 1),
    ("Conspiracy", 1),
];
const DISEASE_MODELS: &[(&str, u32)] = &[("color", 2), ("animal", 1), ("adjective", 1)];
const DISEASES: &[(&str, u32)] = &[
    ("Fever", 5),
    ("Plague", 3),
    ("Cough", 3),
    ("Flu", 2),
    ("Pox", 2),
    ("Cholera", 2),
    ("Typhoid", 2),
    ("Leprosy", 1),
    ("Smallpox", 1),
    ("Pestilence", 1),
    ("Malaria", 1),
    ("Dropsy", 1),
];
const DISEASE_COLORS: &[&str] = &[
    "Amber", "Azure", "Black", "Blue", "Brown", "Crimson", "Emerald", "Golden", "Green", "Grey",
    "Orange", "Pink", "Purple", "Red", "Ruby", "Scarlet", "Silver", "Violet", "White", "Yellow",
];
const DISEASE_ANIMALS: &[&str] = &[
    "Ape", "Bear", "Bird", "Boar", "Cat", "Cow", "Deer", "Dog", "Fox", "Goat", "Horse", "Lion",
    "Pig", "Rat", "Raven", "Sheep", "Spider", "Tiger", "Viper", "Wolf", "Worm", "Wyrm",
];
const DISEASE_ADJECTIVES: &[&str] = &[
    "Blind",
    "Bloody",
    "Brutal",
    "Burning",
    "Deadly",
    "Fatal",
    "Furious",
    "Great",
    "Grim",
    "Horrible",
    "Invisible",
    "Lethal",
    "Loud",
    "Mortal",
    "Savage",
    "Severe",
    "Silent",
    "Unknown",
    "Venomous",
    "Vicious",
];
const DISASTERS: &[(&str, u32)] = &[
    ("Drought", 3),
    ("Earthquake", 3),
    ("Tornadoes", 1),
    ("Wildfires", 1),
    ("Storms", 1),
    ("Blight", 1),
];
const FAMINES: &[(&str, u32)] = &[
    ("Famine", 5),
    ("Hunger", 2),
    ("Dearth", 1),
    ("Starvation", 1),
];

/// The stability that the tax burden takes, its largest loss, for the
/// rebels of a province to be a tax revolt.
const TAX_REVOLT_BURDEN: f64 = 0.1;
/// The stability that a part takes from a land for the note to name it.
const UNREST_CAUSE: f64 = 0.05;
/// The height over the river that a bank can have and still flood.
const FLOOD_BANK: f64 = 3.0;
/// The rural prosperity percent, at most, where a famine breaks out.
const FAMINE_START: u8 = 20;
/// The rural prosperity percent, at most, that a famine spreads through.
const FAMINE_SPREAD: u8 = 30;

/// A binary heap as the `FlatQueue` of FMG: the same order for equal
/// priorities.
#[derive(Debug, Default)]
struct FlatQueue {
    ids: Vec<(usize, f64)>,
    values: Vec<f64>,
}

impl FlatQueue {
    fn len(&self) -> usize {
        self.ids.len()
    }

    fn push(&mut self, id: (usize, f64), value: f64) {
        let mut pos = self.ids.len();
        self.ids.push(id);
        self.values.push(value);
        while pos > 0 {
            let parent = (pos - 1) >> 1;
            let parent_value = self.values[parent];
            if value >= parent_value {
                break;
            }
            self.ids[pos] = self.ids[parent];
            self.values[pos] = parent_value;
            pos = parent;
        }
        self.ids[pos] = id;
        self.values[pos] = value;
    }

    fn pop(&mut self) -> Option<(usize, f64)> {
        if self.ids.is_empty() {
            return None;
        }
        let top = self.ids[0];
        let id = self.ids.pop().expect("not empty");
        let value = self.values.pop().expect("not empty");
        let length = self.ids.len();
        if length > 0 {
            let half = length >> 1;
            let mut pos = 0;
            while pos < half {
                let mut left = (pos << 1) + 1;
                let right = left + 1;
                let mut best_index = self.ids[left];
                let mut best_value = self.values[left];
                if right < length && self.values[right] < best_value {
                    left = right;
                    best_index = self.ids[right];
                    best_value = self.values[right];
                }
                if best_value >= value {
                    break;
                }
                self.ids[pos] = best_index;
                self.values[pos] = best_value;
                pos = left;
            }
            self.ids[pos] = id;
            self.values[pos] = value;
        }
        Some(top)
    }
}

/// `ra` of FMG: one draw, also for an empty list.
fn ra(rng: &mut Rand, len: usize) -> Option<usize> {
    let k = (rng.unit() * len as f64) as usize;
    (k < len).then_some(k)
}

/// One of the items, each as likely as its weight; with equal weights the
/// same pick as `ra` (`pickWeighted`).
fn pick_weighted(rng: &mut Rand, weights: &[f64]) -> Option<usize> {
    let total = weights.iter().fold(0.0, |s, w| s + w);
    if total == 0.0 {
        return ra(rng, weights.len());
    }
    let roll = rng.unit() * total;
    let mut passed = 0.0;
    for (k, w) in weights.iter().enumerate() {
        passed += w;
        if roll < passed {
            return Some(k);
        }
    }
    weights.len().checked_sub(1)
}

/// How unstable a land is: all alike without a stability.
fn instability(stability: Option<f64>) -> f64 {
    let s = stability.unwrap_or(0.5);
    (1.0 - s) * (1.0 - s)
}

/// What weakens a land most, as a sentence; none when nothing takes much
/// (`getUnrestCause`).
fn unrest_cause(parts: &Parts) -> Option<&'static str> {
    // The parts in the key order of FMG: the shared parts first.
    let causes = [
        (
            parts.army,
            Some("The army goes unpaid and does not keep order"),
        ),
        (
            parts.remoteness,
            Some("The capital is far away, and its rule is weak here"),
        ),
        (
            parts.diversity,
            Some("Its people are strangers to the ruling culture"),
        ),
        (
            parts.corruption,
            Some("Corrupt officials have turned the people against their rulers"),
        ),
    ];
    let (value, cause) = causes.iter().fold(
        (0.0, None),
        |worst, &(v, c)| if v < worst.0 { (v, c) } else { worst },
    );
    if value <= -UNREST_CAUSE { cause } else { None }
}

struct Gen<'a> {
    input: &'a Input<'a>,
    rng: Rand,
    used: Vec<bool>,
    chains: Vec<Chain>,
    /// The place index on each cell.
    burg: Vec<Option<usize>>,
    /// The route links of each cell (FMG `cells.routes`).
    links: Vec<BTreeSet<usize>>,
    zones: Vec<Zone>,
}

impl Gen<'_> {
    fn soc(&self) -> &Society {
        self.input.society
    }
    fn h(&self, c: usize) -> f64 {
        self.soc().cells.h[c]
    }
    fn pop(&self, c: usize) -> f64 {
        self.soc().cells.pop[c]
    }
    fn state_of(&self, c: usize) -> u32 {
        self.soc().cell_state[c]
    }
    fn neighbors(&self, c: usize) -> Vec<usize> {
        self.input.grid.neighbors(c).collect()
    }
    fn n(&self) -> usize {
        self.input.grid.len()
    }
    fn state_name(&self, id: u32) -> String {
        self.soc().states[id as usize - 1].name.clone()
    }
    /// A name of the culture of a cell (`Names.getCulture`).
    fn culture_name(&mut self, culture: u32) -> String {
        self.chains[culture as usize].get(&mut self.rng)
    }
    /// A short name of a culture (`Names.getCultureShort`).
    fn culture_short(&mut self, culture: u32) -> String {
        self.chains[culture as usize].get_short(&mut self.rng)
    }
    fn adjective(&mut self, noun: &str) -> String {
        adjective(noun, &mut self.rng)
    }
    fn push(
        &mut self,
        name: String,
        note: String,
        kind: &'static str,
        cells: Vec<usize>,
        color: &'static str,
    ) {
        self.zones.push(Zone {
            id: self.zones.len() as u32,
            name,
            kind,
            cells,
            color,
            note,
        });
    }

    fn invasion(&mut self) {
        let soc = self.input.society;
        let ongoing: Vec<&crate::diplomacy::Campaign> = soc
            .states
            .iter()
            .flat_map(|s| &s.campaigns)
            .filter(|c| c.end.is_none())
            .collect();
        if ongoing.is_empty() {
            return;
        }
        let k = ra(&mut self.rng, ongoing.len()).expect("not empty");
        let (defender, attacker, war) = (
            ongoing[k].defender,
            ongoing[k].attacker,
            ongoing[k].name.clone(),
        );
        let border: Vec<usize> = (0..self.n())
            .filter(|&c| {
                !self.used[c]
                    && self.state_of(c) == defender
                    && self
                        .input
                        .grid
                        .neighbors(c)
                        .any(|n| self.state_of(n) == attacker)
            })
            .collect();
        // Invaders strike where the land is richest.
        let prosperity = &self.input.economy.prosperity;
        let weights: Vec<f64> = border
            .iter()
            .map(|&c| {
                let p = f64::from(prosperity.get(c).copied().unwrap_or(0));
                p * p
            })
            .collect();
        let Some(start) = pick_weighted(&mut self.rng, &weights).map(|k| border[k]) else {
            return;
        };
        let mut cells = Vec::new();
        let mut queue = VecDeque::from([start]);
        let max = self.rng.rand(5, 30) as usize;
        while !queue.is_empty() {
            let c = if self.rng.p(0.4) {
                queue.pop_front()
            } else {
                queue.pop_back()
            }
            .expect("not empty");
            cells.push(c);
            if cells.len() >= max {
                break;
            }
            for n in self.neighbors(c) {
                if self.used[n] || self.state_of(n) != defender {
                    continue;
                }
                self.used[n] = true;
                queue.push_back(n);
            }
        }
        let subtype = self.rng.weighted(INVASIONS);
        let (att, def) = (self.state_name(attacker), self.state_name(defender));
        let name = format!("{} {subtype}", self.adjective(&att));
        let note = format!(
            "Troops of {att} have crossed into {def} in the {war}. They raid, besiege and hold the land here, the richest along the border, and its people flee before them."
        );
        self.push(name, note, "Invasion", cells, "url(#hatch1)");
    }

    fn rebels(&mut self) {
        let soc = self.input.society;
        let stability = self.input.stability;
        let candidates: Vec<usize> = (0..soc.states.len())
            .filter(|&k| soc.states[k].neighbors.iter().any(|&n| n != NONE))
            .collect();
        let weights: Vec<f64> = candidates
            .iter()
            .map(|&k| instability(stability.state(soc.states[k].id).map(|s| s.stability)))
            .collect();
        let Some(st) = pick_weighted(&mut self.rng, &weights).map(|k| candidates[k]) else {
            return;
        };
        let state = &soc.states[st];
        let neighbors: Vec<u32> = state
            .neighbors
            .iter()
            .copied()
            .filter(|&n| n != NONE)
            .collect();
        let Some(neib) = ra(&mut self.rng, neighbors.len()).map(|k| neighbors[k]) else {
            return;
        };
        // The provinces with people, by index in the list of the state.
        let all = stability.provinces(state.id);
        let provinces: Vec<usize> = (0..all.len())
            .filter(|&k| all[k].province.people != 0.0)
            .collect();
        let unstable = if provinces.is_empty() {
            None
        } else {
            let w: Vec<f64> = provinces
                .iter()
                .map(|&k| instability(Some(all[k].stability)))
                .collect();
            pick_weighted(&mut self.rng, &w).map(|k| provinces[k])
        };
        let corruption = &self.input.economy.corruption;
        let inside = |c: usize| {
            soc.cell_state[c] == state.id
                && unstable.is_none_or(|k| {
                    corruption.of_cell.get(c).copied().flatten() == Some((state.id, k))
                })
        };
        let grid = self.input.grid;
        let at_border = |c: usize| grid.neighbors(c).any(|n| soc.cell_state[n] == neib);
        let border_cell = (0..self.n()).find(|&c| inside(c) && at_border(c));
        // Away from the neighbor, it rises around the capital of the
        // province, or anywhere when that lies outside it.
        let risen = unstable.map(|k| &all[k]);
        let capital = risen
            .and_then(|d| soc.places.get((d.province.burg as usize).wrapping_sub(1)))
            .map(|p| p.cell);
        let start = border_cell.or_else(|| {
            risen?;
            match capital {
                Some(c) if inside(c) => Some(c),
                _ => (0..self.n()).find(|&c| inside(c)),
            }
        });
        let mut cells = Vec::new();
        let mut queue: VecDeque<usize> = start.into_iter().collect();
        let max = self.rng.rand(10, 30) as usize;
        while let Some(c) = queue.pop_front() {
            cells.push(c);
            if cells.len() >= max {
                break;
            }
            for n in self.neighbors(c) {
                if self.used[n] || !inside(n) {
                    continue;
                }
                self.used[n] = true;
                if border_cell.is_some() && n % 4 != 0 && !at_border(n) {
                    continue;
                }
                queue.push_back(n);
            }
        }
        let parts = risen.map(|d| d.parts);
        let tax_revolt = parts.is_some_and(|p| {
            p.burden <= -TAX_REVOLT_BURDEN && p.values().iter().all(|&v| v >= p.burden)
        });
        let rebels = if tax_revolt {
            self.rng.weighted(TAX_REVOLTS)
        } else {
            self.rng.weighted(REBELS)
        };
        let province = risen
            .filter(|d| d.province.id != NONE)
            .and_then(|d| soc.provinces.get(d.province.id as usize - 1));
        let neib_name = self.state_name(neib);
        let named = if tax_revolt {
            province.map_or(state.name.clone(), |p| p.name.clone())
        } else if border_cell.is_some() || province.is_none() {
            neib_name.clone()
        } else {
            province.map(|p| p.name.clone()).unwrap_or_default()
        };
        let name = format!("{} {rebels}", self.adjective(&named));
        let place = province.map_or(state.name.clone(), |p| p.name.clone());
        let cause = parts
            .as_ref()
            .and_then(unrest_cause)
            .map_or(String::new(), |c| format!(". {c}"));
        let note = if tax_revolt {
            format!(
                "Crushed by the taxes of {}, the people of {place} refuse to pay and have taken up arms against the tax collectors.",
                state.name
            )
        } else if border_cell.is_some() {
            format!(
                "People of {} along the border with {neib_name} have taken up arms against their rulers, backed by their neighbor{cause}.",
                state.name
            )
        } else {
            format!(
                "{place} has risen against the rule of {}{cause}.",
                state.name
            )
        };
        self.push(name, note, "Rebels", cells, "url(#hatch3)");
    }

    fn proselytism(&mut self) {
        let religions = self.input.religions;
        let organized: Vec<usize> = (0..religions.list.len())
            .filter(|&k| religions.list[k].kind == "Organized")
            .collect();
        let Some(r) = ra(&mut self.rng, organized.len()).map(|k| &religions.list[organized[k]])
        else {
            return;
        };
        let faith = &religions.cell_religion;
        let grid = self.input.grid;
        let targets: Vec<usize> = (0..self.n())
            .filter(|&c| {
                self.h(c) >= 20.0
                    && self.pop(c) != 0.0
                    && faith[c] != r.id
                    && grid.neighbors(c).any(|n| faith[n] == r.id)
            })
            .collect();
        let start = ra(&mut self.rng, targets.len()).map(|k| targets[k]);
        let Some(start) = start.filter(|&c| c != 0) else {
            return;
        };
        let target = faith[start];
        let mut cells = Vec::new();
        let mut queue = VecDeque::from([start]);
        let max = self.rng.rand(10, 30) as usize;
        while let Some(c) = queue.pop_front() {
            cells.push(c);
            if cells.len() >= max {
                break;
            }
            for n in self.neighbors(c) {
                if self.used[n] || faith[n] != target || self.h(n) < 20.0 || self.pop(n) == 0.0 {
                    continue;
                }
                self.used[n] = true;
                queue.push_back(n);
            }
        }
        let first = r.name.split(' ').next().unwrap_or_default().to_owned();
        let name = format!("{} Proselytism", self.adjective(&first));
        let other = if target == NONE {
            "No religion".to_owned()
        } else {
            religions
                .list
                .iter()
                .find(|x| x.id == target)
                .map_or_else(|| "other faiths".to_owned(), |x| x.name.clone())
        };
        let note = format!(
            "Missionaries of {} preach among the followers of {other} here, winning converts village by village.",
            r.name
        );
        self.push(name, note, "Proselytism", cells, "url(#hatch6)");
    }

    fn crusade(&mut self) {
        let religions = self.input.religions;
        let heresies: Vec<usize> = (0..religions.list.len())
            .filter(|&k| religions.list[k].kind == "Heresy")
            .collect();
        if heresies.is_empty() {
            return;
        }
        let heresy =
            &religions.list[heresies[ra(&mut self.rng, heresies.len()).expect("not empty")]];
        let cells: Vec<usize> = (0..self.n())
            .filter(|&c| !self.used[c] && religions.cell_religion[c] == heresy.id)
            .collect();
        if cells.is_empty() {
            return;
        }
        for &c in &cells {
            self.used[c] = true;
        }
        let first = heresy.name.split(' ').next().unwrap_or_default().to_owned();
        let name = format!("{} Crusade", self.adjective(&first));
        let note = format!(
            "{} is condemned as heresy. Crusaders march against its followers wherever they live, burning their shrines.",
            heresy.name
        );
        self.push(name, note, "Crusade", cells, "url(#hatch6)");
    }

    /// Spread from a cell over the cells that cost at most `max`, with the
    /// cost of each step (the queue of `addDisease` and `addDisaster`).
    fn spread(
        &mut self,
        start: usize,
        max: f64,
        step: &mut dyn FnMut(&mut Self, usize, usize) -> f64,
    ) -> Vec<usize> {
        let mut cells = Vec::new();
        let mut cost: BTreeMap<usize, f64> = BTreeMap::new();
        let mut queue = FlatQueue::default();
        queue.push((start, 0.0), 0.0);
        while queue.len() > 0 {
            let (e, p0) = queue.pop().expect("not empty");
            if self.burg[e].is_some() || self.pop(e) != 0.0 {
                cells.push(e);
            }
            self.used[e] = true;
            for n in self.neighbors(e) {
                let p = p0 + step(self, e, n);
                if p > max {
                    continue;
                }
                let known = cost.get(&n).copied().unwrap_or(0.0);
                if known == 0.0 || p < known {
                    cost.insert(n, p);
                    queue.push((n, p), p);
                }
            }
        }
        cells
    }

    fn disease(&mut self) {
        let soc = self.input.society;
        // Disease breaks out in the busiest markets and ports.
        let candidates: Vec<usize> = (0..soc.places.len())
            .filter(|&k| !self.used[soc.places[k].cell])
            .collect();
        let mut trade = vec![0.0; soc.places.len() + 1];
        for d in &self.input.economy.deals {
            if d.kind != DealKind::Kept {
                trade[d.burg as usize] += d.units * d.price;
            }
        }
        let weights: Vec<f64> = candidates
            .iter()
            .map(|&k| {
                let p = &soc.places[k];
                libm::sqrt(trade[p.id as usize]) * if p.port { 2.0 } else { 1.0 }
            })
            .collect();
        let Some(k) = pick_weighted(&mut self.rng, &weights).map(|k| candidates[k]) else {
            return;
        };
        let burg = &soc.places[k];
        let max = self.rng.rand(20, 40) as f64;
        let cells = self.spread(burg.cell, max, &mut |g, a, b| {
            if g.links[a].contains(&b) { 5.0 } else { 100.0 }
        });
        let color = self.rng.pick(DISEASE_COLORS).to_string();
        let animal = self.rng.pick(DISEASE_ANIMALS).to_string();
        let adj = self.rng.pick(DISEASE_ADJECTIVES).to_string();
        let prefix = match self.rng.weighted(DISEASE_MODELS) {
            "color" => color,
            "animal" => animal,
            _ => adj,
        };
        let disease = self.rng.weighted(DISEASES);
        let name = format!("{prefix} {disease}");
        let origin = if burg.port {
            format!("the port of {}, brought by ships", burg.name)
        } else {
            format!("the market of {}", burg.name)
        };
        let note = format!(
            "An outbreak of {} that began in {origin}. Merchants and travelers carry it along the roads.",
            name.to_lowercase()
        );
        self.push(name, note, "Disease", cells, "url(#hatch12)");
    }

    fn disaster(&mut self) {
        let soc = self.input.society;
        let candidates: Vec<usize> = (0..soc.places.len())
            .filter(|&k| !self.used[soc.places[k].cell])
            .collect();
        let Some(k) = ra(&mut self.rng, candidates.len()).map(|k| candidates[k]) else {
            return;
        };
        let burg = &soc.places[k];
        self.used[burg.cell] = true;
        let max = self.rng.rand(5, 25) as f64;
        let cells = self.spread(burg.cell, max, &mut |g, _, _| g.rng.rand(1, 10) as f64);
        let kind = self.rng.weighted(DISASTERS);
        let name = format!("{} {kind}", self.adjective(&burg.name));
        let b = &burg.name;
        let note = match kind {
            "Drought" => format!(
                "No rain has fallen around {b} for months. Wells run dry and the crops wither in the fields."
            ),
            "Earthquake" => format!(
                "An earthquake has struck {b} and the land around, toppling houses and blocking roads."
            ),
            "Tornadoes" => format!(
                "Tornadoes have torn through the land around {b}, leaving a trail of wrecked farms."
            ),
            "Wildfires" => format!(
                "Wildfires burn around {b}, and smoke hangs over the burnt fields and woods."
            ),
            "Storms" => format!(
                "Storms batter {b} and the land around, flattening crops and tearing off roofs."
            ),
            _ => format!("A blight rots the crops around {b}, and the harvest is lost."),
        };
        self.push(name, note, "Disaster", cells, "url(#hatch5)");
    }

    fn eruption(&mut self, markers: &mut [Marker]) {
        let Some(m) = markers
            .iter_mut()
            .find(|m| m.kind == "volcanoes" && !self.used[m.cell])
        else {
            return;
        };
        self.used[m.cell] = true;
        if let Some(note) = &mut m.legend {
            *note = note.replacen("Active volcano", "Erupting volcano", 1);
        }
        let name = match &m.name {
            Some(n) if !n.is_empty() => format!("{} Eruption", n.replacen(" Volcano", "", 1)),
            _ => "Volcano Eruption".to_owned(),
        };
        let volcano = m.name.clone().filter(|n| !n.is_empty());
        let start = m.cell;
        let mut cells = Vec::new();
        let mut queue = VecDeque::from([start]);
        let max = self.rng.rand(10, 30) as usize;
        while !queue.is_empty() {
            let c = if self.rng.p(0.5) {
                queue.pop_front()
            } else {
                queue.pop_back()
            }
            .expect("not empty");
            cells.push(c);
            if cells.len() >= max {
                break;
            }
            for n in self.neighbors(c) {
                if self.used[n] || self.h(n) < 20.0 {
                    continue;
                }
                self.used[n] = true;
                queue.push_back(n);
            }
        }
        let note = format!(
            "{} is erupting. Ash and lava cover the land around it, and people flee its slopes.",
            volcano.unwrap_or_else(|| "The volcano".to_owned())
        );
        self.push(name, note, "Eruption", cells, "url(#hatch7)");
    }

    fn avalanche(&mut self) {
        let candidates: Vec<usize> = (0..self.n())
            .filter(|&c| !self.used[c] && !self.links[c].is_empty() && self.h(c) >= 70.0)
            .collect();
        if candidates.is_empty() {
            return;
        }
        let start = candidates[ra(&mut self.rng, candidates.len()).expect("not empty")];
        self.used[start] = true;
        let mut cells = Vec::new();
        let mut queue = VecDeque::from([start]);
        let max = self.rng.rand(3, 15) as usize;
        while !queue.is_empty() {
            let c = if self.rng.p(0.3) {
                queue.pop_front()
            } else {
                queue.pop_back()
            }
            .expect("not empty");
            cells.push(c);
            if cells.len() >= max {
                break;
            }
            for n in self.neighbors(c) {
                if self.used[n] || self.h(n) < 65.0 {
                    continue;
                }
                self.used[n] = true;
                queue.push_back(n);
            }
        }
        let culture = self.soc().cell_culture[start];
        let short = self.culture_short(culture);
        let name = format!("{} Avalanche", self.adjective(&short));
        let note = "Avalanches have buried the mountain road here. The pass is closed until the snow is cleared.".to_owned();
        self.push(name, note, "Avalanche", cells, "url(#hatch5)");
    }

    fn fault(&mut self) {
        let candidates: Vec<usize> = (0..self.n())
            .filter(|&c| !self.used[c] && self.h(c) > 50.0 && self.h(c) < 70.0)
            .collect();
        if candidates.is_empty() {
            return;
        }
        let start = candidates[ra(&mut self.rng, candidates.len()).expect("not empty")];
        self.used[start] = true;
        let mut cells = Vec::new();
        let mut queue = vec![start];
        let max = self.rng.rand(3, 15) as usize;
        while let Some(c) = queue.pop() {
            if self.h(c) >= 20.0 {
                cells.push(c);
            }
            if cells.len() >= max {
                break;
            }
            for n in self.neighbors(c) {
                if self.used[n] || self.soc().cells.r[n] != 0 {
                    continue;
                }
                self.used[n] = true;
                queue.push(n);
            }
        }
        let culture = self.soc().cell_culture[start];
        let short = self.culture_short(culture);
        let name = format!("{} Fault", self.adjective(&short));
        let note = "A fault line where the ground cracks and shakes with frequent tremors. Few dare to build here.".to_owned();
        self.push(name, note, "Fault", cells, "url(#hatch2)");
    }

    fn flood(&mut self) {
        let soc = self.input.society;
        let cells_data = &soc.cells;
        let flux: Vec<f64> = cells_data
            .fl
            .iter()
            .copied()
            .filter(|&f| f != 0.0)
            .collect();
        let mean = if flux.is_empty() {
            0.0
        } else {
            flux.iter().fold(0.0, |s, f| s + f) / flux.len() as f64
        };
        let big = |c: usize| cells_data.r[c] != 0 && cells_data.fl[c] >= mean;
        // A lowland place on a bigger river than most, the bigger the
        // likelier.
        let candidates: Vec<usize> = (0..self.n())
            .filter(|&c| !self.used[c] && self.h(c) < 50.0 && self.burg[c].is_some() && big(c))
            .collect();
        let weights: Vec<f64> = candidates.iter().map(|&c| cells_data.fl[c]).collect();
        let Some(start) = pick_weighted(&mut self.rng, &weights).map(|k| candidates[k]) else {
            return;
        };
        self.used[start] = true;
        let river = cells_data.r[start];
        let mut cells = Vec::new();
        let mut queue = VecDeque::from([start]);
        let max = self.rng.rand(5, 30) as usize;
        while cells.len() < max {
            let Some(c) = queue.pop_front() else { break };
            cells.push(c);
            // A flooded bank spreads no further.
            if !big(c) {
                continue;
            }
            for n in self.neighbors(c) {
                if self.used[n] || self.h(n) < 20.0 || self.h(n) >= 50.0 {
                    continue;
                }
                if !big(n) && self.h(n) > self.h(c) + FLOOD_BANK {
                    continue;
                }
                self.used[n] = true;
                queue.push_back(n);
            }
        }
        let burg = soc.places[self.burg[start].expect("a flood starts at a place")]
            .name
            .clone();
        let name = format!("{} Flood", self.adjective(&burg));
        let names = self.input.names;
        let river = self
            .input
            .rivers
            .iter()
            .position(|r| r.id == river)
            .and_then(|k| names.rivers.get(k))
            .map_or_else(
                || "river".to_owned(),
                |n| format!("{} {}", n.name, n.kind.to_lowercase()),
            );
        let note = format!(
            "The {river} has burst its banks at {burg}, flooding the streets and the fields along it."
        );
        self.push(name, note, "Flood", cells, "url(#hatch13)");
    }

    fn tsunami(&mut self) {
        let features = self.input.features;
        let t = &self.input.society.cells.t;
        let lake = |c: usize| features.get(features.of_cell[c]).kind == FeatureKind::Lake;
        let candidates: Vec<usize> = (0..self.n())
            .filter(|&c| !self.used[c] && t[c] == -1 && !lake(c))
            .collect();
        if candidates.is_empty() {
            return;
        }
        let start = candidates[ra(&mut self.rng, candidates.len()).expect("not empty")];
        self.used[start] = true;
        let mut cells = Vec::new();
        let mut queue = VecDeque::from([start]);
        let max = self.rng.rand(10, 30) as usize;
        while let Some(c) = queue.pop_front() {
            if t[c] == 1 {
                cells.push(c);
            }
            if cells.len() >= max {
                break;
            }
            for n in self.neighbors(c) {
                if self.used[n] || t[n] > 2 || lake(n) {
                    continue;
                }
                self.used[n] = true;
                queue.push_back(n);
            }
        }
        let culture = self.soc().cell_culture[start];
        let short = self.culture_short(culture);
        let name = format!("{} Tsunami", self.adjective(&short));
        let note = "A great wave from the sea has struck this coast, wrecking harbors and drowning the villages along the shore.".to_owned();
        self.push(name, note, "Tsunami", cells, "url(#hatch13)");
    }

    /// A famine breaks out in the poorest countryside and spreads through
    /// the poor land around (`addFamine`).
    fn famine(&mut self) {
        let prosperity = &self.input.economy.prosperity;
        if prosperity.is_empty() {
            return;
        }
        let poor = |g: &Self, c: usize, top: u8| {
            !g.used[c] && g.pop(c) > 0.0 && prosperity[c] > 0 && prosperity[c] <= top
        };
        let poorest: Vec<usize> = (0..self.n())
            .filter(|&c| poor(self, c, FAMINE_START))
            .collect();
        let weights: Vec<f64> = poorest
            .iter()
            .map(|&c| {
                let d = f64::from(FAMINE_START + 1 - prosperity[c]);
                d * d
            })
            .collect();
        let Some(start) = pick_weighted(&mut self.rng, &weights).map(|k| poorest[k]) else {
            return;
        };
        let mut cells = Vec::new();
        let mut queue = VecDeque::from([start]);
        self.used[start] = true;
        let max = self.rng.rand(10, 40) as usize;
        while cells.len() < max {
            let Some(c) = queue.pop_front() else { break };
            cells.push(c);
            for n in self.neighbors(c) {
                if !poor(self, n, FAMINE_SPREAD) {
                    continue;
                }
                self.used[n] = true;
                queue.push_back(n);
            }
        }
        if cells.len() < 3 {
            return;
        }
        let state = self.state_of(start);
        let place = if state != NONE {
            self.state_name(state)
        } else {
            let culture = self.soc().cell_culture[start];
            self.culture_name(culture)
        };
        let famine = self.rng.weighted(FAMINES);
        let name = format!("{} {famine}", self.adjective(&place));
        let note = format!(
            "The poorest countryside of {place}. The harvest has failed, and its people have no money to buy grain from the markets, so they go hungry."
        );
        self.push(name, note, "Famine", cells, "url(#hatch10)");
    }
}

/// Make the zones (`Zones.generate`). An eruption changes the legend of
/// its volcano marker.
pub fn generate(input: &Input, markers: &mut [Marker]) -> Vec<Zone> {
    let soc = input.society;
    let n = input.grid.len();
    let mut burg = vec![None; n];
    for (k, p) in soc.places.iter().enumerate() {
        burg[p.cell] = Some(k);
    }
    let mut links: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];
    for r in input.roads.iter().chain(input.sea_routes) {
        for w in r.cells.windows(2) {
            if w[0] != w[1] {
                links[w[0]].insert(w[1]);
                links[w[1]].insert(w[0]);
            }
        }
    }
    let chains: Vec<Chain> = std::iter::once(Chain::new(crate::cultures::WILDLANDS_BASE))
        .chain(soc.cultures.iter().map(|c| Chain::new(c.base)))
        .collect();
    let mut g = Gen {
        input,
        rng: Rand::new(SeedTree::new(input.config.seed).part_rng(Layer::Zones, "generate")),
        used: vec![false; n],
        chains,
        burg,
        links,
        zones: Vec::new(),
    };
    for (kind, quantity) in TYPES {
        let number = g.rng.gauss(quantity, quantity / 2.0, 0.0, 100.0, 0) as u32;
        for _ in 0..number {
            match kind {
                "invasion" => g.invasion(),
                "rebels" => g.rebels(),
                "proselytism" => g.proselytism(),
                "crusade" => g.crusade(),
                "disease" => g.disease(),
                "disaster" => g.disaster(),
                "eruption" => g.eruption(markers),
                "avalanche" => g.avalanche(),
                "fault" => g.fault(),
                "flood" => g.flood(),
                "tsunami" => g.tsunami(),
                _ => g.famine(),
            }
        }
    }
    // A spreading zone can reach a cell twice.
    for z in &mut g.zones {
        let mut seen = BTreeSet::new();
        z.cells.retain(|&c| seen.insert(c));
    }
    g.zones
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flat_queue_pops_the_lowest_first() {
        let mut q = FlatQueue::default();
        for (k, v) in [5.0, 1.0, 4.0, 1.0, 3.0, 2.0].into_iter().enumerate() {
            q.push((k, v), v);
        }
        let mut out = Vec::new();
        while let Some((_, v)) = q.pop() {
            out.push(v);
        }
        assert_eq!(out, [1.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn a_weighted_pick_follows_the_weights() {
        let mut rng = Rand::new(SeedTree::new(1).part_rng(Layer::Zones, "t"));
        for _ in 0..50 {
            assert_eq!(pick_weighted(&mut rng, &[0.0, 3.0, 0.0]), Some(1));
        }
        assert_eq!(pick_weighted(&mut rng, &[]), None);
        assert_eq!(instability(None), 0.25);
        assert_eq!(instability(Some(1.0)), 0.0);
    }

    #[test]
    fn the_worst_part_names_the_cause() {
        let mut p = Parts::default();
        assert_eq!(unrest_cause(&p), None);
        p.remoteness = -0.04;
        assert_eq!(unrest_cause(&p), None);
        p.corruption = -0.08;
        p.diversity = -0.06;
        assert_eq!(
            unrest_cause(&p),
            Some("Corrupt officials have turned the people against their rulers")
        );
        // The burden is not a cause of the note.
        p.burden = -0.15;
        assert!(unrest_cause(&p).is_some_and(|c| c.starts_with("Corrupt")));
    }

    #[test]
    fn zones_have_cells_names_and_hatches() {
        let c: crate::config::Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 4, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 5000, "template": "continents", "states": 7 }
        }))
        .unwrap();
        let w = crate::world::World::build(&c).unwrap();
        assert!(!w.zones.is_empty());
        let mut kinds = BTreeSet::new();
        for (k, z) in w.zones.iter().enumerate() {
            assert_eq!(z.id, k as u32);
            assert!(!z.name.is_empty() && !z.note.is_empty());
            assert!(z.color.starts_with("url(#hatch"));
            let unique: BTreeSet<usize> = z.cells.iter().copied().collect();
            assert_eq!(unique.len(), z.cells.len());
            assert!(z.cells.iter().all(|&c| c < w.grid.len()));
            kinds.insert(z.kind);
            if z.kind == "Famine" {
                assert!(z.cells.len() >= 3);
                assert!(
                    z.cells
                        .iter()
                        .all(|&c| w.economy.prosperity[c] <= FAMINE_SPREAD)
                );
            }
            if z.kind == "Invasion" {
                let s = w.society.cell_state[z.cells[0]];
                assert!(z.cells.iter().all(|&c| w.society.cell_state[c] == s));
            }
        }
        assert!(kinds.len() >= 4, "{kinds:?}");
        assert_eq!(crate::world::World::build(&c).unwrap().zones, w.zones);
    }
}
