//! The data of the layers that the world may not have: zones, ice,
//! population, emblems, and markers. Zones and emblems are not in the
//! world yet.
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

/// The ice of a world: its glaciers and icebergs.
pub fn ice(world: &World) -> Option<Vec<Ice>> {
    if world.ice.is_empty() {
        return None;
    }
    Some(
        world
            .ice
            .iter()
            .map(|i| Ice {
                id: i.id,
                glacier: i.kind == inkland_core::ice::IceKind::Glacier,
                points: i.points.clone(),
            })
            .collect(),
    )
}

/// The population of a world: the rural population of each cell and the
/// population of each place (`rankCells` and `burgsSpecify`).
pub fn population(world: &World) -> Option<Population> {
    let soc = &world.society;
    if soc.places.is_empty() && soc.cells.pop.iter().all(|&p| p == 0.0) {
        return None;
    }
    Some(Population {
        rural: soc.cells.pop.clone(),
        urban: soc.places.iter().map(|p| (p.at, p.population)).collect(),
    })
}

/// The coats of arms of a world.
pub fn emblems(_world: &World) -> Option<Vec<Emblem>> {
    None
}

/// The markers of a world, with the icons of FMG.
pub fn markers(world: &World) -> Option<Vec<Marker>> {
    if world.markers.is_empty() {
        return None;
    }
    Some(
        world
            .markers
            .iter()
            .map(|m| Marker {
                id: m.id,
                at: m.at,
                icon: marker_icon(m.kind).to_owned(),
                size: 30.0,
                fill: "#ffffff".to_owned(),
                stroke: "#000000".to_owned(),
            })
            .collect(),
    )
}

/// The icon text of each FMG marker type, as code points.
fn marker_icon(kind: &str) -> &'static str {
    match kind {
        "volcanoes" => "\u{1F30B}",
        "hot-springs" => "\u{2668}\u{FE0F}",
        "water-sources" => "\u{1F4A7}",
        "guild-halls" => "\u{1F3DB}\u{FE0F}",
        "mines" => "\u{26CF}\u{FE0F}",
        "bridges" => "\u{1F309}",
        "merchant-houses" => "\u{2696}\u{FE0F}",
        "inns" => "\u{1F37B}",
        "lighthouses" => "\u{1F6A8}",
        "waterfalls" => "\u{1F4A6}",
        "battlefields" => "\u{2694}\u{FE0F}",
        "dungeons" => "\u{1F5DD}\u{FE0F}",
        "lake-monsters" => "\u{1F409}",
        "sea-monsters" => "\u{1F991}",
        "hill-monsters" => "\u{1F479}",
        "sacred-mountains" => "\u{1F5FB}",
        "sacred-forests" => "\u{1F333}",
        "sacred-pineries" => "\u{1F332}",
        "sacred-palm-groves" => "\u{1F334}",
        "brigands" => "\u{1F4B0}",
        "pirates" => "\u{1F3F4}\u{200D}\u{2620}\u{FE0F}",
        "statues" => "\u{1F5FF}",
        "ruins" => "\u{1F3FA}",
        "libraries" => "\u{1F4DA}",
        "circuses" => "\u{1F3AA}",
        "jousts" => "\u{1F93A}",
        "fairs" => "\u{1F3A0}",
        "customs-houses" => "\u{1F6C3}",
        "canoes" => "\u{1F6F6}",
        "migration" => "\u{1F417}",
        "dances" => "\u{1F483}\u{1F3FD}",
        "mirage" => "\u{AA5C}",
        "caves" => "\u{1F987}",
        "portals" => "\u{1F300}",
        "rifts" => "\u{1F386}",
        "disturbed-burials" => "\u{1F480}",
        "necropolises" => "\u{1FAA6}",
        "encounters" => "\u{1F9D9}",
        "party" => "\u{1F6A9}",
        _ => "\u{2753}",
    }
}
