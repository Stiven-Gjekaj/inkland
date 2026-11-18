//! Each test builds its own config and asks `validate` one question.

use inkland_constraints::validate;
use inkland_core::config::Config;
use inkland_core::error::ErrorCode;
use serde_json::{Value, json};

fn base() -> Value {
    json!({ "version": 1, "seed": 1, "frame": { "width": 400, "height": 300 } })
}

fn with(patch: Value) -> Config {
    let mut v = base();
    for (k, val) in patch.as_object().unwrap() {
        v[k] = val.clone();
    }
    serde_json::from_value(v).expect("the test config agrees with the schema")
}

fn codes(c: &Config) -> Vec<(ErrorCode, String)> {
    validate(c).into_iter().map(|e| (e.code, e.path)).collect()
}

fn one(c: &Config, code: ErrorCode, path: &str) {
    assert_eq!(codes(c), vec![(code, path.to_owned())]);
}

#[test]
fn minimal_config_is_valid() {
    assert_eq!(codes(&with(json!({}))), vec![]);
}

#[test]
fn bottom_left_origin_needs_north_y_axis() {
    one(
        &with(json!({ "frame": { "origin": "bottom-left", "width": 4, "height": 3 } })),
        ErrorCode::FrameAxesInvalid,
        "/frame/y_axis",
    );
}

#[test]
fn zero_frame_width_is_refused() {
    one(
        &with(json!({ "frame": { "width": 0, "height": 3 } })),
        ErrorCode::FrameAxesInvalid,
        "/frame/width",
    );
}

#[test]
fn cells_outside_the_range_are_refused() {
    one(
        &with(json!({ "world": { "cells": 10 } })),
        ErrorCode::ValueOutOfRange,
        "/world/cells",
    );
}

#[test]
fn id_with_capital_letters_is_refused() {
    one(
        &with(json!({ "pins": { "points": [{ "id": "Ostrey", "kind": "city", "at": [1, 1] }] } })),
        ErrorCode::IdInvalid,
        "/pins/points/0/id",
    );
}

#[test]
fn same_id_on_a_place_and_a_region_is_refused() {
    let c = with(json!({ "pins": {
        "points": [{ "id": "ostrey", "kind": "city", "at": [1, 1] }],
        "regions": [{ "id": "ostrey", "kind": "hills", "box": [100, 100, 120, 120] }]
    }}));
    one(&c, ErrorCode::IdDuplicate, "/pins/regions/0/id");
}

#[test]
fn region_with_two_shapes_is_refused() {
    let c = with(json!({ "pins": { "regions": [
        { "id": "r", "kind": "hills", "box": [0, 0, 10, 10], "circle": { "at": [5, 5], "r": 2 } }
    ]}}));
    one(&c, ErrorCode::ShapeInvalid, "/pins/regions/0");
}

#[test]
fn polygon_that_crosses_itself_is_refused() {
    let c = with(json!({ "pins": { "regions": [
        { "id": "r", "kind": "hills", "polygon": [[0, 0], [10, 10], [10, 0], [0, 10]] }
    ]}}));
    one(
        &c,
        ErrorCode::RegionPolygonInvalid,
        "/pins/regions/0/polygon",
    );
}

#[test]
fn font_outside_the_allowed_list_is_refused() {
    let c = with(json!({ "render": { "fonts": { "labels": "EB Garamond" } } }));
    one(&c, ErrorCode::FontNotAllowed, "/render/fonts/labels");
}

#[test]
fn road_to_an_unknown_place_is_refused() {
    let c = with(json!({ "pins": {
        "points": [{ "id": "a", "kind": "town", "at": [1, 1] }],
        "roads": [{ "id": "r", "from": "a", "to": "nowhere" }]
    }}));
    one(&c, ErrorCode::RefUnknown, "/pins/roads/0/to");
}

#[test]
fn variant_with_an_unknown_overlay_is_refused() {
    let c = with(json!({ "variants": [
        { "name": "v", "overlay": "then", "page": { "w_mm": 167, "h_mm": 250 } }
    ]}));
    one(&c, ErrorCode::RefUnknown, "/variants/0/overlay");
}

#[test]
fn overlay_remove_of_an_unknown_item_is_refused() {
    let c = with(json!({
        "pins": { "points": [{ "id": "ostrey", "kind": "ruin", "at": [1, 1] }] },
        "overlays": { "today": { "remove": ["place:ostrey", "place:harrowgate"] } }
    }));
    one(&c, ErrorCode::RefUnknown, "/overlays/today/remove/1");
}

#[test]
fn place_outside_the_frame_is_infeasible() {
    let c = with(json!({ "pins": { "points": [{ "id": "a", "kind": "town", "at": [401, 1] }] } }));
    one(&c, ErrorCode::PinOutsideFrame, "/pins/points/0/at");
}

#[test]
fn crop_outside_the_frame_is_infeasible() {
    let c = with(json!({ "variants": [
        { "name": "act-1", "crop": [80, 40, 500, 160], "page": { "w_mm": 167, "h_mm": 120 } }
    ]}));
    one(&c, ErrorCode::PinOutsideFrame, "/variants/0/crop");
}

#[test]
fn city_in_a_forced_sea_is_infeasible() {
    let c = with(json!({ "pins": {
        "points": [{ "id": "a", "kind": "city", "at": [10, 150] }],
        "regions": [{ "id": "western-sea", "kind": "sea", "box": [0, 0, 20, 300] }]
    }}));
    one(&c, ErrorCode::PinInForcedSea, "/pins/points/0/at");
}

#[test]
fn city_next_to_a_forced_sea_is_feasible() {
    let c = with(json!({ "pins": {
        "points": [{ "id": "a", "kind": "city", "at": [21, 150], "port": true }],
        "regions": [{ "id": "western-sea", "kind": "sea", "box": [0, 0, 20, 300] }]
    }}));
    assert_eq!(codes(&c), vec![]);
}

#[test]
fn overlapping_sea_and_mountains_conflict() {
    let c = with(json!({ "pins": { "regions": [
        { "id": "sea", "kind": "sea", "box": [0, 0, 50, 300] },
        { "id": "teeth", "kind": "mountains", "box": [40, 0, 100, 100] }
    ]}}));
    one(&c, ErrorCode::RegionConflict, "/pins/regions/1");
}

#[test]
fn overlapping_hills_and_plains_do_not_conflict() {
    let c = with(json!({ "pins": { "regions": [
        { "id": "wolds", "kind": "hills", "box": [0, 0, 50, 50] },
        { "id": "vale", "kind": "plains", "box": [40, 0, 100, 100] }
    ]}}));
    assert_eq!(codes(&c), vec![]);
}

#[test]
fn river_cannot_end_in_mountains() {
    let c = with(json!({ "pins": {
        "regions": [{ "id": "teeth", "kind": "mountains", "box": [0, 0, 50, 50] }],
        "rivers": [{ "id": "dunn", "source": [100, 100], "mouth": { "region": "teeth" } }]
    }}));
    let got = codes(&c);
    assert!(
        got.contains(&(
            ErrorCode::RiverMouthRegionKind,
            "/pins/rivers/0/mouth/region".into()
        )),
        "{got:?}"
    );
}

#[test]
fn river_mouth_at_a_place_needs_a_port() {
    let c = with(json!({ "pins": {
        "points": [{ "id": "corrantey", "kind": "city", "at": [30, 150] }],
        "rivers": [{ "id": "tey", "source": [200, 100], "mouth": { "place": "corrantey" } }]
    }}));
    one(
        &c,
        ErrorCode::RiverMouthOnLand,
        "/pins/rivers/0/mouth/place",
    );
}

#[test]
fn river_from_mountains_to_marsh_flows_downhill() {
    let c = with(json!({ "pins": {
        "points": [{ "id": "brannocks-ford", "kind": "village", "at": [150, 150] }],
        "regions": [
            { "id": "wolds", "kind": "hills", "box": [200, 100, 300, 200], "height": [35, 50] },
            { "id": "low-fen", "kind": "marsh", "box": [50, 100, 100, 200] }
        ],
        "rivers": [{ "id": "brann", "source": { "region": "wolds" },
                     "through": [{ "place": "brannocks-ford" }],
                     "mouth": { "region": "low-fen" } }]
    }}));
    assert_eq!(codes(&c), vec![]);
}

#[test]
fn river_from_marsh_up_into_mountains_is_infeasible() {
    let c = with(json!({ "pins": {
        "regions": [
            { "id": "low-fen", "kind": "marsh", "box": [50, 100, 100, 200] },
            { "id": "teeth", "kind": "mountains", "box": [200, 0, 300, 100] }
        ],
        "rivers": [{ "id": "wrong", "source": { "region": "low-fen" },
                     "through": [[250, 50]],
                     "mouth": [399, 299] }]
    }}));
    one(&c, ErrorCode::RiverNotDownhill, "/pins/rivers/0/source");
}

#[test]
fn road_target_shorter_than_the_straight_line_is_infeasible() {
    let c = with(json!({ "pins": {
        "points": [
            { "id": "a", "kind": "town", "at": [0, 0] },
            { "id": "b", "kind": "town", "at": [100, 0] }
        ],
        "roads": [{ "id": "r", "from": "a", "to": "b", "target_length": 90, "tolerance": 0.05 }]
    }}));
    one(
        &c,
        ErrorCode::RoadTargetTooShort,
        "/pins/roads/0/target_length",
    );
}

#[test]
fn road_target_inside_the_tolerance_is_feasible() {
    let c = with(json!({ "pins": {
        "points": [
            { "id": "a", "kind": "town", "at": [0, 0] },
            { "id": "b", "kind": "town", "at": [100, 0] }
        ],
        "roads": [{ "id": "r", "from": "a", "to": "b", "target_length": 96, "tolerance": 0.05 }]
    }}));
    assert_eq!(codes(&c), vec![]);
}

#[test]
fn via_points_make_the_straight_line_longer() {
    let c = with(json!({ "pins": {
        "points": [
            { "id": "a", "kind": "town", "at": [0, 0] },
            { "id": "b", "kind": "town", "at": [100, 0] }
        ],
        "roads": [{ "id": "r", "from": "a", "to": "b", "via": [[50, 100]], "target_length": 150 }]
    }}));
    one(
        &c,
        ErrorCode::RoadTargetTooShort,
        "/pins/roads/0/target_length",
    );
}

#[test]
fn errors_come_in_a_fixed_order() {
    let c = with(json!({
        "frame": { "width": 0, "height": 3 },
        "render": { "fonts": { "other": "Comic Sans" } },
        "pins": { "points": [{ "id": "B", "kind": "town", "at": [1, 1] }] }
    }));
    let first = codes(&c);
    assert_eq!(first, codes(&c));
    assert_eq!(first[0].0, ErrorCode::FrameAxesInvalid);
}

#[test]
fn custom_template_with_a_bad_step_is_refused() {
    let c = with(
        json!({ "world": { "template": { "steps": ["Hill 1 50 40-60 40-60", "Volcano 3"] } } }),
    );
    one(&c, ErrorCode::TemplateInvalid, "/world/template/steps");
}

#[test]
fn custom_template_with_good_steps_is_valid() {
    let c = with(
        json!({ "world": { "template": { "steps": ["Hill 1 50 40-60 40-60", "Smooth 2"] } } }),
    );
    assert_eq!(codes(&c), vec![]);
}

#[test]
fn per_cell_hint_that_gives_too_many_cells_is_refused() {
    // 400 x 300 with cells of 0.1 units is 12 million cells.
    let c = with(json!({ "units": { "per_cell_hint": 0.1 } }));
    one(&c, ErrorCode::ValueOutOfRange, "/units/per_cell_hint");
}

#[test]
fn two_places_at_the_same_position_are_infeasible() {
    let c = with(json!({ "pins": { "points": [
        { "id": "a", "kind": "town", "at": [10, 10] },
        { "id": "b", "kind": "town", "at": [10, 10] }
    ]}}));
    one(&c, ErrorCode::PinSamePosition, "/pins/points/1/at");
}

#[test]
fn height_range_without_a_whole_number_is_refused() {
    let c = with(json!({ "pins": { "regions": [
        { "id": "r", "kind": "hills", "box": [0, 0, 10, 10], "height": [40.2, 40.8] }
    ]}}));
    one(&c, ErrorCode::ValueOutOfRange, "/pins/regions/0/height");
}

#[test]
fn unknown_culture_set_is_refused() {
    one(
        &with(json!({ "world": { "culture_set": "martian" } })),
        ErrorCode::ValueOutOfRange,
        "/world/culture_set",
    );
}

#[test]
fn more_cultures_than_the_set_has_are_refused() {
    one(
        &with(json!({ "world": { "culture_set": "oriental", "cultures": 14 } })),
        ErrorCode::ValueOutOfRange,
        "/world/cultures",
    );
}

fn raster(cols: u32, rows: u32, values: Vec<u8>) -> Config {
    with(json!({ "world": { "template": { "raster": {
        "cols": cols, "rows": rows, "values": values
    } } } }))
}

#[test]
fn raster_template_with_the_right_size_is_valid() {
    assert_eq!(codes(&raster(3, 2, vec![0, 20, 40, 60, 80, 100])), vec![]);
}

#[test]
fn raster_template_with_the_wrong_number_of_values_is_refused() {
    one(
        &raster(3, 2, vec![0, 20, 40]),
        ErrorCode::TemplateInvalid,
        "/world/template/raster/values",
    );
}

#[test]
fn raster_template_with_a_height_above_100_is_refused() {
    one(
        &raster(2, 2, vec![0, 20, 101, 60]),
        ErrorCode::ValueOutOfRange,
        "/world/template/raster/values/2",
    );
}

#[test]
fn raster_template_with_one_column_is_refused() {
    one(
        &raster(1, 2, vec![0, 20]),
        ErrorCode::ValueOutOfRange,
        "/world/template/raster/cols",
    );
}

#[test]
fn a_style_override_with_a_valid_path_is_accepted() {
    let c = with(json!({ "render": { "mode": "colour", "style_overrides": {
        "ocean.groups.base.attrs.fill": "#112233",
        "relief.options.density": 0.6,
        "ocean.options.bands.render": true,
        "labels.groups.state.attrs.filter": null
    }}}));
    assert_eq!(codes(&c), vec![]);
}

#[test]
fn a_style_override_with_a_bad_path_is_refused() {
    for key in [
        "sea.groups.base.attrs.fill",
        "ocean.groups.base.fill",
        "ocean.groups",
        "ocean.attrs",
        "ocean..attrs.fill",
        "ocean.attrs.fill.x",
    ] {
        let c = with(json!({ "render": { "style_overrides": { key: "#000000" } } }));
        one(
            &c,
            ErrorCode::ValueOutOfRange,
            &format!("/render/style_overrides/{key}"),
        );
    }
}

#[test]
fn a_style_override_with_a_list_value_is_refused() {
    one(
        &with(json!({ "render": { "style_overrides": { "ocean.attrs.opacity": [1] } } })),
        ErrorCode::ValueOutOfRange,
        "/render/style_overrides/ocean.attrs.opacity",
    );
}

#[test]
fn each_layer_is_valid_in_each_mode() {
    for mode in ["print-safe", "colour"] {
        let c = with(
            json!({ "render": { "mode": mode, "layers": ["coast", "biomes", "lakes",
            "coastline", "borders", "routes", "burg-icons", "cells", "grid", "coordinates",
            "compass", "scale-bar", "ocean", "texture", "legend"] } }),
        );
        assert_eq!(codes(&c), vec![], "{mode}");
    }
}

#[test]
fn an_unknown_emblem_shape_is_refused() {
    one(
        &with(json!({ "world": { "emblem_shape": "kite2" } })),
        ErrorCode::ValueOutOfRange,
        "/world/emblem_shape",
    );
    assert_eq!(
        codes(&with(json!({ "world": { "emblem_shape": "kite" } }))),
        vec![]
    );
}

#[test]
fn valid_custom_arms_are_accepted() {
    let c = with(json!({ "pins": { "points": [
        { "id": "a", "kind": "city", "at": [10, 10], "capital_of": "Realm",
          "coa": { "t1": "vair-azure-argent", "charges": [{ "charge": "crossPatonce", "t": "gules", "p": "e" }] },
          "state_coa": { "t1": "or", "division": { "division": "perPale", "t": "sable", "line": "wavy" } } }
    ] } }));
    assert_eq!(codes(&c), vec![]);
}

#[test]
fn custom_arms_with_an_unknown_tincture_are_refused() {
    one(
        &with(json!({ "pins": { "points": [
            { "id": "a", "kind": "city", "at": [10, 10], "coa": { "t1": "blue" } }
        ] } })),
        ErrorCode::CoaInvalid,
        "/pins/points/0/coa/t1",
    );
}

#[test]
fn state_arms_need_a_capital() {
    one(
        &with(json!({ "pins": { "points": [
            { "id": "a", "kind": "city", "at": [10, 10], "state_coa": { "t1": "or" } }
        ] } })),
        ErrorCode::CoaInvalid,
        "/pins/points/0/state_coa",
    );
}

#[test]
fn a_charge_without_art_is_refused() {
    // The lion of FMG has a license that does not permit its use here.
    one(
        &with(json!({ "pins": { "points": [
            { "id": "a", "kind": "city", "at": [10, 10],
              "coa": { "t1": "or", "charges": [{ "charge": "lionRampant", "t": "gules", "p": "e" }] } }
        ] } })),
        ErrorCode::CoaInvalid,
        "/pins/points/0/coa/charges/0/charge",
    );
}

#[test]
fn tables_of_goods_units_and_transports_are_checked() {
    let c = with(json!({ "world": {
        "goods": [{ "name": "Wood", "distribution": "biome(5" }],
        "military_units": [{ "name": "pikemen" }],
        "transports": [{ "name": "Stay", "remove": true }]
    }}));
    assert_eq!(
        codes(&c),
        vec![
            (
                ErrorCode::TableInvalid,
                "/world/goods/0/distribution".to_owned()
            ),
            (
                ErrorCode::TableInvalid,
                "/world/military_units/0/kind".to_owned()
            ),
            (ErrorCode::TableInvalid, "/world/transports".to_owned()),
        ]
    );
    let good = with(json!({ "world": {
        "goods": [{ "name": "Wood", "value": 3 }, { "name": "Resin", "unit": "jar" }],
        "military_units": [{ "name": "fleet", "remove": true }],
        "transports": [{ "name": "Ox cart", "speed": 4, "domain": "land" }]
    }}));
    assert_eq!(codes(&good), vec![]);
}

#[test]
fn renames_need_one_reference_and_valid_arms() {
    let c = with(json!({
        "world": { "renames": [
            { "kind": "state", "name": "X" },
            { "kind": "religion", "at": [10, 10], "coa": { "t1": "or" } },
            { "kind": "province", "at": [10, 10], "coa": { "t1": "plaid" } }
        ] }
    }));
    assert_eq!(
        codes(&c),
        vec![
            (ErrorCode::RenameInvalid, "/world/renames/0".to_owned()),
            (ErrorCode::RenameInvalid, "/world/renames/1/coa".to_owned()),
            (ErrorCode::CoaInvalid, "/world/renames/2/coa/t1".to_owned()),
        ]
    );
    let good = with(json!({
        "pins": { "points": [{ "id": "home", "kind": "city", "at": [200, 150], "capital_of": "Realm" }] },
        "world": { "renames": [{ "kind": "state", "pin": "home", "name": "Ostrey" }] }
    }));
    assert_eq!(codes(&good), vec![]);
}
