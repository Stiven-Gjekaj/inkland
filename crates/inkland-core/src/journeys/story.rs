//! The story journey of FMG (`journey-story.ts` `generateStoryJourney`).
//!
//! An archetype gives the party: what it is, how it travels, and how it
//! names its legs and halts. The party starts at a notable place, goes a
//! number of legs from place to place, and stops on the way: a gathering
//! before the start, a wait at a harbor, a camp in the wild in the middle
//! of a long leg, and a rest at a place between two legs.

use super::lore::{self, ARCHETYPES, Archetype, halt};
use super::map::{Burg, Map};
use super::path::Pathfinder;
use super::places::{burg_name, cell_place_phrase};
use super::transports::{self, Domain, Transport};
use super::{Point, Segment};
use crate::rand::Rand;
use std::collections::BTreeMap;

const ORIGIN_POOL_SIZE: usize = 8;
const ORIGIN_RETRIES: usize = 3;
const CANDIDATE_POOL_SIZE: usize = 5;
const LEG_COUNT_WEIGHTS: [(usize, u32); 6] = [(1, 3), (2, 5), (3, 4), (4, 3), (5, 2), (6, 1)];
const LEG_MIN_FACTOR: f64 = 0.08;
const LEG_MAX_FACTOR: f64 = 0.4;
const MAX_PATH_ATTEMPTS: usize = 24;
const MIN_SPLIT_POINTS: usize = 9;
const SPLIT_BAND: [f64; 2] = [0.35, 0.65];
const HARBOR_WAIT_CHANCE: f64 = 0.45;
const MUSTER_CHANCE: f64 = 0.25;
const MAX_SEGMENTS: usize = 15;

/// A story journey before it has an id and a color.
#[derive(Debug, Clone, PartialEq)]
pub struct Story {
    pub name: String,
    pub kind: &'static str,
    pub segments: Vec<Segment>,
}

struct Leg<'a> {
    from: &'a Burg,
    to: &'a Burg,
    domain: Domain,
    transport: &'a Transport,
    avoid_roads: bool,
    points: Vec<Point>,
    distance: f64,
}

/// `rw` of FMG for keys of any type, in list order.
pub(crate) fn rw<T: Copy>(rng: &mut Rand, items: &[(T, u32)]) -> T {
    let total: u32 = items.iter().map(|i| i.1).sum();
    let mut k = (rng.unit() * f64::from(total)) as u32;
    for &(key, w) in items {
        if k < w {
            return key;
        }
        k -= w;
    }
    items[items.len() - 1].0
}

fn hypot(a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
    libm::sqrt(dx * dx + dy * dy)
}

/// Make a party and plot its route; `None` when the map cannot carry one
/// (`generateStoryJourney`).
pub fn generate<P: Pathfinder>(map: &Map, pf: &P, rng: &mut Rand) -> Option<Story> {
    let burgs: Vec<&Burg> = map.burgs.iter().collect();
    if burgs.len() < 2 {
        return None;
    }
    let weights: Vec<(usize, u32)> = ARCHETYPES
        .iter()
        .enumerate()
        .map(|(k, a)| (k, a.weight))
        .collect();
    let archetype = &ARCHETYPES[rw(rng, &weights)];

    // An origin can be a dead end: a lone burg on an island that nothing
    // sails to.
    let mut g = Gen { map, pf, rng };
    for _ in 0..ORIGIN_RETRIES {
        let legs = g.plan_legs(archetype, &burgs);
        if legs.is_empty() {
            continue;
        }
        let segments = g.build_segments(archetype, &legs);
        if !segments.is_empty() {
            return Some(Story {
                name: g.title(archetype, &legs),
                kind: archetype.kind,
                segments,
            });
        }
    }
    None
}

/// The domain that the party is built around (`primaryDomain`).
fn primary_domain(a: &Archetype) -> Domain {
    let mut first = a.domains[0];
    for &d in &a.domains[1..] {
        if d.1 > first.1 {
            first = d;
        }
    }
    first.0
}

/// The part of the travel of the party in `domain`, from 0 to 1
/// (`domainShare`).
fn domain_share(a: &Archetype, domain: Domain) -> f64 {
    let total: u32 = a.domains.iter().map(|d| d.1).sum();
    if total == 0 {
        return 0.0;
    }
    let w = a.domains.iter().find(|d| d.0 == domain).map_or(0, |d| d.1);
    f64::from(w) / f64::from(total)
}

/// `true` if the party can leave its landmass (`crossesWater`).
fn crosses_water(a: &Archetype) -> bool {
    domain_share(a, Domain::Water) + domain_share(a, Domain::Air) > 0.0
}

struct Plan {
    harbor_wait: bool,
    camp: bool,
    rest: bool,
}

struct Slice {
    points: Vec<Point>,
    distance: f64,
}

struct Split {
    first: Slice,
    second: Slice,
    camp_cell: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
    Whole,
    First,
    Second,
}

struct Gen<'a, 'r, P> {
    map: &'a Map,
    pf: &'a P,
    rng: &'r mut Rand,
}

impl<'a, P: Pathfinder> Gen<'a, '_, P> {
    fn plan_legs(&mut self, a: &Archetype, burgs: &[&'a Burg]) -> Vec<Leg<'a>> {
        let diagonal = hypot([self.map.width, self.map.height], [0.0, 0.0]);
        let diagonal = if diagonal == 0.0 { 1.0 } else { diagonal };
        let band = [diagonal * LEG_MIN_FACTOR, diagonal * LEG_MAX_FACTOR];

        let origin = self.pick_origin(burgs, a);
        let mut visited = std::collections::BTreeSet::from([origin.id]);
        let mut legs: Vec<Leg<'a>> = Vec::new();
        let mut current = origin;
        let mut attempts = 0;
        let leg_count = rw(self.rng, &LEG_COUNT_WEIGHTS);

        while legs.len() < leg_count && attempts < MAX_PATH_ATTEMPTS {
            let candidates = self.rank_candidates(current, burgs, &visited, band, a);
            if candidates.is_empty() {
                break;
            }
            let mut leg = None;
            for candidate in candidates {
                let tried = attempts;
                attempts += 1;
                if tried >= MAX_PATH_ATTEMPTS {
                    break;
                }
                // A candidate that fails is not worth a later try either.
                visited.insert(candidate.id);
                leg = self.build_leg(a, current, candidate);
                if leg.is_some() {
                    break;
                }
            }
            match leg {
                Some(leg) => {
                    current = leg.to;
                    legs.push(leg);
                }
                // Stuck in the middle of the route: the journey ends here.
                None if !legs.is_empty() => break,
                // No first leg yet: the next batch of candidates gets a try.
                None => {}
            }
        }
        legs
    }

    /// The start of the journey: a notable burg, a port for a party that
    /// mostly sails (`pickOrigin`).
    fn pick_origin(&mut self, burgs: &[&'a Burg], a: &Archetype) -> &'a Burg {
        let f = &self.map.f;
        let mut neighbours: BTreeMap<u32, u32> = BTreeMap::new();
        for b in burgs {
            *neighbours.entry(f[b.cell]).or_insert(0) += 1;
        }
        let sea = domain_share(a, Domain::Water);
        let over_water = (sea + domain_share(a, Domain::Air)).min(1.0);
        let ports: Vec<&'a Burg> = burgs.iter().copied().filter(|b| b.port).collect();
        let pool: &[&'a Burg] = if primary_domain(a) == Domain::Water && ports.len() > 1 {
            &ports
        } else {
            burgs
        };
        let mut scored: Vec<(&'a Burg, f64)> = pool
            .iter()
            .map(|&b| {
                let mut score = b.population + 1.0;
                if b.capital {
                    score *= 2.5;
                }
                score *= if b.port {
                    1.0 + sea * 2.0
                } else {
                    1.0 - sea * 0.7
                };
                let count = neighbours.get(&f[b.cell]).copied().unwrap_or(1);
                let overland = (f64::from(count) / 3.0).min(1.0);
                score *= overland + (1.0 - overland) * over_water;
                (b, score * (0.5 + self.rng.unit()))
            })
            .collect();
        scored.sort_by(|x, y| y.1.total_cmp(&x.1));
        scored.truncate(ORIGIN_POOL_SIZE);
        self.rng.pick(&scored).0
    }

    /// The next stops to try, best first: a good leg away, notable, and
    /// over a border if it can be (`rankCandidates`).
    fn rank_candidates(
        &mut self,
        current: &'a Burg,
        burgs: &[&'a Burg],
        visited: &std::collections::BTreeSet<u32>,
        band: [f64; 2],
        a: &Archetype,
    ) -> Vec<&'a Burg> {
        let [min_leg, max_leg] = band;
        let mid = (min_leg + max_leg) / 2.0;
        let sea = domain_share(a, Domain::Water);
        let landbound = !crosses_water(a);
        let f = &self.map.f;
        let mut in_band: Vec<(&'a Burg, f64)> = Vec::new();
        let mut out_of_band: Vec<(&'a Burg, f64)> = Vec::new();
        for &b in burgs {
            if visited.contains(&b.id) {
                continue;
            }
            if landbound && f[b.cell] != f[current.cell] {
                continue;
            }
            let mut score = 1.0 + libm::log10(1.0 + b.population);
            if b.capital {
                score += 0.8;
            }
            if b.state != current.state {
                score += 0.8;
            }
            let sailable = b.port && current.port;
            if b.port {
                score += if sailable { sea * 3.0 } else { sea };
            }
            if !sailable {
                score *= 1.0 - sea * 0.8;
            }
            score *= 0.5 + self.rng.unit();
            let d = hypot(b.at, current.at);
            if d >= min_leg && d <= max_leg {
                in_band.push((b, score));
            } else {
                out_of_band.push((b, -(d - mid).abs()));
            }
        }
        let mut pool = if in_band.is_empty() {
            out_of_band
        } else {
            in_band
        };
        pool.sort_by(|x, y| y.1.total_cmp(&x.1));
        pool.into_iter()
            .take(CANDIDATE_POOL_SIZE)
            .map(|x| x.0)
            .collect()
    }

    /// Route one leg in the domains of the party, rolled by weight
    /// (`buildLeg`).
    fn build_leg(&mut self, a: &Archetype, from: &'a Burg, to: &'a Burg) -> Option<Leg<'a>> {
        for domain in self.roll_domains(a) {
            // A water leg needs a port at both ends.
            if domain == Domain::Water && !(from.port && to.port) {
                continue;
            }
            let Some(transport) = self.resolve_transport(a, domain) else {
                continue;
            };
            for avoid_roads in self.road_preference(a, domain) {
                let r = self.pf.find_path(from.cell, to.cell, domain, avoid_roads);
                if r.error.is_some()
                    || r.points.len() < 2
                    || !self.pf.is_valid_path(&r.points, domain)
                {
                    continue;
                }
                return Some(Leg {
                    from,
                    to,
                    domain,
                    transport,
                    avoid_roads,
                    points: r.points,
                    distance: r.distance,
                });
            }
        }
        None
    }

    /// The domains in the order that the party tries them: a shuffle by
    /// weight (`rollDomains`).
    fn roll_domains(&mut self, a: &Archetype) -> Vec<Domain> {
        let mut remaining: Vec<(Domain, u32)> = a.domains.to_vec();
        let mut order = Vec::new();
        while !remaining.is_empty() {
            let d = rw(self.rng, &remaining);
            order.push(d);
            remaining.retain(|x| x.0 != d);
        }
        order
    }

    /// Off-road first for a party that takes to the wild, then the roads
    /// (`roadPreference`).
    fn road_preference(&mut self, a: &Archetype, domain: Domain) -> Vec<bool> {
        if domain != Domain::Land {
            return vec![false];
        }
        if self.rng.p(a.off_road) {
            vec![true, false]
        } else {
            vec![false]
        }
    }

    /// The rolled transport of the party, else another transport it
    /// names, else the slowest transport of the domain
    /// (`resolveTransport`).
    fn resolve_transport(&mut self, a: &Archetype, domain: Domain) -> Option<&'a Transport> {
        let in_domain: Vec<&'a Transport> =
            transports::in_domain(&self.map.transports, domain).collect();
        if in_domain.is_empty() {
            return None;
        }
        let weights: &[(&str, u32)] = a
            .transports
            .iter()
            .find(|t| t.0 == domain)
            .map_or(&[], |t| t.1);
        let by_name = |name: &str| in_domain.iter().copied().find(|t| t.name == name);
        if !weights.is_empty()
            && let Some(t) = by_name(rw(self.rng, weights))
        {
            return Some(t);
        }
        if let Some(t) = weights.iter().find_map(|w| by_name(w.0)) {
            return Some(t);
        }
        let mut slowest = in_domain[0];
        for &t in &in_domain[1..] {
            if t.speed < slowest.speed {
                slowest = t;
            }
        }
        Some(slowest)
    }

    /// A random item of a list.
    fn ra(&mut self, items: &[&'static str]) -> &'static str {
        self.rng.pick(items)
    }

    /// A lore word for a token, drawn each time the token comes
    /// (`LORE_WORDS`).
    fn lore_word(&mut self, token: &str) -> Option<String> {
        Some(match token {
            "rank" => self.ra(&lore::RANKS).to_string(),
            "company" => self.ra(&lore::COMPANY_ADJECTIVES).to_string(),
            "banner" => self.ra(&lore::BANNERS).to_string(),
            "cargo" => self.ra(&lore::CARGO).to_string(),
            "beast" => self.ra(&lore::BEASTS).to_string(),
            "relic" => {
                let adjective = self.ra(&lore::COMPANY_ADJECTIVES);
                format!("{adjective} {}", self.ra(&lore::RELICS))
            }
            "tavern" => {
                let qualifier = self.ra(&lore::TAVERN_QUALIFIERS);
                format!("The {qualifier} {}", self.ra(&lore::TAVERN_SUBJECTS))
            }
            _ => return None,
        })
    }

    /// Pick a template and fill its `{tokens}`: from the context, else a
    /// lore word, else the token stays (`phrase`).
    fn phrase(&mut self, templates: &[&'static str], context: &[(&str, &str)]) -> String {
        let template = self.ra(templates);
        self.fill(template, context)
    }

    fn fill(&mut self, template: &str, context: &[(&str, &str)]) -> String {
        let mut out = String::new();
        let mut rest = template;
        while let Some(open) = rest.find('{') {
            out.push_str(&rest[..open]);
            let after = &rest[open + 1..];
            let len = after
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(after.len());
            if len == 0 || !after[len..].starts_with('}') {
                out.push('{');
                rest = after;
                continue;
            }
            let name = &after[..len];
            if let Some(v) = context.iter().find(|c| c.0 == name) {
                out.push_str(v.1);
            } else if let Some(word) = self.lore_word(name) {
                out.push_str(&word);
            } else {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
            rest = &after[len + 1..];
        }
        out.push_str(rest);
        out
    }

    fn build_segments(&mut self, a: &Archetype, legs: &[Leg<'a>]) -> Vec<Segment> {
        // FMG lets the user remove the stay transport; the list of inkland
        // always has it.
        let stay = transports::by_domain(&self.map.transports, Domain::Stay)
            .expect("the transports have a stay");
        let mut plans: Vec<Plan> = Vec::new();
        for (index, leg) in legs.iter().enumerate() {
            let harbor_wait = leg.domain == Domain::Water && self.rng.p(HARBOR_WAIT_CHANCE);
            // A party already off the road is likelier to sleep beside it.
            let camp = leg.points.len() >= MIN_SPLIT_POINTS
                && self.rng.p(if leg.avoid_roads {
                    a.camp
                } else {
                    a.camp / 2.0
                });
            let rest = index < legs.len() - 1 && self.rng.p(a.rest);
            plans.push(Plan {
                harbor_wait,
                camp,
                rest,
            });
        }
        let muster = self.rng.p(MUSTER_CHANCE);
        // A long route with no halt is the bare A to B line; a single hop
        // can be just that.
        if legs.len() > 1 && !muster && !plans.iter().any(|p| p.harbor_wait || p.camp || p.rest) {
            match legs.iter().position(|l| l.points.len() >= MIN_SPLIT_POINTS) {
                Some(k) => plans[k].camp = true,
                None => plans[0].rest = true,
            }
        }

        let mut segments: Vec<Segment> = Vec::new();
        // A leg goes in whole or not at all, so a short route still ends
        // at a stop.
        let commit = |segments: &mut Vec<Segment>, group: Vec<Segment>| -> bool {
            if segments.len() + group.len() > MAX_SEGMENTS {
                return false;
            }
            for mut s in group {
                s.id = segments.len() as u32 + 1;
                segments.push(s);
            }
            true
        };
        if muster {
            let place = burg_name(&legs[0].from.name, legs[0].from.id);
            let name = self.phrase(&halt::MUSTER, &[("place", &place)]);
            let hours = self.rng.rand(6, 24) as f64;
            let group = vec![self.make_stay(stay, legs[0].from.cell, name, hours)];
            commit(&mut segments, group);
        }

        for (index, (plan, leg)) in plans.iter().zip(legs).enumerate() {
            let from = burg_name(&leg.from.name, leg.from.id);
            let to = burg_name(&leg.to.name, leg.to.id);
            let mut group = Vec::new();
            if plan.harbor_wait {
                // A party of sailors waits on its own ship, not on a berth.
                let pool: &[&'static str] = if primary_domain(a) == Domain::Water {
                    &halt::CASTING_OFF
                } else {
                    &halt::HARBOR_WAIT
                };
                let name = self.phrase(pool, &[("from", &from)]);
                let hours = self.rng.rand(6, 30) as f64;
                group.push(self.make_stay(stay, leg.from.cell, name, hours));
            }
            let split = if plan.camp { self.split_leg(leg) } else { None };
            let wild_cell = split
                .as_ref()
                .map_or(leg.points[leg.points.len() / 2].cell, |s| s.camp_cell);
            let wild = self.describe_wild(wild_cell);
            let naming = Naming {
                from: &from,
                to: &to,
                wild: &wild,
                first: index == 0,
                last: index == legs.len() - 1,
            };
            if let Some(split) = split {
                let name = self.name_travel(a, leg, &naming, Part::First);
                group.push(make_travel(leg, split.first, name));
                let name = self.name_halt(a, leg, split.camp_cell, &wild);
                let hours = self.rng.rand(6, 12) as f64;
                group.push(self.make_stay(stay, split.camp_cell, name, hours));
                let name = self.name_travel(a, leg, &naming, Part::Second);
                group.push(make_travel(leg, split.second, name));
            } else {
                let name = self.name_travel(a, leg, &naming, Part::Whole);
                let whole = Slice {
                    points: leg.points.clone(),
                    distance: leg.distance,
                };
                group.push(make_travel(leg, whole, name));
            }
            if plan.rest {
                let name = self.phrase(a.stopover, &[("place", &to)]);
                let hours = 12.0 * self.rng.rand(1, 3) as f64;
                group.push(self.make_stay(stay, leg.to.cell, name, hours));
            }
            if !commit(&mut segments, group) {
                break;
            }
        }
        segments
    }

    /// Split a leg at a camp in the middle part of its path (`splitLeg`).
    fn split_leg(&mut self, leg: &Leg<'a>) -> Option<Split> {
        let count = leg.points.len();
        let lo = (count as f64 * SPLIT_BAND[0]).floor() as i64;
        let hi = (count as f64 * SPLIT_BAND[1]).floor() as i64;
        let index = self.rng.rand(lo, hi);
        if index < 1 || index > count as i64 - 2 {
            return None;
        }
        let index = index as usize;
        let first = leg.points[..=index].to_vec();
        let second = leg.points[index..].to_vec();
        Some(Split {
            first: Slice {
                distance: self.pf.path_length(&first),
                points: first,
            },
            second: Slice {
                distance: self.pf.path_length(&second),
                points: second,
            },
            camp_cell: leg.points[index].cell,
        })
    }

    fn make_stay(&self, stay: &Transport, cell: usize, name: String, hours: f64) -> Segment {
        Segment {
            id: 0,
            name,
            from: cell,
            to: cell,
            transport: stay.name.clone(),
            domain: stay.domain,
            hours_per_day: transports::resolve_hours_per_day(stay),
            speed: 0.0,
            distance: 0.0,
            points: vec![self.map.point(cell)],
            avoid_roads: false,
            duration: Some(hours),
            hours: 0.0,
            elapsed_hours: 0.0,
            note: None,
        }
    }

    fn title(&mut self, a: &Archetype, legs: &[Leg<'a>]) -> String {
        let origin = legs[0].from;
        let destination = legs[legs.len() - 1].to;
        // A title that names the land crossed wants a land leg.
        let named = legs
            .iter()
            .find(|l| l.domain == Domain::Land)
            .unwrap_or(&legs[legs.len() / 2]);
        let middle = named.points[named.points.len() / 2].cell;

        let hero = self.person_name(origin);
        let origin_name = burg_name(&origin.name, origin.id);
        let destination_name = burg_name(&destination.name, destination.id);
        let state = self
            .map
            .states
            .get(destination.state as usize)
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| destination_name.clone());
        let adjective = crate::names::adjective(&state, self.rng);
        // A title has its own article, so the article of the water goes.
        let wild = if named.domain == Domain::Land {
            self.describe_wild(middle)
        } else {
            let water = self.describe_water(named);
            water
                .strip_prefix("the ")
                .map_or(water.clone(), str::to_string)
        };
        self.phrase(
            a.title,
            &[
                ("hero", &hero),
                ("origin", &origin_name),
                ("destination", &destination_name),
                ("destinationAdjective", &adjective),
                ("wild", &wild),
            ],
        )
    }

    fn name_travel(&mut self, a: &Archetype, leg: &Leg<'a>, n: &Naming, part: Part) -> String {
        let context = [("from", n.from), ("to", n.to), ("wild", n.wild)];
        let pick = |names: &lore::LegNames| match part {
            Part::Whole => names.whole,
            Part::First => names.first,
            Part::Second => names.second,
        };
        match leg.domain {
            Domain::Air => return self.phrase(pick(&lore::AIR_LEGS), &context),
            Domain::Water => {
                let water = self.describe_water(leg);
                let context = [
                    ("from", n.from),
                    ("to", n.to),
                    ("wild", n.wild),
                    ("water", water.as_str()),
                ];
                return self.phrase(pick(&lore::WATER_LEGS), &context);
            }
            Domain::Land | Domain::Stay => {}
        }
        let land = &lore::LAND_LEGS;
        match part {
            Part::First => return self.phrase(land.first, &context),
            Part::Second => return self.phrase(land.second, &context),
            Part::Whole => {}
        }
        // The last leg names where the party ends, however it goes there.
        if leg.avoid_roads {
            let names = if n.last {
                land.off_road_last
            } else {
                land.off_road
            };
            return self.phrase(names, &context);
        }
        if n.first {
            return self.phrase(land.opening, &context);
        }
        if n.last {
            return self.phrase(land.closing, &context);
        }
        // Half the time the party names the leg in its own voice.
        if let Some(own) = a.leg
            && self.rng.p(0.5)
        {
            return self.phrase(own, &context);
        }
        self.phrase(land.whole, &context)
    }

    /// A night in the middle of a leg: at anchor at sea, in a camp on
    /// land (`nameHalt`).
    fn name_halt(&mut self, a: &Archetype, leg: &Leg<'a>, cell: usize, wild: &str) -> String {
        let label = cell_place_phrase(self.map, cell);
        if leg.domain == Domain::Water {
            let names: &[&'static str] = if label.is_some() {
                &halt::ANCHORED_NEAR
            } else {
                &halt::ANCHORED
            };
            let water = self.describe_water(leg);
            let label = label.unwrap_or_default();
            return self.phrase(names, &[("label", &label), ("water", &water)]);
        }
        // Near a named place, the party names where it slept as often as
        // how.
        if let Some(label) = label {
            let own = self.ra(a.bivouac);
            let mut names: Vec<&'static str> = halt::CAMP_NEAR.to_vec();
            names.push(own);
            return self.phrase(&names, &[("label", &label), ("wild", wild)]);
        }
        self.phrase(a.bivouac, &[("wild", wild)])
    }

    /// The name of a hero from the culture of a burg (`personName`).
    fn person_name(&mut self, burg: &Burg) -> String {
        let name = match self.map.chains.get(burg.culture as usize) {
            Some(chain) => chain.get_short(self.rng),
            None => String::new(),
        };
        if name.is_empty() {
            self.ra(&lore::NAMELESS).to_string()
        } else {
            name
        }
    }

    /// The wild land of a cell: by height, else by biome (`describeWild`).
    fn describe_wild(&mut self, cell: usize) -> String {
        let h = self.map.h[cell];
        if h >= 70.0 {
            return self.ra(&lore::HIGHLAND_TERMS).to_string();
        }
        if h >= 50.0 {
            return self.ra(&lore::UPLAND_TERMS).to_string();
        }
        lore::BIOME_TERMS
            .get(usize::from(self.map.biome[cell]))
            .copied()
            .unwrap_or(lore::UNKNOWN_WILD)
            .to_string()
    }

    /// The water of a leg: the name of the feature at its middle, else the
    /// open water of a lake or the open sea (`describeWater`).
    fn describe_water(&self, leg: &Leg<'a>) -> String {
        let middle = leg.points[leg.points.len() / 2].cell;
        let f = self.map.f[middle] as usize;
        if let Some(Some(name)) = self.map.feature_names.get(f)
            && !name.is_empty()
        {
            return name.clone();
        }
        if self.map.lakes.get(f).copied().unwrap_or(false) {
            "the open water".to_string()
        } else {
            "the open sea".to_string()
        }
    }
}

struct Naming<'s> {
    from: &'s str,
    to: &'s str,
    wild: &'s str,
    first: bool,
    last: bool,
}

fn make_travel(leg: &Leg, slice: Slice, name: String) -> Segment {
    Segment {
        id: 0,
        name,
        from: slice.points[0].cell,
        to: slice.points[slice.points.len() - 1].cell,
        transport: leg.transport.name.clone(),
        domain: leg.transport.domain,
        hours_per_day: transports::resolve_hours_per_day(leg.transport),
        speed: leg.transport.speed,
        distance: slice.distance,
        points: slice.points,
        avoid_roads: leg.avoid_roads,
        duration: None,
        hours: 0.0,
        elapsed_hours: 0.0,
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journeys::path::tests::blank;
    use crate::journeys::path::{PathResult, path_length};
    use crate::journeys::{is_stay, transports};
    use crate::seed::{Layer, SeedTree};
    use std::collections::BTreeSet;

    /// A 10 x 10 land grid, 100 units apart, with eight burgs in two
    /// states.
    const GRID: usize = 10;
    const STEP: f64 = 100.0;
    const BURG_CELLS: [usize; 8] = [0, 7, 23, 35, 48, 62, 79, 94];

    fn cell_point(cell: usize) -> [f64; 2] {
        [(cell % GRID) as f64 * STEP, (cell / GRID) as f64 * STEP]
    }

    /// A straight path of 13 points, so that a camp can split it.
    struct Straight;

    impl Pathfinder for Straight {
        fn find_path(&self, from: usize, to: usize, _: Domain, _: bool) -> PathResult {
            let ([x1, y1], [x2, y2]) = (cell_point(from), cell_point(to));
            let steps = 12;
            let points: Vec<Point> = (0..=steps)
                .map(|i| {
                    let k = i as f64 / steps as f64;
                    let (x, y) = (x1 + (x2 - x1) * k, y1 + (y2 - y1) * k);
                    let cell = if i == 0 {
                        from
                    } else if i == steps {
                        to
                    } else {
                        let c = crate::rand::js_round(y / STEP) * GRID as f64
                            + crate::rand::js_round(x / STEP);
                        c.clamp(0.0, 99.0) as usize
                    };
                    Point { at: [x, y], cell }
                })
                .collect();
            PathResult {
                distance: path_length(&points),
                points,
                warning: None,
                error: None,
            }
        }

        fn is_valid_path(&self, _: &[Point], _: Domain) -> bool {
            true
        }
    }

    fn map() -> Map {
        let n = GRID * GRID;
        let mut m = blank((0..n).map(cell_point).collect(), vec![Vec::new(); n]);
        m.h = vec![30.0; n];
        m.biome = vec![6; n];
        for (k, &cell) in BURG_CELLS.iter().enumerate() {
            let id = k as u32 + 1;
            m.burg[cell] = id;
            m.burgs.push(Burg {
                id,
                cell,
                at: cell_point(cell),
                name: format!("Burg{id}"),
                state: if k < 4 { 1 } else { 2 },
                culture: 0,
                capital: k % 4 == 0,
                port: k % 2 == 0,
                population: 10.0 - k as f64,
            });
        }
        m.states = vec!["Neutrals".into(), "Alderia".into(), "Brennmark".into()];
        m
    }

    fn rng() -> Rand {
        Rand::new(SeedTree::new(7).part_rng(Layer::Journeys, "test"))
    }

    #[test]
    fn fewer_than_two_burgs_give_no_journey() {
        let mut m = map();
        m.burgs.truncate(1);
        assert!(generate(&m, &Straight, &mut rng()).is_none());
    }

    #[test]
    fn journeys_keep_their_invariants_over_many_rolls() {
        let m = map();
        let mut r = rng();
        let mut lengths = Vec::new();
        for _ in 0..100 {
            let j = generate(&m, &Straight, &mut r).expect("a journey");
            assert!(!j.name.is_empty() && !j.kind.is_empty());
            let s = &j.segments;
            for (k, seg) in s.iter().enumerate() {
                assert_eq!(seg.id as usize, k + 1);
                assert!(!seg.name.is_empty());
                assert!(!seg.name.contains('{'), "{}", seg.name);
            }
            // The route is continuous, and it starts and ends at a burg.
            for w in s.windows(2) {
                assert_eq!(w[1].from, w[0].to);
            }
            assert!(BURG_CELLS.contains(&s[0].from));
            assert!(BURG_CELLS.contains(&s[s.len() - 1].to));
            assert!(s.len() <= MAX_SEGMENTS);
            lengths.push(s.len());
            for stay in s.iter().filter(|x| x.speed == 0.0) {
                assert_eq!(
                    transports::domain(&m.transports, &stay.transport),
                    Domain::Stay
                );
                assert!(stay.duration.unwrap() > 0.0);
                assert_eq!(stay.distance, 0.0);
                assert_eq!(stay.from, stay.to);
            }
            let travel: Vec<&Segment> = s.iter().filter(|x| !is_stay(x)).collect();
            assert!(!travel.is_empty());
            for t in travel {
                assert!(t.points.len() > 1);
                assert!(t.distance > 0.0);
                assert_eq!(
                    Some(t.speed),
                    transports::get(&m.transports, &t.transport).map(|x| x.speed)
                );
                assert_eq!(t.points[0].cell, t.from);
                assert_eq!(t.points[t.points.len() - 1].cell, t.to);
            }
        }
        // Some journeys are one hop, some run long.
        assert!(lengths.iter().min().unwrap() <= &2);
        assert!(lengths.iter().max().unwrap() >= &6);
    }

    #[test]
    fn an_air_party_stays_in_the_air() {
        let m = map();
        let mut r = rng();
        let (mut air, mut ground) = (0, 0);
        for _ in 0..600 {
            let j = generate(&m, &Straight, &mut r).unwrap();
            if j.kind != "Airship voyage" {
                continue;
            }
            for s in j.segments.iter().filter(|s| !is_stay(s)) {
                if transports::domain(&m.transports, &s.transport) == Domain::Air {
                    air += 1;
                } else {
                    ground += 1;
                }
            }
        }
        assert!(air > 0);
        assert!(f64::from(air) / f64::from(air + ground) > 0.6);
    }

    #[test]
    fn the_party_the_route_and_the_transport_vary() {
        let m = map();
        let mut r = rng();
        let mut names = BTreeSet::new();
        let mut kinds = BTreeSet::new();
        for _ in 0..40 {
            let j = generate(&m, &Straight, &mut r).unwrap();
            names.insert(j.name);
            kinds.extend(j.segments.iter().map(|s| s.transport.clone()));
        }
        assert!(names.len() > 10, "{names:?}");
        assert!(kinds.len() > 2, "{kinds:?}");
    }

    #[test]
    fn the_same_seed_gives_the_same_journey() {
        let m = map();
        assert_eq!(
            generate(&m, &Straight, &mut rng()),
            generate(&m, &Straight, &mut rng())
        );
    }

    #[test]
    fn tokens_take_the_context_then_lore_words_then_stay() {
        let m = map();
        let mut r = rng();
        let mut g = Gen {
            map: &m,
            pf: &Straight,
            rng: &mut r,
        };
        assert_eq!(g.fill("To {to}!", &[("to", "Redgate")]), "To Redgate!");
        assert_eq!(g.fill("{nothing} and {", &[]), "{nothing} and {");
        let tavern = g.fill("{tavern}", &[]);
        let words: Vec<&str> = tavern.split(' ').collect();
        assert_eq!(words[0], "The");
        assert!(lore::TAVERN_QUALIFIERS.contains(&words[1]));
        assert!(lore::TAVERN_SUBJECTS.contains(&words[2]));
        let relic = g.fill("{relic}", &[]);
        let words: Vec<&str> = relic.split(' ').collect();
        assert!(lore::COMPANY_ADJECTIVES.contains(&words[0]));
        assert!(lore::RELICS.contains(&words[1]));
    }

    #[test]
    fn the_weighted_pick_follows_the_weights() {
        let mut r = rng();
        let mut seen = [0u32; 3];
        for _ in 0..3000 {
            seen[rw(&mut r, &[(0usize, 1), (1, 0), (2, 2)])] += 1;
        }
        assert_eq!(seen[1], 0);
        assert!(seen[2] > seen[0] * 3 / 2, "{seen:?}");
    }
}
