//! The PNG preview: a small raster of the SVG, so that an agent can look at
//! the map without a browser.
//!
//! The preview finds fonts in the system and in the given directories. The
//! SVG names its fonts by family; it never embeds them. A family that the
//! preview cannot find gives a warning, and the preview uses a fallback.

use std::path::Path;
use std::sync::Arc;

/// A PNG and the font families that the preview did not find.
pub struct Preview {
    pub png: Vec<u8>,
    pub missing_fonts: Vec<String>,
}

/// The font database of a preview. Loading the system fonts takes time, so
/// the desktop app loads them one time and keeps the database.
pub type FontDb = Arc<resvg::usvg::fontdb::Database>;

/// Load the system fonts and the fonts in `font_dirs`.
pub fn font_db(font_dirs: &[&Path]) -> FontDb {
    let mut db = resvg::usvg::fontdb::Database::new();
    db.load_system_fonts();
    for dir in font_dirs {
        db.load_fonts_dir(dir);
    }
    Arc::new(db)
}

/// A parsed SVG, ready to render many times at different scales.
pub struct Scene {
    tree: resvg::usvg::Tree,
    /// The font families that the database does not have.
    pub missing_fonts: Vec<String>,
}

impl Scene {
    /// Parse SVG text with the fonts of `db`. `families` are the font
    /// families to look for.
    pub fn parse(svg: &str, db: &FontDb, families: &[&str]) -> Result<Scene, String> {
        use resvg::usvg;
        // A font file can name its family with other upper and lower case
        // letters than the SVG, for example "IM FELL English". A book
        // stylesheet maps the name with @font-face; the preview maps it
        // here. The SVG file keeps the exact name.
        let mut svg = svg.to_owned();
        let mut missing: Vec<String> = Vec::new();
        for &f in families {
            let exact = usvg::fontdb::Query {
                families: &[usvg::fontdb::Family::Name(f)],
                ..Default::default()
            };
            if db.query(&exact).is_some() {
                continue;
            }
            let other = db
                .faces()
                .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
                .filter(|name| name.eq_ignore_ascii_case(f))
                .min();
            match other {
                Some(name) => {
                    svg = svg.replace(
                        &format!("font-family=\"{f}\""),
                        &format!("font-family=\"{name}\""),
                    );
                }
                None => missing.push(f.to_owned()),
            }
        }
        missing.sort();
        missing.dedup();
        let opt = usvg::Options {
            fontdb: Arc::clone(db),
            ..Default::default()
        };
        let tree =
            usvg::Tree::from_str(&svg, &opt).map_err(|e| format!("cannot read the SVG: {e}"))?;
        Ok(Scene {
            tree,
            missing_fonts: missing,
        })
    }

    /// The size of the SVG image in CSS pixels (96 for each inch):
    /// `[width, height]`. A page of 10 mm is about 37.8 pixels.
    pub fn size(&self) -> [f32; 2] {
        let s = self.tree.size();
        [s.width(), s.height()]
    }

    /// Render a part of the SVG into an image of `width` x `height` pixels.
    /// A point `q` of the SVG image, in CSS pixels, goes to the pixel
    /// `q * scale + offset`. The
    /// pixels are RGBA, 4 bytes for each pixel, row by row. The image is
    /// first filled with the opaque color `background`, so each pixel is
    /// opaque.
    pub fn render_rgba(
        &self,
        width: u32,
        height: u32,
        scale: f32,
        offset: [f32; 2],
        background: [u8; 3],
    ) -> Result<Vec<u8>, String> {
        use resvg::tiny_skia;
        let mut pixmap = tiny_skia::Pixmap::new(width.max(1), height.max(1))
            .ok_or_else(|| "cannot make the image".to_owned())?;
        pixmap.fill(tiny_skia::Color::from_rgba8(
            background[0],
            background[1],
            background[2],
            255,
        ));
        let t = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, offset[0], offset[1]);
        resvg::render(&self.tree, t, &mut pixmap.as_mut());
        Ok(pixmap.take())
    }
}

/// Render SVG text to PNG bytes. The longer side of the image is
/// `longest_px` pixels. `families` are the font families to look for.
pub fn svg_to_png(
    svg: &str,
    longest_px: u32,
    font_dirs: &[&Path],
    families: &[&str],
) -> Result<Preview, String> {
    use resvg::tiny_skia;
    let scene = Scene::parse(svg, &font_db(font_dirs), families)?;
    let [sw, sh] = scene.size();
    let scale = longest_px as f32 / sw.max(sh);
    let w = (sw * scale).round().max(1.0) as u32;
    let h = (sh * scale).round().max(1.0) as u32;
    let mut pixmap =
        tiny_skia::Pixmap::new(w, h).ok_or_else(|| "cannot make the image".to_owned())?;
    resvg::render(
        &scene.tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let png = pixmap
        .encode_png()
        .map_err(|e| format!("cannot write the PNG: {e}"))?;
    Ok(Preview {
        png,
        missing_fonts: scene.missing_fonts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer_side_has_the_wanted_size() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="167mm" height="250mm" viewBox="0 0 167 250"><rect width="167" height="250" fill="#ffffff"/><path d="M10 10L150 200" stroke="#000000"/></svg>"##;
        let p = svg_to_png(svg, 1024, &[], &[]).unwrap();
        assert_eq!(&p.png[1..4], b"PNG");
        let w = u32::from_be_bytes(p.png[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(p.png[20..24].try_into().unwrap());
        assert_eq!(h, 1024);
        assert_eq!(w, (167.0f32 * 1024.0 / 250.0).round() as u32);
    }

    #[test]
    fn a_missing_family_is_reported() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10mm" height="10mm" viewBox="0 0 10 10"/>"##;
        let p = svg_to_png(svg, 64, &[], &["No Such Family 12345"]).unwrap();
        assert_eq!(p.missing_fonts, vec!["No Such Family 12345".to_owned()]);
    }

    #[test]
    fn rgba_render_puts_the_svg_at_the_scale_and_offset() {
        // A black square from (2, 2) to (4, 4) on a 10 x 10 page.
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><rect width="10" height="10" fill="#ffffff"/><rect x="2" y="2" width="2" height="2" fill="#000000"/></svg>"##;
        let scene = Scene::parse(svg, &font_db(&[]), &[]).unwrap();
        assert_eq!(scene.size(), [10.0, 10.0]);
        // Scale 2 and offset (5, 5): the page is pixels 5 to 25, and the
        // square is pixels 9 to 13.
        let px = scene
            .render_rgba(30, 30, 2.0, [5.0, 5.0], [100, 100, 100])
            .unwrap();
        assert_eq!(px.len(), 30 * 30 * 4);
        let at = |x: usize, y: usize| &px[(y * 30 + x) * 4..(y * 30 + x) * 4 + 4];
        assert_eq!(at(1, 1), [100, 100, 100, 255], "the background");
        assert_eq!(at(7, 7), [255, 255, 255, 255], "the white page");
        assert_eq!(at(11, 11), [0, 0, 0, 255], "the black square");
        assert_eq!(at(27, 27), [100, 100, 100, 255], "the background");
    }
}
