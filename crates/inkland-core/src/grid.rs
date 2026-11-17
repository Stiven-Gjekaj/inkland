//! The grid: a jittered lattice of points and its Voronoi cells.
//!
//! The points are in rows, from the low y edge to the high y edge, and from
//! the low x edge to the high x edge in each row. Thus `slot(x, y)` finds
//! the cell near a position at once. The lattice is square, hexagonal,
//! diamond, or triangular, as in Azgaar's Fantasy Map Generator (FMG,
//! `model/graph/lattice.ts`). Each lattice has rows of `cells_x` slots.
//!
//! A ring of boundary points outside the frame closes each cell. The edges
//! between a real cell and a boundary point are outside the frame.

use crate::config::Lattice;
use crate::rand::Rand;
use delaunator::{EMPTY, Point as DPoint, triangulate};

/// A cell or a vertex index.
pub type Id = u32;

/// Mark for a polygon edge that is next to a boundary point, not a cell.
pub const OUTSIDE: Id = Id::MAX;

/// The form of the lattice of a grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatticeInfo {
    pub kind: Lattice,
    /// The jitter of the points, as a fraction of half the neighbor distance.
    pub jitter: f64,
    /// The side of a square with the mean area of a cell, in units.
    pub spacing: f64,
    /// Odd rows are this far right of even rows, in units.
    pub row_offset: f64,
    /// Triangle centres are this far from the middle of their row, in units.
    pub triangle_shift: f64,
    /// The distance between two neighbor lattice points, in units.
    pub neighbor_distance: f64,
}

/// The grid of one world.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub width: f64,
    pub height: f64,
    /// Number of lattice columns.
    pub cells_x: u32,
    /// Number of lattice rows.
    pub cells_y: u32,
    /// Distance between columns, in units.
    pub step_x: f64,
    /// Distance between rows, in units.
    pub step_y: f64,
    /// The lattice of the grid points.
    pub lattice: LatticeInfo,
    /// The site of each cell, in units.
    pub points: Vec<[f64; 2]>,
    /// The neighbor cells of each cell, in the order of its polygon edges.
    /// `OUTSIDE` marks an edge next to a boundary point.
    pub edge_cells: Vec<Vec<Id>>,
    /// The polygon of each cell: vertex indices. Edge `k` goes from vertex
    /// `k` to vertex `k + 1` and lies between the cell and `edge_cells[k]`.
    pub polygons: Vec<Vec<Id>>,
    /// The Voronoi vertices, in units.
    pub vertices: Vec<[f64; 2]>,
    /// `true` for a cell next to a boundary point.
    pub border: Vec<bool>,
    /// `true` for the second grid (FMG `pack`): its points are not on the
    /// lattice, so a lattice slot is not a cell.
    pub packed: bool,
    /// Buckets of cells for the search of the nearest cell.
    pub index: CellIndex,
}

/// Square buckets of cells. A bucket holds the cells whose sites are in it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CellIndex {
    size: f64,
    cols: usize,
    rows: usize,
    /// The cells of bucket `b` are `cells[start[b]..start[b + 1]]`.
    start: Vec<u32>,
    cells: Vec<u32>,
}

impl CellIndex {
    fn new(points: &[[f64; 2]], width: f64, height: f64, size: f64) -> Self {
        let cols = ((width / size).ceil() as usize).max(1);
        let rows = ((height / size).ceil() as usize).max(1);
        let bucket = |p: [f64; 2]| -> usize {
            let c = ((p[0] / size).floor().max(0.0) as usize).min(cols - 1);
            let r = ((p[1] / size).floor().max(0.0) as usize).min(rows - 1);
            r * cols + c
        };
        let mut count = vec![0u32; cols * rows + 1];
        for &p in points {
            count[bucket(p) + 1] += 1;
        }
        for b in 0..cols * rows {
            count[b + 1] += count[b];
        }
        let start = count.clone();
        let mut fill = count;
        let mut cells = vec![0u32; points.len()];
        for (i, &p) in points.iter().enumerate() {
            let b = bucket(p);
            cells[fill[b] as usize] = i as u32;
            fill[b] += 1;
        }
        CellIndex {
            size,
            cols,
            rows,
            start,
            cells,
        }
    }

    /// The cell whose site is nearest to `(x, y)`. Ties go to the lower
    /// cell index.
    fn nearest(&self, points: &[[f64; 2]], x: f64, y: f64) -> usize {
        let c0 = ((x / self.size).floor().max(0.0) as i64).min(self.cols as i64 - 1);
        let r0 = ((y / self.size).floor().max(0.0) as i64).min(self.rows as i64 - 1);
        let mut best: Option<(f64, usize)> = None;
        let reach = self.cols.max(self.rows) as i64;
        for ring in 0..=reach {
            for r in (r0 - ring)..=(r0 + ring) {
                if r < 0 || r >= self.rows as i64 {
                    continue;
                }
                for c in (c0 - ring)..=(c0 + ring) {
                    if c < 0 || c >= self.cols as i64 {
                        continue;
                    }
                    if (r - r0).abs().max((c - c0).abs()) != ring {
                        continue;
                    }
                    let b = r as usize * self.cols + c as usize;
                    for &cell in &self.cells[self.start[b] as usize..self.start[b + 1] as usize] {
                        let p = points[cell as usize];
                        let d = (p[0] - x) * (p[0] - x) + (p[1] - y) * (p[1] - y);
                        let cell = cell as usize;
                        if best.is_none_or(|(bd, bc)| d < bd || (d == bd && cell < bc)) {
                            best = Some((d, cell));
                        }
                    }
                }
            }
            // A bucket of the next ring is at least `ring` buckets away.
            if let Some((bd, _)) = best {
                let reach = ring as f64 * self.size;
                if bd <= reach * reach {
                    break;
                }
            }
        }
        best.map_or(0, |(_, c)| c)
    }
}

/// How far each point can move from its lattice position, as a fraction of
/// the lattice step. This is the default of FMG.
pub const JITTER: f64 = 0.9;

const SQRT3: f64 = 1.732_050_807_568_877_2;

impl LatticeInfo {
    /// The lattice of a kind for a frame and a cell count. Return the
    /// lattice, the column and row counts, and the column and row steps.
    fn plan(
        kind: Lattice,
        jitter: f64,
        width: f64,
        height: f64,
        cells: u32,
    ) -> (Self, u32, u32, f64, f64) {
        let spacing = libm::sqrt(width * height / f64::from(cells.max(1)));
        let hex_side = libm::sqrt(2.0 / SQRT3);
        // (dx, dy, row offset, triangle shift, neighbor distance), as in
        // FMG `Lattice.computeSteps`.
        let (dx, dy, row_offset, triangle_shift, neighbor_distance) = match kind {
            Lattice::Square => {
                let cells_x = ((width / spacing).round() as u32).max(2);
                let cells_y = ((height / spacing).round() as u32).max(2);
                let info = LatticeInfo {
                    kind,
                    jitter,
                    spacing,
                    row_offset: 0.0,
                    triangle_shift: 0.0,
                    neighbor_distance: spacing,
                };
                return (
                    info,
                    cells_x,
                    cells_y,
                    width / f64::from(cells_x),
                    height / f64::from(cells_y),
                );
            }
            Lattice::HexPointy => {
                let dx = spacing * hex_side;
                (dx, dx * SQRT3 / 2.0, dx / 2.0, 0.0, dx)
            }
            Lattice::HexFlat => {
                let column = spacing * hex_side * SQRT3 / 2.0;
                (
                    column * 2.0,
                    spacing * hex_side / 2.0,
                    column,
                    0.0,
                    spacing * hex_side,
                )
            }
            Lattice::Diamond => {
                let half = spacing / core::f64::consts::SQRT_2;
                (
                    spacing * core::f64::consts::SQRT_2,
                    half,
                    half,
                    0.0,
                    spacing,
                )
            }
            Lattice::Triangle => {
                let side = spacing * libm::sqrt(4.0 / SQRT3);
                let dy = side * SQRT3 / 2.0;
                (side / 2.0, dy, 0.0, dy / 6.0, side / SQRT3)
            }
        };
        let cells_x = (((width - row_offset / 2.0 + 0.5 * dx - 1e-10) / dx).floor() as u32).max(2);
        let cells_y = (((height - triangle_shift + 0.5 * dy - 1e-10) / dy).floor() as u32).max(2);
        let info = LatticeInfo {
            kind,
            jitter,
            spacing,
            row_offset,
            triangle_shift,
            neighbor_distance,
        };
        (info, cells_x, cells_y, dx, dy)
    }

    /// Even rows move left and odd rows move right by half the offset.
    fn row_shift(&self, row: i64) -> f64 {
        ((row & 1) as f64 - 0.5) * self.row_offset
    }

    /// Triangle centres are below the middle of an upward triangle and
    /// above it in a downward one.
    fn triangle_offset(&self, row: i64, col: i64) -> f64 {
        if (row + col) & 1 == 1 {
            -self.triangle_shift
        } else {
            self.triangle_shift
        }
    }
}

impl Grid {
    /// Make a square grid of about `cells` cells on a `width` x `height`
    /// frame.
    pub fn new(width: f64, height: f64, cells: u32, rng: &mut Rand) -> Grid {
        Self::with_sites(width, height, cells, &[], rng).0
    }

    /// Make a square grid in which each of `sites` is exactly the site of a
    /// cell. See `generate`.
    pub fn with_sites(
        width: f64,
        height: f64,
        cells: u32,
        sites: &[[f64; 2]],
        rng: &mut Rand,
    ) -> (Grid, Vec<usize>) {
        Self::generate(width, height, cells, Lattice::Square, JITTER, sites, rng)
    }

    /// Make a grid on a lattice, in which each of `sites` is exactly the
    /// site of a cell. Return the grid and the cell of each site.
    ///
    /// A site takes the lattice slot that contains it. If an earlier site
    /// has that slot, the site takes the nearest free slot. The sites must
    /// be different from each other. The random numbers do not depend on the
    /// sites, so a new site does not move the other cells.
    pub fn generate(
        width: f64,
        height: f64,
        cells: u32,
        kind: Lattice,
        jitter: f64,
        sites: &[[f64; 2]],
        rng: &mut Rand,
    ) -> (Grid, Vec<usize>) {
        let (lattice, cells_x, cells_y, step_x, step_y) =
            LatticeInfo::plan(kind, jitter, width, height, cells);

        let mut points = Vec::with_capacity((cells_x * cells_y) as usize);
        // Two random numbers for each point, for each lattice.
        let max_jitter = lattice.neighbor_distance / 2.0 * jitter;
        for row in 0..cells_y {
            for col in 0..cells_x {
                if kind == Lattice::Square {
                    let jx = (rng.unit() - 0.5) * jitter * step_x;
                    let jy = (rng.unit() - 0.5) * jitter * step_y;
                    points.push([
                        (f64::from(col) + 0.5) * step_x + jx,
                        (f64::from(row) + 0.5) * step_y + jy,
                    ]);
                    continue;
                }
                // Off the square lattice the neighbors are at an angle, so a
                // point moves inside a circle.
                let radius = max_jitter * libm::sqrt(rng.unit());
                let angle = 2.0 * core::f64::consts::PI * rng.unit();
                let (r, c) = (i64::from(row), i64::from(col));
                let x = step_x / 2.0 + f64::from(col) * step_x + lattice.row_shift(r);
                let y = step_y / 2.0 + f64::from(row) * step_y + lattice.triangle_offset(r, c);
                points.push([
                    (x + radius * libm::cos(angle)).clamp(0.0, width),
                    (y + radius * libm::sin(angle)).clamp(0.0, height),
                ]);
            }
        }
        let mut grid = Grid {
            width,
            height,
            cells_x,
            cells_y,
            step_x,
            step_y,
            lattice,
            points: Vec::new(),
            edge_cells: Vec::new(),
            polygons: Vec::new(),
            vertices: Vec::new(),
            border: Vec::new(),
            packed: false,
            index: CellIndex::default(),
        };
        let mut taken = vec![false; points.len()];
        let mut site_cells = Vec::with_capacity(sites.len());
        for &site in sites {
            let slot = grid.slot(site[0], site[1]);
            let (col, row) = (
                (slot % cells_x as usize) as i64,
                (slot / cells_x as usize) as i64,
            );
            let slot = nearest_free_slot(col, row, cells_x, cells_y, &taken)
                .expect("a grid has more slots than pins");
            taken[slot] = true;
            points[slot] = site;
            site_cells.push(slot);
        }
        let boundary = grid.boundary();
        grid.set_points(points, &boundary);
        (grid, site_cells)
    }

    /// The boundary points of the lattice of the grid. A square lattice
    /// with the default jitter has one ring of points one step outside each
    /// edge. Other lattices have the lattice slots of two rings around the
    /// frame that are not inside it, as in FMG `getLatticeRing`.
    pub fn boundary(&self) -> Vec<[f64; 2]> {
        let (w, h) = (self.width, self.height);
        let (sx, sy) = (self.step_x, self.step_y);
        let (cx, cy) = (i64::from(self.cells_x), i64::from(self.cells_y));
        let mut ring = Vec::new();
        if self.lattice.kind == Lattice::Square {
            for col in -1..=cx {
                let x = (col as f64 + 0.5) * sx;
                ring.push([x, -sy]);
                ring.push([x, h + sy]);
            }
            for row in 0..cy {
                let y = (row as f64 + 0.5) * sy;
                ring.push([-sx, y]);
                ring.push([w + sx, y]);
            }
            return ring;
        }
        for row in -2..cy + 2 {
            let inside = row >= 0 && row < cy;
            let mut col = -2;
            while col < cx + 2 {
                let x = sx / 2.0 + col as f64 * sx + self.lattice.row_shift(row);
                let y = sy / 2.0 + row as f64 * sy + self.lattice.triangle_offset(row, col);
                if x <= 0.0 || y <= 0.0 || x >= w || y >= h {
                    ring.push([x, y]);
                }
                col = if inside && col == -1 { cx } else { col + 1 };
            }
        }
        ring
    }

    /// Make the cells of the given square lattice points.
    pub fn from_points(
        width: f64,
        height: f64,
        cells_x: u32,
        cells_y: u32,
        points: Vec<[f64; 2]>,
    ) -> Grid {
        let spacing = libm::sqrt(width * height / f64::from(cells_x * cells_y));
        let mut grid = Grid {
            width,
            height,
            cells_x,
            cells_y,
            step_x: width / f64::from(cells_x),
            step_y: height / f64::from(cells_y),
            lattice: LatticeInfo {
                kind: Lattice::Square,
                jitter: JITTER,
                spacing,
                row_offset: 0.0,
                triangle_shift: 0.0,
                neighbor_distance: spacing,
            },
            points: Vec::new(),
            edge_cells: Vec::new(),
            polygons: Vec::new(),
            vertices: Vec::new(),
            border: Vec::new(),
            packed: false,
            index: CellIndex::default(),
        };
        let boundary = grid.boundary();
        grid.set_points(points, &boundary);
        grid
    }

    /// Set the sites of the grid and make their Voronoi cells. The
    /// boundary points close the cells at the frame edge.
    pub fn set_points(&mut self, points: Vec<[f64; 2]>, boundary: &[[f64; 2]]) {
        let n = points.len();
        let all: Vec<DPoint> = points
            .iter()
            .chain(boundary)
            .map(|p| DPoint { x: p[0], y: p[1] })
            .collect();

        let t = triangulate(&all);
        let tri_count = t.triangles.len() / 3;
        let vertices: Vec<[f64; 2]> = (0..tri_count)
            .map(|i| {
                circumcenter(
                    &all[t.triangles[3 * i]],
                    &all[t.triangles[3 * i + 1]],
                    &all[t.triangles[3 * i + 2]],
                )
            })
            .collect();

        // One incoming half-edge for each point.
        let mut incoming = vec![EMPTY; all.len()];
        for e in 0..t.triangles.len() {
            let end = t.triangles[next_half_edge(e)];
            if incoming[end] == EMPTY || t.halfedges[e] == EMPTY {
                incoming[end] = e;
            }
        }

        let mut polygons = Vec::with_capacity(n);
        let mut edge_cells = Vec::with_capacity(n);
        let mut border = vec![false; n];
        for (p, start) in incoming.iter().take(n).enumerate() {
            let start = *start;
            assert!(start != EMPTY, "each real point is in a triangle");
            let mut around = Vec::new();
            let mut e = start;
            loop {
                around.push(e);
                let out = next_half_edge(e);
                e = t.halfedges[out];
                if e == EMPTY || e == start {
                    break;
                }
            }
            // A real point is inside the boundary ring, so its edges close.
            assert!(e == start, "the cell of point {p} is not closed");
            let k = around.len();
            let polygon: Vec<Id> = around.iter().map(|&e| (e / 3) as Id).collect();
            let neighbors: Vec<Id> = (0..k)
                .map(|i| {
                    let q = t.triangles[around[(i + 1) % k]];
                    if q < n {
                        q as Id
                    } else {
                        border[p] = true;
                        OUTSIDE
                    }
                })
                .collect();
            polygons.push(polygon);
            edge_cells.push(neighbors);
        }

        self.index = CellIndex::new(&points, self.width, self.height, self.lattice.spacing);
        self.points = points;
        self.edge_cells = edge_cells;
        self.polygons = polygons;
        self.vertices = vertices;
        self.border = border;
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The neighbor cells of a cell, without boundary points.
    pub fn neighbors(&self, cell: usize) -> impl Iterator<Item = usize> + '_ {
        self.edge_cells[cell]
            .iter()
            .filter(|&&c| c != OUTSIDE)
            .map(|&c| c as usize)
    }

    /// The lattice slot that contains a position. The slot is a cell index.
    /// In a second grid, it is the nearest cell.
    pub fn slot(&self, x: f64, y: f64) -> usize {
        if self.packed {
            return self.find_cell(x, y);
        }
        let row = ((y / self.step_y).floor() as i64).clamp(0, i64::from(self.cells_y) - 1);
        let col = (((x - self.lattice.row_shift(row)) / self.step_x).floor() as i64)
            .clamp(0, i64::from(self.cells_x) - 1);
        (row * i64::from(self.cells_x) + col) as usize
    }

    /// The cell whose site is nearest to a position.
    pub fn find_cell(&self, x: f64, y: f64) -> usize {
        self.index.nearest(&self.points, x, y)
    }

    /// The area of the polygon of a cell, in square units.
    pub fn area(&self, cell: usize) -> f64 {
        let poly = &self.polygons[cell];
        let k = poly.len();
        let mut sum = 0.0;
        for i in 0..k {
            let a = self.vertices[poly[i] as usize];
            let b = self.vertices[poly[(i + 1) % k] as usize];
            sum += a[0] * b[1] - b[0] * a[1];
        }
        (sum / 2.0).abs()
    }

    /// Make a second grid of the given sites, with the frame, the lattice,
    /// and the boundary points of this grid (FMG `Pack.generate`).
    pub fn packed(&self, points: Vec<[f64; 2]>) -> Grid {
        let mut g = Grid {
            points: Vec::new(),
            edge_cells: Vec::new(),
            polygons: Vec::new(),
            vertices: Vec::new(),
            border: Vec::new(),
            packed: true,
            index: CellIndex::default(),
            ..self.clone()
        };
        let boundary = self.boundary();
        g.set_points(points, &boundary);
        g
    }
}

/// The free slot nearest to `(col, row)`: first by ring, then by row, then
/// by column, so the choice is the same on each run.
fn nearest_free_slot(
    col: i64,
    row: i64,
    cells_x: u32,
    cells_y: u32,
    taken: &[bool],
) -> Option<usize> {
    let (cx, cy) = (i64::from(cells_x), i64::from(cells_y));
    for ring in 0..cx.max(cy) {
        for dy in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs().max(dy.abs()) != ring {
                    continue;
                }
                let (c, r) = (col + dx, row + dy);
                if c < 0 || r < 0 || c >= cx || r >= cy {
                    continue;
                }
                let slot = (r * cx + c) as usize;
                if !taken[slot] {
                    return Some(slot);
                }
            }
        }
    }
    None
}

fn next_half_edge(e: usize) -> usize {
    if e % 3 == 2 { e - 2 } else { e + 1 }
}

fn circumcenter(a: &DPoint, b: &DPoint, c: &DPoint) -> [f64; 2] {
    let (bx, by) = (b.x - a.x, b.y - a.y);
    let (cx, cy) = (c.x - a.x, c.y - a.y);
    let bl = bx * bx + by * by;
    let cl = cx * cx + cy * cy;
    let d = 0.5 / (bx * cy - by * cx);
    [a.x + (cy * bl - by * cl) * d, a.y + (bx * cl - cx * bl) * d]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::{Layer, SeedTree};

    fn grid(cells: u32) -> Grid {
        let mut rng = Rand::new(SeedTree::new(7).part_rng(Layer::Terrain, "grid"));
        Grid::new(400.0, 300.0, cells, &mut rng)
    }

    #[test]
    fn grid_has_about_the_wanted_cell_count() {
        let g = grid(10_000);
        let n = g.len() as f64;
        assert!((n - 10_000.0).abs() / 10_000.0 < 0.02, "{n}");
        assert_eq!(g.len(), (g.cells_x * g.cells_y) as usize);
    }

    #[test]
    fn points_are_inside_the_frame() {
        let g = grid(2000);
        for p in &g.points {
            assert!(p[0] > 0.0 && p[0] < 400.0 && p[1] > 0.0 && p[1] < 300.0);
        }
    }

    #[test]
    fn neighbors_are_symmetric() {
        let g = grid(2000);
        for c in 0..g.len() {
            for n in g.neighbors(c) {
                assert!(g.neighbors(n).any(|m| m == c), "{c} -> {n}");
            }
        }
    }

    #[test]
    fn each_cell_has_one_polygon_edge_for_each_neighbor() {
        let g = grid(2000);
        for c in 0..g.len() {
            assert_eq!(g.polygons[c].len(), g.edge_cells[c].len());
            assert!(g.polygons[c].len() >= 3);
        }
    }

    #[test]
    fn border_cells_are_near_the_frame_edge() {
        let g = grid(2000);
        for c in 0..g.len() {
            let col = c as u32 % g.cells_x;
            let row = c as u32 / g.cells_x;
            let near = col <= 1 || row <= 1 || col + 2 >= g.cells_x || row + 2 >= g.cells_y;
            if g.border[c] {
                assert!(near, "cell {c} at column {col}, row {row} is a border cell");
            }
        }
        // The border cells go around the full frame: there are at least as
        // many as the lattice slots on the longer two sides.
        let count = g.border.iter().filter(|&&b| b).count() as u32;
        assert!(count >= g.cells_x + g.cells_y, "{count}");
    }

    /// An edge next to a boundary point stays outside the frame, or inside
    /// it by less than half a lattice step. Thus the real cells cover the
    /// frame except for a thin band at the edge.
    #[test]
    fn edges_next_to_boundary_points_are_at_the_frame_edge() {
        let g = grid(2000);
        let margin_x = g.step_x / 2.0;
        let margin_y = g.step_y / 2.0;
        for c in 0..g.len() {
            let poly = &g.polygons[c];
            for (k, &other) in g.edge_cells[c].iter().enumerate() {
                if other != OUTSIDE {
                    continue;
                }
                let a = g.vertices[poly[k] as usize];
                let b = g.vertices[poly[(k + 1) % poly.len()] as usize];
                let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                let deep = mid[0] > margin_x
                    && mid[0] < 400.0 - margin_x
                    && mid[1] > margin_y
                    && mid[1] < 300.0 - margin_y;
                assert!(
                    !deep,
                    "cell {c} has a boundary edge deep inside the frame at {mid:?}"
                );
            }
        }
    }

    #[test]
    fn find_cell_gives_the_nearest_site() {
        let g = grid(2000);
        for &(x, y) in &[(10.0, 10.0), (200.0, 150.0), (399.0, 299.0), (123.4, 56.7)] {
            let found = g.find_cell(x, y);
            let d = |c: usize| (g.points[c][0] - x).powi(2) + (g.points[c][1] - y).powi(2);
            let best = (0..g.len()).min_by(|&a, &b| d(a).total_cmp(&d(b))).unwrap();
            assert_eq!(d(found), d(best));
        }
    }

    #[test]
    fn sites_are_exact_cell_sites() {
        let sites = [[120.0, 88.0], [120.5, 88.5], [0.0, 0.0], [400.0, 150.0]];
        let mut rng = Rand::new(SeedTree::new(7).part_rng(Layer::Terrain, "grid"));
        let (g, cells) = Grid::with_sites(400.0, 300.0, 2000, &sites, &mut rng);
        assert_eq!(cells.len(), sites.len());
        for (site, &cell) in sites.iter().zip(&cells) {
            assert_eq!(g.points[cell], *site);
            assert_eq!(g.find_cell(site[0], site[1]), cell);
        }
        // Two sites in one slot get two cells.
        assert_ne!(cells[0], cells[1]);
    }

    #[test]
    fn a_site_does_not_move_far_cells() {
        let plain = grid(2000);
        let mut rng = Rand::new(SeedTree::new(7).part_rng(Layer::Terrain, "grid"));
        let (with, cells) = Grid::with_sites(400.0, 300.0, 2000, &[[200.0, 150.0]], &mut rng);
        let changed: Vec<usize> = (0..plain.len())
            .filter(|&c| plain.points[c] != with.points[c])
            .collect();
        assert_eq!(changed, vec![cells[0]]);
    }

    #[test]
    fn same_seed_gives_the_same_grid() {
        assert_eq!(grid(2000), grid(2000));
    }

    const LATTICES: [Lattice; 5] = [
        Lattice::Square,
        Lattice::HexPointy,
        Lattice::HexFlat,
        Lattice::Diamond,
        Lattice::Triangle,
    ];

    fn lattice_grid(kind: Lattice, jitter: f64, sites: &[[f64; 2]]) -> (Grid, Vec<usize>) {
        let mut rng = Rand::new(SeedTree::new(7).part_rng(Layer::Terrain, "grid"));
        Grid::generate(400.0, 300.0, 3000, kind, jitter, sites, &mut rng)
    }

    #[test]
    fn each_lattice_gives_closed_cells_near_the_wanted_count() {
        for kind in LATTICES {
            let (g, _) = lattice_grid(kind, JITTER, &[]);
            let n = g.len() as f64;
            assert!((n - 3000.0).abs() / 3000.0 < 0.05, "{kind:?}: {n}");
            assert_eq!(g.len(), (g.cells_x * g.cells_y) as usize);
            // Each slot covers the area of one cell.
            let area = g.step_x * g.step_y;
            assert!(
                (area - g.lattice.spacing.powi(2)).abs() < 0.05 * area,
                "{kind:?}"
            );
            for c in 0..g.len() {
                let [x, y] = g.points[c];
                assert!((0.0..=400.0).contains(&x) && (0.0..=300.0).contains(&y));
                assert!(g.polygons[c].len() >= 3, "{kind:?} cell {c}");
                for m in g.neighbors(c) {
                    assert!(g.neighbors(m).any(|k| k == c), "{kind:?} {c} -> {m}");
                }
            }
        }
    }

    #[test]
    fn a_regular_lattice_has_its_form() {
        // Without jitter, an inner cell of a hexagon lattice has 6
        // neighbors. (The points of the other lattices are on common
        // circles, so their Delaunay edges are not unique.)
        for (kind, want) in [(Lattice::HexPointy, 6), (Lattice::HexFlat, 6)] {
            let (g, _) = lattice_grid(kind, 0.0, &[]);
            let c = g.slot(200.0, 150.0);
            assert_eq!(g.neighbors(c).count(), want, "{kind:?}");
        }
    }

    #[test]
    fn the_slot_of_a_cell_site_is_near_the_cell() {
        for kind in LATTICES {
            let (g, _) = lattice_grid(kind, JITTER, &[]);
            for c in (0..g.len()).step_by(7) {
                let [x, y] = g.points[c];
                let s = g.slot(x, y);
                let d = ((g.points[s][0] - x).powi(2) + (g.points[s][1] - y).powi(2)).sqrt();
                assert!(
                    d <= 2.5 * g.lattice.neighbor_distance,
                    "{kind:?} cell {c}: {d}"
                );
            }
        }
    }

    #[test]
    fn sites_are_exact_on_each_lattice() {
        let sites = [[120.0, 88.0], [120.5, 88.5], [0.0, 0.0], [400.0, 150.0]];
        for kind in LATTICES {
            let (g, cells) = lattice_grid(kind, JITTER, &sites);
            for (site, &cell) in sites.iter().zip(&cells) {
                assert_eq!(g.points[cell], *site, "{kind:?}");
                assert_eq!(g.find_cell(site[0], site[1]), cell, "{kind:?}");
            }
        }
    }

    #[test]
    fn the_square_lattice_keeps_the_old_points() {
        // The square lattice with the default jitter is the grid of `new`.
        let (g, _) = lattice_grid(Lattice::Square, JITTER, &[]);
        let mut rng = Rand::new(SeedTree::new(7).part_rng(Layer::Terrain, "grid"));
        assert_eq!(g, Grid::new(400.0, 300.0, 3000, &mut rng));
    }
}
