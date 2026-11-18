//! Markers (`story/markers-generator.ts`): points of interest as world
//! data.
//!
//! Each marker kind has a list of candidate cells, a minimum candidate
//! count, a rate (`each`), and a multiplier. The count of a kind is
//! `ceil(candidates / each * multiplier)`, and 0 when there are fewer than
//! `min / multiplier` candidates. Markers are drawn at random from the
//! candidates, one marker for each cell. Each marker gets a name and a
//! legend from the FMG word lists (`data/markers.ts`, MIT license, see
//! THIRD_PARTY_NOTICES.md).
//!
//! inkland: no marker goes on the cell of a pinned place. The kinds that
//! need the FMG economy (mines, guild halls, merchant houses, customs
//! houses) are not made. The kinds that FMG weighs by trade use an even
//! draw, as FMG does when there is no trade. Legends are plain text: a
//! legend with a web page in FMG (dungeons) gives the address, and the
//! encounters use the offline text of FMG. The portals, rifts, and
//! disturbed burials come only with a fantasy culture set, as in FMG.

use crate::geonames::Names;
use crate::grid::Grid;
use crate::names::{Chain, adjective, capitalize};
use crate::rand::{Rand, rn};
use crate::religions::Religions;
use crate::rivers::River;
use crate::roads::Road;
use crate::society::{NONE, Society};
use std::collections::BTreeMap;

/// One marker.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    /// 1, 2, 3, ...
    pub id: u32,
    /// The FMG type, for example `volcanoes`.
    pub kind: &'static str,
    /// The name of the icon. FMG draws an emoji; inkland keeps a name.
    pub icon: &'static str,
    pub cell: usize,
    /// The position: the place on the cell, else the cell site.
    pub at: [f64; 2],
    pub name: Option<String>,
    pub legend: Option<String>,
}

/// What the marker stage reads.
pub struct Input<'a> {
    pub config: &'a crate::config::Config,
    pub grid: &'a Grid,
    pub features: &'a crate::features::Features,
    pub biomes: &'a [u8],
    pub temperature: &'a [f64],
    pub rivers: &'a [River],
    pub names: &'a Names,
    pub society: &'a Society,
    pub religions: &'a Religions,
    pub roads: &'a [Road],
    pub sea_routes: &'a [Road],
    pub pin_cells: &'a [usize],
}

const BRIDGE_ADJECTIVES: &[(&str, u32)] = &[
    ("stone", 10),
    ("wooden", 1),
    ("lengthy", 2),
    ("formidable", 2),
    ("rickety", 1),
    ("beaten", 1),
    ("weathered", 1),
];
const BRIGAND_ANIMALS: &[&str] = &[
    "Apes",
    "Badgers",
    "Bears",
    "Beavers",
    "Bisons",
    "Boars",
    "Cats",
    "Crows",
    "Dogs",
    "Foxes",
    "Hares",
    "Hawks",
    "Hyenas",
    "Jackals",
    "Jaguars",
    "Leopards",
    "Lions",
    "Owls",
    "Panthers",
    "Rats",
    "Ravens",
    "Rooks",
    "Scorpions",
    "Sharks",
    "Snakes",
    "Spiders",
    "Tigers",
    "Wolfs",
    "Wolverines",
    "Falcons",
];
const BRIGAND_TYPES: &[(&str, u32)] = &[
    ("brigands", 4),
    ("bandits", 3),
    ("robbers", 1),
    ("highwaymen", 1),
];
const CAVE_FORMATIONS: &[(&str, u32)] = &[
    ("Cave", 10),
    ("Cavern", 8),
    ("Chasm", 6),
    ("Ravine", 6),
    ("Fracture", 5),
    ("Grotto", 4),
    ("Pit", 4),
    ("Sinkhole", 2),
    ("Hole", 2),
];
const CAVE_STATUSES: &[(&str, u32)] = &[
    ("a good spot to hid treasure", 5),
    ("the home of strange monsters", 5),
    ("totally empty", 4),
    ("endlessly deep and unexplored", 4),
    ("completely flooded", 2),
    ("slowly filling with lava", 1),
];
const CIRCUS_ADJECTIVES: &[&str] = &[
    "Fantastical",
    "Wondrous",
    "Incomprehensible",
    "Magical",
    "Extraordinary",
    "Unmissable",
    "World-famous",
    "Breathtaking",
];
const DANCE_GUESTS: &[&str] = &[
    "great and the good",
    "nobility",
    "local elders",
    "foreign dignitaries",
    "spiritual leaders",
    "suspected revolutionaries",
];
const DANCE_TYPES: &[&str] = &[
    "gala",
    "dance",
    "performance",
    "ball",
    "soiree",
    "jamboree",
    "exhibition",
    "carnival",
    "festival",
    "jubilee",
    "celebration",
    "gathering",
    "fete",
];
const HILL_MONSTER_ADJECTIVES: &[&str] = &[
    "great",
    "big",
    "huge",
    "prime",
    "golden",
    "proud",
    "lucky",
    "fat",
    "giant",
    "hungry",
    "magical",
    "superior",
    "terrifying",
    "horrifying",
    "feared",
];
const HILL_MONSTER_HABITS: &[&str] = &[
    "steals cattle at night",
    "prefers eating children",
    "doesn't mind human flesh",
    "keeps the region at bay",
    "eats kids whole",
    "abducts young women",
    "terrorizes the region",
    "harasses travelers in the area",
    "snatches people from homes",
    "attacks anyone who dares to approach its lair",
    "attacks unsuspecting victims",
];
const HILL_MONSTER_SPECIES: &[&str] = &[
    "Ogre",
    "Troll",
    "Cyclops",
    "Giant",
    "Monster",
    "Beast",
    "Dragon",
    "Undead",
    "Ghoul",
    "Vampire",
    "Hag",
    "Banshee",
    "Bearded Devil",
    "Roc",
    "Hydra",
    "Warg",
];
const INN_ADJECTIVES: &[&str] = &[
    "New",
    "Good",
    "High",
    "Old",
    "Great",
    "Big",
    "Major",
    "Happy",
    "Main",
    "Huge",
    "Far",
    "Beautiful",
    "Fair",
    "Prime",
    "Ancient",
    "Golden",
    "Proud",
    "Lucky",
    "Fat",
    "Honest",
    "Giant",
    "Distant",
    "Friendly",
    "Loud",
    "Hungry",
    "Magical",
    "Superior",
    "Peaceful",
    "Frozen",
    "Divine",
    "Favorable",
    "Brave",
    "Sunny",
    "Flying",
];
const INN_ANIMALS: &[&str] = &[
    "Antelope",
    "Ape",
    "Badger",
    "Bear",
    "Beaver",
    "Bison",
    "Boar",
    "Buffalo",
    "Cat",
    "Crane",
    "Crocodile",
    "Crow",
    "Deer",
    "Dog",
    "Eagle",
    "Elk",
    "Fox",
    "Goat",
    "Goose",
    "Hare",
    "Hawk",
    "Heron",
    "Horse",
    "Hyena",
    "Ibis",
    "Jackal",
    "Jaguar",
    "Lark",
    "Leopard",
    "Lion",
    "Mantis",
    "Marten",
    "Moose",
    "Mule",
    "Narwhal",
    "Owl",
    "Panther",
    "Rat",
    "Raven",
    "Rook",
    "Scorpion",
    "Shark",
    "Sheep",
    "Snake",
    "Spider",
    "Swan",
    "Tiger",
    "Turtle",
    "Wolf",
    "Wolverine",
    "Camel",
    "Falcon",
    "Hound",
    "Ox",
];
const INN_COLORS: &[&str] = &[
    "Dark", "Light", "Bright", "Golden", "White", "Black", "Red", "Pink", "Purple", "Blue",
    "Green", "Yellow", "Amber", "Orange", "Brown", "Grey",
];
const INN_COMMON_COURSES: &[&str] = &[
    "pork",
    "bacon",
    "chicken",
    "goose",
    "duck",
    "pigeon",
    "chevon",
    "eggs",
    "onions",
    "carrot",
    "potato",
    "beet",
    "garlic",
    "cabbage",
    "spinach",
    "peas",
    "beans",
    "pumpkin",
    "apples",
    "pears",
    "rat tails",
    "pig ears",
];
const INN_COMMON_DRINKS: &[&str] = &["water", "cider", "juice", "sap"];
const INN_COOKING_METHODS: &[&str] = &[
    "Boiled",
    "Grilled",
    "Roasted",
    "Spit-roasted",
    "Stewed",
    "Stuffed",
    "Jugged",
    "Mashed",
    "Baked",
    "Braised",
    "Poached",
    "Marinated",
    "Pickled",
    "Smoked",
    "Dried",
    "Dry-aged",
    "Corned",
    "Fried",
    "Pan-fried",
    "Deep-fried",
    "Dressed",
    "Steamed",
    "Cured",
    "Syrupped",
    "Flame-Broiled",
];
const INN_DRINK_KINDS: &[&str] = &[
    "hot",
    "cold",
    "fire",
    "ice",
    "smoky",
    "misty",
    "shiny",
    "sweet",
    "bitter",
    "salty",
    "sour",
    "sparkling",
    "smelly",
];
const INN_WARM_COURSES: &[&str] = &[
    "rice", "chickpea", "eggplant", "zucchini", "pepper", "tomatoes", "melon", "oranges", "mango",
];
const JOUST_TYPES: &[&str] = &["Joust", "Competition", "Melee", "Tournament", "Contest"];
const JOUST_VIRTUES: &[&str] = &[
    "cunning",
    "might",
    "speed",
    "the greats",
    "acumen",
    "brutality",
];
const LIBRARY_TYPES: &[(&str, u32)] = &[("Library", 3), ("Archive", 1), ("Collection", 1)];
const MIRAGE_ADJECTIVES: &[&str] = &[
    "Entrancing",
    "Diaphanous",
    "Illusory",
    "Distant",
    "Peculiar",
];
const NECROPOLIS_LEGENDS: &[&str] = &[
    "A foreboding necropolis shrouded in perpetual darkness, where eerie whispers echo through the winding corridors and spectral guardians stand watch over the tombs of long-forgotten souls.",
    "A towering necropolis adorned with macabre sculptures and guarded by formidable undead sentinels. Its ancient halls house the remains of fallen heroes, entombed alongside their cherished relics.",
    "This ethereal necropolis seems suspended between the realms of the living and the dead. Wisps of mist dance around the tombstones, while haunting melodies linger in the air, commemorating the departed.",
    "Rising from the desolate landscape, this sinister necropolis is a testament to necromantic power. Its skeletal spires cast ominous shadows, concealing forbidden knowledge and arcane secrets.",
    "An eerie necropolis where nature intertwines with death. Overgrown tombstones are entwined by thorny vines, and mournful spirits wander among the fading petals of once-vibrant flowers.",
    "A labyrinthine necropolis where each step echoes with haunting murmurs. The walls are adorned with ancient runes, and restless spirits guide or hinder those who dare to delve into its depths.",
    "This cursed necropolis is veiled in perpetual twilight, perpetuating a sense of impending doom. Dark enchantments shroud the tombs, and the moans of anguished souls resound through its crumbling halls.",
    "A sprawling necropolis built within a labyrinthine network of catacombs. Its halls are lined with countless alcoves, each housing the remains of the departed, while the distant sound of rattling bones fills the air.",
    "A desolate necropolis where an eerie stillness reigns. Time seems frozen amidst the decaying mausoleums, and the silence is broken only by the whispers of the wind and the rustle of tattered banners.",
    "A foreboding necropolis perched atop a jagged cliff, overlooking a desolate wasteland. Its towering walls harbor restless spirits, and the imposing gates bear the marks of countless battles and ancient curses.",
];
const NECROPOLIS_TYPES: &[(&str, u32)] = &[
    ("Necropolis", 5),
    ("Crypt", 2),
    ("Tomb", 2),
    ("Graveyard", 1),
    ("Cemetery", 2),
    ("Mausoleum", 1),
    ("Sepulchre", 1),
];
const RIFT_EFFECTS: &[&str] = &[
    "all known nearby beings to flee in terror",
    "cracks in reality itself to form",
    "swarms of foes to spill forth",
    "nearby plants to wither and decay",
    "an emmissary to step through with an all-powerful relic",
];
const RIFT_TYPES: &[&str] = &[
    "Demonic",
    "Interdimensional",
    "Abyssal",
    "Cosmic",
    "Cataclysmic",
    "Subterranean",
    "Ancient",
];
const RUIN_TYPES: &[&str] = &[
    "City",
    "Town",
    "Settlement",
    "Pyramid",
    "Fort",
    "Stronghold",
    "Temple",
    "Sacred site",
    "Mausoleum",
    "Outpost",
    "Fortification",
    "Fortress",
    "Castle",
];
const RUMOR_SOURCES: &[&str] = &[
    "Locals",
    "Elders",
    "Inscriptions",
    "Tipplers",
    "Legends",
    "Whispers",
    "Rumors",
    "Journeying folk",
    "Tales",
];
const STATUE_VARIANTS: &[&str] = &[
    "Statue",
    "Obelisk",
    "Monument",
    "Column",
    "Monolith",
    "Pillar",
    "Megalith",
    "Stele",
    "Runestone",
    "Sculpture",
    "Effigy",
    "Idol",
];
const WATERFALL_DESCRIPTIONS: &[&str] = &[
    "A gorgeous waterfall flows here.",
    "The rapids of an exceptionally beautiful waterfall.",
    "An impressive waterfall has cut through the land.",
    "The cascades of a stunning waterfall.",
    "A river drops down from a great height forming a wondrous waterfall.",
    "A breathtaking waterfall cuts through the landscape.",
];
const WATER_SOURCE_TYPES: &[(&str, u32)] = &[
    ("Healing Spring", 5),
    ("Purifying Well", 2),
    ("Enchanted Reservoir", 1),
    ("Creek of Luck", 1),
    ("Fountain of Youth", 1),
    ("Wisdom Spring", 1),
    ("Spring of Life", 1),
    ("Spring of Youth", 1),
    ("Healing Stream", 1),
];
const STATUE_SCRIPTS: &[&str] = &[
    "𐠁𐠂𐠃𐠄𐠅𐠈𐠊𐠋𐠌𐠍𐠎𐠏𐠐𐠑𐠒𐠓𐠔𐠕𐠖𐠗𐠘𐠙𐠚𐠛𐠜𐠝𐠞𐠟𐠠𐠡𐠢𐠣𐠤𐠥𐠦𐠧𐠨𐠩𐠪𐠫𐠬𐠭𐠮𐠯𐠰𐠱𐠲𐠳𐠴𐠵𐠷𐠸𐠼𐠿      ",
    "ሀለሐመሠረሰቀበተኀነአከወዐዘየደገጠጰጸፀፈፐ   ",
    "ⲲⲴⲶⲸⲺⲼⲾⳀⳁⳂⳃⳄⳆⳈⳊⳌⳎⳐⳒⳔⳖⳘⳚⳜⳞⳠⳢⳤ⳥⳧⳩⳪ⳫⳬⳭⳲ⳹⳾   ",
    "ༀ༁༂༃༄༅༆༇༈༉༊་༌༐༑༒༓༔༕༖༗༘༙༚༛༜༠༡༢༣༤༥༦༧༨༩༪༫༬༭༮༯༰༱༲༳༴༵༶༷༸༹༺༻༼༽༾༿",
    "᠀᠐᠑᠒ᠠᠡᠦᠧᠨᠩᠪᠭᠮᠯᠰᠱᠲᠳᠵᠻᠼᠽᠾᠿᡀᡁᡆᡍᡎᡏᡐᡑᡒᡓᡔᡕᡖᡗᡙᡜᡝᡞᡟᡠᡡᡭᡮᡯᡰᡱᡲᡳᡴᢀᢁᢂᢋᢏᢐᢑᢒᢓᢛᢜᢞᢟᢠᢡᢢᢤᢥᢦ",
];
const ENCOUNTER_KINDS: &[(&str, &str)] = &[
    ("Bandits", "have set an ambush"),
    ("Wild beasts", "have been sighted"),
    ("A lone traveler", "was seen wandering"),
    ("Cultists", "gather in secret"),
    ("A pilgrim", "walks the road"),
    ("Refugees", "have made camp"),
    ("Smugglers", "move under cover of night"),
    ("A hermit", "dwells alone"),
    ("Mercenaries", "ride through"),
    ("Poachers", "have been active"),
];
/// Animals that migrate across each biome.
fn migrating_animals(biome: u8) -> &'static [&'static str] {
    match biome {
        1 => &[
            "Camels",
            "Antelopes",
            "Jackals",
            "Hyenas",
            "Scorpions",
            "Locusts",
        ],
        2 => &["Camels", "Goats", "Wolves", "Foxes", "Eagles", "Falcons"],
        3 => &[
            "Antelopes",
            "Buffalo",
            "Elephants",
            "Lions",
            "Hyenas",
            "Cranes",
            "Zebras",
        ],
        4 => &[
            "Bisons", "Horses", "Deer", "Hares", "Geese", "Larks", "Wolves",
        ],
        5 => &[
            "Apes",
            "Elephants",
            "Leopards",
            "Tigers",
            "Parrots",
            "Mantises",
        ],
        6 => &[
            "Deer", "Boars", "Badgers", "Foxes", "Crows", "Rooks", "Bears", "Wolves",
        ],
        7 => &[
            "Apes", "Jaguars", "Panthers", "Snakes", "Spiders", "Parrots",
        ],
        8 => &["Elk", "Deer", "Bears", "Beavers", "Owls", "Ravens"],
        9 => &[
            "Elk",
            "Moose",
            "Wolves",
            "Wolverines",
            "Martens",
            "Bears",
            "Ravens",
        ],
        10 => &[
            "Reindeer",
            "Musk oxen",
            "Wolves",
            "Foxes",
            "Geese",
            "Hares",
            "Owls",
        ],
        12 => &["Herons", "Cranes", "Ibises", "Geese", "Crocodiles", "Frogs"],
        _ => &[],
    }
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

use crate::diplomacy::WARS;

/// The calendar year of FMG.
const YEAR: f64 = crate::diplomacy::YEAR;

/// A marker kind: type, icon, min, each, multiplier.
struct Kind {
    kind: &'static str,
    icon: &'static str,
    min: f64,
    each: f64,
    multiplier: f64,
}

const fn k(kind: &'static str, icon: &'static str, min: f64, each: f64, multiplier: f64) -> Kind {
    Kind {
        kind,
        icon,
        min,
        each,
        multiplier,
    }
}

/// A campaign of a state (`generateCampaign`).
struct Campaign {
    name: String,
    start: f64,
    end: f64,
}

struct Gen<'a> {
    input: &'a Input<'a>,
    rng: Rand,
    occupied: Vec<bool>,
    chains: Vec<Chain>,
    /// The place on each cell, as an index in the place list.
    burg: Vec<Option<usize>>,
    /// The route links of each cell: the neighbor and the route group.
    links: Vec<BTreeMap<usize, &'static str>>,
    campaigns: BTreeMap<u32, Vec<Campaign>>,
    era: String,
    markers: Vec<Marker>,
}

impl Gen<'_> {
    fn soc(&self) -> &Society {
        self.input.society
    }
    fn land(&self, c: usize) -> bool {
        self.soc().cells.land(c)
    }
    fn h(&self, c: usize) -> f64 {
        self.soc().cells.h[c]
    }
    fn pop(&self, c: usize) -> f64 {
        self.soc().cells.pop[c]
    }
    fn culture(&self, c: usize) -> u32 {
        self.soc().cell_culture[c]
    }
    fn religion(&self, c: usize) -> u32 {
        self.input.religions.cell_religion[c]
    }
    fn burg_pop(&self, c: usize) -> Option<f64> {
        self.burg[c].map(|k| self.soc().places[k].population)
    }
    fn connected(&self, c: usize) -> bool {
        !self.links[c].is_empty()
    }
    fn has_road(&self, c: usize) -> bool {
        self.links[c].values().any(|&g| g == "roads")
    }
    fn crossroad(&self, c: usize) -> bool {
        self.links[c].len() > 3 || self.links[c].values().filter(|&&g| g == "roads").count() > 2
    }
    /// A name from the culture of a cell (`getCulture`).
    fn culture_name(&mut self, culture: u32) -> String {
        let chain = &self.chains[culture as usize];
        chain.get(&mut self.rng)
    }
    fn religion_name(&self, id: u32) -> String {
        self.input
            .religions
            .list
            .iter()
            .find(|r| r.id == id)
            .map_or_else(String::new, |r| r.name.clone())
    }
    /// `getFriendlyHeight`: meters with the FMG exponent 2.
    fn height(&self, c: usize) -> String {
        let meters = libm::pow(self.h(c) - 18.0, 2.0);
        format!("{}m", rn(meters, 0))
    }
    fn river_name(&self, c: usize) -> Option<String> {
        let r = self.soc().cells.r[c];
        let k = self.input.rivers.iter().position(|x| x.id == r)?;
        let n = self.input.names.rivers.get(k)?;
        Some(format!("{} {}", n.name, n.kind))
    }
    fn place_name(&self, c: usize) -> Option<String> {
        self.burg[c].map(|k| self.soc().places[k].name.clone())
    }
    fn date(&mut self, from: f64, to: f64) -> String {
        let year = self.rng.rand(from as i64, to as i64);
        let month = self.rng.rand(0, 11) as usize;
        let day = self.rng.rand(1, 28);
        format!("{} {day}, {year}", MONTHS[month])
    }

    fn list(&self, kind: &str) -> Vec<usize> {
        let soc = self.soc();
        let cells = &soc.cells;
        let biomes = self.input.biomes;
        let n = self.input.grid.len();
        let free = |c: usize| !self.occupied[c];
        let all = (0..n).filter(|&c| free(c));
        match kind {
            "volcanoes" => all.filter(|&c| self.h(c) >= 70.0).collect(),
            "hot-springs" => all
                .filter(|&c| self.h(c) > 50.0 && self.culture(c) != 0)
                .collect(),
            "water-sources" => all
                .filter(|&c| self.h(c) > 30.0 && cells.r[c] != 0)
                .collect(),
            "bridges" => {
                let flux: Vec<f64> = cells.fl.iter().copied().filter(|&f| f != 0.0).collect();
                let mean = flux.iter().sum::<f64>() / flux.len().max(1) as f64;
                all.filter(|&c| {
                    self.burg[c].is_some()
                        && cells.t[c] != 1
                        && self.burg_pop(c).unwrap_or(0.0) > 20.0
                        && cells.r[c] != 0
                        && cells.fl[c] > mean
                })
                .collect()
            }
            "inns" => all
                .filter(|&c| self.pop(c) > 5.0 && self.crossroad(c))
                .collect(),
            "lighthouses" => all
                .filter(|&c| {
                    cells.harbor.get(c).is_some_and(|&h| h > 6)
                        && self
                            .input
                            .grid
                            .neighbors(c)
                            .any(|m| !self.land(m) && self.connected(m))
                })
                .collect(),
            "waterfalls" => all
                .filter(|&c| {
                    cells.r[c] != 0
                        && self.h(c) >= 50.0
                        && self
                            .input
                            .grid
                            .neighbors(c)
                            .any(|m| self.h(m) < 40.0 && cells.r[m] != 0)
                })
                .collect(),
            "battlefields" => all
                .filter(|&c| {
                    soc.cell_state[c] != NONE
                        && self.pop(c) > 2.0
                        && self.h(c) < 50.0
                        && self.h(c) > 25.0
                })
                .collect(),
            "dungeons" => all
                .filter(|&c| self.pop(c) != 0.0 && self.pop(c) < 3.0)
                .collect(),
            "lake-monsters" => self
                .input
                .features
                .list
                .iter()
                .filter(|f| {
                    f.kind == crate::features::FeatureKind::Lake
                        && cells.group[f.first_cell as usize] == "freshwater"
                        && free(f.first_cell as usize)
                })
                .map(|f| f.first_cell as usize)
                .collect(),
            "sea-monsters" => all
                .filter(|&c| {
                    !self.land(c)
                        && self.connected(c)
                        && self.input.features.get(self.input.features.of_cell[c]).kind
                            == crate::features::FeatureKind::Ocean
                })
                .collect(),
            "hill-monsters" | "caves" => all
                .filter(|&c| self.h(c) >= 50.0 && self.pop(c) != 0.0)
                .collect(),
            "sacred-mountains" => all
                .filter(|&c| {
                    let g = self.input.grid;
                    self.h(c) >= 70.0
                        && g.neighbors(c).any(|m| self.culture(m) != 0)
                        && g.neighbors(c).all(|m| self.h(m) < 60.0)
                })
                .collect(),
            "sacred-forests" => all
                .filter(|&c| {
                    self.culture(c) != 0 && self.religion(c) != 0 && [6, 8].contains(&biomes[c])
                })
                .collect(),
            "sacred-pineries" => all
                .filter(|&c| self.culture(c) != 0 && self.religion(c) != 0 && biomes[c] == 9)
                .collect(),
            "sacred-palm-groves" => all
                .filter(|&c| {
                    self.culture(c) != 0
                        && self.religion(c) != 0
                        && biomes[c] == 1
                        && self.pop(c) > 1.0
                        && self.connected(c)
                })
                .collect(),
            "brigands" => all
                .filter(|&c| self.culture(c) != 0 && self.has_road(c))
                .collect(),
            "pirates" => all
                .filter(|&c| !self.land(c) && self.connected(c))
                .collect(),
            "statues" => all
                .filter(|&c| self.h(c) >= 20.0 && self.h(c) < 40.0)
                .collect(),
            "ruins" => all
                .filter(|&c| self.culture(c) != 0 && self.h(c) >= 20.0 && self.h(c) < 60.0)
                .collect(),
            "libraries" => all
                .filter(|&c| self.culture(c) != 0 && self.burg[c].is_some() && self.pop(c) > 10.0)
                .collect(),
            "circuses" => all
                .filter(|&c| self.culture(c) != 0 && self.h(c) >= 20.0 && self.connected(c))
                .collect(),
            "jousts" => all
                .filter(|&c| self.burg_pop(c).is_some_and(|p| p > 20.0))
                .collect(),
            "fairs" => all
                .filter(|&c| self.burg_pop(c).is_some_and(|p| p < 20.0 && p > 5.0))
                .collect(),
            "canoes" => all.filter(|&c| cells.r[c] != 0).collect(),
            "migration" => all
                .filter(|&c| {
                    self.h(c) >= 20.0
                        && self.pop(c) <= 2.0
                        && !migrating_animals(biomes[c]).is_empty()
                })
                .collect(),
            "dances" => all
                .filter(|&c| self.burg_pop(c).is_some_and(|p| p > 15.0))
                .collect(),
            "mirage" => all.filter(|&c| biomes[c] == 1).collect(),
            "portals" => {
                let places = &soc.places;
                let k = (places.len() as f64 / 10.0).ceil() as usize;
                places
                    .iter()
                    .take(k)
                    .map(|p| p.cell)
                    .filter(|&c| free(c))
                    .collect()
            }
            "rifts" => all
                .filter(|&c| {
                    self.pop(c) <= 3.0 && crate::biomes::HABITABILITY[biomes[c] as usize] != 0.0
                })
                .collect(),
            "disturbed-burials" => all
                .filter(|&c| self.h(c) >= 20.0 && self.pop(c) > 2.0)
                .collect(),
            "necropolises" => all
                .filter(|&c| self.h(c) >= 20.0 && self.pop(c) < 2.0)
                .collect(),
            "encounters" => all
                .filter(|&c| self.h(c) >= 20.0 && self.pop(c) > 1.0)
                .collect(),
            "party" => all.filter(|&c| self.burg[c].is_some()).collect(),
            _ => Vec::new(),
        }
    }

    /// The name and the legend of a new marker (the `add` functions).
    fn describe(&mut self, kind: &str, c: usize) -> (Option<String>, Option<String>) {
        let culture = self.culture(c);
        let s = |x: &str| Some(x.to_owned());
        match kind {
            "volcanoes" => {
                let proper = self.culture_name(culture);
                let name = if self.rng.p(0.3) {
                    format!("Mount {proper}")
                } else if self.rng.p(0.7) {
                    format!("{proper} Volcano")
                } else {
                    proper
                };
                let status = if self.rng.p(0.6) {
                    "Dormant"
                } else if self.rng.p(0.4) {
                    "Active"
                } else {
                    "Erupting"
                };
                let legend = format!("{status} volcano. Height: {}.", self.height(c));
                (Some(name), Some(legend))
            }
            "hot-springs" => {
                let proper = self.culture_name(culture);
                let temp = self.rng.gauss(35.0, 15.0, 20.0, 100.0, 0);
                let name = if self.rng.p(0.3) {
                    format!("Hot Springs of {proper}")
                } else if self.rng.p(0.7) {
                    format!("{proper} Hot Springs")
                } else {
                    proper
                };
                let legend = format!(
                    "A geothermal springs with naturally heated water that provide relaxation and medicinal benefits. Average temperature is {}\u{b0}C.",
                    rn(temp, 0)
                );
                (Some(name), Some(legend))
            }
            "water-sources" => {
                let kind = self.rng.weighted(WATER_SOURCE_TYPES);
                let proper = self.culture_name(culture);
                (
                    Some(format!("{proper} {kind}")),
                    s(
                        "This legendary water source is whispered about in ancient tales and believed to possess mystical properties. The spring emanates crystal-clear water, shimmering with an otherworldly iridescence that sparkles even in the dimmest light.",
                    ),
                )
            }
            "bridges" => {
                let burg = self.place_name(c).unwrap_or_default();
                let river = self.river_name(c);
                let river_short = river.as_ref().and_then(|_| {
                    let r = self.soc().cells.r[c];
                    let k = self.input.rivers.iter().position(|x| x.id == r)?;
                    Some(self.input.names.rivers[k].name.clone())
                });
                let name = match river_short {
                    Some(r) if self.rng.p(0.2) => format!("{r} Bridge"),
                    _ => format!("{burg} Bridge"),
                };
                let adj = self.rng.weighted(BRIDGE_ADJECTIVES);
                let river = river.unwrap_or_else(|| "river".to_owned());
                let legend = format!(
                    "A {adj} bridge spans over the {river} at {burg}. Carts and pack trains cross it from dawn to dusk, bound for the market."
                );
                (Some(name), Some(legend))
            }
            "inns" => {
                let type_name = if self.rng.p(0.3) { "inn" } else { "tavern" };
                let themed = self.rng.p(0.7);
                let animal = *self.rng.pick(INN_ANIMALS);
                let name = if themed {
                    if self.rng.p(0.6) {
                        format!("{} {animal}", self.rng.pick(INN_COLORS))
                    } else {
                        format!("{} {animal}", self.rng.pick(INN_ADJECTIVES))
                    }
                } else {
                    format!(
                        "{} {}",
                        self.rng.pick(INN_ADJECTIVES),
                        capitalize(type_name)
                    )
                };
                let warm = self.input.temperature[c] >= 18.0;
                let mut courses: Vec<&str> = INN_COMMON_COURSES.to_vec();
                if warm {
                    courses.extend_from_slice(INN_WARM_COURSES);
                }
                let served = courses.contains(&animal.to_lowercase().as_str());
                let meal = if themed && served && self.rng.p(0.5) {
                    animal.to_owned()
                } else {
                    (*self.rng.pick(&courses)).to_owned()
                };
                let course =
                    format!("{} {meal}", self.rng.pick(INN_COOKING_METHODS)).to_lowercase();
                let first = if self.rng.p(0.5) {
                    *self.rng.pick(INN_DRINK_KINDS)
                } else {
                    *self.rng.pick(INN_COLORS)
                };
                let drink = format!("{first} {}", self.rng.pick(INN_COMMON_DRINKS)).to_lowercase();
                let legend = format!(
                    "A big and famous roadside {type_name}. Delicious {course} with {drink} is served here."
                );
                (Some(format!("The {name}")), Some(legend))
            }
            "lighthouses" | "waterfalls" => {
                let proper = match self.place_name(c) {
                    Some(n) => n,
                    None => self.culture_name(culture),
                };
                let adj = adjective(&proper, &mut self.rng);
                if kind == "lighthouses" {
                    (
                        Some(format!("{adj} Lighthouse")),
                        s("A lighthouse to serve as a beacon for ships in the open sea."),
                    )
                } else {
                    let legend = (*self.rng.pick(WATERFALL_DESCRIPTIONS)).to_owned();
                    (Some(format!("{adj} Waterfall")), Some(legend))
                }
            }
            "battlefields" => {
                let state = self.soc().cell_state[c];
                if !self.campaigns.contains_key(&state) {
                    // The campaigns of the state (diplomacy), else new ones.
                    let own: Vec<Campaign> = self
                        .soc()
                        .states
                        .iter()
                        .find(|s| s.id == state)
                        .map(|s| {
                            s.campaigns
                                .iter()
                                .map(|cp| Campaign {
                                    name: cp.name.clone(),
                                    start: cp.start,
                                    end: cp.end.unwrap_or(YEAR),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let list = if own.is_empty() {
                        self.campaign(state)
                    } else {
                        own
                    };
                    self.campaigns.insert(state, list);
                }
                let n = self.campaigns[&state].len();
                let k = self.rng.rand(0, n as i64 - 1).clamp(0, n as i64 - 1) as usize;
                let (cname, start, end) = {
                    let cp = &self.campaigns[&state][k];
                    (cp.name.clone(), cp.start, cp.end)
                };
                let date = self.date(start, end);
                let name = format!("{} Battlefield", self.culture_name(culture));
                let legend = format!(
                    "A historical battle of the {cname}. \r\nDate: {date} {}.",
                    self.era
                );
                (Some(name), Some(legend))
            }
            "dungeons" => (
                s("Dungeon"),
                Some(format!(
                    "Undiscovered dungeon. See One page dungeon: https://watabou.github.io/one-page-dungeon/?seed={}{c}",
                    self.input.config.seed
                )),
            ),
            "lake-monsters" => {
                let f = self.input.features.of_cell[c];
                let lake = self
                    .input
                    .names
                    .features
                    .iter()
                    .find(|x| x.id == f)
                    .and_then(|x| x.name.clone())
                    .unwrap_or_else(|| "Lake".to_owned());
                let short = lake.trim_start_matches("Lake ").to_owned();
                let length = self.rng.gauss(10.0, 5.0, 5.0, 100.0, 0);
                let rumor = self.rng.pick(RUMOR_SOURCES);
                let legend = format!(
                    "{rumor} say a relic monster of {length} m long inhabits {lake}. Truth or lie, folks are afraid to fish in the lake."
                );
                (Some(format!("{short} Monster")), Some(legend))
            }
            "sea-monsters" => {
                let name = self.chains[0].get_short(&mut self.rng);
                let length = self.rng.gauss(25.0, 10.0, 10.0, 100.0, 0);
                let legend = format!(
                    "Old sailors tell stories of a gigantic sea monster inhabiting these dangerous waters. Rumors say it can be {length} m long."
                );
                (Some(format!("{name} Monster")), Some(legend))
            }
            "hill-monsters" => {
                let monster = *self.rng.pick(HILL_MONSTER_SPECIES);
                let toponym = self.culture_name(culture);
                let rumor = self.rng.pick(RUMOR_SOURCES);
                let adj = self.rng.pick(HILL_MONSTER_ADJECTIVES);
                let habit = self.rng.pick(HILL_MONSTER_HABITS);
                let legend = format!(
                    "{rumor} speak of a {adj} {monster} who inhabits {toponym} hills and {habit}."
                );
                (Some(format!("{toponym} {monster}")), Some(legend))
            }
            "sacred-mountains" => {
                let g = self.input.grid;
                let near_culture = g
                    .neighbors(c)
                    .map(|m| self.culture(m))
                    .find(|&x| x != 0)
                    .unwrap_or(0);
                let religion = match self.religion(c) {
                    0 => g
                        .neighbors(c)
                        .map(|m| self.religion(m))
                        .find(|&r| r != 0)
                        .unwrap_or(0),
                    r => r,
                };
                let name = format!("{} Mountain", self.culture_name(near_culture));
                let to = if religion != 0 {
                    format!(" of {}", self.religion_name(religion))
                } else {
                    String::new()
                };
                let legend = format!("A sacred mountain{to}. Height: {}.", self.height(c));
                (Some(name), Some(legend))
            }
            "sacred-forests" | "sacred-pineries" | "sacred-palm-groves" => {
                let (noun, what) = match kind {
                    "sacred-forests" => ("Forest", "forest"),
                    "sacred-pineries" => ("Pinery", "pinery"),
                    _ => ("Palm Grove", "palm grove"),
                };
                let name = format!("{} {noun}", self.culture_name(culture));
                let legend = format!(
                    "A {what} sacred to local {}.",
                    self.religion_name(self.religion(c))
                );
                (Some(name), Some(legend))
            }
            "brigands" => {
                let biome = self.input.biomes[c];
                let locality = if self.h(c) >= 70.0 {
                    "highlander"
                } else if [1, 2].contains(&biome) {
                    "desert"
                } else if [3, 4].contains(&biome) {
                    "mounted"
                } else if (5..=9).contains(&biome) {
                    "forest"
                } else if biome == 12 {
                    "swamp"
                } else {
                    "angry"
                };
                let proper = self.culture_name(culture);
                let animal = self.rng.pick(BRIGAND_ANIMALS);
                let kind = self.rng.weighted(BRIGAND_TYPES);
                (
                    Some(format!("{proper} {animal}")),
                    Some(format!("A gang of {locality} {kind}.")),
                )
            }
            "pirates" => (
                s("Pirates"),
                s("Pirate ships have been spotted in these waters."),
            ),
            "statues" => {
                let variant = *self.rng.pick(STATUE_VARIANTS);
                let name = format!("{} {variant}", self.culture_name(culture));
                let script: Vec<char> = self.rng.pick(STATUE_SCRIPTS).chars().collect();
                let count = self.rng.rand(40, 100);
                let inscription: String = (0..count).map(|_| *self.rng.pick(&script)).collect();
                let legend = format!(
                    "An ancient {}. It has an inscription, but no one can translate it: {inscription}",
                    variant.to_lowercase()
                );
                (Some(name), Some(legend))
            }
            "ruins" => {
                let t = *self.rng.pick(RUIN_TYPES);
                (
                    Some(format!("Ruined {t}")),
                    Some(format!(
                        "Ruins of an ancient {}. Untold riches may lie within.",
                        t.to_lowercase()
                    )),
                )
            }
            "libraries" => {
                let t = self.rng.weighted(LIBRARY_TYPES);
                (
                    Some(format!("{} {t}", self.culture_name(culture))),
                    s("A vast collection of knowledge, including many rare and ancient tomes."),
                )
            }
            "circuses" => {
                let adj = *self.rng.pick(CIRCUS_ADJECTIVES);
                (
                    Some(format!("Travelling {adj} Circus")),
                    Some(format!(
                        "Roll up, roll up, this {} circus is here for a limited time only.",
                        adj.to_lowercase()
                    )),
                )
            }
            "jousts" => {
                let burg = self.place_name(c).unwrap_or_default();
                let t = *self.rng.pick(JOUST_TYPES);
                let virtue = self.rng.pick(JOUST_VIRTUES);
                (
                    Some(format!("{burg} {t}")),
                    Some(format!(
                        "Warriors from around the land gather for a {} of {virtue} in {burg}, with fame, fortune and favour on offer to the victor.",
                        t.to_lowercase()
                    )),
                )
            }
            "fairs" => {
                let burg = self.place_name(c).unwrap_or_default();
                (
                    Some(format!("{burg} Fair")),
                    Some(format!(
                        "A fair is being held in {burg}, with all manner of local and foreign goods and services on offer."
                    )),
                )
            }
            "canoes" => {
                let river = self.river_name(c).unwrap_or_else(|| "river".to_owned());
                (
                    s("Minor Jetty"),
                    Some(format!(
                        "A small location along the {river} to launch boats from sits here, along with a weary looking owner, willing to sell passage along the river."
                    )),
                )
            }
            "migration" => {
                let list = migrating_animals(self.input.biomes[c]);
                let animal = if list.is_empty() {
                    "Birds"
                } else {
                    *self.rng.pick(list)
                };
                (
                    Some(format!("{animal} migration")),
                    Some(format!(
                        "A huge group of {} are migrating, whether part of their annual routine, or something more extraordinary.",
                        animal.to_lowercase()
                    )),
                )
            }
            "dances" => {
                let burg = self.place_name(c).unwrap_or_default();
                let t = self.rng.pick(DANCE_TYPES);
                let guests = self.rng.pick(DANCE_GUESTS);
                (
                    Some(format!("{burg} {t}")),
                    Some(format!(
                        "A {t} has been organised at {burg} as a chance to gather the {guests} of the area together to be merry, make alliances and scheme around the crisis."
                    )),
                )
            }
            "mirage" => {
                let adj = *self.rng.pick(MIRAGE_ADJECTIVES);
                (
                    Some(format!("{adj} mirage")),
                    Some(format!(
                        "This {} mirage has been luring travellers out of their way for eons.",
                        adj.to_lowercase()
                    )),
                )
            }
            "caves" => {
                let mut formation = self.rng.weighted(CAVE_FORMATIONS).to_owned();
                let toponym = self.culture_name(culture);
                if self.input.biomes[c] == 11 {
                    formation = format!("Glacial {formation}");
                }
                let name = format!("{toponym} {formation}");
                let status = self.rng.weighted(CAVE_STATUSES);
                let legend = format!("The {name}. Locals claim that it is {status}.");
                (Some(name), Some(legend))
            }
            "portals" => {
                let burg = self.place_name(c).unwrap_or_default();
                (
                    Some(format!("{burg} Portal")),
                    s(
                        "An element of the magic portal system connecting major cities. The portals were installed centuries ago, but still work fine.",
                    ),
                )
            }
            "rifts" => {
                let t = *self.rng.pick(RIFT_TYPES);
                let effect = self.rng.pick(RIFT_EFFECTS);
                (
                    Some(format!("{t} Rift")),
                    Some(format!(
                        "A rumoured {} rift in this area is causing {effect}.",
                        t.to_lowercase()
                    )),
                )
            }
            "disturbed-burials" => (
                s("Disturbed Burial"),
                s(
                    "A burial site has been disturbed in this area, causing the dead to rise and attack the living.",
                ),
            ),
            "necropolises" => {
                let toponym = self.culture_name(culture);
                let t = self.rng.weighted(NECROPOLIS_TYPES);
                let legend = (*self.rng.pick(NECROPOLIS_LEGENDS)).to_owned();
                (Some(format!("{toponym} {t}")), Some(legend))
            }
            "encounters" => {
                let name = self.culture_name(culture);
                let biome = crate::biomes::NAMES[self.input.biomes[c] as usize].replace('-', " ");
                let (subject, verb) = *self.rng.pick(ENCOUNTER_KINDS);
                (
                    Some(format!("{subject} of {name}")),
                    Some(format!("{subject} {verb} in the {biome} of {name} lands.")),
                )
            }
            "party" => {
                if self.markers.iter().any(|m| m.kind == "party") {
                    (None, None)
                } else {
                    (
                        s("The Party"),
                        s("Current location of the adventuring party."),
                    )
                }
            }
            _ => (None, None),
        }
    }

    /// The campaigns of a state (`generateCampaign`).
    fn campaign(&mut self, state: u32) -> Vec<Campaign> {
        let soc = self.input.society;
        let st = soc.states.iter().find(|s| s.id == state);
        let neighbors: Vec<u32> = st
            .map(|s| s.neighbors.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| vec![0]);
        let culture = st.map_or(0, |s| s.culture);
        let mut out: Vec<Campaign> = neighbors
            .iter()
            .map(|&i| {
                let other = soc.states.iter().find(|s| s.id == i);
                let name = match other {
                    Some(o) if i != 0 && self.rng.p(0.8) => o.name.clone(),
                    _ => self.chains[culture as usize].get_short(&mut self.rng),
                };
                let start = self.rng.gauss(YEAR - 100.0, 150.0, 1.0, YEAR - 6.0, 0);
                let end = start + self.rng.gauss(4.0, 5.0, 1.0, YEAR - start - 1.0, 0);
                let war = self.rng.weighted(&WARS);
                Campaign {
                    name: format!("{} {war}", adjective(&name, &mut self.rng)),
                    start,
                    end,
                }
            })
            .collect();
        out.sort_by(|a, b| a.start.total_cmp(&b.start));
        out
    }
}

/// Make the markers (`generate`).
pub fn generate(input: &Input) -> Vec<Marker> {
    let soc = input.society;
    let n = input.grid.len();
    let mut occupied = vec![false; n];
    // inkland: no marker on a pinned place.
    for &c in input.pin_cells {
        occupied[c] = true;
    }
    let mut burg = vec![None; n];
    for (k, p) in soc.places.iter().enumerate() {
        burg[p.cell] = Some(k);
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
    let chains: Vec<Chain> = std::iter::once(Chain::new(crate::cultures::WILDLANDS_BASE))
        .chain(soc.cultures.iter().map(|c| Chain::new(c.base)))
        .collect();
    let mut rng = Rand::new(
        crate::seed::SeedTree::new(input.config.seed)
            .part_rng(crate::seed::Layer::Markers, "generate"),
    );
    // The era of the calendar (`getEra`).
    let base = if rng.p(0.7) {
        1
    } else {
        rng.rand(0, crate::names::data::NAME_BASES.len() as i64) as usize
    };
    let era = format!("{} Era", Chain::new(base).get_short(&mut rng));
    let fantasy = f64::from(u8::from(input.config.world.culture_set.contains("fantasy")));
    let kinds = [
        k("volcanoes", "volcano", 10.0, 500.0, 1.0),
        k("hot-springs", "hot-springs", 30.0, 1200.0, 1.0),
        k("water-sources", "droplet", 1.0, 1000.0, 1.0),
        k("bridges", "bridge", 1.0, 5.0, 1.0),
        k("inns", "beer", 1.0, 10.0, 1.0),
        k("lighthouses", "lighthouse", 1.0, 2.0, 1.0),
        k("waterfalls", "waterfall", 1.0, 5.0, 1.0),
        k("battlefields", "crossed-swords", 50.0, 700.0, 1.0),
        k("dungeons", "key", 30.0, 200.0, 1.0),
        k("lake-monsters", "dragon", 2.0, 10.0, 1.0),
        k("sea-monsters", "squid", 50.0, 700.0, 1.0),
        k("hill-monsters", "ogre", 30.0, 600.0, 1.0),
        k("sacred-mountains", "mountain", 1.0, 5.0, 1.0),
        k("sacred-forests", "tree", 30.0, 1000.0, 1.0),
        k("sacred-pineries", "evergreen", 30.0, 800.0, 1.0),
        k("sacred-palm-groves", "palm", 1.0, 100.0, 1.0),
        k("brigands", "money-bag", 50.0, 100.0, 1.0),
        k("pirates", "pirate-flag", 40.0, 400.0, 1.0),
        k("statues", "moai", 80.0, 1200.0, 1.0),
        k("ruins", "amphora", 80.0, 1200.0, 1.0),
        k("libraries", "books", 10.0, 1200.0, 1.0),
        k("circuses", "circus-tent", 80.0, 1000.0, 1.0),
        k("jousts", "fencer", 5.0, 500.0, 1.0),
        k("fairs", "carousel", 50.0, 1000.0, 1.0),
        k("canoes", "canoe", 500.0, 2000.0, 1.0),
        k("migration", "boar", 20.0, 1000.0, 1.0),
        k("dances", "dancer", 50.0, 1000.0, 1.0),
        k("mirage", "spiral", 10.0, 600.0, 1.0),
        k("caves", "bat", 60.0, 1000.0, 1.0),
        k("portals", "cyclone", 16.0, 8.0, fantasy),
        k("rifts", "fireworks", 5.0, 3000.0, fantasy),
        k("disturbed-burials", "skull", 20.0, 3000.0, fantasy),
        k("necropolises", "headstone", 20.0, 1000.0, 1.0),
        k("encounters", "mage", 10.0, 600.0, 1.0),
        k("party", "flag", 1.0, 9_007_199_254_740_991.0, 1.0),
    ];
    let mut g = Gen {
        input,
        rng,
        occupied,
        chains,
        burg,
        links,
        campaigns: BTreeMap::new(),
        era,
        markers: Vec::new(),
    };
    for kind in &kinds {
        if kind.multiplier == 0.0 {
            continue;
        }
        let mut candidates = g.list(kind.kind);
        let len = candidates.len() as f64;
        let mut quantity = if len == 0.0 || len < kind.min / kind.multiplier {
            0
        } else {
            ((len / kind.each * kind.multiplier).ceil() as usize).min(candidates.len())
        };
        while quantity > 0 && !candidates.is_empty() {
            let k = (g.rng.unit() * candidates.len() as f64) as usize;
            let cell = candidates.remove(k);
            let at = match g.burg[cell] {
                Some(p) => soc.places[p].at,
                None => input.grid.points[cell],
            };
            let (name, legend) = g.describe(kind.kind, cell);
            let name = name.or_else(|| Some(capitalize(&kind.kind.replace('-', " "))));
            g.markers.push(Marker {
                id: g.markers.len() as u32 + 1,
                kind: kind.kind,
                icon: kind.icon,
                cell,
                at,
                name,
                legend,
            });
            g.occupied[cell] = true;
            quantity -= 1;
        }
    }
    g.markers
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    fn world(set: &str) -> (Config, World) {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 21, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 6000, "template": "continents", "culture_set": set },
            "pins": { "points": [{ "id": "ostrey", "kind": "city", "at": [200, 150] }] }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        (c, w)
    }

    #[test]
    fn markers_follow_the_fmg_rules() {
        let (_, w) = world("european");
        let m = &w.markers;
        assert!(m.len() > 10, "{}", m.len());
        let mut cells = std::collections::BTreeSet::new();
        for x in m {
            assert!(cells.insert(x.cell), "two markers on cell {}", x.cell);
            assert!(!w.pin_cells.contains(&x.cell));
            assert_eq!(w.grid.find_cell(x.at[0], x.at[1]), x.cell);
            assert!(x.name.as_deref().is_some_and(|n| !n.is_empty()));
            assert!(!x.icon.is_empty());
        }
        let kinds: std::collections::BTreeSet<&str> = m.iter().map(|x| x.kind).collect();
        for k in ["volcanoes", "ruins", "statues", "party"] {
            assert!(kinds.contains(k), "{k} in {kinds:?}");
        }
        // Fantasy kinds need a fantasy culture set.
        assert!(!kinds.contains("portals") && !kinds.contains("rifts"));
        assert_eq!(m.iter().filter(|x| x.kind == "party").count(), 1);
    }

    #[test]
    fn a_fantasy_set_gives_portals_and_the_same_seed_the_same_markers() {
        let (_, a) = world("high-fantasy");
        assert!(
            a.markers
                .iter()
                .any(|x| x.kind == "portals" || x.kind == "rifts")
        );
        let (_, b) = world("high-fantasy");
        assert_eq!(a.markers, b.markers);
    }
}
