//! The FMG icon font of the app has the family and the glyphs that
//! `ui/theme.slint` names.

#[test]
fn the_icon_font_has_each_glyph_that_the_app_uses() {
    let data = include_bytes!("../ui/fonts/fmg-icons.ttf");
    let face = ttf_parser::Face::parse(data, 0).expect("the icon font is a TTF file");
    let family = face
        .names()
        .into_iter()
        .filter(|n| n.name_id == ttf_parser::name_id::FAMILY)
        .find_map(|n| n.to_string());
    assert_eq!(family.as_deref(), Some("fontello"));
    let theme = include_str!("../ui/theme.slint");
    let icons = theme
        .split("export global Icons {")
        .nth(1)
        .and_then(|s| s.split("public pure function").next())
        .expect("theme.slint has the Icons global");
    let codes: Vec<u32> = icons
        .split("\\u{")
        .skip(1)
        .map(|s| u32::from_str_radix(s.split('}').next().unwrap(), 16).unwrap())
        .collect();
    assert!(codes.len() >= 7, "{codes:?}");
    for code in codes {
        let c = char::from_u32(code).unwrap();
        assert!(
            face.glyph_index(c).is_some(),
            "the font has no glyph {code:x}"
        );
    }
}
