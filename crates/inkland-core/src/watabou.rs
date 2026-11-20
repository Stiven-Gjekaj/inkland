//! Links to the town generators of Watabou for each place, as the original
//! program makes them (`createWatabouCityLinks`,
//! `createWatabouVillageLinks`, and `createWatabouDwellingLinks` of
//! `burgs-generator.ts`).
//!
//! A link opens a town map of the place on watabou.github.io. The site
//! draws the map; inkland only writes the link. The parameters come from
//! the place and its cell, so the same seed and config give the same link.
//!
//! - The groups `capital`, `city`, and `town` link to the city generator.
//! - The groups `village` and `hamlet` link to the village generator.
//! - The other groups have no link.

use crate::military::{POPULATION_SCALE, URBANIZATION_RATE};
use crate::society::Place;
use crate::world::World;
use std::collections::BTreeMap;

/// The people of one unit of area of a city (`urbanization.density`).
pub const URBAN_DENSITY: f64 = 10.0;

/// The generator of the town map of a place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generator {
    City,
    Village,
    Dwelling,
}

impl Generator {
    /// The generator of an FMG burg group, if it has one.
    pub fn of_group(group: &str) -> Option<Generator> {
        match group {
            "capital" | "city" | "town" => Some(Generator::City),
            "village" | "hamlet" => Some(Generator::Village),
            _ => None,
        }
    }
}

/// Round to `d` decimals (`rn` of the original program).
fn rn(v: f64, d: i32) -> f64 {
    let m = libm::pow(10.0, f64::from(d));
    libm::round(v * m) / m
}

/// A number as JavaScript writes it: no `.0` for a whole number.
fn js(v: f64) -> String {
    if v == libm::trunc(v) && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// A query value, encoded as `URLSearchParams` does: letters, digits, and
/// `*-._` stay, a space becomes `+`, and each other byte becomes `%XX`.
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn url(base: &str, params: &[(&str, String)]) -> String {
    let query: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect();
    format!("{base}?{}", query.join("&"))
}

/// The seed of the town map of a place: the map seed, then the place id
/// with 4 digits.
fn seed_of(world: &World, place: &Place) -> String {
    format!("{}{:04}", world.seed, place.id)
}

/// The route links of each cell: the neighbour cell and the route group.
fn links(world: &World) -> Vec<BTreeMap<usize, &'static str>> {
    let mut links: Vec<BTreeMap<usize, &'static str>> = vec![BTreeMap::new(); world.grid.len()];
    for r in world.roads.iter().chain(&world.sea_routes) {
        for w in r.cells.windows(2) {
            if w[0] != w[1] {
                links[w[0]].insert(w[1], r.group);
                links[w[1]].insert(w[0], r.group);
            }
        }
    }
    links
}

/// The links to the town maps of all places, in the order of the places.
/// A place whose group has no generator gets `None`.
pub fn town_links(world: &World) -> Vec<Option<String>> {
    let links = links(world);
    let connectivity =
        crate::routes::connectivity(&world.roads, &world.sea_routes, world.grid.len());
    world
        .society
        .places
        .iter()
        .map(|p| match Generator::of_group(&p.group)? {
            Generator::City => Some(city(world, p, &links)),
            Generator::Village => Some(village(world, p, &connectivity)),
            Generator::Dwelling => Some(dwelling(world, p)),
        })
        .collect()
}

/// The link to the city generator.
fn city(world: &World, p: &Place, links: &[BTreeMap<usize, &'static str>]) -> String {
    let cells = &world.society.cells;
    let c = p.cell;
    let size_raw = 2.13 * libm::pow(p.population * POPULATION_SCALE / URBAN_DENSITY, 0.385);
    let size = libm::ceil(size_raw).clamp(6.0, 100.0);
    let population = rn(p.population * POPULATION_SCALE * URBANIZATION_RATE, 0);
    // A river port inland has no coast.
    let coast = p.port && cells.haven[c].is_some();
    let river = cells.r[c] != 0;
    // The direction of the sea: 0 east, 0.5 north, 1 west, 1.5 south.
    let sea = cells.haven[c].filter(|_| coast).map(|h| {
        let [x1, y1] = world.grid.points[c];
        let [x2, y2] = world.grid.points[h];
        let deg = libm::atan2(y2 - y1, x2 - x1).to_degrees();
        if deg <= 0.0 {
            rn((deg.abs() / 180.0).clamp(0.0, 1.0), 2)
        } else {
            rn(2.0 - (deg / 180.0).clamp(0.0, 1.0), 2)
        }
    });
    let biome = world.biomes[c];
    let arable = if river { 1..=8 } else { 5..=8 };
    let farms = arable.contains(&biome);
    let urban_castle = p.citadel && p.id.is_multiple_of(2);
    let l = &links[c];
    let hub = l.len() > 3 || l.values().filter(|&&g| g == "roads").count() > 2;
    let b = |v: bool| if v { "1" } else { "0" }.to_owned();
    let mut params = vec![
        ("name", p.name.clone()),
        ("population", js(population)),
        ("size", js(size)),
        ("seed", seed_of(world, p)),
        ("river", b(river)),
        ("coast", b(coast)),
        ("farms", b(farms)),
        ("citadel", b(p.citadel)),
        ("urban_castle", b(urban_castle)),
        ("hub", b(hub)),
        ("plaza", b(p.plaza)),
        ("greens", b(p.plaza)),
        ("temple", b(p.temple)),
        ("walls", b(p.walls)),
        ("shantytown", b(p.shanty)),
        ("style", "natural".to_owned()),
    ];
    // The original program leaves out a sea direction of 0.
    if let Some(s) = sea.filter(|&s| s != 0.0) {
        params.push(("sea", js(s)));
    }
    url("https://watabou.github.io/city-generator/", &params)
}

/// The link to the village generator.
fn village(world: &World, p: &Place, connectivity: &[f64]) -> String {
    let cells = &world.society.cells;
    let c = p.cell;
    let pop = rn(p.population * POPULATION_SCALE * URBANIZATION_RATE, 0);
    let each = |n: usize| c.is_multiple_of(n);
    let mut tags: Vec<&str> = Vec::new();
    let feature_cells = world.features.get(world.features.of_cell[c]).cells;
    if cells.r[c] != 0 && cells.haven[c].is_some() {
        tags.push("estuary");
    } else if cells.haven[c].is_some() && feature_cells == 1 {
        tags.push("island,district");
    } else if p.port && cells.haven[c].is_some() {
        tags.push("coast");
    } else if cells.conf[c] != 0.0 {
        tags.push("confluence");
    } else if cells.r[c] != 0 {
        tags.push("river");
    } else if pop < 200.0 && each(4) {
        tags.push("pond");
    }
    let rate = connectivity[c];
    tags.push(if rate > 1.0 {
        "highway"
    } else if rate == 1.0 {
        "dead end"
    } else {
        "isolated"
    });
    let biome = world.biomes[c];
    let arable = if cells.r[c] != 0 { 1..=8 } else { 5..=8 };
    if !arable.contains(&biome) {
        tags.push("uncultivated");
    } else if each(6) {
        tags.push("farmland");
    }
    let temp = world.temperature[c];
    if temp <= 0.0 || temp > 28.0 || (temp > 25.0 && each(3)) {
        tags.push("no orchards");
    }
    if !p.plaza {
        tags.push("no square");
    }
    if p.walls {
        tags.push("palisade");
    }
    if pop < 100.0 {
        tags.push("sparse");
    } else if pop > 300.0 {
        tags.push("dense");
    }
    let width: f64 = if pop > 1500.0 {
        1600.0
    } else if pop > 1000.0 {
        1400.0
    } else if pop > 500.0 {
        1000.0
    } else if pop > 200.0 {
        800.0
    } else if pop > 100.0 {
        600.0
    } else {
        400.0
    };
    let height = rn(width / 2.05, 0);
    let style = if matches!(biome, 1 | 2) {
        "sand"
    } else if temp <= 5.0 || matches!(biome, 9..=11) {
        "snow"
    } else {
        "default"
    };
    url(
        "https://watabou.github.io/village-generator/",
        &[
            ("pop", js(pop)),
            ("name", p.name.clone()),
            ("seed", seed_of(world, p)),
            ("width", js(width)),
            ("height", js(height)),
            ("style", style.to_owned()),
            ("tags", tags.join(",")),
        ],
    )
}

/// The link to the dwellings generator.
fn dwelling(world: &World, p: &Place) -> String {
    let pop = rn(p.population * POPULATION_SCALE * URBANIZATION_RATE, 0);
    let tags = if pop > 200.0 {
        "large,tall"
    } else if pop > 100.0 {
        "large"
    } else if pop > 50.0 {
        "tall"
    } else if pop > 20.0 {
        "low"
    } else {
        "small"
    };
    url(
        "https://watabou.github.io/dwellings/",
        &[
            ("pop", js(pop)),
            ("name", String::new()),
            ("seed", seed_of(world, p)),
            ("tags", tags.to_owned()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use serde_json::json;

    #[test]
    fn a_query_is_encoded_as_url_search_params() {
        assert_eq!(encode("Saint Ivo's"), "Saint+Ivo%27s");
        assert_eq!(encode("no square,dense"), "no+square%2Cdense");
        assert_eq!(encode("Ærø"), "%C3%86r%C3%B8");
        assert_eq!(js(55.0), "55");
        assert_eq!(js(0.45), "0.45");
    }

    fn world() -> World {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 42, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "continents", "states": 4, "towns": 30 }
        }))
        .unwrap();
        World::build(&c).unwrap()
    }

    #[test]
    fn each_town_links_to_its_generator_and_the_others_have_none() {
        let w = world();
        let links = town_links(&w);
        assert_eq!(links.len(), w.society.places.len());
        let mut seen = (0, 0);
        for (p, l) in w.society.places.iter().zip(&links) {
            match Generator::of_group(&p.group) {
                Some(Generator::City) => {
                    let l = l.as_deref().unwrap();
                    assert!(l.starts_with("https://watabou.github.io/city-generator/?name="));
                    assert!(l.contains(&format!("&seed=42{:04}&", p.id)), "{l}");
                    let size: f64 = l
                        .split("&size=")
                        .nth(1)
                        .unwrap()
                        .split('&')
                        .next()
                        .unwrap()
                        .parse()
                        .unwrap();
                    assert!((6.0..=100.0).contains(&size));
                    seen.0 += 1;
                }
                Some(Generator::Village) => {
                    let l = l.as_deref().unwrap();
                    assert!(l.starts_with("https://watabou.github.io/village-generator/?pop="));
                    assert!(l.contains("&tags="));
                    seen.1 += 1;
                }
                _ => assert!(l.is_none(), "{}", p.group),
            }
        }
        assert!(seen.0 > 0 && seen.1 > 0, "{seen:?}");
        // The same world gives the same links.
        assert_eq!(town_links(&world()), links);
    }

    #[test]
    fn a_city_link_has_the_parameters_of_the_original_program() {
        let w = world();
        let links = town_links(&w);
        let (p, l) = w
            .society
            .places
            .iter()
            .zip(&links)
            .find(|(p, _)| p.group == "capital")
            .unwrap();
        let l = l.as_deref().unwrap();
        let keys: Vec<&str> = l
            .split('?')
            .nth(1)
            .unwrap()
            .split('&')
            .map(|kv| kv.split('=').next().unwrap())
            .collect();
        let expected = [
            "name",
            "population",
            "size",
            "seed",
            "river",
            "coast",
            "farms",
            "citadel",
            "urban_castle",
            "hub",
            "plaza",
            "greens",
            "temple",
            "walls",
            "shantytown",
            "style",
        ];
        assert_eq!(&keys[..expected.len()], &expected);
        assert!(l.contains(&format!("&walls={}&", u8::from(p.walls))));
        assert!(l.contains(&format!("&citadel={}&", u8::from(p.citadel))));
    }

    #[test]
    fn a_dwelling_link_gives_its_size_tags() {
        let w = world();
        let mut p = w.society.places[0].clone();
        p.population = 0.03;
        let l = dwelling(&w, &p);
        assert!(l.starts_with("https://watabou.github.io/dwellings/?pop=30&name=&seed="));
        assert!(l.ends_with("&tags=low"));
    }
}
