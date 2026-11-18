//! The colours of biomes, states, provinces, cultures, and religions.
//!
//! The world has no colours, so the renderer gives them as the generators
//! of the original program do, with random numbers from the `decoration`
//! part of the seed tree.

use super::super::color::{colors, mixed_color, random_color};
use inkland_core::rand::Rand;
use inkland_core::seed::{Layer, SeedTree};
use inkland_core::world::World;
use std::collections::{BTreeMap, BTreeSet};

/// The colour of each biome (`biomes-generator.ts`).
pub const BIOME_COLORS: [&str; 13] = [
    "#466eab", "#fbe79f", "#b5b887", "#d2d082", "#c8d68f", "#b6d95d", "#29bc56", "#7dcb35",
    "#409c43", "#4b6b32", "#96784b", "#d5e7eb", "#0b9131",
];

/// The colours of a world, by id.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub states: BTreeMap<u32, String>,
    pub provinces: BTreeMap<u32, String>,
    pub cultures: BTreeMap<u32, String>,
    pub religions: BTreeMap<u32, String>,
}

impl Palette {
    pub fn of(world: &World) -> Palette {
        let tree = SeedTree::new(world.seed);
        let rng = |part: &str| Rand::new(tree.part_rng(Layer::Decoration, part));
        let soc = &world.society;

        // Cultures: `getColors`.
        let mut r = rng("culture-colors");
        let list = colors(soc.cultures.len(), &mut r);
        let cultures: BTreeMap<u32, String> = soc
            .cultures
            .iter()
            .zip(list)
            .map(|(c, color)| (c.id, color))
            .collect();

        // States: greedy colouring with Set2 so that neighbours differ, then
        // a small random change of each colour after its first use
        // (`assignColors`).
        let g = &world.grid;
        let mut neighbours: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for c in 0..g.len() {
            let a = soc.cell_state[c];
            if a == 0 {
                continue;
            }
            for o in g.neighbors(c) {
                let b = soc.cell_state[o];
                if b != 0 && b != a {
                    neighbours.entry(a).or_default().insert(b);
                }
            }
        }
        let mut r = rng("state-colors");
        let mut set2: Vec<&str> = vec![
            "#66c2a5", "#fc8d62", "#8da0cb", "#e78ac3", "#a6d854", "#ffd92f",
        ];
        let mut states: BTreeMap<u32, String> = BTreeMap::new();
        for st in &soc.states {
            let near = neighbours.get(&st.id);
            let pick = set2.iter().find(|c| {
                near.is_none_or(|n| {
                    n.iter()
                        .all(|o| states.get(o).map(String::as_str) != Some(**c))
                })
            });
            let color = match pick {
                Some(c) => (*c).to_owned(),
                None => random_color(&mut r),
            };
            states.insert(st.id, color);
            set2.rotate_left(1);
        }
        let used: Vec<String> = set2.iter().map(|c| (*c).to_owned()).collect();
        for c in used {
            let same: Vec<u32> = soc
                .states
                .iter()
                .filter(|s| states.get(&s.id) == Some(&c))
                .map(|s| s.id)
                .collect();
            for id in same.into_iter().skip(1) {
                let mixed = mixed_color(&c, 0.2, 0.3, &mut r);
                states.insert(id, mixed);
            }
        }

        // Provinces: a mix of the state colour.
        let mut r = rng("province-colors");
        let mut provinces: BTreeMap<u32, String> = soc
            .provinces
            .iter()
            .map(|p| {
                let base = states
                    .get(&p.state)
                    .cloned()
                    .unwrap_or_else(|| "#999999".to_owned());
                (p.id, mixed_color(&base, 0.2, 0.3, &mut r))
            })
            .collect();

        // Religions: a mix of the culture colour.
        let mut r = rng("religion-colors");
        let mut religions: BTreeMap<u32, String> = world
            .religions
            .list
            .iter()
            .map(|rel| {
                let color = match cultures.get(&rel.culture) {
                    Some(base) if rel.kind == "Folk" => mixed_color(base, 0.1, 0.0, &mut r),
                    Some(base) => mixed_color(base, 0.25, 0.4, &mut r),
                    None => random_color(&mut r),
                };
                (rel.id, color)
            })
            .collect();

        // The world keeps the FMG colours of states, provinces, and
        // religions; they replace the colours above.
        for st in soc.states.iter().filter(|s| !s.color.is_empty()) {
            states.insert(st.id, st.color.clone());
        }
        for p in soc.provinces.iter().filter(|p| !p.color.is_empty()) {
            provinces.insert(p.id, p.color.clone());
        }
        for rel in world.religions.list.iter().filter(|r| !r.color.is_empty()) {
            religions.insert(rel.id, rel.color.clone());
        }
        Palette {
            states,
            provinces,
            cultures,
            religions,
        }
    }
}
