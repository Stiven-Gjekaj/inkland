//! Pinned rivers: find the course of each river and carve its heights.
//!
//! A course goes from the source, through each `through` anchor in order, to
//! the mouth. It does not use a cell of an earlier course, except to join a
//! river with the same mouth. A least-cost path finds each leg: going up
//! costs more than going down, and crossing water costs much more. A water
//! cell on the course becomes land, except in a forced water region.
//!
//! Then the heights along the course are carved: each height is in the
//! range of the regions of its cell, and no height is more than the height
//! before it. The drainage then gives the course strictly falling
//! elevations, so the river flows downhill.

use crate::config::{Anchor, Config, RegionKind, RiverPin};
use crate::error::{Error, ErrorCode};
use crate::fit::{FitRegion, forced_land, forced_water, ruling_at};
use crate::grid::Grid;
use crate::rivers::{Course, CourseEnd};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

/// The cells that the anchors of the rivers need as exact sites: each point
/// anchor of each river, in config order.
pub fn river_points(config: &Config) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for r in &config.pins.rivers {
        for a in std::iter::once(&r.source)
            .chain(&r.through)
            .chain(std::iter::once(&r.mouth))
        {
            if let Anchor::Point(p) = a {
                out.push(*p);
            }
        }
    }
    out
}

/// What the course planner needs to know about the world.
pub struct Plan<'a> {
    pub grid: &'a Grid,
    pub config: &'a Config,
    pub regions: &'a [FitRegion<'a>],
    pub sea_level: f64,
    /// The cell of each point pin, in config order.
    pub place_cells: &'a [usize],
    /// The cell of each exact site, by the bits of its coordinates.
    pub site_cells: &'a BTreeMap<(u64, u64), usize>,
}

/// The cells that an anchor can match.
enum Target {
    One(usize),
    Any(BTreeSet<usize>),
}

impl Target {
    fn contains(&self, c: usize) -> bool {
        match self {
            Target::One(t) => *t == c,
            Target::Any(s) => s.contains(&c),
        }
    }
}

struct Key(f64);
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Plan<'_> {
    fn site(&self, p: [f64; 2]) -> usize {
        self.site_cells[&(p[0].to_bits(), p[1].to_bits())]
    }

    fn place(&self, id: &str) -> usize {
        let i = self
            .config
            .pins
            .points
            .iter()
            .position(|p| p.id == id)
            .expect("a valid config names only known places");
        self.place_cells[i]
    }

    fn region(&self, id: &str) -> &FitRegion<'_> {
        self.regions
            .iter()
            .find(|r| r.pin.id == id)
            .expect("a valid config names only known regions")
    }

    fn land(&self, heights: &[f64], c: usize) -> bool {
        heights[c] >= self.sea_level
    }

    /// The height range of a course cell: the ranges of its regions, and at
    /// least sea level.
    fn range(&self, c: usize) -> [f64; 2] {
        let p = self.grid.points[c];
        let [lo, hi] = ruling_at(self.regions, p)
            .into_iter()
            .fold([self.sea_level, 100.0], |[lo, hi], r| {
                [lo.max(r.range[0]), hi.min(r.range[1])]
            });
        [lo.ceil(), hi.floor()]
    }
}

/// Plan and carve the course of each pinned river, in config order.
pub fn plan_courses(plan: &Plan, heights: &mut [f64]) -> Result<Vec<Course>, Error> {
    let rivers = &plan.config.pins.rivers;
    // The anchor cells of each river. A course must not use the anchor
    // cell of another river.
    let anchors: Vec<BTreeSet<usize>> = rivers
        .iter()
        .map(|r| {
            std::iter::once(&r.source)
                .chain(&r.through)
                .chain(std::iter::once(&r.mouth))
                .filter_map(|a| match a {
                    Anchor::Point(p) => Some(plan.site(*p)),
                    Anchor::Place(r) => Some(plan.place(&r.place)),
                    Anchor::Region(_) => None,
                })
                .collect()
        })
        .collect();
    // Plan in config order. If a course is blocked by an earlier course,
    // plan that river first and try again. The order of tries is fixed, so
    // the result is the same on each run.
    let mut order: Vec<usize> = (0..rivers.len()).collect();
    let original = heights.to_vec();
    let mut tries = 0;
    loop {
        heights.copy_from_slice(&original);
        match plan_in_order(plan, heights, &order, &anchors) {
            Ok(courses) => return Ok(courses),
            Err((k, e)) if e.code == ErrorCode::RiverCrossesRiver && tries < rivers.len() => {
                tries += 1;
                order.retain(|&i| i != k);
                order.insert(0, k);
            }
            Err((_, e)) => return Err(e),
        }
    }
}

/// Plan the rivers in the given order. Return the courses in config order,
/// or the index of the river that failed and its error.
fn plan_in_order(
    plan: &Plan,
    heights: &mut [f64],
    order: &[usize],
    anchors: &[BTreeSet<usize>],
) -> Result<Vec<Course>, (usize, Error)> {
    let rivers = &plan.config.pins.rivers;
    let mut done: Vec<(usize, Course)> = Vec::new();
    let mut used = vec![false; plan.grid.len()];
    for &i in order {
        let river = &rivers[i];
        let mut blocked = used.clone();
        for (j, cells) in anchors.iter().enumerate() {
            if j != i {
                for &c in cells.difference(&anchors[i]) {
                    blocked[c] = true;
                }
            }
        }
        let course = plan_one(plan, heights, i, river, &done, &blocked).map_err(|e| (i, e))?;
        for &c in &course.cells {
            used[c] = true;
        }
        // The water cell at the end stays water: a later course must not
        // fill it.
        if let CourseEnd::Into(t) = course.end
            && heights[t] < plan.sea_level
        {
            used[t] = true;
        }
        carve(plan, heights, i, river, &course).map_err(|e| (i, e))?;
        done.push((i, course));
    }
    done.sort_by_key(|(i, _)| *i);
    Ok(done.into_iter().map(|(_, c)| c).collect())
}

fn not_downhill(i: usize, river: &RiverPin, why: String) -> Error {
    Error::new(
        ErrorCode::RiverNotDownhill,
        format!("/pins/rivers/{i}"),
        format!("the river {:?} cannot flow downhill: {why}", river.id),
    )
}

fn plan_one(
    plan: &Plan,
    heights: &mut [f64],
    i: usize,
    river: &RiverPin,
    earlier: &[(usize, Course)],
    used: &[bool],
) -> Result<Course, Error> {
    let grid = plan.grid;
    let sea = plan.sea_level;
    let path = format!("/pins/rivers/{i}");

    // The source cell.
    let source = match &river.source {
        Anchor::Point(p) => plan.site(*p),
        Anchor::Place(r) => plan.place(&r.place),
        Anchor::Region(r) => {
            let region = plan.region(&r.region);
            (0..grid.len())
                .filter(|&c| region.contains(grid.points[c]) && plan.land(heights, c) && !used[c])
                .max_by(|&a, &b| heights[a].total_cmp(&heights[b]).then(b.cmp(&a)))
                .ok_or_else(|| {
                    not_downhill(
                        i,
                        river,
                        format!(
                            "the region {:?} has no free land cell for the source",
                            r.region
                        ),
                    )
                })?
        }
    };
    if !plan.land(heights, source) {
        let p = grid.points[source];
        if forced_water(plan.regions, p) {
            return Err(not_downhill(
                i,
                river,
                "the source is in forced water".to_owned(),
            ));
        }
        // A source in free water becomes land, as the course cells do.
        heights[source] = sea;
    }

    // The mouth: the target of the last leg, and how the course ends.
    enum MouthKind {
        /// Step into this water cell.
        Water(Target),
        /// End on a cell of the target, which drains into water.
        Port(usize),
        /// End on a cell of the target and keep the water.
        Sink(Target),
    }
    let mut end_point = None;
    let mouth = match &river.mouth {
        Anchor::Point(p) => {
            let c = plan.site(*p);
            end_point = Some(*p);
            if !plan.land(heights, c) {
                MouthKind::Water(Target::One(shore_cell(plan, heights, c, *p)))
            } else {
                let next_to_water = grid.neighbors(c).any(|m| heights[m] < sea);
                let forced_land = forced_land(plan.regions, *p);
                if next_to_water && !forced_land {
                    heights[c] = sea - 1.0;
                    MouthKind::Water(Target::One(c))
                } else {
                    return Err(Error::new(
                        ErrorCode::RiverMouthOnLand,
                        format!("{path}/mouth"),
                        format!(
                            "the mouth of the river {:?} is on land and not next to water",
                            river.id
                        ),
                    ));
                }
            }
        }
        Anchor::Place(r) => MouthKind::Port(plan.place(&r.place)),
        Anchor::Region(r) => {
            let region = plan.region(&r.region);
            let cells: BTreeSet<usize> = (0..grid.len())
                .filter(|&c| region.contains(grid.points[c]))
                .collect();
            if region.pin.kind == RegionKind::Marsh {
                MouthKind::Sink(Target::Any(
                    cells
                        .into_iter()
                        .filter(|&c| plan.land(heights, c))
                        .collect(),
                ))
            } else {
                MouthKind::Water(Target::Any(
                    cells
                        .into_iter()
                        .filter(|&c| !plan.land(heights, c))
                        .collect(),
                ))
            }
        }
    };

    // A port mouth drains into a water cell next to the place. Choose it
    // now, and keep the path off it, so that it stays water.
    let mut used = used.to_vec();
    let port_water = match &mouth {
        MouthKind::Port(c) => {
            let water: Vec<usize> = grid.neighbors(*c).filter(|&m| heights[m] < sea).collect();
            let pick = |free_only: bool| {
                water
                    .iter()
                    .copied()
                    .filter(|&m| !free_only || !used[m])
                    .min_by(|&a, &b| heights[a].total_cmp(&heights[b]).then(a.cmp(&b)))
            };
            let w = pick(true).or_else(|| pick(false)).ok_or_else(|| {
                Error::new(
                    ErrorCode::RiverMouthOnLand,
                    format!("{path}/mouth"),
                    format!(
                        "the river {:?} ends at a place with no water next to it",
                        river.id
                    ),
                )
            })?;
            used[w] = true;
            Some(w)
        }
        _ => None,
    };
    let used = &used[..];

    // Earlier rivers with the same mouth: this river can join them.
    let joinable: BTreeSet<usize> = earlier
        .iter()
        .filter(|(j, _)| plan.config.pins.rivers[*j].mouth == river.mouth)
        .flat_map(|(_, c)| c.cells.iter().copied())
        .collect();

    // The legs: through each anchor, then to the mouth.
    let mut cells = vec![source];
    let mut legs: Vec<(Target, bool)> = Vec::new();
    for a in &river.through {
        let t = match a {
            Anchor::Point(p) => Target::One(plan.site(*p)),
            Anchor::Place(r) => Target::One(plan.place(&r.place)),
            Anchor::Region(r) => {
                let region = plan.region(&r.region);
                Target::Any(
                    (0..grid.len())
                        .filter(|&c| region.contains(grid.points[c]) && plan.land(heights, c))
                        .collect(),
                )
            }
        };
        legs.push((t, false));
    }
    let last_target = match &mouth {
        MouthKind::Water(t) | MouthKind::Sink(t) => match t {
            Target::One(c) => Target::One(*c),
            Target::Any(s) => Target::Any(s.clone()),
        },
        MouthKind::Port(c) => Target::One(*c),
    };
    legs.push((last_target, true));

    let mut join = None;
    let no_join = BTreeSet::new();
    let mut ceiling = plan.range(source)[1];
    for (target, last) in &legs {
        let start = *cells.last().expect("a course has a source");
        if target.contains(start) {
            continue;
        }
        let water_target = *last && matches!(mouth, MouthKind::Water(_));
        let leg = find_leg(plan, heights, start, target, water_target, if *last { &joinable } else { &no_join }, used, &cells, ceiling)
            .ok_or_else(|| {
                // Without the other courses, is there a path? Then they block it.
                let free = vec![false; grid.len()];
                let open = find_leg(plan, heights, start, target, water_target, &BTreeSet::new(), &free, &cells, ceiling);
                if open.is_some() {
                    Error::new(
                        ErrorCode::RiverCrossesRiver,
                        path.clone(),
                        format!("the river {:?} cannot reach its next point without crossing another pinned river", river.id),
                    )
                } else {
                    not_downhill(i, river, "no land path joins its points".to_owned())
                }
            })?;
        let (leg, leg_ceiling) = leg;
        ceiling = leg_ceiling;
        for c in leg {
            if joinable.contains(&c) {
                join = Some(c);
                break;
            }
            cells.push(c);
        }
        if join.is_some() {
            break;
        }
    }

    let end = if let Some(j) = join {
        CourseEnd::Into(j)
    } else {
        match &mouth {
            MouthKind::Water(_) => {
                // The last cell of the path is the water cell.
                let w = cells.pop().expect("the last leg reaches water");
                CourseEnd::Into(w)
            }
            MouthKind::Port(_) => CourseEnd::Into(port_water.expect("a port mouth has its water")),
            MouthKind::Sink(_) => CourseEnd::Sink,
        }
    };
    if join.is_some() {
        end_point = None;
    }
    Ok(Course {
        cells,
        end,
        end_point,
    })
}

/// The water cell of the water body of `mouth` that a course can reach and
/// that is nearest to the mouth point. A course reaches a water cell from
/// land, or from water that no region forces.
fn shore_cell(plan: &Plan, heights: &[f64], mouth: usize, point: [f64; 2]) -> usize {
    let grid = plan.grid;
    let forced_water = |c: usize| forced_water(plan.regions, grid.points[c]);
    let reachable = |c: usize| {
        grid.neighbors(c)
            .any(|m| plan.land(heights, m) || !forced_water(m))
    };
    let mut seen = vec![false; grid.len()];
    let mut queue = std::collections::VecDeque::from([mouth]);
    seen[mouth] = true;
    let mut best: Option<(f64, usize)> = None;
    while let Some(c) = queue.pop_front() {
        if reachable(c) {
            let q = grid.points[c];
            let d = (q[0] - point[0]).powi(2) + (q[1] - point[1]).powi(2);
            if best.is_none_or(|(bd, bc)| d < bd || (d == bd && c < bc)) {
                best = Some((d, c));
            }
        }
        for m in grid.neighbors(c) {
            if !seen[m] && !plan.land(heights, m) {
                seen[m] = true;
                queue.push_back(m);
            }
        }
    }
    best.map_or(mouth, |(_, c)| c)
}

/// The least-cost path from `start` to a cell of `target`. The path does
/// not include `start`. It uses land cells that no other course uses; with
/// `water_target` its last cell is a water cell of the target. A cell of
/// `joinable` also ends the path.
#[allow(clippy::too_many_arguments)]
fn find_leg(
    plan: &Plan,
    heights: &[f64],
    start: usize,
    target: &Target,
    water_target: bool,
    joinable: &BTreeSet<usize>,
    used: &[bool],
    before: &[usize],
    ceiling0: f64,
) -> Option<(Vec<usize>, f64)> {
    let grid = plan.grid;
    let n = grid.len();
    let goal_point = match target {
        Target::One(t) => Some(grid.points[*t]),
        Target::Any(_) => None,
    };
    let h_est = |c: usize| -> f64 {
        goal_point.map_or(0.0, |g| {
            let p = grid.points[c];
            ((p[0] - g[0]).powi(2) + (p[1] - g[1]).powi(2)).sqrt()
        })
    };
    let blocked: BTreeSet<usize> = before.iter().copied().collect();
    let mut cost = vec![f64::INFINITY; n];
    let mut from = vec![usize::MAX; n];
    // The highest height that the course can still have at each cell: no
    // height goes up, so it is the least upper limit of the cells before.
    let mut ceiling = vec![f64::NEG_INFINITY; n];
    let mut heap = BinaryHeap::new();
    cost[start] = 0.0;
    ceiling[start] = ceiling0;
    heap.push(Reverse((Key(h_est(start)), start)));
    while let Some(Reverse((_, c))) = heap.pop() {
        if c != start && (target.contains(c) || joinable.contains(&c)) {
            let mut leg = vec![c];
            let mut cur = c;
            while from[cur] != start {
                cur = from[cur];
                leg.push(cur);
            }
            leg.reverse();
            return Some((leg, ceiling[c]));
        }
        for m in grid.neighbors(c) {
            if blocked.contains(&m) || m == start {
                continue;
            }
            let is_goal = target.contains(m) || joinable.contains(&m);
            let water = !plan.land(heights, m);
            // Water can be a step of the course only where no region forces
            // water: carving makes it land. Forced water is only the goal.
            let forced_water = forced_water(plan.regions, grid.points[m]);
            if water && forced_water && !(is_goal && water_target) {
                continue;
            }
            if used[m] && !joinable.contains(&m) {
                continue;
            }
            let pc = grid.points[c];
            let pm = grid.points[m];
            let d = ((pc[0] - pm[0]).powi(2) + (pc[1] - pm[1]).powi(2)).sqrt();
            let up = (heights[m] - heights[c]).max(0.0);
            // Filling water to make land costs much more than going up.
            let fill = if water && !(is_goal && water_target) {
                20.0 + (plan.sea_level - heights[m])
            } else {
                0.0
            };
            let next = cost[c] + d * (1.0 + 0.5 * up + fill);
            // A step must leave a height that does not go up.
            let [lo, hi] = if water && is_goal && water_target {
                [f64::NEG_INFINITY, f64::INFINITY]
            } else if joinable.contains(&m) {
                [heights[m], heights[m]]
            } else {
                plan.range(m)
            };
            if lo > ceiling[c] {
                continue;
            }
            if next < cost[m] {
                cost[m] = next;
                from[m] = c;
                ceiling[m] = ceiling[c].min(hi);
                heap.push(Reverse((Key(next + h_est(m)), m)));
            }
        }
    }
    None
}

/// Give the course heights that do not go up and that stay in the ranges of
/// its cells.
fn carve(
    plan: &Plan,
    heights: &mut [f64],
    i: usize,
    river: &RiverPin,
    course: &Course,
) -> Result<(), Error> {
    let cells = &course.cells;
    let m = cells.len();
    let ranges: Vec<[f64; 2]> = cells.iter().map(|&c| plan.range(c)).collect();
    let floor_end = match course.end {
        CourseEnd::Into(t) if heights[t] >= plan.sea_level => heights[t],
        _ => f64::NEG_INFINITY,
    };
    // The least height that each cell needs, from the mouth up.
    let mut need = vec![0.0; m];
    let mut next = floor_end;
    for k in (0..m).rev() {
        need[k] = ranges[k][0].max(next);
        if need[k] > ranges[k][1] {
            return Err(not_downhill(
                i,
                river,
                format!(
                    "a cell at [{:.3}, {:.3}] allows heights {} to {}, but the course below it needs {}",
                    plan.grid.points[cells[k]][0],
                    plan.grid.points[cells[k]][1],
                    ranges[k][0],
                    ranges[k][1],
                    need[k]
                ),
            ));
        }
        next = need[k];
    }
    let mut prev = f64::INFINITY;
    for k in 0..m {
        let c = cells[k];
        let h = heights[c].min(prev).clamp(need[k], ranges[k][1]);
        heights[c] = h;
        prev = h;
    }
    Ok(())
}
