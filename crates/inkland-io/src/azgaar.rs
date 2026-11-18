//! Import a map of Azgaar's Fantasy Map Generator (FMG): a `.map` file.
//!
//! A `.map` file is text. Its records are separated by CRLF. The file can
//! also be gzip data or base64 text. The import reads these records:
//!
//! - record 0: the version, the seed, and the map size in pixels,
//! - record 1: the settings, with the distance unit and the distance scale
//!   (units for each pixel),
//! - record 6: the grid, with its lattice size `cellsX` x `cellsY`,
//! - record 7: the height of each grid cell, row by row from the top,
//! - record 14: the states,
//! - record 15: the burgs (the places).
//!
//! The import makes a config with a raster template and a point pin for each
//! burg. It does not make pinned rivers, regions, or roads. FMG writes the
//! records in `src/services/io/save.ts` and reads them in
//! `src/services/io/load.ts`.

use inkland_core::config::{Config, PlaceKind, PointPin, Raster, Template, Towns, Units, World};
use inkland_core::error::{Error, ErrorCode};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Read;

/// The oldest map version that the import reads.
pub const MIN_VERSION: [u32; 3] = [1, 100, 0];
/// The newest map version that the import reads, with any patch number.
pub const MAX_VERSION: [u32; 2] = [1, 158];

const RECORD_PARAMS: usize = 0;
const RECORD_SETTINGS: usize = 1;
const RECORD_GRID: usize = 6;
const RECORD_HEIGHTS: usize = 7;
const RECORD_STATES: usize = 14;
const RECORD_BURGS: usize = 15;

/// The facts of one import, for the summary of `inkland import`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportSummary {
    /// The version of the map, for example `1.154.0`.
    pub version: String,
    /// The seed of the config.
    pub seed: u64,
    /// The map size in pixels, `[width, height]`.
    pub size_px: [f64; 2],
    /// The distance unit of the map, and of the config.
    pub unit: String,
    /// Units for each pixel.
    pub scale: f64,
    /// The size of the raster, `[cols, rows]`.
    pub raster: [u32; 2],
    /// The number of point pins: one for each burg that the import kept.
    pub places: usize,
    /// The number of capitals among the places.
    pub capitals: usize,
    /// The number of ports among the places.
    pub ports: usize,
    /// The number of states that have a pinned capital.
    pub states: usize,
    /// The burgs that the import did not keep, with the reason.
    pub skipped: Skipped,
}

/// Counts of the burgs that the import did not keep.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Skipped {
    /// Burgs with the flag `removed`, and empty entries.
    pub removed: usize,
    /// Burgs without a position.
    pub no_position: usize,
    /// Burgs outside the frame.
    pub outside_frame: usize,
    /// Burgs at the same position as a burg before them.
    pub same_position: usize,
}

/// A config and the facts of its import.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub config: Config,
    pub summary: ImportSummary,
}

/// Make a config from the bytes of a `.map` file.
pub fn import_map(bytes: &[u8]) -> Result<Config, Error> {
    read_map(bytes).map(|i| i.config)
}

/// Make a config from the bytes of a `.map` file, and give the facts of the
/// import.
pub fn read_map(bytes: &[u8]) -> Result<Import, Error> {
    let text = decode(bytes)?;
    let records = split_records(&text);
    let params: Vec<&str> = record(&records, RECORD_PARAMS)?.split('|').collect();
    let version = params[0].trim();
    check_version(version)?;

    let seed_text = params.get(3).copied().unwrap_or("").trim();
    let seed = parse_seed(seed_text);
    let width_px = positive(params.get(4).copied(), RECORD_PARAMS, "the map width")?;
    let height_px = positive(params.get(5).copied(), RECORD_PARAMS, "the map height")?;
    let (unit, scale) = distance(record(&records, RECORD_SETTINGS)?)?;

    let grid = json(&records, RECORD_GRID)?;
    let count = |key: &str| -> Result<u32, Error> {
        grid.get(key)
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .filter(|&v| v >= 2)
            .ok_or_else(|| bad(RECORD_GRID, format!("the grid has no valid {key}")))
    };
    let (cols, rows) = (count("cellsX")?, count("cellsY")?);
    let values = heights(record(&records, RECORD_HEIGHTS)?)?;
    let size = u64::from(cols) * u64::from(rows);
    if values.len() as u64 != size {
        return Err(bad(
            RECORD_HEIGHTS,
            format!(
                "the grid is {cols} x {rows} = {size} cells, but the heights record has {} values",
                values.len()
            ),
        ));
    }

    let states = state_names(&json(&records, RECORD_STATES)?)?;
    let (width, height) = (width_px * scale, height_px * scale);
    let (points, skipped) = burgs(
        &json(&records, RECORD_BURGS)?,
        &states,
        scale,
        [width, height],
    )?;

    let capital_states: BTreeSet<&str> = points
        .iter()
        .filter_map(|p| p.capital_of.as_deref())
        .collect();
    let summary = ImportSummary {
        version: version.to_owned(),
        seed,
        size_px: [width_px, height_px],
        unit: unit.clone(),
        scale,
        raster: [cols, rows],
        places: points.len(),
        capitals: points.iter().filter(|p| p.kind == PlaceKind::City).count(),
        ports: points.iter().filter(|p| p.port).count(),
        states: capital_states.len(),
        skipped,
    };
    let world = World {
        cells: (cols * rows).clamp(1000, 100_000),
        template: Template::Raster {
            raster: Raster { cols, rows, values },
        },
        states: (capital_states.len() as u32).max(1),
        towns: Towns::Count(0),
        ..World::default()
    };
    let mut config: Config = serde_json::from_value(serde_json::json!({
        "version": inkland_core::config::CONFIG_VERSION,
        "seed": seed,
        "frame": { "width": width, "height": height },
    }))
    .expect("a minimal config always converts");
    config.units = Units {
        name: unit,
        per_cell_hint: None,
    };
    config.world = world;
    config.pins.points = points;
    Ok(Import { config, summary })
}

fn bad(record: usize, message: impl Into<String>) -> Error {
    Error::new(ErrorCode::MapParse, "", message).with("record", record)
}

/// The text of the file: plain text, gzip data, or base64 text.
fn decode(bytes: &[u8]) -> Result<String, Error> {
    let fail = |m: String| Error::new(ErrorCode::MapParse, "", m);
    let mut gunzipped = Vec::new();
    let bytes = if bytes.starts_with(&[0x1f, 0x8b]) {
        flate2::read::MultiGzDecoder::new(bytes)
            .read_to_end(&mut gunzipped)
            .map_err(|e| fail(format!("the gzip data cannot be read: {e}")))?;
        &gunzipped[..]
    } else {
        bytes
    };
    let text = std::str::from_utf8(bytes)
        .map_err(|_| fail("the map file is not UTF-8 text, gzip data, or base64 text".into()))?;
    if is_delimited(text) {
        return Ok(text.to_owned());
    }
    // FMG reads base64 with `decodeURIComponent(atob(text))`.
    let raw = base64(text)
        .ok_or_else(|| fail("the map file is not FMG text, gzip data, or base64 text".into()))?;
    let text = percent_decode(&raw)
        .and_then(|b| String::from_utf8(b).ok())
        .ok_or_else(|| fail("the base64 text does not hold FMG text".into()))?;
    if !is_delimited(&text) {
        return Err(fail("the base64 text does not hold FMG text".into()));
    }
    Ok(text)
}

/// FMG text has a `|` in its first 10 characters.
fn is_delimited(text: &str) -> bool {
    text.chars().take(10).any(|c| c == '|')
}

fn base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for b in text.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b' ' | b'\t' | b'\r' | b'\n' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn percent_decode(raw: &[u8]) -> Option<Vec<u8>> {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            let hi = hex(*raw.get(i + 1)?)?;
            let lo = hex(*raw.get(i + 2)?)?;
            out.push(hi * 16 + lo);
            i += 3;
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    Some(out)
}

/// Split the text into records. A CRLF inside the SVG record of the map is
/// not a record separator, as in FMG.
fn split_records(text: &str) -> Vec<String> {
    let mut text = text.to_owned();
    if let Some((start, end)) = map_svg(&text) {
        let svg = text[start..end].replace("\r\n", "\n");
        text.replace_range(start..end, &svg);
    }
    text.split("\r\n").map(str::to_owned).collect()
}

/// The byte range of the first `<svg ...>` element with `id="map"`.
fn map_svg(text: &str) -> Option<(usize, usize)> {
    let mut from = 0;
    while let Some(i) = text[from..].find("<svg") {
        let start = from + i;
        let tag_end = start + text[start..].find('>')?;
        if text[start..tag_end].contains("id=\"map\"") {
            let end = start + text[start..].find("</svg>")? + "</svg>".len();
            return Some((start, end));
        }
        from = tag_end;
    }
    None
}

fn record(records: &[String], n: usize) -> Result<&str, Error> {
    records.get(n).map(String::as_str).ok_or_else(|| {
        bad(
            n,
            format!(
                "the map file has {} records; record {n} is missing",
                records.len()
            ),
        )
    })
}

fn json(records: &[String], n: usize) -> Result<Value, Error> {
    serde_json::from_str(record(records, n)?)
        .map_err(|e| bad(n, format!("record {n} is not valid JSON: {e}")))
}

/// The version must be `MAJOR.MINOR.PATCH`, from `MIN_VERSION` to
/// `MAX_VERSION`.
fn check_version(version: &str) -> Result<(), Error> {
    let supported = format!(
        "{}.{}.{} to {}.{}.x",
        MIN_VERSION[0], MIN_VERSION[1], MIN_VERSION[2], MAX_VERSION[0], MAX_VERSION[1]
    );
    let refuse = || {
        Error::new(
            ErrorCode::MapVersion,
            "",
            format!(
                "the import cannot read a map of version {version:?}; it reads versions {supported}"
            ),
        )
        .with("found", version)
        .with("supported", supported.clone())
    };
    let parts: Vec<u32> = version
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| refuse())?;
    let [major, minor, patch] = parts[..] else {
        return Err(refuse());
    };
    if [major, minor, patch] < MIN_VERSION || [major, minor] > MAX_VERSION {
        return Err(refuse());
    }
    Ok(())
}

/// A number seed is the seed. FMG can also have a text seed: then the seed
/// is the FNV-1a hash of the text.
fn parse_seed(text: &str) -> u64 {
    text.parse().unwrap_or_else(|_| {
        text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    })
}

fn positive(text: Option<&str>, n: usize, what: &str) -> Result<f64, Error> {
    text.and_then(|t| t.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or_else(|| bad(n, format!("{what} is not a number more than 0")))
}

/// The distance unit and the units for each pixel. The settings record is
/// JSON from version 1.152.0. Before it, the record is text with `|`
/// between the values, and the first two values are the unit and the scale.
/// A missing value gets the FMG default: `km` and 3.
fn distance(settings: &str) -> Result<(String, f64), Error> {
    let n = RECORD_SETTINGS;
    let (unit, scale) = if settings.trim_start().starts_with('{') {
        let v: Value = serde_json::from_str(settings)
            .map_err(|e| bad(n, format!("the settings are not valid JSON: {e}")))?;
        let d = &v["units"]["distance"];
        let scale = match &d["scale"] {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        };
        (d["unit"].as_str().map(str::to_owned), scale)
    } else {
        let parts: Vec<&str> = settings.split('|').collect();
        let get = |i: usize| {
            parts
                .get(i)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        (get(0), get(1))
    };
    let unit = unit
        .map(|u| u.trim().to_owned())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "km".to_owned());
    let scale = match scale {
        Some(s) => positive(Some(&s), n, "the distance scale")?,
        None => 3.0,
    };
    Ok((unit, scale))
}

fn heights(text: &str) -> Result<Vec<u8>, Error> {
    text.split(',')
        .enumerate()
        .map(|(i, t)| {
            t.trim()
                .parse::<u8>()
                .ok()
                .filter(|&h| h <= 100)
                .ok_or_else(|| {
                    bad(
                        RECORD_HEIGHTS,
                        format!("height {i} is {t:?}, not a whole number from 0 to 100"),
                    )
                })
        })
        .collect()
}

/// The name of each state, at the index of the state. The entry is `None`
/// for state 0 (no state), for a removed state, and for a state without a
/// name.
fn state_names(states: &Value) -> Result<Vec<Option<String>>, Error> {
    let list = states
        .as_array()
        .ok_or_else(|| bad(RECORD_STATES, "the states record is not a list"))?;
    Ok(list
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 0 || is_removed(s) {
                return None;
            }
            s.get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_owned)
        })
        .collect())
}

/// `true` for an entry that is not an object, an empty object, or an object
/// with the flag `removed`.
fn is_removed(v: &Value) -> bool {
    match v.as_object() {
        Some(o) => o.is_empty() || truthy(o.get("removed")),
        None => true,
    }
}

/// A JavaScript truth value: `true`, or a number that is not 0.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        _ => false,
    }
}

/// Quantize a coordinate to 3 decimals, so that the float noise of the
/// scale does not reach the config.
fn quantize(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// A point pin for each burg that the import keeps.
fn burgs(
    burgs: &Value,
    states: &[Option<String>],
    scale: f64,
    [width, height]: [f64; 2],
) -> Result<(Vec<PointPin>, Skipped), Error> {
    let list = burgs
        .as_array()
        .ok_or_else(|| bad(RECORD_BURGS, "the burgs record is not a list"))?;
    let mut skipped = Skipped::default();
    let mut ids: BTreeSet<String> = BTreeSet::new();
    let mut positions: BTreeSet<(u64, u64)> = BTreeSet::new();
    let mut points = Vec::new();
    for (i, b) in list.iter().enumerate() {
        if is_removed(b) {
            skipped.removed += 1;
            continue;
        }
        let coord = |key: &str| b.get(key).and_then(Value::as_f64).filter(|v| v.is_finite());
        let (Some(x), Some(y)) = (coord("x"), coord("y")) else {
            skipped.no_position += 1;
            continue;
        };
        let at = [quantize(x * scale), quantize(y * scale)];
        if !(0.0..=width).contains(&at[0]) || !(0.0..=height).contains(&at[1]) {
            skipped.outside_frame += 1;
            continue;
        }
        if !positions.insert((at[0].to_bits(), at[1].to_bits())) {
            skipped.same_position += 1;
            continue;
        }
        let name = b
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned);
        let capital = truthy(b.get("capital"));
        let state = b
            .get("state")
            .and_then(Value::as_u64)
            .and_then(|s| states.get(s as usize))
            .and_then(Clone::clone);
        let mut id = name.as_deref().map(slug).unwrap_or_default();
        if id.is_empty() {
            id = format!("burg-{i}");
        }
        let id = unique(id, &mut ids);
        points.push(PointPin {
            id,
            kind: if capital {
                PlaceKind::City
            } else {
                PlaceKind::Town
            },
            at,
            name,
            port: truthy(b.get("port")),
            capital_of: if capital { state } else { None },
            custom_kind: None,
            coa: None,
            state_coa: None,
        });
    }
    Ok((points, skipped))
}

/// Add `-2`, `-3`, ... to an id that is already in `ids`.
fn unique(id: String, ids: &mut BTreeSet<String>) -> String {
    let mut candidate = id.clone();
    let mut n = 2;
    while ids.contains(&candidate) {
        candidate = format!("{id}-{n}");
        n += 1;
    }
    ids.insert(candidate.clone());
    candidate
}

/// An id from a name: lower-case ASCII letters and digits, words joined by
/// `-`. Latin letters with a mark lose the mark. Other characters separate
/// words.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in name.chars().flat_map(char::to_lowercase) {
        let folded: Option<&str> = if c.is_ascii_alphanumeric() {
            None
        } else {
            fold(c)
        };
        let piece: String = match folded {
            Some(s) => s.to_owned(),
            None if c.is_ascii_alphanumeric() => c.to_string(),
            None => {
                gap = true;
                continue;
            }
        };
        if gap && !out.is_empty() {
            out.push('-');
        }
        gap = false;
        out.push_str(&piece);
    }
    out
}

/// The ASCII letters of a lower-case Latin letter with a mark.
fn fold(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'œ' => "oe",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => "s",
        'ß' => "ss",
        'ţ' | 'ť' | 'ŧ' | 'ț' => "t",
        'þ' => "th",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    /// A small map file in the record layout of FMG. Records that the
    /// import does not read are empty.
    fn map_text(version: &str, settings: &str, burgs: Value) -> String {
        let mut records = vec![String::new(); 20];
        records[0] = format!("{version}|File can be loaded|2026-1-1|4242|300|200|1");
        records[1] = settings.to_owned();
        records[5] = "<svg id=\"map\" width=\"300\">\r\n<g></g>\r\n</svg>".to_owned();
        records[6] = json!({ "spacing": 100, "cellsX": 3, "cellsY": 2 }).to_string();
        records[7] = "5,30,40,10,50,100".to_owned();
        records[14] = json!([
            { "i": 0, "name": "Neutrals" },
            { "i": 1, "name": "Ostria" },
            { "i": 2, "name": "Gone", "removed": true }
        ])
        .to_string();
        records[15] = burgs.to_string();
        records.join("\r\n")
    }

    fn sample_burgs() -> Value {
        json!([
            0,
            { "i": 1, "name": "Ostrey", "x": 150.5, "y": 50.25, "capital": 1, "port": 3, "state": 1 },
            { "i": 2, "name": "Harrow Gate", "x": 10, "y": 190, "capital": 0, "port": 0, "state": 1 },
            { "i": 3, "name": "Old", "x": 20, "y": 20, "removed": true },
            { "i": 4, "name": "Ostrey", "x": 200, "y": 100, "state": 1 },
            { "i": 5, "name": "Far", "x": 400, "y": 100, "state": 1 },
            { "i": 6, "name": "Twin", "x": 10, "y": 190, "state": 1 },
            { "i": 7, "name": "Ærø Öster", "x": 250, "y": 150, "capital": true, "state": 2 },
            { "i": 8, "name": "", "x": 260, "y": 160 },
            {}
        ])
    }

    fn sample() -> String {
        map_text(
            "1.154.0",
            r#"{"units":{"distance":{"unit":"mi","scale":2}}}"#,
            sample_burgs(),
        )
    }

    #[test]
    fn a_map_becomes_a_config_with_a_raster_and_pins() {
        let i = read_map(sample().as_bytes()).unwrap();
        let c = &i.config;
        assert_eq!(c.seed, 4242);
        assert_eq!(c.units.name, "mi");
        assert_eq!((c.frame.width, c.frame.height), (600.0, 400.0));
        assert_eq!(c.world.cells, 1000);
        assert_eq!(c.world.towns, Towns::Count(0));
        assert_eq!(c.world.states, 1);
        let Template::Raster { raster } = &c.world.template else {
            panic!("no raster");
        };
        assert_eq!((raster.cols, raster.rows), (3, 2));
        assert_eq!(raster.values, [5, 30, 40, 10, 50, 100]);
        assert!(c.pins.regions.is_empty() && c.pins.rivers.is_empty());

        let ids: Vec<&str> = c.pins.points.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            ["ostrey", "harrow-gate", "ostrey-2", "aero-oster", "burg-8"]
        );
        let ostrey = &c.pins.points[0];
        assert_eq!(ostrey.kind, PlaceKind::City);
        assert_eq!(ostrey.at, [301.0, 100.5]);
        assert!(ostrey.port);
        assert_eq!(ostrey.capital_of.as_deref(), Some("Ostria"));
        assert_eq!(ostrey.name.as_deref(), Some("Ostrey"));
        let harrow = &c.pins.points[1];
        assert_eq!(harrow.kind, PlaceKind::Town);
        assert!(!harrow.port);
        assert_eq!(harrow.capital_of, None);
        // A capital of a removed state is a city without a state.
        assert_eq!(c.pins.points[3].kind, PlaceKind::City);
        assert_eq!(c.pins.points[3].capital_of, None);
        assert_eq!(c.pins.points[4].name, None);

        let s = &i.summary;
        assert_eq!(s.version, "1.154.0");
        assert_eq!(s.places, 5);
        assert_eq!(s.capitals, 2);
        assert_eq!(s.ports, 1);
        assert_eq!(s.states, 1);
        assert_eq!(
            s.skipped,
            Skipped {
                removed: 3,
                no_position: 0,
                outside_frame: 1,
                same_position: 1
            }
        );
    }

    #[test]
    fn the_pipe_settings_of_older_maps_give_the_unit_and_the_scale() {
        let text = map_text("1.112.1", "km|1.5|square|m|2|°C||", sample_burgs());
        let c = import_map(text.as_bytes()).unwrap();
        assert_eq!(c.units.name, "km");
        assert_eq!((c.frame.width, c.frame.height), (450.0, 300.0));
        // Missing values get the FMG defaults.
        let text = map_text("1.112.1", "||square", sample_burgs());
        let c = import_map(text.as_bytes()).unwrap();
        assert_eq!(c.units.name, "km");
        assert_eq!(c.frame.width, 900.0);
    }

    #[test]
    fn gzip_and_base64_files_give_the_same_config() {
        let text = sample();
        let plain = import_map(text.as_bytes()).unwrap();

        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(text.as_bytes()).unwrap();
        let gz = gz.finish().unwrap();
        assert_eq!(import_map(&gz).unwrap(), plain);

        // FMG writes base64 as btoa(encodeURIComponent(text)).
        let mut escaped = String::new();
        for b in text.bytes() {
            if b.is_ascii_alphanumeric() {
                escaped.push(b as char);
            } else {
                escaped.push_str(&format!("%{b:02X}"));
            }
        }
        let b64 = encode64(escaped.as_bytes());
        assert_eq!(import_map(b64.as_bytes()).unwrap(), plain);
    }

    fn encode64(bytes: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk.len();
            let v = (u32::from(chunk[0]) << 16)
                | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
                | u32::from(*chunk.get(2).unwrap_or(&0));
            for k in 0..4 {
                if k <= n {
                    out.push(T[((v >> (18 - 6 * k)) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    #[test]
    fn versions_outside_the_range_are_refused() {
        for v in ["1.99.9", "1.159.0", "2.0.0", "1.73", "abc", ""] {
            let e = import_map(map_text(v, "", sample_burgs()).as_bytes()).unwrap_err();
            assert_eq!(e.code, ErrorCode::MapVersion, "{v}");
            assert_eq!(e.details["found"], json!(v));
        }
        for v in ["1.100.0", "1.158.7"] {
            assert!(
                import_map(map_text(v, "", sample_burgs()).as_bytes()).is_ok(),
                "{v}"
            );
        }
    }

    #[test]
    fn a_file_that_is_not_a_map_is_refused() {
        for bytes in [
            &b"hello world, no map"[..],
            &[0xff, 0xfe, 0x00][..],
            &[0x1f, 0x8b, 1, 2][..],
        ] {
            let e = import_map(bytes).unwrap_err();
            assert_eq!(e.code, ErrorCode::MapParse);
        }
    }

    #[test]
    fn a_bad_record_names_its_number() {
        let text = sample().replace("5,30,40,10,50,100", "5,30,40,10,50");
        let e = import_map(text.as_bytes()).unwrap_err();
        assert_eq!(e.code, ErrorCode::MapParse);
        assert_eq!(e.details["record"], json!(7));
        let text = sample().replace("5,30,40,10,50,100", "5,30,40,10,50,101");
        assert_eq!(
            import_map(text.as_bytes()).unwrap_err().details["record"],
            json!(7)
        );
        let short: String = sample()
            .split("\r\n")
            .take(10)
            .collect::<Vec<_>>()
            .join("\r\n");
        let e = import_map(short.as_bytes()).unwrap_err();
        assert_eq!(e.details["record"], json!(14));
    }

    #[test]
    fn a_text_seed_gets_a_stable_number() {
        let text = sample().replace("|4242|", "|dragon|");
        let a = import_map(text.as_bytes()).unwrap().seed;
        assert_eq!(a, import_map(text.as_bytes()).unwrap().seed);
        assert_ne!(a, parse_seed("dragons"));
    }

    #[test]
    fn slugs_match_the_id_pattern() {
        assert_eq!(slug("Harrow Gate"), "harrow-gate");
        assert_eq!(slug("  St. Ælfric's  "), "st-aelfric-s");
        assert_eq!(slug("Ōsaka-ken"), "osaka-ken");
        assert_eq!(slug("北京"), "");
        assert_eq!(slug("Città 2"), "citta-2");
    }
}
