//! Checks that need no terrain: the config rules and the pin feasibility
//! checks of `--dry-run`.
//!
//! The checks run in a fixed order, and each check gives its errors in config
//! order. Thus the list of errors is the same on each run.

use crate::geom::{Shape, dist, polygon_self_intersects};
use inkland_core::config::{
    Anchor, Config, Feature, Origin, Point, PointPin, RegionKind, RegionPin, RoadPin, Waypoint,
    YAxis,
};
use inkland_core::error::{Error, ErrorCode};
use std::collections::{BTreeMap, BTreeSet};

/// Run all checks. An empty list means that the config is valid and that no
/// check found an infeasible pin.
pub fn validate(config: &Config) -> Vec<Error> {
    let mut v = Validator {
        c: config,
        errors: Vec::new(),
    };
    v.frame();
    v.ranges();
    v.ids();
    v.shapes();
    v.fonts();
    v.references();
    v.inside_frame();
    v.places_on_land();
    v.same_positions();
    v.region_conflicts();
    v.river_mouths();
    v.rivers_downhill();
    v.road_targets();
    v.errors
}

/// The names of the checks that `validate` runs, in order. The dry-run report
/// lists them.
pub const CHECKS: [&str; 13] = [
    "frame",
    "ranges",
    "ids",
    "shapes",
    "fonts",
    "references",
    "pins_inside_frame",
    "places_on_land",
    "same_positions",
    "region_conflicts",
    "river_mouths",
    "rivers_downhill",
    "road_targets",
];

/// The style elements of Azgaar's Fantasy Map Generator: the first word of
/// each style path.
pub const STYLE_ELEMENTS: [&str; 39] = [
    "biomes",
    "borders",
    "burgIcons",
    "cells",
    "coastline",
    "compass",
    "coordinates",
    "cultures",
    "emblems",
    "fogging",
    "goods",
    "grid",
    "heightmap",
    "ice",
    "journeys",
    "labels",
    "lakes",
    "landmass",
    "legend",
    "map",
    "markers",
    "markets",
    "military",
    "ocean",
    "population",
    "precipitation",
    "provinces",
    "relief",
    "religions",
    "rivers",
    "routes",
    "rulers",
    "scaleBar",
    "states",
    "temperature",
    "texture",
    "trade",
    "vignette",
    "zones",
];

/// Check the form of a style path: an element, then `groups.<name>` zero
/// or more times, then `attrs` or `options`, then one or more keys.
fn style_path_is_valid(path: &str) -> Result<(), &'static str> {
    let words: Vec<&str> = path.split('.').collect();
    if words.iter().any(|w| w.is_empty()) {
        return Err("a word of the path is empty");
    }
    if !STYLE_ELEMENTS.contains(&words[0]) {
        return Err("the first word is not a style element");
    }
    let mut i = 1;
    while i < words.len() && words[i] == "groups" {
        if i + 1 >= words.len() {
            return Err("a group name is missing after \"groups\"");
        }
        i += 2;
    }
    if i >= words.len() || !matches!(words[i], "attrs" | "options") {
        return Err("the path does not name \"attrs\" or \"options\"");
    }
    if i + 1 >= words.len() {
        return Err("the path does not name a key after \"attrs\" or \"options\"");
    }
    if words[i] == "attrs" && i + 2 != words.len() {
        return Err("an attribute has no sub-keys");
    }
    Ok(())
}

const ID_PATTERN: &str = "^[a-z0-9]+(-[a-z0-9]+)*$";

fn id_is_valid(id: &str) -> bool {
    !id.is_empty()
        && id.split('-').all(|w| {
            !w.is_empty()
                && w.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

struct Validator<'a> {
    c: &'a Config,
    errors: Vec<Error>,
}

/// A place that a road or a river can name: a base pin or an overlay place.
struct PlaceEntry<'a> {
    pin: &'a PointPin,
}

impl<'a> Validator<'a> {
    fn err(
        &mut self,
        code: ErrorCode,
        path: impl Into<String>,
        msg: impl Into<String>,
    ) -> &mut Error {
        self.errors.push(Error::new(code, path, msg));
        self.errors.last_mut().expect("just pushed")
    }

    fn frame(&mut self) {
        let f = &self.c.frame;
        let agree = matches!(
            (f.origin, f.y_axis),
            (Origin::TopLeft, YAxis::South) | (Origin::BottomLeft, YAxis::North)
        );
        if !agree {
            self.err(
                ErrorCode::FrameAxesInvalid,
                "/frame/y_axis",
                "with origin top-left, y_axis must be south; with origin bottom-left, y_axis must be north",
            );
        }
        for (name, value) in [("width", f.width), ("height", f.height)] {
            if !(value.is_finite() && value > 0.0) {
                self.err(
                    ErrorCode::FrameAxesInvalid,
                    format!("/frame/{name}"),
                    format!("the frame {name} must be more than 0, found {value}"),
                );
            }
        }
    }

    /// A raster has 2 or more columns and rows, `cols` x `rows` values, and
    /// each value is 100 or less.
    fn raster(&mut self, r: &inkland_core::config::Raster) {
        let path = "/world/template/raster";
        for (name, n) in [("cols", r.cols), ("rows", r.rows)] {
            if n < 2 {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{path}/{name}"),
                    format!("the raster {name} must be 2 or more, found {n}"),
                );
            }
        }
        let size = u64::from(r.cols) * u64::from(r.rows);
        if r.values.len() as u64 != size {
            self.err(
                ErrorCode::TemplateInvalid,
                format!("{path}/values"),
                format!(
                    "the raster has {} x {} = {size} rectangles, but {} values",
                    r.cols,
                    r.rows,
                    r.values.len()
                ),
            );
        }
        if let Some(i) = r.values.iter().position(|&v| v > 100) {
            self.err(
                ErrorCode::ValueOutOfRange,
                format!("{path}/values/{i}"),
                format!("a raster height must be 0 to 100, found {}", r.values[i]),
            );
        }
    }

    fn ranges(&mut self) {
        self.render_style();
        let c = self.c;
        let w = &c.world;
        // The tables of the config: goods, military units, and transports.
        let tables = [
            inkland_core::economy::goods::catalogue_from(&w.goods).err(),
            inkland_core::military::units_from(&w.military_units).err(),
            inkland_core::journeys::transports::transports_from(&w.transports).err(),
        ];
        self.errors.extend(tables.into_iter().flatten());
        for (i, m) in w.measurers.iter().enumerate() {
            let least = if m.kind == inkland_core::annotations::MeasurerKind::Planimeter {
                3
            } else {
                2
            };
            if m.points.len() < least {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("/world/measurers/{i}/points"),
                    format!(
                        "a {} needs {least} points or more, found {}",
                        m.kind.name(),
                        m.points.len()
                    ),
                );
            }
        }
        let goods = inkland_core::economy::goods::catalogue_from(&w.goods).unwrap_or_default();
        for (i, f) in w.finance.iter().enumerate() {
            self.errors
                .extend(inkland_core::economy::finance::check(c, &goods, i, f));
        }
        for (i, m) in w.markets.iter().enumerate() {
            if m.remove && m.to.is_some() {
                self.err(
                    ErrorCode::MarketInvalid,
                    format!("/world/markets/{i}"),
                    "a market entry cannot have both remove and to",
                );
            }
        }
        for (i, z) in w.zones.iter().enumerate() {
            self.errors.extend(inkland_core::zones::check(i, z));
        }
        for (i, e) in w.renames.iter().enumerate() {
            self.errors.extend(inkland_core::renames::check(c, i, e));
            for f in e.coa.iter().flat_map(inkland_core::emblems::validate) {
                self.err(
                    ErrorCode::CoaInvalid,
                    format!("/world/renames/{i}/coa{}", f.path),
                    f.message,
                );
            }
        }
        for (i, b) in w.brushes.iter().enumerate() {
            if !(b.radius > 0.0 && b.radius.is_finite()) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("/world/brushes/{i}/radius"),
                    format!("a brush radius must be more than 0, found {}", b.radius),
                );
            }
            if !(-100.0..=100.0).contains(&b.delta) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("/world/brushes/{i}/delta"),
                    format!("a brush delta must be -100 to 100, found {}", b.delta),
                );
            }
        }
        if !(1000..=100_000).contains(&w.cells) {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/cells",
                format!("cells must be 1000 to 100000, found {}", w.cells),
            );
        }
        if !(0.0..=0.9).contains(&w.jitter) {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/jitter",
                format!("jitter must be 0 to 0.9, found {}", w.jitter),
            );
        }
        if w.lake_elevation_limit > inkland_core::lakes::NO_LAKES_LIMIT {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/lake_elevation_limit",
                format!(
                    "lake_elevation_limit must be 0 to 80, found {}",
                    w.lake_elevation_limit
                ),
            );
        }
        if !(0.0..=1.0).contains(&w.border_terrain) {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/border_terrain",
                "border_terrain must be 0 to 1",
            );
        }
        if !(0.0..=1.0).contains(&w.border_noise) {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/border_noise",
                "border_noise must be 0 to 1",
            );
        }
        if !(1..=99).contains(&w.sea_level) {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/sea_level",
                format!("sea_level must be 1 to 99, found {}", w.sea_level),
            );
        }
        if let inkland_core::config::Template::Steps { steps } = &w.template {
            if let Err(e) = inkland_core::heightmap::parse_steps(steps.iter().map(String::as_str)) {
                self.err(ErrorCode::TemplateInvalid, "/world/template/steps", e);
            } else if steps.iter().all(|s| s.trim().is_empty()) {
                self.err(
                    ErrorCode::TemplateInvalid,
                    "/world/template/steps",
                    "a custom template needs one or more steps",
                );
            }
        }
        if let Some(raster) = w.template.raster() {
            self.raster(raster);
        }
        if let inkland_core::config::Template::Image { heights: None, .. } = &w.template {
            self.err(
                ErrorCode::TemplateInvalid,
                "/world/template/image",
                "the image of the template is not read; the program that reads the config file reads it",
            );
        }
        let shape = w.emblem_shape.as_str();
        if !["culture", "state", "random"].contains(&shape)
            && !inkland_core::emblems::is_shield(shape)
        {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/emblem_shape",
                format!(
                    "there is no shield shape {shape:?}; use culture, state, random, or a shield: {}",
                    inkland_core::emblems::shield_names().collect::<Vec<_>>().join(", ")
                ),
            );
        }
        match inkland_core::culture_sets::culture_set(&w.culture_set) {
            None => {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    "/world/culture_set",
                    format!(
                        "there is no culture set {:?}; use one of {:?}",
                        w.culture_set,
                        inkland_core::culture_sets::set_names()
                    ),
                );
            }
            Some(set) if w.cultures == 0 || w.cultures as usize > set.max => {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    "/world/cultures",
                    format!(
                        "the culture set {:?} has {} cultures; cultures must be 1 to {}",
                        w.culture_set, set.max, set.max
                    ),
                );
            }
            Some(_) => {}
        }
        if !(0.0..=10.0).contains(&w.size_variety) {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/size_variety",
                "size_variety must be 0 to 10",
            );
        }
        if w.provinces_ratio > 100 {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/provinces_ratio",
                "provinces_ratio must be 0 to 100",
            );
        }
        if !(0.1..=2.0).contains(&w.growth_rate) {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/growth_rate",
                "growth_rate must be 0.1 to 2",
            );
        }
        let [south, north] = w.climate.latitude_range;
        if !(-90.0..=90.0).contains(&south) || !(-90.0..=90.0).contains(&north) || south >= north {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/climate/latitude_range",
                "latitude_range must be [south, north], each from -90 to 90, with south less than north",
            );
        }
        if let Some(g) = &w.climate.geography {
            let checks = [
                ("map_size", g.map_size, 1.0, 100.0),
                ("latitude", g.latitude, 0.0, 100.0),
                ("longitude", g.longitude, 0.0, 100.0),
            ];
            for (name, value, lo, hi) in checks {
                if let Some(v) = value
                    && !(lo..=hi).contains(&v)
                {
                    self.err(
                        ErrorCode::ValueOutOfRange,
                        format!("/world/climate/geography/{name}"),
                        format!("{name} must be {lo} to {hi}, found {v}"),
                    );
                }
            }
        }
        if let Some(h) = c.units.per_cell_hint {
            if !(h.is_finite() && h > 0.0) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    "/units/per_cell_hint",
                    "per_cell_hint must be more than 0",
                );
            } else {
                let cells = inkland_core::world::cell_count(c);
                if !(1000.0..=100_000.0).contains(&cells) {
                    self.err(
                        ErrorCode::ValueOutOfRange,
                        "/units/per_cell_hint",
                        format!("per_cell_hint {h} gives {cells} cells; the cell count must be 1000 to 100000"),
                    );
                }
            }
        }
        if !(c.render.min_line_width_pt.is_finite() && c.render.min_line_width_pt > 0.0) {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/render/min_line_width_pt",
                "min_line_width_pt must be more than 0",
            );
        }
        if !(0.0..=1.0).contains(&c.render.relief.density) {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/render/relief/density",
                "relief density must be 0 to 1",
            );
        }
        if c.world
            .edges
            .width
            .is_some_and(|w| !(w.is_finite() && w > 0.0))
        {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/world/edges/width",
                "the edge width must be more than 0",
            );
        }
        for (i, r) in c.pins.regions.iter().enumerate() {
            let p = format!("/pins/regions/{i}");
            if let Some([lo, hi]) = r.height
                && !(0.0 <= lo && lo <= hi && hi <= 100.0)
            {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{p}/height"),
                    "height must be [min, max] with 0 <= min <= max <= 100",
                );
            } else if let Some([lo, hi]) = r.height
                && lo.ceil() > hi.floor()
            {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{p}/height"),
                    "heights are whole numbers, so the height range must contain a whole number",
                );
            }
            if let Some([lo, hi]) = r.height {
                let water = r.kind.is_water();
                if water && hi >= f64::from(c.world.sea_level) {
                    self.err(
                        ErrorCode::ValueOutOfRange,
                        format!("{p}/height"),
                        "a water region must have a height range below sea level",
                    );
                }
                if !water && lo < f64::from(c.world.sea_level) {
                    self.err(
                        ErrorCode::ValueOutOfRange,
                        format!("{p}/height"),
                        "a land region must have a height range at or above sea level",
                    );
                }
            }
            if !(r.edge.is_finite() && r.edge >= 0.0) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{p}/edge"),
                    "edge must be 0 or more",
                );
            }
            if r.roughness.is_some_and(|v| !(v.is_finite() && v >= 0.0)) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{p}/roughness"),
                    "roughness must be 0 or more",
                );
            }
            if r.wander.is_some_and(|v| !(v.is_finite() && v >= 0.0)) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{p}/wander"),
                    "wander must be 0 or more",
                );
            }
            if let Some(i) = r.islets {
                if !(0.0..=1.0).contains(&i) {
                    self.err(
                        ErrorCode::ValueOutOfRange,
                        format!("{p}/islets"),
                        "islets must be 0 to 1",
                    );
                } else if r.kind.is_water() {
                    self.err(
                        ErrorCode::ValueOutOfRange,
                        format!("{p}/islets"),
                        "islets does not apply to regions of kind sea or lake",
                    );
                }
            }
            if r.coast_detail.is_some_and(|d| !(1..=8).contains(&d)) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{p}/coast_detail"),
                    "coast_detail must be 1 to 8",
                );
            }
            if let Some(w) = r.shelf {
                if !(w.is_finite() && w > 0.0) {
                    self.err(
                        ErrorCode::ValueOutOfRange,
                        format!("{p}/shelf"),
                        "shelf must be more than 0",
                    );
                } else if !r.kind.is_water() {
                    self.err(
                        ErrorCode::ValueOutOfRange,
                        format!("{p}/shelf"),
                        "shelf applies only to regions of kind sea or lake",
                    );
                }
            }
            if r.landlocked && !r.kind.is_water() {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{p}/landlocked"),
                    "landlocked applies only to regions of kind sea or lake",
                );
            }
        }
        for (i, pin) in c.pins.points.iter().enumerate() {
            self.place_fields(pin, &format!("/pins/points/{i}"));
        }
        for (name, o) in &c.overlays {
            for (i, pin) in o.add.places.iter().enumerate() {
                self.place_fields(pin, &format!("/overlays/{}/add/places/{i}", esc(name)));
            }
        }
        for (path, road) in self.all_roads() {
            if let Some(t) = road.target_length
                && !(t.is_finite() && t > 0.0)
            {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{path}/target_length"),
                    "target_length must be more than 0",
                );
            }
            if !(0.0..=1.0).contains(&road.tolerance) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{path}/tolerance"),
                    "tolerance must be 0 to 1",
                );
            }
        }
        for (i, v) in c.variants.iter().enumerate() {
            let p = &v.page;
            if !(p.w_mm > 0.0 && p.h_mm > 0.0 && p.margin_mm >= 0.0)
                || 2.0 * p.margin_mm >= p.w_mm.min(p.h_mm)
            {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("/variants/{i}/page"),
                    "the page must have a size more than 0 and a margin less than half of the smaller side",
                );
            }
            if let Some([x0, y0, x1, y1]) = v.crop
                && !(x0 < x1 && y0 < y1)
            {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("/variants/{i}/crop"),
                    "crop must be [x0, y0, x1, y1] with x0 < x1 and y0 < y1",
                );
            }
        }
    }

    fn place_fields(&mut self, pin: &PointPin, path: &str) {
        use inkland_core::config::PlaceKind;
        if pin.kind == PlaceKind::Custom && pin.custom_kind.is_none() {
            self.err(
                ErrorCode::ValueOutOfRange,
                format!("{path}/custom_kind"),
                "a place of kind custom needs custom_kind",
            );
        }
        if pin.state_coa.is_some() && pin.capital_of.is_none() {
            self.err(
                ErrorCode::CoaInvalid,
                format!("{path}/state_coa"),
                "state_coa needs capital_of: only a capital gives arms to a state",
            );
        }
        if let Some(x) = pin.expansionism {
            if pin.capital_of.is_none() {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{path}/expansionism"),
                    "expansionism needs capital_of: only a state grows",
                );
            } else if !(0.1..=10.0).contains(&x) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{path}/expansionism"),
                    "expansionism must be 0.1 to 10",
                );
            }
        }
        if let Some(poly) = &pin.claim {
            let message = if pin.capital_of.is_none() {
                Some("claim needs capital_of: only a state claims land")
            } else if poly.len() < 3 {
                Some("a polygon needs 3 or more points")
            } else if polygon_self_intersects(poly) {
                Some("the polygon crosses itself")
            } else {
                None
            };
            if let Some(m) = message {
                let code = if pin.capital_of.is_none() {
                    ErrorCode::ValueOutOfRange
                } else {
                    ErrorCode::RegionPolygonInvalid
                };
                self.err(code, format!("{path}/claim"), m);
            }
        }
        for (key, coa) in [("coa", &pin.coa), ("state_coa", &pin.state_coa)] {
            for f in coa.iter().flat_map(inkland_core::emblems::validate) {
                self.err(
                    ErrorCode::CoaInvalid,
                    format!("{path}/{key}{}", f.path),
                    f.message,
                );
            }
        }
    }

    /// Each road with its JSON Pointer: base pins first, then overlays in
    /// name order.
    fn all_roads(&self) -> Vec<(String, &'a RoadPin)> {
        let c = self.c;
        let mut out: Vec<(String, &RoadPin)> = c
            .pins
            .roads
            .iter()
            .enumerate()
            .map(|(i, r)| (format!("/pins/roads/{i}"), r))
            .collect();
        for (name, o) in &c.overlays {
            for (i, r) in o.add.roads.iter().enumerate() {
                out.push((format!("/overlays/{}/add/roads/{i}", esc(name)), r));
            }
        }
        out
    }

    fn ids(&mut self) {
        let c = self.c;
        // Base pins share one name space.
        let mut base: Vec<(String, &str)> = Vec::new();
        for (i, p) in c.pins.points.iter().enumerate() {
            base.push((format!("/pins/points/{i}/id"), &p.id));
        }
        for (i, r) in c.pins.regions.iter().enumerate() {
            base.push((format!("/pins/regions/{i}/id"), &r.id));
        }
        for (i, r) in c.pins.rivers.iter().enumerate() {
            base.push((format!("/pins/rivers/{i}/id"), &r.id));
        }
        for (i, r) in c.pins.roads.iter().enumerate() {
            base.push((format!("/pins/roads/{i}/id"), &r.id));
        }
        for (i, a) in c.pins.areas.iter().enumerate() {
            base.push((format!("/pins/areas/{i}/id"), &a.id));
        }
        self.unique(&base);
        // Each overlay adds to the base name space, but not to the space of
        // another overlay.
        for (name, o) in &c.overlays {
            let mut ids = base.clone();
            let n = esc(name);
            for (i, p) in o.add.places.iter().enumerate() {
                ids.push((format!("/overlays/{n}/add/places/{i}/id"), &p.id));
            }
            for (i, f) in o.add.features.iter().enumerate() {
                ids.push((format!("/overlays/{n}/add/features/{i}/id"), &f.id));
            }
            for (i, r) in o.add.roads.iter().enumerate() {
                ids.push((format!("/overlays/{n}/add/roads/{i}/id"), &r.id));
            }
            let extra = ids.split_off(base.len());
            let mut seen: BTreeMap<&str, String> =
                base.iter().map(|(p, id)| (*id, p.clone())).collect();
            for (path, id) in extra {
                self.id_pattern(&path, id);
                if let Some(first) = seen.get(id) {
                    self.err(
                        ErrorCode::IdDuplicate,
                        path.clone(),
                        format!("the id {id:?} is used more than once"),
                    )
                    .details
                    .insert("first".into(), first.clone().into());
                } else {
                    seen.insert(id, path);
                }
            }
        }
        let names: Vec<(String, &str)> = c
            .variants
            .iter()
            .enumerate()
            .map(|(i, v)| (format!("/variants/{i}/name"), v.name.as_str()))
            .collect();
        self.unique(&names);
        for name in c.overlays.keys() {
            self.id_pattern(&format!("/overlays/{}", esc(name)), name);
        }
    }

    fn unique(&mut self, ids: &[(String, &str)]) {
        let mut seen: BTreeMap<&str, &String> = BTreeMap::new();
        for (path, id) in ids {
            self.id_pattern(path, id);
            if let Some(first) = seen.get(id) {
                let first = (*first).clone();
                self.err(
                    ErrorCode::IdDuplicate,
                    path.clone(),
                    format!("the id {id:?} is used more than once"),
                )
                .details
                .insert("first".into(), first.into());
            } else {
                seen.insert(id, path);
            }
        }
    }

    fn id_pattern(&mut self, path: &str, id: &str) {
        if !id_is_valid(id) {
            self.err(
                ErrorCode::IdInvalid,
                path,
                format!("the id {id:?} does not match {ID_PATTERN}"),
            );
        }
    }

    fn shapes(&mut self) {
        let c = self.c;
        for (i, r) in c.pins.regions.iter().enumerate() {
            self.region_shape(r, &format!("/pins/regions/{i}"));
        }
        for (i, a) in c.pins.areas.iter().enumerate() {
            let path = format!("/pins/areas/{i}");
            self.shape_fields(&a.polygon, &a.rect, &a.circle, "an area", &path);
            if a.strength.is_some_and(|s| !(0.0..=1.0).contains(&s)) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{path}/strength"),
                    "strength must be 0 to 1",
                );
            }
            if a.edge.is_some_and(|e| !(e.is_finite() && e >= 0.0)) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("{path}/edge"),
                    "edge must be 0 or more",
                );
            }
        }
        for (name, o) in &c.overlays {
            for (i, f) in o.add.features.iter().enumerate() {
                self.feature_shape(f, &format!("/overlays/{}/add/features/{i}", esc(name)));
            }
        }
    }

    fn region_shape(&mut self, r: &RegionPin, path: &str) {
        self.shape_fields(&r.polygon, &r.rect, &r.circle, "a region", path);
    }

    /// The shape fields of a region or an area (`what`).
    fn shape_fields(
        &mut self,
        polygon: &Option<Vec<inkland_core::config::Point>>,
        rect: &Option<[f64; 4]>,
        circle: &Option<inkland_core::config::Circle>,
        what: &str,
        path: &str,
    ) {
        let count = usize::from(polygon.is_some())
            + usize::from(rect.is_some())
            + usize::from(circle.is_some());
        if count != 1 {
            self.err(
                ErrorCode::ShapeInvalid,
                path,
                format!("{what} needs exactly one of polygon, box, and circle; found {count}"),
            );
            return;
        }
        if let Some(poly) = polygon {
            if poly.len() < 3 {
                self.err(
                    ErrorCode::RegionPolygonInvalid,
                    format!("{path}/polygon"),
                    "a polygon needs 3 or more points",
                );
            } else if polygon_self_intersects(poly) {
                self.err(
                    ErrorCode::RegionPolygonInvalid,
                    format!("{path}/polygon"),
                    "the polygon crosses itself",
                );
            }
        }
        if let Some([x0, y0, x1, y1]) = *rect
            && !(x0 < x1 && y0 < y1)
        {
            self.err(
                ErrorCode::ShapeInvalid,
                format!("{path}/box"),
                "box must be [x0, y0, x1, y1] with x0 < x1 and y0 < y1",
            );
        }
        if let Some(circle) = circle
            && !(circle.r.is_finite() && circle.r > 0.0)
        {
            self.err(
                ErrorCode::ShapeInvalid,
                format!("{path}/circle/r"),
                "r must be more than 0",
            );
        }
    }

    fn feature_shape(&mut self, f: &Feature, path: &str) {
        let count = usize::from(f.at.is_some())
            + usize::from(f.polyline.is_some())
            + usize::from(f.polygon.is_some());
        if count != 1 {
            self.err(
                ErrorCode::ShapeInvalid,
                path,
                format!("a feature needs exactly one of at, polyline, and polygon; found {count}"),
            );
        }
        if let Some(line) = &f.polyline
            && line.len() < 2
        {
            self.err(
                ErrorCode::ShapeInvalid,
                format!("{path}/polyline"),
                "a polyline needs 2 or more points",
            );
        }
        if let Some(poly) = &f.polygon {
            if poly.len() < 3 {
                self.err(
                    ErrorCode::RegionPolygonInvalid,
                    format!("{path}/polygon"),
                    "a polygon needs 3 or more points",
                );
            } else if polygon_self_intersects(poly) {
                self.err(
                    ErrorCode::RegionPolygonInvalid,
                    format!("{path}/polygon"),
                    "the polygon crosses itself",
                );
            }
        }
    }

    /// The style overrides.
    fn render_style(&mut self) {
        let r = &self.c.render;
        if !(0.0..=2.0).contains(&r.river_contrast) {
            self.err(
                ErrorCode::ValueOutOfRange,
                "/render/river_contrast",
                "river_contrast must be 0 to 2",
            );
        }
        for (i, d) in r.depth_lines.iter().enumerate() {
            if !(d.is_finite() && *d > 0.0 && *d <= 100.0) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    format!("/render/depth_lines/{i}"),
                    "a depth must be more than 0 and at most 100",
                );
            }
        }
        for (key, value) in &r.style_overrides {
            let pointer = format!(
                "/render/style_overrides/{}",
                key.replace('~', "~0").replace('/', "~1")
            );
            if let Err(why) = style_path_is_valid(key) {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    pointer.clone(),
                    format!("{key:?} is not a style path: {why}"),
                );
            }
            if value.is_array() || value.is_object() {
                self.err(
                    ErrorCode::ValueOutOfRange,
                    pointer,
                    "a style value must be a string, a number, a boolean, or null",
                );
            }
        }
    }

    fn fonts(&mut self) {
        let r = &self.c.render;
        let allowed: BTreeSet<&str> = r.fonts_allowed.iter().map(String::as_str).collect();
        for (field, family) in r.fonts.families() {
            if !allowed.contains(family) {
                self.err(
                    ErrorCode::FontNotAllowed,
                    format!("/render/fonts/{field}"),
                    format!("the font family {family:?} is not in render.fonts_allowed"),
                )
                .details
                .insert("allowed".into(), r.fonts_allowed.clone().into());
            }
        }
    }

    fn base_places(&self) -> BTreeMap<&'a str, PlaceEntry<'a>> {
        self.c
            .pins
            .points
            .iter()
            .map(|p| (p.id.as_str(), PlaceEntry { pin: p }))
            .collect()
    }

    fn regions(&self) -> BTreeMap<&'a str, &'a RegionPin> {
        self.c
            .pins
            .regions
            .iter()
            .map(|r| (r.id.as_str(), r))
            .collect()
    }

    fn references(&mut self) {
        let c = self.c;
        let places = self.base_places();
        let regions = self.regions();
        for (i, river) in c.pins.rivers.iter().enumerate() {
            let p = format!("/pins/rivers/{i}");
            self.anchor_ref(&river.source, &format!("{p}/source"), &places, &regions);
            for (j, a) in river.through.iter().enumerate() {
                self.anchor_ref(a, &format!("{p}/through/{j}"), &places, &regions);
            }
            self.anchor_ref(&river.mouth, &format!("{p}/mouth"), &places, &regions);
        }
        for (i, road) in c.pins.roads.iter().enumerate() {
            self.road_refs(road, &format!("/pins/roads/{i}"), &places);
        }
        let rivers: BTreeSet<&str> = c.pins.rivers.iter().map(|r| r.id.as_str()).collect();
        let base_roads: BTreeSet<&str> = c.pins.roads.iter().map(|r| r.id.as_str()).collect();
        let capitals: BTreeSet<&str> = c
            .pins
            .points
            .iter()
            .filter(|p| p.capital_of.is_some())
            .map(|p| p.id.as_str())
            .collect();
        for (name, o) in &c.overlays {
            let n = esc(name);
            let mut all_places = self.base_places();
            for p in &o.add.places {
                all_places.insert(p.id.as_str(), PlaceEntry { pin: p });
            }
            for (i, road) in o.add.roads.iter().enumerate() {
                self.road_refs(road, &format!("/overlays/{n}/add/roads/{i}"), &all_places);
            }
            let features: BTreeSet<&str> = o.add.features.iter().map(|f| f.id.as_str()).collect();
            let roads: BTreeSet<&str> = base_roads
                .iter()
                .copied()
                .chain(o.add.roads.iter().map(|r| r.id.as_str()))
                .collect();
            let exists = |kind: &str, id: &str| match kind {
                "place" => all_places.contains_key(id),
                "region" => regions.contains_key(id),
                "river" => rivers.contains(id),
                "road" => roads.contains(id),
                "feature" => features.contains(id),
                _ => false,
            };
            // A label key can also name a state by the id of its capital
            // pin: a point pin with `capital_of`.
            let label_exists = |kind: &str, id: &str| match kind {
                "state" => capitals.contains(id),
                _ => exists(kind, id),
            };
            let mut bad: Vec<(String, String, bool)> = Vec::new();
            for (j, item) in o.remove.iter().enumerate() {
                if !item_ref_ok(item, &exists) {
                    bad.push((format!("/overlays/{n}/remove/{j}"), item.clone(), false));
                }
            }
            for key in o.rename.keys() {
                if !item_ref_ok(key, &exists) {
                    bad.push((
                        format!("/overlays/{n}/rename/{}", esc(key)),
                        key.clone(),
                        false,
                    ));
                }
            }
            for (key, opts) in &o.labels {
                if opts.angle.is_some_and(|a| a != 0 && a != 90) {
                    self.err(
                        ErrorCode::ValueOutOfRange,
                        format!("/overlays/{n}/labels/{}/angle", esc(key)),
                        "angle must be 0 or 90",
                    );
                }
            }
            for key in o.labels.keys() {
                if !item_ref_ok(key, &label_exists) {
                    bad.push((
                        format!("/overlays/{n}/labels/{}", esc(key)),
                        key.clone(),
                        true,
                    ));
                }
            }
            for (path, item, label) in bad {
                let kinds = if label {
                    "place, region, river, road, feature, or state (the id of its capital pin)"
                } else {
                    "place, region, river, road, or feature"
                };
                self.err(
                    ErrorCode::RefUnknown,
                    path,
                    format!(
                        "{item:?} does not name an item; use \"<kind>:<id>\" with kind {kinds}"
                    ),
                );
            }
        }
        // The regions and areas whose relief symbols a map leaves out.
        let regions: Vec<&str> = c
            .pins
            .regions
            .iter()
            .map(|r| r.id.as_str())
            .chain(c.pins.areas.iter().map(|a| a.id.as_str()))
            .collect();
        let reliefs = std::iter::once(("/render/relief".to_owned(), &c.render.relief)).chain(
            c.variants.iter().enumerate().filter_map(|(i, v)| {
                v.relief
                    .as_ref()
                    .map(|r| (format!("/variants/{i}/relief"), r))
            }),
        );
        for (path, relief) in reliefs {
            for (k, id) in relief.hide_in.iter().enumerate() {
                if !regions.contains(&id.as_str()) {
                    self.err(
                        ErrorCode::RefUnknown,
                        format!("{path}/hide_in/{k}"),
                        format!("{id:?} is not the id of a region pin or an area pin"),
                    );
                }
            }
        }
        for (i, v) in c.variants.iter().enumerate() {
            if let Some(o) = &v.overlay
                && !c.overlays.contains_key(o)
            {
                self.err(
                    ErrorCode::RefUnknown,
                    format!("/variants/{i}/overlay"),
                    format!("there is no overlay {o:?}"),
                );
            }
        }
    }

    fn anchor_ref(
        &mut self,
        a: &Anchor,
        path: &str,
        places: &BTreeMap<&str, PlaceEntry>,
        regions: &BTreeMap<&str, &RegionPin>,
    ) {
        match a {
            Anchor::Point(_) => {}
            Anchor::Place(r) if !places.contains_key(r.place.as_str()) => {
                self.err(
                    ErrorCode::RefUnknown,
                    format!("{path}/place"),
                    format!("there is no point pin {:?}", r.place),
                );
            }
            Anchor::Region(r) if !regions.contains_key(r.region.as_str()) => {
                self.err(
                    ErrorCode::RefUnknown,
                    format!("{path}/region"),
                    format!("there is no region pin {:?}", r.region),
                );
            }
            _ => {}
        }
    }

    fn road_refs(&mut self, road: &RoadPin, path: &str, places: &BTreeMap<&str, PlaceEntry>) {
        for (field, id) in [("from", &road.from), ("to", &road.to)] {
            if !places.contains_key(id.as_str()) {
                self.err(
                    ErrorCode::RefUnknown,
                    format!("{path}/{field}"),
                    format!("there is no place {id:?}"),
                );
            }
        }
        for (j, w) in road.via.iter().enumerate() {
            if let Waypoint::Place(id) = w
                && !places.contains_key(id.as_str())
            {
                self.err(
                    ErrorCode::RefUnknown,
                    format!("{path}/via/{j}"),
                    format!("there is no place {id:?}"),
                );
            }
        }
    }

    fn in_frame(&self, p: Point) -> bool {
        let f = &self.c.frame;
        p[0] >= 0.0 && p[0] <= f.width && p[1] >= 0.0 && p[1] <= f.height
    }

    fn outside(&mut self, path: String, what: &str, p: Point) {
        let f = &self.c.frame;
        self.err(
            ErrorCode::PinOutsideFrame,
            path,
            format!(
                "{what} [{}, {}] is outside the frame [0, 0, {}, {}]",
                p[0], p[1], f.width, f.height
            ),
        );
    }

    fn inside_frame(&mut self) {
        let c = self.c;
        let mut points: Vec<(String, &str, Point)> = Vec::new();
        for (i, p) in c.pins.points.iter().enumerate() {
            points.push((format!("/pins/points/{i}/at"), "the place", p.at));
        }
        for (i, r) in c.pins.regions.iter().enumerate() {
            if let Some(shape) = Shape::of(r) {
                for q in shape.extreme_points() {
                    if !self.in_frame(q) {
                        points.push((format!("/pins/regions/{i}"), "a point of the region", q));
                        break;
                    }
                }
            }
        }
        for (i, river) in c.pins.rivers.iter().enumerate() {
            let p = format!("/pins/rivers/{i}");
            let anchors = std::iter::once((format!("{p}/source"), &river.source))
                .chain(
                    river
                        .through
                        .iter()
                        .enumerate()
                        .map(|(j, a)| (format!("{p}/through/{j}"), a)),
                )
                .chain(std::iter::once((format!("{p}/mouth"), &river.mouth)));
            for (path, a) in anchors {
                if let Anchor::Point(q) = a {
                    points.push((path, "the river point", *q));
                }
            }
        }
        for (path, road) in self.all_roads() {
            for (j, w) in road.via.iter().enumerate() {
                if let Waypoint::Point(q) = w {
                    points.push((format!("{path}/via/{j}"), "the road point", *q));
                }
            }
        }
        for (name, o) in &c.overlays {
            let n = esc(name);
            for (i, p) in o.add.places.iter().enumerate() {
                points.push((
                    format!("/overlays/{n}/add/places/{i}/at"),
                    "the place",
                    p.at,
                ));
            }
            for (i, f) in o.add.features.iter().enumerate() {
                let all =
                    f.at.iter()
                        .chain(f.polyline.iter().flatten())
                        .chain(f.polygon.iter().flatten());
                if let Some(q) = all.copied().find(|&q| !self.in_frame(q)) {
                    points.push((
                        format!("/overlays/{n}/add/features/{i}"),
                        "a point of the feature",
                        q,
                    ));
                }
            }
        }
        for (i, v) in c.variants.iter().enumerate() {
            if let Some([x0, y0, x1, y1]) = v.crop {
                for q in [[x0, y0], [x1, y1]] {
                    if !self.in_frame(q) {
                        points.push((format!("/variants/{i}/crop"), "a corner of the crop", q));
                        break;
                    }
                }
            }
        }
        for (path, what, p) in points {
            if !self.in_frame(p) {
                self.outside(path, what, p);
            }
        }
    }

    /// Each region with a shape, in config order.
    fn shaped_regions(&self) -> Vec<(usize, &'a RegionPin, Shape)> {
        self.c
            .pins
            .regions
            .iter()
            .enumerate()
            .filter_map(|(i, r)| Shape::of(r).map(|s| (i, r, s)))
            .collect()
    }

    fn places_on_land(&mut self) {
        let c = self.c;
        let shaped = self.shaped_regions();
        let mut places: Vec<(String, &PointPin)> = c
            .pins
            .points
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.island)
            .map(|(i, p)| (format!("/pins/points/{i}/at"), p))
            .collect();
        for (name, o) in &c.overlays {
            for (i, p) in o.add.places.iter().enumerate() {
                places.push((format!("/overlays/{}/add/places/{i}/at", esc(name)), p));
            }
        }
        for (path, pin) in places {
            // Of the regions that hold the place, the ones with the highest
            // priority rule it. A land region with a higher priority than
            // the sea around it can hold a place.
            let holding: Vec<&RegionPin> = shaped
                .iter()
                .filter(|(_, _, shape)| shape.contains(pin.at))
                .map(|(_, r, _)| *r)
                .collect();
            let top = holding.iter().map(|r| r.priority).max();
            for region in holding {
                if region.kind.is_water() && Some(region.priority) == top {
                    self.err(
                        ErrorCode::PinInForcedSea,
                        path.clone(),
                        format!(
                            "the place {:?} is inside the {:?} region {:?}",
                            pin.id,
                            kind_name(region.kind),
                            region.id
                        ),
                    )
                    .details
                    .insert("region".into(), region.id.clone().into());
                }
            }
        }
    }

    fn same_positions(&mut self) {
        let mut seen: BTreeMap<(u64, u64), usize> = BTreeMap::new();
        for (i, p) in self.c.pins.points.iter().enumerate() {
            let key = (p.at[0].to_bits(), p.at[1].to_bits());
            if let Some(&first) = seen.get(&key) {
                self.err(
                    ErrorCode::PinSamePosition,
                    format!("/pins/points/{i}/at"),
                    format!(
                        "the places {:?} and {:?} have the same position",
                        self.c.pins.points[first].id, p.id
                    ),
                )
                .details
                .insert("other".into(), format!("/pins/points/{first}").into());
            } else {
                seen.insert(key, i);
            }
        }
    }

    fn region_conflicts(&mut self) {
        let regions = self.shaped_regions();
        for (a_i, (i, a, sa)) in regions.iter().enumerate() {
            for (j, b, sb) in regions.iter().skip(a_i + 1) {
                let [alo, ahi] = a.height_range();
                let [blo, bhi] = b.height_range();
                // A region with a higher priority rules where they overlap.
                let disjoint = (ahi < blo || bhi < alo) && a.priority == b.priority;
                if disjoint && sa.overlaps(sb) {
                    self.err(
                        ErrorCode::RegionConflict,
                        format!("/pins/regions/{j}"),
                        format!(
                            "the regions {:?} ({}, height {alo} to {ahi}) and {:?} ({}, height {blo} to {bhi}) overlap, but their height ranges have no common value",
                            a.id,
                            kind_name(a.kind),
                            b.id,
                            kind_name(b.kind)
                        ),
                    )
                    .details
                    .insert("other".into(), format!("/pins/regions/{i}").into());
                }
            }
        }
    }

    fn river_mouths(&mut self) {
        let c = self.c;
        let places = self.base_places();
        let regions = self.regions();
        let land: Vec<(usize, &RegionPin, Shape)> = self
            .shaped_regions()
            .into_iter()
            .filter(|(_, r, _)| !r.kind.is_water())
            .collect();
        for (i, river) in c.pins.rivers.iter().enumerate() {
            let path = format!("/pins/rivers/{i}/mouth");
            match &river.mouth {
                Anchor::Region(r) => {
                    if let Some(region) = regions.get(r.region.as_str())
                        && !matches!(
                            region.kind,
                            RegionKind::Marsh | RegionKind::Lake | RegionKind::Sea
                        )
                    {
                        self.err(
                            ErrorCode::RiverMouthRegionKind,
                            format!("{path}/region"),
                            format!(
                                "a river can end only in a marsh, lake, or sea region; {:?} is {}",
                                region.id,
                                kind_name(region.kind)
                            ),
                        );
                    }
                }
                Anchor::Place(r) => {
                    if let Some(entry) = places.get(r.place.as_str())
                        && !entry.pin.port
                    {
                        self.err(
                            ErrorCode::RiverMouthOnLand,
                            format!("{path}/place"),
                            format!("the place {:?} is the mouth of a river, so it must have port: true", entry.pin.id),
                        );
                    }
                }
                Anchor::Point(p) => {
                    if let Some((_, region, _)) = land.iter().find(|(_, _, s)| s.contains(*p)) {
                        self.err(
                            ErrorCode::RiverMouthOnLand,
                            path,
                            format!(
                                "the river mouth is inside the {} region {:?}, which is land",
                                kind_name(region.kind),
                                region.id
                            ),
                        );
                    }
                }
            }
        }
    }

    /// A river needs heights that fall strictly from the source to the mouth.
    /// Each anchor in a region must have a height in the range of the region.
    /// The check walks from the mouth to the source and keeps the lowest
    /// height that is possible at each anchor.
    fn rivers_downhill(&mut self) {
        let c = self.c;
        let places = self.base_places();
        let regions = self.regions();
        let shaped = self.shaped_regions();
        let sea_level = f64::from(c.world.sea_level);
        // The range at a point: of the regions that hold it, the ones with
        // the highest priority set it, as in generation. Each end keeps the
        // name of what sets it.
        let range_at = |p: Point| -> Span {
            let inside: Vec<&RegionPin> = shaped
                .iter()
                .filter(|(_, _, s)| s.contains(p))
                .map(|(_, r, _)| *r)
                .collect();
            let top = inside.iter().map(|r| r.priority).max();
            let mut span = Span::open();
            for r in inside.into_iter().filter(|r| Some(r.priority) == top) {
                span.limit(r.height_range(), &format!("the region {:?}", r.id));
            }
            span
        };
        for (i, river) in c.pins.rivers.iter().enumerate() {
            let anchors: Vec<&Anchor> = std::iter::once(&river.source)
                .chain(river.through.iter())
                .chain(std::iter::once(&river.mouth))
                .collect();
            let mut spans: Vec<Option<Span>> = Vec::new();
            for a in &anchors {
                spans.push(match a {
                    Anchor::Point(p) => Some(range_at(*p)),
                    Anchor::Place(r) => places.get(r.place.as_str()).map(|e| range_at(e.pin.at)),
                    Anchor::Region(r) => regions.get(r.region.as_str()).map(|reg| {
                        let mut span = Span::open();
                        span.limit(reg.height_range(), &format!("the region {:?}", reg.id));
                        span
                    }),
                });
            }
            // An unknown reference is a REF_UNKNOWN error already.
            if spans.iter().any(Option::is_none) {
                continue;
            }
            let spans: Vec<Span> = spans.into_iter().flatten().collect();
            let ranges: Vec<[f64; 2]> = spans.iter().map(|s| [s.lo, s.hi]).collect();
            // The mouth of a river into water is at most at sea level. A
            // marsh mouth keeps the range of the marsh.
            let last = ranges.len() - 1;
            let mut lowest = match anchors[last] {
                Anchor::Region(r)
                    if regions
                        .get(r.region.as_str())
                        .is_some_and(|g| g.kind == RegionKind::Marsh) =>
                {
                    ranges[last][0]
                }
                _ => ranges[last][0].min(sea_level),
            };
            for k in (0..last).rev() {
                let [lo, hi] = ranges[k];
                // A height must be more than the next one. The step is small,
                // so that any range with room above `lowest` is possible.
                let need = lowest.max(lo);
                let fits = if need > lowest {
                    need <= hi
                } else {
                    lowest < hi
                };
                if !fits {
                    let path = if k == 0 {
                        format!("/pins/rivers/{i}/source")
                    } else {
                        format!("/pins/rivers/{i}/through/{}", k - 1)
                    };
                    self.err(
                        ErrorCode::RiverNotDownhill,
                        path,
                        format!(
                            "the river {:?} cannot flow downhill here: {}",
                            river.id,
                            spans[k].why(lowest)
                        ),
                    );
                    break;
                }
                lowest = if need > lowest { need } else { lowest };
            }
        }
    }

    fn road_targets(&mut self) {
        let c = self.c;
        let mut checks: Vec<(String, &RoadPin, BTreeMap<&str, Point>)> = Vec::new();
        let base: BTreeMap<&str, Point> = c
            .pins
            .points
            .iter()
            .map(|p| (p.id.as_str(), p.at))
            .collect();
        for (i, r) in c.pins.roads.iter().enumerate() {
            checks.push((format!("/pins/roads/{i}"), r, base.clone()));
        }
        for (name, o) in &c.overlays {
            let mut all = base.clone();
            for p in &o.add.places {
                all.insert(p.id.as_str(), p.at);
            }
            for (i, r) in o.add.roads.iter().enumerate() {
                checks.push((
                    format!("/overlays/{}/add/roads/{i}", esc(name)),
                    r,
                    all.clone(),
                ));
            }
        }
        for (path, road, at) in checks {
            let Some(target) = road.target_length else {
                continue;
            };
            let mut points: Vec<Point> = Vec::new();
            let mut known = true;
            let ends = std::iter::once(Waypoint::Place(road.from.clone()))
                .chain(road.via.iter().cloned())
                .chain(std::iter::once(Waypoint::Place(road.to.clone())));
            for w in ends {
                match w {
                    Waypoint::Point(p) => points.push(p),
                    Waypoint::Place(id) => match at.get(id.as_str()) {
                        Some(p) => points.push(*p),
                        None => known = false,
                    },
                }
            }
            if !known {
                continue;
            }
            let straight: f64 = points.windows(2).map(|w| dist(w[0], w[1])).sum();
            let longest_allowed = target * (1.0 + road.tolerance);
            if straight > longest_allowed {
                self.err(
                    ErrorCode::RoadTargetTooShort,
                    format!("{path}/target_length"),
                    format!(
                        "the road {:?} is at least {} {} long (straight lines through its points), but the target with tolerance allows at most {}",
                        road.id,
                        round3(straight),
                        c.units.name,
                        round3(longest_allowed)
                    ),
                )
                .details
                .insert("straight_length".into(), round3(straight).into());
            }
        }
    }
}

fn item_ref_ok(item: &str, exists: &dyn Fn(&str, &str) -> bool) -> bool {
    match item.split_once(':') {
        Some((kind, id)) => exists(kind, id),
        None => false,
    }
}

fn kind_name(k: RegionKind) -> &'static str {
    match k {
        RegionKind::Sea => "sea",
        RegionKind::Lake => "lake",
        RegionKind::Mountains => "mountains",
        RegionKind::Hills => "hills",
        RegionKind::Plains => "plains",
        RegionKind::Marsh => "marsh",
        RegionKind::Forest => "forest",
        RegionKind::Desert => "desert",
        RegionKind::Land => "land",
    }
}

/// Escape a key for a JSON Pointer.
fn esc(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// A height range, with the name of what sets each end.
struct Span {
    lo: f64,
    lo_by: String,
    hi: f64,
    hi_by: String,
}

impl Span {
    /// The whole height scale.
    fn open() -> Self {
        Span {
            lo: 0.0,
            lo_by: "the bottom of the height scale".to_owned(),
            hi: 100.0,
            hi_by: "the top of the height scale".to_owned(),
        }
    }

    /// Narrow the range to `[lo, hi]`, which `by` sets.
    fn limit(&mut self, [lo, hi]: [f64; 2], by: &str) {
        if lo > self.lo {
            self.lo = lo;
            self.lo_by = by.to_owned();
        }
        if hi < self.hi {
            self.hi = hi;
            self.hi_by = by.to_owned();
        }
    }

    /// Why a point with this range cannot be above `lowest`, the height of
    /// the next point downstream.
    fn why(&self, lowest: f64) -> String {
        if self.lo > self.hi {
            format!(
                "no height fits this point: {} gives at least {}, and {} gives at most {}",
                self.lo_by, self.lo, self.hi_by, self.hi
            )
        } else {
            format!(
                "this point can be at most {} ({}), but the next point downstream needs a height above {}",
                self.hi, self.hi_by, lowest
            )
        }
    }
}
