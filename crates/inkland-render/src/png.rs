//! The PNG preview: a small raster of the SVG, so that an agent can look at
//! the map without a browser.

/// Render SVG text to PNG bytes. The longer side of the image is
/// `longest_px` pixels.
pub fn svg_to_png(svg: &str, longest_px: u32) -> Result<Vec<u8>, String> {
    use resvg::{tiny_skia, usvg};
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &opt).map_err(|e| format!("cannot read the SVG: {e}"))?;
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
    pixmap
        .encode_png()
        .map_err(|e| format!("cannot write the PNG: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer_side_has_the_wanted_size() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="167mm" height="250mm" viewBox="0 0 167 250"><rect width="167" height="250" fill="#ffffff"/><path d="M10 10L150 200" stroke="#000000"/></svg>"##;
        let png = svg_to_png(svg, 1024).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        // Width and height are big-endian at bytes 16..24 of the IHDR chunk.
        let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!(h, 1024);
        assert_eq!(w, (167.0f32 * 1024.0 / 250.0).round() as u32);
    }
}
