//! Colors of FMG (`color-utils.ts`) with the d3 functions that they use:
//! `interpolateRainbow`, `interpolateRgb`, and `brighter`.
//!
//! Colors are data in `world.json`. The print-safe SVG does not use them.

use crate::rand::{Rand, js_round};

/// An RGB color with values from 0 to 255, not rounded.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rgb(f64, f64, f64);

impl Rgb {
    /// The color that d3 gets when it reads the text of this color: each
    /// value rounded and in 0 to 255.
    fn parsed(self) -> Rgb {
        let f = |v: f64| js_round(v).clamp(0.0, 255.0);
        Rgb(f(self.0), f(self.1), f(self.2))
    }

    fn hex(self) -> String {
        let Rgb(r, g, b) = self.parsed();
        format!("#{:02x}{:02x}{:02x}", r as u8, g as u8, b as u8)
    }

    fn from_hex(hex: &str) -> Option<Rgb> {
        let h = hex.strip_prefix('#')?;
        if h.len() != 6 {
            return None;
        }
        let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(f64::from);
        Some(Rgb(v(0)?, v(2)?, v(4)?))
    }
}

/// `d3.interpolateRainbow(t)`: a cubehelix color.
fn rainbow(t: f64) -> Rgb {
    let t = if !(0.0..=1.0).contains(&t) {
        t - t.floor()
    } else {
        t
    };
    let ts = (t - 0.5).abs();
    let hue = 360.0 * t - 100.0;
    let s = 1.5 - 1.5 * ts;
    let l = 0.8 - 0.9 * ts;
    let h = (hue + 120.0) * std::f64::consts::PI / 180.0;
    let a = s * l * (1.0 - l);
    let (cosh, sinh) = (libm::cos(h), libm::sin(h));
    Rgb(
        255.0 * (l + a * (-0.14861 * cosh + 1.78277 * sinh)),
        255.0 * (l + a * (-0.29227 * cosh + -0.90649 * sinh)),
        255.0 * (l + a * (1.97294 * cosh)),
    )
    .parsed()
}

/// A random color of the rainbow (`getRandomColor`).
pub fn random_color(rng: &mut Rand) -> String {
    rainbow(rng.unit()).hex()
}

/// A color that mixes `base` with a random color and makes it brighter
/// (`getMixedColor`).
pub fn mixed_color(base: &str, rng: &mut Rand, mix: f64, bright: f64) -> String {
    let a = Rgb::from_hex(base).unwrap_or_else(|| rainbow(rng.unit()));
    let b = rainbow(rng.unit());
    let m = Rgb(
        a.0 + (b.0 - a.0) * mix,
        a.1 + (b.1 - a.1) * mix,
        a.2 + (b.2 - a.2) * mix,
    )
    .parsed();
    let k = libm::pow(1.0 / 0.7, bright);
    Rgb(m.0 * k, m.1 * k, m.2 * k).hex()
}

/// The 12 colors of FMG `C_12`.
pub const C_12: [&str; 12] = [
    "#dababf", "#fb8072", "#80b1d3", "#fdb462", "#b3de69", "#fccde5", "#c6b9c1", "#bc80bd",
    "#ccebc5", "#ffed6f", "#8dd3c7", "#eb8de7",
];

/// `count` distinct colors in a random order (`getColors`): the 12 colors
/// of `C_12` first, then colors of the rainbow, shuffled with d3
/// `shuffler`.
pub fn distinct_colors(count: usize, rng: &mut Rand) -> Vec<String> {
    let mut colors: Vec<String> = (0..count)
        .map(|i| {
            if i < 12 {
                C_12[i].to_owned()
            } else {
                rainbow((i - 12) as f64 / (count - 12) as f64).hex()
            }
        })
        .collect();
    let mut m = colors.len();
    while m > 0 {
        let i = (rng.unit() * m as f64) as usize;
        m -= 1;
        colors.swap(m, i);
    }
    colors
}

/// The colors of `d3.schemeSet2`.
pub const SET2: [&str; 6] = [
    "#66c2a5", "#fc8d62", "#8da0cb", "#e78ac3", "#a6d854", "#ffd92f",
];

/// The colors of `d3.schemeCategory10`.
pub const CATEGORY10: [&str; 10] = [
    "#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd", "#8c564b", "#e377c2", "#7f7f7f",
    "#bcbd22", "#17becf",
];

/// The color of item `index` from `d3.schemeCategory10`, in a cycle
/// (`getCardinalColor`).
pub fn cardinal_color(index: usize) -> &'static str {
    CATEGORY10[index % CATEGORY10.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rainbow_matches_d3() {
        // d3.interpolateRainbow(0) is rgb(110, 64, 170); at 0.5 it is
        // rgb(175, 240, 91).
        assert_eq!(rainbow(0.0).hex(), "#6e40aa");
        assert_eq!(rainbow(0.5).hex(), "#aff05b");
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(Rgb::from_hex("#66c2a5").unwrap().hex(), "#66c2a5");
        assert!(Rgb::from_hex("red").is_none());
    }

    #[test]
    fn cardinal_colors_cycle() {
        assert_eq!(cardinal_color(0), "#1f77b4");
        assert_eq!(cardinal_color(9), "#17becf");
        assert_eq!(cardinal_color(10), "#1f77b4");
    }
}
