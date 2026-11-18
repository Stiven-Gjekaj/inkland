//! Draw a coat of arms as SVG, as the emblem renderer of Azgaar's Fantasy
//! Map Generator does (`src/renderers/emblems/renderer.ts`): the field,
//! the division, the ordinaries, the charges, and the shield outline.
//!
//! Two looks:
//!
//! - colour: the tinctures, patterns, and the backlight of FMG.
//! - hatching: black and white for print. Each tincture is a hatching of
//!   Petra Sancta, each line is at least `min_line` wide, and no element
//!   has an opacity, a mask, or a gradient.
//!
//! Each id starts with the prefix of the coat of arms, so that many coats
//! of arms can share one SVG.

mod charges;
mod data;

use inkland_core::emblems::{Charge, Coa, Divided, Ordinary};
use inkland_core::rand::js_round;
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub use data::Shape;

/// How to draw a coat of arms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    /// `true`: black and white with hatching. `false`: the colours of FMG.
    pub hatching: bool,
    /// With hatching: the smallest line width, in units of the shield box
    /// (200 units wide).
    pub min_line: f64,
}

impl Look {
    /// The colours of FMG.
    pub const COLOUR: Look = Look {
        hatching: false,
        min_line: 0.0,
    };
}

/// A drawn coat of arms: the view box and the content of an `<svg>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawn {
    /// `x y width height`.
    pub view: [f64; 4],
    /// The definitions and the drawing.
    pub body: String,
}

impl Drawn {
    /// The view box as SVG text.
    pub fn view_box(&self) -> String {
        let [x, y, w, h] = self.view;
        format!("{} {} {} {}", num(x), num(y), num(w), num(h))
    }
}

/// A whole SVG file of one coat of arms, `size` pixels wide.
pub fn document(coa: &Coa, prefix: &str, look: &Look, size: f64) -> String {
    let d = draw(coa, prefix, look);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{s}\" height=\"{s}\" viewBox=\"{}\">\n{}</svg>\n",
        d.view_box(),
        d.body,
        s = num(size)
    )
}

/// A number as FMG writes it: at most 2 decimals, no trailing zeros.
fn num(v: f64) -> String {
    let r = js_round(v * 100.0) / 100.0;
    let r = if r == 0.0 { 0.0 } else { r };
    format!("{r}")
}

/// The shape of a shield, or `None`.
pub fn shape(name: &str) -> Option<&'static Shape> {
    data::SHIELDS
        .binary_search_by(|s| s.name.cmp(name))
        .ok()
        .map(|i| &data::SHIELDS[i])
}

/// The names of the charges that inkland has the art of, sorted.
pub fn charge_names() -> impl Iterator<Item = &'static str> {
    charges::FILES.iter().map(|(name, _)| *name)
}

fn lookup<'a>(table: &'a [(&str, &'a str)], key: &str) -> Option<&'a str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

fn template(
    table: &[(&str, &str, Option<&str>)],
    name: &str,
    line: Option<&str>,
) -> Option<String> {
    let (_, plain, lined) = table.iter().find(|(k, _, _)| *k == name)?;
    match (line, lined) {
        (Some(l), Some(lined)) if l != "straight" => match lookup(data::LINES, l) {
            Some(path) => Some(lined.replace("{LINE}", path)),
            None => Some((*plain).to_owned()),
        },
        _ => Some((*plain).to_owned()),
    }
}

/// `true` if a colour is dark: black in the hatching look.
fn is_dark(hex: &str) -> bool {
    let h = hex.trim_start_matches('#');
    let full: String = match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).collect(),
        6 => h.to_owned(),
        _ => return false,
    };
    let v = |i: usize| f64::from(u8::from_str_radix(&full[i..i + 2], 16).unwrap_or(255)) / 255.0;
    0.2126 * v(0) + 0.7152 * v(2) + 0.0722 * v(4) < 0.5
}

/// The state of one drawing: its definitions.
struct Painter<'a> {
    prefix: &'a str,
    look: &'a Look,
    shield: &'static str,
    shield_path: &'static str,
    /// Definitions by id, in the order of their ids.
    defs: BTreeMap<String, String>,
    /// The id of each tincture paint (a pattern or a hatching).
    paints: BTreeMap<String, String>,
    /// The id of each charge definition, by its name and tinctures.
    charge_ids: BTreeMap<(String, String, String, String), String>,
}

impl Painter<'_> {
    fn new_id(&self, kind: &str, count: usize) -> String {
        format!("{}-{kind}{count}", self.prefix)
    }

    /// The paint of a tincture: a colour, or `url(#...)` of a pattern or a
    /// hatching (`clr`).
    /// `scale` is the scale of the user space of the element, for the
    /// width of the hatching lines.
    fn paint(&mut self, tincture: &str, scale: f64) -> String {
        let key = if self.look.hatching {
            format!("{tincture} {}", num(scale))
        } else {
            tincture.to_owned()
        };
        if let Some(id) = self.paints.get(&key) {
            return format!("url(#{id})");
        }
        if !tincture.contains('-') {
            if self.look.hatching {
                return self.hatch(tincture, scale, &key);
            }
            if let Some(c) = lookup(data::COLORS, tincture) {
                return c.to_owned();
            }
            if tincture.starts_with('#') {
                return tincture.to_owned();
            }
            return "#000000".to_owned();
        }
        let parts: Vec<&str> = tincture.split('-').collect();
        let (pattern, t1, t2) = (
            parts[0],
            parts.get(1).copied().unwrap_or(""),
            parts.get(2).copied().unwrap_or(""),
        );
        let size = match parts.get(3).copied() {
            Some("small") => 0.8,
            Some("smaller") => 0.5,
            Some("smallest") => 0.25,
            Some("big") => 1.6,
            Some("bigger") => 2.0,
            _ => 1.0,
        };
        let semy = pattern.strip_prefix("semy_of_");
        let Some(markup) = lookup(
            data::PATTERNS,
            if semy.is_some() { "semy" } else { pattern },
        ) else {
            return if self.look.hatching {
                "#000".to_owned()
            } else {
                "#000000".to_owned()
            };
        };
        let mut text = markup.to_owned();
        // {size*N}
        while let Some(at) = text.find("{size*") {
            let end = at + text[at..].find('}').unwrap_or(0);
            let n: f64 = text[at + 6..end].parse().unwrap_or(25.0);
            text.replace_range(at..=end, &js_num(size * n));
        }
        // The content of the tile has the scale of the tile: a hatching in
        // it needs that scale.
        let tile = tile_scale(&text);
        let (c1, c2) = (self.paint(t1, scale * tile), self.paint(t2, scale * tile));
        let id = self.new_id("p", self.paints.len());
        self.paints.insert(key, id.clone());
        let charge = semy
            .map(|c| self.charge_def(c, &c2, &c2, &c2, None))
            .unwrap_or_default();
        text = text
            .replace("{p}", &id)
            .replace("{c1}", &c1)
            .replace("{c2}", &c2)
            .replace("{charge}", &charge);
        if self.look.hatching {
            text = print_pattern(&text, self.look.min_line / scale);
        }
        self.defs.insert(id.clone(), text);
        format!("url(#{id})")
    }

    /// The hatching of Petra Sancta for a plain tincture.
    fn hatch(&mut self, tincture: &str, scale: f64, key: &str) -> String {
        let lines: &[&str] = match tincture {
            "argent" => return "#fff".to_owned(),
            "sable" => return "#000".to_owned(),
            "or" => &["dots"],
            "gules" => &["v"],
            "azure" => &["h"],
            "vert" => &["d"],
            "purpure" => &["s"],
            "tenné" => &["s", "h"],
            "sanguine" => &["d", "s"],
            "murrey" => &["d", "h"],
            "carnation" => &["v", "dots"],
            "celeste" => &["h", "dots"],
            "cendrée" => &["h", "v"],
            t if t.starts_with('#') => return if is_dark(t) { "#000" } else { "#fff" }.to_owned(),
            _ => return "#000".to_owned(),
        };
        let count = self.paints.len();
        let id = format!("{}-h{count}-{}", self.prefix, hatch_name(tincture));
        self.paints.insert(key.to_owned(), id.clone());
        let w = self.look.min_line / scale;
        let step = (w * 3.0).max(8.0);
        let mut body = format!(
            "<rect x=\"0\" y=\"0\" width=\"{s}\" height=\"{s}\" fill=\"#fff\" stroke=\"none\"/>",
            s = js_num(step)
        );
        for l in lines {
            let d = match *l {
                "v" => format!("M{h} 0 V{s}", h = js_num(step / 2.0), s = js_num(step)),
                "h" => format!("M0 {h} H{s}", h = js_num(step / 2.0), s = js_num(step)),
                "d" => format!(
                    "M0 0 L{s} {s} M{n} {e} L{e} {p} M{e} {n} L{p} {e}",
                    s = js_num(step),
                    n = js_num(-step),
                    e = "0",
                    p = js_num(step)
                ),
                "s" => format!(
                    "M{s} 0 L0 {s} M0 0 L{n} {s2} M{s2} {n} L{s} 0",
                    s = js_num(step),
                    n = js_num(-step),
                    s2 = js_num(step)
                ),
                _ => String::new(),
            };
            if *l == "dots" {
                let _ = write!(
                    body,
                    "<circle cx=\"{c}\" cy=\"{c}\" r=\"{r}\" fill=\"#000\" stroke=\"none\"/>",
                    c = js_num(step / 2.0),
                    r = js_num(w.max(1.0))
                );
            } else {
                let _ = write!(
                    body,
                    "<path d=\"{d}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>",
                    js_num(w)
                );
            }
        }
        let markup = format!(
            "<pattern id=\"{id}\" width=\"{s}\" height=\"{s}\" patternUnits=\"userSpaceOnUse\">{body}</pattern>",
            s = js_num(step)
        );
        self.defs.insert(id.clone(), markup);
        format!("url(#{id})")
    }

    /// The id of a charge definition with its tinctures (`getCharge`).
    /// `width` is the line width of an inescutcheon, in its own units.
    fn charge_def(
        &mut self,
        name: &str,
        t: &str,
        t2: &str,
        t3: &str,
        width: Option<f64>,
    ) -> String {
        let key = (
            name.to_owned(),
            t.to_owned(),
            t2.to_owned(),
            format!("{t3} {}", width.map(num).unwrap_or_default()),
        );
        if let Some(id) = self.charge_ids.get(&key) {
            return id.clone();
        }
        let id = self.new_id("c", self.charge_ids.len());
        self.charge_ids.insert(key, id.clone());
        let inner = if let Some(shape_name) = inescutcheon_shape(name, self.shield) {
            let path = shape(&shape_name).map_or(self.shield_path, |s| s.path);
            let w = width
                .map(|w| format!(" stroke-width=\"{}\"", num(w)))
                .unwrap_or_default();
            format!("<path transform=\"translate(67 67) scale(.33)\" d=\"{path}\"{w}/>")
        } else {
            let file = charges::FILES
                .binary_search_by(|(k, _)| (*k).cmp(name))
                .map(|i| charges::FILES[i].1)
                .unwrap_or("");
            charge_markup(file, &id, t2, t3, self.look)
        };
        self.defs
            .insert(id.clone(), format!("<g id=\"{id}\">{inner}</g>"));
        id
    }
}

fn hatch_name(t: &str) -> &str {
    match t {
        "tenné" => "tenne",
        "cendrée" => "cendree",
        other => other,
    }
}

/// A number as JavaScript writes it.
fn js_num(v: f64) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    format!("{v}")
}

/// The shield an inescutcheon charge draws: `inescutcheonRound` is
/// `round`, a bare `inescutcheon` takes the shield of the arms.
fn inescutcheon_shape(charge: &str, shield: &str) -> Option<String> {
    let named = charge.strip_prefix("inescutcheon")?;
    if named.is_empty() {
        return Some(shield.to_owned());
    }
    let mut chars = named.chars();
    let first = chars.next().map(|c| c.to_ascii_lowercase());
    Some(first.into_iter().chain(chars).collect())
}

/// The value of an attribute in a start tag.
fn attr<'t>(tag: &'t str, name: &str) -> Option<&'t str> {
    let key = format!(" {name}=\"");
    let at = tag.find(&key)? + key.len();
    let end = tag[at..].find('"')? + at;
    Some(&tag[at..end])
}

/// A start tag without an attribute.
fn drop_attr(tag: &str, name: &str) -> String {
    let key = format!(" {name}=\"");
    match tag.find(&key) {
        Some(at) => {
            let rest = &tag[at + key.len()..];
            let end = rest.find('"').map_or(rest.len(), |e| e + 1);
            format!("{}{}", &tag[..at], &rest[end..])
        }
        None => tag.to_owned(),
    }
}

/// A start tag with a new attribute before its end.
fn add_attr(tag: &str, name: &str, value: &str) -> String {
    let close = if tag.ends_with("/>") {
        tag.len() - 2
    } else {
        tag.len() - 1
    };
    format!("{} {name}=\"{value}\"{}", &tag[..close], &tag[close..])
}

/// Change the ids in a charge file to ids under `prefix`, as lower-case
/// words joined by `-`.
fn scoped(prefix: &str, id: &str) -> String {
    let mut out = format!("{prefix}-");
    for ch in id.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    out
}

/// The markup of a charge file for one set of tinctures: the content of
/// the file without the outer `<svg>` and the metadata, with the classes
/// of FMG (`secondary`, `tertiary`, `pseudostroke`, `background`) changed
/// to fills. With hatching, each colour becomes black or white, the
/// elements with an opacity go, and the lines get the width of the use.
fn charge_markup(file: &str, prefix: &str, t2: &str, t3: &str, look: &Look) -> String {
    let start = file
        .find("<svg")
        .and_then(|s| file[s..].find('>').map(|e| s + e + 1))
        .unwrap_or(0);
    let end = file.rfind("</svg>").unwrap_or(file.len());
    let text = &file[start..end];
    let mut out = String::new();
    let mut rest = text;
    let mut first_g = true;
    // Depth of an element that the hatching look leaves out.
    let mut skip: Option<usize> = None;
    let mut depth = 0usize;
    while let Some(lt) = rest.find('<') {
        let between = &rest[..lt];
        if skip.is_none() {
            out.push_str(between.trim());
        }
        let gt = rest[lt..].find('>').map_or(rest.len(), |e| lt + e + 1);
        let tag = &rest[lt..gt];
        rest = &rest[gt..];
        let closing = tag.starts_with("</");
        let self_closing = tag.ends_with("/>");
        if closing {
            depth = depth.saturating_sub(1);
            if skip == Some(depth) {
                skip = None;
                continue;
            }
            if skip.is_none() {
                out.push_str(tag);
            }
            continue;
        }
        let name: String = tag[1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == ':')
            .collect();
        if !self_closing {
            depth += 1;
        }
        if skip.is_some() || name == "metadata" || tag.starts_with("<?") || tag.starts_with("<!") {
            continue;
        }
        let mut t = tag.to_owned();
        if look.hatching {
            let faint = ["opacity", "fill-opacity", "stroke-opacity"]
                .iter()
                .any(|k| {
                    attr(&t, k)
                        .and_then(|v| v.parse::<f64>().ok())
                        .is_some_and(|v| v < 1.0)
                });
            if faint {
                if !self_closing {
                    skip = Some(depth - 1);
                }
                continue;
            }
        }
        if name == "g" && first_g {
            first_g = false;
            t = drop_attr(&t, "id");
        }
        if let Some(id) = attr(&t, "id").map(str::to_owned) {
            t = t.replace(
                &format!(" id=\"{id}\""),
                &format!(" id=\"{}\"", scoped(prefix, &id)),
            );
        }
        for key in ["href", "xlink:href"] {
            if let Some(h) = attr(&t, key)
                .and_then(|h| h.strip_prefix('#'))
                .map(str::to_owned)
            {
                t = drop_attr(&t, key);
                t = add_attr(&t, "href", &format!("#{}", scoped(prefix, &h)));
            }
        }
        if let Some(classes) = attr(&t, "class").map(str::to_owned) {
            t = drop_attr(&t, "class");
            let styled = matches!(name.as_str(), "g" | "path" | "circle" | "ellipse" | "rect");
            let mut fill = None;
            for c in classes.split_whitespace() {
                match c {
                    "secondary" => fill = Some(t2.to_owned()),
                    "tertiary" => fill = Some(t3.to_owned()),
                    "pseudostroke" => fill = Some("#000".to_owned()),
                    _ => {}
                }
            }
            if styled && let Some(f) = fill {
                t = drop_attr(&t, "fill");
                t = add_attr(&t, "fill", &f);
            }
        }
        if look.hatching {
            for key in ["fill", "stroke"] {
                if let Some(v) = attr(&t, key).map(str::to_owned)
                    && v.starts_with('#')
                    && !v.starts_with("url")
                {
                    t = drop_attr(&t, key);
                    t = add_attr(&t, key, if is_dark(&v) { "#000" } else { "#fff" });
                }
                if attr(&t, key).is_some_and(|v| v == "black") {
                    t = drop_attr(&t, key);
                    t = add_attr(&t, key, "#000");
                }
            }
            for key in ["stroke-width", "opacity", "fill-opacity", "stroke-opacity"] {
                t = drop_attr(&t, key);
            }
        }
        out.push_str(&t);
    }
    out
}

/// The scale of the content of a pattern tile: its width over the width
/// of its view box.
fn tile_scale(text: &str) -> f64 {
    let open_end = text.find('>').unwrap_or(text.len());
    let open = &text[..open_end];
    let width: f64 = attr(open, "width")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let view_w: f64 = attr(open, "viewBox")
        .and_then(|v| v.split_whitespace().nth(2))
        .and_then(|v| v.parse().ok())
        .unwrap_or(width);
    if view_w > 0.0 { width / view_w } else { 1.0 }
}

/// A pattern for print: each line is `min_line` wide or wider on the
/// page. The width is in the units of the pattern tile.
fn print_pattern(text: &str, min_line: f64) -> String {
    let open_end = text.find('>').unwrap_or(text.len());
    let open = &text[..open_end];
    let width: f64 = attr(open, "width")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let view_w: f64 = attr(open, "viewBox")
        .and_then(|v| v.split_whitespace().nth(2))
        .and_then(|v| v.parse().ok())
        .unwrap_or(width);
    let tile_scale = if view_w > 0.0 { width / view_w } else { 1.0 };
    let local = min_line / tile_scale.max(1e-9);
    let mut out = String::new();
    let mut rest = text;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let gt = rest[lt..].find('>').map_or(rest.len(), |e| lt + e + 1);
        let mut t = rest[lt..gt].to_owned();
        rest = &rest[gt..];
        if !t.starts_with("</") {
            if let Some(v) = attr(&t, "stroke-width").and_then(|v| v.parse::<f64>().ok()) {
                t = drop_attr(&t, "stroke-width");
                t = add_attr(&t, "stroke-width", &js_num(v.max(local)));
            } else if t.starts_with("<pattern") {
                t = add_attr(&t, "stroke-width", &js_num(local.max(1.0)));
            }
            for key in ["opacity", "fill-opacity", "stroke-opacity"] {
                t = drop_attr(&t, key);
            }
        }
        out.push_str(&t);
    }
    out.push_str(rest);
    out
}

/// The transform of a charge at a position (`useTransform`).
fn use_transform(charge: &Charge, at: [f64; 2], size_factor: f64) -> (String, f64) {
    let r = |v: f64| js_round(v * 100.0) / 100.0;
    let size = r(charge.size.unwrap_or(1.0) * size_factor);
    let sx = if charge.sinister { -size } else { size };
    let sy = if charge.reversed { -size } else { size };
    let x = r(at[0] - 100.0 * (sx - 1.0));
    let y = r(at[1] - 100.0 * (sy - 1.0));
    let mut t = String::new();
    if x != 0.0 || y != 0.0 {
        let _ = write!(t, "translate({} {})", js_num(x), js_num(y));
    }
    if sx != 1.0 || sy != 1.0 {
        if sx == sy {
            let _ = write!(t, " scale({})", js_num(sx));
        } else {
            let _ = write!(t, " scale({} {})", js_num(sx), js_num(sy));
        }
    }
    (t.trim().to_owned(), size.abs())
}

/// Draw a coat of arms (`EmblemRenderer.draw`). `prefix` starts each id;
/// it must be lower-case words joined by `-`.
pub fn draw(coa: &Coa, prefix: &str, look: &Look) -> Drawn {
    let sh = coa
        .shield
        .as_deref()
        .and_then(shape)
        .or_else(|| shape("heater"))
        .expect("the heater shield exists");
    let shield_name = sh.name;
    let positions: &[(char, [f64; 2])] = if sh.positions.is_empty() {
        shape("spanish").map_or(&[], |s| s.positions)
    } else {
        sh.positions
    };
    let mut pt = Painter {
        prefix,
        look,
        shield: shield_name,
        shield_path: sh.path,
        defs: BTreeMap::new(),
        paints: BTreeMap::new(),
        charge_ids: BTreeMap::new(),
    };
    let lw = look.min_line;
    let division = coa.division.as_ref().filter(|d| {
        d.division != "no" && data::DIVISION_TEMPLATES.iter().any(|t| t.0 == d.division)
    });
    let regular: Vec<&Ordinary> = coa.ordinaries.iter().collect();

    let draw_charge =
        |pt: &mut Painter, c: &Charge, t: &str, t2: Option<&str>, t3: Option<&str>| -> String {
            let name = if c.charge == "inescutcheon" {
                let mut chars = pt.shield.chars();
                let first = chars.next().map(|ch| ch.to_ascii_uppercase());
                format!(
                    "inescutcheon{}",
                    first.into_iter().chain(chars).collect::<String>()
                )
            } else {
                c.charge.clone()
            };
            // The scale of the uses: the same at each position.
            let (_, use_scale) = use_transform(c, [0.0, 0.0], sh.size);
            let use_scale = use_scale.max(0.01);
            let inescutcheon = name.starts_with("inescutcheon");
            let art_scale = if inescutcheon {
                use_scale * 0.33
            } else {
                use_scale
            };
            let fill = pt.paint(t, art_scale);
            let f2 = t2.map_or_else(|| fill.clone(), |x| pt.paint(x, art_scale));
            let f3 = t3.map_or_else(|| fill.clone(), |x| pt.paint(x, art_scale));
            let width = (look.hatching && inescutcheon).then(|| lw / art_scale);
            let id = pt.charge_def(&name, &fill, &f2, &f3, width);
            let mut seen = String::new();
            let mut uses = String::new();
            for ch in c.p.chars() {
                if seen.contains(ch) {
                    continue;
                }
                seen.push(ch);
                let Some(at) = positions.iter().find(|(k, _)| *k == ch).map(|(_, v)| *v) else {
                    continue;
                };
                let (tr, _) = use_transform(c, at, sh.size);
                let width = if look.hatching {
                    format!(" stroke-width=\"{}\"", num(lw / use_scale))
                } else {
                    String::new()
                };
                let tr = if tr.is_empty() {
                    String::new()
                } else {
                    format!(" transform=\"{tr}\"")
                };
                let _ = write!(uses, "<use href=\"#{id}\"{tr}{width}/>");
            }
            format!("<g fill=\"{fill}\" stroke=\"#000\">{uses}</g>")
        };

    let draw_ordinary = |pt: &mut Painter, o: &Ordinary, t: &str| -> String {
        let fill = pt.paint(t, 1.0);
        let kind = o.ordinary.as_str();
        if kind == "bordure" || kind == "orle" {
            let width = if kind == "bordure" { 33.3 } else { 10.0 };
            let tr = if kind == "orle" {
                " transform=\"translate(15 15) scale(.85)\""
            } else {
                ""
            };
            let path = pt.shield_path;
            let mut s = String::new();
            if look.hatching {
                let _ = write!(
                    s,
                    "<path d=\"{path}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"{tr}/>",
                    num(width + 2.0 * lw)
                );
                if kind == "orle" {
                    let _ = write!(
                        s,
                        "<path d=\"{path}\" fill=\"none\" stroke=\"#fff\" stroke-width=\"{}\"{tr}/>",
                        num(width)
                    );
                }
            }
            let _ = write!(
                s,
                "<path d=\"{path}\" fill=\"none\" stroke=\"{fill}\" stroke-width=\"{}\"{tr}/>",
                num(width)
            );
            return format!("<g fill=\"{fill}\" stroke=\"none\">{s}</g>");
        }
        let Some(content) = template(data::ORDINARY_TEMPLATES, kind, o.line.as_deref()) else {
            return String::new();
        };
        if look.hatching {
            format!(
                "<g fill=\"{fill}\" stroke=\"#000\" stroke-width=\"{}\">{content}</g>",
                num(lw)
            )
        } else {
            format!("<g fill=\"{fill}\" stroke=\"none\" stroke-width=\"1\">{content}</g>")
        }
    };

    let field_paint = pt.paint(&coa.t1, 1.0);
    let mut body =
        format!("<rect x=\"0\" y=\"0\" width=\"200\" height=\"200\" fill=\"{field_paint}\"/>");

    if let Some(d) = division {
        // The parts: own elements, and the counterchanged ones in the
        // tincture of the other part.
        let part = |pt: &mut Painter, which: Divided, counter: &str| -> String {
            let mut s = String::new();
            for o in regular
                .iter()
                .filter(|o| o.divided == Some(which) || o.divided == Some(Divided::Counter))
            {
                let t = if o.divided == Some(which) {
                    o.t.clone()
                } else {
                    counter.to_owned()
                };
                s.push_str(&draw_ordinary(pt, o, &t));
            }
            for c in coa
                .charges
                .iter()
                .filter(|c| c.divided == Some(which) || c.divided == Some(Divided::Counter))
            {
                if c.divided == Some(which) {
                    s.push_str(&draw_charge(pt, c, &c.t, c.t2.as_deref(), c.t3.as_deref()));
                } else {
                    s.push_str(&draw_charge(pt, c, counter, None, None));
                }
            }
            s
        };
        body.push_str(&part(&mut pt, Divided::Field, &d.t));
        let clip_id = format!("{prefix}-division");
        let shape =
            template(data::DIVISION_TEMPLATES, &d.division, d.line.as_deref()).unwrap_or_default();
        pt.defs.insert(
            clip_id.clone(),
            format!("<clipPath id=\"{clip_id}\">{shape}</clipPath>"),
        );
        let dt = pt.paint(&d.t, 1.0);
        let _ = write!(
            body,
            "<g clip-path=\"url(#{clip_id})\"><rect x=\"0\" y=\"0\" width=\"200\" height=\"200\" fill=\"{dt}\"/>"
        );
        body.push_str(&part(&mut pt, Divided::Division, &coa.t1));
        body.push_str("</g>");
        if look.hatching {
            // The line of partition.
            let _ = write!(
                body,
                "<g fill=\"none\" stroke=\"#000\" stroke-width=\"{}\">{shape}</g>",
                num(lw)
            );
        }
    }

    let on_field = |c: &&Charge| c.divided.is_none() || division.is_none();
    for o in regular
        .iter()
        .filter(|o| o.divided.is_none() || division.is_none())
    {
        body.push_str(&draw_ordinary(&mut pt, o, &o.t));
    }
    for c in coa.charges.iter().filter(on_field) {
        body.push_str(&draw_charge(
            &mut pt,
            c,
            &c.t,
            c.t2.as_deref(),
            c.t3.as_deref(),
        ));
    }

    let clip = format!("{prefix}-shield");
    let mut defs = format!(
        "<clipPath id=\"{clip}\"><path d=\"{}\"/></clipPath>",
        sh.path
    );
    let overlay = if look.hatching {
        format!(
            "<path d=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>",
            sh.path,
            num(lw * 2.0)
        )
    } else {
        let light = format!("{prefix}-backlight");
        let _ = write!(
            defs,
            "<radialGradient id=\"{light}\" cx=\"100%\" cy=\"100%\" r=\"150%\"><stop stop-color=\"#fff\" stop-opacity=\".3\" offset=\"0\"/><stop stop-color=\"#fff\" stop-opacity=\".15\" offset=\".25\"/><stop stop-color=\"#000\" stop-opacity=\"0\" offset=\"1\"/></radialGradient>"
        );
        format!(
            "<path d=\"{}\" fill=\"url(#{light})\" stroke=\"#333\"/>",
            sh.path
        )
    };
    for v in pt.defs.values() {
        defs.push_str(v);
    }
    let [x0, y0, w0, h0] = sh.view;
    let w = js_round(w0);
    let h = js_round(h0);
    Drawn {
        view: [x0 - w / 2.0 + 100.0, y0 - h / 2.0 + 100.0, w, h],
        body: format!("<defs>{defs}</defs><g clip-path=\"url(#{clip})\">{body}</g>{overlay}"),
    }
}

#[cfg(test)]
mod tests;
