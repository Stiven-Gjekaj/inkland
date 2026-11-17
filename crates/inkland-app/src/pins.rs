//! The edit actions of the pin editors: burgs, regions, rivers, routes, and
//! the heightmap brushes. Each action is one `inkland::Command`.

use crate::model::{Editor, round2};
use crate::options::number;
use inkland::{Command, Config, PinList};
use inkland_core::config::{Anchor, PlaceRef, Waypoint};
use serde_json::{Value, json};

/// The shape of a new region pin.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// `[x0, y0, x1, y1]`.
    Box([f64; 4]),
    /// The centre and the radius.
    Circle([f64; 2], f64),
    /// The outline.
    Polygon(Vec<[f64; 2]>),
}

/// The kind of shape that a drag on the map makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Box,
    Circle,
    Polygon,
}

impl Shape {
    /// The shape of a drag from `a` to `b`: a box with the two corners, or
    /// a circle with the centre `a` that goes through `b`.
    pub fn from_drag(kind: ShapeKind, a: [f64; 2], b: [f64; 2]) -> Option<Shape> {
        match kind {
            ShapeKind::Box => {
                let r = [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[0].max(b[0]),
                    a[1].max(b[1]),
                ];
                (r[2] > r[0] && r[3] > r[1]).then_some(Shape::Box(r))
            }
            ShapeKind::Circle => {
                let r = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
                (r > 0.0).then_some(Shape::Circle(a, r))
            }
            ShapeKind::Polygon => None,
        }
    }

    fn json(&self) -> (&'static str, Value) {
        let p = |q: [f64; 2]| json!([number(round2(q[0])), number(round2(q[1]))]);
        match self {
            Shape::Box(r) => ("box", json!(r.map(|v| number(round2(v))))),
            Shape::Circle(at, r) => ("circle", json!({ "at": p(*at), "r": number(round2(*r)) })),
            Shape::Polygon(pts) => (
                "polygon",
                json!(pts.iter().map(|q| p(*q)).collect::<Vec<_>>()),
            ),
        }
    }
}

/// The point pin within `tolerance` of `p`, if there is one: the nearest.
pub fn place_near(c: &Config, p: [f64; 2], tolerance: f64) -> Option<String> {
    c.pins
        .points
        .iter()
        .map(|pin| {
            let d = ((pin.at[0] - p[0]).powi(2) + (pin.at[1] - p[1]).powi(2)).sqrt();
            (d, &pin.id)
        })
        .filter(|(d, _)| *d <= tolerance)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id.clone())
}

/// A river anchor at `p`: the place near it, or the point.
pub fn anchor_at(c: &Config, p: [f64; 2], tolerance: f64) -> Anchor {
    match place_near(c, p, tolerance) {
        Some(place) => Anchor::Place(PlaceRef { place }),
        None => Anchor::Point([round2(p[0]), round2(p[1])]),
    }
}

/// A road waypoint at `p`: the place near it, or the point.
pub fn waypoint_at(c: &Config, p: [f64; 2], tolerance: f64) -> Waypoint {
    match place_near(c, p, tolerance) {
        Some(place) => Waypoint::Place(place),
        None => Waypoint::Point([round2(p[0]), round2(p[1])]),
    }
}

/// The position of an anchor, if it is a point or a place.
pub fn anchor_point(c: &Config, a: &Anchor) -> Option<[f64; 2]> {
    match a {
        Anchor::Point(p) => Some(*p),
        Anchor::Place(r) => c.pins.points.iter().find(|p| p.id == r.place).map(|p| p.at),
        Anchor::Region(r) => c
            .pins
            .regions
            .iter()
            .find(|g| g.id == r.region)
            .and_then(|g| {
                g.rect
                    .map(|b| [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0])
                    .or(g.circle.as_ref().map(|c| c.at))
                    .or(g.polygon.as_ref().and_then(|p| p.first().copied()))
            }),
    }
}

/// The position of a road waypoint.
pub fn waypoint_point(c: &Config, w: &Waypoint) -> Option<[f64; 2]> {
    match w {
        Waypoint::Point(p) => Some(*p),
        Waypoint::Place(id) => c.pins.points.iter().find(|p| &p.id == id).map(|p| p.at),
    }
}

/// The text of an anchor: the place id, the region id, or the point.
pub fn anchor_text(a: &Anchor) -> String {
    match a {
        Anchor::Point(p) => format!("({}, {})", p[0], p[1]),
        Anchor::Place(r) => r.place.clone(),
        Anchor::Region(r) => format!("region {}", r.region),
    }
}

/// The text of a road waypoint.
pub fn waypoint_text(w: &Waypoint) -> String {
    match w {
        Waypoint::Point(p) => format!("({}, {})", p[0], p[1]),
        Waypoint::Place(id) => id.clone(),
    }
}

fn index_of(c: &Config, list: PinList, id: &str) -> Result<usize, String> {
    let ids: Vec<&str> = match list {
        PinList::Points => c.pins.points.iter().map(|p| p.id.as_str()).collect(),
        PinList::Regions => c.pins.regions.iter().map(|p| p.id.as_str()).collect(),
        PinList::Rivers => c.pins.rivers.iter().map(|p| p.id.as_str()).collect(),
        PinList::Roads => c.pins.roads.iter().map(|p| p.id.as_str()).collect(),
    };
    ids.iter()
        .position(|i| *i == id)
        .ok_or_else(|| format!("there is no pin {id:?}"))
}

fn list_key(list: PinList) -> &'static str {
    match list {
        PinList::Points => "points",
        PinList::Regions => "regions",
        PinList::Rivers => "rivers",
        PinList::Roads => "roads",
    }
}

impl Editor {
    /// Set a field of a pin: `/pins/<list>/<index>/<field>`.
    pub fn set_pin_field(
        &mut self,
        list: PinList,
        id: &str,
        field: &str,
        value: Option<Value>,
    ) -> Result<(), String> {
        let i = index_of(self.config(), list, id)?;
        let path = format!("/pins/{}/{i}/{field}", list_key(list));
        match value {
            Some(v) => self.set(&path, v),
            None => self.unset(&path),
        }
    }

    /// Give a pin of any list a name. An empty name removes the name.
    pub fn rename_in(&mut self, list: PinList, id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        let old = catalog_name(self.config(), list, id)?;
        let new = (!name.is_empty()).then(|| name.to_owned());
        if old == new {
            return Ok(());
        }
        self.run(Command::Rename {
            list,
            id: id.to_owned(),
            name: new,
        })
    }

    /// Remove a pin of any list.
    pub fn remove_in(&mut self, list: PinList, id: &str) -> Result<(), String> {
        if list == PinList::Points {
            return self.remove_pin(id);
        }
        self.run(Command::RemovePin {
            list,
            id: id.to_owned(),
        })
    }

    /// Add a point pin with a name: the "pin it" action of a generated
    /// place. The result is the new id.
    pub fn pin_place(&mut self, at: [f64; 2], kind: &str, name: &str) -> Result<String, String> {
        let id = self.free_id("pin");
        self.run(Command::AddPin {
            list: PinList::Points,
            pin: json!({
                "id": id, "kind": kind, "name": name,
                "at": [number(round2(at[0])), number(round2(at[1]))]
            }),
            index: None,
        })?;
        self.selected = Some(id.clone());
        Ok(id)
    }

    /// Remove the heightmap brush stroke at `index`.
    pub fn remove_brush(&mut self, index: usize) -> Result<(), String> {
        self.run(Command::RemoveBrush { index })
    }

    /// Add a region pin. The result is the new id.
    pub fn add_region(&mut self, kind: &str, shape: &Shape) -> Result<String, String> {
        let id = self.free_id(kind);
        let (key, value) = shape.json();
        let mut pin = json!({ "id": id, "kind": kind });
        pin[key] = value;
        self.run(Command::AddPin {
            list: PinList::Regions,
            pin,
            index: None,
        })?;
        Ok(id)
    }

    /// Change the kind of a region. The height range goes back to the
    /// range of the new kind. It is one edit of the region.
    pub fn set_region_kind(&mut self, id: &str, kind: &str) -> Result<(), String> {
        let i = index_of(self.config(), PinList::Regions, id)?;
        let mut pin =
            serde_json::to_value(&self.config().pins.regions[i]).map_err(|e| e.to_string())?;
        if pin["kind"] == kind {
            return Ok(());
        }
        pin["kind"] = json!(kind);
        if let Some(o) = pin.as_object_mut() {
            o.remove("height");
            if !matches!(kind, "sea" | "lake") {
                o.remove("landlocked");
            }
        }
        self.set(&format!("/pins/regions/{i}"), pin)
    }

    /// Set the height range of a region. `None`: the range of the kind.
    pub fn set_region_height(&mut self, id: &str, range: Option<[f64; 2]>) -> Result<(), String> {
        self.set_pin_field(
            PinList::Regions,
            id,
            "height",
            range.map(|r| json!([number(r[0]), number(r[1])])),
        )
    }

    /// Set the width of the soft edge of a region.
    pub fn set_region_edge(&mut self, id: &str, edge: f64) -> Result<(), String> {
        self.set_pin_field(PinList::Regions, id, "edge", Some(number(edge)))
    }

    /// Add a river pin: the first anchor is the source, the last is the
    /// mouth, and the others are the course. The result is the new id.
    pub fn add_river(&mut self, anchors: &[Anchor]) -> Result<String, String> {
        let [source, through @ .., mouth] = anchors else {
            return Err("a river needs a source and a mouth".to_owned());
        };
        let id = self.free_id("river");
        self.run(Command::AddPin {
            list: PinList::Rivers,
            pin: json!({ "id": id, "source": source, "through": through, "mouth": mouth }),
            index: None,
        })?;
        Ok(id)
    }

    /// Add a road pin from the place `from` to the place `to`. The result
    /// is the new id.
    pub fn add_road(
        &mut self,
        from: &str,
        to: &str,
        via: &[Waypoint],
        kind: &str,
    ) -> Result<String, String> {
        let id = self.free_id(if kind == "trail" { "trail" } else { "road" });
        let mut pin = json!({ "id": id, "from": from, "to": to, "kind": kind });
        if !via.is_empty() {
            pin["via"] = json!(via);
        }
        self.run(Command::AddPin {
            list: PinList::Roads,
            pin,
            index: None,
        })?;
        Ok(id)
    }

    /// Add a road along the waypoints of a drawing: the first and the last
    /// must be places.
    pub fn add_road_along(&mut self, points: &[Waypoint], kind: &str) -> Result<String, String> {
        let [Waypoint::Place(from), via @ .., Waypoint::Place(to)] = points else {
            return Err("a road starts and ends at a pinned place".to_owned());
        };
        self.add_road(from, to, via, kind)
    }

    /// Set the wanted length of a road. `None`: no wanted length.
    pub fn set_road_target(&mut self, id: &str, length: Option<f64>) -> Result<(), String> {
        self.set_pin_field(PinList::Roads, id, "target_length", length.map(number))
    }

    /// Set the permitted difference from the wanted length (0 to 1).
    pub fn set_road_tolerance(&mut self, id: &str, tolerance: f64) -> Result<(), String> {
        self.set_pin_field(PinList::Roads, id, "tolerance", Some(number(tolerance)))
    }
}

fn catalog_name(c: &Config, list: PinList, id: &str) -> Result<Option<String>, String> {
    let i = index_of(c, list, id)?;
    Ok(match list {
        PinList::Points => c.pins.points[i].name.clone(),
        PinList::Regions => c.pins.regions[i].name.clone(),
        PinList::Rivers => c.pins.rivers[i].name.clone(),
        PinList::Roads => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> Editor {
        Editor::new(
            serde_json::from_value(json!({
                "version": 1, "seed": 11,
                "frame": { "width": 200, "height": 150 },
                "world": { "cells": 1500, "template": "pangea", "states": 2, "towns": 3 },
                "pins": {
                    "points": [
                        { "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [50, 70] },
                        { "id": "harrow", "kind": "town", "at": [140, 80] }
                    ]
                }
            }))
            .unwrap(),
        )
    }

    #[test]
    fn a_drag_gives_a_box_or_a_circle() {
        assert_eq!(
            Shape::from_drag(ShapeKind::Box, [10.0, 40.0], [5.0, 20.0]),
            Some(Shape::Box([5.0, 20.0, 10.0, 40.0]))
        );
        assert_eq!(
            Shape::from_drag(ShapeKind::Circle, [10.0, 10.0], [13.0, 14.0]),
            Some(Shape::Circle([10.0, 10.0], 5.0))
        );
        assert_eq!(
            Shape::from_drag(ShapeKind::Box, [1.0, 1.0], [1.0, 9.0]),
            None
        );
    }

    #[test]
    fn the_region_editor_adds_edits_and_removes_regions() {
        let mut e = editor();
        let a = e
            .add_region("hills", &Shape::Box([80.0, 90.0, 120.0, 130.0]))
            .unwrap();
        assert_eq!(a, "hills-1");
        let b = e
            .add_region("lake", &Shape::Circle([100.0, 40.0], 12.345))
            .unwrap();
        let c = e
            .add_region(
                "forest",
                &Shape::Polygon(vec![[10.0, 10.0], [40.0, 12.0], [25.0, 40.0]]),
            )
            .unwrap();
        let r = &e.config().pins.regions;
        assert_eq!(r.len(), 3);
        assert_eq!(r[1].circle.as_ref().unwrap().r, 12.35);
        assert_eq!(r[2].polygon.as_ref().unwrap().len(), 3);
        e.set_region_height(&a, Some([40.0, 55.0])).unwrap();
        assert_eq!(e.config().pins.regions[0].height, Some([40.0, 55.0]));
        // A new kind drops the height range of the old kind.
        e.set_region_kind(&a, "mountains").unwrap();
        assert_eq!(e.config().pins.regions[0].height, None);
        e.set_region_edge(&a, 4.0).unwrap();
        e.rename_in(PinList::Regions, &b, "Mirror Mere").unwrap();
        assert_eq!(
            e.config().pins.regions[1].name.as_deref(),
            Some("Mirror Mere")
        );
        e.remove_in(PinList::Regions, &c).unwrap();
        assert_eq!(e.config().pins.regions.len(), 2);
        // A land range is refused for a lake.
        assert!(e.set_region_height(&b, Some([50.0, 60.0])).is_err());
        while e.undo().unwrap() {}
        assert_eq!(e.config(), e.original());
    }

    #[test]
    fn the_river_editor_snaps_to_places_and_needs_two_anchors() {
        let mut e = editor();
        let source = anchor_at(e.config(), [51.0, 71.0], 3.0);
        assert_eq!(
            source,
            Anchor::Place(PlaceRef {
                place: "ostrey".into()
            })
        );
        let mouth = anchor_at(e.config(), [199.0, 149.0], 3.0);
        assert_eq!(mouth, Anchor::Point([199.0, 149.0]));
        assert!(e.add_river(std::slice::from_ref(&source)).is_err());
        let id = e
            .add_river(&[source, Anchor::Point([100.0, 100.0]), mouth])
            .unwrap();
        assert_eq!(id, "river-1");
        let r = &e.config().pins.rivers[0];
        assert_eq!(r.through.len(), 1);
        e.rename_in(PinList::Rivers, &id, "Brann").unwrap();
        // The river needs its source place.
        assert!(e.remove_pin("ostrey").is_err());
        e.remove_in(PinList::Rivers, &id).unwrap();
        assert!(e.config().pins.rivers.is_empty());
    }

    #[test]
    fn the_route_editor_adds_a_road_between_places() {
        let mut e = editor();
        let pts = [
            waypoint_at(e.config(), [50.0, 70.0], 3.0),
            waypoint_at(e.config(), [90.0, 60.0], 3.0),
            waypoint_at(e.config(), [140.0, 81.0], 3.0),
        ];
        assert_eq!(pts[1], Waypoint::Point([90.0, 60.0]));
        let id = e.add_road_along(&pts, "road").unwrap();
        assert_eq!(id, "road-1");
        let road = &e.config().pins.roads[0];
        assert_eq!((road.from.as_str(), road.to.as_str()), ("ostrey", "harrow"));
        assert_eq!(road.via.len(), 1);
        e.set_road_target(&id, Some(120.0)).unwrap();
        e.set_road_tolerance(&id, 0.1).unwrap();
        assert_eq!(e.config().pins.roads[0].target_length, Some(120.0));
        e.set_road_target(&id, None).unwrap();
        assert_eq!(e.config().pins.roads[0].target_length, None);
        // A road must end at a place.
        assert!(e.add_road_along(&pts[..2], "trail").is_err());
        assert!(e.set_road_tolerance(&id, 2.0).is_err(), "more than 1");
        while e.undo().unwrap() {}
        assert_eq!(e.config(), e.original());
    }

    #[test]
    fn the_burgs_editor_pins_a_place_and_edits_its_fields() {
        let mut e = editor();
        let id = e.pin_place([33.333, 44.0], "town", "Millford").unwrap();
        let p = e.config().pins.points.last().unwrap();
        assert_eq!(
            (p.id.as_str(), p.name.as_deref(), p.at),
            (id.as_str(), Some("Millford"), [33.33, 44.0])
        );
        e.set_pin_field(PinList::Points, &id, "kind", Some(json!("village")))
            .unwrap();
        e.set_pin_field(PinList::Points, &id, "port", Some(json!(true)))
            .unwrap();
        assert!(e.config().pins.points.last().unwrap().port);
        e.rename_in(PinList::Points, &id, "Millford").unwrap();
        assert_eq!(e.undo_len(), 3, "the same name makes no edit");
        e.remove_in(PinList::Points, &id).unwrap();
        assert_eq!(e.selected, None);
    }

    #[test]
    fn the_heightmap_editor_removes_a_brush() {
        let mut e = editor();
        e.brush([50.0, 50.0], 10.0, 5.0).unwrap();
        e.brush([60.0, 50.0], 10.0, -5.0).unwrap();
        e.remove_brush(0).unwrap();
        assert_eq!(e.config().world.brushes.len(), 1);
        assert_eq!(e.config().world.brushes[0].delta, -5.0);
        assert!(e.remove_brush(5).is_err());
        e.undo().unwrap();
        assert_eq!(e.config().world.brushes.len(), 2);
        assert_eq!(e.config().world.brushes[0].delta, 5.0);
    }
}
