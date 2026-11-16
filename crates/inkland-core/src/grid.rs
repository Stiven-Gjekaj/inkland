//! The grid: a jittered lattice of points and its Voronoi cells.
//!
//! The points are in rows, from the low y edge to the high y edge, and from
//! the low x edge to the high x edge in each row. Thus `slot(x, y)` finds
//! the cell near a position at once.
//!
//! A ring of boundary points outside the frame closes each cell. The edges
//! between a real cell and a boundary point are outside the frame.

use crate::rand::Rand;
use delaunator::{EMPTY, Point as DPoint, triangulate};

/// A cell or a vertex index.
pub type Id = u32;

/// Mark for a polygon edge that is next to a boundary point, not a cell.
pub const OUTSIDE: Id = Id::MAX;

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
}

/// How far each point can move from its lattice position, as a fraction of
/// the lattice step.
pub const JITTER: f64 = 0.9;

impl Grid {
    /// Make a grid of about `cells` cells on a `width` x `height` frame.
    pub fn new(width: f64, height: f64, cells: u32, rng: &mut Rand) -> Grid {
        Self::with_sites(width, height, cells, &[], rng).0
    }

    /// Make a grid in which each of `sites` is exactly the site of a cell.
    /// Return the grid and the cell of each site.
    ///
    /// A site takes the lattice slot that contains it. If an earlier site
    /// has that slot, the site takes the nearest free slot. The sites must
    /// be different from each other. The random numbers do not depend on the
    /// sites, so a new site does not move the other cells.
    pub fn with_sites(
        width: f64,
        height: f64,
        cells: u32,
        sites: &[[f64; 2]],
        rng: &mut Rand,
    ) -> (Grid, Vec<usize>) {
        let spacing = (width * height / f64::from(cells.max(1))).sqrt();
        let cells_x = ((width / spacing).round() as u32).max(2);
        let cells_y = ((height / spacing).round() as u32).max(2);
        let step_x = width / f64::from(cells_x);
        let step_y = height / f64::from(cells_y);

        let mut points = Vec::with_capacity((cells_x * cells_y) as usize);
        for row in 0..cells_y {
            for col in 0..cells_x {
                let jx = (rng.unit() - 0.5) * JITTER * step_x;
                let jy = (rng.unit() - 0.5) * JITTER * step_y;
                points.push([
                    (f64::from(col) + 0.5) * step_x + jx,
                    (f64::from(row) + 0.5) * step_y + jy,
                ]);
            }
        }
        let slot_of = |x: f64, y: f64| -> (i64, i64) {
            let col = ((x / step_x).floor() as i64).clamp(0, i64::from(cells_x) - 1);
            let row = ((y / step_y).floor() as i64).clamp(0, i64::from(cells_y) - 1);
            (col, row)
        };
        let mut taken = vec![false; points.len()];
        let mut site_cells = Vec::with_capacity(sites.len());
        for &site in sites {
            let (col, row) = slot_of(site[0], site[1]);
            let slot = nearest_free_slot(col, row, cells_x, cells_y, &taken)
                .expect("a grid has more slots than pins");
            taken[slot] = true;
            points[slot] = site;
            site_cells.push(slot);
        }
        (
            Self::from_points(width, height, cells_x, cells_y, points),
            site_cells,
        )
    }

    /// Make the cells of the given lattice points.
    pub fn from_points(
        width: f64,
        height: f64,
        cells_x: u32,
        cells_y: u32,
        points: Vec<[f64; 2]>,
    ) -> Grid {
        let step_x = width / f64::from(cells_x);
        let step_y = height / f64::from(cells_y);
        let n = points.len();

        // Boundary points: one lattice step outside each edge.
        let mut all: Vec<DPoint> = points.iter().map(|p| DPoint { x: p[0], y: p[1] }).collect();
        for col in -1..=(cells_x as i64) {
            let x = (col as f64 + 0.5) * step_x;
            all.push(DPoint { x, y: -step_y });
            all.push(DPoint {
                x,
                y: height + step_y,
            });
        }
        for row in 0..(cells_y as i64) {
            let y = (row as f64 + 0.5) * step_y;
            all.push(DPoint { x: -step_x, y });
            all.push(DPoint {
                x: width + step_x,
                y,
            });
        }

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

        Grid {
            width,
            height,
            cells_x,
            cells_y,
            step_x,
            step_y,
            points,
            edge_cells,
            polygons,
            vertices,
            border,
        }
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
    pub fn slot(&self, x: f64, y: f64) -> usize {
        let col = ((x / self.step_x).floor() as i64).clamp(0, i64::from(self.cells_x) - 1);
        let row = ((y / self.step_y).floor() as i64).clamp(0, i64::from(self.cells_y) - 1);
        (row * i64::from(self.cells_x) + col) as usize
    }

    /// The cell whose site is nearest to a position.
    pub fn find_cell(&self, x: f64, y: f64) -> usize {
        let mut best = self.slot(x, y);
        let d = |c: usize| {
            let p = self.points[c];
            (p[0] - x) * (p[0] - x) + (p[1] - y) * (p[1] - y)
        };
        loop {
            let next = self
                .neighbors(best)
                .fold(best, |b, c| if d(c) < d(b) { c } else { b });
            if next == best {
                return best;
            }
            best = next;
        }
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
}
