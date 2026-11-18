//! The raster images of FMG that the style presets name: the paper
//! textures and the ocean patterns, from `public/images`. An SVG holds each
//! image as a data URI, so that the file has all that it needs.

/// The images in the program, by the `href` of the style presets.
const IMAGES: &[(&str, &[u8])] = &[
    (
        "./images/textures/antique-small.jpg",
        include_bytes!("../../assets/fmg/images/textures/antique-small.jpg"),
    ),
    (
        "./images/textures/gray-paper.jpg",
        include_bytes!("../../assets/fmg/images/textures/gray-paper.jpg"),
    ),
    (
        "./images/textures/iran-small.jpg",
        include_bytes!("../../assets/fmg/images/textures/iran-small.jpg"),
    ),
    (
        "./images/textures/marble-big.jpg",
        include_bytes!("../../assets/fmg/images/textures/marble-big.jpg"),
    ),
    (
        "./images/textures/marble-blue-small.jpg",
        include_bytes!("../../assets/fmg/images/textures/marble-blue-small.jpg"),
    ),
    (
        "./images/textures/mercury-small.jpg",
        include_bytes!("../../assets/fmg/images/textures/mercury-small.jpg"),
    ),
    (
        "./images/textures/plaster.jpg",
        include_bytes!("../../assets/fmg/images/textures/plaster.jpg"),
    ),
    (
        "./images/kiwiroo.png",
        include_bytes!("../../assets/fmg/images/kiwiroo.png"),
    ),
    (
        "./images/pattern1.png",
        include_bytes!("../../assets/fmg/images/pattern1.png"),
    ),
    (
        "./images/pattern2.png",
        include_bytes!("../../assets/fmg/images/pattern2.png"),
    ),
    (
        "./images/pattern3.png",
        include_bytes!("../../assets/fmg/images/pattern3.png"),
    ),
    (
        "./images/waves.png",
        include_bytes!("../../assets/fmg/images/waves.png"),
    ),
];

/// The `href` to write for an image of a style: a data URI for an image in
/// the program, or the text as it is for another image. `None` for an
/// empty `href`.
pub fn href(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let Some((_, data)) = IMAGES.iter().find(|(n, _)| *n == name) else {
        return Some(name.to_owned());
    };
    let mime = if name.ends_with(".png") {
        "image/png"
    } else {
        "image/jpeg"
    };
    Some(format!("data:{mime};base64,{}", base64(data)))
}

/// Standard base64 with padding.
fn base64(data: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let v = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[((v >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn an_image_of_a_preset_is_a_data_uri() {
        let png = href("./images/pattern1.png").unwrap();
        assert!(png.starts_with("data:image/png;base64,iVBORw0KGgo"));
        let jpg = href(" ./images/textures/plaster.jpg ").unwrap();
        assert!(jpg.starts_with("data:image/jpeg;base64,/9j/"));
        assert_eq!(
            href("https://example.com/a.png").as_deref(),
            Some("https://example.com/a.png")
        );
        assert_eq!(href(""), None);
    }

    #[test]
    fn each_image_that_a_preset_names_is_in_the_program() {
        for style in inkland_core::config::StyleName::ALL {
            let tree = crate::fmg::style::Style::preset(style);
            for path in [&["texture"][..], &["ocean", "pattern"][..]] {
                let name = tree
                    .option_str(path, "href")
                    .or_else(|| tree.attr(path, "href"))
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                assert!(
                    IMAGES.iter().any(|(n, _)| *n == name),
                    "{style:?} {path:?} {name}"
                );
            }
        }
    }
}
