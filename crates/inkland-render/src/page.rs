//! Page geometry: from world units to millimetres on the page.

use inkland_core::config::{Config, Origin, Variant};

/// Millimetres in one point.
pub const MM_PER_PT: f64 = 25.4 / 72.0;

/// Points to millimetres.
pub fn pt(v: f64) -> f64 {
    v * MM_PER_PT
}

/// A line width in points to millimetres, rounded up to 3 decimals. The SVG
/// writes 3 decimals, and a width that rounds down could fall below the
/// least permitted width.
pub fn line_mm(v: f64) -> f64 {
    (pt(v) * 1000.0 - 1e-9).ceil() / 1000.0
}

/// Where the world goes on the page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Page {
    /// Page width and height, in mm.
    pub w_mm: f64,
    pub h_mm: f64,
    /// Millimetres for one world unit.
    pub scale: f64,
    /// Top-left corner of the map area on the page, in mm.
    pub ox: f64,
    pub oy: f64,
    /// The world rectangle on the page: `[x0, y0, x1, y1]` in units.
    pub view: [f64; 4],
    /// `true` if world y increases to the north (up on the page).
    pub flip_y: bool,
}

impl Page {
    /// Fit the variant's part of the world in the page, inside the margin.
    /// The map keeps its proportions and is centered.
    pub fn new(config: &Config, variant: &Variant) -> Page {
        let p = &variant.page;
        let view = variant
            .crop
            .unwrap_or([0.0, 0.0, config.frame.width, config.frame.height]);
        let (vw, vh) = (view[2] - view[0], view[3] - view[1]);
        let (aw, ah) = (p.w_mm - 2.0 * p.margin_mm, p.h_mm - 2.0 * p.margin_mm);
        let scale = (aw / vw).min(ah / vh);
        Page {
            w_mm: p.w_mm,
            h_mm: p.h_mm,
            scale,
            ox: p.margin_mm + (aw - vw * scale) / 2.0,
            oy: p.margin_mm + (ah - vh * scale) / 2.0,
            view,
            flip_y: config.frame.origin == Origin::BottomLeft,
        }
    }

    /// A world point on the page, in mm.
    pub fn to_page(&self, p: [f64; 2]) -> [f64; 2] {
        let x = self.ox + (p[0] - self.view[0]) * self.scale;
        let y = if self.flip_y {
            self.oy + (self.view[3] - p[1]) * self.scale
        } else {
            self.oy + (p[1] - self.view[1]) * self.scale
        };
        [x, y]
    }

    /// The map area on the page: `[x, y, width, height]` in mm.
    pub fn map_rect(&self) -> [f64; 4] {
        [
            self.ox,
            self.oy,
            (self.view[2] - self.view[0]) * self.scale,
            (self.view[3] - self.view[1]) * self.scale,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn config(origin: &str) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 1,
            "frame": { "origin": origin, "y_axis": if origin == "top-left" { "south" } else { "north" },
                       "width": 400, "height": 300 },
            "variants": [{ "name": "v", "page": { "w_mm": 167, "h_mm": 250, "margin_mm": 5 } }]
        }))
        .unwrap()
    }

    #[test]
    fn a_wide_world_fills_the_page_width() {
        let c = config("top-left");
        let p = Page::new(&c, &c.variants[0]);
        let [x, _, w, h] = p.map_rect();
        assert!((x - 5.0).abs() < 1e-9);
        assert!((w - 157.0).abs() < 1e-9);
        assert!((h - 157.0 * 0.75).abs() < 1e-9);
        // The map is centered in height.
        let top = p.oy;
        let bottom = 250.0 - (p.oy + h);
        assert!((top - bottom).abs() < 1e-9);
    }

    #[test]
    fn bottom_left_origin_puts_north_up() {
        let top_left = config("top-left");
        let bottom_left = config("bottom-left");
        let a = Page::new(&top_left, &top_left.variants[0]);
        let b = Page::new(&bottom_left, &bottom_left.variants[0]);
        // y = 0 is the top of the page with top-left, the bottom with bottom-left.
        assert!(a.to_page([0.0, 0.0])[1] < a.to_page([0.0, 300.0])[1]);
        assert!(b.to_page([0.0, 0.0])[1] > b.to_page([0.0, 300.0])[1]);
    }

    #[test]
    fn a_line_width_never_rounds_below_its_points() {
        for v in [0.5, 0.75, 1.0, 1.3, 2.0] {
            let mm = line_mm(v);
            assert!(mm / MM_PER_PT >= v, "{v} pt gives {mm} mm");
            assert_eq!(mm, inkland_core::num::round(mm, 3));
        }
    }

    #[test]
    fn one_point_is_the_documented_millimetres() {
        assert!((pt(72.0) - 25.4).abs() < 1e-12);
    }
}
