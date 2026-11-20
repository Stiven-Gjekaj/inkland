//! The SVG of a town map.
//!
//! The drawing follows `com.watabou.towngenerator.mapping.CityMap` of
//! TownGeneratorOS by Watabou (GPL-3.0,
//! <https://github.com/watabou/TownGeneratorOS>): the roads, the buildings
//! of each ward, then the walls with their gates and towers. `colour` uses
//! the default palette of the original. `print-safe` is black and white:
//! white buildings with black outlines.

use crate::page::line_mm;
use inkland_core::town::{Kind, P, Town, TownWall};
use std::fmt::Write as _;

/// A normal stroke of the original, in town units.
const NORMAL_STROKE: f64 = 0.3;
/// A thick stroke of the original: the walls.
const THICK_STROKE: f64 = 1.8;
/// The width of a main street, in town units.
const MAIN_STREET: f64 = 2.0;
/// The height of the title band above the map, in mm.
const TITLE_MM: f64 = 12.0;
/// The part of the town radius that the map shows around the centre.
const VIEW: f64 = 1.25;

/// The look of a town map.
#[derive(Debug, Clone, PartialEq)]
pub struct TownStyle {
    /// `true`: the colours of the original. `false`: black and white.
    pub colour: bool,
    /// The width of the map, in mm. The page is this wide, and the title
    /// band is above the map.
    pub size_mm: f64,
    /// The font family of the title.
    pub font: String,
    /// The least width of a line, in points.
    pub min_line_pt: f64,
    /// inkland: the hand of the hand-drawn look. In black and white it
    /// moves the walls, the shore, the river, the moat, and the roads a
    /// little, as a pen line. `None`: exact lines.
    pub hand: Option<crate::hand::Hand>,
    /// The name of the distance unit of the world, for example `mile`. A
    /// scale bar in feet goes with an imperial unit, and one in metres goes
    /// with all other units.
    pub unit: String,
}

/// inkland: the length of a town unit, in metres. A main street is then
/// 6 m wide.
const METRES_PER_UNIT: f64 = 3.0;

/// inkland: the unit of the scale bar of a town map for a world unit: its
/// name and its length in metres.
fn bar_unit(world_unit: &str) -> (&'static str, f64) {
    let u = world_unit.to_lowercase();
    if ["mile", "league", "foot", "feet", "yard", "furlong"]
        .iter()
        .any(|w| u.contains(w))
    {
        ("feet", 0.3048)
    } else {
        ("metres", 1.0)
    }
}

/// The four colours of the palette: paper, light, medium, and dark.
fn palette(colour: bool) -> [&'static str; 4] {
    if colour {
        ["#ccc5b8", "#99948a", "#67635c", "#1a1917"]
    } else {
        ["#ffffff", "#ffffff", "#000000", "#000000"]
    }
}

/// inkland: the colour of the water. Black and white has white water with
/// a black shore.
fn water_colour(colour: bool) -> &'static str {
    if colour { "#9fadab" } else { "#ffffff" }
}

/// inkland: the width of a river, in town units.
const RIVER: f64 = 4.0;

/// inkland: the distance between the lines of the sea in black and white,
/// in mm.
const HATCH_MM: f64 = 1.2;

/// inkland: the width of a moat, in town units.
const MOAT: f64 = 3.0;
/// inkland: the distance from the wall to the middle of the moat.
const MOAT_OFFSET: f64 = 4.5;

/// inkland: each vertex of a polygon moved out by `d`, so that each edge
/// is `d` from its old place. Out is away from the centre of the polygon.
fn outset(shape: &[P], d: f64) -> Vec<P> {
    use inkland_core::town::poly;
    let m = shape.len();
    let c = poly::center(shape);
    let normal = |i: usize| {
        let (a, b) = (shape[i], shape[(i + 1) % m]);
        let nrm = poly::norm(poly::rotate90(poly::sub(b, a)), 1.0);
        let mid = poly::scale(poly::add(a, b), 0.5);
        if poly::dot(nrm, poly::sub(mid, c)) < 0.0 {
            poly::scale(nrm, -1.0)
        } else {
            nrm
        }
    };
    (0..m)
        .map(|i| {
            let (n0, n1) = (normal((i + m - 1) % m), normal(i));
            let sum = poly::add(n0, n1);
            let l = poly::len(sum);
            if l < 1e-9 {
                return poly::add(shape[i], poly::scale(n1, d));
            }
            // The miter: the bisector, longer at a sharp corner, at most 2 d.
            let cos = (l / 2.0).max(0.5);
            poly::add(shape[i], poly::scale(sum, d / cos / l))
        })
        .collect()
}

/// inkland: the colour of a lawn in a park.
const LAWN_COLOUR: &str = "#b5ae9e";
/// inkland: the distance between two trees of a park, in town units.
const TREE_STEP: f64 = 3.2;

/// inkland: the trees of a park, as `[x, y, radius]` in town units: a
/// grid with a fixed jitter, inside the lawns.
fn park_trees(key: usize, lawns: &[inkland_core::town::Poly]) -> Vec<[f64; 3]> {
    let mix = |mut z: u64| {
        z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    let unit = |h: u64| (h >> 11) as f64 / (1u64 << 53) as f64;
    let pts = lawns.iter().flatten();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in pts {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let mut out = Vec::new();
    let mut row = 0u64;
    let mut y = y0 + TREE_STEP / 2.0;
    while y < y1 {
        let mut col = 0u64;
        // Each second row moves half a step, as trees in a wood.
        let mut x = x0 + TREE_STEP * if row.is_multiple_of(2) { 0.5 } else { 1.0 };
        while x < x1 {
            let h = mix((key as u64) << 32 ^ row << 16 ^ col);
            let p = [
                x + (unit(h) - 0.5) * TREE_STEP * 0.5,
                y + (unit(mix(h)) - 0.5) * TREE_STEP * 0.5,
            ];
            let r = TREE_STEP * (0.35 + 0.15 * unit(mix(mix(h))));
            if lawns.iter().any(|l| {
                inkland_core::town::districts::contains(
                    &inkland_core::town::poly::shrink_eq(l, r),
                    p,
                )
            }) {
                out.push([p[0], p[1], r]);
            }
            x += TREE_STEP;
            col += 1;
        }
        y += TREE_STEP * 0.87;
        row += 1;
    }
    out
}

/// inkland: the offset of the shadow of a building, in town units.
const SHADOW: f64 = 0.7;
/// inkland: the colour of the shadows, between the paper and the light
/// colour.
const SHADOW_COLOUR: &str = "#aea79a";

fn n(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    let r = if r == 0.0 { 0.0 } else { r };
    format!("{r}")
}

/// The SVG of a town map with its name in the title.
pub fn town_svg(town: &Town, name: &str, style: &TownStyle) -> String {
    let [paper, light, medium, dark] = palette(style.colour);
    let size = style.size_mm;
    let r = (town.radius * VIEW).max(1.0);
    let k = size / (2.0 * r);
    let top = TITLE_MM;
    let to = |p: P| -> [f64; 2] { [(p[0] + r) * k, (p[1] + r) * k + top] };
    let min = line_mm(style.min_line_pt);
    let w = |units: f64| (units * k).max(min);
    let path = |pts: &[P], close: bool| -> String {
        let mut d = String::new();
        for (i, p) in pts.iter().enumerate() {
            let [x, y] = to(*p);
            let _ = write!(d, "{}{} {}", if i == 0 { "M" } else { "L" }, n(x), n(y));
        }
        if close {
            d.push('Z');
        }
        d
    };
    // inkland: an open line, wobbled in black and white with a hand.
    let hand = style.hand.filter(|_| !style.colour);
    let pen = |key: &str, pts: &[P]| -> String {
        let Some(h) = hand else {
            return path(pts, false);
        };
        let page: Vec<[f64; 2]> = pts.iter().map(|&p| to(p)).collect();
        let mut d = String::new();
        for (i, [x, y]) in h.wobble(key, &page, false).into_iter().enumerate() {
            let _ = write!(d, "{}{} {}", if i == 0 { "M" } else { "L" }, n(x), n(y));
        }
        d
    };
    let mut s = String::new();
    let height = size + top;
    let _ = writeln!(
        s,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">",
        w = n(size),
        h = n(height)
    );
    let _ = writeln!(
        s,
        "<defs><clipPath id=\"town-clip\"><rect x=\"0\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath></defs>",
        n(top),
        n(size),
        n(size)
    );
    let _ = writeln!(
        s,
        "<rect id=\"town-paper\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{paper}\"/>",
        n(size),
        n(height)
    );
    let _ = writeln!(
        s,
        "<text id=\"town-title\" x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"6\" text-anchor=\"middle\" fill=\"{dark}\">{}</text>",
        n(size / 2.0),
        n(top - 3.5),
        xml(&style.font),
        xml(name)
    );
    let _ = writeln!(s, "<g id=\"town-map\" clip-path=\"url(#town-clip)\">");

    // inkland: the sea, then the river: a dark bank line on each side.
    let water = water_colour(style.colour);
    if !town.water.is_empty() {
        let d: String = town.water.iter().map(|p| path(p, true)).collect();
        let _ = writeln!(
            s,
            "<path id=\"town-sea\" d=\"{d}\" fill=\"{water}\" stroke=\"{water}\" stroke-width=\"{}\"/>",
            n(w(NORMAL_STROKE))
        );
    }
    if !town.water.is_empty() {
        // In black and white, the sea has lines across it, clipped to the
        // sea.
        if !style.colour {
            let _ = write!(s, "<clipPath id=\"town-sea-clip\">");
            for p in &town.water {
                let _ = write!(s, "<path d=\"{}\"/>", path(p, true));
            }
            s.push_str("</clipPath>\n");
            let mut d = String::new();
            let mut y = top + HATCH_MM / 2.0;
            while y < top + size {
                let _ = write!(d, "M0 {}H{}", n(y), n(size));
                y += HATCH_MM;
            }
            let _ = writeln!(
                s,
                "<path id=\"town-sea-hatch\" d=\"{d}\" fill=\"none\" stroke=\"{dark}\" stroke-width=\"{}\" clip-path=\"url(#town-sea-clip)\"/>",
                n(min)
            );
        }
        // The shore: each edge of the sea that a land patch has too.
        let mut d = String::new();
        for (si, sea) in town.water.iter().enumerate() {
            let m = sea.len();
            for i in 0..m {
                let (a, b) = (sea[i], sea[(i + 1) % m]);
                let land = town.patches.iter().any(|p| {
                    let l = p.shape.len();
                    (0..l).any(|j| p.shape[j] == b && p.shape[(j + 1) % l] == a)
                });
                if land {
                    d.push_str(&pen(&format!("shore-{si}-{i}"), &[a, b]));
                }
            }
        }
        if !d.is_empty() {
            let _ = writeln!(
                s,
                "<path id=\"town-shore\" d=\"{d}\" fill=\"none\" stroke=\"{dark}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>",
                n(w(NORMAL_STROKE))
            );
        }
    }
    if let Some(river) = &town.river {
        let d = pen("river", river);
        let _ = writeln!(
            s,
            "<g id=\"town-river\" fill=\"none\" stroke-linejoin=\"round\" stroke-linecap=\"round\"><path d=\"{d}\" stroke=\"{dark}\" stroke-width=\"{}\"/><path d=\"{d}\" stroke=\"{water}\" stroke-width=\"{}\"/></g>",
            n(w(RIVER + NORMAL_STROKE * 2.0)),
            n(w(RIVER))
        );
    }

    // inkland: a moat around the wall of a town with a river or the sea to
    // fill it. The roads cross it at the gates.
    if let Some(wl) = &town.wall
        && (town.river.is_some() || !town.water.is_empty())
    {
        let ring = outset(&wl.shape, MOAT_OFFSET);
        let m = ring.len();
        let mut d = String::new();
        let castle = town.castle.as_ref().map(|c| c.shape.clone());
        for i in 0..m {
            let (a, b) = (ring[i], ring[(i + 1) % m]);
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            // No moat inside the castle, where its wall joins the town wall.
            let in_castle = castle
                .as_ref()
                .is_some_and(|c| inkland_core::town::districts::contains(c, mid));
            if wl.segments.get(i).copied().unwrap_or(true) && !in_castle {
                d.push_str(&pen(&format!("moat-{i}"), &[a, b]));
            }
        }
        if !d.is_empty() {
            let _ = writeln!(
                s,
                "<g id=\"town-moat\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"{d}\" stroke=\"{dark}\" stroke-width=\"{}\"/><path d=\"{d}\" stroke=\"{water}\" stroke-width=\"{}\"/></g>",
                n(w(MOAT + NORMAL_STROKE * 2.0)),
                n(w(MOAT))
            );
        }
    }

    // inkland: the fields of the farms, under the roads.
    if !town.fields.is_empty() {
        let d: String = town.fields.iter().map(|f| path(f, true)).collect();
        let fill = if style.colour { "#c2bba9" } else { "none" };
        let _ = writeln!(
            s,
            "<path id=\"town-fields\" d=\"{d}\" fill=\"{fill}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"/>",
            if style.colour { light } else { dark },
            n(w(NORMAL_STROKE * 0.5))
        );
    }

    // The roads: a dark line with a paper line inside.
    let _ = writeln!(
        s,
        "<g id=\"town-roads\" fill=\"none\" stroke-linejoin=\"round\">"
    );
    for (ri, road) in town.roads.iter().enumerate() {
        let d = pen(&format!("road-{ri}"), road);
        let _ = writeln!(
            s,
            "<path d=\"{d}\" stroke=\"{medium}\" stroke-width=\"{}\"/>",
            n(w(MAIN_STREET + NORMAL_STROKE))
        );
        let _ = writeln!(
            s,
            "<path d=\"{d}\" stroke=\"{paper}\" stroke-width=\"{}\"/>",
            n(w(MAIN_STREET - NORMAL_STROKE))
        );
    }
    s.push_str("</g>\n");

    // inkland: in colour, each building casts a shadow to the south-east.
    if style.colour {
        let d: String = town
            .patches
            .iter()
            .filter(|p| p.kind != Kind::Park)
            .flat_map(|p| p.buildings.iter())
            .map(|b| path(b, true))
            .collect();
        if !d.is_empty() {
            let off = n(SHADOW * k);
            let _ = writeln!(
                s,
                "<path id=\"town-shadows\" d=\"{d}\" fill=\"{SHADOW_COLOUR}\" transform=\"translate({off} {off})\"/>"
            );
        }
    }

    // The wards.
    let _ = writeln!(s, "<g id=\"town-wards\" stroke-linejoin=\"miter\">");
    for (i, p) in town.patches.iter().enumerate() {
        if p.buildings.is_empty() {
            continue;
        }
        let d: String = p.buildings.iter().map(|b| path(b, true)).collect();
        let class = p.kind.label().to_ascii_lowercase();
        match p.kind {
            Kind::Castle | Kind::Cathedral => {
                // A thick outline under a fill, as `drawBuilding`.
                let t = if p.kind == Kind::Castle { 2.0 } else { 1.0 };
                let _ = writeln!(
                    s,
                    "<g id=\"town-ward-{i}\" class=\"{class}\"><path d=\"{d}\" fill=\"none\" stroke=\"{dark}\" stroke-width=\"{}\"/><path d=\"{d}\" fill=\"{light}\" stroke=\"none\"/></g>",
                    n(w(NORMAL_STROKE * t * 2.0))
                );
            }
            Kind::Park => {
                // inkland: a lawn with trees on it.
                let (lawn, tree) = if style.colour {
                    (LAWN_COLOUR, light)
                } else {
                    ("none", paper)
                };
                let mut trees = String::new();
                for t in park_trees(i, &p.buildings) {
                    let [x, y] = to([t[0], t[1]]);
                    let _ = write!(
                        trees,
                        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
                        n(x),
                        n(y),
                        n(t[2] * k)
                    );
                }
                let _ = writeln!(
                    s,
                    "<g id=\"town-ward-{i}\" class=\"{class}\" stroke=\"{dark}\" stroke-width=\"{}\"><path d=\"{d}\" fill=\"{lawn}\"/><g fill=\"{tree}\">{trees}</g></g>",
                    n(w(NORMAL_STROKE))
                );
            }
            _ => {
                let _ = writeln!(
                    s,
                    "<path id=\"town-ward-{i}\" class=\"{class}\" d=\"{d}\" fill=\"{light}\" stroke=\"{dark}\" stroke-width=\"{}\"/>",
                    n(w(NORMAL_STROKE))
                );
            }
        }
    }
    s.push_str("</g>\n");

    // inkland: the piers and the bridges.
    if !town.piers.is_empty() {
        let d: String = town.piers.iter().map(|p| path(p, true)).collect();
        let _ = writeln!(
            s,
            "<path id=\"town-piers\" d=\"{d}\" fill=\"{light}\" stroke=\"{dark}\" stroke-width=\"{}\"/>",
            n(w(NORMAL_STROKE))
        );
    }
    if !town.bridges.is_empty() {
        let _ = writeln!(s, "<g id=\"town-bridges\">");
        for (i, b) in town.bridges.iter().enumerate() {
            // A deck across the river, along the street.
            let along = [
                b.dir[0] * (RIVER / 2.0 + 1.5),
                b.dir[1] * (RIVER / 2.0 + 1.5),
            ];
            let side = [-b.dir[1] * MAIN_STREET * 0.6, b.dir[0] * MAIN_STREET * 0.6];
            let deck = [
                [b.at[0] - along[0] - side[0], b.at[1] - along[1] - side[1]],
                [b.at[0] + along[0] - side[0], b.at[1] + along[1] - side[1]],
                [b.at[0] + along[0] + side[0], b.at[1] + along[1] + side[1]],
                [b.at[0] - along[0] + side[0], b.at[1] - along[1] + side[1]],
            ];
            let _ = writeln!(
                s,
                "<path id=\"town-bridge-{i}\" d=\"{}\" fill=\"{paper}\" stroke=\"{dark}\" stroke-width=\"{}\"/>",
                path(&deck, true),
                n(w(NORMAL_STROKE))
            );
        }
        s.push_str("</g>\n");
    }

    // The walls, their gates, and their towers.
    let _ = writeln!(s, "<g id=\"town-walls\">");
    let mut wall = |id: &str, wl: &TownWall, large: bool| {
        // The parts of the wall that stand: a shore has none.
        let m = wl.shape.len();
        let mut d = String::new();
        for i in 0..m {
            if wl.segments.get(i).copied().unwrap_or(true) {
                d.push_str(&pen(
                    &format!("{id}-{i}"),
                    &[wl.shape[i], wl.shape[(i + 1) % m]],
                ));
            }
        }
        let _ = writeln!(
            s,
            "<path id=\"{id}\" d=\"{d}\" fill=\"none\" stroke=\"{dark}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>",
            n(w(THICK_STROKE))
        );
        for (g, gate) in wl.gates.iter().enumerate() {
            let Some(i) = wl.shape.iter().position(|v| v == gate) else {
                continue;
            };
            let (a, b) = (wl.shape[(i + 1) % m], wl.shape[(i + m - 1) % m]);
            let dir = [a[0] - b[0], a[1] - b[1]];
            let l = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt().max(1e-9);
            let f = THICK_STROKE * 1.5 / l;
            let (p, q) = (
                [gate[0] - dir[0] * f, gate[1] - dir[1] * f],
                [gate[0] + dir[0] * f, gate[1] + dir[1] * f],
            );
            let _ = writeln!(
                s,
                "<path id=\"{id}-gate-{g}\" d=\"{}\" stroke=\"{dark}\" stroke-width=\"{}\" stroke-linecap=\"butt\"/>",
                path(&[p, q], false),
                n(w(THICK_STROKE * 2.0))
            );
        }
        let tr = THICK_STROKE * if large { 1.5 } else { 1.0 };
        for t in &wl.towers {
            let [x, y] = to(*t);
            let _ = writeln!(
                s,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{dark}\"/>",
                n(x),
                n(y),
                n(tr * k)
            );
        }
    };
    if let Some(wl) = &town.wall {
        wall("town-wall", wl, false);
    }
    if let Some(wl) = &town.castle {
        wall("town-castle-wall", wl, true);
    }
    s.push_str("</g>\n");
    district_labels(&mut s, town, style, &to, paper, dark);
    s.push_str("</g>\n");
    scale_and_compass(&mut s, style, k, paper, dark);
    s.push_str("</svg>\n");
    s
}

/// inkland: a scale bar at the bottom left of the map and a compass at the
/// bottom right, each on a paper box. `k` is mm for each town unit.
fn scale_and_compass(s: &mut String, style: &TownStyle, k: f64, paper: &str, dark: &str) {
    let size = style.size_mm;
    let font = (size / 62.0).clamp(2.0, 3.5);
    let line = line_mm(style.min_line_pt).max(0.15);
    let (name, metres) = bar_unit(&style.unit);
    // mm on the page for one unit of the bar.
    let mm = k * metres / METRES_PER_UNIT;
    let length = crate::extras::nice_length(0.25 * size / mm);
    let bar = length * mm;
    let bottom = TITLE_MM + size;
    let (x, y) = (5.0, bottom - 5.0);
    let _ = writeln!(s, "<g id=\"town-scale-bar\">");
    let _ = writeln!(
        s,
        "<rect id=\"town-scale-bar-back\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{paper}\"/>",
        n(x - 2.0),
        n(y - font - 2.0),
        n(bar + 4.0 + font * 3.0),
        n(font + 5.0)
    );
    for i in 0..4 {
        let fill = if i % 2 == 0 { dark } else { paper };
        let _ = writeln!(
            s,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"1\" fill=\"{fill}\" stroke=\"{dark}\" stroke-width=\"{}\"/>",
            n(x + bar * f64::from(i) / 4.0),
            n(y),
            n(bar / 4.0),
            n(line)
        );
    }
    let num = |v: f64| inkland_core::num::fixed(v, 3);
    for (v, at, last) in [
        (0.0, x, false),
        (length / 2.0, x + bar / 2.0, false),
        (length, x + bar, true),
    ] {
        let text = if last {
            format!("{} {name}", num(v))
        } else {
            num(v)
        };
        let _ = writeln!(
            s,
            "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" text-anchor=\"{}\" fill=\"{dark}\">{}</text>",
            n(at),
            n(y - 0.8),
            xml(&style.font),
            n(font),
            if last { "start" } else { "middle" },
            xml(&text)
        );
    }
    s.push_str("</g>\n");
    // The compass: a north arrow with an N above it.
    let (cx, cy) = (size - 8.0, bottom - 9.0);
    let _ = writeln!(s, "<g id=\"town-compass\">");
    let _ = writeln!(
        s,
        "<circle id=\"town-compass-back\" cx=\"{}\" cy=\"{}\" r=\"6\" fill=\"{paper}\"/>",
        n(cx),
        n(cy - 0.5)
    );
    let _ = writeln!(
        s,
        "<path d=\"M{} {}L{} {}L{} {}Z\" fill=\"{dark}\" stroke=\"{dark}\" stroke-width=\"{}\" stroke-linejoin=\"round\"/>",
        n(cx),
        n(cy - 3.0),
        n(cx + 1.4),
        n(cy + 3.0),
        n(cx),
        n(cy + 1.8),
        n(line)
    );
    let _ = writeln!(
        s,
        "<path d=\"M{} {}L{} {}L{} {}Z\" fill=\"{paper}\" stroke=\"{dark}\" stroke-width=\"{}\" stroke-linejoin=\"round\"/>",
        n(cx),
        n(cy - 3.0),
        n(cx - 1.4),
        n(cy + 3.0),
        n(cx),
        n(cy + 1.8),
        n(line)
    );
    let _ = writeln!(
        s,
        "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" text-anchor=\"middle\" fill=\"{dark}\">N</text>",
        n(cx),
        n(cy - 3.6),
        xml(&style.font),
        n(font)
    );
    s.push_str("</g>\n");
}

/// inkland: the names of the districts, in capitals on a paper box.
/// The larger districts come first. A label that would cover a label
/// before it tries the other patches of its district, and is left out if
/// no patch gives it space.
fn district_labels(
    s: &mut String,
    town: &Town,
    style: &TownStyle,
    to: &dyn Fn(P) -> [f64; 2],
    paper: &str,
    dark: &str,
) {
    if town.districts.is_empty() {
        return;
    }
    let size = (style.size_mm / 62.0).clamp(2.0, 3.5);
    let spacing = size * 0.12;
    let metrics = crate::metrics::FontMetrics::of(&style.font);
    let cap = metrics.cap(size);
    let pad = size * 0.18;
    let (lo, hi) = (TITLE_MM, TITLE_MM + style.size_mm);
    let mut order: Vec<usize> = (0..town.districts.len()).collect();
    order.sort_by(|&a, &b| {
        town.districts[b]
            .area
            .total_cmp(&town.districts[a].area)
            .then(a.cmp(&b))
    });
    let mut boxes: Vec<[f64; 4]> = Vec::new();
    let mut texts = String::new();
    for i in order {
        let d = &town.districts[i];
        let text = d.name.to_uppercase();
        let width = metrics.width(&text, size, spacing) + 2.0 * pad;
        let mut spots = vec![d.label];
        spots.extend(
            d.patches
                .iter()
                .map(|&p| inkland_core::town::poly::centroid(&town.patches[p].shape)),
        );
        let fits = |b: &[f64; 4]| {
            b[0] >= 0.0
                && b[2] <= style.size_mm
                && b[1] >= lo
                && b[3] <= hi
                && !boxes
                    .iter()
                    .any(|o| b[0] < o[2] && o[0] < b[2] && b[1] < o[3] && o[1] < b[3])
        };
        let found = spots
            .into_iter()
            .map(|p| {
                let [x, y] = to(p);
                let h = cap / 2.0 + pad;
                ([x, y], [x - width / 2.0, y - h, x + width / 2.0, y + h])
            })
            .find(|(_, b)| fits(b));
        let Some(([x, y], b)) = found else {
            continue;
        };
        boxes.push(b);
        let _ = writeln!(
            texts,
            "<text id=\"town-district-{i}\" x=\"{}\" y=\"{}\">{}</text>",
            n(x),
            n(y + cap / 2.0),
            xml(&text)
        );
    }
    // The paper box behind each label keeps it clear of the buildings.
    let _ = writeln!(s, "<g id=\"town-district-boxes\" fill=\"{paper}\">");
    for (i, b) in boxes.iter().enumerate() {
        let _ = writeln!(
            s,
            "<rect id=\"town-district-box-{i}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"/>",
            n(b[0]),
            n(b[1]),
            n(b[2] - b[0]),
            n(b[3] - b[1]),
            n(pad)
        );
    }
    s.push_str("</g>\n");
    let _ = writeln!(
        s,
        "<g id=\"town-districts\" font-family=\"{}\" font-size=\"{}\" letter-spacing=\"{}\" text-anchor=\"middle\" fill=\"{dark}\">",
        xml(&style.font),
        n(size),
        n(spacing)
    );
    s.push_str(&texts);
    s.push_str("</g>\n");
}

fn xml(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkland_core::town::{TownOptions, generate};

    fn town() -> Town {
        generate(&TownOptions {
            patches: 15,
            seed: 4242,
            plaza: Some(true),
            citadel: Some(true),
            walls: Some(true),
            lots: false,
            coast: None,
            river: None,
            names: 1,
            uphill: None,
            roads: Vec::new(),
        })
        .unwrap()
    }

    fn style(colour: bool) -> TownStyle {
        TownStyle {
            colour,
            size_mm: 150.0,
            font: "IM Fell English".to_owned(),
            min_line_pt: 0.25,
            unit: "mile".to_owned(),
            hand: None,
        }
    }

    #[test]
    fn the_hand_moves_the_walls_in_black_and_white_only() {
        let t = town();
        let hand = TownStyle {
            hand: Some(crate::hand::Hand::new(7)),
            ..style(false)
        };
        let wall = |svg: &str| {
            let i = svg.find("id=\"town-wall\"").unwrap();
            let j = i + svg[i..].find("/>").unwrap();
            svg[i..j].to_owned()
        };
        let exact = town_svg(&t, "K", &style(false));
        let drawn = town_svg(&t, "K", &hand);
        assert_ne!(wall(&exact), wall(&drawn));
        // The colour look keeps exact lines.
        let colour = TownStyle {
            colour: true,
            ..hand.clone()
        };
        assert_eq!(town_svg(&t, "K", &colour), town_svg(&t, "K", &style(true)));
        let opts = crate::checks::CheckOptions {
            min_line_width_pt: 0.25,
            fonts_allowed: vec!["IM Fell English".to_owned()],
        };
        for r in crate::checks::check_svg(&drawn, &opts) {
            assert!(r.pass, "{} {:?}", r.name, r.details);
        }
    }

    #[test]
    fn the_scale_bar_uses_feet_for_miles_and_metres_else() {
        assert_eq!(bar_unit("mile").0, "feet");
        assert_eq!(bar_unit("League").0, "feet");
        assert_eq!(bar_unit("km").0, "metres");
        let svg = town_svg(&town(), "Kingsholm", &style(true));
        assert!(svg.contains("id=\"town-scale-bar\""));
        assert!(svg.contains(" feet</text>"));
        assert!(svg.contains("id=\"town-compass\""));
        assert!(svg.contains("id=\"town-fields\""));
    }

    #[test]
    fn the_sea_has_a_shore_and_lines_in_black_and_white() {
        let t = generate(&TownOptions {
            patches: 15,
            seed: 4242,
            plaza: Some(true),
            citadel: Some(false),
            walls: Some(true),
            lots: false,
            coast: Some(0.0),
            river: None,
            names: 1,
            uphill: None,
            roads: Vec::new(),
        })
        .unwrap();
        let print = town_svg(&t, "K", &style(false));
        assert!(print.contains("id=\"town-sea-hatch\""));
        assert!(print.contains("id=\"town-shore\""));
        let colour = town_svg(&t, "K", &style(true));
        assert!(!colour.contains("id=\"town-sea-hatch\""));
        let opts = crate::checks::CheckOptions {
            min_line_width_pt: 0.25,
            fonts_allowed: vec!["IM Fell English".to_owned()],
        };
        for r in crate::checks::check_svg(&print, &opts) {
            assert!(r.pass, "{} {:?}", r.name, r.details);
        }
    }

    #[test]
    fn a_park_has_trees_inside_its_lawns() {
        let lawn = inkland_core::town::poly::rect(20.0, 20.0);
        let trees = park_trees(3, std::slice::from_ref(&lawn));
        assert!(trees.len() >= 20, "{}", trees.len());
        for t in &trees {
            assert!(
                t[0].abs() + t[2] <= 10.0 && t[1].abs() + t[2] <= 10.0,
                "{t:?}"
            );
        }
        assert_eq!(trees, park_trees(3, &[lawn]));
    }

    #[test]
    fn an_outset_square_is_larger_by_the_distance() {
        let sq = inkland_core::town::poly::rect(10.0, 10.0);
        let o = outset(&sq, 2.0);
        for (a, b) in sq.iter().zip(&o) {
            assert!((b[0].abs() - a[0].abs() - 2.0).abs() < 1e-9, "{b:?}");
            assert!((b[1].abs() - a[1].abs() - 2.0).abs() < 1e-9, "{b:?}");
        }
    }

    #[test]
    fn buildings_cast_shadows_in_colour_only() {
        let t = town();
        assert!(town_svg(&t, "K", &style(true)).contains("id=\"town-shadows\""));
        assert!(!town_svg(&t, "K", &style(false)).contains("id=\"town-shadows\""));
    }

    #[test]
    fn a_print_safe_town_map_passes_the_print_checks() {
        let svg = town_svg(&town(), "Kingsholm", &style(false));
        let opts = crate::checks::CheckOptions {
            min_line_width_pt: 0.25,
            fonts_allowed: vec!["IM Fell English".to_owned()],
        };
        for r in crate::checks::check_svg(&svg, &opts) {
            assert!(r.pass, "{} {:?}", r.name, r.details);
        }
        assert!(svg.contains(">Kingsholm</text>"));
        assert!(svg.contains("id=\"town-wall\""));
        assert!(svg.contains("id=\"town-castle-wall\""));
        assert!(svg.contains("class=\"market\""));
    }

    #[test]
    fn a_colour_town_map_uses_the_palette_of_the_original() {
        let svg = town_svg(&town(), "Kingsholm", &style(true));
        assert!(svg.contains("fill=\"#ccc5b8\""));
        assert!(svg.contains("fill=\"#99948a\""));
        roxmltree::Document::parse(&svg).expect("valid XML");
        // The same town and style give the same bytes.
        assert_eq!(svg, town_svg(&town(), "Kingsholm", &style(true)));
    }
}
