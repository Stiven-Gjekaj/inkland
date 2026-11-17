//! Names: a Markov chain of letters, learned from a name base.
//!
//! The chain looks at the last two letters and picks the next letter with
//! the same frequency as in the base. A name ends where a base name ends.
//! A name is refused, and the generator tries again, if it is too short or
//! too long, if it has three equal letters in a row, if it doubles a letter
//! that the base does not double, or if it is already in use.

pub mod data;

use crate::rand::Rand;
use data::{NAME_BASES, NameBase};
use std::collections::{BTreeMap, BTreeSet};

/// The start and end mark of a name in the chain.
const EDGE: char = '\0';

/// A chain learned from one name base.
pub struct Chain {
    base: &'static NameBase,
    next: BTreeMap<(char, char), Vec<char>>,
}

impl Chain {
    pub fn new(base_index: usize) -> Chain {
        let base = &NAME_BASES[base_index];
        let mut next: BTreeMap<(char, char), Vec<char>> = BTreeMap::new();
        for name in base.names.split(',') {
            let name = name.trim().to_lowercase();
            if name.is_empty() {
                continue;
            }
            let mut a = EDGE;
            let mut b = EDGE;
            for c in name.chars().chain(std::iter::once(EDGE)) {
                next.entry((a, b)).or_default().push(c);
                a = b;
                b = c;
            }
        }
        Chain { base, next }
    }

    fn try_one(&self, rng: &mut Rand) -> Option<String> {
        let mut out = String::new();
        let (mut a, mut b) = (EDGE, EDGE);
        let limit = self.base.max + 4;
        loop {
            let options = self.next.get(&(a, b))?;
            let c = options[rng.int(0, options.len() as i64 - 1) as usize];
            if c == EDGE {
                break;
            }
            out.push(c);
            if out.chars().count() > limit {
                return None;
            }
            a = b;
            b = c;
        }
        Some(out)
    }

    fn acceptable(&self, name: &str) -> bool {
        let letters: Vec<char> = name.chars().collect();
        let len = letters.iter().filter(|c| c.is_alphabetic()).count();
        if len < self.base.min || len > self.base.max {
            return false;
        }
        for w in letters.windows(2) {
            if w[0] == w[1] && !self.base.double.contains(w[0]) {
                return false;
            }
        }
        if letters.windows(3).any(|w| w[0] == w[1] && w[1] == w[2]) {
            return false;
        }
        let edge_mark = |c: &char| matches!(c, ' ' | '-' | '\'');
        !(letters.first().is_some_and(edge_mark) || letters.last().is_some_and(edge_mark))
    }

    /// A new name that is not in `used`. The name is added to `used`.
    pub fn name(&self, rng: &mut Rand, used: &mut BTreeSet<String>) -> String {
        for _ in 0..200 {
            if let Some(n) = self.try_one(rng)
                && self.acceptable(&n)
            {
                let n = capitalize(&n);
                if used.insert(n.clone()) {
                    return n;
                }
            }
        }
        // The base cannot make a new name: use a base name with a number.
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
    fn names_follow_the_base_rules() {
        for (i, base) in NAME_BASES.iter().enumerate() {
            let chain = Chain::new(i);
            let mut used = BTreeSet::new();
            let mut r = rng();
            for _ in 0..30 {
                let n = chain.name(&mut r, &mut used);
                if n.contains(char::is_numeric) {
                    continue;
                }
                let len = n.chars().filter(|c| c.is_alphabetic()).count();
                assert!(len >= base.min && len <= base.max, "{} gave {n}", base.name);
                assert!(n.chars().next().unwrap().is_uppercase(), "{n}");
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
