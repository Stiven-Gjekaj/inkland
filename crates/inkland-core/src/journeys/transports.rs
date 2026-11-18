//! The transports of FMG (`transports-generator.ts`).
//!
//! A transport has a speed in km/h, a travel domain, and the hours of
//! travel that one day sustains. FMG keeps the list in the map options,
//! and the user can edit it. The world holds the list.

/// The travel domain of a transport. It sets the pathfinding and the
/// cells that a segment can start and end on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    /// Walks, wheels, and hooves: land cells only. The path uses the
    /// route network if it can.
    Land,
    /// Boats and ships: water cells, moorages, and navigable rivers.
    Water,
    /// Flight and magic: a direct line that ignores the terrain.
    Air,
    /// No movement: a halt for the story (a rest or a wait).
    Stay,
}

impl Domain {
    /// The FMG name of the domain.
    pub fn name(self) -> &'static str {
        match self {
            Domain::Land => "land",
            Domain::Water => "water",
            Domain::Air => "air",
            Domain::Stay => "stay",
        }
    }
}

/// One transport type.
#[derive(Debug, Clone, PartialEq)]
pub struct Transport {
    /// 1, 2, 3, ... as in FMG.
    pub id: u32,
    pub name: String,
    /// The speed in km/h.
    pub speed: f64,
    pub domain: Domain,
    /// The hours of travel that one day sustains.
    pub hours_per_day: f64,
}

/// The longest travel day, in hours.
pub const MAX_HOURS_PER_DAY: f64 = 24.0;

/// One default transport: id, name, speed, domain, and hours per day.
type Raw = (u32, &'static str, f64, Domain, f64);

const fn t(id: u32, name: &'static str, speed: f64, domain: Domain, hours: f64) -> Raw {
    (id, name, speed, domain, hours)
}

/// The default transports of FMG (`DEFAULT_TRANSPORTS`).
pub fn default_transports() -> Vec<Transport> {
    RAW.iter()
        .map(|&(id, name, speed, domain, hours)| Transport {
            id,
            name: name.to_owned(),
            speed,
            domain,
            hours_per_day: hours,
        })
        .collect()
}

const RAW: [Raw; 20] = [
    t(1, "On foot (laden)", 3.0, Domain::Land, 8.0),
    t(2, "On foot (light)", 5.0, Domain::Land, 8.0),
    t(3, "Horseback (no spare horse)", 7.0, Domain::Land, 8.0),
    t(4, "Horseback (spare horse)", 12.0, Domain::Land, 10.0),
    t(5, "Carriage", 6.0, Domain::Land, 10.0),
    t(6, "Stagecoach", 10.0, Domain::Land, 12.0),
    t(7, "Train", 40.0, Domain::Land, 24.0),
    t(8, "Automobile", 50.0, Domain::Land, 10.0),
    t(9, "Modern Automobile", 80.0, Domain::Land, 12.0),
    t(10, "Rowboat", 4.0, Domain::Water, 8.0),
    t(11, "Sailing boat", 6.0, Domain::Water, 12.0),
    t(12, "Sailing Ship", 10.0, Domain::Water, 24.0),
    t(13, "Steamship", 25.0, Domain::Water, 24.0),
    t(14, "Modern Ship", 35.0, Domain::Water, 24.0),
    t(15, "Aircraft", 120.0, Domain::Air, 4.0),
    t(16, "Dirigible", 20.0, Domain::Air, 24.0),
    t(17, "Helicopter", 220.0, Domain::Air, 6.0),
    t(18, "Modern Airplane", 800.0, Domain::Air, 12.0),
    t(19, "Teleport", 10000.0, Domain::Air, 24.0),
    t(20, "Stay", 0.0, Domain::Stay, 24.0),
];

/// The hours per day of a domain, for a transport without a valid value
/// (`FALLBACK_HOURS_PER_DAY`).
pub fn fallback_hours_per_day(domain: Domain) -> f64 {
    match domain {
        Domain::Land | Domain::Air => 8.0,
        Domain::Water => 12.0,
        Domain::Stay => MAX_HOURS_PER_DAY,
    }
}

/// The transport with this name (`Transports.get`).
pub fn get<'a>(list: &'a [Transport], name: &str) -> Option<&'a Transport> {
    list.iter().find(|t| t.name == name)
}

/// The domain of the named transport. An unknown name has no limits, so
/// its domain is `Air` (`getDomain`).
pub fn domain(list: &[Transport], name: &str) -> Domain {
    get(list, name).map_or(Domain::Air, |t| t.domain)
}

/// The hours of travel a day with the named transport
/// (`getHoursPerDay`).
pub fn hours_per_day(list: &[Transport], name: &str) -> f64 {
    get(list, name).map_or_else(
        || fallback_hours_per_day(domain(list, name)),
        resolve_hours_per_day,
    )
}

/// The hours per day of a transport, or the fallback of its domain when
/// the value is out of range (`resolveHoursPerDay`).
pub fn resolve_hours_per_day(transport: &Transport) -> f64 {
    let hours = transport.hours_per_day;
    if hours.is_finite() && hours > 0.0 && hours <= MAX_HOURS_PER_DAY {
        hours
    } else {
        fallback_hours_per_day(transport.domain)
    }
}

/// The first transport of a domain (`getByDomain`).
pub fn by_domain(list: &[Transport], domain: Domain) -> Option<&Transport> {
    list.iter().find(|t| t.domain == domain)
}

/// All transports of a domain, in list order.
pub fn in_domain(list: &[Transport], domain: Domain) -> impl Iterator<Item = &Transport> {
    list.iter().filter(move |t| t.domain == domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hours_per_day_come_from_the_transport() {
        let l = default_transports();
        assert_eq!(hours_per_day(&l, "On foot (laden)"), 8.0);
        assert_eq!(hours_per_day(&l, "Dirigible"), 24.0);
        // An unknown name is an air transport, with the air fallback.
        assert_eq!(hours_per_day(&l, "Removed type"), 8.0);
        assert_eq!(domain(&l, "Removed type"), Domain::Air);
    }

    #[test]
    fn an_out_of_range_rate_takes_the_fallback_of_its_domain() {
        let mut boat = get(&default_transports(), "Rowboat").unwrap().clone();
        boat.hours_per_day = 30.0;
        assert_eq!(resolve_hours_per_day(&boat), 12.0);
        boat.hours_per_day = 0.0;
        assert_eq!(resolve_hours_per_day(&boat), 12.0);
    }

    #[test]
    fn each_domain_has_a_first_transport() {
        let l = default_transports();
        assert_eq!(by_domain(&l, Domain::Land).unwrap().name, "On foot (laden)");
        assert_eq!(by_domain(&l, Domain::Water).unwrap().name, "Rowboat");
        assert_eq!(by_domain(&l, Domain::Air).unwrap().name, "Aircraft");
        assert_eq!(by_domain(&l, Domain::Stay).unwrap().name, "Stay");
    }

    #[test]
    fn ids_are_sequential_and_names_unique() {
        let l = default_transports();
        for (k, t) in l.iter().enumerate() {
            assert_eq!(t.id as usize, k + 1);
            assert_eq!(get(&l, &t.name).unwrap().id, t.id);
        }
    }
}
