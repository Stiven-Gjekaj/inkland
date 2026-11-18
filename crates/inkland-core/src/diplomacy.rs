//! Diplomacy (`states-generator.ts`): `generateCampaigns` and
//! `generateDiplomacy`, with the relation table of `data/diplomacy.ts`.
//!
//! - Each state gets a relation to each other state. Neighbors are more
//!   often allies or rivals, far states are more often neutral or unknown.
//! - A strong neighbor can make a weak state its vassal. A vassal copies
//!   the relations of its suzerain.
//! - A state with a rival can declare a war. Vassals and allies join the
//!   war, and the chronicle records each step.
//! - The campaigns of each state are old wars with its neighbors. An
//!   ongoing war adds a campaign without an end.

use crate::cultures::CultureType;
use crate::features::Features;
use crate::names::Chain;
use crate::population::Cells;
use crate::rand::Rand;
use crate::society::{NONE, State};

/// The calendar year of FMG (`options.map.lore.calendar.year`).
pub const YEAR: f64 = 1000.0;

/// The relation of a state to another state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Relation {
    /// No relation: the state itself, or the neutral lands (FMG `x`).
    None,
    Ally,
    Friendly,
    Neutral,
    Suspicion,
    Enemy,
    Unknown,
    Rival,
    Vassal,
    Suzerain,
}

impl Relation {
    /// All relations with a name, in the order of FMG `RELATIONS`.
    pub const ALL: [Relation; 9] = [
        Relation::Ally,
        Relation::Friendly,
        Relation::Neutral,
        Relation::Suspicion,
        Relation::Enemy,
        Relation::Unknown,
        Relation::Rival,
        Relation::Vassal,
        Relation::Suzerain,
    ];

    /// The FMG name of the relation. `None` is `x`.
    pub fn name(self) -> &'static str {
        match self {
            Relation::None => "x",
            Relation::Ally => "Ally",
            Relation::Friendly => "Friendly",
            Relation::Neutral => "Neutral",
            Relation::Suspicion => "Suspicion",
            Relation::Enemy => "Enemy",
            Relation::Unknown => "Unknown",
            Relation::Rival => "Rival",
            Relation::Vassal => "Vassal",
            Relation::Suzerain => "Suzerain",
        }
    }

    /// The relation with an FMG name.
    pub fn from_name(name: &str) -> Option<Relation> {
        std::iter::once(Relation::None)
            .chain(Relation::ALL)
            .find(|r| r.name() == name)
    }

    /// The FMG color of the relation (`RELATIONS[..].color`).
    pub fn color(self) -> &'static str {
        match self {
            Relation::None => "#4682b4",
            Relation::Ally => "#00b300",
            Relation::Friendly => "#d4f8aa",
            Relation::Neutral => "#edeee8",
            Relation::Suspicion => "#eeafaa",
            Relation::Enemy => "#e64b40",
            Relation::Unknown => "#a9a9a9",
            Relation::Rival => "#ad5a1f",
            Relation::Vassal => "#87CEFA",
            Relation::Suzerain => "#00008B",
        }
    }

    /// The relation that the other state holds back
    /// (`getInverseRelation`).
    pub fn inverse(self) -> Relation {
        match self {
            Relation::Vassal => Relation::Suzerain,
            Relation::Suzerain => Relation::Vassal,
            r => r,
        }
    }

    /// "subject is an ally of object" (`RELATIONS[..].describe`).
    pub fn describe(self, subject: &str, object: &str) -> String {
        match self {
            Relation::None => format!("{subject} has no relation to {object}"),
            Relation::Ally => format!("{subject} is an ally of {object}"),
            Relation::Friendly => format!("{subject} is friendly to {object}"),
            Relation::Neutral => format!("{subject} is neutral to {object}"),
            Relation::Suspicion => format!("{subject} is suspicious of {object}"),
            Relation::Enemy => format!("{subject} is at war with {object}"),
            Relation::Unknown => format!("{subject} does not know about {object}"),
            Relation::Rival => format!("{subject} is a rival of {object}"),
            Relation::Vassal => format!("{subject} is a vassal of {object}"),
            Relation::Suzerain => format!("{subject} is suzerain to {object}"),
        }
    }
}

/// A war of a state: an old campaign, or an ongoing war without an end.
#[derive(Debug, Clone, PartialEq)]
pub struct Campaign {
    pub name: String,
    /// The first year.
    pub start: f64,
    /// The last year. An ongoing war has no end.
    pub end: Option<f64>,
    /// The state id of the attacker.
    pub attacker: u32,
    /// The state id of the defender, or 0 for the people of the culture.
    pub defender: u32,
}

/// The kinds of old wars and their weights (`generateCampaign`).
pub const WARS: [(&str, u32); 9] = [
    ("War", 6),
    ("Conflict", 2),
    ("Campaign", 4),
    ("Invasion", 2),
    ("Rebellion", 2),
    ("Conquest", 2),
    ("Intervention", 1),
    ("Expedition", 1),
    ("Crusade", 1),
];

/// The campaigns of each state (`generateCampaigns`). Chain 0 is the
/// wildlands; chain `k` is culture `k`.
pub fn campaigns(states: &mut [State], chains: &[Chain], rng: &mut Rand) {
    for k in 0..states.len() {
        let neighbors: Vec<u32> = if states[k].neighbors.is_empty() {
            vec![NONE]
        } else {
            states[k].neighbors.clone()
        };
        let culture = states[k].culture as usize;
        let mut list: Vec<Campaign> = neighbors
            .iter()
            .map(|&i| {
                let name = if i != NONE && rng.p(0.8) {
                    states[i as usize - 1].name.clone()
                } else {
                    chains[culture.min(chains.len() - 1)].get_short(rng)
                };
                let start = rng.gauss(YEAR - 100.0, 150.0, 1.0, YEAR - 6.0, 0);
                let end = start + rng.gauss(4.0, 5.0, 1.0, YEAR - start - 1.0, 0);
                let adj = crate::names::adjective(&name, rng);
                Campaign {
                    name: format!("{adj} {}", rng.weighted(&WARS)),
                    start,
                    end: Some(end),
                    attacker: states[k].id,
                    defender: i,
                }
            })
            .collect();
        list.sort_by(|a, b| a.start.total_cmp(&b.start));
        states[k].campaigns = list;
    }
}

// Relations to neighbors, to neighbors of neighbors, to far states, and
// between naval powers on other landmasses.
const NEIBS: [(&str, u32); 5] = [
    ("Ally", 1),
    ("Friendly", 2),
    ("Neutral", 1),
    ("Suspicion", 10),
    ("Rival", 9),
];
const NEIBS_OF_NEIBS: [(&str, u32); 4] = [
    ("Ally", 10),
    ("Friendly", 8),
    ("Neutral", 5),
    ("Suspicion", 1),
];
const FAR: [(&str, u32); 4] = [
    ("Friendly", 1),
    ("Neutral", 12),
    ("Suspicion", 2),
    ("Unknown", 6),
];
const NAVALS: [(&str, u32); 4] = [
    ("Neutral", 1),
    ("Suspicion", 2),
    ("Rival", 1),
    ("Unknown", 1),
];

fn draw(rng: &mut Rand, table: &[(&str, u32)]) -> Relation {
    Relation::from_name(rng.weighted(table)).unwrap_or(Relation::Neutral)
}

/// What diplomacy reads.
pub struct Input<'a> {
    pub cells: &'a Cells,
    pub features: &'a Features,
    pub cell_state: &'a [u32],
}

/// Set the relations of all states and return the chronicle
/// (`generateDiplomacy`). A record of the chronicle is a title and its
/// lines. `State::diplomacy` has one entry for each state id, and entry 0
/// is the neutral lands.
pub fn generate(input: &Input, states: &mut [State], rng: &mut Rand) -> Vec<Vec<String>> {
    let mut chronicle: Vec<Vec<String>> = Vec::new();
    let n = states.len() + 1;
    for s in states.iter_mut() {
        s.diplomacy = vec![Relation::None; n];
    }
    if states.len() < 2 {
        return chronicle;
    }
    // The state population approximates its power.
    let mut pops = vec![0.0f64; n];
    for (c, &s) in input.cell_state.iter().enumerate() {
        if input.cells.land(c) && s != NONE {
            pops[s as usize] += input.cells.pop[c];
        }
    }
    let pop_mean = pops[1..].iter().sum::<f64>() / states.len() as f64;
    // Index by state id: `st(i)` is the state with id `i`.
    let ix = |id: usize| id - 1;
    let vassal_of = |st: &State| st.diplomacy.iter().position(|&r| r == Relation::Vassal);

    // Generic relations.
    for f in 1..n {
        if let Some(suzerain) = vassal_of(&states[ix(f)]) {
            // Vassals copy relations from their suzerains.
            for i in 1..n {
                if i == f || i == suzerain {
                    continue;
                }
                let r = states[ix(suzerain)].diplomacy[i];
                states[ix(f)].diplomacy[i] = if r == Relation::Suzerain {
                    Relation::Ally
                } else {
                    r
                };
            }
            // The other states hold the relation they hold to the
            // suzerain.
            for e in 1..n {
                if e == f || e == suzerain {
                    continue;
                }
                let es = states[ix(e)].diplomacy[suzerain];
                if es != Relation::Suzerain && es != Relation::Vassal {
                    states[ix(e)].diplomacy[f] = es;
                }
            }
            continue;
        }
        for t in f + 1..n {
            if let Some(suzerain) = vassal_of(&states[ix(t)]) {
                states[ix(f)].diplomacy[t] = states[ix(f)].diplomacy[suzerain];
                continue;
            }
            let (sf, stt) = (&states[ix(f)], &states[ix(t)]);
            let naval = sf.kind == CultureType::Naval
                && stt.kind == CultureType::Naval
                && input.features.of_cell[sf.center] != input.features.of_cell[stt.center];
            let neib = !naval && sf.neighbors.contains(&(t as u32));
            let neib_of_neib = !naval
                && !neib
                && sf
                    .neighbors
                    .iter()
                    .any(|&k| k != NONE && states[ix(k as usize)].neighbors.contains(&(t as u32)));
            let mut status = if naval {
                draw(rng, &NAVALS)
            } else if neib {
                draw(rng, &NEIBS)
            } else if neib_of_neib {
                draw(rng, &NEIBS_OF_NEIBS)
            } else {
                draw(rng, &FAR)
            };
            // A strong neighbor makes a weak state its vassal.
            if neib
                && rng.p(0.8)
                && pops[f] > pop_mean
                && pops[t] < pop_mean
                && pops[f] / pops[t] > 2.0
            {
                status = Relation::Vassal;
            }
            states[ix(f)].diplomacy[t] = if status == Relation::Vassal {
                Relation::Suzerain
            } else {
                status
            };
            states[ix(t)].diplomacy[f] = status;
        }
    }

    // Declare wars.
    let power = |states: &[State], pops: &[f64], k: usize| pops[k] * states[ix(k)].expansionism;
    for attacker in 1..n {
        let ad = states[ix(attacker)].diplomacy.clone();
        if !ad.contains(&Relation::Rival)
            || ad.contains(&Relation::Vassal)
            || ad.contains(&Relation::Enemy)
        {
            continue;
        }
        // A random independent rival.
        let rivals: Vec<usize> = (1..n)
            .filter(|&d| {
                ad[d] == Relation::Rival && !states[ix(d)].diplomacy.contains(&Relation::Vassal)
            })
            .collect();
        if rivals.is_empty() {
            continue;
        }
        let defender = *rng.pick(&rivals);
        let mut ap = power(states, &pops, attacker);
        let mut dp = power(states, &pops, defender);
        if ap < dp * rng.gauss(1.6, 0.8, 0.0, 10.0, 2) {
            continue; // The defender is too strong.
        }
        let an = states[ix(attacker)].name.clone();
        let dn = states[ix(defender)].name.clone();
        let mut attackers = vec![attacker];
        let mut defenders = vec![defender];
        let name = format!("{an}-{}ian War", crate::names::trim_vowels(&dn, 3));
        let start = YEAR - rng.gauss(2.0, 3.0, 0.0, 10.0, 0);
        let mut war = vec![
            name.clone(),
            format!("{an} declared a war on its rival {dn}"),
        ];
        let campaign = Campaign {
            name,
            start,
            end: None,
            attacker: attacker as u32,
            defender: defender as u32,
        };
        states[ix(attacker)].campaigns.push(campaign.clone());
        states[ix(defender)].campaigns.push(campaign);

        // The vassals of the attacker and of the defender join the war.
        for (d, &r) in ad.iter().enumerate() {
            if r == Relation::Suzerain {
                attackers.push(d);
                war.push(format!(
                    "{an}'s vassal {} joined the war on attackers side",
                    states[ix(d)].name
                ));
            }
        }
        let dd = states[ix(defender)].diplomacy.clone();
        for (d, &r) in dd.iter().enumerate() {
            if r == Relation::Suzerain {
                defenders.push(d);
                war.push(format!(
                    "{dn}'s vassal {} joined the war on defenders side",
                    states[ix(d)].name
                ));
            }
        }
        ap = attackers.iter().map(|&a| power(states, &pops, a)).sum();
        dp = defenders.iter().map(|&d| power(states, &pops, d)).sum();

        // The allies of the defender join.
        for d in 0..n {
            if states[ix(defender)].diplomacy[d] != Relation::Ally
                || states[ix(d)].diplomacy.contains(&Relation::Vassal)
            {
                continue;
            }
            if states[ix(d)].diplomacy[attacker] != Relation::Rival
                && ap / dp > 2.0 * rng.gauss(1.6, 0.8, 0.0, 10.0, 2)
            {
                let reason = if states[ix(d)].diplomacy.contains(&Relation::Enemy) {
                    "Being already at war,".to_owned()
                } else {
                    format!("Frightened by {an},")
                };
                war.push(format!(
                    "{reason} {} severed the defense pact with {dn}",
                    states[ix(d)].name
                ));
                states[ix(defender)].diplomacy[d] = Relation::Suspicion;
                states[ix(d)].diplomacy[defender] = Relation::Suspicion;
                continue;
            }
            defenders.push(d);
            dp += power(states, &pops, d);
            war.push(format!(
                "{dn}'s ally {} joined the war on defenders side",
                states[ix(d)].name
            ));
            let vassals: Vec<usize> = (1..n)
                .filter(|&v| states[ix(d)].diplomacy[v] == Relation::Suzerain)
                .collect();
            for v in vassals {
                defenders.push(v);
                dp += power(states, &pops, v);
                war.push(format!(
                    "{}'s vassal {} joined the war on defenders side",
                    states[ix(d)].name,
                    states[ix(v)].name
                ));
            }
        }

        // The allies of the attacker join if the defender is their rival,
        // or if the joined power is larger than that of the defenders.
        for (d, &r) in ad.iter().enumerate() {
            if r != Relation::Ally
                || states[ix(d)].diplomacy.contains(&Relation::Vassal)
                || defenders.contains(&d)
            {
                continue;
            }
            let ally = states[ix(d)].name.clone();
            if states[ix(d)].diplomacy[defender] != Relation::Rival
                && (rng.p(0.2) || ap <= dp * 1.2)
            {
                war.push(format!("{an}'s ally {ally} avoided entering the war"));
                continue;
            }
            let both_sides = (1..n)
                .filter(|&a| states[ix(d)].diplomacy[a] == Relation::Ally)
                .any(|a| defenders.contains(&a));
            if both_sides {
                war.push(format!(
                    "{an}'s ally {ally} did not join the war as its allies are in war on both sides"
                ));
                continue;
            }
            attackers.push(d);
            ap += power(states, &pops, d);
            war.push(format!(
                "{an}'s ally {ally} joined the war on attackers side"
            ));
            let vassals: Vec<usize> = (1..n)
                .filter(|&v| states[ix(d)].diplomacy[v] == Relation::Suzerain)
                .collect();
            for v in vassals {
                attackers.push(v);
                ap += power(states, &pops, v);
                war.push(format!(
                    "{ally}'s vassal {} joined the war on attackers side",
                    states[ix(v)].name
                ));
            }
        }

        // All participants become enemies.
        for &a in &attackers {
            for &d in &defenders {
                states[ix(a)].diplomacy[d] = Relation::Enemy;
                states[ix(d)].diplomacy[a] = Relation::Enemy;
            }
        }
        chronicle.push(war);
    }
    chronicle
}

/// `true` if the state is a vassal of another state.
pub fn is_vassal(state: &State) -> bool {
    state.diplomacy.contains(&Relation::Vassal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    fn world(seed: u64, states: u32) -> World {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": seed, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "pangea", "states": states }
        }))
        .unwrap();
        World::build(&c).unwrap()
    }

    #[test]
    fn relations_are_symmetric_and_cover_each_state() {
        for seed in [3, 8, 21] {
            let w = world(seed, 9);
            let s = &w.society.states;
            for a in s {
                assert_eq!(a.diplomacy.len(), s.len() + 1);
                assert_eq!(a.diplomacy[0], Relation::None);
                assert_eq!(a.diplomacy[a.id as usize], Relation::None);
                for b in s.iter().filter(|b| b.id != a.id) {
                    let ab = a.diplomacy[b.id as usize];
                    let ba = b.diplomacy[a.id as usize];
                    assert_ne!(ab, Relation::None, "{} to {}", a.name, b.name);
                    // A vassal copies the relations of its suzerain, so
                    // only the vassal pair is strictly inverse.
                    if ab == Relation::Vassal {
                        assert_eq!(ba, Relation::Suzerain);
                    }
                    if ab == Relation::Enemy {
                        assert_eq!(ba, Relation::Enemy);
                    }
                }
            }
        }
    }

    #[test]
    fn a_war_is_in_the_chronicle_and_in_the_campaigns() {
        let mut found = false;
        for seed in 0..12 {
            let w = world(seed, 12);
            let soc = &w.society;
            for record in &soc.chronicle {
                found = true;
                assert!(record[0].ends_with("ian War"));
                let ongoing: Vec<&Campaign> = soc
                    .states
                    .iter()
                    .flat_map(|s| &s.campaigns)
                    .filter(|c| c.name == record[0])
                    .collect();
                assert_eq!(ongoing.len(), 2);
                assert!(ongoing.iter().all(|c| c.end.is_none()));
            }
            for s in &soc.states {
                assert!(!s.campaigns.is_empty());
            }
        }
        assert!(found, "no war in 12 worlds");
    }

    #[test]
    fn relation_names_round_trip() {
        for r in Relation::ALL {
            assert_eq!(Relation::from_name(r.name()), Some(r));
            assert_eq!(r.inverse().inverse(), r);
        }
    }
}
