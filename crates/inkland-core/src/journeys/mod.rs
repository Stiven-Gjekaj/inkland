//! The journeys of FMG (`model/story/journeys`).
//!
//! A journey is the route of a party between places: a list of segments.
//! A travel segment has a path, a transport, a speed, and a distance. A
//! stay segment is a halt at one cell with a duration in hours.
//!
//! - `transports`: the transports and their speeds.
//! - `lore`: the archetypes of a party and the name tables.
//! - `map`: the world data that journeys read.
//! - `path`: the pathfinding over land, routes, rivers, and water.
//! - `places`: the place that names a cell.
//! - `story`: the story journey of a party.
//!
//! Journeys read the world. They never change it.

pub mod lore;
pub mod map;
pub mod path;
pub mod places;
pub mod story;
pub mod transports;

pub use transports::{Domain, MAX_HOURS_PER_DAY, Transport};

/// A point of a journey path: a position in world units and its cell
/// (FMG `JourneyPoint`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub at: [f64; 2],
    pub cell: usize,
}

/// One segment of a journey (FMG `JourneySegment`).
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// 1, 2, 3, ... in journey order.
    pub id: u32,
    pub name: String,
    /// The start cell.
    pub from: usize,
    /// The end cell.
    pub to: usize,
    /// The name of the transport (see `transports::TRANSPORTS`).
    pub transport: &'static str,
    /// The speed in km/h. A stay has a speed of 0.
    pub speed: f64,
    /// The length of the path, in world units. A stay has a distance of 0.
    pub distance: f64,
    /// The path. A stay has one point.
    pub points: Vec<Point>,
    /// `true` for a land segment off the road network. It travels at half
    /// speed.
    pub avoid_roads: bool,
    /// The hours that the segment takes, when they do not come from the
    /// distance and the speed. Each stay has a duration.
    pub duration: Option<f64>,
    /// The hours of travel or of waiting (FMG `getSegmentTime`).
    pub hours: f64,
    /// The calendar hours: each full travel day of the transport costs 24
    /// hours (FMG `getSegmentElapsedHours`).
    pub elapsed_hours: f64,
    /// A note. Generation writes no note; FMG notes come from the user.
    pub note: Option<String>,
}

/// The totals of a journey (FMG `getTotals`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Totals {
    /// The distance in world units.
    pub distance: f64,
    /// The hours of travel and of waiting.
    pub hours: f64,
    /// The mean speed of the moving segments, in km/h.
    pub avg_speed: f64,
    /// The calendar hours.
    pub elapsed_hours: f64,
    /// The calendar days.
    pub days: f64,
}

/// One journey (FMG `Journey`).
#[derive(Debug, Clone, PartialEq)]
pub struct Journey {
    /// 1, 2, 3, ...
    pub id: u32,
    pub name: String,
    /// What kind of travel this is, for example `Quest` or `Raid`.
    pub kind: &'static str,
    /// A color as `#rrggbb` (FMG `getCardinalColor`).
    pub color: String,
    pub segments: Vec<Segment>,
    pub totals: Totals,
}

/// Rounding of a duration: from 10 days, only days show; from 10 hours,
/// only hours show.
const COARSE_UNIT_THRESHOLD: u64 = 10;
const MINUTES_PER_DAY: u64 = 24 * 60;
/// The speed factor of a land segment off the road network.
pub const OFF_ROAD_SPEED_FACTOR: f64 = 0.5;

/// Kilometers in one distance unit (FMG `KM_IN_DISTANCE_UNIT`). FMG keys
/// the table by its short names; inkland also knows the long names, as
/// `units.name` holds them.
fn km_in_unit(name: &str) -> Option<f64> {
    let name = name.trim().to_ascii_lowercase();
    let name = name.strip_suffix('s').unwrap_or(&name);
    Some(match name {
        "km" | "kilometer" | "kilometre" => 1.0,
        "mi" | "mile" => 1.609344,
        "lg" | "league" | "land league" => 4.828032,
        "vr" | "versta" => 1.0668,
        "nmi" | "nautical mile" => 1.852,
        "nlg" | "nautical league" => 5.556,
        _ => return None,
    })
}

/// The distance units in one kilometer. An unknown unit counts as a
/// kilometer (FMG `getDistanceUnitRatio`).
pub fn distance_unit_ratio(unit: &str) -> f64 {
    km_in_unit(unit).map_or(1.0, |km| 1.0 / km)
}

/// `true` for a halt. The domain of the transport decides, not the speed
/// (`isStaySegment`).
pub fn is_stay(segment: &Segment) -> bool {
    transports::domain(segment.transport) == Domain::Stay
}

/// The length of a segment in units. A stay covers no ground
/// (`getSegmentDistance`; the world units are the distance units).
pub fn segment_distance(segment: &Segment) -> f64 {
    if is_stay(segment) {
        0.0
    } else {
        segment.distance
    }
}

/// The speed in km/h after the off-road penalty (`getEffectiveSpeed`).
pub fn effective_speed(segment: &Segment) -> f64 {
    if segment.speed.is_nan() || segment.speed <= 0.0 {
        return 0.0;
    }
    if segment.avoid_roads {
        segment.speed * OFF_ROAD_SPEED_FACTOR
    } else {
        segment.speed
    }
}

/// The hours of a segment: its duration, else distance over speed. `ratio`
/// is `distance_unit_ratio` of the unit (`getSegmentTime`).
pub fn segment_time(segment: &Segment, ratio: f64) -> f64 {
    if let Some(duration) = segment.duration {
        return duration;
    }
    let speed = effective_speed(segment) * ratio;
    if speed > 0.0 {
        segment_distance(segment) / speed
    } else {
        0.0
    }
}

/// The calendar hours of `hours` of travel: each full travel day costs 24
/// hours, and the hours left over cost only themselves
/// (`getElapsedHours`).
pub fn elapsed_hours(hours: f64, hours_per_day: f64) -> f64 {
    if !hours.is_finite() || hours <= 0.0 {
        return 0.0;
    }
    let rate = if hours_per_day > 0.0 {
        hours_per_day
    } else {
        MAX_HOURS_PER_DAY
    };
    let days = (hours / rate).floor();
    days * MAX_HOURS_PER_DAY + (hours - days * rate)
}

/// The calendar hours of a segment at the travel day of its transport
/// (`getSegmentElapsedHours`).
pub fn segment_elapsed_hours(segment: &Segment, ratio: f64) -> f64 {
    elapsed_hours(
        segment_time(segment, ratio),
        transports::hours_per_day(segment.transport),
    )
}

/// Set `hours` and `elapsed_hours` of each segment, and the totals of the
/// journey.
pub fn finish(journey: &mut Journey, ratio: f64) {
    for s in &mut journey.segments {
        s.hours = segment_time(s, ratio);
        s.elapsed_hours = segment_elapsed_hours(s, ratio);
    }
    journey.totals = totals(&journey.segments, ratio);
}

/// The totals of the segments (`getTotals`).
pub fn totals(segments: &[Segment], ratio: f64) -> Totals {
    let mut t = Totals::default();
    let mut moving = 0.0;
    for s in segments {
        let hours = segment_time(s, ratio);
        t.distance += segment_distance(s);
        t.hours += hours;
        t.elapsed_hours += segment_elapsed_hours(s, ratio);
        if !is_stay(s) {
            moving += hours;
        }
    }
    t.avg_speed = if moving > 0.0 {
        t.distance / moving / ratio
    } else {
        0.0
    };
    t.days = t.elapsed_hours / MAX_HOURS_PER_DAY;
    t
}

fn total_minutes(hours: f64) -> u64 {
    if hours.is_finite() && hours > 0.0 {
        crate::rand::js_round(hours * 60.0) as u64
    } else {
        0
    }
}

fn split_travel_time(elapsed: f64) -> (u64, u64, u64) {
    let minutes = total_minutes(elapsed);
    let days = minutes / MINUTES_PER_DAY;
    let rest = minutes - days * MINUTES_PER_DAY;
    (days, rest / 60, rest % 60)
}

/// A short calendar duration, for example `2d 3h` (`formatTravelTime`).
pub fn format_travel_time(elapsed: f64) -> String {
    let (days, hours, minutes) = split_travel_time(elapsed);
    if days >= COARSE_UNIT_THRESHOLD {
        return format!("{days}d");
    }
    if days > 0 {
        return if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        };
    }
    if hours >= COARSE_UNIT_THRESHOLD {
        return format!("{hours}h");
    }
    if hours > 0 {
        return if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        };
    }
    format!("{minutes}m")
}

/// Hours of moving or of waiting, with no travel day, for example `733h`
/// (`formatHours`).
pub fn format_hours(hours: f64) -> String {
    let minutes = total_minutes(hours);
    let (h, m) = (minutes / 60, minutes % 60);
    if h >= COARSE_UNIT_THRESHOLD {
        return format!("{h}h");
    }
    if h > 0 {
        return if m > 0 {
            format!("{h}h {m}m")
        } else {
            format!("{h}h")
        };
    }
    format!("{m}m")
}

/// The full calendar duration to the minute, for example `52d 4h 9m`
/// (`formatTravelTimeFull`).
pub fn format_travel_time_full(elapsed: f64) -> String {
    let (days, hours, minutes) = split_travel_time(elapsed);
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 || parts.is_empty() {
        parts.push(format!("{minutes}m"));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(distance: f64, speed: f64, transport: &'static str) -> Segment {
        Segment {
            id: 1,
            name: "s".into(),
            from: 0,
            to: 1,
            transport,
            speed,
            distance,
            points: Vec::new(),
            avoid_roads: false,
            duration: None,
            hours: 0.0,
            elapsed_hours: 0.0,
            note: None,
        }
    }

    fn walk(distance: f64, speed: f64) -> Segment {
        seg(distance, speed, "On foot (laden)")
    }

    fn stay(duration: f64) -> Segment {
        Segment {
            duration: Some(duration),
            ..seg(0.0, 0.0, "Stay")
        }
    }

    #[test]
    fn segment_time_is_distance_over_speed() {
        assert_eq!(segment_time(&walk(10.0, 5.0), 1.0), 2.0);
        assert_eq!(segment_time(&walk(10.0, 0.0), 1.0), 0.0);
        assert_eq!(effective_speed(&walk(10.0, 8.0)), 8.0);
        let off = Segment {
            avoid_roads: true,
            ..walk(10.0, 8.0)
        };
        assert_eq!(effective_speed(&off), 4.0);
    }

    #[test]
    fn the_unit_ratio_turns_km_per_hour_into_units() {
        // 1.609344 km is one mile: 16.09344 km/h is 10 miles an hour.
        let ratio = distance_unit_ratio("mile");
        assert_eq!(distance_unit_ratio("mi"), ratio);
        assert_eq!(distance_unit_ratio("miles"), ratio);
        assert!((segment_time(&walk(10.0, 16.09344), ratio) - 1.0).abs() < 1e-12);
        assert_eq!(distance_unit_ratio("km"), 1.0);
        assert_eq!(distance_unit_ratio("furlong"), 1.0);
    }

    #[test]
    fn totals_sum_with_a_weighted_mean_speed() {
        let t = totals(&[walk(10.0, 5.0), walk(20.0, 10.0)], 1.0);
        assert_eq!(t.distance, 30.0);
        assert_eq!(t.hours, 4.0);
        assert_eq!(t.avg_speed, 7.5);
    }

    #[test]
    fn elapsed_hours_spend_a_whole_day_on_each_full_travel_day() {
        assert_eq!(elapsed_hours(25.0, 8.0), 73.0);
        assert_eq!(elapsed_hours(8.0, 8.0), 24.0);
        assert_eq!(elapsed_hours(12.0, 24.0), 12.0);
        assert_eq!(elapsed_hours(0.0, 8.0), 0.0);
        assert_eq!(elapsed_hours(12.0, 0.0), 12.0);
    }

    #[test]
    fn totals_count_days_with_the_transport_of_each_segment() {
        // 24 h on foot is 3 travel days; 24 h in a dirigible is 1.
        let t = totals(&[walk(72.0, 3.0), seg(480.0, 20.0, "Dirigible")], 1.0);
        assert_eq!(t.hours, 48.0);
        assert_eq!(t.elapsed_hours, 96.0);
        assert_eq!(t.days, 4.0);
        assert_eq!(format_travel_time(t.elapsed_hours), "4d");
        let t = totals(&[walk(24.0, 3.0), stay(12.0)], 1.0);
        assert_eq!(t.hours, 20.0);
        assert_eq!(t.elapsed_hours, 36.0);
        assert_eq!(format_travel_time(t.elapsed_hours), "1d 12h");
    }

    #[test]
    fn a_stay_adds_its_duration_and_no_distance() {
        let t = totals(&[walk(10.0, 5.0), stay(4.0)], 1.0);
        assert_eq!(t.distance, 10.0);
        assert_eq!(t.hours, 6.0);
        assert_eq!(t.avg_speed, 5.0);
    }

    #[test]
    fn a_zero_speed_does_not_make_a_stay() {
        let stalled = walk(10.0, 0.0);
        assert!(!is_stay(&stalled));
        assert_eq!(segment_distance(&stalled), 10.0);
        assert!(is_stay(&seg(10.0, 0.0, "Stay")));
    }

    #[test]
    fn a_duration_overrides_the_time() {
        let ride = Segment {
            duration: Some(7.0),
            ..walk(10.0, 5.0)
        };
        assert_eq!(segment_time(&ride, 1.0), 7.0);
        assert_eq!(segment_distance(&ride), 10.0);
        let zero = Segment {
            duration: Some(0.0),
            ..walk(10.0, 5.0)
        };
        assert_eq!(segment_time(&zero, 1.0), 0.0);
        let t = totals(
            &[Segment {
                duration: Some(5.0),
                ..walk(10.0, 5.0)
            }],
            1.0,
        );
        assert_eq!(t.hours, 5.0);
        assert_eq!(t.avg_speed, 2.0);
    }

    #[test]
    fn travel_time_formats_as_fmg() {
        assert_eq!(format_travel_time(0.0), "0m");
        assert_eq!(format_travel_time(0.5), "30m");
        assert_eq!(format_travel_time(1.5), "1h 30m");
        assert_eq!(format_travel_time(elapsed_hours(25.0, 8.0)), "3d 1h");
        assert_eq!(format_travel_time(7200.0), "300d");
        assert_eq!(format_travel_time(7204.15), "300d");
        assert_eq!(format_travel_time(219.0), "9d 3h");
        assert_eq!(format_travel_time(11.5), "11h");
        assert_eq!(format_travel_time(3.25), "3h 15m");
        assert_eq!(format_travel_time(elapsed_hours(12.0, 24.0)), "12h");
        assert_eq!(format_travel_time(elapsed_hours(12.0, 8.0)), "1d 4h");
        assert_eq!(format_travel_time_full(146.2521), "6d 2h 15m");
        assert_eq!(format_travel_time_full(24.1), "1d 6m");
        assert_eq!(format_travel_time_full(7204.15), "300d 4h 9m");
        assert_eq!(format_travel_time_full(0.0), "0m");
        assert_eq!(format_travel_time_full(25.0), "1d 1h");
        assert_eq!(format_hours(733.0), "733h");
        assert_eq!(format_hours(1.25), "1h 15m");
    }
}
