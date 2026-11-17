//! Random numbers for generation.
//!
//! Each function turns the bits of the generator into a number with integer
//! steps and one multiplication, so the result is the same on each machine.

use rand_chacha::ChaCha8Rng;
use rand_core::Rng as _;

/// A random number source for one layer or one part of a layer.
pub struct Rand {
    inner: ChaCha8Rng,
}

impl Rand {
    pub fn new(inner: ChaCha8Rng) -> Self {
        Self { inner }
    }

    /// A number in `[0, 1)`, with 53 random bits.
    pub fn unit(&mut self) -> f64 {
        (self.inner.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// A number in `[min, max)`.
    pub fn uniform(&mut self, min: f64, max: f64) -> f64 {
        min + (max - min) * self.unit()
    }

    /// An integer in `[min, max]`, both included. The draw has no bias.
    pub fn int(&mut self, min: i64, max: i64) -> i64 {
        if max <= min {
            return min;
        }
        let span = (max - min) as u64 + 1;
        // Refuse the top part of the u64 range that would give a bias.
        let zone = u64::MAX - (u64::MAX % span) - 1;
        loop {
            let v = self.inner.next_u64();
            if v <= zone {
                return min + (v % span) as i64;
            }
        }
    }

    /// `true` with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// `P` of FMG: `true` with the probability `p`. A `p` of 1 or more, or
    /// of 0 or less, uses no random number.
    pub fn p(&mut self, p: f64) -> bool {
        if p >= 1.0 {
            return true;
        }
        if p <= 0.0 {
            return false;
        }
        self.unit() < p
    }

    /// A random item of a list that is not empty (`ra` in FMG).
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.unit() * items.len() as f64) as usize]
    }

    /// A random whole number from `min` to `max`, both included (`rand` in
    /// FMG): `floor(unit * (max - min + 1)) + min`.
    pub fn rand(&mut self, min: i64, max: i64) -> i64 {
        (self.unit() * (max - min + 1) as f64).floor() as i64 + min
    }

    /// A key from a list of keys with whole-number weights (`rw` in FMG).
    pub fn weighted<'a>(&mut self, items: &[(&'a str, u32)]) -> &'a str {
        let total: u32 = items.iter().map(|i| i.1).sum();
        let mut k = (self.unit() * f64::from(total)) as u32;
        for &(key, w) in items {
            if k < w {
                return key;
            }
            k -= w;
        }
        items[items.len() - 1].0
    }

    /// A whole number from `min` to `max` with a bias to `min` that grows
    /// with `ex` (`biased` in FMG).
    pub fn biased(&mut self, min: f64, max: f64, ex: f64) -> f64 {
        js_round(min + (max - min) * libm::pow(self.unit(), ex))
    }

    /// A normal random number, limited to `[min, max]` and rounded to
    /// `round` decimals (`gauss` in FMG, with the polar method of d3).
    pub fn gauss(&mut self, expected: f64, deviation: f64, min: f64, max: f64, round: u32) -> f64 {
        let (y, r) = loop {
            let x = self.unit() * 2.0 - 1.0;
            let y = self.unit() * 2.0 - 1.0;
            let r = x * x + y * y;
            if r != 0.0 && r <= 1.0 {
                break (y, r);
            }
        };
        let v = expected + deviation * y * libm::sqrt(-2.0 * libm::log(r) / r);
        rn(v.clamp(min, max), round)
    }
}

/// `Math.round` of JavaScript: a half goes up.
pub fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// Round to `decimals` places as FMG does (`rn`).
pub fn rn(v: f64, decimals: u32) -> f64 {
    let m = libm::pow(10.0, f64::from(decimals));
    js_round(v * m) / m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::{Layer, SeedTree};

    fn r() -> Rand {
        Rand::new(SeedTree::new(1).part_rng(Layer::Terrain, "test"))
    }

    #[test]
    fn unit_stays_in_range() {
        let mut r = r();
        for _ in 0..10_000 {
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
        }
    }

    #[test]
    fn int_gives_each_value_and_only_those() {
        let mut r = r();
        let mut seen = [0u32; 5];
        for _ in 0..10_000 {
            let v = r.int(3, 7);
            assert!((3..=7).contains(&v));
            seen[(v - 3) as usize] += 1;
        }
        assert!(seen.iter().all(|&n| n > 1500), "{seen:?}");
    }

    #[test]
    fn chance_zero_and_one() {
        let mut r = r();
        assert!((0..1000).all(|_| !r.chance(0.0)));
        assert!((0..1000).all(|_| r.chance(1.0)));
    }
}
