//! Optional data of the layers that the world does not have yet: zones,
//! ice, population, emblems, and markers.
//!
//! The drawing of each of these layers reads its data from here. Each
//! function gives `None` while the world has no such data, and the layer
//! then draws nothing. When the world gets the data, only the function in
//! this file changes.

use inkland_core::world::World;

/// A zone: an area of cells with a colour.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub id: u32,
    pub kind: String,
    pub color: String,
    pub cells: Vec<usize>,
}

/// A glacier or an iceberg: a polygon in world units.
#[derive(Debug, Clone, PartialEq)]
pub struct Ice {
    pub id: u32,
    pub glacier: bool,
    pub points: Vec<[f64; 2]>,
}

/// The population: the rural population of each cell and the urban
/// population of each place, in thousands.
#[derive(Debug, Clone, PartialEq)]
pub struct Population {
    pub rural: Vec<f64>,
    pub urban: Vec<([f64; 2], f64)>,
}

/// A coat of arms: SVG markup in a box of 200 x 200, at a point.
#[derive(Debug, Clone, PartialEq)]
pub struct Emblem {
    pub id: String,
    /// `state`, `province`, or `burg`.
    pub group: &'static str,
    pub at: [f64; 2],
    pub svg: String,
}

/// A marker: a pin with an icon (a short text, for example an emoji).
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub id: u32,
    pub at: [f64; 2],
    pub icon: String,
    /// The size, in percent of the marker font size. The original program
    /// uses 30.
    pub size: f64,
    pub fill: String,
    pub stroke: String,
}

/// The zones of a world.
pub fn zones(_world: &World) -> Option<Vec<Zone>> {
    None
}

/// The ice of a world.
pub fn ice(_world: &World) -> Option<Vec<Ice>> {
    None
}

/// The population of a world.
pub fn population(_world: &World) -> Option<Population> {
    None
}

/// The coats of arms of a world.
pub fn emblems(_world: &World) -> Option<Vec<Emblem>> {
    None
}

/// The markers of a world.
pub fn markers(_world: &World) -> Option<Vec<Marker>> {
    None
}
