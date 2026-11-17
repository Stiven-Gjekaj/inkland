//! Fit the terrain around the pins.
//!
//! The template makes the free terrain first. Then this module forces the
//! heights inside pinned regions and around pinned places. A pin never
//! moves. If the terrain cannot satisfy a pin, the result is an error with a
//! specific code.
//!
//! Regions: each cell whose site is inside a region gets a height in the
//! range of the region. Where regions overlap, the cell gets a height in all
//! of their ranges. The template height is scaled into the range, so the
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

/// A whole-number height in the range.
fn put(v: f64, [lo, hi]: [f64; 2]) -> f64 {
    v.trunc().clamp(lo.ceil(), hi.floor().max(lo.ceil()))
}

/// Force the heights in and near the regions.
pub fn apply_regions(grid: &Grid, heights: &mut [f64], regions: &[FitRegion], sea_level: f64) {
    for c in 0..grid.len() {
        let p = grid.points[c];
        let inside: Vec<&FitRegion> = regions.iter().filter(|r| r.contains(p)).collect();
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
    let in_land_region = |c: usize| {
        regions
            .iter()
            .any(|r| !r.pin.kind.is_water() && r.contains(grid.points[c]))
    };

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
    fn put_gives_a_whole_number_in_the_range() {
        assert_eq!(put(35.7, [35.5, 50.0]), 36.0);
        assert_eq!(put(99.0, [35.0, 50.0]), 50.0);
        assert_eq!(put(12.9, [5.0, 15.0]), 12.0);
    }
}
