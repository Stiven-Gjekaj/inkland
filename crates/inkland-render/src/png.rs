//! The PNG preview: a small raster of the SVG, so that an agent can look at
//! the map without a browser.
//!
//! The preview finds fonts in the system, in the given directories, and in
//! the fonts that inkland has in its program: the free fonts (SIL Open Font
//! License) that the style presets and the print defaults name. The SVG
//! names its fonts by family; it never embeds them. A family that the
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

/// The font files in the program, from `assets/fonts`. Each directory
/// there has the license of its fonts.
const BUNDLED: &[&[u8]] = &[
    include_bytes!("../assets/fonts/almendrasc/AlmendraSC-Regular.ttf"),
    include_bytes!("../assets/fonts/arimamadurai/ArimaMadurai-Bold.ttf"),
    include_bytes!("../assets/fonts/arimamadurai/ArimaMadurai-Regular.ttf"),
    include_bytes!("../assets/fonts/caladea/Caladea-Bold.ttf"),
    include_bytes!("../assets/fonts/caladea/Caladea-BoldItalic.ttf"),
    include_bytes!("../assets/fonts/caladea/Caladea-Italic.ttf"),
    include_bytes!("../assets/fonts/caladea/Caladea-Regular.ttf"),
    include_bytes!("../assets/fonts/cinzel/Cinzel[wght].ttf"),
    include_bytes!("../assets/fonts/comfortaa/Comfortaa[wght].ttf"),
    include_bytes!("../assets/fonts/eaglelake/EagleLake-Regular.ttf"),
    include_bytes!("../assets/fonts/gloriahallelujah/GloriaHallelujah.ttf"),
    include_bytes!("../assets/fonts/greatvibes/GreatVibes-Regular.ttf"),
    include_bytes!("../assets/fonts/iceberg/Iceberg-Regular.ttf"),
    include_bytes!("../assets/fonts/imfellenglish/IMFeENit28P.ttf"),
    include_bytes!("../assets/fonts/imfellenglish/IMFeENrm28P.ttf"),
    include_bytes!("../assets/fonts/imfellenglishsc/IMFeENsc28P.ttf"),
    include_bytes!("../assets/fonts/lugrasimo/Lugrasimo-Regular.ttf"),
    include_bytes!("../assets/fonts/orbitron/Orbitron[wght].ttf"),
    include_bytes!("../assets/fonts/pirataone/PirataOne-Regular.ttf"),
    include_bytes!("../assets/fonts/snowburstone/SnowburstOne-Regular.ttf"),
    include_bytes!("../assets/fonts/tapestry/Tapestry-Regular.ttf"),
    include_bytes!("../assets/fonts/underdog/Underdog-Regular.ttf"),
    include_bytes!("../assets/fonts/unifrakturmaguntia/UnifrakturMaguntia-Book.ttf"),
];

/// Load the fonts in the program, the system fonts, and the fonts in
/// `font_dirs`.
pub fn font_db(font_dirs: &[&Path]) -> FontDb {
    let mut db = resvg::usvg::fontdb::Database::new();
    for data in BUNDLED {
        db.load_font_data(data.to_vec());
    }
    db.load_system_fonts();
    for dir in font_dirs {
        db.load_fonts_dir(dir);
    }
    Arc::new(db)
}

/// A font database without fonts, for an SVG without text.
pub fn empty_font_db() -> FontDb {
    Arc::new(resvg::usvg::fontdb::Database::new())
}

/// The fallback font for a character that the font of a text does not
/// have. An emoji font comes first. The marker icons of FMG are emoji, and a
/// browser draws them with an emoji font. The default fallback can select a
/// font that draws a box for each character, for example "LastResort" on
/// macOS.
fn emoji_first_fallback() -> resvg::usvg::FallbackSelectionFn<'static> {
    use resvg::usvg::FontResolver;
    let default = FontResolver::default_fallback_selector();
    Box::new(move |c, exclude, db| {
        let emoji = db
            .faces()
            .filter(|face| !exclude.contains(&face.id))
            .filter(|face| face.families.iter().any(|(name, _)| name.contains("Emoji")))
            .filter(|face| {
                db.with_face_data(face.id, |data, index| {
                    ttf_parser::Face::parse(data, index).is_ok_and(|f| f.glyph_index(c).is_some())
                })
                .unwrap_or(false)
            })
            .map(|face| face.id)
            .min();
        emoji.or_else(|| default(c, exclude, db))
    })
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
            font_resolver: usvg::FontResolver {
                select_font: usvg::FontResolver::default_font_selector(),
                select_fallback: emoji_first_fallback(),
            },
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

/// Encode RGBA pixels (4 bytes for each pixel, row by row) as PNG bytes.
/// The pixels must be opaque or premultiplied by their alpha.
pub fn rgba_to_png(rgba: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let size = resvg::tiny_skia::IntSize::from_wh(width, height)
        .ok_or_else(|| "the image has no pixels".to_owned())?;
    resvg::tiny_skia::Pixmap::from_vec(rgba, size)
        .ok_or_else(|| "the pixels do not agree with the size".to_owned())?
        .encode_png()
        .map_err(|e| format!("cannot write the PNG: {e}"))
}

/// The font families that SVG text names in its `font-family`
/// attributes, sorted, each once.
pub fn families_in(svg: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = svg;
    while let Some(at) = rest.find("font-family=\"") {
        rest = &rest[at + 13..];
        let end = rest.find('"').unwrap_or(rest.len());
        for family in rest[..end].split(',') {
            let f = family.trim().trim_matches(|c| c == '\'' || c == '"');
            if !f.is_empty() {
                out.push(f.to_owned());
            }
        }
        rest = &rest[end..];
    }
    out.sort();
    out.dedup();
    out
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
    fn families_come_from_the_font_family_attributes() {
        let svg = r#"<g font-family="Almendra SC"><text font-family="Cinzel, 'Times New Roman'">a</text><text font-family="Almendra SC">b</text></g>"#;
        assert_eq!(
            families_in(svg),
            vec!["Almendra SC", "Cinzel", "Times New Roman"]
        );
    }

    #[test]
    fn the_fonts_in_the_program_have_the_families_of_the_presets() {
        // Only the fonts in the program, not the fonts of this system.
        let mut db = resvg::usvg::fontdb::Database::new();
        for data in BUNDLED {
            db.load_font_data(data.to_vec());
        }
        assert_eq!(db.len(), BUNDLED.len());
        let names: Vec<String> = db
            .faces()
            .flat_map(|f| f.families.iter().map(|(n, _)| n.to_ascii_lowercase()))
            .collect();
        for family in [
            "Almendra SC",
            "Arima Madurai",
            "Caladea",
            "Cinzel",
            "Comfortaa",
            "Eagle Lake",
            "Gloria Hallelujah",
            "Great Vibes",
            "IM Fell English",
            "IM Fell English SC",
            "Iceberg",
            "Lugrasimo",
            "Orbitron",
            "Pirata One",
            "Snowburst One",
            "Tapestry",
            "Underdog",
            "UnifrakturMaguntia",
        ] {
            assert!(names.contains(&family.to_ascii_lowercase()), "{family}");
        }
    }

    #[test]
    fn a_family_in_the_program_is_not_reported() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10mm" height="10mm" viewBox="0 0 10 10"><text font-family="Lugrasimo" y="5">a</text></svg>"##;
        let p = svg_to_png(svg, 64, &[], &["Lugrasimo", "Almendra SC"]).unwrap();
        assert!(p.missing_fonts.is_empty(), "{:?}", p.missing_fonts);
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
        let png = rgba_to_png(px, 30, 30).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 30);
        assert!(rgba_to_png(vec![0; 8], 30, 30).is_err());
    }

    #[test]
    fn a_marker_emoji_draws_with_an_emoji_font() {
        let db = font_db(&[]);
        let volcano = '\u{1F30B}';
        let has_emoji_font = db.faces().any(|face| {
            face.families.iter().any(|(name, _)| name.contains("Emoji"))
                && db
                    .with_face_data(face.id, |data, index| {
                        // resvg draws the color glyphs of sbix and COLR
                        // fonts, but not of CBDT fonts.
                        ttf_parser::Face::parse(data, index).is_ok_and(|f| {
                            f.glyph_index(volcano).is_some()
                                && (f.tables().sbix.is_some() || f.tables().colr.is_some())
                        })
                    })
                    .unwrap_or(false)
        });
        if !has_emoji_font {
            eprintln!(
                "This system has no sbix or COLR emoji font with the volcano. The test checks nothing."
            );
            return;
        }
        let draw = |text: char| {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><text x="20" y="20" font-size="24" text-anchor="middle" dominant-baseline="central">{text}</text></svg>"##
            );
            let scene = Scene::parse(&svg, &db, &[]).unwrap();
            scene
                .render_rgba(40, 40, 1.0, [0.0, 0.0], [255, 255, 255])
                .unwrap()
        };
        // A private use character has no glyph: it draws as a box or not
        // at all.
        let missing = draw('\u{E000}');
        let emoji = draw(volcano);
        assert_ne!(emoji, missing);
        // A color emoji has pixels that are not grey.
        assert!(
            emoji
                .chunks(4)
                .any(|p| p[0].abs_diff(p[1]) > 40 || p[1].abs_diff(p[2]) > 40),
            "the volcano has no color"
        );
    }
}
