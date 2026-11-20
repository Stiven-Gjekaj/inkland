//! The offline town map of a burg in the Town Map dialog: the SVG of the
//! port of TownGeneratorOS, and its image.

use inkland::{Config, World};

/// A drawn town map: its SVG, its name, and its image.
pub struct TownImage {
    pub svg: String,
    pub name: String,
    pub buildings: usize,
    /// RGBA pixels of `width` x `height`.
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Draw the town map of the place at `index` of the world, `width` pixels
/// wide. `colour`: the colours of the original generator; else black and
/// white.
pub fn draw(
    world: &World,
    config: &Config,
    index: usize,
    colour: bool,
    width: u32,
) -> Result<TownImage, String> {
    let place = world
        .society
        .places
        .get(index)
        .ok_or_else(|| "the world has no such burg".to_owned())?;
    let t = inkland::town_map(world, config, &place.id.to_string(), colour, 150.0)
        .map_err(|e| e.message)?;
    let dirs: Vec<&std::path::Path> = config.font_dirs.iter().map(std::path::Path::new).collect();
    let db = inkland_render::png::font_db(&dirs);
    let family = config.render.fonts.title.as_str();
    let scene = inkland_render::png::Scene::parse(&t.svg, &db, &[family])?;
    let [sw, sh] = scene.size();
    let scale = width as f32 / sw.max(1.0);
    let height = (sh * scale).round().max(1.0) as u32;
    let pixels = scene.render_rgba(width, height, scale, [0.0, 0.0], [255, 255, 255])?;
    Ok(TownImage {
        svg: t.svg,
        name: t.name,
        buildings: t.buildings,
        pixels,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_town_map_draws_as_an_image() {
        let c: Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 5, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 2000, "template": "continents", "states": 3, "towns": 20 }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        let i = (0..w.society.places.len())
            .max_by(|&a, &b| {
                w.society.places[a]
                    .population
                    .total_cmp(&w.society.places[b].population)
            })
            .unwrap();
        let img = draw(&w, &c, i, true, 300).unwrap();
        assert_eq!(img.pixels.len(), (img.width * img.height * 4) as usize);
        assert!(img.height > img.width);
        assert!(img.buildings > 0);
        // Not all one colour: the map has buildings on paper.
        let first = &img.pixels[..4];
        assert!(img.pixels.chunks(4).any(|p| p != first));
        assert!(draw(&w, &c, usize::MAX, true, 300).is_err());
    }
}
