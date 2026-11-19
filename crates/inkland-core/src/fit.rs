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

use crate::config::{PointPin, RegionKind, RegionPin};
use crate::error::{Error, ErrorCode};
use crate::geom::Shape;
use crate::grid::Grid;
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
}

impl<'a> FitRegion<'a> {
    pub fn all(regions: &'a [RegionPin], sea_level: f64) -> Vec<FitRegion<'a>> {
        regions
            .iter()
            .enumerate()
            .filter_map(|(index, pin)| {
                let shape = Shape::of(pin)?;
                let bbox = shape.bbox();
                Some(FitRegion {
                    index,
                    pin,
                    range: effective_range(pin, sea_level),
                    land_only: pin.height.is_none()
                        && matches!(pin.kind, RegionKind::Forest | RegionKind::Desert),
                    shape,
                    bbox,
                })
            })
            .collect()
    }

    pub fn contains(&self, p: [f64; 2]) -> bool {
        let [x0, y0, x1, y1] = self.bbox;
        p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1 && self.shape.contains(p)
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
        let d = self.shape.distance(p);
        (d > 0.0 && d < e).then_some(d)
    }

    /// The forced height for a template height `h`.
    fn target(&self, h: f64, sea_level: f64) -> f64 {
        if self.land_only {
            return put(h.max(sea_level), self.range);
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
            heights[c] = if land_only {
                put(heights[c].max(sea_level), range)
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
/// a cell of the first grid.
pub fn clamp_regions(grid: &Grid, heights: &mut [f64], regions: &[FitRegion], sea_level: f64) {
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
            heights[c].max(sea_level)
        } else {
            heights[c]
        };
        heights[c] = put(h, range);
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
        let fit = FitRegion::all(&r, 20.0);
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
