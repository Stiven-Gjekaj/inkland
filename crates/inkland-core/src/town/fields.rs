//! inkland: the fields of the farms around a town.
//!
//! Each farm patch becomes strips of fields along one direction, with a
//! path between the strips. Long strips are cut across into shorter
//! fields. The fields use a random source of their own, so that they do
//! not change the rest of the town.

use super::poly::{self, P, Poly};
use super::random::Random;
use super::{Kind, Town};

/// The width of the path between two fields, in town units.
const PATH: f64 = 1.0;

/// The fields of the farm patches of a town.
pub fn fields(town: &Town, seed: i64) -> Vec<Poly> {
    let mut rng = Random::new(seed ^ 0x5eed_f1e1);
    let mut out = Vec::new();
    for p in &town.patches {
        if p.kind != Kind::Farm {
            continue;
        }
        let shape = poly::shrink_eq(&p.shape, PATH);
        if shape.len() < 3 || poly::square(&shape).abs() < 50.0 {
            continue;
        }
        let a = rng.float() * std::f64::consts::PI;
        let along = [libm::cos(a), libm::sin(a)];
        let across = poly::rotate90(along);
        let width = 8.0 + rng.float() * 6.0;
        for strip in strips(&shape, across, width) {
            let length = 18.0 + rng.float() * 20.0;
            out.extend(strips(&strip, along, length));
        }
    }
    out
}

/// Cut a polygon into strips of about `width` across the direction `dir`.
fn strips(shape: &[P], dir: P, width: f64) -> Vec<Poly> {
    let ts: Vec<f64> = shape.iter().map(|&v| poly::dot(v, dir)).collect();
    let lo = ts.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = ts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let count = ((hi - lo) / width).round().max(1.0) as usize;
    let step = (hi - lo) / count as f64;
    let side = poly::rotate90(dir);
    let mut rest: Poly = shape.to_vec();
    let mut out = Vec::new();
    for i in 1..count {
        let p1 = poly::scale(dir, lo + step * i as f64);
        let p2 = poly::add(p1, side);
        let halves = poly::cut(&rest, p1, p2, PATH);
        if halves.len() != 2 {
            continue;
        }
        let t = |h: &Poly| poly::dot(poly::centroid(h), dir);
        let (near, far) = if t(&halves[0]) < t(&halves[1]) {
            (halves[0].clone(), halves[1].clone())
        } else {
            (halves[1].clone(), halves[0].clone())
        };
        out.push(near);
        rest = far;
    }
    out.push(rest);
    out.retain(|f| f.len() >= 3 && poly::square(f).abs() > 4.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_cuts_into_strips_with_paths() {
        let s = poly::rect(40.0, 40.0);
        let out = strips(&s, [1.0, 0.0], 10.0);
        assert_eq!(out.len(), 4);
        let total: f64 = out.iter().map(|f| poly::square(f).abs()).sum();
        // The paths take a part of the area.
        assert!(total < 1600.0 && total > 1400.0, "{total}");
    }
}
