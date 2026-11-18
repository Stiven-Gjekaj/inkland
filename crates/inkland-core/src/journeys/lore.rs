//! The lore tables of FMG journeys (`data/journey-lore.ts`).
//!
//! The tables are the data of FMG, in its order. Two templates of FMG have
//! an em-dash; inkland writes a colon there.

use super::transports::Domain;

/// Adjectives for a company, a relic, and other names.
pub const COMPANY_ADJECTIVES: [&str; 62] = [
    "Iron",
    "Gilded",
    "Silent",
    "Crimson",
    "Wandering",
    "Broken",
    "Grey",
    "Long",
    "Salt",
    "Amber",
    "Thorn",
    "Ashen",
    "Hollow",
    "Winter",
    "Last",
    "Patient",
    "Bronze",
    "Copper",
    "Silver",
    "Golden",
    "Leaden",
    "Scarlet",
    "Sable",
    "Russet",
    "Pale",
    "Bright",
    "Bitter",
    "Quiet",
    "Restless",
    "Weary",
    "Ragged",
    "Faithful",
    "Fearless",
    "Stubborn",
    "Solemn",
    "Steadfast",
    "Vigilant",
    "Wayward",
    "Errant",
    "Crooked",
    "Nameless",
    "Lost",
    "Free",
    "Sworn",
    "Bold",
    "Proud",
    "Cold",
    "Frost",
    "Storm",
    "Ember",
    "Cinder",
    "Flint",
    "Stone",
    "Glass",
    "Bramble",
    "Briar",
    "Starless",
    "Sunless",
    "Hidden",
    "Veiled",
    "Hallowed",
    "Weathered",
];

/// The charges of a banner.
pub const BANNERS: [&str; 64] = [
    "Hart", "Rose", "Chalice", "Tower", "Serpent", "Star", "Lantern", "Owl", "Wolf", "Falcon",
    "Oak", "Key", "Lion", "Bear", "Boar", "Stag", "Ram", "Bull", "Hound", "Fox", "Hare", "Adder",
    "Raven", "Eagle", "Hawk", "Swan", "Heron", "Magpie", "Pike", "Salmon", "Griffin", "Wyvern",
    "Dragon", "Phoenix", "Unicorn", "Basilisk", "Crown", "Crescent", "Sun", "Moon", "Comet",
    "Flame", "Anchor", "Helm", "Gauntlet", "Sword", "Spear", "Arrow", "Shield", "Chain", "Bell",
    "Harp", "Horn", "Sheaf", "Wheel", "Anvil", "Hammer", "Gate", "Thistle", "Ivy", "Yew", "Lily",
    "Feather", "Antler",
];

/// The goods of a caravan or a smuggling run.
pub const CARGO: [&str; 65] = [
    "Salt",
    "Amber",
    "Silk",
    "Spice",
    "Wool",
    "Iron",
    "Wine",
    "Furs",
    "Ivory",
    "Glass",
    "Pearl",
    "Tin",
    "Copper",
    "Bronze",
    "Steel",
    "Silver",
    "Gold",
    "Lead",
    "Flint",
    "Marble",
    "Obsidian",
    "Jade",
    "Lapis",
    "Alum",
    "Saltpetre",
    "Quicksilver",
    "Flax",
    "Linen",
    "Cotton",
    "Velvet",
    "Dye",
    "Indigo",
    "Pepper",
    "Cinnamon",
    "Cloves",
    "Saffron",
    "Incense",
    "Myrrh",
    "Sandalwood",
    "Grain",
    "Barley",
    "Olives",
    "Honey",
    "Cheese",
    "Ale",
    "Mead",
    "Vinegar",
    "Sugar",
    "Tea",
    "Fish",
    "Timber",
    "Charcoal",
    "Coal",
    "Pitch",
    "Tar",
    "Hemp",
    "Wax",
    "Oil",
    "Leather",
    "Hides",
    "Horses",
    "Cattle",
    "Tobacco",
    "Parchment",
    "Ink",
];

/// The relics that a party searches for.
pub const RELICS: [&str; 62] = [
    "Crown",
    "Chalice",
    "Codex",
    "Shard",
    "Seal",
    "Blade",
    "Ring",
    "Mask",
    "Horn",
    "Tome",
    "Sceptre",
    "Circlet",
    "Diadem",
    "Torc",
    "Amulet",
    "Talisman",
    "Sigil",
    "Cloak",
    "Mantle",
    "Veil",
    "Gauntlet",
    "Orb",
    "Staff",
    "Rod",
    "Crozier",
    "Spear",
    "Dagger",
    "Bow",
    "Shield",
    "Helm",
    "Hammer",
    "Scroll",
    "Tablet",
    "Grimoire",
    "Chronicle",
    "Ledger",
    "Reliquary",
    "Casket",
    "Coffer",
    "Urn",
    "Vial",
    "Flask",
    "Cup",
    "Lantern",
    "Mirror",
    "Hourglass",
    "Bell",
    "Harp",
    "Key",
    "Chain",
    "Coin",
    "Banner",
    "Throne",
    "Wheel",
    "Stone",
    "Heart",
    "Eye",
    "Tear",
    "Feather",
    "Fang",
    "Egg",
    "Thorn",
];

/// The beasts of a monster hunt.
pub const BEASTS: [&str; 48] = [
    "dragon",
    "wyrm",
    "basilisk",
    "griffin",
    "manticore",
    "troll",
    "werewolf",
    "kraken",
    "chimera",
    "serpent",
    "direwolf",
    "wendigo",
    "wyvern",
    "drake",
    "hydra",
    "cockatrice",
    "salamander",
    "leviathan",
    "harpy",
    "gorgon",
    "minotaur",
    "sphinx",
    "roc",
    "ogre",
    "giant",
    "ettin",
    "yeti",
    "behemoth",
    "colossus",
    "golem",
    "gargoyle",
    "homunculus",
    "banshee",
    "wraith",
    "revenant",
    "ghoul",
    "lich",
    "vampire",
    "wight",
    "draugr",
    "shade",
    "nightmare",
    "kelpie",
    "siren",
    "hellhound",
    "barghest",
    "boggart",
    "sabrecat",
];

/// The rank of a hero.
pub const RANKS: [&str; 8] = [
    "Ser", "Dame", "Captain", "Brother", "Sister", "Master", "Mistress", "Old",
];

/// The first word of a tavern name.
pub const TAVERN_QUALIFIERS: [&str; 57] = [
    "Golden",
    "Crooked",
    "Sleeping",
    "Laughing",
    "Drowned",
    "Rusty",
    "Silver",
    "Hungry",
    "Painted",
    "Old",
    "Weeping",
    "Merry",
    "Broken",
    "Bent",
    "Leaning",
    "Battered",
    "Cracked",
    "Tarnished",
    "Hollow",
    "Dancing",
    "Singing",
    "Whistling",
    "Grinning",
    "Jolly",
    "Roaring",
    "Prancing",
    "Galloping",
    "Thirsty",
    "Drunken",
    "Weary",
    "Idle",
    "Restless",
    "Limping",
    "Blind",
    "Toothless",
    "Green",
    "Blue",
    "Black",
    "White",
    "Scarlet",
    "Copper",
    "Brazen",
    "Gilded",
    "Lucky",
    "Lost",
    "Wayward",
    "Wandering",
    "Quiet",
    "Sly",
    "Fat",
    "Bold",
    "Proud",
    "Hidden",
    "Wild",
    "Salty",
    "Smoky",
    "Frosty",
];

/// The second word of a tavern name.
pub const TAVERN_SUBJECTS: [&str; 102] = [
    "Stag",
    "Anchor",
    "Boar",
    "Lantern",
    "Kettle",
    "Griffin",
    "Mermaid",
    "Wheel",
    "Crow",
    "Hound",
    "Bell",
    "Otter",
    "Pilgrim",
    "Gate",
    "Barrel",
    "Cask",
    "Tankard",
    "Flagon",
    "Goblet",
    "Jug",
    "Ladle",
    "Hearth",
    "Smith",
    "Miller",
    "Cooper",
    "Drover",
    "Sailor",
    "Widow",
    "Maiden",
    "Monk",
    "Friar",
    "Knight",
    "Squire",
    "Herald",
    "Minstrel",
    "Piper",
    "Fiddler",
    "Jester",
    "Hammer",
    "Anvil",
    "Plough",
    "Cart",
    "Wagon",
    "Saddle",
    "Horseshoe",
    "Compass",
    "Beacon",
    "Goose",
    "Swan",
    "Magpie",
    "Raven",
    "Rook",
    "Wren",
    "Cat",
    "Fox",
    "Badger",
    "Hare",
    "Ram",
    "Bull",
    "Ox",
    "Pony",
    "Mare",
    "Goat",
    "Salmon",
    "Trout",
    "Pike",
    "Eel",
    "Whale",
    "Crab",
    "Dragon",
    "Wyvern",
    "Unicorn",
    "Phoenix",
    "Serpent",
    "Lion",
    "Bear",
    "Wolf",
    "Star",
    "Moon",
    "Sun",
    "Bridge",
    "Ford",
    "Mill",
    "Well",
    "Oak",
    "Willow",
    "Yew",
    "Thorn",
    "Rose",
    "Thistle",
    "Crown",
    "Sword",
    "Shield",
    "Helm",
    "Arrow",
    "Key",
    "Chain",
    "Boot",
    "Glove",
    "Cloak",
    "Purse",
    "Coin",
];

/// The names of a hero without a culture.
pub const NAMELESS: [&str; 10] = [
    "Aldric", "Marek", "Ysolde", "Corvin", "Nadia", "Halvar", "Ilona", "Tarrin", "Sable", "Ostrik",
];

/// The wild land at a height of 70 or more.
pub const HIGHLAND_TERMS: [&str; 2] = ["mountains", "mountain passes"];

/// The wild land at a height of 50 or more.
pub const UPLAND_TERMS: [&str; 2] = ["hills", "highlands"];

/// The wild land of a biome without a term.
pub const UNKNOWN_WILD: &str = "wilderness";

/// The journey type of a plain trip from A to B.
pub const DEFAULT_JOURNEY_TYPE: &str = "Travel";

/// The wild land of each biome, by biome id (`BIOME_TERMS`). The biome
/// names of FMG are in the comments.
pub const BIOME_TERMS: [&str; 13] = [
    "open water",  //   Marine
    "desert",      // Hot desert
    "cold desert", // Cold desert
    "savanna",     //   Savanna
    "grasslands",  //   Grassland
    "jungle",      // Tropical seasonal forest
    "forest",      // Temperate deciduous forest
    "rainforest",  // Tropical rainforest
    "rainforest",  // Temperate rainforest
    "pine forest", //   Taiga
    "tundra",      //   Tundra
    "ice",         //   Glacier
    "marshes",     //   Wetland
];

/// Names of a travel leg (`LEG_NAMES`). Tokens: `{from}`, `{to}`, `{wild}`,
/// and `{water}` for water legs.
pub struct LegNames {
    pub first: &'static [&'static str],
    pub second: &'static [&'static str],
    pub whole: &'static [&'static str],
}

/// Names of a land leg (`LEG_NAMES.land`).
pub struct LandLegNames {
    pub first: &'static [&'static str],
    pub second: &'static [&'static str],
    pub off_road_last: &'static [&'static str],
    pub off_road: &'static [&'static str],
    pub opening: &'static [&'static str],
    pub closing: &'static [&'static str],
    pub whole: &'static [&'static str],
}

/// The air leg names.
pub const AIR_LEGS: LegNames = LegNames {
    first: &[
        "Takeoff from {from}",
        "Up over the {wild}",
        "Leaving {from} by air",
    ],
    second: &["Landing at {to}", "Down to {to}", "Approach to {to}"],
    whole: &[
        "Flight to {to}",
        "Over the {wild} to {to}",
        "{from} to {to} by air",
        "Air route to {to}",
    ],
};

/// The water leg names.
pub const WATER_LEGS: LegNames = LegNames {
    first: &[
        "Out of {from} port",
        "Sailing from {from}",
        "Leaving {from} by sea",
    ],
    second: &["Landing at {to}", "Into {to} port", "Last stretch to {to}"],
    whole: &[
        "Sailing to {to}",
        "By sea to {to}",
        "Across {water} to {to}",
        "{from} to {to} by sea",
    ],
};

/// The land leg names.
pub const LAND_LEGS: LandLegNames = LandLegNames {
    first: &[
        "Into the {wild}",
        "Out of {from} into the {wild}",
        "{from} to the {wild}",
    ],
    second: &[
        "On to {to}",
        "Out of the {wild} to {to}",
        "Last stretch to {to}",
    ],
    off_road_last: &[
        "Off-road to {to}",
        "Out of the {wild} to {to}",
        "Into {to} the back way",
    ],
    off_road: &[
        "Off-road to {to}",
        "Through the {wild}",
        "Across the {wild} to {to}",
        "Around the road to {to}",
    ],
    opening: &[
        "Out of {from}",
        "{from} to {to}",
        "The road from {from}",
        "Setting out for {to}",
    ],
    closing: &["Last miles to {to}", "The road into {to}", "Down to {to}"],
    whole: &[
        "{from} to {to}",
        "On to {to}",
        "The road to {to}",
        "The {wild} road",
    ],
};

/// Names of the stops that no archetype names (`HALT_NAMES`).
pub mod halt {

    /// A gathering before the journey. Tokens: `{place}`, `{tavern}`.
    pub const MUSTER: [&str; 3] = [
        "Gathering in {place}",
        "Getting supplies in {place}",
        "Meeting at {tavern}",
    ];

    /// A party that takes passage on the ship of someone else. Token: `{from}`.
    pub const HARBOR_WAIT: [&str; 3] = [
        "Waiting for a ship in {from}",
        "Held up in {from} port",
        "Booking passage in {from}",
    ];

    /// A party that sails its own ship. Token: `{from}`.
    pub const CASTING_OFF: [&str; 3] = [
        "Loading at {from} quay",
        "Tide and weather in {from}",
        "Making ready in {from} port",
    ];

    /// A night at sea near a named place. Token: `{label}`.
    pub const ANCHORED_NEAR: [&str; 3] = [
        "Anchored {label}",
        "A night at anchor {label}",
        "Stopped {label}",
    ];

    /// A night at sea away from a named place. Token: `{water}`.
    pub const ANCHORED: [&str; 3] = [
        "Night at anchor",
        "A night out on {water}",
        "Anchored till morning",
    ];

    /// A camp near a named place. Token: `{label}`.
    pub const CAMP_NEAR: [&str; 2] = ["Camp {label}", "A night {label}"];
}

/// One kind of party (`JourneyArchetype`).
pub struct Archetype {
    /// The FMG key, for example `heroes`.
    pub key: &'static str,
    /// The journey type, for example `Quest`.
    pub kind: &'static str,
    /// How often the party comes, relative to the others.
    pub weight: u32,
    /// The weight of each domain, in FMG order. The heaviest domain is
    /// what the party is.
    pub domains: &'static [(Domain, u32)],
    /// The weighted transport names of each domain.
    pub transports: &'static [(Domain, &'static [(&'static str, u32)])],
    /// The chance that a land leg leaves the road network.
    pub off_road: f64,
    /// The chance of a stop at a place between two legs.
    pub rest: f64,
    /// The chance that a camp in the wild splits a long leg.
    pub camp: f64,
    /// Journey titles. Tokens: `{hero}`, `{origin}`, `{destination}`,
    /// `{destinationAdjective}`, `{wild}`.
    pub title: &'static [&'static str],
    /// Land leg names in the voice of the party. Tokens: `{to}`, `{wild}`.
    pub leg: Option<&'static [&'static str]>,
    /// Names of a night in a place. Token: `{place}`.
    pub stopover: &'static [&'static str],
    /// Names of a night in the open. Token: `{wild}`.
    pub bivouac: &'static [&'static str],
}

/// The archetypes of FMG, in its order (`JOURNEY_ARCHETYPES`).
pub const ARCHETYPES: [Archetype; 18] = [
    Archetype {
        key: "heroes",
        kind: "Quest",
        weight: 10,
        domains: &[(Domain::Land, 7), (Domain::Water, 2)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (light)", 4),
                    ("Horseback (no spare horse)", 4),
                    ("Carriage", 1),
                ],
            ),
            (
                Domain::Water,
                &[("Sailing boat", 3), ("Sailing Ship", 2), ("Rowboat", 1)],
            ),
        ],
        off_road: 0.4,
        rest: 0.7,
        camp: 0.7,
        title: &[
            "The Company of the {company} {banner}",
            "The Quest of {rank} {hero}",
            "{hero}'s Company",
            "The {company} {banner} rides to {destination}",
            "{hero} and the road through the {wild}",
        ],
        leg: Some(&["The road to {to}", "Through the {wild}", "Riding for {to}"]),
        stopover: &[
            "A night at {tavern}",
            "Rumours in {place}",
            "Resupply in {place}",
        ],
        bivouac: &[
            "Camp in the {wild}",
            "Watches kept in the {wild}",
            "A fire in the {wild}",
        ],
    },
    Archetype {
        key: "wanderer",
        kind: "Wandering",
        weight: 4,
        domains: &[(Domain::Land, 8), (Domain::Water, 2)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (light)", 5),
                    ("On foot (laden)", 2),
                    ("Horseback (no spare horse)", 1),
                ],
            ),
            (
                Domain::Water,
                &[("Rowboat", 3), ("Sailing boat", 2), ("Sailing Ship", 1)],
            ),
        ],
        off_road: 0.5,
        rest: 0.6,
        camp: 0.7,
        title: &[
            "{rank} {hero} of {origin}",
            "The Long Road of {hero}",
            "{hero} walks to {destination}",
            "The {company} Wanderer",
        ],
        leg: Some(&["Walking to {to}", "Alone to {to}", "The road to {to}"]),
        stopover: &[
            "A bed at {tavern}",
            "Working for board in {place}",
            "A night in {place}",
        ],
        bivouac: &[
            "Sleeping rough in the {wild}",
            "A cold night in the {wild}",
            "Alone in the {wild}",
        ],
    },
    Archetype {
        key: "caravan",
        kind: "Caravan",
        weight: 4,
        domains: &[(Domain::Land, 6), (Domain::Water, 3)],
        transports: &[
            (Domain::Land, &[("Carriage", 3), ("On foot (laden)", 1)]),
            (Domain::Water, &[("Sailing Ship", 4), ("Sailing boat", 1)]),
        ],
        off_road: 0.1,
        rest: 0.8,
        camp: 0.4,
        title: &[
            "The {company} Caravan",
            "{cargo} Road to {destination}",
            "The {origin} Caravan",
            "{cargo} out of {origin}",
        ],
        leg: Some(&["Hauling to {to}", "The road to {to}", "Toll road to {to}"]),
        stopover: &[
            "Wagon yard at {tavern}",
            "A night at {tavern}",
            "Market day in {place}",
        ],
        bivouac: &[
            "Wagons circled in the {wild}",
            "Night halt in the {wild}",
            "Cold camp in the {wild}",
        ],
    },
    Archetype {
        key: "campaign",
        kind: "Military campaign",
        weight: 4,
        domains: &[(Domain::Land, 7), (Domain::Water, 3)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (laden)", 5),
                    ("Horseback (no spare horse)", 3),
                    ("Carriage", 1),
                ],
            ),
            (Domain::Water, &[("Sailing Ship", 5), ("Sailing boat", 1)]),
        ],
        off_road: 0.25,
        rest: 0.5,
        camp: 0.8,
        title: &[
            "The March on {destination}",
            "{rank} {hero}'s Host",
            "The {destinationAdjective} Campaign",
            "The {company} Host",
        ],
        leg: Some(&[
            "March on {to}",
            "The column crosses the {wild}",
            "Forced march to {to}",
        ]),
        stopover: &[
            "Billeted in {place}",
            "Requisitions in {place}",
            "{place} opens its gates",
        ],
        bivouac: &[
            "War camp in the {wild}",
            "Pickets set in the {wild}",
            "Night muster in the {wild}",
        ],
    },
    Archetype {
        key: "embassy",
        kind: "Embassy",
        weight: 2,
        domains: &[(Domain::Land, 6), (Domain::Water, 4)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("Carriage", 4),
                    ("Stagecoach", 3),
                    ("Horseback (no spare horse)", 3),
                    ("On foot (light)", 1),
                ],
            ),
            (Domain::Water, &[("Sailing Ship", 5), ("Sailing boat", 1)]),
        ],
        off_road: 0.05,
        rest: 0.9,
        camp: 0.25,
        title: &[
            "Embassy to {destination}",
            "The {destinationAdjective} Mission",
            "{rank} {hero} goes to {destination}",
            "Errand to the court of {destination}",
        ],
        leg: None,
        stopover: &[
            "Guested at {tavern}",
            "Audience in {place}",
            "Two nights in {place}",
        ],
        bivouac: &[
            "Escort camp in the {wild}",
            "Night under guard in the {wild}",
        ],
    },
    Archetype {
        key: "pilgrimage",
        kind: "Pilgrimage",
        weight: 4,
        domains: &[(Domain::Land, 8), (Domain::Water, 2)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (laden)", 4),
                    ("On foot (light)", 3),
                    ("Horseback (no spare horse)", 1),
                ],
            ),
            (
                Domain::Water,
                &[("Sailing boat", 3), ("Sailing Ship", 2), ("Rowboat", 1)],
            ),
        ],
        off_road: 0.35,
        rest: 0.7,
        camp: 0.6,
        title: &[
            "Pilgrimage to {destination}",
            "The {company} Pilgrims",
            "The Long Walk to {destination}",
            "{rank} {hero} walks to {destination}",
        ],
        leg: Some(&["Barefoot to {to}", "On to {to}", "The pilgrim road to {to}"]),
        stopover: &[
            "Alms and rest at {tavern}",
            "Vigil in {place}",
            "Shelter in {place}",
        ],
        bivouac: &[
            "Vigil in the {wild}",
            "Night prayer in the {wild}",
            "Sleeping rough in the {wild}",
        ],
    },
    Archetype {
        key: "mercenaries",
        kind: "Mercenary contract",
        weight: 3,
        domains: &[(Domain::Land, 7), (Domain::Water, 3)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("Horseback (no spare horse)", 5),
                    ("On foot (laden)", 3),
                    ("Horseback (spare horse)", 1),
                    ("Carriage", 1),
                ],
            ),
            (Domain::Water, &[("Sailing Ship", 4), ("Sailing boat", 2)]),
        ],
        off_road: 0.3,
        rest: 0.6,
        camp: 0.6,
        title: &[
            "The {company} Blades",
            "{hero}'s Free Company",
            "Contract to {destination}",
            "The {company} {banner} takes coin in {destination}",
        ],
        leg: Some(&["Riding for {to}", "Paid road to {to}", "On to {to}"]),
        stopover: &[
            "Drinking the advance at {tavern}",
            "Recruiting in {place}",
            "Paid off in {place}",
        ],
        bivouac: &[
            "Camp in the {wild}",
            "Cold camp in the {wild}",
            "Dice and watches in the {wild}",
        ],
    },
    Archetype {
        key: "courier",
        kind: "Courier ride",
        weight: 3,
        domains: &[(Domain::Land, 7), (Domain::Water, 3)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("Horseback (spare horse)", 3),
                    ("Stagecoach", 1),
                    ("On foot (light)", 1),
                ],
            ),
            (Domain::Water, &[("Sailing boat", 3), ("Sailing Ship", 3)]),
        ],
        off_road: 0.15,
        rest: 0.5,
        camp: 0.3,
        title: &[
            "The Ride to {destination}",
            "News out of {origin}",
            "{hero} carries word to {destination}",
            "The {company} Post",
        ],
        leg: Some(&[
            "Hard riding to {to}",
            "Fast road to {to}",
            "Post road to {to}",
        ]),
        stopover: &[
            "Change of horses in {place}",
            "Three hours at {tavern}",
            "Fresh mount in {place}",
        ],
        bivouac: &["Snatched sleep in the {wild}", "Horse rested in the {wild}"],
    },
    Archetype {
        key: "expedition",
        kind: "Expedition",
        weight: 3,
        domains: &[(Domain::Land, 7), (Domain::Water, 3)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (laden)", 5),
                    ("Horseback (no spare horse)", 2),
                    ("On foot (light)", 1),
                ],
            ),
            (
                Domain::Water,
                &[("Sailing boat", 3), ("Sailing Ship", 2), ("Rowboat", 1)],
            ),
        ],
        off_road: 0.7,
        rest: 0.5,
        camp: 0.8,
        title: &[
            "Expedition to the {wild}",
            "The {company} Expedition",
            "Survey of the {wild}",
            "{rank} {hero}'s survey of the road to {destination}",
        ],
        leg: Some(&["Mapping the {wild}", "Into the {wild}", "Traverse to {to}"]),
        stopover: &[
            "Resupply in {place}",
            "Notes and repairs at {tavern}",
            "Hired guides in {place}",
        ],
        bivouac: &[
            "Base camp in the {wild}",
            "Survey camp in the {wild}",
            "Weathered in on the {wild}",
        ],
    },
    Archetype {
        key: "refugees",
        kind: "Refugee flight",
        weight: 3,
        domains: &[(Domain::Land, 6), (Domain::Water, 3)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (laden)", 7),
                    ("Carriage", 2),
                    ("On foot (light)", 1),
                ],
            ),
            (
                Domain::Water,
                &[("Rowboat", 3), ("Sailing boat", 2), ("Sailing Ship", 1)],
            ),
        ],
        off_road: 0.4,
        rest: 0.5,
        camp: 0.8,
        title: &[
            "The Road out of {origin}",
            "{origin} on the Move",
            "The {company} Column",
            "Seeking shelter in {destination}",
        ],
        leg: Some(&["Trudging to {to}", "The long walk to {to}", "On to {to}"]),
        stopover: &[
            "Turned away at {place}",
            "Bread and straw in {place}",
            "Counting the missing in {place}",
        ],
        bivouac: &[
            "Camp in the {wild}",
            "A hungry night in the {wild}",
            "Burying the dead in the {wild}",
        ],
    },
    Archetype {
        key: "relicseekers",
        kind: "Treasure hunt",
        weight: 3,
        domains: &[(Domain::Land, 7), (Domain::Water, 3)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (light)", 4),
                    ("Horseback (no spare horse)", 3),
                    ("On foot (laden)", 1),
                ],
            ),
            (Domain::Water, &[("Sailing boat", 3), ("Sailing Ship", 2)]),
        ],
        off_road: 0.6,
        rest: 0.6,
        camp: 0.7,
        title: &[
            "The Search for the {relic}",
            "{hero} and the {relic}",
            "The {relic} lies in the {wild}",
            "Digging for the {relic}",
        ],
        leg: Some(&[
            "Following the map into the {wild}",
            "On to {to}",
            "Old roads to {to}",
        ]),
        stopover: &[
            "Buying rumours at {tavern}",
            "Bribing a clerk in {place}",
            "Lying low in {place}",
        ],
        bivouac: &[
            "Camp in the {wild}",
            "Reading the map in the {wild}",
            "A watchful night in the {wild}",
        ],
    },
    Archetype {
        key: "monsterhunt",
        kind: "Monster hunt",
        weight: 3,
        domains: &[(Domain::Land, 8), (Domain::Water, 2)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("Horseback (no spare horse)", 4),
                    ("On foot (light)", 4),
                    ("On foot (laden)", 1),
                ],
            ),
            (
                Domain::Water,
                &[("Sailing boat", 3), ("Rowboat", 2), ("Sailing Ship", 1)],
            ),
        ],
        off_road: 0.65,
        rest: 0.6,
        camp: 0.7,
        title: &[
            "The Hunt for the {wild} {beast}",
            "{rank} {hero} hunts the {beast}",
            "The {beast} of {destination}",
            "Bounty on the {beast}",
        ],
        leg: Some(&[
            "Following the trail into the {wild}",
            "The kills lead to {to}",
            "Tracking to {to}",
        ]),
        stopover: &[
            "Questioning the locals in {place}",
            "Bounty posted in {place}",
            "A night at {tavern}",
        ],
        bivouac: &[
            "Blind camp in the {wild}",
            "Bait set in the {wild}",
            "No fire tonight: {wild}",
        ],
    },
    Archetype {
        key: "exiles",
        kind: "Exile's flight",
        weight: 2,
        domains: &[(Domain::Water, 6), (Domain::Land, 4)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (laden)", 5),
                    ("Carriage", 2),
                    ("On foot (light)", 2),
                    ("Horseback (no spare horse)", 1),
                ],
            ),
            (
                Domain::Water,
                &[("Rowboat", 3), ("Sailing boat", 2), ("Sailing Ship", 2)],
            ),
        ],
        off_road: 0.6,
        rest: 0.4,
        camp: 0.7,
        title: &[
            "Flight from {origin}",
            "The {company} Exiles",
            "Exodus from {origin}",
            "{hero} is banished to {destination}",
        ],
        leg: None,
        stopover: &[
            "Hidden a night in {place}",
            "Begging bread in {place}",
            "Back room of {tavern}",
        ],
        bivouac: &[
            "Fireless camp in the {wild}",
            "Hiding in the {wild}",
            "A cold night in the {wild}",
        ],
    },
    Archetype {
        key: "smugglers",
        kind: "Smuggling run",
        weight: 2,
        domains: &[(Domain::Water, 6), (Domain::Land, 4)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (light)", 3),
                    ("Horseback (no spare horse)", 3),
                    ("On foot (laden)", 1),
                    ("Carriage", 1),
                ],
            ),
            (
                Domain::Water,
                &[("Rowboat", 4), ("Sailing boat", 3), ("Sailing Ship", 1)],
            ),
        ],
        off_road: 0.8,
        rest: 0.35,
        camp: 0.6,
        title: &[
            "The {company} Run",
            "Smugglers' road to {destination}",
            "The {cargo} Run",
            "Untaxed to {destination}",
        ],
        leg: Some(&[
            "Quiet road to {to}",
            "Around the tollgates to {to}",
            "Night haul to {to}",
        ]),
        stopover: &[
            "Lying low at {tavern}",
            "Unloading quietly in {place}",
            "Palms greased in {place}",
        ],
        bivouac: &[
            "Cache dug in the {wild}",
            "No fire, no names: {wild}",
            "Waiting out the patrol in the {wild}",
        ],
    },
    Archetype {
        key: "raiders",
        kind: "Raid",
        weight: 2,
        domains: &[(Domain::Water, 8), (Domain::Land, 2)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (light)", 4),
                    ("Horseback (spare horse)", 3),
                    ("Horseback (no spare horse)", 1),
                ],
            ),
            (
                Domain::Water,
                &[("Sailing Ship", 4), ("Sailing boat", 3), ("Rowboat", 1)],
            ),
        ],
        off_road: 0.6,
        rest: 0.3,
        camp: 0.6,
        title: &[
            "The {company} Reavers",
            "{hero}'s Raid",
            "Raid on {destination}",
            "The {origin} Keels",
        ],
        leg: Some(&[
            "Falling on {to}",
            "Overland to {to}",
            "Driving the herd to {to}",
        ]),
        stopover: &[
            "Dividing the take in {place}",
            "Selling the plunder in {place}",
            "Drinking {place} dry",
        ],
        bivouac: &[
            "Beach camp in the {wild}",
            "Keel hauled up in the {wild}",
            "A watchful night in the {wild}",
        ],
    },
    Archetype {
        key: "progress",
        kind: "Royal progress",
        weight: 2,
        domains: &[(Domain::Land, 7), (Domain::Water, 3)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("Carriage", 6),
                    ("Stagecoach", 3),
                    ("Horseback (no spare horse)", 3),
                    ("On foot (laden)", 1),
                ],
            ),
            (Domain::Water, &[("Sailing Ship", 6), ("Sailing boat", 1)]),
        ],
        off_road: 0.02,
        rest: 0.95,
        camp: 0.1,
        title: &[
            "The Progress of {rank} {hero}",
            "The {destinationAdjective} Progress",
            "The Crown rides out of {origin}",
            "The {company} Progress",
        ],
        leg: Some(&[
            "The royal road to {to}",
            "Received at {to}",
            "Procession to {to}",
        ]),
        stopover: &[
            "Feasted in {place}",
            "Court held in {place}",
            "Stay in {place}",
        ],
        bivouac: &[
            "Pavilions raised in the {wild}",
            "The hunt camps in the {wild}",
        ],
    },
    Archetype {
        key: "skyfarers",
        kind: "Airship voyage",
        weight: 2,
        domains: &[(Domain::Air, 12), (Domain::Land, 1), (Domain::Water, 1)],
        transports: &[
            (
                Domain::Land,
                &[("Horseback (no spare horse)", 3), ("On foot (light)", 2)],
            ),
            (Domain::Water, &[("Sailing boat", 2), ("Sailing Ship", 2)]),
            (Domain::Air, &[("Dirigible", 6), ("Teleport", 1)]),
        ],
        off_road: 0.1,
        rest: 0.2,
        camp: 0.1,
        title: &[
            "The {company} Airship",
            "{hero} sails over {origin}",
            "Skyroad to {destination}",
            "The {banner} takes to the air",
        ],
        leg: Some(&[
            "Over the {wild} to {to}",
            "Above the {wild}",
            "Sky lane to {to}",
        ]),
        stopover: &[
            "Moored over {place}",
            "Gas and ballast in {place}",
            "A night at {tavern}",
        ],
        bivouac: &[
            "Grounded in the {wild}",
            "Repairs in the {wild}",
            "Anchored above the {wild}",
        ],
    },
    Archetype {
        key: "arcanists",
        kind: "Arcane errand",
        weight: 2,
        domains: &[(Domain::Land, 4), (Domain::Air, 3), (Domain::Water, 1)],
        transports: &[
            (
                Domain::Land,
                &[
                    ("On foot (light)", 3),
                    ("Horseback (no spare horse)", 2),
                    ("Carriage", 1),
                ],
            ),
            (Domain::Water, &[("Sailing boat", 2), ("Sailing Ship", 1)]),
            (Domain::Air, &[("Teleport", 4), ("Dirigible", 1)]),
        ],
        off_road: 0.4,
        rest: 0.5,
        camp: 0.3,
        title: &[
            "The {company} Circle",
            "{rank} {hero} carries the {relic}",
            "The {relic} must reach {destination}",
            "{hero} steps out of {origin}",
            "The short road to {destination}",
        ],
        leg: Some(&[
            "Warding the road to {to}",
            "Through the {wild} unseen",
            "The long way round to {to}",
        ]),
        stopover: &[
            "Reading the signs in {place}",
            "A sealed room at {tavern}",
            "A night of study in {place}",
        ],
        bivouac: &[
            "A circle drawn in the {wild}",
            "Wards set in the {wild}",
            "A silent camp in the {wild}",
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journeys::transports::{self, default_transports};
    use std::collections::BTreeSet;

    /// The modern types stay in the list for the user. Generation never
    /// takes them.
    const MODERN: [&str; 8] = [
        "Train",
        "Automobile",
        "Modern Automobile",
        "Steamship",
        "Modern Ship",
        "Aircraft",
        "Helicopter",
        "Modern Airplane",
    ];

    #[test]
    fn each_preferred_transport_exists_in_its_domain() {
        for a in &ARCHETYPES {
            for (domain, weights) in a.transports {
                for (name, _) in *weights {
                    let list = default_transports();
                    let t = transports::get(&list, name);
                    assert_eq!(t.map(|t| t.domain), Some(*domain), "{}: {name}", a.key);
                }
            }
        }
    }

    #[test]
    fn transports_cover_exactly_the_domains_of_the_party() {
        for a in &ARCHETYPES {
            let travels: BTreeSet<Domain> = a.domains.iter().map(|d| d.0).collect();
            let rides: BTreeSet<Domain> = a.transports.iter().map(|t| t.0).collect();
            assert_eq!(travels, rides, "{}", a.key);
            assert!(a.domains.iter().all(|d| d.1 > 0), "{}", a.key);
        }
    }

    #[test]
    fn modern_types_are_for_the_user_and_the_rest_are_used() {
        let named: BTreeSet<&str> = ARCHETYPES
            .iter()
            .flat_map(|a| a.transports.iter().flat_map(|t| t.1.iter().map(|w| w.0)))
            .collect();
        assert!(named.iter().all(|n| !MODERN.contains(n)));
        let list = default_transports();
        let unused: Vec<&str> = list
            .iter()
            .filter(|t| t.domain != Domain::Stay && !MODERN.contains(&t.name.as_str()))
            .filter(|t| !named.contains(t.name.as_str()))
            .map(|t| t.name.as_str())
            .collect();
        assert!(unused.is_empty(), "{unused:?}");
    }

    #[test]
    fn the_tables_have_the_sizes_of_fmg() {
        assert_eq!(ARCHETYPES.len(), 18);
        assert_eq!(ARCHETYPES.iter().map(|a| a.weight).sum::<u32>(), 58);
        assert_eq!(BIOME_TERMS.len(), crate::biomes::NAMES.len());
        assert!(
            ARCHETYPES
                .iter()
                .flat_map(|a| a.bivouac.iter())
                .all(|b| !b.contains('\u{2014}'))
        );
    }
}
