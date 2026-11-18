//! Engraved height hachures: tapered strokes down the fall line, seeded
//! along height levels, and denser where the ground is steep
//! (`heightmap-hachures.ts`, `getHeightContourChains`).

use super::super::geom::n;
use inkland_core::rand::Rand;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The surface and the look of the hachures.
pub struct Params<'a> {
    /// The cell sites, in page pixels.
    pub points: &'a [[f64; 2]],
    /// The height of each cell, on the scale of FMG (sea level is 20).
    pub heights: &'a [f64],
    pub neighbors: &'a [Vec<usize>],
    /// Three cells for each inner Voronoi vertex.
    pub triangles: &'a [[usize; 3]],
    /// The cell spacing, in page pixels.
    pub spacing: f64,
    /// The cell at a page point, or `None` off the map.
    pub find_cell: &'a dyn Fn(f64, f64) -> Option<usize>,
    /// Land cells or sea cells.
    pub in_band: &'a dyn Fn(f64) -> bool,
    /// The levels that strokes start along.
    pub thresholds: &'a [f64],
    pub density: f64,
    pub length: f64,
    pub width: f64,
    /// One page pixel in map units of FMG.
    pub unit: f64,
}

const STEP: f64 = 0.3;
const ROW_GAP: f64 = 0.2;
const LENGTH: f64 = 0.5;
const WIDTH: f64 = 0.2;
const ROW_SCATTER: f64 = 0.45;
const ROOT_JITTER: f64 = 0.06;
const ANGLE_JITTER: f64 = 0.05;
const MIN_LENGTH: f64 = 0.15;
const FADE_STEPS: u32 = 2;
const MIN_SLOPE: f64 = 1.8;
const FULL_SLOPE: f64 = 4.5;
const MIN_LEVEL_DENSITY: f64 = 0.12;
const MAX_LEVEL_DENSITY: f64 = 0.9;
const LEVEL_POWER: f64 = 1.7;

/// The two cells of a triangle edge.
type EdgeKey = (usize, usize);

/// One contour line at a height: its points, and whether it closes.
#[derive(Debug, Clone, PartialEq)]
pub struct Chain {
    pub height: f64,
    pub closed: bool,
    pub points: Vec<[f64; 2]>,
}

/// The mean of each height and the mean of its neighbors
/// (`smoothContourHeights`).
pub fn smooth(heights: &[f64], neighbors: &[Vec<usize>]) -> Vec<f64> {
    heights
        .iter()
        .zip(neighbors)
        .map(|(&h, adjacent)| {
            if adjacent.is_empty() {
                h
            } else {
                let mean =
                    adjacent.iter().map(|&c| heights[c]).sum::<f64>() / adjacent.len() as f64;
                (h + mean) / 2.0
            }
        })
        .collect()
}

/// The least-squares slope of the heights at each cell
/// (`getSlopeGradients`).
pub fn gradients(points: &[[f64; 2]], heights: &[f64], neighbors: &[Vec<usize>]) -> Vec<[f64; 2]> {
    neighbors
        .iter()
        .enumerate()
        .map(|(i, adjacent)| {
            let [x, y] = points[i];
            let (mut sxx, mut syy, mut sxy, mut sxh, mut syh) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for &c in adjacent {
                let (dx, dy, dh) = (points[c][0] - x, points[c][1] - y, heights[c] - heights[i]);
                sxx += dx * dx;
                syy += dy * dy;
                sxy += dx * dy;
                sxh += dx * dh;
                syh += dy * dh;
            }
            let det = sxx * syy - sxy * sxy;
            if det.abs() < 1e-9 {
                [0.0, 0.0]
            } else {
                [(sxh * syy - syh * sxy) / det, (syh * sxx - sxh * sxy) / det]
            }
        })
        .collect()
}

/// The contour lines of the heights at each threshold, by a march over the
/// triangles (`getHeightContourChains`).
pub fn contour_chains(
    points: &[[f64; 2]],
    heights: &[f64],
    triangles: &[[usize; 3]],
    thresholds: &[f64],
) -> Vec<Chain> {
    let mut out = Vec::new();
    for &height in thresholds {
        // The crossing on each edge, by its two cells, and its two
        // neighbor crossings. A BTreeMap gives the same order each time.
        let mut crossings: BTreeMap<EdgeKey, ([f64; 2], Vec<EdgeKey>)> = BTreeMap::new();
        let mut order: Vec<(usize, usize)> = Vec::new();
        for t in triangles {
            let above = heights[t[0]] >= height;
            if above == (heights[t[1]] >= height) && above == (heights[t[2]] >= height) {
                continue;
            }
            let mut edges = Vec::new();
            for i in 0..3 {
                let (a, b) = (t[i], t[(i + 1) % 3]);
                if (heights[a] >= height) == (heights[b] >= height) {
                    continue;
                }
                let key = (a.min(b), a.max(b));
                crossings.entry(key).or_insert_with(|| {
                    order.push(key);
                    let r = (height - heights[a]) / (heights[b] - heights[a]);
                    let p = [
                        points[a][0] + (points[b][0] - points[a][0]) * r,
                        points[a][1] + (points[b][1] - points[a][1]) * r,
                    ];
                    (p, Vec::new())
                });
                edges.push(key);
            }
            if edges.len() != 2 {
                continue;
            }
            if let Some(c) = crossings.get_mut(&edges[0]) {
                c.1.push(edges[1]);
            }
            if let Some(c) = crossings.get_mut(&edges[1]) {
                c.1.push(edges[0]);
            }
        }
        let mut visited = std::collections::BTreeSet::new();
        let mut trace =
            |start: (usize, usize), visited: &mut std::collections::BTreeSet<(usize, usize)>| {
                let mut chain = Vec::new();
                let mut closed = false;
                let mut current = Some(start);
                while let Some(key) = current {
                    if !visited.insert(key) {
                        break;
                    }
                    let (p, next) = &crossings[&key];
                    chain.push(*p);
                    current = next.iter().copied().find(|k| !visited.contains(k));
                    closed = current.is_none() && chain.len() > 2 && next.contains(&start);
                }
                if chain.len() >= 2 {
                    out.push(Chain {
                        height,
                        closed,
                        points: chain,
                    });
                }
            };
        // Open chains start at an end, so that no chain breaks in two.
        for key in &order {
            if crossings[key].1.len() == 1 && !visited.contains(key) {
                trace(*key, &mut visited);
            }
        }
        for key in &order {
            if !visited.contains(key) {
                trace(*key, &mut visited);
            }
        }
    }
    out
}

/// The hachures as one path to fill (`getHachures`).
pub fn hachures(p: &Params, rng: &mut Rand) -> String {
    let grads = gradients(p.points, p.heights, p.neighbors);
    let weight_of = |slope: f64| ((slope - MIN_SLOPE) / (FULL_SLOPE - MIN_SLOPE)).clamp(0.0, 1.0);
    let band: Vec<f64> = p
        .heights
        .iter()
        .copied()
        .filter(|&h| (p.in_band)(h))
        .collect();
    let Some(lowest) = band.iter().copied().reduce(f64::min) else {
        return String::new();
    };
    let highest = band.iter().copied().fold(lowest, f64::max);
    let level_of = |h: f64| {
        if highest == lowest {
            1.0
        } else {
            (h - lowest) / (highest - lowest)
        }
    };
    // An inverse distance blend of the slopes around a point: the fall line
    // with no bends at the cell edges.
    let blend = |x: f64, y: f64, cell: usize| -> [f64; 2] {
        let w0 = 1.0 / (0.01 + (p.points[cell][0] - x).powi(2) + (p.points[cell][1] - y).powi(2));
        let mut sum = w0;
        let mut g = [grads[cell][0] * w0, grads[cell][1] * w0];
        for &j in &p.neighbors[cell] {
            let w = 1.0 / (0.01 + (p.points[j][0] - x).powi(2) + (p.points[j][1] - y).powi(2));
            g[0] += grads[j][0] * w;
            g[1] += grads[j][1] * w;
            sum += w;
        }
        [g[0] / sum, g[1] / sum]
    };
    let cell_at = |x: f64, y: f64| (p.find_cell)(x, y).filter(|&c| (p.in_band)(p.heights[c]));
    let step = p.spacing * STEP;
    let gap = p.spacing * ROW_GAP / p.density.max(0.01);
    let mut d = String::new();

    let mut trace = |mut x: f64, mut y: f64, rng: &mut Rand| -> f64 {
        let Some(cell) = cell_at(x, y) else {
            return 0.0;
        };
        let g = blend(x, y, cell);
        let weight = weight_of(libm::hypot(g[0], g[1]) * p.spacing);
        if weight == 0.0 {
            return 0.0;
        }
        let angle = libm::atan2(-g[1], -g[0]) + (rng.unit() - 0.5) * ANGLE_JITTER * (0.5 + weight);
        let (dx, dy) = (libm::cos(angle), libm::sin(angle));
        let slide = rng.unit() * ROW_SCATTER * p.spacing;
        let lateral = (rng.unit() - 0.5) * ROOT_JITTER * p.spacing;
        x += dx * slide - dy * lateral;
        y += dy * slide + dx * lateral;
        // The stroke runs down the fall line while the ground is steep.
        let longest = LENGTH * p.length * p.spacing * (0.5 + 0.5 * weight);
        let wanted = longest * (MIN_LENGTH + (1.0 - MIN_LENGTH) * libm::pow(rng.unit(), 1.5));
        let mut run = 0.0;
        let mut fading = 0;
        while run < wanted {
            let (px, py) = (x + dx * (run + step), y + dy * (run + step));
            let Some(here) = cell_at(px, py) else {
                break;
            };
            let g = blend(px, py, here);
            if libm::hypot(g[0], g[1]) * p.spacing < MIN_SLOPE {
                fading += 1;
                if fading > FADE_STEPS {
                    break;
                }
            }
            run += step;
        }
        if run < step {
            return weight;
        }
        let root = WIDTH * p.unit * p.width * (0.4 + 0.6 * weight) * (0.75 + rng.unit() * 0.5);
        let len = run.min(wanted);
        taper(&mut d, x, y, dx * len, dy * len, root);
        weight
    };

    for chain in contour_chains(p.points, p.heights, p.triangles, p.thresholds) {
        let level = level_of(chain.height).clamp(0.0, 1.0);
        let density = MIN_LEVEL_DENSITY
            + (MAX_LEVEL_DENSITY - MIN_LEVEL_DENSITY) * libm::pow(level, LEVEL_POWER);
        if rng.unit() > density {
            continue;
        }
        let mut row = chain.points.clone();
        if chain.closed {
            row.push(chain.points[0]);
        }
        let mut until_next = rng.unit() * gap;
        for w in row.windows(2) {
            let ([x0, y0], [x1, y1]) = (w[0], w[1]);
            let segment = libm::hypot(x1 - x0, y1 - y0);
            let mut along = until_next;
            while along < segment {
                let t = along / segment;
                let weight = trace(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, rng);
                // Steep ground packs the strokes.
                along += gap * (1.6 - weight) * (0.65 + rng.unit() * 0.7);
            }
            until_next = along - segment;
        }
    }
    d
}

/// The outline of a straight stroke: `root` wide at (x, y), and a sharp
/// point at (x + dx, y + dy).
fn taper(d: &mut String, x: f64, y: f64, dx: f64, dy: f64, root: f64) {
    let len = libm::hypot(dx, dy);
    let len = if len == 0.0 { 1.0 } else { len };
    let (ax, ay) = (-dy / len, dx / len);
    let _ = write!(
        d,
        "M{},{}l{},{}l{},{}Z",
        n(x + ax * root / 2.0),
        n(y + ay * root / 2.0),
        n(dx - ax * root / 2.0),
        n(dy - ay * root / 2.0),
        n(-dx - ax * root / 2.0),
        n(-dy - ay * root / 2.0)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square lattice of `k` x `k` points, 10 apart, with two triangles
    /// in each square.
    type Surface = (Vec<[f64; 2]>, Vec<Vec<usize>>, Vec<[usize; 3]>);

    fn lattice(k: usize) -> Surface {
        let id = |x: usize, y: usize| y * k + x;
        let mut points = Vec::new();
        let mut neighbors = Vec::new();
        let mut triangles = Vec::new();
        for y in 0..k {
            for x in 0..k {
                points.push([x as f64 * 10.0, y as f64 * 10.0]);
                let mut adj = Vec::new();
                if x > 0 {
                    adj.push(id(x - 1, y));
                }
                if x + 1 < k {
                    adj.push(id(x + 1, y));
                }
                if y > 0 {
                    adj.push(id(x, y - 1));
                }
                if y + 1 < k {
                    adj.push(id(x, y + 1));
                }
                neighbors.push(adj);
                if x + 1 < k && y + 1 < k {
                    triangles.push([id(x, y), id(x + 1, y), id(x, y + 1)]);
                    triangles.push([id(x + 1, y), id(x + 1, y + 1), id(x, y + 1)]);
                }
            }
        }
        (points, neighbors, triangles)
    }

    fn rng() -> Rand {
        use inkland_core::seed::{Layer, SeedTree};
        Rand::new(SeedTree::new(3).part_rng(Layer::Decoration, "hachures"))
    }

    #[test]
    fn a_contour_of_a_ramp_is_one_open_line_across_it() {
        let (points, _, triangles) = lattice(5);
        // The height grows 10 for each column.
        let heights: Vec<f64> = points.iter().map(|p| p[0]).collect();
        let chains = contour_chains(&points, &heights, &triangles, &[15.0]);
        assert_eq!(chains.len(), 1);
        let c = &chains[0];
        assert!(!c.closed);
        assert!(c.points.iter().all(|p| (p[0] - 15.0).abs() < 1e-9), "{c:?}");
        assert_eq!(c.points.len(), 5 + 4);
    }

    #[test]
    fn a_contour_around_a_peak_closes() {
        let (points, _, triangles) = lattice(5);
        let heights: Vec<f64> = (0..25).map(|i| if i == 12 { 50.0 } else { 0.0 }).collect();
        let chains = contour_chains(&points, &heights, &triangles, &[25.0]);
        assert_eq!(chains.len(), 1);
        assert!(chains[0].closed, "{:?}", chains[0]);
    }

    #[test]
    fn steep_ground_gets_strokes_that_point_down_the_slope() {
        let k = 20;
        let (points, neighbors, triangles) = lattice(k);
        // A slope of 4 for each 10 units, down to the east.
        let heights: Vec<f64> = points.iter().map(|p| 100.0 - p[0] * 0.4).collect();
        let find = |x: f64, y: f64| {
            let (i, j) = ((x / 10.0).round(), (y / 10.0).round());
            (i >= 0.0 && j >= 0.0 && i < k as f64 && j < k as f64)
                .then(|| j as usize * k + i as usize)
        };
        let thresholds: Vec<f64> = (6..25).map(|v| f64::from(v) * 4.0).collect();
        let params = Params {
            points: &points,
            heights: &heights,
            neighbors: &neighbors,
            triangles: &triangles,
            spacing: 10.0,
            find_cell: &find,
            in_band: &|h| h >= 20.0,
            thresholds: &thresholds,
            density: 1.0,
            length: 1.0,
            width: 1.0,
            unit: 1.0,
        };
        let d = hachures(&params, &mut rng());
        let strokes: Vec<&str> = d.split('M').skip(1).collect();
        assert!(strokes.len() > 20, "{}", strokes.len());
        for s in strokes {
            // The first `l` goes from the root to the point: east.
            let l = s.split('l').nth(1).unwrap();
            let dx: f64 = l.split(',').next().unwrap().parse().unwrap();
            assert!(dx > 0.0, "{s}");
        }
        // Flat ground has no strokes.
        let flat = vec![50.0; points.len()];
        let params = Params {
            heights: &flat,
            ..params
        };
        assert!(hachures(&params, &mut rng()).is_empty());
    }
}
