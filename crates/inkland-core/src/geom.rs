//! Plane geometry for the checks that do not need terrain.
//!
//! The functions use only `+`, `-`, `*`, `/`, and comparisons, so they give
//! the same result on each machine.

use crate::config::{Point, RegionPin};

/// The shape of a region.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Polygon(Vec<Point>),
    Circle { at: Point, r: f64 },
}

impl Shape {
    /// The shape of a region pin, if it has exactly one valid shape field.
    pub fn of(region: &RegionPin) -> Option<Shape> {
        match (&region.polygon, &region.rect, &region.circle) {
            (Some(p), None, None) => Some(Shape::Polygon(p.clone())),
            (None, Some([x0, y0, x1, y1]), None) => Some(Shape::Polygon(vec![
                [*x0, *y0],
                [*x1, *y0],
                [*x1, *y1],
                [*x0, *y1],
            ])),
            (None, None, Some(c)) => Some(Shape::Circle { at: c.at, r: c.r }),
            _ => None,
        }
    }

    /// The points that define the shape. For a circle: the four points of
    /// its bounding box.
    pub fn extreme_points(&self) -> Vec<Point> {
        match self {
            Shape::Polygon(p) => p.clone(),
            Shape::Circle { at: [x, y], r } => {
                vec![[x - r, *y], [x + r, *y], [*x, y - r], [*x, y + r]]
            }
        }
    }

    /// `true` if `p` is inside the shape or on its edge.
    pub fn contains(&self, p: Point) -> bool {
        match self {
            Shape::Polygon(poly) => polygon_contains(poly, p),
            Shape::Circle { at, r } => dist2(*at, p) <= r * r,
        }
    }

    /// The distance from `p` to the shape: 0 inside or on the edge.
    pub fn distance(&self, p: Point) -> f64 {
        if self.contains(p) {
            return 0.0;
        }
        match self {
            Shape::Polygon(poly) => edges(poly)
                .map(|(a, b)| segment_point_dist2(a, b, p))
                .fold(f64::INFINITY, f64::min)
                .sqrt(),
            Shape::Circle { at, r } => (dist2(*at, p).sqrt() - r).max(0.0),
        }
    }

    /// The bounding box `[x0, y0, x1, y1]` of the shape.
    pub fn bbox(&self) -> [f64; 4] {
        self.extreme_points().iter().fold(
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            |[x0, y0, x1, y1], p| [x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1])],
        )
    }

    /// `true` if the insides of the two shapes have a common part.
    /// Shapes that only touch at an edge do not overlap.
    pub fn overlaps(&self, other: &Shape) -> bool {
        match (self, other) {
            (Shape::Circle { at: a, r: ra }, Shape::Circle { at: b, r: rb }) => {
                dist2(*a, *b) < (ra + rb) * (ra + rb)
            }
            (Shape::Circle { at, r }, Shape::Polygon(poly))
            | (Shape::Polygon(poly), Shape::Circle { at, r }) => {
                polygon_contains_strictly(poly, *at)
                    || edges(poly).any(|(a, b)| segment_point_dist2(a, b, *at) < r * r)
            }
            (Shape::Polygon(a), Shape::Polygon(b)) => {
                a.iter().any(|&p| polygon_contains_strictly(b, p))
                    || b.iter().any(|&p| polygon_contains_strictly(a, p))
                    || edges(a).any(|(p, q)| edges(b).any(|(r, s)| segments_cross(p, q, r, s)))
                    || (centroid(a).is_some_and(|c| polygon_contains_strictly(b, c)))
            }
        }
    }
}

/// `true` if the polygon has two edges that cross, or two edges that are
/// not neighbors and that touch.
pub fn polygon_self_intersects(poly: &[Point]) -> bool {
    let n = poly.len();
    if n < 4 {
        return false;
    }
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        for j in (i + 1)..n {
            // Neighbor edges share a point. They are not a self-crossing.
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (c, d) = (poly[j], poly[(j + 1) % n]);
            if segments_touch(a, b, c, d) {
                return true;
            }
        }
    }
    false
}

/// Clip a polygon to the box `[x0, y0, x1, y1]` (Sutherland-Hodgman, as
/// `clipPolygon` of the `lineclip` library that FMG `clipPoly` uses). The
/// polygon is open: the last point does not repeat the first. The result
/// is empty when no part of the polygon is in the box.
pub fn clip_polygon(points: &[Point], bbox: [f64; 4]) -> Vec<Point> {
    let code = |p: Point| -> u8 {
        let mut c = 0;
        if p[0] < bbox[0] {
            c |= 1;
        } else if p[0] > bbox[2] {
            c |= 2;
        }
        if p[1] < bbox[1] {
            c |= 4;
        } else if p[1] > bbox[3] {
            c |= 8;
        }
        c
    };
    let cut = |a: Point, b: Point, edge: u8| -> Point {
        match edge {
            8 => [
                a[0] + (b[0] - a[0]) * (bbox[3] - a[1]) / (b[1] - a[1]),
                bbox[3],
            ],
            4 => [
                a[0] + (b[0] - a[0]) * (bbox[1] - a[1]) / (b[1] - a[1]),
                bbox[1],
            ],
            2 => [
                bbox[2],
                a[1] + (b[1] - a[1]) * (bbox[2] - a[0]) / (b[0] - a[0]),
            ],
            _ => [
                bbox[0],
                a[1] + (b[1] - a[1]) * (bbox[0] - a[0]) / (b[0] - a[0]),
            ],
        }
    };
    let mut points = points.to_vec();
    for edge in [1u8, 2, 4, 8] {
        let Some(&last) = points.last() else { break };
        let mut result = Vec::with_capacity(points.len() + 4);
        let (mut prev, mut prev_inside) = (last, code(last) & edge == 0);
        for &p in &points {
            let inside = code(p) & edge == 0;
            if inside != prev_inside {
                result.push(cut(prev, p, edge));
            }
            if inside {
                result.push(p);
            }
            prev = p;
            prev_inside = inside;
        }
        points = result;
    }
    points
}

pub fn dist(a: Point, b: Point) -> f64 {
    dist2(a, b).sqrt()
}

fn dist2(a: Point, b: Point) -> f64 {
    let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
    dx * dx + dy * dy
}

fn edges(poly: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    (0..poly.len()).map(move |i| (poly[i], poly[(i + 1) % poly.len()]))
}

fn centroid(poly: &[Point]) -> Option<Point> {
    if poly.is_empty() {
        return None;
    }
    let n = poly.len() as f64;
    let (sx, sy) = poly
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p[0], y + p[1]));
    Some([sx / n, sy / n])
}

/// The sign of the turn from `a` to `b` to `c`: 1 left, -1 right, 0 none.
fn orient(a: Point, b: Point, c: Point) -> i8 {
    let v = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if v > 0.0 {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

fn on_segment(a: Point, b: Point, p: Point) -> bool {
    orient(a, b, p) == 0
        && p[0] >= a[0].min(b[0])
        && p[0] <= a[0].max(b[0])
        && p[1] >= a[1].min(b[1])
        && p[1] <= a[1].max(b[1])
}

/// `true` if the two segments cross at one point inside both of them.
fn segments_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let (o1, o2, o3, o4) = (
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    );
    o1 * o2 < 0 && o3 * o4 < 0
}

/// `true` if the two segments have a common point.
fn segments_touch(a: Point, b: Point, c: Point, d: Point) -> bool {
    segments_cross(a, b, c, d)
        || on_segment(a, b, c)
        || on_segment(a, b, d)
        || on_segment(c, d, a)
        || on_segment(c, d, b)
}

/// The distance from `p` to the segment from `a` to `b`.
pub fn segment_distance(a: Point, b: Point, p: Point) -> f64 {
    segment_point_dist2(a, b, p).sqrt()
}

fn segment_point_dist2(a: Point, b: Point, p: Point) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return dist2(a, p);
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0);
    dist2([a[0] + t * dx, a[1] + t * dy], p)
}

fn polygon_contains(poly: &[Point], p: Point) -> bool {
    edges(poly).any(|(a, b)| on_segment(a, b, p)) || polygon_contains_strictly(poly, p)
}

/// Even-odd ray test. A point on an edge can give either result.
fn polygon_contains_strictly(poly: &[Point], p: Point) -> bool {
    if edges(poly).any(|(a, b)| on_segment(a, b, p)) {
        return false;
    }
    let mut inside = false;
    for (a, b) in edges(poly) {
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
            if p[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_polygon_clips_to_the_box() {
        let bbox = [0.0, 0.0, 10.0, 10.0];
        // Inside: no change.
        let inside = [[1.0, 1.0], [9.0, 1.0], [5.0, 9.0]];
        assert_eq!(clip_polygon(&inside, bbox), inside.to_vec());
        // Outside: nothing.
        assert!(clip_polygon(&[[20.0, 20.0], [30.0, 20.0], [25.0, 30.0]], bbox).is_empty());
        // A square over the left edge: the part left of x = 0 goes, and
        // the cut points are on the edge.
        let over = [[-5.0, 2.0], [5.0, 2.0], [5.0, 8.0], [-5.0, 8.0]];
        let c = clip_polygon(&over, bbox);
        assert_eq!(c, vec![[0.0, 2.0], [5.0, 2.0], [5.0, 8.0], [0.0, 8.0]]);
        // A diamond over a corner keeps its area inside the box.
        let corner = [[-2.0, 5.0], [5.0, -2.0], [12.0, 5.0], [5.0, 12.0]];
        let c = clip_polygon(&corner, bbox);
        assert!(
            c.iter()
                .all(|p| p[0] >= 0.0 && p[0] <= 10.0 && p[1] >= 0.0 && p[1] <= 10.0)
        );
        let area = |q: &[Point]| {
            (0..q.len())
                .map(|i| {
                    let (a, b) = (q[i], q[(i + 1) % q.len()]);
                    a[0] * b[1] - b[0] * a[1]
                })
                .sum::<f64>()
                / 2.0
        };
        // 100 minus the four corner triangles of 3 x 3 / 2.
        assert!((area(&c) - 82.0).abs() < 1e-9, "{}", area(&c));
    }

    fn square(x0: f64, y0: f64, size: f64) -> Shape {
        Shape::Polygon(vec![
            [x0, y0],
            [x0 + size, y0],
            [x0 + size, y0 + size],
            [x0, y0 + size],
        ])
    }

    #[test]
    fn polygon_contains_inside_and_edge_points() {
        let s = square(0.0, 0.0, 10.0);
        assert!(s.contains([5.0, 5.0]));
        assert!(s.contains([0.0, 5.0]));
        assert!(!s.contains([11.0, 5.0]));
    }

    #[test]
    fn circle_contains() {
        let c = Shape::Circle {
            at: [0.0, 0.0],
            r: 2.0,
        };
        assert!(c.contains([1.0, 1.0]));
        assert!(!c.contains([2.0, 2.0]));
    }

    #[test]
    fn crossing_squares_overlap() {
        assert!(square(0.0, 0.0, 10.0).overlaps(&square(5.0, 5.0, 10.0)));
    }

    #[test]
    fn square_inside_square_overlaps() {
        assert!(square(0.0, 0.0, 10.0).overlaps(&square(2.0, 2.0, 2.0)));
        assert!(square(2.0, 2.0, 2.0).overlaps(&square(0.0, 0.0, 10.0)));
    }

    #[test]
    fn touching_squares_do_not_overlap() {
        assert!(!square(0.0, 0.0, 10.0).overlaps(&square(10.0, 0.0, 10.0)));
    }

    #[test]
    fn same_squares_overlap() {
        assert!(square(0.0, 0.0, 10.0).overlaps(&square(0.0, 0.0, 10.0)));
    }

    #[test]
    fn circle_and_square() {
        let c = Shape::Circle {
            at: [12.0, 5.0],
            r: 3.0,
        };
        assert!(c.overlaps(&square(0.0, 0.0, 10.0)));
        let far = Shape::Circle {
            at: [15.0, 5.0],
            r: 3.0,
        };
        assert!(!far.overlaps(&square(0.0, 0.0, 10.0)));
    }

    #[test]
    fn distance_is_zero_inside_and_exact_outside() {
        let s = square(0.0, 0.0, 10.0);
        assert_eq!(s.distance([5.0, 5.0]), 0.0);
        assert_eq!(s.distance([13.0, 5.0]), 3.0);
        assert_eq!(s.distance([13.0, 14.0]), 5.0);
        let c = Shape::Circle {
            at: [0.0, 0.0],
            r: 2.0,
        };
        assert_eq!(c.distance([5.0, 0.0]), 3.0);
        assert_eq!(c.bbox(), [-2.0, -2.0, 2.0, 2.0]);
    }

    #[test]
    fn bow_tie_crosses_itself() {
        let bow = [[0.0, 0.0], [10.0, 10.0], [10.0, 0.0], [0.0, 10.0]];
        assert!(polygon_self_intersects(&bow));
        let ok = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        assert!(!polygon_self_intersects(&ok));
    }
}
