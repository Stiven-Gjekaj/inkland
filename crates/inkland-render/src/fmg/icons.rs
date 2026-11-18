//! The icons of `colour` mode: relief icons, place icons, port anchors, and
//! goods icons, and glyphs.
//!
//! Each icon file becomes an SVG `<symbol>`, as `icon-sets.ts` of the
//! original program makes it. A symbol id is the set and the file name in
//! lower case words, for example `relief-simple-mount-snow-1`.

use super::icons_data::FILES;

/// The text of an icon file, by its path without `.svg`.
pub fn file(path: &str) -> Option<&'static str> {
    FILES
        .binary_search_by(|(p, _)| p.cmp(&path))
        .ok()
        .map(|i| FILES[i].1)
}

/// A word in lower case with `-` between its parts: `mountSnow` gives
/// `mount-snow`.
pub fn kebab(word: &str) -> String {
    let mut out = String::new();
    for c in word.chars() {
        if c.is_ascii_uppercase() {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else if c == '_' || c == '/' {
            out.push('-');
        } else {
            out.push(c);
        }
    }
    out
}

/// The `<symbol>` of an icon file. A place icon keeps the art that is
/// outside its frame.
pub fn symbol(path: &str, id: &str) -> Option<String> {
    let text = file(path)?.trim();
    let open_end = text.find('>')?;
    let head = &text[4..open_end];
    let body = text[open_end + 1..].strip_suffix("</svg>")?;
    let mut attrs = String::new();
    // Keep the attributes of the root, except xmlns and id.
    let mut rest = head.trim();
    while let Some(eq) = rest.find('=') {
        let name = rest[..eq].trim();
        let after = rest[eq + 1..].trim_start();
        let quote = after.chars().next()?;
        let end = after[1..].find(quote)? + 1;
        let value = &after[1..end];
        if !(name.starts_with("xmlns") || name == "id") {
            attrs.push_str(&format!(" {name}=\"{value}\""));
        }
        rest = after[end + 1..].trim_start();
    }
    let overflow = if path.starts_with("burgs/") || path.starts_with("ports/") {
        " overflow=\"visible\""
    } else {
        ""
    };
    Some(format!(
        "<symbol id=\"{id}\"{overflow}{attrs}>{}</symbol>",
        body.trim()
    ))
}

/// The frame `[x, y, width, height]` of a place icon, in icon em: its
/// view box over 10.
pub fn anchored_box(path: &str) -> [f64; 4] {
    let Some(text) = file(path) else {
        return [-0.5, -0.5, 1.0, 1.0];
    };
    let Some(at) = text.find("viewBox=\"") else {
        return [-0.5, -0.5, 1.0, 1.0];
    };
    let rest = &text[at + 9..];
    let v: Vec<f64> = rest[..rest.find('"').unwrap_or(0)]
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    if v.len() == 4 && v[2] > 0.0 && v[3] > 0.0 {
        [v[0] / 10.0, v[1] / 10.0, v[2] / 10.0, v[3] / 10.0]
    } else {
        [-0.5, -0.5, 1.0, 1.0]
    }
}

/// The path of a place icon id of a style, for example
/// `burgs-atlas-circle` gives `burgs/atlas/circle`.
pub fn place_icon_path(id: &str) -> Option<String> {
    for set in [
        "burgs-atlas-",
        "burgs-watabou-",
        "burgs-illustrated-",
        "ports-",
    ] {
        if let Some(name) = id.strip_prefix(set) {
            let dir = set.trim_end_matches('-').replace('-', "/");
            let path = format!("{dir}/{name}");
            return file(&path).is_some().then_some(path);
        }
    }
    None
}

/// The path of a goods icon of the economy, for example `goods-amber`
/// gives `goods/amber`. An unknown icon gives the `unknown` icon of FMG.
pub fn goods_icon_path(icon: &str) -> String {
    let path = format!("goods/{}", icon.strip_prefix("goods-").unwrap_or(icon));
    if file(&path).is_some() {
        path
    } else {
        "goods/unknown".to_owned()
    }
}

/// The prefix of a glyph id.
const GLYPH_PREFIX: &str = "glyph-";

/// The id of a glyph: its code points in hex, `XIV` gives `glyph-58-49-56`
/// (`Icons.glyph`). An empty text gives an empty id.
pub fn glyph_id(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let hex: Vec<String> = text
        .chars()
        .map(|c| format!("{:x}", u32::from(c)))
        .collect();
    format!("{GLYPH_PREFIX}{}", hex.join("-"))
}

/// The text of a glyph id, or `None` for another id (`Icons.glyphText`).
pub fn glyph_text(id: &str) -> Option<String> {
    let rest = id.strip_prefix(GLYPH_PREFIX)?;
    rest.split('-')
        .map(|h| u32::from_str_radix(h, 16).ok().and_then(char::from_u32))
        .collect()
}

/// The `<symbol>` of a glyph: its text in a 100 x 100 frame
/// (`Icons.ensureGlyph`). The text takes the fill of the place that uses
/// it.
pub fn glyph_symbol(id: &str) -> Option<String> {
    let text = glyph_text(id)?;
    Some(format!(
        "<symbol id=\"{id}\" viewBox=\"0 0 100 100\" overflow=\"visible\"><text x=\"50\" y=\"50\" font-size=\"100\" text-anchor=\"middle\" dominant-baseline=\"central\" stroke=\"none\">{}</text></symbol>",
        crate::extras::xml(&text)
    ))
}

/// The relief icon types (`TYPES` in `relief-generator.ts`): the name, the
/// number of variants, and the fallback type.
pub const RELIEF_TYPES: [(&str, u32, Option<&str>); 14] = [
    ("mount", 6, None),
    ("mountSnow", 6, Some("mount")),
    ("vulcan", 3, Some("mount")),
    ("hill", 4, None),
    ("dune", 1, None),
    ("deciduous", 2, None),
    ("conifer", 1, None),
    ("coniferSnow", 1, Some("conifer")),
    ("acacia", 1, None),
    ("palm", 1, None),
    ("grass", 1, None),
    ("swamp", 2, None),
    ("cactus", 3, Some("dune")),
    ("deadTree", 2, Some("dune")),
];

/// The relief sets.
pub const RELIEF_SETS: [&str; 5] = ["simple", "colored", "gray", "illustrated", "stickers"];

/// The number of variants of a relief type.
pub fn variants(kind: &str) -> u32 {
    RELIEF_TYPES.iter().find(|t| t.0 == kind).map_or(1, |t| t.1)
}

/// The file of a relief icon slot in a set: the file of the type and the
/// variant if the set has it, else another variant, else the fallback type
/// (`resolveSlot`).
pub fn relief_file(set: &str, kind: &str, variant: u32) -> Option<String> {
    let set = if RELIEF_SETS.contains(&set) {
        set
    } else {
        "simple"
    };
    let mut name = kind;
    let mut seen: Vec<&str> = Vec::new();
    loop {
        if seen.contains(&name) {
            return None;
        }
        seen.push(name);
        let prefix = format!("relief/{set}/{name}-");
        let mut found: Vec<(u32, &str)> = FILES
            .iter()
            .filter_map(|(p, _)| {
                p.strip_prefix(&prefix)
                    .and_then(|v| v.parse::<u32>().ok())
                    .map(|v| (v, *p))
            })
            .collect();
        found.sort();
        if !found.is_empty() {
            let k = (variant.max(1) - 1) as usize % found.len();
            return Some(found[k].1.to_owned());
        }
        name = RELIEF_TYPES.iter().find(|t| t.0 == name)?.2?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_list_is_sorted_and_has_each_set() {
        assert!(FILES.windows(2).all(|w| w[0].0 < w[1].0));
        for set in RELIEF_SETS {
            assert!(relief_file(set, "mount", 1).is_some(), "{set}");
            for (kind, n, _) in RELIEF_TYPES {
                for v in 1..=n {
                    assert!(relief_file(set, kind, v).is_some(), "{set} {kind} {v}");
                }
            }
        }
    }

    #[test]
    fn a_symbol_keeps_the_view_box_and_parses() {
        let s = symbol("relief/simple/mount-1", "relief-simple-mount-1").unwrap();
        assert!(s.starts_with("<symbol id=\"relief-simple-mount-1\""));
        assert!(s.contains("viewBox=\"0 0 130 130\""));
        assert!(!s.contains("xmlns"));
        let doc = format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{s}</svg>");
        assert!(roxmltree::Document::parse(&doc).is_ok());
        let b = symbol("burgs/atlas/circle", "burgs-atlas-circle").unwrap();
        assert!(b.contains("overflow=\"visible\""));
        assert_eq!(anchored_box("burgs/atlas/circle"), [-0.6, -0.6, 1.2, 1.2]);
    }

    #[test]
    fn each_good_has_its_icon() {
        for good in inkland_core::economy::goods::catalogue() {
            let path = goods_icon_path(&good.icon);
            assert_ne!(path, "goods/unknown", "{}", good.name);
            let s = symbol(&path, &kebab(&path)).unwrap();
            assert!(s.contains("viewBox=\"0 0 100 100\""), "{}", good.name);
        }
        assert_eq!(goods_icon_path("goods-nothing"), "goods/unknown");
        assert_eq!(
            kebab(&goods_icon_path("goods-tropicalTimber")),
            "goods-tropical-timber"
        );
    }

    #[test]
    fn a_glyph_id_holds_its_text() {
        assert_eq!(glyph_id("XIV"), "glyph-58-49-56");
        assert_eq!(glyph_id("\u{2696}\u{fe0f}"), "glyph-2696-fe0f");
        assert_eq!(glyph_id(""), "");
        assert_eq!(
            glyph_text("glyph-2696-fe0f").as_deref(),
            Some("\u{2696}\u{fe0f}")
        );
        assert_eq!(glyph_text("glyph-zz"), None);
        assert_eq!(glyph_text("goods-amber"), None);
        let s = glyph_symbol("glyph-3c-26").unwrap();
        assert!(s.starts_with("<symbol id=\"glyph-3c-26\" viewBox=\"0 0 100 100\""));
        assert!(s.contains(">&lt;&amp;</text>"));
        let doc = format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{s}</svg>");
        assert!(roxmltree::Document::parse(&doc).is_ok());
    }

    #[test]
    fn ids_are_lower_case_words() {
        assert_eq!(kebab("mountSnow"), "mount-snow");
        assert_eq!(kebab("trading_post"), "trading-post");
        assert_eq!(kebab("dropShadow05"), "drop-shadow05");
        assert_eq!(
            place_icon_path("burgs-watabou-capital").as_deref(),
            Some("burgs/watabou/capital")
        );
        assert_eq!(
            place_icon_path("ports-anchor").as_deref(),
            Some("ports/anchor")
        );
        assert_eq!(place_icon_path("burgs-none"), None);
    }
}
