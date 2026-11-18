//! Journeys (`draw-journeys.ts`).

use super::super::Ctx;
use super::super::geom::catmull_rom;
use crate::extras::xml;
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
