//! The meandered line of a river.
//!
//! This follows `meander` of Azgaar's Fantasy Map Generator (FMG,
//! `utils/path-utils.ts`). Between two anchors (the cell sites of a river),
//! the line gets one or two bend points. Near the source, the bends are
//! larger. Next to water, they are a quarter of the size. Then a bend point
//! that makes an acute corner flips to the other side of its segment
//! (`relaxAcuteAngles`). A river that joins another river turns toward the
//! flow of that river at its end (`alignJoin`).
//!
//! The anchors do not move. A pinned river thus passes its points exactly.
//!
//! The distances of FMG are in pixels of its default map. `scale` gives the
//! FMG pixels of one unit, so the line has the same shape on each map size.

use std::f64::consts::PI;

/// The bend of a river line (FMG `meandering`).
pub const MEANDERING: f64 = 0.5;
const WATER_MEANDER_SCALE: f64 = 0.25;
const RELAX_ITERATIONS: usize = 4;
const MAX_JOIN_ANGLE: f64 = PI / 5.0;
const MAX_JOIN_TURN: f64 = PI / 5.0;

/// What the meander reads for each anchor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub at: [f64; 2],
    /// `true` for a cell of water (FMG `isWaterCell`).
    pub water: bool,
    /// `true` for the point on the frame edge where a river leaves the map
    /// (the cell -1 of FMG). No bend goes before it.
    pub edge: bool,
}

/// A meandered line.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub points: Vec<[f64; 2]>,
    /// The index in `points` of each anchor.
    pub anchors: Vec<usize>,
}

/// Make the meandered line through the anchors. `start_step` is 1 for a
/// river that starts in water, else 10. `end_direction` is the flow of the
/// river that this river joins at its last anchor.
pub fn meander(
    anchors: &[Anchor],
    start_step: usize,
    scale: f64,
    end_direction: Option<[f64; 2]>,
) -> Line {
    let count = anchors.len();
    let mut points: Vec<[f64; 2]> = Vec::with_capacity(count * 2);
    let mut index = Vec::with_capacity(count);
    for i in 0..count {
        let [x1, y1] = anchors[i].at;
        index.push(points.len());
        points.push([x1, y1]);
        if i + 1 == count {
            break;
        }
        let next = anchors[i + 1];
        if next.edge {
            continue;
        }
        let [x2, y2] = next.at;
        let dist2 = ((x2 - x1).powi(2) + (y2 - y1).powi(2)) * scale * scale;
        if dist2 <= 25.0 && count >= 6 {
            continue;
        }
        let step = (start_step + i) as f64;
        let mut m = MEANDERING + 1.0 / step + (MEANDERING - step / 100.0).max(0.0);
        if anchors[i].water || next.water {
            m *= WATER_MEANDER_SCALE;
        }
        // The bend is in FMG pixels; the line is in units.
        let m = m / scale;
        let angle = libm::atan2(y2 - y1, x2 - x1);
        let (sin_m, cos_m) = (libm::sin(angle) * m, libm::cos(angle) * m);
        if step < 20.0 && (dist2 > 64.0 || (dist2 > 36.0 && count < 5)) {
            points.push([(x1 * 2.0 + x2) / 3.0 - sin_m, (y1 * 2.0 + y2) / 3.0 + cos_m]);
            points.push([
                (x1 + x2 * 2.0) / 3.0 + sin_m / 2.0,
                (y1 + y2 * 2.0) / 3.0 - cos_m / 2.0,
            ]);
        } else if dist2 > 25.0 || count < 6 {
            points.push([(x1 + x2) / 2.0 - sin_m, (y1 + y2) / 2.0 + cos_m]);
        }
    }
    relax_acute_angles(&mut points, &index);
    if let Some(d) = end_direction {
        align_join(&mut points, &mut index, d);
    }
    Line {
        points,
        anchors: index,
    }
}

/// Bend the end toward the flow that the line joins (FMG `alignJoin`).
fn align_join(points: &mut Vec<[f64; 2]>, anchors: &mut [usize], [dx, dy]: [f64; 2]) {
    let n = anchors.len();
    let last = points.len() - 1;
    if n < 2 || anchors[n - 1] != last {
        return;
    }
    let [x, y] = points[last];
    let [px, py] = points[last - 1];
    let approach = libm::atan2(y - py, x - px);
    let off_flow = approach - libm::atan2(dy, dx);
    let deviation = libm::atan2(libm::sin(off_flow), libm::cos(off_flow));
    if deviation.abs() <= MAX_JOIN_ANGLE {
        return;
    }
    let turn = deviation.signum() * (deviation.abs() - MAX_JOIN_ANGLE).min(MAX_JOIN_TURN);
    let length = libm::hypot(x - px, y - py);
    let back = |angle: f64, distance: f64| -> [f64; 2] {
        [
            x - libm::cos(angle) * distance,
            y - libm::sin(angle) * distance,
        ]
    };
    if anchors[n - 2] != last - 1 {
        points[last - 1] = back(approach - turn / 2.0, length);
    }
    points.insert(last, back(approach - turn, length / 2.0));
    anchors[n - 1] += 1;
}

/// The cosine of the corner at `b`, or -1 for a zero side.
fn corner_cos(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    let (ax, ay) = (a[0] - b[0], a[1] - b[1]);
    let (cx, cy) = (c[0] - b[0], c[1] - b[1]);
    let (la, lc) = (libm::hypot(ax, ay), libm::hypot(cx, cy));
    if la == 0.0 || lc == 0.0 {
        return -1.0;
    }
    (ax * cx + ay * cy) / (la * lc)
}

/// The mirror image of `m` across the line through `p` and `q`.
fn reflect(m: [f64; 2], p: [f64; 2], q: [f64; 2]) -> [f64; 2] {
    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return m;
    }
    let t = ((m[0] - p[0]) * dx + (m[1] - p[1]) * dy) / len2;
    let (fx, fy) = (p[0] + t * dx, p[1] + t * dy);
    [2.0 * fx - m[0], 2.0 * fy - m[1]]
}

/// Flip each bend point that makes an acute corner to the other side of
/// the line between its two anchors (FMG `relaxAcuteAngles`).
fn relax_acute_angles(points: &mut [[f64; 2]], anchors: &[usize]) {
    let n = points.len();
    if n < 3 {
        return;
    }
    let mut is_anchor = vec![false; n];
    for &i in anchors {
        is_anchor[i] = true;
    }
    let mut prev = vec![None; n];
    let mut last = None;
    for i in 0..n {
        prev[i] = last;
        if is_anchor[i] {
            last = Some(i);
        }
    }
    let mut next = vec![None; n];
    let mut last = None;
    for i in (0..n).rev() {
        next[i] = last;
        if is_anchor[i] {
            last = Some(i);
        }
    }
    let cost = |pos: &dyn Fn(usize) -> [f64; 2], i: usize| -> f64 {
        if i == 0 || i >= n - 1 {
            return 0.0;
        }
        corner_cos(pos(i - 1), pos(i), pos(i + 1)).max(0.0)
    };
    for _ in 0..RELAX_ITERATIONS {
        let snapshot = points.to_vec();
        let mut flipped_any = false;
        for i in 1..n - 1 {
            if is_anchor[i] {
                continue;
            }
            let (Some(p), Some(q)) = (prev[i], next[i]) else {
                continue;
            };
            let flipped = reflect(snapshot[i], snapshot[p], snapshot[q]);
            let at = |k: usize| snapshot[k];
            let with_flip = |k: usize| if k == i { flipped } else { snapshot[k] };
            let before = cost(&at, i - 1) + cost(&at, i) + cost(&at, i + 1);
            let after = cost(&with_flip, i - 1) + cost(&with_flip, i) + cost(&with_flip, i + 1);
            if after < before - 1e-6 {
                points[i] = flipped;
                flipped_any = true;
            }
        }
        if !flipped_any {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn land(at: [f64; 2]) -> Anchor {
        Anchor {
            at,
            water: false,
            edge: false,
        }
    }

    #[test]
    fn the_anchors_stay_and_bends_go_between_them() {
        let a: Vec<Anchor> = [[0.0, 0.0], [20.0, 0.0], [40.0, 10.0], [60.0, 10.0]]
            .into_iter()
            .map(land)
            .collect();
        let line = meander(&a, 10, 1.0, None);
        assert_eq!(line.anchors.len(), a.len());
        for (k, &i) in line.anchors.iter().enumerate() {
            assert_eq!(line.points[i], a[k].at);
        }
        // Near the source (step < 20) and far apart: two bends each.
        assert_eq!(line.points.len(), 4 + 3 * 2);
    }

    #[test]
    fn the_first_bend_follows_the_fmg_formula() {
        // Step 10: 0.5 + 1/10 + max(0.5 - 0.1, 0) = 1. The segment goes
        // east, so the first bend is 1 to the south (+y) of the 1/3 point.
        let a = [land([0.0, 0.0]), land([30.0, 0.0])];
        let line = meander(&a, 10, 1.0, None);
        assert_eq!(line.points.len(), 4);
        assert!((line.points[1][0] - 10.0).abs() < 1e-12);
        assert!((line.points[1][1] - 1.0).abs() < 1e-12);
        assert!((line.points[2][0] - 20.0).abs() < 1e-12);
        assert!((line.points[2][1] + 0.5).abs() < 1e-12);
    }

    #[test]
    fn the_bend_scales_with_the_map_and_is_small_next_to_water() {
        let a = [land([0.0, 0.0]), land([30.0, 0.0])];
        // Two FMG pixels for each unit: the bend is half as many units.
        let line = meander(&a, 10, 2.0, None);
        assert!((line.points[1][1] - 0.5).abs() < 1e-12);
        let w = [
            land([0.0, 0.0]),
            Anchor {
                water: true,
                ..land([30.0, 0.0])
            },
        ];
        let line = meander(&w, 10, 1.0, None);
        assert!((line.points[1][1] - 0.25).abs() < 1e-12);
    }

    #[test]
    fn no_bend_goes_before_the_frame_edge() {
        let a = [
            land([0.0, 50.0]),
            land([30.0, 50.0]),
            Anchor {
                edge: true,
                ..land([30.0, 0.0])
            },
        ];
        let line = meander(&a, 10, 1.0, None);
        assert_eq!(line.anchors, vec![0, 3, 4]);
    }

    #[test]
    fn close_anchors_get_no_bend_on_a_long_river() {
        let a: Vec<Anchor> = (0..6).map(|i| land([f64::from(i) * 4.0, 0.0])).collect();
        let line = meander(&a, 10, 1.0, None);
        assert_eq!(line.points.len(), 6);
    }

    #[test]
    fn an_acute_bend_flips() {
        let mut points = vec![[0.0, 0.0], [10.0, 0.0], [5.0, 1.0], [20.0, 0.0]];
        // The corner at the anchor 1 is acute, but the flip of point 2 to
        // the other side of the line 1-3 gives the same corners. It stays.
        relax_acute_angles(&mut points, &[0, 1, 3]);
        assert_eq!(points[2], [5.0, 1.0]);
        // Here the bend point makes an acute corner at the anchor 1, and
        // the flip removes it.
        let mut points = vec![[0.0, 0.0], [10.0, 0.0], [8.0, 5.0], [10.0, 10.0]];
        relax_acute_angles(&mut points, &[0, 1, 3]);
        assert_eq!(points[2], [12.0, 5.0]);
    }

    #[test]
    fn a_join_turns_toward_the_flow() {
        // The line comes from the west; the river that it joins flows
        // south. The end turns by at most 36 degrees.
        let a = [land([0.0, 0.0]), land([30.0, 0.0])];
        let line = meander(&a, 30, 1.0, Some([0.0, 1.0]));
        let n = line.points.len();
        assert_eq!(*line.anchors.last().unwrap(), n - 1);
        assert_eq!(line.points[n - 1], [30.0, 0.0]);
        let [px, py] = line.points[n - 2];
        // The last part comes in from the north-west: it turned toward south.
        assert!(py < 0.0 && px < 30.0);
        // A join along the flow does not change.
        let line = meander(&a, 30, 1.0, Some([1.0, 0.0]));
        let plain = meander(&a, 30, 1.0, None);
        assert_eq!(line, plain);
    }
}
