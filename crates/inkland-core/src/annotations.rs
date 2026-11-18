//! Annotations: measurers (`measurers-generator.ts`), legend boxes
//! (`legends.ts`), and the notes of the map entities (`story/notes.ts`).
//!
//! - Generation makes one default ruler across the largest landmass
//!   (`createDefaultRuler`).
//! - Generation makes no legend box. FMG editors add them; the list starts
//!   empty (`Legends.initiate`).
//! - A note is the text of an entity. `notes` lists each note of the
//!   world in the FMG order of the entity types.

use crate::features::Features;
use crate::grid::Grid;

/// The kind of a measurer (FMG `MeasurerType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurerKind {
    Ruler,
    Opisometer,
    RouteOpisometer,
    Planimeter,
}

impl MeasurerKind {
    /// The FMG name.
    pub fn name(self) -> &'static str {
        match self {
            MeasurerKind::Ruler => "Ruler",
            MeasurerKind::Opisometer => "Opisometer",
            MeasurerKind::RouteOpisometer => "RouteOpisometer",
            MeasurerKind::Planimeter => "Planimeter",
        }
    }
}

/// A measurer: a kind and its points in world units.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurer {
    pub kind: MeasurerKind,
    pub points: Vec<[f64; 2]>,
}

impl Measurer {
    /// The length of the line through the points, in world units.
    pub fn length(&self) -> f64 {
        self.points
            .windows(2)
            .map(|w| libm::hypot(w[1][0] - w[0][0], w[1][1] - w[0][1]))
            .sum()
    }
}

/// One item of a legend box: a color and its text.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendItem {
    pub color: String,
    pub text: String,
}

/// A titled legend box (FMG `pack.legends`).
#[derive(Debug, Clone, PartialEq)]
pub struct Legend {
    pub name: String,
    pub items: Vec<LegendItem>,
}

/// The FMG margin of the default ruler, in FMG pixels.
const MARGIN_PX: f64 = 100.0;

/// The default ruler across the largest landmass (`createDefaultRuler`).
/// `px` is the FMG pixels for each world unit. The ruler goes from the
/// leftmost to the rightmost outline vertex of the landmass that is at
/// least the margin from the frame edges.
pub fn default_ruler(grid: &Grid, features: &Features, px: f64) -> Vec<Measurer> {
    let (w, h) = (grid.width, grid.height);
    let margin = MARGIN_PX / px.max(1e-9);
    let largest = features
        .list
        .iter()
        .filter(|f| f.kind.is_land())
        .max_by(|a, b| a.area.total_cmp(&b.area).then(b.id.cmp(&a.id)));
    let (min_x, max_x, min_y, max_y) = (margin, w - margin, margin, h - margin);
    let mut left = [w - min_x, h / 2.0];
    let mut right = [min_x, h / 2.0];
    if let Some(f) = largest {
        for &v in &f.outline {
            let [x, y] = grid.vertices[v as usize];
            if y < min_y || y > max_y {
                continue;
            }
            if x < left[0] && x >= min_x {
                left = [x, y];
            }
            if x > right[0] && x <= max_x {
                right = [x, y];
            }
        }
    }
    vec![Measurer {
        kind: MeasurerKind::Ruler,
        points: vec![left, right],
    }]
}

/// A note of an entity of the world (FMG `NoteEntry`).
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    /// The entity type: `state`, `marker`, `regiment`, or `zone`.
    pub kind: &'static str,
    /// The id of the entity. A regiment has the id of its state first.
    pub id: String,
    /// The name of the entity.
    pub name: String,
    pub note: String,
}

/// All notes of a world, grouped by entity type (`Notes.list`).
pub fn notes(world: &crate::world::World) -> Vec<Note> {
    let mut out = Vec::new();
    for st in &world.society.states {
        for r in &st.military {
            if !r.note.is_empty() {
                out.push(Note {
                    kind: "regiment",
                    id: format!("{}-{}", st.id, r.id),
                    name: r.name.clone(),
                    note: r.note.clone(),
                });
            }
        }
    }
    for m in &world.markers {
        if let Some(legend) = &m.legend {
            out.push(Note {
                kind: "marker",
                id: m.id.to_string(),
                name: m.name.clone().unwrap_or_default(),
                note: legend.clone(),
            });
        }
    }
    for z in &world.zones {
        if !z.note.is_empty() {
            out.push(Note {
                kind: "zone",
                id: z.id.to_string(),
                name: z.name.clone(),
                note: z.note.clone(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::world::World;
    use serde_json::json;

    fn world(seed: u64) -> World {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": seed, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 3000, "template": "continents" }
        }))
        .unwrap();
        World::build(&c).unwrap()
    }

    #[test]
    fn the_default_ruler_lies_inside_the_margins() {
        for seed in [1, 2, 3] {
            let w = world(seed);
            assert_eq!(w.measurers.len(), 1);
            let r = &w.measurers[0];
            assert_eq!(r.kind, MeasurerKind::Ruler);
            assert_eq!(r.points.len(), 2);
            let margin = MARGIN_PX / w.society.cells.px;
            for p in &r.points {
                assert!(p[0] >= margin - 1e-9 && p[0] <= w.width - margin + 1e-9);
            }
            assert!(r.points[0][0] <= r.points[1][0]);
            assert!(r.length() > 0.0);
        }
    }

    #[test]
    fn notes_list_the_marker_legends_and_generation_adds_no_legend() {
        let w = world(5);
        assert!(w.legends.is_empty());
        let notes = notes(&w);
        let markers = w.markers.iter().filter(|m| m.legend.is_some()).count();
        assert_eq!(notes.iter().filter(|n| n.kind == "marker").count(), markers);
        assert!(notes.iter().all(|n| !n.note.is_empty()));
    }
}
