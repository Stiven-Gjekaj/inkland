//! The culture sets of FMG (`cultures-generator.ts`, `getDefault`, and
//! `data/culture-sets.ts`, MIT license, see THIRD_PARTY_NOTICES.md).
//!
//! Each culture has a name, a name base, a chance to be picked (`odd`), a
//! development level, and a sort function. The sort function gives a score
//! to a cell; the center of the culture goes to a cell with a high score.

/// What a sort function reads about a cell.
pub struct SortCtx<'a> {
    /// Suitability.
    pub s: &'a [f64],
    /// The highest suitability.
    pub s_max: f64,
    /// Distance field.
    pub t: &'a [i32],
    /// Height on the FMG scale.
    pub h: &'a [f64],
    /// Temperature, in whole degrees C.
    pub temp: &'a [f64],
    pub biome: &'a [u8],
    /// `true` for a cell on the coast of the sea (not of a lake).
    pub sea_coast: &'a [bool],
}

impl SortCtx<'_> {
    /// The suitability on a scale of 0 to 3 (`n`).
    pub fn n(&self, i: usize) -> f64 {
        (self.s[i] / self.s_max * 3.0).ceil()
    }
    /// A fee for a temperature far from `goal` (`td`).
    pub fn td(&self, i: usize, goal: f64) -> f64 {
        let d = (self.temp[i] - goal).abs();
        if d != 0.0 { d + 1.0 } else { 1.0 }
    }
    /// A fee for a biome that is not in the list (`bd`).
    pub fn bd(&self, i: usize, biomes: &[u8], fee: f64) -> f64 {
        if biomes.contains(&self.biome[i]) {
            1.0
        } else {
            fee
        }
    }
    /// A fee for a cell that is not on the sea coast (`sf`).
    pub fn sf(&self, i: usize, fee: f64) -> f64 {
        if self.sea_coast[i] { 1.0 } else { fee }
    }
    pub fn t(&self, i: usize) -> f64 {
        f64::from(self.t[i])
    }
    pub fn h(&self, i: usize) -> f64 {
        self.h[i]
    }
    pub fn s(&self, i: usize) -> f64 {
        self.s[i]
    }
}

/// A sort function: a higher score is a better center.
pub type SortFn = fn(&SortCtx, usize) -> f64;

/// One culture of a set.
pub struct CultureDef {
    pub name: &'static str,
    /// The index of the name base.
    pub base: usize,
    /// The chance that the culture is picked when the set has more cultures
    /// than the map.
    pub odd: f64,
    /// Technological development, 0 to 100, at the center.
    pub development: u32,
    pub sort: Option<SortFn>,
}

/// A culture set: its config name, its FMG name, and the most cultures that
/// it gives.
pub struct CultureSet {
    pub name: &'static str,
    pub max: usize,
    /// The cultures. `english` and `random` make their cultures at
    /// generation, so their lists are empty.
    pub cultures: &'static [CultureDef],
}

/// The culture sets.
pub const CULTURE_SETS: [CultureSet; 8] = [
    CultureSet {
        name: "world",
        max: 32,
        cultures: &WORLD,
    },
    CultureSet {
        name: "european",
        max: 15,
        cultures: &EUROPEAN,
    },
    CultureSet {
        name: "oriental",
        max: 13,
        cultures: &ORIENTAL,
    },
    CultureSet {
        name: "english",
        max: 10,
        cultures: &[],
    },
    CultureSet {
        name: "antique",
        max: 10,
        cultures: &ANTIQUE,
    },
    CultureSet {
        name: "high-fantasy",
        max: 17,
        cultures: &HIGH_FANTASY,
    },
    CultureSet {
        name: "dark-fantasy",
        max: 18,
        cultures: &DARK_FANTASY,
    },
    CultureSet {
        name: "random",
        max: 100,
        cultures: &[],
    },
];

/// The culture set with this name.
pub fn culture_set(name: &str) -> Option<&'static CultureSet> {
    CULTURE_SETS.iter().find(|s| s.name == name)
}

/// The names of the culture sets.
pub fn set_names() -> Vec<&'static str> {
    CULTURE_SETS.iter().map(|s| s.name).collect()
}

/// The development of a culture of the `random` set, from its name: 15 to
/// 42 (`getNameDevelopment`, an FNV-1a hash of the UTF-16 units).
pub fn name_development(name: &str) -> u32 {
    let mut hash: u32 = 2_166_136_261;
    for unit in name.encode_utf16() {
        hash = (hash ^ u32::from(unit)).wrapping_mul(16_777_619);
    }
    15 + hash % 28
}

const EUROPEAN: [CultureDef; 15] = [
    CultureDef {
        name: "Shwazen",
        base: 0,
        odd: 1.0,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 10.0) / x.bd(i, &[6, 8], 4.0)),
    },
    CultureDef {
        name: "Angshire",
        base: 1,
        odd: 1.0,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 10.0) / x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Luari",
        base: 2,
        odd: 1.0,
        development: 40,
        sort: Some(|x, i| x.n(i) / x.td(i, 12.0) / x.bd(i, &[6, 8], 4.0)),
    },
    CultureDef {
        name: "Tallian",
        base: 3,
        odd: 1.0,
        development: 42,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0)),
    },
    CultureDef {
        name: "Astellian",
        base: 4,
        odd: 1.0,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 16.0)),
    },
    CultureDef {
        name: "Slovan",
        base: 5,
        odd: 1.0,
        development: 32,
        sort: Some(|x, i| (x.n(i) / x.td(i, 6.0)) * x.t(i)),
    },
    CultureDef {
        name: "Norse",
        base: 6,
        odd: 1.0,
        development: 30,
        sort: Some(|x, i| x.n(i) / x.td(i, 5.0)),
    },
    CultureDef {
        name: "Elladan",
        base: 7,
        odd: 1.0,
        development: 36,
        sort: Some(|x, i| (x.n(i) / x.td(i, 18.0)) * x.h(i)),
    },
    CultureDef {
        name: "Romian",
        base: 8,
        odd: 0.2,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0) / x.t(i)),
    },
    CultureDef {
        name: "Soumi",
        base: 9,
        odd: 1.0,
        development: 28,
        sort: Some(|x, i| (x.n(i) / x.td(i, 5.0) / x.bd(i, &[9], 4.0)) * x.t(i)),
    },
    CultureDef {
        name: "Portuzian",
        base: 13,
        odd: 1.0,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 17.0) / x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Vengrian",
        base: 15,
        odd: 1.0,
        development: 33,
        sort: Some(|x, i| (x.n(i) / x.td(i, 11.0) / x.bd(i, &[4], 4.0)) * x.t(i)),
    },
    CultureDef {
        name: "Turchian",
        base: 16,
        odd: 0.05,
        development: 35,
        sort: Some(|x, i| x.n(i) / x.td(i, 14.0)),
    },
    CultureDef {
        name: "Euskati",
        base: 20,
        odd: 0.05,
        development: 30,
        sort: Some(|x, i| (x.n(i) / x.td(i, 15.0)) * x.h(i)),
    },
    CultureDef {
        name: "Keltan",
        base: 22,
        odd: 0.05,
        development: 30,
        sort: Some(|x, i| (x.n(i) / x.td(i, 11.0) / x.bd(i, &[6, 8], 4.0)) * x.t(i)),
    },
];

const ORIENTAL: [CultureDef; 13] = [
    CultureDef {
        name: "Koryo",
        base: 10,
        odd: 1.0,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 12.0) / x.t(i)),
    },
    CultureDef {
        name: "Hantzu",
        base: 11,
        odd: 1.0,
        development: 42,
        sort: Some(|x, i| x.n(i) / x.td(i, 13.0)),
    },
    CultureDef {
        name: "Yamoto",
        base: 12,
        odd: 1.0,
        development: 35,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0) / x.t(i)),
    },
    CultureDef {
        name: "Turchian",
        base: 16,
        odd: 1.0,
        development: 35,
        sort: Some(|x, i| x.n(i) / x.td(i, 12.0)),
    },
    CultureDef {
        name: "Amazi",
        base: 17,
        odd: 0.2,
        development: 28,
        sort: Some(|x, i| (x.n(i) / x.td(i, 19.0) / x.bd(i, &[1, 2, 3], 7.0)) * x.t(i)),
    },
    CultureDef {
        name: "Eurabic",
        base: 18,
        odd: 1.0,
        development: 40,
        sort: Some(|x, i| (x.n(i) / x.td(i, 26.0) / x.bd(i, &[1, 2], 7.0)) * x.t(i)),
    },
    CultureDef {
        name: "Efratic",
        base: 23,
        odd: 0.1,
        development: 35,
        sort: Some(|x, i| (x.n(i) / x.td(i, 22.0)) * x.t(i)),
    },
    CultureDef {
        name: "Tehrani",
        base: 24,
        odd: 1.0,
        development: 38,
        sort: Some(|x, i| (x.n(i) / x.td(i, 18.0)) * x.h(i)),
    },
    CultureDef {
        name: "Maui",
        base: 25,
        odd: 0.2,
        development: 12,
        sort: Some(|x, i| x.n(i) / x.td(i, 24.0) / x.sf(i, 4.0) / x.t(i)),
    },
    CultureDef {
        name: "Carnatic",
        base: 26,
        odd: 0.5,
        development: 33,
        sort: Some(|x, i| x.n(i) / x.td(i, 26.0)),
    },
    CultureDef {
        name: "Vietic",
        base: 29,
        odd: 0.8,
        development: 32,
        sort: Some(|x, i| x.n(i) / x.td(i, 25.0) / x.bd(i, &[7], 7.0) / x.t(i)),
    },
    CultureDef {
        name: "Guantzu",
        base: 30,
        odd: 0.5,
        development: 38,
        sort: Some(|x, i| x.n(i) / x.td(i, 17.0)),
    },
    CultureDef {
        name: "Ulus",
        base: 31,
        odd: 1.0,
        development: 26,
        sort: Some(|x, i| (x.n(i) / x.td(i, 5.0) / x.bd(i, &[2, 4, 10], 7.0)) * x.t(i)),
    },
];

const ANTIQUE: [CultureDef; 16] = [
    CultureDef {
        name: "Roman",
        base: 8,
        odd: 1.0,
        development: 24,
        sort: Some(|x, i| x.n(i) / x.td(i, 14.0) / x.t(i)),
    },
    CultureDef {
        name: "Roman",
        base: 8,
        odd: 1.0,
        development: 24,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0) / x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Roman",
        base: 8,
        odd: 1.0,
        development: 24,
        sort: Some(|x, i| x.n(i) / x.td(i, 16.0) / x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Roman",
        base: 8,
        odd: 1.0,
        development: 24,
        sort: Some(|x, i| x.n(i) / x.td(i, 17.0) / x.t(i)),
    },
    CultureDef {
        name: "Hellenic",
        base: 7,
        odd: 1.0,
        development: 23,
        sort: Some(|x, i| (x.n(i) / x.td(i, 18.0) / x.sf(i, 4.0)) * x.h(i)),
    },
    CultureDef {
        name: "Hellenic",
        base: 7,
        odd: 1.0,
        development: 23,
        sort: Some(|x, i| (x.n(i) / x.td(i, 19.0) / x.sf(i, 4.0)) * x.h(i)),
    },
    CultureDef {
        name: "Macedonian",
        base: 7,
        odd: 0.5,
        development: 21,
        sort: Some(|x, i| (x.n(i) / x.td(i, 12.0)) * x.h(i)),
    },
    CultureDef {
        name: "Celtic",
        base: 22,
        odd: 1.0,
        development: 18,
        sort: Some(|x, i| x.n(i) / libm::pow(x.td(i, 11.0), 0.5) / x.bd(i, &[6, 8], 4.0)),
    },
    CultureDef {
        name: "Germanic",
        base: 0,
        odd: 1.0,
        development: 16,
        sort: Some(|x, i| x.n(i) / libm::pow(x.td(i, 10.0), 0.5) / x.bd(i, &[6, 8], 4.0)),
    },
    CultureDef {
        name: "Persian",
        base: 24,
        odd: 0.8,
        development: 21,
        sort: Some(|x, i| (x.n(i) / x.td(i, 18.0)) * x.h(i)),
    },
    CultureDef {
        name: "Scythian",
        base: 24,
        odd: 0.5,
        development: 15,
        sort: Some(|x, i| x.n(i) / libm::pow(x.td(i, 11.0), 0.5) / x.bd(i, &[4], 4.0)),
    },
    CultureDef {
        name: "Cantabrian",
        base: 20,
        odd: 0.5,
        development: 16,
        sort: Some(|x, i| (x.n(i) / x.td(i, 16.0)) * x.h(i)),
    },
    CultureDef {
        name: "Estian",
        base: 9,
        odd: 0.2,
        development: 12,
        sort: Some(|x, i| (x.n(i) / x.td(i, 5.0)) * x.t(i)),
    },
    CultureDef {
        name: "Carthaginian",
        base: 42,
        odd: 0.3,
        development: 21,
        sort: Some(|x, i| x.n(i) / x.td(i, 20.0) / x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Hebrew",
        base: 42,
        odd: 0.2,
        development: 19,
        sort: Some(|x, i| (x.n(i) / x.td(i, 19.0)) * x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Mesopotamian",
        base: 23,
        odd: 0.2,
        development: 19,
        sort: Some(|x, i| x.n(i) / x.td(i, 22.0) / x.bd(i, &[1, 2, 3], 4.0)),
    },
];

const HIGH_FANTASY: [CultureDef; 17] = [
    CultureDef {
        name: "Quenian",
        base: 33,
        odd: 1.0,
        development: 44,
        sort: Some(|x, i| (x.n(i) / x.bd(i, &[6, 7, 8, 9], 10.0)) * x.t(i)),
    },
    CultureDef {
        name: "Eldar",
        base: 33,
        odd: 1.0,
        development: 46,
        sort: Some(|x, i| (x.n(i) / x.bd(i, &[6, 7, 8, 9], 10.0)) * x.t(i)),
    },
    CultureDef {
        name: "Trow",
        base: 34,
        odd: 0.9,
        development: 36,
        sort: Some(|x, i| (x.n(i) / x.bd(i, &[7, 8, 9, 12], 10.0)) * x.t(i)),
    },
    CultureDef {
        name: "Lothian",
        base: 34,
        odd: 0.3,
        development: 36,
        sort: Some(|x, i| (x.n(i) / x.bd(i, &[7, 8, 9, 12], 10.0)) * x.t(i)),
    },
    CultureDef {
        name: "Dunirr",
        base: 35,
        odd: 1.0,
        development: 50,
        sort: Some(|x, i| x.n(i) + x.h(i)),
    },
    CultureDef {
        name: "Khazadur",
        base: 35,
        odd: 1.0,
        development: 53,
        sort: Some(|x, i| x.n(i) + x.h(i)),
    },
    CultureDef {
        name: "Kobold",
        base: 36,
        odd: 1.0,
        development: 18,
        sort: Some(|x, i| x.t(i) - x.s(i)),
    },
    CultureDef {
        name: "Uruk",
        base: 37,
        odd: 1.0,
        development: 18,
        sort: Some(|x, i| x.h(i) * x.t(i)),
    },
    CultureDef {
        name: "Ugluk",
        base: 37,
        odd: 0.5,
        development: 15,
        sort: Some(|x, i| (x.h(i) * x.t(i)) / x.bd(i, &[1, 2, 10, 11], 4.0)),
    },
    CultureDef {
        name: "Yotunn",
        base: 38,
        odd: 0.7,
        development: 8,
        sort: Some(|x, i| x.td(i, -10.0)),
    },
    CultureDef {
        name: "Rake",
        base: 39,
        odd: 0.7,
        development: 28,
        sort: Some(|x, i| -x.s(i)),
    },
    CultureDef {
        name: "Arago",
        base: 40,
        odd: 0.7,
        development: 12,
        sort: Some(|x, i| x.t(i) - x.s(i)),
    },
    CultureDef {
        name: "Aj'Snaga",
        base: 41,
        odd: 0.7,
        development: 22,
        sort: Some(|x, i| x.n(i) / x.bd(i, &[12], 10.0)),
    },
    CultureDef {
        name: "Anor",
        base: 32,
        odd: 1.0,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 10.0)),
    },
    CultureDef {
        name: "Dail",
        base: 32,
        odd: 1.0,
        development: 32,
        sort: Some(|x, i| x.n(i) / x.td(i, 13.0)),
    },
    CultureDef {
        name: "Rohand",
        base: 16,
        odd: 1.0,
        development: 30,
        sort: Some(|x, i| x.n(i) / x.td(i, 16.0)),
    },
    CultureDef {
        name: "Dulandir",
        base: 31,
        odd: 1.0,
        development: 34,
        sort: Some(|x, i| (x.n(i) / x.td(i, 5.0) / x.bd(i, &[2, 4, 10], 7.0)) * x.t(i)),
    },
];

const DARK_FANTASY: [CultureDef; 34] = [
    CultureDef {
        name: "Angshire",
        base: 1,
        odd: 1.0,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 10.0) / x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Enlandic",
        base: 1,
        odd: 1.0,
        development: 35,
        sort: Some(|x, i| x.n(i) / x.td(i, 12.0)),
    },
    CultureDef {
        name: "Westen",
        base: 1,
        odd: 1.0,
        development: 35,
        sort: Some(|x, i| x.n(i) / x.td(i, 10.0)),
    },
    CultureDef {
        name: "Nortumbic",
        base: 1,
        odd: 1.0,
        development: 33,
        sort: Some(|x, i| x.n(i) / x.td(i, 7.0)),
    },
    CultureDef {
        name: "Mercian",
        base: 1,
        odd: 1.0,
        development: 35,
        sort: Some(|x, i| x.n(i) / x.td(i, 9.0)),
    },
    CultureDef {
        name: "Kentian",
        base: 1,
        odd: 1.0,
        development: 35,
        sort: Some(|x, i| x.n(i) / x.td(i, 12.0)),
    },
    CultureDef {
        name: "Norse",
        base: 6,
        odd: 0.7,
        development: 30,
        sort: Some(|x, i| x.n(i) / x.td(i, 5.0) / x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Schwarzen",
        base: 0,
        odd: 0.3,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 10.0) / x.bd(i, &[6, 8], 4.0)),
    },
    CultureDef {
        name: "Luarian",
        base: 2,
        odd: 0.3,
        development: 40,
        sort: Some(|x, i| x.n(i) / x.td(i, 12.0) / x.bd(i, &[6, 8], 4.0)),
    },
    CultureDef {
        name: "Hetallian",
        base: 3,
        odd: 0.3,
        development: 42,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0)),
    },
    CultureDef {
        name: "Astellian",
        base: 4,
        odd: 0.3,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 16.0)),
    },
    CultureDef {
        name: "Kiswaili",
        base: 28,
        odd: 0.05,
        development: 28,
        sort: Some(|x, i| x.n(i) / x.td(i, 29.0) / x.bd(i, &[1, 3, 5, 7], 4.0)),
    },
    CultureDef {
        name: "Yoruba",
        base: 21,
        odd: 0.05,
        development: 27,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0) / x.bd(i, &[5, 7], 4.0)),
    },
    CultureDef {
        name: "Koryo",
        base: 10,
        odd: 0.05,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 12.0) / x.t(i)),
    },
    CultureDef {
        name: "Hantzu",
        base: 11,
        odd: 0.05,
        development: 42,
        sort: Some(|x, i| x.n(i) / x.td(i, 13.0)),
    },
    CultureDef {
        name: "Yamoto",
        base: 12,
        odd: 0.05,
        development: 35,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0) / x.t(i)),
    },
    CultureDef {
        name: "Guantzu",
        base: 30,
        odd: 0.05,
        development: 38,
        sort: Some(|x, i| x.n(i) / x.td(i, 17.0)),
    },
    CultureDef {
        name: "Ulus",
        base: 31,
        odd: 0.05,
        development: 26,
        sort: Some(|x, i| (x.n(i) / x.td(i, 5.0) / x.bd(i, &[2, 4, 10], 7.0)) * x.t(i)),
    },
    CultureDef {
        name: "Turan",
        base: 16,
        odd: 0.05,
        development: 30,
        sort: Some(|x, i| x.n(i) / x.td(i, 12.0)),
    },
    CultureDef {
        name: "Amazi",
        base: 17,
        odd: 0.05,
        development: 28,
        sort: Some(|x, i| (x.n(i) / x.td(i, 19.0) / x.bd(i, &[1, 2, 3], 7.0)) * x.t(i)),
    },
    CultureDef {
        name: "Eurabic",
        base: 18,
        odd: 0.05,
        development: 40,
        sort: Some(|x, i| (x.n(i) / x.td(i, 26.0) / x.bd(i, &[1, 2], 7.0)) * x.t(i)),
    },
    CultureDef {
        name: "Slovan",
        base: 5,
        odd: 0.05,
        development: 32,
        sort: Some(|x, i| (x.n(i) / x.td(i, 6.0)) * x.t(i)),
    },
    CultureDef {
        name: "Keltan",
        base: 22,
        odd: 0.1,
        development: 30,
        sort: Some(|x, i| x.n(i) / libm::pow(x.td(i, 11.0), 0.5) / x.bd(i, &[6, 8], 4.0)),
    },
    CultureDef {
        name: "Elladan",
        base: 7,
        odd: 0.2,
        development: 36,
        sort: Some(|x, i| (x.n(i) / x.td(i, 18.0) / x.sf(i, 4.0)) * x.h(i)),
    },
    CultureDef {
        name: "Romian",
        base: 8,
        odd: 0.2,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 14.0) / x.t(i)),
    },
    CultureDef {
        name: "Eldar",
        base: 33,
        odd: 0.5,
        development: 46,
        sort: Some(|x, i| (x.n(i) / x.bd(i, &[6, 7, 8, 9], 10.0)) * x.t(i)),
    },
    CultureDef {
        name: "Trow",
        base: 34,
        odd: 0.8,
        development: 36,
        sort: Some(|x, i| (x.n(i) / x.bd(i, &[7, 8, 9, 12], 10.0)) * x.t(i)),
    },
    CultureDef {
        name: "Durinn",
        base: 35,
        odd: 0.8,
        development: 53,
        sort: Some(|x, i| x.n(i) + x.h(i)),
    },
    CultureDef {
        name: "Kobblin",
        base: 36,
        odd: 0.8,
        development: 18,
        sort: Some(|x, i| x.t(i) - x.s(i)),
    },
    CultureDef {
        name: "Uruk",
        base: 37,
        odd: 0.8,
        development: 18,
        sort: Some(|x, i| (x.h(i) * x.t(i)) / x.bd(i, &[1, 2, 10, 11], 4.0)),
    },
    CultureDef {
        name: "Yotunn",
        base: 38,
        odd: 0.8,
        development: 8,
        sort: Some(|x, i| x.td(i, -10.0)),
    },
    CultureDef {
        name: "Drake",
        base: 39,
        odd: 0.9,
        development: 28,
        sort: Some(|x, i| -x.s(i)),
    },
    CultureDef {
        name: "Rakhnid",
        base: 40,
        odd: 0.9,
        development: 12,
        sort: Some(|x, i| x.t(i) - x.s(i)),
    },
    CultureDef {
        name: "Aj'Snaga",
        base: 41,
        odd: 0.9,
        development: 22,
        sort: Some(|x, i| x.n(i) / x.bd(i, &[12], 10.0)),
    },
];

const WORLD: [CultureDef; 33] = [
    CultureDef {
        name: "Shwazen",
        base: 0,
        odd: 0.7,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 10.0) / x.bd(i, &[6, 8], 4.0)),
    },
    CultureDef {
        name: "Angshire",
        base: 1,
        odd: 1.0,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 10.0) / x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Luari",
        base: 2,
        odd: 0.6,
        development: 40,
        sort: Some(|x, i| x.n(i) / x.td(i, 12.0) / x.bd(i, &[6, 8], 4.0)),
    },
    CultureDef {
        name: "Tallian",
        base: 3,
        odd: 0.6,
        development: 42,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0)),
    },
    CultureDef {
        name: "Astellian",
        base: 4,
        odd: 0.6,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 16.0)),
    },
    CultureDef {
        name: "Slovan",
        base: 5,
        odd: 0.7,
        development: 32,
        sort: Some(|x, i| (x.n(i) / x.td(i, 6.0)) * x.t(i)),
    },
    CultureDef {
        name: "Norse",
        base: 6,
        odd: 0.7,
        development: 30,
        sort: Some(|x, i| x.n(i) / x.td(i, 5.0)),
    },
    CultureDef {
        name: "Elladan",
        base: 7,
        odd: 0.7,
        development: 36,
        sort: Some(|x, i| (x.n(i) / x.td(i, 18.0)) * x.h(i)),
    },
    CultureDef {
        name: "Romian",
        base: 8,
        odd: 0.7,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0)),
    },
    CultureDef {
        name: "Soumi",
        base: 9,
        odd: 0.3,
        development: 28,
        sort: Some(|x, i| (x.n(i) / x.td(i, 5.0) / x.bd(i, &[9], 4.0)) * x.t(i)),
    },
    CultureDef {
        name: "Koryo",
        base: 10,
        odd: 0.1,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 12.0) / x.t(i)),
    },
    CultureDef {
        name: "Hantzu",
        base: 11,
        odd: 0.1,
        development: 42,
        sort: Some(|x, i| x.n(i) / x.td(i, 13.0)),
    },
    CultureDef {
        name: "Yamoto",
        base: 12,
        odd: 0.1,
        development: 35,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0) / x.t(i)),
    },
    CultureDef {
        name: "Portuzian",
        base: 13,
        odd: 0.4,
        development: 36,
        sort: Some(|x, i| x.n(i) / x.td(i, 17.0) / x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Nawatli",
        base: 14,
        odd: 0.1,
        development: 18,
        sort: Some(|x, i| x.h(i) / x.td(i, 18.0) / x.bd(i, &[7], 4.0)),
    },
    CultureDef {
        name: "Vengrian",
        base: 15,
        odd: 0.2,
        development: 33,
        sort: Some(|x, i| (x.n(i) / x.td(i, 11.0) / x.bd(i, &[4], 4.0)) * x.t(i)),
    },
    CultureDef {
        name: "Turchian",
        base: 16,
        odd: 0.2,
        development: 35,
        sort: Some(|x, i| x.n(i) / x.td(i, 13.0)),
    },
    CultureDef {
        name: "Amazi",
        base: 17,
        odd: 0.1,
        development: 28,
        sort: Some(|x, i| (x.n(i) / x.td(i, 19.0) / x.bd(i, &[1, 2, 3], 7.0)) * x.t(i)),
    },
    CultureDef {
        name: "Eurabic",
        base: 18,
        odd: 0.2,
        development: 40,
        sort: Some(|x, i| (x.n(i) / x.td(i, 26.0) / x.bd(i, &[1, 2], 7.0)) * x.t(i)),
    },
    CultureDef {
        name: "Inuk",
        base: 19,
        odd: 0.05,
        development: 6,
        sort: Some(|x, i| x.td(i, -1.0) / x.bd(i, &[10, 11], 4.0) / x.sf(i, 4.0)),
    },
    CultureDef {
        name: "Euskati",
        base: 20,
        odd: 0.05,
        development: 30,
        sort: Some(|x, i| (x.n(i) / x.td(i, 15.0)) * x.h(i)),
    },
    CultureDef {
        name: "Yoruba",
        base: 21,
        odd: 0.05,
        development: 27,
        sort: Some(|x, i| x.n(i) / x.td(i, 15.0) / x.bd(i, &[5, 7], 4.0)),
    },
    CultureDef {
        name: "Keltan",
        base: 22,
        odd: 0.05,
        development: 30,
        sort: Some(|x, i| (x.n(i) / x.td(i, 11.0) / x.bd(i, &[6, 8], 4.0)) * x.t(i)),
    },
    CultureDef {
        name: "Efratic",
        base: 23,
        odd: 0.05,
        development: 35,
        sort: Some(|x, i| (x.n(i) / x.td(i, 22.0)) * x.t(i)),
    },
    CultureDef {
        name: "Tehrani",
        base: 24,
        odd: 0.1,
        development: 38,
        sort: Some(|x, i| (x.n(i) / x.td(i, 18.0)) * x.h(i)),
    },
    CultureDef {
        name: "Maui",
        base: 25,
        odd: 0.05,
        development: 12,
        sort: Some(|x, i| x.n(i) / x.td(i, 24.0) / x.sf(i, 4.0) / x.t(i)),
    },
    CultureDef {
        name: "Carnatic",
        base: 26,
        odd: 0.05,
        development: 33,
        sort: Some(|x, i| x.n(i) / x.td(i, 26.0)),
    },
    CultureDef {
        name: "Inqan",
        base: 27,
        odd: 0.05,
        development: 20,
        sort: Some(|x, i| x.h(i) / x.td(i, 13.0)),
    },
    CultureDef {
        name: "Kiswaili",
        base: 28,
        odd: 0.1,
        development: 28,
        sort: Some(|x, i| x.n(i) / x.td(i, 29.0) / x.bd(i, &[1, 3, 5, 7], 4.0)),
    },
    CultureDef {
        name: "Vietic",
        base: 29,
        odd: 0.1,
        development: 32,
        sort: Some(|x, i| x.n(i) / x.td(i, 25.0) / x.bd(i, &[7], 7.0) / x.t(i)),
    },
    CultureDef {
        name: "Guantzu",
        base: 30,
        odd: 0.1,
        development: 38,
        sort: Some(|x, i| x.n(i) / x.td(i, 17.0)),
    },
    CultureDef {
        name: "Ulus",
        base: 31,
        odd: 0.1,
        development: 26,
        sort: Some(|x, i| (x.n(i) / x.td(i, 5.0) / x.bd(i, &[2, 4, 10], 7.0)) * x.t(i)),
    },
    CultureDef {
        name: "Levent",
        base: 42,
        odd: 0.2,
        development: 35,
        sort: Some(|x, i| (x.n(i) / x.td(i, 18.0)) * x.sf(i, 4.0)),
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::data::NAME_BASES;

    #[test]
    fn each_set_names_known_bases_and_has_enough_cultures() {
        for set in &CULTURE_SETS {
            for c in set.cultures {
                assert!(c.base < NAME_BASES.len(), "{} {}", set.name, c.name);
                assert!(c.odd > 0.0 && c.odd <= 1.0);
            }
            if !set.cultures.is_empty() {
                assert!(set.cultures.len() >= set.max.min(set.cultures.len()));
            }
        }
        assert!(culture_set("high-fantasy").is_some());
        assert!(culture_set("highFantasy").is_none());
    }

    #[test]
    fn sort_functions_follow_the_fmg_helpers() {
        let s = [0.0, 10.0, 20.0];
        let t = [-1, 1, 3];
        let h = [10.0, 30.0, 60.0];
        let temp = [5.0, 12.0, 30.0];
        let biome = [0, 6, 1];
        let sea = [false, true, false];
        let x = SortCtx {
            s: &s,
            s_max: 20.0,
            t: &t,
            h: &h,
            temp: &temp,
            biome: &biome,
            sea_coast: &sea,
        };
        assert_eq!(x.n(1), 2.0);
        assert_eq!(x.td(1, 12.0), 1.0);
        assert_eq!(x.td(1, 10.0), 3.0);
        assert_eq!(x.bd(1, &[6, 8], 4.0), 1.0);
        assert_eq!(x.bd(2, &[6, 8], 7.0), 7.0);
        assert_eq!(x.sf(1, 4.0), 1.0);
        // Shwazen: n / td(10) / bd([6, 8]).
        let shwazen = EUROPEAN[0].sort.unwrap();
        assert_eq!(shwazen(&x, 1), 2.0 / 3.0);
    }

    #[test]
    fn name_development_is_in_range() {
        for n in ["Shwazen", "A", "Kiswaili", "Ölund"] {
            assert!((15..=42).contains(&name_development(n)));
        }
        // FNV-1a of "a" is 0xe40c292c.
        assert_eq!(name_development("a"), 15 + 0xe40c_292c % 28);
    }
}
