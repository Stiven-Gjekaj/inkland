//! Line layers: borders, routes, and rivers (`draw-borders.ts`,
//! `draw-routes.ts`, `draw-rivers.ts`, `river-generator.ts`,
//! `path-utils.ts`).

use super::super::Ctx;
use super::super::geom::{catmull_rom, catmull_rom_into, n};
use inkland_core::config::RoadKind;
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// State and province borders: straight lines along the cell edges
/// (`drawBorders`).
pub fn borders(s: &mut String, ctx: &Ctx) {
    let w = ctx.world;
    let (states, provinces) = inkland_core::borders::state_and_province_edges(w);
    let path = |edges: &[(u32, u32)]| -> String {
        let mut d = String::new();
        for line in inkland_core::borders::chain_edges(edges) {
            for (i, v) in line.iter().enumerate() {
                let q = ctx.p(w.grid.vertices[*v as usize]);
                let _ = write!(
                    d,
                    "{}{},{}",
                    if i == 0 { 'M' } else { 'L' },
                    n(q[0]),
                    n(q[1])
                );
            }
        }
        d
    };
    let _ = writeln!(
        s,
        "<g id=\"borders\" stroke-linejoin=\"round\" fill=\"none\"{}>",
        ctx.attrs(&["borders"], &[])
    );
    let _ = writeln!(
        s,
        "<g id=\"state-borders\"{}><path d=\"{}\"/></g>",
        ctx.attrs(&["borders", "stateBorders"], &[]),
        path(&states)
    );
    let _ = writeln!(
        s,
        "<g id=\"province-borders\"{}><path d=\"{}\"/></g>",
        ctx.attrs(&["borders", "provinceBorders"], &[]),
        path(&provinces)
    );
    s.push_str("</g>\n");
}

/// Roads and trails: Catmull-Rom curves through the cells of each route
/// (`drawRoutes`, `Routes.getPath`). The generated network draws each
/// shared stretch once.
pub fn routes(s: &mut String, ctx: &Ctx) {
    let w = ctx.world;
    let cfg = ctx.config;
    let mut pinned_edges: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut named: Vec<(String, RoadKind, String)> = Vec::new();
    for r in &w.roads {
        let id = match (r.pin, &r.overlay) {
            (Some(k), _) => cfg.pins.roads[k].id.clone(),
            (None, Some(id)) => id.clone(),
            (None, None) => continue,
        };
        for e in r.cells.windows(2) {
            pinned_edges.insert((e[0].min(e[1]), e[0].max(e[1])));
        }
        let pts: Vec<[f64; 2]> = r.points.iter().map(|&p| ctx.p(p)).collect();
        if pts.len() >= 2 {
            named.push((format!("route-{id}"), r.kind, catmull_rom(&pts, 0.1)));
        }
    }
    named.sort_by(|a, b| a.0.cmp(&b.0));
    let mut network: Vec<(RoadKind, String)> = Vec::new();
    for kind in [RoadKind::Road, RoadKind::Trail] {
        let mut edges: BTreeSet<(usize, usize)> = BTreeSet::new();
        for r in w
            .roads
            .iter()
            .filter(|r| r.pin.is_none() && r.overlay.is_none() && r.kind == kind)
        {
            for e in r.cells.windows(2) {
                let k = (e[0].min(e[1]), e[0].max(e[1]));
                if !pinned_edges.contains(&k) {
                    edges.insert(k);
                }
            }
        }
        if kind == RoadKind::Trail {
            for r in w
                .roads
                .iter()
                .filter(|r| r.pin.is_none() && r.overlay.is_none() && r.kind == RoadKind::Road)
            {
                for e in r.cells.windows(2) {
                    edges.remove(&(e[0].min(e[1]), e[0].max(e[1])));
                }
            }
        }
        let list: Vec<(u32, u32)> = edges.iter().map(|&(a, b)| (a as u32, b as u32)).collect();
        let mut d = String::new();
        for line in inkland_core::borders::chain_edges(&list) {
            let pts: Vec<[f64; 2]> = line
                .iter()
                .map(|&c| ctx.p(w.grid.points[c as usize]))
                .collect();
            d.push_str(&catmull_rom(&pts, 0.1));
        }
        if !d.is_empty() {
            network.push((kind, d));
        }
    }
    let _ = writeln!(
        s,
        "<g id=\"routes\" fill=\"none\"{}>",
        ctx.attrs(&["routes"], &[])
    );
    for (group, kind) in [("roads", RoadKind::Road), ("trails", RoadKind::Trail)] {
        let _ = writeln!(
            s,
            "<g id=\"routes-{group}\"{}>",
            ctx.attrs(&["routes", group], &[])
        );
        for (id, _, d) in named.iter().filter(|x| x.1 == kind) {
            let _ = writeln!(s, "<path id=\"{id}\" d=\"{d}\"/>");
        }
        for (_, d) in network.iter().filter(|x| x.0 == kind) {
            let _ = writeln!(s, "<path id=\"{group}-net\" d=\"{d}\"/>");
        }
        s.push_str("</g>\n");
    }
    // Sea routes between ports, with the curve of FMG for this group.
    let _ = writeln!(
        s,
        "<g id=\"routes-searoutes\"{}>",
        ctx.attrs(&["routes", "searoutes"], &[])
    );
    for r in &w.sea_routes {
        let pts: Vec<[f64; 2]> = r.points.iter().map(|&p| ctx.p(p)).collect();
        if pts.len() >= 2 {
            let _ = writeln!(
                s,
                "<path id=\"searoute-{}\" d=\"{}\"/>",
                r.id,
                catmull_rom(&pts, 0.5)
            );
        }
    }
    s.push_str("</g>\n");
    s.push_str("</g>\n");
}

const FLUX_FACTOR: f64 = 500.0;
const MAX_FLUX_WIDTH: f64 = 1.0;
const LENGTH_FACTOR: f64 = 200.0;
const LENGTH_PROGRESSION: [f64; 9] = [1.0, 1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 21.0, 34.0];

/// The half width of a river at a point, in pixels (`getOffset`).
pub fn offset(flux: f64, index: usize, width_factor: f64, start: f64) -> f64 {
    if index == 0 {
        return start;
    }
    let flux_width = (libm::pow(flux, 0.7) / FLUX_FACTOR).min(MAX_FLUX_WIDTH);
    let progression = LENGTH_PROGRESSION
        .get(index)
        .copied()
        .unwrap_or(LENGTH_PROGRESSION[8])
        / LENGTH_FACTOR;
    let length_width = index as f64 / LENGTH_FACTOR + progression;
    width_factor * (length_width + flux_width) + start
}

/// Bends between the points of a river (`meander`). Each point of the
/// course stays where it is. The result is the points and, for each point,
/// the index of its course point or `None`.
pub fn meander(points: &[[f64; 2]], water_last: bool) -> Vec<([f64; 2], Option<usize>)> {
    let meandering: f64 = 0.5;
    let count = points.len();
    let mut out: Vec<([f64; 2], Option<usize>)> = Vec::new();
    let mut step: f64 = 10.0;
    for i in 0..count {
        let [x1, y1] = points[i];
        out.push(([x1, y1], Some(i)));
        if i + 1 == count {
            break;
        }
        let [x2, y2] = points[i + 1];
        let dist2 = (x2 - x1).powi(2) + (y2 - y1).powi(2);
        step += 1.0;
        if dist2 <= 25.0 && count >= 6 {
            continue;
        }
        let mut m = meandering + 1.0 / (step - 1.0) + (meandering - (step - 1.0) / 100.0).max(0.0);
        if water_last && i + 2 == count {
            m *= 0.25;
        }
        let angle = libm::atan2(y2 - y1, x2 - x1);
        let (sin_m, cos_m) = (libm::sin(angle) * m, libm::cos(angle) * m);
        if step - 1.0 < 20.0 && (dist2 > 64.0 || (dist2 > 36.0 && count < 5)) {
            out.push((
                [(x1 * 2.0 + x2) / 3.0 - sin_m, (y1 * 2.0 + y2) / 3.0 + cos_m],
                None,
            ));
            out.push((
                [
                    (x1 + x2 * 2.0) / 3.0 + sin_m / 2.0,
                    (y1 + y2 * 2.0) / 3.0 - cos_m / 2.0,
                ],
                None,
            ));
        } else if dist2 > 25.0 || count < 6 {
            out.push(([(x1 + x2) / 2.0 - sin_m, (y1 + y2) / 2.0 + cos_m], None));
        }
    }
    out
}

/// The outline of a river with its width (`getRiverPath`).
pub fn river_path(points: &[([f64; 2], f64)], width_factor: f64, start: f64) -> String {
    let m = points.len();
    let mut left = Vec::with_capacity(m);
    let mut right = Vec::with_capacity(m);
    let mut flux = 0.0f64;
    for i in 0..m {
        let [x0, y0] = points[i.saturating_sub(1)].0;
        let ([x1, y1], f) = points[i];
        let [x2, y2] = points[(i + 1).min(m - 1)].0;
        flux = flux.max(f);
        let off = offset(flux, i, width_factor, start);
        let angle = libm::atan2(y0 - y2, x0 - x2);
        let (so, co) = (libm::sin(angle) * off, libm::cos(angle) * off);
        left.push([x1 - so, y1 + co]);
        right.push([x1 + so, y1 - co]);
    }
    right.reverse();
    let mut d = catmull_rom(&right, 0.1);
    catmull_rom_into(&mut d, &left, 0.1, false);
    d
}

/// The style group of a river (`pickGroup`).
fn river_group(ctx: &Ctx, cells: &[usize]) -> &'static str {
    let w = ctx.world;
    let land: Vec<usize> = cells
        .iter()
        .copied()
        .filter(|&c| w.features.is_land(c))
        .collect();
    if land.is_empty() {
        return "regular";
    }
    let h = |c: usize| w.heights[c] - w.sea_level + 20.0;
    let temps: Vec<f64> = land.iter().map(|&c| w.temperature[c]).collect();
    if temps.iter().all(|&t| t <= 0.0) {
        return "frozen";
    }
    if h(cells[0]) >= 60.0 && land.iter().all(|&c| h(c) >= 45.0) {
        return "lava";
    }
    let arid = land.iter().all(|&c| w.precipitation[c] <= 4.0);
    if arid && temps.iter().all(|&t| t >= 20.0) {
        return "dry";
    }
    "regular"
}

/// Rivers: filled shapes that grow wider with their water
/// (`drawRivers`).
pub fn rivers(s: &mut String, ctx: &Ctx) {
    let w = ctx.world;
    let cfg = ctx.config;
    let overlay = crate::labels::overlay_of(cfg, ctx.variant);
    let mut items: Vec<(String, &'static str, String)> = Vec::new();
    for r in &w.rivers {
        if r.course.is_some_and(|k| {
            overlay
                .is_some_and(|o| inkland_core::overlay::removes(o, "river", &cfg.pins.rivers[k].id))
        }) {
            continue;
        }
        if r.points.len() < 2 {
            continue;
        }
        let id = match r.course {
            Some(k) => format!("river-{}", cfg.pins.rivers[k].id),
            None => format!("river-{}", r.id),
        };
        // The water of each course point (FMG `cells.fl`).
        let flux_at = |i: usize| -> f64 { r.cells.get(i).map_or(r.flux, |&c| w.drainage.flux[c]) };
        let pts: Vec<[f64; 2]> = r.points.iter().map(|&p| ctx.p(p)).collect();
        let water_last = r.points.len() > r.cells.len();
        let bent: Vec<([f64; 2], f64)> = meander(&pts, water_last)
            .into_iter()
            .map(|(p, k)| (p, k.map_or(0.0, flux_at)))
            .collect();
        // The width factor and the source width of the river stage.
        items.push((
            id,
            river_group(ctx, &r.cells),
            river_path(&bent, r.width_factor, r.source_width),
        ));
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let _ = writeln!(
        s,
        "<g id=\"rivers\" stroke=\"none\" mask=\"url(#land)\" fill-rule=\"nonzero\"{}>",
        ctx.attrs(&["rivers"], &[])
    );
    let mut groups = ctx.style.groups(&["rivers"]);
    if !groups.iter().any(|g| g == "regular") {
        groups.push("regular".to_owned());
    }
    for group in groups {
        let _ = writeln!(
            s,
            "<g id=\"rivers-{group}\"{}>",
            ctx.attrs(&["rivers", &group], &[])
        );
        for (id, _, d) in items.iter().filter(|x| x.1 == group) {
            let _ = writeln!(s, "<path id=\"{id}\" d=\"{d}\"/>");
        }
        s.push_str("</g>\n");
    }
    s.push_str("</g>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_river_is_wider_at_its_mouth() {
        assert!(offset(5000.0, 20, 1.0, 0.1) > offset(10.0, 1, 1.0, 0.1));
        assert_eq!(offset(5000.0, 0, 1.0, 0.1), 0.1);
    }

    #[test]
    fn meander_keeps_the_course_points() {
        let pts = [[0.0, 0.0], [20.0, 0.0], [40.0, 10.0], [60.0, 10.0]];
        let m = meander(&pts, false);
        let kept: Vec<[f64; 2]> = m.iter().filter(|x| x.1.is_some()).map(|x| x.0).collect();
        assert_eq!(kept, pts.to_vec());
        assert!(m.len() > pts.len());
    }
}
