//! Names: the syllable chain of FMG (`names-generator.ts`).
//!
//! `calculate_chain` splits each name of a base into pseudo-syllables. The
//! key of a syllable is the letter before it. A new name starts with a
//! syllable of the empty key and adds syllables until it reaches the end
//! mark or the longest length of the base. Then double letters that the
//! base does not double, three equal letters, and "ae" are cleaned.
//!
//! FMG does not keep names unique. inkland does: a name that is already in
//! use is made again, up to 200 times, and then gets a number.

pub mod data;

use crate::rand::Rand;
use data::{NAME_BASES, NameBase};
use std::collections::{BTreeMap, BTreeSet};

/// The vowels of FMG (`isVowel`).
const VOWELS: &str = "aeiouy'əøɛœæɶɒɨɪɔɐʊɤɯаоиеёэыуюяàèìòùỳẁȁȅȉȍȕáéíóúýẃőűâêîôûŷŵäëïöüÿẅãẽĩõũỹąęįǫųāēīōūȳăĕĭŏŭǎěǐǒǔȧėȯẏẇạẹịọụỵẉḛḭṵṳɑ";

/// `true` for a vowel.
pub fn is_vowel(c: char) -> bool {
    VOWELS.contains(c)
}

/// `isVowel` on a text of 0 or 1 letters: the empty text is a vowel in FMG.
fn is_vowel_text(s: &[char]) -> bool {
    match s {
        [] => true,
        [c] => is_vowel(*c),
        _ => false,
    }
}

/// A letter of the CJK scripts: a syllable on its own.
fn syllabic(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FDF | 0x3005 | 0x3007 | 0x3021..=0x3029 | 0x3038..=0x303B
        | 0x3040..=0x30FF | 0x3131..=0x318E | 0x3200..=0x321E | 0x3260..=0x327E
        | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xA960..=0xA97F | 0xAC00..=0xD7FF
        | 0xF900..=0xFAFF | 0xFF66..=0xFFDC | 0x1100..=0x11FF | 0x20000..=0x323AF)
}

/// The syllable chain of a list of names (`calculateChain`). The key is the
/// letter before a syllable, or `None` at the start of a name. An empty
/// syllable marks the end of a name.
pub fn calculate_chain(names: &str) -> BTreeMap<Option<char>, Vec<String>> {
    let mut chain: BTreeMap<Option<char>, Vec<String>> = BTreeMap::new();
    for n in names.split(',') {
        let name: Vec<char> = n.trim().to_lowercase().chars().collect();
        let basic = name.iter().all(|&c| (' '..='~').contains(&c));
        let at = |i: usize| name.get(i).copied();
        let mut i: isize = -1;
        while i < name.len() as isize {
            let prev = if i >= 0 { at(i as usize) } else { None };
            let mut syllable: Vec<char> = Vec::new();
            let mut v = false;
            let mut c = (i + 1) as usize;
            while let Some(that) = at(c) {
                if syllable.len() >= 5 {
                    break;
                }
                let next = at(c + 1);
                syllable.push(that);
                if syllable == [' '] || syllable == ['-'] {
                    break;
                }
                let Some(next) = next else { break };
                if next == ' ' || next == '-' {
                    break;
                }
                if syllabic(that) || syllabic(next) {
                    break;
                }
                if is_vowel(that) {
                    v = true;
                }
                if that == 'y' && next == 'e' {
                    c += 1;
                    continue;
                }
                if basic
                    && matches!(
                        (that, next),
                        ('o', 'o') | ('e', 'e') | ('a', 'e') | ('c', 'h')
                    )
                {
                    c += 1;
                    continue;
                }
                if v && at(c + 2).is_some_and(is_vowel) {
                    break;
                }
                c += 1;
            }
            let step = syllable.len().max(1) as isize;
            chain
                .entry(prev)
                .or_default()
                .push(syllable.into_iter().collect());
            i += step;
        }
    }
    chain
}

/// A chain learned from one name base.
pub struct Chain {
    base: &'static NameBase,
    index: usize,
    data: BTreeMap<Option<char>, Vec<String>>,
}

impl Chain {
    pub fn new(base_index: usize) -> Chain {
        let index = base_index % NAME_BASES.len();
        let base = &NAME_BASES[index];
        Chain {
            base,
            index,
            data: calculate_chain(base.names),
        }
    }

    /// The index of the name base.
    pub fn base_index(&self) -> usize {
        self.index
    }

    /// One name from the chain (`getBase`). `dupl`: the letters that can
    /// come twice; `None` uses the letters of the base.
    pub fn get_base(&self, rng: &mut Rand, min: usize, max: usize, dupl: Option<&str>) -> String {
        let dupl = dupl.unwrap_or(self.base.double);
        let start = &self.data[&None];
        let mut v: &Vec<String> = start;
        let mut cur: String = rng.pick(v).clone();
        let mut w = String::new();
        for _ in 0..20 {
            if cur.is_empty() {
                if w.chars().count() < min {
                    cur.clear();
                    w.clear();
                    v = start;
                } else {
                    break;
                }
            } else {
                let (wl, cl) = (w.chars().count(), cur.chars().count());
                if wl + cl > max {
                    if wl < min {
                        w.push_str(&cur);
                    }
                    break;
                }
                let last = cur.chars().last();
                v = self.data.get(&last).unwrap_or(start);
            }
            w.push_str(&cur);
            cur = rng.pick(v).clone();
        }
        if w.ends_with(['\'', ' ', '-']) {
            w.pop();
        }
        let d: Vec<char> = w.chars().collect();
        let mut r: Vec<char> = Vec::new();
        for (i, &c) in d.iter().enumerate() {
            let next = d.get(i + 1).copied();
            if Some(c) == next && !dupl.contains(c) {
                continue;
            }
            if r.is_empty() {
                r.extend(c.to_uppercase());
                continue;
            }
            let last = r[r.len() - 1];
            if last == '-' && c == ' ' {
                continue;
            }
            if last == ' ' || last == '-' {
                r.extend(c.to_uppercase());
                continue;
            }
            if c == 'a' && next == Some('e') {
                continue;
            }
            if i + 2 < d.len() && Some(c) == next && c == d[i + 2] {
                continue;
            }
            r.push(c);
        }
        let mut name: String = r.into_iter().collect();
        if name.split(' ').any(|p| p.chars().count() < 2) {
            name = name
                .split(' ')
                .enumerate()
                .map(|(i, p)| {
                    if i > 0 {
                        p.to_lowercase()
                    } else {
                        p.to_owned()
                    }
                })
                .collect();
        }
        if name.chars().count() < 2 {
            let names: Vec<&str> = self.base.names.split(',').collect();
            name = rng.pick(&names).to_string();
        }
        name
    }

    /// A short name (`getBaseShort`).
    pub fn get_short(&self, rng: &mut Rand) -> String {
        let min = self.base.min - 1;
        let max = (self.base.max - 2).max(min);
        self.get_base(rng, min, max, Some(""))
    }

    /// A name with the lengths of the base that is not in `used`. The name
    /// is added to `used`.
    pub fn name(&self, rng: &mut Rand, used: &mut BTreeSet<String>) -> String {
        let (min, max) = (self.base.min, self.base.max);
        self.unique(used, || self.get_base(rng, min, max, None))
    }

    /// A short name that is not in `used`. The name is added to `used`.
    pub fn short(&self, rng: &mut Rand, used: &mut BTreeSet<String>) -> String {
        self.unique(used, || self.get_short(rng))
    }

    /// A state name made from `name` that is not in `used` (`getState`).
    pub fn state(&self, rng: &mut Rand, name: &str, used: &mut BTreeSet<String>) -> String {
        let first = state_name(name, self.index, rng);
        if used.insert(first.clone()) {
            return first;
        }
        self.unique(used, || {
            let n = self.get_short(rng);
            state_name(&n, self.index, rng)
        })
    }

    fn unique(&self, used: &mut BTreeSet<String>, mut make: impl FnMut() -> String) -> String {
        for _ in 0..200 {
            let n = make();
            if used.insert(n.clone()) {
                return n;
            }
        }
        let first = self.base.names.split(',').next().unwrap_or("Place").trim();
        let mut k = 2;
        loop {
            let n = format!("{first} {k}");
            if used.insert(n.clone()) {
                return n;
            }
            k += 1;
        }
    }
}

/// Add a suffix to a name and keep it easy to say (`validateSuffix`).
fn validate_suffix(name: &str, suffix: &str) -> String {
    let mut n: Vec<char> = name.chars().collect();
    let s: Vec<char> = suffix.chars().collect();
    if n.ends_with(&s) {
        return name.to_owned();
    }
    let s1 = s[0];
    if n.last() == Some(&s1) {
        n.pop();
    }
    let k = n.len();
    let last1 = if k >= 1 { &n[k - 1..] } else { &n[..0] };
    let last2 = if k >= 2 { &n[k - 2..k - 1] } else { &n[..0] };
    if is_vowel(s1) == is_vowel_text(last1) && is_vowel(s1) == is_vowel_text(last2) {
        n.pop();
    }
    if n.last() == Some(&s1) {
        n.pop();
    }
    n.into_iter().collect::<String>() + suffix
}

/// The name of a state, made from a place name or a short name and the name
/// base (`getState`).
pub fn state_name(name: &str, base: usize, rng: &mut Rand) -> String {
    let mut name: Vec<char> = if name.contains(' ') {
        capitalize_first(&name.replace(' ', "").to_lowercase())
            .chars()
            .collect()
    } else {
        name.chars().collect()
    };
    let ends = |n: &[char], s: &str| n.iter().collect::<String>().ends_with(s);
    if name.len() > 6 && ends(&name, "berg") {
        name.truncate(name.len() - 4);
    }
    if name.len() > 5 && ends(&name, "ton") {
        name.truncate(name.len() - 3);
    }
    let text = |n: &[char]| n.iter().collect::<String>();
    if base == 5 && (ends(&name, "sk") || ends(&name, "ev") || ends(&name, "ov")) {
        name.truncate(name.len() - 2);
    } else if base == 12 {
        let last = name.len().saturating_sub(1);
        return if is_vowel_text(&name[last..]) {
            text(&name)
        } else {
            text(&name) + "u"
        };
    } else if base == 18 && rng.p(0.4) {
        let first: Vec<char> = name.iter().take(1).flat_map(|c| c.to_lowercase()).collect();
        name = if is_vowel_text(&first) {
            format!("Al{}", text(&name).to_lowercase())
                .chars()
                .collect()
        } else {
            format!("Al {}", text(&name)).chars().collect()
        };
    }
    if base > 32 && base < 42 {
        return text(&name);
    }
    let k = name.len();
    if k > 3 && is_vowel_text(&name[k - 1..]) {
        if is_vowel_text(&name[k - 2..k - 1]) && rng.p(0.85) {
            name.truncate(k - 2);
        } else if rng.p(0.7) {
            name.truncate(k - 1);
        } else {
            return text(&name);
        }
    } else if rng.p(0.4) {
        return text(&name);
    }
    let rnd = rng.unit();
    let l = name.len();
    let suffix = match base {
        3 | 4 | 13 if rnd < 0.03 && l < 7 => "terra",
        2 if rnd < 0.03 && l < 7 => "terre",
        0 if rnd < 0.5 && l < 7 => "land",
        1 if rnd < 0.4 && l < 7 => "land",
        6 if rnd < 0.3 && l < 7 => "land",
        32 if rnd < 0.1 && l < 7 => "land",
        7 if rnd < 0.1 => "eia",
        9 if rnd < 0.35 => "maa",
        15 if rnd < 0.4 && l < 6 => "orszag",
        16 if rnd < 0.6 => "yurt",
        16 => "eli",
        10 => "guk",
        11 => " Guo",
        14 if rnd < 0.5 && l < 6 => "tlan",
        14 => "co",
        17 | 18 if rnd < 0.8 => "a",
        _ => "ia",
    };
    validate_suffix(&text(&name), suffix)
}

/// Remove vowels at the end while the text is longer than `min`
/// (`trimVowels`).
pub fn trim_vowels(s: &str, min: usize) -> String {
    let mut c: Vec<char> = s.chars().collect();
    while c.len() > min && c.last().is_some_and(|&x| is_vowel(x)) {
        c.pop();
    }
    c.into_iter().collect()
}

/// The adjective of a name (`getAdjective`).
pub fn adjective(noun: &str, rng: &mut Rand) -> String {
    let len = noun.chars().count();
    let cut = |n: usize| -> String { noun.chars().take(len.saturating_sub(n)).collect() };
    let letters_only = noun.chars().all(|c| c.is_ascii_alphabetic());
    // Each rule: probability, condition, action.
    type Rule<'a> = (f64, bool, Box<dyn Fn() -> String + 'a>);
    let rules: Vec<Rule> = vec![
        (1.0, noun.ends_with(" Guo"), Box::new(|| cut(4))),
        (
            1.0,
            noun.ends_with("orszag"),
            Box::new(|| {
                if len < 9 {
                    format!("{noun}ian")
                } else {
                    cut(6)
                }
            }),
        ),
        (
            1.0,
            noun.ends_with("stan"),
            Box::new(|| {
                if len < 9 {
                    format!("{noun}i")
                } else {
                    trim_vowels(&cut(4), 3)
                }
            }),
        ),
        (
            1.0,
            noun.ends_with("land"),
            Box::new(|| {
                if len > 9 {
                    return cut(4);
                }
                let root = trim_vowels(&cut(4), 0);
                match root.chars().count() {
                    0..=2 => format!("{noun}ic"),
                    3 => format!("{root}lish"),
                    _ => format!("{root}ish"),
                }
            }),
        ),
        (1.0, noun.ends_with("que"), Box::new(|| cut(3) + "can")),
        (1.0, noun.ends_with('a'), Box::new(|| format!("{noun}n"))),
        (1.0, noun.ends_with('o'), Box::new(|| cut(1) + "an")),
        (1.0, noun.ends_with('u'), Box::new(|| format!("{noun}an"))),
        (1.0, noun.ends_with('i'), Box::new(|| format!("{noun}an"))),
        (1.0, noun.ends_with('e'), Box::new(|| format!("{noun}an"))),
        (1.0, noun.ends_with("ay"), Box::new(|| format!("{noun}an"))),
        (
            1.0,
            noun.ends_with("os"),
            Box::new(|| {
                let root = trim_vowels(&cut(2), 0);
                if root.chars().count() < 4 {
                    cut(1)
                } else {
                    format!("{root}ian")
                }
            }),
        ),
        (
            1.0,
            noun.ends_with("es"),
            Box::new(|| {
                let root = trim_vowels(&cut(2), 0);
                if root.chars().count() > 7 {
                    cut(1)
                } else {
                    format!("{root}ian")
                }
            }),
        ),
        (0.8, noun.ends_with('l'), Box::new(|| format!("{noun}ese"))),
        (0.8, noun.ends_with('n'), Box::new(|| format!("{noun}ese"))),
        (0.8, noun.ends_with("ad"), Box::new(|| format!("{noun}ian"))),
        (0.8, noun.ends_with("an"), Box::new(|| format!("{noun}ian"))),
        (
            0.25,
            letters_only && len == 6,
            Box::new(|| format!("{}ish", trim_vowels(&cut(1), 3))),
        ),
        (
            0.5,
            letters_only && len <= 7,
            Box::new(|| format!("{}an", trim_vowels(noun, 3))),
        ),
    ];
    for (p, cond, action) in rules {
        if rng.p(p) && cond {
            return action();
        }
    }
    noun.to_owned()
}

/// A short code of a name that is not in `restricted` (`abbreviate`).
pub fn abbreviate(name: &str, restricted: &[String]) -> String {
    let parsed = name.replacen("Old ", "O ", 1).replace(['(', ')'], "");
    let words: Vec<&str> = parsed.split(' ').collect();
    let letters: Vec<char> = words.concat().chars().collect();
    let first = |w: &str| w.chars().next().map(String::from).unwrap_or_default();
    let mut code = if words.len() == 2 {
        first(words[0]) + &first(words[1])
    } else {
        letters.iter().take(2).collect()
    };
    let mut i = 1;
    while i + 1 < letters.len() && restricted.contains(&code) {
        code = letters[0].to_string() + &letters[i].to_uppercase().to_string();
        i += 1;
    }
    code
}

fn capitalize_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Upper case at the start of each word.
pub fn capitalize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut start = true;
    for c in name.chars() {
        if start {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
        start = matches!(c, ' ' | '-' | '\'');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::{Layer, SeedTree};

    fn rng() -> Rand {
        Rand::new(SeedTree::new(5).part_rng(Layer::Names, "test"))
    }

    #[test]
    fn a_name_splits_into_syllables_as_in_fmg() {
        let chain = calculate_chain("Achern");
        assert_eq!(chain[&None], vec!["acher".to_owned()]);
        assert_eq!(chain[&Some('r')], vec!["n".to_owned()]);
        assert_eq!(chain[&Some('n')], vec![String::new()]);
    }

    #[test]
    fn names_are_capitalized_and_unique() {
        for (i, base) in NAME_BASES.iter().enumerate() {
            let chain = Chain::new(i);
            let mut used = BTreeSet::new();
            let mut r = rng();
            for _ in 0..30 {
                let n = chain.name(&mut r, &mut used);
                assert!(n.chars().count() >= 2, "{n}");
                let first = n.chars().next().unwrap();
                assert!(!first.is_lowercase(), "{n}");
            }
            assert_eq!(used.len(), 30, "{}", base.name);
        }
    }

    #[test]
    fn same_seed_gives_the_same_names() {
        let chain = Chain::new(1);
        let a: Vec<String> = {
            let (mut r, mut u) = (rng(), BTreeSet::new());
            (0..10).map(|_| chain.name(&mut r, &mut u)).collect()
        };
        let b: Vec<String> = {
            let (mut r, mut u) = (rng(), BTreeSet::new());
            (0..10).map(|_| chain.name(&mut r, &mut u)).collect()
        };
        assert_eq!(a, b);
    }

    #[test]
    fn state_names_follow_the_base_rules() {
        let mut r = rng();
        // Fantasy bases get no suffix.
        assert_eq!(state_name("Quenya", 33, &mut r), "Quenya");
        // Japanese names end on a vowel.
        assert_eq!(state_name("Edok", 12, &mut r), "Edoku");
        // Spaces go out.
        assert_eq!(state_name("Lo Ren", 33, &mut r), "Loren");
        assert_eq!(validate_suffix("Gallia", "ia"), "Gallia");
        assert_eq!(validate_suffix("Brit", "ia"), "Britia");
    }

    #[test]
    fn adjectives_follow_the_fmg_rules() {
        let mut r = rng();
        assert_eq!(adjective("Roma", &mut r), "Roman");
        assert_eq!(adjective("Shan Guo", &mut r), "Shan");
        assert_eq!(trim_vowels("Kiraia", 3), "Kir");
        assert_eq!(abbreviate("Old Tallian", &[]), "OT");
        assert_eq!(abbreviate("Luari", &["Lu".to_owned()]), "LU");
    }

    #[test]
    fn capitalize_each_word() {
        assert_eq!(capitalize("port o'neil-by-sea"), "Port O'Neil-By-Sea");
    }

    #[test]
    fn each_culture_set_names_a_known_base() {
        for set in data::CULTURE_SETS {
            for &(_, base) in data::culture_set(set).unwrap() {
                assert!(base < NAME_BASES.len());
            }
        }
    }
}
