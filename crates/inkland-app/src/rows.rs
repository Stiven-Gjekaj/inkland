//! The rows of the editor tables, and the text of the cell information.
//! The rows come from the config and from the world in memory. They do not
//! change anything.

use crate::options::number_text;
use crate::pins::{anchor_text, waypoint_text};
use inkland::{Config, World};
use serde_json::Value;

/// A row of a table: an id, the cell texts, and `true` for a pin.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: String,
    pub cells: Vec<String>,
    pub pinned: bool,
}

fn row(id: impl Into<String>, cells: Vec<String>, pinned: bool) -> Row {
    Row {
        id: id.into(),
        cells,
        pinned,
    }
}

/// The JSON of a kind enum.
pub struct KindValue(Option<Value>);

macro_rules! kind_value {
    ($($t:ty),*) => {$(
        impl From<&$t> for KindValue {
            fn from(v: &$t) -> KindValue {
                KindValue(serde_json::to_value(v).ok())
            }
        }
    )*};
}

kind_value!(
    inkland_core::config::PlaceKind,
    inkland_core::config::RegionKind,
    inkland_core::config::RoadKind
);

/// `"village"` and so on for a value that serializes to a string.
pub fn kind_text(v: impl Into<KindValue>) -> String {
    v.into()
        .0
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn pt(p: [f64; 2]) -> String {
    format!("{}, {}", number_text(p[0]), number_text(p[1]))
}

/// The id of a generated place in the burg table: `place:<n>`.
pub fn place_row_id(n: u32) -> String {
    format!("place:{n}")
}

/// The generated place of a row id `place:<n>`.
pub fn generated_place<'a>(world: &'a World, id: &str) -> Option<&'a inkland_core::society::Place> {
    let n: u32 = id.strip_prefix("place:")?.parse().ok()?;
    world.society.places.iter().find(|p| p.id == n)
}

fn state_name(world: &World, id: u32) -> String {
    world
        .society
        .states
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.name.clone())
        .unwrap_or_default()
}

fn culture_name(world: &World, id: u32) -> String {
    world
        .society
        .cultures
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.name.clone())
        .unwrap_or_default()
}

fn place_name(world: &World, id: u32) -> String {
    world
        .society
        .places
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.name.clone())
        .unwrap_or_default()
}

/// The burgs: the point pins first (name, kind, position), then the places
/// that generation adds (name, kind, state, position). The world gives the
/// names of the pins without a name.
pub fn burg_rows(c: &Config, world: Option<&World>) -> Vec<Row> {
    let mut rows: Vec<Row> = c
        .pins
        .points
        .iter()
        .map(|p| {
            let generated = world.and_then(|w| {
                w.society
                    .places
                    .iter()
                    .find(|g| g.pin.as_deref() == Some(p.id.as_str()))
            });
            let name = p
                .name
                .clone()
                .or_else(|| generated.map(|g| format!("{} (generated)", g.name)))
                .unwrap_or_else(|| p.id.clone());
            let state = match (world, generated) {
                (Some(w), Some(g)) => state_name(w, g.state),
                _ => String::new(),
            };
            row(&p.id, vec![name, kind_text(&p.kind), state, pt(p.at)], true)
        })
        .collect();
    if let Some(w) = world {
        rows.extend(
            w.society
                .places
                .iter()
                .filter(|g| g.pin.is_none())
                .map(|g| {
                    let mut kind = kind_text(&g.kind);
                    if g.capital {
                        kind.push_str(", capital");
                    }
                    row(
                        place_row_id(g.id),
                        vec![
                            g.name.clone(),
                            kind,
                            state_name(w, g.state),
                            pt([
                                (g.at[0] * 100.0).round() / 100.0,
                                (g.at[1] * 100.0).round() / 100.0,
                            ]),
                        ],
                        false,
                    )
                }),
        );
    }
    rows
}

/// The region pins: name, kind, shape, height range.
pub fn region_rows(c: &Config) -> Vec<Row> {
    c.pins
        .regions
        .iter()
        .map(|r| {
            let shape = if let Some(b) = r.rect {
                format!("box {} to {}", pt([b[0], b[1]]), pt([b[2], b[3]]))
            } else if let Some(k) = &r.circle {
                format!("circle {} r {}", pt(k.at), number_text(k.r))
            } else {
                format!(
                    "polygon of {} points",
                    r.polygon.as_ref().map_or(0, Vec::len)
                )
            };
            let [lo, hi] = r.height_range();
            row(
                &r.id,
                vec![
                    r.name.clone().unwrap_or_else(|| r.id.clone()),
                    kind_text(&r.kind),
                    shape,
                    format!("{}-{}", number_text(lo), number_text(hi)),
                ],
                true,
            )
        })
        .collect()
}

/// The river pins: name, source, the number of course points, and mouth.
/// The world gives the length.
pub fn river_rows(c: &Config, world: Option<&World>) -> Vec<Row> {
    c.pins
        .rivers
        .iter()
        .enumerate()
        .map(|(k, r)| {
            let length = world
                .and_then(|w| w.rivers.iter().find(|x| x.course == Some(k)))
                .map(|x| number_text((x.length * 10.0).round() / 10.0))
                .unwrap_or_default();
            row(
                &r.id,
                vec![
                    r.name.clone().unwrap_or_else(|| r.id.clone()),
                    anchor_text(&r.source),
                    r.through.len().to_string(),
                    anchor_text(&r.mouth),
                    length,
                ],
                true,
            )
        })
        .collect()
}

/// The road pins: id, kind, from, to, the waypoints, the target length,
/// and the length in the world.
pub fn route_rows(c: &Config, world: Option<&World>) -> Vec<Row> {
    c.pins
        .roads
        .iter()
        .enumerate()
        .map(|(k, r)| {
            let length = world
                .and_then(|w| w.roads.iter().find(|x| x.pin == Some(k)))
                .map(|x| number_text((x.length * 10.0).round() / 10.0))
                .unwrap_or_default();
            let via: Vec<String> = r.via.iter().map(waypoint_text).collect();
            let target = r.target_length.map_or(String::new(), |t| {
                format!(
                    "{} +/- {}%",
                    number_text(t),
                    number_text(r.tolerance * 100.0)
                )
            });
            row(
                &r.id,
                vec![
                    r.id.clone(),
                    kind_text(&r.kind),
                    format!("{} to {}", r.from, r.to),
                    via.join(", "),
                    target,
                    length,
                ],
                true,
            )
        })
        .collect()
}

/// The heightmap brushes: number, centre, radius, change.
pub fn brush_rows(c: &Config) -> Vec<Row> {
    c.world
        .brushes
        .iter()
        .enumerate()
        .map(|(i, b)| {
            row(
                i.to_string(),
                vec![
                    (i + 1).to_string(),
                    pt(b.at),
                    number_text(b.radius),
                    format!("{:+}", b.delta),
                ],
                true,
            )
        })
        .collect()
}

fn count(cells: &[u32], id: u32) -> usize {
    cells.iter().filter(|c| **c == id).count()
}

/// The states: name, capital, culture, cells, burgs.
pub fn state_rows(world: &World) -> Vec<Row> {
    let s = &world.society;
    s.states
        .iter()
        .map(|st| {
            row(
                st.id.to_string(),
                vec![
                    st.name.clone(),
                    place_name(world, st.capital),
                    culture_name(world, st.culture),
                    count(&s.cell_state, st.id).to_string(),
                    s.places
                        .iter()
                        .filter(|p| p.state == st.id)
                        .count()
                        .to_string(),
                ],
                false,
            )
        })
        .collect()
}

/// The cultures: name, cells, burgs.
pub fn culture_rows(world: &World) -> Vec<Row> {
    let s = &world.society;
    s.cultures
        .iter()
        .map(|cu| {
            row(
                cu.id.to_string(),
                vec![
                    cu.name.clone(),
                    count(&s.cell_culture, cu.id).to_string(),
                    s.places
                        .iter()
                        .filter(|p| p.culture == cu.id)
                        .count()
                        .to_string(),
                ],
                false,
            )
        })
        .collect()
}

/// The religions: name, kind, culture, cells.
pub fn religion_rows(world: &World) -> Vec<Row> {
    let r = &world.religions;
    r.list
        .iter()
        .map(|re| {
            row(
                re.id.to_string(),
                vec![
                    re.name.clone(),
                    re.kind.to_owned(),
                    culture_name(world, re.culture),
                    count(&r.cell_religion, re.id).to_string(),
                ],
                false,
            )
        })
        .collect()
}

/// The provinces: name, state, seat, cells.
pub fn province_rows(world: &World) -> Vec<Row> {
    let s = &world.society;
    s.provinces
        .iter()
        .map(|p| {
            row(
                p.id.to_string(),
                vec![
                    p.name.clone(),
                    state_name(world, p.state),
                    place_name(world, p.seat),
                    count(&s.cell_province, p.id).to_string(),
                ],
                false,
            )
        })
        .collect()
}

fn name_of(v: &Value) -> String {
    v["name"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| "none".to_owned())
}

/// The lines of the Cell Details dialog, from the answer of
/// `inkland::query_at`: a label and a value.
pub fn cell_lines(q: &Value) -> Vec<(String, String)> {
    let num = |v: &Value| v.as_f64().map(number_text).unwrap_or_else(|| "?".into());
    let mut lines = vec![
        ("Cell".to_owned(), q["cell"]["id"].to_string()),
        (
            "X, Y".to_owned(),
            format!(
                "{}, {} {}",
                num(&q["at"][0]),
                num(&q["at"][1]),
                q["units"].as_str().unwrap_or("")
            ),
        ),
        (
            "Type".to_owned(),
            format!(
                "{} {}",
                q["feature"]["kind"].as_str().unwrap_or("?"),
                q["feature"]["name"].as_str().unwrap_or("")
            )
            .trim()
            .to_owned(),
        ),
        (
            "Elevation".to_owned(),
            format!(
                "{} ({} above the sea)",
                num(&q["height"]),
                num(&q["height_above_sea"])
            ),
        ),
        (
            "Temperature".to_owned(),
            format!("{} C", num(&q["temperature_c"])),
        ),
        ("Precipitation".to_owned(), num(&q["precipitation"])),
        (
            "Biome".to_owned(),
            q["biome"].as_str().unwrap_or("?").to_owned(),
        ),
        ("State".to_owned(), name_of(&q["state"])),
        ("Province".to_owned(), name_of(&q["province"])),
        ("Culture".to_owned(), name_of(&q["culture"])),
        ("Religion".to_owned(), name_of(&q["religion"])),
    ];
    let regions: Vec<&str> = q["regions"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    lines.push((
        "Pinned areas".to_owned(),
        if regions.is_empty() {
            "none".to_owned()
        } else {
            regions.join(", ")
        },
    ));
    for (key, label) in [
        ("nearest_place", "Burg"),
        ("nearest_river", "River"),
        ("nearest_road", "Route"),
    ] {
        let v = &q[key];
        if v.is_null() {
            continue;
        }
        let name = v["name"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| v["id"].to_string());
        lines.push((
            label.to_owned(),
            format!("{name}, {} away", num(&v["distance"])),
        ));
    }
    lines
}

/// The cell text of the status bar: height, land or water, biome, state,
/// culture, religion, province, and the nearest place.
pub fn status_text(q: &Value) -> String {
    let mut parts = vec![
        format!(
            "height {} ({})",
            q["height"],
            if q["land"] == true { "land" } else { "water" }
        ),
        q["biome"].as_str().unwrap_or("?").to_owned(),
    ];
    for (key, label) in [
        ("state", "state"),
        ("culture", "culture"),
        ("religion", "religion"),
        ("province", "province"),
    ] {
        if let Some(n) = q[key]["name"].as_str() {
            parts.push(format!("{label} {n}"));
        }
    }
    if let Some(n) = q["nearest_place"]["name"].as_str() {
        parts.push(format!("near {n}"));
    }
    parts.join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn world() -> (Config, World) {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 4, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "pangea", "states": 3, "towns": 6 },
            "pins": {
                "points": [
                    { "id": "ostrey", "kind": "city", "name": "Ostrey", "at": [200, 150] },
                    { "id": "mill", "kind": "village", "at": [230, 160] }
                ],
                "regions": [{ "id": "hills", "kind": "hills", "box": [150, 100, 260, 200] }],
                "roads": [{ "id": "mill-road", "from": "ostrey", "to": "mill", "target_length": 40 }]
            }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        (c, w)
    }

    #[test]
    fn the_burg_table_has_the_pins_then_the_generated_places() {
        let (c, w) = world();
        let rows = burg_rows(&c, Some(&w));
        assert_eq!(rows[0].id, "ostrey");
        assert!(rows[0].pinned);
        assert_eq!(rows[0].cells[0], "Ostrey");
        assert!(rows[1].cells[0].ends_with("(generated)"), "{:?}", rows[1]);
        let generated: Vec<&Row> = rows.iter().filter(|r| !r.pinned).collect();
        assert!(!generated.is_empty());
        let g = generated_place(&w, &generated[0].id).unwrap();
        assert_eq!(g.name, generated[0].cells[0]);
        // Without a world, only the pins show.
        assert_eq!(burg_rows(&c, None).len(), 2);
    }

    #[test]
    fn the_pin_tables_show_the_config() {
        let (c, w) = world();
        let r = region_rows(&c);
        assert_eq!(
            r[0].cells,
            vec!["hills", "hills", "box 150, 100 to 260, 200", "35-60"]
        );
        let roads = route_rows(&c, Some(&w));
        assert_eq!(roads[0].cells[2], "ostrey to mill");
        assert_eq!(roads[0].cells[4], "40 +/- 5%");
        assert!(!roads[0].cells[5].is_empty(), "the world has the length");
        assert!(river_rows(&c, Some(&w)).is_empty());
        assert!(brush_rows(&c).is_empty());
    }

    #[test]
    fn the_society_tables_count_cells_and_burgs() {
        let (_, w) = world();
        let states = state_rows(&w);
        assert_eq!(states.len(), w.society.states.len());
        let cells: usize = states
            .iter()
            .map(|r| r.cells[3].parse::<usize>().unwrap())
            .sum();
        assert!(cells > 0);
        assert!(!culture_rows(&w).is_empty());
        assert!(!religion_rows(&w).is_empty());
        assert_eq!(province_rows(&w).len(), w.society.provinces.len());
    }

    #[test]
    fn the_cell_text_comes_from_the_query() {
        let (c, w) = world();
        let q = inkland::query_at(&w, &c, [200.0, 150.0]).unwrap();
        let lines = cell_lines(&q);
        let get = |k: &str| lines.iter().find(|(l, _)| l == k).map(|(_, v)| v.clone());
        assert_eq!(get("Pinned areas").as_deref(), Some("hills"));
        assert!(get("Burg").unwrap().starts_with("Ostrey, 0 away"));
        let status = status_text(&q);
        assert!(
            status.contains("(land)") && status.contains("near Ostrey"),
            "{status}"
        );
    }
}
