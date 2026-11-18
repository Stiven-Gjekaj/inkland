use super::*;
use inkland_core::emblems::{Generator, Parent};
use inkland_core::rand::Rand;
use inkland_core::seed::{Layer, SeedTree};

fn coa(json: &str) -> Coa {
    serde_json::from_str(json).unwrap()
}

/// Many generated coats of arms, with each shield.
fn generated(n: usize) -> Vec<Coa> {
    let mut r = Rand::new(SeedTree::new(9).part_rng(Layer::States, "emblems"));
    let mut g = Generator::new(&mut r);
    let none = Parent {
        coa: None,
        kinship: 0.0,
        dominion: 0.0,
    };
    let shields: Vec<&str> = data::SHIELDS.iter().map(|s| s.name).collect();
    (0..n)
        .map(|i| {
            let mut c = g.generate(&none, Some("Generic"));
            c.shield = Some(shields[i % shields.len()].to_owned());
            c
        })
        .collect()
}

fn render_png(svg: &str) -> Vec<u8> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .unwrap_or_else(|e| panic!("{e}: {svg}"));
    let mut pix = resvg::tiny_skia::Pixmap::new(100, 100).unwrap();
    let size = tree.size();
    let t = resvg::tiny_skia::Transform::from_scale(100.0 / size.width(), 100.0 / size.height());
    resvg::render(&tree, t, &mut pix.as_mut());
    pix.data().to_vec()
}

#[test]
fn the_art_list_agrees_with_the_generator() {
    let names: Vec<&str> = charge_names().collect();
    assert_eq!(names, inkland_core::emblems::data::ART);
    for (name, text) in charges::FILES {
        assert!(text.contains(&format!("<g id=\"{name}\"")), "{name}");
        // Only licenses that permit use in inkland, also for sale.
        let ok = [
            "publicdomain/zero/1.0",
            "licenses/by/",
            "licenses/by-sa/",
            "artlibre.org",
            "gnu.org/licenses/fdl-1.3",
        ];
        assert!(ok.iter().any(|l| text.contains(l)), "{name}");
        assert!(!text.contains("by-nc"), "{name}");
    }
    // Each shield of the generator has a shape.
    for s in inkland_core::emblems::shield_names() {
        assert!(shape(s).is_some(), "{s}");
    }
}

#[test]
fn a_simple_coat_of_arms_has_the_parts_of_fmg() {
    let c = coa(r#"{"t1":"azure","division":{"division":"perPale","t":"or"},
        "ordinaries":[{"ordinary":"bend","t":"gules","line":"wavy"}],
        "charges":[{"charge":"mullet","t":"argent","p":"jln","size":0.7}],"shield":"spanish"}"#);
    let d = draw(&c, "coa-test", &Look::COLOUR);
    assert_eq!(d.view_box(), "0 10 200 200");
    let b = &d.body;
    assert!(b.contains("fill=\"#377cd7\""), "azure");
    assert!(b.contains("<clipPath id=\"coa-test-division\">"));
    assert!(b.contains("clip-path=\"url(#coa-test-shield)\""));
    assert!(b.contains("radialGradient id=\"coa-test-backlight\""));
    assert_eq!(b.matches("<use href=\"#coa-test-c0\"").count(), 3);
    // The positions j, l, n of the spanish shield, at size 0.7.
    assert!(
        b.contains("transform=\"translate(-7.5 -7.5) scale(0.7)\""),
        "{b}"
    );
}

#[test]
fn ids_are_lower_case_words_and_unique() {
    for (i, c) in generated(300).iter().enumerate() {
        let prefix = format!("coa-t{i}");
        let svg = document(c, &prefix, &Look::COLOUR, 200.0);
        let doc = roxmltree::Document::parse(&svg).unwrap_or_else(|e| panic!("{e}: {svg}"));
        let mut seen = std::collections::BTreeSet::new();
        for n in doc.descendants().filter(|n| n.is_element()) {
            if let Some(id) = n.attribute("id") {
                assert!(id.starts_with(&prefix), "{id}");
                assert!(
                    id.split('-').all(|w| !w.is_empty()
                        && w.bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())),
                    "{id}"
                );
                assert!(seen.insert(id.to_owned()), "{id} twice");
            }
            for key in ["href", "clip-path", "fill", "stroke"] {
                if let Some(v) = n.attribute(key)
                    && let Some(r) = v
                        .strip_prefix("url(#")
                        .or_else(|| v.strip_prefix('#'))
                        .filter(|_| v.contains('#'))
                {
                    let r = r.trim_end_matches(')');
                    if !v.starts_with('#') || key == "href" {
                        assert!(svg.contains(&format!("id=\"{r}\"")), "{r} is not defined");
                    }
                }
            }
        }
    }
}

#[test]
fn each_generated_coat_of_arms_renders() {
    for (i, c) in generated(90).iter().enumerate() {
        let svg = document(c, &format!("coa-r{i}"), &Look::COLOUR, 200.0);
        let px = render_png(&svg);
        // The shield is not empty.
        assert!(px.chunks(4).filter(|p| p[3] > 0).count() > 1000, "{c:?}");
    }
}

#[test]
fn the_hatching_look_passes_the_print_checks() {
    let opts = crate::checks::CheckOptions {
        min_line_width_pt: 0.5,
        fonts_allowed: Vec::new(),
    };
    for (i, c) in generated(135).iter().enumerate() {
        // The smallest line that the check permits, and not more. The
        // check applies the transforms, so it tests each line as printed.
        let look = print_look(c, 200.0, 0.5);
        let svg = document(c, &format!("coa-h{i}"), &look, 200.0);
        for r in crate::checks::check_svg(&svg, &opts) {
            assert!(r.pass, "{} {:?} {c:?}", r.name, r.details);
        }
        render_png(&svg);
    }
}

#[test]
fn hatching_follows_petra_sancta() {
    let look = Look {
        hatching: true,
        min_line: 2.0,
    };
    let d = draw(
        &coa(r#"{"t1":"gules","charges":[{"charge":"roundel","t":"or","p":"e"}]}"#),
        "coa-p",
        &look,
    );
    assert!(
        d.body.contains("<pattern id=\"coa-p-h0-gules\""),
        "{}",
        d.body
    );
    assert!(d.body.contains("-or\" width="));
    // Gules: upright lines.
    assert!(d.body.contains("<path d=\"M4 0 V8\""), "{}", d.body);
    // Argent is white and sable is black, with no pattern.
    let d = draw(
        &coa(r#"{"t1":"argent","ordinaries":[{"ordinary":"fess","t":"sable"}]}"#),
        "coa-q",
        &look,
    );
    assert!(!d.body.contains("<pattern"));
    assert!(d.body.contains("fill=\"#000\""));
}

#[test]
fn an_unknown_shield_draws_the_heater() {
    let d = draw(
        &coa(r#"{"t1":"or","shield":"nothing"}"#),
        "coa-x",
        &Look::COLOUR,
    );
    let heater = shape("heater").unwrap();
    assert!(d.body.contains(heater.path));
}

#[test]
fn the_same_arms_give_the_same_bytes() {
    let c = &generated(5)[3];
    assert_eq!(
        document(c, "coa-a", &Look::COLOUR, 100.0),
        document(c, "coa-a", &Look::COLOUR, 100.0)
    );
}

#[test]
fn line_widths_round_up() {
    assert_eq!(up(1.411), "1.42");
    assert_eq!(up(1.42), "1.42");
    assert_eq!(up(2.0), "2");
}

#[test]
fn each_charge_renders_in_colour_and_with_hatching() {
    let opts = crate::checks::CheckOptions {
        min_line_width_pt: 0.5,
        fonts_allowed: Vec::new(),
    };
    for (i, name) in charge_names().enumerate() {
        let c = coa(&format!(
            r#"{{"t1":"azure","charges":[{{"charge":"{name}","t":"or","t2":"gules","t3":"argent","p":"e","size":1.5}}],"shield":"heater"}}"#
        ));
        let svg = document(&c, &format!("coa-c{i}"), &Look::COLOUR, 200.0);
        let before = render_png(&document(
            &coa(r#"{"t1":"azure","shield":"heater"}"#),
            &format!("coa-c{i}"),
            &Look::COLOUR,
            200.0,
        ));
        // The charge changes the picture.
        assert_ne!(render_png(&svg), before, "{name}");
        // The smallest line that the check permits, and not more.
        let hatched = print_look(&c, 200.0, 0.5);
        let svg = document(&c, &format!("coa-ch{i}"), &hatched, 200.0);
        for r in crate::checks::check_svg(&svg, &opts) {
            assert!(r.pass, "{name}: {} {:?}", r.name, r.details);
        }
        render_png(&svg);
    }
}

/// The hatching look whose lines are `min_pt` wide on a page where the
/// shield is `size_mm` wide, as the map and the CLI make it.
fn print_look(c: &Coa, size_mm: f64, min_pt: f64) -> Look {
    let view = draw(c, "coa-v", &Look::COLOUR).view;
    Look {
        hatching: true,
        min_line: crate::page::line_mm(min_pt) * view[2] / size_mm,
    }
}

#[test]
fn a_transform_in_a_charge_file_does_not_make_lines_thinner() {
    // The file of the millstone draws its art at 0.18 of the size.
    let file = charges::FILES
        .iter()
        .find(|(n, _)| *n == "millstone")
        .unwrap()
        .1;
    assert!(file.contains("transform=\"matrix(0.18,0,0,0.18,"));
    let c = coa(
        r#"{"t1":"argent","charges":[{"charge":"millstone","t":"azure","p":"e"}],"shield":"heater"}"#,
    );
    let opts = crate::checks::CheckOptions {
        min_line_width_pt: 0.5,
        fonts_allowed: Vec::new(),
    };
    let svg = document(&c, "coa-m", &print_look(&c, 40.0, 0.5), 40.0);
    for r in crate::checks::check_svg(&svg, &opts) {
        assert!(r.pass, "{} {:?}", r.name, r.details);
    }
}

#[test]
fn a_transformed_part_of_a_charge_gets_a_wider_line() {
    // The file of the millstone draws its art at 0.18 of the size.
    let c = coa(
        r#"{"t1":"argent","charges":[{"charge":"millstone","t":"azure","p":"e"}],"shield":"heater"}"#,
    );
    let look = Look {
        hatching: true,
        min_line: 2.0,
    };
    let d = draw(&c, "coa-w", &look);
    let svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\">{}</svg>", d.body);
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let width =
        |n: roxmltree::Node| -> f64 { n.attribute("stroke-width").unwrap().parse().unwrap() };
    let used = doc
        .descendants()
        .find(|n| n.tag_name().name() == "use")
        .map(width)
        .unwrap();
    let scaled = doc
        .descendants()
        .find(|n| n.attribute("transform") == Some("matrix(0.18,0,0,0.18,63.25,73.1)"))
        .unwrap();
    // After the scale, the line is as wide as the line of the use.
    assert!(width(scaled) * 0.18 >= used - 1e-9, "{svg}");
    // The hatching in the scaled part is for its scale: its lines are
    // wider than those of the hatching of the use.
    let fill = scaled.attribute("fill").unwrap();
    assert!(fill.starts_with("url(#coa-w-h"), "{fill}");
    let outer = doc
        .descendants()
        .find(|n| {
            n.tag_name().name() == "g"
                && n.attribute("fill").is_some_and(|f| f.starts_with("url("))
                && n.children().any(|c| c.tag_name().name() == "use")
        })
        .and_then(|n| n.attribute("fill"))
        .unwrap();
    assert_ne!(fill, outer);
}

#[test]
fn a_hatched_orle_is_at_least_one_line_wide() {
    // A small shield: the line is wider than the band of the orle.
    let c =
        coa(r#"{"t1":"argent","ordinaries":[{"ordinary":"orle","t":"azure"}],"shield":"heater"}"#);
    let look = Look {
        hatching: true,
        min_line: 20.0,
    };
    let d = draw(&c, "coa-o", &look);
    // The band of the orle is 20 / 0.85 wide before its scale of 0.85.
    assert!(
        d.body.contains("stroke=\"#fff\" stroke-width=\"23.53\""),
        "{}",
        d.body
    );
}
