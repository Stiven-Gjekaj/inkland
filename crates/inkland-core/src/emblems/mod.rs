//! Coats of arms (emblems) of states, provinces, and places.
//!
//! The generator follows `src/model/political/emblems-generator.ts` of
//! Azgaar's Fantasy Map Generator (FMG): tinctures with the rule of
//! tincture, divisions, ordinaries, charges, and the inheritance from the
//! arms of the parent. The data tables are in `data.rs`.
//!
//! A coat of arms is data in the JSON shape of FMG (`Coa`). The renderer
//! draws it (`inkland_render::emblem`).

pub mod data;

use crate::rand::Rand;
use data::Weights;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A coat of arms, in the JSON shape of Azgaar's Fantasy Map Generator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Coa {
    /// The tincture of the field: a metal (`argent`, `or`), a colour
    /// (`gules`, `azure`, `sable`, `purpure`, `vert`, ...), a stain
    /// (`murrey`, `sanguine`, `tenné`), or a pattern
    /// `<pattern>-<tincture>-<tincture>[-<size>]`, for example
    /// `vair-azure-argent` or `semy_of_mullet-or-azure-small`.
    pub t1: String,
    /// The division of the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub division: Option<Division>,
    /// The ordinaries, from bottom to top.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ordinaries: Vec<Ordinary>,
    /// The charges, from bottom to top.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub charges: Vec<Charge>,
    /// The shield shape, for example `heater`, `spanish`, or `round`.
    /// Default: `heater`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield: Option<String>,
}

/// The division of the field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Division {
    /// The division, for example `perPale`, `perFess`, or `perCross`.
    pub division: String,
    /// The tincture of the second part.
    pub t: String,
    /// The line of partition, for example `straight`, `wavy`, or
    /// `embattled`. Default: `straight`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
}

/// An ordinary: a band, a cross, a border, and so on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ordinary {
    /// The ordinary, for example `pale`, `fess`, `bend`, `chief`, `cross`,
    /// `bordure`, or `canton`.
    pub ordinary: String,
    /// The tincture.
    pub t: String,
    /// The line of its edges. Default: `straight`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    /// With a division: `field` draws it only on the field, `division` only
    /// on the second part, `counter` in the tincture of the other part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divided: Option<Divided>,
}

/// A charge: a figure at one or more positions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Charge {
    /// The charge, for example `mullet`, `crossHummetty`, or
    /// `inescutcheon`.
    pub charge: String,
    /// The tincture.
    pub t: String,
    /// The positions on the shield: one letter for each copy. `a` to `i`
    /// are a grid of 3 x 3 from the top left, `e` is the center, `j` to `q`
    /// are smaller grids, `y` is the canton, `z` the base, and `A` to `L`
    /// go around the border.
    pub p: String,
    /// The second tincture of a charge with more colours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t2: Option<String>,
    /// The third tincture of a charge with more colours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t3: Option<String>,
    /// With a division: as `divided` of an ordinary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divided: Option<Divided>,
    /// The size. Default: 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub size: Option<f64>,
    /// `1` or `true`: turned to face the other side.
    #[serde(
        default,
        skip_serializing_if = "is_false",
        serialize_with = "flag_out",
        deserialize_with = "flag_in"
    )]
    #[schemars(with = "Flag")]
    pub sinister: bool,
    /// `1` or `true`: upside down.
    #[serde(
        default,
        skip_serializing_if = "is_false",
        serialize_with = "flag_out",
        deserialize_with = "flag_in"
    )]
    #[schemars(with = "Flag")]
    pub reversed: bool,
}

/// The part of a division that an ordinary or a charge is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Divided {
    Field,
    Division,
    Counter,
}

/// A flag of FMG: a boolean, or the number 0 or 1.
#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
enum Flag {
    Bool(bool),
    /// 0 or 1.
    Number(u8),
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn flag_out<S: Serializer>(v: &bool, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_u8(u8::from(*v))
}

fn flag_in<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    match Flag::deserialize(d)? {
        Flag::Bool(b) => Ok(b),
        Flag::Number(0) => Ok(false),
        Flag::Number(1) => Ok(true),
        Flag::Number(n) => Err(serde::de::Error::custom(format!(
            "a flag is true, false, 0, or 1, not {n}"
        ))),
    }
}

impl Charge {
    fn new(charge: &str, t: &str, p: &str) -> Charge {
        Charge {
            charge: charge.to_owned(),
            t: t.to_owned(),
            p: p.to_owned(),
            t2: None,
            t3: None,
            divided: None,
            size: None,
            sinister: false,
            reversed: false,
        }
    }
}

/// The weights of a key in a table, or `None`.
fn lookup<'a, T>(table: &'a [(&str, T)], key: &str) -> Option<&'a T> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

/// The data of a charge, or `None`.
pub fn charge_data(name: &str) -> Option<&'static data::ChargeData> {
    data::CHARGE_DATA
        .binary_search_by(|(k, _)| (*k).cmp(name))
        .ok()
        .map(|i| &data::CHARGE_DATA[i].1)
}

/// `true` if inkland has the art of a charge: a charge of `data::ART`, or
/// an inescutcheon (`inescutcheon`, or `inescutcheon` and a shield name).
pub fn has_art(name: &str) -> bool {
    if let Some(shape) = name.strip_prefix("inescutcheon") {
        if shape.is_empty() {
            return true;
        }
        let mut chars = shape.chars();
        let first = chars.next().map(|c| c.to_ascii_lowercase());
        let shape: String = first.into_iter().chain(chars).collect();
        return is_shield(&shape);
    }
    data::ART.binary_search(&name).is_ok()
}

/// The names of all shields.
pub fn shield_names() -> impl Iterator<Item = &'static str> {
    data::SHIELDS
        .iter()
        .flat_map(|(_, list)| list.iter().map(|(k, _)| *k))
}

/// `true` if `name` is a shield shape.
pub fn is_shield(name: &str) -> bool {
    shield_names().any(|s| s == name)
}

/// The charges of a category that inkland has the art of.
fn drawable(list: Weights) -> Vec<(&'static str, u32)> {
    list.iter()
        .copied()
        .filter(|(name, w)| *w > 0 && has_art(name))
        .collect()
}

/// The categories of a set that have one charge or more with art.
fn drawable_categories(set: Weights) -> Vec<(&'static str, u32)> {
    set.iter()
        .copied()
        .filter(|(cat, w)| {
            *w > 0
                && (*cat == "inescutcheon"
                    || lookup(data::CHARGES, cat).is_some_and(|l| !drawable(l).is_empty()))
        })
        .collect()
}

/// The type of a plain tincture: `metals`, `colours`, `stains`, or `None`.
fn plain_type(t: &str) -> Option<&'static str> {
    if lookup(data::METALS, t).is_some() {
        Some("metals")
    } else if lookup(data::COLOURS, t).is_some() {
        Some("colours")
    } else if lookup(data::STAINS, t).is_some() {
        Some("stains")
    } else {
        None
    }
}

/// `getType`: the type of the tincture, or of the first tincture of a
/// pattern.
fn tincture_type(t: &str) -> Option<&'static str> {
    let base = if t.contains('-') {
        t.split('-').nth(1).unwrap_or("")
    } else {
        t
    };
    plain_type(base)
}

/// `typeOf`: the type of a tincture; a pattern is `pattern`.
fn type_of(t: &str) -> &'static str {
    plain_type(t).unwrap_or("pattern")
}

fn same_type(a: &str, b: &str) -> bool {
    type_of(a) == type_of(b)
}

/// `true` if `value` is a tincture: a plain tincture, or a pattern of two
/// plain tinctures with an optional size.
pub fn is_tincture(value: &str) -> bool {
    if plain_type(value).is_some() {
        return true;
    }
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() < 3 || parts.len() > 4 {
        return false;
    }
    if plain_type(parts[1]).is_none() || plain_type(parts[2]).is_none() {
        return false;
    }
    if parts.len() == 4 && !SIZES.contains(&parts[3]) {
        return false;
    }
    lookup(data::PATTERNS, parts[0]).is_some()
        || parts[0]
            .strip_prefix("semy_of_")
            .is_some_and(|c| has_art(c) && !c.starts_with("inescutcheon"))
}

/// The size words of a pattern.
pub const SIZES: [&str; 5] = ["small", "smaller", "smallest", "big", "bigger"];

/// The parent of a new coat of arms, with the kinship and dominion of
/// FMG.
pub struct Parent<'a> {
    pub coa: Option<&'a Coa>,
    /// The chance to take the field, the ordinary, the division, and the
    /// charge of the parent.
    pub kinship: f64,
    /// The chance to put the arms of the parent on a canton.
    pub dominion: f64,
}

/// The generator. It holds the random source of the emblem stage.
pub struct Generator<'r> {
    rng: &'r mut Rand,
}

impl<'r> Generator<'r> {
    pub fn new(rng: &'r mut Rand) -> Self {
        Generator { rng }
    }

    fn p(&mut self, p: f64) -> bool {
        self.rng.p(p)
    }

    fn rw(&mut self, items: &[(&'static str, u32)]) -> &'static str {
        self.rng.weighted(items)
    }

    /// A random shield (`getRandomShield`).
    pub fn random_shield(&mut self) -> String {
        let kind = self.rw(data::SHIELD_TYPES);
        let list = lookup(data::SHIELDS, kind).copied().unwrap_or(&[]);
        self.rw(list).to_owned()
    }

    /// `selectCharge`: a category of the set, then a charge of it.
    fn select_charge(&mut self, set: Weights) -> String {
        let cats = drawable_categories(set);
        let kind = self.rw(&cats);
        if kind == "inescutcheon" {
            return "inescutcheon".to_owned();
        }
        let list = drawable(lookup(data::CHARGES, kind).copied().unwrap_or(&[]));
        self.rw(&list).to_owned()
    }

    /// `getTincture`: a tincture for the field, the division, or a charge.
    /// `fields` holds the tinctures of the field. `rot` is the tincture
    /// that the rule of tincture follows.
    fn tincture(&mut self, element: &str, fields: &mut Vec<String>, rot: Option<&str>) -> String {
        let base = rot.map(|r| {
            if r.contains('-') {
                r.split('-').nth(1).unwrap_or("").to_owned()
            } else {
                r.to_owned()
            }
        });
        // createTinctures: each call draws the chance of the stains.
        let mut types = [0.0f64; 3];
        for (k, w) in [data::TYPES_FIELD, data::TYPES_DIVISION, data::TYPES_CHARGE]
            .iter()
            .enumerate()
        {
            types[k] = f64::from(u8::from(self.p(w[2])));
        }
        let table = match element {
            "field" => data::TYPES_FIELD,
            "division" => data::TYPES_DIVISION,
            _ => data::TYPES_CHARGE,
        };
        let stains = match element {
            "field" => types[0],
            "division" => types[1],
            _ => types[2],
        };
        let weights = [
            ("metals", table[0] as u32),
            ("colours", table[1] as u32),
            ("stains", stains as u32),
            ("patterns", table[3] as u32),
        ];
        let mut kind = self.rw(&weights);
        if rot.is_some() && kind != "patterns" {
            let b = base.as_deref().unwrap_or("");
            kind = if tincture_type(b) == Some("metals") {
                "colours"
            } else {
                "metals"
            };
        }
        if kind == "metals"
            && fields.iter().any(|f| f == "or")
            && fields.iter().any(|f| f == "argent")
        {
            kind = "colours";
        }
        let list = match kind {
            "metals" => data::METALS,
            "colours" => data::COLOURS,
            "stains" => data::STAINS,
            _ => data::PATTERNS,
        };
        let mut t = self.rw(list);
        while base.as_deref() == Some(t) || fields.iter().any(|f| f == t) {
            t = self.rw(list);
        }
        if kind != "patterns" && element != "charge" {
            fields.push(t.to_owned());
        }
        if kind == "patterns" {
            return self.pattern(t, element, fields);
        }
        t.to_owned()
    }

    /// `definePattern`: the tinctures and the size of a pattern.
    fn pattern(&mut self, pattern: &str, element: &str, fields: &mut Vec<String>) -> String {
        let mut t1: Option<&str> = None;
        let mut t2: Option<&str> = None;
        let mut pattern = pattern.to_owned();
        let size = if self.p(0.1) {
            "-small"
        } else if self.p(0.1) {
            "-smaller"
        } else if self.p(0.01) {
            "-big"
        } else if self.p(0.005) {
            "-smallest"
        } else {
            ""
        };
        let p = pattern.as_str();
        if ["vair", "vairInPale", "vairEnPointe"].contains(&p) && self.p(0.5) {
            (t1, t2) = (Some("azure"), Some("argent"));
        } else if p == "ermine" && self.p(0.8) {
            (t1, t2) = (Some("argent"), Some("sable"));
        } else if p == "pappellony" {
            if self.p(0.2) {
                (t1, t2) = (Some("gules"), Some("or"));
            } else if self.p(0.2) {
                (t1, t2) = (Some("argent"), Some("sable"));
            } else if self.p(0.2) {
                (t1, t2) = (Some("azure"), Some("argent"));
            }
        } else if p == "masoned" {
            if self.p(0.3) {
                (t1, t2) = (Some("gules"), Some("argent"));
            } else if self.p(0.3) {
                (t1, t2) = (Some("argent"), Some("sable"));
            } else if self.p(0.1) {
                (t1, t2) = (Some("or"), Some("sable"));
            }
        } else if p == "fretty" {
            // FMG tests `t2 === "sable"` first; t2 is not set yet.
            if self.p(0.35) {
                (t1, t2) = (Some("argent"), Some("gules"));
            } else if self.p(0.25) {
                (t1, t2) = (Some("sable"), Some("or"));
            } else if self.p(0.15) {
                (t1, t2) = (Some("gules"), Some("argent"));
            }
        } else if p == "semy" {
            pattern = format!("semy_of_{}", self.select_charge(data::CHARGES_SEMY));
        }
        let (mut a, mut b) = match (t1, t2) {
            (Some(a), Some(b)) => (a.to_owned(), b.to_owned()),
            _ => {
                let metal = self.p(0.7);
                let m = self.rw(data::METALS).to_owned();
                let c = self.rw(data::COLOURS).to_owned();
                // FMG draws the first tincture first: metal then colour, or
                // colour then metal.
                if metal { (m, c) } else { (c, m) }
            }
        };
        if element == "division" {
            if fields.contains(&a) {
                a = self.replace_tincture(&a);
            }
            if fields.contains(&b) {
                b = self.replace_tincture(&b);
            }
        }
        fields.push(a.clone());
        fields.push(b.clone());
        format!("{pattern}-{a}-{b}{size}")
    }

    /// `replaceTincture`: another tincture of the same type.
    fn replace_tincture(&mut self, t: &str) -> String {
        let list = match tincture_type(t) {
            Some("metals") => data::METALS,
            Some("colours") => data::COLOURS,
            _ => data::STAINS,
        };
        loop {
            let n = self.rw(list);
            if n != t {
                return n.to_owned();
            }
        }
    }

    /// `generate`: a new coat of arms. `kind` is the type of the culture or
    /// the place, for the charges of `data::TYPE_MAPPING`.
    pub fn generate(&mut self, parent: &Parent, kind: Option<&str>) -> Coa {
        let (parent, kinship, dominion) = match parent.coa {
            Some(c) => (Some(c), parent.kinship, parent.dominion),
            None => (None, 0.0, 0.0),
        };
        let mut used_pattern: Option<String> = None;
        let mut used: Vec<String> = Vec::new();

        let t1 = match parent {
            Some(p) if self.p(kinship) => p.t1.clone(),
            _ => self.tincture("field", &mut used, None),
        };
        if t1.contains('-') {
            used_pattern = Some(t1.clone());
        }
        let mut coa = Coa {
            t1: t1.clone(),
            division: None,
            ordinaries: Vec::new(),
            charges: Vec::new(),
            shield: None,
        };

        let add_charge = self.p(if used_pattern.is_some() { 0.5 } else { 0.93 });
        let lined_ordinary = if (add_charge && self.p(0.3)) || self.p(0.5) {
            match parent {
                Some(p) if !p.ordinaries.is_empty() && self.p(kinship) => {
                    Some(p.ordinaries[0].ordinary.clone())
                }
                _ => Some(self.rw(data::ORDINARIES_LINED).to_owned()),
            }
        } else {
            None
        };
        let ordinary = if (!add_charge && self.p(0.65)) || self.p(0.3) {
            Some(match &lined_ordinary {
                Some(o) => o.clone(),
                None => self.rw(data::ORDINARIES_STRAIGHT).to_owned(),
            })
        } else {
            None
        };
        let o = ordinary.as_deref();
        let rare_divided =
            o.is_some_and(|o| ["chief", "terrace", "chevron", "quarter", "flaunches"].contains(&o));
        let divisioned = if rare_divided || (add_charge && o.is_some()) {
            self.p(0.03)
        } else if add_charge {
            self.p(0.3)
        } else if o.is_some() {
            self.p(0.7)
        } else {
            self.p(0.995)
        };
        let division = if divisioned {
            match parent.and_then(|p| p.division.as_ref()) {
                Some(d) if self.p(kinship - 0.1) => Some(d.division.clone()),
                _ => Some(self.rw(data::DIVISIONS).to_owned()),
            }
        } else {
            None
        };
        let d = division.as_deref();

        if let Some(div) = d {
            let rot = if self.p(0.98) { Some(t1.clone()) } else { None };
            let t = self.tincture("division", &mut used, rot.as_deref());
            let mut line = None;
            if data::LINED_DIVISIONS.contains(&div) {
                line = Some(if used_pattern.is_some() || (o.is_some() && self.p(0.7)) {
                    "straight".to_owned()
                } else if div == "perCross" {
                    self.rw(data::LINES_PER_CROSS).to_owned()
                } else {
                    self.rw(data::LINES).to_owned()
                });
            }
            coa.division = Some(Division {
                division: div.to_owned(),
                t,
                line,
            });
        }

        if let Some(ord) = o {
            let t = self.tincture("charge", &mut used, Some(&t1));
            let mut item = Ordinary {
                ordinary: ord.to_owned(),
                t,
                line: None,
                divided: None,
            };
            if lined_ordinary.is_some() {
                item.line = Some(
                    if used_pattern.is_some() || (d.is_some() && self.p(0.7)) {
                        "straight"
                    } else {
                        self.rw(data::LINES)
                    }
                    .to_owned(),
                );
            }
            if d.is_some()
                && !add_charge
                && used_pattern.is_none()
                && self.p(0.5)
                && ord != "bordure"
                && ord != "orle"
            {
                item.divided = Some(if self.p(0.8) {
                    Divided::Counter
                } else if self.p(0.6) {
                    Divided::Field
                } else {
                    Divided::Division
                });
            }
            coa.ordinaries.push(item);
        }

        if add_charge {
            self.add_charges(
                &mut coa,
                parent,
                kinship,
                kind,
                o,
                d,
                divisioned,
                &used_pattern,
                &mut used,
            );
        }

        // Dominions have a canton with the arms of the parent.
        if self.p(dominion)
            && let Some(parent) = parent.filter(|p| !p.charges.is_empty())
        {
            let invert = same_type(&parent.t1, &coa.t1);
            let t = if invert {
                self.tincture("division", &mut used, Some(&t1))
            } else {
                parent.t1.clone()
            };
            for i in (0..coa.charges.len()).rev() {
                let c = &mut coa.charges[i];
                if c.size == Some(1.5) {
                    c.size = Some(1.4);
                }
                c.p.retain(|ch| !matches!(ch, 'a' | 'j' | 'y'));
                if c.p.is_empty() {
                    coa.charges.remove(i);
                }
            }
            let mut charge = parent.charges[0].charge.clone();
            if charge == "inescutcheon"
                && let Some(second) = parent.charges.get(1)
            {
                charge = second.charge.clone();
            }
            let mut t2 = if invert {
                parent.t1.clone()
            } else {
                parent.charges[0].t.clone()
            };
            if same_type(&t, &t2) {
                t2 = self.tincture("charge", &mut used, Some(&t));
            }
            let mut c = Charge::new(&charge, &t2, "y");
            c.size = Some(0.5);
            coa.charges.push(c);
            coa.ordinaries.push(Ordinary {
                ordinary: "canton".to_owned(),
                t,
                line: None,
                divided: None,
            });
        }
        coa
    }

    #[allow(clippy::too_many_arguments)]
    fn add_charges(
        &mut self,
        coa: &mut Coa,
        parent: Option<&Coa>,
        kinship: f64,
        kind: Option<&str>,
        o: Option<&str>,
        d: Option<&str>,
        divisioned: bool,
        used_pattern: &Option<String>,
        used: &mut Vec<String>,
    ) {
        let t1 = coa.t1.clone();
        let charge = match parent {
            Some(p) if !p.charges.is_empty() && self.p(kinship - 0.1) => {
                p.charges[0].charge.clone()
            }
            _ => {
                let mapped = match kind {
                    Some(k) if k != "Generic" && self.p(0.3) => {
                        let list = drawable(lookup(data::TYPE_MAPPING, k).copied().unwrap_or(&[]));
                        // inkland: a type without a charge with art draws as
                        // FMG does without a type.
                        (!list.is_empty()).then(|| self.rw(&list).to_owned())
                    }
                    _ => None,
                };
                match mapped {
                    Some(c) => c,
                    None => self.select_charge(if o.is_some() || divisioned {
                        data::CHARGES_TYPES
                    } else {
                        data::CHARGES_SINGLE
                    }),
                }
            }
        };
        let cdata = charge_data(&charge);
        let ord_pos = o.and_then(|o| data::ORDINARY_POSITIONS.iter().find(|x| x.0 == o));
        let t_ord = coa.ordinaries.first().map(|x| x.t.clone());
        let on = ord_pos.map(|x| x.1).filter(|l| !l.is_empty());
        let off = ord_pos.map(|x| x.2).filter(|l| !l.is_empty());
        let div_pos = d
            .and_then(|d| lookup(data::POSITIONS_DIVISIONS, d))
            .copied();

        let (p, mut t);
        if let Some(list) = on.filter(|_| self.p(0.8)) {
            // On the ordinary: the tincture of the field type.
            p = self.rw(list).to_owned();
            t = if used_pattern.is_none() && self.p(0.3) {
                t1.clone()
            } else {
                self.tincture("charge", &mut Vec::new(), t_ord.as_deref())
            };
        } else if let Some(list) = off.filter(|_| self.p(0.95)) {
            // Off the ordinary: the tincture of the ordinary type.
            p = self.rw(list).to_owned();
            t = if used_pattern.is_none() && self.p(0.3) {
                t_ord.clone().unwrap_or_default()
            } else {
                self.tincture("charge", used, Some(&t1))
            };
        } else if let Some(list) = div_pos {
            // In the fields of the division.
            p = self.rw(list).to_owned();
            let mut fields = used.clone();
            if let Some(x) = &t_ord {
                fields.push(x.clone());
            }
            t = self.tincture("charge", &mut fields, Some(&t1));
        } else if let Some(list) = cdata.map(|c| c.positions).filter(|l| !l.is_empty()) {
            // A position that suits the charge.
            p = self.rw(list).to_owned();
            t = self.tincture("charge", used, Some(&t1));
        } else {
            // A standard position, with a new tincture.
            p = if used_pattern.is_some() {
                "e".to_owned()
            } else if lookup(data::CHARGES, "conventional")
                .is_some_and(|l| lookup(l, &charge).is_some())
            {
                self.rw(data::POSITIONS_CONVENTIONAL).to_owned()
            } else {
                self.rw(data::POSITIONS_COMPLEX).to_owned()
            };
            let mut fields = used.clone();
            if let Some(x) = &t_ord {
                fields.push(x.clone());
            }
            t = self.tincture("charge", &mut fields, Some(&t1));
        }
        if let Some(natural) = cdata.and_then(|c| c.natural)
            && natural != t
            && Some(natural) != t_ord.as_deref()
        {
            t = natural.to_owned();
        }

        let mut item = Charge::new(&charge, &t, &p);
        let colors = cdata.map_or(1, |c| c.colors);
        if colors > 1 {
            item.t2 = Some(if self.p(0.25) {
                self.tincture("charge", used, Some(&t1))
            } else {
                t.clone()
            });
        }
        if colors > 2 && item.t2.is_some() {
            item.t3 = Some(if self.p(0.5) {
                self.tincture("charge", used, Some(&t1))
            } else {
                t.clone()
            });
        }
        coa.charges = vec![item];

        let conventional = lookup(data::CHARGES, "conventional")
            .copied()
            .unwrap_or(&[]);
        if p == "ABCDEFGHIJKL" && self.p(0.95) {
            // A charge in the bordure gets a central charge.
            let list = drawable(conventional);
            coa.charges[0].charge = self.rw(&list).to_owned();
            let new = self.select_charge(data::CHARGES_SINGLE);
            let tn = self.tincture("charge", used, Some(&t1));
            coa.charges.push(Charge::new(&new, &tn, "e"));
        } else if self.p(0.8) && charge == "inescutcheon" {
            // A charge on the inescutcheon.
            let new = self.select_charge(data::CHARGES_TYPES);
            let t2 = self.tincture("charge", &mut Vec::new(), Some(&t));
            let mut c = Charge::new(&new, &t2, &p);
            c.size = Some(0.5);
            coa.charges.push(c);
        } else if let (Some(div), None) = (d, o) {
            let dline = coa.division.as_ref().and_then(|x| x.line.clone());
            let dt = coa
                .division
                .as_ref()
                .map(|x| x.t.clone())
                .unwrap_or_default();
            let allow_counter =
                used_pattern.is_none() && dline.as_deref().is_none_or(|l| l == "straight");
            if self.p(0.3)
                && ["perPale", "perFess"].contains(&div)
                && dline.as_deref() == Some("straight")
            {
                // Dimidiation: a second charge on the second part.
                coa.charges[0].divided = Some(Divided::Field);
                if self.p(0.95) {
                    let p2 = if p == "e" || self.p(0.5) {
                        "e".to_owned()
                    } else {
                        self.rw(div_pos.unwrap_or(&[("e", 1)])).to_owned()
                    };
                    let new = self.select_charge(data::CHARGES_SINGLE);
                    let tn = self.tincture("charge", used, Some(&dt));
                    let mut c = Charge::new(&new, &tn, &p2);
                    c.divided = Some(Divided::Division);
                    coa.charges.push(c);
                }
            } else if allow_counter && self.p(0.4) {
                coa.charges[0].divided = Some(Divided::Counter);
            } else if ["perPale", "perFess", "perBend", "perBendSinister"].contains(&div)
                && self.p(0.8)
            {
                // Two charges in the standard positions of the division.
                let (p1, p2) = match div {
                    "perPale" => ("p", "q"),
                    "perFess" => ("k", "n"),
                    "perBend" => ("l", "m"),
                    _ => ("j", "o"),
                };
                coa.charges[0].p = p1.to_owned();
                let new = self.select_charge(data::CHARGES_SINGLE);
                let tn = self.tincture("charge", used, Some(&dt));
                coa.charges.push(Charge::new(&new, &tn, p2));
            } else if ["perCross", "perSaltire"].contains(&div) && self.p(0.5) {
                // Four charges in the standard positions of the division.
                let ps = if div == "perCross" {
                    ["j", "l", "m", "o"]
                } else {
                    ["b", "d", "f", "h"]
                };
                coa.charges[0].p = ps[0].to_owned();
                let c2 = self.select_charge(data::CHARGES_SINGLE);
                let t2 = self.tincture("charge", &mut Vec::new(), Some(&dt));
                let c3 = self.select_charge(data::CHARGES_SINGLE);
                let t3 = self.tincture("charge", &mut Vec::new(), Some(&dt));
                let c4 = self.select_charge(data::CHARGES_SINGLE);
                let t4 = self.tincture("charge", &mut Vec::new(), Some(&t1));
                coa.charges.push(Charge::new(&c2, &t2, ps[1]));
                coa.charges.push(Charge::new(&c3, &t3, ps[2]));
                coa.charges.push(Charge::new(&c4, &t4, ps[3]));
            } else if allow_counter && p.len() > 1 {
                coa.charges[0].divided = Some(Divided::Counter);
            }
        }

        for c in &mut coa.charges {
            self.charge_attributes(o, d, c);
        }
    }

    /// `defineChargeAttributes`: the size, the clean position, and the
    /// orientation.
    fn charge_attributes(&mut self, o: Option<&str>, d: Option<&str>, c: &mut Charge) {
        c.size = Some(c.size.unwrap_or(1.0) * size_of(&c.p, o, d));
        let mut seen = String::new();
        for ch in c.p.chars() {
            if !seen.contains(ch) {
                seen.push(ch);
            }
        }
        c.p = seen;
        let data = charge_data(&c.charge);
        if self.p(0.02) && data.is_some_and(|x| x.sinister) {
            c.sinister = true;
        }
        if self.p(0.02) && data.is_some_and(|x| x.reversed) {
            c.reversed = true;
        }
    }
}

/// `getSize`: the size of a charge from its positions.
fn size_of(p: &str, o: Option<&str>, d: Option<&str>) -> f64 {
    let n = p.chars().count();
    if p == "e" && matches!(o, Some("bordure" | "orle")) {
        1.1
    } else if p == "e" {
        1.5
    } else if p == "jln" || p == "jlh" {
        0.7
    } else if [
        "abcpqh", "ez", "be", "a", "b", "c", "d", "f", "g", "h", "i", "bh", "df",
    ]
    .contains(&p)
    {
        0.5
    } else if ["j", "l", "m", "o", "jlmo"].contains(&p) && d == Some("perCross") {
        0.6
    } else if n > 10 {
        0.18
    } else if n > 7 {
        0.3
    } else if n > 4 {
        0.4
    } else if n > 2 {
        0.5
    } else {
        0.7
    }
}

/// A fault in a coat of arms of the config: a JSON Pointer under the coat
/// of arms, and a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoaFault {
    pub path: String,
    pub message: String,
}

fn fault(path: impl Into<String>, message: impl Into<String>) -> CoaFault {
    CoaFault {
        path: path.into(),
        message: message.into(),
    }
}

fn line_ok(line: Option<&str>) -> bool {
    line.is_none_or(|l| l == "straight" || lookup(data::LINES, l).is_some())
}

/// Check a coat of arms as FMG does (`requireHeraldic`): known tinctures,
/// shield, division, ordinaries, lines, and charges with art. The result
/// holds each fault; a path is relative to the coat of arms.
pub fn validate(coa: &Coa) -> Vec<CoaFault> {
    let mut out = Vec::new();
    if !is_tincture(&coa.t1) {
        out.push(fault("/t1", format!("{:?} is not a tincture", coa.t1)));
    }
    if let Some(s) = &coa.shield
        && !is_shield(s)
    {
        out.push(fault("/shield", format!("{s:?} is not a shield shape")));
    }
    if let Some(d) = &coa.division {
        if lookup(data::DIVISIONS, &d.division).is_none() {
            out.push(fault(
                "/division/division",
                format!("{:?} is not a division", d.division),
            ));
        }
        if !is_tincture(&d.t) {
            out.push(fault("/division/t", format!("{:?} is not a tincture", d.t)));
        }
        if !line_ok(d.line.as_deref()) {
            out.push(fault(
                "/division/line",
                "the line is not a line of partition",
            ));
        }
    }
    for (i, o) in coa.ordinaries.iter().enumerate() {
        if lookup(data::ORDINARIES_LINED, &o.ordinary).is_none()
            && lookup(data::ORDINARIES_STRAIGHT, &o.ordinary).is_none()
        {
            out.push(fault(
                format!("/ordinaries/{i}/ordinary"),
                format!("{:?} is not an ordinary", o.ordinary),
            ));
        }
        if !is_tincture(&o.t) {
            out.push(fault(
                format!("/ordinaries/{i}/t"),
                format!("{:?} is not a tincture", o.t),
            ));
        }
        if !line_ok(o.line.as_deref()) {
            out.push(fault(
                format!("/ordinaries/{i}/line"),
                "the line is not a line of partition",
            ));
        }
    }
    for (i, c) in coa.charges.iter().enumerate() {
        if !has_art(&c.charge) {
            out.push(fault(
                format!("/charges/{i}/charge"),
                format!("{:?} is not a charge that inkland can draw", c.charge),
            ));
        }
        for (key, t) in [
            ("t", Some(&c.t)),
            ("t2", c.t2.as_ref()),
            ("t3", c.t3.as_ref()),
        ] {
            if let Some(t) = t
                && !is_tincture(t)
            {
                out.push(fault(
                    format!("/charges/{i}/{key}"),
                    format!("{t:?} is not a tincture"),
                ));
            }
        }
        if c.p.is_empty() || !c.p.chars().all(|ch| POSITIONS.contains(ch)) {
            out.push(fault(
                format!("/charges/{i}/p"),
                format!("{:?} is not a list of positions ({POSITIONS})", c.p),
            ));
        }
    }
    out
}

/// The position letters of a charge.
pub const POSITIONS: &str = "abcdefghijklmnopqyzABCDEFGHIJKL";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::{Layer, SeedTree};

    fn rng(seed: u64) -> Rand {
        Rand::new(SeedTree::new(seed).part_rng(Layer::States, "emblems"))
    }

    fn many(n: u64) -> Vec<Coa> {
        let mut r = rng(7);
        let mut g = Generator::new(&mut r);
        let none = Parent {
            coa: None,
            kinship: 0.0,
            dominion: 0.0,
        };
        (0..n).map(|_| g.generate(&none, Some("Naval"))).collect()
    }

    #[test]
    fn each_generated_coat_of_arms_is_valid() {
        for coa in many(2000) {
            assert_eq!(validate(&coa), Vec::new(), "{coa:?}");
        }
    }

    #[test]
    fn the_rule_of_tincture_holds_for_plain_charges() {
        // A plain charge on a plain field: a metal on a colour, or a colour
        // on a metal (FMG follows the field type).
        let mut checked = 0;
        for coa in many(2000) {
            if coa.t1.contains('-') || coa.division.is_some() || !coa.ordinaries.is_empty() {
                continue;
            }
            // The first charge is on the field; a later one can be on an
            // inescutcheon.
            for c in coa.charges.iter().take(1) {
                let natural = charge_data(&c.charge).and_then(|d| d.natural).is_some();
                if c.charge == "inescutcheon" || natural || c.size == Some(0.5) {
                    continue;
                }
                let (a, b) = (plain_type(&coa.t1), plain_type(&c.t));
                if a == Some("stains") || b == Some("stains") {
                    continue;
                }
                assert_ne!(a, b, "{coa:?}");
                checked += 1;
            }
        }
        assert!(checked > 100);
    }

    #[test]
    fn the_same_seed_gives_the_same_arms() {
        assert_eq!(many(50), many(50));
    }

    #[test]
    fn a_child_inherits_from_its_parent() {
        let mut r = rng(3);
        let mut g = Generator::new(&mut r);
        let none = Parent {
            coa: None,
            kinship: 0.0,
            dominion: 0.0,
        };
        let parent = g.generate(&none, None);
        let close = Parent {
            coa: Some(&parent),
            kinship: 1.0,
            dominion: 0.0,
        };
        for _ in 0..20 {
            assert_eq!(g.generate(&close, None).t1, parent.t1);
        }
        // A dominion has a canton with the charge of the parent.
        let mut parent = parent;
        if parent.charges.is_empty() {
            parent.charges.push(Charge::new("mullet", "or", "e"));
        }
        let dominion = Parent {
            coa: Some(&parent),
            kinship: 0.0,
            dominion: 1.0,
        };
        let child = g.generate(&dominion, None);
        assert_eq!(child.ordinaries.last().unwrap().ordinary, "canton");
        let last = child.charges.last().unwrap();
        assert_eq!((last.p.as_str(), last.size), ("y", Some(0.5)));
    }

    #[test]
    fn the_generator_draws_only_charges_with_art() {
        for coa in many(1000) {
            for c in &coa.charges {
                assert!(has_art(&c.charge), "{}", c.charge);
            }
            if let Some(semy) = coa.t1.strip_prefix("semy_of_") {
                assert!(has_art(semy.split('-').next().unwrap()));
            }
        }
    }

    #[test]
    fn json_has_the_shape_of_fmg() {
        let coa: Coa = serde_json::from_str(
            r#"{"t1":"azure","division":{"division":"perPale","t":"or","line":"wavy"},
                "ordinaries":[{"ordinary":"bend","t":"gules"}],
                "charges":[{"charge":"mullet","t":"argent","p":"jln","size":0.7,"sinister":1,"reversed":true}],
                "shield":"heater"}"#,
        )
        .unwrap();
        assert!(coa.charges[0].sinister && coa.charges[0].reversed);
        assert_eq!(validate(&coa), Vec::new());
        let text = serde_json::to_string(&coa).unwrap();
        assert!(text.contains(r#""sinister":1"#), "{text}");
        assert!(serde_json::from_str::<Coa>(r#"{"t1":"azure","icon":"x"}"#).is_err());
        let bad: Coa = serde_json::from_str(
            r#"{"t1":"blue","charges":[{"charge":"lionRampant","t":"or","p":"e!"}],"shield":"kite2"}"#,
        )
        .unwrap();
        let paths: Vec<String> = validate(&bad).into_iter().map(|f| f.path).collect();
        assert_eq!(
            paths,
            ["/t1", "/shield", "/charges/0/charge", "/charges/0/p"]
        );
    }

    #[test]
    fn sizes_follow_fmg() {
        assert_eq!(size_of("e", None, None), 1.5);
        assert_eq!(size_of("e", Some("orle"), None), 1.1);
        assert_eq!(size_of("ABCDEFGHIJKL", None, None), 0.18);
        assert_eq!(size_of("jlmo", None, Some("perCross")), 0.6);
        assert_eq!(size_of("kn", None, None), 0.7);
    }
}
