//! Names and groups of rivers, islands, and lakes.
//!
//! This stage runs after the society stage, because names come from the
//! culture of the land:
//!
//! - A river gets a name from the culture at its last land cell. A pinned
//!   river with a `name` keeps it. A river is a `river` when its water at
//!   the mouth is at least 4 times the river threshold, else a `stream`.
//! - A land feature is a `continent` when it has at least 10 % of all cells,
//!   an `island` when it has at least 0.5 % (and 10 cells or more), else an
//!   `isle`. A continent and an island get a name from the culture with the
//!   most of their cells.
//! - A lake is `frozen` when its mean temperature is below -5 C, else
//!   `freshwater`. A lake of 2 or more cells gets the name `Lake <name>` from
//!   the culture with the most of its shore cells.
//! - An ocean is `ocean` and has no name. Seas in a book come from pinned
//!   regions.

use crate::config::Config;
use crate::features::{FeatureKind, Features};
use crate::grid::Grid;
use crate::names::Chain;
use crate::rand::Rand;
use crate::rivers::River;
use crate::seed::{Layer, SeedTree};
use crate::society::Society;
use std::collections::{BTreeMap, BTreeSet};

/// The name and the type of a river.
#[derive(Debug, Clone, PartialEq)]
pub struct RiverName {
    pub name: String,
    /// `river` or `stream`.
    pub kind: &'static str,
}

/// The group and the name of a feature.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureName {
    /// The feature id.
    pub id: u32,
    /// `continent`, `island`, `isle`, `freshwater`, `frozen`, or `ocean`.
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
    pub temperature: &'a [f64],
    pub river_threshold: f64,
    pub society: &'a Society,
}

/// The culture with the most of the cells. Ties go to the lower id. `None`
/// when no cell has a culture.
fn main_culture(cells: impl Iterator<Item = usize>, culture: &[u32]) -> Option<u32> {
    let mut count: BTreeMap<u32, usize> = BTreeMap::new();
    for c in cells {
        if culture[c] != 0 {
            *count.entry(culture[c]).or_default() += 1;
        }
    }
    count
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(k, _)| k)
}

/// Give names and groups.
pub fn build(input: &Input) -> Names {
    let g = input.grid;
    let soc = input.society;
    let tree = SeedTree::new(input.config.seed);
    let chains: Vec<Chain> = soc.cultures.iter().map(|c| Chain::new(c.base)).collect();
    let chain = |culture: u32| -> &Chain {
        let k = culture.max(1) as usize - 1;
        &chains[k.min(chains.len() - 1)]
    };
    let mut used: BTreeSet<String> = soc.places.iter().map(|p| p.name.clone()).collect();
    for s in &soc.states {
        used.insert(s.name.clone());
    }

    // Rivers.
    let mut rng = Rand::new(tree.part_rng(Layer::Names, "rivers"));
    let mut rivers = Vec::with_capacity(input.rivers.len());
    for r in input.rivers {
        let pinned = r
            .course
            .and_then(|k| input.config.pins.rivers[k].name.clone());
        let name = match pinned {
            Some(n) => n,
            None => {
                let last = r.cells.last().copied().unwrap_or(0);
                let culture = main_culture(std::iter::once(last), &soc.cell_culture).unwrap_or(1);
                if chains.is_empty() {
                    String::new()
                } else {
                    chain(culture).name(&mut rng, &mut used)
                }
            }
        };
        used.insert(name.clone());
        rivers.push(RiverName {
            name,
            kind: if r.flux >= 4.0 * input.river_threshold {
                "river"
            } else {
                "stream"
            },
        });
    }

    // Features.
    let total = g.len().max(1);
    let mut cells_of: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (c, &f) in input.features.of_cell.iter().enumerate() {
        cells_of.entry(f).or_default().push(c);
    }
    let mut rng = Rand::new(tree.part_rng(Layer::Names, "features"));
    let mut features = Vec::with_capacity(input.features.list.len());
    for f in &input.features.list {
        let cells = cells_of.get(&f.id).map(Vec::as_slice).unwrap_or(&[]);
        let share = f64::from(f.cells) / total as f64;
        let (group, culture) = match f.kind {
            FeatureKind::Island => {
                let group = if share >= 0.1 {
                    "continent"
                } else if share >= 0.005 && f.cells >= 10 {
                    "island"
                } else {
                    "isle"
                };
                let culture = (group != "isle")
                    .then(|| main_culture(cells.iter().copied(), &soc.cell_culture))
                    .flatten();
                (group, culture)
            }
            FeatureKind::Lake => {
                let mean = cells.iter().map(|&c| input.temperature[c]).sum::<f64>()
                    / cells.len().max(1) as f64;
                let group = if mean < -5.0 { "frozen" } else { "freshwater" };
                let shore = cells
                    .iter()
                    .flat_map(|&c| g.neighbors(c))
                    .filter(|&m| input.features.is_land(m));
                let culture = (f.cells >= 2)
                    .then(|| main_culture(shore, &soc.cell_culture))
                    .flatten();
                (group, culture)
            }
            FeatureKind::Ocean => ("ocean", None),
        };
        let name = culture.filter(|_| !chains.is_empty()).map(|k| {
            let base = chain(k).name(&mut rng, &mut used);
            let name = if f.kind == FeatureKind::Lake {
                format!("Lake {base}")
            } else {
                base
            };
            used.insert(name.clone());
            name
        });
        features.push(FeatureName {
            id: f.id,
            group,
            name,
        });
    }
    Names { rivers, features }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
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
    fn each_river_and_feature_has_a_group_and_rivers_have_names() {
        let w = world(json!({}));
        assert_eq!(w.names.rivers.len(), w.rivers.len());
        assert_eq!(w.names.features.len(), w.features.list.len());
        assert!(w.names.rivers.iter().all(|r| !r.name.is_empty()));
        assert!(
            w.names
                .rivers
                .iter()
                .all(|r| r.kind == "river" || r.kind == "stream")
        );
        let groups: Vec<&str> = w.names.features.iter().map(|f| f.group).collect();
        assert!(groups.contains(&"ocean"));
        assert!(
            groups
                .iter()
                .any(|g| matches!(*g, "continent" | "island" | "isle"))
        );
        // Isles and oceans have no name.
        for f in &w.names.features {
            if matches!(f.group, "isle" | "ocean") {
                assert!(f.name.is_none(), "{f:?}");
            }
        }
        // Names do not repeat.
        let mut all: Vec<&str> = w.names.rivers.iter().map(|r| r.name.as_str()).collect();
        all.extend(w.names.features.iter().filter_map(|f| f.name.as_deref()));
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n);
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
