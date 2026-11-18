//! The layers of `colour` mode, and the definitions that they use.

pub(crate) mod areas;
mod base;
mod decor;
mod lines;
pub(crate) mod marks;
pub(crate) mod overlays;
mod palette;
pub(crate) mod text;

pub use decor::boxes;
pub use palette::Palette;

use super::Ctx;
use inkland_core::config::LayerName;
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// Write one layer.
pub fn write(s: &mut String, ctx: &Ctx, layer: LayerName) {
    use LayerName::*;
    match layer {
        Ocean => base::ocean(s, ctx),
        Landmass => base::landmass(s, ctx),
        Texture => {}
        Heightmap => base::heightmap(s, ctx),
        Lakes => base::lakes(s, ctx),
        Coastline => base::coastline(s, ctx),
        Biomes => areas::biomes(s, ctx),
        Religions => areas::religions(s, ctx),
        Cultures => areas::cultures(s, ctx),
        States => areas::states(s, ctx),
        Provinces => areas::provinces(s, ctx),
        Zones => areas::zones(s, ctx),
        Borders => lines::borders(s, ctx),
        Routes => lines::routes(s, ctx),
        Rivers => lines::rivers(s, ctx),
        Relief => marks::relief(s, ctx),
        BurgIcons => marks::burg_icons(s, ctx),
        Ice => marks::ice(s, ctx),
        Emblems => marks::emblems(s, ctx),
        Markers => marks::markers(s, ctx),
        Features => marks::features(s, ctx),
        Cells => overlays::cells(s, ctx),
        Grid => overlays::grid(s, ctx),
        Coordinates => overlays::coordinates(s, ctx),
        Temperature => overlays::temperature(s, ctx),
        Precipitation => overlays::precipitation(s, ctx),
        Population => overlays::population(s, ctx),
        Compass => decor::compass(s, ctx),
        ScaleBar => decor::scale_bar(s, ctx),
        Vignette => decor::vignette(s, ctx),
        Legend => decor::legend(s, ctx),
        Labels => text::labels(s, ctx),
        Coast | Regions | Roads | Places => {}
    }
}

/// Write the extras of the variant that are not layers: the scale bar and
/// the compass when the map does not draw them as layers.
pub fn write_extras(s: &mut String, ctx: &Ctx) {
    let e = crate::extras::merged(ctx.config, ctx.variant);
    if e.compass == Some(true) && !ctx.has(LayerName::Compass) {
        decor::compass(s, ctx);
    }
    if e.scale_bar == Some(true) && !ctx.has(LayerName::ScaleBar) {
        decor::scale_bar(s, ctx);
    }
}

/// The extras that go on the page outside the map group: the grid and the
/// cartouche of inkland.
pub fn page_extras(ctx: &Ctx) -> String {
    let mut s = String::new();
    let e = crate::extras::merged(ctx.config, ctx.variant);
    let symbols = crate::labels::symbol_points(ctx.world, &ctx.px.page);
    crate::extras::write_only(
        &mut s,
        ctx.config,
        ctx.variant,
        &ctx.px.page,
        &symbols,
        crate::extras::Parts {
            scale_bar: false,
            compass: false,
            cartouche: e.cartouche.is_some(),
            grid: e.grid.is_some(),
        },
    );
    s
}

/// The ids that the body refers to with `url(#id)` or `href="#id"`.
fn references(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for pat in ["url(#", "href=\"#"] {
        let mut rest = body;
        while let Some(at) = rest.find(pat) {
            rest = &rest[at + pat.len()..];
            let end = rest.find([')', '"']).unwrap_or(rest.len());
            out.insert(rest[..end].to_owned());
        }
    }
    out
}

/// The `<defs>` of the map: the clip of the map, the feature shapes, the
/// masks, the filters, the patterns, and the symbols that the body uses.
pub fn defs(ctx: &Ctx, body: &str) -> String {
    let refs = references(body);
    let mut d = String::new();
    d.push_str("<defs>\n");
    let _ = writeln!(
        d,
        "<clipPath id=\"clip-map-px\"><rect {}/></clipPath>",
        ctx.map_rect_attrs(0.0)
    );
    let needs_features = refs
        .iter()
        .any(|r| r.starts_with("feature-") || matches!(r.as_str(), "land" | "water"))
        || refs
            .iter()
            .any(|r| r.starts_with("coastal-") || r == "waves-mask");
    if needs_features {
        base::feature_defs(&mut d, ctx);
    }
    decor::filter_defs(&mut d, &refs);
    decor::other_defs(&mut d, ctx, &refs);
    marks::symbol_defs(&mut d, &refs);
    for (id, def) in ctx.extra_defs.borrow().iter() {
        if refs.contains(id) {
            d.push_str(def);
            d.push('\n');
        }
    }
    d.push_str("</defs>\n");
    d
}
