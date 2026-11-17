//! The marks that the editors draw over the map: the outlines of the pinned
//! regions, the courses of the pinned rivers and roads, the crop of the
//! variant, and the shape that the user draws now. Each mark is the path
//! text of a Slint `Path`, in screen pixels.

use crate::pins::{anchor_point, waypoint_point};
use inkland::Config;
use std::fmt::Write;

/// The paths of the pins of one editor: all pins, and the selected pin.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paths {
    pub normal: String,
    pub selected: String,
}

/// A polyline. `close`: back to the first point.
pub fn polyline(pts: &[[f64; 2]], close: bool) -> String {
    let mut out = String::new();
    for (i, p) in pts.iter().enumerate() {
        let _ = write!(
            out,
            "{} {:.1} {:.1} ",
            if i == 0 { "M" } else { "L" },
            p[0],
            p[1]
        );
    }
    if close && pts.len() > 2 {
        out.push('Z');
    }
    out.trim_end().to_owned()
}

/// A circle with two arcs.
pub fn circle(c: [f64; 2], r: f64) -> String {
    format!(
        "M {:.1} {:.1} A {r:.1} {r:.1} 0 1 0 {:.1} {:.1} A {r:.1} {r:.1} 0 1 0 {:.1} {:.1} Z",
        c[0] - r,
        c[1],
        c[0] + r,
        c[1],
        c[0] - r,
        c[1]
    )
}

/// A rectangle `[x0, y0, x1, y1]` of the world.
pub fn rect(r: [f64; 4], to_screen: &impl Fn([f64; 2]) -> [f64; 2]) -> String {
    let pts = [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]].map(to_screen);
    polyline(&pts, true)
}

fn join(paths: &mut String, p: &str) {
    if p.is_empty() {
        return;
    }
    if !paths.is_empty() {
        paths.push(' ');
    }
    paths.push_str(p);
}

fn add(out: &mut Paths, selected: bool, p: &str) {
    join(
        if selected {
            &mut out.selected
        } else {
            &mut out.normal
        },
        p,
    );
}

/// The outlines of the region pins.
pub fn region_paths(c: &Config, selected: &str, to_screen: impl Fn([f64; 2]) -> [f64; 2]) -> Paths {
    let mut out = Paths::default();
    for r in &c.pins.regions {
        let p = if let Some(b) = r.rect {
            rect(b, &to_screen)
        } else if let Some(k) = &r.circle {
            let at = to_screen(k.at);
            let edge = to_screen([k.at[0] + k.r, k.at[1]]);
            circle(at, (edge[0] - at[0]).abs())
        } else if let Some(poly) = &r.polygon {
            let pts: Vec<[f64; 2]> = poly.iter().map(|q| to_screen(*q)).collect();
            polyline(&pts, true)
        } else {
            String::new()
        };
        add(&mut out, r.id == selected, &p);
    }
    out
}

/// The courses of the river pins: source, course points, and mouth.
pub fn river_paths(c: &Config, selected: &str, to_screen: impl Fn([f64; 2]) -> [f64; 2]) -> Paths {
    let mut out = Paths::default();
    for r in &c.pins.rivers {
        let pts: Vec<[f64; 2]> = std::iter::once(&r.source)
            .chain(&r.through)
            .chain(std::iter::once(&r.mouth))
            .filter_map(|a| anchor_point(c, a))
            .map(&to_screen)
            .collect();
        add(&mut out, r.id == selected, &polyline(&pts, false));
    }
    out
}

/// The courses of the road pins: from, the waypoints, and to.
pub fn road_paths(c: &Config, selected: &str, to_screen: impl Fn([f64; 2]) -> [f64; 2]) -> Paths {
    let mut out = Paths::default();
    for r in &c.pins.roads {
        let place = |id: &str| c.pins.points.iter().find(|p| p.id == id).map(|p| p.at);
        let pts: Vec<[f64; 2]> = place(&r.from)
            .into_iter()
            .chain(r.via.iter().filter_map(|w| waypoint_point(c, w)))
            .chain(place(&r.to))
            .map(&to_screen)
            .collect();
        add(&mut out, r.id == selected, &polyline(&pts, false));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_follow_the_pins_and_mark_the_selection() {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 1, "frame": { "width": 200, "height": 150 },
            "pins": {
                "points": [
                    { "id": "a", "kind": "town", "at": [10, 10] },
                    { "id": "b", "kind": "town", "at": [50, 10] }
                ],
                "regions": [
                    { "id": "box", "kind": "hills", "box": [0, 0, 10, 20] },
                    { "id": "round", "kind": "lake", "circle": { "at": [100, 100], "r": 5 } }
                ],
                "rivers": [{ "id": "r", "source": { "place": "a" }, "through": [[30, 30]], "mouth": [60, 60] }],
                "roads": [{ "id": "road", "from": "a", "to": "b", "via": [[30, 0]] }]
            }
        }))
        .unwrap();
        let double = |p: [f64; 2]| [p[0] * 2.0, p[1] * 2.0];
        let regions = region_paths(&c, "round", double);
        assert_eq!(
            regions.normal,
            "M 0.0 0.0 L 20.0 0.0 L 20.0 40.0 L 0.0 40.0 Z"
        );
        assert!(regions.selected.starts_with("M 190.0 200.0 A 10.0 10.0"));
        let rivers = river_paths(&c, "", double);
        assert_eq!(rivers.normal, "M 20.0 20.0 L 60.0 60.0 L 120.0 120.0");
        assert!(rivers.selected.is_empty());
        let roads = road_paths(&c, "road", double);
        assert_eq!(roads.selected, "M 20.0 20.0 L 60.0 0.0 L 100.0 20.0");
        assert_eq!(polyline(&[[1.0, 2.0]], true), "M 1.0 2.0");
    }
}
