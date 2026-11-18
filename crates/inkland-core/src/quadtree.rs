//! A quadtree as `d3-quadtree` builds it, and the search of FMG
//! (`utils/quadtree.ts`, `findAllInQuadtree`).
//!
//! The order of the search results follows the shape of the tree. FMG takes
//! the first fit in that order, so the tree must be the same as in d3. The
//! tree keeps item indices; the caller keeps the items.

/// A node: an internal node with four children, or a leaf with an item and
/// the next leaf at the same point.
#[derive(Debug, Clone)]
enum Node {
    Internal([Option<usize>; 4]),
    Leaf { item: usize, next: Option<usize> },
}

/// A quadtree of points, as `d3.quadtree`.
#[derive(Debug, Clone)]
pub struct Quadtree {
    nodes: Vec<Node>,
    root: Option<usize>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    /// The point of each item, by item index.
    points: Vec<[f64; 2]>,
}

impl Default for Quadtree {
    fn default() -> Self {
        Quadtree {
            nodes: Vec::new(),
            root: None,
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            points: Vec::new(),
        }
    }
}

fn quadrant(right: bool, bottom: bool) -> usize {
    (usize::from(bottom) << 1) | usize::from(right)
}

impl Quadtree {
    /// An empty tree.
    pub fn new() -> Quadtree {
        Quadtree::default()
    }

    /// A tree of all points (`d3.quadtree(data)`, that is `addAll`). The item
    /// index of each point is its index in `points`.
    pub fn from_points(points: &[[f64; 2]]) -> Quadtree {
        let mut t = Quadtree::new();
        t.add_all(points);
        t
    }

    fn set_point(&mut self, item: usize, p: [f64; 2]) {
        if self.points.len() <= item {
            self.points.resize(item + 1, [f64::NAN, f64::NAN]);
        }
        self.points[item] = p;
    }

    fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    fn child(&self, node: usize, i: usize) -> Option<usize> {
        match &self.nodes[node] {
            Node::Internal(c) => c[i],
            Node::Leaf { .. } => None,
        }
    }

    fn set_child(&mut self, node: usize, i: usize, value: Option<usize>) {
        if let Node::Internal(c) = &mut self.nodes[node] {
            c[i] = value;
        }
    }

    fn is_internal(&self, node: usize) -> bool {
        matches!(self.nodes[node], Node::Internal(_))
    }

    /// Grow the extent to hold a point (`cover`).
    fn cover(&mut self, x: f64, y: f64) {
        if x.is_nan() || y.is_nan() {
            return;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (self.x0, self.y0, self.x1, self.y1);
        if x0.is_nan() {
            x0 = x.floor();
            x1 = x0 + 1.0;
            y0 = y.floor();
            y1 = y0 + 1.0;
        } else {
            let mut z = if x1 - x0 != 0.0 { x1 - x0 } else { 1.0 };
            let mut node = self.root;
            while x0 > x || x >= x1 || y0 > y || y >= y1 {
                let i = (usize::from(y < y0) << 1) | usize::from(x < x0);
                let mut children = [None; 4];
                children[i] = node;
                node = Some(self.push(Node::Internal(children)));
                z *= 2.0;
                match i {
                    0 => {
                        x1 = x0 + z;
                        y1 = y0 + z;
                    }
                    1 => {
                        x0 = x1 - z;
                        y1 = y0 + z;
                    }
                    2 => {
                        x1 = x0 + z;
                        y0 = y1 - z;
                    }
                    _ => {
                        x0 = x1 - z;
                        y0 = y1 - z;
                    }
                }
            }
            if self.root.is_some_and(|r| self.is_internal(r)) {
                self.root = node;
            }
        }
        self.x0 = x0;
        self.y0 = y0;
        self.x1 = x1;
        self.y1 = y1;
    }

    /// Add one item at a point (`add`).
    pub fn add(&mut self, item: usize, p: [f64; 2]) {
        self.cover(p[0], p[1]);
        self.insert(item, p);
    }

    /// Add all points, the item index of each is its index (`addAll`).
    pub fn add_all(&mut self, points: &[[f64; 2]]) {
        let (mut x0, mut y0, mut x1, mut y1) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for &[x, y] in points {
            if x.is_nan() || y.is_nan() {
                continue;
            }
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        if x0 > x1 || y0 > y1 {
            return;
        }
        self.cover(x0, y0);
        self.cover(x1, y1);
        for (item, &p) in points.iter().enumerate() {
            self.insert(item, p);
        }
    }

    fn insert(&mut self, item: usize, p: [f64; 2]) {
        let [x, y] = p;
        if x.is_nan() || y.is_nan() {
            return;
        }
        self.set_point(item, p);
        let leaf = self.push(Node::Leaf { item, next: None });
        let Some(mut node) = self.root else {
            self.root = Some(leaf);
            return;
        };
        let (mut x0, mut y0, mut x1, mut y1) = (self.x0, self.y0, self.x1, self.y1);
        let mut parent: Option<usize> = None;
        let mut i = 0;
        while self.is_internal(node) {
            let xm = (x0 + x1) / 2.0;
            let right = x >= xm;
            if right {
                x0 = xm;
            } else {
                x1 = xm;
            }
            let ym = (y0 + y1) / 2.0;
            let bottom = y >= ym;
            if bottom {
                y0 = ym;
            } else {
                y1 = ym;
            }
            i = quadrant(right, bottom);
            parent = Some(node);
            match self.child(node, i) {
                Some(n) => node = n,
                None => {
                    self.set_child(node, i, Some(leaf));
                    return;
                }
            }
        }
        let other = match self.nodes[node] {
            Node::Leaf { item, .. } => item,
            Node::Internal(_) => unreachable!(),
        };
        let [xp, yp] = self.points[other];
        if x == xp && y == yp {
            if let Node::Leaf { next, .. } = &mut self.nodes[leaf] {
                *next = Some(node);
            }
            match parent {
                Some(par) => self.set_child(par, i, Some(leaf)),
                None => self.root = Some(leaf),
            }
            return;
        }
        loop {
            let fresh = self.push(Node::Internal([None; 4]));
            match parent {
                Some(par) => self.set_child(par, i, Some(fresh)),
                None => self.root = Some(fresh),
            }
            parent = Some(fresh);
            let xm = (x0 + x1) / 2.0;
            let right = x >= xm;
            if right {
                x0 = xm;
            } else {
                x1 = xm;
            }
            let ym = (y0 + y1) / 2.0;
            let bottom = y >= ym;
            if bottom {
                y0 = ym;
            } else {
                y1 = ym;
            }
            i = quadrant(right, bottom);
            let j = quadrant(xp >= xm, yp >= ym);
            if i != j {
                self.set_child(fresh, j, Some(node));
                self.set_child(fresh, i, Some(leaf));
                return;
            }
        }
    }

    /// Remove one item (`remove`).
    pub fn remove(&mut self, item: usize) {
        let Some(&[x, y]) = self.points.get(item) else {
            return;
        };
        if x.is_nan() || y.is_nan() {
            return;
        }
        let Some(mut node) = self.root else {
            return;
        };
        let (mut x0, mut y0, mut x1, mut y1) = (self.x0, self.y0, self.x1, self.y1);
        let mut parent: Option<usize> = None;
        let mut retainer: Option<(usize, usize)> = None;
        let mut i = 0;
        if self.is_internal(node) {
            loop {
                let xm = (x0 + x1) / 2.0;
                let right = x >= xm;
                if right {
                    x0 = xm;
                } else {
                    x1 = xm;
                }
                let ym = (y0 + y1) / 2.0;
                let bottom = y >= ym;
                if bottom {
                    y0 = ym;
                } else {
                    y1 = ym;
                }
                i = quadrant(right, bottom);
                parent = Some(node);
                match self.child(node, i) {
                    Some(n) => node = n,
                    None => return,
                }
                if !self.is_internal(node) {
                    break;
                }
                let par = parent.unwrap_or(0);
                if (1..4).any(|k| self.child(par, (i + k) & 3).is_some()) {
                    retainer = Some((par, i));
                }
            }
        }
        // Find the leaf of the item in the chain of the point.
        let mut previous: Option<usize> = None;
        loop {
            let Node::Leaf { item: d, next } = self.nodes[node] else {
                return;
            };
            if d == item {
                break;
            }
            match next {
                Some(n) => {
                    previous = Some(node);
                    node = n;
                }
                None => return,
            }
        }
        let next = match &mut self.nodes[node] {
            Node::Leaf { next, .. } => next.take(),
            Node::Internal(_) => None,
        };
        if let Some(prev) = previous {
            if let Node::Leaf { next: n, .. } = &mut self.nodes[prev] {
                *n = next;
            }
            return;
        }
        let Some(par) = parent else {
            self.root = next;
            return;
        };
        self.set_child(par, i, next);
        let first = (0..4).find_map(|k| self.child(par, k));
        let last = (0..4).rev().find_map(|k| self.child(par, k));
        if let (Some(f), Some(l)) = (first, last)
            && f == l
            && !self.is_internal(f)
        {
            match retainer {
                Some((r, j)) => self.set_child(r, j, Some(f)),
                None => self.root = Some(f),
            }
        }
    }

    /// The nearest item within `radius` of a point (`find`): the first item
    /// of its leaf.
    pub fn find(&self, x: f64, y: f64, radius: f64) -> Option<usize> {
        let mut found = None;
        let root = self.root?;
        let (mut x0, mut y0) = (x - radius, y - radius);
        let (mut x3, mut y3) = (x + radius, y + radius);
        let mut r2 = radius * radius;
        let mut quads = vec![(Some(root), self.x0, self.y0, self.x1, self.y1)];
        while let Some((node, qx1, qy1, qx2, qy2)) = quads.pop() {
            let Some(node) = node else {
                continue;
            };
            if qx1 > x3 || qy1 > y3 || qx2 < x0 || qy2 < y0 {
                continue;
            }
            match self.nodes[node] {
                Node::Internal(c) => {
                    let xm = (qx1 + qx2) / 2.0;
                    let ym = (qy1 + qy2) / 2.0;
                    quads.push((c[3], xm, ym, qx2, qy2));
                    quads.push((c[2], qx1, ym, xm, qy2));
                    quads.push((c[1], xm, qy1, qx2, ym));
                    quads.push((c[0], qx1, qy1, xm, ym));
                    let k = quadrant(x >= xm, y >= ym);
                    if k != 0 {
                        let last = quads.len() - 1;
                        quads.swap(last, last - k);
                    }
                }
                Node::Leaf { item, .. } => {
                    let [px, py] = self.points[item];
                    let (dx, dy) = (x - px, y - py);
                    let d2 = dx * dx + dy * dy;
                    if d2 < r2 {
                        r2 = d2;
                        let d = libm::sqrt(d2);
                        x0 = x - d;
                        y0 = y - d;
                        x3 = x + d;
                        y3 = y + d;
                        found = Some(item);
                    }
                }
            }
        }
        found
    }

    /// All items within `radius` of a point, the nearest quadrants first
    /// (`findAllInQuadtree`).
    pub fn find_all(&self, x: f64, y: f64, radius: f64) -> Vec<usize> {
        let mut result = Vec::new();
        let Some(root) = self.root else {
            return result;
        };
        if self.x0.is_nan() {
            return result;
        }
        let (x0, y0, x3, y3) = (x - radius, y - radius, x + radius, y + radius);
        let r2 = radius * radius;
        let mut quads = vec![(Some(root), self.x0, self.y0, self.x1, self.y1)];
        while let Some((node, qx1, qy1, qx2, qy2)) = quads.pop() {
            let Some(node) = node else {
                continue;
            };
            if qx1 > x3 || qy1 > y3 || qx2 < x0 || qy2 < y0 {
                continue;
            }
            match self.nodes[node] {
                Node::Internal(c) => {
                    let xm = (qx1 + qx2) / 2.0;
                    let ym = (qy1 + qy2) / 2.0;
                    quads.push((c[3], xm, ym, qx2, qy2));
                    quads.push((c[2], qx1, ym, xm, qy2));
                    quads.push((c[1], xm, qy1, qx2, ym));
                    quads.push((c[0], qx1, qy1, xm, ym));
                    let k = quadrant(x >= xm, y >= ym);
                    if k != 0 {
                        let last = quads.len() - 1;
                        quads.swap(last, last - k);
                    }
                }
                Node::Leaf { item, .. } => {
                    let [px, py] = self.points[item];
                    let (dx, dy) = (x - px, y - py);
                    if dx * dx + dy * dy < r2 {
                        let mut leaf = Some(node);
                        while let Some(l) = leaf {
                            let Node::Leaf { item, next } = self.nodes[l] else {
                                break;
                            };
                            result.push(item);
                            leaf = next;
                        }
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn find_gives_the_nearest_point_in_the_radius() {
        let t = Quadtree::from_points(&[[0.0, 0.0], [10.0, 0.0], [3.0, 4.0]]);
        assert_eq!(t.find(2.0, 2.0, 100.0), Some(2));
        assert_eq!(t.find(9.0, 0.0, 2.0), Some(1));
        assert_eq!(t.find(50.0, 50.0, 2.0), None);
    }

    #[test]
    fn points_at_one_place_chain_with_the_newest_first() {
        let mut t = Quadtree::new();
        t.add(0, [1.0, 1.0]);
        t.add(1, [5.0, 5.0]);
        t.add(2, [1.0, 1.0]);
        assert_eq!(t.find_all(1.0, 1.0, 0.5), vec![2, 0]);
        t.remove(2);
        assert_eq!(t.find_all(1.0, 1.0, 0.5), vec![0]);
    }

    proptest! {
        #[test]
        fn search_and_remove_agree_with_a_scan(
            points in proptest::collection::vec((0.0f64..100.0, 0.0f64..100.0), 1..60),
            drop in proptest::collection::vec(any::<bool>(), 60),
            (x, y, r) in (0.0f64..100.0, 0.0f64..100.0, 1.0f64..40.0),
        ) {
            let pts: Vec<[f64; 2]> = points.iter().map(|&(a, b)| [a, b]).collect();
            let mut t = Quadtree::from_points(&pts);
            for (k, &d) in drop.iter().take(pts.len()).enumerate() {
                if d { t.remove(k); }
            }
            let alive = |k: usize| !drop[k];
            let mut all = t.find_all(x, y, r);
            all.sort_unstable();
            let scan: Vec<usize> = (0..pts.len())
                .filter(|&k| alive(k))
                .filter(|&k| (pts[k][0] - x).powi(2) + (pts[k][1] - y).powi(2) < r * r)
                .collect();
            prop_assert_eq!(&all, &scan);
            let best = scan.iter().map(|&k| (pts[k][0] - x).powi(2) + (pts[k][1] - y).powi(2))
                .fold(f64::INFINITY, f64::min);
            match t.find(x, y, r) {
                Some(k) => prop_assert_eq!((pts[k][0] - x).powi(2) + (pts[k][1] - y).powi(2), best),
                None => prop_assert!(scan.is_empty()),
            }
        }
    }
}
