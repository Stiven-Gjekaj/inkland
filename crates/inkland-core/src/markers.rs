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
//! The kinds that read the economy (mines, guild halls, merchant houses,
//! customs houses, bridges, inns, brigands, pirates, fairs) weigh their
//! candidates by the trade of the last run (`indexTrade`, `indexGuilds`),
//! as FMG does.
//!
//! inkland: no marker goes on the cell of a pinned place. The name of a
//! mine has a colon where FMG writes an em-dash. Legends are plain text: a
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
    /// The last economy run.
    pub economy: &'a crate::economy::Economy,
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
/// Inn fare by good name, served only where the market of the inn has the
/// good (`INN_COURSES_BY_GOOD`).
const INN_COURSES_BY_GOOD: &[(&str, &[&str])] = &[
    ("Cattle", &["beef", "veal"]),
    ("Sheep", &["lamb", "mutton"]),
    (
        "Game",
        &[
            "hare", "rabbit", "hart", "deer", "antlers", "bear", "buffalo", "badger", "beaver",
            "boar", "pheasant",
        ],
    ),
    (
        "Fish",
        &[
            "carp", "bass", "pike", "catfish", "sturgeon", "escallop", "eel",
        ],
    ),
    ("Whales", &["whale", "seal"]),
    ("Camels", &["camel"]),
    (
        "Grain",
        &["bread", "pie", "cake", "pottage", "pudding", "pasta"],
    ),
    ("Olives", &["olives"]),
    ("Dates", &["dates"]),
    ("Cheese", &["cheese"]),
];
/// Inn drinks by good name (`INN_DRINKS_BY_GOOD`).
const INN_DRINKS_BY_GOOD: &[(&str, &[&str])] = &[
    ("Wine", &["wine", "brandy"]),
    ("Beer", &["beer", "ale"]),
    (
        "Liquor",
        &[
            "gin", "whisky", "rum", "vodka", "tequila", "absinthe", "liquor", "spirits",
        ],
    ),
    ("Grain", &["kvass"]),
    ("Honey", &["mead", "nectar"]),
    ("Sugarcane", &["rum"]),
    ("Tea", &["tea"]),
    ("Cattle", &["milk"]),
    ("Horses", &["kumis"]),
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

/// The last economy run, summed for the markers (`TradeIndex`).
#[derive(Debug, Clone, Default)]
struct Trade {
    /// By market id: the value of its trade.
    volume: Vec<f64>,
    /// By market id: what its center earned on it.
    margin: Vec<f64>,
    /// By place id: the number of goods it traded.
    goods: Vec<usize>,
    /// By place id: the value of its trade.
    burg_trade: Vec<f64>,
    /// By state pair: the tariffs paid.
    tariffs: BTreeMap<(u32, u32), f64>,
    /// By place id: the made good it is famed for and its renown.
    guilds: BTreeMap<u32, (u32, f64)>,
}

/// A pair of states in one order (`getStatePair`).
fn state_pair(a: u32, b: u32) -> (u32, u32) {
    if a < b { (a, b) } else { (b, a) }
}

/// Upper case at the start (`capitalize` of FMG).
fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// A list in English: `a`, `a and b`, `a, b, and c` (`list`).
fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// Sum the deals of the last run (`indexTrade`).
fn index_trade(soc: &Society, e: &crate::economy::Economy) -> Trade {
    use crate::economy::deals::DealKind;
    let markets = e.markets.len() + 1;
    let places = soc.places.len() + 1;
    let mut t = Trade {
        volume: vec![0.0; markets],
        margin: vec![0.0; markets],
        goods: vec![0; places],
        burg_trade: vec![0.0; places],
        ..Trade::default()
    };
    let mut traded: Vec<std::collections::BTreeSet<u32>> = vec![Default::default(); places];
    let market_state = |id: u32| {
        e.market(id)
            .and_then(|m| soc.places.get(m.center as usize - 1))
            .map_or(NONE, |p| p.state)
    };
    for d in &e.deals {
        if d.kind == DealKind::Kept {
            continue;
        }
        let (m, b) = (d.market as usize, d.burg as usize);
        let value = d.units * d.price;
        t.volume[m] += value;
        t.burg_trade[b] += value;
        let fee = e.market(d.market).map_or(0.0, |m| m.fee_paid);
        t.margin[m] += d.margin(fee);
        traded[b].insert(d.good);
        if d.tariff == 0.0 {
            continue;
        }
        let state = soc.places.get(b.wrapping_sub(1)).map_or(NONE, |p| p.state);
        *t.tariffs
            .entry(state_pair(state, market_state(d.market)))
            .or_insert(0.0) += d.tariff;
    }
    for m in &e.markets {
        let Some(rural) = &m.rural else { continue };
        let k = m.id as usize;
        t.volume[k] += rural.earned + rural.spent;
        t.margin[k] += rural.margin;
    }
    t.goods = traded.iter().map(std::collections::BTreeSet::len).collect();
    t.guilds = index_guilds(soc, e);
    t
}

/// The places famed for a made good: leaders among many makers and in
/// their state (`indexGuilds`).
fn index_guilds(soc: &Society, e: &crate::economy::Economy) -> BTreeMap<u32, (u32, f64)> {
    /// World share times the root of the makers times the root of the
    /// state share that a place needs.
    const GUILD_RENOWN: f64 = 0.75;
    /// The share of the value that a place makes that the good must be.
    const GUILD_FOCUS: f64 = 0.25;
    /// The value of the good that a place makes a day.
    const GUILD_MIN_VALUE: f64 = 15.0;
    let goods = e.goods.len() + 1;
    let mut world = vec![0.0; goods];
    let mut makers = vec![0u32; goods];
    let mut in_state: BTreeMap<(u32, u32), f64> = BTreeMap::new();
    let mut worth = vec![0.0; soc.places.len() + 1];
    let mut crafts: Vec<(u32, u32, f64, u32)> = Vec::new();
    for (k, p) in soc.places.iter().enumerate() {
        let Some(pe) = e.places.get(k) else { continue };
        for &(id, units) in &pe.production {
            let Some(good) = e.good(id) else { continue };
            if good.recipes.is_empty() {
                continue;
            }
            world[id as usize] += units;
            makers[id as usize] += 1;
            *in_state.entry((p.state, id)).or_insert(0.0) += units;
            worth[p.id as usize] += units * good.value;
            crafts.push((p.id, id, units, p.state));
        }
    }
    let mut guilds: BTreeMap<u32, (u32, f64)> = BTreeMap::new();
    for (burg, id, units, state) in crafts {
        let good = e.good(id).expect("a made good is in the catalogue");
        let value = units * good.value;
        if value < GUILD_MIN_VALUE || value / worth[burg as usize] < GUILD_FOCUS {
            continue;
        }
        let renown = units / world[id as usize]
            * libm::sqrt(f64::from(makers[id as usize]) * (units / in_state[&(state, id)]));
        if renown < GUILD_RENOWN || renown <= guilds.get(&burg).map_or(0.0, |g| g.1) {
            continue;
        }
        // One hall a place, for what it is most famed for.
        guilds.insert(burg, (id, renown));
    }
    guilds
}

/// One draw over the weights (`extractWeighted`): an even draw when no
/// candidate weighs anything.
fn extract_weighted(rng: &mut Rand, cells: &mut Vec<usize>, weights: &mut Vec<f64>) -> usize {
    let total = weights.iter().fold(0.0, |s, w| s + w);
    let roll = rng.unit();
    let mut index = (roll * cells.len() as f64) as usize;
    if total != 0.0 {
        let mut rest = roll * total - weights[0];
        index = 0;
        while rest >= 0.0 && index < weights.len() - 1 {
            index += 1;
            rest -= weights[index];
        }
    }
    weights.remove(index);
    cells.remove(index)
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
    /// The route links of each cell: the neighbor, the route index (roads,
    /// then sea routes), and the route group (FMG `cells.routes`).
    links: Vec<BTreeMap<usize, (usize, &'static str)>>,
    /// The first and the last cell of each route.
    route_ends: Vec<Option<(usize, usize)>>,
    /// The last economy run, summed for the markers.
    trade: Trade,
    campaigns: BTreeMap<u32, Vec<Campaign>>,
    era: String,
    markers: Vec<Marker>,
}

impl<'a> Gen<'a> {
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
        self.links[c].values().any(|&(_, g)| g == "roads")
    }
    fn crossroad(&self, c: usize) -> bool {
        self.links[c].len() > 3
            || self.links[c]
                .values()
                .filter(|&&(_, g)| g == "roads")
                .count()
                > 2
    }
    /// The good of the deposit of a cell, if it is an ore or a mineral.
    fn ore(&self, c: usize) -> Option<&'a crate::economy::goods::Good> {
        let e = self.input.economy;
        let good = e.good(u32::from(self.soc().cells.good[c]))?;
        good.tags
            .iter()
            .any(|t| t == "ore" || t == "mineral")
            .then_some(good)
    }
    /// The ore or mineral that the place of a cell extracts in the last run
    /// (`getMinedGood`).
    fn mined_good(&self, c: usize) -> Option<&'a crate::economy::goods::Good> {
        let good = self.ore(c)?;
        let k = self.burg[c]?;
        let e = self.input.economy;
        e.places
            .get(k)?
            .production
            .iter()
            .any(|&(id, _)| id == good.id)
            .then_some(good)
    }
    /// The ore or mineral deposit under the place of a cell, mined or not
    /// (`getDeposit`).
    fn deposit(&self, c: usize) -> Option<&'a crate::economy::goods::Good> {
        self.burg[c]?;
        self.ore(c)
    }
    /// The names of the goods that the market of a cell had in stock or
    /// traded in the last run (`getMarketGoods`).
    fn market_goods(&self, c: usize) -> std::collections::BTreeSet<&'a str> {
        let e = self.input.economy;
        let id = e.cell_market.get(c).copied().unwrap_or(0);
        let Some(market) = e.market(id) else {
            return Default::default();
        };
        let stocked = market
            .goods
            .iter()
            .enumerate()
            .filter(|(_, g)| g.stock > 0.0)
            .map(|(k, _)| k as u32 + 1);
        let dealt = e.deals.iter().filter(|d| d.market == id).map(|d| d.good);
        stocked
            .chain(dealt)
            .filter_map(|g| e.good(g).map(|g| g.name.as_str()))
            .collect()
    }
    /// The trade of the market of a cell, square-rooted so the busiest
    /// markets do not take every marker (`getMarketTrade`).
    fn market_trade(&self, c: usize) -> f64 {
        let id = self.input.economy.cell_market.get(c).copied().unwrap_or(0) as usize;
        libm::sqrt(self.trade.volume.get(id).copied().unwrap_or(0.0))
    }
    /// The neighbor state that a road from the cell leads to, with the most
    /// tariffs between the two states (`getCustomsBorder`).
    fn customs_border(&self, c: usize) -> Option<(u32, f64)> {
        let cell_state = &self.soc().cell_state;
        let own = cell_state[c];
        let mut border: Option<(u32, f64)> = None;
        for (&n, &(_, group)) in &self.links[c] {
            let state = cell_state[n];
            if own == NONE || state == NONE || state == own || group != "roads" {
                continue;
            }
            let tariffs = self
                .trade
                .tariffs
                .get(&state_pair(own, state))
                .copied()
                .unwrap_or(0.0);
            if tariffs > border.map_or(0.0, |b| b.1) {
                border = Some((state, tariffs));
            }
        }
        border
    }
    /// The weight of a candidate cell, for the kinds that FMG weighs.
    fn weight(&self, kind: &str, c: usize) -> Option<f64> {
        let burg_id = |c: usize| self.burg[c].map_or(0, |k| self.soc().places[k].id) as usize;
        let at = |v: &[f64], k: usize| v.get(k).copied().unwrap_or(0.0);
        let market =
            |c: usize| self.input.economy.cell_market.get(c).copied().unwrap_or(0) as usize;
        Some(match kind {
            // Rich deposits draw miners: gold, silver, and gems most.
            "mines" => self.deposit(c).map_or(0.0, |d| {
                d.value
                    * d.value
                    * if d.tags.iter().any(|t| t == "luxury") {
                        4.0
                    } else {
                        1.0
                    }
            }),
            "bridges" => libm::sqrt(at(&self.trade.burg_trade, burg_id(c))),
            "merchant-houses" => at(&self.trade.margin, market(c)).max(0.0),
            "inns" => self.market_trade(c),
            // Brigands prey on the roads of busy markets where the land is
            // poor.
            "brigands" => {
                let prosperity = self.input.economy.prosperity.get(c).copied().unwrap_or(0);
                self.market_trade(c) * (1.2 - f64::from(prosperity) / 100.0)
            }
            // Pirates prey on the busiest sea route through the cell.
            "pirates" => {
                let routes: std::collections::BTreeSet<usize> =
                    self.links[c].values().map(|&(r, _)| r).collect();
                routes
                    .iter()
                    .fold(0.0, |best: f64, &r| match self.route_ends[r] {
                        Some((a, b)) => best.max(self.market_trade(a) + self.market_trade(b)),
                        None => best,
                    })
            }
            "fairs" => self.trade.goods.get(burg_id(c)).copied().unwrap_or(0) as f64,
            "customs-houses" => self.customs_border(c).map_or(0.0, |b| b.1),
            _ => return None,
        })
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
            "guild-halls" => all
                .filter(|&c| {
                    self.burg[c].is_some_and(|k| self.trade.guilds.contains_key(&soc.places[k].id))
                })
                .collect(),
            "mines" => all.filter(|&c| self.deposit(c).is_some()).collect(),
            "merchant-houses" => self
                .input
                .economy
                .markets
                .iter()
                .filter_map(|m| soc.places.get(m.center as usize - 1).map(|p| p.cell))
                .filter(|&c| free(c) && self.burg[c].is_some())
                .collect(),
            "customs-houses" => {
                // One road crossing for each pair of states that trade under
                // tariffs.
                let mut crossings: BTreeMap<(u32, u32), usize> = BTreeMap::new();
                let mut order: Vec<usize> = Vec::new();
                for (c, &own) in soc.cell_state.iter().enumerate().take(n) {
                    if !free(c) {
                        continue;
                    }
                    if let Some((state, _)) = self.customs_border(c) {
                        let pair = state_pair(own, state);
                        if let std::collections::btree_map::Entry::Vacant(v) = crossings.entry(pair)
                        {
                            v.insert(c);
                            order.push(c);
                        }
                    }
                }
                order
            }
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

    /// A local item with the chance 0.7 if there is one, else a common item
    /// (`pick` of `addInn`).
    fn pick_fare(&mut self, local: &[&'static str], common: &[&'static str]) -> &'static str {
        if !local.is_empty() && self.rng.p(0.7) {
            self.rng.pick(local)
        } else {
            self.rng.pick(common)
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
            "guild-halls" => {
                let soc = self.soc();
                let burg = self.burg[c].map(|k| &soc.places[k]);
                let guild = burg.and_then(|b| self.trade.guilds.get(&b.id).copied());
                match (burg, guild) {
                    (Some(b), Some((good_id, _))) => {
                        let e = self.input.economy;
                        let made = e.good(good_id).expect("a guild good is in the catalogue");
                        let good = made.name.to_lowercase();
                        let worked = self.mined_good(c).filter(|m| {
                            made.recipes
                                .iter()
                                .any(|r| r.iter().any(|&(id, _)| id == m.id))
                        });
                        let source = worked.map_or(String::new(), |m| {
                            format!(
                                ", worked from the {} of its own mines",
                                m.name.to_lowercase()
                            )
                        });
                        (
                            Some(format!("{} {} Guild", b.name, upper_first(&good))),
                            Some(format!(
                                "{} is famed for its {good}{source}. The guild hall of its makers keeps the secrets of the craft, and its masters set the price far beyond the town.",
                                b.name
                            )),
                        )
                    }
                    (b, _) => (
                        Some(b.map_or_else(
                            || "Guild Hall".to_owned(),
                            |b| format!("{} Guild Hall", b.name),
                        )),
                        s("The hall of a craft guild that keeps the secrets of its trade."),
                    ),
                }
            }
            "mines" => {
                let mined = self.mined_good(c);
                let resource = mined
                    .or_else(|| self.deposit(c))
                    .map_or_else(|| "ore".to_owned(), |g| g.name.to_lowercase());
                let Some(k) = self.burg[c] else {
                    return (None, None);
                };
                let burg = &self.soc().places[k];
                if mined.is_none() && !self.input.economy.deals.is_empty() {
                    return (
                        Some(format!("{}: old {resource} mine", burg.name)),
                        Some(format!(
                            "The {resource} mine by {} is played out. Its shafts stand empty, and the miners have long gone to work the fields.",
                            burg.name
                        )),
                    );
                }
                let population = rn(
                    burg.population
                        * crate::military::POPULATION_SCALE
                        * crate::military::URBANIZATION_RATE,
                    0,
                );
                (
                    Some(format!("{}: {resource} mining town", burg.name)),
                    Some(format!(
                        "{} is a mining town of {population} people just nearby the {resource} mine.",
                        burg.name
                    )),
                )
            }
            "merchant-houses" => {
                let e = self.input.economy;
                let market = e.cell_market.get(c).copied().unwrap_or(0) as usize;
                let margin_of = |id: usize| self.trade.margin.get(id).copied().unwrap_or(0.0);
                let margin = margin_of(market);
                let richer = e
                    .markets
                    .iter()
                    .filter(|m| margin_of(m.id as usize) > margin)
                    .count();
                let rank = if richer > 0 {
                    "one of the richest"
                } else {
                    "the richest"
                };
                let burg = self.place_name(c).unwrap_or_else(|| "the town".to_owned());
                let family = self.culture_name(culture);
                (
                    Some(format!("House of {family}")),
                    Some(format!(
                        "A merchant family of {burg}, {rank} in the land. Its factors buy and sell across the whole market, and a share of every deal ends in its coffers."
                    )),
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
                let local = self.market_goods(c);
                let local_fare =
                    |by_good: &[(&str, &'static [&'static str])]| -> Vec<&'static str> {
                        by_good
                            .iter()
                            .filter(|(good, _)| local.contains(good))
                            .flat_map(|(_, fare)| fare.iter().copied())
                            .collect()
                    };
                let warm = self.input.temperature[c] >= 18.0;
                let mut courses: Vec<&str> = INN_COMMON_COURSES.to_vec();
                if warm {
                    courses.extend_from_slice(INN_WARM_COURSES);
                }
                let local_courses = local_fare(INN_COURSES_BY_GOOD);
                let lower = animal.to_lowercase();
                let served =
                    courses.contains(&lower.as_str()) || local_courses.contains(&lower.as_str());
                let meal = if themed && served && self.rng.p(0.5) {
                    animal.to_owned()
                } else {
                    self.pick_fare(&local_courses, &courses).to_owned()
                };
                let course =
                    format!("{} {meal}", self.rng.pick(INN_COOKING_METHODS)).to_lowercase();
                let first = if self.rng.p(0.5) {
                    *self.rng.pick(INN_DRINK_KINDS)
                } else {
                    *self.rng.pick(INN_COLORS)
                };
                let local_drinks = local_fare(INN_DRINKS_BY_GOOD);
                let drink = format!(
                    "{first} {}",
                    self.pick_fare(&local_drinks, INN_COMMON_DRINKS)
                )
                .to_lowercase();
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
                let e = self.input.economy;
                let mut made: Vec<(u32, f64)> = self.burg[c]
                    .and_then(|k| e.places.get(k))
                    .map_or_else(Vec::new, |p| p.production.clone());
                made.sort_by(|a, b| b.1.total_cmp(&a.1));
                let mut wares: Vec<String> = Vec::new();
                for (id, _) in made {
                    if let Some(g) = e.good(id) {
                        let name = g.name.to_lowercase();
                        if !wares.contains(&name) {
                            wares.push(name);
                        }
                    }
                }
                wares.truncate(3);
                let offer = if wares.is_empty() {
                    "with all manner of local and foreign goods and services on offer".to_owned()
                } else {
                    format!(
                        "offering local {} alongside foreign goods and services",
                        list(&wares)
                    )
                };
                (
                    Some(format!("{burg} Fair")),
                    Some(format!("A fair is being held in {burg}, {offer}.")),
                )
            }
            "customs-houses" => {
                let soc = self.soc();
                let state = soc.cell_state[c];
                let state_name = soc
                    .states
                    .get((state as usize).wrapping_sub(1))
                    .map_or_else(|| "the state".to_owned(), |s| s.name.clone());
                let from = self.customs_border(c).map_or(String::new(), |(b, _)| {
                    format!(" from {}", soc.states[b as usize - 1].name)
                });
                let place = match self.place_name(c) {
                    Some(n) => n,
                    None => self.culture_name(culture),
                };
                (
                    Some(format!("{place} Customs House")),
                    Some(format!(
                        "A customs house of {state_name} on the road{from}. Carts queue at its barrier while the wardens weigh their loads and collect the tariffs."
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

/// The random numbers of the markers.
fn marker_rng(seed: u64) -> Rand {
    Rand::new(crate::seed::SeedTree::new(seed).part_rng(crate::seed::Layer::Markers, "generate"))
}

/// The era of the calendar (`getEra`).
fn draw_era(rng: &mut Rand) -> String {
    let base = if rng.p(0.7) {
        1
    } else {
        rng.rand(0, crate::names::data::NAME_BASES.len() as i64) as usize
    };
    format!("{} Era", Chain::new(base).get_short(rng))
}

/// The era of the calendar of a seed (FMG `lore.calendar.era`). The
/// markers draw it first; this gives the same name.
pub fn era(seed: u64) -> String {
    draw_era(&mut marker_rng(seed))
}

/// The short era: the first letter of each word (`getEraShort`).
pub fn era_short(era: &str) -> String {
    era.split(' ')
        .filter(|w| !w.is_empty())
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .collect()
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
    let mut links: Vec<BTreeMap<usize, (usize, &'static str)>> = vec![BTreeMap::new(); n];
    let mut route_ends: Vec<Option<(usize, usize)>> = Vec::new();
    for (k, r) in input.roads.iter().chain(input.sea_routes).enumerate() {
        route_ends.push(r.cells.first().zip(r.cells.last()).map(|(&a, &b)| (a, b)));
        for w in r.cells.windows(2) {
            if w[0] != w[1] {
                links[w[0]].insert(w[1], (k, r.group));
                links[w[1]].insert(w[0], (k, r.group));
            }
        }
    }
    let chains: Vec<Chain> = std::iter::once(Chain::new(crate::cultures::WILDLANDS_BASE))
        .chain(soc.cultures.iter().map(|c| Chain::new(c.base)))
        .collect();
    let mut rng = marker_rng(input.config.seed);
    let era = draw_era(&mut rng);
    let fantasy = f64::from(u8::from(input.config.world.culture_set.contains("fantasy")));
    let kinds = [
        k("volcanoes", "volcano", 10.0, 500.0, 1.0),
        k("hot-springs", "hot-springs", 30.0, 1200.0, 1.0),
        k("water-sources", "droplet", 1.0, 1000.0, 1.0),
        // Before the mines: a guild town often mines what its guild works.
        k("guild-halls", "classical-building", 1.0, 1.0, 1.0),
        k("mines", "pick", 1.0, 6.0, 1.0),
        k("bridges", "bridge", 1.0, 5.0, 1.0),
        k("merchant-houses", "balance-scale", 3.0, 8.0, 1.0),
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
        k("customs-houses", "customs", 1.0, 3.0, 1.0),
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
        route_ends,
        trade: index_trade(soc, input.economy),
        campaigns: BTreeMap::new(),
        era,
        markers: Vec::new(),
    };
    for kind in &kinds {
        if kind.multiplier == 0.0 {
            continue;
        }
        let mut candidates = g.list(kind.kind);
        let mut weights: Option<Vec<f64>> =
            candidates.iter().map(|&c| g.weight(kind.kind, c)).collect();
        let len = candidates.len() as f64;
        let mut quantity = if len == 0.0 || len < kind.min / kind.multiplier {
            0
        } else {
            ((len / kind.each * kind.multiplier).ceil() as usize).min(candidates.len())
        };
        while quantity > 0 && !candidates.is_empty() {
            let cell = match weights.as_mut() {
                Some(w) => extract_weighted(&mut g.rng, &mut candidates, w),
                None => {
                    let k = (g.rng.unit() * candidates.len() as f64) as usize;
                    candidates.remove(k)
                }
            };
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
    fn a_weighted_draw_follows_the_weights() {
        use crate::rand::Rand;
        let mut rng =
            Rand::new(crate::seed::SeedTree::new(3).part_rng(crate::seed::Layer::Markers, "t"));
        // One heavy candidate wins nearly each draw.
        let mut heavy = 0;
        for _ in 0..200 {
            let mut cells = vec![10, 11, 12];
            let mut w = vec![0.0, 1000.0, 1.0];
            let c = super::extract_weighted(&mut rng, &mut cells, &mut w);
            assert_eq!(cells.len(), 2);
            assert_eq!(w.len(), 2);
            assert!(!cells.contains(&c));
            heavy += usize::from(c == 11);
        }
        assert!(heavy > 190, "{heavy}");
        // A zero weight is never drawn while another weighs something.
        for _ in 0..100 {
            let mut cells = vec![1, 2];
            let mut w = vec![0.0, 5.0];
            assert_eq!(super::extract_weighted(&mut rng, &mut cells, &mut w), 2);
        }
        // No weight at all: an even draw.
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..100 {
            let mut cells = vec![1, 2, 3];
            let mut w = vec![0.0; 3];
            seen.insert(super::extract_weighted(&mut rng, &mut cells, &mut w));
        }
        assert_eq!(seen.len(), 3);
    }

    #[test]
    fn lists_and_capitals_follow_fmg() {
        let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        assert_eq!(super::list(&s(&["salt"])), "salt");
        assert_eq!(super::list(&s(&["salt", "wine"])), "salt and wine");
        assert_eq!(
            super::list(&s(&["salt", "wine", "iron"])),
            "salt, wine, and iron"
        );
        assert_eq!(super::upper_first("fine cloth"), "Fine cloth");
        assert_eq!(super::state_pair(5, 2), (2, 5));
    }

    #[test]
    fn the_economy_markers_follow_the_last_run() {
        let (_, w) = world("european");
        let soc = &w.society;
        let e = &w.economy;
        let kinds: std::collections::BTreeSet<&str> = w.markers.iter().map(|x| x.kind).collect();
        for k in ["guild-halls", "mines", "merchant-houses", "inns"] {
            assert!(kinds.contains(k), "{k} in {kinds:?}");
        }
        let trade = super::index_trade(soc, e);
        let place = |c: usize| soc.places.iter().find(|p| p.cell == c);
        for m in &w.markers {
            let name = m.name.as_deref().unwrap_or_default();
            let legend = m.legend.as_deref().unwrap_or_default();
            assert!(
                !name.contains('\u{2014}') && !legend.contains('\u{2014}'),
                "{name}"
            );
            match m.kind {
                "guild-halls" => {
                    let p = place(m.cell).expect("a guild hall is in a place");
                    let (good, renown) = trade.guilds[&p.id];
                    assert!(renown >= 0.75);
                    assert!(
                        e.places[p.id as usize - 1]
                            .production
                            .iter()
                            .any(|g| g.0 == good)
                    );
                    assert!(name.ends_with(" Guild"), "{name}");
                }
                "mines" => {
                    assert!(place(m.cell).is_some());
                    let good = e.good(u32::from(soc.cells.good[m.cell])).unwrap();
                    assert!(good.tags.iter().any(|t| t == "ore" || t == "mineral"));
                    assert!(name.contains(": "), "{name}");
                }
                "merchant-houses" => {
                    let p = place(m.cell).unwrap();
                    assert!(e.markets.iter().any(|k| k.center == p.id));
                    assert!(name.starts_with("House of "));
                }
                "customs-houses" => {
                    assert!(legend.contains(" on the road from "), "{legend}");
                }
                _ => {}
            }
        }
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
