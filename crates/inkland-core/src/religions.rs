//! Religions (`religions-generator.ts`).
//!
//! - Folk religions: one for each culture, on all cells of the culture.
//! - Organized religions and cults: `world.religions` centers at places,
//!   with a spacing of `(width + height) / 2 / religions`. 10 to 40 % of
//!   them are cults.
//! - Each religion gets a form, a supreme deity, a name from the naming
//!   method of its form, an expansion (`global`, `state`, or `culture`),
//!   an expansionism, a color, and a code.
//! - Organized religions and cults grow by least cost. Roads make the
//!   passage cheap; the sea is dear. A religion with the expansion `state`
//!   or `culture` stays in its state or culture.
//! - Organized religions with an expansionism of 3 or more get 0 to 3
//!   heresies at their borders. Heresies grow inside their parent.
//! - Origins: folk religions come from their culture (0); other religions
//!   come from the religions near their center.
//!
//! Religions have no layer in print-safe mode. `world.json` and `query`
//! give them.

use crate::colors::{mixed_color, random_color};
use crate::config::Config;
use crate::cultures::{BIOME_COST, Key, WILDLANDS_BASE};
use crate::grid::Grid;
use crate::names::{Chain, abbreviate, adjective, trim_vowels};
use crate::rand::Rand;
use crate::roads::Road;
use crate::seed::{Layer, SeedTree};
use crate::society::{NONE, Society};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

/// One religion.
#[derive(Debug, Clone, PartialEq)]
pub struct Religion {
    /// 1, 2, 3, ...: folk religions first, in culture order.
    pub id: u32,
    pub name: String,
    /// `Folk`, `Organized`, `Cult`, or `Heresy`.
    pub kind: &'static str,
    /// For example `Polytheism` or `Monotheism`.
    pub form: String,
    /// The supreme deity, if the form has one.
    pub deity: Option<String>,
    /// The culture where the religion starts.
    pub culture: u32,
    /// The center cell.
    pub center: usize,
    /// The place on the center cell, if any.
    pub center_place: Option<u32>,
    /// `global`, `state`, or `culture`.
    pub expansion: &'static str,
    pub expansionism: f64,
    /// A color as `#rrggbb`. Data only.
    pub color: String,
    pub code: String,
    /// The religions that this religion comes from. 0: its culture.
    pub origins: Vec<u32>,
}

/// The religions of a world.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Religions {
    pub list: Vec<Religion>,
    /// The religion of each cell. 0: no religion.
    pub cell_religion: Vec<u32>,
}

/// What the religion stage reads.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub biomes: &'a [u8],
    pub society: &'a Society,
    pub roads: &'a [Road],
    pub sea_routes: &'a [Road],
}

const NUMBER: [&str; 12] = [
    "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten", "Eleven",
    "Twelve",
];
const BEING: [&str; 39] = [
    "Ancestor",
    "Ancient",
    "Avatar",
    "Brother",
    "Champion",
    "Chief",
    "Council",
    "Creator",
    "Deity",
    "Divine One",
    "Elder",
    "Enlightened Being",
    "Father",
    "Forebear",
    "Forefather",
    "Giver",
    "God",
    "Goddess",
    "Guardian",
    "Guide",
    "Hierarch",
    "Lady",
    "Lord",
    "Maker",
    "Master",
    "Mother",
    "Numen",
    "Oracle",
    "Overlord",
    "Protector",
    "Reaper",
    "Ruler",
    "Sage",
    "Seer",
    "Sister",
    "Spirit",
    "Supreme Being",
    "Transcendent",
    "Virgin",
];
const ANIMAL: [&str; 85] = [
    "Antelope",
    "Ape",
    "Badger",
    "Basilisk",
    "Bear",
    "Beaver",
    "Bison",
    "Boar",
    "Buffalo",
    "Camel",
    "Cat",
    "Centaur",
    "Cerberus",
    "Chimera",
    "Cobra",
    "Cockatrice",
    "Crane",
    "Crocodile",
    "Crow",
    "Cyclope",
    "Deer",
    "Dog",
    "Direwolf",
    "Drake",
    "Dragon",
    "Eagle",
    "Elephant",
    "Elk",
    "Falcon",
    "Fox",
    "Goat",
    "Goose",
    "Gorgon",
    "Gryphon",
    "Hare",
    "Hawk",
    "Heron",
    "Hippogriff",
    "Horse",
    "Hound",
    "Hyena",
    "Ibis",
    "Jackal",
    "Jaguar",
    "Kitsune",
    "Kraken",
    "Lark",
    "Leopard",
    "Lion",
    "Manticore",
    "Mantis",
    "Marten",
    "Minotaur",
    "Moose",
    "Mule",
    "Narwhal",
    "Owl",
    "Ox",
    "Panther",
    "Pegasus",
    "Phoenix",
    "Python",
    "Rat",
    "Raven",
    "Roc",
    "Rook",
    "Scorpion",
    "Serpent",
    "Shark",
    "Sheep",
    "Snake",
    "Sphinx",
    "Spider",
    "Swan",
    "Tiger",
    "Turtle",
    "Unicorn",
    "Viper",
    "Vulture",
    "Walrus",
    "Wolf",
    "Wolverine",
    "Worm",
    "Wyvern",
    "Yeti",
];
const ADJECTIVE: [&str; 110] = [
    "Aggressive",
    "Almighty",
    "Ancient",
    "Beautiful",
    "Benevolent",
    "Big",
    "Blind",
    "Blond",
    "Bloody",
    "Brave",
    "Broken",
    "Brutal",
    "Burning",
    "Calm",
    "Celestial",
    "Cheerful",
    "Crazy",
    "Cruel",
    "Dead",
    "Deadly",
    "Devastating",
    "Distant",
    "Disturbing",
    "Divine",
    "Dying",
    "Eternal",
    "Ethernal",
    "Empyreal",
    "Enigmatic",
    "Enlightened",
    "Evil",
    "Explicit",
    "Fair",
    "Far",
    "Fat",
    "Fatal",
    "Favorable",
    "Flying",
    "Friendly",
    "Frozen",
    "Giant",
    "Good",
    "Grateful",
    "Great",
    "Happy",
    "High",
    "Holy",
    "Honest",
    "Huge",
    "Hungry",
    "Illustrious",
    "Immutable",
    "Ineffable",
    "Infallible",
    "Inherent",
    "Last",
    "Latter",
    "Lost",
    "Loud",
    "Lucky",
    "Mad",
    "Magical",
    "Main",
    "Major",
    "Marine",
    "Mythical",
    "Mystical",
    "Naval",
    "New",
    "Noble",
    "Old",
    "Otherworldly",
    "Patient",
    "Peaceful",
    "Pregnant",
    "Prime",
    "Proud",
    "Pure",
    "Radiant",
    "Resplendent",
    "Sacred",
    "Sacrosanct",
    "Sad",
    "Scary",
    "Secret",
    "Selected",
    "Serene",
    "Severe",
    "Silent",
    "Sleeping",
    "Slumbering",
    "Sovereign",
    "Strong",
    "Sunny",
    "Superior",
    "Supernatural",
    "Sustainable",
    "Transcendent",
    "Transcendental",
    "Troubled",
    "Unearthly",
    "Unfathomable",
    "Unhappy",
    "Unknown",
    "Unseen",
    "Waking",
    "Wild",
    "Wise",
    "Worried",
    "Young",
];
const GENITIVE: [&str; 28] = [
    "Cold",
    "Day",
    "Death",
    "Doom",
    "Fate",
    "Fire",
    "Fog",
    "Frost",
    "Gates",
    "Heaven",
    "Home",
    "Ice",
    "Justice",
    "Life",
    "Light",
    "Lightning",
    "Love",
    "Nature",
    "Night",
    "Pain",
    "Snow",
    "Springs",
    "Summer",
    "Thunder",
    "Time",
    "Victory",
    "War",
    "Winter",
];
const THE_GENITIVE: [&str; 24] = [
    "Abyss",
    "Blood",
    "Dawn",
    "Earth",
    "East",
    "Eclipse",
    "Fall",
    "Harvest",
    "Moon",
    "North",
    "Peak",
    "Rainbow",
    "Sea",
    "Sky",
    "South",
    "Stars",
    "Storm",
    "Sun",
    "Tree",
    "Underworld",
    "West",
    "Wild",
    "Word",
    "World",
];
const COLOR: [&str; 29] = [
    "Amber",
    "Black",
    "Blue",
    "Bright",
    "Bronze",
    "Brown",
    "Coral",
    "Crimson",
    "Dark",
    "Emerald",
    "Golden",
    "Green",
    "Grey",
    "Indigo",
    "Lavender",
    "Light",
    "Magenta",
    "Maroon",
    "Orange",
    "Pink",
    "Plum",
    "Purple",
    "Red",
    "Ruby",
    "Sapphire",
    "Teal",
    "Turquoise",
    "White",
    "Yellow",
];

/// Name approaches for the meaning of a deity, with weights.
const APPROACHES: [(&str, u32); 14] = [
    ("Number", 1),
    ("Being", 3),
    ("Adjective", 5),
    ("Color + Animal", 5),
    ("Adjective + Animal", 5),
    ("Adjective + Being", 5),
    ("Adjective + Genitive", 1),
    ("Color + Being", 3),
    ("Color + Genitive", 3),
    ("Being + of + Genitive", 2),
    ("Being + of the + Genitive", 1),
    ("Animal + of + Genitive", 1),
    ("Adjective + Being + of + Genitive", 2),
    ("Adjective + Animal + of + Genitive", 2),
];

type Weights = &'static [(&'static str, u32)];

fn forms(kind: &str) -> Weights {
    match kind {
        "Folk" => &[
            ("Shamanism", 4),
            ("Animism", 4),
            ("Polytheism", 4),
            ("Ancestor Worship", 2),
            ("Nature Worship", 1),
            ("Totemism", 1),
        ],
        "Organized" => &[
            ("Polytheism", 7),
            ("Monotheism", 7),
            ("Dualism", 3),
            ("Syncretism", 2),
            ("Pantheism", 2),
            ("Non-theism", 2),
            ("Philosophical", 1),
            ("Ethical", 1),
            ("Deism", 1),
            ("Henotheism", 1),
        ],
        "Cult" => &[("Cult", 5), ("Dark Cult", 5), ("Sect", 1)],
        _ => &[("Heresy", 1)],
    }
}

fn naming_methods(form: &str) -> Weights {
    match form {
        "Shamanism" | "Animism" | "Ancestor Worship" | "Nature Worship" | "Totemism" => {
            &[("Culture + type", 1)]
        }
        "Polytheism" => &[
            ("Random + type", 4),
            ("Random + ism", 1),
            ("Place + ism", 1),
            ("Culture + ism", 2),
            ("Place + ian + type", 6),
            ("Culture + type", 4),
            ("Type + of the + meaning", 2),
        ],
        "Monotheism" => &[
            ("Random + type", 2),
            ("Random + ism", 1),
            ("Supreme + ism", 6),
            ("Faith of + Supreme", 6),
            ("Place + ism", 1),
            ("Culture + ism", 1),
            ("Place + ian + type", 4),
            ("Culture + type", 3),
        ],
        "Dualism" => &[
            ("Random + type", 3),
            ("Random + ism", 1),
            ("Place + ism", 1),
            ("Culture + ism", 2),
            ("Place + ian + type", 5),
            ("Culture + type", 4),
            ("Type + of the + meaning", 3),
        ],
        "Syncretism" => &[
            ("Random + type", 3),
            ("Random + ism", 2),
            ("Place + ism", 1),
            ("Culture + ism", 2),
            ("Place + ian + type", 5),
            ("Culture + type", 4),
            ("Type + of the + meaning", 2),
        ],
        "Pantheism" => &[
            ("Random + type", 3),
            ("Random + ism", 1),
            ("Place + ism", 1),
            ("Culture + ism", 2),
            ("Place + ian + type", 5),
            ("Culture + type", 4),
            ("Type + of the + meaning", 2),
        ],
        "Non-theism" | "Philosophical" | "Ethical" => &[
            ("Random + type", 3),
            ("Random + ism", 2),
            ("Place + ism", 1),
            ("Culture + ism", 3),
            ("Place + ian + type", 4),
            ("Culture + type", 5),
        ],
        "Deism" => &[
            ("Random + type", 2),
            ("Random + ism", 1),
            ("Supreme + ism", 3),
            ("Faith of + Supreme", 3),
            ("Place + ism", 1),
            ("Culture + ism", 2),
            ("Place + ian + type", 5),
            ("Culture + type", 4),
        ],
        "Henotheism" => &[
            ("Random + type", 2),
            ("Random + ism", 1),
            ("Supreme + ism", 2),
            ("Faith of + Supreme", 2),
            ("Place + ism", 1),
            ("Culture + ism", 2),
            ("Place + ian + type", 6),
            ("Culture + type", 4),
            ("Type + of the + meaning", 2),
        ],
        "Cult" | "Sect" => &[
            ("Burg + ian + type", 2),
            ("Random + ian + type", 1),
            ("Type + of the + meaning", 2),
        ],
        "Dark Cult" => &[
            ("Burg + ian + type", 2),
            ("Random + ian + type", 1),
            ("Type + of the + meaning", 3),
        ],
        _ => &[
            ("Burg + ian + type", 3),
            ("Random + ism", 3),
            ("Random + ian + type", 2),
            ("Type + of the + meaning", 1),
        ],
    }
}

fn types(form: &str) -> Weights {
    match form {
        "Shamanism" => &[
            ("Beliefs", 3),
            ("Shamanism", 1),
            ("Druidism", 1),
            ("Spirits", 1),
        ],
        "Animism" => &[("Spirits", 3), ("Beliefs", 1)],
        "Polytheism" => &[("Pantheon", 3), ("Deities", 2), ("Gods", 1)],
        "Ancestor Worship" => &[("Beliefs", 1), ("Forefathers", 2), ("Ancestors", 2)],
        "Nature Worship" => &[("Beliefs", 3), ("Druids", 1), ("Spirits", 1)],
        "Totemism" => &[("Beliefs", 2), ("Totems", 2), ("Idols", 1)],
        "Monotheism" => &[
            ("Religion", 2),
            ("Church", 3),
            ("Faith", 1),
            ("Creed", 1),
            ("Commandments", 1),
        ],
        "Dualism" => &[("Religion", 3), ("Faith", 1)],
        "Syncretism" => &[("Faith", 2), ("Religion", 2), ("Beliefs", 1)],
        "Pantheism" => &[("Religion", 1), ("Faith", 1)],
        "Non-theism" => &[("Beliefs", 3), ("Spirits", 1)],
        "Philosophical" => &[
            ("Philosophy", 3),
            ("Doctrine", 2),
            ("School", 1),
            ("Way", 1),
        ],
        "Ethical" => &[
            ("Ethics", 2),
            ("Doctrine", 2),
            ("Path", 2),
            ("Way", 2),
            ("Precepts", 1),
        ],
        "Deism" => &[("Faith", 2), ("Religion", 1), ("Church", 1), ("Creed", 1)],
        "Henotheism" => &[("Faith", 2), ("Religion", 1), ("Gods", 1), ("Pantheon", 1)],
        "Cult" => &[
            ("Cult", 4),
            ("Sect", 2),
            ("Arcanum", 1),
            ("Order", 1),
            ("Worship", 1),
            ("Mysticism", 1),
        ],
        "Dark Cult" => &[
            ("Cult", 2),
            ("Blasphemy", 1),
            ("Circle", 1),
            ("Coven", 1),
            ("Idols", 1),
            ("Occultism", 1),
        ],
        "Sect" => &[
            ("Sect", 4),
            ("Society", 1),
            ("Brotherhood", 1),
            ("Circle", 1),
            ("Way", 1),
        ],
        _ => &[
            ("Heresy", 3),
            ("Sect", 2),
            ("Apostates", 1),
            ("Brotherhood", 1),
            ("Circle", 1),
            ("Dissent", 1),
            ("Dissenters", 1),
            ("Iconoclasm", 1),
            ("Schism", 1),
            ("Society", 1),
        ],
    }
}

/// The expansionism of a new religion (`expansionismMap`).
fn expansionism(kind: &str, rng: &mut Rand) -> f64 {
    match kind {
        "Folk" => 0.0,
        "Organized" => rng.gauss(5.0, 3.0, 0.0, 10.0, 1),
        "Cult" => rng.gauss(0.5, 0.5, 0.0, 5.0, 1),
        _ => rng.gauss(1.0, 0.5, 0.0, 5.0, 1),
    }
}

/// A meaning for a deity name (`generateMeaning`).
fn meaning(rng: &mut Rand) -> String {
    let a = rng.weighted(&APPROACHES);
    match a {
        "Number" => rng.pick(&NUMBER).to_string(),
        "Being" => rng.pick(&BEING).to_string(),
        "Adjective" => rng.pick(&ADJECTIVE).to_string(),
        "Color + Animal" => format!("{} {}", rng.pick(&COLOR), rng.pick(&ANIMAL)),
        "Adjective + Animal" => format!("{} {}", rng.pick(&ADJECTIVE), rng.pick(&ANIMAL)),
        "Adjective + Being" => format!("{} {}", rng.pick(&ADJECTIVE), rng.pick(&BEING)),
        "Adjective + Genitive" => format!("{} {}", rng.pick(&ADJECTIVE), rng.pick(&GENITIVE)),
        "Color + Being" => format!("{} {}", rng.pick(&COLOR), rng.pick(&BEING)),
        "Color + Genitive" => format!("{} {}", rng.pick(&COLOR), rng.pick(&GENITIVE)),
        "Being + of + Genitive" => format!("{} of {}", rng.pick(&BEING), rng.pick(&GENITIVE)),
        "Being + of the + Genitive" => {
            format!("{} of the {}", rng.pick(&BEING), rng.pick(&THE_GENITIVE))
        }
        "Animal + of + Genitive" => format!("{} of {}", rng.pick(&ANIMAL), rng.pick(&GENITIVE)),
        "Adjective + Being + of + Genitive" => format!(
            "{} {} of {}",
            rng.pick(&ADJECTIVE),
            rng.pick(&BEING),
            rng.pick(&GENITIVE)
        ),
        _ => format!(
            "{} {} of {}",
            rng.pick(&ADJECTIVE),
            rng.pick(&ANIMAL),
            rng.pick(&GENITIVE)
        ),
    }
}

/// A religion before it has an id.
struct Draft {
    kind: &'static str,
    form: String,
    culture: u32,
    center: usize,
}

struct Ctx<'a> {
    input: &'a Input<'a>,
    chains: Vec<Chain>,
    /// The place on each cell, or 0.
    burg: Vec<u32>,
    /// Route links of each cell: neighbor cell and route group.
    links: Vec<BTreeMap<usize, &'static str>>,
}

impl Ctx<'_> {
    fn culture_name(&self, culture: u32, rng: &mut Rand) -> String {
        let chain = &self.chains[culture as usize];
        let base = crate::names::data::NAME_BASES[chain.base_index()].min;
        let max = crate::names::data::NAME_BASES[chain.base_index()].max;
        chain.get_base(rng, base, max, None)
    }

    /// `getDeityName`.
    fn deity(&self, culture: u32, rng: &mut Rand) -> String {
        let m = meaning(rng);
        let name = self.culture_name(culture, rng);
        format!("{name}, The {m}")
    }

    /// `generateReligionName`: the name and the expansion. inkland cultures
    /// have no ancestry, so the folk rule for an ancestry does not apply.
    fn name(
        &self,
        form: &str,
        deity: &str,
        center: usize,
        rng: &mut Rand,
    ) -> (String, &'static str) {
        let soc = self.input.society;
        let culture_id = soc.cell_culture[center];
        let kind = rng.weighted(types(form));
        let supreme = deity.split([' ', ',']).next().unwrap_or("").to_owned();
        let culture = soc
            .cultures
            .get((culture_id as usize).wrapping_sub(1))
            .map_or_else(|| "Wildlands".to_owned(), |c| c.name.clone());
        let place = |adj: bool, rng: &mut Rand| -> String {
            let burg = self.burg[center];
            let base = if burg != NONE {
                soc.places[burg as usize - 1].name.clone()
            } else {
                let st = soc.cell_state[center];
                soc.states
                    .get((st as usize).wrapping_sub(1))
                    .map_or_else(|| "Neutrals".to_owned(), |s| s.name.clone())
            };
            let first = base.split([' ', ',']).next().unwrap_or("").to_owned();
            let name = trim_vowels(&first, 3);
            if adj { adjective(&name, rng) } else { name }
        };
        let m = rng.weighted(naming_methods(form));
        match m {
            "Random + type" => (
                format!("{} {kind}", self.culture_name(culture_id, rng)),
                "global",
            ),
            "Random + ism" => (
                format!("{}ism", trim_vowels(&self.culture_name(culture_id, rng), 3)),
                "global",
            ),
            "Supreme + ism" if !deity.is_empty() => {
                (format!("{}ism", trim_vowels(&supreme, 3)), "global")
            }
            "Faith of + Supreme" if !deity.is_empty() => {
                let w = rng.pick(&["Faith", "Way", "Path", "Word", "Witnesses"]);
                (format!("{w} of {supreme}"), "global")
            }
            "Place + ism" => (format!("{}ism", place(false, rng)), "state"),
            "Culture + ism" => (format!("{}ism", trim_vowels(&culture, 3)), "culture"),
            "Place + ian + type" => (format!("{} {kind}", place(true, rng)), "state"),
            "Culture + type" => (format!("{culture} {kind}"), "culture"),
            "Burg + ian + type" => (format!("{} {kind}", place(true, rng)), "global"),
            "Random + ian + type" => {
                let r = self.culture_name(culture_id, rng);
                (format!("{} {kind}", adjective(&r, rng)), "global")
            }
            "Type + of the + meaning" => (format!("{kind} of the {}", meaning(rng)), "global"),
            _ => (
                format!("{}ism", trim_vowels(&self.culture_name(culture_id, rng), 3)),
                "global",
            ),
        }
    }

    /// `getPassageCost`.
    fn passage(&self, cell: usize, next: usize) -> f64 {
        let route = self.links[cell].get(&next);
        if !self.input.society.cells.land(cell) {
            return if route.is_some() { 50.0 } else { 500.0 };
        }
        let biome = BIOME_COST[self.input.biomes[next] as usize];
        match route {
            None => biome,
            Some(&"roads") => 1.0,
            Some(_) => biome / 3.0,
        }
    }
}

/// `wouldCreateOriginCycle`.
fn cycle(id: u32, origin: u32, list: &[Religion]) -> bool {
    let mut queue = VecDeque::from([origin]);
    let mut checked = BTreeSet::new();
    while let Some(cur) = queue.pop_front() {
        if cur == id {
            return true;
        }
        if cur == 0 || !checked.insert(cur) {
            continue;
        }
        if let Some(r) = list.get(cur as usize - 1) {
            queue.extend(r.origins.iter().copied());
        }
    }
    false
}

/// Make the religions (`generate`).
pub fn build(input: &Input) -> Religions {
    let soc = input.society;
    let grid = input.grid;
    let cells = &soc.cells;
    let n = grid.len();
    let tree = SeedTree::new(input.config.seed);
    let mut rng = Rand::new(tree.part_rng(Layer::Religions, "generate"));
    let mut burg = vec![NONE; n];
    for p in &soc.places {
        burg[p.cell] = p.id;
    }
    let mut links: Vec<BTreeMap<usize, &'static str>> = vec![BTreeMap::new(); n];
    for r in input.roads.iter().chain(input.sea_routes) {
        for w in r.cells.windows(2) {
            if w[0] != w[1] {
                links[w[0]].insert(w[1], r.group);
                links[w[1]].insert(w[0], r.group);
            }
        }
    }
    let ctx = Ctx {
        input,
        chains: std::iter::once(Chain::new(WILDLANDS_BASE))
            .chain(soc.cultures.iter().map(|c| Chain::new(c.base)))
            .collect(),
        burg,
        links,
    };
    // inkland cultures have no colors; each culture gets a rainbow color
    // here, as a base for the colors of its religions.
    let culture_colors: Vec<String> = soc
        .cultures
        .iter()
        .map(|_| random_color(&mut rng))
        .collect();

    // Folk religions.
    let mut drafts: Vec<Draft> = soc
        .cultures
        .iter()
        .map(|c| Draft {
            kind: "Folk",
            form: rng.weighted(forms("Folk")).to_owned(),
            culture: c.id,
            center: c.center,
        })
        .collect();

    // Organized religions and cults.
    let limit = input.config.world.religions as usize;
    if limit > 0 {
        let candidates: Vec<usize> = if soc.places.len() >= limit {
            // FMG sorts by burg population, which is not set yet at this
            // stage, so the burg order stays.
            soc.places.iter().map(|p| p.cell).collect()
        } else {
            let mut c: Vec<usize> = (0..n).filter(|&i| cells.s[i] > 2.0).collect();
            c.sort_by(|&a, &b| cells.s[b].total_cmp(&cells.s[a]));
            c
        };
        let spacing = (input.config.frame.width + input.config.frame.height) / 2.0 / limit as f64;
        let mut cores: Vec<usize> = Vec::new();
        for c in candidates {
            let p = grid.points[c];
            let near = cores.iter().any(|&q| {
                let d = grid.points[q];
                (d[0] - p[0]).powi(2) + (d[1] - p[1]).powi(2) < spacing * spacing
            });
            if !near {
                cores.push(c);
                if cores.len() == limit {
                    break;
                }
            }
        }
        let cults = (rng.rand(1, 4) as f64 / 10.0 * cores.len() as f64).floor() as usize;
        let organized = cores.len() - cults;
        for (k, c) in cores.into_iter().enumerate() {
            let kind = if k < organized { "Organized" } else { "Cult" };
            drafts.push(Draft {
                kind,
                form: rng.weighted(forms(kind)).to_owned(),
                culture: soc.cell_culture[c],
                center: c,
            });
        }
    }

    // Names, deities, expansion, and colors (`specifyReligions`).
    let mut list: Vec<Religion> = Vec::new();
    for d in &drafts {
        let supreme = ctx.deity(d.culture, &mut rng);
        let deity = (d.form != "Non-theism" && d.form != "Animism").then(|| supreme.clone());
        let (name, mut expansion) = ctx.name(&d.form, &supreme, d.center, &mut rng);
        if expansion == "state" && soc.cell_state[d.center] == NONE {
            expansion = "global";
        }
        let expansionism = expansionism(d.kind, &mut rng);
        let color = match soc.cultures.get((d.culture as usize).wrapping_sub(1)) {
            None => random_color(&mut rng),
            Some(c) => {
                let base = &culture_colors[c.id as usize - 1];
                match d.kind {
                    "Folk" => base.clone(),
                    "Cult" => mixed_color(base, &mut rng, 0.5, 0.0),
                    _ => mixed_color(base, &mut rng, 0.25, 0.4),
                }
            }
        };
        list.push(Religion {
            id: list.len() as u32 + 1,
            name,
            kind: d.kind,
            form: d.form.clone(),
            deity,
            culture: d.culture,
            center: d.center,
            center_place: None,
            expansion,
            expansionism,
            color,
            code: String::new(),
            origins: Vec::new(),
        });
    }
    // "Old" for a folk religion with an organized rival in its culture, and
    // codes (`combineReligions`).
    let mut codes: Vec<String> = Vec::new();
    for k in 0..list.len() {
        if list[k].kind == "Folk" {
            let culture = list[k].culture;
            let rival = list
                .iter()
                .any(|r| r.culture == culture && r.kind == "Organized" && r.expansion == "culture");
            if rival && !list[k].name.starts_with("Old") {
                list[k].name = format!("Old {}", list[k].name);
            }
        }
        let code = abbreviate(&list[k].name, &codes);
        codes.push(code.clone());
        list[k].code = code;
    }

    // Folk religions on their cultures, then the flood (`expandReligions`).
    let mut religion: Vec<u32> = vec![NONE; n];
    let folk_of: BTreeMap<u32, u32> = list
        .iter()
        .filter(|r| r.kind == "Folk")
        .map(|r| (r.culture, r.id))
        .collect();
    for c in 0..n {
        religion[c] = folk_of.get(&soc.cell_culture[c]).copied().unwrap_or(NONE);
    }
    let max_cost = n as f64 / 20.0 * input.config.world.growth_rate;
    {
        let mut cost = vec![0.0f64; n];
        let mut heap = BinaryHeap::new();
        let mut seq = 0u64;
        for r in list
            .iter()
            .filter(|r| r.kind != "Folk" && r.kind != "Heresy")
        {
            religion[r.center] = r.id;
            heap.push(Reverse((
                Key(0.0, seq),
                r.center,
                r.id,
                soc.cell_state[r.center],
            )));
            seq += 1;
            cost[r.center] = 1.0;
        }
        while let Some(Reverse((Key(p, _), cell, id, state))) = heap.pop() {
            let r = &list[id as usize - 1];
            for next in grid.neighbors(cell) {
                if r.expansion == "culture" && r.culture != soc.cell_culture[next] {
                    continue;
                }
                if r.expansion == "state" && state != soc.cell_state[next] {
                    continue;
                }
                let culture_cost = if r.culture != soc.cell_culture[next] {
                    10.0
                } else {
                    0.0
                };
                let state_cost = if state != soc.cell_state[next] {
                    10.0
                } else {
                    0.0
                };
                let total = p
                    + 10.0
                    + (culture_cost + state_cost + ctx.passage(cell, next)) / r.expansionism;
                if total > max_cost {
                    continue;
                }
                if cost[next] == 0.0 || total < cost[next] {
                    if soc.cell_culture[next] != NONE {
                        religion[next] = id;
                    }
                    cost[next] = total;
                    heap.push(Reverse((Key(total, seq), next, id, state)));
                    seq += 1;
                }
            }
        }
    }

    // Origins (`defineOrigins`).
    for k in 0..list.len() {
        let (id, kind, culture, expansion, center) = (
            list[k].id,
            list[k].kind,
            list[k].culture,
            list[k].expansion,
            list[k].center,
        );
        if kind == "Folk" {
            list[k].origins = vec![0];
            continue;
        }
        let folk = list
            .iter()
            .find(|r| r.kind == "Folk" && r.culture == culture)
            .map(|r| r.id);
        if let Some(f) = folk
            && culture != NONE
            && expansion == "culture"
            && center % 2 == 0
        {
            list[k].origins = vec![f];
            continue;
        }
        let (cluster, max_religions) = if kind == "Organized" {
            (100, 2)
        } else {
            (50, 3)
        };
        let fallback = folk.unwrap_or(0);
        let mut found: Vec<u32> = Vec::new();
        let mut queue = VecDeque::from([center]);
        let mut checked = vec![false; n];
        let mut size = 0;
        'search: while let Some(c) = queue.pop_front() {
            if size >= cluster {
                break;
            }
            size += 1;
            checked[c] = true;
            for m in grid.neighbors(c) {
                if checked[m] {
                    continue;
                }
                checked[m] = true;
                let r = religion[m];
                if r != NONE && r < id && !cycle(id, r, &list) && !found.contains(&r) {
                    found.push(r);
                }
                if found.len() >= max_religions {
                    break 'search;
                }
                queue.push_back(m);
            }
        }
        list[k].origins = if !found.is_empty() {
            found
        } else if cycle(id, fallback, &list) {
            vec![0]
        } else {
            vec![fallback]
        };
    }

    // Heresies (`generateHeresies`).
    let parents: Vec<usize> = (0..list.len())
        .filter(|&k| list[k].kind == "Organized" && list[k].expansionism >= 3.0)
        .collect();
    let mut heresies: Vec<Religion> = Vec::new();
    if !parents.is_empty() {
        let mut boundary: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for c in 0..n {
            let r = religion[c];
            if !parents.iter().any(|&k| list[k].id == r) {
                continue;
            }
            if grid.neighbors(c).any(|m| religion[m] != r) {
                boundary.entry(r).or_default().push(c);
            }
        }
        let mut occupied: BTreeSet<usize> = list.iter().map(|r| r.center).collect();
        for &k in &parents {
            let parent = list[k].clone();
            let mut candidates: Vec<usize> = boundary
                .get(&parent.id)
                .map(|v| {
                    v.iter()
                        .copied()
                        .filter(|c| !occupied.contains(c))
                        .collect()
                })
                .unwrap_or_default();
            let count = rng.gauss(0.0, 1.0, 0.0, 3.0, 0);
            let mut index = 0.0;
            while index < count && !candidates.is_empty() {
                let pick = rng.rand(0, candidates.len() as i64 - 1) as usize;
                let center = candidates.remove(pick);
                let (name, expansion) = ctx.name(
                    "Heresy",
                    parent.deity.as_deref().unwrap_or(""),
                    center,
                    &mut rng,
                );
                let color = mixed_color(&parent.color, &mut rng, 0.4, 0.2);
                let code = abbreviate(&name, &codes);
                codes.push(code.clone());
                occupied.insert(center);
                heresies.push(Religion {
                    id: (list.len() + heresies.len()) as u32 + 1,
                    name,
                    kind: "Heresy",
                    form: parent.form.clone(),
                    deity: parent.deity.clone(),
                    culture: soc.cell_culture[center],
                    center,
                    center_place: None,
                    expansion,
                    expansionism: expansionism("Heresy", &mut rng),
                    color,
                    code,
                    origins: vec![parent.id],
                });
                index += 1.0;
            }
        }
    }
    list.extend(heresies.iter().cloned());

    // Heresies grow inside their parent (`expandHeresies`).
    if !heresies.is_empty() {
        let mut cost = vec![0.0f64; n];
        let mut heap = BinaryHeap::new();
        let mut seq = 0u64;
        for h in &heresies {
            religion[h.center] = h.id;
            heap.push(Reverse((Key(0.0, seq), h.center, h.id, h.origins[0])));
            seq += 1;
            cost[h.center] = 1.0;
        }
        while let Some(Reverse((Key(p, _), cell, id, parent))) = heap.pop() {
            let ex = list[id as usize - 1].expansionism.max(0.1);
            for next in grid.neighbors(cell) {
                let religion_cost = if religion[next] == parent {
                    0.0
                } else {
                    2000.0
                };
                let total = p + 10.0 + (religion_cost + ctx.passage(cell, next)) / ex;
                if total > max_cost {
                    continue;
                }
                if cost[next] == 0.0 || total < cost[next] {
                    if soc.cell_culture[next] != NONE {
                        religion[next] = id;
                    }
                    cost[next] = total;
                    heap.push(Reverse((Key(total, seq), next, id, parent)));
                    seq += 1;
                }
            }
        }
    }

    // Centers in their areas (`checkCenters`).
    for r in &mut list {
        if religion[r.center] == r.id {
            continue;
        }
        if let Some(first) = (0..n).find(|&c| religion[c] == r.id) {
            r.center = first;
        } else if r.kind == "Folk"
            && let Some(c) = soc.cultures.get((r.culture as usize).wrapping_sub(1))
        {
            r.center = c.center;
        }
    }
    for r in &mut list {
        r.center_place = (ctx.burg[r.center] != NONE).then_some(ctx.burg[r.center]);
    }

    Religions {
        list,
        cell_religion: religion,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    fn world(religions: u32) -> World {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 3, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "continents", "religions": religions }
        }))
        .unwrap();
        World::build(&c).unwrap()
    }

    #[test]
    fn religions_follow_the_rules_of_fmg() {
        let w = world(5);
        let r = &w.religions;
        let ids: Vec<u32> = r.list.iter().map(|x| x.id).collect();
        assert_eq!(ids, (1..=r.list.len() as u32).collect::<Vec<_>>());
        // One folk religion for each culture.
        for c in &w.society.cultures {
            let folk: Vec<_> = r
                .list
                .iter()
                .filter(|x| x.kind == "Folk" && x.culture == c.id)
                .collect();
            assert_eq!(folk.len(), 1, "{}", c.name);
            assert_eq!(folk[0].origins, vec![0]);
        }
        let organized = r
            .list
            .iter()
            .filter(|x| x.kind == "Organized" || x.kind == "Cult")
            .count();
        assert!((1..=5).contains(&organized));
        for x in &r.list {
            assert!(["Folk", "Organized", "Cult", "Heresy"].contains(&x.kind));
            assert!(["global", "state", "culture"].contains(&x.expansion));
            assert!(x.color.starts_with('#') && x.color.len() == 7);
            assert!(!x.code.is_empty());
            assert!(x.origins.iter().all(|&o| o < x.id));
            if x.form == "Non-theism" || x.form == "Animism" {
                assert!(x.deity.is_none());
            }
            if x.kind == "Heresy" {
                let parent = &r.list[x.origins[0] as usize - 1];
                assert_eq!(parent.kind, "Organized");
            }
        }
        // Water and wildland have no religion; each religion with cells
        // holds its center.
        for c in 0..w.grid.len() {
            if w.society.cell_culture[c] == 0 {
                let rel = r.cell_religion[c];
                // Only the center cell of a religion can be outside the cultures.
                assert!(
                    rel == 0 || r.list.iter().any(|x| x.center == c && x.id == rel),
                    "cell {c}"
                );
            }
        }
        for x in &r.list {
            if r.cell_religion.contains(&x.id) {
                assert_eq!(r.cell_religion[x.center], x.id, "{}", x.name);
            }
        }
    }

    #[test]
    fn no_organized_religion_when_the_count_is_zero() {
        let w = world(0);
        assert!(w.religions.list.iter().all(|x| x.kind == "Folk"));
    }
}
