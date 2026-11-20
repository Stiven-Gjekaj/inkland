//! The town model: the patches, the walls, the streets, and the wards.
//!
//! A port of `com.watabou.towngenerator.building.Model`, `CurtainWall`,
//! `Topology`, `Patch`, and `com.watabou.geom.Graph` of TownGeneratorOS by
//! Watabou (GPL-3.0, <https://github.com/watabou/TownGeneratorOS>).
//!
//! The patches share their vertices, and the original moves a shared
//! vertex in place. Here each vertex is an index in `Model::v`, so that a
//! move changes each patch that has the vertex.

use super::poly::{self, P, Poly, dist, len};
use super::random::Random;
use super::voronoi::{Voronoi, sort_with};
use super::wards::{self, Kind, Ward};
use std::collections::BTreeMap;

/// A vertex: an index in `Model::v`.
pub type V = usize;

/// A patch: a cell of the town.
#[derive(Debug, Clone)]
pub struct Patch {
    pub shape: Vec<V>,
    pub ward: Option<Ward>,
    pub within_walls: bool,
    pub within_city: bool,
}

impl Patch {
    fn new(shape: Vec<V>) -> Patch {
        Patch {
            shape,
            ward: None,
            within_walls: false,
            within_city: false,
        }
    }
}

/// A wall around some patches.
#[derive(Debug, Clone, Default)]
pub struct CurtainWall {
    pub shape: Vec<V>,
    pub segments: Vec<bool>,
    pub gates: Vec<V>,
    pub towers: Vec<V>,
    /// The patches inside the wall, by id.
    pub patches: Vec<usize>,
}

/// Why a build failed. The model then builds again.
#[derive(Debug)]
pub struct Retry(pub &'static str);

/// The town model.
#[derive(Debug, Clone)]
pub struct Model {
    pub rng: Random,
    pub n_patches: usize,
    pub plaza_needed: bool,
    pub citadel_needed: bool,
    pub walls_needed: bool,
    /// The vertices.
    pub v: Vec<P>,
    /// Each patch, by id. A patch that a gate road splits stays here, out
    /// of `patches`.
    pub store: Vec<Patch>,
    /// The patches of the town, in order.
    pub patches: Vec<usize>,
    /// The patches within the walls, or all town patches with no walls.
    pub inner: Vec<usize>,
    pub citadel: Option<usize>,
    pub plaza: Option<usize>,
    pub center: V,
    pub border: CurtainWall,
    /// `true` if `border` is a real wall.
    pub walled: bool,
    pub city_radius: f64,
    /// All gates, with the gates of the castle.
    pub gates: Vec<V>,
    pub arteries: Vec<Vec<V>>,
    pub streets: Vec<Vec<V>>,
    pub roads: Vec<Vec<V>>,
}

/// The ward kinds of the inner town, in the order of the original.
const WARDS: [Kind; 36] = {
    use Kind::*;
    [
        Craftsmen,
        Craftsmen,
        Merchant,
        Craftsmen,
        Craftsmen,
        Cathedral,
        Craftsmen,
        Craftsmen,
        Craftsmen,
        Craftsmen,
        Craftsmen,
        Craftsmen,
        Craftsmen,
        Craftsmen,
        Administration,
        Craftsmen,
        Slum,
        Craftsmen,
        Slum,
        Patriciate,
        Market,
        Slum,
        Craftsmen,
        Craftsmen,
        Craftsmen,
        Slum,
        Craftsmen,
        Craftsmen,
        Craftsmen,
        Military,
        Slum,
        Craftsmen,
        Park,
        Patriciate,
        Market,
        Merchant,
    ]
};

/// The most builds of a town before it gives up.
const MAX_TRIES: usize = 200;

impl Model {
    /// A town of `n_patches` patches. `plaza`, `citadel`, and `walls`
    /// replace the random choices of the original when they are given.
    pub fn new(
        n_patches: usize,
        seed: i64,
        plaza: Option<bool>,
        citadel: Option<bool>,
        walls: Option<bool>,
    ) -> Result<Model, Retry> {
        let mut rng = Random::new(seed);
        let n_patches = n_patches.max(3);
        // The original draws the three choices; a given value replaces the
        // draw, and the draws still happen so that the rest stays the same.
        let (p, c, w) = (rng.bool(0.5), rng.bool(0.5), rng.bool(0.5));
        let plaza_needed = plaza.unwrap_or(p);
        let citadel_needed = citadel.unwrap_or(c);
        let walls_needed = walls.unwrap_or(w);
        let mut model = Model {
            rng,
            n_patches,
            plaza_needed,
            citadel_needed,
            walls_needed,
            v: Vec::new(),
            store: Vec::new(),
            patches: Vec::new(),
            inner: Vec::new(),
            citadel: None,
            plaza: None,
            center: 0,
            border: CurtainWall::default(),
            walled: false,
            city_radius: 0.0,
            gates: Vec::new(),
            arteries: Vec::new(),
            streets: Vec::new(),
            roads: Vec::new(),
        };
        let mut last = Retry("no build");
        for _ in 0..MAX_TRIES {
            match model.build() {
                Ok(()) => return Ok(model),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn build(&mut self) -> Result<(), Retry> {
        self.v.clear();
        self.store.clear();
        self.patches.clear();
        self.inner.clear();
        self.citadel = None;
        self.plaza = None;
        self.streets.clear();
        self.roads.clear();
        self.arteries.clear();
        self.gates.clear();
        self.walled = false;
        self.build_patches();
        self.optimize_junctions();
        self.build_walls()?;
        self.build_streets()?;
        self.create_wards();
        self.build_geometry();
        Ok(())
    }

    /// The coordinates of a list of vertices.
    pub fn pts(&self, shape: &[V]) -> Poly {
        shape.iter().map(|&i| self.v[i]).collect()
    }

    pub fn shape(&self, patch: usize) -> &Vec<V> {
        &self.store[patch].shape
    }

    /// The coordinates of the shape of a patch.
    pub fn shape_pts(&self, patch: usize) -> Poly {
        self.pts(&self.store[patch].shape)
    }

    fn new_patch(&mut self, shape: Vec<V>) -> usize {
        self.store.push(Patch::new(shape));
        self.store.len() - 1
    }

    fn build_patches(&mut self) {
        let sa = self.rng.float() * 2.0 * std::f64::consts::PI;
        let points: Vec<P> = (0..self.n_patches * 8)
            .map(|i| {
                let a = sa + (i as f64).sqrt() * 5.0;
                let r = if i == 0 {
                    0.0
                } else {
                    10.0 + i as f64 * (2.0 + self.rng.float())
                };
                [libm::cos(a) * r, libm::sin(a) * r]
            })
            .collect();
        let mut voronoi = Voronoi::build(&points);
        // The original relaxes points 0, 1, 2, and n of the list, and the
        // first three are the frame.
        for _ in 0..3 {
            let mut to_relax: Vec<usize> = voronoi.points.iter().take(3).copied().collect();
            if let Some(&p) = voronoi.points.get(self.n_patches) {
                to_relax.push(p);
            }
            voronoi = voronoi.relax(&to_relax);
        }
        let pts = voronoi.pts.clone();
        sort_with(&mut voronoi.points, &|&a, &b| {
            let d = len(pts[a]) - len(pts[b]);
            if d == 0.0 {
                0
            } else if d < 0.0 {
                -1
            } else {
                1
            }
        });
        let regions = voronoi.partitioning();
        // A vertex for each triangle, so that the patches share them.
        let mut tri_vertex: BTreeMap<usize, V> = BTreeMap::new();
        for (count, r) in regions.iter().enumerate() {
            let shape: Vec<V> = r
                .vertices
                .iter()
                .map(|&t| {
                    *tri_vertex.entry(t).or_insert_with(|| {
                        self.v.push(voronoi.triangles[t].c);
                        self.v.len() - 1
                    })
                })
                .collect();
            let id = self.new_patch(shape);
            self.patches.push(id);
            if count == 0 {
                let s = self.store[id].shape.clone();
                self.center = poly::min_by(&s, |&p| len(self.v[p]));
                if self.plaza_needed {
                    self.plaza = Some(id);
                }
            } else if count == self.n_patches && self.citadel_needed {
                self.citadel = Some(id);
                self.store[id].within_city = true;
            }
            if count < self.n_patches {
                self.store[id].within_city = true;
                self.store[id].within_walls = self.walls_needed;
                self.inner.push(id);
            }
        }
    }

    /// The patches that have the vertex `v`.
    pub fn patch_by_vertex(&self, v: V) -> Vec<usize> {
        self.patches
            .iter()
            .copied()
            .filter(|&p| self.store[p].shape.contains(&v))
            .collect()
    }

    fn optimize_junctions(&mut self) {
        let mut to_optimize = self.inner.clone();
        if let Some(c) = self.citadel {
            to_optimize.push(c);
        }
        let mut to_clean = Vec::new();
        for w in to_optimize {
            let mut index = 0;
            while index < self.store[w].shape.len() {
                let n = self.store[w].shape.len();
                let v0 = self.store[w].shape[index];
                let v1 = self.store[w].shape[(index + 1) % n];
                if v0 != v1 && dist(self.v[v0], self.v[v1]) < 8.0 {
                    for w1 in self.patch_by_vertex(v1) {
                        if w1 != w {
                            if let Some(k) = self.store[w1].shape.iter().position(|&x| x == v1) {
                                self.store[w1].shape[k] = v0;
                            }
                            to_clean.push(w1);
                        }
                    }
                    let (a, b) = (self.v[v0], self.v[v1]);
                    self.v[v0] = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
                    if let Some(k) = self.store[w].shape.iter().position(|&x| x == v1) {
                        self.store[w].shape.remove(k);
                    }
                }
                index += 1;
            }
        }
        // Remove the duplicate vertices.
        for w in to_clean {
            let mut i = 0;
            while i < self.store[w].shape.len() {
                let v = self.store[w].shape[i];
                while let Some(d) = self.store[w].shape[i + 1..].iter().position(|&x| x == v) {
                    self.store[w].shape.remove(i + 1 + d);
                }
                i += 1;
            }
        }
    }

    /// The outline of a set of patches.
    pub fn find_circumference(&self, wards: &[usize]) -> Result<Vec<V>, Retry> {
        match wards.len() {
            0 => return Ok(Vec::new()),
            1 => return Ok(self.store[wards[0]].shape.clone()),
            _ => {}
        }
        let (mut a_list, mut b_list) = (Vec::new(), Vec::new());
        for &w1 in wards {
            let s = &self.store[w1].shape;
            let n = s.len();
            for i in 0..n {
                let (a, b) = (s[i], s[(i + 1) % n]);
                let outer = !wards
                    .iter()
                    .any(|&w2| find_edge(&self.store[w2].shape, b, a).is_some());
                if outer {
                    a_list.push(a);
                    b_list.push(b);
                }
            }
        }
        let mut result = Vec::new();
        let mut index = 0;
        for _ in 0..=a_list.len() {
            result.push(a_list[index]);
            match a_list.iter().position(|&x| x == b_list[index]) {
                Some(0) => return Ok(result),
                Some(i) => index = i,
                None => return Err(Retry("broken town outline")),
            }
        }
        Err(Retry("broken town outline"))
    }

    /// A wall around `patches`. `reserved` vertices do not move and have no
    /// gate.
    fn curtain_wall(
        &mut self,
        real: bool,
        patches: &[usize],
        reserved: &[V],
    ) -> Result<CurtainWall, Retry> {
        let shape = if patches.len() == 1 {
            self.store[patches[0]].shape.clone()
        } else {
            let shape = self.find_circumference(patches)?;
            if real {
                let f = f64::min(1.0, 40.0 / patches.len() as f64);
                let moved: Vec<P> = shape
                    .iter()
                    .map(|&v| {
                        if reserved.contains(&v) {
                            self.v[v]
                        } else {
                            smooth_vertex(&self.v, &shape, v, f)
                        }
                    })
                    .collect();
                for (i, &v) in shape.iter().enumerate() {
                    self.v[v] = moved[i];
                }
            }
            shape
        };
        let mut wall = CurtainWall {
            segments: vec![true; shape.len()],
            shape,
            gates: Vec::new(),
            towers: Vec::new(),
            patches: patches.to_vec(),
        };
        self.build_gates(&mut wall, real, reserved)?;
        Ok(wall)
    }

    fn build_gates(
        &mut self,
        wall: &mut CurtainWall,
        real: bool,
        reserved: &[V],
    ) -> Result<(), Retry> {
        let mut entrances: Vec<V> = if wall.patches.len() > 1 {
            wall.shape
                .iter()
                .copied()
                .filter(|v| {
                    !reserved.contains(v)
                        && wall
                            .patches
                            .iter()
                            .filter(|&&p| self.store[p].shape.contains(v))
                            .count()
                            > 1
                })
                .collect()
        } else {
            wall.shape
                .iter()
                .copied()
                .filter(|v| !reserved.contains(v))
                .collect()
        };
        if entrances.is_empty() {
            return Err(Retry("bad walled area shape"));
        }
        loop {
            let index = self.rng.int(0, entrances.len() as i64) as usize;
            let index = index.min(entrances.len() - 1);
            let gate = entrances[index];
            wall.gates.push(gate);
            if real {
                let outer: Vec<usize> = self
                    .patch_by_vertex(gate)
                    .into_iter()
                    .filter(|w| !wall.patches.contains(w))
                    .collect();
                if outer.len() == 1 {
                    // No road leads out of the walls here: split the outer
                    // patch to make one.
                    let o = outer[0];
                    if self.store[o].shape.len() > 3 {
                        let a = self.v[next_of(&wall.shape, gate)];
                        let b = self.v[prev_of(&wall.shape, gate)];
                        let w = poly::sub(a, b);
                        let out = [w[1], -w[0]];
                        let g = self.v[gate];
                        let os = self.store[o].shape.clone();
                        let farthest = poly::max_by(&os, |&v| {
                            if wall.shape.contains(&v) || reserved.contains(&v) {
                                f64::NEG_INFINITY
                            } else {
                                let dir = poly::sub(self.v[v], g);
                                poly::dot(dir, out) / len(dir)
                            }
                        });
                        let halves = split(&os, gate, farthest);
                        let ids: Vec<usize> =
                            halves.into_iter().map(|h| self.new_patch(h)).collect();
                        if let Some(k) = self.patches.iter().position(|&p| p == o) {
                            self.patches.splice(k..=k, ids);
                        }
                    }
                }
            }
            // Remove the entrances next to the gate, so that no two gates
            // are too close.
            let n = entrances.len();
            if index == 0 {
                entrances.drain(0..n.min(2));
                entrances.pop();
            } else if index == n - 1 {
                entrances.drain(index - 1..n);
                if !entrances.is_empty() {
                    entrances.remove(0);
                }
            } else {
                entrances.drain(index - 1..(index + 2).min(n));
            }
            if entrances.len() < 3 {
                break;
            }
        }
        if wall.gates.is_empty() {
            return Err(Retry("bad walled area shape"));
        }
        // Smooth the wall at the gates.
        if real {
            for &gate in &wall.gates {
                self.v[gate] = smooth_vertex(&self.v, &wall.shape, gate, 1.0);
            }
        }
        Ok(())
    }

    fn build_towers(wall: &mut CurtainWall) {
        let n = wall.shape.len();
        wall.towers = (0..n)
            .filter(|&i| {
                !wall.gates.contains(&wall.shape[i])
                    && (wall.segments[(i + n - 1) % n] || wall.segments[i])
            })
            .map(|i| wall.shape[i])
            .collect();
    }

    /// The real wall of the town, if it has one.
    pub fn wall(&self) -> Option<&CurtainWall> {
        self.walled.then_some(&self.border)
    }

    fn build_walls(&mut self) -> Result<(), Retry> {
        let reserved = self
            .citadel
            .map(|c| self.store[c].shape.clone())
            .unwrap_or_default();
        let inner = self.inner.clone();
        let mut border = self.curtain_wall(self.walls_needed, &inner, &reserved)?;
        if self.walls_needed {
            Self::build_towers(&mut border);
            self.walled = true;
        }
        let radius = border
            .shape
            .iter()
            .map(|&v| len(self.v[v]))
            .fold(0.0, f64::max);
        let center = self.v[self.center];
        // The original measures to the first vertex of each patch.
        self.patches.retain(|&p| {
            self.store[p]
                .shape
                .first()
                .is_some_and(|&v| dist(self.v[v], center) < radius * 3.0)
        });
        self.gates = border.gates.clone();
        self.border = border;
        if let Some(c) = self.citadel {
            let shape = self.store[c].shape.clone();
            let reserved: Vec<V> = shape
                .iter()
                .copied()
                .filter(|&v| {
                    self.patch_by_vertex(v)
                        .iter()
                        .any(|&p| !self.store[p].within_city)
                })
                .collect();
            let mut wall = self.curtain_wall(true, &[c], &reserved)?;
            Self::build_towers(&mut wall);
            let gates = wall.gates.clone();
            self.store[c].ward = Some(Ward::new(Kind::Castle, Some(wall)));
            if poly::compactness(&self.shape_pts(c)) < 0.75 {
                return Err(Retry("bad citadel shape"));
            }
            self.gates.extend(gates);
        }
        Ok(())
    }

    fn build_streets(&mut self) -> Result<(), Retry> {
        let topology = Topology::new(self);
        let gates = self.gates.clone();
        for gate in gates {
            // Each gate joins the nearest corner of the plaza, or the
            // centre.
            let end = match self.plaza {
                Some(p) => {
                    let s = self.store[p].shape.clone();
                    let g = self.v[gate];
                    poly::min_by(&s, |&v| dist(self.v[v], g))
                }
                None => self.center,
            };
            let Some(street) = topology.build_path(gate, end, &topology.outer) else {
                return Err(Retry("unable to build a street"));
            };
            self.streets.push(street);
            if self.border.gates.contains(&gate) {
                let dir = poly::norm(self.v[gate], 1000.0);
                let mut start = None;
                let mut best = f64::INFINITY;
                for &p in &topology.node2pt {
                    let d = dist(self.v[p], dir);
                    if d < best {
                        best = d;
                        start = Some(p);
                    }
                }
                if let Some(s) = start
                    && let Some(road) = topology.build_path(s, gate, &topology.inner)
                {
                    self.roads.push(road);
                }
            }
        }
        self.tidy_up_roads();
        let arteries = self.arteries.clone();
        for a in arteries {
            let pts = self.pts(&a);
            let smoothed = poly::smooth_vertex_eq(&pts, 3.0);
            for i in 1..a.len().saturating_sub(1) {
                self.v[a[i]] = smoothed[i];
            }
        }
        Ok(())
    }

    fn tidy_up_roads(&mut self) {
        let mut segments: Vec<(V, V)> = Vec::new();
        let plaza = self.plaza.map(|p| self.store[p].shape.clone());
        let lists: Vec<Vec<V>> = self.streets.iter().chain(&self.roads).cloned().collect();
        for street in lists {
            for w in street.windows(2) {
                let (v0, v1) = (w[0], w[1]);
                // Leave out the segments along the plaza.
                if plaza
                    .as_ref()
                    .is_some_and(|s| s.contains(&v0) && s.contains(&v1))
                {
                    continue;
                }
                if !segments.iter().any(|&(a, b)| a == v0 && b == v1) {
                    segments.push((v0, v1));
                }
            }
        }
        self.arteries.clear();
        while let Some((start, end)) = segments.pop() {
            let mut attached = false;
            for a in &mut self.arteries {
                if a[0] == end {
                    a.insert(0, start);
                    attached = true;
                    break;
                } else if a[a.len() - 1] == start {
                    a.push(end);
                    attached = true;
                    break;
                }
            }
            if !attached {
                self.arteries.push(vec![start, end]);
            }
        }
    }

    fn create_wards(&mut self) {
        let mut unassigned = self.inner.clone();
        if let Some(p) = self.plaza {
            self.store[p].ward = Some(Ward::new(Kind::Market, None));
            unassigned.retain(|&x| x != p);
        }
        // Gate wards inside the town.
        let chance = if self.walled { 0.5 } else { 0.2 };
        for gate in self.border.gates.clone() {
            for patch in self.patch_by_vertex(gate) {
                if self.store[patch].within_city
                    && self.store[patch].ward.is_none()
                    && self.rng.bool(chance)
                {
                    let w = wards::common(Kind::Gate, &mut self.rng);
                    self.store[patch].ward = Some(w);
                    unassigned.retain(|&x| x != patch);
                }
            }
        }
        let mut list = WARDS.to_vec();
        for _ in 0..list.len() / 10 {
            let index = self.rng.int(0, list.len() as i64 - 1) as usize;
            list.swap(index, index + 1);
        }
        let mut list: std::collections::VecDeque<Kind> = list.into();
        while !unassigned.is_empty() {
            let kind = list.pop_front().unwrap_or(Kind::Slum);
            let best = if wards::has_rate(kind) {
                poly::min_by(&unassigned, |&p| {
                    if self.store[p].ward.is_none() {
                        wards::rate(self, kind, p)
                    } else {
                        f64::INFINITY
                    }
                })
            } else {
                unassigned[self.rng.index(unassigned.len())]
            };
            let w = wards::create(kind, &mut self.rng);
            self.store[best].ward = Some(w);
            unassigned.retain(|&x| x != best);
        }
        // The outskirts at the gates.
        if self.walled {
            for gate in self.border.gates.clone() {
                let p = 1.0 / (self.n_patches as f64 - 5.0);
                if !self.rng.bool(p) {
                    for patch in self.patch_by_vertex(gate) {
                        if self.store[patch].ward.is_none() {
                            self.store[patch].within_city = true;
                            let w = wards::common(Kind::Gate, &mut self.rng);
                            self.store[patch].ward = Some(w);
                        }
                    }
                }
            }
        }
        // The radius of the town, and the countryside.
        self.city_radius = 0.0;
        for patch in self.patches.clone() {
            if self.store[patch].within_city {
                for &v in &self.store[patch].shape {
                    self.city_radius = self.city_radius.max(len(self.v[v]));
                }
            } else if self.store[patch].ward.is_none() {
                let farm = self.rng.bool(0.2) && poly::compactness(&self.shape_pts(patch)) >= 0.7;
                self.store[patch].ward =
                    Some(Ward::new(if farm { Kind::Farm } else { Kind::Empty }, None));
            }
        }
    }

    fn build_geometry(&mut self) {
        for patch in self.patches.clone() {
            if let Some(mut w) = self.store[patch].ward.take() {
                w.geometry = wards::geometry(self, patch, &w);
                self.store[patch].ward = Some(w);
            }
        }
    }

    /// The patch on the other side of the edge that starts at `v`.
    pub fn get_neighbour(&self, patch: usize, v: V) -> Option<usize> {
        let nx = next_of(&self.store[patch].shape, v);
        self.patches
            .iter()
            .copied()
            .find(|&p| find_edge(&self.store[p].shape, nx, v).is_some())
    }

    pub fn get_neighbours(&self, patch: usize) -> Vec<usize> {
        self.patches
            .iter()
            .copied()
            .filter(|&p| p != patch && borders(&self.store[p].shape, &self.store[patch].shape))
            .collect()
    }

    /// `true` if the patch is in the town and only town patches are around
    /// it.
    pub fn is_enclosed(&self, patch: usize) -> bool {
        let p = &self.store[patch];
        p.within_city
            && (p.within_walls
                || self
                    .get_neighbours(patch)
                    .iter()
                    .all(|&n| self.store[n].within_city))
    }
}

/// The index of the edge from `a` to `b` of a shape.
pub fn find_edge(s: &[V], a: V, b: V) -> Option<usize> {
    let i = s.iter().position(|&x| x == a)?;
    (s[(i + 1) % s.len()] == b).then_some(i)
}

pub fn next_of(s: &[V], a: V) -> V {
    let i = s.iter().position(|&x| x == a).unwrap_or(0);
    s[(i + 1) % s.len()]
}

pub fn prev_of(s: &[V], a: V) -> V {
    let i = s.iter().position(|&x| x == a).unwrap_or(0);
    s[(i + s.len() - 1) % s.len()]
}

/// `true` if two shapes share an edge.
pub fn borders(a: &[V], b: &[V]) -> bool {
    let (n1, n2) = (a.len(), b.len());
    for i in 0..n1 {
        if let Some(j) = b.iter().position(|&x| x == a[i]) {
            let nx = a[(i + 1) % n1];
            if nx == b[(j + 1) % n2] || nx == b[(j + n2 - 1) % n2] {
                return true;
            }
        }
    }
    false
}

/// The weighted mean of a vertex and its two neighbours in a shape.
fn smooth_vertex(v: &[P], s: &[V], at: V, f: f64) -> P {
    let p = v[prev_of(s, at)];
    let n = v[next_of(s, at)];
    let c = v[at];
    [
        (p[0] + c[0] * f + n[0]) / (2.0 + f),
        (p[1] + c[1] * f + n[1]) / (2.0 + f),
    ]
}

/// The two parts of a shape between the vertices `a` and `b`.
fn split(s: &[V], a: V, b: V) -> Vec<Vec<V>> {
    let mut i1 = s.iter().position(|&x| x == a).unwrap_or(0);
    let mut i2 = s.iter().position(|&x| x == b).unwrap_or(0);
    if i1 > i2 {
        std::mem::swap(&mut i1, &mut i2);
    }
    let first = s[i1..=i2].to_vec();
    let mut second = s[i2..].to_vec();
    second.extend_from_slice(&s[..=i1]);
    vec![first, second]
}

impl CurtainWall {
    /// `true` if the wall runs along the edge from `v0` to `v1` of `p`.
    pub fn borders_by(&self, p: usize, v0: V, v1: V) -> bool {
        let index = if self.patches.contains(&p) {
            find_edge(&self.shape, v0, v1)
        } else {
            find_edge(&self.shape, v1, v0)
        };
        index.is_some_and(|i| self.segments[i])
    }

    /// `true` if the wall runs along an edge of the patch.
    pub fn borders(&self, model: &Model, p: usize) -> bool {
        let within = self.patches.contains(&p);
        let n = self.shape.len();
        let s = &model.store[p].shape;
        (0..n).filter(|&i| self.segments[i]).any(|i| {
            let (v0, v1) = (self.shape[i], self.shape[(i + 1) % n]);
            if within {
                find_edge(s, v0, v1).is_some()
            } else {
                find_edge(s, v1, v0).is_some()
            }
        })
    }
}

/// The graph of the vertices of the patches, for the streets.
struct Topology {
    /// The vertex of each node, by node index.
    node2pt: Vec<V>,
    pt2node: BTreeMap<V, usize>,
    /// The links of each node to its neighbours, with the price.
    links: Vec<BTreeMap<usize, f64>>,
    inner: Vec<usize>,
    outer: Vec<usize>,
}

impl Topology {
    fn new(model: &Model) -> Topology {
        let mut blocked: Vec<V> = Vec::new();
        if let Some(c) = model.citadel {
            blocked.extend(&model.store[c].shape);
        }
        if let Some(w) = model.wall() {
            blocked.extend(&w.shape);
        }
        blocked.retain(|v| !model.gates.contains(v));
        let border = &model.border.shape;
        let mut t = Topology {
            node2pt: Vec::new(),
            pt2node: BTreeMap::new(),
            links: Vec::new(),
            inner: Vec::new(),
            outer: Vec::new(),
        };
        let process = |t: &mut Topology, v: V| -> Option<usize> {
            let n = match t.pt2node.get(&v) {
                Some(&n) => n,
                None => {
                    let n = t.node2pt.len();
                    t.node2pt.push(v);
                    t.links.push(BTreeMap::new());
                    t.pt2node.insert(v, n);
                    n
                }
            };
            (!blocked.contains(&v)).then_some(n)
        };
        for &p in &model.patches {
            let within = model.store[p].within_city;
            let s = &model.store[p].shape;
            let mut v1 = s[s.len() - 1];
            let mut n1 = process(&mut t, v1);
            for &vi in s {
                let v0 = v1;
                v1 = vi;
                let n0 = n1;
                n1 = process(&mut t, v1);
                for (n, v) in [(n0, v0), (n1, v1)] {
                    if let Some(n) = n
                        && !border.contains(&v)
                    {
                        let list = if within { &mut t.inner } else { &mut t.outer };
                        if !list.contains(&n) {
                            list.push(n);
                        }
                    }
                }
                if let (Some(a), Some(b)) = (n0, n1) {
                    let d = dist(model.v[v0], model.v[v1]);
                    t.links[a].insert(b, d);
                    t.links[b].insert(a, d);
                }
            }
        }
        t
    }

    /// The path from `from` to `to`, from the end back to the start, that
    /// does not go through the `exclude` nodes. The search of the original
    /// takes the open nodes in the order that it finds them.
    fn build_path(&self, from: V, to: V, exclude: &[usize]) -> Option<Vec<V>> {
        let start = *self.pt2node.get(&from)?;
        let goal = *self.pt2node.get(&to)?;
        let mut closed: Vec<bool> = vec![false; self.node2pt.len()];
        for &e in exclude {
            closed[e] = true;
        }
        let mut open: std::collections::VecDeque<usize> = std::collections::VecDeque::from([start]);
        let mut in_open = vec![false; self.node2pt.len()];
        in_open[start] = true;
        let mut came_from: BTreeMap<usize, usize> = BTreeMap::new();
        let mut g: BTreeMap<usize, f64> = BTreeMap::from([(start, 0.0)]);
        while let Some(current) = open.pop_front() {
            in_open[current] = false;
            if current == goal {
                let mut path = vec![self.node2pt[current]];
                let mut c = current;
                while let Some(&p) = came_from.get(&c) {
                    c = p;
                    path.push(self.node2pt[c]);
                    if path.len() > self.node2pt.len() {
                        break;
                    }
                }
                return Some(path);
            }
            closed[current] = true;
            let cur = g[&current];
            for (&nb, &price) in &self.links[current] {
                if closed[nb] {
                    continue;
                }
                let score = cur + price;
                if !in_open[nb] {
                    open.push_back(nb);
                    in_open[nb] = true;
                } else if score >= g[&nb] {
                    continue;
                }
                came_from.insert(nb, current);
                g.insert(nb, score);
            }
        }
        None
    }
}
