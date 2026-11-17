//! The map view transform: from the page (in mm) to the screen (in logical
//! pixels), and from the screen to the world (in config units).
//!
//! A page point `q` (mm) is at the screen point `q * zoom + pan`.

use inkland_render::page::Page;

/// The smallest and the largest zoom, in screen pixels for each mm.
pub const MIN_ZOOM: f64 = 0.2;
pub const MAX_ZOOM: f64 = 400.0;

/// The zoom and the pan of the map view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    /// Screen pixels for each mm of the page.
    pub zoom: f64,
    /// The screen position of the top-left corner of the page.
    pub pan: [f64; 2],
}

impl Default for View {
    fn default() -> Self {
        View {
            zoom: 1.0,
            pan: [0.0, 0.0],
        }
    }
}

impl View {
    /// The view that shows the full page in the middle of a viewport of
    /// `size` pixels, with a small gap at the sides.
    pub fn fit(size: [f64; 2], page_mm: [f64; 2]) -> View {
        let zoom =
            ((size[0] / page_mm[0]).min(size[1] / page_mm[1]) * 0.95).clamp(MIN_ZOOM, MAX_ZOOM);
        View {
            zoom,
            pan: [
                (size[0] - page_mm[0] * zoom) / 2.0,
                (size[1] - page_mm[1] * zoom) / 2.0,
            ],
        }
    }

    /// A page point (mm) on the screen.
    pub fn to_screen(&self, q: [f64; 2]) -> [f64; 2] {
        [
            q[0] * self.zoom + self.pan[0],
            q[1] * self.zoom + self.pan[1],
        ]
    }

    /// A screen point on the page (mm).
    pub fn to_page(&self, s: [f64; 2]) -> [f64; 2] {
        [
            (s[0] - self.pan[0]) / self.zoom,
            (s[1] - self.pan[1]) / self.zoom,
        ]
    }

    /// A world point (config units) on the screen.
    pub fn world_to_screen(&self, page: &Page, p: [f64; 2]) -> [f64; 2] {
        self.to_screen(page.to_page(p))
    }

    /// A screen point in the world (config units).
    pub fn screen_to_world(&self, page: &Page, s: [f64; 2]) -> [f64; 2] {
        page.to_world(self.to_page(s))
    }

    /// Move the view by `d` screen pixels.
    pub fn pan_by(&mut self, d: [f64; 2]) {
        self.pan[0] += d[0];
        self.pan[1] += d[1];
    }

    /// Multiply the zoom by `factor`. The page point under the screen point
    /// `at` stays under it.
    pub fn zoom_at(&mut self, at: [f64; 2], factor: f64) {
        let q = self.to_page(at);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = [at[0] - q[0] * self.zoom, at[1] - q[1] * self.zoom];
    }

    /// Where an image that was rendered for the view `rendered` goes on the
    /// screen in this view: `[x, y, width, height]`. `size` is the size of
    /// the image in logical pixels. This gives a rough preview while the
    /// sharp image renders.
    pub fn placement(&self, rendered: &View, size: [f64; 2]) -> [f64; 4] {
        let k = self.zoom / rendered.zoom;
        [
            self.pan[0] - rendered.pan[0] * k,
            self.pan[1] - rendered.pan[1] * k,
            size[0] * k,
            size[1] * k,
        ]
    }
}

/// Distance between two screen points.
pub fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page(origin: &str) -> Page {
        let c: inkland::Config = serde_json::from_value(json!({
            "version": 1, "seed": 1,
            "frame": { "origin": origin, "y_axis": if origin == "top-left" { "south" } else { "north" },
                       "width": 400, "height": 300 },
            "variants": [{ "name": "v", "page": { "w_mm": 167, "h_mm": 250, "margin_mm": 5 } }]
        }))
        .unwrap();
        Page::new(&c, &c.variants[0])
    }

    fn near(a: [f64; 2], b: [f64; 2]) -> bool {
        distance(a, b) < 1e-9
    }

    #[test]
    fn fit_puts_the_page_in_the_middle() {
        let v = View::fit([1000.0, 500.0], [167.0, 250.0]);
        assert!((v.zoom - 500.0 / 250.0 * 0.95).abs() < 1e-12);
        let top_left = v.to_screen([0.0, 0.0]);
        let bottom_right = v.to_screen([167.0, 250.0]);
        assert!((top_left[0] + bottom_right[0] - 1000.0).abs() < 1e-9);
        assert!((top_left[1] + bottom_right[1] - 500.0).abs() < 1e-9);
    }

    #[test]
    fn screen_and_world_agree_for_each_origin() {
        for origin in ["top-left", "bottom-left"] {
            let p = page(origin);
            let mut v = View::fit([800.0, 600.0], [p.w_mm, p.h_mm]);
            v.zoom_at([300.0, 200.0], 3.0);
            v.pan_by([-40.0, 25.0]);
            for w in [[0.0, 0.0], [123.4, 56.7], [400.0, 300.0]] {
                let s = v.world_to_screen(&p, w);
                assert!(near(v.screen_to_world(&p, s), w), "{origin} {w:?}");
            }
        }
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut v = View::fit([800.0, 600.0], [167.0, 250.0]);
        let at = [321.0, 123.0];
        let q = v.to_page(at);
        v.zoom_at(at, 2.5);
        assert!(near(v.to_screen(q), at));
        v.zoom_at(at, 1e9);
        assert_eq!(v.zoom, MAX_ZOOM);
    }

    #[test]
    fn an_old_image_goes_where_its_page_is_now() {
        let old = View::fit([800.0, 600.0], [167.0, 250.0]);
        let mut now = old;
        now.zoom_at([100.0, 100.0], 2.0);
        now.pan_by([10.0, -5.0]);
        let [x, y, w, h] = now.placement(&old, [800.0, 600.0]);
        // A page point that the old image showed at screen point s is at
        // (x, y) + s * k now.
        let k = w / 800.0;
        assert!((h / 600.0 - k).abs() < 1e-12);
        let q = [50.0, 70.0];
        let s = old.to_screen(q);
        assert!(near([x + s[0] * k, y + s[1] * k], now.to_screen(q)));
    }
}
