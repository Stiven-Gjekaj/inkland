//! Drainage and rivers.
//!
//! A priority flood from the ocean gives each cell a drainage elevation.
//! The elevation is the height of the cell, or a little more than the
//! elevation of the cell that drains it. Thus each cell, lakes too, has a
//! neighbor with a lower elevation, and water always flows downhill to the
//! ocean (or to a sink). Each cell sends its water to its lowest neighbor.
//!
//! A land cell with enough water is a river cell. A river follows the cells
//! from its source to its mouth. At a confluence the river with more water
//! keeps its name, and the other river ends there as a tributary.
//!
//! A fixed course (a pinned river) is a chain of cells with given
//! elevations. Its cells always drain along the chain, so no other river can
//! cross it: a river that reaches it joins it.

use crate::features::{FeatureKind, Features};
use crate::grid::Grid;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

/// No receiver: the cell is ocean or a sink.
pub const NONE: usize = usize::MAX;

/// The elevation step of the flood. It is small, so it changes only flat
/// areas.
const FLOOD_STEP: f64 = 1e-6;

/// The least number of cells of water that make a river: a fixed part plus
/// a part that grows with the cell count.
pub fn river_threshold(cells: usize) -> f64 {
    15.0 + 0.004 * cells as f64
}

/// The end of a fixed course.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CourseEnd {
    /// The last cell drains into this cell: water, or a cell of an earlier
    /// course (a confluence).
    Into(usize),
    /// The last cell keeps its water (a river that ends in a marsh).
    Sink,
}

/// A fixed course: the cells of a pinned river, from source to the last
/// land cell, with their heights already carved so that they do not go up.
#[derive(Debug, Clone, PartialEq)]
pub struct Course {
    pub cells: Vec<usize>,
    pub end: CourseEnd,
    /// The exact end point of the line, if the config gives one.
    pub end_point: Option<[f64; 2]>,
}

/// Where the water of each cell goes, and how much water each cell carries.
#[derive(Debug, Clone, PartialEq)]
pub struct Drainage {
    /// The drainage elevation of each cell.
    pub elevation: Vec<f64>,
    /// The cell that receives the water of each cell, or `NONE`.
    pub receiver: Vec<usize>,
    /// The water of each cell, in cells of mean land precipitation.
    pub flux: Vec<f64>,
}

/// A heap key that orders by `total_cmp`.
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

/// Find the drainage of the world.
pub fn drain(
    grid: &Grid,
    heights: &[f64],
    features: &Features,
    prec: &[f64],
    courses: &[Course],
) -> Drainage {
    let n = grid.len();
    let mut elevation = vec![f64::INFINITY; n];
    let mut receiver = vec![NONE; n];
    let mut fixed = vec![false; n];
    let mut heap = BinaryHeap::new();

    let ocean = |c: usize| features.get(features.of_cell[c]).kind == FeatureKind::Ocean;
    for c in 0..n {
        if ocean(c) {
            elevation[c] = heights[c];
            fixed[c] = true;
            heap.push(Reverse((Key(elevation[c]), c)));
        }
    }
    // A course that ends in a lake: that lake cell is a sink with the
    // height of the lake, so the flood cannot raise it above the course.
    for course in courses {
        if let CourseEnd::Into(t) = course.end
            && !fixed[t]
            && !features.is_land(t)
        {
            elevation[t] = heights[t];
            fixed[t] = true;
            heap.push(Reverse((Key(elevation[t]), t)));
        }
    }
    // Fixed courses. A course can end in another course, so it gets its
    // elevations after that course, and they rise from the cell it joins.
    let mut course_cell = vec![usize::MAX; n];
    for (k, course) in courses.iter().enumerate() {
        for &c in &course.cells {
            course_cell[c] = k;
        }
    }
    let mut order: Vec<usize> = Vec::new();
    let mut placed = vec![false; courses.len()];
    while order.len() < courses.len() {
        let before = order.len();
        for (k, course) in courses.iter().enumerate() {
            if placed[k] {
                continue;
            }
            let waits = match course.end {
                CourseEnd::Into(t) => {
                    course_cell[t] != usize::MAX && course_cell[t] != k && !placed[course_cell[t]]
                }
                CourseEnd::Sink => false,
            };
            if !waits {
                placed[k] = true;
                order.push(k);
            }
        }
        assert!(order.len() > before, "courses join each other in a loop");
    }
    for course in order.iter().map(|&k| &courses[k]) {
        let m = course.cells.len();
        let gap = 0.5 / (m as f64 + 1.0);
        let base = match course.end {
            CourseEnd::Into(t) if elevation[t].is_finite() => elevation[t],
            CourseEnd::Into(t) => heights[t],
            CourseEnd::Sink => f64::NEG_INFINITY,
        };
        for (i, &c) in course.cells.iter().enumerate() {
            let k = (m - i) as f64;
            elevation[c] = (heights[c] + k * gap).max(base + k * gap);
            fixed[c] = true;
            receiver[c] = match course.cells.get(i + 1) {
                Some(&next) => next,
                None => match course.end {
                    CourseEnd::Into(t) => t,
                    CourseEnd::Sink => NONE,
                },
            };
            heap.push(Reverse((Key(elevation[c]), c)));
        }
    }

    while let Some(Reverse((_, c))) = heap.pop() {
        for m in grid.neighbors(c) {
            if fixed[m] || elevation[m].is_finite() {
                continue;
            }
            elevation[m] = heights[m].max(elevation[c] + FLOOD_STEP);
            heap.push(Reverse((Key(elevation[m]), m)));
        }
    }

    for c in 0..n {
        if fixed[c] {
            continue;
        }
        receiver[c] = grid
            .neighbors(c)
            .min_by(|&a, &b| elevation[a].total_cmp(&elevation[b]).then(a.cmp(&b)))
            .filter(|&m| elevation[m] < elevation[c])
            .unwrap_or(NONE);
    }

    // Water: each cell adds its precipitation, in units of the mean land
    // precipitation, and passes all of its water down.
    let land: Vec<usize> = (0..n).filter(|&c| features.is_land(c)).collect();
    let mean = if land.is_empty() {
        1.0
    } else {
        (land.iter().map(|&c| prec[c]).sum::<f64>() / land.len() as f64).max(1.0)
    };
    let mut order: Vec<usize> = (0..n).filter(|&c| !ocean(c)).collect();
    order.sort_by(|&a, &b| elevation[b].total_cmp(&elevation[a]).then(a.cmp(&b)));
    let mut flux = vec![0.0; n];
    for c in order {
        flux[c] += prec[c] / mean;
        if receiver[c] != NONE {
            let r = receiver[c];
            flux[r] += flux[c];
        }
    }
    Drainage {
        elevation,
        receiver,
        flux,
    }
}

/// How a river ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mouth {
    /// In the ocean or a lake, at this water cell.
    Water(usize),
    /// In another river, at this cell.
    Join(usize),
    /// In a sink, such as a marsh.
    Sink,
}

/// One river.
#[derive(Debug, Clone, PartialEq)]
pub struct River {
    /// 1, 2, 3, ... in a fixed order: by mouth water, then tributaries.
    pub id: u32,
    /// The index of the course (pinned river) in the config, if any.
    pub course: Option<usize>,
    /// The cells from the source to the last land cell.
    pub cells: Vec<usize>,
    /// The line from the source to the mouth, in units.
    pub points: Vec<[f64; 2]>,
    pub mouth: Mouth,
    /// The river that this river joins, if it is a tributary.
    pub parent: Option<u32>,
    /// The water at the last cell.
    pub flux: f64,
    /// The length of the line, in units.
    pub length: f64,
}

/// The midpoint of the polygon edge between two neighbor cells.
fn shared_edge_midpoint(grid: &Grid, a: usize, b: usize) -> Option<[f64; 2]> {
    let poly = &grid.polygons[a];
    let k = grid.edge_cells[a].iter().position(|&x| x as usize == b)?;
    let p = grid.vertices[poly[k] as usize];
    let q = grid.vertices[poly[(k + 1) % poly.len()] as usize];
    Some([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0])
}

/// Trace the rivers of a drainage.
pub fn trace(
    grid: &Grid,
    features: &Features,
    drainage: &Drainage,
    courses: &[Course],
    threshold: f64,
) -> Vec<River> {
    let n = grid.len();
    let Drainage { receiver, flux, .. } = drainage;
    let mut course_of = vec![None; n];
    let mut course_prev = vec![NONE; n];
    for (k, course) in courses.iter().enumerate() {
        for (i, &c) in course.cells.iter().enumerate() {
            course_of[c] = Some(k);
            course_prev[c] = if i == 0 { NONE } else { course.cells[i - 1] };
        }
    }
    let is_river =
        |c: usize| features.is_land(c) && (flux[c] >= threshold || course_of[c].is_some());

    // The river cells upstream of each river cell.
    let mut upstream: Vec<Vec<usize>> = vec![Vec::new(); n];
    for c in 0..n {
        if is_river(c) && receiver[c] != NONE && is_river(receiver[c]) {
            upstream[receiver[c]].push(c);
        }
    }
    let main_upstream = |c: usize| -> Option<usize> {
        if course_of[c].is_some() {
            return (course_prev[c] != NONE).then_some(course_prev[c]);
        }
        upstream[c]
            .iter()
            .copied()
            .max_by(|&a, &b| flux[a].total_cmp(&flux[b]).then(b.cmp(&a)))
    };

    let mut mouths: Vec<usize> = (0..n)
        .filter(|&c| is_river(c) && (receiver[c] == NONE || !features.is_land(receiver[c])))
        .collect();
    mouths.sort_by(|&a, &b| flux[b].total_cmp(&flux[a]).then(a.cmp(&b)));

    // Work items: (last cell, parent river index).
    let mut queue: VecDeque<(usize, Option<usize>)> =
        mouths.into_iter().map(|c| (c, None)).collect();
    let mut rivers: Vec<River> = Vec::new();
    while let Some((last, parent)) = queue.pop_front() {
        let mut chain = vec![last];
        let mut cur = last;
        while let Some(up) = main_upstream(cur) {
            chain.push(up);
            cur = up;
        }
        chain.reverse();
        let course = course_of[last];
        let index = rivers.len();
        // Tributaries: the upstream river cells that are not on the chain,
        // with the most water first.
        let mut tributaries: Vec<usize> = Vec::new();
        for &c in &chain {
            let main = main_upstream(c);
            for &u in &upstream[c] {
                if Some(u) != main {
                    tributaries.push(u);
                }
            }
        }
        tributaries.sort_by(|&a, &b| flux[b].total_cmp(&flux[a]).then(a.cmp(&b)));
        for t in tributaries {
            queue.push_back((t, Some(index)));
        }
        let r = receiver[last];
        let mouth = if r == NONE {
            Mouth::Sink
        } else if features.is_land(r) {
            Mouth::Join(r)
        } else {
            Mouth::Water(r)
        };
        let mut points: Vec<[f64; 2]> = chain.iter().map(|&c| grid.points[c]).collect();
        let end = match (course.and_then(|k| courses[k].end_point), mouth) {
            (Some(p), _) => Some(p),
            (None, Mouth::Water(w)) => shared_edge_midpoint(grid, last, w),
            (None, Mouth::Join(j)) => Some(grid.points[j]),
            (None, Mouth::Sink) => None,
        };
        if let Some(p) = end {
            points.push(p);
        }
        let length = points
            .windows(2)
            .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
            .sum();
        rivers.push(River {
            id: 0,
            course,
            cells: chain,
            points,
            mouth,
            parent: parent.map(|p| p as u32),
            flux: flux[last],
            length,
        });
    }

    // Drop short rivers that are not pinned, and the tributaries of dropped
    // rivers. Then number the rivers that stay.
    let mut keep = vec![false; rivers.len()];
    for i in 0..rivers.len() {
        let parent_kept = rivers[i].parent.is_none_or(|p| keep[p as usize]);
        keep[i] = parent_kept && (rivers[i].course.is_some() || rivers[i].cells.len() >= 3);
    }
    let mut new_id = vec![0u32; rivers.len()];
    let mut next = 1;
    for i in 0..rivers.len() {
        if keep[i] {
            new_id[i] = next;
            next += 1;
        }
    }
    rivers
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(i, mut r)| {
            r.id = new_id[i];
            r.parent = r.parent.map(|p| new_id[p as usize]);
            r
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rand::Rand;
    use crate::seed::{Layer, SeedTree};

    fn grid() -> Grid {
        Grid::new(
            400.0,
            300.0,
            3000,
            &mut Rand::new(SeedTree::new(2).part_rng(Layer::Terrain, "grid")),
        )
    }

    /// A dome island: high in the center, sea at the edges.
    fn dome(g: &Grid) -> Vec<f64> {
        g.points
            .iter()
            .map(|p| {
                let d = ((p[0] - 200.0).powi(2) + (p[1] - 150.0).powi(2)).sqrt();
                (90.0 - d * 0.6).max(5.0).round()
            })
            .collect()
    }

    #[test]
    fn water_always_flows_to_a_lower_cell_and_reaches_the_ocean() {
        let g = grid();
        let h = dome(&g);
        let f = Features::mark(&g, &h, 20.0);
        let d = drain(&g, &h, &f, &vec![10.0; g.len()], &[]);
        for c in 0..g.len() {
            let mut cur = c;
            let mut steps = 0;
            while d.receiver[cur] != NONE {
                let r = d.receiver[cur];
                assert!(d.elevation[r] < d.elevation[cur]);
                cur = r;
                steps += 1;
                assert!(steps <= g.len());
            }
            assert_eq!(
                f.get(f.of_cell[cur]).kind,
                FeatureKind::Ocean,
                "cell {c} ends at {cur}"
            );
        }
    }

    #[test]
    fn rivers_flow_downhill_to_water() {
        let g = grid();
        let h = dome(&g);
        let f = Features::mark(&g, &h, 20.0);
        let d = drain(&g, &h, &f, &vec![10.0; g.len()], &[]);
        let rivers = trace(&g, &f, &d, &[], river_threshold(g.len()));
        assert!(rivers.len() >= 3, "{}", rivers.len());
        for r in &rivers {
            for w in r.cells.windows(2) {
                assert!(d.elevation[w[1]] < d.elevation[w[0]]);
                assert_eq!(d.receiver[w[0]], w[1]);
            }
            match r.mouth {
                Mouth::Water(w) => assert!(!f.is_land(w)),
                Mouth::Join(j) => assert!(rivers.iter().any(|o| o.cells.contains(&j))),
                Mouth::Sink => panic!("no sink without a course"),
            }
            assert!(r.length > 0.0);
        }
        let ids: Vec<u32> = rivers.iter().map(|r| r.id).collect();
        assert_eq!(ids, (1..=rivers.len() as u32).collect::<Vec<_>>());
    }

    #[test]
    fn a_course_is_kept_and_other_water_joins_it() {
        let g = grid();
        let h = dome(&g);
        let f = Features::mark(&g, &h, 20.0);
        // A straight course from the top of the dome to the east coast.
        let mut cells = vec![g.find_cell(200.0, 150.0)];
        loop {
            let last = *cells.last().unwrap();
            let next = g
                .neighbors(last)
                .filter(|&m| !cells.contains(&m))
                .min_by(|&a, &b| {
                    let da = (g.points[a][1] - 150.0).abs() - g.points[a][0];
                    let db = (g.points[b][1] - 150.0).abs() - g.points[b][0];
                    da.total_cmp(&db)
                })
                .unwrap();
            if !f.is_land(next) {
                break;
            }
            cells.push(next);
        }
        let last = *cells.last().unwrap();
        let water = g.neighbors(last).find(|&m| !f.is_land(m)).unwrap();
        let course = Course {
            cells: cells.clone(),
            end: CourseEnd::Into(water),
            end_point: None,
        };
        let d = drain(
            &g,
            &h,
            &f,
            &vec![10.0; g.len()],
            std::slice::from_ref(&course),
        );
        let rivers = trace(
            &g,
            &f,
            &d,
            std::slice::from_ref(&course),
            river_threshold(g.len()),
        );
        let pinned: Vec<&River> = rivers.iter().filter(|r| r.course == Some(0)).collect();
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].cells, cells);
        assert_eq!(pinned[0].mouth, Mouth::Water(water));
        // No other river uses a cell of the course except to join it.
        for r in rivers.iter().filter(|r| r.course.is_none()) {
            assert!(r.cells.iter().all(|c| !cells.contains(c)));
        }
    }

    #[test]
    fn a_sink_course_keeps_its_water() {
        let g = grid();
        let h = dome(&g);
        let f = Features::mark(&g, &h, 20.0);
        let a = g.find_cell(200.0, 150.0);
        let b = g.neighbors(a).next().unwrap();
        let course = Course {
            cells: vec![a, b],
            end: CourseEnd::Sink,
            end_point: None,
        };
        let d = drain(
            &g,
            &h,
            &f,
            &vec![10.0; g.len()],
            std::slice::from_ref(&course),
        );
        assert_eq!(d.receiver[b], NONE);
        assert!(d.elevation[a] > d.elevation[b]);
    }
}
