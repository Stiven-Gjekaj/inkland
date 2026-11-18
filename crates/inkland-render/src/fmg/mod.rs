//! The `colour` mode: the map drawing of Azgaar's Fantasy Map Generator.
//!
//! The layers, their order, their SVG groups, and their styles follow the
//! renderers of the original program (`src/renderers/draw-*.ts`). The
//! style presets are data (see `style`). All drawing is in map pixels inside
//! one group that scales the pixels to millimetres (see `geom::Px`).
//!
//! Element ids are lower-case words joined by `-`, and the writer puts all
//! elements in a fixed order, so the output is the same on each run.

pub mod color;
pub mod data;
pub mod geom;
pub mod icons;
mod icons_data;
pub mod layers;
pub mod style;

use crate::labels::Labels;
use crate::page::Page;
use geom::{Outline, Px, n};
use inkland_core::config::{CoastlineDetail, Config, LayerName, Variant};
use inkland_core::world::World;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use style::Style;

/// The layers of the original program, in its drawing order
/// (`renderers/layers.ts`). The overlay features of inkland go after the
/// place icons.
pub const ORDER: [LayerName; 32] = [
    LayerName::Ocean,
    LayerName::Landmass,
    LayerName::Texture,
    LayerName::Heightmap,
    LayerName::Lakes,
    LayerName::Biomes,
    LayerName::Cells,
    LayerName::Grid,
    LayerName::Coordinates,
    LayerName::Compass,
    LayerName::Rivers,
    LayerName::Relief,
    LayerName::Religions,
    LayerName::Cultures,
    LayerName::States,
    LayerName::Provinces,
    LayerName::Zones,
    LayerName::Borders,
    LayerName::Routes,
    LayerName::Temperature,
    LayerName::Coastline,
    LayerName::Ice,
    LayerName::Precipitation,
    LayerName::Population,
    LayerName::Emblems,
    LayerName::BurgIcons,
    LayerName::Features,
    LayerName::Labels,
    LayerName::Markers,
    LayerName::ScaleBar,
    LayerName::Vignette,
    LayerName::Legend,
];

/// The layers that a config layer name stands for in `colour` mode. The
/// names of `print-safe` mode stand for the layers that the original program
/// draws for the same thing.
pub fn expand(layer: LayerName) -> Vec<LayerName> {
    use LayerName::*;
    match layer {
        Coast => vec![Ocean, Landmass, Lakes, Coastline],
        Regions => vec![States, Borders],
        Roads => vec![Routes],
        Places => vec![BurgIcons],
        other => vec![other],
    }
}

/// The layers to draw in `colour` mode, in the drawing order.
pub fn layers_of(config: &Config) -> Vec<LayerName> {
    let wanted: BTreeSet<LayerName> = config
        .render
        .layers
        .iter()
        .flat_map(|&l| expand(l))
        .collect();
    ORDER.into_iter().filter(|l| wanted.contains(l)).collect()
}

/// All that a layer needs to draw itself.
pub struct Ctx<'a> {
    pub world: &'a World,
    pub config: &'a Config,
    pub variant: &'a Variant,
    pub px: Px,
    pub style: Style,
    pub outlines: Vec<Outline>,
    pub labels: &'a Labels,
    /// The layers that the map has.
    pub layers: Vec<LayerName>,
    /// The colours of states, provinces, cultures, and religions.
    pub palette: layers::Palette,
    /// Definitions that layers add, by id. The defs get each one that the
    /// body refers to.
    pub extra_defs: std::cell::RefCell<std::collections::BTreeMap<String, String>>,
}

impl<'a> Ctx<'a> {
    pub fn new(
        world: &'a World,
        config: &'a Config,
        variant: &'a Variant,
        page: &Page,
        labels: &'a Labels,
    ) -> Ctx<'a> {
        let px = Px::new(page, config.frame.width, config.frame.height);
        let detail = config.render.coastline.unwrap_or(CoastlineDetail::Fractal);
        let settings = geom::CoastSettings {
            enabled: detail == CoastlineDetail::Fractal,
            ..geom::CoastSettings::DEFAULT
        };
        Ctx {
            world,
            config,
            variant,
            px,
            style: Style::with_overrides(config.render.style, &config.render.style_overrides),
            outlines: geom::outlines(world, px.unit_px, &settings),
            labels,
            layers: layers_of(config),
            palette: layers::Palette::of(world),
            extra_defs: std::cell::RefCell::new(std::collections::BTreeMap::new()),
        }
    }

    /// Add a definition with an id.
    pub fn def(&self, id: &str, markup: String) {
        self.extra_defs.borrow_mut().insert(id.to_owned(), markup);
    }

    /// `true` if the map draws a layer.
    pub fn has(&self, layer: LayerName) -> bool {
        self.layers.contains(&layer)
    }

    /// The attributes of a style node as SVG text: ` name="value"` for each
    /// attribute with a value. `skip` names attributes to leave out.
    pub fn attrs(&self, path: &[&str], skip: &[&str]) -> String {
        let mut out = String::new();
        for (k, v) in self.style.attrs(path) {
            if skip.contains(&k.as_str()) {
                continue;
            }
            let v = fix_refs(&v);
            let _ = write!(out, " {k}=\"{}\"", crate::extras::xml(&v));
        }
        out
    }

    /// The map area in pixels with a margin of `m` pixels: `x y width
    /// height` attributes of a rect.
    pub fn map_rect_attrs(&self, m: f64) -> String {
        let [x, y, w, h] = self.px.map_rect();
        format!(
            "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
            n(x - m),
            n(y - m),
            n(w + 2.0 * m),
            n(h + 2.0 * m)
        )
    }

    /// A world point in page pixels.
    pub fn p(&self, w: [f64; 2]) -> [f64; 2] {
        self.px.p(w)
    }
}

/// Change the ids in `url(#...)` references to lower-case words, as the
/// writer names its definitions: `url(#dropShadow05)` gives
/// `url(#drop-shadow05)`.
pub fn fix_refs(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(at) = rest.find("url(#") {
        out.push_str(&rest[..at + 5]);
        rest = &rest[at + 5..];
        let end = rest.find(')').unwrap_or(rest.len());
        out.push_str(&icons::kebab(&rest[..end]));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The style group of a label group of inkland. The original program has
/// no label groups for lakes, oceans, and islands: lakes take the group of
/// rivers, oceans the group of custom labels (`added`), and islands the
/// group of provinces.
pub fn style_group(group: &str) -> &str {
    match group {
        "lake" => "river",
        "island" => "province",
        "region" | "feature" | "ocean" => "added",
        other => other,
    }
}

/// The font size of a label group of a style, in mm. The sizes of the
/// original program are parts of the map (a percentage of 100 pixels). A
/// page shows the map smaller than a screen, so inkland keeps the ratios of
/// the style and gives the capitals the size of the cities of the config,
/// as the place icons do (see `layers::marks::burg_em`). A label group of a
/// style can be much smaller than the capitals, and a smaller text is hard
/// to read on paper, so no size is less than `labels::MIN_LABEL_PT`.
pub fn label_size(style: &Style, config: &Config, group: &str) -> f64 {
    let pct = |g: &str| {
        style
            .attr_f64(&["labels", g], "font-size")
            .filter(|v| *v > 0.0)
    };
    let capital = pct("capital").unwrap_or(6.0);
    let own = pct(group).unwrap_or(capital);
    let city = config.render.font_sizes_pt.place_city;
    crate::page::pt((city * own / capital).max(crate::labels::MIN_LABEL_PT))
}

/// Place the labels of a variant with the fonts, the letter case, the halo,
/// and the font size of the label groups of the style.
pub fn place_labels(world: &World, config: &Config, variant: &Variant, page: &Page) -> Labels {
    let style = Style::with_overrides(config.render.style, &config.render.style_overrides);
    let look = |group: &'static str| {
        let path = ["labels", style_group(group)];
        let css = style.attr(&path, "style").unwrap_or_default();
        crate::labels::Look {
            size: Some(label_size(&style, config, style_group(group))),
            family: style
                .attr(&path, "font-family")
                .unwrap_or_else(|| config.render.fonts.labels.clone()),
            upper: css.contains("uppercase"),
            halo: layers::text::halo(&style, style_group(group)),
        }
    };
    crate::labels::place_with(world, config, variant, page, &look)
}

/// Remove the earlier of two attributes with the same name in each start
/// tag. A layer writes its fixed attributes first and the attributes of the
/// style after them, so the style wins, as in the original program.
pub fn dedupe_attrs(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        rest = &rest[lt..];
        let Some(gt) = rest.find('>') else {
            break;
        };
        let tag = &rest[..=gt];
        out.push_str(&dedupe_tag(tag));
        rest = &rest[gt + 1..];
    }
    out.push_str(rest);
    out
}

fn dedupe_tag(tag: &str) -> String {
    let body = tag.trim_start_matches('<');
    if !body.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return tag.to_owned();
    }
    let end_slash = tag.ends_with("/>");
    let inner = &body[..body.len() - if end_slash { 2 } else { 1 }];
    let name_end = inner.find(char::is_whitespace).unwrap_or(inner.len());
    let (name, mut attrs) = (&inner[..name_end], &inner[name_end..]);
    let mut list: Vec<(&str, &str)> = Vec::new();
    while let Some(eq) = attrs.find("=\"") {
        let key = attrs[..eq].trim();
        let after = &attrs[eq + 2..];
        let Some(close) = after.find('"') else {
            return tag.to_owned();
        };
        list.push((key, &after[..close]));
        attrs = &after[close + 1..];
    }
    let mut seen = BTreeSet::new();
    if list.iter().all(|(k, _)| seen.insert(*k)) {
        return tag.to_owned();
    }
    let mut kept: Vec<(&str, &str)> = Vec::new();
    for (k, v) in list {
        kept.retain(|(x, _)| *x != k);
        kept.push((k, v));
    }
    let mut t = format!("<{name}");
    for (k, v) in kept {
        let _ = write!(t, " {k}=\"{v}\"");
    }
    t.push_str(if end_slash { "/>" } else { ">" });
    t
}

/// Write the SVG of one variant in `colour` mode.
pub fn render(
    world: &World,
    config: &Config,
    variant: &Variant,
    page: &Page,
    labels: &Labels,
) -> String {
    let ctx = Ctx::new(world, config, variant, page, labels);
    let mut body = String::new();
    for layer in &ctx.layers {
        layers::write(&mut body, &ctx, *layer);
    }
    layers::write_extras(&mut body, &ctx);
    let body = dedupe_attrs(&body);

    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">",
        w = inkland_core::num::fixed(page.w_mm, 3),
        h = inkland_core::num::fixed(page.h_mm, 3)
    );
    s.push_str(&layers::defs(&ctx, &body));
    let _ = writeln!(
        s,
        "<rect id=\"page-paper\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>",
        inkland_core::num::fixed(page.w_mm, 3),
        inkland_core::num::fixed(page.h_mm, 3)
    );
    let _ = writeln!(
        s,
        "<g id=\"viewbox\" transform=\"scale({})\" clip-path=\"url(#clip-map-px)\">",
        inkland_core::num::fixed(ctx.px.mm, 6)
    );
    s.push_str(&body);
    s.push_str("</g>\n");
    s.push_str(&layers::page_extras(&ctx));
    s.push_str("</svg>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_get_lower_case_ids() {
        assert_eq!(fix_refs("url(#dropShadow05)"), "url(#drop-shadow05)");
        assert_eq!(fix_refs("url(#land)"), "url(#land)");
        assert_eq!(fix_refs("blur(3.5px)"), "blur(3.5px)");
    }

    #[test]
    fn the_style_wins_over_a_fixed_attribute() {
        assert_eq!(
            dedupe_attrs(
                "<g id=\"a\" mask=\"url(#land)\" fill=\"x\" mask=\"url(#water)\"><path d=\"M0 0\"/></g>"
            ),
            "<g id=\"a\" fill=\"x\" mask=\"url(#water)\"><path d=\"M0 0\"/></g>"
        );
        assert_eq!(dedupe_attrs("<g a=\"1\" b=\"2\"/>"), "<g a=\"1\" b=\"2\"/>");
    }

    #[test]
    fn old_layer_names_stand_for_fmg_layers() {
        use LayerName::*;
        assert_eq!(expand(Coast), vec![Ocean, Landmass, Lakes, Coastline]);
        assert_eq!(expand(Regions), vec![States, Borders]);
        let all: BTreeSet<LayerName> = LayerName::ALL.iter().flat_map(|&l| expand(l)).collect();
        // Each layer that a name stands for has a place in the order.
        for l in all {
            assert!(ORDER.contains(&l), "{l:?}");
        }
    }
}

#[cfg(test)]
mod layer_tests;
