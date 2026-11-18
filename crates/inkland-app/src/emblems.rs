//! The Emblems editor: the coat of arms of a state, a province, or a burg
//! of the shown world, as FMG shows it in its Emblems editor
//! (`controllers/political/emblems/emblems-editor.ts`).
//!
//! The arms come from the world. "Set custom" writes a coat of arms to the
//! config: `coa` of a burg pin, or `state_coa` of the pin that is the
//! capital of a state. A province has no config field.

use inkland_core::config::Config;
use inkland_core::emblems::Coa;
use inkland_core::world::World;

/// The kinds of items, in the order of the select of the dialog.
pub const KINDS: [&str; 3] = ["State", "Province", "Burg"];

/// The items of a kind: an id and a label, in the order of the world.
pub fn items(world: &World, kind: usize) -> Vec<(u32, String)> {
    let s = &world.society;
    match kind {
        0 => s
            .states
            .iter()
            .map(|x| (x.id, x.full_name.clone()))
            .collect(),
        1 => s
            .provinces
            .iter()
            .map(|x| (x.id, x.full_name.clone()))
            .collect(),
        _ => s.places.iter().map(|x| (x.id, x.name.clone())).collect(),
    }
}

/// The coat of arms of an item.
pub fn coa(world: &World, kind: usize, id: u32) -> Option<&Coa> {
    let s = &world.society;
    let i = (id as usize).checked_sub(1)?;
    match kind {
        0 => s.states.get(i)?.coa.as_ref(),
        1 => s.provinces.get(i)?.coa.as_ref(),
        _ => s.places.get(i)?.coa.as_ref(),
    }
}

/// The JSON Pointer of the config field for custom arms of an item, or
/// `None` if the config has no field for it: a state needs a pinned
/// capital with `capital_of`, a burg must be a pin, and a province has no
/// field.
pub fn custom_path(config: &Config, world: &World, kind: usize, id: u32) -> Option<String> {
    let s = &world.society;
    let i = (id as usize).checked_sub(1)?;
    let place = match kind {
        0 => s
            .places
            .get((s.states.get(i)?.capital as usize).checked_sub(1)?)?,
        2 => s.places.get(i)?,
        _ => return None,
    };
    let pin = place.pin.as_deref()?;
    let k = config.pins.points.iter().position(|p| p.id == pin)?;
    match kind {
        0 if config.pins.points[k].capital_of.is_some() => {
            Some(format!("/pins/points/{k}/state_coa"))
        }
        0 => None,
        _ => Some(format!("/pins/points/{k}/coa")),
    }
}

/// The custom arms of the config at a path, if there are any.
pub fn custom(config: &Config, path: &str) -> Option<Coa> {
    let (k, field) = path.strip_prefix("/pins/points/")?.split_once('/')?;
    let pin = config.pins.points.get(k.parse::<usize>().ok()?)?;
    match field {
        "coa" => pin.coa.clone(),
        "state_coa" => pin.state_coa.clone(),
        _ => None,
    }
}

/// Read the JSON text of a coat of arms, and check it as the config does.
pub fn parse(text: &str) -> Result<serde_json::Value, String> {
    let coa: Coa = serde_json::from_str(text.trim())
        .map_err(|e| format!("the arms are not valid JSON of FMG: {e}"))?;
    if let Some(f) = inkland_core::emblems::validate(&coa).first() {
        return Err(format!("{}: {}", f.path, f.message));
    }
    Ok(serde_json::to_value(&coa).expect("arms convert to JSON"))
}

/// A short text of a coat of arms: the shield, the field, the division,
/// the ordinaries, and the charges.
pub fn summary(coa: &Coa) -> String {
    let mut parts = vec![format!(
        "Shield {}. Field {}.",
        coa.shield.as_deref().unwrap_or("heater"),
        coa.t1
    )];
    if let Some(d) = &coa.division {
        let line = d.line.as_deref().filter(|l| *l != "straight");
        parts.push(match line {
            Some(l) => format!("Division {} {l}, {}.", d.division, d.t),
            None => format!("Division {}, {}.", d.division, d.t),
        });
    }
    for o in &coa.ordinaries {
        parts.push(format!("Ordinary {}, {}.", o.ordinary, o.t));
    }
    for c in &coa.charges {
        parts.push(format!("Charge {}, {}, at {}.", c.charge, c.t, c.p));
    }
    parts.join(" ")
}

/// The image of a coat of arms: RGBA pixels of `size` x `size` on white.
pub fn image(coa: &Coa, size: u32) -> Result<Vec<u8>, String> {
    let look = inkland_render::emblem::Look::COLOUR;
    // 25.4 mm is 96 CSS pixels.
    let mm = f64::from(size) * 25.4 / 96.0;
    let svg = inkland_render::emblem::document(coa, "emblem-dialog", &look, mm);
    let scene =
        inkland_render::png::Scene::parse(&svg, &inkland_render::png::empty_font_db(), &[])?;
    let scale = size as f32 / scene.size()[0].max(1.0);
    scene.render_rgba(size, size, scale, [0.0, 0.0], [255, 255, 255])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn world() -> (Config, World) {
        let c: Config = serde_json::from_value(json!({
            "version": 1, "seed": 2, "frame": { "width": 300, "height": 200 },
            "world": { "cells": 2000, "template": "pangea" },
            "pins": { "points": [
                { "id": "home", "kind": "city", "at": [150, 100], "capital_of": "Realm" },
                { "id": "mill", "kind": "village", "at": [120, 90] }
            ] }
        }))
        .unwrap();
        let w = World::build(&c).unwrap();
        (c, w)
    }

    #[test]
    fn each_item_has_arms_and_an_image() {
        let (_, w) = world();
        for kind in 0..3 {
            let list = items(&w, kind);
            assert!(!list.is_empty());
            let coa = coa(&w, kind, list[0].0).expect("arms");
            assert!(summary(coa).starts_with("Shield "));
            let px = image(coa, 64).unwrap();
            assert_eq!(px.len(), 64 * 64 * 4);
            // Not all white.
            assert!(px.chunks(4).any(|p| p[..3] != [255, 255, 255]));
        }
    }

    #[test]
    fn custom_arms_go_to_the_pin_of_a_burg_or_a_capital() {
        let (c, w) = world();
        let realm = w.society.states.iter().find(|s| s.name == "Realm").unwrap();
        assert_eq!(
            custom_path(&c, &w, 0, realm.id).as_deref(),
            Some("/pins/points/0/state_coa")
        );
        assert_eq!(
            custom_path(&c, &w, 2, 2).as_deref(),
            Some("/pins/points/1/coa")
        );
        assert_eq!(custom_path(&c, &w, 1, 1), None);
        // A generated place has no field.
        let generated = w.society.places.iter().find(|p| p.pin.is_none()).unwrap();
        assert_eq!(custom_path(&c, &w, 2, generated.id), None);
        assert!(
            parse(r#"{"t1":"or","charges":[{"charge":"mullet","t":"azure","p":"e"}]}"#).is_ok()
        );
        assert!(parse(r#"{"t1":"blue"}"#).unwrap_err().contains("/t1"));
        assert!(parse("not json").is_err());
    }

    #[test]
    fn set_custom_is_one_undoable_edit() {
        let (c, w) = world();
        let mut e = crate::model::Editor::new(c);
        let path = custom_path(e.config(), &w, 2, 2).unwrap();
        let value = parse(r#"{"t1":"vert"}"#).unwrap();
        e.set(&path, value).unwrap();
        assert_eq!(custom(e.config(), &path).unwrap().t1, "vert");
        assert!(e.undo().unwrap());
        assert_eq!(custom(e.config(), &path), None);
    }
}
