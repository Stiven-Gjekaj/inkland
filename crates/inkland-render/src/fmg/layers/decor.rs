//! Decoration: the compass rose, the scale bar, the vignette, the legend,
//! and the SVG filters of the styles (`index.html`, `draw-scalebar.ts`,
//! `draw-vignette.ts`, `draw-legend.ts`).

use super::super::Ctx;
use super::super::geom::{Px, n};
use crate::page::Page;
use inkland_core::config::{Config, LayerName, Variant};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// The filters of the original program (`index.html`), by their ids in
/// lower-case words.
const FILTERS: [(&str, &str); 21] = [
    (
        "blur-filter",
        r#"<filter id="blur-filter" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur in="SourceGraphic" stdDeviation="0.2"/></filter>"#,
    ),
    (
        "blur1",
        r#"<filter id="blur1" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur in="SourceGraphic" stdDeviation="1"/></filter>"#,
    ),
    (
        "blur3",
        r#"<filter id="blur3" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur in="SourceGraphic" stdDeviation="3"/></filter>"#,
    ),
    (
        "blur5",
        r#"<filter id="blur5" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur in="SourceGraphic" stdDeviation="5"/></filter>"#,
    ),
    (
        "blur7",
        r#"<filter id="blur7" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur in="SourceGraphic" stdDeviation="7"/></filter>"#,
    ),
    (
        "blur10",
        r#"<filter id="blur10" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur in="SourceGraphic" stdDeviation="10"/></filter>"#,
    ),
    (
        "splotch",
        r#"<filter id="splotch"><feTurbulence type="fractalNoise" baseFrequency=".01" numOctaves="4"/><feColorMatrix values="0 0 0 0 0, 0 0 0 0 0, 0 0 0 0 0, 0 0 0 -0.9 1.2" result="texture"/><feComposite in="SourceGraphic" in2="texture" operator="in"/></filter>"#,
    ),
    (
        "blured-splotch",
        r#"<filter id="blured-splotch"><feTurbulence type="fractalNoise" baseFrequency=".01" numOctaves="4"/><feColorMatrix values="0 0 0 0 0, 0 0 0 0 0, 0 0 0 0 0, 0 0 0 -0.9 1.2" result="texture"/><feComposite in="SourceGraphic" in2="texture" operator="in"/><feGaussianBlur stdDeviation="4"/></filter>"#,
    ),
    (
        "drop-shadow",
        r#"<filter id="drop-shadow"><feGaussianBlur in="SourceAlpha" stdDeviation="2"/><feOffset dx="1" dy="2"/><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter>"#,
    ),
    (
        "drop-shadow01",
        r#"<filter id="drop-shadow01"><feGaussianBlur in="SourceAlpha" stdDeviation=".1"/><feOffset dx=".2" dy=".3"/><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter>"#,
    ),
    (
        "drop-shadow05",
        r#"<filter id="drop-shadow05"><feGaussianBlur in="SourceAlpha" stdDeviation=".5"/><feOffset dx=".5" dy=".7"/><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter>"#,
    ),
    (
        "outline",
        r#"<filter id="outline"><feGaussianBlur in="SourceAlpha" stdDeviation="1"/><feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter>"#,
    ),
    (
        "pencil",
        r#"<filter id="pencil"><feTurbulence baseFrequency="0.03" numOctaves="6" type="fractalNoise"/><feDisplacementMap scale="3" in="SourceGraphic" xChannelSelector="R" yChannelSelector="G"/></filter>"#,
    ),
    (
        "turbulence",
        r#"<filter id="turbulence"><feTurbulence baseFrequency="0.1" numOctaves="3" type="fractalNoise"/><feDisplacementMap scale="10" in="SourceGraphic" xChannelSelector="R" yChannelSelector="G"/></filter>"#,
    ),
    (
        "paper",
        r##"<filter id="paper" filterUnits="objectBoundingBox" primitiveUnits="userSpaceOnUse" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="1 1" in="SourceGraphic" edgeMode="none" result="blur"/><feTurbulence type="fractalNoise" baseFrequency="0.05 0.05" numOctaves="4" seed="1" stitchTiles="stitch" result="turbulence"/><feDiffuseLighting surfaceScale="2" diffuseConstant="1" lighting-color="#707070" in="turbulence" result="diffuseLighting"><feDistantLight azimuth="45" elevation="20"/></feDiffuseLighting><feComposite in="diffuseLighting" in2="blur" operator="lighter" result="composite"/><feComposite in="composite" in2="SourceGraphic" operator="in" result="composite1"/></filter>"##,
    ),
    (
        "crumpled",
        r##"<filter id="crumpled" filterUnits="objectBoundingBox" primitiveUnits="userSpaceOnUse" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="2 2" in="SourceGraphic" edgeMode="none" result="blur"/><feTurbulence type="turbulence" baseFrequency="0.05 0.05" numOctaves="4" seed="1" stitchTiles="stitch" result="turbulence"/><feDiffuseLighting surfaceScale="2" diffuseConstant="1" lighting-color="#828282" in="turbulence" result="diffuseLighting"><feDistantLight azimuth="320" elevation="10"/></feDiffuseLighting><feComposite in="diffuseLighting" in2="blur" operator="lighter" result="composite"/><feComposite in="composite" in2="SourceGraphic" operator="in" result="composite1"/></filter>"##,
    ),
    (
        "filter-grayscale",
        r#"<filter id="filter-grayscale"><feColorMatrix values="0.3333 0.3333 0.3333 0 0 0.3333 0.3333 0.3333 0 0 0.3333 0.3333 0.3333 0 0 0 0 0 1 0"/></filter>"#,
    ),
    (
        "filter-sepia",
        r#"<filter id="filter-sepia"><feColorMatrix values="0.393 0.769 0.189 0 0 0.349 0.686 0.168 0 0 0.272 0.534 0.131 0 0 0 0 0 1 0"/></filter>"#,
    ),
    (
        "filter-dingy",
        r#"<filter id="filter-dingy"><feColorMatrix values="1 0 0 0 0 0 1 0 0 0 0 0.3 0.3 0 0 0 0 0 1 0"/></filter>"#,
    ),
    (
        "filter-tint",
        r#"<filter id="filter-tint"><feColorMatrix values="1.1 0 0 0 0  0 1.1 0 0 0  0 0 0.9 0 0  0 0 0 1 0"/></filter>"#,
    ),
    (
        "fog",
        r#"<mask id="fog"><rect x="-100000" y="-100000" width="200000" height="200000" fill="white"/></mask>"#,
    ),
];

/// The filters that the body uses.
pub fn filter_defs(d: &mut String, refs: &BTreeSet<String>) {
    for (id, def) in FILTERS {
        if refs.contains(id) {
            d.push_str(def);
            d.push('\n');
        }
    }
}

/// The compass rose of the original program (`defs-compass-rose` in
/// `index.html`), with ids in lower-case words.
const ROSE: &str = r##"<g id="defs-compass-rose" stroke-width="1.1"><g id="rose-coord-line" stroke="#3f3f3f"><line x1="0" y1="-20000" x2="0" y2="20000"/><line x1="-20000" y1="0" x2="20000" y2="0"/></g><use href="#rose-coord-line" transform="rotate(45)"/><use href="#rose-coord-line" transform="rotate(22.5)"/><use href="#rose-coord-line" transform="rotate(-22.5)"/><use href="#rose-coord-line" transform="rotate(11.25)"/><use href="#rose-coord-line" transform="rotate(-11.25)"/><use href="#rose-coord-line" transform="rotate(56.25)"/><use href="#rose-coord-line" transform="rotate(-56.25)"/><g stroke-width="8" stroke-opacity="1"><circle r="9" stroke="#000" fill="#1b1b1b"/><circle r="75" stroke="#008000" fill="#ffffff" fill-opacity=".1"/><circle r="212" stroke="#1b1b1b"/><circle r="211" stroke="#008000" fill="#ffffff" fill-opacity=".1"/></g><g stroke="#1b1b1b" stroke-opacity="1"><circle r="71"/><circle r="79"/><circle r="94"/><circle r="152"/><circle r="164"/><circle r="207"/></g><g id="rose-s3" stroke-opacity="1"><g id="rose-s2"><g id="rose-s1" stroke="#1b1b1b"><path d="M 39.416,95.16 C 33.65,103.95 30.76,110.5 28.93,117.18 C 15.24,113.43 13.54,127.15 23.04,131 C 13.71,145.8 7.84,173.93 0,212 L 0,103 A 103,103 0 0,0 39.416,95.16 z" fill="#47a3d1"/><path d="M 39.416,95.16 C 33.65,103.95 30.76,110.5 28.93,117.18 C 15.24,113.43 13.54,127.15 23.04,131 C 13.71,145.8 7.84,173.93 0,212 L 0,103 A 103,103 0 0,0 39.416,95.16 z" fill="black" transform="scale(-1,1)"/><path d="M -31.995,160.849 A 164,164 0 0,0 31.995,160.849 C 18.9,170.1 8.4,176.3 0,207 C -8.4,176.3 -18.9,170.1 -31.995,160.849 z" fill="#c2390f" transform="rotate(22.5)"/></g><use href="#rose-s1" transform="rotate(45)"/></g><use href="#rose-s2" transform="rotate(90)"/></g><use href="#rose-s3" transform="scale(-1)"/></g>"##;

/// Other definitions: the compass rose and the vignette mask.
pub fn other_defs(d: &mut String, ctx: &Ctx, refs: &BTreeSet<String>) {
    if refs.contains("defs-compass-rose") {
        d.push_str(ROSE);
        d.push('\n');
    }
    if refs.contains("vignette-mask") {
        let [mx, my, mw, mh] = ctx.px.map_rect();
        let st = &ctx.style;
        let pct = |k: &str, total: f64, start: f64, default: f64| {
            let v = st
                .option_str(&["vignette"], k)
                .and_then(|t| super::super::style::number(&t))
                .unwrap_or(default);
            start + total * v / 100.0
        };
        let blur = st
            .option_str(&["vignette"], "filter")
            .unwrap_or_else(|| "blur(20px)".into());
        let _ = writeln!(
            d,
            "<mask id=\"vignette-mask\" maskUnits=\"userSpaceOnUse\" {}><rect {} fill=\"white\"/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"black\" filter=\"{}\"/></mask>",
            ctx.map_rect_attrs(10.0),
            ctx.map_rect_attrs(10.0),
            n(pct("x", mw, mx, 0.3)),
            n(pct("y", mh, my, 0.4)),
            n(pct("width", mw, 0.0, 99.6)),
            n(pct("height", mh, 0.0, 99.2)),
            n(pct("rx", mw.min(mh), 0.0, 5.0)),
            n(pct("ry", mw.min(mh), 0.0, 5.0)),
            crate::extras::xml(&blur)
        );
    }
}

/// The centre and the scale of the compass rose, in page pixels: 80 pixels
/// from the top-left corner, as in the original program, or from the
/// top-right corner when the title cartouche of inkland is in the top-left
/// corner.
fn rose_at(px: &Px, config: &Config, variant: &Variant) -> ([f64; 2], f64) {
    let [mx, my, mw, _] = px.map_rect();
    let cartouche = crate::extras::merged(config, variant).cartouche.is_some();
    let x = if cartouche { mx + mw - 80.0 } else { mx + 80.0 };
    ([x, my + 80.0], 0.25)
}

/// The compass rose with its lines over the sea (`compass` layer).
pub fn compass(s: &mut String, ctx: &Ctx) {
    let ([x, y], k) = rose_at(&ctx.px, ctx.config, ctx.variant);
    let _ = writeln!(
        s,
        "<g id=\"compass\" fill=\"none\"{}><use id=\"compass-rose\" href=\"#defs-compass-rose\" transform=\"translate({} {}) scale({})\"/></g>",
        ctx.attrs(&["compass"], &[]),
        n(x),
        n(y),
        n(k)
    );
}

/// The scale of the scale bar: the text of the bar has the size of the
/// "other" text of inkland, not the screen size of the original program.
fn bar_zoom(config: &Config, px: &Px, font_px: f64) -> f64 {
    px.of_mm(crate::page::pt(config.render.font_sizes_pt.other)) / font_px
}

/// The length of the scale bar in pixels and in units (`getLength`).
fn bar_length(config: &Config, px: &Px, size: f64, zoom: f64) -> (f64, f64) {
    // One page pixel in config units.
    let unit_per_px = px.mm / px.page.scale;
    let mut val = 100.0 * size * zoom * unit_per_px;
    let round_to = |v: f64, step: f64| (v / step).round() * step;
    val = if val > 900.0 {
        round_to(val, 1000.0)
    } else if val > 90.0 {
        round_to(val, 100.0)
    } else if val > 9.0 {
        round_to(val, 10.0)
    } else {
        val.round().max(1.0)
    };
    let _ = config;
    (val / unit_per_px, val)
}

/// The box of the scale bar in page pixels: `[x0, y0, x1, y1]`, and the
/// zoom and the bar length.
fn bar_box(
    ctx_style: &super::super::style::Style,
    config: &Config,
    px: &Px,
) -> ([f64; 4], f64, f64, f64) {
    let size = ctx_style
        .option_f64(&["scaleBar"], "barSize")
        .unwrap_or(2.0);
    let font = ctx_style
        .attr_f64(&["scaleBar"], "font-size")
        .unwrap_or(10.0);
    let zoom = bar_zoom(config, px, font);
    let (len, val) = bar_length(config, px, size, zoom);
    let [mx, my, mw, mh] = px.map_rect();
    let x_pct = ctx_style.option_f64(&["scaleBar"], "x").unwrap_or(99.0);
    let y_pct = ctx_style.option_f64(&["scaleBar"], "y").unwrap_or(99.0);
    let back = ["scaleBar", "back"];
    let pad = |k: &str, d: f64| ctx_style.option_f64(&back, k).unwrap_or(d) * zoom;
    let width = (len / zoom + size) * zoom + pad("left", 10.0) + pad("right", 15.0);
    let height =
        (font * 1.6 + size * 2.0) * zoom + pad("top", 20.0) + pad("bottom", 15.0) - 20.0 * zoom;
    let x1 = mx + mw * x_pct / 100.0 - 4.0 * zoom;
    let y1 = my + mh * y_pct / 100.0 - 4.0 * zoom;
    ([x1 - width, y1 - height, x1, y1], zoom, len, val)
}

/// The scale bar (`drawScaleBar`), in the bottom-right corner.
pub fn scale_bar(s: &mut String, ctx: &Ctx) {
    let st = &ctx.style;
    let size = st.option_f64(&["scaleBar"], "barSize").unwrap_or(2.0);
    let ([x0, y0, x1, y1], zoom, len, val) = bar_box(st, ctx.config, &ctx.px);
    let back = ["scaleBar", "back"];
    let pad = |k: &str, d: f64| st.option_f64(&back, k).unwrap_or(d);
    let unit = crate::extras::xml(&ctx.config.units.name);
    let mut c = String::new();
    let _ = write!(
        c,
        "<line x1=\"0.5\" y1=\"0\" x2=\"{}\" y2=\"0\" stroke-width=\"{size}\" stroke=\"white\"/><line x1=\"0\" y1=\"{size}\" x2=\"{}\" y2=\"{size}\" stroke-width=\"{size}\" stroke=\"#3d3d3d\"/><line x1=\"0\" y1=\"0\" x2=\"{}\" y2=\"0\" stroke-width=\"{}\" stroke-dasharray=\"{size} {}\" stroke=\"#3d3d3d\"/>",
        n(len / zoom + size - 0.5),
        n(len / zoom + size),
        n(len / zoom + size),
        n(size * 3.0),
        n(len / zoom / 5.0 - size)
    );
    c.push_str("<g text-anchor=\"middle\">");
    for i in 0..6 {
        let v = val * f64::from(i) / 5.0;
        let text = if i < 5 {
            inkland_core::num::fixed(v, 0)
        } else {
            format!("{} {unit}s", inkland_core::num::fixed(v, 0))
        };
        let _ = write!(
            c,
            "<text x=\"{}\" y=\"0\" dy=\"-.6em\">{}</text>",
            n(len / zoom * f64::from(i) / 5.0),
            text
        );
    }
    c.push_str("</g>");
    let left = pad("left", 10.0);
    let top = pad("top", 20.0);
    let w = (x1 - x0) / zoom;
    let h = (y1 - y0) / zoom;
    let font = crate::extras::xml(&ctx.config.render.fonts.other);
    let _ = writeln!(
        s,
        "<g id=\"scale-bar\" font-family=\"{font}\" transform=\"translate({} {}) scale({})\"{}><rect id=\"scale-bar-back\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\"{}/><g transform=\"translate({} {})\">{c}</g></g>",
        n(x0),
        n(y0),
        inkland_core::num::fixed(zoom, 4),
        ctx.attrs(&["scaleBar"], &[]),
        n(w),
        n(h),
        ctx.attrs(&back, &[]),
        n(left),
        n(top)
    );
}

/// The vignette: a dark edge around the map (`drawVignette`).
pub fn vignette(s: &mut String, ctx: &Ctx) {
    let _ = writeln!(
        s,
        "<g id=\"vignette\" mask=\"url(#vignette-mask)\"{}><rect {}/></g>",
        ctx.attrs(&["vignette"], &[]),
        ctx.map_rect_attrs(10.0)
    );
}

/// A legend box of the area layer of the map (`drawLegend`): the states,
/// else the cultures, else the religions, else the biomes.
pub fn legend(s: &mut String, ctx: &Ctx) {
    let soc = &ctx.world.society;
    let pal = &ctx.palette;
    let (title, items): (&str, Vec<(String, String)>) = if ctx.has(LayerName::States) {
        (
            "States",
            soc.states
                .iter()
                .map(|x| {
                    (
                        pal.states.get(&x.id).cloned().unwrap_or_default(),
                        x.name.clone(),
                    )
                })
                .collect(),
        )
    } else if ctx.has(LayerName::Cultures) {
        (
            "Cultures",
            soc.cultures
                .iter()
                .map(|x| {
                    (
                        pal.cultures.get(&x.id).cloned().unwrap_or_default(),
                        x.name.clone(),
                    )
                })
                .collect(),
        )
    } else if ctx.has(LayerName::Religions) {
        (
            "Religions",
            ctx.world
                .religions
                .list
                .iter()
                .map(|x| {
                    (
                        pal.religions.get(&x.id).cloned().unwrap_or_default(),
                        x.name.clone(),
                    )
                })
                .collect(),
        )
    } else {
        let mut used: BTreeSet<u8> = BTreeSet::new();
        for c in 0..ctx.world.grid.len() {
            if ctx.world.features.is_land(c) {
                used.insert(ctx.world.biomes[c]);
            }
        }
        (
            "Biomes",
            used.into_iter()
                .map(|b| {
                    (
                        super::palette::BIOME_COLORS[b as usize].to_owned(),
                        inkland_core::biomes::NAMES[b as usize].replace('-', " "),
                    )
                })
                .collect(),
        )
    };
    if !items.is_empty() {
        let at = |width: f64, height: f64, zoom: f64| {
            let [mx, my, mw, mh] = ctx.px.map_rect();
            [
                mx + mw * 0.99 - width * zoom,
                my + mh * 0.93 - height * zoom,
            ]
        };
        legend_box(s, ctx, "legend", title, &items, &at);
    }
    // The legend boxes of the world (`pack.legends`), down from the top left
    // of the map, or at their own place.
    let mut stack = 0.0;
    for (k, l) in ctx.world.legends.iter().enumerate() {
        let items: Vec<(String, String)> = l
            .items
            .iter()
            .map(|i| (i.color.clone(), i.text.clone()))
            .collect();
        let place = |_: f64, _: f64, _: f64| match l.at {
            Some(p) => ctx.p(p),
            None => {
                let [mx, my, mw, mh] = ctx.px.map_rect();
                [mx + mw * 0.01, my + mh * 0.01 + stack]
            }
        };
        let height = legend_box(
            s,
            ctx,
            &format!("legend-box-{}", k + 1),
            &l.name,
            &items,
            &place,
        );
        if l.at.is_none() {
            stack += height + 4.0;
        }
    }
}

/// Draw one legend box with its title and items, and give back its height
/// on the page. `place` gives the top left corner from the width, the
/// height, and the zoom of the box.
fn legend_box(
    s: &mut String,
    ctx: &Ctx,
    id: &str,
    title: &str,
    items: &[(String, String)],
    place: &dyn Fn(f64, f64, f64) -> [f64; 2],
) -> f64 {
    let st = &ctx.style;
    let size = st.attr_f64(&["legend"], "font-size").unwrap_or(13.0);
    let zoom = bar_zoom(ctx.config, &ctx.px, size);
    let per_col = st
        .option_f64(&["legend"], "columns")
        .unwrap_or(8.0)
        .max(1.0) as usize;
    let line_h = (size * 1.7).round();
    let box_s = (size / 1.7).round();
    let col_off = size;
    let v_off = size / 2.0;
    let columns = items.len().div_ceil(per_col).max(1);
    let lines = items.len().div_ceil(columns);
    let f = crate::metrics::FontMetrics::of(
        &st.attr(&["legend"], "font-family")
            .unwrap_or_else(|| ctx.config.render.fonts.labels.clone()),
    );
    let col_w = items
        .iter()
        .map(|(_, t)| f.width(t, size, 0.0))
        .fold(0.0, f64::max)
        + box_s * 1.6
        + col_off;
    let width = col_off * 2.0 + col_w * columns as f64;
    let height = line_h * (lines.max(1) as f64 + 1.0) + v_off * 2.0;
    let [x, y] = place(width, height, zoom);
    let mut body = String::new();
    for (i, (color, name)) in items.iter().enumerate() {
        let col = i / lines;
        let row = i % lines;
        let ox = col_off + col as f64 * col_w;
        let oy = line_h + row as f64 * line_h + v_off;
        let _ = write!(
            body,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\" stroke-width=\"0.5\" stroke=\"#111111\" stroke-dasharray=\"none\"/><text x=\"{}\" y=\"{}\" stroke=\"none\">{}</text>",
            n(ox),
            n(oy),
            n(box_s),
            n(box_s),
            n(ox + box_s * 1.6),
            n(oy + size / 1.6),
            crate::extras::xml(name)
        );
    }
    let _ = writeln!(
        s,
        "<g id=\"{id}\"{} transform=\"translate({} {}) scale({})\"{}><rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" stroke=\"none\"{}/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-weight=\"bold\" font-size=\"1.2em\" stroke=\"none\">{}</text>{body}</g>",
        if id == "legend" {
            ""
        } else {
            " data-legend=\"true\""
        },
        n(x),
        n(y),
        inkland_core::num::fixed(zoom, 4),
        ctx.attrs(&["legend"], &[]),
        n(width),
        n(height),
        ctx.attrs(&["legend", "box"], &[]),
        n(width / 2.0),
        n(size * 1.1 + v_off / 2.0),
        crate::extras::xml(title)
    );
    height * zoom
}

/// The boxes of the compass and the scale bar of `colour` mode on the page,
/// in mm: `[x0, y0, x1, y1]`. Labels keep off them.
pub fn boxes(config: &Config, variant: &Variant, page: &Page) -> Vec<[f64; 4]> {
    let e = crate::extras::merged(config, variant);
    let layers = super::super::layers_of(config);
    let px = Px::new(page, config.frame.width, config.frame.height);
    let style = super::super::style::Style::with_overrides(
        config.render.style,
        &config.render.style_overrides,
    );
    let mut out = Vec::new();
    if e.compass == Some(true) || layers.contains(&LayerName::Compass) {
        let ([x, y], k) = rose_at(&px, config, variant);
        let r = 212.0 * k;
        out.push([
            (x - r) * px.mm,
            (y - r) * px.mm,
            (x + r) * px.mm,
            (y + r) * px.mm,
        ]);
    }
    if e.scale_bar == Some(true) || layers.contains(&LayerName::ScaleBar) {
        let ([a, b, c, d], _, _, _) = bar_box(&style, config, &px);
        out.push([a * px.mm, b * px.mm, c * px.mm, d * px.mm]);
    }
    out
}
