//! The hand-drawn look of `print-safe` mode (`render.hand_drawn`).
//!
//! A pen line is not exact. This module gives the lines of `print-safe`
//! mode three small faults of a hand:
//!
//! - A wobble: each line moves a little to the left and to the right of
//!   its true position. The wobble is a smooth noise along the length of
//!   the line, so the line has no sharp corners from it.
//! - A taper: a line is in pieces, and each piece has its own width. A
//!   river becomes wider from its source to its mouth. A coast becomes
//!   wider and thinner along its length. No piece is thinner than
//!   `min_line_width_pt`.
//! - Irregular dashes: each dash and each gap of a dash pattern has a
//!   small change in its length.
//!
//! All values are in millimetres on the page. The noise comes from the
//! seed tree and from a key for each line, for example its id. Thus a
//! line does not change when another line changes.

use inkland_core::seed::{Layer, SeedTree};

/// The distance between two points of a wobbled line, in millimetres.
pub const STEP_MM: f64 = 0.8;
/// The largest wobble, in millimetres.
pub const WOBBLE_MM: f64 = 0.16;
/// The length of one wave of the wobble, in millimetres.
pub const WAVE_MM: f64 = 5.0;
/// The length at each end of an open line where the wobble grows from 0.
pub const EASE_MM: f64 = 2.0;
/// The length of one piece of a tapered line, in millimetres.
pub const PIECE_MM: f64 = 4.0;
/// The most pieces of one tapered line.
pub const MAX_PIECES: usize = 24;
/// The largest change of the length of a dash or a gap, as a fraction.
pub const DASH_JITTER: f64 = 0.3;
/// The dash pattern repeats this many times with other changes before it
/// starts again.
pub const DASH_REPEATS: usize = 4;

/// The hand of one map. `None` from `Hand::of` when the look is off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hand {
    seed: u64,
}

/// A 64-bit mix (SplitMix64).
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A value from -1 to 1.
fn signed(h: u64) -> f64 {
    let unit = (h >> 11) as f64 / (1u64 << 53) as f64;
    2.0 * unit - 1.0
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    libm::hypot(b[0] - a[0], b[1] - a[1])
}

/// The length of a line.
pub fn length(points: &[[f64; 2]]) -> f64 {
    points.windows(2).map(|w| dist(w[0], w[1])).sum()
}

impl Hand {
    /// The hand of a map with this master seed.
    pub fn new(master: u64) -> Self {
        let s = SeedTree::new(master).part_seed(Layer::Decoration, "hand");
        Self {
            seed: u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]),
        }
    }

    /// The hand of a config, or `None` if `render.hand_drawn` is `false`.
    pub fn of(config: &inkland_core::config::Config) -> Option<Self> {
        config.render.hand_drawn.then(|| Self::new(config.seed))
    }

    /// The seed of one line.
    fn line_seed(&self, key: &str) -> u64 {
        let mut h = mix(self.seed);
        for b in key.bytes() {
            h = mix(h ^ u64::from(b));
        }
        h
    }

    /// A smooth noise from -1 to 1 at the length `s` along a line. With
    /// `period`, the noise repeats after that length.
    fn noise(seed: u64, s: f64, wave: f64, period: Option<f64>) -> f64 {
        let (wave, count) = match period {
            Some(p) => {
                let count = libm::round(p / wave).max(1.0);
                (p / count, Some(count as i64))
            }
            None => (wave, None),
        };
        let x = s / wave;
        let i = libm::floor(x);
        let f = x - i;
        let at = |k: i64| -> f64 {
            let k = match count {
                Some(c) => k.rem_euclid(c),
                None => k,
            };
            signed(mix(seed ^ (k as u64).wrapping_mul(0x2545_f491_4f6c_dd1d)))
        };
        let a = at(i as i64);
        let b = at(i as i64 + 1);
        let t = (1.0 - libm::cos(std::f64::consts::PI * f)) / 2.0;
        a + (b - a) * t
    }

    /// The line with a point at least each `STEP_MM`. The points of the
    /// input stay.
    fn dense(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
        let mut out = Vec::with_capacity(points.len() * 2);
        for (i, &p) in points.iter().enumerate() {
            if i > 0 {
                let a = points[i - 1];
                let parts = libm::ceil(dist(a, p) / STEP_MM).max(1.0) as usize;
                for k in 1..parts {
                    let t = k as f64 / parts as f64;
                    out.push([a[0] + (p[0] - a[0]) * t, a[1] + (p[1] - a[1]) * t]);
                }
            }
            out.push(p);
        }
        out
    }

    /// The wobbled line through the points, in millimetres on the page. A
    /// closed line has its first point again at its end; the wobble of a
    /// closed line repeats, so that the line has no step where it closes.
    /// An open line keeps its two end points.
    pub fn wobble(&self, key: &str, points: &[[f64; 2]], closed: bool) -> Vec<[f64; 2]> {
        if points.len() < 2 {
            return points.to_vec();
        }
        let pts = Self::dense(points);
        let m = pts.len();
        let mut along = vec![0.0; m];
        for i in 1..m {
            along[i] = along[i - 1] + dist(pts[i - 1], pts[i]);
        }
        let total = along[m - 1];
        if total <= 1e-9 {
            return pts;
        }
        let seed = self.line_seed(key);
        let ring = closed && m > 2 && dist(pts[0], pts[m - 1]) < 1e-9;
        let period = ring.then_some(total);
        let mut out = Vec::with_capacity(m);
        for i in 0..m {
            // The direction at the point, from its two neighbours.
            let (a, b) = if ring && (i == 0 || i == m - 1) {
                (pts[m - 2], pts[1])
            } else {
                (pts[i.saturating_sub(1)], pts[(i + 1).min(m - 1)])
            };
            let d = dist(a, b);
            if d <= 1e-12 {
                out.push(pts[i]);
                continue;
            }
            let normal = [-(b[1] - a[1]) / d, (b[0] - a[0]) / d];
            let s = along[i];
            let mut amp = WOBBLE_MM
                * (0.75 * Self::noise(seed, s, WAVE_MM, period)
                    + 0.25 * Self::noise(mix(seed), s, WAVE_MM / 3.0, period));
            if !ring {
                let edge = s.min(total - s) / EASE_MM;
                if edge < 1.0 {
                    amp *= edge * edge * (3.0 - 2.0 * edge);
                }
            }
            out.push([pts[i][0] + amp * normal[0], pts[i][1] + amp * normal[1]]);
        }
        if ring {
            out[m - 1] = out[0];
        }
        out
    }

    /// Cut a line into pieces of about `PIECE_MM`. Two pieces that touch
    /// share a point. Each piece gets the length along the line at its
    /// middle, as a fraction of the full length.
    pub fn pieces(points: &[[f64; 2]]) -> Vec<(Vec<[f64; 2]>, f64)> {
        let total = length(points);
        let count = (libm::round(total / PIECE_MM) as usize).clamp(1, MAX_PIECES);
        if count == 1 || points.len() < 3 {
            return vec![(points.to_vec(), 0.5)];
        }
        let mut out = Vec::with_capacity(count);
        let mut current = vec![points[0]];
        let mut s = 0.0;
        let mut start = 0.0;
        let mut k = 1;
        for w in points.windows(2) {
            s += dist(w[0], w[1]);
            current.push(w[1]);
            if k < count && s >= total * k as f64 / count as f64 {
                out.push((current, (start + s) / 2.0 / total));
                current = vec![w[1]];
                start = s;
                k += 1;
            }
        }
        if current.len() >= 2 {
            out.push((current, (start + total) / 2.0 / total));
        }
        out
    }

    /// The width of each piece of a coast, in points: the base width,
    /// more and less along the line, never less than `min_pt`.
    pub fn coast_widths(&self, key: &str, count: usize, base_pt: f64, min_pt: f64) -> Vec<f64> {
        let seed = mix(self.line_seed(key));
        (0..count)
            .map(|i| {
                let v = Self::noise(seed, i as f64, 2.5, Some(count as f64));
                (base_pt * (1.0 + 0.3 * v)).max(min_pt)
            })
            .collect()
    }

    /// The width of a river at the fraction `t` of its length, from the
    /// source (0) to the mouth (1), in points. The source is 45 percent of
    /// the mouth width, and no width is less than `min_pt`.
    pub fn river_width(mouth_pt: f64, t: f64, min_pt: f64) -> f64 {
        let source = mouth_pt * 0.45;
        (source + (mouth_pt - source) * t.clamp(0.0, 1.0)).max(min_pt)
    }

    /// An irregular dash pattern from a base pattern, in millimetres. The
    /// pattern repeats `DASH_REPEATS` times, each with other changes. No
    /// dash or gap is less than 70 percent of its base length.
    pub fn dashes(&self, key: &str, base: &[f64]) -> Vec<f64> {
        let seed = mix(self.line_seed(key) ^ 0xd45);
        let mut out = Vec::with_capacity(base.len() * DASH_REPEATS);
        let mut h = seed;
        for _ in 0..DASH_REPEATS {
            for &v in base {
                h = mix(h);
                out.push(v * (1.0 + DASH_JITTER * signed(h)));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> Vec<[f64; 2]> {
        vec![[0.0, 0.0], [20.0, 0.0], [40.0, 10.0]]
    }

    #[test]
    fn the_wobble_is_small_and_comes_from_the_seed() {
        let h = Hand::new(5);
        let a = h.wobble("a", &line(), false);
        assert_eq!(a, h.wobble("a", &line(), false));
        assert_ne!(a, Hand::new(6).wobble("a", &line(), false));
        assert_ne!(a, h.wobble("b", &line(), false));
        // Each point is near the true line.
        for p in &a {
            let near = if p[0] <= 20.0 {
                p[1].abs()
            } else {
                // The distance to the line from (20, 0) to (40, 10).
                ((p[0] - 20.0) * 10.0 - p[1] * 20.0).abs() / 500f64.sqrt()
            };
            assert!(near <= WOBBLE_MM + 1e-9, "{p:?}");
        }
        assert!(a.iter().any(|p| p[1].abs() > 0.01));
    }

    #[test]
    fn an_open_line_keeps_its_ends() {
        let a = Hand::new(5).wobble("a", &line(), false);
        assert_eq!(a[0], [0.0, 0.0]);
        assert_eq!(*a.last().unwrap(), [40.0, 10.0]);
    }

    #[test]
    fn a_closed_line_stays_closed() {
        let ring = vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0],
        ];
        let a = Hand::new(5).wobble("ring", &ring, true);
        assert_eq!(a[0], *a.last().unwrap());
        assert_ne!(a[0], [0.0, 0.0]);
    }

    #[test]
    fn the_pieces_cover_the_line() {
        let pts = Hand::dense(&line());
        let pieces = Hand::pieces(&pts);
        assert!(pieces.len() > 1);
        let sum: f64 = pieces.iter().map(|(p, _)| length(p)).sum();
        assert!((sum - length(&pts)).abs() < 1e-9);
        for w in pieces.windows(2) {
            assert_eq!(w[0].0.last(), w[1].0.first());
            assert!(w[0].1 < w[1].1);
        }
    }

    #[test]
    fn no_width_is_less_than_the_minimum() {
        let h = Hand::new(5);
        for w in h.coast_widths("c", 20, 0.75, 0.7) {
            assert!(w >= 0.7);
        }
        assert!(h.coast_widths("c", 20, 0.75, 0.0).iter().any(|&w| w < 0.7));
        assert_eq!(Hand::river_width(2.0, 0.0, 0.5), 0.9);
        assert_eq!(Hand::river_width(2.0, 1.0, 0.5), 2.0);
        assert_eq!(Hand::river_width(0.6, 0.0, 0.5), 0.5);
    }

    #[test]
    fn the_dashes_change_but_stay_near_the_base() {
        let d = Hand::new(5).dashes("road", &[1.6, 0.5]);
        assert_eq!(d.len(), 2 * DASH_REPEATS);
        for (i, v) in d.iter().enumerate() {
            let base = [1.6, 0.5][i % 2];
            assert!(*v >= base * (1.0 - DASH_JITTER) && *v <= base * (1.0 + DASH_JITTER));
        }
        assert!(d[0] != d[2]);
    }
}
