//! Economy layers: goods (`draw-goods.ts`) and the trade traffic
//! (`draw-trade-traffic.ts`).
//!
//! FMG shows the goods that the user selects. inkland has no selection, so
//! it draws the overview of FMG: each producing cell in the colour of its
//! most valuable good, each place as a pie of its goods, and each deposit
//! as an icon.

use super::super::Ctx;
use super::super::color::Rgb;
use super::super::geom::{catmull_rom, n};
use super::super::icons;
use inkland_core::world::World;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The goods icons are drawn in a 0 0 100 100 frame.
const ICON_FRAME: f64 = 100.0;
/// The percentile from which cells get the darkest colour and places the
/// largest circle.
const OVERVIEW_SATURATION: f64 = 0.95;
/// A pie shows the most valuable goods of a place; the rest go in one
/// sector.
const PIE_GOODS: usize = 5;
/// A sector that is thinner than this joins the rest, which is drawn this
/// wide.
const MIN_SECTOR: f64 = 0.04;
/// The colour of the rest sector.
const OTHER_FILL: &str = "#9e9e9e";
/// The icon lines are this part of the circle outline.
const LINEWORK: f64 = 0.4;
/// The percentile of the units of the links from which routes are drawn at
/// full width.
const TRAFFIC_SATURATION: f64 = 0.95;
/// Links with less than this part of the saturation units are left out.
const TRAFFIC_MIN_SHARE: f64 = 0.01;
/// The least width of a traffic line, in pixels.
const TRAFFIC_MIN_WIDTH: f64 = 0.15;

/// The value at a share of the sorted values (`getPercentile`).
fn percentile(values: &[f64], share: f64) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let k = ((sorted.len() as f64 - 1.0) * share).floor();
    (k >= 0.0).then(|| sorted[k as usize])
}

/// The saturation of a list of values: a high percentile, so that a few
/// large values do not leave the rest pale (`getSaturation`).
fn saturation(values: &[f64]) -> f64 {
    percentile(values, OVERVIEW_SATURATION)
        .filter(|v| *v != 0.0)
        .unwrap_or(1.0)
}

/// The outline colour of a good colour (`Goods.getStroke`).
pub fn stroke_of(color: &str) -> String {
    Rgb::parse(color).map_or_else(|| "#000000".to_owned(), |c| c.darker(2.0).hex())
}

/// `true` if a box of world units `[x0, y0, x1, y1]` is in the view of the
/// page.
fn in_view(ctx: &Ctx, b: [f64; 4]) -> bool {
    let [vx0, vy0, vx1, vy1] = ctx.px.page.view;
    b[2] >= vx0 && b[0] <= vx1 && b[3] >= vy0 && b[1] <= vy1
}

/// The cell of each place, by cell (FMG `cells.burg`).
fn place_of_cell(world: &World) -> BTreeMap<usize, usize> {
    world
        .society
        .places
        .iter()
        .enumerate()
        .map(|(i, p)| (p.cell, i))
        .collect()
}

/// A producing cell: its colour and opacity.
struct CellShade {
    cell: usize,
    color: &'static str,
    opacity: f64,
}

/// Each producing cell in the colour of its most valuable good, with an
/// opacity by its output value (`buildCellOverview`).
fn cell_overview(world: &World) -> Vec<CellShade> {
    let e = &world.economy;
    let mut cells: Vec<(usize, f64, &'static str)> = Vec::new();
    for (c, rural) in e.rural_output.iter().enumerate() {
        // FMG reads the goods of a cell in the order of their ids.
        let mut rural = rural.clone();
        rural.sort_by_key(|&(g, _)| g);
        let mut value = 0.0;
        let mut top: Option<(&'static str, f64)> = None;
        for (g, units) in rural {
            let Some(good) = e.good(g) else {
                continue;
            };
            let v = units * good.value;
            if v <= 0.0 {
                continue;
            }
            value += v;
            if top.is_none_or(|(_, t)| v > t) {
                top = Some((good.color, v));
            }
        }
        if let Some((color, _)) = top {
            cells.push((c, value, color));
        }
    }
    let sat = saturation(&cells.iter().map(|x| x.1).collect::<Vec<_>>());
    cells
        .into_iter()
        .map(|(cell, value, color)| CellShade {
            cell,
            color,
            opacity: 0.1 + 0.9 * (value / sat).min(1.0),
        })
        .collect()
}

/// A sector of a pie: its goods, colour, and part of the circle.
struct Sector {
    goods: Vec<u32>,
    color: &'static str,
    share: f64,
}

/// A place as a pie of the goods that it made.
struct BurgCircle {
    id: u32,
    at: [f64; 2],
    /// The value as a part of the saturation value, at most 1.
    share: f64,
    sectors: Vec<Sector>,
}

/// Each place that made goods as a pie of its goods, by their value at the
/// base prices (`buildBurgCircles`).
fn burg_circles(world: &World) -> Vec<BurgCircle> {
    let e = &world.economy;
    let mut values: Vec<(usize, f64, Vec<Sector>)> = Vec::new();
    for (i, pe) in e.places.iter().enumerate() {
        let mut goods: Vec<(u32, &'static str, f64)> = Vec::new();
        let mut production = pe.production.clone();
        production.sort_by_key(|&(g, _)| g);
        for (g, units) in production {
            if let Some(good) = e.good(g) {
                let v = units * good.value;
                if v > 0.0 {
                    goods.push((g, good.color, v));
                }
            }
        }
        // A stable sort: equal values keep the order of the ids.
        goods.sort_by(|a, b| b.2.total_cmp(&a.2));
        let value: f64 = goods.iter().map(|g| g.2).sum();
        if value == 0.0 {
            continue;
        }
        let own = goods
            .iter()
            .take(PIE_GOODS)
            .take_while(|g| g.2 / value >= MIN_SECTOR)
            .count();
        let rest = &goods[own..];
        let rest_share = rest.iter().map(|g| g.2).sum::<f64>() / value;
        let rest_drawn = if rest.is_empty() {
            0.0
        } else {
            rest_share.max(MIN_SECTOR)
        };
        // The other sectors give up what a wider rest takes.
        let scale = (1.0 - rest_drawn) / (1.0 - rest_share);
        let mut sectors: Vec<Sector> = goods[..own]
            .iter()
            .map(|&(g, color, v)| Sector {
                goods: vec![g],
                color,
                share: v / value * scale,
            })
            .collect();
        if !rest.is_empty() {
            sectors.push(Sector {
                goods: rest.iter().map(|g| g.0).collect(),
                color: OTHER_FILL,
                share: rest_drawn,
            });
        }
        values.push((i, value, sectors));
    }
    let sat = saturation(&values.iter().map(|x| x.1).collect::<Vec<_>>());
    values
        .into_iter()
        .map(|(i, value, sectors)| {
            let p = &world.society.places[i];
            BurgCircle {
                id: p.id,
                at: p.at,
                share: (value / sat).min(1.0),
                sectors,
            }
        })
        .collect()
}

/// A deposit: an icon at the cell, or at the place of the cell.
struct ResourceIcon {
    cell: usize,
    at: [f64; 2],
    good: u32,
    color: &'static str,
    icon: &'static str,
}

/// The deposits of the cells (`buildResourceIcons`).
fn resource_icons(world: &World) -> Vec<ResourceIcon> {
    let e = &world.economy;
    let places = place_of_cell(world);
    let mut out = Vec::new();
    for (cell, &g) in world.society.cells.good.iter().enumerate() {
        let Some(good) = e.good(u32::from(g)) else {
            continue;
        };
        let at = places
            .get(&cell)
            .map_or(world.grid.points[cell], |&i| world.society.places[i].at);
        out.push(ResourceIcon {
            cell,
            at,
            good: good.id,
            color: good.color,
            icon: good.icon,
        });
    }
    out
}

/// The `stroke` and `stroke-width` of the lines of a goods icon, so that
/// they match the outline of its circle (`goodIconLines`).
fn icon_lines(ctx: &Ctx) -> String {
    let path = ["goods", "goodsIcons"];
    let size = ctx.style.option_f64(&path, "size").unwrap_or(2.5);
    let stroke = ctx
        .style
        .attr(&path, "stroke")
        .unwrap_or_else(|| "none".to_owned());
    let width = ctx.style.attr_f64(&path, "stroke-width").unwrap_or(1.0);
    format!(
        " stroke=\"{}\" stroke-width=\"{}\"",
        crate::extras::xml(&stroke),
        n(width * LINEWORK * ICON_FRAME / size)
    )
}

/// A pie: sectors clockwise from the top, the most valuable first
/// (`renderPie`).
fn pie(s: &mut String, x: f64, y: f64, r: f64, sectors: &[Sector]) {
    let ids = |goods: &[u32]| {
        goods
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    if let [one] = sectors {
        let _ = write!(
            s,
            "<circle data-goods=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
            ids(&one.goods),
            inkland_core::num::fixed(x, 1),
            inkland_core::num::fixed(y, 1),
            n(r),
            one.color
        );
        return;
    }
    let point = |a: f64| format!("{},{}", n(x + r * libm::sin(a)), n(y - r * libm::cos(a)));
    let mut start = 0.0;
    for sec in sectors {
        let end = start + sec.share * 2.0 * std::f64::consts::PI;
        let large = u8::from(end - start > std::f64::consts::PI);
        let _ = write!(
            s,
            "<path data-goods=\"{}\" d=\"M{},{}L{}A{},{} 0 {large} 1 {}Z\" fill=\"{}\"/>",
            ids(&sec.goods),
            n(x),
            n(y),
            point(start),
            n(r),
            n(r),
            point(end),
            sec.color
        );
        start = end;
    }
}

/// A line of trade traffic: a run of a route over one link.
pub struct TrafficLine {
    /// The path in page pixels.
    pub d: String,
    /// The part of the full width, at most 1.
    pub share: f64,
    /// The two cells of the link.
    pub cells: [usize; 2],
}

/// The link of each segment of a route. A segment inside one cell takes
/// the link that it leads into or out of (`getSegmentLinks`).
fn segment_links(cells: &[usize], index: &BTreeMap<(usize, usize), usize>) -> Vec<Option<usize>> {
    let k = cells.len().saturating_sub(1);
    let mut links = vec![None; k];
    let mut inside = vec![false; k];
    for i in 0..k {
        let (a, b) = (cells[i], cells[i + 1]);
        inside[i] = a == b;
        if !inside[i] {
            links[i] = index.get(&(a.min(b), a.max(b))).copied();
        }
    }
    for i in 1..k {
        if inside[i] {
            links[i] = links[i - 1];
        }
    }
    for i in (0..k.saturating_sub(1)).rev() {
        if inside[i] && links[i].is_none() {
            links[i] = links[i + 1];
        }
    }
    links
}

/// The routes with their cells, their points, and their curve, in the
/// order of FMG `pack.routes`: the roads and trails, then the sea routes.
fn routes(world: &World) -> Vec<(Vec<usize>, Vec<[f64; 2]>, f64)> {
    world
        .roads
        .iter()
        .chain(&world.sea_routes)
        .filter(|r| r.cells.len() >= 2)
        .map(|r| {
            let points = if r.points.len() == r.cells.len() {
                r.points.clone()
            } else {
                r.cells.iter().map(|&c| world.grid.points[c]).collect()
            };
            let alpha = if r.group == "searoutes" { 0.5 } else { 0.1 };
            (r.cells.clone(), points, alpha)
        })
        .collect()
}

/// Each route stretch that carried goods, one link at a time; a link that
/// two routes share is drawn one time (`buildTraffic`). The widest lines
/// come first.
pub fn traffic_lines(ctx: &Ctx) -> Vec<TrafficLine> {
    let world = ctx.world;
    let links = &world.economy.traffic;
    let units: Vec<f64> = links
        .iter()
        .map(|l| l.goods.iter().map(|g| g.1).sum())
        .collect();
    let used: Vec<f64> = units.iter().copied().filter(|&u| u != 0.0).collect();
    let sat = percentile(&used, TRAFFIC_SATURATION).unwrap_or(0.0);
    if sat == 0.0 {
        return Vec::new();
    }
    let index: BTreeMap<(usize, usize), usize> = links
        .iter()
        .enumerate()
        .map(|(i, l)| ((l.cells[0].min(l.cells[1]), l.cells[0].max(l.cells[1])), i))
        .collect();
    let mut drawn_by: BTreeMap<usize, usize> = BTreeMap::new();
    let mut out = Vec::new();
    for (route, (cells, points, alpha)) in routes(world).into_iter().enumerate() {
        let segs = segment_links(&cells, &index);
        let mut start = 0;
        while start < segs.len() {
            let link = segs[start];
            let mut end = start;
            while end + 1 < segs.len() && segs[end + 1] == link {
                end += 1;
            }
            if let Some(l) = link {
                let share = (units[l] / sat).min(1.0);
                if share >= TRAFFIC_MIN_SHARE && *drawn_by.entry(l).or_insert(route) == route {
                    let run: Vec<[f64; 2]> =
                        points[start..end + 2].iter().map(|&p| ctx.p(p)).collect();
                    out.push(TrafficLine {
                        d: catmull_rom(&run, alpha),
                        share: share.sqrt(),
                        cells: links[l].cells,
                    });
                }
            }
            start = end + 1;
        }
    }
    // A stable sort: the narrowest lines on top.
    out.sort_by(|a, b| b.share.total_cmp(&a.share));
    out
}

/// Write the traffic lines with the widths of a style node
/// (`renderTraffic`).
pub fn write_traffic(s: &mut String, lines: &[TrafficLine], max_width: f64) {
    for line in lines {
        let width = (max_width * line.share).max(TRAFFIC_MIN_WIDTH);
        let _ = writeln!(
            s,
            "<path d=\"{}\" stroke-width=\"{}\" data-cells=\"{},{}\"/>",
            line.d,
            n(width),
            line.cells[0],
            line.cells[1]
        );
    }
}

/// Goods (`drawGoods`): the output of the cells, the traffic when the style
/// shows it, the pies of the places, and the deposit icons.
pub fn goods(s: &mut String, ctx: &Ctx) {
    let w = ctx.world;
    if w.economy.goods.is_empty() {
        return;
    }
    let st = &ctx.style;
    let on = |group: &str| st.option_bool(&["goods", group], "render");
    let cells = if on("goodsCells") {
        cell_overview(w)
    } else {
        Vec::new()
    };
    let traffic = if on("goodsTraffic") {
        traffic_lines(ctx)
    } else {
        Vec::new()
    };
    let burgs = if on("goodsBurgs") {
        burg_circles(w)
    } else {
        Vec::new()
    };
    let deposits = if on("goodsIcons") {
        resource_icons(w)
    } else {
        Vec::new()
    };
    if cells.is_empty() && traffic.is_empty() && burgs.is_empty() && deposits.is_empty() {
        return;
    }
    let g = &w.grid;
    let _ = writeln!(s, "<g id=\"goods\"{}>", ctx.attrs(&["goods"], &[]));

    let _ = writeln!(
        s,
        "<g id=\"goods-cells\"{}>",
        ctx.attrs(&["goods", "goodsCells"], &[])
    );
    for c in &cells {
        let poly: Vec<[f64; 2]> = g.polygons[c.cell]
            .iter()
            .map(|&v| g.vertices[v as usize])
            .collect();
        let (mut b0, mut b1) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in &poly {
            b0 = [b0[0].min(p[0]), b0[1].min(p[1])];
            b1 = [b1[0].max(p[0]), b1[1].max(p[1])];
        }
        if !in_view(ctx, [b0[0], b0[1], b1[0], b1[1]]) {
            continue;
        }
        let points: Vec<String> = poly
            .iter()
            .map(|&p| {
                let q = ctx.p(p);
                format!("{},{}", n(q[0]), n(q[1]))
            })
            .collect();
        let _ = writeln!(
            s,
            "<polygon id=\"goods-cell-{}\" points=\"{}\" fill=\"{}\" fill-opacity=\"{}\"/>",
            c.cell,
            points.join(" "),
            c.color,
            n(c.opacity)
        );
    }
    s.push_str("</g>\n");

    let _ = writeln!(
        s,
        "<g id=\"goods-traffic\" fill=\"none\"{}>",
        ctx.attrs(&["goods", "goodsTraffic"], &[])
    );
    let max_width = st
        .option_f64(&["goods", "goodsTraffic"], "size")
        .unwrap_or(3.0);
    write_traffic(s, &traffic, max_width);
    s.push_str("</g>\n");

    let _ = writeln!(
        s,
        "<g id=\"goods-burgs\"{}>",
        ctx.attrs(&["goods", "goodsBurgs"], &[])
    );
    // The area grows with the output; the largest radius is 1.5 times the
    // size of the style.
    let max_r = st
        .option_f64(&["goods", "goodsBurgs"], "size")
        .unwrap_or(3.0)
        * 1.5;
    for b in &burgs {
        if !in_view(ctx, [b.at[0], b.at[1], b.at[0], b.at[1]]) {
            continue;
        }
        let r = (max_r * b.share.sqrt()).max(0.3);
        let [x, y] = ctx.p(b.at);
        let _ = write!(s, "<g id=\"goods-burg-{}\">", b.id);
        pie(s, x, y, r, &b.sectors);
        s.push_str("</g>\n");
    }
    s.push_str("</g>\n");

    let _ = writeln!(
        s,
        "<g id=\"goods-icons\"{}>",
        ctx.attrs(&["goods", "goodsIcons"], &[])
    );
    let path = ["goods", "goodsIcons"];
    let size = st.option_f64(&path, "size").unwrap_or(2.5);
    let circle = st.option_bool(&path, "circle");
    let half = size / 2.0;
    let lines = icon_lines(ctx);
    for d in &deposits {
        if !in_view(ctx, [d.at[0], d.at[1], d.at[0], d.at[1]]) {
            continue;
        }
        let [x, y] = ctx.p(d.at);
        let _ = write!(
            s,
            "<g id=\"goods-icon-{}\" data-good=\"{}\">",
            d.cell, d.good
        );
        if circle {
            let _ = write!(
                s,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"{}\"/>",
                n(x),
                n(y),
                n(half),
                d.color,
                stroke_of(d.color)
            );
        }
        let _ = writeln!(
            s,
            "<use href=\"#{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{lines}/></g>",
            icons::kebab(&icons::goods_icon_path(d.icon)),
            n(x - half),
            n(y - half),
            n(size),
            n(size)
        );
    }
    s.push_str("</g>\n");
    s.push_str("</g>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_percentile_is_the_value_at_the_share() {
        let v = [5.0, 1.0, 4.0, 2.0, 3.0];
        assert_eq!(percentile(&v, 0.95), Some(4.0));
        assert_eq!(percentile(&v, 1.0), Some(5.0));
        assert_eq!(percentile(&[], 0.95), None);
        assert_eq!(saturation(&[0.0]), 1.0);
    }

    #[test]
    fn a_segment_inside_a_cell_takes_the_next_link() {
        let index: BTreeMap<(usize, usize), usize> = [((1, 2), 0), ((2, 3), 1)].into();
        assert_eq!(
            segment_links(&[1, 1, 2, 3, 3], &index),
            vec![Some(0), Some(0), Some(1), Some(1)]
        );
        assert_eq!(segment_links(&[4, 5], &index), vec![None]);
    }

    #[test]
    fn a_pie_goes_clockwise_from_the_top() {
        let mut s = String::new();
        let sectors = [
            Sector {
                goods: vec![1],
                color: "#ff0000",
                share: 0.75,
            },
            Sector {
                goods: vec![2, 3],
                color: OTHER_FILL,
                share: 0.25,
            },
        ];
        pie(&mut s, 10.0, 10.0, 2.0, &sectors);
        assert!(s.starts_with("<path data-goods=\"1\" d=\"M10,10L10,8A2,2 0 1 1 8,10Z\""));
        assert!(s.contains("data-goods=\"2,3\" d=\"M10,10L8,10A2,2 0 0 1 10,8Z\""));
        let mut s = String::new();
        pie(&mut s, 10.04, 10.0, 2.0, &sectors[..1]);
        assert_eq!(
            s,
            "<circle data-goods=\"1\" cx=\"10\" cy=\"10\" r=\"2\" fill=\"#ff0000\"/>"
        );
    }

    #[test]
    fn a_stroke_is_two_steps_darker() {
        assert_eq!(stroke_of("#ffffff"), "#7d7d7d");
    }
}
