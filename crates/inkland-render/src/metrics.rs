//! Text widths for label placement.
//!
//! The SVG keeps real text, and the book build loads the fonts. To place
//! labels without the font files, inkland keeps a table of advance widths
//! for each permitted family. `examples/font_metrics.rs` makes the table
//! from the font files.

use crate::metrics_data as data;

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
    /// family uses the first table.
    pub fn of(family: &str) -> &'static FontMetrics {
        data::FONTS
            .iter()
            .find(|f| f.family.eq_ignore_ascii_case(family.trim()))
            .unwrap_or(&data::FONTS[0])
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
