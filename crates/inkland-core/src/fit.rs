//! Fit the terrain around the pins.
//!
//! The template makes the free terrain first. Then this module forces the
//! heights inside pinned regions and around pinned places. A pin never
//! moves. If the terrain cannot satisfy a pin, the result is an error with a
//! specific code.
//!
//! Regions: each cell whose site is inside a region gets a height in the
//! range of the region. Where regions overlap, the regions with the highest
//! `priority` rule, and the cell gets a height in all of their ranges. The template height is scaled into the range, so the
//! terrain keeps its form. `forest` and `desert` regions without a `height`
//! only make sure of land. Outside a region with an `edge`, the heights blend
//! from the region to the free terrain over the edge width.
//!
//! Places: the cell of each place is land. Its neighbors keep their terrain,
//! so a place can be on the coast. A port needs one water cell next to it.
//! If it has none, a short channel connects it to the nearest water. The
//! channel does not go through the cell of a place or through forced land.

use crate::config::{Point, PointPin, RegionKind, RegionPin};
use crate::error::{Error, ErrorCode};
use crate::geom::{Shape, clip_polygon, dist, segment_distance};
use crate::grid::Grid;
use crate::noise::Fractal;
use crate::seed::{Layer, SeedTree};
use std::collections::VecDeque;

/// The longest channel, in cells, that may connect a port to water.
pub const MAX_PORT_CHANNEL: usize = 8;

/// A region with its shape and its height range on the 0 to 100 scale.
pub struct FitRegion<'a> {
    pub index: usize,
    pub pin: &'a RegionPin,
    pub shape: Shape,
    bbox: [f64; 4],
    /// The height range, with water below sea level and land at or above it.
    pub range: [f64; 2],
    /// `true` if the region only makes sure of land.
    pub land_only: bool,
    /// `true` if the region lifts the water of the template to low land
    /// (a `land` region without a `height`).
    lift: bool,
    /// The noise of a rough outline.
    rough: Option<Rough>,
}

/// A rough outline: the shape, moved in or out by fractal noise.
struct Rough {
    /// The largest distance that the outline moves, in units.
    amount: f64,
    noise: Fractal,
    /// The sides of a polygon that are coast: the sides of the part of the
    /// polygon in the frame, without the sides on a frame edge.
    coast: Vec<(Point, Point)>,
}

impl Rough {
    fn of(pin: &RegionPin, shape: &Shape, frame: [f64; 2], tree: &SeedTree) -> Option<Rough> {
        let amount = pin.roughness;
        if !(amount.is_finite() && amount > 0.0) {
            return None;
        }
        let [w, h] = frame;
        let coast = match shape {
            Shape::Polygon(poly) => {
                let inside = clip_polygon(poly, [0.0, 0.0, w, h]);
                let n = inside.len();
                let on_frame = |a: Point, b: Point| {
                    (a[0] == b[0] && (a[0] <= 0.0 || a[0] >= w))
                        || (a[1] == b[1] && (a[1] <= 0.0 || a[1] >= h))
                };
                (0..n)
                    .map(|i| (inside[i], inside[(i + 1) % n]))
                    .filter(|&(a, b)| !on_frame(a, b))
                    .collect()
            }
            Shape::Circle { .. } => Vec::new(),
        };
        let seed = Fractal::seed_of(tree.part_seed(Layer::Terrain, &format!("coast/{}", pin.id)));
        let octaves = pin.coast_detail.unwrap_or(4).clamp(1, 8);
        Some(Rough {
            amount,
            noise: Fractal::new(seed, octaves, 4.0 * amount),
            coast,
        })
    }

    /// The distance from `p` to the nearest coast side of the region.
    #[cfg(test)]
    fn coast_distance(region: &FitRegion, p: Point) -> f64 {
        let r = region.rough.as_ref().expect("a rough region");
        r.coast
            .iter()
            .map(|&(a, b)| segment_distance(a, b, p))
            .fold(f64::INFINITY, f64::min)
    }

    /// The distance from `p` to the rough outline: less than 0 inside, more
    /// than 0 outside.
    fn signed(&self, shape: &Shape, p: Point) -> f64 {
        let d = match shape {
            Shape::Circle { at, r } => (dist(*at, p) - r).abs(),
            Shape::Polygon(_) => self
                .coast
                .iter()
                .map(|&(a, b)| segment_distance(a, b, p))
                .fold(f64::INFINITY, f64::min),
        };
        let s = if shape.contains(p) { -d } else { d };
        // Fractal noise is seldom far from 0. Twice the noise, cut at -1
        // and 1, moves the outline by up to the full amount.
        s - self.amount * (2.0 * self.noise.at(p)).clamp(-1.0, 1.0)
    }
}

impl<'a> FitRegion<'a> {
    /// The regions of a config. `frame` is the width and the height of the
    /// frame. The noise of a rough outline comes from `tree`.
    pub fn all(
        regions: &'a [RegionPin],
        sea_level: f64,
        frame: [f64; 2],
        tree: &SeedTree,
    ) -> Vec<FitRegion<'a>> {
        regions
            .iter()
            .enumerate()
            .filter_map(|(index, pin)| {
                let shape = Shape::of(pin)?;
                let rough = Rough::of(pin, &shape, frame, tree);
                let grow = rough.as_ref().map_or(0.0, |r| r.amount);
                let [x0, y0, x1, y1] = shape.bbox();
                let bbox = [x0 - grow, y0 - grow, x1 + grow, y1 + grow];
                Some(FitRegion {
                    index,
                    pin,
                    range: effective_range(pin, sea_level),
                    land_only: pin.height.is_none()
                        && matches!(
                            pin.kind,
                            RegionKind::Forest | RegionKind::Desert | RegionKind::Land
                        ),
                    lift: pin.height.is_none() && pin.kind == RegionKind::Land,
                    rough,
                    shape,
                    bbox,
                })
            })
            .collect()
    }

    /// `true` if `p` is inside the outline of the region. A rough outline
    /// is the shape moved by its noise.
    pub fn contains(&self, p: [f64; 2]) -> bool {
        let [x0, y0, x1, y1] = self.bbox;
        if !(p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1) {
            return false;
        }
        match &self.rough {
            Some(r) => r.signed(&self.shape, p) <= 0.0,
            None => self.shape.contains(p),
        }
    }

    fn near(&self, p: [f64; 2]) -> Option<f64> {
        let e = self.pin.edge;
        if e <= 0.0 {
            return None;
        }
        let [x0, y0, x1, y1] = self.bbox;
        if p[0] < x0 - e || p[0] > x1 + e || p[1] < y0 - e || p[1] > y1 + e {
            return None;
        }
        let d = match &self.rough {
            Some(r) => r.signed(&self.shape, p),
            None => self.shape.distance(p),
        };
        (d > 0.0 && d < e).then_some(d)
    }

    /// The forced height for a template height `h`.
    fn target(&self, h: f64, sea_level: f64) -> f64 {
        if self.land_only {
            return put(land_height(h, sea_level, self.lift), self.range);
        }
        let [lo, hi] = self.range;
        put(lo + (hi - lo) * h / 100.0, self.range)
    }
}

/// The height range of a region for a sea level: a water range ends below
/// sea level, and a land range starts at sea level or above.
pub fn effective_range(region: &RegionPin, sea_level: f64) -> [f64; 2] {
    let [lo, hi] = region.height_range();
    if region.kind.is_water() {
        let hi = hi.min(sea_level - 1.0);
        [lo.min(hi), hi]
    } else {
        let lo = lo.max(sea_level);
        [lo, hi.max(lo)]
    }
}

/// The regions that set the range of a cell: of the regions that hold it,
/// the ones with the highest priority.
fn ruling<'r, 'a>(inside: Vec<&'r FitRegion<'a>>) -> Vec<&'r FitRegion<'a>> {
    let top = inside.iter().map(|r| r.pin.priority).max();
    inside
        .into_iter()
        .filter(|r| Some(r.pin.priority) == top)
        .collect()
}

/// The regions that rule the point `p`: of the regions that hold it, the
/// ones with the highest priority. The list is empty outside all regions.
pub fn ruling_at<'r, 'a>(regions: &'r [FitRegion<'a>], p: [f64; 2]) -> Vec<&'r FitRegion<'a>> {
    ruling(regions.iter().filter(|r| r.contains(p)).collect())
}

/// `true` if a water region rules the point `p`. A land region with a
/// higher priority than the sea around it makes its cells land.
pub fn forced_water(regions: &[FitRegion], p: [f64; 2]) -> bool {
    ruling_at(regions, p).iter().any(|r| r.pin.kind.is_water())
}

/// `true` if only land regions rule the point `p`.
pub fn forced_land(regions: &[FitRegion], p: [f64; 2]) -> bool {
    let rule = ruling_at(regions, p);
    !rule.is_empty() && rule.iter().all(|r| !r.pin.kind.is_water())
}

/// The height of a cell that only needs to be land. With `lift`, water
/// becomes low land as with `fill_land`: deeper water gives a little higher
/// land, so the land keeps some form. Without it, water goes up to sea
/// level.
fn land_height(h: f64, sea_level: f64, lift: bool) -> f64 {
    if h >= sea_level {
        h
    } else if lift {
        (sea_level + 1.0 + 2.0 * (sea_level - h) / sea_level).trunc()
    } else {
        sea_level
    }
}

/// A whole-number height in the range.
fn put(v: f64, [lo, hi]: [f64; 2]) -> f64 {
    v.trunc().clamp(lo.ceil(), hi.floor().max(lo.ceil()))
}

/// Force the heights in and near the regions.
pub fn apply_regions(grid: &Grid, heights: &mut [f64], regions: &[FitRegion], sea_level: f64) {
    for c in 0..grid.len() {
        let p = grid.points[c];
        let inside = ruling(regions.iter().filter(|r| r.contains(p)).collect());
        if let Some(first) = inside.first() {
            let range = inside.iter().skip(1).fold(first.range, |[lo, hi], r| {
                [lo.max(r.range[0]), hi.min(r.range[1])]
            });
            let land_only = inside.iter().all(|r| r.land_only);
            let lift = inside.iter().any(|r| r.lift);
            heights[c] = if land_only {
                put(land_height(heights[c], sea_level, lift), range)
            } else {
                let [lo, hi] = range;
                put(lo + (hi - lo) * heights[c] / 100.0, range)
            };
            continue;
        }
        // The nearest region whose soft edge covers the cell.
        let nearest = regions
            .iter()
            .filter_map(|r| r.near(p).map(|d| (d, r)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, r)) = nearest {
            let t = d / r.pin.edge;
            let target = r.target(heights[c], sea_level);
            heights[c] = (target * (1.0 - t) + heights[c] * t)
                .trunc()
                .clamp(0.0, 100.0);
        }
    }
}

/// Put the height of each cell inside a region into the range of its
/// regions. A height that is in the range already does not change. This
/// holds the regions on the second grid, whose new cells take the height of
/// a cell of the first grid. The cells in `places` (the cells of the point
/// pins) stay land, also in a rough sea or as an island.
pub fn clamp_regions(
    grid: &Grid,
    heights: &mut [f64],
    regions: &[FitRegion],
    sea_level: f64,
    places: &[usize],
) {
    for c in 0..grid.len() {
        let p = grid.points[c];
        let inside = ruling(regions.iter().filter(|r| r.contains(p)).collect());
        let Some(first) = inside.first() else {
            continue;
        };
        let range = inside.iter().skip(1).fold(first.range, |[lo, hi], r| {
            [lo.max(r.range[0]), hi.min(r.range[1])]
        });
        let h = if inside.iter().all(|r| r.land_only) {
            land_height(heights[c], sea_level, inside.iter().any(|r| r.lift))
        } else {
            heights[c]
        };
        heights[c] = put(h, range);
        if places.contains(&c) {
            heights[c] = heights[c].max(sea_level);
        }
    }
}

/// A point pin and its cell.
pub struct FitPoint<'a> {
    pub index: usize,
    pub pin: &'a PointPin,
    pub cell: usize,
}

/// Make the places land, and give each port a water cell next to it.
pub fn apply_points(
    grid: &Grid,
    heights: &mut [f64],
    points: &[FitPoint],
    regions: &[FitRegion],
    sea_level: f64,
) -> Result<(), Error> {
    let n = grid.len();
    let in_land_region = |c: usize| forced_land(regions, grid.points[c]);

    // Cells that a port channel must not make water.
    let mut keep_land = vec![false; n];
    for p in points {
        keep_land[p.cell] = true;
        heights[p.cell] = heights[p.cell].max(sea_level);
    }
    for p in points.iter().filter(|p| p.pin.port) {
        if grid.neighbors(p.cell).any(|m| heights[m] < sea_level) {
            continue;
        }
        let channel = port_channel(grid, heights, p.cell, sea_level, &keep_land, &in_land_region)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::PortNotNearCoast,
                    format!("/pins/points/{}", p.index),
                    format!(
                        "the port {:?} has no water within {MAX_PORT_CHANNEL} cells that a channel can reach",
                        p.pin.id
                    ),
                )
                .with("max_channel_cells", MAX_PORT_CHANNEL)
            })?;
        for c in channel {
            heights[c] = heights[c].min(sea_level - 1.0);
        }
    }
    Ok(())
}

/// The shortest path of cells from a neighbor of `port` to a water cell.
/// The path does not use cells that must stay land.
fn port_channel(
    grid: &Grid,
    heights: &[f64],
    port: usize,
    sea_level: f64,
    keep_land: &[bool],
    in_land_region: &dyn Fn(usize) -> bool,
) -> Option<Vec<usize>> {
    let n = grid.len();
    let mut from = vec![usize::MAX; n];
    let mut depth = vec![0usize; n];
    let mut queue = VecDeque::new();
    from[port] = port;
    for m in grid.neighbors(port) {
        if !keep_land[m] && !in_land_region(m) {
            from[m] = port;
            depth[m] = 1;
            queue.push_back(m);
        }
    }
    while let Some(c) = queue.pop_front() {
        let water_next = grid
            .neighbors(c)
            .find(|&m| heights[m] < sea_level && m != port);
        if water_next.is_some() {
            let mut path = vec![c];
            let mut cur = c;
            while from[cur] != port {
                cur = from[cur];
                path.push(cur);
            }
            return Some(path);
        }
        if depth[c] >= MAX_PORT_CHANNEL {
            continue;
        }
        for m in grid.neighbors(c) {
            if from[m] == usize::MAX && !keep_land[m] && !in_land_region(m) {
                from[m] = c;
                depth[m] = depth[c] + 1;
                queue.push_back(m);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use serde_json::json;

    #[test]
    fn a_region_with_a_higher_priority_rules_where_regions_overlap() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 2, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 4000, "template": "pangea" },
            "pins": { "regions": [
                { "id": "lowland", "kind": "plains", "height": [22, 34], "box": [40, 30, 260, 170] },
                { "id": "wolds", "kind": "hills", "height": [40, 50], "box": [120, 70, 180, 130], "priority": 1 }
            ] }
        }))
        .unwrap();
        let w = crate::world::World::build(&c).unwrap();
        let inside =
            |b: [f64; 4], p: [f64; 2]| p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3];
        let (mut wolds, mut low) = (0, 0);
        for (cell, &p) in w.grid.points.iter().enumerate() {
            let h = w.heights[cell];
            if inside([120.0, 70.0, 180.0, 130.0], p) {
                assert!((40.0..=50.0).contains(&h), "wolds {h}");
                wolds += 1;
            } else if inside([40.0, 30.0, 260.0, 170.0], p) {
                assert!((22.0..=34.0).contains(&h), "lowland {h}");
                low += 1;
            }
        }
        assert!(wolds > 10 && low > 10);
    }

    #[test]
    fn a_town_on_an_island_in_a_pinned_sea_is_land() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 3, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 4000, "template": "pangea" },
            "pins": {
                "points": [{ "id": "portvale", "kind": "town", "at": [65, 100], "port": true }],
                "regions": [
                    { "id": "bay", "kind": "sea", "box": [0, 0, 120, 200] },
                    { "id": "isle", "kind": "hills", "height": [22, 28],
                      "circle": { "at": [60, 100], "r": 8 }, "priority": 1 }
                ]
            }
        }))
        .unwrap();
        let w = crate::world::World::build(&c).unwrap();
        let cell = w.pin_cells[0];
        assert!(w.heights[cell] >= 20.0, "the town is in water");
        assert!(
            w.grid.neighbors(cell).any(|m| w.heights[m] < 20.0),
            "the port has no water"
        );
        // The sea outside the island stays water. The second grid has no
        // cells in the deep sea, so look at the cells it has.
        let sea: Vec<f64> = (0..w.grid.len())
            .filter(|&c| {
                let [x, y] = w.grid.points[c];
                x < 110.0 && ((x - 60.0).powi(2) + (y - 100.0).powi(2)).sqrt() > 14.0
            })
            .map(|c| w.heights[c])
            .collect();
        assert!(!sea.is_empty() && sea.iter().all(|&h| h < 20.0));
    }

    #[test]
    fn an_island_place_in_a_pinned_sea_is_land_with_water_around_it() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 3, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 4000, "template": "pangea" },
            "pins": {
                "points": [{ "id": "rock", "kind": "fortress", "at": [60, 100],
                             "port": true, "island": true }],
                "regions": [{ "id": "bay", "kind": "sea", "box": [0, 0, 120, 200] }]
            }
        }))
        .unwrap();
        let w = crate::world::World::build(&c).unwrap();
        let cell = w.pin_cells[0];
        assert!(w.heights[cell] >= 20.0, "the island is water");
        assert!(w.grid.neighbors(cell).all(|m| w.heights[m] < 20.0));
    }

    #[test]
    fn an_island_flag_is_not_written_when_it_is_false() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 3, "frame": { "width": 300, "height": 200 },
            "pins": { "points": [{ "id": "a", "kind": "town", "at": [6, 1] }] }
        }))
        .unwrap();
        let v = serde_json::to_value(&c.pins.points[0]).unwrap();
        assert!(v.get("island").is_none());
    }

    #[test]
    fn a_land_region_draws_a_coast_on_a_template_of_sea() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 4, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 4000, "template": { "steps": [] } },
            "pins": { "regions": [
                { "id": "maeris", "kind": "land",
                  "polygon": [[60, 40], [240, 50], [220, 160], [80, 150]] }
            ] }
        }))
        .unwrap();
        let fit_regions = c.pins.regions.clone();
        let fit = FitRegion::all(&fit_regions, 20.0, [300.0, 200.0], &SeedTree::new(4));
        let w = crate::world::World::build(&c).unwrap();
        let (mut land, mut sea) = (0, 0);
        for (cell, &p) in w.grid.points.iter().enumerate() {
            if fit[0].contains(p) {
                assert!(w.heights[cell] >= 20.0, "water in the land at {p:?}");
                land += 1;
            } else if fit[0].shape.distance(p) > 6.0 {
                assert!(w.heights[cell] < 20.0, "land in the sea at {p:?}");
                sea += 1;
            }
        }
        assert!(land > 100 && sea > 10, "{land} {sea}");
    }

    #[test]
    fn land_height_lifts_water_only_with_lift() {
        assert_eq!(land_height(30.0, 20.0, true), 30.0);
        assert_eq!(land_height(0.0, 20.0, true), 23.0);
        assert_eq!(land_height(15.0, 20.0, true), 21.0);
        assert_eq!(land_height(0.0, 20.0, false), 20.0);
    }

    #[test]
    fn a_rough_outline_moves_by_at_most_the_roughness() {
        let r = regions(json!([
            { "id": "gulf", "kind": "sea", "polygon": [[20, 20], [80, 25], [75, 80], [25, 75]],
              "roughness": 4 }
        ]));
        let fit = FitRegion::all(&r, 20.0, [100.0, 100.0], &SeedTree::new(1));
        let (mut out, mut extra) = (0, 0);
        for i in 0..100 {
            for j in 0..100 {
                let p = [f64::from(i) + 0.5, f64::from(j) + 0.5];
                let exact = fit[0].shape.contains(p);
                let rough = fit[0].contains(p);
                if exact != rough {
                    // Only points near the exact outline change.
                    let d = if exact {
                        Rough::coast_distance(&fit[0], p)
                    } else {
                        fit[0].shape.distance(p)
                    };
                    assert!(d <= 4.0 + 1e-9, "{p:?} moved {d}");
                }
                out += usize::from(exact && !rough);
                extra += usize::from(!exact && rough);
            }
        }
        // The noise moves the outline both in and out.
        assert!(out > 20 && extra > 20, "{out} {extra}");
    }

    #[test]
    fn a_side_of_a_rough_outline_on_the_frame_edge_does_not_move() {
        let r = regions(json!([
            { "id": "west", "kind": "sea", "box": [0, 0, 30, 100], "roughness": 6 }
        ]));
        let fit = FitRegion::all(&r, 20.0, [100.0, 100.0], &SeedTree::new(1));
        // Along the west, north, and south frame edges, the sea stays.
        for k in 0..100 {
            let t = f64::from(k);
            assert!(fit[0].contains([0.0, t]), "west edge at {t}");
            assert!(fit[0].contains([t.min(20.0), 0.0]), "north edge at {t}");
        }
        // The east side moves.
        let moved = (0..100).any(|k| fit[0].contains([32.0, f64::from(k)]))
            && (0..100).any(|k| !fit[0].contains([28.0, f64::from(k)]));
        assert!(moved);
    }

    #[test]
    fn coast_detail_and_the_seed_change_a_rough_outline() {
        let make = |detail: u32, seed: u64| {
            let r = regions(json!([
                { "id": "gulf", "kind": "sea", "circle": { "at": [50, 50], "r": 30 },
                  "roughness": 5, "coast_detail": detail }
            ]));
            let fit = FitRegion::all(&r, 20.0, [100.0, 100.0], &SeedTree::new(seed));
            (0..10_000)
                .map(|k| fit[0].contains([f64::from(k % 100), f64::from(k / 100)]))
                .collect::<Vec<bool>>()
        };
        assert_eq!(make(4, 1), make(4, 1));
        assert_ne!(make(1, 1), make(8, 1));
        assert_ne!(make(4, 1), make(4, 2));
    }

    #[test]
    fn a_rough_land_region_keeps_its_places_on_land() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 5, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 4000, "template": { "steps": [] } },
            "pins": {
                "points": [
                    { "id": "cape", "kind": "town", "at": [62, 100] },
                    { "id": "haven", "kind": "town", "at": [150, 45], "port": true }
                ],
                "regions": [
                    { "id": "maeris", "kind": "land", "roughness": 10,
                      "polygon": [[60, 40], [240, 50], [220, 160], [80, 150]] }
                ]
            }
        }))
        .unwrap();
        let w = crate::world::World::build(&c).unwrap();
        for (i, p) in c.pins.points.iter().enumerate() {
            assert!(w.features.is_land(w.pin_cells[i]), "{} is in water", p.id);
        }
        let port = w.pin_cells[1];
        assert!(w.grid.neighbors(port).any(|m| !w.features.is_land(m)));
    }

    fn regions(v: serde_json::Value) -> Vec<RegionPin> {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 1, "frame": { "width": 100, "height": 100 },
            "pins": { "regions": v }
        }))
        .unwrap();
        c.pins.regions
    }

    #[test]
    fn water_range_ends_below_sea_level_and_land_range_starts_at_it() {
        let r = regions(json!([
            { "id": "s", "kind": "sea", "box": [0, 0, 1, 1] },
            { "id": "m", "kind": "marsh", "box": [0, 0, 1, 1] }
        ]));
        assert_eq!(effective_range(&r[0], 10.0), [5.0, 9.0]);
        assert_eq!(effective_range(&r[1], 30.0), [30.0, 30.0]);
    }

    #[test]
    fn a_land_region_with_a_higher_priority_rules_over_the_sea() {
        let r = regions(json!([
            { "id": "bay", "kind": "sea", "box": [0, 0, 60, 60] },
            { "id": "isle", "kind": "hills", "circle": { "at": [30, 30], "r": 5 }, "priority": 1 },
            { "id": "rock", "kind": "hills", "circle": { "at": [10, 10], "r": 3 } }
        ]));
        let fit = FitRegion::all(&r, 20.0, [100.0, 100.0], &SeedTree::new(1));
        // The island rules its cells.
        assert!(forced_land(&fit, [30.0, 30.0]));
        assert!(!forced_water(&fit, [30.0, 30.0]));
        // Outside the island, the sea rules.
        assert!(forced_water(&fit, [50.0, 50.0]));
        assert!(!forced_land(&fit, [50.0, 50.0]));
        // Regions with the same priority share the cell: water is forced.
        assert!(forced_water(&fit, [10.0, 10.0]));
        assert!(!forced_land(&fit, [10.0, 10.0]));
        // Outside all regions, nothing is forced.
        assert!(!forced_water(&fit, [80.0, 80.0]) && !forced_land(&fit, [80.0, 80.0]));
    }

    #[test]
    fn put_gives_a_whole_number_in_the_range() {
        assert_eq!(put(35.7, [35.5, 50.0]), 36.0);
        assert_eq!(put(99.0, [35.0, 50.0]), 50.0);
        assert_eq!(put(12.9, [5.0, 15.0]), 12.0);
    }
}
