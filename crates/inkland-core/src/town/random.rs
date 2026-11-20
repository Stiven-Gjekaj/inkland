//! The random numbers of the town generator.
//!
//! A port of `com.watabou.utils.Random` of TownGeneratorOS by Watabou
//! (GPL-3.0, <https://github.com/watabou/TownGeneratorOS>): the Lehmer
//! generator with the multiplier 48271 and the modulus 2^31 - 1.

const G: i64 = 48271;
const N: i64 = 2_147_483_647;

/// A seeded random number generator.
#[derive(Debug, Clone)]
pub struct Random {
    seed: i64,
}

impl Random {
    /// A generator with a seed. A seed of 0 or less, or of the modulus or
    /// more, becomes a seed in the range.
    pub fn new(seed: i64) -> Self {
        let s = seed.rem_euclid(N);
        Random {
            seed: if s == 0 { 1 } else { s },
        }
    }

    fn next(&mut self) -> i64 {
        self.seed = (self.seed * G) % N;
        self.seed
    }

    /// A number from 0 (included) to 1 (excluded).
    pub fn float(&mut self) -> f64 {
        self.next() as f64 / N as f64
    }

    /// The mean of 3 random numbers, near 0.5.
    pub fn normal(&mut self) -> f64 {
        (self.float() + self.float() + self.float()) / 3.0
    }

    /// A whole number from `min` (included) to `max` (excluded).
    pub fn int(&mut self, min: i64, max: i64) -> i64 {
        let v = min as f64 + self.next() as f64 / N as f64 * (max - min) as f64;
        v.trunc() as i64
    }

    /// `true` with the probability `chance`.
    pub fn bool(&mut self, chance: f64) -> bool {
        self.float() < chance
    }

    /// A number near 0.5. `f` is how much it changes: 0 gives 0.5.
    pub fn fuzzy(&mut self, f: f64) -> f64 {
        if f == 0.0 {
            0.5
        } else {
            (1.0 - f) / 2.0 + f * self.normal()
        }
    }

    /// The index of a random item of a list of `len` items.
    pub fn index(&mut self, len: usize) -> usize {
        ((self.float() * len as f64) as usize).min(len.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generator_is_the_lehmer_generator() {
        let mut r = Random::new(1);
        assert_eq!(r.next(), 48271);
        assert_eq!(r.next(), 182_605_794);
        let mut a = Random::new(12345);
        let mut b = Random::new(12345);
        for _ in 0..100 {
            let x = a.float();
            assert_eq!(x, b.float());
            assert!((0.0..1.0).contains(&x));
        }
        assert!(a.int(3, 7) >= 3);
    }
}
