//! Area layers: biomes, states, provinces, cultures, religions, and zones
//! (`draw-biomes.ts`, `draw-states.ts`, `draw-provinces.ts`,
//! `draw-cultures.ts`, `draw-religions.ts`, `draw-zones.ts`,
//! `isoline-fills.ts`).

use super::super::Ctx;
use super::super::geom::{self, Edge, n};
use super::palette::BIOME_COLORS;
use inkland_core::features::FeatureKind;
use inkland_core::grid::OUTSIDE;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// The fill path and the water gap path of an area: the fill of its rings,
/// and the parts of its rings next to water (`getIsolines` with `fill` and
/// `waterGap`).
fn fill_and_gap(ctx: &Ctx, rings: &[Vec<Edge>]) -> (String, String) {
    let w = ctx.world;
    let g = &w.grid;
    let mut fill = String::new();
    let mut gap = String::new();
    for ring in rings {
        if ring.len() < 3 {
            continue;
        }
        for (i, e) in ring.iter().enumerate() {
            let q = ctx.p(g.vertices[e.from as usize]);
            let _ = write!(
                fill,
                "{}{},{}",
                if i == 0 { 'M' } else { 'L' },
                n(q[0]),
                n(q[1])
            );
        }
        fill.push('Z');
        // Runs of edges next to water or to the frame.
        let wet = |e: &Edge| e.other == OUTSIDE || !w.features.is_land(e.other as usize);
        let k = ring.len();
        let Some(start) = (0..k).find(|&i| !wet(&ring[i])) else {
            // All edges are wet: the whole ring.
            for (i, e) in ring.iter().chain(ring.first()).enumerate() {
                let q = ctx.p(g.vertices[e.from as usize]);
                let _ = write!(
                    gap,
                    "{}{},{}",
                    if i == 0 { 'M' } else { 'L' },
                    n(q[0]),
                    n(q[1])
                );
            }
            continue;
        };
        let mut open = false;
        for j in 1..=k {
            let e = &ring[(start + j) % k];
            if wet(e) {
                let a = ctx.p(g.vertices[e.from as usize]);
                let b = ctx.p(g.vertices[e.to as usize]);
                if !open {
                    let _ = write!(gap, "M{},{}", n(a[0]), n(a[1]));
                    open = true;
                }
                let _ = write!(gap, "L{},{}", n(b[0]), n(b[1]));
            } else {
                open = false;
            }
        }
    }
    (fill, gap)
}

/// One fill path and one water gap path for each type (`buildFillPaths`).
fn fill_paths(
    s: &mut String,
    ctx: &Ctx,
    name: &str,
    rings: &BTreeMap<u32, Vec<Vec<Edge>>>,
    color_of: &dyn Fn(u32) -> String,
) {
    for (t, list) in rings {
        let (fill, gap) = fill_and_gap(ctx, list);
        let color = color_of(*t);
        if !fill.is_empty() {
            let _ = writeln!(s, "<path d=\"{fill}\" fill=\"{color}\" id=\"{name}-{t}\"/>");
        }
        if !gap.is_empty() {
            let _ = writeln!(
                s,
                "<path d=\"{gap}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"3\" id=\"{name}-gap-{t}\"/>"
            );
        }
    }
}

/// Biomes.
pub fn biomes(s: &mut String, ctx: &Ctx) {
    let w = ctx.world;
    let rings = geom::rings(w, &|c| {
        if w.features.is_land(c) {
            u32::from(w.biomes[c])
        } else {
            0
        }
    });
    let _ = writeln!(
        s,
        "<g id=\"biomes\" mask=\"url(#land)\" stroke-linejoin=\"round\" fill-rule=\"evenodd\"{}>",
        ctx.attrs(&["biomes"], &[])
    );
    fill_paths(s, ctx, "biome", &rings, &|t| {
        BIOME_COLORS
            .get(t as usize)
            .copied()
            .unwrap_or("#999999")
            .to_owned()
    });
    s.push_str("</g>\n");
}

/// The rings of the land cells by an owner of each cell.
fn land_rings(ctx: &Ctx, owner: &[u32]) -> BTreeMap<u32, Vec<Vec<Edge>>> {
    let w = ctx.world;
    geom::rings(w, &|c| if w.features.is_land(c) { owner[c] } else { 0 })
}

/// States: the fill of each state, and a halo along its border
/// (`drawStates`).
pub fn states(s: &mut String, ctx: &Ctx) {
    let soc = &ctx.world.society;
    let rings = land_rings(ctx, &soc.cell_state);
    let color = |t: u32| {
        ctx.palette
            .states
            .get(&t)
            .cloned()
            .unwrap_or_else(|| "#999999".to_owned())
    };
    let _ = writeln!(s, "<g id=\"regions\"{}>", ctx.attrs(&["states"], &[]));
    let _ = writeln!(
        s,
        "<g id=\"states-body\" mask=\"url(#land)\" stroke-linejoin=\"round\" fill-rule=\"evenodd\"{}>",
        ctx.attrs(&["states", "statesBody"], &[])
    );
    fill_paths(s, ctx, "state", &rings, &color);
    s.push_str("</g>\n");
    let _ = writeln!(
        s,
        "<g id=\"states-halo\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{}>",
        ctx.attrs(&["states", "statesHalo"], &[])
    );
    let g = &ctx.world.grid;
    for (t, list) in &rings {
        // The border without the parts on the frame edge.
        let mut d = String::new();
        for ring in list {
            let mut open = false;
            for e in ring.iter().chain(ring.first()) {
                let on_frame = e.other == OUTSIDE;
                if on_frame {
                    open = false;
                    continue;
                }
                let q = ctx.p(g.vertices[e.from as usize]);
                let _ = write!(d, "{}{},{}", if open { 'L' } else { 'M' }, n(q[0]), n(q[1]));
                open = true;
            }
        }
        if d.is_empty() {
            continue;
        }
        let halo = super::super::color::Rgb::parse(&color(*t))
            .map_or_else(|| "#666666".to_owned(), |c| c.darker(1.0).hex());
        ctx.def(
            &format!("state-clip-{t}"),
            format!("<clipPath id=\"state-clip-{t}\"><use href=\"#state-{t}\"/></clipPath>"),
        );
        let _ = writeln!(
            s,
            "<path id=\"state-border-{t}\" d=\"{d}\" clip-path=\"url(#state-clip-{t})\" stroke=\"{halo}\"/>"
        );
    }
    s.push_str("</g>\n</g>\n");
}

/// Provinces (`drawProvinces`).
pub fn provinces(s: &mut String, ctx: &Ctx) {
    let rings = land_rings(ctx, &ctx.world.society.cell_province);
    let _ = writeln!(s, "<g id=\"provs\"{}>", ctx.attrs(&["provinces"], &[]));
    s.push_str(
        "<g id=\"provinces-body\" mask=\"url(#land)\" stroke-linejoin=\"round\" fill-rule=\"evenodd\">\n",
    );
    fill_paths(s, ctx, "province", &rings, &|t| {
        ctx.palette
            .provinces
            .get(&t)
            .cloned()
            .unwrap_or_else(|| "#999999".to_owned())
    });
    s.push_str("</g>\n</g>\n");
}

/// Cultures (`drawCultures`).
pub fn cultures(s: &mut String, ctx: &Ctx) {
    let rings = land_rings(ctx, &ctx.world.society.cell_culture);
    let _ = writeln!(
        s,
        "<g id=\"cults\" mask=\"url(#land)\" stroke-linejoin=\"round\" fill-rule=\"evenodd\"{}>",
        ctx.attrs(&["cultures"], &[])
    );
    fill_paths(s, ctx, "culture", &rings, &|t| {
        ctx.palette
            .cultures
            .get(&t)
            .cloned()
            .unwrap_or_else(|| "#999999".to_owned())
    });
    s.push_str("</g>\n");
}

/// Religions (`drawReligions`).
pub fn religions(s: &mut String, ctx: &Ctx) {
    let rings = land_rings(ctx, &ctx.world.religions.cell_religion);
    let _ = writeln!(
        s,
        "<g id=\"relig\" mask=\"url(#land)\" stroke-linejoin=\"round\" fill-rule=\"evenodd\"{}>",
        ctx.attrs(&["religions"], &[])
    );
    fill_paths(s, ctx, "religion", &rings, &|t| {
        ctx.palette
            .religions
            .get(&t)
            .cloned()
            .unwrap_or_else(|| "#999999".to_owned())
    });
    s.push_str("</g>\n");
}

/// Zones (`drawZones`): one path for each zone, with its hatch fill.
pub fn zones(s: &mut String, ctx: &Ctx) {
    let Some(list) = super::super::data::zones(ctx.world) else {
        return;
    };
    write_zones(s, ctx, &list);
}

/// The rings of the outline of a set of cells (`getVertexPath`).
fn cell_rings(ctx: &Ctx, cells: &[usize]) -> Vec<Vec<Edge>> {
    let g = &ctx.world.grid;
    let set: BTreeSet<usize> = cells.iter().copied().filter(|&c| c < g.len()).collect();
    let mut edges = Vec::new();
    for &c in &set {
        let poly = &g.polygons[c];
        for (k, &o) in g.edge_cells[c].iter().enumerate() {
            if o != OUTSIDE && set.contains(&(o as usize)) {
                continue;
            }
            edges.push(Edge {
                from: poly[k],
                to: poly[(k + 1) % poly.len()],
                other: o,
            });
        }
    }
    let mut rings = geom::chain_rings(&edges);
    // A ring around a lake whose shore is all in the set is not a hole: the
    // zone covers the lake (`getVertexPath`, "inner lake").
    rings.retain(|ring| !inner_lake(ctx, &set, ring));
    rings
}

/// Whether all cells across a ring are of one lake, and all shore cells of
/// that lake are in the set.
fn inner_lake(ctx: &Ctx, set: &BTreeSet<usize>, ring: &[Edge]) -> bool {
    let f = &ctx.world.features;
    let mut lake = None;
    for e in ring {
        if e.other == OUTSIDE {
            return false;
        }
        let id = f.of_cell[e.other as usize];
        if f.get(id).kind != FeatureKind::Lake || lake.is_some_and(|l| l != id) {
            return false;
        }
        lake = Some(id);
    }
    lake.is_some_and(|id| {
        let shore = &f.get(id).shoreline;
        !shore.is_empty() && shore.iter().all(|c| set.contains(c))
    })
}

/// Write zones. A hatch fill adds its pattern to the defs.
pub fn write_zones(s: &mut String, ctx: &Ctx, list: &[super::super::data::Zone]) {
    let _ = writeln!(s, "<g id=\"zones\"{}>", ctx.attrs(&["zones"], &[]));
    for z in list {
        let rings = cell_rings(ctx, &z.cells);
        let (fill, _) = fill_and_gap(ctx, &rings);
        if fill.is_empty() {
            continue;
        }
        super::super::hatches::ensure(ctx, &z.color);
        let _ = writeln!(
            s,
            "<path id=\"zone-{}\" d=\"{fill}\" fill=\"{}\"/>",
            z.id,
            crate::extras::xml(&z.color)
        );
    }
    s.push_str("</g>\n");
}
