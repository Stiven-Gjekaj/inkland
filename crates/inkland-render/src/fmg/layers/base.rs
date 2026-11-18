//! The base layers: ocean, landmass, lakes, coastline, and heightmap
//! (`draw-ocean.ts`, `draw-landmass.ts`, `draw-lakes.ts`,
//! `draw-coastline.ts`, `heightmap/draw-heightmap.ts`,
//! `coastal-waves.ts`, `draw-coastal-bands.ts`).

use super::super::Ctx;
use super::super::color::{height_color, height_scheme};
use super::super::geom::{self, Edge, basis_closed, n};
use inkland_core::grid::{Grid, OUTSIDE};
use inkland_core::rand::Rand;
use inkland_core::seed::{Layer, SeedTree};
use std::collections::VecDeque;
use std::fmt::Write as _;

/// The feature paths, and the masks of land and water (`drawLandmass`).
pub fn feature_defs(d: &mut String, ctx: &Ctx) {
    let px = ctx.px;
    let map = |p: [f64; 2]| px.from_world_px(p);
    let mut land = String::new();
    let mut water = String::new();
    for o in &ctx.outlines {
        let path = geom::coast_path(&o.shape, &map, 2);
        if path.is_empty() {
            continue;
        }
        let _ = writeln!(d, "<path id=\"feature-{}\" d=\"{path}\"/>", o.feature);
        let (l, w) = if o.lake {
            ("black", "white")
        } else {
            ("white", "black")
        };
        let _ = write!(land, "<use href=\"#feature-{}\" fill=\"{l}\"/>", o.feature);
        let _ = write!(water, "<use href=\"#feature-{}\" fill=\"{w}\"/>", o.feature);
    }
    let rect = ctx.map_rect_attrs(50.0);
    let _ = writeln!(
        d,
        "<mask id=\"land\" maskUnits=\"userSpaceOnUse\" {rect}>{land}</mask>"
    );
    let _ = writeln!(
        d,
        "<mask id=\"water\" maskUnits=\"userSpaceOnUse\" {rect}><rect {rect} fill=\"white\"/>{water}</mask>"
    );
}

/// The distance of each cell of the first grid from land, in cells: 0 on
/// land. The ocean layers of the original program use the first grid.
fn coast_distance(ctx: &Ctx, limit: u32) -> Vec<u32> {
    let w = &ctx.world.base;
    let g = &w.grid;
    let mut d = vec![u32::MAX; g.len()];
    let mut queue = VecDeque::new();
    for (c, dc) in d.iter_mut().enumerate() {
        if w.features.is_land(c) {
            *dc = 0;
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        if d[c] >= limit {
            continue;
        }
        for m in g.neighbors(c) {
            if d[m] == u32::MAX {
                d[m] = d[c] + 1;
                queue.push_back(m);
            }
        }
    }
    d
}

/// Keep each `every`-th point of a ring, and each point on the frame.
fn relax(ctx: &Ctx, g: &Grid, ring: &[Edge], every: usize) -> Vec<[f64; 2]> {
    ring.iter()
        .enumerate()
        .filter(|(i, e)| i % every == 0 || e.other == OUTSIDE)
        .map(|(_, e)| ctx.p(g.vertices[e.from as usize]))
        .collect()
}

/// The ocean: the base colour, the depth layers, the waves, and the coastal
/// bands (`drawOcean`).
pub fn ocean(s: &mut String, ctx: &Ctx) {
    let st = &ctx.style;
    let _ = writeln!(s, "<g id=\"ocean\"{}>", ctx.attrs(&["ocean"], &[]));
    let _ = writeln!(
        s,
        "<g id=\"ocean-layers\" fill-rule=\"evenodd\"{}>",
        ctx.attrs(&["ocean", "oceanLayers"], &[])
    );
    let _ = writeln!(
        s,
        "<rect id=\"ocean-base\" {}{}/>",
        ctx.map_rect_attrs(10.0),
        ctx.attrs(&["ocean", "base"], &[])
    );
    let outline = st
        .option_str(&["ocean", "oceanLayers"], "outline")
        .unwrap_or_default();
    let limits: Vec<u32> = if outline.is_empty() || outline == "none" {
        Vec::new()
    } else {
        outline
            .split(',')
            .filter_map(|t| t.trim().parse::<i64>().ok())
            .filter(|&t| t != 0)
            .map(|t| t.unsigned_abs() as u32)
            .collect()
    };
    if !limits.is_empty() {
        let max = limits.iter().copied().max().unwrap_or(1);
        let dist = coast_distance(ctx, max + 1);
        let opacity = inkland_core::num::fixed(0.4 / limits.len() as f64, 2);
        for t in &limits {
            let rings = geom::rings_on(&ctx.world.base.grid, &|c| u32::from(dist[c] <= *t));
            let mut d = String::new();
            for ring in rings.get(&1).into_iter().flatten() {
                let pts = relax(ctx, &ctx.world.base.grid, ring, 1 + 2 * (*t as usize));
                if pts.len() >= 4 {
                    d.push_str(&basis_closed(&pts));
                }
            }
            if !d.is_empty() {
                let _ = writeln!(
                    s,
                    "<path d=\"{d}\" fill=\"#ecf2f9\" fill-opacity=\"{opacity}\"/>"
                );
            }
        }
    }
    s.push_str("</g>\n");
    ocean_pattern(s, ctx);
    waves(s, ctx);
    bands(s, ctx);
    s.push_str("</g>\n");
}

/// The page pixels in one map pixel of the original program.
fn map_px(ctx: &Ctx) -> f64 {
    ctx.px.from_world_px([1.0, 0.0])[0] - ctx.px.from_world_px([0.0, 0.0])[0]
}

/// The ocean pattern: an image tile of 100 map pixels over the whole map
/// (`drawOceanBase`).
fn ocean_pattern(s: &mut String, ctx: &Ctx) {
    let p = ["ocean", "pattern"];
    let Some(href) = ctx
        .style
        .attr(&p, "href")
        .and_then(|h| super::super::images::href(&h))
    else {
        return;
    };
    let tile = n(100.0 * map_px(ctx));
    let opacity = ctx
        .style
        .attr(&p, "opacity")
        .map(|o| format!(" opacity=\"{}\"", crate::extras::xml(&o)))
        .unwrap_or_default();
    ctx.def(
        "oceanic",
        format!(
            "<pattern id=\"oceanic\" width=\"{tile}\" height=\"{tile}\" patternUnits=\"userSpaceOnUse\"><image id=\"oceanic-pattern\" width=\"{tile}\" height=\"{tile}\" href=\"{href}\"{opacity}/></pattern>"
        ),
    );
    let _ = writeln!(
        s,
        "<g id=\"ocean-pattern\"><rect {} fill=\"url(#oceanic)\"/></g>",
        ctx.map_rect_attrs(10.0)
    );
}

/// The paper texture: one image over the map, from the point `x`, `y`
/// (`drawTexture`).
pub fn texture(s: &mut String, ctx: &Ctx) {
    let p = ["texture"];
    let Some(href) = ctx
        .style
        .option_str(&p, "href")
        .and_then(|h| super::super::images::href(&h))
    else {
        return;
    };
    let k = map_px(ctx);
    let x = ctx.style.option_f64(&p, "x").unwrap_or(0.0) * k;
    let y = ctx.style.option_f64(&p, "y").unwrap_or(0.0) * k;
    let [mx, my, mw, mh] = ctx.px.map_rect();
    let _ = writeln!(
        s,
        "<g id=\"texture\"{}><image preserveAspectRatio=\"xMidYMid slice\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" href=\"{href}\"/></g>",
        ctx.attrs(&p, &[]),
        n(mx + x),
        n(my + y),
        n((mw - x).max(0.0)),
        n((mh - y).max(0.0))
    );
}

/// The reach of the coastal bands, in cell spacings.
fn band_radius(spacing: f64, i: f64) -> f64 {
    spacing * (i + 0.15 * i * (i - 1.0))
}

/// The cell spacing, in pixels.
fn spacing_px(ctx: &Ctx) -> f64 {
    let g = &ctx.world.grid;
    (g.step_x * g.step_y).sqrt() * ctx.px.unit_px
}

/// Bands along the coast (`drawCoastalBands`).
fn bands(s: &mut String, ctx: &Ctx) {
    let st = &ctx.style;
    let p = ["ocean"];
    if !st.option_bool(&p, "bands.render") {
        return;
    }
    let count = st.option_f64(&p, "bands.count").unwrap_or(5.0).max(1.0);
    let spacing = st.option_f64(&p, "bands.spacing").unwrap_or(1.1);
    let width = st.option_f64(&p, "bands.width").unwrap_or(0.25);
    let color = st
        .option_str(&p, "bands.color")
        .unwrap_or_else(|| "#575448".to_owned());
    let shore = st
        .option_str(&p, "bands.shore")
        .unwrap_or_else(|| "#b9b6a1".to_owned());
    let shade = st.option_f64(&p, "bands.shade").unwrap_or(0.35);
    let opacity = st.option_f64(&p, "bands.opacity").unwrap_or(1.0);
    let uses: String = ctx
        .outlines
        .iter()
        .filter(|o| !o.lake && !o.in_lake)
        .map(|o| format!("<use href=\"#feature-{}\"/>", o.feature))
        .collect();
    let mut strokes = String::new();
    let mut shades = String::new();
    let mut i = count;
    while i >= 1.0 {
        let r = band_radius(spacing, i);
        let edge = width.min(r);
        let _ = write!(
            strokes,
            "<g stroke=\"white\" stroke-width=\"{}\">{uses}</g><g stroke=\"black\" stroke-width=\"{}\">{uses}</g>",
            n(2.0 * r),
            n(2.0 * (r - edge).max(0.0))
        );
        let l = (255.0 * (1.0 - (i - 1.0) / count)).round() as u8;
        let _ = write!(
            shades,
            "<g stroke=\"#{l:02x}{l:02x}{l:02x}\" stroke-width=\"{}\">{uses}</g>",
            n(2.0 * r)
        );
        i -= 1.0;
    }
    let rect = ctx.map_rect_attrs(0.0);
    let _ = writeln!(
        s,
        "<g id=\"ocean-bands\" mask=\"url(#coastal-bands-mask)\" opacity=\"{}\" fill=\"none\" stroke-linejoin=\"round\" stroke-linecap=\"round\">",
        inkland_core::num::fixed(opacity, 3)
    );
    let _ = writeln!(
        s,
        "<rect {rect} fill=\"{shore}\" fill-opacity=\"{}\" mask=\"url(#coastal-bands-shade)\"/>",
        inkland_core::num::fixed(shade, 3)
    );
    let _ = writeln!(
        s,
        "<rect {rect} fill=\"{color}\" mask=\"url(#coastal-bands-lines)\"/>"
    );
    s.push_str("</g>\n");
    let mrect = ctx.map_rect_attrs(10.0);
    let land: String = ctx
        .outlines
        .iter()
        .filter(|o| !o.lake)
        .map(|o| format!("<use href=\"#feature-{}\" fill=\"black\"/>", o.feature))
        .collect();
    ctx.def(
        "coastal-bands-mask",
        format!("<mask id=\"coastal-bands-mask\" maskUnits=\"userSpaceOnUse\" {mrect}><rect {mrect} fill=\"white\"/>{land}</mask>"),
    );
    for (id, body) in [
        ("coastal-bands-lines", strokes),
        ("coastal-bands-shade", shades),
    ] {
        ctx.def(
            id,
            format!("<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" {mrect}><g fill=\"none\" stroke-linejoin=\"round\" stroke-linecap=\"round\">{body}</g></mask>"),
        );
    }
}

/// Wave dashes on the sea near the coast (`getCoastalWaves`).
fn waves(s: &mut String, ctx: &Ctx) {
    let st = &ctx.style;
    let p = ["ocean", "oceanWaves"];
    if !st.option_bool(&p, "render") {
        return;
    }
    let kind = st.option_str(&p, "type").unwrap_or_else(|| "waves".into());
    let density = st.option_f64(&p, "density").unwrap_or(1.0).max(0.01);
    let length = st.option_f64(&p, "length").unwrap_or(1.0);
    let reach = st.option_f64(&p, "reach").unwrap_or(4.0).max(0.5);
    let halo_opt = st.option_f64(&p, "halo").unwrap_or(0.25);
    let spacing = spacing_px(ctx);
    let bands_on = st.option_bool(&["ocean"], "bands.render");
    let band_reach = if bands_on {
        band_radius(
            st.option_f64(&["ocean"], "bands.spacing").unwrap_or(1.1),
            st.option_f64(&["ocean"], "bands.count").unwrap_or(5.0),
        )
    } else {
        0.0
    };
    let band_cells = band_reach / spacing;
    let dist = coast_distance(ctx, (reach * 2.0 + band_cells).ceil() as u32 + 1);
    let w = ctx.world;
    let [mx, my, mw, mh] = ctx.px.map_rect();
    let distance_at = |x: f64, y: f64| -> f64 {
        let wp = ctx.px.page.to_world([x * ctx.px.mm, y * ctx.px.mm]);
        let b = &w.base;
        let c = b
            .grid
            .find_cell(wp[0].clamp(0.0, w.width), wp[1].clamp(0.0, w.height));
        if b.features.is_land(c)
            || b.features.get(b.features.of_cell[c]).kind
                == inkland_core::features::FeatureKind::Lake
        {
            return 0.0;
        }
        let dc = dist[c];
        if dc == u32::MAX {
            f64::INFINITY
        } else {
            (f64::from(dc) - band_cells).max(1.0)
        }
    };
    let mut rng = Rand::new(SeedTree::new(ctx.config.seed).part_rng(Layer::Decoration, "waves"));
    let row_gap = spacing * 0.4 / density;
    let half_period = spacing * 0.5 / 2.0;
    let amplitude = (spacing * 0.06).min(row_gap * 0.3);
    let mut d = String::new();
    let mut y = my + row_gap * rng.unit();
    while y < my + mh {
        let mut x = mx - rng.unit() * 2.5 * spacing;
        while x < mx + mw {
            let sample_x = (x + spacing).clamp(mx, mx + mw);
            let coast = distance_at(sample_x, y);
            let distance = if coast > reach * 2.0 + 1.0 {
                1.0
            } else {
                coast
            };
            let closeness = if distance > 0.0 {
                (1.0 - (distance - 1.0 + rng.unit() * 1.5) / reach).max(0.0)
            } else {
                0.0
            };
            let dash = 2.5 * length * spacing * (0.4 + 0.6 * rng.unit()) * (0.4 + 0.6 * closeness);
            if closeness > 0.0 && rng.unit() < closeness {
                let up = if rng.unit() < 0.5 { -1.0 } else { 1.0 };
                if kind == "lines" {
                    let _ = write!(d, "M{},{}h{}", n(x), n(y), n(dash));
                } else {
                    let halves = ((dash / half_period).round() as usize).max(2);
                    let step = halves as f64 * half_period / halves as f64;
                    let _ = write!(
                        d,
                        "M{},{}q{},{} {},0",
                        n(x),
                        n(y),
                        n(step / 2.0),
                        n(amplitude * up),
                        n(step)
                    );
                    for _ in 1..halves {
                        let _ = write!(d, "t{},0", n(step));
                    }
                }
            }
            x += dash + 0.5 * spacing * (0.5 + rng.unit()) * (1.0 + 3.0 * (1.0 - closeness));
        }
        y += row_gap * (0.85 + rng.unit() * 0.3);
    }
    let halo = n((band_reach + halo_opt * spacing) * 2.0);
    let land: String = ctx
        .outlines
        .iter()
        .filter(|o| !o.lake)
        .map(|o| {
            format!(
                "<use href=\"#feature-{}\" fill=\"black\" stroke=\"black\" stroke-width=\"{halo}\"/>",
                o.feature
            )
        })
        .collect();
    let mrect = ctx.map_rect_attrs(10.0);
    ctx.def(
        "waves-mask",
        format!("<mask id=\"waves-mask\" maskUnits=\"userSpaceOnUse\" {mrect}><rect {mrect} fill=\"white\"/><g stroke-linejoin=\"round\">{land}</g></mask>"),
    );
    let a = |k: &str| st.attr(&p, k);
    let dash = a("stroke-dasharray")
        .map(|v| format!(" stroke-dasharray=\"{v}\""))
        .unwrap_or_default();
    let _ = writeln!(
        s,
        "<g id=\"ocean-waves\"><path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"{dash} opacity=\"{}\" stroke-linecap=\"round\" mask=\"url(#waves-mask)\"/></g>",
        a("stroke").unwrap_or_else(|| "none".into()),
        a("stroke-width").unwrap_or_else(|| "0.5".into()),
        a("opacity").unwrap_or_else(|| "1".into())
    );
}

/// The land colour, through the land mask (`drawLandmass`).
pub fn landmass(s: &mut String, ctx: &Ctx) {
    let _ = writeln!(
        s,
        "<g id=\"landmass\" mask=\"url(#land)\" fill-rule=\"evenodd\"{}><rect {}/></g>",
        ctx.attrs(&["landmass"], &[]),
        ctx.map_rect_attrs(10.0)
    );
}

/// The ripples of a lake, after its outline, with a mask that keeps them
/// off the shore and off the islands in lakes (`drawLakeEmbellishments`).
fn lake_ripples(ctx: &Ctx, o: &geom::Outline, group: &str) -> String {
    let st = &ctx.style;
    let p = ["lakes", group];
    let kind = st
        .option_str(&p, "embellishment")
        .unwrap_or_else(|| "none".into());
    if kind == "none" {
        return String::new();
    }
    let look = super::ripples::Look {
        embellishment: kind,
        density: st.option_f64(&p, "density").unwrap_or(1.0),
        length: st.option_f64(&p, "length").unwrap_or(1.0),
        halo: st.option_f64(&p, "halo").unwrap_or(0.2),
        width: st.option_f64(&p, "width").unwrap_or(0.3),
    };
    let px = ctx.px;
    let points: Vec<[f64; 2]> = o
        .shape
        .points
        .iter()
        .map(|&q| px.from_world_px(q))
        .collect();
    let k = px.from_world_px([1.0, 0.0])[0] - px.from_world_px([0.0, 0.0])[0];
    let spacing = spacing_px(ctx) * k;
    let mut rng = Rand::new(
        SeedTree::new(ctx.config.seed).part_rng(Layer::Decoration, &format!("lake-{}", o.feature)),
    );
    let r = super::ripples::lake_ripples(&points, spacing, &look, &mut rng);
    if r.path.is_empty() {
        return String::new();
    }
    let id = format!("lake-ripples-{}", o.feature);
    let [x, y, w, h] = r.bounds;
    let stroke = n(r.gap * 2.0);
    let islands: String = ctx
        .outlines
        .iter()
        .filter(|i| !i.lake && i.in_lake)
        .map(|i| {
            format!(
                "<use href=\"#feature-{}\" fill=\"black\" stroke=\"black\"/>",
                i.feature
            )
        })
        .collect();
    ctx.def(
        &id,
        format!(
            "<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><use href=\"#feature-{}\" fill=\"white\" stroke=\"black\" stroke-width=\"{stroke}\" stroke-linejoin=\"round\"/><g stroke-width=\"{stroke}\" stroke-linejoin=\"round\">{islands}</g></mask>",
            n(x),
            n(y),
            n(w),
            n(h),
            o.feature
        ),
    );
    format!(
        "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-dasharray=\"none\" stroke-linecap=\"round\" opacity=\"{}\" mask=\"url(#{id})\"/>",
        r.path,
        crate::extras::xml(
            &st.option_str(&p, "color")
                .unwrap_or_else(|| "#000000".into())
        ),
        n(look.width),
        n(st.option_f64(&p, "opacity").unwrap_or(0.6))
    )
}

/// The lake group of a lake feature.
fn lake_group(ctx: &Ctx, feature: u32) -> &'static str {
    ctx.world
        .names
        .features
        .iter()
        .find(|f| f.id == feature)
        .map_or("freshwater", |f| {
            if f.group == "frozen" {
                "frozen"
            } else {
                "freshwater"
            }
        })
}

/// Lakes, in their groups, through the water mask (`drawLakes`).
pub fn lakes(s: &mut String, ctx: &Ctx) {
    let _ = writeln!(
        s,
        "<g id=\"lakes\" mask=\"url(#water)\"{}>",
        ctx.attrs(&["lakes"], &[])
    );
    for group in ctx.style.groups(&["lakes"]) {
        let uses: String = ctx
            .outlines
            .iter()
            .filter(|o| o.lake && lake_group(ctx, o.feature) == group)
            .map(|o| {
                format!(
                    "<use href=\"#feature-{}\"/>{}",
                    o.feature,
                    lake_ripples(ctx, o, &group)
                )
            })
            .collect();
        let _ = writeln!(
            s,
            "<g id=\"lakes-{}\"{}>{uses}</g>",
            super::super::icons::kebab(&group),
            ctx.attrs(&["lakes", &group], &[])
        );
    }
    s.push_str("</g>\n");
}

/// The coastline of each island (`drawCoastline`).
pub fn coastline(s: &mut String, ctx: &Ctx) {
    let _ = writeln!(
        s,
        "<g id=\"coastline\" fill=\"none\" stroke-linejoin=\"round\"{}>",
        ctx.attrs(&["coastline"], &[])
    );
    for group in ["sea_island", "lake_island"] {
        let uses: String = ctx
            .outlines
            .iter()
            .filter(|o| !o.lake && o.in_lake == (group == "lake_island"))
            .map(|o| format!("<use href=\"#feature-{}\"/>", o.feature))
            .collect();
        let _ = writeln!(
            s,
            "<g id=\"coastline-{}\"{}>{uses}</g>",
            super::super::icons::kebab(group),
            ctx.attrs(&["coastline", group], &[])
        );
    }
    s.push_str("</g>\n");
}

/// Height shading: a filled shape for each step of height
/// (`drawHeightmap`).
pub fn heightmap(s: &mut String, ctx: &Ctx) {
    let w = ctx.world;
    // The original program draws the heights of the first grid.
    let g = &w.base.grid;
    let st = &ctx.style;
    // Heights on the scale of the original program: sea level is 20.
    let h = |c: usize| {
        (w.base.heights[c] - w.sea_level + 20.0)
            .clamp(0.0, 100.0)
            .round()
    };
    let _ = writeln!(
        s,
        "<g id=\"terrs\" fill-rule=\"evenodd\"{}>",
        ctx.attrs(&["heightmap"], &[])
    );
    for (group, ocean) in [("oceanHeights", true), ("landHeights", false)] {
        let p = ["heightmap", group];
        if ocean && !st.option_bool(&p, "render") {
            continue;
        }
        let contours = st.option_str(&p, "contours.mode").unwrap_or_default();
        let hachures = st.option_str(&p, "hachures.mode").unwrap_or_default();
        if !ocean && !matches!(contours.as_str(), "" | "off") {
            contour_lines(s, ctx, &p, &h);
        }
        let draw_hachures = !matches!(hachures.as_str(), "" | "off");
        if contours == "only" || hachures == "only" {
            if draw_hachures {
                height_hachures(s, ctx, &p, ocean);
            }
            continue;
        }
        let scheme = height_scheme(
            &st.option_str(&p, "scheme")
                .unwrap_or_else(|| "bright".into()),
        );
        let skip = st.option_f64(&p, "skip").unwrap_or(0.0).max(0.0) as u32 + 1;
        let terracing = st.option_f64(&p, "terracing").unwrap_or(0.0) / 10.0;
        let _ = writeln!(
            s,
            "<g id=\"{}\"{}>",
            super::super::icons::kebab(group),
            ctx.attrs(&p, &[])
        );
        let (lo, hi) = if ocean {
            (1u32, 19u32)
        } else {
            (20u32, 100u32)
        };
        if ocean {
            let _ = writeln!(
                s,
                "<rect {} fill=\"{}\"/>",
                ctx.map_rect_attrs(10.0),
                super::super::color::rgb_basis(&scheme, 1.0).hex()
            );
        } else {
            let _ = writeln!(
                s,
                "<rect {} fill=\"{}\"/>",
                ctx.map_rect_attrs(10.0),
                super::super::color::rgb_basis(&scheme, 0.8).hex()
            );
        }
        let mut level = lo;
        while level <= hi {
            let lv = f64::from(level);
            let rings = geom::rings_on(g, &|c| u32::from(h(c) >= lv));
            let mut d = String::new();
            for ring in rings.get(&1).into_iter().flatten() {
                let pts = relax(ctx, g, ring, 1);
                if pts.len() >= 3 {
                    d.push_str(&basis_closed(&pts));
                }
            }
            if d.len() >= 10 {
                let fill = height_color(lv, &scheme);
                if terracing > 0.0 {
                    let _ = writeln!(
                        s,
                        "<path d=\"{d}\" transform=\"translate(.7,1.4)\" fill=\"{}\"/>",
                        fill.darker(terracing).hex()
                    );
                }
                let _ = writeln!(s, "<path d=\"{d}\" fill=\"{}\"/>", fill.hex());
            }
            level += skip;
        }
        s.push_str("</g>\n");
        if draw_hachures {
            height_hachures(s, ctx, &p, ocean);
        }
    }
    s.push_str("</g>\n");
}

/// Hachures down the slopes of the land or of the sea floor
/// (`drawHeightmap` with `getHachures`).
fn height_hachures(s: &mut String, ctx: &Ctx, p: &[&str], ocean: bool) {
    let w = ctx.world;
    let g = &w.base.grid;
    let st = &ctx.style;
    let px = ctx.px;
    let heights: Vec<f64> = (0..g.len())
        .map(|c| {
            (w.base.heights[c] - w.sea_level + 20.0)
                .clamp(0.0, 100.0)
                .round()
        })
        .collect();
    let neighbors: Vec<Vec<usize>> = g
        .edge_cells
        .iter()
        .map(|e| {
            e.iter()
                .filter(|&&o| o != OUTSIDE)
                .map(|&o| o as usize)
                .collect()
        })
        .collect();
    // The three cells around each inner vertex.
    let mut around: Vec<Vec<usize>> = vec![Vec::new(); g.vertices.len()];
    for (c, poly) in g.polygons.iter().enumerate() {
        for &v in poly {
            around[v as usize].push(c);
        }
    }
    let triangles: Vec<[usize; 3]> = around
        .iter()
        .filter_map(|cells| <[usize; 3]>::try_from(cells.as_slice()).ok())
        .collect();
    // Smoothed twice: a calm fall line.
    let smooth =
        super::hachures::smooth(&super::hachures::smooth(&heights, &neighbors), &neighbors);
    let points: Vec<[f64; 2]> = g.points.iter().map(|&q| ctx.p(q)).collect();
    let k = px.from_world_px([1.0, 0.0])[0] - px.from_world_px([0.0, 0.0])[0];
    let find = |x: f64, y: f64| {
        let q = px.page.to_world([x * px.mm, y * px.mm]);
        let inside = (0.0..=g.width).contains(&q[0]) && (0.0..=g.height).contains(&q[1]);
        inside.then(|| g.find_cell(q[0], q[1]))
    };
    let thresholds: Vec<f64> = if ocean {
        (1..5).rev().map(|v| f64::from(v) * 4.0).collect()
    } else {
        (6..25).map(|v| f64::from(v) * 4.0).collect()
    };
    let in_band = |h: f64| if ocean { h < 20.0 } else { h >= 20.0 };
    let params = super::hachures::Params {
        points: &points,
        heights: &smooth,
        neighbors: &neighbors,
        triangles: &triangles,
        spacing: g.lattice.spacing * px.unit_px * k,
        find_cell: &find,
        in_band: &in_band,
        thresholds: &thresholds,
        density: st.option_f64(p, "hachures.density").unwrap_or(1.0),
        length: st.option_f64(p, "hachures.length").unwrap_or(1.0),
        width: st.option_f64(p, "hachures.width").unwrap_or(1.0),
        unit: k,
    };
    let part = if ocean {
        "hachures-ocean"
    } else {
        "hachures-land"
    };
    let mut rng = Rand::new(SeedTree::new(ctx.config.seed).part_rng(Layer::Decoration, part));
    let d = super::hachures::hachures(&params, &mut rng);
    if d.is_empty() {
        return;
    }
    let _ = writeln!(
        s,
        "<g class=\"heightmap-hachures\" fill=\"{}\" fill-rule=\"nonzero\" opacity=\"{}\" mask=\"url(#{})\"><path d=\"{d}\"/></g>",
        crate::extras::xml(
            &st.option_str(p, "hachures.color")
                .unwrap_or_else(|| "#000000".into())
        ),
        n(st.option_f64(p, "hachures.opacity").unwrap_or(0.65)),
        if ocean { "water" } else { "land" }
    );
}

/// Contour lines of the land heights (`heightmap-contours.ts`, simpler):
/// a line at each fourth height unit, and a wider line at each twentieth.
fn contour_lines(s: &mut String, ctx: &Ctx, p: &[&str], h: &dyn Fn(usize) -> f64) {
    let g = &ctx.world.base.grid;
    let st = &ctx.style;
    let color = st
        .option_str(p, "contours.color")
        .unwrap_or_else(|| "#5c513e".into());
    let width = st.option_f64(p, "contours.width").unwrap_or(0.35);
    let opacity = st.option_f64(p, "contours.opacity").unwrap_or(0.5);
    let _ = writeln!(
        s,
        "<g id=\"heightmap-contours\" fill=\"none\" stroke=\"{}\" stroke-linejoin=\"round\" stroke-linecap=\"round\" opacity=\"{}\" mask=\"url(#land)\">",
        crate::extras::xml(&color),
        inkland_core::num::fixed(opacity, 3)
    );
    let mut level = 24u32;
    while level < 100 {
        let lv = f64::from(level);
        let rings = geom::rings_on(g, &|c| u32::from(h(c) >= lv));
        let mut d = String::new();
        for ring in rings.get(&1).into_iter().flatten() {
            let pts = relax(ctx, g, ring, 1);
            if pts.len() >= 3 {
                d.push_str(&basis_closed(&pts));
            }
        }
        if !d.is_empty() {
            let major = level.is_multiple_of(20);
            let _ = writeln!(
                s,
                "<path d=\"{d}\" stroke-width=\"{}\"/>",
                n(width * if major { 2.0 } else { 1.0 })
            );
        }
        level += 4;
    }
    s.push_str("</g>\n");
}
