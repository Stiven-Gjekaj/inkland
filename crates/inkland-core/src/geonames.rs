//! Names and groups of rivers, islands, lakes, and oceans.
//!
//! This stage runs after the society stage, because names come from the
//! culture of the land. It follows Azgaar's Fantasy Map Generator (FMG,
//! `riversSpecify` in `model/physical/river-generator.ts`, and
//! `defineGroups` and `defineNames` in `features-generator.ts`):
//!
//! - A river gets a name from the culture at its last land cell. A pinned
//!   river with a `name` keeps it. Its type is a weighted draw: a main
//!   river longer than the shortest 15 % of rivers is a `river`; a shorter
//!   one is a `creek`, `river`, `brook`, or `stream` (weights 9, 3, 3, 1).
//!   Each third river (by id) with a parent is a fork: `fork` when long,
//!   `branch` when short.
//! - A land feature is a `lake_island` when all water around it is lake
//!   water, a `continent` when it has more than a tenth of the cells of the
//!   first grid, an `island` when it has more than a thousandth, else an
//!   `isle`.
//! - A lake is `frozen` below -3 C, and `lava` when it is high (40 above
//!   sea level), small, and its first cell is a tenth cell. A lake without
//!   rivers is `dry` when it loses more than 4 times its water, or a small
//!   `sinkhole`. A lake without an outlet that loses more water than it
//!   gets is `salt`. Other lakes are `freshwater`.
//! - An ocean is an `ocean` when it has more than a 25th of the cells of
//!   the first grid, a `sea` when it has more than a thousandth, else a
//!   `gulf`.
//! - Each feature gets a name. An ocean gets an adjective (8 in 10), or the
//!   side of the map where it starts. Another feature gets an adjective (1
//!   in 10), or a name from the culture of its first cell (a lake: of its
//!   first shore cell). A lake is `Lake <name>`, and an ocean is
//!   `<name> Ocean`, `<name> Sea`, or `<name> Gulf`.

use crate::config::Config;
use crate::features::{FeatureKind, Features};
use crate::grid::Grid;
use crate::names::Chain;
use crate::rand::Rand;
use crate::rivers::{Lake, River};
use crate::seed::{Layer, SeedTree};
use crate::society::Society;
use std::collections::BTreeSet;

/// The name and the type of a river.
#[derive(Debug, Clone, PartialEq)]
pub struct RiverName {
    pub name: String,
    /// `river`, `creek`, `brook`, `stream`, `fork`, or `branch`.
    pub kind: &'static str,
}

/// The group and the name of a feature.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureName {
    /// The feature id.
    pub id: u32,
    /// The FMG subtype: `continent`, `island`, `isle`, `lake_island`;
    /// `freshwater`, `salt`, `frozen`, `dry`, `sinkhole`, `lava`; `ocean`,
    /// `sea`, `gulf`.
    pub group: &'static str,
    pub name: Option<String>,
}

/// The names of a world.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Names {
    /// One for each river, in the order of `World::rivers`.
    pub rivers: Vec<RiverName>,
    /// One for each feature, in the order of `Features::list`.
    pub features: Vec<FeatureName>,
}

/// What the naming stage reads.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub features: &'a Features,
    pub rivers: &'a [River],
    /// The water data of each lake, in feature order.
    pub lakes: &'a [Lake],
    /// The number of cells of the first grid.
    pub base_cells: usize,
    pub sea_level: f64,
    pub society: &'a Society,
}

/// Adjectives for names (FMG `ADJECTIVES` in `features-generator.ts`).
pub const ADJECTIVES: [&str; 70] = [
    "Autumn",
    "Azure",
    "Black",
    "Blue",
    "Bony",
    "Boundless",
    "Broken",
    "Calm",
    "Cold",
    "Crimson",
    "Deep",
    "Draconic",
    "Dreamy",
    "Echoing",
    "Emerald",
    "Endless",
    "Far",
    "Forbidden",
    "Forgotten",
    "Fortunate",
    "Frozen",
    "Glassy",
    "Glittering",
    "Golden",
    "Great",
    "Green",
    "Grey",
    "Icy",
    "Inner",
    "Kingly",
    "Lost",
    "Misty",
    "Outer",
    "Pale",
    "Pearly",
    "Reedy",
    "Red",
    "Restless",
    "Roaring",
    "Ruinous",
    "Sailing",
    "Salty",
    "Sapphire",
    "Serene",
    "Serpentine",
    "Shattered",
    "Shining",
    "Shadowy",
    "Silent",
    "Sirenic",
    "Sleeping",
    "Sorrowful",
    "Starry",
    "Still",
    "Stormy",
    "Sunlit",
    "Summer",
    "Tearful",
    "Thunderous",
    "Tidal",
    "Yellow",
    "Wandering",
    "Whispering",
    "White",
    "Wide",
    "Wild",
    "Windy",
    "Winter",
    "Wondrous",
    "World",
];

/// River types and their weights (FMG `riverTypes`).
const MAIN_SMALL: [(&str, usize); 4] = [("creek", 9), ("river", 3), ("brook", 3), ("stream", 1)];

/// A weighted draw (FMG `rw`): each key as often as its weight, then one
/// even draw.
fn weighted(rng: &mut Rand, table: &[(&'static str, usize)]) -> &'static str {
    let total: usize = table.iter().map(|&(_, w)| w).sum();
    let mut k = rng.int(0, total as i64 - 1) as usize;
    for &(key, w) in table {
        if k < w {
            return key;
        }
        k -= w;
    }
    table[table.len() - 1].0
}

/// An even draw from a list (FMG `ra`).
fn pick<'a>(rng: &mut Rand, list: &[&'a str]) -> &'a str {
    list[rng.int(0, list.len() as i64 - 1) as usize]
}

/// Give names and groups.
pub fn build(input: &Input) -> Names {
    let g = input.grid;
    let soc = input.society;
    let tree = SeedTree::new(input.config.seed);
    let chains: Vec<Chain> = soc.cultures.iter().map(|c| Chain::new(c.base)).collect();
    let mut used: BTreeSet<String> = soc.places.iter().map(|p| p.name.clone()).collect();
    for s in &soc.states {
        used.insert(s.name.clone());
    }
    // A name from the culture of a cell, or `None` when the cell has no
    // culture.
    let culture_name = |cell: usize, rng: &mut Rand, used: &mut BTreeSet<String>| {
        let culture = soc.cell_culture.get(cell).copied().unwrap_or(0);
        if culture == 0 || chains.is_empty() {
            return None;
        }
        let k = (culture as usize - 1).min(chains.len() - 1);
        Some(chains[k].name(rng, used))
    };

    // Rivers (FMG `riversSpecify`).
    let mut rng = Rand::new(tree.part_rng(Layer::Names, "rivers"));
    let mut lengths: Vec<f64> = input.rivers.iter().map(|r| r.length).collect();
    lengths.sort_by(f64::total_cmp);
    let small_length = lengths
        .get((input.rivers.len() as f64 * 0.15).ceil() as usize)
        .copied();
    let mut rivers = Vec::with_capacity(input.rivers.len());
    for r in input.rivers {
        let pinned = r
            .course
            .and_then(|k| input.config.pins.rivers[k].name.clone());
        let name = match pinned {
            Some(n) => n,
            None => {
                let last = r
                    .cells
                    .iter()
                    .rev()
                    .copied()
                    .find(|&c| input.features.is_land(c))
                    .unwrap_or(0);
                culture_name(last, &mut rng, &mut used)
                    .unwrap_or_else(|| pick(&mut rng, &ADJECTIVES).to_owned())
            }
        };
        used.insert(name.clone());
        let small = small_length.is_some_and(|s| r.length < s);
        let fork = r.id.is_multiple_of(3) && r.parent.is_some_and(|p| p != r.id);
        let kind = match (fork, small) {
            (false, false) => weighted(&mut rng, &[("river", 1)]),
            (false, true) => weighted(&mut rng, &MAIN_SMALL),
            (true, false) => weighted(&mut rng, &[("fork", 1)]),
            (true, true) => weighted(&mut rng, &[("branch", 1)]),
        };
        rivers.push(RiverName { name, kind });
    }

    // Feature groups (FMG `defineGroups`).
    let base = input.base_cells as f64;
    let mut lake_of_feature = vec![None; input.features.list.len()];
    for lake in input.lakes {
        lake_of_feature[lake.feature as usize - 1] = Some(lake);
    }
    // The water features around each land feature.
    let mut around: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); input.features.list.len()];
    for c in 0..g.len() {
        if !input.features.is_land(c) {
            continue;
        }
        let f = input.features.of_cell[c];
        for m in g.neighbors(c) {
            if !input.features.is_land(m) {
                around[f as usize - 1].insert(input.features.of_cell[m]);
            }
        }
    }
    let groups: Vec<&'static str> = input
        .features
        .list
        .iter()
        .map(|f| {
            let cells = f64::from(f.cells);
            match f.kind {
                FeatureKind::Island => {
                    let water = &around[f.id as usize - 1];
                    let in_lake = !water.is_empty()
                        && !f.border
                        && water
                            .iter()
                            .all(|&w| input.features.get(w).kind == FeatureKind::Lake);
                    if in_lake {
                        "lake_island"
                    } else if cells > base / 10.0 {
                        "continent"
                    } else if cells > base / 1000.0 {
                        "island"
                    } else {
                        "isle"
                    }
                }
                FeatureKind::Lake => match lake_of_feature[f.id as usize - 1] {
                    Some(l) => lake_group(l, f.height, f.cells, f.first_cell, input.sea_level),
                    None => "freshwater",
                },
                FeatureKind::Ocean => {
                    if cells > base / 25.0 {
                        "ocean"
                    } else if cells > base / 1000.0 {
                        "sea"
                    } else {
                        "gulf"
                    }
                }
            }
        })
        .collect();

    // Feature names (FMG `defineNames`).
    let mut rng = Rand::new(tree.part_rng(Layer::Names, "features"));
    let mut features = Vec::with_capacity(input.features.list.len());
    for (f, &group) in input.features.list.iter().zip(&groups) {
        let first = f.first_cell as usize;
        let name = match f.kind {
            FeatureKind::Ocean => {
                let base = if rng.chance(0.8) {
                    pick(&mut rng, &ADJECTIVES).to_owned()
                } else {
                    let [x, y] = g.points[first];
                    let (dx, dy) = (x / g.width - 0.5, y / g.height - 0.5);
                    let side = if dx.abs() > dy.abs() {
                        if dx < 0.0 { "Western" } else { "Eastern" }
                    } else if dy < 0.0 {
                        "Northern"
                    } else {
                        "Southern"
                    };
                    side.to_owned()
                };
                let noun = match group {
                    "ocean" => "Ocean",
                    "sea" => "Sea",
                    _ => "Gulf",
                };
                format!("{base} {noun}")
            }
            _ => {
                let cell = if f.kind == FeatureKind::Lake {
                    f.shoreline.first().copied().unwrap_or(first)
                } else {
                    first
                };
                let base = if rng.chance(0.1) {
                    pick(&mut rng, &ADJECTIVES).to_owned()
                } else {
                    culture_name(cell, &mut rng, &mut used)
                        .unwrap_or_else(|| pick(&mut rng, &ADJECTIVES).to_owned())
                };
                if f.kind == FeatureKind::Lake {
                    format!("Lake {base}")
                } else {
                    base
                }
            }
        };
        used.insert(name.clone());
        features.push(FeatureName {
            id: f.id,
            group,
            name: Some(name),
        });
    }
    Names { rivers, features }
}

/// The group of a lake (FMG `defineLakeSubtype`). FMG compares the height
/// with 60 on its scale with sea level 20, so inkland uses `sea_level + 40`.
fn lake_group(
    lake: &Lake,
    height: f64,
    cells: u32,
    first_cell: u32,
    sea_level: f64,
) -> &'static str {
    if lake.temp < -3.0 {
        return "frozen";
    }
    if height > sea_level + 40.0 && cells < 10 && first_cell.is_multiple_of(10) {
        return "lava";
    }
    if lake.inlets.is_empty() && lake.outlet.is_none() {
        if lake.evaporation > lake.flux * 4.0 {
            return "dry";
        }
        if cells < 3 && first_cell.is_multiple_of(10) {
            return "sinkhole";
        }
    }
    if lake.outlet.is_none() && lake.evaporation > lake.flux {
        return "salt";
    }
    "freshwater"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;
    use serde_json::json;

    fn world(extra: serde_json::Value) -> World {
        let mut v = json!({
            "version": 1, "seed": 6, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "archipelago" }
        });
        for (k, val) in extra.as_object().unwrap() {
            v[k] = val.clone();
        }
        let c: Config = serde_json::from_value(v).unwrap();
        World::build(&c).unwrap()
    }

    #[test]
    fn each_river_and_feature_has_a_group_and_a_name() {
        let w = world(json!({}));
        assert_eq!(w.names.rivers.len(), w.rivers.len());
        assert_eq!(w.names.features.len(), w.features.list.len());
        assert!(w.names.rivers.iter().all(|r| !r.name.is_empty()));
        for (r, n) in w.rivers.iter().zip(&w.names.rivers) {
            assert!(
                ["river", "creek", "brook", "stream", "fork", "branch"].contains(&n.kind),
                "{n:?}"
            );
            if matches!(n.kind, "fork" | "branch") {
                assert_eq!(r.id % 3, 0);
                assert!(r.parent.is_some());
            }
        }
        for (f, n) in w.features.list.iter().zip(&w.names.features) {
            let groups: &[&str] = match f.kind {
                FeatureKind::Island => &["continent", "island", "isle", "lake_island"],
                FeatureKind::Lake => &["freshwater", "salt", "frozen", "dry", "sinkhole", "lava"],
                FeatureKind::Ocean => &["ocean", "sea", "gulf"],
            };
            assert!(groups.contains(&n.group), "{n:?}");
            let name = n.name.as_deref().unwrap();
            match f.kind {
                FeatureKind::Lake => assert!(name.starts_with("Lake ")),
                FeatureKind::Ocean => assert!(
                    name.ends_with(" Ocean") || name.ends_with(" Sea") || name.ends_with(" Gulf")
                ),
                FeatureKind::Island => assert!(!name.is_empty()),
            }
        }
        // Names from the cultures do not repeat; adjectives can repeat, as
        // in FMG.
        let mut all: Vec<&str> = w.names.rivers.iter().map(|r| r.name.as_str()).collect();
        all.extend(w.names.features.iter().filter_map(|f| f.name.as_deref()));
        let mut culture: Vec<&str> = all
            .into_iter()
            .map(|n| n.trim_start_matches("Lake "))
            .filter(|n| !n.split(' ').any(|word| ADJECTIVES.contains(&word)))
            .collect();
        let count = culture.len();
        culture.sort_unstable();
        culture.dedup();
        assert_eq!(culture.len(), count);
    }

    #[test]
    fn an_island_in_a_lake_is_a_lake_island() {
        // Land at 40, a lake in it, and an island in the lake. Where the
        // two regions overlap, the hills win.
        let w = world(json!({
            "world": { "cells": 4000, "template": { "steps": ["Add 40 all"] },
                       "lake_elevation_limit": 80 },
            "pins": { "regions": [
                { "id": "ring", "kind": "lake", "circle": { "at": [200, 150], "r": 40 }, "landlocked": true },
                { "id": "isle", "kind": "hills", "circle": { "at": [200, 150], "r": 15 }, "height": [40, 40] }
            ] }
        }));
        let centre = w.grid.find_cell(200.0, 150.0);
        let f = w.features.of_cell[centre];
        assert_eq!(w.names.features[f as usize - 1].group, "lake_island");
    }

    #[test]
    fn lake_groups_follow_the_fmg_rules() {
        let lake =
            |temp: f64, flux: f64, evaporation: f64, inlets: Vec<u32>, outlet: Option<u32>| Lake {
                feature: 1,
                height: 30.0,
                closed: false,
                flux,
                evaporation,
                temp,
                inlets,
                outlet,
            };
        let g = |l: &Lake, height: f64, cells: u32, first: u32| {
            lake_group(l, height, cells, first, 20.0)
        };
        assert_eq!(
            g(&lake(-5.0, 10.0, 1.0, vec![], None), 30.0, 5, 1),
            "frozen"
        );
        assert_eq!(
            g(&lake(10.0, 10.0, 1.0, vec![1], None), 70.0, 5, 20),
            "lava"
        );
        assert_eq!(g(&lake(10.0, 1.0, 5.0, vec![], None), 30.0, 5, 1), "dry");
        assert_eq!(
            g(&lake(10.0, 10.0, 1.0, vec![], None), 30.0, 2, 30),
            "sinkhole"
        );
        assert_eq!(g(&lake(10.0, 1.0, 2.0, vec![1], None), 30.0, 5, 1), "salt");
        assert_eq!(
            g(&lake(10.0, 10.0, 1.0, vec![1], Some(2)), 30.0, 5, 1),
            "freshwater"
        );
    }

    #[test]
    fn a_pinned_river_keeps_its_name() {
        let w = world(json!({
            "world": { "cells": 4000, "template": "pangea" },
            "pins": {
                "regions": [
                    { "id": "hills", "kind": "mountains", "box": [40, 100, 90, 200] },
                    { "id": "sea", "kind": "sea", "box": [330, 0, 400, 300] }
                ],
                "rivers": [{ "id": "brann", "name": "Brann", "source": { "region": "hills" }, "mouth": { "region": "sea" } }]
            }
        }));
        let i = w.rivers.iter().position(|r| r.course == Some(0)).unwrap();
        assert_eq!(w.names.rivers[i].name, "Brann");
    }
}
