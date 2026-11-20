//! The Voronoi diagram of the town generator, from a Delaunay
//! triangulation that adds one point at a time.
//!
//! A port of `com.watabou.geom.Voronoi` of TownGeneratorOS by Watabou
//! (GPL-3.0, <https://github.com/watabou/TownGeneratorOS>). The points are
//! indexes in a list, so that the triangles and the regions can compare
//! them as the original compares its point objects.

use super::poly::{P, dist};

/// A triangle of the triangulation, counter-clockwise, with its
/// circumcircle.
#[derive(Debug, Clone)]
pub struct Triangle {
    pub p: [usize; 3],
    /// The centre of the circumcircle: a vertex of the Voronoi regions.
    pub c: P,
    pub r: f64,
}

impl Triangle {
    fn new(pts: &[P], p1: usize, p2: usize, p3: usize) -> Triangle {
        let (a, b, c) = (pts[p1], pts[p2], pts[p3]);
        let s = (b[0] - a[0]) * (b[1] + a[1])
            + (c[0] - b[0]) * (c[1] + b[1])
            + (a[0] - c[0]) * (a[1] + c[1]);
        let (q2, q3) = if s > 0.0 { (p2, p3) } else { (p3, p2) };
        // The original computes the centre from the points in the order of
        // its arguments, before it orders them.
        let x1 = (a[0] + b[0]) / 2.0;
        let y1 = (a[1] + b[1]) / 2.0;
        let x2 = (b[0] + c[0]) / 2.0;
        let y2 = (b[1] + c[1]) / 2.0;
        let dx1 = a[1] - b[1];
        let dy1 = b[0] - a[0];
        let dx2 = b[1] - c[1];
        let dy2 = c[0] - b[0];
        let tg1 = dy1 / dx1;
        let t2 = ((y1 - y2) - (x1 - x2) * tg1) / (dy2 - dx2 * tg1);
        let center = [x2 + dx2 * t2, y2 + dy2 * t2];
        Triangle {
            p: [p1, q2, q3],
            c: center,
            r: dist(center, a),
        }
    }

    fn has_edge(&self, a: usize, b: usize) -> bool {
        let [p1, p2, p3] = self.p;
        (p1 == a && p2 == b) || (p2 == a && p3 == b) || (p3 == a && p1 == b)
    }

    fn has_vertex(&self, v: usize) -> bool {
        self.p.contains(&v)
    }
}

/// The region of a seed point: the triangles around it, in order of angle.
/// The circumcentres of the triangles are the vertices of the region.
#[derive(Debug, Clone)]
pub struct Region {
    pub seed: usize,
    /// Indexes of triangles.
    pub vertices: Vec<usize>,
}

/// A Voronoi diagram.
#[derive(Debug, Clone)]
pub struct Voronoi {
    /// All points, the frame first. The list `points` refers to them.
    pub pts: Vec<P>,
    /// The points in the order of the original, the frame first.
    pub points: Vec<usize>,
    pub frame: [usize; 4],
    pub triangles: Vec<Triangle>,
}

/// A stable merge sort with a comparator that gives -1, 0, or 1, as the
/// sort of the original. It accepts a comparator that is not a total order.
pub fn sort_with<T: Clone>(v: &mut Vec<T>, cmp: &dyn Fn(&T, &T) -> i32) {
    if v.len() < 2 {
        return;
    }
    let mid = v.len() / 2;
    let mut right = v.split_off(mid);
    sort_with(v, cmp);
    sort_with(&mut right, cmp);
    let left = std::mem::take(v);
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if cmp(&right[j], &left[i]) < 0 {
            v.push(right[j].clone());
            j += 1;
        } else {
            v.push(left[i].clone());
            i += 1;
        }
    }
    v.extend_from_slice(&left[i..]);
    v.extend_from_slice(&right[j..]);
}

impl Voronoi {
    fn new(minx: f64, miny: f64, maxx: f64, maxy: f64) -> Voronoi {
        let pts = vec![[minx, miny], [minx, maxy], [maxx, miny], [maxx, maxy]];
        let triangles = vec![Triangle::new(&pts, 0, 1, 2), Triangle::new(&pts, 1, 2, 3)];
        Voronoi {
            pts,
            points: vec![0, 1, 2, 3],
            frame: [0, 1, 2, 3],
            triangles,
        }
    }

    /// Add a point and update the triangles.
    pub fn add_point(&mut self, p: P) {
        let to_split: Vec<usize> = (0..self.triangles.len())
            .filter(|&t| dist(p, self.triangles[t].c) < self.triangles[t].r)
            .collect();
        if to_split.is_empty() {
            return;
        }
        let k = self.pts.len();
        self.pts.push(p);
        self.points.push(k);
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for &t1 in &to_split {
            let tr1 = &self.triangles[t1];
            let [p1, p2, p3] = tr1.p;
            let (mut e1, mut e2, mut e3) = (true, true, true);
            for &t2 in &to_split {
                if t2 == t1 {
                    continue;
                }
                let tr2 = &self.triangles[t2];
                // A common edge goes in opposite directions.
                if e1 && tr2.has_edge(p2, p1) {
                    e1 = false;
                }
                if e2 && tr2.has_edge(p3, p2) {
                    e2 = false;
                }
                if e3 && tr2.has_edge(p1, p3) {
                    e3 = false;
                }
                if !(e1 || e2 || e3) {
                    break;
                }
            }
            if e1 {
                a.push(p1);
                b.push(p2);
            }
            if e2 {
                a.push(p2);
                b.push(p3);
            }
            if e3 {
                a.push(p3);
                b.push(p1);
            }
        }
        let mut fresh = Vec::new();
        let mut index = 0;
        for _ in 0..=a.len() {
            fresh.push(Triangle::new(&self.pts, k, a[index], b[index]));
            match a.iter().position(|&x| x == b[index]) {
                Some(0) | None => break,
                Some(i) => index = i,
            }
        }
        let mut kept: Vec<Triangle> = (0..self.triangles.len())
            .filter(|t| !to_split.contains(t))
            .map(|t| self.triangles[t].clone())
            .collect();
        kept.extend(fresh);
        self.triangles = kept;
    }

    fn region(&self, p: usize) -> Region {
        let mut vertices: Vec<usize> = (0..self.triangles.len())
            .filter(|&t| self.triangles[t].has_vertex(p))
            .collect();
        let seed = self.pts[p];
        let tris = &self.triangles;
        sort_with(&mut vertices, &|&t1, &t2| {
            let (c1, c2) = (tris[t1].c, tris[t2].c);
            let (x1, y1) = (c1[0] - seed[0], c1[1] - seed[1]);
            let (x2, y2) = (c2[0] - seed[0], c2[1] - seed[1]);
            if x1 >= 0.0 && x2 < 0.0 {
                return 1;
            }
            if x2 >= 0.0 && x1 < 0.0 {
                return -1;
            }
            if x1 == 0.0 && x2 == 0.0 {
                return if y2 > y1 { 1 } else { -1 };
            }
            let s = x2 * y1 - x1 * y2;
            if s == 0.0 {
                0
            } else if s < 0.0 {
                -1
            } else {
                1
            }
        });
        Region { seed: p, vertices }
    }

    fn is_real(&self, t: usize) -> bool {
        !self.triangles[t].p.iter().any(|v| self.frame.contains(v))
    }

    /// The centre of a region: the mean of its vertices.
    pub fn region_center(&self, r: &Region) -> P {
        let mut c = [0.0, 0.0];
        for &t in &r.vertices {
            c[0] += self.triangles[t].c[0];
            c[1] += self.triangles[t].c[1];
        }
        let n = r.vertices.len() as f64;
        [c[0] / n, c[1] / n]
    }

    /// The regions with no frame point in their triangles, in the order of
    /// `points`.
    pub fn partitioning(&self) -> Vec<Region> {
        self.points
            .iter()
            .map(|&p| self.region(p))
            .filter(|r| r.vertices.iter().all(|&t| self.is_real(t)))
            .collect()
    }

    /// Move the seeds of the regions in `to_relax` (indexes of points) to
    /// the centres of their regions, and build the diagram again.
    pub fn relax(&self, to_relax: &[usize]) -> Voronoi {
        let regions = self.partitioning();
        let mut points: Vec<P> = self
            .points
            .iter()
            .filter(|p| !self.frame.contains(p))
            .map(|&p| self.pts[p])
            .collect();
        let mut keys: Vec<usize> = self
            .points
            .iter()
            .copied()
            .filter(|p| !self.frame.contains(p))
            .collect();
        for r in &regions {
            if to_relax.contains(&r.seed)
                && let Some(i) = keys.iter().position(|&k| k == r.seed)
            {
                keys.remove(i);
                points.remove(i);
                points.push(self.region_center(r));
                keys.push(usize::MAX);
            }
        }
        Voronoi::build(&points)
    }

    /// The diagram of a list of points.
    pub fn build(vertices: &[P]) -> Voronoi {
        let (mut minx, mut miny, mut maxx, mut maxy) = (1e10, 1e10, -1e9, -1e9);
        for v in vertices {
            minx = f64::min(minx, v[0]);
            miny = f64::min(miny, v[1]);
            maxx = f64::max(maxx, v[0]);
            maxy = f64::max(maxy, v[1]);
        }
        let dx = (maxx - minx) * 0.5;
        let dy = (maxy - miny) * 0.5;
        let mut v = Voronoi::new(
            minx - dx / 2.0,
            miny - dy / 2.0,
            maxx + dx / 2.0,
            maxy + dy / 2.0,
        );
        for p in vertices {
            v.add_point(*p);
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_of_points_gives_closed_regions_inside() {
        let pts: Vec<P> = (0..49)
            .map(|i| {
                [
                    f64::from(i % 7) * 10.0 + f64::from(i / 7) * 0.37,
                    f64::from(i / 7) * 10.0 + f64::from(i % 7) * 0.21,
                ]
            })
            .collect();
        let v = Voronoi::build(&pts);
        assert_eq!(v.points.len(), 4 + 49);
        let regions = v.partitioning();
        assert!(regions.len() >= 9, "{}", regions.len());
        for r in &regions {
            assert!(r.vertices.len() >= 3);
            // The seed is inside the hull of its region.
            let c = v.region_center(r);
            assert!(dist(c, v.pts[r.seed]) < 10.0);
        }
    }

    #[test]
    fn the_sort_is_stable() {
        let mut v = vec![(1, 'a'), (0, 'b'), (1, 'c'), (0, 'd')];
        sort_with(&mut v, &|x, y| x.0 - y.0);
        assert_eq!(v, vec![(0, 'b'), (0, 'd'), (1, 'a'), (1, 'c')]);
    }
}
