//! The hatch patterns of FMG (`#defs-hatching` in `index.html`,
//! `renderers/hatches.ts`). A zone, a market, or a legend item can have a
//! hatch fill, for example `url(#hatch3)`.
//!
//! A derived fill gives a base pattern in another colour and tile size:
//! `url(#hatch3-c0392b-s150)` is `hatch3` in `#c0392b` at 150 percent.

use super::Ctx;

/// The tile size of a base pattern, in percent.
const DEFAULT_SIZE: f64 = 100.0;

/// The base color of a pattern.
const BLACK: &str = "#000000";

/// A hatch fill, split into its parts (`Hatches.parse`).
#[derive(Debug, Clone, PartialEq)]
pub struct Hatch {
    /// The index of the base pattern: 3 for `hatch3`.
    pub base: usize,
    /// The color as `#rrggbb`.
    pub color: String,
    /// The tile size, in percent.
    pub size: f64,
    /// The id of the pattern: the text in `url(#...)`.
    pub id: String,
}

/// Split a hatch fill into its parts. Any other fill gives `None`.
pub fn parse(fill: &str) -> Option<Hatch> {
    let id = fill.strip_prefix("url(#")?.strip_suffix(')')?;
    let mut parts = id.split('-');
    let base: usize = parts.next()?.strip_prefix("hatch")?.parse().ok()?;
    if base >= PATTERNS.len() {
        return None;
    }
    let mut color = BLACK.to_owned();
    let mut size = DEFAULT_SIZE;
    for part in parts {
        if let Some(s) = part.strip_prefix('s') {
            size = s.parse().ok().filter(|v: &f64| *v > 0.0)?;
        } else if part.len() == 6 && part.chars().all(|c| c.is_ascii_hexdigit()) {
            color = format!("#{}", part.to_ascii_lowercase());
        } else {
            return None;
        }
    }
    Some(Hatch {
        base,
        color,
        size,
        id: id.to_owned(),
    })
}

/// The `<pattern>` of a hatch: the base pattern in the color of the
/// hatch, its tile scaled to the size (`Hatches.recolor`).
pub fn pattern(h: &Hatch) -> String {
    let (attrs, body) = PATTERNS[h.base];
    let mut attrs = attrs.to_owned();
    if h.size != DEFAULT_SIZE {
        let scale = format!("scale({})", super::geom::n(h.size / DEFAULT_SIZE));
        attrs = match attrs.find("patternTransform=\"") {
            Some(at) => {
                let end = at + 18 + attrs[at + 18..].find('"').unwrap_or(0);
                format!("{} {scale}{}", &attrs[..end], &attrs[end..])
            }
            None => format!("patternTransform=\"{scale}\" {attrs}"),
        };
    }
    let body = body.replace(BLACK, &h.color);
    format!("<pattern id=\"{}\" {attrs}>{body}</pattern>", h.id)
}

/// Add the pattern of a fill to the defs if the fill is a hatch.
pub fn ensure(ctx: &Ctx, fill: &str) {
    if let Some(h) = parse(fill) {
        ctx.def(&h.id, pattern(&h));
    }
}

/// The base patterns: the attributes of each `<pattern>` without its id,
/// and its content. The color of each pattern is black.
pub const PATTERNS: [(&str, &str); 47] = [
    // hatch0
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch1
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch2
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch3
    (
        r##"patternTransform="rotate(-45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch4
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="1.5"/><line x1="0" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch5
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="1.5"/><line x1="0" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch6
    (
        r##"patternUnits="userSpaceOnUse" width="5" height="5""##,
        r##"<circle cx="2.5" cy="2.5" r="1" fill="#000000"/>"##,
    ),
    // hatch7
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="3" transform="rotate(-45 0 0)" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch8
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="3" stroke="#000000" stroke-width="2.5"/>"##,
    ),
    // hatch9
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="3" stroke="#000000" stroke-width="2.5"/>"##,
    ),
    // hatch10
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="3" y2="0" stroke="#000000" stroke-width="2.5"/>"##,
    ),
    // hatch11
    (
        r##"patternTransform="rotate(-45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="3" stroke="#000000" stroke-width="2.5"/>"##,
    ),
    // hatch12
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="1" x2="0" y2="3" stroke="#000000" stroke-width="1.5"/><line x1="1" y1="0" x2="3" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch13
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="3" stroke="#000000" stroke-width="1.5"/><line x1="0" y1="0" x2="3" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch14
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="1" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch15
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="1" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch16
    (
        r##"patternTransform="rotate(-45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="1" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch17
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="1" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="1.5"/><line x1="1" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch18
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="1" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="1.5"/><line x1="1" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch19
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="2" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch20
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="2" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch21
    (
        r##"patternTransform="rotate(-45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="2" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch22
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="2" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="1.5"/><line x1="2" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch23
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="3" stroke="#000000" stroke-width="1.5"/><line x1="0" y1="0" x2="3" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch24
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="2" y1="0" x2="0" y2="4" stroke="#000000" stroke-width="1.5"/><line x1="2" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch25
    (
        r##"patternTransform="rotate(-45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="2" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch26
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="2" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch27
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="2" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch28
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="2" y1="0" x2="0" y2="2" stroke="#000000" stroke-width="2"/>"##,
    ),
    // hatch29
    (
        r##"patternTransform="rotate(30 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="1" x2="0" y2="3" stroke="#000000" stroke-width="1.5"/><line x1="1" y1="0" x2="3" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch30
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="1" y1="0" x2="0" y2="3" stroke="#000000" stroke-width="1.5"/><line x1="1" y1="0" x2="3" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch31
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="1" y1="0" x2="0" y2="3" stroke="#000000" stroke-width="1.5"/><line x1="1" y1="0" x2="3" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch32
    (
        r##"patternUnits="userSpaceOnUse" width="5" height="5""##,
        r##"<circle cx="2.5" cy="2.5" r="0.5" fill="#000000"/>"##,
    ),
    // hatch33
    (
        r##"patternUnits="userSpaceOnUse" width="5" height="5""##,
        r##"<circle cx="2.5" cy="2.5" r="1.5" fill="#000000"/>"##,
    ),
    // hatch34
    (
        r##"patternUnits="userSpaceOnUse" width="5" height="5""##,
        r##"<circle cx="3" cy="3" r="1" fill="#000000"/><circle cx="1" cy="1" r="1" fill="#000000"/>"##,
    ),
    // hatch35
    (
        r##"patternUnits="userSpaceOnUse" width="5" height="5""##,
        r##"<circle cx="3" cy="3" r="1.5" fill="#000000"/><circle cx="1" cy="1" r="1.5" fill="#000000"/>"##,
    ),
    // hatch36
    (
        r##"patternTransform="rotate(-45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="3" transform="rotate(-45 0 0)" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch37
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="0" y2="3" transform="rotate(-45 0 0)" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch38
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="4" y2="4" stroke="#000000" stroke-width="1.5"/><line x1="0" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch39
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="4" y2="4" stroke="#000000" stroke-width="1.5"/><line x1="0" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch40
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="3" y2="3" stroke="#000000" stroke-width="1.5"/><line x1="0" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch41
    (
        r##"patternTransform="rotate(45 0 0)" patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<line x1="0" y1="0" x2="3" y2="3" stroke="#000000" stroke-width="1.5"/><line x1="0" y1="0" x2="4" y2="0" stroke="#000000" stroke-width="1.5"/>"##,
    ),
    // hatch42
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<path d="M0 2Q1 .5 2 2T4 2" fill="none" stroke="#000000" stroke-width="1"/>"##,
    ),
    // hatch43
    (
        r##"patternUnits="userSpaceOnUse" width="4" height="4""##,
        r##"<path d="M0 2L1 1 3 3 4 2" fill="none" stroke="#000000" stroke-width="1"/>"##,
    ),
    // hatch44
    (
        r##"patternUnits="userSpaceOnUse" width="5" height="5""##,
        r##"<path d="M2.5 1V4M1 2.5H4" stroke="#000000" stroke-width="1"/>"##,
    ),
    // hatch45
    (
        r##"patternUnits="userSpaceOnUse" width="8" height="8""##,
        r##"<path d="M0 1H8M0 5H8M1 1V5M5 5V8M5 0V1" stroke="#000000" stroke-width="1"/>"##,
    ),
    // hatch46
    (
        r##"patternUnits="userSpaceOnUse" width="8" height="8""##,
        r##"<path d="M1 1.5h.01M4.5 1h.01M7 3h.01M2.5 4.5h.01M5.5 5.5h.01M1 7h.01M7.2 6.8h.01" stroke="#000000" stroke-width="1.2" stroke-linecap="round"/>"##,
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_fill_gives_the_black_pattern() {
        let h = parse("url(#hatch3)").unwrap();
        assert_eq!((h.base, h.color.as_str(), h.size), (3, BLACK, 100.0));
        let p = pattern(&h);
        assert!(p.starts_with("<pattern id=\"hatch3\" patternTransform=\"rotate(-45 0 0)\""));
        assert!(p.contains("stroke=\"#000000\""));
        let doc = format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{p}</svg>");
        assert!(roxmltree::Document::parse(&doc).is_ok());
    }

    #[test]
    fn a_derived_fill_recolors_and_scales_the_base() {
        let h = parse("url(#hatch1-C0392B-s150)").unwrap();
        assert_eq!((h.base, h.color.as_str(), h.size), (1, "#c0392b", 150.0));
        let p = pattern(&h);
        assert!(p.contains("patternTransform=\"rotate(45 0 0) scale(1.5)\""));
        assert!(p.contains("stroke=\"#c0392b\"") && !p.contains(BLACK));
        let h = parse("url(#hatch6-s50)").unwrap();
        assert!(pattern(&h).contains("patternTransform=\"scale(0.5)\""));
    }

    #[test]
    fn other_fills_are_not_hatches() {
        for fill in [
            "#ff0000",
            "url(#land)",
            "url(#hatch47)",
            "url(#hatch1-xyz)",
            "hatch1",
        ] {
            assert_eq!(parse(fill), None, "{fill}");
        }
    }

    #[test]
    fn each_pattern_parses() {
        for i in 0..PATTERNS.len() {
            let p = pattern(&parse(&format!("url(#hatch{i})")).unwrap());
            let doc = format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{p}</svg>");
            assert!(roxmltree::Document::parse(&doc).is_ok(), "{i}");
        }
    }
}
