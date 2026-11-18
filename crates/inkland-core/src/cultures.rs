//! Cultures (`cultures-generator.ts`): `generate` and `expand`.
//!
//! - The set gives the cultures. When the set has more cultures than the
//!   map needs, a culture is picked with the chance `odd`.
//! - Each culture puts its center on a cell with a high score of its sort
//!   function, at a distance from the other centers. The distance becomes
//!   smaller on each retry.
//! - The center cell gives the type: Nomadic, Highland, Lake, Naval, River,
//!   Hunting, or Generic. The type gives the expansionism and the costs of
//!   the expansion.
//! - The expansion is a least-cost flood from the centers with a total
//!   budget. Only populated cells get a culture. Other land is wildland
//!   (culture 0).

use crate::config::Config;
use crate::culture_sets::{CultureDef, SortCtx, culture_set, name_development};
use crate::features::{FeatureKind, Features};
use crate::grid::Grid;
use crate::names::{Chain, abbreviate};
use crate::population::Cells;
use crate::rand::{Rand, rn};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// The cost to move through each biome (`biomesData.cost`).
pub const BIOME_COST: [f64; 13] = [
    10.0, 200.0, 150.0, 60.0, 50.0, 70.0, 70.0, 80.0, 90.0, 200.0, 1000.0, 5000.0, 150.0,
];

/// The development of a culture without a value.
pub const DEFAULT_DEVELOPMENT: u32 = 33;

/// The name base of the wildlands (culture 0).
pub const WILDLANDS_BASE: usize = 1;

/// The type of a culture, a state, or a place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CultureType {
    Generic,
    Hunting,
    Highland,
    River,
    Lake,
    Naval,
    Nomadic,
}

impl CultureType {
    pub fn name(self) -> &'static str {
        match self {
            CultureType::Generic => "Generic",
            CultureType::Hunting => "Hunting",
            CultureType::Highland => "Highland",
            CultureType::River => "River",
            CultureType::Lake => "Lake",
            CultureType::Naval => "Naval",
            CultureType::Nomadic => "Nomadic",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Culture {
    /// 1, 2, 3, ... Culture 0 is the wildlands and is not in the list.
    pub id: u32,
    pub name: String,
    /// The index of its name base.
    pub base: usize,
    pub center: usize,
    pub kind: CultureType,
    pub expansionism: f64,
    /// Technological development, 0 to 100, at the center.
    pub development: u32,
    /// A short code of the name.
    pub code: String,
    /// The shield shape of its coats of arms (FMG `shield`). Empty for a
    /// culture of the `random` set: the emblem stage draws its shape.
    pub shield: String,
}

/// What the culture stage reads.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub cells: &'a Cells,
    pub features: &'a Features,
    pub biomes: &'a [u8],
    pub temperature: &'a [f64],
}

impl Input<'_> {
    fn is_lake(&self, c: usize) -> bool {
        self.features.get(self.features.of_cell[c]).kind == FeatureKind::Lake
    }
}

/// A culture of the set before it has a center.
struct Pick {
    name: String,
    shield: &'static str,
    base: usize,
    odd: f64,
    development: u32,
    sort: Option<crate::culture_sets::SortFn>,
}

fn of_def(d: &CultureDef) -> Pick {
    Pick {
        name: d.name.to_owned(),
        shield: d.shield,
        base: d.base,
        odd: d.odd,
        development: d.development,
        sort: d.sort,
    }
}

/// The shields of the 10 cultures of the `english` set, in order.
const ENGLISH_SHIELDS: [&str; 10] = [
    "heater",
    "wedged",
    "swiss",
    "oldFrench",
    "swiss",
    "spanish",
    "hessen",
    "fantasy5",
    "fantasy4",
    "fantasy1",
];

/// The cultures of the set (`getDefault`).
fn defaults(set: &str, count: usize, rng: &mut Rand) -> Vec<Pick> {
    match set {
        "english" => {
            let chain = Chain::new(1);
            ENGLISH_SHIELDS
                .iter()
                .map(|&shield| Pick {
                    name: chain.get_base(rng, 5, 9, Some("")),
                    shield,
                    base: 1,
                    odd: 1.0,
                    development: 36,
                    sort: None,
                })
                .collect()
        }
        "random" => (0..count)
            .map(|_| {
                let base = rng.rand(0, crate::names::data::NAME_BASES.len() as i64 - 1) as usize;
                let name = Chain::new(base).get_short(rng);
                Pick {
                    development: name_development(&name),
                    name,
                    shield: "",
                    base,
                    odd: 1.0,
                    sort: None,
                }
            })
            .collect(),
        _ => culture_set(set)
            .or_else(|| culture_set("european"))
            .map(|s| s.cultures.iter().map(of_def).collect())
            .unwrap_or_default(),
    }
}

/// Pick `count` cultures of the set (`selectCultures`).
fn select(mut list: Vec<Pick>, count: usize, rng: &mut Rand) -> Vec<Pick> {
    if count == list.len() {
        return list;
    }
    if list.iter().all(|d| d.odd == 1.0) {
        list.truncate(count);
        return list;
    }
    let mut out = Vec::new();
    let mut i = 0;
    while out.len() < count && !list.is_empty() {
        let mut k;
        loop {
            k = rng.rand(0, list.len() as i64 - 1) as usize;
            i += 1;
            if !(i < 200 && !rng.p(list[k].odd)) {
                break;
            }
        }
        out.push(list.remove(k));
    }
    out
}

/// Make the cultures and place their centers (`generate`).
pub fn generate(input: &Input, rng: &mut Rand) -> Vec<Culture> {
    let Input {
        config,
        grid,
        cells,
        biomes,
        temperature,
        ..
    } = *input;
    let n = grid.len();
    let set_name = config.world.culture_set.as_str();
    let set_max = culture_set(set_name).map_or(15, |s| s.max);
    let mut count = (config.world.cultures as usize).min(set_max);
    let populated: Vec<usize> = (0..n).filter(|&i| cells.s[i] != 0.0).collect();
    if populated.len() < count * 25 {
        count = populated.len() / 50;
    }
    // inkland: a world with people has one culture or more, so that each
    // place and state has a culture.
    if count == 0 {
        count = usize::from(!populated.is_empty());
    }
    if count == 0 {
        return Vec::new();
    }
    let picks = select(defaults(set_name, count, rng), count, rng);
    let count = picks.len();

    let s_max = cells.s.iter().copied().fold(0.0, f64::max);
    let temp: Vec<f64> = temperature.iter().map(|t| t.trunc()).collect();
    let sea_coast: Vec<bool> = (0..n)
        .map(|c| cells.haven[c].is_some_and(|h| !input.is_lake(h)))
        .collect();
    let ctx = SortCtx {
        s: &cells.s,
        s_max,
        t: &cells.t,
        h: &cells.h,
        temp: &temp,
        biome: biomes,
        sea_coast: &sea_coast,
    };

    let mut is_center = vec![false; n];
    let mut centers: Vec<[f64; 2]> = Vec::new();
    let mut codes: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (k, pick) in picks.into_iter().enumerate() {
        // placeCenter.
        let mut spacing = (config.frame.width + config.frame.height) / 2.0 / count as f64;
        let score: Vec<f64> = populated
            .iter()
            .map(|&i| pick.sort.map_or(cells.s[i], |f| f(&ctx, i)))
            .collect();
        let mut order: Vec<usize> = (0..populated.len()).collect();
        order.sort_by(|&a, &b| {
            score[b]
                .partial_cmp(&score[a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let sorted: Vec<usize> = order.into_iter().map(|k| populated[k]).collect();
        let max = (sorted.len() / 2) as f64;
        let mut center = 0;
        for _ in 0..100 {
            center = sorted[rng.biased(0.0, max, 5.0) as usize];
            spacing *= 0.9;
            let p = grid.points[center];
            let near = centers
                .iter()
                .any(|q| (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) < spacing * spacing);
            if !is_center[center] && !near {
                break;
            }
        }
        centers.push(grid.points[center]);
        is_center[center] = true;
        let kind = culture_type(input, center, rng);
        let expansionism = expansionism(kind, config.world.size_variety, rng);
        let code = abbreviate(&pick.name, &codes);
        codes.push(code.clone());
        out.push(Culture {
            id: k as u32 + 1,
            name: pick.name,
            base: pick.base % crate::names::data::NAME_BASES.len(),
            center,
            kind,
            expansionism,
            development: if pick.development == 0 {
                DEFAULT_DEVELOPMENT
            } else {
                pick.development
            },
            code,
            shield: pick.shield.to_owned(),
        });
    }
    out
}

/// The type of a culture from its center cell (`defineCultureType`).
fn culture_type(input: &Input, i: usize, rng: &mut Rand) -> CultureType {
    let cells = input.cells;
    let b = input.biomes[i];
    if cells.h[i] < 70.0 && [1, 2, 4].contains(&b) {
        return CultureType::Nomadic;
    }
    if cells.h[i] > 50.0 {
        return CultureType::Highland;
    }
    let haven = cells.haven[i];
    let lake = haven.is_some_and(|h| input.is_lake(h));
    if let Some(h) = haven
        && lake
        && input.features.get(input.features.of_cell[h]).cells > 5
    {
        return CultureType::Lake;
    }
    let harbor = cells.harbor[i];
    if (harbor != 0 && !lake && rng.p(0.1))
        || (harbor == 1 && rng.p(0.6))
        || (cells.isle[i] && rng.p(0.4))
    {
        return CultureType::Naval;
    }
    if cells.r[i] != 0 && cells.fl[i] > 100.0 {
        return CultureType::River;
    }
    if cells.t[i] > 2 && [3, 7, 8, 9, 10, 12].contains(&b) {
        return CultureType::Hunting;
    }
    CultureType::Generic
}

/// `defineCultureExpansionism`.
fn expansionism(kind: CultureType, size_variety: f64, rng: &mut Rand) -> f64 {
    let base = match kind {
        CultureType::Lake => 0.8,
        CultureType::Naval => 1.5,
        CultureType::River => 0.9,
        CultureType::Nomadic => 1.5,
        CultureType::Hunting => 0.7,
        CultureType::Highland => 1.2,
        CultureType::Generic => 1.0,
    };
    rn((rng.unit() * size_variety / 2.0 + 1.0) * base, 1)
}

/// A min-heap key with a sequence number, so that equal costs come out in
/// the order they went in.
pub(crate) struct Key(pub f64, pub u64);
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

/// Grow the cultures over the map (`expand`). The result is the culture of
/// each cell, or 0.
pub fn expand(input: &Input, cultures: &[Culture]) -> Vec<u32> {
    let Input {
        config,
        grid,
        cells,
        biomes,
        ..
    } = *input;
    let n = grid.len();
    let mut culture = vec![0u32; n];
    let mut cost = vec![0.0f64; n];
    let max_cost = n as f64 * 0.6 * config.world.growth_rate;
    let mut heap = BinaryHeap::new();
    let mut seq = 0u64;
    for c in cultures {
        heap.push(Reverse((Key(0.0, seq), c.center, c.id)));
        seq += 1;
    }
    let biome_cost = |c: &Culture, biome: u8| -> f64 {
        let b = biome as usize;
        if biomes[c.center] == biome {
            10.0
        } else if c.kind == CultureType::Hunting {
            BIOME_COST[b] * 5.0
        } else if c.kind == CultureType::Nomadic && biome > 4 && biome < 10 {
            BIOME_COST[b] * 10.0
        } else {
            BIOME_COST[b] * 2.0
        }
    };
    let height_cost = |i: usize, kind: CultureType| -> f64 {
        let h = cells.h[i];
        let a = cells.area_px(i);
        match kind {
            CultureType::Lake if input.is_lake(i) => 10.0,
            CultureType::Naval if h < 20.0 => a * 2.0,
            CultureType::Nomadic if h < 20.0 => a * 50.0,
            _ if h < 20.0 => a * 6.0,
            CultureType::Highland if h < 44.0 => 3000.0,
            CultureType::Highland if h < 62.0 => 200.0,
            CultureType::Highland => 0.0,
            _ if h >= 67.0 => 200.0,
            _ if h >= 44.0 => 30.0,
            _ => 0.0,
        }
    };
    let river_cost = |i: usize, kind: CultureType| -> f64 {
        if kind == CultureType::River {
            return if cells.r[i] != 0 { 0.0 } else { 100.0 };
        }
        if cells.r[i] == 0 {
            return 0.0;
        }
        (cells.fl[i] / 10.0).clamp(20.0, 100.0)
    };
    while let Some(Reverse((Key(priority, _), cell, id))) = heap.pop() {
        let c = &cultures[id as usize - 1];
        let source_biome = biomes[cell];
        for m in grid.neighbors(cell) {
            let target_biome = biomes[m];
            let change = if source_biome == target_biome {
                0.0
            } else {
                20.0
            };
            let cell_cost = (biome_cost(c, target_biome)
                + change
                + height_cost(m, c.kind)
                + river_cost(m, c.kind)
                + type_cost(cells.t[m], c.kind))
                / c.expansionism;
            let total = priority + cell_cost;
            if total > max_cost {
                continue;
            }
            if cost[m] == 0.0 || total < cost[m] {
                if cells.pop[m] > 0.0 {
                    culture[m] = id;
                }
                cost[m] = total;
                heap.push(Reverse((Key(total, seq), m, id)));
                seq += 1;
            }
        }
    }
    // The center of each culture keeps it.
    for c in cultures {
        if culture[c.center] == 0 {
            culture[c.center] = c.id;
        }
    }
    culture
}

/// inkland: a pinned place on wildland takes the culture of the nearest
/// land cell with a culture, so that each pinned place has a culture.
pub fn adopt(grid: &Grid, cells: &Cells, culture: &mut [u32], pinned: &[usize]) {
    for &start in pinned {
        if culture[start] != 0 {
            continue;
        }
        let mut seen = vec![false; grid.len()];
        let mut queue = std::collections::VecDeque::from([start]);
        seen[start] = true;
        while let Some(c) = queue.pop_front() {
            if culture[c] != 0 {
                culture[start] = culture[c];
                break;
            }
            for m in grid.neighbors(c) {
                if !seen[m] && cells.land(m) {
                    seen[m] = true;
                    queue.push_back(m);
                }
            }
        }
    }
}

/// The cost of the distance field for a culture or a state (`getTypeCost`).
pub fn type_cost(t: i32, kind: CultureType) -> f64 {
    use CultureType::*;
    match t {
        1 => match kind {
            Naval | Lake => 0.0,
            Nomadic => 60.0,
            _ => 20.0,
        },
        2 => match kind {
            Naval | Nomadic => 30.0,
            _ => 0.0,
        },
        -1 => 0.0,
        _ => match kind {
            Naval | Lake => 100.0,
            _ => 0.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::{Layer, SeedTree};

    fn rng() -> Rand {
        Rand::new(SeedTree::new(2).part_rng(Layer::Cultures, "test"))
    }

    #[test]
    fn selection_takes_the_first_cultures_when_each_odd_is_one() {
        let mut r = rng();
        let pick = |name: &str| Pick {
            name: name.to_owned(),
            shield: "heater",
            base: 0,
            odd: 1.0,
            development: 0,
            sort: None,
        };
        let list = vec![pick("A"), pick("B"), pick("C"), pick("D"), pick("E")];
        let picked = select(list, 3, &mut r);
        let names: Vec<&str> = picked.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["A", "B", "C"]);
    }

    #[test]
    fn selection_with_odds_gives_distinct_cultures() {
        let mut r = rng();
        let list = defaults("world", 12, &mut r);
        let picked = select(list, 12, &mut r);
        assert_eq!(picked.len(), 12);
        let mut names: Vec<&str> = picked.iter().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 12);
    }

    #[test]
    fn the_random_and_english_sets_make_their_cultures() {
        let mut r = rng();
        assert_eq!(defaults("random", 7, &mut r).len(), 7);
        let english = defaults("english", 3, &mut r);
        assert_eq!(english.len(), 10);
        assert!(english.iter().all(|p| p.base == 1));
        assert_eq!(english[0].shield, "heater");
        assert_eq!(english[9].shield, "fantasy1");
        assert!(
            defaults("random", 2, &mut r)
                .iter()
                .all(|p| p.shield.is_empty())
        );
    }

    #[test]
    fn each_culture_of_a_set_has_the_shield_of_fmg() {
        let mut r = rng();
        let european = defaults("european", 15, &mut r);
        assert_eq!(european[0].name, "Shwazen");
        assert_eq!(european[0].shield, "swiss");
        let dark = defaults("dark-fantasy", 34, &mut r);
        assert_eq!(dark.last().map(|p| p.shield), Some("fantasy1"));
        for set in crate::culture_sets::CULTURE_SETS {
            assert!(
                set.cultures.iter().all(|c| !c.shield.is_empty()),
                "{}",
                set.name
            );
        }
    }

    #[test]
    fn type_costs_follow_fmg() {
        assert_eq!(type_cost(1, CultureType::Naval), 0.0);
        assert_eq!(type_cost(1, CultureType::Nomadic), 60.0);
        assert_eq!(type_cost(1, CultureType::Generic), 20.0);
        assert_eq!(type_cost(2, CultureType::Naval), 30.0);
        assert_eq!(type_cost(5, CultureType::Lake), 100.0);
        assert_eq!(type_cost(-1, CultureType::Lake), 0.0);
        assert_eq!(type_cost(-3, CultureType::Naval), 100.0);
    }
}
