//! Fine ripples along the shore of a lake, and a few open strokes on a
//! pond (`lake-ripples.ts`). All lengths are in page pixels.

use super::super::geom::n;
use inkland_core::rand::Rand;
use std::fmt::Write as _;

/// The look of the ripples of a lake group (the `options` of a lakes group).
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    /// "ripples", "lines", or "none".
    pub embellishment: String,
    pub density: f64,
    pub length: f64,
    pub halo: f64,
    pub width: f64,
}

/// The ripples of one lake: the path, the bounds of the lake, and the blank
/// water along the shore.
#[derive(Debug, Clone, PartialEq)]
pub struct Ripples {
    pub path: String,
    pub bounds: [f64; 4],
    pub gap: f64,
}

// The cell spacing is the ripple unit. A lake narrower than UNIT_CELLS
// cells has a smaller unit.
const UNIT_CELLS: f64 = 4.0;
const MIN_SIZE: f64 = 0.65;
const MIN_SIZE_WIDTHS: f64 = 8.0;
const POND_SIZE: f64 = 4.0;
const POND_ROWS: f64 = 1.5;
const POND_LENGTH: f64 = 1.5;
const POND_MIN_LENGTH: f64 = 0.3;
const POND_MIN_WIDTHS: f64 = 6.0;
const POND_BOW: f64 = 0.04;
const ROW_GAP: f64 = 0.32;
const AMPLITUDE: f64 = 0.045;
const REACH: f64 = 3.0;
const FRINGE: f64 = 0.75;
const FRINGE_WOBBLE: f64 = 0.15;
const DASH: f64 = 1.8;
const MIN_DASH: f64 = 0.4;
const HALF_PERIOD: f64 = 0.3;

/// The ripples of a lake with the outline `points` (`getLakeRipples`).
pub fn lake_ripples(points: &[[f64; 2]], spacing: f64, look: &Look, rng: &mut Rand) -> Ripples {
    let (mut x, mut y) = (f64::INFINITY, f64::INFINITY);
    let (mut right, mut bottom) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &[px, py] in points {
        x = x.min(px);
        right = right.max(px);
        y = y.min(py);
        bottom = bottom.max(py);
    }
    let (width, height) = (right - x, bottom - y);
    let unit = spacing.min(width.min(height) / UNIT_CELLS).max(1.0);
    let gap = look.width.max(unit * look.halo);
    let mut out = Ripples {
        path: String::new(),
        bounds: [x, y, width, height],
        gap,
    };
    let blank = points.len() < 3
        || width.min(height) < (spacing * MIN_SIZE).max(look.width * MIN_SIZE_WIDTHS)
        || look.embellishment == "none";
    if blank {
        return out;
    }
    out.path = if width.max(height) <= spacing * POND_SIZE {
        pond_strokes(points, spacing, look, &out, rng)
    } else {
        shore_ripples(points, unit, look, &out, rng)
    };
    out
}

/// A wavy dash: `halves` half waves of height `amplitude` (`wavyDash`).
fn wavy_dash(d: &mut String, x: f64, y: f64, length: f64, halves: usize, amplitude: f64) {
    let step = length / halves as f64;
    let _ = write!(
        d,
        "M{},{}q{},{} {},0",
        n(x),
        n(y),
        n(step / 2.0),
        n(amplitude),
        n(step)
    );
    for _ in 1..halves {
        let _ = write!(d, "t{},0", n(step));
    }
}

fn pond_strokes(
    points: &[[f64; 2]],
    spacing: f64,
    look: &Look,
    b: &Ripples,
    rng: &mut Rand,
) -> String {
    let [_, y, _, height] = b.bounds;
    let rows = ((height / spacing) * POND_ROWS * look.density)
        .round()
        .clamp(2.0, 5.0) as usize;
    let min_length = (spacing * POND_MIN_LENGTH).max(look.width * POND_MIN_WIDTHS);
    let mut d = String::new();
    for i in 0..rows {
        // The middle three fifths of the pond.
        let row = y + height * (0.2 + ((i as f64 + 0.5) / rows as f64) * 0.6);
        let spans = water_spans(points, row);
        for pair in spans.as_chunks::<2>().0 {
            let available = pair[1] - pair[0] - b.gap * 2.0;
            let length =
                (available * (0.55 + rng.unit() * 0.2)).min(spacing * look.length * POND_LENGTH);
            if length < min_length {
                continue;
            }
            let left = pair[0] + b.gap + (available - length) * (0.35 + rng.unit() * 0.3);
            if look.embellishment == "lines" {
                let _ = write!(d, "M{},{}h{}", n(left), n(row), n(length));
            } else {
                wavy_dash(&mut d, left, row, length, 2, length * POND_BOW);
            }
        }
    }
    d
}

fn shore_ripples(
    points: &[[f64; 2]],
    unit: f64,
    look: &Look,
    b: &Ripples,
    rng: &mut Rand,
) -> String {
    let [_, y, _, height] = b.bounds;
    let gap = b.gap;
    let row_gap = unit * ROW_GAP / look.density.max(0.01);
    let amplitude = (unit * AMPLITUDE).min(row_gap * 0.2);
    let reach = unit * REACH;
    let phase = rng.unit() * std::f64::consts::TAU;
    let mut d = String::new();
    let mut row = y + gap + row_gap * (0.4 + rng.unit() * 0.4);
    while row < y + height - gap {
        let crossings = water_spans(points, row);
        let nearby = edges_near(points, row, reach);
        for pair in crossings.as_chunks::<2>().0 {
            let (start, end) = (pair[0] + gap, pair[1] - gap);
            let mut left = start + unit * rng.unit() * 0.25;
            while left < end {
                let length = (unit * look.length * (DASH + rng.unit() * DASH)).min(end - left);
                if length < unit * MIN_DASH {
                    break;
                }
                let mid = left + length / 2.0;
                let distance = shore_distance(points, &nearby, mid, row);
                let fringe = reach
                    * (FRINGE
                        + FRINGE_WOBBLE
                            * libm::sin((mid / unit) * 0.7 + (row / unit) * 0.4 + phase));
                if distance < fringe && rng.unit() < ((1.0 - distance / fringe) * 2.0).min(1.0) {
                    if look.embellishment == "lines" {
                        let _ = write!(d, "M{},{}h{}", n(left), n(row), n(length));
                    } else {
                        let halves = ((length / (unit * HALF_PERIOD)).round() as usize).max(2);
                        let up = if rng.unit() < 0.5 { -1.0 } else { 1.0 };
                        wavy_dash(&mut d, left, row, length, halves, amplitude * up);
                    }
                }
                left += length + unit * (0.15 + rng.unit() * 0.4);
            }
        }
        row += row_gap * (0.9 + rng.unit() * 0.2);
    }
    d
}

/// The x values where the line at `row` crosses the outline, in order.
/// Each pair bounds water.
fn water_spans(points: &[[f64; 2]], row: f64) -> Vec<f64> {
    let k = points.len();
    let mut out: Vec<f64> = (0..k)
        .filter_map(|i| {
            let [ax, ay] = points[i];
            let [bx, by] = points[(i + 1) % k];
            ((ay > row) != (by > row)).then(|| ax + (row - ay) * (bx - ax) / (by - ay))
        })
        .collect();
    out.sort_by(f64::total_cmp);
    out
}

/// The outline edges that come within `reach` of the line at `row`.
fn edges_near(points: &[[f64; 2]], row: f64, reach: f64) -> Vec<usize> {
    let k = points.len();
    (0..k)
        .filter(|&i| {
            let (ay, by) = (points[i][1], points[(i + 1) % k][1]);
            row >= ay.min(by) - reach && row <= ay.max(by) + reach
        })
        .collect()
}

fn shore_distance(points: &[[f64; 2]], edges: &[usize], px: f64, py: f64) -> f64 {
    let k = points.len();
    let mut distance = f64::INFINITY;
    for &i in edges {
        let [ax, ay] = points[i];
        let [bx, by] = points[(i + 1) % k];
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = dx * dx + dy * dy;
        let t = (((px - ax) * dx + (py - ay) * dy) / if len2 == 0.0 { 1.0 } else { len2 })
            .clamp(0.0, 1.0);
        distance = distance.min(libm::hypot(px - ax - t * dx, py - ay - t * dy));
    }
    distance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(kind: &str) -> Look {
        Look {
            embellishment: kind.to_owned(),
            density: 2.0,
            length: 0.4,
            halo: 0.1,
            width: 0.2,
        }
    }

    fn rng() -> Rand {
        use inkland_core::seed::{Layer, SeedTree};
        Rand::new(SeedTree::new(7).part_rng(Layer::Decoration, "lake-1"))
    }

    fn square(size: f64) -> Vec<[f64; 2]> {
        vec![[0.0, 0.0], [size, 0.0], [size, size], [0.0, size]]
    }

    #[test]
    fn a_large_lake_gets_ripples_near_the_shore_only() {
        let mut rng = rng();
        let r = lake_ripples(&square(100.0), 5.0, &look("ripples"), &mut rng);
        assert_eq!(r.bounds, [0.0, 0.0, 100.0, 100.0]);
        assert!(r.path.contains('q') && r.path.contains('t'));
        // Each ripple starts inside the lake, and none is at the center:
        // the ripples fade out at 3 units from the shore.
        for part in r.path.split('M').skip(1) {
            let mut xy = part.split(['q', 'h']).next().unwrap().split(',');
            let x: f64 = xy.next().unwrap().parse().unwrap();
            let y: f64 = xy.next().unwrap().parse().unwrap();
            assert!((0.0..100.0).contains(&x) && (0.0..100.0).contains(&y));
            let shore = x.min(100.0 - x).min(y).min(100.0 - y);
            assert!(shore < 5.0 * 3.0 + 5.0 * 3.6, "{x},{y}");
        }
    }

    #[test]
    fn a_pond_gets_a_few_open_strokes() {
        let mut rng = rng();
        let r = lake_ripples(&square(15.0), 5.0, &look("lines"), &mut rng);
        let strokes = r.path.matches('M').count();
        assert!((2..=5).contains(&strokes), "{}", r.path);
        assert!(r.path.contains('h') && !r.path.contains('q'));
    }

    #[test]
    fn a_small_lake_or_none_stays_blank() {
        let mut rng = rng();
        assert!(
            lake_ripples(&square(2.0), 5.0, &look("ripples"), &mut rng)
                .path
                .is_empty()
        );
        assert!(
            lake_ripples(&square(100.0), 5.0, &look("none"), &mut rng)
                .path
                .is_empty()
        );
    }
}
