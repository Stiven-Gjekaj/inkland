//! The heightmap: a height for each grid cell, on the scale 0 to 100.
//!
//! A template is a list of steps. Each step changes the heights:
//!
//! - `Hill N H X Y`: add N blobs of height H. X and Y are the start area,
//!   in percent of the frame width and height.
//! - `Pit N H X Y`: dig N blobs of depth H. A pit starts on land.
//! - `Range N H X Y`: add N ridges. A ridge goes from a start in the area to
//!   a random end, and the height falls off on both sides.
//! - `Trough N H X Y`: dig N valleys, as `Range` but down.
//! - `Strait W vertical|horizontal`: cut a channel of width W across the map.
//! - `Mask P`: lower the heights near the frame edges (P < 0: near the
//!   center). A larger P is a weaker effect.
//! - `Add V SPAN` and `Multiply F SPAN`: change the heights in a span:
//!   `all`, `land`, or `A-B`.
//! - `Smooth F`: blend each height with its neighbors. A larger F is a
//!   weaker effect.
//! - `Invert P x|y|both`: with probability P, mirror the map.
//!
//! A number such as `1.5` is 1, plus 1 with probability 0.5. A range such as
//! `5-7` is a random integer from 5 to 7.

pub mod templates;

use crate::grid::Grid;
use crate::rand::Rand;
use std::collections::VecDeque;

/// A count or a height: a fixed number, or a range of integers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Amount {
    Fixed(f64),
    Range(f64, f64),
}

impl Amount {
    fn parse(text: &str) -> Result<Amount, String> {
        if let Ok(v) = text.parse::<f64>() {
            return Ok(Amount::Fixed(v));
        }
        let (a, b) = text
            .split_once('-')
            .ok_or_else(|| format!("{text:?} is not a number or a range A-B"))?;
        let a: f64 = a
            .parse()
            .map_err(|_| format!("{text:?} is not a range A-B"))?;
        let b: f64 = b
            .parse()
            .map_err(|_| format!("{text:?} is not a range A-B"))?;
        if a > b {
            return Err(format!(
                "{text:?}: the start of the range is more than its end"
            ));
        }
        Ok(Amount::Range(a, b))
    }

    /// A count: the whole part, and 1 more with the probability of the
    /// fraction. A range gives an integer in the range.
    fn count(self, rng: &mut Rand) -> u32 {
        match self {
            Amount::Fixed(v) => {
                let whole = v.trunc();
                let extra = u32::from(rng.chance(v - whole));
                whole.max(0.0) as u32 + extra
            }
            Amount::Range(a, b) => rng.int(a as i64, b as i64).max(0) as u32,
        }
    }

    /// A height value.
    fn value(self, rng: &mut Rand) -> f64 {
        match self {
            Amount::Fixed(v) => v,
            Amount::Range(a, b) => rng.int(a as i64, b as i64) as f64,
        }
    }
}

/// A part of the frame side, in fractions from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span(pub f64, pub f64);

impl Span {
    fn parse_percent(text: &str) -> Result<Span, String> {
        let (a, b) = match text.split_once('-') {
            Some((a, b)) => (a, b),
            None => (text, text),
        };
        let a: f64 = a
            .parse()
            .map_err(|_| format!("{text:?} is not a percent range A-B"))?;
        let b: f64 = b
            .parse()
            .map_err(|_| format!("{text:?} is not a percent range A-B"))?;
        if !(0.0..=100.0).contains(&a) || !(0.0..=100.0).contains(&b) || a > b {
            return Err(format!("{text:?} must be a range in 0 to 100"));
        }
        Ok(Span(a / 100.0, b / 100.0))
    }

    fn pick(self, length: f64, rng: &mut Rand) -> f64 {
        rng.uniform(self.0 * length, self.1 * length)
    }
}

/// The heights that `Add` and `Multiply` change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Band {
    All,
    /// Heights at sea level and above. A change keeps land at or above sea
    /// level.
    Land,
    Between(f64, f64),
}

impl Band {
    fn parse(text: &str) -> Result<Band, String> {
        match text {
            "all" => Ok(Band::All),
            "land" => Ok(Band::Land),
            _ => {
                let (a, b) = text
                    .split_once('-')
                    .ok_or_else(|| format!("{text:?} is not all, land, or A-B"))?;
                let a: f64 = a
                    .parse()
                    .map_err(|_| format!("{text:?} is not a range A-B"))?;
                let b: f64 = b
                    .parse()
                    .map_err(|_| format!("{text:?} is not a range A-B"))?;
                Ok(Band::Between(a, b))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Axes {
    X,
    Y,
    Both,
}

/// One template step.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Hill {
        count: Amount,
        height: Amount,
        x: Span,
        y: Span,
    },
    Pit {
        count: Amount,
        height: Amount,
        x: Span,
        y: Span,
    },
    Range {
        count: Amount,
        height: Amount,
        x: Span,
        y: Span,
    },
    Trough {
        count: Amount,
        height: Amount,
        x: Span,
        y: Span,
    },
    Strait {
        width: Amount,
        vertical: bool,
    },
    Mask {
        power: f64,
    },
    Add {
        value: f64,
        band: Band,
    },
    Multiply {
        factor: f64,
        band: Band,
    },
    Smooth {
        factor: f64,
    },
    Invert {
        chance: f64,
        axes: Axes,
    },
}

/// Read a template: one step in each line. Empty lines are ignored.
pub fn parse_steps<'a>(lines: impl IntoIterator<Item = &'a str>) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for (n, line) in lines.into_iter().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        steps.push(parse_step(line).map_err(|e| format!("step {}: {e}", n + 1))?);
    }
    Ok(steps)
}

fn parse_step(line: &str) -> Result<Step, String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let arg = |i: usize| -> Result<&str, String> {
        words
            .get(i)
            .copied()
            .ok_or_else(|| format!("{:?} needs more values", words[0]))
    };
    let num = |i: usize| -> Result<f64, String> {
        let t = arg(i)?;
        t.parse()
            .map_err(|_| format!("{t:?} is not a number"))
            .and_then(|v: f64| {
                if v.is_finite() {
                    Ok(v)
                } else {
                    Err(format!("{t:?} is not finite"))
                }
            })
    };
    let blob = |make: fn(Amount, Amount, Span, Span) -> Step| -> Result<Step, String> {
        Ok(make(
            Amount::parse(arg(1)?)?,
            Amount::parse(arg(2)?)?,
            Span::parse_percent(arg(3)?)?,
            Span::parse_percent(arg(4)?)?,
        ))
    };
    match words.first().copied() {
        Some("Hill") => blob(|count, height, x, y| Step::Hill {
            count,
            height,
            x,
            y,
        }),
        Some("Pit") => blob(|count, height, x, y| Step::Pit {
            count,
            height,
            x,
            y,
        }),
        Some("Range") => blob(|count, height, x, y| Step::Range {
            count,
            height,
            x,
            y,
        }),
        Some("Trough") => blob(|count, height, x, y| Step::Trough {
            count,
            height,
            x,
            y,
        }),
        Some("Strait") => Ok(Step::Strait {
            width: Amount::parse(arg(1)?)?,
            vertical: match arg(2)? {
                "vertical" => true,
                "horizontal" => false,
                other => return Err(format!("{other:?} is not vertical or horizontal")),
            },
        }),
        Some("Mask") => Ok(Step::Mask { power: num(1)? }),
        Some("Add") => Ok(Step::Add {
            value: num(1)?,
            band: Band::parse(arg(2)?)?,
        }),
        Some("Multiply") => Ok(Step::Multiply {
            factor: num(1)?,
            band: Band::parse(arg(2)?)?,
        }),
        Some("Smooth") => Ok(Step::Smooth { factor: num(1)? }),
        Some("Invert") => Ok(Step::Invert {
            chance: num(1)?,
            axes: match arg(2)? {
                "x" => Axes::X,
                "y" => Axes::Y,
                "both" => Axes::Both,
                other => return Err(format!("{other:?} is not x, y, or both")),
            },
        }),
        Some(other) => Err(format!("{other:?} is not a template tool")),
        None => Err("the step is empty".to_owned()),
    }
}

/// Fall-off of blobs and of ridges for a cell count. More cells need a slower
/// fall-off, so that a blob covers the same area. The table values come from
/// Azgaar's Fantasy Map Generator; inkland interpolates between them.
const BLOB_POWER: [(f64, f64); 13] = [
    (1000.0, 0.93),
    (2000.0, 0.95),
    (5000.0, 0.97),
    (10000.0, 0.98),
    (20000.0, 0.99),
    (30000.0, 0.991),
    (40000.0, 0.993),
    (50000.0, 0.994),
    (60000.0, 0.995),
    (70000.0, 0.9955),
    (80000.0, 0.996),
    (90000.0, 0.9964),
    (100000.0, 0.9973),
];
const LINE_POWER: [(f64, f64); 13] = [
    (1000.0, 0.75),
    (2000.0, 0.77),
    (5000.0, 0.79),
    (10000.0, 0.81),
    (20000.0, 0.82),
    (30000.0, 0.83),
    (40000.0, 0.84),
    (50000.0, 0.86),
    (60000.0, 0.87),
    (70000.0, 0.88),
    (80000.0, 0.91),
    (90000.0, 0.92),
    (100000.0, 0.93),
];

fn interpolate(table: &[(f64, f64)], x: f64) -> f64 {
    let first = table[0];
    let last = table[table.len() - 1];
    if x <= first.0 {
        return first.1;
    }
    if x >= last.0 {
        return last.1;
    }
    for w in table.windows(2) {
        let (x0, y0) = w[0];
        let (x1, y1) = w[1];
        if x <= x1 {
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        }
    }
    last.1
}

fn pow(a: f64, b: f64) -> f64 {
    libm::pow(a, b)
}

fn lim(h: f64) -> f64 {
    h.clamp(0.0, 100.0)
}

/// Heights are whole numbers from 0 to 100. Each write drops the fraction,
/// so a blob loses up to 1 unit at each ring and stops at a finite size.
fn store(h: f64) -> f64 {
    lim(h).trunc()
}

/// Runs template steps on the heights of a grid.
pub struct Heightmap<'a> {
    grid: &'a Grid,
    pub heights: Vec<f64>,
    sea_level: f64,
    blob_power: f64,
    line_power: f64,
}

impl<'a> Heightmap<'a> {
    /// Heights of 0 for each cell.
    pub fn new(grid: &'a Grid, sea_level: f64) -> Self {
        let n = grid.len() as f64;
        Heightmap {
            grid,
            heights: vec![0.0; grid.len()],
            sea_level,
            blob_power: interpolate(&BLOB_POWER, n),
            line_power: interpolate(&LINE_POWER, n),
        }
    }

    pub fn run(&mut self, steps: &[Step], rng: &mut Rand) {
        for step in steps {
            self.step(step, rng);
        }
    }

    fn step(&mut self, step: &Step, rng: &mut Rand) {
        match *step {
            Step::Hill {
                count,
                height,
                x,
                y,
            } => {
                for _ in 0..count.count(rng) {
                    self.blob(height.value(rng), x, y, false, rng);
                }
            }
            Step::Pit {
                count,
                height,
                x,
                y,
            } => {
                for _ in 0..count.count(rng) {
                    self.blob(height.value(rng), x, y, true, rng);
                }
            }
            Step::Range {
                count,
                height,
                x,
                y,
            } => {
                for _ in 0..count.count(rng) {
                    self.ridge(height.value(rng), x, y, false, rng);
                }
            }
            Step::Trough {
                count,
                height,
                x,
                y,
            } => {
                for _ in 0..count.count(rng) {
                    self.ridge(height.value(rng), x, y, true, rng);
                }
            }
            Step::Strait { width, vertical } => self.strait(width, vertical, rng),
            Step::Mask { power } => self.mask(power),
            Step::Add { value, band } => self.modify(band, value, 1.0),
            Step::Multiply { factor, band } => self.modify(band, 0.0, factor),
            Step::Smooth { factor } => self.smooth(factor),
            Step::Invert { chance, axes } => {
                if rng.chance(chance) {
                    self.invert(axes);
                }
            }
        }
    }

    fn random_slot(&self, x: Span, y: Span, rng: &mut Rand) -> usize {
        let px = x.pick(self.grid.width, rng);
        let py = y.pick(self.grid.height, rng);
        self.grid.slot(px, py)
    }

    /// A hill (or a pit, with `down`) that starts in the given area.
    fn blob(&mut self, height: f64, x: Span, y: Span, down: bool, rng: &mut Rand) {
        let h = store(height);
        let mut start = self.random_slot(x, y, rng);
        for _ in 0..50 {
            let ok = if down {
                self.heights[start] >= self.sea_level
            } else {
                self.heights[start] + h <= 90.0
            };
            if ok {
                break;
            }
            start = self.random_slot(x, y, rng);
        }
        if down {
            self.pit_at(start, h, rng);
        } else {
            self.hill_at(start, h, rng);
        }
    }

    /// A hill spreads ring by ring. Each cell gets the change of the cell
    /// before it, raised to the blob power, so the hill becomes lower at
    /// each ring.
    fn hill_at(&mut self, start: usize, h: f64, rng: &mut Rand) {
        let n = self.grid.len();
        let mut change = vec![0.0; n];
        let mut seen = vec![false; n];
        change[start] = h;
        seen[start] = true;
        let mut queue = VecDeque::from([start]);
        while let Some(q) = queue.pop_front() {
            for c in self.grid.neighbors(q) {
                if seen[c] {
                    continue;
                }
                seen[c] = true;
                change[c] = (pow(change[q], self.blob_power) * rng.uniform(0.9, 1.1)).trunc();
                if change[c] > 1.0 {
                    queue.push_back(c);
                }
            }
        }
        for (h, d) in self.heights.iter_mut().zip(&change) {
            *h = store(*h + d);
        }
    }

    /// A pit becomes shallower at each cell that it visits, not at each
    /// ring, so it stays small.
    fn pit_at(&mut self, start: usize, depth: f64, rng: &mut Rand) {
        let mut d = depth;
        let mut seen = vec![false; self.grid.len()];
        seen[start] = true;
        let mut queue = VecDeque::from([start]);
        while let Some(q) = queue.pop_front() {
            d = pow(d, self.blob_power) * rng.uniform(0.9, 1.1);
            if d < 1.0 {
                return;
            }
            for c in self.grid.neighbors(q) {
                if seen[c] {
                    continue;
                }
                seen[c] = true;
                self.heights[c] = store(self.heights[c] - d * rng.uniform(0.9, 1.1));
                queue.push_back(c);
            }
        }
    }

    /// A greedy path from `from` to `to`. With probability `wobble` a step
    /// looks twice as good, so the path bends.
    fn path(
        &self,
        from: usize,
        to: usize,
        wobble: f64,
        used: &mut [bool],
        rng: &mut Rand,
    ) -> Vec<usize> {
        let p = &self.grid.points;
        let mut cur = from;
        let mut path = vec![cur];
        used[cur] = true;
        while cur != to {
            let mut best = f64::INFINITY;
            let mut next = None;
            for e in self.grid.neighbors(cur) {
                if used[e] {
                    continue;
                }
                let dx = p[to][0] - p[e][0];
                let dy = p[to][1] - p[e][1];
                let mut d = dx * dx + dy * dy;
                if rng.chance(wobble) {
                    d /= 2.0;
                }
                if d < best {
                    best = d;
                    next = Some(e);
                }
            }
            let Some(n) = next else { break };
            cur = n;
            used[cur] = true;
            path.push(cur);
        }
        path
    }

    /// A range (or a trough, with `down`).
    fn ridge(&mut self, height: f64, x: Span, y: Span, down: bool, rng: &mut Rand) {
        let (w, hgt) = (self.grid.width, self.grid.height);
        let mut h = store(height);
        let (mut sx, mut sy) = (x.pick(w, rng), y.pick(hgt, rng));
        if down {
            for _ in 0..50 {
                if self.heights[self.grid.slot(sx, sy)] >= self.sea_level {
                    break;
                }
                sx = x.pick(w, rng);
                sy = y.pick(hgt, rng);
            }
        }
        let max_dist = if down { w / 2.0 } else { w / 3.0 };
        let (mut ex, mut ey) = (0.0, 0.0);
        for _ in 0..50 {
            ex = rng.uniform(0.1 * w, 0.9 * w);
            ey = rng.uniform(0.15 * hgt, 0.85 * hgt);
            let d = (ex - sx).abs() + (ey - sy).abs();
            if d >= w / 8.0 && d <= max_dist {
                break;
            }
        }
        let start = self.grid.slot(sx, sy);
        let end = self.grid.slot(ex, ey);
        let mut used = vec![false; self.grid.len()];
        let wobble = if down { 0.2 } else { 0.15 };
        let ridge = self.path(start, end, wobble, &mut used, rng);

        let sign = if down { -1.0 } else { 1.0 };
        let mut frontier = ridge.clone();
        let mut rings = 0;
        while !frontier.is_empty() {
            rings += 1;
            for &c in &frontier {
                self.heights[c] = store(self.heights[c] + sign * h * rng.uniform(0.85, 1.15));
            }
            h = pow(h, self.line_power) - 1.0;
            if h < 2.0 {
                break;
            }
            let mut next = Vec::new();
            for &f in &frontier {
                for c in self.grid.neighbors(f) {
                    if !used[c] {
                        used[c] = true;
                        next.push(c);
                    }
                }
            }
            frontier = next;
        }

        // Spurs: from each sixth cell of the ridge, go downhill and pull the
        // lower cell toward the height of the cell above it.
        for (i, &first) in ridge.iter().enumerate() {
            if i % 6 != 0 {
                continue;
            }
            let mut cur = first;
            for _ in 0..rings {
                let Some(low) = self
                    .grid
                    .neighbors(cur)
                    .min_by(|&a, &b| self.heights[a].total_cmp(&self.heights[b]))
                else {
                    break;
                };
                self.heights[low] = store((self.heights[cur] * 2.0 + self.heights[low]) / 3.0);
                cur = low;
            }
        }
    }

    fn strait(&mut self, width: Amount, vertical: bool, rng: &mut Rand) {
        let (w, h) = (self.grid.width, self.grid.height);
        let width = (width.count(rng) as f64).min(f64::from(self.grid.cells_x) / 3.0);
        if width < 1.0 {
            return;
        }
        let edge_x = self.grid.step_x / 2.0;
        let edge_y = self.grid.step_y / 2.0;
        let (sx, sy, ex, ey) = if vertical {
            let sx = rng.uniform(0.3 * w, 0.7 * w);
            let ex = (w - sx - 0.1 * w + rng.uniform(0.0, 0.2 * w)).clamp(0.0, w);
            (sx, edge_y, ex, h - edge_y)
        } else {
            let sy = rng.uniform(0.3 * h, 0.7 * h);
            let ey = (h - sy - 0.1 * h + rng.uniform(0.0, 0.2 * h)).clamp(0.0, h);
            (edge_x, sy, w - edge_x, ey)
        };
        let start = self.grid.slot(sx, sy);
        let end = self.grid.slot(ex, ey);
        let mut no_use = vec![false; self.grid.len()];
        let mut ring = self.path(start, end, 0.2, &mut no_use, rng);

        let mut used = vec![false; self.grid.len()];
        let step = 0.1 / width;
        let rounds = width as u32;
        for i in 0..rounds {
            let exp = 0.9 - step * (width - f64::from(i));
            let mut next = Vec::new();
            for &r in &ring {
                for e in self.grid.neighbors(r) {
                    if used[e] {
                        continue;
                    }
                    used[e] = true;
                    next.push(e);
                    self.heights[e] = store(pow(self.heights[e], exp));
                }
            }
            ring = next;
        }
    }

    fn mask(&mut self, power: f64) {
        let fr = if power == 0.0 { 1.0 } else { power.abs() };
        for (c, h) in self.heights.iter_mut().enumerate() {
            let [x, y] = self.grid.points[c];
            let nx = 2.0 * x / self.grid.width - 1.0;
            let ny = 2.0 * y / self.grid.height - 1.0;
            let mut d = (1.0 - nx * nx) * (1.0 - ny * ny);
            if power < 0.0 {
                d = 1.0 - d;
            }
            *h = store((*h * (fr - 1.0) + *h * d) / fr);
        }
    }

    fn modify(&mut self, band: Band, add: f64, mult: f64) {
        let sea = self.sea_level;
        let (lo, hi, land) = match band {
            Band::All => (0.0, 100.0, false),
            Band::Land => (sea, 100.0, true),
            Band::Between(a, b) => (a, b, false),
        };
        for h in &mut self.heights {
            if *h < lo || *h > hi {
                continue;
            }
            let mut v = *h;
            if add != 0.0 {
                v = if land { (v + add).max(sea) } else { v + add };
            }
            if mult != 1.0 {
                v = if land {
                    (v - sea) * mult + sea
                } else {
                    v * mult
                };
            }
            *h = store(v);
        }
    }

    fn smooth(&mut self, factor: f64) {
        let fr = factor.max(1.0);
        let old = self.heights.clone();
        for (c, h) in self.heights.iter_mut().enumerate() {
            let (sum, n) = self
                .grid
                .neighbors(c)
                .fold((old[c], 1.0), |(s, n), x| (s + old[x], n + 1.0));
            let mean = sum / n;
            *h = store((old[c] * (fr - 1.0) + mean) / fr);
        }
    }

    fn invert(&mut self, axes: Axes) {
        let (cx, cy) = (self.grid.cells_x as usize, self.grid.cells_y as usize);
        let old = self.heights.clone();
        for (i, h) in self.heights.iter_mut().enumerate() {
            let (x, y) = (i % cx, i / cx);
            let nx = if axes == Axes::Y { x } else { cx - 1 - x };
            let ny = if axes == Axes::X { y } else { cy - 1 - y };
            *h = old[nx + ny * cx];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TemplateName;
    use crate::seed::{Layer, SeedTree};

    fn grid() -> Grid {
        let mut rng = Rand::new(SeedTree::new(3).part_rng(Layer::Terrain, "grid"));
        Grid::new(400.0, 300.0, 5000, &mut rng)
    }

    fn rng() -> Rand {
        Rand::new(SeedTree::new(3).part_rng(Layer::Terrain, "heightmap"))
    }

    fn run(grid: &Grid, text: &str) -> Vec<f64> {
        let steps = parse_steps(text.lines()).unwrap();
        let mut hm = Heightmap::new(grid, 20.0);
        hm.run(&steps, &mut rng());
        hm.heights
    }

    #[test]
    fn each_built_in_template_parses() {
        use TemplateName::*;
        for t in [
            Volcano,
            HighIsland,
            LowIsland,
            Continents,
            Archipelago,
            Atoll,
            Mediterranean,
            Peninsula,
            Pangea,
            Isthmus,
            Shattered,
            Taklamakan,
            OldWorld,
            Fractious,
        ] {
            let steps = parse_steps(templates::steps(t).lines()).unwrap();
            assert!(!steps.is_empty(), "{t:?}");
        }
    }

    #[test]
    fn bad_steps_are_refused() {
        for bad in [
            "Hill 1 2",
            "Lake 1 2 3 4",
            "Hill 1 9-5 10-20 10-20",
            "Strait 2 diagonal",
            "Hill 1 50 10-200 0-10",
        ] {
            assert!(parse_steps([bad]).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_hill_is_highest_near_its_start() {
        let g = grid();
        let h = run(&g, "Hill 1 50 50-50 50-50");
        let center = g.slot(200.0, 150.0);
        let top = (0..g.len()).max_by(|&a, &b| h[a].total_cmp(&h[b])).unwrap();
        assert_eq!(top, center);
        assert!((h[center] - 50.0).abs() < 1e-9);
        assert!(h[g.slot(5.0, 5.0)] < h[center] / 2.0);
    }

    #[test]
    fn a_pit_lowers_land() {
        let g = grid();
        let flat = run(&g, "Add 50 all");
        let pit = run(&g, "Add 50 all\nPit 1 30 50-50 50-50");
        let c = g.slot(200.0, 150.0);
        let deepest = g.neighbors(c).map(|n| pit[n]).fold(f64::INFINITY, f64::min);
        assert!(deepest < flat[c] - 20.0, "{deepest}");
        // A pit is small: far cells keep their height.
        assert_eq!(pit[g.slot(5.0, 5.0)], flat[g.slot(5.0, 5.0)]);
    }

    #[test]
    fn heights_stay_in_range() {
        let g = grid();
        for t in [
            TemplateName::Continents,
            TemplateName::Archipelago,
            TemplateName::Fractious,
        ] {
            let h = run(&g, templates::steps(t));
            assert!(h.iter().all(|v| (0.0..=100.0).contains(v)), "{t:?}");
        }
    }

    #[test]
    fn land_multiply_keeps_land_at_or_above_sea_level() {
        let g = grid();
        let h = run(&g, "Add 30 all\nMultiply 0.1 land");
        assert!(h.iter().all(|&v| (v - 21.0).abs() < 1e-9), "{:?}", &h[..3]);
    }

    #[test]
    fn mask_lowers_the_edges_more_than_the_center() {
        let g = grid();
        let h = run(&g, "Add 60 all\nMask 1");
        assert!(h[g.slot(200.0, 150.0)] > 55.0);
        assert!(h[g.slot(2.0, 2.0)] < 5.0);
    }

    #[test]
    fn invert_both_mirrors_the_lattice() {
        let g = grid();
        let a = run(&g, "Hill 1 50 10-10 10-10");
        let b = run(&g, "Hill 1 50 10-10 10-10\nInvert 1 both");
        let n = a.len();
        for i in 0..n {
            assert_eq!(b[i], a[n - 1 - i]);
        }
    }

    #[test]
    fn same_seed_gives_the_same_heights() {
        let g = grid();
        let t = templates::steps(TemplateName::Continents);
        assert_eq!(run(&g, t), run(&g, t));
    }

    #[test]
    fn continents_make_land_and_sea() {
        let g = grid();
        let h = run(&g, templates::steps(TemplateName::Continents));
        let land = h.iter().filter(|&&v| v >= 20.0).count() as f64 / h.len() as f64;
        assert!((0.1..0.9).contains(&land), "land fraction {land}");
    }

    #[test]
    fn interpolation_hits_the_table_points_and_between() {
        assert_eq!(interpolate(&BLOB_POWER, 10000.0), 0.98);
        assert_eq!(interpolate(&BLOB_POWER, 500.0), 0.93);
        let mid = interpolate(&BLOB_POWER, 15000.0);
        assert!(mid > 0.98 && mid < 0.99);
    }
}
