//! Polygons of points by value, for the buildings of the town generator.
//!
//! A port of `com.watabou.geom.Polygon`, `GeomUtils`, and `Cutter` of
//! TownGeneratorOS by Watabou (GPL-3.0,
//! <https://github.com/watabou/TownGeneratorOS>). The original compares
//! points by identity. Here two points are the same if their coordinates
//! are the same, which agrees for the polygons of the buildings.

/// A point.
pub type P = [f64; 2];

/// A polygon: its vertices in order.
pub type Poly = Vec<P>;

const DELTA: f64 = 0.000_001;

pub fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}

pub fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn scale(a: P, f: f64) -> P {
    [a[0] * f, a[1] * f]
}

pub fn len(a: P) -> f64 {
    (a[0] * a[0] + a[1] * a[1]).sqrt()
}

pub fn dist(a: P, b: P) -> f64 {
    len(sub(a, b))
}

pub fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// The point turned by 90 degrees: `(-y, x)`.
pub fn rotate90(a: P) -> P {
    [-a[1], a[0]]
}

/// The point with the length `length` in the same direction.
pub fn norm(a: P, length: f64) -> P {
    let l = len(a);
    if l == 0.0 { a } else { scale(a, length / l) }
}

pub fn cross(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    x1 * y2 - y1 * x2
}

/// The point at `ratio` of the way from `p1` to `p2`.
pub fn interpolate(p1: P, p2: P, ratio: f64) -> P {
    let d = sub(p2, p1);
    [p1[0] + d[0] * ratio, p1[1] + d[1] * ratio]
}

/// The parameters `(t1, t2)` of the crossing of two lines, each given by a
/// point and a direction, or `None` for parallel lines.
#[allow(clippy::too_many_arguments)]
pub fn intersect_lines(
    x1: f64,
    y1: f64,
    dx1: f64,
    dy1: f64,
    x2: f64,
    y2: f64,
    dx2: f64,
    dy2: f64,
) -> Option<(f64, f64)> {
    let d = dx1 * dy2 - dy1 * dx2;
    if d == 0.0 {
        return None;
    }
    let t2 = (dy1 * (x2 - x1) - dx1 * (y2 - y1)) / d;
    let t1 = if dx1 != 0.0 {
        (x2 - x1 + dx2 * t2) / dx1
    } else {
        (y2 - y1 + dy2 * t2) / dy1
    };
    Some((t1, t2))
}

/// The signed distance from the point `(x0, y0)` to a line.
pub fn distance2line(x1: f64, y1: f64, dx1: f64, dy1: f64, x0: f64, y0: f64) -> f64 {
    (dx1 * y0 - dy1 * x0 + (y1 + dy1) * x1 - (x1 + dx1) * y1) / (dx1 * dx1 + dy1 * dy1).sqrt()
}

/// The signed area.
pub fn square(p: &[P]) -> f64 {
    if p.is_empty() {
        return 0.0;
    }
    let mut v1 = p[p.len() - 1];
    let mut v2 = p[0];
    let mut s = v1[0] * v2[1] - v2[0] * v1[1];
    for &v in &p[1..] {
        v1 = v2;
        v2 = v;
        s += v1[0] * v2[1] - v2[0] * v1[1];
    }
    s * 0.5
}

pub fn perimeter(p: &[P]) -> f64 {
    let n = p.len();
    (0..n).map(|i| dist(p[i], p[(i + 1) % n])).sum()
}

/// 1 for a circle, 0.79 for a square, 0.6 for a triangle.
pub fn compactness(p: &[P]) -> f64 {
    let per = perimeter(p);
    4.0 * std::f64::consts::PI * square(p) / (per * per)
}

/// The mean of the vertices.
pub fn center(p: &[P]) -> P {
    let mut c = [0.0, 0.0];
    for v in p {
        c = add(c, *v);
    }
    scale(c, 1.0 / p.len() as f64)
}

pub fn centroid(p: &[P]) -> P {
    let (mut x, mut y, mut a) = (0.0, 0.0, 0.0);
    let n = p.len();
    for i in 0..n {
        let (v0, v1) = (p[i], p[(i + 1) % n]);
        let f = cross(v0[0], v0[1], v1[0], v1[1]);
        a += f;
        x += (v0[0] + v1[0]) * f;
        y += (v0[1] + v1[1]) * f;
    }
    let s6 = 1.0 / (3.0 * a);
    [s6 * x, s6 * y]
}

/// The index of the first vertex equal to `v`.
pub fn index_of(p: &[P], v: P) -> Option<usize> {
    p.iter().position(|q| *q == v)
}

pub fn next(p: &[P], i: usize) -> P {
    p[(i + 1) % p.len()]
}

/// The vector from vertex `i` to the next vertex.
pub fn vector(p: &[P], i: usize) -> P {
    sub(next(p, i), p[i])
}

pub fn is_convex(p: &[P]) -> bool {
    let n = p.len();
    (0..n).all(|i| {
        // The original finds the neighbours of a vertex by its first index.
        let k = index_of(p, p[i]).unwrap_or(i);
        let v0 = p[(k + n - 1) % n];
        let v1 = p[k];
        let v2 = p[(k + 1) % n];
        cross(v1[0] - v0[0], v1[1] - v0[1], v2[0] - v1[0], v2[1] - v1[1]) > 0.0
    })
}

/// Each vertex moved to the weighted mean of it and its two neighbours.
pub fn smooth_vertex_eq(p: &[P], f: f64) -> Poly {
    let n = p.len();
    let mut v1 = p[n - 1];
    let mut v2 = p[0];
    (0..n)
        .map(|i| {
            let v0 = v1;
            v1 = v2;
            v2 = p[(i + 1) % n];
            [
                (v0[0] + v1[0] * f + v2[0]) / (2.0 + f),
                (v0[1] + v1[1] * f + v2[1]) / (2.0 + f),
            ]
        })
        .collect()
}

/// The index of the edge from `a` to `b`, if the polygon has it.
pub fn find_edge(p: &[P], a: P, b: P) -> Option<usize> {
    let i = index_of(p, a)?;
    (next(p, i) == b).then_some(i)
}

/// A rectangle around the origin.
pub fn rect(w: f64, h: f64) -> Poly {
    vec![
        [-w / 2.0, -h / 2.0],
        [w / 2.0, -h / 2.0],
        [w / 2.0, h / 2.0],
        [-w / 2.0, h / 2.0],
    ]
}

/// A regular polygon of `n` vertices around the origin.
pub fn regular(n: usize, r: f64) -> Poly {
    (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::PI * 2.0;
            [r * libm::cos(a), r * libm::sin(a)]
        })
        .collect()
}

pub fn circle(r: f64) -> Poly {
    regular(16, r)
}

pub fn rotate(p: &mut [P], a: f64) {
    let (cos_a, sin_a) = (libm::cos(a), libm::sin(a));
    for v in p.iter_mut() {
        *v = [v[0] * cos_a - v[1] * sin_a, v[1] * cos_a + v[0] * sin_a];
    }
}

pub fn offset(p: &mut [P], d: P) {
    for v in p.iter_mut() {
        *v = add(*v, d);
    }
}

/// The weight of each vertex for a point: 1 over the distance, scaled so
/// that the weights add up to 1.
pub fn interpolate_weights(p: &[P], q: P) -> Vec<f64> {
    let dd: Vec<f64> = p.iter().map(|v| 1.0 / dist(*v, q)).collect();
    let sum: f64 = dd.iter().sum();
    dd.into_iter().map(|d| d / sum).collect()
}

/// Cut a polygon along the line through `p1` and `p2`. The result is the
/// two halves, the half on the left of the line first, or the polygon if
/// the line does not cross it twice. With a `gap`, each half loses a strip
/// of half the gap along the cut.
pub fn cut(p: &[P], p1: P, p2: P, gap: f64) -> Vec<Poly> {
    let (x1, y1) = (p1[0], p1[1]);
    let (dx1, dy1) = (p2[0] - x1, p2[1] - y1);
    let n = p.len();
    let (mut edge1, mut ratio1, mut edge2, mut ratio2) = (0, 0.0, 0, 0.0);
    let mut count = 0;
    for i in 0..n {
        let (v0, v1) = (p[i], p[(i + 1) % n]);
        let (x2, y2) = (v0[0], v0[1]);
        let (dx2, dy2) = (v1[0] - x2, v1[1] - y2);
        if let Some((t1, t2)) = intersect_lines(x1, y1, dx1, dy1, x2, y2, dx2, dy2)
            && (0.0..=1.0).contains(&t2)
        {
            match count {
                0 => {
                    edge1 = i;
                    ratio1 = t1;
                }
                1 => {
                    edge2 = i;
                    ratio2 = t1;
                }
                _ => {}
            }
            count += 1;
        }
    }
    if count != 2 {
        return vec![p.to_vec()];
    }
    let point1 = add(p1, scale(sub(p2, p1), ratio1));
    let point2 = add(p1, scale(sub(p2, p1), ratio2));
    let mut half1: Poly = vec![point1];
    half1.extend_from_slice(&p[edge1 + 1..edge2 + 1]);
    half1.push(point2);
    let mut half2: Poly = vec![point2];
    half2.extend_from_slice(&p[edge2 + 1..]);
    half2.extend_from_slice(&p[..edge1 + 1]);
    half2.push(point1);
    if gap > 0.0 {
        half1 = peel(&half1, half1.len() - 1, gap / 2.0);
        half2 = peel(&half2, half2.len() - 1, gap / 2.0);
    }
    let v = vector(p, edge1);
    if cross(dx1, dy1, v[0], v[1]) > 0.0 {
        vec![half1, half2]
    } else {
        vec![half2, half1]
    }
}

/// Cut a strip of width `d` along the edge that starts at vertex `i`.
pub fn peel(p: &[P], i: usize, d: f64) -> Poly {
    let v1 = p[i];
    let v2 = next(p, i);
    let n = norm(rotate90(sub(v2, v1)), d);
    cut(p, add(v1, n), add(v2, n), 0.0).swap_remove(0)
}

/// Move each edge in by its distance in `d`. The result is convex, and an
/// edge cannot move out.
pub fn shrink(p: &[P], d: &[f64]) -> Poly {
    let mut q = p.to_vec();
    let n = p.len();
    for i in 0..n {
        let dd = d[i];
        if dd > 0.0 {
            let (v1, v2) = (p[i], p[(i + 1) % n]);
            let nv = norm(rotate90(sub(v2, v1)), dd);
            q = cut(&q, add(v1, nv), add(v2, nv), 0.0).swap_remove(0);
        }
    }
    q
}

pub fn shrink_eq(p: &[P], d: f64) -> Poly {
    shrink(p, &vec![d; p.len()])
}

/// Move each edge by its distance in `d`, for a polygon that is not
/// convex. Where the moved edges cross, the largest part stays.
pub fn buffer(p: &[P], d: &[f64]) -> Poly {
    let n0 = p.len();
    let mut q: Poly = Vec::new();
    for i in 0..n0 {
        let (v0, v1) = (p[i], p[(i + 1) % n0]);
        let dd = d[i];
        if dd == 0.0 {
            q.push(v0);
            q.push(v1);
        } else {
            let nv = norm(rotate90(sub(v1, v0)), dd);
            q.push(add(v0, nv));
            q.push(add(v1, nv));
        }
    }
    // Add each crossing of two edges, once for each edge.
    let mut last_edge = 0;
    loop {
        let mut was_cut = false;
        let n = q.len();
        let mut i = last_edge;
        while i + 2 <= n && i < n.saturating_sub(2) {
            last_edge = i;
            let (p11, p12) = (q[i], q[i + 1]);
            let (x1, y1) = (p11[0], p11[1]);
            let (dx1, dy1) = (p12[0] - x1, p12[1] - y1);
            let end = if i > 0 { n } else { n - 1 };
            let mut j = i + 2;
            while j < end {
                let p21 = q[j];
                let p22 = if j < n - 1 { q[j + 1] } else { q[0] };
                let (x2, y2) = (p21[0], p21[1]);
                let (dx2, dy2) = (p22[0] - x2, p22[1] - y2);
                if let Some((t1, t2)) = intersect_lines(x1, y1, dx1, dy1, x2, y2, dx2, dy2)
                    && t1 > DELTA
                    && t1 < 1.0 - DELTA
                    && t2 > DELTA
                    && t2 < 1.0 - DELTA
                {
                    let pn = [x1 + dx1 * t1, y1 + dy1 * t1];
                    q.insert(j + 1, pn);
                    q.insert(i + 1, pn);
                    was_cut = true;
                    break;
                }
                j += 1;
            }
            if was_cut {
                break;
            }
            i += 1;
        }
        if !was_cut || q.len() > 10_000 {
            break;
        }
    }
    // Follow each part of the polygon, and keep the largest.
    let mut regular: Vec<usize> = (0..q.len()).collect();
    let mut best: Option<Poly> = None;
    let mut best_sq = f64::NEG_INFINITY;
    while let Some(&start) = regular.first() {
        let mut indices = Vec::new();
        let mut i = start;
        let mut steps = 0;
        loop {
            indices.push(i);
            regular.retain(|&k| k != i);
            let next = (i + 1) % q.len();
            let v = q[next];
            let mut next1 = index_of(&q, v);
            if next1 == Some(next) {
                next1 = q.iter().rposition(|w| *w == v);
            }
            i = next1.unwrap_or(next);
            steps += 1;
            if i == start || steps > q.len() {
                break;
            }
        }
        let part: Poly = indices.iter().map(|&k| q[k]).collect();
        let s = square(&part);
        if s > best_sq {
            best = Some(part);
            best_sq = s;
        }
    }
    best.unwrap_or_default()
}

/// Two parts of a polygon, cut by a line through a point at `ratio` of the
/// edge from vertex `i`, at `angle` to the normal of the edge.
pub fn bisect(p: &[P], i: usize, ratio: f64, angle: f64, gap: f64) -> Vec<Poly> {
    let vertex = p[i];
    let nx = next(p, i);
    let p1 = interpolate(vertex, nx, ratio);
    let d = sub(nx, vertex);
    let (cos_b, sin_b) = (libm::cos(angle), libm::sin(angle));
    let vx = d[0] * cos_b - d[1] * sin_b;
    let vy = d[1] * cos_b + d[0] * sin_b;
    let p2 = [p1[0] - vy, p1[1] + vx];
    cut(p, p1, p2, gap)
}

/// Triangles from the centroid to each edge.
pub fn radial(p: &[P], gap: f64) -> Vec<Poly> {
    let c = centroid(p);
    let n = p.len();
    (0..n)
        .map(|i| {
            let sector = vec![c, p[i], p[(i + 1) % n]];
            if gap > 0.0 {
                shrink(&sector, &[gap / 2.0, 0.0, gap / 2.0])
            } else {
                sector
            }
        })
        .collect()
}

/// Triangles from the vertex nearest to the centroid to each other edge.
pub fn semi_radial(p: &[P], gap: f64) -> Vec<Poly> {
    let c = centroid(p);
    let center = min_by(p, |v| dist(*v, c));
    let gap = gap / 2.0;
    let n = p.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (v0, v1) = (p[i], p[(i + 1) % n]);
        if v0 != center && v1 != center {
            let mut sector = vec![center, v0, v1];
            if gap > 0.0 {
                let d = [
                    if find_edge(p, center, v0).is_none() {
                        gap
                    } else {
                        0.0
                    },
                    0.0,
                    if find_edge(p, v1, center).is_none() {
                        gap
                    } else {
                        0.0
                    },
                ];
                sector = shrink(&sector, &d);
            }
            out.push(sector);
        }
    }
    out
}

/// The strips along each edge of a polygon, of the width `thickness`.
pub fn ring(p: &[P], thickness: f64) -> Vec<Poly> {
    let n = p.len();
    let mut slices: Vec<(P, P, f64)> = (0..n)
        .map(|i| {
            let (v1, v2) = (p[i], p[(i + 1) % n]);
            let v = sub(v2, v1);
            let nv = norm(rotate90(v), thickness);
            (add(v1, nv), add(v2, nv), len(v))
        })
        .collect();
    // Short sides first.
    slices.sort_by(|a, b| a.2.total_cmp(&b.2));
    let mut peel_out = Vec::new();
    let mut q = p.to_vec();
    for (a, b, _) in slices {
        let mut halves = cut(&q, a, b, 0.0);
        if halves.len() == 2 {
            peel_out.push(halves.pop().expect("two halves"));
        }
        q = halves.swap_remove(0);
    }
    peel_out
}

/// The first item with the least measure.
pub fn min_by<T: Copy>(items: &[T], f: impl Fn(&T) -> f64) -> T {
    let mut best = items[0];
    let mut m = f(&best);
    for it in &items[1..] {
        let v = f(it);
        if v < m {
            best = *it;
            m = v;
        }
    }
    best
}

/// The first item with the largest measure.
pub fn max_by<T: Copy>(items: &[T], f: impl Fn(&T) -> f64) -> T {
    let mut best = items[0];
    let mut m = f(&best);
    for it in &items[1..] {
        let v = f(it);
        if v > m {
            best = *it;
            m = v;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit() -> Poly {
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]
    }

    #[test]
    fn the_measures_of_a_square() {
        let p = unit();
        assert_eq!(square(&p), 100.0);
        assert_eq!(perimeter(&p), 40.0);
        assert!((compactness(&p) - 0.785).abs() < 0.001);
        assert_eq!(centroid(&p), [5.0, 5.0]);
        assert!(is_convex(&p));
    }

    #[test]
    fn a_cut_makes_two_halves_that_fill_the_polygon() {
        let p = unit();
        let halves = cut(&p, [4.0, -1.0], [4.0, 11.0], 0.0);
        assert_eq!(halves.len(), 2);
        let total: f64 = halves.iter().map(|h| square(h).abs()).sum();
        assert!((total - 100.0).abs() < 1e-9);
        // A line that misses the polygon leaves it whole.
        assert_eq!(cut(&p, [20.0, 0.0], [20.0, 1.0], 0.0).len(), 1);
        // A gap takes a strip from each half.
        let gap = cut(&p, [4.0, -1.0], [4.0, 11.0], 1.0);
        let total: f64 = gap.iter().map(|h| square(h).abs()).sum();
        assert!((total - 90.0).abs() < 1e-6, "{total}");
    }

    #[test]
    fn shrink_and_buffer_move_the_edges_in() {
        let p = unit();
        let s = shrink_eq(&p, 1.0);
        assert!((square(&s) - 64.0).abs() < 1e-9);
        let b = buffer(&p, &[1.0; 4]);
        assert!((square(&b).abs() - 64.0).abs() < 1e-6, "{}", square(&b));
    }

    #[test]
    fn a_ring_and_the_radial_sectors_cover_the_edges() {
        let p = unit();
        assert_eq!(ring(&p, 2.0).len(), 4);
        assert_eq!(radial(&p, 0.0).len(), 4);
        assert_eq!(semi_radial(&p, 0.0).len(), 2);
    }
}
