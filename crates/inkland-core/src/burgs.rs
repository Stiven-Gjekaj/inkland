//! Places (burgs, `burgs-generator.ts`): `generate` and `assignPorts`.
//!
//! - Pinned places come first, exactly where they are, with their kind.
//! - Capitals: the populated cells with a culture, by suitability with a
//!   random part. A capital keeps a spacing of `(width + height) / 2 /
//!   capitals` from the other capitals and the pinned places. When no cell
//!   fits, the spacing becomes smaller.
//! - Towns: the cells by suitability with a normal random factor. A town
//!   keeps a random spacing from the other places. Each round halves the
//!   spacing.
//! - Generated places keep `places.min_distance_from_pins` from each pinned
//!   place (inkland).
//! - Ports: the coastal places and the places on navigable rivers, grouped by
//!   water body. A water body needs two ports or more. A coastal port moves
//!   to the edge of its haven cell; a river place moves to a river bank.

use crate::config::{Config, PlaceKind};
use crate::features::{FeatureKind, Features};
use crate::grid::Grid;
use crate::names::Chain;
use crate::population::Cells;
use crate::rand::Rand;
use crate::rivers::{Mouth, River};
use crate::society::{NONE, Place};
use std::collections::{BTreeMap, BTreeSet};

/// What the place stage reads.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub cells: &'a Cells,
    pub features: &'a Features,
    pub rivers: &'a [River],
    pub temperature: &'a [f64],
    pub cell_culture: &'a [u32],
    pub pin_cells: &'a [usize],
    /// Chain 0 is the wildlands; chain `k` is culture `k`.
    pub chains: &'a [Chain],
    pub lakes: &'a [crate::rivers::Lake],
    /// The number of cells of the first grid (FMG `grid.points.length`).
    pub base_cells: usize,
}

fn dist2(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])
}

/// `true` if a point of `points` is nearer to `p` than `r` (`quadtree.find`).
fn near(points: &[[f64; 2]], p: [f64; 2], r: f64) -> bool {
    points.iter().any(|&q| dist2(p, q) < r * r)
}

/// Sort cells by a score, best first; equal scores keep their order.
fn by_score(cells: &[usize], score: impl Fn(usize) -> f64) -> Vec<usize> {
    let keys: Vec<f64> = cells.iter().map(|&c| score(c)).collect();
    let mut order: Vec<usize> = (0..cells.len()).collect();
    order.sort_by(|&a, &b| keys[b].total_cmp(&keys[a]));
    order.into_iter().map(|k| cells[k]).collect()
}

/// The smallest distance from a generated place to a pinned place.
pub fn pin_gap(config: &Config, grid: &Grid) -> f64 {
    let mean_width = (grid.step_x + grid.step_y) / 2.0;
    config
        .world
        .places
        .min_distance_from_pins
        .unwrap_or(3.0 * mean_width)
}

/// The automatic town count of FMG (`getTownsNumber`). FMG divides by the
/// cells of the first grid, not by the cells of the second grid.
pub fn auto_towns(populated: usize, base_cells: usize) -> usize {
    let v = populated as f64 / 5.0 / libm::pow(base_cells.max(1) as f64 / 10_000.0, 0.8);
    crate::rand::js_round(v) as usize
}

/// Place the pinned places, the capitals, and the towns (`generate`). The
/// result has the pinned places first, in config order, then the
/// capitals, then the towns.
pub fn generate(
    input: &Input,
    used: &mut BTreeSet<String>,
    rng: &mut Rand,
    nrng: &mut Rand,
) -> Vec<Place> {
    let Input {
        config,
        grid,
        cells,
        features,
        cell_culture,
        pin_cells,
        chains,
        ..
    } = *input;
    let n = grid.len();
    let (w, h) = (config.frame.width, config.frame.height);
    let mut places: Vec<Place> = Vec::new();
    let mut burg_of = vec![NONE; n];

    for (i, pin) in config.pins.points.iter().enumerate() {
        let cell = pin_cells[i];
        let name = match &pin.name {
            Some(n) => n.clone(),
            None => chains[cell_culture[cell] as usize].name(nrng, used),
        };
        places.push(Place {
            pin: Some(pin.id.clone()),
            port: pin.port,
            capital: pin.capital_of.is_some(),
            ..Place::new(places.len() as u32 + 1, name, pin.kind, cell, pin.at)
        });
        burg_of[cell] = places.len() as u32;
    }
    let pin_points: Vec<[f64; 2]> = config.pins.points.iter().map(|p| p.at).collect();
    let pin_gap = pin_gap(config, grid);
    let free = |c: usize| burg_of[c] == NONE && !near(&pin_points, grid.points[c], pin_gap);
    let populated: Vec<usize> = (0..n)
        .filter(|&i| cells.s[i] > 0.0 && cell_culture[i] != 0 && free(i))
        .collect();

    // Capitals.
    let mut state_names: Vec<&str> = Vec::new();
    for pin in &config.pins.points {
        if let Some(s) = &pin.capital_of
            && !state_names.contains(&s.as_str())
        {
            state_names.push(s);
        }
    }
    let pinned_capitals = places.iter().filter(|p| p.capital).count();
    let mut number = (config.world.states as usize).max(state_names.len());
    if populated.len() < number * 10 {
        number = (populated.len() / 10).max(state_names.len());
    }
    let more = number.saturating_sub(pinned_capitals);
    let score: Vec<f64> = cells
        .s
        .iter()
        .map(|&s| (s * (0.5 + rng.unit() * 0.5)).trunc())
        .collect();
    let sorted = by_score(&populated, |c| score[c]);
    let base_points: Vec<[f64; 2]> = places.iter().map(|p| p.at).collect();
    let mut spacing = (w + h) / 2.0 / number.max(1) as f64;
    let mut capitals: Vec<usize> = Vec::new();
    if more > 0 && !sorted.is_empty() {
        let mut points = base_points.clone();
        let mut i = 0;
        let mut resets = 0;
        while capitals.len() < more {
            let cell = sorted[i];
            let p = grid.points[cell];
            if !near(&points, p, spacing) {
                capitals.push(cell);
                points.push(p);
            }
            if i == sorted.len() - 1 && capitals.len() < more {
                resets += 1;
                if resets > 200 {
                    break;
                }
                points = base_points.clone();
                capitals.clear();
                spacing /= 1.2;
                i = 0;
                continue;
            }
            i += 1;
        }
    }
    let mut quad: Vec<[f64; 2]> = base_points;
    for &cell in &capitals {
        let culture = cell_culture[cell];
        let name = chains[culture as usize].short(nrng, used);
        places.push(Place {
            capital: true,
            ..Place::new(
                places.len() as u32 + 1,
                name,
                PlaceKind::City,
                cell,
                grid.points[cell],
            )
        });
        burg_of[cell] = places.len() as u32;
        quad.push(grid.points[cell]);
    }

    // Towns.
    let number = match config.world.towns {
        crate::config::Towns::Count(k) => (k as usize).min(populated.len()),
        crate::config::Towns::Auto(_) => auto_towns(populated.len(), input.base_cells),
    };
    let score: Vec<f64> = cells
        .s
        .iter()
        .map(|&s| (s * rng.gauss(1.0, 3.0, 0.0, 20.0, 3)).trunc())
        .collect();
    let sorted = by_score(&populated, |c| score[c]);
    let mut spacing = (w + h) / 150.0 / (libm::pow(number.max(1) as f64, 0.7) / 66.0);
    let mut added = 0;
    while added < number && spacing * cells.px > 1.0 {
        for &cell in &sorted {
            if added >= number {
                break;
            }
            if burg_of[cell] != NONE {
                continue;
            }
            let p = grid.points[cell];
            let min_spacing = spacing * rng.gauss(1.0, 0.3, 0.2, 2.0, 2);
            if near(&quad, p, min_spacing) {
                continue;
            }
            let name = chains[cell_culture[cell] as usize].name(nrng, used);
            places.push(Place::new(
                places.len() as u32 + 1,
                name,
                PlaceKind::Town,
                cell,
                p,
            ));
            burg_of[cell] = places.len() as u32;
            // The FMG commit of the port does not add towns to the spacing
            // tree. Older FMG versions do; inkland does, so that towns do
            // not form clusters.
            quad.push(p);
            added += 1;
        }
        spacing *= 0.5;
    }

    for p in &mut places {
        p.culture = cell_culture[p.cell];
        p.landmass = features.of_cell[p.cell];
    }
    places
}

/// The type of a place (`getType`).
pub fn place_type(
    cells: &Cells,
    features: &Features,
    biomes: &[u8],
    cell: usize,
    port: bool,
) -> crate::cultures::CultureType {
    use crate::cultures::CultureType;
    if port {
        return CultureType::Naval;
    }
    if let Some(h) = cells.haven[cell]
        && features.get(features.of_cell[h]).kind == FeatureKind::Lake
    {
        return CultureType::Lake;
    }
    if cells.h[cell] > 60.0 {
        return CultureType::Highland;
    }
    if cells.r[cell] != 0 && cells.fl[cell] >= 100.0 {
        return CultureType::River;
    }
    let biome = biomes[cell];
    // The cell has a place, so only a small rural population counts.
    if cells.pop[cell] <= 5.0 {
        if cells.pop[cell] < 5.0 && (1..=4).contains(&biome) {
            return CultureType::Nomadic;
        }
        if biome > 4 && biome < 10 {
            return CultureType::Hunting;
        }
    }
    CultureType::Generic
}

/// One burg group of FMG (`getDefaultGroups`), in the order of the list.
struct Group {
    name: &'static str,
    min: f64,
    max: f64,
    percentile: f64,
    /// capital, citadel, walls, plaza, port, temple: `Some(value)` to match.
    features: [Option<bool>; 6],
    biomes: &'static [u8],
}

const GROUPS: [Group; 9] = [
    Group {
        name: "capital",
        min: 0.0,
        max: 0.0,
        percentile: 0.0,
        features: [Some(true), None, None, None, None, None],
        biomes: &[],
    },
    Group {
        name: "city",
        min: 5.0,
        max: 0.0,
        percentile: 90.0,
        features: [None; 6],
        biomes: &[],
    },
    Group {
        name: "fort",
        min: 0.0,
        max: 1.0,
        percentile: 0.0,
        features: [
            None,
            Some(true),
            Some(false),
            Some(false),
            Some(false),
            None,
        ],
        biomes: &[],
    },
    Group {
        name: "monastery",
        min: 0.0,
        max: 0.8,
        percentile: 0.0,
        features: [
            None,
            None,
            Some(false),
            Some(false),
            Some(false),
            Some(true),
        ],
        biomes: &[],
    },
    Group {
        name: "caravanserai",
        min: 0.0,
        max: 0.8,
        percentile: 0.0,
        features: [None, None, None, Some(true), Some(false), None],
        biomes: &[1, 2, 3],
    },
    Group {
        name: "trading_post",
        min: 0.0,
        max: 0.8,
        percentile: 0.0,
        features: [None, None, None, Some(true), None, None],
        biomes: &[5, 6, 7, 8, 9, 10, 11, 12],
    },
    Group {
        name: "village",
        min: 0.1,
        max: 2.0,
        percentile: 0.0,
        features: [None; 6],
        biomes: &[],
    },
    Group {
        name: "hamlet",
        min: 0.0,
        max: 0.1,
        percentile: 0.0,
        features: [None, None, None, Some(false), None, None],
        biomes: &[],
    },
    Group {
        name: "town",
        min: 0.0,
        max: 0.0,
        percentile: 0.0,
        features: [None; 6],
        biomes: &[],
    },
];

/// The group of a place (`defineGroup`). `populations` is sorted up.
fn group(p: &Place, biome: u8, populations: &[f64]) -> &'static str {
    let have = [p.capital, p.citadel, p.walls, p.plaza, p.port, p.temple];
    for g in &GROUPS {
        if g.min != 0.0 && p.population < g.min {
            continue;
        }
        if g.max != 0.0 && p.population > g.max {
            continue;
        }
        if g.features
            .iter()
            .zip(have)
            .any(|(want, has)| want.is_some_and(|w| w != has))
        {
            continue;
        }
        if !g.biomes.is_empty() && !g.biomes.contains(&biome) {
            continue;
        }
        if g.percentile != 0.0 {
            let index = populations
                .iter()
                .position(|&x| x == p.population)
                .unwrap_or(0);
            if (index as f64) < (populations.len() as f64 * g.percentile / 100.0).floor() {
                continue;
            }
        }
        return g.name;
    }
    "town"
}

/// The kind of a generated place from its group.
fn kind_of(group: &str) -> PlaceKind {
    match group {
        "capital" | "city" => PlaceKind::City,
        "fort" => PlaceKind::Fortress,
        "village" | "hamlet" => PlaceKind::Village,
        _ => PlaceKind::Town,
    }
}

/// The group of a pinned place, from its kind.
fn pinned_group(p: &Place) -> &'static str {
    if p.capital {
        return "capital";
    }
    match p.kind {
        PlaceKind::City => "city",
        PlaceKind::Town => "town",
        PlaceKind::Village => "village",
        PlaceKind::Fortress => "fort",
        PlaceKind::Ruin => "ruin",
        PlaceKind::Custom => "custom",
    }
}

/// Population, type, buildings, and group of each place (`specify`).
/// `connectivity` is the route connectivity of each cell. A pinned place keeps its kind; its group comes
/// from its kind.
pub fn specify(
    places: &mut [Place],
    cells: &Cells,
    features: &Features,
    biomes: &[u8],
    connectivity: &[f64],
    rng: &mut Rand,
) {
    for p in places.iter_mut() {
        // definePopulation.
        let mut pop = cells.s[p.cell] / 5.0;
        if p.capital {
            pop *= 1.5;
        }
        if connectivity[p.cell] != 0.0 {
            pop *= connectivity[p.cell];
        }
        pop *= rng.gauss(1.0, 1.0, 0.25, 4.0, 5);
        pop += (f64::from(p.id % 100) - (p.cell % 100) as f64) / 1000.0;
        p.population = crate::rand::rn(pop.max(0.01), 3);
        p.ptype = place_type(cells, features, biomes, p.cell, p.port);
        // defineFeatures. FMG defines state forms after this step, so no
        // state is a theocracy yet.
        let pop = p.population;
        p.citadel =
            p.capital || (pop > 50.0 && rng.p(0.75)) || (pop > 15.0 && rng.p(0.5)) || rng.p(0.1);
        // The markets give the plaza later: each market center has one.
        p.plaza = false;
        p.walls = p.capital
            || pop > 30.0
            || (pop > 20.0 && rng.p(0.75))
            || (pop > 10.0 && rng.p(0.5))
            || rng.p(0.1);
        p.shanty =
            pop > 60.0 || (pop > 40.0 && rng.p(0.75)) || (pop > 20.0 && p.walls && rng.p(0.4));
        p.temple = pop > 50.0 || (pop > 35.0 && rng.p(0.75)) || (pop > 20.0 && rng.p(0.5));
    }
    let mut populations: Vec<f64> = places.iter().map(|p| p.population).collect();
    populations.sort_by(f64::total_cmp);
    for p in places.iter_mut() {
        if p.pin.is_some() {
            p.group = pinned_group(p).to_owned();
            continue;
        }
        let g = group(p, biomes[p.cell], &populations);
        p.group = g.to_owned();
        p.kind = kind_of(g);
    }
}

/// The water feature where a river ends (`resolveDrainFeature`).
fn drain_feature(input: &Input, cell: usize) -> u32 {
    drain_of_river(input, input.cells.r[cell])
}

/// The water feature where a river and the rivers it joins end.
fn drain_of_river(input: &Input, river: u32) -> u32 {
    let by_id: BTreeMap<u32, &River> = input.rivers.iter().map(|r| (r.id, r)).collect();
    let mut id = river;
    let mut seen = BTreeSet::new();
    while let Some(r) = by_id.get(&id) {
        if !seen.insert(id) {
            break;
        }
        match r.mouth {
            Mouth::Water(c) => return input.features.of_cell[c],
            Mouth::Join(_) => match r.parent {
                Some(p) => id = p,
                None => break,
            },
            Mouth::Sink | Mouth::Edge => break,
        }
    }
    0
}

struct Candidate {
    place: usize,
    haven: Option<usize>,
    water: u32,
    landmass: u32,
    preferred: bool,
}

/// Make ports and move places to the water (`assignPorts`).
pub fn assign_ports(input: &Input, places: &mut [Place]) {
    let Input {
        config,
        grid,
        cells,
        features,
        temperature,
        ..
    } = *input;
    // inkland: a move never takes a place nearer to a pin than the gap.
    let pin_points: Vec<[f64; 2]> = config.pins.points.iter().map(|p| p.at).collect();
    let gap = pin_gap(config, grid);
    let keep = |site: [f64; 2], moved: [f64; 2]| {
        if near(&pin_points, moved, gap) {
            site
        } else {
            moved
        }
    };
    // A pinned port trades on the water next to it.
    for p in places.iter_mut().filter(|p| p.pin.is_some()) {
        if p.port {
            let water = cells
                .haven
                .get(p.cell)
                .copied()
                .flatten()
                .or_else(|| grid.neighbors(p.cell).find(|&m| !cells.land(m)));
            p.water = water.map_or(NONE, |c| features.of_cell[c]);
        }
    }
    let mut by_water: BTreeMap<u32, Vec<Candidate>> = BTreeMap::new();
    for (k, p) in places.iter().enumerate() {
        if p.pin.is_some() {
            continue;
        }
        if let Some(haven) = cells.haven[p.cell] {
            let harbor = cells.harbor[p.cell];
            let mut water = features.of_cell[haven];
            let f = features.get(water);
            if f.cells <= 1 || ["dry", "frozen", "lava"].contains(&cells.group[haven]) {
                continue;
            }
            // A lake with an outlet trades on the water where it drains.
            if f.kind == FeatureKind::Lake
                && let Some(out) = input
                    .lakes
                    .iter()
                    .find(|l| l.feature == water)
                    .and_then(|l| l.outlet)
            {
                let drain = drain_of_river(input, out);
                if drain != NONE {
                    water = drain;
                }
            }
            if temperature[p.cell].trunc() <= 0.0 {
                continue;
            }
            by_water.entry(water).or_default().push(Candidate {
                place: k,
                haven: Some(haven),
                water,
                landmass: p.landmass,
                preferred: (harbor != 0 && p.capital) || harbor == 1,
            });
        } else {
            if !cells.navigable(p.cell) {
                continue;
            }
            let water = drain_feature(input, p.cell);
            if water == NONE {
                continue;
            }
            by_water.entry(water).or_default().push(Candidate {
                place: k,
                haven: None,
                water,
                landmass: p.landmass,
                preferred: true,
            });
        }
    }
    for candidates in by_water.values() {
        for c in select_ports(candidates, places, cells) {
            let p = &mut places[c.place];
            p.port = true;
            p.water = c.water;
            let moved = match c.haven {
                Some(h) => edge_point(grid, p.cell, h),
                None => river_bank(input, p.cell),
            };
            p.at = keep(grid.points[p.cell], moved);
        }
    }
    // Other river places move to a bank.
    for p in places.iter_mut() {
        if p.pin.is_some() || p.port || cells.r[p.cell] == 0 {
            continue;
        }
        p.at = keep(grid.points[p.cell], river_bank(input, p.cell));
    }
}

fn select_ports<'a>(
    candidates: &'a [Candidate],
    places: &[Place],
    cells: &Cells,
) -> Vec<&'a Candidate> {
    let rank = |c: &Candidate| -> i64 {
        let capital = if places[c.place].capital { -1000 } else { 0 };
        let harbor = if c.haven.is_some() {
            i64::from(cells.harbor[places[c.place].cell])
        } else {
            0
        };
        capital + harbor
    };
    let mut promoted: Vec<bool> = candidates.iter().map(|c| c.preferred).collect();
    let mut lands: Vec<u32> = candidates.iter().map(|c| c.landmass).collect();
    lands.dedup();
    let mut seen = BTreeSet::new();
    let lands: Vec<u32> = lands.into_iter().filter(|l| seen.insert(*l)).collect();
    for land in lands {
        let group: Vec<usize> = (0..candidates.len())
            .filter(|&k| candidates[k].landmass == land)
            .collect();
        if group.iter().any(|&k| promoted[k]) {
            continue;
        }
        let mut best = group[0];
        for &k in &group[1..] {
            if rank(&candidates[k]) < rank(&candidates[best]) {
                best = k;
            }
        }
        promoted[best] = true;
    }
    let count = |p: &[bool]| p.iter().filter(|&&x| x).count();
    if count(&promoted) < 2 {
        let mut rest: Vec<usize> = (0..candidates.len()).filter(|&k| !promoted[k]).collect();
        rest.sort_by_key(|&k| rank(&candidates[k]));
        for k in rest {
            promoted[k] = true;
            if count(&promoted) >= 2 {
                break;
            }
        }
    }
    if count(&promoted) < 2 {
        return Vec::new();
    }
    (0..candidates.len())
        .filter(|&k| promoted[k])
        .map(|k| &candidates[k])
        .collect()
}

/// A point 95 % of the way from the site of `cell` to the middle of its edge
/// with `other` (`getCloseToEdgePoint`).
fn edge_point(grid: &Grid, cell: usize, other: usize) -> [f64; 2] {
    let [x0, y0] = grid.points[cell];
    let poly = &grid.polygons[cell];
    let k = grid.edge_cells[cell]
        .iter()
        .position(|&c| c as usize == other);
    let Some(k) = k else { return [x0, y0] };
    let a = grid.vertices[poly[k] as usize];
    let b = grid.vertices[poly[(k + 1) % poly.len()] as usize];
    let (xe, ye) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
    [x0 + 0.95 * (xe - x0), y0 + 0.95 * (ye - y0)]
}

/// The site of a river cell moved off the river line, to one bank
/// (`shiftTowardsRiverBank`). The shift is at most 0.6 FMG pixels.
fn river_bank(input: &Input, cell: usize) -> [f64; 2] {
    let Input {
        grid,
        cells,
        rivers,
        ..
    } = *input;
    let [x, y] = grid.points[cell];
    let shift = (cells.fl[cell] / 200.0).min(0.6) / cells.px;
    let within = |p: [f64; 2]| {
        if grid.find_cell(p[0], p[1]) == cell {
            p
        } else {
            [x, y]
        }
    };
    let tangent = rivers.iter().find(|r| r.id == cells.r[cell]).and_then(|r| {
        let idx = r.cells.iter().position(|&c| c == cell)?;
        let from = if idx > 0 {
            grid.points[r.cells[idx - 1]]
        } else {
            [x, y]
        };
        let to = r.cells.get(idx + 1).map_or([x, y], |&c| grid.points[c]);
        let t = [to[0] - from[0], to[1] - from[1]];
        (t != [0.0, 0.0]).then_some(t)
    });
    match tangent {
        None => {
            let xs = if cell % 2 == 1 { x + shift } else { x - shift };
            let ys = if cells.r[cell] % 2 == 1 {
                y + shift
            } else {
                y - shift
            };
            within([xs, ys])
        }
        Some([tx, ty]) => {
            let len = (tx * tx + ty * ty).sqrt();
            let side = if cell % 2 == 1 { 1.0 } else { -1.0 };
            within([
                x + (-ty / len) * shift * side,
                y + (tx / len) * shift * side,
            ])
        }
    }
}
