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
    /// inkland: a patch of the sea.
    pub water: bool,
}

impl Patch {
    fn new(shape: Vec<V>) -> Patch {
        Patch {
            shape,
            ward: None,
            within_walls: false,
            within_city: false,
            water: false,
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

/// inkland: the land around a town, from the world map. All directions
/// are in radians from the centre: 0 east, a quarter turn south.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Terrain {
    /// The direction of the sea, or `None` for no sea.
    pub coast: Option<f64>,
    /// The direction in which a river flows through the town, or `None`.
    pub river: Option<f64>,
    /// The direction in which the land goes up, or `None` for flat land.
    /// The castle stands on this side.
    pub uphill: Option<f64>,
    /// The directions of the roads to the places near the town. Empty: a
    /// road from each gate, as in the original.
    pub roads: Vec<f64>,
}

/// The town model.
#[derive(Debug, Clone)]
pub struct Model {
    pub rng: Random,
    pub n_patches: usize,
    /// inkland: narrow lots along the streets, with yards in the blocks.
    /// `false`: the buildings of the original.
    pub lots: bool,
    /// inkland: the direction of the sea from the centre, in radians (0
    /// east, a quarter turn south), or `None` for no sea.
    pub coast: Option<f64>,
    /// inkland: the direction in which a river flows through the town, or
    /// `None` for no river.
    pub river_dir: Option<f64>,
    /// inkland: the direction of the higher land, for the castle.
    pub uphill: Option<f64>,
    /// inkland: the directions of the roads out of the town.
    pub road_dirs: Vec<f64>,
    /// inkland: the vertices of the river, from upstream.
    pub river: Vec<V>,
    /// inkland: the piers on the shore of the town.
    pub piers: Vec<Poly>,
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
        lots: bool,
        terrain: &Terrain,
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
            lots,
            coast: terrain.coast,
            river_dir: terrain.river,
            uphill: terrain.uphill,
            road_dirs: terrain.roads.clone(),
            river: Vec::new(),
            piers: Vec::new(),
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
        self.river.clear();
        self.piers.clear();
        self.build_patches();
        self.optimize_junctions();
        self.build_river()?;
        self.build_walls()?;
        self.build_streets()?;
        self.create_wards();
        self.build_geometry();
        self.build_piers();
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
        // inkland: the sea covers the regions beyond a shore line across the
        // direction of the sea, so that it takes a part of the town.
        let centres: Vec<P> = regions.iter().map(|r| voronoi.region_center(r)).collect();
        let sea = self.coast.map(|a| {
            let dir = [libm::cos(a), libm::sin(a)];
            let reach = centres.get(self.n_patches).map_or(50.0, |c| len(*c));
            (dir, reach * (0.45 + 0.2 * self.rng.float()))
        });
        // inkland: on higher land the castle takes the patch most uphill
        // of the first ring outside the town, not the first of the ring.
        let is_sea =
            |k: usize| sea.is_some_and(|(dir, shore)| k > 0 && poly::dot(centres[k], dir) > shore);
        let land: Vec<usize> = (0..regions.len()).filter(|&k| !is_sea(k)).collect();
        let castle_region = match self.uphill {
            Some(a) => {
                let up = [libm::cos(a), libm::sin(a)];
                let ring: Vec<usize> = land.iter().copied().skip(self.n_patches).take(5).collect();
                (!ring.is_empty()).then(|| poly::max_by(&ring, |&k| poly::dot(centres[k], up)))
            }
            None => land.get(self.n_patches).copied(),
        };
        // A vertex for each triangle, so that the patches share them.
        let mut tri_vertex: BTreeMap<usize, V> = BTreeMap::new();
        let mut count = 0;
        for (k, r) in regions.iter().enumerate() {
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
            if is_sea(k) {
                self.store[id].water = true;
                continue;
            }
            if count == 0 {
                let s = self.store[id].shape.clone();
                self.center = poly::min_by(&s, |&p| len(self.v[p]));
                if self.plaza_needed {
                    self.plaza = Some(id);
                }
            } else if Some(k) == castle_region && self.citadel_needed {
                self.citadel = Some(id);
                self.store[id].within_city = true;
            }
            if count < self.n_patches {
                self.store[id].within_city = true;
                self.store[id].within_walls = self.walls_needed;
                self.inner.push(id);
            }
            count += 1;
        }
    }

    /// `true` if the edge from `a` to `b` is a part of the river.
    pub fn on_river(&self, a: V, b: V) -> bool {
        self.river
            .windows(2)
            .any(|w| (w[0] == a && w[1] == b) || (w[0] == b && w[1] == a))
    }

    /// `true` if a water patch has the edge from `b` to `a`, so that the
    /// edge from `a` to `b` of a land patch is on the shore.
    pub fn on_shore(&self, a: V, b: V) -> bool {
        self.patches
            .iter()
            .any(|&p| self.store[p].water && find_edge(&self.store[p].shape, b, a).is_some())
    }

    /// `true` if a water patch has the vertex.
    pub fn at_water(&self, v: V) -> bool {
        self.patches
            .iter()
            .any(|&p| self.store[p].water && self.store[p].shape.contains(&v))
    }

    /// inkland: `true` if a patch is on the shore of the sea.
    pub fn at_water_patch(&self, patch: usize) -> bool {
        self.store[patch].shape.iter().any(|&v| self.at_water(v))
    }

    /// inkland: a river along the edges of the patches, across the town in
    /// its direction, from the far edge upstream to the far edge downstream
    /// or to the sea.
    fn build_river(&mut self) -> Result<(), Retry> {
        let Some(a) = self.river_dir else {
            return Ok(());
        };
        let dir = [libm::cos(a), libm::sin(a)];
        // The river goes through the town: an edge far from the line
        // through the centre costs more.
        let reach = self
            .patches
            .iter()
            .filter(|&&p| self.store[p].within_city)
            .flat_map(|&p| self.store[p].shape.iter())
            .map(|&v| len(self.v[v]))
            .fold(1.0, f64::max);
        let off = |q: P| (q[0] * dir[1] - q[1] * dir[0]).abs() / reach;
        // The graph of the edges of the land patches.
        let mut links: BTreeMap<V, BTreeMap<V, f64>> = BTreeMap::new();
        for &p in &self.patches {
            if self.store[p].water {
                continue;
            }
            let s = self.store[p].shape.clone();
            let n = s.len();
            for i in 0..n {
                let (x, y) = (s[i], s[(i + 1) % n]);
                if x == y {
                    continue;
                }
                let mid = poly::scale(poly::add(self.v[x], self.v[y]), 0.5);
                let o = off(mid);
                let d = dist(self.v[x], self.v[y])
                    * (1.0 + 0.8 * self.rng.float())
                    * (1.0 + 4.0 * o * o);
                links.entry(x).or_default().insert(y, d);
                links.entry(y).or_default().insert(x, d);
            }
        }
        let verts: Vec<V> = links.keys().copied().collect();
        if verts.is_empty() {
            return Ok(());
        }
        let water_end = self.coast.is_some();
        // The river starts on the land, not at the shore.
        let land: Vec<V> = verts
            .iter()
            .copied()
            .filter(|&v| !(water_end && self.at_water(v)))
            .collect();
        if land.is_empty() {
            return Ok(());
        }
        let start = poly::max_by(&land, |&v| {
            -poly::dot(self.v[v], dir) - 2.0 * reach * off(self.v[v])
        });
        let far = poly::max_by(&verts, |&v| {
            poly::dot(self.v[v], dir) - 2.0 * reach * off(self.v[v])
        });
        let is_goal = |m: &Model, v: V| if water_end { m.at_water(v) } else { v == far };
        // Dijkstra over the vertices, in a fixed order.
        let mut cost: BTreeMap<V, f64> = BTreeMap::from([(start, 0.0)]);
        let mut from: BTreeMap<V, V> = BTreeMap::new();
        let mut done: std::collections::BTreeSet<V> = std::collections::BTreeSet::new();
        let mut goal = None;
        while let Some((&v, &c)) = cost
            .iter()
            .filter(|(k, _)| !done.contains(k))
            .min_by(|a, b| a.1.total_cmp(b.1).then(a.0.cmp(b.0)))
        {
            done.insert(v);
            if is_goal(self, v) {
                goal = Some(v);
                break;
            }
            for (&w, &d) in &links[&v] {
                if done.contains(&w) {
                    continue;
                }
                let nc = c + d;
                if cost.get(&w).is_none_or(|&old| nc < old) {
                    cost.insert(w, nc);
                    from.insert(w, v);
                }
            }
        }
        let Some(mut v) = goal else {
            return Err(Retry("no course for the river"));
        };
        let mut path = vec![v];
        while let Some(&p) = from.get(&v) {
            v = p;
            path.push(v);
        }
        path.reverse();
        if path.len() < 3 {
            // The sea is too near the source: the town has no river.
            return Ok(());
        }
        // Smooth the course: each inner vertex moves toward the middle of
        // its neighbours. The patches share the vertices, so they follow.
        for _ in 0..3 {
            let pts = self.pts(&path);
            for i in 1..path.len() - 1 {
                let (a, b, c) = (pts[i - 1], pts[i], pts[i + 1]);
                self.v[path[i]] = [
                    (a[0] + 2.0 * b[0] + c[0]) / 4.0,
                    (a[1] + 2.0 * b[1] + c[1]) / 4.0,
                ];
            }
        }
        self.river = path;
        Ok(())
    }

    /// inkland: piers from the shore of the town into the sea.
    fn build_piers(&mut self) {
        let mut piers = Vec::new();
        for p in self.patches.clone() {
            if self.store[p].water || !self.store[p].within_city {
                continue;
            }
            let s = self.store[p].shape.clone();
            let n = s.len();
            for i in 0..n {
                let (a, b) = (s[i], s[(i + 1) % n]);
                if !self.on_shore(a, b) {
                    continue;
                }
                let (pa, pb) = (self.v[a], self.v[b]);
                let l = dist(pa, pb);
                let u = poly::scale(poly::sub(pb, pa), 1.0 / l.max(1e-9));
                // Out to the sea: the side away from the land patch.
                let c = poly::centroid(&self.shape_pts(p));
                let mut out = [u[1], -u[0]];
                if poly::dot(poly::sub(c, pa), out) > 0.0 {
                    out = [-out[0], -out[1]];
                }
                let mut t = 3.0 + 3.0 * self.rng.float();
                while t < l - 2.0 {
                    let base = poly::add(pa, poly::scale(u, t));
                    let length = 3.0 + 5.0 * self.rng.float();
                    let half = poly::scale(u, 0.6);
                    let tip = poly::scale(out, length);
                    piers.push(vec![
                        poly::sub(base, half),
                        poly::add(base, half),
                        poly::add(poly::add(base, half), tip),
                        poly::add(poly::sub(base, half), tip),
                    ]);
                    t += 6.0 + 6.0 * self.rng.float();
                }
            }
        }
        self.piers = piers;
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
            segments: (0..shape.len())
                .map(|i| !self.on_shore(shape[i], shape[(i + 1) % shape.len()]))
                .collect(),
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
                        && !self.at_water(*v)
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
                .filter(|v| !reserved.contains(v) && !self.at_water(*v))
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
                    .filter(|w| !wall.patches.contains(w) && !self.store[*w].water)
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
            // inkland: with the roads of the world, a gate gets a road only
            // if a road of the world leaves on its side.
            let world_roads: Vec<f64> = self.road_dirs.clone();
            let road_dir = if world_roads.is_empty() {
                Some(poly::norm(self.v[gate], 1000.0))
            } else {
                world_roads
                    .iter()
                    .map(|&a| [libm::cos(a), libm::sin(a)])
                    .filter(|&d| {
                        let border: Vec<V> = self.border.gates.clone();
                        let best =
                            poly::max_by(&border, |&g| poly::dot(poly::norm(self.v[g], 1.0), d));
                        best == gate
                    })
                    .map(|d| poly::scale(d, 1000.0))
                    .next()
            };
            if self.border.gates.contains(&gate)
                && let Some(dir) = road_dir
            {
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
                        if self.store[patch].ward.is_none() && !self.store[patch].water {
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
            } else if self.store[patch].ward.is_none() && !self.store[patch].water {
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
                    .all(|&n| self.store[n].within_city || self.store[n].water))
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
            // inkland: no street on the water, and none along the river.
            if model.store[p].water {
                continue;
            }
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
                if let (Some(a), Some(b)) = (n0, n1)
                    && !model.on_river(v0, v1)
                {
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
