//! The drawn line of a road.
//!
//! A road goes from cell to cell, so its points make straight segments
//! with corners. The drawn line is a smooth curve through these points:
//! a centripetal Catmull-Rom spline, with a few points on each segment.
//! Each segment also gets a small wander to one side. The wander is zero
//! at the two ends of the segment, and it has no slope there. Thus the
//! line goes through each point exactly, with no corner.
//!
//! The wander of a segment comes from the seed tree and from the two end
//! points of the segment. It does not come from the order of the roads.
//! Thus two roads that share a segment draw it on the same line, and a
//! change to one road does not move the line of another road.
//!
//! The renderers draw this line, and the road stage measures its length.
//! Thus the length in the report is the length of the drawn road.

use crate::seed::{Layer, SeedTree};

/// The points of the drawn line on each segment, the end point included.
pub const SAMPLES: usize = 6;

/// The largest wander of a segment, as a fraction of its length.
pub const WANDER: f64 = 0.06;

/// The seed of the road wander of a map.
pub fn wander_seed(master: u64) -> u64 {
    let s = SeedTree::new(master).part_seed(Layer::Roads, "wander");
    u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])
}

/// A 64-bit mix (SplitMix64).
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A point in thousandths of a unit, so that the hash does not depend on
/// the last bits of a coordinate.
fn key(p: [f64; 2]) -> (i64, i64) {
    (
        libm::round(p[0] * 1000.0) as i64,
        libm::round(p[1] * 1000.0) as i64,
    )
}

/// The wander of the segment from `a` to `b`, from -1 to 1. The value
/// does not depend on the direction of the segment: from `b` to `a`, the
/// sign changes, so that the offset goes to the same side.
fn wander(seed: u64, a: [f64; 2], b: [f64; 2]) -> f64 {
    let (ka, kb) = (key(a), key(b));
    let (lo, hi, sign) = if ka <= kb {
        (ka, kb, 1.0)
    } else {
        (kb, ka, -1.0)
    };
    let mut h = mix(seed);
    for v in [lo.0, lo.1, hi.0, hi.1] {
        h = mix(h ^ v as u64);
    }
    // 53 bits give a value from 0 to 1.
    let unit = (h >> 11) as f64 / (1u64 << 53) as f64;
    sign * (2.0 * unit - 1.0)
}

/// The drawn line through the points of a road. The point `k` of the
/// input is the point `k * SAMPLES` of the output.
pub fn road_line(seed: u64, points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let m = points.len();
    if m < 2 {
        return points.to_vec();
    }
    // The centripetal parameter: the square root of the distance.
    let knot = |a: [f64; 2], b: [f64; 2]| -> f64 {
        let d2 = (a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]);
        libm::sqrt(libm::sqrt(d2))
    };
    let mut out = Vec::with_capacity((m - 1) * SAMPLES + 1);
    out.push(points[0]);
    for k in 0..m - 1 {
        let p1 = points[k];
        let p2 = points[k + 1];
        // The end points repeat: the tangent at an end follows its segment.
        let p0 = if k == 0 {
            [2.0 * p1[0] - p2[0], 2.0 * p1[1] - p2[1]]
        } else {
            points[k - 1]
        };
        let p3 = if k + 2 < m {
            points[k + 2]
        } else {
            [2.0 * p2[0] - p1[0], 2.0 * p2[1] - p1[1]]
        };
        let t01 = knot(p0, p1);
        let t12 = knot(p1, p2);
        let t23 = knot(p2, p3);
        if t12 <= 1e-12 {
            for _ in 0..SAMPLES {
                out.push(p2);
            }
            continue;
        }
        // The tangents of the Hermite form (Barry and Goldman), for the
        // parameter from 0 to 1 on this segment.
        let tangent = |a: [f64; 2], b: [f64; 2], c: [f64; 2], ta: f64, tb: f64| -> [f64; 2] {
            let mut v = [0.0; 2];
            for i in 0..2 {
                let ab = if ta > 1e-12 { (b[i] - a[i]) / ta } else { 0.0 };
                let ac = if ta + tb > 1e-12 {
                    (c[i] - a[i]) / (ta + tb)
                } else {
                    0.0
                };
                let bc = if tb > 1e-12 { (c[i] - b[i]) / tb } else { 0.0 };
                v[i] = (ab - ac + bc) * t12;
            }
            v
        };
        let m1 = tangent(p0, p1, p2, t01, t12);
        let m2 = tangent(p1, p2, p3, t12, t23);
        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        let len = libm::sqrt(dx * dx + dy * dy);
        let off = WANDER * len * wander(seed, p1, p2);
        let normal = [-dy / len, dx / len];
        for s in 1..=SAMPLES {
            if s == SAMPLES {
                out.push(p2);
                break;
            }
            let t = s as f64 / SAMPLES as f64;
            let (t2, t3) = (t * t, t * t * t);
            let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
            let h10 = t3 - 2.0 * t2 + t;
            let h01 = -2.0 * t3 + 3.0 * t2;
            let h11 = t3 - t2;
            // The square of a sine has no slope at the ends, so the line
            // has no corner at a point.
            let sine = libm::sin(std::f64::consts::PI * t);
            let bump = off * sine * sine;
            out.push([
                h00 * p1[0] + h10 * m1[0] + h01 * p2[0] + h11 * m2[0] + bump * normal[0],
                h00 * p1[1] + h10 * m1[1] + h01 * p2[1] + h11 * m2[1] + bump * normal[1],
            ]);
        }
    }
    out
}

/// The length of a line.
pub fn line_length(points: &[[f64; 2]]) -> f64 {
    points
        .windows(2)
        .map(|w| libm::hypot(w[1][0] - w[0][0], w[1][1] - w[0][1]))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zigzag() -> Vec<[f64; 2]> {
        vec![
            [0.0, 0.0],
            [10.0, 4.0],
            [20.0, 0.0],
            [30.0, 5.0],
            [40.0, 1.0],
        ]
    }

    #[test]
    fn the_line_goes_through_each_point() {
        let pts = zigzag();
        let line = road_line(wander_seed(7), &pts);
        assert_eq!(line.len(), (pts.len() - 1) * SAMPLES + 1);
        for (k, p) in pts.iter().enumerate() {
            assert_eq!(line[k * SAMPLES], *p);
        }
    }

    /// The turn at the sample `i` of a line.
    fn turn(line: &[[f64; 2]], i: usize) -> f64 {
        let a = [line[i][0] - line[i - 1][0], line[i][1] - line[i - 1][1]];
        let b = [line[i + 1][0] - line[i][0], line[i + 1][1] - line[i][1]];
        libm::atan2(a[0] * b[1] - a[1] * b[0], a[0] * b[0] + a[1] * b[1]).abs()
    }

    #[test]
    fn the_line_has_no_corner_at_a_point() {
        let pts = zigzag();
        let line = road_line(wander_seed(7), &pts);
        // At an inner point, the line turns less than the straight
        // segments turn, and not more than in the samples next to it.
        for k in 1..pts.len() - 1 {
            let i = k * SAMPLES;
            let corner = turn(&pts, k);
            let near = turn(&line, i - 1).max(turn(&line, i + 1));
            assert!(turn(&line, i) < corner, "point {k}");
            assert!(turn(&line, i) <= near * 1.5, "point {k}");
        }
    }

    #[test]
    fn the_wander_comes_from_the_seed() {
        let pts = vec![[0.0, 0.0], [10.0, 0.0]];
        let a = road_line(wander_seed(1), &pts);
        let b = road_line(wander_seed(1), &pts);
        let c = road_line(wander_seed(2), &pts);
        assert_eq!(a, b);
        assert_ne!(a, c);
        // The wander stays small.
        for p in &a {
            assert!(p[1].abs() <= WANDER * 10.0 + 1e-9);
        }
    }

    #[test]
    fn a_shared_segment_has_one_line_in_both_directions() {
        let seed = wander_seed(3);
        let pts = vec![[0.0, 0.0], [12.0, 3.0]];
        let back: Vec<[f64; 2]> = pts.iter().rev().copied().collect();
        let a = road_line(seed, &pts);
        let mut b = road_line(seed, &back);
        b.reverse();
        for (p, q) in a.iter().zip(&b) {
            assert!((p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9);
        }
    }

    #[test]
    fn the_length_is_the_length_of_the_samples() {
        let line = road_line(wander_seed(5), &zigzag());
        let sum: f64 = line
            .windows(2)
            .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
            .sum();
        assert!((line_length(&line) - sum).abs() < 1e-9);
        assert!(line_length(&line) >= line_length(&zigzag()) * 0.95);
    }
}
