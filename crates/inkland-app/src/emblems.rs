//! The Emblems editor: the coat of arms of a state, a province, or a burg
//! of the shown world, as FMG shows it in its Emblems editor
//! (`controllers/political/emblems/emblems-editor.ts`).
//!
//! The arms come from the world. "Set custom" writes a coat of arms to the
//! config: `coa` of a burg pin, `state_coa` of the pin that is the capital
//! of a state, or the rename entry of a province. The visual editor
//! changes one part of the arms at a time, as the FMG editor does.

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

/// The prefix of the custom arms path of a province: `area:province:x,y`,
/// with the site of its center cell. The arms go into `world.renames`.
pub const AREA_PREFIX: &str = "area:province:";

/// The point of a province path.
pub fn area_point(path: &str) -> Option<[f64; 2]> {
    let (x, y) = path.strip_prefix(AREA_PREFIX)?.split_once(',')?;
    Some([x.parse().ok()?, y.parse().ok()?])
}

/// The JSON Pointer of the config field for custom arms of an item, or
/// `None` if the config has no field for it: a state needs a pinned
/// capital with `capital_of`, and a burg must be a pin. A province gives
/// `area:province:x,y` (see `AREA_PREFIX`).
pub fn custom_path(config: &Config, world: &World, kind: usize, id: u32) -> Option<String> {
    let s = &world.society;
    let i = (id as usize).checked_sub(1)?;
    if kind == 1 {
        let (_, at) = crate::rows::area_point(world, "provinces", &id.to_string())?;
        let at = [crate::model::round2(at[0]), crate::model::round2(at[1])];
        return Some(format!("{AREA_PREFIX}{},{}", at[0], at[1]));
    }
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
    if let Some(at) = area_point(path) {
        return config
            .world
            .renames
            .iter()
            .find(|r| {
                r.kind == inkland_core::config::AreaKind::Province
                    && r.at == Some(at)
                    && r.pin.is_none()
            })
            .and_then(|r| r.coa.clone());
    }
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

/// The parts of the arms that the visual editor changes, in the order of
/// its selects.
pub const PARTS: [&str; 8] = [
    "shield",
    "field",
    "division",
    "division-tincture",
    "ordinary",
    "ordinary-tincture",
    "charge",
    "charge-tincture",
];

/// The choices of each part, in `PARTS` order. A part that can be empty
/// starts with `none`.
pub fn choices() -> &'static [Vec<String>; 8] {
    static CHOICES: std::sync::OnceLock<[Vec<String>; 8]> = std::sync::OnceLock::new();
    CHOICES.get_or_init(make_choices)
}

fn make_choices() -> [Vec<String>; 8] {
    use inkland_core::emblems::data as d;
    let names = |w: d::Weights| w.iter().map(|(n, _)| (*n).to_owned()).collect::<Vec<_>>();
    let mut tinctures = names(d::METALS);
    tinctures.extend(names(d::COLOURS));
    tinctures.extend(names(d::STAINS));
    let none = || vec!["none".to_owned()];
    let mut shields: Vec<String> = d::SHIELDS.iter().flat_map(|(_, w)| names(w)).collect();
    shields.dedup();
    let mut divisions = none();
    divisions.extend(names(d::DIVISIONS));
    let mut ordinaries = none();
    for n in names(d::ORDINARIES_LINED)
        .into_iter()
        .chain(names(d::ORDINARIES_STRAIGHT))
    {
        if !ordinaries.contains(&n) {
            ordinaries.push(n);
        }
    }
    // Only the charges that inkland can draw: some FMG charges have no art
    // with a free license.
    let mut charges = none();
    charges.extend(
        d::CHARGES
            .iter()
            .flat_map(|(_, w)| w.iter().map(|(n, _)| *n))
            .chain(d::CHARGE_DATA.iter().map(|(n, _)| *n))
            .collect::<std::collections::BTreeSet<&str>>()
            .into_iter()
            .filter(|n| {
                let coa: Option<Coa> = serde_json::from_value(serde_json::json!({
                    "t1": "gules", "charges": [{ "charge": n, "t": "or", "p": "e" }]
                }))
                .ok();
                coa.is_some_and(|c| inkland_core::emblems::validate(&c).is_empty())
            })
            .map(str::to_owned),
    );
    [
        shields,
        tinctures.clone(),
        divisions,
        tinctures.clone(),
        ordinaries,
        tinctures.clone(),
        charges,
        tinctures,
    ]
}

/// The index of the value of each part of a coat of arms in `choices`.
/// A value that is not in the list gives 0.
pub fn indexes(coa: &Coa) -> [usize; 8] {
    let c = choices();
    let find = |k: usize, v: Option<&str>| {
        v.and_then(|v| c[k].iter().position(|x| x == v))
            .unwrap_or(0)
    };
    [
        find(0, coa.shield.as_deref()),
        find(1, Some(&coa.t1)),
        find(2, coa.division.as_ref().map(|d| d.division.as_str())),
        find(3, coa.division.as_ref().map(|d| d.t.as_str())),
        find(4, coa.ordinaries.first().map(|o| o.ordinary.as_str())),
        find(5, coa.ordinaries.first().map(|o| o.t.as_str())),
        find(6, coa.charges.first().map(|o| o.charge.as_str())),
        find(7, coa.charges.first().map(|o| o.t.as_str())),
    ]
}

/// A tincture that shows on the field: a metal on a colour, and a colour
/// on a metal (the rule of tincture).
fn contrast(field: &str) -> &'static str {
    use inkland_core::emblems::data as d;
    if d::METALS.iter().any(|(n, _)| *n == field) {
        "azure"
    } else {
        "or"
    }
}

/// The arms with the part `part` (see `PARTS`) set to choice `index`. A new
/// division, ordinary, or charge gets a tincture that shows on the field.
pub fn compose(coa: &Coa, part: usize, index: usize) -> Coa {
    use inkland_core::emblems::{Charge, Division, Ordinary};
    let c = choices();
    let Some(value) = c.get(part).and_then(|l| l.get(index)).cloned() else {
        return coa.clone();
    };
    let none = value == "none";
    let mut out = coa.clone();
    let on_field = contrast(&out.t1).to_owned();
    match part {
        0 => out.shield = Some(value),
        1 => out.t1 = value,
        2 if none => out.division = None,
        2 => {
            let t = out.division.as_ref().map_or(on_field, |d| d.t.clone());
            out.division = Some(Division {
                division: value,
                t,
                line: None,
            });
        }
        3 => {
            if let Some(d) = &mut out.division {
                d.t = value;
            }
        }
        4 if none => out.ordinaries.clear(),
        4 => {
            let t = out.ordinaries.first().map_or(on_field, |o| o.t.clone());
            let o: Ordinary =
                serde_json::from_value(serde_json::json!({ "ordinary": value, "t": t }))
                    .expect("an ordinary of a name and a tincture");
            out.ordinaries = vec![o];
        }
        5 => {
            if let Some(o) = out.ordinaries.first_mut() {
                o.t = value;
            }
        }
        6 if none => out.charges.clear(),
        6 => {
            let t = out.charges.first().map_or(on_field, |o| o.t.clone());
            let p = out
                .charges
                .first()
                .map_or_else(|| "e".to_owned(), |o| o.p.clone());
            let ch: Charge =
                serde_json::from_value(serde_json::json!({ "charge": value, "t": t, "p": p }))
                    .expect("a charge of a name, a tincture, and a position");
            out.charges = vec![ch];
        }
        7 => {
            if let Some(o) = out.charges.first_mut() {
                o.t = value;
            }
        }
        _ => {}
    }
    out
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
        // A province has a path to its rename entry.
        let province = custom_path(&c, &w, 1, 1).unwrap();
        assert!(area_point(&province).is_some(), "{province}");
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

    #[test]
    fn province_arms_go_into_the_rename_entry_and_come_back_on_a_new_build() {
        let (c, w) = world();
        let mut e = crate::model::Editor::new(c.clone());
        let path = custom_path(e.config(), &w, 1, 1).unwrap();
        let at = area_point(&path).unwrap();
        let coa: Coa = serde_json::from_value(parse(r#"{"t1":"vert"}"#).unwrap()).unwrap();
        let kind = inkland_core::config::AreaKind::Province;
        e.set_area_coa(kind, at, Some(coa)).unwrap();
        assert_eq!(custom(e.config(), &path).unwrap().t1, "vert");
        let w2 = World::build(e.config()).unwrap();
        assert_eq!(w2.society.provinces[0].coa.as_ref().unwrap().t1, "vert");
        // A name stays when the arms go.
        e.rename_area(kind, at, "Hearth").unwrap();
        e.set_area_coa(kind, at, None).unwrap();
        assert_eq!(custom(e.config(), &path), None);
        assert_eq!(e.config().world.renames[0].name.as_deref(), Some("Hearth"));
        e.rename_area(kind, at, "").unwrap();
        assert_eq!(e.config(), &c);
    }

    #[test]
    fn the_visual_editor_changes_one_part_and_keeps_valid_arms() {
        let base: Coa = serde_json::from_value(serde_json::json!({ "t1": "gules" })).unwrap();
        let c = choices();
        let pick = |k: usize, v: &str| c[k].iter().position(|x| x == v).unwrap();
        let mut coa = compose(&base, 0, pick(0, "heater"));
        coa = compose(&coa, 2, pick(2, "perPale"));
        coa = compose(&coa, 4, pick(4, "fess"));
        coa = compose(&coa, 6, pick(6, "mullet"));
        coa = compose(&coa, 7, pick(7, "argent"));
        assert_eq!(coa.shield.as_deref(), Some("heater"));
        assert_eq!(
            coa.division.as_ref().unwrap().t,
            "or",
            "a metal on the colour field"
        );
        assert_eq!(coa.ordinaries[0].ordinary, "fess");
        assert_eq!(
            (coa.charges[0].charge.as_str(), coa.charges[0].t.as_str()),
            ("mullet", "argent")
        );
        assert!(
            inkland_core::emblems::validate(&coa).is_empty(),
            "{:?}",
            inkland_core::emblems::validate(&coa)
        );
        let idx = indexes(&coa);
        assert!(
            c[6].iter().any(|x| x == "fusil"),
            "a conventional charge is a choice"
        );
        assert_eq!(c[6][idx[6]], "mullet");
        assert_eq!(c[1][idx[1]], "gules");
        // "none" takes a part away.
        let plain = compose(&compose(&coa, 6, 0), 2, 0);
        assert!(plain.charges.is_empty() && plain.division.is_none());
        // Each choice of each part gives valid arms.
        for (k, list) in c.iter().enumerate() {
            for (i, choice) in list.iter().enumerate() {
                let x = compose(&coa, k, i);
                assert!(
                    inkland_core::emblems::validate(&x).is_empty(),
                    "{} {choice}",
                    PARTS[k]
                );
            }
        }
    }
}
