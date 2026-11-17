//! The seed tree.
//!
//! Each layer of generation gets its own random number generator. The seed
//! of a layer comes from the master seed and the name of the layer. Thus a
//! change in one layer never changes the random numbers of another layer.

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use sha2::{Digest, Sha256};

/// The version of the seed derivation. Change it only when all golden files
/// can change.
const DERIVATION: &str = "inkland/v1";

/// A layer of generation. Each layer has its own random numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    Terrain,
    Climate,
    Rivers,
    Biomes,
    Cultures,
    Names,
    Places,
    States,
    Roads,
    Labels,
    Decoration,
    Religions,
    Markers,
}

impl Layer {
    /// All layers, in a fixed order.
    pub const ALL: [Layer; 13] = [
        Layer::Terrain,
        Layer::Climate,
        Layer::Rivers,
        Layer::Biomes,
        Layer::Cultures,
        Layer::Names,
        Layer::Places,
        Layer::States,
        Layer::Roads,
        Layer::Labels,
        Layer::Decoration,
        Layer::Religions,
        Layer::Markers,
    ];

    /// The name of the layer. The seed derivation uses this name, so it
    /// must never change.
    pub fn name(self) -> &'static str {
        match self {
            Layer::Terrain => "terrain",
            Layer::Climate => "climate",
            Layer::Rivers => "rivers",
            Layer::Biomes => "biomes",
            Layer::Cultures => "cultures",
            Layer::Names => "names",
            Layer::Places => "places",
            Layer::States => "states",
            Layer::Roads => "roads",
            Layer::Labels => "labels",
            Layer::Decoration => "decoration",
            Layer::Religions => "religions",
            Layer::Markers => "markers",
        }
    }
}

/// The seed tree of one map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedTree {
    master: u64,
}

impl SeedTree {
    pub fn new(master: u64) -> Self {
        Self { master }
    }

    pub fn master(&self) -> u64 {
        self.master
    }

    /// The 32-byte seed of a layer:
    /// `sha256("inkland/v1/" + master seed in decimal + "/" + layer name)`.
    pub fn layer_seed(&self, layer: Layer) -> [u8; 32] {
        let text = format!("{DERIVATION}/{}/{}", self.master, layer.name());
        Sha256::digest(text.as_bytes()).into()
    }

    /// A new random number generator for a layer. Two calls with the same
    /// layer give the same numbers.
    pub fn rng(&self, layer: Layer) -> ChaCha8Rng {
        ChaCha8Rng::from_seed(self.layer_seed(layer))
    }

    /// The 32-byte seed of one part of a layer:
    /// `sha256("inkland/v1/" + master seed + "/" + layer name + "/" + part)`.
    /// A part has its own numbers, so a change in one part of a layer does
    /// not move the numbers of another part.
    pub fn part_seed(&self, layer: Layer, part: &str) -> [u8; 32] {
        let text = format!("{DERIVATION}/{}/{}/{part}", self.master, layer.name());
        Sha256::digest(text.as_bytes()).into()
    }

    /// A new random number generator for one part of a layer.
    pub fn part_rng(&self, layer: Layer, part: &str) -> ChaCha8Rng {
        ChaCha8Rng::from_seed(self.part_seed(layer, part))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::Rng;
    use std::collections::BTreeSet;

    fn first(tree: SeedTree, layer: Layer) -> [u64; 4] {
        let mut rng = tree.rng(layer);
        [
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
        ]
    }

    #[test]
    fn same_seed_and_layer_give_same_numbers() {
        let a = SeedTree::new(42);
        let b = SeedTree::new(42);
        for layer in Layer::ALL {
            assert_eq!(first(a, layer), first(b, layer));
        }
    }

    #[test]
    fn each_layer_has_different_numbers() {
        let tree = SeedTree::new(42);
        let streams: BTreeSet<[u64; 4]> = Layer::ALL.iter().map(|&l| first(tree, l)).collect();
        assert_eq!(streams.len(), Layer::ALL.len());
    }

    #[test]
    fn a_different_master_seed_changes_each_layer() {
        for layer in Layer::ALL {
            assert_ne!(
                first(SeedTree::new(42), layer),
                first(SeedTree::new(43), layer)
            );
        }
    }

    #[test]
    fn layer_names_are_unique() {
        let names: BTreeSet<&str> = Layer::ALL.iter().map(|l| l.name()).collect();
        assert_eq!(names.len(), Layer::ALL.len());
    }

    #[test]
    fn parts_differ_from_each_other_and_from_the_layer() {
        let tree = SeedTree::new(42);
        let layer = first(tree, Layer::Terrain);
        let mut grid = tree.part_rng(Layer::Terrain, "grid");
        let mut height = tree.part_rng(Layer::Terrain, "heightmap");
        let g = grid.next_u64();
        let h = height.next_u64();
        assert_ne!(g, h);
        assert_ne!(g, layer[0]);
    }

    /// The seed derivation is part of the file format. This test holds the
    /// value that `sha256("inkland/v1/42/terrain")` gives, computed outside
    /// of inkland with `printf 'inkland/v1/42/terrain' | shasum -a 256`.
    #[test]
    fn layer_seed_is_the_documented_hash() {
        let seed = SeedTree::new(42).layer_seed(Layer::Terrain);
        let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, EXPECTED_TERRAIN_42);
    }

    const EXPECTED_TERRAIN_42: &str =
        "4e0c36187ccedd0a7a599b393c11d112ac4ab5ea529ecf6a9893f8a85d76f3a1";
}
