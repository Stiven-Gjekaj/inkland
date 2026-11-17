//! Icon layers: relief icons, place icons, markers, emblems, ice, and the
//! overlay features of inkland (`draw-relief-icons.ts`,
//! `relief-generator.ts`, `draw-burg-icons.ts`, `draw-markers.ts`,
//! `draw-emblems.ts`, `draw-ice.ts`).

use super::super::Ctx;
use super::super::geom::n;
use super::super::icons;
use inkland_core::config::PlaceKind;
use inkland_core::rand::Rand;
use inkland_core::seed::{Layer, SeedTree};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// The relief pool of each biome: the density and the weighted types
/// (`biomes-generator.ts`).
const POOLS: [(f64, &[(&str, f64)]); 13] = [
    (0.0, &[]),
    (27.0, &[("dune", 3.0), ("cactus", 6.0), ("deadTree", 1.0)]),
    (20.0, &[("dune", 9.0), ("deadTree", 1.0)]),
    (120.0, &[("acacia", 1.0), ("grass", 9.0)]),
    (120.0, &[("grass", 1.0)]),
    (120.0, &[("acacia", 8.0), ("palm", 1.0)]),
    (120.0, &[("deciduous", 2.0), ("conifer", 1.0)]),
    (
        150.0,
        &[
            ("acacia", 5.0),
            ("palm", 3.0),
            ("deciduous", 1.0),
            ("swamp", 1.0),
        ],
    ),
    (
        150.0,
        &[("deciduous", 6.0), ("conifer", 1.0), ("swamp", 1.0)],
    ),
    (100.0, &[("coniferSnow", 1.0)]),
    (39.0, &[("grass", 1.0)]),
    (0.0, &[]),
    (250.0, &[("swamp", 1.0)]),
];

/// A relief rule (`getDefaultRules`): heights, the highest temperature, the
/// icon sizes, and the type.
struct Rule {
    height: [f64; 2],
    temp_max: Option<f64>,
    size: [f64; 2],
    kind: &'static str,
}

const RULES: [Rule; 3] = [
    Rule {
        height: [71.0, 100.0],
        temp_max: Some(-1.0),
        size: [20.8, 44.0],
        kind: "mountSnow",
    },
    Rule {
        height: [71.0, 100.0],
        temp_max: None,
        size: [20.8, 44.0],
        kind: "mount",
    },
    Rule {
        height: [50.0, 70.0],
        temp_max: None,
        size: [8.0, 12.0],
        kind: "hill",
    },
];

/// Points in square buckets, for a fast distance test.
type Buckets = BTreeMap<(i64, i64), Vec<[f64; 2]>>;

/// One relief icon: its type, its variant, its box `x y s` in world pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct ReliefIcon {
    pub kind: &'static str,
    pub variant: u32,
    pub x: f64,
    pub y: f64,
    pub s: f64,
}

/// Points of a Poisson-disc sampling in a box (mbostock's sampler).
fn poisson(x0: f64, y0: f64, x1: f64, y1: f64, r: f64, rng: &mut Rand) -> Vec<[f64; 2]> {
    let (width, height) = (x1 - x0, y1 - y0);
    if !(width >= 0.0 && height >= 0.0 && r > 0.0) {
        return Vec::new();
    }
    let r2 = r * r;
    let r2_3 = 3.0 * r2;
    let cell = r * std::f64::consts::FRAC_1_SQRT_2;
    let gw = ((width / cell).ceil() as usize).max(1);
    let gh = ((height / cell).ceil() as usize).max(1);
    let mut grid: Vec<Option<[f64; 2]>> = vec![None; gw * gh];
    let mut queue: Vec<[f64; 2]> = Vec::new();
    let mut out = Vec::new();
    let far = |grid: &[Option<[f64; 2]>], x: f64, y: f64| {
        let i = (x / cell) as usize;
        let j = (y / cell) as usize;
        for jj in j.saturating_sub(2)..(j + 3).min(gh) {
            for ii in i.saturating_sub(2)..(i + 3).min(gw) {
                if let Some(s) = grid[jj * gw + ii] {
                    let (dx, dy) = (s[0] - x, s[1] - y);
                    if dx * dx + dy * dy < r2 {
                        return false;
                    }
                }
            }
        }
        true
    };
    let mut sample =
        |grid: &mut Vec<Option<[f64; 2]>>, queue: &mut Vec<[f64; 2]>, x: f64, y: f64| {
            let i = ((x / cell) as usize).min(gw - 1);
            let j = ((y / cell) as usize).min(gh - 1);
            grid[j * gw + i] = Some([x, y]);
            queue.push([x, y]);
            out.push([x + x0, y + y0]);
        };
    sample(&mut grid, &mut queue, width / 2.0, height / 2.0);
    'pick: while !queue.is_empty() {
        let i = ((rng.unit() * queue.len() as f64) as usize).min(queue.len() - 1);
        let parent = queue[i];
        for _ in 0..3 {
            let a = std::f64::consts::TAU * rng.unit();
            let rr = (rng.unit() * r2_3 + r2).sqrt();
            let x = parent[0] + rr * libm::cos(a);
            let y = parent[1] + rr * libm::sin(a);
            if (0.0..width).contains(&x) && (0.0..height).contains(&y) && far(&grid, x, y) {
                sample(&mut grid, &mut queue, x, y);
                continue 'pick;
            }
        }
        let last = queue.pop().expect("not empty");
        if i < queue.len() {
            queue[i] = last;
        }
    }
    out
}

/// `true` if a point is inside a polygon (even-odd rule).
fn contains(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    let k = poly.len();
    for i in 0..k {
        let a = poly[i];
        let b = poly[(i + k - 1) % k];
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

/// Place the relief icons of a world (`Relief.generate`). The icons are in
/// world pixels.
pub fn relief_icons(ctx: &Ctx) -> Vec<ReliefIcon> {
    let w = ctx.world;
    let g = &w.grid;
    let up = ctx.px.unit_px;
    let density_opt = ctx
        .style
        .option_f64(&["relief"], "density")
        .unwrap_or(0.4)
        .max(0.01);
    let spacing = |density: f64| 2.0 / (density / 100.0) / density_opt;
    let mut river = vec![false; g.len()];
    for r in &w.rivers {
        for &c in &r.cells {
            river[c] = true;
        }
    }
    let mut rng = Rand::new(SeedTree::new(w.seed).part_rng(Layer::Decoration, "fmg-relief"));
    let mut order: Vec<(u64, usize)> = (0..g.len())
        .map(|c| ((rng.unit() * (1u64 << 52) as f64) as u64, c))
        .collect();
    order.sort();
    let [vx0, vy0, vx1, vy1] = ctx.px.page.view;
    // Placed points of each pool, in buckets, for the spacing test.
    let mut placed: BTreeMap<usize, Buckets> = BTreeMap::new();
    let mut out = Vec::new();
    for (_, c) in order {
        if !w.features.is_land(c) || river[c] {
            continue;
        }
        let site = g.points[c];
        let margin = g.step_x.max(g.step_y) * 3.0;
        if site[0] < vx0 - margin
            || site[0] > vx1 + margin
            || site[1] < vy0 - margin
            || site[1] > vy1 + margin
        {
            continue;
        }
        let h = (w.heights[c] - w.sea_level + 20.0).round();
        let t = w.temperature[c];
        let rule = RULES.iter().position(|r| {
            h >= r.height[0] && h <= r.height[1] && r.temp_max.is_none_or(|m| t <= m)
        });
        let biome = w.biomes[c] as usize;
        let (pool_key, density) = match rule {
            Some(k) => (100 + k, 100.0),
            None => (biome, POOLS.get(biome).map_or(0.0, |p| p.0)),
        };
        if density <= 0.0 {
            continue;
        }
        if rule.is_none() && POOLS[biome].1.is_empty() {
            continue;
        }
        let radius = spacing(density);
        let poly: Vec<[f64; 2]> = g.polygons[c]
            .iter()
            .map(|&v| {
                let p = g.vertices[v as usize];
                [p[0] * up, p[1] * up]
            })
            .collect();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in &poly {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        let tree = placed.entry(pool_key).or_default();
        let bucket = |p: [f64; 2]| {
            (
                (p[0] / radius).floor() as i64,
                (p[1] / radius).floor() as i64,
            )
        };
        let mut points = Vec::new();
        for p in poisson(x0, y0, x1, y1, radius, &mut rng) {
            if !contains(&poly, p) {
                continue;
            }
            let (bx, by) = bucket(p);
            let near = (-1..=1).any(|dx| {
                (-1..=1).any(|dy| {
                    tree.get(&(bx + dx, by + dy)).is_some_and(|list| {
                        list.iter().any(|q| {
                            (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) < radius * radius
                        })
                    })
                })
            });
            if !near {
                points.push(p);
            }
        }
        for &p in &points {
            tree.entry(bucket(p)).or_default().push(p);
        }
        if points.is_empty() {
            continue;
        }
        match rule {
            Some(k) => {
                let r = &RULES[k];
                let kind = r.kind;
                let variant = 1 + (rng.unit() * f64::from(icons::variants(kind))) as u32;
                let s = (r.size[0] + 0.8 * (h - r.height[0])).clamp(r.size[0], r.size[1]);
                for p in points {
                    out.push(ReliefIcon {
                        kind,
                        variant,
                        x: p[0] - s / 2.0,
                        y: p[1] - s / 2.0,
                        s,
                    });
                }
            }
            None => {
                let pool = POOLS[biome].1;
                let total: f64 = pool.iter().map(|e| e.1).sum();
                for p in points {
                    let s = (4.0 + rng.unit()) * 4.0;
                    let target = rng.unit() * total;
                    let mut acc = 0.0;
                    let mut kind = pool[0].0;
                    for (name, weight) in pool {
                        acc += weight;
                        if target < acc {
                            kind = name;
                            break;
                        }
                    }
                    if kind == "conifer" && t < 0.0 {
                        kind = "coniferSnow";
                    }
                    let variant = 1 + (rng.unit() * f64::from(icons::variants(kind))) as u32;
                    out.push(ReliefIcon {
                        kind,
                        variant,
                        x: p[0] - s / 2.0,
                        y: p[1] - s / 2.0,
                        s,
                    });
                }
            }
        }
    }
    out
}

/// Relief icons (`drawRelief`). An icon under a label is left out, as in
/// `print-safe` mode.
pub fn relief(s: &mut String, ctx: &Ctx) {
    let set = ctx
        .style
        .option_str(&["relief"], "set")
        .unwrap_or_else(|| "simple".into());
    let size = ctx.style.option_f64(&["relief"], "size").unwrap_or(1.0);
    let [mx, my, mw, mh] = ctx.px.map_rect();
    let mut items: Vec<(f64, String)> = Vec::new();
    for icon in relief_icons(ctx) {
        let drawn = icon.s * size;
        // World pixels and page pixels have the same scale.
        let centre = ctx
            .px
            .from_world_px([icon.x + icon.s / 2.0, icon.y + icon.s / 2.0]);
        let left = centre[0] - drawn / 2.0;
        let top = centre[1] - drawn / 2.0;
        if left > mx + mw || top > my + mh || left + drawn < mx || top + drawn < my {
            continue;
        }
        let mm = [centre[0] * ctx.px.mm, centre[1] * ctx.px.mm];
        if ctx.labels.covers(mm, 0.5) {
            continue;
        }
        let Some(file) = icons::relief_file(&set, icon.kind, icon.variant) else {
            continue;
        };
        items.push((
            top + drawn,
            format!(
                "<use href=\"#{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                icons::kebab(&file),
                n(left),
                n(top),
                n(drawn),
                n(drawn)
            ),
        ));
    }
    // An icon whose box ends lower draws on top.
    items.sort_by(|a, b| a.0.total_cmp(&b.0));
    let _ = writeln!(s, "<g id=\"terrain\"{}>", ctx.attrs(&["relief"], &[]));
    for (_, u) in items {
        s.push_str(&u);
        s.push('\n');
    }
    s.push_str("</g>\n");
}

/// The style group of a place.
pub fn burg_group(kind: PlaceKind, capital: bool) -> &'static str {
    if capital {
        return "capital";
    }
    match kind {
        PlaceKind::City => "city",
        PlaceKind::Town | PlaceKind::Custom => "town",
        PlaceKind::Village => "village",
        PlaceKind::Fortress => "fort",
        PlaceKind::Ruin => "hamlet",
    }
}

/// The size of one icon em in pixels for an icon size of 1 %. The place
/// labels of inkland are larger than at the scale of the original program,
/// so the icons grow with them. The original program also keeps its icons
/// larger than its labels when the map is zoomed in; the factor 2 gives
/// this look on a page.
pub fn burg_em(ctx: &Ctx) -> f64 {
    let city = crate::page::pt(ctx.config.render.font_sizes_pt.place_city);
    let pct = ctx
        .style
        .attr_f64(&["labels", "capital"], "font-size")
        .filter(|v| *v > 0.0)
        .unwrap_or(6.0);
    2.0 * ctx.px.of_mm(city) / pct
}

/// Place icons, in groups by kind, and port anchors (`drawBurgIcons`).
pub fn burg_icons(s: &mut String, ctx: &Ctx) {
    let w = ctx.world;
    let em1 = burg_em(ctx);
    let [vx0, vy0, vx1, vy1] = ctx.px.page.view;
    let _ = writeln!(s, "<g id=\"burg-icons\"{}>", ctx.attrs(&["burgIcons"], &[]));
    for group in ["capital", "city", "town", "village", "fort", "hamlet"] {
        let places: Vec<_> = w
            .society
            .places
            .iter()
            .filter(|p| burg_group(p.kind, p.capital) == group)
            .filter(|p| p.at[0] >= vx0 && p.at[0] <= vx1 && p.at[1] >= vy0 && p.at[1] <= vy1)
            .collect();
        let gid = icons::kebab(group);
        let _ = writeln!(s, "<g id=\"burg-icons-{gid}\">");
        for (part, anchors) in [("icons", false), ("anchors", true)] {
            let path = ["burgIcons", group, part];
            let id = ctx.style.option_str(&path, "icon").unwrap_or_default();
            let Some(file) = icons::place_icon_path(&id) else {
                continue;
            };
            let size = ctx.style.option_f64(&path, "size").unwrap_or(1.0);
            let [bx, by, bw, bh] = icons::anchored_box(&file);
            let em = size * em1;
            let _ = writeln!(s, "<g{}>", ctx.attrs(&path, &[]));
            for p in &places {
                if anchors && !p.port {
                    continue;
                }
                let [x, y] = ctx.p(p.at);
                let _ = writeln!(
                    s,
                    "<use href=\"#{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                    icons::kebab(&file),
                    n(x + bx * em),
                    n(y + by * em),
                    n(bw * em),
                    n(bh * em)
                );
            }
            s.push_str("</g>\n");
        }
        s.push_str("</g>\n");
    }
    s.push_str("</g>\n");
}

/// The symbols that the body uses.
pub fn symbol_defs(d: &mut String, refs: &BTreeSet<String>) {
    let mut by_id: BTreeMap<String, &str> = BTreeMap::new();
    for (path, _) in super::super::icons_data::FILES {
        by_id.insert(icons::kebab(path), path);
    }
    for r in refs {
        if let Some(path) = by_id.get(r)
            && let Some(sym) = icons::symbol(path, r)
        {
            d.push_str(&sym);
            d.push('\n');
        }
    }
}

/// Glaciers and icebergs (`drawIce`). The world has no ice yet; see
/// `data::ice`.
pub fn ice(s: &mut String, ctx: &Ctx) {
    if let Some(list) = super::super::data::ice(ctx.world) {
        write_ice(s, ctx, &list);
    }
}

/// Write ice.
pub fn write_ice(s: &mut String, ctx: &Ctx, list: &[super::super::data::Ice]) {
    let _ = writeln!(s, "<g id=\"ice\"{}>", ctx.attrs(&["ice"], &[]));
    for ice in list {
        let pts: Vec<String> = ice
            .points
            .iter()
            .map(|&p| {
                let q = ctx.p(p);
                format!("{},{}", n(q[0]), n(q[1]))
            })
            .collect();
        let kind = if ice.glacier { " type=\"glacier\"" } else { "" };
        let _ = writeln!(
            s,
            "<polygon id=\"ice-{}\" points=\"{}\"{kind}/>",
            ice.id,
            pts.join(" ")
        );
    }
    s.push_str("</g>\n");
}

/// Coats of arms (`drawEmblems`). The world has no emblems yet; see
/// `data::emblems`.
pub fn emblems(s: &mut String, ctx: &Ctx) {
    if let Some(list) = super::super::data::emblems(ctx.world) {
        write_emblems(s, ctx, &list);
    }
}

/// Write coats of arms, in their groups.
pub fn write_emblems(s: &mut String, ctx: &Ctx, list: &[super::super::data::Emblem]) {
    let em = burg_em(ctx);
    let _ = writeln!(s, "<g id=\"emblems\"{}>", ctx.attrs(&["emblems"], &[]));
    for (group, base) in [("burg", 2.0), ("province", 3.5), ("state", 5.0)] {
        let key = format!("{group}Emblems");
        let size = ctx
            .style
            .option_f64(&["emblems", &key], "size")
            .unwrap_or(1.0)
            * base
            * em;
        let _ = writeln!(
            s,
            "<g id=\"{group}-emblems\"{}>",
            ctx.attrs(&["emblems", &key], &[])
        );
        for e in list.iter().filter(|e| e.group == group) {
            let [x, y] = ctx.p(e.at);
            let _ = writeln!(
                s,
                "<svg id=\"emblem-{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 200 200\">{}</svg>",
                e.id,
                n(x - size / 2.0),
                n(y - size / 2.0),
                n(size),
                n(size),
                e.svg
            );
        }
        s.push_str("</g>\n");
    }
    s.push_str("</g>\n");
}

/// Markers (`drawMarkers`). The world has no markers yet; see
/// `data::markers`.
pub fn markers(s: &mut String, ctx: &Ctx) {
    if let Some(list) = super::super::data::markers(ctx.world) {
        write_markers(s, ctx, &list);
    }
}

/// Write markers: a bubble pin with an icon.
pub fn write_markers(s: &mut String, ctx: &Ctx, list: &[super::super::data::Marker]) {
    let font = burg_em(ctx) * 100.0 / 8.0;
    let _ = writeln!(s, "<g id=\"markers\"{}>", ctx.attrs(&["markers"], &[]));
    for m in list {
        let size = m.size / 100.0 * font;
        let [x, y] = ctx.p(m.at);
        let (fill, stroke) = (crate::extras::xml(&m.fill), crate::extras::xml(&m.stroke));
        let _ = writeln!(
            s,
            "<svg id=\"marker-{}\" viewBox=\"0 0 30 30\" width=\"{}\" height=\"{}\" x=\"{}\" y=\"{}\" overflow=\"visible\"><path d=\"M6,19 l9,10 L24,19\" fill=\"{stroke}\" stroke=\"none\"/><circle cx=\"15\" cy=\"15\" r=\"10\" fill=\"{fill}\" stroke=\"{stroke}\"/><text x=\"15\" y=\"15\" font-size=\"12\" text-anchor=\"middle\" dominant-baseline=\"central\">{}</text></svg>",
            m.id,
            n(size),
            n(size),
            n(x - size / 2.0),
            n(y - size),
            crate::extras::xml(&m.icon)
        );
    }
    s.push_str("</g>\n");
}

/// The overlay features of inkland, with the colours of the borders and
/// markers of the style.
pub fn features(s: &mut String, ctx: &Ctx) {
    use inkland_core::config::FeatureKind;
    let Some(overlay) = crate::labels::overlay_of(ctx.config, ctx.variant) else {
        return;
    };
    let mut feats: Vec<_> = overlay
        .add
        .features
        .iter()
        .filter(|f| !inkland_core::overlay::removes(overlay, "feature", &f.id))
        .collect();
    if feats.is_empty() {
        return;
    }
    feats.sort_by(|a, b| a.id.cmp(&b.id));
    let stroke = ctx
        .style
        .attr(&["borders", "stateBorders"], "stroke")
        .unwrap_or_else(|| "#56566d".into());
    let _ = writeln!(
        s,
        "<g id=\"features\" fill=\"none\" stroke=\"{stroke}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">"
    );
    let em = burg_em(ctx);
    for f in feats {
        let (width, dash) = match f.kind {
            FeatureKind::Border => (1.0, " stroke-dasharray=\"4 1.5 1 1.5\""),
            FeatureKind::Wall => (2.0, ""),
            _ => (0.8, " stroke-dasharray=\"1 1.5\""),
        };
        let id = format!("feature-{}", f.id);
        if let Some(at) = f.at {
            let [x, y] = ctx.p(at);
            let r = 1.2 * em;
            let d = match f.kind {
                FeatureKind::Ruin => format!(
                    "M{},{}l{},{}m0,{}l{},{}",
                    n(x - r),
                    n(y - r),
                    n(2.0 * r),
                    n(2.0 * r),
                    n(-2.0 * r),
                    n(-2.0 * r),
                    n(2.0 * r)
                ),
                _ => format!(
                    "M{},{}l{},{}h{}z",
                    n(x),
                    n(y - r),
                    n(r),
                    n(1.8 * r),
                    n(-2.0 * r)
                ),
            };
            let _ = writeln!(
                s,
                "<path id=\"overlay-{id}\" d=\"{d}\" stroke-width=\"{}\"/>",
                n(0.4 * em)
            );
        } else if let Some(line) = &f.polyline {
            let pts: Vec<[f64; 2]> = line.iter().map(|&p| ctx.p(p)).collect();
            let _ = writeln!(
                s,
                "<path id=\"overlay-{id}\" d=\"{}\" stroke-width=\"{}\"{dash}/>",
                super::super::geom::line_path(&pts),
                n(width)
            );
        } else if let Some(poly) = &f.polygon {
            let pts: Vec<[f64; 2]> = poly.iter().map(|&p| ctx.p(p)).collect();
            let _ = writeln!(
                s,
                "<path id=\"overlay-{id}\" d=\"{}\" stroke-width=\"{}\"{dash}/>",
                super::super::geom::polygon_path(&pts),
                n(width)
            );
        }
    }
    s.push_str("</g>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisson_points_keep_their_distance() {
        let mut rng = Rand::new(SeedTree::new(5).part_rng(Layer::Decoration, "t"));
        let pts = poisson(0.0, 0.0, 50.0, 30.0, 4.0, &mut rng);
        assert!(pts.len() > 10);
        for (i, a) in pts.iter().enumerate() {
            for b in &pts[i + 1..] {
                assert!((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) >= 16.0 - 1e-9);
            }
        }
    }

    #[test]
    fn contains_finds_inside_and_outside() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        assert!(contains(&sq, [5.0, 5.0]));
        assert!(!contains(&sq, [15.0, 5.0]));
    }
}
