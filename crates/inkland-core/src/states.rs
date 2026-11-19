//! States (`states-generator.ts`): `createStates`, `expandStates`,
//! `normalize`, `findNeighbors`, and `assignColors`.
//!
//! - Each capital founds a state. A pinned capital gives its state the name
//!   in `capital_of`. A later pin with the same `capital_of` is a seat of
//!   that state (inkland): the state grows from its capital and its seats,
//!   so it can hold land on two sides of a sea.
//! - States grow by least cost from their capitals with the costs of FMG:
//!   culture, population, biome, height, river, and coast, divided by the
//!   expansionism of the state. The growth stops at a total budget. A
//!   capital pin can give the expansionism (inkland); else it is random.
//! - inkland: a capital pin can give a soft claim, a polygon. The state
//!   grows into it first; the other states pay more for its cells.
//! - `normalize` gives a lone cell to the state around it.
//! - inkland: each land cell must be in a state. A land cell that no state
//!   reaches takes the state of the nearest cell with a state.

use crate::cultures::{BIOME_COST, Culture, CultureType, Key, type_cost};
use crate::economy::finance::{Budget, Tariff, TariffRule};
use crate::features::{FeatureKind, Features};
use crate::grid::Grid;
use crate::names::Chain;
use crate::population::Cells;
use crate::rand::{Rand, js_round, rn};
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

/// The extra cost of a cell in the claim of another state (inkland). It is
/// the cost of a sea cell for most states.
const CLAIM_COST: f64 = 1000.0;

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
        // inkland: the random value is always drawn, so that a given
        // expansionism does not change the other states.
        let random = rn(rng.unit() * config.world.size_variety + 1.0, 1);
        let given = pinned.as_ref().and_then(|s| {
            config
                .pins
                .points
                .iter()
                .filter(|q| q.capital_of.as_ref() == Some(s))
                .find_map(|q| q.expansionism)
        });
        let expansionism = given.unwrap_or(random);
        let chain = &input.chains[p.culture as usize];
        let name = match pinned {
            Some(s) if !states.iter().any(|x| x.name == s) => s,
            Some(s) => {
                // A second pin with the same state name is not a capital. It
                // is a seat of the state.
                p.capital = false;
                if let Some(st) = states.iter_mut().find(|x| x.name == s) {
                    st.seats.push(p.id);
                }
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
    let seat_cell = |id: u32| places.iter().find(|p| p.id == id).map(|p| p.cell);
    for s in states {
        let native = cultures
            .get((s.culture as usize).wrapping_sub(1))
            .map_or(biomes[s.center], |c| biomes[c.center]);
        let seats = s.seats.iter().filter_map(|&id| seat_cell(id));
        for c in std::iter::once(s.center).chain(seats) {
            state[c] = s.id;
            is_center[c] = true;
            heap.push(Reverse((Key(0.0, seq), c, s.id, native)));
            seq += 1;
            cost[c] = 1.0;
        }
    }
    // inkland: the soft claims. A cell in the claim of its own state costs
    // nothing more than the step; a cell in the claim of another state
    // costs `CLAIM_COST` more. The first state with a claim on a cell wins.
    let claims: Vec<(u32, crate::geom::Shape)> = states
        .iter()
        .flat_map(|s| {
            config
                .pins
                .points
                .iter()
                .filter(move |q| q.capital_of.as_ref() == Some(&s.name))
                .filter_map(move |q| {
                    let poly = q.claim.as_ref().filter(|p| p.len() >= 3)?;
                    Some((s.id, crate::geom::Shape::Polygon(poly.clone())))
                })
        })
        .collect();
    let claim_of: Vec<u32> = if claims.is_empty() {
        Vec::new()
    } else {
        (0..n)
            .map(|c| {
                claims
                    .iter()
                    .find(|(_, shape)| shape.contains(grid.points[c]))
                    .map_or(NONE, |(id, _)| *id)
            })
            .collect()
    };
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
            let cell_cost = match claim_of.get(m) {
                Some(&c) if c == s => 0.0,
                Some(&c) if c != NONE => cell_cost + CLAIM_COST,
                _ => cell_cost,
            };
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
    let mut capital_cell = is_center.clone();
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
    pub cells: &'a Cells,
    pub cell_state: &'a [u32],
    pub cell_culture: &'a [u32],
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

/// The form, form name, and full name of each state, and its taxes, market
/// fee, tariffs, war alert, and budget (`defineStateForms`). FMG has a
/// "Neutrals" state 0 with the area of the land without a state; in inkland
/// that area is 0.
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
        // The tax rates draw last, as in FMG; the fee follows the sales tax.
        let (sales_tax, poll_tax) = tax_rates(s, rng);
        s.economy.sales_tax = sales_tax;
        s.economy.poll_tax = poll_tax;
        s.economy.market_fee = market_fee(s);
    }
    // These read the other states but draw no random numbers.
    let alerts: Vec<f64> = states.iter().map(|s| alert(s, states)).collect();
    let tariffs: Vec<Vec<TariffRule>> = states.iter().map(|s| define_tariffs(s, states)).collect();
    for (k, (a, t)) in alerts.into_iter().zip(tariffs).enumerate() {
        states[k].alert = a;
        states[k].economy.tariffs = t;
    }
    let budgets: Vec<Budget> = states
        .iter()
        .map(|s| Budget {
            expenses: expenses(s, capital_population(input, s), states),
            corruption: corruption(s, diversity(input, s), states),
        })
        .collect();
    for (s, b) in states.iter_mut().zip(budgets) {
        s.economy.budget = b;
    }
}

/// The base tax rates and market fee of each form (`DEFAULT_TAX_BY_FORM`):
/// sales tax, poll tax, and fee.
fn tax_bases(form: &str) -> (f64, f64, f64) {
    match form {
        "Theocracy" => (0.05, 0.07, 50.0),
        "Union" => (0.1, 0.0, 0.0),
        "Republic" => (0.05, 0.05, 15.0),
        "Anarchy" => (0.0, 0.0, 0.0),
        _ => (0.08, 0.07, 30.0),
    }
}

/// The most that the market fee of a state lies above the base of its form
/// (`MAX_MARKET_FEE_FACTOR`).
const MAX_MARKET_FEE_FACTOR: f64 = 1.5;
/// The corruption of each form (`CORRUPTION_BY_FORM`).
fn form_corruption(form: &str) -> f64 {
    match form {
        "Theocracy" => 12.0,
        "Republic" => 8.0,
        "Union" => 4.0,
        "Anarchy" => 0.0,
        _ => 15.0,
    }
}
/// The form names that double the corruption of their form.
const DESPOTIC_FORMS: [&str; 5] = ["Despotate", "Horde", "Junta", "Satrapy", "Shogunate"];
/// What the size of the largest state adds to corruption.
const SIZE_CORRUPTION: f64 = 15.0;
/// What a state of strangers only adds to corruption, half for culture and
/// half for faith.
const DIVERSITY_CORRUPTION: f64 = 10.0;
/// The base percents of court, administration, and infrastructure of each
/// form (`EXPENSES_BY_FORM`).
fn form_expenses(form: &str) -> [f64; 3] {
    match form {
        "Theocracy" => [30.0, 10.0, 8.0],
        "Republic" | "Union" => [5.0, 20.0, 12.0],
        "Anarchy" => [0.0, 0.0, 0.0],
        _ => [20.0, 10.0, 8.0],
    }
}
/// What ruling the most places adds to administration.
const SIZE_ADMINISTRATION: f64 = 15.0;
/// What an all-urban state adds to infrastructure.
const URBAN_INFRASTRUCTURE: f64 = 20.0;
/// The base of the military expense.
const MILITARY_SPENDING: f64 = 25.0;
/// What war alert 1 adds to the military expense, by its root.
const ALERT_MILITARY: f64 = 20.0;

/// `true` if the state levies taxes (`levies`): an anarchy does not.
pub fn levies(s: &State) -> bool {
    s.id != NONE && s.form != "Anarchy"
}

/// The sales and poll tax rates (`defineTaxRates`).
fn tax_rates(s: &State, rng: &mut Rand) -> (f64, f64) {
    let (sales, poll, _) = tax_bases(&s.form);
    let sales_tax = rn(
        rng.gauss(sales, sales * 0.5, sales * 0.25, sales * 2.0, 4),
        2,
    );
    let poll_tax = rn(rng.gauss(poll, poll * 0.5, poll * 0.25, poll * 2.0, 4), 2);
    (sales_tax, poll_tax)
}

/// The fee of the form, moved as the sales tax is (`defineMarketFee`).
pub fn market_fee(s: &State) -> f64 {
    let (sales, _, fee) = tax_bases(&s.form);
    if s.id == NONE || sales == 0.0 || fee == 0.0 {
        return 0.0;
    }
    js_round(fee * (s.economy.sales_tax / sales).clamp(0.25, MAX_MARKET_FEE_FACTOR))
}

/// The import rules of a state from its relations (`defineTariffs`): 5% on
/// all, none for allies, friends, vassals, and suzerains, 20% for
/// suspicion, 30% for rivals, and an embargo for enemies.
pub fn define_tariffs(s: &State, states: &[State]) -> Vec<TariffRule> {
    use crate::diplomacy::Relation;
    if !levies(s) {
        return Vec::new();
    }
    let mut rules = vec![TariffRule {
        states: None,
        goods: None,
        rate: Tariff::Rate(0.05),
    }];
    let (mut free, mut suspicion, mut rival, mut enemy) = (vec![], vec![], vec![], vec![]);
    for (other, &relation) in s.diplomacy.iter().enumerate().skip(1) {
        let other = other as u32;
        if other == s.id || other as usize > states.len() {
            continue;
        }
        match relation {
            Relation::Ally | Relation::Friendly | Relation::Vassal | Relation::Suzerain => {
                free.push(other)
            }
            Relation::Suspicion => suspicion.push(other),
            Relation::Rival => rival.push(other),
            Relation::Enemy => enemy.push(other),
            _ => {}
        }
    }
    for (list, rate) in [
        (free, Tariff::Rate(0.0)),
        (suspicion, Tariff::Rate(0.2)),
        (rival, Tariff::Rate(0.3)),
        (enemy, Tariff::Embargo),
    ] {
        if !list.is_empty() {
            rules.push(TariffRule {
                states: Some(list),
                goods: None,
                rate,
            });
        }
    }
    rules
}

/// How ready for war the state is, from 0.1 to 5 (`defineAlert`).
pub fn alert(s: &State, states: &[State]) -> f64 {
    use crate::diplomacy::Relation;
    let expansionism = states.iter().fold(0.0, |t, x| t + x.expansionism);
    let area = states.iter().fold(0.0, |t, x| t + x.area);
    let rate = |r: Relation| match r {
        Relation::Ally => -0.2,
        Relation::Friendly => -0.1,
        Relation::Suspicion => 0.1,
        Relation::Enemy => 1.0,
        Relation::Rival => 0.5,
        Relation::Vassal => 0.5,
        Relation::Suzerain => -0.5,
        _ => 0.0,
    };
    let expansion_rate = (s.expansionism / expansionism / (s.area / area)).clamp(0.25, 4.0);
    let has = |r: Relation| s.diplomacy.contains(&r);
    let diplomacy_rate = if has(Relation::Enemy) {
        1.0
    } else if has(Relation::Rival) {
        0.8
    } else if has(Relation::Suspicion) {
        0.5
    } else {
        0.1
    };
    let neighbors_rate = s.neighbors.iter().fold(0.5f64, |t, &n| {
        let relation = if n == NONE {
            Relation::Suspicion
        } else {
            states
                .get(n as usize - 1)
                .and_then(|o| o.diplomacy.get(s.id as usize).copied())
                .unwrap_or(Relation::None)
        };
        t + rate(relation)
    });
    rn(
        expansion_rate * diplomacy_rate * neighbors_rate.clamp(0.3, 3.0),
        2,
    )
    .clamp(0.1, 5.0)
}

/// The population of the capital of a state.
fn capital_population(input: &FormInput, s: &State) -> f64 {
    input
        .places
        .get((s.capital as usize).wrapping_sub(1))
        .map_or(0.0, |p| p.population)
}

/// The four expenses in whole percents of the taxes (`defineExpenses`).
/// `capital` is the population of the capital.
pub fn expenses(s: &State, capital: f64, states: &[State]) -> [f64; 4] {
    if !levies(s) {
        return [0.0; 4];
    }
    let [court, administration, infrastructure] = form_expenses(&s.form);
    let urban = s.urban;
    let people = urban + s.rural;
    let primacy = if urban != 0.0 { capital / urban } else { 1.0 };
    let most = states.iter().fold(1.0f64, |m, x| m.max(f64::from(x.burgs)));
    [
        js_round(court * (0.5 + primacy.min(1.0))),
        js_round(administration + SIZE_ADMINISTRATION * libm::sqrt(f64::from(s.burgs) / most)),
        js_round(
            infrastructure
                + URBAN_INFRASTRUCTURE * if people != 0.0 { urban / people } else { 0.0 },
        ),
        js_round(MILITARY_SPENDING + ALERT_MILITARY * libm::sqrt(s.alert)).min(100.0),
    ]
}

/// The percent of each expense that officials pocket (`defineCorruption`).
/// `diversity` is the share of strangers (`diversity`).
pub fn corruption(s: &State, diversity: f64, states: &[State]) -> f64 {
    if !levies(s) {
        return 0.0;
    }
    let despotic = if DESPOTIC_FORMS.contains(&s.form_name.as_str()) {
        2.0
    } else {
        1.0
    };
    let form = form_corruption(&s.form) * despotic;
    js_round(form + SIZE_CORRUPTION * size(s, states) + DIVERSITY_CORRUPTION * diversity)
}

/// The size against the largest state by the root of cells, 0 to 1; 0 for
/// a lone city-state (`getSize`).
pub fn size(s: &State, states: &[State]) -> f64 {
    let largest = states.iter().fold(0.0f64, |m, x| m.max(f64::from(x.cells)));
    if s.burgs > 1 && largest != 0.0 {
        libm::sqrt(f64::from(s.cells) / largest)
    } else {
        0.0
    }
}

/// The share of people outside the culture of the state and the faith of
/// its capital, each counting half: 0 to 1 (`getDiversity`).
pub fn diversity(input: &FormInput, s: &State) -> f64 {
    let religion_of = |c: usize| {
        input
            .religions
            .cell_religion
            .get(c)
            .copied()
            .unwrap_or(NONE)
    };
    let faith = input
        .places
        .get((s.capital as usize).wrapping_sub(1))
        .map_or(NONE, |p| religion_of(p.cell));
    let mut burg_pop = vec![0.0; input.cell_state.len()];
    for p in input.places {
        burg_pop[p.cell] = p.population;
    }
    let (mut people, mut strangers) = (0.0, 0.0);
    for c in 0..input.cell_state.len() {
        if input.cell_state[c] != s.id {
            continue;
        }
        let weight = input.cells.pop[c] + burg_pop[c];
        people += weight;
        if input.cell_culture[c] != s.culture {
            strangers += weight / 2.0;
        }
        if religion_of(c) != faith {
            strangers += weight / 2.0;
        }
    }
    if people != 0.0 {
        strangers / people
    } else {
        0.0
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
            // Vassal forms: some vassal duchies on the borderland, English
            // vassals, and some other vassals.
            let vassal = crate::diplomacy::is_vassal(s);
            if form == "Duchy"
                && s.neighbors.len() > 1
                && (rng.rand(0, 6) as usize) < s.neighbors.len()
                && vassal
            {
                return name("Marches");
            }
            if base == 1 && rng.p(0.3) && vassal {
                return name("Dominion");
            }
            if rng.p(0.3) && vassal {
                return name("Protectorate");
            }
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

    #[test]
    fn a_second_pin_with_the_same_state_is_a_seat_that_the_state_grows_from() {
        let config = |second: &str| -> Config {
            serde_json::from_value(json!({
                "version": 1, "seed": 8, "frame": { "width": 400, "height": 300 },
                "world": { "cells": 4000, "template": "pangea", "states": 4,
                           "growth_rate": 0.3 },
                "pins": { "points": [
                    { "id": "home", "kind": "city", "at": [110, 150], "capital_of": "Realm" },
                    { "id": "far", "kind": "city", "at": [300, 140], "capital_of": second }
                ] }
            }))
            .unwrap()
        };
        let w = World::build(&config("Realm")).unwrap();
        let s = &w.society;
        let realm: Vec<_> = s.states.iter().filter(|x| x.name == "Realm").collect();
        assert_eq!(realm.len(), 1, "one state for the two pins");
        let realm = realm[0];
        let home = s
            .places
            .iter()
            .find(|p| p.pin.as_deref() == Some("home"))
            .unwrap();
        let far = s
            .places
            .iter()
            .find(|p| p.pin.as_deref() == Some("far"))
            .unwrap();
        assert_eq!(realm.capital, home.id, "the first pin is the capital");
        assert_eq!(realm.seats, vec![far.id]);
        assert!(home.capital && !far.capital);
        assert_eq!(s.cell_state[far.cell], realm.id);
        assert_eq!(far.state, realm.id);
        // The state grows from the seat: the land around it is in the state.
        for c in w.grid.neighbors(far.cell) {
            if s.cells.land(c) {
                assert_eq!(s.cell_state[c], realm.id, "cell {c}");
            }
        }
        // A seat does not take the place of a generated capital.
        assert_eq!(s.states.len(), 4);
        let two = World::build(&config("Other")).unwrap();
        assert_eq!(two.society.states.len(), 4);
        // The same config gives the same states.
        assert_eq!(
            World::build(&config("Realm")).unwrap().society.cell_state,
            s.cell_state
        );
    }

    #[test]
    fn a_capital_pin_gives_the_expansionism_of_its_state() {
        let config = |a: Option<f64>, b: Option<f64>| -> Config {
            let mut c: Config = serde_json::from_value(json!({
                "version": 1, "seed": 8, "frame": { "width": 400, "height": 300 },
                "world": { "cells": 4000, "template": "pangea", "states": 4 },
                "pins": { "points": [
                    { "id": "west", "kind": "city", "at": [140, 150], "capital_of": "West" },
                    { "id": "east", "kind": "city", "at": [260, 150], "capital_of": "East" }
                ] }
            }))
            .unwrap();
            c.pins.points[0].expansionism = a;
            c.pins.points[1].expansionism = b;
            c
        };
        let size = |w: &World, name: &str| {
            let id = w.society.states.iter().find(|s| s.name == name).unwrap().id;
            w.society.cell_state.iter().filter(|&&s| s == id).count()
        };
        let base = World::build(&config(None, None)).unwrap();
        let big_west = World::build(&config(Some(6.0), Some(0.5))).unwrap();
        let big_east = World::build(&config(Some(0.5), Some(6.0))).unwrap();
        let exp =
            |w: &World| -> Vec<f64> { w.society.states.iter().map(|s| s.expansionism).collect() };
        assert_eq!(&exp(&big_west)[..2], &[6.0, 0.5]);
        // The generated states keep their random expansionism.
        assert_eq!(&exp(&big_west)[2..], &exp(&base)[2..]);
        assert!(size(&big_west, "West") > size(&big_west, "East"));
        assert!(size(&big_east, "East") > size(&big_east, "West"));
        assert!(size(&big_west, "West") > size(&big_east, "West"));
    }

    #[test]
    fn a_state_grows_into_its_claim_first() {
        let config = |claim: Option<serde_json::Value>| -> Config {
            let mut east = json!({ "id": "east", "kind": "city", "at": [260, 150],
                                   "capital_of": "East" });
            if let Some(c) = claim {
                east["claim"] = c;
            }
            serde_json::from_value(json!({
                "version": 1, "seed": 8, "frame": { "width": 400, "height": 300 },
                "world": { "cells": 4000, "template": "pangea", "states": 4 },
                "pins": { "points": [
                    { "id": "west", "kind": "city", "at": [140, 150], "capital_of": "West" },
                    east
                ] }
            }))
            .unwrap()
        };
        // A claim of East, around its capital, that reaches near the
        // capital of West.
        let poly = [
            [165.0, 110.0],
            [275.0, 110.0],
            [275.0, 190.0],
            [165.0, 190.0],
        ];
        let shape = crate::geom::Shape::Polygon(poly.to_vec());
        let share = |w: &World| {
            let s = &w.society;
            let east = s.states.iter().find(|x| x.name == "East").unwrap().id;
            let cells: Vec<usize> = (0..w.grid.len())
                .filter(|&c| s.cells.land(c) && shape.contains(w.grid.points[c]))
                .collect();
            assert!(cells.len() > 20);
            let held = cells.iter().filter(|&&c| s.cell_state[c] == east).count();
            held as f64 / cells.len() as f64
        };
        let without = share(&World::build(&config(None)).unwrap());
        let claimed = World::build(&config(Some(json!(poly)))).unwrap();
        let with = share(&claimed);
        assert!(with > 0.9, "{with} {without}");
        assert!(with > without, "{with} {without}");
        assert_eq!(
            World::build(&config(Some(json!(poly))))
                .unwrap()
                .society
                .cell_state,
            claimed.society.cell_state
        );
    }

    fn state(id: u32, form: &str) -> crate::society::State {
        crate::society::State {
            id,
            form: form.to_owned(),
            ..crate::society::State::default()
        }
    }

    #[test]
    fn the_fee_follows_the_sales_tax_of_the_form() {
        // Twice the base sales tax of a monarchy: its base fee x 1.5 at
        // most; a tenth of the base of a republic: a quarter at least.
        let mut a = state(1, "Monarchy");
        a.economy.sales_tax = 0.2;
        let mut b = state(2, "Republic");
        b.economy.sales_tax = 0.005;
        let c = state(3, "Anarchy");
        assert_eq!(super::market_fee(&a), 45.0);
        assert_eq!(super::market_fee(&b), 4.0);
        assert_eq!(super::market_fee(&c), 0.0);
    }

    #[test]
    fn a_budget_comes_from_the_form_the_state_and_its_war_alert() {
        let mut a = state(1, "Monarchy");
        a.burgs = 4;
        a.urban = 20.0;
        a.rural = 80.0;
        a.alert = 0.25;
        a.cells = 100;
        let mut b = a.clone();
        b.id = 2;
        b.alert = 100.0;
        let states = vec![a.clone(), b.clone()];
        // Court 20 x (0.5 + half of the townspeople in the capital),
        // administration 10 + 15, infrastructure 8 + 20 x 0.2, military
        // 25 + 20 x 0.5.
        assert_eq!(super::expenses(&a, 10.0, &states), [20.0, 25.0, 12.0, 35.0]);
        // At war alert 100 the military takes all the taxes.
        assert_eq!(super::expenses(&b, 10.0, &states)[3], 100.0);
        assert_eq!(
            super::expenses(&state(3, "Anarchy"), 10.0, &states),
            [0.0; 4]
        );
        // Corruption: the form, the size (as large as the largest), and the
        // strangers.
        assert_eq!(super::corruption(&a, 0.0, &states), 30.0);
        assert_eq!(super::corruption(&a, 0.5, &states), 35.0);
    }

    #[test]
    fn tariffs_follow_relations() {
        use crate::diplomacy::Relation;
        use crate::economy::finance::Tariff;
        let mut a = state(1, "Monarchy");
        a.diplomacy = vec![
            Relation::None,
            Relation::None,
            Relation::Ally,
            Relation::Rival,
            Relation::Enemy,
            Relation::Neutral,
        ];
        let states: Vec<_> = (1..=5).map(|k| state(k, "Monarchy")).collect();
        let rules = super::define_tariffs(&a, &states);
        assert_eq!(rules.len(), 4);
        assert_eq!(rules[0].states, None);
        assert_eq!(rules[0].rate, Tariff::Rate(0.05));
        assert_eq!(rules[1].states, Some(vec![2]));
        assert_eq!(rules[1].rate, Tariff::Rate(0.0));
        assert_eq!(rules[2].states, Some(vec![3]));
        assert_eq!(rules[3].rate, Tariff::Embargo);
        assert!(super::define_tariffs(&state(1, "Anarchy"), &states).is_empty());
    }

    #[test]
    fn each_state_gets_its_taxes_alert_and_budget() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 3, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "pangea", "states": 8 }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        for st in &w.society.states {
            let e = &st.economy;
            assert!((0.1..=5.0).contains(&st.alert), "{}", st.alert);
            if super::levies(st) {
                assert!(e.sales_tax > 0.0 && !e.tariffs.is_empty());
                assert!(e.budget.of(crate::economy::finance::Expense::Military) >= 25.0);
                assert!(e.budget.corruption > 0.0);
            } else {
                assert_eq!(e.budget.total(), 0.0);
            }
        }
    }

    #[test]
    fn only_vassals_take_the_vassal_forms() {
        let mut seen = 0;
        for seed in 0..16 {
            let c: Config = serde_json::from_value(json!({
                "version": 1, "seed": seed, "frame": { "width": 400, "height": 300 },
                "world": { "cells": 4000, "template": "pangea", "states": 12 }
            }))
            .unwrap();
            let w = World::build(&c).unwrap();
            for st in &w.society.states {
                if ["Marches", "Dominion", "Protectorate"].contains(&st.form_name.as_str()) {
                    assert!(
                        crate::diplomacy::is_vassal(st),
                        "{} is no vassal",
                        st.full_name
                    );
                    assert_eq!(st.form, "Monarchy");
                    seen += 1;
                }
            }
        }
        assert!(seen > 0, "no vassal form in 16 worlds");
    }
}
