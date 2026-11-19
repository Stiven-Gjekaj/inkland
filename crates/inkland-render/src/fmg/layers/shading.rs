//! The relief effects of inkland in `colour` mode: hill shading from the
//! heights (`render.hill_shading`), and echo lines in the sea along the
//! coast (`render.coast_echo`). The original program does not have them.

use super::super::Ctx;
use super::super::geom::{self, n};
use std::fmt::Write as _;

/// The rise in height units across one cell spacing that makes a slope of
/// 45 degrees for the light.
const RELIEF_SCALE: f64 = 6.0;
/// The steps of the shade. A step is `SHADE_STEP` of light.
const SHADE_STEP: f64 = 0.08;
/// The darkest step and the lightest step.
const DARK_STEPS: u32 = 6;
const LIGHT_STEPS: u32 = 3;
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
    let l = light();
    let flat = l[2];
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
            let norm = libm::sqrt(gx * gx + gy * gy + 1.0);
            let shade = (-gx * l[0] - gy * l[1] + l[2]) / norm;
            shade - flat
        })
        .collect()
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
}
