//! The names that the app shows in its lists: the map layers, the layer
//! presets, the style presets, the render modes, the templates, the culture
//! sets, and the tools.
//!
//! The lists follow the web app of Azgaar's Fantasy Map Generator (FMG).
//! Each list is in this file only. The app asks the config types which
//! names they accept, so a name that the renderer adds later becomes
//! available without a change here.

use inkland::Config;
use inkland_core::config::LayerName;
use serde_json::{Value, json};

/// What a layer button changes in the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A name in `render.layers`.
    Layer(&'static str),
    /// A field of `render.extras`: `scale_bar`, `compass`, or `grid`.
    Extra(&'static str),
    /// The renderer has no such layer.
    None,
}

/// A layer button of the Layers tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerDef {
    /// The FMG id of the layer, for example `burgIcons`.
    pub id: &'static str,
    /// The text of the button.
    pub label: &'static str,
    /// The key that shows or hides the layer, as typed text.
    pub key: Option<char>,
    pub target: Target,
}

const fn layer(
    id: &'static str,
    label: &'static str,
    key: Option<char>,
    target: Target,
) -> LayerDef {
    LayerDef {
        id,
        label,
        key,
        target,
    }
}

use Target::{Extra, Layer};

/// The layer buttons, in the order of the FMG layer registry. The inkland
/// layers that FMG does not have (coast, pinned regions, overlay features)
/// are at their place in the inkland draw order.
///
/// A `Layer` name is available when `LayerName` accepts it. The render
/// branch adds the FMG layers to `LayerName` with these names.
pub const LAYERS: &[LayerDef] = &[
    layer("texture", "Texture", Some('x'), Layer("texture")),
    layer("heightmap", "Heightmap", Some('h'), Layer("heightmap")),
    layer("coastline", "Coastline", None, Layer("coast")),
    layer("lakes", "Lakes", Some('q'), Layer("lakes")),
    layer("biomes", "Biomes", Some('b'), Layer("biomes")),
    layer("cells", "Cells", Some('e'), Layer("cells")),
    layer("grid", "Grid", Some(';'), Extra("grid")),
    layer(
        "coordinates",
        "Coordinates",
        Some('o'),
        Layer("coordinates"),
    ),
    layer("compass", "Wind Rose", Some('w'), Extra("compass")),
    layer("relief", "Relief", Some('f'), Layer("relief")),
    layer("rivers", "Rivers", Some('v'), Layer("rivers")),
    layer("religions", "Religions", Some('r'), Layer("religions")),
    layer("cultures", "Cultures", Some('c'), Layer("cultures")),
    layer("states", "States", Some('s'), Layer("states")),
    layer("provinces", "Provinces", Some('p'), Layer("provinces")),
    layer("zones", "Zones", Some('z'), Layer("zones")),
    layer("regions", "Pinned Areas", None, Layer("regions")),
    layer("borders", "Borders", Some('d'), Layer("borders")),
    layer("routes", "Routes", Some('u'), Layer("roads")),
    layer("features", "Overlay Items", None, Layer("features")),
    layer(
        "temperature",
        "Temperature",
        Some('t'),
        Layer("temperature"),
    ),
    layer("ice", "Ice", Some('j'), Layer("ice")),
    layer("goods", "Goods", Some('g'), Layer("goods")),
    layer("markets", "Markets", None, Layer("markets")),
    layer("trade", "Trade", Some('`'), Layer("trade")),
    layer(
        "precipitation",
        "Precipitation",
        Some('a'),
        Layer("precipitation"),
    ),
    layer("population", "Population", Some('n'), Layer("population")),
    layer("emblems", "Emblems", Some('y'), Layer("emblems")),
    layer("burgIcons", "Icons", Some('i'), Layer("places")),
    layer("labels", "Labels", Some('l'), Layer("labels")),
    layer("military", "Military", Some('m'), Layer("military")),
    layer("markers", "Markers", Some('k'), Layer("markers")),
    layer("journeys", "Journeys", None, Layer("journeys")),
    layer("rulers", "Rulers", Some('='), Layer("rulers")),
    layer("scaleBar", "Scale Bar", Some('/'), Extra("scale_bar")),
    layer("vignette", "Vignette", Some('['), Layer("vignette")),
];

/// The layers that FMG always draws. A layer preset does not hide them.
pub const PERMANENT: &[&str] = &["coastline"];

/// The layers that only inkland has. A layer preset does not change them.
pub const INKLAND_ONLY: &[&str] = &["regions", "features"];

/// The FMG layer presets: the id, the text, and the FMG layer ids.
pub const PRESETS: &[(&str, &str, &[&str])] = &[
    (
        "political",
        "Political map",
        &[
            "borders",
            "burgIcons",
            "ice",
            "labels",
            "lakes",
            "rivers",
            "routes",
            "scaleBar",
            "states",
            "vignette",
        ],
    ),
    (
        "cultural",
        "Cultural map",
        &[
            "borders",
            "burgIcons",
            "cultures",
            "labels",
            "lakes",
            "rivers",
            "routes",
            "scaleBar",
            "vignette",
        ],
    ),
    (
        "religions",
        "Religions map",
        &[
            "borders",
            "burgIcons",
            "labels",
            "lakes",
            "religions",
            "rivers",
            "routes",
            "scaleBar",
            "vignette",
        ],
    ),
    (
        "provinces",
        "Provinces map",
        &[
            "borders",
            "burgIcons",
            "labels",
            "lakes",
            "provinces",
            "rivers",
            "scaleBar",
            "vignette",
        ],
    ),
    (
        "biomes",
        "Biomes map",
        &["biomes", "ice", "lakes", "rivers", "scaleBar", "vignette"],
    ),
    (
        "heightmap",
        "Heightmap",
        &["heightmap", "lakes", "rivers", "vignette"],
    ),
    (
        "physical",
        "Physical map",
        &[
            "coordinates",
            "heightmap",
            "ice",
            "lakes",
            "rivers",
            "scaleBar",
            "vignette",
        ],
    ),
    (
        "poi",
        "Places of interest",
        &[
            "borders",
            "burgIcons",
            "heightmap",
            "ice",
            "lakes",
            "markers",
            "rivers",
            "routes",
            "scaleBar",
            "vignette",
        ],
    ),
    (
        "goods",
        "Goods and production",
        &[
            "borders", "goods", "ice", "lakes", "rivers", "scaleBar", "vignette",
        ],
    ),
    (
        "trade",
        "Trade map",
        &[
            "borders",
            "burgIcons",
            "ice",
            "lakes",
            "markets",
            "rivers",
            "routes",
            "scaleBar",
            "trade",
            "vignette",
        ],
    ),
    (
        "military",
        "Military map",
        &[
            "borders",
            "burgIcons",
            "labels",
            "lakes",
            "military",
            "rivers",
            "routes",
            "scaleBar",
            "states",
            "vignette",
        ],
    ),
    (
        "emblems",
        "Emblems",
        &[
            "borders",
            "burgIcons",
            "emblems",
            "ice",
            "lakes",
            "rivers",
            "routes",
            "scaleBar",
            "states",
            "vignette",
        ],
    ),
    ("landmass", "Pure landmass", &["scaleBar"]),
];

/// The text of the preset select when no preset matches the layers.
pub const CUSTOM_PRESET: &str = "Custom (not saved)";

/// The FMG style presets.
pub const STYLE_PRESETS: &[(&str, &str)] = &[
    ("default", "Default"),
    ("ancient", "Ancient"),
    ("gloom", "Gloom"),
    ("pale", "Pale"),
    ("light", "Light"),
    ("watercolor", "Watercolor"),
    ("clean", "Clean"),
    ("atlas", "Atlas"),
    ("darkSeas", "Dark Seas"),
    ("cyberpunk", "Cyberpunk"),
    ("night", "Night"),
    ("monochrome", "Monochrome"),
    ("cinderwood", "Cinderwood"),
    ("frostbite", "Frostbite"),
    ("ink", "Ink"),
];

/// The JSON Pointer of the style preset in the config. The render branch
/// adds this field.
pub const STYLE_PRESET_PATH: &str = "/render/style";

/// The render modes that the app offers, if the config accepts them.
pub const RENDER_MODES: &[(&str, &str)] = &[("print-safe", "Print-safe"), ("colour", "Colour")];

/// The heightmap templates, with the FMG names.
pub const TEMPLATES: &[(&str, &str)] = &[
    ("volcano", "Volcano"),
    ("high-island", "High Island"),
    ("low-island", "Low Island"),
    ("continents", "Continents"),
    ("archipelago", "Archipelago"),
    ("atoll", "Atoll"),
    ("mediterranean", "Mediterranean"),
    ("peninsula", "Peninsula"),
    ("pangea", "Pangea"),
    ("isthmus", "Isthmus"),
    ("shattered", "Shattered"),
    ("taklamakan", "Taklamakan"),
    ("old-world", "Old World"),
    ("fractious", "Fractious"),
];

/// The culture sets, with the FMG names.
pub fn culture_sets() -> Vec<(&'static str, String)> {
    inkland_core::culture_sets::set_names()
        .into_iter()
        .map(|id| (id, title_case(id)))
        .collect()
}

/// The kinds of point pins, in the order of the kind lists.
pub const PLACE_KINDS: &[&str] = &["village", "town", "city", "fortress", "ruin"];

/// The kinds of region pins.
pub const REGION_KINDS: &[&str] = &[
    "sea",
    "lake",
    "mountains",
    "hills",
    "plains",
    "marsh",
    "forest",
    "desert",
];

/// A button of the Tools tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolDef {
    /// The id that `open` takes: a dialog id, or `add-<thing>` for a tool
    /// that adds on the map.
    pub id: &'static str,
    pub label: &'static str,
    /// The group: `Edit`, `Add`, or `Show`.
    pub group: &'static str,
    /// The hotkey hint, for example `Shift+T`.
    pub hint: &'static str,
    /// `false`: inkland does not have the data for this FMG tool yet.
    pub enabled: bool,
}

const fn tool(
    id: &'static str,
    label: &'static str,
    group: &'static str,
    hint: &'static str,
    enabled: bool,
) -> ToolDef {
    ToolDef {
        id,
        label,
        group,
        hint,
        enabled,
    }
}

/// The buttons of the Tools tab, in the FMG order.
pub const TOOLS: &[ToolDef] = &[
    tool("biomes", "Biomes", "Edit", "Shift+B", false),
    tool("burgs", "Burgs", "Edit", "Shift+T", true),
    tool("cultures", "Cultures", "Edit", "Shift+C", true),
    tool("diplomacy", "Diplomacy", "Edit", "Shift+D", true),
    tool("economy", "Economy", "Edit", "", true),
    tool("emblems", "Emblems", "Edit", "Shift+Y", true),
    tool("goods", "Goods", "Edit", "Shift+G", true),
    tool("heightmap", "Heightmap", "Edit", "Shift+H", true),
    tool("labels", "Labels", "Edit", "Shift+L", true),
    tool("markers", "Markers", "Edit", "Shift+K", true),
    tool("markets", "Markets", "Edit", "", true),
    tool("military", "Military", "Edit", "Shift+M", true),
    tool("namesbase", "Namesbase", "Edit", "Shift+N", false),
    tool("notes", "Notes", "Edit", "Shift+O", true),
    tool("provinces", "Provinces", "Edit", "Shift+P", true),
    tool("regions", "Regions", "Edit", "Shift+Z", true),
    tool("religions", "Religions", "Edit", "Shift+R", true),
    tool("rivers", "Rivers", "Edit", "Shift+V", true),
    tool("routes", "Routes", "Edit", "Shift+U", true),
    tool("journeys", "Journeys", "Edit", "Shift+J", true),
    tool("stability", "Stability", "Edit", "", true),
    tool("states", "States", "Edit", "Shift+S", true),
    tool("units", "Units", "Edit", "Shift+Q", true),
    tool("zones", "Zones", "Edit", "", true),
    tool("variants", "Variants", "Edit", "Shift+W", true),
    tool("submap", "Submap", "Edit", "", true),
    tool("add-burg", "Burg", "Add", "Shift+1", true),
    tool("add-region", "Region", "Add", "", true),
    tool("add-river", "River", "Add", "Shift+4", true),
    tool("add-route", "Route", "Add", "Shift+5", true),
    tool("cells", "Cells", "Show", "Shift+E", true),
    tool("print", "Print", "Show", "", true),
];

/// `high-fantasy` as `High Fantasy`.
pub fn title_case(id: &str) -> String {
    id.split('-')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// `true` if `LayerName` accepts the name.
pub fn layer_known(name: &str) -> bool {
    serde_json::from_value::<LayerName>(json!(name)).is_ok()
}

/// The names of `render.layers`, as strings.
pub fn config_layers(c: &Config) -> Vec<String> {
    serde_json::to_value(&c.render.layers)
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// The layer button with the FMG id `id`.
pub fn layer_def(id: &str) -> Option<&'static LayerDef> {
    LAYERS.iter().find(|l| l.id == id)
}

/// The layer button of a hotkey.
pub fn layer_of_key(key: char) -> Option<&'static LayerDef> {
    LAYERS.iter().find(|l| l.key == Some(key))
}

/// `true` if the renderer can draw the layer.
pub fn available(def: &LayerDef) -> bool {
    match def.target {
        Target::Layer(name) => layer_known(name),
        Target::Extra(_) => true,
        Target::None => false,
    }
}

/// `true` if the config shows the layer.
pub fn is_on(c: &Config, def: &LayerDef) -> bool {
    match def.target {
        Target::Layer(name) => config_layers(c).iter().any(|l| l == name),
        Target::Extra("scale_bar") => c.render.extras.scale_bar == Some(true),
        Target::Extra("compass") => c.render.extras.compass == Some(true),
        Target::Extra("grid") => c.render.extras.grid.is_some(),
        _ => false,
    }
}

/// The place of an inkland layer name in the draw order: its place in
/// `LAYERS`.
pub fn order_of(name: &str) -> usize {
    LAYERS
        .iter()
        .position(|l| matches!(l.target, Target::Layer(n) if n == name))
        .unwrap_or(usize::MAX)
}

/// The FMG ids of the shown layers that a preset controls.
fn shown_ids(c: &Config) -> Vec<&'static str> {
    LAYERS
        .iter()
        .filter(|l| {
            available(l)
                && !PERMANENT.contains(&l.id)
                && !INKLAND_ONLY.contains(&l.id)
                && is_on(c, l)
        })
        .map(|l| l.id)
        .collect()
}

/// The FMG ids of a preset that the renderer can draw.
fn preset_ids(ids: &[&str]) -> Vec<&'static str> {
    LAYERS
        .iter()
        .filter(|l| ids.contains(&l.id) && available(l))
        .map(|l| l.id)
        .collect()
}

/// The preset that gives the shown layers, or `None` for a custom set.
pub fn current_preset(c: &Config) -> Option<&'static str> {
    let shown = shown_ids(c);
    PRESETS
        .iter()
        .find(|(_, _, ids)| preset_ids(ids) == shown)
        .map(|(id, _, _)| *id)
}

/// The `render` value of the config with the layers of a preset. The
/// permanent layers and the inkland layers do not change.
pub fn render_with_preset(c: &Config, preset: &str) -> Option<Value> {
    let (_, _, ids) = PRESETS.iter().find(|(id, _, _)| *id == preset)?;
    let mut render = serde_json::to_value(&c.render).ok()?;
    let on = |def: &LayerDef| {
        if PERMANENT.contains(&def.id) || INKLAND_ONLY.contains(&def.id) {
            is_on(c, def)
        } else {
            ids.contains(&def.id)
        }
    };
    let mut layers: Vec<String> = LAYERS
        .iter()
        .filter(|l| available(l) && on(l))
        .filter_map(|l| match l.target {
            Target::Layer(name) => Some(name.to_owned()),
            _ => None,
        })
        .collect();
    // Keep the layers that the catalog does not know, at the end.
    for name in config_layers(c) {
        if order_of(&name) == usize::MAX {
            layers.push(name);
        }
    }
    render["layers"] = json!(layers);
    let extras = render["extras"]
        .as_object_mut()
        .expect("render.extras is an object");
    for def in LAYERS.iter() {
        if let Target::Extra(field) = def.target {
            let want = on(def);
            match field {
                "grid" => {
                    if want && c.render.extras.grid.is_none() {
                        extras.insert("grid".into(), json!({ "step": grid_step(c) }));
                    } else if !want {
                        extras.remove("grid");
                    }
                }
                _ => {
                    extras.insert(field.into(), json!(want));
                }
            }
        }
    }
    Some(render)
}

/// The default grid step: a round number near a tenth of the frame.
pub fn grid_step(c: &Config) -> f64 {
    let raw = c.frame.width.max(c.frame.height) / 10.0;
    let p = libm::pow(10.0, libm::floor(libm::log10(raw.max(1e-9))));
    [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * p)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * p)
}

/// `true` if the config accepts the style preset `name` at
/// `STYLE_PRESET_PATH`.
pub fn style_available(c: &Config, name: &str) -> bool {
    accepts(c, STYLE_PRESET_PATH, json!(name))
}

/// `true` if the config accepts the render mode `name`.
pub fn mode_available(c: &Config, name: &str) -> bool {
    accepts(c, "/render/mode", json!(name))
}

/// `true` if the config stays valid JSON for its type when the value at
/// `path` is `value`.
fn accepts(c: &Config, path: &str, value: Value) -> bool {
    inkland::Command::Set {
        path: path.to_owned(),
        value,
    }
    .apply(c)
    .is_ok()
}

/// The value at a JSON Pointer of the config.
pub fn value_at(c: &Config, path: &str) -> Option<Value> {
    serde_json::to_value(c).ok()?.pointer(path).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(extra: Value) -> Config {
        let mut v = json!({ "version": 1, "seed": 1, "frame": { "width": 400, "height": 300 } });
        if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
            for (k, x) in e {
                o.insert(k.clone(), x.clone());
            }
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn each_layer_name_of_the_config_type_has_a_button() {
        // Each name that `LayerName` has today must have a button, so that
        // the Layers tab can show and hide it.
        let names = config_layers(&config(json!({})));
        assert!(!names.is_empty());
        for name in names {
            assert!(
                LAYERS
                    .iter()
                    .any(|l| matches!(l.target, Target::Layer(n) if n == name)),
                "{name} has no layer button"
            );
        }
    }

    #[test]
    fn a_layer_is_available_only_if_the_config_type_knows_it() {
        let relief = layer_def("relief").unwrap();
        assert!(available(relief));
        let rulers = layer_def("rulers").unwrap();
        assert!(available(rulers));
        // The scale bar is an extra, so it is always available.
        assert!(available(layer_def("scaleBar").unwrap()));
        // A made-up name is not known.
        assert!(!layer_known("no-such-layer"));
    }

    #[test]
    fn the_draw_order_of_the_catalog_agrees_with_the_default_order() {
        let names = config_layers(&config(json!({})));
        let order: Vec<usize> = names.iter().map(|n| order_of(n)).collect();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(order, sorted);
    }

    #[test]
    fn hotkeys_and_ids_are_unique() {
        let mut keys: Vec<char> = LAYERS.iter().filter_map(|l| l.key).collect();
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), n);
        let mut ids: Vec<&str> = TOOLS.iter().map(|t| t.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn a_preset_keeps_the_coast_and_the_inkland_layers() {
        let c = config(json!({ "render": { "layers": ["coast", "relief", "regions", "labels"] } }));
        let render = render_with_preset(&c, "political").unwrap();
        let layers: Vec<&str> = render["layers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(layers.contains(&"coast"));
        assert!(layers.contains(&"regions"), "an inkland layer stays on");
        assert!(!layers.contains(&"features"), "an inkland layer stays off");
        assert!(!layers.contains(&"relief"), "political has no relief");
        assert!(layers.contains(&"labels") && layers.contains(&"places"));
        assert_eq!(render["extras"]["scale_bar"], true);
        // The new render is valid, and the preset is now the current one.
        let mut v = serde_json::to_value(&c).unwrap();
        v["render"] = render;
        let next: Config = serde_json::from_value(v).unwrap();
        assert_eq!(current_preset(&next), Some("political"));
    }

    #[test]
    fn the_trade_preset_shows_the_markets_and_the_trade() {
        let c = config(json!({}));
        let render = render_with_preset(&c, "trade").unwrap();
        let layers: Vec<&str> = render["layers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(layers.contains(&"markets") && layers.contains(&"trade"));
        assert!(!layers.contains(&"goods") && !layers.contains(&"labels"));
        let render = render_with_preset(&c, "goods").unwrap();
        assert!(
            render["layers"]
                .as_array()
                .unwrap()
                .contains(&json!("goods"))
        );
    }

    #[test]
    fn the_landmass_preset_hides_the_labels() {
        let c = config(json!({}));
        let render = render_with_preset(&c, "landmass").unwrap();
        assert_eq!(render["layers"], json!(["coast", "regions", "features"]));
        assert_eq!(current_preset(&c), None);
    }

    #[test]
    fn the_grid_step_is_a_round_number() {
        assert_eq!(grid_step(&config(json!({}))), 50.0);
        let c = config(json!({ "frame": { "width": 37, "height": 12 } }));
        assert_eq!(grid_step(&c), 5.0);
    }

    #[test]
    fn modes_and_style_presets_come_from_the_config_type() {
        let c = config(json!({}));
        assert!(mode_available(&c, "print-safe"));
        assert!(!mode_available(&c, "sepia"));
        // Each template name of the catalog is a template of the config.
        for (id, _) in TEMPLATES {
            assert!(accepts(&c, "/world/template", json!(id)), "{id}");
        }
        let _ = style_available(&c, "default");
        assert_eq!(title_case("high-fantasy"), "High Fantasy");
        // Each culture set of the catalog is a culture set of the config.
        for (id, _) in culture_sets() {
            assert!(accepts(&c, "/world/culture_set", json!(id)), "{id}");
        }
    }
}
