//! The relief effects of inkland in `colour` mode: hill shading from the
//! heights (`render.hill_shading`), and echo lines in the sea along the
//! coast (`render.coast_echo`). The original program does not have them.

use super::super::Ctx;
use super::super::color::Rgb;
use super::super::geom::{self, n};
use std::fmt::Write as _;

/// The rise in height units across one cell spacing that makes a slope of
/// 45 degrees for the light.
const RELIEF_SCALE: f64 = 6.0;
/// The steps of the shade. A step is `SHADE_STEP` of light.
const SHADE_STEP: f64 = 0.08;
/// The darkest step and the lightest step.
const DARK_STEPS: u32 = 6;
const LIGHT_STEPS: u32 = 6;
/// The opacity of one dark step and of one light step.
const DARK_OPACITY: f64 = 0.06;
const LIGHT_OPACITY: f64 = 0.07;

/// The light comes from the north-west, 45 degrees above the horizon. The
/// y axis of the page goes down, so north is -y.
fn light() -> [f64; 3] {
    let h = std::f64::consts::FRAC_1_SQRT_2;
    [-h * h, -h * h, h]
}

/// The shade of each cell of the second grid, from -1 (in full shadow) to
/// 1 (in full light), less the shade of flat land. Water cells get 0.
pub fn shades(ctx: &Ctx) -> Vec<f64> {
    let w = ctx.world;
    let g = &w.grid;
    let spacing = libm::sqrt(g.step_x * g.step_y).max(1e-9);
    (0..g.len())
        .map(|c| {
            if !w.features.is_land(c) {
                return 0.0;
            }
            // The gradient of the height: a least-squares plane through the
            // neighbours. Water neighbours count at sea level.
            let p = g.points[c];
            let h0 = w.heights[c];
            let (mut a, mut b, mut cc, mut bx, mut by) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for m in g.neighbors(c) {
                let q = g.points[m];
                let dx = (q[0] - p[0]) / spacing;
                let dy = (q[1] - p[1]) / spacing;
                let dh = (w.heights[m].max(w.sea_level) - h0) / RELIEF_SCALE;
                a += dx * dx;
                b += dx * dy;
                cc += dy * dy;
                bx += dx * dh;
                by += dy * dh;
            }
            let det = a * cc - b * b;
            if det.abs() < 1e-12 {
                return 0.0;
            }
            let gx = (cc * bx - b * by) / det;
            let gy = (a * by - b * bx) / det;
            shade_of(gx, gy)
        })
        .collect()
}

/// The shade of a slope with the height gradient `[gx, gy]` (in relief
/// units for each cell spacing): more than 0 on a slope that turns to the
/// light, less than 0 on a slope that turns away from it, and 0 on flat
/// land and on a slope along the light. Only the turn of the slope to the
/// light counts. A steep slope that turns neither way is not dark, so that
/// rough land does not show a shadow all over.
fn shade_of(gx: f64, gy: f64) -> f64 {
    let l = light();
    let norm = libm::sqrt(gx * gx + gy * gy + 1.0);
    (-gx * l[0] - gy * l[1]) / norm
}

/// The step of a shade: 1 to `DARK_STEPS` in shadow, `DARK_STEPS + 1` and
/// more in light, 0 for no change.
fn step(shade: f64) -> u32 {
    let k = libm::round(shade.abs() / SHADE_STEP) as u32;
    if k == 0 {
        0
    } else if shade < 0.0 {
        k.min(DARK_STEPS)
    } else {
        DARK_STEPS + k.min(LIGHT_STEPS)
    }
}

/// Hill shading: a dark and a light area for each step of the shade,
/// blurred, on the land only.
pub fn hills(s: &mut String, ctx: &Ctx) {
    if !ctx.config.render.hill_shading {
        return;
    }
    let w = ctx.world;
    let shade = shades(ctx);
    let rings = geom::rings_on(&w.grid, &|c| step(shade[c]));
    if rings.is_empty() {
        return;
    }
    let spacing = libm::sqrt(w.grid.step_x * w.grid.step_y) * ctx.px.unit_px;
    let rect = ctx.map_rect_attrs(10.0);
    ctx.def(
        "hill-shading-blur",
        format!(
            "<filter id=\"hill-shading-blur\" filterUnits=\"userSpaceOnUse\" {rect}><feGaussianBlur stdDeviation=\"{}\"/></filter>",
            n(spacing * 0.6)
        ),
    );
    let _ = writeln!(
        s,
        "<g id=\"hill-shading\" mask=\"url(#land)\"><g filter=\"url(#hill-shading-blur)\" stroke=\"none\">"
    );
    for (k, list) in &rings {
        let mut d = String::new();
        for ring in list {
            let pts: Vec<[f64; 2]> = ring
                .iter()
                .map(|e| ctx.p(w.grid.vertices[e.from as usize]))
                .collect();
            if pts.len() >= 3 {
                d.push_str(&geom::polygon_path(&pts));
            }
        }
        if d.is_empty() {
            continue;
        }
        let (fill, opacity) = if *k <= DARK_STEPS {
            ("#000000", f64::from(*k) * DARK_OPACITY)
        } else {
            ("#ffffff", f64::from(k - DARK_STEPS) * LIGHT_OPACITY)
        };
        let _ = writeln!(
            s,
            "<path id=\"hill-shade-{k}\" d=\"{d}\" fill=\"{fill}\" fill-opacity=\"{}\" fill-rule=\"evenodd\"/>",
            inkland_core::num::fixed(opacity, 3)
        );
    }
    s.push_str("</g></g>\n");
}

/// The coast echo lines: the distance of each line from the coast, its
/// width, both in millimetres on the page, and its strength from 0 to 1.
pub const ECHO: [(f64, f64, f64); 4] = [
    (0.7, 0.2, 1.0),
    (1.65, 0.17, 0.72),
    (2.85, 0.14, 0.48),
    (4.3, 0.12, 0.28),
];

/// The colour of the echo lines: the colour of the sea, darker. A sea
/// without a plain colour gets a blue.
fn echo_colour(sea: Option<String>) -> String {
    sea.as_deref()
        .and_then(Rgb::parse)
        .unwrap_or(Rgb(70.0, 110.0, 171.0))
        .darker(1.6)
        .hex()
}

/// Coast echo lines: thin lines in the sea that follow the coast of each
/// island, each further from the coast and fainter. A mask holds the
/// lines: for each line, from the outer line in, a stroke of the coast as
/// wide as the far edge of the line, then a black stroke as wide as its
/// near edge. Lines of two islands that are near join as one line.
pub fn echo(s: &mut String, ctx: &Ctx) {
    if !ctx.config.render.coast_echo {
        return;
    }
    let coasts: Vec<u32> = ctx
        .outlines
        .iter()
        .filter(|o| !o.lake && !o.in_lake)
        .map(|o| o.feature)
        .collect();
    if coasts.is_empty() {
        return;
    }
    let mm = ctx.px.mm;
    let uses = |stroke: &str, width: f64| -> String {
        let mut g = format!("<g stroke=\"{stroke}\" stroke-width=\"{}\">", n(width / mm));
        for f in &coasts {
            let _ = write!(g, "<use href=\"#feature-{f}\"/>");
        }
        g.push_str("</g>");
        g
    };
    let mut lines = String::new();
    for &(at, width, strength) in ECHO.iter().rev() {
        let grey = libm::round(255.0 * strength) as u8;
        lines.push_str(&uses(
            &format!("#{grey:02x}{grey:02x}{grey:02x}"),
            2.0 * (at + width / 2.0),
        ));
        lines.push_str(&uses("#000000", 2.0 * (at - width / 2.0)));
    }
    let rect = ctx.map_rect_attrs(10.0);
    ctx.def(
        "coastal-echo",
        format!(
            "<mask id=\"coastal-echo\" maskUnits=\"userSpaceOnUse\" {rect}><g fill=\"none\" stroke-linejoin=\"round\" stroke-linecap=\"round\">{lines}</g></mask>"
        ),
    );
    let colour = echo_colour(ctx.style.attr(&["ocean", "base"], "fill"));
    let _ = writeln!(
        s,
        "<g id=\"coast-echo\"><rect {} fill=\"{colour}\" mask=\"url(#coastal-echo)\"/></g>",
        ctx.map_rect_attrs(0.0)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_land_has_no_shade_step() {
        assert_eq!(step(0.0), 0);
        assert_eq!(step(0.03), 0);
    }

    #[test]
    fn the_steps_are_dark_in_shadow_and_light_in_light() {
        assert_eq!(step(-0.08), 1);
        assert_eq!(step(-0.5), DARK_STEPS);
        assert_eq!(step(0.08), DARK_STEPS + 1);
        assert_eq!(step(0.9), DARK_STEPS + LIGHT_STEPS);
    }

    #[test]
    fn the_light_comes_from_the_north_west() {
        let l = light();
        assert!(l[0] < 0.0 && l[1] < 0.0 && l[2] > 0.0);
        let len = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_echo_colour_is_darker_than_the_sea() {
        let echo = echo_colour(Some("#c7ecff".into()));
        assert_eq!(echo, Rgb(199.0, 236.0, 255.0).darker(1.6).hex());
        assert_eq!(echo_colour(None), echo_colour(Some("url(#x)".into())));
    }

    #[test]
    fn each_echo_line_is_further_out_and_fainter() {
        for w in ECHO.windows(2) {
            // The far edge of a line is nearer than the near edge of the next.
            assert!(w[0].0 + w[0].1 / 2.0 < w[1].0 - w[1].1 / 2.0);
            // The next line is fainter.
            assert!(w[1].2 < w[0].2);
        }
        // The gaps grow.
        let gaps: Vec<f64> = ECHO.windows(2).map(|w| w[1].0 - w[0].0).collect();
        assert!(gaps.windows(2).all(|g| g[0] < g[1]));
    }

    #[test]
    fn only_the_turn_to_the_light_shades_a_slope() {
        // The light comes from the north-west: -x and -y.
        assert_eq!(shade_of(0.0, 0.0), 0.0);
        // A slope that goes up to the south-east turns to the light.
        assert!(shade_of(1.0, 1.0) > 0.0);
        // A slope that goes up to the north-west turns away from it.
        assert!(shade_of(-1.0, -1.0) < 0.0);
        // A steep slope along the light, up to the north-east, is not dark.
        assert!(shade_of(5.0, -5.0).abs() < 1e-12);
        // A steeper slope that turns away is darker.
        assert!(shade_of(-2.0, -2.0) < shade_of(-0.5, -0.5));
    }
}
