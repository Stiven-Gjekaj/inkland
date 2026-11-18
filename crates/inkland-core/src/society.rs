//! Cultures, places, states, and provinces: the society stages of FMG in
//! its order.
//!
//! - `build`: rankCells (`population.rs`), cultures and their expansion
//!   (`cultures.rs`), burgs and ports (`burgs.rs`), states (`states.rs`),
//!   and development (`development.rs`).
//! - `specify`, after the routes and the religions: burgsSpecify, state
//!   statistics and forms, and provinces (`provinces.rs`).
//! - Pins win: pinned places stay exact and keep their kind, and a pinned
//!   capital leads the state of its `capital_of`. Each land cell gets
//!   exactly one state and one province.

use crate::config::{Config, PlaceKind};
use crate::features::Features;
use crate::grid::Grid;
use crate::names::Chain;
use crate::rand::Rand;
use crate::seed::{Layer, SeedTree};
use std::collections::BTreeSet;

/// No owner.
pub const NONE: u32 = 0;

pub use crate::cultures::Culture;

/// A place: a pinned place, a capital, or a town (a burg of FMG).
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// 1, 2, 3, ... Pinned places first, in config order.
    pub id: u32,
    /// The config id of a pinned place.
    pub pin: Option<String>,
    pub name: String,
    pub kind: PlaceKind,
    pub cell: usize,
    /// The position. A pinned place is exactly at its pin. A generated port
    /// moves to the edge of its cell, and a river place to a bank.
    pub at: [f64; 2],
    pub port: bool,
    /// The water feature of a port, or 0.
    pub water: u32,
    /// The land feature of the place.
    pub landmass: u32,
    pub capital: bool,
    pub culture: u32,
    pub state: u32,
    pub province: u32,
    /// Population in thousands (FMG population points).
    pub population: f64,
    /// The type of the place (`getType`).
    pub ptype: crate::cultures::CultureType,
    /// The FMG burg group: `capital`, `city`, `fort`, `monastery`,
    /// `caravanserai`, `trading_post`, `village`, `hamlet`, or `town`.
    pub group: String,
    pub citadel: bool,
    pub plaza: bool,
    pub walls: bool,
    pub shanty: bool,
    pub temple: bool,
    /// The coat of arms (`emblems.rs`).
    pub coa: Option<crate::emblems::Coa>,
}

impl Place {
    /// A place with no port, culture, state, or province.
    pub fn new(id: u32, name: String, kind: PlaceKind, cell: usize, at: [f64; 2]) -> Place {
        Place {
            id,
            pin: None,
            name,
            kind,
            cell,
            at,
            port: false,
            water: NONE,
            landmass: NONE,
            capital: false,
            culture: NONE,
            state: NONE,
            province: NONE,
            population: 0.0,
            ptype: crate::cultures::CultureType::Generic,
            group: String::new(),
            citadel: false,
            plaza: false,
            walls: false,
            shanty: false,
            temple: false,
            coa: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub id: u32,
    pub name: String,
    /// The place id of the capital.
    pub capital: u32,
    pub culture: u32,
    /// The cell of the capital.
    pub center: usize,
    /// The type of the culture of the state.
    pub kind: crate::cultures::CultureType,
    pub expansionism: f64,
    /// The neighbor states.
    pub neighbors: Vec<u32>,
    /// A color as `#rrggbb`. Data only; the print-safe SVG does not use it.
    pub color: String,
    /// The form of government: `Monarchy`, `Republic`, `Union`,
    /// `Theocracy`, or `Anarchy`.
    pub form: String,
    /// The form name, for example `Kingdom` or `Free City`.
    pub form_name: String,
    /// The full name, for example `Kingdom of Ostrey`.
    pub full_name: String,
    /// Statistics: land cells, area in square units, places, and the rural
    /// and urban population in population points.
    pub cells: u32,
    pub area: f64,
    pub burgs: u32,
    pub rural: f64,
    pub urban: f64,
    /// The coat of arms (`emblems.rs`).
    pub coa: Option<crate::emblems::Coa>,
}

impl Default for State {
    fn default() -> Self {
        State {
            id: NONE,
            name: String::new(),
            capital: NONE,
            culture: NONE,
            center: 0,
            kind: crate::cultures::CultureType::Generic,
            expansionism: 1.0,
            neighbors: Vec::new(),
            color: String::new(),
            form: String::new(),
            form_name: String::new(),
            full_name: String::new(),
            cells: 0,
            area: 0.0,
            burgs: 0,
            rural: 0.0,
            urban: 0.0,
            coa: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Province {
    pub id: u32,
    pub name: String,
    pub state: u32,
    /// The place id of the seat, or 0 for a wild province without a place.
    pub seat: u32,
    /// The center cell.
    pub center: usize,
    /// The form name, for example `County` or `Territory`.
    pub form_name: String,
    /// The full name, for example `Ostrey County`.
    pub full_name: String,
    /// A color as `#rrggbb`. Data only.
    pub color: String,
    /// `true` for a wild province: land that no seat reaches (an island, a
    /// colony, or a territory).
    pub wild: bool,
    /// The coat of arms (`emblems.rs`).
    pub coa: Option<crate::emblems::Coa>,
}

/// The society of a world.
#[derive(Debug, Clone, PartialEq)]
pub struct Society {
    /// The cell data of FMG, with suitability and rural population.
    pub cells: crate::population::Cells,
    /// The development of each cell, 0 to 100. 0: nobody lives there.
    pub development: Vec<u8>,
    pub cultures: Vec<Culture>,
    pub cell_culture: Vec<u32>,
    pub places: Vec<Place>,
    pub states: Vec<State>,
    pub cell_state: Vec<u32>,
    pub provinces: Vec<Province>,
    pub cell_province: Vec<u32>,
}

/// What the society stage reads from the world.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub heights: &'a [f64],
    pub sea_level: f64,
    pub features: &'a Features,
    pub biomes: &'a [u8],
    pub temperature: &'a [f64],
    pub drainage: &'a crate::rivers::Drainage,
    pub rivers: &'a [crate::rivers::River],
    pub river_threshold: f64,
    pub pin_cells: &'a [usize],
    /// The lakes, for the outlets of lake ports.
    pub lakes: &'a [crate::rivers::Lake],
    /// The group of each feature (`geonames::groups`).
    pub groups: &'a [&'static str],
}

/// Build the society.
pub fn build(input: &Input) -> Society {
    let Input {
        config,
        grid,
        heights,
        sea_level,
        features,
        biomes,
        temperature,
        drainage,
        rivers,
        river_threshold,
        pin_cells,
        lakes,
        groups,
    } = *input;
    let n = grid.len();
    let tree = SeedTree::new(config.seed);

    // Suitability and population (rankCells).
    let cells = crate::population::Cells::new(&crate::population::Input {
        grid,
        heights,
        sea_level,
        features,
        biomes,
        drainage,
        rivers,
        river_threshold,
        groups,
    });
    // Cultures (FMG generate and expand).
    let culture_input = crate::cultures::Input {
        config,
        grid,
        cells: &cells,
        features,
        biomes,
        temperature,
    };
    let mut crng = Rand::new(tree.part_rng(Layer::Cultures, "generate"));
    let cultures = crate::cultures::generate(&culture_input, &mut crng);
    let mut cell_culture = crate::cultures::expand(&culture_input, &cultures);
    crate::cultures::adopt(grid, &cells, &mut cell_culture, pin_cells);
    // Chain 0 is the wildlands.
    let chains: Vec<Chain> = std::iter::once(Chain::new(crate::cultures::WILDLANDS_BASE))
        .chain(cultures.iter().map(|c| Chain::new(c.base)))
        .collect();

    // Places (FMG burgs generate and assignPorts).
    let mut nrng = Rand::new(tree.part_rng(Layer::Names, "places"));
    let mut used_names: BTreeSet<String> = config
        .pins
        .points
        .iter()
        .filter_map(|p| p.name.clone())
        .collect();
    let burg_input = crate::burgs::Input {
        config,
        grid,
        cells: &cells,
        features,
        rivers,
        temperature,
        cell_culture: &cell_culture,
        pin_cells,
        chains: &chains,
        lakes,
    };
    let mut prng = Rand::new(tree.part_rng(Layer::Places, "generate"));
    let mut places = crate::burgs::generate(&burg_input, &mut used_names, &mut prng, &mut nrng);
    crate::burgs::assign_ports(&burg_input, &mut places);

    // States (FMG createStates, expandStates, normalize).
    let state_input = crate::states::Input {
        config,
        grid,
        cells: &cells,
        features,
        biomes,
        cultures: &cultures,
        cell_culture: &cell_culture,
        chains: &chains,
    };
    let mut srng = Rand::new(tree.part_rng(Layer::States, "generate"));
    let mut name_rng = Rand::new(tree.part_rng(Layer::Names, "states"));
    let mut states = crate::states::create(
        &state_input,
        &mut places,
        &mut used_names,
        &mut srng,
        &mut name_rng,
    );
    if states.is_empty() && !places.is_empty() {
        // inkland: no capital, so the first place leads one state.
        places[0].capital = true;
        states = crate::states::create(
            &state_input,
            &mut places[..1],
            &mut used_names,
            &mut srng,
            &mut name_rng,
        );
    }
    let cell_state = crate::states::expand(&state_input, &states, &places);
    crate::states::neighbors(grid, &cells, &cell_state, &mut states);
    crate::states::colors(&mut states, &mut srng);
    for p in &mut places {
        p.state = cell_state[p.cell];
    }

    // Provinces come later, in `specify`.
    let provinces: Vec<Province> = Vec::new();
    let cell_province = vec![NONE; n];

    let mut burg_cell = vec![false; n];
    for p in &places {
        burg_cell[p.cell] = true;
    }
    let development = crate::development::generate(
        grid,
        &cells,
        biomes,
        &cultures,
        &cell_culture,
        &burg_cell,
        config.seed,
    );

    Society {
        cells,
        development,
        cultures,
        cell_culture,
        places,
        states,
        cell_state,
        provinces,
        cell_province,
    }
}

/// What `specify` reads: the stages after the society stages.
pub struct Later<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub features: &'a Features,
    pub biomes: &'a [u8],
    pub roads: &'a [crate::roads::Road],
    pub sea_routes: &'a [crate::roads::Road],
    pub religions: &'a crate::religions::Religions,
}

/// The FMG stages after routes and religions: burgsSpecify,
/// collectStatistics, defineStateForms, and provinces.
pub fn specify(soc: &mut Society, later: &Later) {
    let Later {
        config,
        grid,
        features,
        biomes,
        roads,
        sea_routes,
        religions,
    } = *later;
    let tree = SeedTree::new(config.seed);
    let n = grid.len();
    let connectivity = crate::routes::connectivity(roads, sea_routes, n);
    let mut brng = Rand::new(tree.part_rng(Layer::Places, "specify"));
    crate::burgs::specify(
        &mut soc.places,
        &soc.cells,
        features,
        biomes,
        &connectivity,
        &mut brng,
    );
    crate::states::statistics(&soc.cells, &soc.cell_state, &soc.places, &mut soc.states);
    let mut frng = Rand::new(tree.part_rng(Layer::States, "forms"));
    crate::states::forms(
        &crate::states::FormInput {
            config,
            cultures: &soc.cultures,
            places: &soc.places,
            religions,
            development: &soc.development,
        },
        &mut soc.states,
        &mut frng,
    );
    let chains: Vec<Chain> = std::iter::once(Chain::new(crate::cultures::WILDLANDS_BASE))
        .chain(soc.cultures.iter().map(|c| Chain::new(c.base)))
        .collect();
    let mut prng = Rand::new(tree.part_rng(Layer::States, "provinces"));
    let (provinces, cell_province) = crate::provinces::generate(
        &crate::provinces::Input {
            config,
            grid,
            cells: &soc.cells,
            features,
            cell_state: &soc.cell_state,
            cell_culture: &soc.cell_culture,
            chains: &chains,
        },
        &soc.states,
        &soc.places,
        &mut prng,
    );
    soc.provinces = provinces;
    soc.cell_province = cell_province;
    for p in &mut soc.places {
        p.province = soc.cell_province[p.cell];
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::world::World;
    use proptest::prelude::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    const TEMPLATES: [&str; 5] = [
        "continents",
        "archipelago",
        "pangea",
        "high-island",
        "old-world",
    ];
    const SETS: [&str; 5] = [
        "european",
        "oriental",
        "antique",
        "high-fantasy",
        "dark-fantasy",
    ];

    fn config(seed: u64, t: usize, set: usize, places: &[(f64, f64)]) -> Config {
        let points: Vec<serde_json::Value> = places
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                let mut p = json!({ "id": format!("p{i}"), "kind": "town", "at": [x, y] });
                if i == 0 {
                    p["capital_of"] = json!("Pinned Realm");
                    p["name"] = json!("Pinholm");
                }
                p
            })
            .collect();
        serde_json::from_value(json!({
            "version": 1, "seed": seed,
            "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": TEMPLATES[t], "culture_set": SETS[set],
                       "cultures": 5, "states": 4, "places": { "min_distance_from_pins": 12 } },
            "pins": { "points": points }
        }))
        .unwrap()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        #[test]
        fn society_keeps_its_rules(
            seed in 0u64..1_000_000,
            t in 0usize..5,
            set in 0usize..5,
            places in proptest::collection::vec((10.0..390.0f64, 10.0..290.0f64), 0..5),
        ) {
            // Places at least 1 unit apart.
            let mut unique: Vec<(f64, f64)> = Vec::new();
            for p in places {
                if unique.iter().all(|q| ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt() >= 1.0) {
                    unique.push(p);
                }
            }
            let c = config(seed, t, set, &unique);
            let w = World::build(&c).map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
            let s = &w.society;
            let state_ids: BTreeSet<u32> = s.states.iter().map(|x| x.id).collect();
            // Each land cell is in exactly one state; water is in none.
            for cell in 0..w.grid.len() {
                if w.features.is_land(cell) {
                    prop_assert!(state_ids.contains(&s.cell_state[cell]), "land cell {cell} has no state");
                    prop_assert!(s.cell_province[cell] != 0, "land cell {cell} has no province");
                    let prov = &s.provinces[s.cell_province[cell] as usize - 1];
                    prop_assert_eq!(prov.state, s.cell_state[cell]);
                } else {
                    prop_assert_eq!(s.cell_state[cell], 0);
                }
            }
            // Places are on land, pinned places are exact, names are unique.
            let mut names = BTreeSet::new();
            for p in &s.places {
                prop_assert!(w.features.is_land(p.cell), "{} is not on land", p.name.clone());
                prop_assert!(names.insert(p.name.clone()), "{} twice", p.name.clone());
                // A generated port or river place moves inside its cell.
                prop_assert_eq!(w.grid.find_cell(p.at[0], p.at[1]), p.cell);
            }
            for (i, pin) in c.pins.points.iter().enumerate() {
                prop_assert_eq!(s.places[i].at, pin.at);
                prop_assert_eq!(s.places[i].pin.as_deref(), Some(pin.id.as_str()));
            }
            // Generated places keep their distance from each pin.
            for p in s.places.iter().filter(|p| p.pin.is_none()) {
                for pin in &c.pins.points {
                    let d = ((p.at[0] - pin.at[0]).powi(2) + (p.at[1] - pin.at[1]).powi(2)).sqrt();
                    prop_assert!(d >= 12.0, "{} is {d} from {}", p.name.clone(), pin.id.clone());
                }
            }
            // The pinned capital leads the pinned state.
            if !c.pins.points.is_empty() {
                let st = s.states.iter().find(|x| x.name == "Pinned Realm");
                prop_assert!(st.is_some());
                prop_assert_eq!(st.unwrap().capital, 1);
                prop_assert_eq!(&s.places[0].name, "Pinholm");
            }
        }
    }

    #[test]
    fn same_config_gives_the_same_society() {
        let c = config(5, 0, 0, &[(150.0, 120.0)]);
        assert_eq!(
            World::build(&c).unwrap().society,
            World::build(&c).unwrap().society
        );
    }
}
