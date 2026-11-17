//! Where the map is on the globe: its size as a part of the world, and its
//! latitude and longitude box.
//!
//! This follows Azgaar's Fantasy Map Generator (FMG,
//! `model/physical/coordinates.ts`). The config gives the latitude range
//! (`world.climate.latitude_range`), or a geography: the map size in percent
//! of the world and the position of the map. A value of the geography that
//! the config does not give comes from a random draw, as in FMG.

use crate::config::{Config, Geography, Template, TemplateName};
use crate::rand::Rand;

/// The latitude and longitude box of a map, in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinates {
    /// The map size in percent of the world, 1 to 100.
    pub map_size: f64,
    /// The north-south position in percent: 0 puts the north edge at the
    /// north pole, 100 puts the south edge at the south pole.
    pub latitude: f64,
    /// The west-east position in percent.
    pub longitude: f64,
    /// The latitude span, and the latitudes of the north and south edges.
    pub lat_t: f64,
    pub lat_n: f64,
    pub lat_s: f64,
    /// The longitude span, and the longitudes of the west and east edges.
    pub lon_t: f64,
    pub lon_w: f64,
    pub lon_e: f64,
}

/// Round to `d` decimals, as FMG `rn`.
fn rn(v: f64, d: i32) -> f64 {
    let m = libm::pow(10.0, f64::from(d));
    (v * m).round() / m
}

impl Coordinates {
    /// The box of a map size and position on a `width` x `height` frame
    /// (FMG `Coordinates.calculate`).
    pub fn calculate(
        map_size: f64,
        latitude: f64,
        longitude: f64,
        width: f64,
        height: f64,
    ) -> Self {
        let lat_t = rn(map_size / 100.0 * 180.0, 1);
        let lat_n = rn(90.0 - (180.0 - lat_t) * latitude / 100.0, 1);
        let lat_s = rn(lat_n - lat_t, 1);
        let lon_t = rn((width / height * lat_t).min(360.0), 1);
        let lon_e = rn(180.0 - (360.0 - lon_t) * longitude / 100.0, 1);
        let lon_w = rn(lon_e - lon_t, 1);
        Coordinates {
            map_size,
            latitude,
            longitude,
            lat_t,
            lat_n,
            lat_s,
            lon_t,
            lon_w,
            lon_e,
        }
    }

    /// The box of a latitude range `[south, north]`. The map is in the
    /// middle of the longitudes.
    pub fn of_range([south, north]: [f64; 2], width: f64, height: f64) -> Self {
        let lat_t = north - south;
        let lon_t = (width / height * lat_t).min(360.0);
        let map_size = lat_t / 180.0 * 100.0;
        let latitude = if lat_t >= 180.0 {
            50.0
        } else {
            (90.0 - north) / (180.0 - lat_t) * 100.0
        };
        Coordinates {
            map_size,
            latitude,
            longitude: 50.0,
            lat_t,
            lat_n: north,
            lat_s: south,
            lon_t,
            lon_w: -lon_t / 2.0,
            lon_e: lon_t / 2.0,
        }
    }

    /// The latitude range `[south, north]`.
    pub fn latitude_range(&self) -> [f64; 2] {
        [self.lat_s, self.lat_n]
    }
}

/// The chance that a template covers the whole world when its land does not
/// touch the frame edge (FMG `WHOLE_WORLD_CHANCE`).
fn whole_world_chance(t: Option<TemplateName>) -> f64 {
    match t {
        Some(TemplateName::Pangea) => 1.0,
        Some(TemplateName::Shattered) => 0.7,
        Some(TemplateName::Continents) => 0.5,
        Some(TemplateName::Archipelago) => 0.35,
        Some(TemplateName::HighIsland) => 0.25,
        Some(TemplateName::LowIsland) => 0.1,
        _ => 0.0,
    }
}

/// The size draw of a template: expected value, deviation, least, most
/// (FMG `RANDOM_SIZE`).
fn random_size(t: Option<TemplateName>, max_size: f64) -> [f64; 4] {
    match t {
        Some(TemplateName::Pangea) => [70.0, 20.0, 30.0, 100.0],
        Some(TemplateName::Volcano) => [20.0, 20.0, 10.0, 100.0],
        Some(TemplateName::Mediterranean) => [25.0, 30.0, 15.0, 80.0],
        Some(TemplateName::Peninsula) => [15.0, 15.0, 5.0, 80.0],
        Some(TemplateName::Isthmus) => [15.0, 20.0, 3.0, 80.0],
        Some(TemplateName::Atoll) => [3.0, 2.0, 1.0, 5.0],
        _ => [30.0, 20.0, 15.0, max_size],
    }
}

/// A normal draw, clamped to `[min, max]` and rounded to `round` decimals
/// (FMG `gauss`). The draw uses the polar method.
pub fn gauss(rng: &mut Rand, expected: f64, deviation: f64, min: f64, max: f64, round: i32) -> f64 {
    let z = loop {
        let x = 2.0 * rng.unit() - 1.0;
        let y = 2.0 * rng.unit() - 1.0;
        let r = x * x + y * y;
        if r > 0.0 && r < 1.0 {
            break y * libm::sqrt(-2.0 * libm::log(r) / r);
        }
    };
    rn((expected + deviation * z).clamp(min, max), round)
}

/// The coordinates of a world. `partial` is `true` when a land feature
/// touches the frame edge.
pub fn coordinates(config: &Config, partial: bool, rng: &mut Rand) -> Coordinates {
    let (w, h) = (config.frame.width, config.frame.height);
    let Some(geo) = &config.world.climate.geography else {
        return Coordinates::of_range(config.world.climate.latitude_range, w, h);
    };
    let [size, latitude, longitude] = draw(config, geo, partial, rng);
    Coordinates::calculate(size, latitude, longitude, w, h)
}

/// The size and the position: the config values, else the draw of FMG
/// `getSizeAndPosition`. The draw happens in full, so a config value does
/// not change the other values.
fn draw(config: &Config, geo: &Geography, partial: bool, rng: &mut Rand) -> [f64; 3] {
    let template = match config.world.template {
        Template::Named(t) => Some(t),
        _ => None,
    };
    let chance = whole_world_chance(template);
    let drawn = if !partial && chance > 0.0 && (chance >= 1.0 || rng.chance(chance)) {
        [100.0, 50.0, 50.0]
    } else {
        let max_size = if partial { 80.0 } else { 100.0 };
        let [expected, deviation, min, max] = random_size(template, max_size);
        let round = if template == Some(TemplateName::Atoll) {
            1
        } else {
            0
        };
        let size = gauss(rng, expected, deviation, min, max.min(max_size), round);
        let mean = if rng.chance(0.5) { 40.0 } else { 60.0 };
        let latitude = gauss(rng, mean, 20.0, 25.0, 75.0, 0);
        [size, latitude, 50.0]
    };
    [
        geo.map_size.unwrap_or(drawn[0]),
        geo.latitude.unwrap_or(drawn[1]),
        geo.longitude.unwrap_or(drawn[2]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::{Layer, SeedTree};
    use serde_json::json;

    fn config(climate: serde_json::Value, template: &str) -> Config {
        serde_json::from_value(json!({
            "version": 1, "seed": 1,
            "frame": { "width": 400, "height": 300 },
            "world": { "template": template, "climate": climate }
        }))
        .unwrap()
    }

    fn rng() -> Rand {
        Rand::new(SeedTree::new(1).part_rng(Layer::Climate, "coordinates"))
    }

    #[test]
    fn the_box_follows_the_fmg_formula() {
        // 20 % of the world, at 30 % from the north pole.
        let c = Coordinates::calculate(20.0, 30.0, 50.0, 400.0, 300.0);
        assert_eq!(c.lat_t, 36.0);
        assert_eq!(c.lat_n, 46.8);
        assert_eq!(c.lat_s, rn(c.lat_n - 36.0, 1));
        assert_eq!(c.lon_t, 48.0);
        assert_eq!(c.lon_e, 24.0);
        assert_eq!(c.lon_w, -24.0);
        let whole = Coordinates::calculate(100.0, 50.0, 50.0, 400.0, 300.0);
        assert_eq!(whole.latitude_range(), [-90.0, 90.0]);
        assert_eq!(whole.lon_t, 240.0);
    }

    #[test]
    fn without_a_geography_the_latitude_range_is_kept() {
        let c = config(json!({ "latitude_range": [30, 55] }), "continents");
        let k = coordinates(&c, true, &mut rng());
        assert_eq!(k.latitude_range(), [30.0, 55.0]);
        assert_eq!(k.lat_t, 25.0);
    }

    #[test]
    fn a_geography_sets_the_latitudes() {
        let c = config(
            json!({ "geography": { "map_size": 20, "latitude": 30 } }),
            "continents",
        );
        let k = coordinates(&c, true, &mut rng());
        assert_eq!(k.latitude_range(), [rn(46.8 - 36.0, 1), 46.8]);
    }

    #[test]
    fn a_drawn_geography_stays_in_the_fmg_limits() {
        for seed in 0..200 {
            let mut r = Rand::new(SeedTree::new(seed).part_rng(Layer::Climate, "coordinates"));
            let c = config(json!({ "geography": {} }), "volcano");
            let k = coordinates(&c, true, &mut r);
            assert!((10.0..=80.0).contains(&k.map_size), "{}", k.map_size);
            assert!((25.0..=75.0).contains(&k.latitude), "{}", k.latitude);
            assert!(k.lat_s >= -90.0 && k.lat_n <= 90.0 && k.lat_s < k.lat_n);
        }
        // A pangea that does not touch the edge is the whole world.
        let c = config(json!({ "geography": {} }), "pangea");
        assert_eq!(coordinates(&c, false, &mut rng()).map_size, 100.0);
    }
}
