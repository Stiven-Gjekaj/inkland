//! Colours for `colour` mode: the d3 colour functions that the original
//! program uses, and the colours of states, cultures, religions, and
//! provinces.

use inkland_core::rand::Rand;

/// An RGB colour, each channel from 0 to 255.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb(pub f64, pub f64, pub f64);

impl Rgb {
    /// Read `#rgb` or `#rrggbb`, and a few CSS names.
    pub fn parse(text: &str) -> Option<Rgb> {
        let t = text.trim().to_ascii_lowercase();
        let named = match t.as_str() {
            "white" => Some("#ffffff"),
            "black" => Some("#000000"),
            "tan" => Some("#d2b48c"),
            "green" => Some("#008000"),
            "teal" => Some("#008080"),
            "red" => Some("#ff0000"),
            "blue" => Some("#0000ff"),
            "gray" | "grey" => Some("#808080"),
            _ => None,
        };
        let t = named.map_or(t.clone(), str::to_owned);
        let hex = t.strip_prefix('#')?;
        let v = |s: &str| u8::from_str_radix(s, 16).ok().map(f64::from);
        match hex.len() {
            3 => {
                let c: Vec<f64> = hex
                    .chars()
                    .map(|c| v(&format!("{c}{c}")))
                    .collect::<Option<_>>()?;
                Some(Rgb(c[0], c[1], c[2]))
            }
            6 => Some(Rgb(v(&hex[0..2])?, v(&hex[2..4])?, v(&hex[4..6])?)),
            _ => None,
        }
    }

    /// `#rrggbb`, with each channel rounded and clamped, as d3 does.
    pub fn hex(self) -> String {
        let c = |v: f64| v.round().clamp(0.0, 255.0) as u8;
        format!("#{:02x}{:02x}{:02x}", c(self.0), c(self.1), c(self.2))
    }

    /// d3 `color.darker(k)`.
    pub fn darker(self, k: f64) -> Rgb {
        let f = libm::pow(0.7, k);
        Rgb(self.0 * f, self.1 * f, self.2 * f)
    }

    /// d3 `color.brighter(k)`.
    pub fn brighter(self, k: f64) -> Rgb {
        let f = libm::pow(1.0 / 0.7, k);
        Rgb(self.0 * f, self.1 * f, self.2 * f)
    }

    /// d3 `interpolateRgb(a, b)(t)`.
    pub fn mix(self, other: Rgb, t: f64) -> Rgb {
        Rgb(
            self.0 + (other.0 - self.0) * t,
            self.1 + (other.1 - self.1) * t,
            self.2 + (other.2 - self.2) * t,
        )
    }
}

/// d3 `interpolateRgbBasis(colors)(t)`: a B-spline through the colours.
pub fn rgb_basis(colors: &[Rgb], t: f64) -> Rgb {
    let ch = |f: &dyn Fn(&Rgb) -> f64| -> f64 {
        let values: Vec<f64> = colors.iter().map(f).collect();
        basis(&values, t)
    };
    Rgb(ch(&|c| c.0), ch(&|c| c.1), ch(&|c| c.2))
}

fn basis(values: &[f64], t: f64) -> f64 {
    let n = values.len() - 1;
    let (i, t) = if t <= 0.0 {
        (0, 0.0)
    } else if t >= 1.0 {
        (n - 1, 1.0)
    } else {
        ((t * n as f64).floor() as usize, t)
    };
    let v1 = values[i];
    let v2 = values[i + 1];
    let v0 = if i > 0 { values[i - 1] } else { 2.0 * v1 - v2 };
    let v3 = if i < n - 1 {
        values[i + 2]
    } else {
        2.0 * v2 - v1
    };
    let t1 = (t - i as f64 / n as f64) * n as f64;
    let t2 = t1 * t1;
    let t3 = t2 * t1;
    ((1.0 - 3.0 * t1 + 3.0 * t2 - t3) * v0
        + (4.0 - 6.0 * t2 + 3.0 * t3) * v1
        + (1.0 + 3.0 * t1 + 3.0 * t2 - 3.0 * t3) * v2
        + t3 * v3)
        / 6.0
}

fn scheme(hex: &str) -> Vec<Rgb> {
    (0..hex.len() / 6)
        .filter_map(|i| Rgb::parse(&format!("#{}", &hex[6 * i..6 * i + 6])))
        .collect()
}

/// A colour scheme of the heightmap layer (`heightmap-color-schemes.ts`).
/// An unknown name, or a list `#hex,#hex,...`, makes a custom scheme.
pub fn height_scheme(name: &str) -> Vec<Rgb> {
    match name {
        "bright" => scheme("9e0142d53e4ff46d43fdae61fee08bffffbfe6f598abdda466c2a53288bd5e4fa2"),
        "light" => scheme("a50026d73027f46d43fdae61fee08bffffbfd9ef8ba6d96a66bd631a9850006837"),
        "green" => scheme("f7fcf5e5f5e0c7e9c0a1d99b74c47641ab5d238b45006d2c00441b"),
        "monochrome" => scheme("fffffff0f0f0d9d9d9bdbdbd969696737373525252252525000000"),
        "natural" => ["white", "#EEEECC", "tan", "green", "teal"]
            .iter()
            .filter_map(|c| Rgb::parse(c))
            .collect(),
        "olive" => ["#ffffff", "#cea48d", "#d5b085", "#0c2c19", "#151320"]
            .iter()
            .filter_map(|c| Rgb::parse(c))
            .collect(),
        "livid" => ["#BBBBDD", "#2A3440", "#17343B", "#0A1E24"]
            .iter()
            .filter_map(|c| Rgb::parse(c))
            .collect(),
        other => {
            let list: Vec<Rgb> = other.split(',').filter_map(Rgb::parse).collect();
            if list.len() >= 2 {
                list
            } else {
                height_scheme("bright")
            }
        }
    }
}

/// d3 `interpolateSpectral(t)`.
pub fn spectral(t: f64) -> Rgb {
    rgb_basis(&height_scheme("bright"), t)
}

/// The colour of a height in a scheme (`HeightmapColorSchemes.getColor`).
pub fn height_color(h: f64, colors: &[Rgb]) -> Rgb {
    let v = if h < 20.0 { h - 5.0 } else { h };
    rgb_basis(colors, 1.0 - v / 100.0)
}

/// d3 `interpolateRainbow(t)`, through the cubehelix colour space.
pub fn rainbow(t: f64) -> Rgb {
    let t = if !(0.0..=1.0).contains(&t) {
        t - t.floor()
    } else {
        t
    };
    let ts = (t - 0.5).abs();
    let h = 360.0 * t - 100.0;
    let s = 1.5 - 1.5 * ts;
    let l = 0.8 - 0.9 * ts;
    let hr = (h + 120.0).to_radians();
    let a = s * l * (1.0 - l);
    let (cosh, sinh) = (libm::cos(hr), libm::sin(hr));
    Rgb(
        255.0 * (l + a * (-0.14861 * cosh + 1.78277 * sinh)),
        255.0 * (l + a * (-0.29227 * cosh - 0.90649 * sinh)),
        255.0 * (l + a * (1.97294 * cosh)),
    )
}

/// The 12 pastel colours of the original program (`C_12`).
pub const C_12: [&str; 12] = [
    "#dababf", "#fb8072", "#80b1d3", "#fdb462", "#b3de69", "#fccde5", "#c6b9c1", "#bc80bd",
    "#ccebc5", "#ffed6f", "#8dd3c7", "#eb8de7",
];

/// `getColors(count)`: the pastel colours, then rainbow colours, in a
/// random order.
pub fn colors(count: usize, rng: &mut Rand) -> Vec<String> {
    let mut list: Vec<String> = (0..count)
        .map(|i| {
            if i < 12 {
                C_12[i].to_owned()
            } else {
                rainbow((i - 12) as f64 / (count - 12) as f64).hex()
            }
        })
        .collect();
    // Fisher-Yates, as d3 `shuffle`.
    let mut m = list.len();
    while m > 1 {
        let i = (rng.unit() * m as f64) as usize;
        m -= 1;
        list.swap(m, i.min(m));
    }
    list
}

/// `getRandomColor()`.
pub fn random_color(rng: &mut Rand) -> String {
    rainbow(rng.unit()).hex()
}

/// `getMixedColor(color, mix, bright)`.
pub fn mixed_color(color: &str, mix: f64, bright: f64, rng: &mut Rand) -> String {
    let base = Rgb::parse(color).unwrap_or_else(|| rainbow(rng.unit()));
    let other = rainbow(rng.unit());
    base.mix(other, mix).brighter(bright).hex()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip_and_darker() {
        let c = Rgb::parse("#466eab").unwrap();
        assert_eq!(c.hex(), "#466eab");
        assert_eq!(Rgb::parse("#fff").unwrap().hex(), "#ffffff");
        assert_eq!(Rgb::parse("white").unwrap().hex(), "#ffffff");
        assert_eq!(Rgb(100.0, 50.0, 10.0).darker(1.0).hex(), "#462307");
    }

    #[test]
    fn spectral_ends_are_the_scheme_ends() {
        assert_eq!(spectral(0.0).hex(), "#9e0142");
        assert_eq!(spectral(1.0).hex(), "#5e4fa2");
    }

    #[test]
    fn rainbow_matches_d3() {
        // d3.interpolateRainbow(0) is rgb(110, 64, 170).
        assert_eq!(rainbow(0.0).hex(), "#6e40aa");
        // d3.interpolateRainbow(0.5) is rgb(175, 240, 91).
        assert_eq!(rainbow(0.5).hex(), "#aff05b");
    }

    #[test]
    fn colors_has_each_pastel_once() {
        let mut rng = Rand::new(
            inkland_core::seed::SeedTree::new(1)
                .part_rng(inkland_core::seed::Layer::Decoration, "test"),
        );
        let list = colors(14, &mut rng);
        assert_eq!(list.len(), 14);
        for c in C_12 {
            assert_eq!(list.iter().filter(|x| *x == c).count(), 1);
        }
    }
}
