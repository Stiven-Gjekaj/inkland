//! The regiments of the states (`draw-military.ts`): a box in the colour of
//! the state with the troops, and a square in a darker colour with the
//! icon of the regiment.

use super::super::Ctx;
use super::super::color::Rgb;
use super::super::geom::n;
use super::super::icons;
use crate::extras::xml;
use inkland_core::military::Regiment;
use inkland_core::num::fixed;
use inkland_core::rand::rn;
use std::fmt::Write as _;

/// A number with a unit prefix: `1.5K`, `12K`, `3.4M` (`si`).
pub fn si(v: f64) -> String {
    let (value, decimals, unit) = if v >= 1e9 {
        (v / 1e9, 1, "B")
    } else if v >= 1e8 {
        (v / 1e6, 0, "M")
    } else if v >= 1e6 {
        (v / 1e6, 1, "M")
    } else if v >= 1e4 {
        (v / 1e3, 0, "K")
    } else if v >= 1e3 {
        (v / 1e3, 1, "K")
    } else {
        (v, 0, "")
    };
    format!("{}{unit}", fixed(rn(value, decimals), decimals))
}

/// The troops of a regiment as its box shows them (`Military.getTotal`).
pub fn total(r: &Regiment) -> String {
    let troops = r.troops();
    if troops > if r.naval { 999.0 } else { 99_999.0 } {
        si(troops)
    } else {
        fixed(troops, 0)
    }
}

/// The icon reference of a regiment: a glyph id for an emoji, else the id
/// itself.
fn icon_id(icon: &str) -> String {
    if icon.starts_with("glyph-") || icons::file(&icon.replacen('-', "/", 1)).is_some() {
        icon.to_owned()
    } else {
        icons::glyph_id(icon)
    }
}

/// The regiments of each state (`drawMilitary`).
pub fn military(s: &mut String, ctx: &Ctx) {
    let soc = &ctx.world.society;
    if soc.states.iter().all(|st| st.military.is_empty()) {
        return;
    }
    let st = &ctx.style;
    let size = st.option_f64(&["military"], "boxSize").unwrap_or(3.0);
    let show_names = st.option_bool(&["military"], "showNames");
    let h = size * 2.0;
    let font = xml(&ctx.config.render.fonts.other);
    // `#armies text` of FMG: white text with a dark shadow. The shadow
    // becomes a halo.
    let text = format!(
        " fill=\"#ffffff\" fill-opacity=\"1\" stroke=\"#000000\" stroke-width=\"{}\" stroke-opacity=\"0.85\" stroke-linejoin=\"round\" paint-order=\"stroke\" dominant-baseline=\"central\" text-anchor=\"middle\" font-family=\"{font}\"",
        n(0.2 * h)
    );
    let _ = writeln!(
        s,
        "<g id=\"armies\" font-size=\"{}\"{}>",
        n(h),
        ctx.attrs(&["military"], &[])
    );
    for state in &soc.states {
        if state.military.is_empty() {
            continue;
        }
        let color = ctx
            .palette
            .states
            .get(&state.id)
            .filter(|c| c.starts_with('#'))
            .cloned()
            .unwrap_or_else(|| "#999".to_owned());
        let darker =
            Rgb::parse(&color).map_or_else(|| "#6b6b6b".to_owned(), |c| c.darker(1.0).hex());
        let _ = writeln!(
            s,
            "<g id=\"army-{}\" fill=\"{color}\" color=\"{darker}\">",
            state.id
        );
        for r in &state.military {
            let w = if r.naval { size * 4.0 } else { size * 6.0 };
            let [cx, cy] = ctx.p(r.at);
            let x = rn(cx - w / 2.0, 2);
            let y = rn(cy - size, 2);
            let icon = icon_id(&r.icon);
            let inset = if icon.starts_with("glyph-") {
                h * 0.1
            } else {
                0.0
            };
            let _ = write!(
                s,
                "<g id=\"regiment-{}-{}\" font-size=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/><text x=\"{}\" y=\"{}\"{text}>{}</text><rect fill=\"currentColor\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                state.id,
                r.id,
                n(h),
                n(x),
                n(y),
                n(w),
                n(h),
                n(cx),
                n(cy),
                total(r),
                n(x - h),
                n(y),
                n(h),
                n(h)
            );
            if !icon.is_empty() {
                let _ = write!(
                    s,
                    "<use href=\"#{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>",
                    xml(&icon),
                    n(x - h + inset),
                    n(y + inset),
                    n(h - 2.0 * inset),
                    n(h - 2.0 * inset)
                );
            }
            if show_names {
                let _ = write!(
                    s,
                    "<text x=\"{}\" y=\"{}\" font-size=\"{}px\"{text}>{}</text>",
                    n(cx - size),
                    n(cy + size * 2.3),
                    n(size * 1.6),
                    xml(&r.name)
                );
            }
            s.push_str("</g>\n");
        }
        s.push_str("</g>\n");
    }
    s.push_str("</g>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn si_gives_the_short_forms_of_fmg() {
        assert_eq!(si(999.0), "999");
        assert_eq!(si(1250.0), "1.3K");
        assert_eq!(si(12_345.0), "12K");
        assert_eq!(si(2_500_000.0), "2.5M");
        assert_eq!(si(250_000_000.0), "250M");
        assert_eq!(si(3e9), "3B");
    }

    #[test]
    fn an_emoji_icon_becomes_a_glyph() {
        assert_eq!(icon_id("\u{2694}\u{FE0F}"), "glyph-2694-fe0f");
        assert_eq!(icon_id("glyph-2694-fe0f"), "glyph-2694-fe0f");
        assert_eq!(icon_id("goods-amber"), "goods-amber");
    }
}
