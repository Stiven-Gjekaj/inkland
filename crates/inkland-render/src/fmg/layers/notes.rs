//! Journeys (`draw-journeys.ts`) and measurers (`draw-measurers.ts`).

use super::super::Ctx;
use super::super::geom::{self, catmull_rom, n};
use crate::extras::xml;
use inkland_core::annotations::{Measurer, MeasurerKind};
use inkland_core::num::fixed;
use inkland_core::rand::rn;
use std::fmt::Write as _;

/// The start dot and the end arrow of a journey segment (`index.html`).
/// Both take the stroke colour of the segment.
const JOURNEY_START: &str = "<marker id=\"journey-start\" viewBox=\"0 0 10 10\" refX=\"5\" refY=\"5\" markerWidth=\"2.4\" markerHeight=\"2.4\"><circle cx=\"5\" cy=\"5\" r=\"4\" fill=\"context-stroke\" stroke=\"#fff\" stroke-width=\"1\"/></marker>";
const JOURNEY_ARROW: &str = "<marker id=\"journey-arrow\" viewBox=\"0 0 10 10\" refX=\"8\" refY=\"5\" markerWidth=\"3\" markerHeight=\"3\" orient=\"auto-start-reverse\"><path d=\"M 0 0 L 10 5 L 0 10 z\" fill=\"context-stroke\"/></marker>";

/// Journeys (`drawJourneys`): a curve for each segment that moves, in the
/// colour of the journey, with a dot at its start and an arrow at its end
/// (`#journeys` in `index.css`).
pub fn journeys(s: &mut String, ctx: &Ctx) {
    let list = &ctx.world.journeys;
    if list
        .iter()
        .all(|j| j.segments.iter().all(|g| g.points.len() < 2))
    {
        return;
    }
    ctx.def("journey-start", JOURNEY_START.to_owned());
    ctx.def("journey-arrow", JOURNEY_ARROW.to_owned());
    let arrow = if ctx.style.option_bool(&["journeys"], "arrows") {
        " marker-end=\"url(#journey-arrow)\""
    } else {
        ""
    };
    let _ = writeln!(
        s,
        "<g id=\"journeys\" fill=\"none\" marker-start=\"url(#journey-start)\" marker-end=\"url(#journey-arrow)\"{}>",
        ctx.attrs(&["journeys"], &[])
    );
    for j in list {
        let _ = write!(s, "<g id=\"journey-{}\">", j.id);
        for g in j.segments.iter().filter(|g| g.points.len() > 1) {
            let pts: Vec<[f64; 2]> = g.points.iter().map(|p| ctx.p(p.at)).collect();
            let _ = write!(
                s,
                "<path id=\"segment-{}-{}\" d=\"{}\" stroke=\"{}\"{arrow}/>",
                j.id,
                g.id,
                catmull_rom(&pts, 0.5),
                xml(&j.color)
            );
        }
        s.push_str("</g>\n");
    }
    s.push_str("</g>\n");
}

/// A length in world units as text in the unit of the config
/// (`getDistance`).
fn distance(ctx: &Ctx, length: f64) -> String {
    format!(
        "{} {}s",
        fixed(rn(length, 0), 0),
        xml(&ctx.config.units.name)
    )
}

/// The points of the measurer: the round dots at its ends, and grey dots
/// at its control points (`#ruler .rulerPoints`).
fn measurer_points(s: &mut String, points: &[[f64; 2]], edges_only: bool) {
    s.push_str("<g font-size=\"4\" stroke-width=\"1\" stroke=\"#4e5a69\" fill=\"yellow\" stroke-dasharray=\"none\">");
    let last = points.len().saturating_sub(1);
    for (i, p) in points.iter().enumerate() {
        let edge = i == 0 || i == last;
        if edges_only && !edge {
            continue;
        }
        let fill = if edge { "" } else { " fill=\"#999999\"" };
        let _ = write!(
            s,
            "<circle r=\"1em\" cx=\"{}\" cy=\"{}\"{fill}/>",
            n(p[0]),
            n(p[1])
        );
    }
    s.push_str("</g>");
}

/// The label of a measurer at a point.
fn measurer_text(s: &mut String, ctx: &Ctx, at: [f64; 2], text: &str, offset: bool) {
    if ctx.style.attr(&["rulers"], "data-labels").as_deref() == Some("false") {
        return;
    }
    let dx = if offset {
        " dx=\".35em\" dy=\"-.45em\""
    } else {
        ""
    };
    let _ = write!(
        s,
        "<text x=\"{}\" y=\"{}\"{dx}>{text}</text>",
        n(at[0]),
        n(at[1])
    );
}

/// One measurer in page pixels (`renderRuler`, `renderPathMeasurer`,
/// `renderPlanimeter`).
fn measurer(s: &mut String, ctx: &Ctx, i: usize, m: &Measurer, width: f64, dash: &str) {
    let pts: Vec<[f64; 2]> = m.points.iter().map(|&p| ctx.p(p)).collect();
    if pts.len() < 2 {
        return;
    }
    let lines = |s: &mut String, tag: &str, attr: &str| {
        let _ = write!(
            s,
            "<{tag} {attr} stroke=\"currentColor\" stroke-width=\"{}\" stroke-dasharray=\"none\"/><{tag} {attr} stroke-width=\"{}\" stroke-dasharray=\"{}\"/>",
            n(width),
            n(width),
            xml(dash)
        );
    };
    match m.kind {
        MeasurerKind::Ruler => {
            let points: Vec<String> = pts
                .iter()
                .map(|p| format!("{},{}", n(p[0]), n(p[1])))
                .collect();
            let _ = write!(s, "<g id=\"ruler-{i}\">");
            lines(s, "polyline", &format!("points=\"{}\"", points.join(" ")));
            measurer_points(s, &pts, false);
            measurer_text(s, ctx, pts[pts.len() - 1], &distance(ctx, m.length()), true);
        }
        MeasurerKind::Opisometer | MeasurerKind::RouteOpisometer => {
            let _ = write!(s, "<g id=\"opisometer-{i}\">");
            lines(s, "path", &format!("d=\"{}\"", catmull_rom(&pts, 0.5)));
            measurer_points(s, &pts, true);
            measurer_text(s, ctx, pts[pts.len() - 1], &distance(ctx, m.length()), true);
        }
        MeasurerKind::Planimeter => {
            let _ = write!(
                s,
                "<g id=\"planimeter-{i}\"><path d=\"{}\" fill=\"lightblue\" fill-rule=\"evenodd\" fill-opacity=\"0.5\" stroke=\"#737373\" stroke-width=\"{}\" stroke-dasharray=\"{}\"/>",
                geom::polygon_path(&pts),
                n(width),
                xml(dash)
            );
            if pts.len() >= 3 {
                let area = (geom::area2(&m.points) / 2.0).abs();
                let at = ctx.p(geom::polylabel(std::slice::from_ref(&m.points), 1.0));
                let text = format!(
                    "{} square {}s",
                    super::military::si(rn(area, 0)),
                    xml(&ctx.config.units.name)
                );
                measurer_text(s, ctx, at, &text, false);
            }
        }
    }
    s.push_str("</g>\n");
}

/// The measurers: the default ruler of FMG and any other measurer of the
/// world (`drawMeasurers`). The font, the line width, and the dashes come
/// from the style; the dots and the text from `#ruler` in `index.css`.
pub fn rulers(s: &mut String, ctx: &Ctx) {
    let list = &ctx.world.measurers;
    if list.iter().all(|m| m.points.len() < 2) {
        return;
    }
    let st = &ctx.style;
    let font_size = st.attr_f64(&["rulers"], "font-size").unwrap_or(20.0);
    let width = st.attr_f64(&["rulers"], "stroke-width").unwrap_or(1.0);
    let dash = st
        .attr(&["rulers"], "stroke-dasharray")
        .unwrap_or_else(|| "none".to_owned());
    let font = xml(&ctx.config.render.fonts.other);
    let _ = writeln!(
        s,
        "<g id=\"ruler\" fill=\"none\"{} font-size=\"{}\">",
        ctx.attrs(&["rulers"], &["font-size"]),
        n(font_size)
    );
    let _ = writeln!(s, "<g text-anchor=\"middle\" font-family=\"{font}\">");
    let mut body = String::new();
    for (i, m) in list.iter().enumerate() {
        measurer(&mut body, ctx, i, m, width, &dash);
    }
    // `#ruler text`: grey text with a white shadow. The shadow becomes a
    // halo.
    let halo = format!(
        "<text fill=\"#3d3d3d\" stroke=\"#ffffff\" stroke-width=\"{}\" stroke-opacity=\"0.85\" stroke-linejoin=\"round\" stroke-dasharray=\"none\" paint-order=\"stroke\" ",
        n(0.2 * font_size)
    );
    s.push_str(&body.replace("<text ", &halo));
    s.push_str("</g>\n</g>\n");
}
