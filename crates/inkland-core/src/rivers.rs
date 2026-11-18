//! Water flow and rivers.
//!
//! This stage follows Azgaar's Fantasy Map Generator (FMG,
//! `model/physical/river-generator.ts` and `lakes.ts`):
//!
//! 1. `alterHeights`: land tilts a little toward the coast. A land cell
//!    gets its distance to the coast / 100 and the mean distance of its
//!    neighbors / 10000 on top of its height.
//! 2. `detectCloseLakes`: a lake in a deep depression is closed. It has no
//!    outlet.
//! 3. `resolveDepressions`: low points rise in steps of 0.1 (lakes in steps
//!    of 0.2 above their lowest shore cell) until each land cell has a lower
//!    neighbor, for up to 250 passes.
//! 4. `drainWater`: from the highest land cell to the lowest, each cell
//!    adds its precipitation to its water (flux) and gives the water to its
//!    lowest neighbor, or to its haven at the coast. A lake that gets more
//!    water than it loses to evaporation drains at its lowest shore cell.
//!    A cell with a flux of 30 or more is on a river. At a confluence the
//!    river with more water keeps its id downstream.
//! 5. `defineRivers`: rivers with fewer than 3 cells go. Each river gets its
//!    discharge, its width at the source and at the mouth, and its parent.
//! 6. `mergeLocked`: the pinned rivers are FMG locked rivers. A generated
//!    river that reaches a pinned course ends there as its tributary.
//! 7. The heights take the tilt and the filled depressions, and rivers cut
//!    their beds down by up to 5 (`downcutRivers`).
//!
//! FMG has the fixed sea level 20. inkland uses the sea level of the config:
//! the downcut starts at `sea_level + 15` (35 in FMG). The flux is a whole
//! number, as in the `Uint16Array` of FMG.
//!
//! Pins win. A pinned river is a fixed course: a chain of cells with fixed,
//! strictly falling elevations. Its cells always drain along the chain, so
//! no other river can cross it: a river that reaches it joins it. A lake at
//! the end of a course is closed, and a course does not rise or cut.

use crate::features::{FeatureKind, Features, NO_HAVEN};
use crate::grid::Grid;
use crate::meander::{Anchor, Line, meander};
use std::collections::BTreeMap;

/// No receiver: the cell is water, a sink, or a depression.
pub const NONE: usize = usize::MAX;

/// The least flux of a river cell (FMG `MIN_FLUX_TO_FORM_RIVER`).
pub const MIN_FLUX_TO_FORM_RIVER: f64 = 30.0;

/// The most passes of the depression filling (FMG
/// `resolveDepressionsSteps`).
pub const RESOLVE_DEPRESSIONS_STEPS: usize = 250;

/// The largest river cut of one cell (FMG `MAX_DOWNCUT`).
const MAX_DOWNCUT: f64 = 5.0;

/// River width constants of FMG.
const FLUX_FACTOR: f64 = 500.0;
const MAX_FLUX_WIDTH: f64 = 1.0;
const LENGTH_FACTOR: f64 = 200.0;
const LENGTH_PROGRESSION: [f64; 9] = [1.0, 1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 21.0, 34.0];

/// The default map of FMG: 1280 x 800 px with 10000 points. Its point
/// spacing scales the distances of the river meander.
const FMG_SPACING: f64 = 10.119_288_512_538_814;

/// The least flux of a river cell. The cell count does not change it; the
/// argument stays for the callers.
pub fn river_threshold(_cells: usize) -> f64 {
    MIN_FLUX_TO_FORM_RIVER
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
    /// The drainage elevation of each cell: the tilted and filled height of
    /// land, the surface of a lake, and the height of the ocean.
    pub elevation: Vec<f64>,
    /// The cell that receives the water of each cell, or `NONE`.
    pub receiver: Vec<usize>,
    /// The water of each cell (FMG `cells.fl`), a whole number.
    pub flux: Vec<f64>,
    /// The water of the smaller rivers at each confluence (FMG
    /// `cells.conf`): the sum of the water of all rivers that flow in,
    /// without the largest one. 0 for a cell that is not a confluence.
    pub confluence: Vec<f64>,
}

/// The water data of one lake (FMG lake fields).
#[derive(Debug, Clone, PartialEq)]
pub struct Lake {
    /// The feature id.
    pub feature: u32,
    /// The surface height after the depression filling.
    pub height: f64,
    /// `true` for a lake in a deep depression: it has no outlet.
    pub closed: bool,
    /// The water from the shore and from the rivers that flow in.
    pub flux: f64,
    /// The water that the lake loses to evaporation.
    pub evaporation: f64,
    /// The temperature of the lake, in degrees C.
    pub temp: f64,
    /// The rivers that flow into the lake, by river id.
    pub inlets: Vec<u32>,
    /// The river that flows out of the lake, by river id.
    pub outlet: Option<u32>,
}

/// How a river ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mouth {
    /// In the ocean or a lake, at this water cell.
    Water(usize),
    /// In another river, at this cell.
    Join(usize),
    /// In a sink, such as a marsh or a depression.
    Sink,
    /// At the frame edge: the water leaves the map.
    Edge,
}

/// One river.
#[derive(Debug, Clone, PartialEq)]
pub struct River {
    /// 1, 2, 3, ...: pinned rivers first, in config order, then the
    /// generated rivers in the order that FMG makes them.
    pub id: u32,
    /// The index of the course (pinned river) in the config, if any.
    pub course: Option<usize>,
    /// The cells from the source to the cell before the mouth. A river that
    /// passes a lake has the lake cells where it enters and leaves it.
    pub cells: Vec<usize>,
    /// The meandered line from the source to the mouth, in units (FMG
    /// `addMeandering`).
    pub points: Vec<[f64; 2]>,
    /// The index in `points` of each anchor of the line: the site of each
    /// cell, then the end point if the river has one. The anchors are
    /// exact, so a pinned river passes its points.
    pub anchors: Vec<usize>,
    pub mouth: Mouth,
    /// The river that this river flows into, if it is a tributary.
    pub parent: Option<u32>,
    /// The main river of the river system (FMG `basin`): this river, or the
    /// end of its chain of parents.
    pub basin: u32,
    /// The water at the last cell (FMG `discharge`).
    pub flux: f64,
    /// The width at the source, in km (FMG `sourceWidth`).
    pub source_width: f64,
    /// How fast the river widens downstream (FMG `widthFactor`).
    pub width_factor: f64,
    /// The width at the mouth, in km (FMG `width`).
    pub width: f64,
    /// The length of the meandered line, in units.
    pub length: f64,
}

/// What the river stage reads.
pub struct Input<'a> {
    pub grid: &'a Grid,
    /// The whole-number heights of the cells.
    pub heights: &'a [f64],
    pub sea_level: f64,
    /// The features with the markup of the second grid.
    pub features: &'a Features,
    /// The precipitation and the temperature of each cell.
    pub precipitation: &'a [f64],
    pub temperature: &'a [f64],
    /// The pinned courses.
    pub courses: &'a [Course],
    /// The number of cells of the first grid that the config asks for.
    pub cells_desired: f64,
    /// The lake elevation limit of the config.
    pub lake_elevation_limit: f64,
}

/// The result of the river stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub drainage: Drainage,
    pub rivers: Vec<River>,
    /// The water data of each lake, in feature order.
    pub lakes: Vec<Lake>,
    /// The new heights: tilted, filled, and cut by the rivers. Whole
    /// numbers.
    pub heights: Vec<f64>,
}

/// Round to `d` decimals, as FMG `rn`.
fn rn(v: f64, d: i32) -> f64 {
    let m = libm::pow(10.0, f64::from(d));
    (v * m).round() / m
}

/// Store a flux as FMG does in a `Uint16Array`.
fn u16(v: f64) -> f64 {
    v.trunc().clamp(0.0, 65535.0)
}

/// Add a flux to a mark of a `Uint8Array`, as FMG does: the sum wraps
/// at 256.
fn add_u8(mark: u8, flux: f64) -> u8 {
    ((f64::from(mark) + flux.trunc()).rem_euclid(256.0)) as u8
}

/// The midpoint of the polygon edge between two neighbor cells.
fn shared_edge_midpoint(grid: &Grid, a: usize, b: usize) -> Option<[f64; 2]> {
    let poly = &grid.polygons[a];
    let k = grid.edge_cells[a].iter().position(|&x| x as usize == b)?;
    let p = grid.vertices[poly[k] as usize];
    let q = grid.vertices[poly[(k + 1) % poly.len()] as usize];
    Some([(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0])
}

/// The nearest point of the frame edge (FMG `projectToNearestEdge`).
fn nearest_edge_point(p: [f64; 2], width: f64, height: f64) -> [f64; 2] {
    let d = [p[0], width - p[0], p[1], height - p[1]];
    let k = (0..4)
        .min_by(|&a, &b| d[a].total_cmp(&d[b]))
        .expect("four edges");
    match k {
        0 => [0.0, p[1]],
        1 => [width, p[1]],
        2 => [p[0], 0.0],
        _ => [p[0], height],
    }
}

/// The width offset at a point of a river (FMG `getOffset`).
fn offset(flux: f64, point_index: usize, width_factor: f64, starting_width: f64) -> f64 {
    if point_index == 0 {
        return starting_width;
    }
    let flux_width = (libm::pow(flux, 0.7) / FLUX_FACTOR).min(MAX_FLUX_WIDTH);
    let progression = LENGTH_PROGRESSION
        .get(point_index)
        .copied()
        .unwrap_or(LENGTH_PROGRESSION[8]);
    let length_width = point_index as f64 / LENGTH_FACTOR + progression / LENGTH_FACTOR;
    width_factor * (length_width + flux_width) + starting_width
}

/// The width at the source, in km (FMG `getSourceWidth`).
fn source_width(flux: f64) -> f64 {
    rn((libm::pow(flux, 0.9) / FLUX_FACTOR).min(MAX_FLUX_WIDTH), 2)
}

/// The width at the mouth, in km (FMG `getWidth`).
fn mouth_width(offset: f64) -> f64 {
    rn(libm::pow(offset / 1.5, 1.8), 2)
}

/// A FMG river before it becomes a `River`.
struct Raw {
    id: u32,
    course: Option<usize>,
    /// The cells, with -1 for the frame edge. The last entry is the mouth
    /// target.
    cells: Vec<i64>,
    parent: u32,
}

/// Run the river stage.
pub fn generate(input: &Input) -> Output {
    let grid = input.grid;
    let n = grid.len();
    let sea = input.sea_level;
    let features = input.features;
    let t = &features.distance;
    let feature_of = |c: usize| features.get(features.of_cell[c]);

    // The fixed courses: the cells of each course and their order.
    let mut course_of = vec![usize::MAX; n];
    for (k, course) in input.courses.iter().enumerate() {
        for &c in &course.cells {
            course_of[c] = k;
        }
    }
    let on_course = |c: usize| course_of[c] != usize::MAX;

    // Lakes, in feature order. A lake at the end of a course is closed.
    let mut lake_index = vec![usize::MAX; features.list.len()];
    let mut lakes: Vec<Lake> = Vec::new();
    let mut shorelines: Vec<&[usize]> = Vec::new();
    let mut lake_cells: Vec<u32> = Vec::new();
    let mut lake_first: Vec<usize> = Vec::new();
    for f in features.list.iter().filter(|f| f.kind == FeatureKind::Lake) {
        lake_index[f.id as usize - 1] = lakes.len();
        lakes.push(Lake {
            feature: f.id,
            height: f.height,
            closed: false,
            flux: 0.0,
            evaporation: 0.0,
            temp: 0.0,
            inlets: Vec::new(),
            outlet: None,
        });
        shorelines.push(&f.shoreline);
        lake_cells.push(f.cells);
        lake_first.push(f.first_cell as usize);
    }
    let lake_of = |c: usize| -> Option<usize> {
        let k = lake_index[features.of_cell[c] as usize - 1];
        (k != usize::MAX).then_some(k)
    };
    let mut course_lake = vec![false; lakes.len()];
    for course in input.courses {
        if let CourseEnd::Into(e) = course.end
            && let Some(k) = lake_of(e)
        {
            course_lake[k] = true;
        }
    }

    // 1. The tilt toward the coast, and the fixed elevations of the
    // courses.
    let alter = |lakes: &[Lake]| -> Vec<f64> {
        let mut h: Vec<f64> = (0..n)
            .map(|c| {
                let hc = input.heights[c];
                if hc < sea || t[c] < 1 {
                    return hc;
                }
                let (sum, count) = grid
                    .neighbors(c)
                    .fold((0.0, 0.0), |(s, k), m| (s + f64::from(t[m]), k + 1.0));
                let mean = if count > 0.0 { sum / count } else { 0.0 };
                hc + f64::from(t[c]) / 100.0 + mean / 10000.0
            })
            .collect();
        fix_courses(input, &mut h, lakes, &lake_of);
        h
    };
    let mut h = alter(&lakes);

    // 2. Closed lakes.
    let limit = input.lake_elevation_limit;
    for (k, lake) in lakes.iter_mut().enumerate() {
        let max_elevation = lake.height + limit;
        if course_lake[k] {
            lake.closed = true;
            continue;
        }
        if max_elevation > 99.0 || shorelines[k].is_empty() {
            lake.closed = false;
            continue;
        }
        let lowest = lowest_cell(shorelines[k], &h);
        let mut deep = true;
        let mut checked = vec![false; n];
        checked[lowest] = true;
        let mut stack = vec![lowest];
        while deep {
            let Some(c) = stack.pop() else { break };
            for m in grid.neighbors(c) {
                if checked[m] || h[m] >= max_elevation {
                    continue;
                }
                if h[m] < sea {
                    let other = feature_of(m);
                    if other.kind == FeatureKind::Ocean || lake.height > other.height {
                        deep = false;
                    }
                }
                checked[m] = true;
                stack.push(m);
            }
        }
        lake.closed = deep;
    }

    // 3. Fill the depressions.
    resolve_depressions(
        input,
        &mut h,
        &mut lakes,
        &shorelines,
        &lake_of,
        &on_course,
        &alter,
    );

    // 4. Drain the water.
    let modifier = libm::pow(input.cells_desired / 10000.0, 0.25);
    let mut fl = vec![0.0f64; n];
    let mut r = vec![0u32; n];
    let mut conf = vec![0u8; n];
    let mut rivers_data: BTreeMap<u32, Vec<i64>> = BTreeMap::new();
    let mut river_parents: BTreeMap<u32, u32> = BTreeMap::new();
    let mut river_next: u32 = 1;
    let mut receiver = vec![NONE; n];
    // Lake climate (FMG `defineClimateData`).
    let mut out_cell = vec![None; lakes.len()];
    let mut lake_out = vec![usize::MAX; n];
    let mut lake_river: Vec<Option<u32>> = vec![None; lakes.len()];
    let mut entering_flux = vec![0.0; lakes.len()];
    for (k, lake) in lakes.iter_mut().enumerate() {
        let shore = shorelines[k];
        lake.flux = shore.iter().map(|&c| input.precipitation[c]).sum();
        lake.temp = if lake_cells[k] < 6 || shore.is_empty() {
            input.temperature[lake_first[k]]
        } else {
            rn(
                shore.iter().map(|&c| input.temperature[c]).sum::<f64>() / shore.len() as f64,
                1,
            )
        };
        let metres = libm::pow(lake.height - 18.0, crate::climate::HEIGHT_EXPONENT);
        let evaporation =
            ((700.0 * (lake.temp + 0.006 * metres)) / 50.0 + 75.0) / (80.0 - lake.temp);
        lake.evaporation = rn(evaporation * f64::from(lake_cells[k]), 0);
        if lake.closed || shore.is_empty() {
            continue;
        }
        let c = lowest_cell(shore, &h);
        out_cell[k] = Some(c);
        lake_out[c] = k;
    }
    let mut land: Vec<usize> = (0..n).filter(|&c| h[c] >= sea).collect();
    land.sort_by(|&a, &b| h[b].total_cmp(&h[a]));

    let add_cell = |data: &mut BTreeMap<u32, Vec<i64>>, cell: i64, river: u32| {
        data.entry(river).or_default().push(cell);
    };
    for &i in &land {
        fl[i] = u16(fl[i] + input.precipitation[i] / modifier);

        // A lake that drains here.
        let outflowing: Vec<usize> = if lake_out[i] != usize::MAX {
            (0..lakes.len())
                .filter(|&k| out_cell[k] == Some(i) && lakes[k].flux > lakes[k].evaporation)
                .collect()
        } else {
            Vec::new()
        };
        for &k in &outflowing {
            let id = lakes[k].feature;
            let Some(lake_cell) = grid
                .neighbors(i)
                .find(|&c| h[c] < sea && features.of_cell[c] == id)
            else {
                continue;
            };
            fl[lake_cell] = u16(fl[lake_cell] + (lakes[k].flux - lakes[k].evaporation).max(0.0));
            if Some(r[lake_cell]) != lake_river[k] {
                let same =
                    lake_river[k].is_some_and(|lr| grid.neighbors(lake_cell).any(|c| r[c] == lr));
                if same {
                    let lr = lake_river[k].expect("a same river is a river");
                    r[lake_cell] = lr;
                    add_cell(&mut rivers_data, lake_cell as i64, lr);
                } else {
                    r[lake_cell] = river_next;
                    add_cell(&mut rivers_data, lake_cell as i64, river_next);
                    river_next += 1;
                }
            }
            let outlet = r[lake_cell];
            lakes[k].outlet = Some(outlet);
            let flux = fl[lake_cell];
            flow_down(
                &mut FlowState {
                    fl: &mut fl,
                    r: &mut r,
                    conf: &mut conf,
                    data: &mut rivers_data,
                    parents: &mut river_parents,
                    lakes: &mut lakes,
                    lake_river: &mut lake_river,
                    entering_flux: &mut entering_flux,
                },
                &FlowEnv {
                    h: &h,
                    sea,
                    lake_of: &lake_of,
                },
                i,
                flux,
                outlet,
            );
        }
        // All rivers into the drained lakes are tributaries of the outlet.
        if let Some(&first) = outflowing.first()
            && let Some(outlet) = lakes[first].outlet
        {
            for &k in &outflowing {
                for &inlet in &lakes[k].inlets {
                    river_parents.insert(inlet, outlet);
                }
            }
        }

        let course = course_of[i];
        if course == usize::MAX && grid.border[i] && r[i] != 0 {
            add_cell(&mut rivers_data, -1, r[i]);
            continue;
        }

        // The cell that gets the water.
        let min = if course != usize::MAX {
            let cells = &input.courses[course].cells;
            let at = cells
                .iter()
                .position(|&c| c == i)
                .expect("a course cell is on its course");
            match cells.get(at + 1) {
                Some(&next) => next,
                None => match input.courses[course].end {
                    CourseEnd::Into(e) => e,
                    CourseEnd::Sink => NONE,
                },
            }
        } else if lake_out[i] != usize::MAX {
            let drained: Vec<u32> = outflowing.iter().map(|&k| lakes[k].feature).collect();
            grid.neighbors(i)
                .filter(|&c| !drained.contains(&features.of_cell[c]))
                .min_by(|&a, &b| h[a].total_cmp(&h[b]))
                .unwrap_or(NONE)
        } else if features.haven[i] != NO_HAVEN {
            features.haven[i] as usize
        } else {
            grid.neighbors(i)
                .min_by(|&a, &b| h[a].total_cmp(&h[b]))
                .unwrap_or(NONE)
        };
        if min == NONE || (course == usize::MAX && h[i] <= h[min]) {
            continue;
        }
        receiver[i] = min;

        if fl[i] < MIN_FLUX_TO_FORM_RIVER {
            if h[min] >= sea {
                fl[min] = u16(fl[min] + fl[i]);
            }
            continue;
        }
        if r[i] == 0 {
            r[i] = river_next;
            add_cell(&mut rivers_data, i as i64, river_next);
            river_next += 1;
        }
        let (flux, river) = (fl[i], r[i]);
        flow_down(
            &mut FlowState {
                fl: &mut fl,
                r: &mut r,
                conf: &mut conf,
                data: &mut rivers_data,
                parents: &mut river_parents,
                lakes: &mut lakes,
                lake_river: &mut lake_river,
                entering_flux: &mut entering_flux,
            },
            &FlowEnv {
                h: &h,
                sea,
                lake_of: &lake_of,
            },
            min,
            flux,
            river,
        );
    }

    // 5. Rivers of 3 or more cells (FMG `defineRivers`).
    let mut raws: Vec<Raw> = rivers_data
        .into_iter()
        .filter(|(_, cells)| cells.len() >= 3)
        .map(|(id, cells)| Raw {
            id,
            course: None,
            parent: river_parents.get(&id).copied().unwrap_or(0),
            cells,
        })
        .collect();

    // 6. The pinned courses are locked rivers (FMG `mergeLocked`).
    if !input.courses.is_empty() {
        let first_id = river_next;
        let mut locked: Vec<Raw> = Vec::new();
        let mut locked_at: BTreeMap<usize, u32> = BTreeMap::new();
        for (k, course) in input.courses.iter().enumerate() {
            let id = first_id + k as u32;
            let mut cells: Vec<i64> = course.cells.iter().map(|&c| c as i64).collect();
            if let CourseEnd::Into(e) = course.end {
                cells.push(e as i64);
            }
            for &c in &course.cells {
                locked_at.entry(c).or_insert(id);
            }
            locked.push(Raw {
                id,
                course: Some(k),
                cells,
                parent: 0,
            });
        }
        let is_land = |c: i64| c >= 0 && input.heights[c as usize] >= sea;
        let mut generated = Vec::new();
        for raw in raws {
            let land: Vec<i64> = raw.cells.iter().copied().filter(|&c| is_land(c)).collect();
            let on = land
                .iter()
                .filter(|&&c| locked_at.contains_key(&(c as usize)))
                .count();
            if on * 2 >= land.len() {
                continue;
            }
            let junction = raw
                .cells
                .iter()
                .position(|&c| c >= 0 && locked_at.contains_key(&(c as usize)));
            match junction {
                None => generated.push(raw),
                Some(j) if j < 2 => {}
                Some(j) => {
                    let cells = raw.cells[..=j].to_vec();
                    let parent = locked_at[&(cells[j] as usize)];
                    generated.push(Raw {
                        cells,
                        parent,
                        ..raw
                    });
                }
            }
        }
        // A locked river carries no less water downstream than upstream.
        for lr in &mut locked {
            let mut upstream = 0.0;
            for &c in &lr.cells {
                if is_land(c) {
                    let c = c as usize;
                    fl[c] = fl[c].max(upstream);
                    upstream = fl[c];
                }
            }
            let end = *lr.cells.last().expect("a course has cells");
            if is_land(end)
                && let Some(&other) = locked_at.get(&(end as usize))
                && other != lr.id
            {
                lr.parent = other;
            }
        }
        locked.extend(generated);
        raws = locked;
    }

    let confluence = confluence_flux(grid, &raws, &|c| is_land_cell(input, c), &h, &fl);

    // Cell owners after the merge: the first river that claims a land cell.
    let mut owner = vec![0u32; n];
    for raw in &raws {
        // The cells of a river without its mouth target. A course has its
        // own cells, also the last one of a course that ends in a sink.
        let own = match raw.course {
            Some(k) => input.courses[k].cells.len(),
            None => raw.cells.len() - 1,
        };
        for &c in &raw.cells[..own] {
            if c >= 0 && owner[c as usize] == 0 {
                owner[c as usize] = raw.id;
            }
        }
    }

    // 7. The heights: the tilt and the fill (FMG truncates them to whole
    // numbers), then the cuts of the rivers. A course keeps its heights.
    let mut heights: Vec<f64> = (0..n)
        .map(|c| {
            if on_course(c) || input.heights[c] < sea {
                input.heights[c]
            } else {
                h[c].trunc().clamp(sea, 100.0)
            }
        })
        .collect();
    for c in 0..n {
        if heights[c] < sea + 15.0 || fl[c] == 0.0 || on_course(c) {
            continue;
        }
        let higher: Vec<usize> = grid
            .neighbors(c)
            .filter(|&m| heights[m] > heights[c])
            .collect();
        if higher.is_empty() {
            continue;
        }
        let higher_flux = higher.iter().map(|&m| fl[m]).sum::<f64>() / higher.len() as f64;
        if higher_flux == 0.0 {
            continue;
        }
        let cut = (fl[c] / higher_flux).floor();
        if cut > 0.0 {
            heights[c] -= cut.min(MAX_DOWNCUT);
        }
    }

    // The rivers of inkland: ids from 1, parents, basins, widths, lines.
    let default_width_factor = rn(1.0 / libm::pow(input.cells_desired / 10000.0, 0.25), 2);
    let new_id: BTreeMap<u32, u32> = raws
        .iter()
        .enumerate()
        .map(|(i, raw)| (raw.id, i as u32 + 1))
        .collect();
    let mut rivers: Vec<River> = Vec::with_capacity(raws.len());
    let mut anchors_of: Vec<Vec<Anchor>> = Vec::with_capacity(raws.len());
    for raw in &raws {
        let id = new_id[&raw.id];
        let parent = (raw.parent != 0 && raw.parent != raw.id)
            .then(|| new_id.get(&raw.parent).copied())
            .flatten();
        let last = *raw.cells.last().expect("a river has cells");
        let body: Vec<usize> = raw.cells[..raw.cells.len() - 1]
            .iter()
            .filter(|&&c| c >= 0)
            .map(|&c| c as usize)
            .collect();
        let mouth = if last < 0 {
            Mouth::Edge
        } else if input.heights[last as usize] < sea {
            Mouth::Water(last as usize)
        } else if owner[last as usize] != 0 && owner[last as usize] != raw.id {
            Mouth::Join(last as usize)
        } else {
            Mouth::Sink
        };
        let (cells, mouth) = match (raw.course, mouth) {
            (Some(k), _) => {
                let course = &input.courses[k];
                let m = match course.end {
                    CourseEnd::Into(e) if input.heights[e] < sea => Mouth::Water(e),
                    CourseEnd::Into(e) => Mouth::Join(e),
                    CourseEnd::Sink => Mouth::Sink,
                };
                (course.cells.clone(), m)
            }
            (None, Mouth::Sink) => {
                let mut c = body.clone();
                c.push(last as usize);
                (c, Mouth::Sink)
            }
            (None, m) => (body.clone(), m),
        };
        let Some(&end_cell) = cells.last() else {
            continue;
        };
        let mut points: Vec<[f64; 2]> = cells.iter().map(|&c| grid.points[c]).collect();
        let end = match (raw.course.and_then(|k| input.courses[k].end_point), mouth) {
            (Some(p), _) => Some(p),
            (None, Mouth::Water(w)) => shared_edge_midpoint(grid, end_cell, w),
            (None, Mouth::Join(j)) => Some(grid.points[j]),
            (None, Mouth::Edge) => Some(nearest_edge_point(
                grid.points[end_cell],
                grid.width,
                grid.height,
            )),
            (None, Mouth::Sink) => None,
        };
        if let Some(p) = end {
            points.push(p);
        }
        let discharge = fl[end_cell];
        let width_factor = if parent.is_none() {
            default_width_factor * 1.2
        } else {
            default_width_factor
        };
        let sw = source_width(fl[cells[0]]);
        // The anchors of the line, for the meander after this loop.
        let mut marks: Vec<Anchor> = points
            .iter()
            .zip(&cells)
            .map(|(&at, &c)| Anchor {
                at,
                water: input.heights[c] < sea,
                edge: false,
            })
            .collect();
        if let Some(at) = end {
            marks.push(Anchor {
                at,
                water: matches!(mouth, Mouth::Water(_)),
                edge: mouth == Mouth::Edge,
            });
        }
        anchors_of.push(marks);
        rivers.push(River {
            id,
            course: raw.course,
            cells,
            points,
            anchors: Vec::new(),
            mouth,
            parent,
            basin: id,
            flux: discharge,
            source_width: sw,
            width_factor,
            width: 0.0,
            length: 0.0,
        });
    }
    // The meandered lines (FMG `addMeandering`), the lengths, and the
    // widths at the mouth, which grow with the number of line points.
    let scale = FMG_SPACING / grid.lattice.spacing;
    let lines: Vec<Line> = rivers
        .iter()
        .zip(&anchors_of)
        .map(|(river, marks)| {
            let start_step = if input.heights[river.cells[0]] < sea {
                1
            } else {
                10
            };
            let join = match river.mouth {
                Mouth::Join(j) => join_direction(&rivers, &anchors_of, river.id, j),
                _ => None,
            };
            meander(marks, start_step, scale, join)
        })
        .collect();
    for (river, line) in rivers.iter_mut().zip(lines) {
        river.length = line
            .points
            .windows(2)
            .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
            .sum();
        river.width = mouth_width(offset(
            river.flux,
            line.points.len(),
            river.width_factor,
            river.source_width,
        ));
        river.points = line.points;
        river.anchors = line.anchors;
    }
    // Basins: the end of the chain of parents (FMG `getBasin`).
    let parent_of: BTreeMap<u32, Option<u32>> = rivers.iter().map(|r| (r.id, r.parent)).collect();
    for river in &mut rivers {
        let mut b = river.id;
        let mut seen = vec![b];
        while let Some(Some(p)) = parent_of.get(&b) {
            if seen.contains(p) {
                break;
            }
            seen.push(*p);
            b = *p;
        }
        river.basin = b;
    }

    // Lake inlets and outlets by the new river ids (FMG `cleanupLakeData`).
    for lake in &mut lakes {
        lake.inlets = lake
            .inlets
            .iter()
            .filter_map(|i| new_id.get(i).copied())
            .collect();
        lake.inlets.dedup();
        lake.outlet = lake.outlet.and_then(|o| new_id.get(&o).copied());
    }
    for river in &rivers {
        let Some(k) = river.course else { continue };
        if let Some(&first) = river.cells.first()
            && let Some(j) = lake_of(first)
            && lakes[j].outlet.is_none()
        {
            lakes[j].outlet = Some(river.id);
        }
        if let CourseEnd::Into(e) = input.courses[k].end
            && let Some(j) = lake_of(e)
            && !lakes[j].inlets.contains(&river.id)
        {
            lakes[j].inlets.push(river.id);
        }
    }

    // The drainage elevation: lakes at their surface.
    let elevation: Vec<f64> = (0..n)
        .map(|c| lake_of(c).map_or(h[c], |k| lakes[k].height))
        .collect();
    Output {
        drainage: Drainage {
            elevation,
            receiver,
            flux: fl,
            confluence,
        },
        rivers,
        lakes,
        heights,
    }
}

/// The flow of the river that passes the cell `j`, where the river `id`
/// joins it (FMG `getJoinDirection`): from the anchor before `j` to the
/// anchor after it.
fn join_direction(
    rivers: &[River],
    anchors: &[Vec<Anchor>],
    id: u32,
    j: usize,
) -> Option<[f64; 2]> {
    rivers.iter().zip(anchors).find_map(|(r, a)| {
        if r.id == id {
            return None;
        }
        let k = r.cells.iter().position(|&c| c == j)?;
        let next = a.get(k + 1)?;
        let prev = a[k.saturating_sub(1)];
        Some([next.at[0] - prev.at[0], next.at[1] - prev.at[1]])
    })
}

/// `true` for a land cell of the river cells, where -1 is the frame edge.
fn is_land_cell(input: &Input, c: i64) -> bool {
    c >= 0 && input.heights[c as usize] >= input.sea_level
}

/// The water of the smaller rivers at each confluence (FMG
/// `calculateConfluenceFlux`). A land cell is a confluence when two rivers
/// claim it. A river claims its cells, then the cell that it flows into
/// (FMG `defineRivers` and `mergeLocked`). At a confluence, the flux of
/// the higher neighbors on a river adds up, without the largest one.
fn confluence_flux(
    grid: &Grid,
    raws: &[Raw],
    land: &dyn Fn(i64) -> bool,
    h: &[f64],
    fl: &[f64],
) -> Vec<f64> {
    let n = grid.len();
    let mut r = vec![0u32; n];
    let mut mark = vec![false; n];
    let mut claim = |c: i64, id: u32| {
        if !land(c) {
            return;
        }
        let c = c as usize;
        if r[c] != 0 && r[c] != id {
            mark[c] = true;
        } else {
            r[c] = id;
        }
    };
    for raw in raws {
        for &c in &raw.cells[..raw.cells.len() - 1] {
            claim(c, raw.id);
        }
    }
    for raw in raws {
        claim(*raw.cells.last().expect("a river has cells"), raw.id);
    }
    (0..n)
        .map(|i| {
            if !mark[i] {
                return 0.0;
            }
            let mut influx: Vec<f64> = grid
                .neighbors(i)
                .filter(|&c| r[c] != 0 && h[c] > h[i])
                .map(|c| fl[c])
                .collect();
            influx.sort_by(|a, b| b.total_cmp(a));
            u16(influx.iter().skip(1).sum())
        })
        .collect()
}

/// The cell with the lowest height; the first one of equal cells.
fn lowest_cell(cells: &[usize], h: &[f64]) -> usize {
    cells
        .iter()
        .copied()
        .reduce(|a, c| if h[c] < h[a] { c } else { a })
        .expect("a shoreline has cells")
}

/// Give the cells of the courses their fixed, strictly falling elevations.
/// A course that ends in another course gets its elevations after it.
fn fix_courses(
    input: &Input,
    h: &mut [f64],
    lakes: &[Lake],
    lake_of: &dyn Fn(usize) -> Option<usize>,
) {
    let courses = input.courses;
    let n = input.grid.len();
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
            CourseEnd::Into(t) if input.heights[t] < input.sea_level => {
                lake_of(t).map_or(input.heights[t], |k| lakes[k].height)
            }
            CourseEnd::Into(t) => h[t],
            CourseEnd::Sink => f64::NEG_INFINITY,
        };
        for (i, &c) in course.cells.iter().enumerate() {
            let k = (m - i) as f64;
            h[c] = (input.heights[c] + k * gap).max(base + k * gap);
        }
    }
}

/// Fill the depressions (FMG `resolveDepressions`). A course cell keeps its
/// fixed elevation.
fn resolve_depressions(
    input: &Input,
    h: &mut Vec<f64>,
    lakes: &mut [Lake],
    shorelines: &[&[usize]],
    lake_of: &dyn Fn(usize) -> Option<usize>,
    on_course: &dyn Fn(usize) -> bool,
    alter: &dyn Fn(&[Lake]) -> Vec<f64>,
) {
    let grid = input.grid;
    let sea = input.sea_level;
    let n = grid.len();
    let max_iterations = RESOLVE_DEPRESSIONS_STEPS as f64;
    let check_lake_max = max_iterations * 0.85;
    let elevate_lake_max = max_iterations * 0.75;
    let lake_cell: Vec<Option<usize>> = (0..n)
        .map(|c| {
            if input.heights[c] < sea {
                lake_of(c)
            } else {
                None
            }
        })
        .collect();
    let mut land: Vec<usize> = (0..n)
        .filter(|&c| h[c] >= sea && !grid.border[c] && !on_course(c))
        .collect();
    land.sort_by(|&a, &b| h[a].total_cmp(&h[b]));

    let mut progress: Vec<i64> = Vec::new();
    let mut depressions: i64 = i64::MAX;
    let mut previous: Option<i64> = None;
    let mut iteration = 0usize;
    while depressions != 0 && iteration < RESOLVE_DEPRESSIONS_STEPS {
        if progress.len() > 5 && progress.iter().sum::<i64>() > 0 {
            // Bad progress: stop, and take back the tilted heights.
            *h = alter(lakes);
            break;
        }
        depressions = 0;
        let it = iteration as f64;
        if it < check_lake_max {
            for (k, lake) in lakes.iter_mut().enumerate() {
                if lake.closed || shorelines[k].is_empty() {
                    continue;
                }
                let min_height = shorelines[k]
                    .iter()
                    .map(|&s| h[s])
                    .fold(f64::INFINITY, f64::min);
                if min_height >= 100.0 || lake.height > min_height {
                    continue;
                }
                if it > elevate_lake_max {
                    for &s in shorelines[k] {
                        if !on_course(s) {
                            h[s] = input.heights[s];
                        }
                    }
                    lake.height = shorelines[k]
                        .iter()
                        .map(|&s| h[s])
                        .fold(f64::INFINITY, f64::min)
                        - 1.0;
                    lake.closed = true;
                    continue;
                }
                depressions += 1;
                lake.height = min_height + 0.2;
            }
        }
        for &i in &land {
            let min_height = grid
                .neighbors(i)
                .map(|c| match lake_cell[c] {
                    Some(k) if lakes[k].height != 0.0 => lakes[k].height,
                    _ => h[c],
                })
                .fold(f64::INFINITY, f64::min);
            if min_height >= 100.0 || h[i] > min_height {
                continue;
            }
            depressions += 1;
            h[i] = min_height + 0.1;
        }
        if let Some(p) = previous {
            progress.push(depressions - p);
        }
        previous = Some(depressions);
        iteration += 1;
    }
}

/// The mutable state of the water flow.
struct FlowState<'a> {
    fl: &'a mut [f64],
    r: &'a mut [u32],
    /// The confluence marks of the drain (FMG `cells.conf`, a
    /// `Uint8Array` at this step).
    conf: &'a mut [u8],
    data: &'a mut BTreeMap<u32, Vec<i64>>,
    parents: &'a mut BTreeMap<u32, u32>,
    lakes: &'a mut [Lake],
    lake_river: &'a mut [Option<u32>],
    entering_flux: &'a mut [f64],
}

/// What the water flow reads.
struct FlowEnv<'a> {
    h: &'a [f64],
    sea: f64,
    lake_of: &'a dyn Fn(usize) -> Option<usize>,
}

/// Give the water of a river to a cell (FMG `flowDown`).
fn flow_down(s: &mut FlowState, env: &FlowEnv, to: usize, from_flux: f64, river: u32) {
    // The water of the river that is already in the cell, without the
    // water of its tributaries.
    let to_flux = s.fl[to] - f64::from(s.conf[to]);
    let to_river = s.r[to];
    let land = env.h[to] >= env.sea;
    if to_river != 0 {
        if from_flux > to_flux {
            s.conf[to] = add_u8(s.conf[to], s.fl[to]);
            if land {
                s.parents.insert(to_river, river);
            }
            s.r[to] = river;
        } else {
            s.conf[to] = add_u8(s.conf[to], from_flux);
            if land {
                s.parents.insert(river, to_river);
            }
        }
    } else {
        s.r[to] = river;
    }
    if land {
        s.fl[to] = u16(s.fl[to] + from_flux);
    } else if let Some(k) = (env.lake_of)(to) {
        if s.lake_river[k].is_none() || from_flux > s.entering_flux[k] {
            s.lake_river[k] = Some(river);
            s.entering_flux[k] = from_flux;
        }
        s.lakes[k].flux += from_flux;
        s.lakes[k].inlets.push(river);
    }
    s.data.entry(river).or_default().push(to as i64);
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

    fn run(g: &Grid, h: &[f64], courses: &[Course]) -> (Features, Output) {
        let f = Features::mark_pack(g, h, 20.0);
        let rain = vec![30.0; g.len()];
        let temp = vec![10.0; g.len()];
        let out = generate(&Input {
            grid: g,
            heights: h,
            sea_level: 20.0,
            features: &f,
            precipitation: &rain,
            temperature: &temp,
            courses,
            cells_desired: g.len() as f64,
            lake_elevation_limit: 20.0,
        });
        (f, out)
    }

    #[test]
    fn water_flows_down_to_the_ocean_on_a_dome() {
        let g = grid();
        let h = dome(&g);
        let (f, out) = run(&g, &h, &[]);
        let d = &out.drainage;
        for c in 0..g.len() {
            if !f.is_land(c) {
                continue;
            }
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
        // A whole-number flux.
        assert!(d.flux.iter().all(|f| f.fract() == 0.0));
    }

    #[test]
    fn rivers_have_three_cells_flow_down_and_end_in_water_or_a_river() {
        let g = grid();
        let h = dome(&g);
        let (f, out) = run(&g, &h, &[]);
        assert!(out.rivers.len() >= 5, "{}", out.rivers.len());
        let ids: Vec<u32> = out.rivers.iter().map(|r| r.id).collect();
        assert_eq!(ids, (1..=out.rivers.len() as u32).collect::<Vec<_>>());
        for r in &out.rivers {
            assert!(r.cells.len() >= 2, "river {} has {:?}", r.id, r.cells);
            for w in r.cells.windows(2) {
                let (a, b) = (out.drainage.elevation[w[0]], out.drainage.elevation[w[1]]);
                // Two cells of one lake have the height of the lake.
                let same_lake = !f.is_land(w[0]) && f.of_cell[w[0]] == f.of_cell[w[1]];
                assert!(b < a || (same_lake && b == a), "river {} goes up", r.id);
            }
            match r.mouth {
                Mouth::Water(w) => assert!(!f.is_land(w)),
                Mouth::Join(j) => assert!(
                    out.rivers
                        .iter()
                        .any(|o| o.id != r.id && o.cells.contains(&j))
                ),
                Mouth::Sink | Mouth::Edge => {}
            }
            assert!(r.flux >= MIN_FLUX_TO_FORM_RIVER);
            assert!(r.width > 0.0 && r.source_width >= 0.0);
            // The basin is the end of the parent chain.
            let mut b = r.id;
            while let Some(p) = out.rivers[b as usize - 1].parent {
                b = p;
            }
            assert_eq!(r.basin, b);
        }
        // A tributary is narrower at its start than its main river at the
        // mouth.
        let main = out
            .rivers
            .iter()
            .max_by(|a, b| a.flux.total_cmp(&b.flux))
            .unwrap();
        assert!(
            main.width
                >= out
                    .rivers
                    .iter()
                    .map(|r| r.source_width)
                    .fold(0.0, f64::max)
        );
    }

    #[test]
    fn rivers_cut_their_beds_and_heights_stay_whole() {
        let g = grid();
        let h = dome(&g);
        let (f, out) = run(&g, &h, &[]);
        let mut cut = 0;
        for c in 0..g.len() {
            assert_eq!(out.heights[c].fract(), 0.0);
            assert_eq!(out.heights[c] >= 20.0, f.is_land(c));
            if out.heights[c] < h[c] {
                cut += 1;
                assert!(h[c] - out.heights[c] <= 5.0);
                assert!(h[c] >= 35.0);
            }
        }
        assert!(cut > 0);
    }

    #[test]
    fn a_lake_with_more_water_than_evaporation_drains() {
        let g = grid();
        // A dome with a crater lake on its east side.
        let mut h = dome(&g);
        for (c, p) in g.points.iter().enumerate() {
            if ((p[0] - 240.0).powi(2) + (p[1] - 150.0).powi(2)).sqrt() < 15.0 {
                h[c] = 10.0;
            }
        }
        let (f, out) = run(&g, &h, &[]);
        assert_eq!(out.lakes.len(), 1);
        let lake = &out.lakes[0];
        assert!(lake.flux > lake.evaporation, "{lake:?}");
        assert!(!lake.closed);
        let outlet = lake.outlet.expect("the lake drains");
        let river = &out.rivers[outlet as usize - 1];
        assert!(river.cells.iter().any(|&c| f.of_cell[c] == lake.feature));
    }

    #[test]
    fn a_course_is_kept_and_other_water_joins_it() {
        let g = grid();
        let h = dome(&g);
        let f = Features::mark_pack(&g, &h, 20.0);
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
        let (_, out) = run(&g, &h, std::slice::from_ref(&course));
        let pinned: Vec<&River> = out.rivers.iter().filter(|r| r.course == Some(0)).collect();
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].id, 1);
        assert_eq!(pinned[0].cells, cells);
        assert_eq!(pinned[0].mouth, Mouth::Water(water));
        for w in cells.windows(2) {
            assert!(out.drainage.elevation[w[1]] < out.drainage.elevation[w[0]]);
            assert_eq!(out.drainage.receiver[w[0]], w[1]);
            assert_eq!(out.heights[w[0]], h[w[0]]);
        }
        // No other river uses a cell of the course except to join it.
        for r in out.rivers.iter().filter(|r| r.course.is_none()) {
            assert!(r.cells.iter().all(|c| !cells.contains(c)));
            if let Mouth::Join(j) = r.mouth
                && cells.contains(&j)
            {
                assert_eq!(r.parent, Some(1));
            }
        }
    }

    #[test]
    fn a_sink_course_keeps_its_water() {
        let g = grid();
        let h = dome(&g);
        let a = g.find_cell(200.0, 150.0);
        let b = g.neighbors(a).next().unwrap();
        let course = Course {
            cells: vec![a, b],
            end: CourseEnd::Sink,
            end_point: None,
        };
        let (_, out) = run(&g, &h, std::slice::from_ref(&course));
        assert_eq!(out.drainage.receiver[b], NONE);
        assert!(out.drainage.elevation[a] > out.drainage.elevation[b]);
        assert_eq!(out.rivers[0].mouth, Mouth::Sink);
    }

    #[test]
    fn a_confluence_holds_the_water_of_the_smaller_rivers() {
        let g = grid();
        let h = dome(&g);
        let (_, out) = run(&g, &h, &[]);
        let d = &out.drainage;
        let joins: Vec<usize> = out
            .rivers
            .iter()
            .filter_map(|r| match r.mouth {
                Mouth::Join(j) => Some(j),
                _ => None,
            })
            .collect();
        assert!(d.confluence.iter().any(|&c| c > 0.0));
        for c in 0..g.len() {
            if d.confluence[c] == 0.0 {
                continue;
            }
            // Only a cell where a river ends on another river.
            assert!(joins.contains(&c), "cell {c}");
            // FMG adds the water of each higher neighbor on a river, also
            // one that does not flow into the cell, without the largest.
            let mut higher: Vec<f64> = g
                .neighbors(c)
                .filter(|&m| out.rivers.iter().any(|r| r.cells.contains(&m)))
                .map(|m| d.flux[m])
                .collect();
            higher.sort_by(|a, b| b.total_cmp(a));
            assert!(
                d.confluence[c] <= higher.iter().skip(1).sum::<f64>(),
                "cell {c}"
            );
            assert_eq!(d.confluence[c].fract(), 0.0);
        }
    }

    #[test]
    fn a_confluence_compares_the_own_water_of_the_river() {
        // Cell 0 is on river 1 with 100 water, and 80 of it comes from
        // tributaries. River 2 brings 50: more than the own 20 of river 1,
        // so river 2 keeps its id downstream.
        let (mut fl, mut r, mut conf) = (vec![100.0], vec![1u32], vec![80u8]);
        let (mut data, mut parents) = (BTreeMap::new(), BTreeMap::new());
        let (mut lakes, mut lake_river, mut entering): (Vec<Lake>, Vec<Option<u32>>, Vec<f64>) =
            (Vec::new(), Vec::new(), Vec::new());
        let h = [50.0];
        let no_lake = |_: usize| None;
        let env = FlowEnv {
            h: &h,
            sea: 20.0,
            lake_of: &no_lake,
        };
        let mut state = FlowState {
            fl: &mut fl,
            r: &mut r,
            conf: &mut conf,
            data: &mut data,
            parents: &mut parents,
            lakes: &mut lakes,
            lake_river: &mut lake_river,
            entering_flux: &mut entering,
        };
        flow_down(&mut state, &env, 0, 50.0, 2);
        assert_eq!(r[0], 2);
        assert_eq!(parents.get(&1), Some(&2));
        assert_eq!(fl[0], 150.0);
        // The mark is a byte: 80 + 100 = 180.
        assert_eq!(conf[0], 180);
        // The own water of river 2 is now 150 - 180 < 0, so river 3 takes
        // the id, as in FMG. The mark grows by 150 and wraps at 256.
        let mut state = FlowState {
            fl: &mut fl,
            r: &mut r,
            conf: &mut conf,
            data: &mut data,
            parents: &mut parents,
            lakes: &mut lakes,
            lake_river: &mut lake_river,
            entering_flux: &mut entering,
        };
        flow_down(&mut state, &env, 0, 100.0, 3);
        assert_eq!(r[0], 3);
        assert_eq!(parents.get(&2), Some(&3));
        assert_eq!(conf[0], 74);
    }

    #[test]
    fn a_river_line_meanders_between_its_cell_sites() {
        let g = grid();
        let h = dome(&g);
        let (_, out) = run(&g, &h, &[]);
        let mut bent = 0;
        for r in &out.rivers {
            // Each cell site is an anchor, in order; the end point follows.
            let ends = usize::from(r.mouth != Mouth::Sink);
            assert_eq!(r.anchors.len(), r.cells.len() + ends, "river {}", r.id);
            assert!(r.anchors.windows(2).all(|w| w[0] < w[1]));
            assert_eq!(r.anchors[0], 0);
            assert_eq!(*r.anchors.last().unwrap(), r.points.len() - 1);
            for (k, &c) in r.cells.iter().enumerate() {
                assert_eq!(r.points[r.anchors[k]], g.points[c]);
            }
            bent += r.points.len() - r.anchors.len();
            // The width grows with the number of line points (FMG `getWidth`).
            let o = offset(r.flux, r.points.len(), r.width_factor, r.source_width);
            assert_eq!(r.width, mouth_width(o));
            let length: f64 = r
                .points
                .windows(2)
                .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
                .sum();
            assert_eq!(r.length, length);
        }
        assert!(bent > 0);
    }

    #[test]
    fn widths_follow_the_fmg_formulas() {
        assert_eq!(source_width(0.0), 0.0);
        assert_eq!(source_width(1e9), 1.0);
        assert_eq!(offset(100.0, 0, 1.0, 0.3), 0.3);
        // pointIndex 2: 2/200 + 2/200, flux 100^0.7/500.
        let o = offset(100.0, 2, 1.0, 0.0);
        assert!((o - (0.02 + libm::pow(100.0, 0.7) / 500.0)).abs() < 1e-12);
        assert_eq!(mouth_width(1.5), 1.0);
    }
}
