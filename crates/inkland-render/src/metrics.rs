//! Text widths for label placement.
//!
//! The SVG keeps real text, and the book build loads the fonts. To place
//! labels without the font files, inkland keeps a table of advance widths
//! for each permitted family. `examples/font_metrics.rs` makes the table
//! from the font files. `metrics_styles` has the tables of the free fonts
//! of the style presets of `colour` mode, from `assets/fonts`.
//!
//! A family without a table, for example Georgia, uses a wide table: each character has the largest advance
//! of all the tables, made 10 % larger, and at least the advance of a
//! monospace font. A label in such a family is then never wider on the page
//! than the placer measured it, in the font or in a fallback font.

use crate::metrics_data as data;
use crate::metrics_styles as styles;

/// The metrics of one font.
#[derive(Debug)]
pub struct FontMetrics {
    pub family: &'static str,
    pub units_per_em: u16,
    /// Height of a capital letter, in font units.
    pub cap_height: i16,
    /// Depth below the baseline, in font units. It is less than zero.
    pub descender: i16,
    /// The character of the first advance.
    pub first: u32,
    /// Advance widths, in font units. 0: the font has no glyph.
    pub advances: &'static [u16],
}

impl FontMetrics {
    /// The metrics of a family. The name match ignores case. An unknown
    /// family uses the wide table.
    pub fn of(family: &str) -> &'static FontMetrics {
        // A list of families has the metrics of its first family: the font
        // that the map asks for, which a system can have.
        let first = family
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(|c| c == '\'' || c == '"');
        data::FONTS
            .iter()
            .chain(styles::STYLE_FONTS.iter())
            .find(|f| f.family.eq_ignore_ascii_case(first))
            .unwrap_or(&WIDE)
    }

    /// The advance of a character, in font units. A character without a
    /// glyph has the advance of `n`.
    fn advance(&self, c: char) -> f64 {
        let get = |c: char| {
            (c as u32)
                .checked_sub(self.first)
                .and_then(|i| self.advances.get(i as usize))
                .copied()
                .filter(|&a| a > 0)
        };
        f64::from(get(c).or_else(|| get('n')).unwrap_or(self.units_per_em / 2))
    }

    /// The width of a text, in the unit of `size`, with `spacing` added
    /// between the characters.
    pub fn width(&self, text: &str, size: f64, spacing: f64) -> f64 {
        let em = f64::from(self.units_per_em);
        let count = text.chars().count();
        let sum: f64 = text.chars().map(|c| self.advance(c)).sum();
        sum / em * size + spacing * count.saturating_sub(1) as f64
    }

    /// The height of a capital letter, in the unit of `size`.
    pub fn cap(&self, size: f64) -> f64 {
        f64::from(self.cap_height) / f64::from(self.units_per_em) * size
    }

    /// The depth below the baseline, in the unit of `size`. It is more than
    /// zero.
    pub fn descent(&self, size: f64) -> f64 {
        -f64::from(self.descender) / f64::from(self.units_per_em) * size
    }
}

/// The number of characters in each table.
const CHARS: usize = data::FONTS[0].advances.len();
/// The units in one em of the wide table.
const WIDE_EM: i64 = 1000;
/// The least advance of the wide table: the advance of a monospace font,
/// 0.6 em.
const WIDE_LEAST: i64 = 600;

/// A value of a table in the units of the wide table, rounded up.
const fn to_wide(v: i64, em: u16) -> i64 {
    let em = em as i64;
    (v * WIDE_EM + em - 1).div_euclid(em)
}

/// The advances of the wide table.
const fn wide_advances() -> [u16; CHARS] {
    let mut out = [0u16; CHARS];
    let mut i = 0;
    while i < CHARS {
        let mut most = 0i64;
        let mut k = 0;
        while k < data::FONTS.len() {
            let f = &data::FONTS[k];
            let a = to_wide(f.advances[i] as i64, f.units_per_em);
            if a > most {
                most = a;
            }
            k += 1;
        }
        if most > 0 {
            // 10 % larger, rounded up, and at least a monospace advance.
            let wide = (most * 11 + 9) / 10;
            out[i] = if wide > WIDE_LEAST { wide } else { WIDE_LEAST } as u16;
        }
        i += 1;
    }
    out
}

/// The largest cap height and the deepest descender of all the tables.
const fn wide_heights() -> (i16, i16) {
    let (mut cap, mut desc) = (0i64, 0i64);
    let mut k = 0;
    while k < data::FONTS.len() {
        let f = &data::FONTS[k];
        let c = to_wide(f.cap_height as i64, f.units_per_em);
        let d = -to_wide(-(f.descender as i64), f.units_per_em);
        if c > cap {
            cap = c;
        }
        if d < desc {
            desc = d;
        }
        k += 1;
    }
    (cap as i16, desc as i16)
}

const WIDE_ADVANCES: [u16; CHARS] = wide_advances();
const WIDE_HEIGHTS: (i16, i16) = wide_heights();

/// The table of a family that has no table of its own.
static WIDE: FontMetrics = FontMetrics {
    family: "",
    units_per_em: WIDE_EM as u16,
    cap_height: WIDE_HEIGHTS.0,
    descender: WIDE_HEIGHTS.1,
    first: data::FONTS[0].first,
    advances: &WIDE_ADVANCES,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_permitted_family_has_a_table() {
        for family in ["IM Fell English", "IM Fell English SC", "Caladea", "Inter"] {
            assert_eq!(FontMetrics::of(family).family, family);
        }
        assert_eq!(FontMetrics::of("im fell english").family, "IM Fell English");
    }

    #[test]
    fn a_longer_text_is_wider_and_spacing_adds_width() {
        let f = FontMetrics::of("IM Fell English");
        let a = f.width("Ostrey", 3.0, 0.0);
        let b = f.width("Ostrey Ford", 3.0, 0.0);
        assert!(a > 0.0 && b > a);
        assert!((f.width("abc", 3.0, 1.0) - f.width("abc", 3.0, 0.0) - 2.0).abs() < 1e-12);
        // W is wider than i.
        assert!(f.width("W", 3.0, 0.0) > f.width("i", 3.0, 0.0));
    }

    #[test]
    fn each_free_font_of_the_style_presets_has_a_table() {
        use crate::fmg::style::Style;
        use inkland_core::config::StyleName;
        // These fonts have no free license, and the program has no file.
        let not_free = ["Arial", "Courier New", "Georgia", "Times New Roman"];
        for &name in &StyleName::ALL {
            let s = Style::preset(name);
            for g in s.groups(&["labels"]) {
                let Some(family) = s.attr(&["labels", &g], "font-family") else {
                    continue;
                };
                let f = FontMetrics::of(&family);
                // A font without a free license comes with a list of free
                // fonts after it; its metrics are of the first family.
                let first = family.split(',').next().unwrap_or("").trim();
                if not_free.contains(&first) {
                    assert_eq!(f.family, "", "{family}");
                    if matches!(first, "Georgia" | "Times New Roman") {
                        assert!(family.contains("Caladea"), "{family} has a free serif");
                    }
                } else {
                    assert_eq!(f.family, first, "{name:?} {g}");
                }
            }
        }
        // A wide font is wider than the first table.
        let text = "Khuregkhum";
        assert!(
            FontMetrics::of("Eagle Lake").width(text, 10.0, 0.0)
                > FontMetrics::of("IM Fell English").width(text, 10.0, 0.0)
        );
    }

    #[test]
    fn an_unknown_family_is_at_least_as_wide_as_each_table() {
        let wide = FontMetrics::of("Georgia");
        assert_eq!(wide.family, "");
        assert!(std::ptr::eq(wide, FontMetrics::of("Courier New")));
        for text in ["Sanhaveiria", "SANHAVEIRIA", "Illi", "W\u{e9}st\u{f8}"] {
            let w = wide.width(text, 10.0, 0.0);
            for f in &data::FONTS {
                assert!(
                    w >= 1.1 * f.width(text, 10.0, 0.0) - 1e-9,
                    "{text} {}",
                    f.family
                );
            }
            // A monospace font has 0.6 em for each character.
            assert!(
                w >= 0.6 * 10.0 * text.chars().count() as f64 - 1e-9,
                "{text}"
            );
        }
        for f in &data::FONTS {
            assert!(wide.cap(10.0) >= f.cap(10.0) - 1e-9);
            assert!(wide.descent(10.0) >= f.descent(10.0) - 1e-9);
        }
    }

    #[test]
    fn a_character_without_a_glyph_has_a_width() {
        let f = FontMetrics::of("Caladea");
        assert!(f.width("\u{4e00}", 3.0, 0.0) > 0.0);
    }

    #[test]
    fn cap_height_and_descent_are_parts_of_the_size() {
        for family in ["IM Fell English", "Caladea", "Inter"] {
            let f = FontMetrics::of(family);
            assert!(f.cap(10.0) > 5.0 && f.cap(10.0) < 9.0, "{family}");
            assert!(f.descent(10.0) > 1.0 && f.descent(10.0) < 4.0, "{family}");
        }
    }
}
