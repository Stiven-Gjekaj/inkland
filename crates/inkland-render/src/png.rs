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

/// Render SVG text to PNG bytes. The longer side of the image is
/// `longest_px` pixels. `families` are the font families to look for.
pub fn svg_to_png(
    svg: &str,
    longest_px: u32,
    font_dirs: &[&Path],
    families: &[&str],
) -> Result<Preview, String> {
    use resvg::{tiny_skia, usvg};
    let mut db = usvg::fontdb::Database::new();
    db.load_system_fonts();
    for dir in font_dirs {
        db.load_fonts_dir(dir);
    }
    // A font file can name its family with other upper and lower case
    // letters than the SVG, for example "IM FELL English". A book stylesheet
    // maps the name with @font-face; the preview maps it here. The SVG file
    // keeps the exact name.
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
        fontdb: Arc::new(db),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&svg, &opt).map_err(|e| format!("cannot read the SVG: {e}"))?;
    let size = tree.size();
    let scale = longest_px as f32 / size.width().max(size.height());
    let w = (size.width() * scale).round().max(1.0) as u32;
    let h = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap =
        tiny_skia::Pixmap::new(w, h).ok_or_else(|| "cannot make the image".to_owned())?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let png = pixmap
        .encode_png()
        .map_err(|e| format!("cannot write the PNG: {e}"))?;
    Ok(Preview {
        png,
        missing_fonts: missing,
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
}
