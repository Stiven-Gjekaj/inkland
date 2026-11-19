//! Fractal value noise.
//!
//! The noise uses only integer hashes, `floor`, `+`, `-`, `*`, and `/`, so it
//! gives the same value on each machine. The seed comes from the seed tree.

/// Fractal noise: the sum of `octaves` layers of value noise. Each layer has
/// half the size and half the strength of the layer before it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fractal {
    seed: u64,
    octaves: u32,
    /// The size of the largest features, in units.
    scale: f64,
}

impl Fractal {
    /// Noise with the largest features of `scale` units. `octaves` is 1 or
    /// more. A `scale` of 0 or less gives noise that is always 0.
    pub fn new(seed: u64, octaves: u32, scale: f64) -> Self {
        Self {
            seed,
            octaves: octaves.max(1),
            scale,
        }
    }

    /// The seed from the first 8 bytes of a seed of the seed tree.
    pub fn seed_of(bytes: [u8; 32]) -> u64 {
        let mut first = [0u8; 8];
        first.copy_from_slice(&bytes[..8]);
        u64::from_le_bytes(first)
    }

    /// The noise at `p`, from -1 to 1.
    pub fn at(&self, p: [f64; 2]) -> f64 {
        if self.scale <= 0.0 || !self.scale.is_finite() {
            return 0.0;
        }
        let (mut sum, mut total, mut amp, mut freq) = (0.0, 0.0, 1.0, 1.0 / self.scale);
        for o in 0..self.octaves {
            let seed = self
                .seed
                .wrapping_add(u64::from(o).wrapping_mul(0x9e37_79b9));
            sum += amp * value(seed, p[0] * freq, p[1] * freq);
            total += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / total
    }
}

/// Value noise at `(x, y)`, from -1 to 1. The value at each whole-number
/// point comes from a hash. Between the points, a smooth curve blends them.
fn value(seed: u64, x: f64, y: f64) -> f64 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i64, fy as i64);
    let (tx, ty) = (smooth(x - fx), smooth(y - fy));
    let a = lattice(seed, ix, iy);
    let b = lattice(seed, ix + 1, iy);
    let c = lattice(seed, ix, iy + 1);
    let d = lattice(seed, ix + 1, iy + 1);
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * ty
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// The value at a whole-number point, from -1 to 1.
fn lattice(seed: u64, x: i64, y: i64) -> f64 {
    let h = mix(seed ^ mix((x as u64) ^ mix(y as u64)));
    // The top 53 bits give a number from 0 to 1.
    (h >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// The finalizer of SplitMix64.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_in_the_range_and_not_flat() {
        let n = Fractal::new(7, 4, 20.0);
        let values: Vec<f64> = (0..400)
            .map(|i| n.at([f64::from(i % 20) * 3.1, f64::from(i / 20) * 2.7]))
            .collect();
        assert!(values.iter().all(|v| (-1.0..=1.0).contains(v)));
        let (lo, hi) = values
            .iter()
            .fold((1.0f64, -1.0f64), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        assert!(hi - lo > 0.5, "{lo} {hi}");
    }

    #[test]
    fn noise_is_the_same_for_the_same_seed_and_changes_with_it() {
        let p = [12.5, 40.25];
        assert_eq!(
            Fractal::new(1, 3, 10.0).at(p),
            Fractal::new(1, 3, 10.0).at(p)
        );
        assert_ne!(
            Fractal::new(1, 3, 10.0).at(p),
            Fractal::new(2, 3, 10.0).at(p)
        );
    }

    #[test]
    fn noise_is_continuous() {
        let n = Fractal::new(3, 1, 10.0);
        let (a, b) = (n.at([5.0, 5.0]), n.at([5.001, 5.0]));
        assert!((a - b).abs() < 0.01);
    }

    #[test]
    fn noise_with_no_scale_is_zero() {
        assert_eq!(Fractal::new(3, 4, 0.0).at([1.0, 2.0]), 0.0);
    }

    #[test]
    fn noise_has_fixed_values() {
        // The values must be the same on each machine.
        let n = Fractal::new(42, 3, 16.0);
        let v = [n.at([0.0, 0.0]), n.at([10.5, 3.25]), n.at([-7.0, 99.0])];
        let bits: Vec<u64> = v.iter().map(|x| x.to_bits()).collect();
        assert_eq!(bits, FIXED.to_vec(), "{v:?}");
    }

    const FIXED: [u64; 3] = [
        13826715613022833453,
        13819745236357728549,
        4590386786293308459,
    ];
}
