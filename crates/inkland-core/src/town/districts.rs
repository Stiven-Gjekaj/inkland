//! inkland: the districts of a town and their names.
//!
//! A district is a group of patches next to each other, with wards of the
//! same character. The wall, the river, and the sea are the borders of the
//! districts. The names come from the name base of the culture of the
//! place, with words that tell the character, the position, or the water
//! of the district.

use std::collections::BTreeSet;

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

use super::poly::{self, P};
use super::{Kind, Town};
use crate::names::Chain;
use crate::rand::Rand;

/// A district of a town.
#[derive(Debug, Clone, PartialEq)]
pub struct District {
    pub name: String,
    /// Indexes of `Town::patches`.
    pub patches: Vec<usize>,
    /// The point of the label: inside a patch of the district.
    pub label: P,
    /// The area of the district.
    pub area: f64,
}

/// The character of a ward, for the grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Poor,
    Work,
    Rich,
}

fn class(k: Kind) -> Option<Class> {
    match k {
        Kind::Slum => Some(Class::Poor),
        Kind::Craftsmen | Kind::Gate | Kind::Military => Some(Class::Work),
        Kind::Merchant | Kind::Patriciate | Kind::Administration | Kind::Cathedral | Kind::Park => {
            Some(Class::Rich)
        }
        // The market and the castle have a shape of their own. Farms and
        // empty land are not districts.
        Kind::Market | Kind::Castle | Kind::Farm | Kind::Empty => None,
    }
}

/// The number of common vertices of two shapes, with the common vertices
/// on the river left out.
fn shared(a: &[P], b: &[P], river: &[P]) -> usize {
    a.iter()
        .filter(|p| b.contains(p) && !river.contains(p))
        .count()
}

fn touches(shape: &[P], pts: &[P]) -> bool {
    shape.iter().any(|p| pts.contains(p))
}

/// The districts of a town, with names from the name base `base`.
pub fn districts(town: &Town, base: usize, seed: i64) -> Vec<District> {
    let river: &[P] = town.river.as_deref().unwrap_or(&[]);
    let shore: Vec<P> = town.water.iter().flatten().copied().collect();
    let n = town.patches.len();
    let classes: Vec<Option<Class>> = town
        .patches
        .iter()
        .map(|p| {
            // A patch out of the town with no building is not a district.
            if !p.within_city && p.buildings.len() < 3 {
                None
            } else {
                class(p.kind)
            }
        })
        .collect();
    let mut order: Vec<usize> = (0..n).filter(|&i| classes[i].is_some()).collect();
    order.sort_by(|&a, &b| {
        let (ca, cb) = (
            poly::centroid(&town.patches[a].shape),
            poly::centroid(&town.patches[b].shape),
        );
        poly::len(ca).total_cmp(&poly::len(cb)).then(a.cmp(&b))
    });
    let mut rng = Rand::new(ChaCha8Rng::seed_from_u64(seed as u64));
    let mut owner: Vec<Option<usize>> = vec![None; n];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &start in &order {
        if owner[start].is_some() {
            continue;
        }
        let g = groups.len();
        let limit = rng.int(3, 5) as usize;
        let mut members = vec![start];
        owner[start] = Some(g);
        let mut i = 0;
        while i < members.len() && members.len() < limit {
            let a = &town.patches[members[i]];
            for &b in &order {
                if members.len() >= limit {
                    break;
                }
                let pb = &town.patches[b];
                if owner[b].is_none()
                    && classes[b] == classes[start]
                    && pb.within_walls == a.within_walls
                    && pb.within_city == a.within_city
                    && shared(&a.shape, &pb.shape, river) >= 2
                {
                    owner[b] = Some(g);
                    members.push(b);
                }
            }
            i += 1;
        }
        groups.push(members);
    }
    // A district of one patch joins a district next to it on the same
    // side of the wall, if there is one.
    for g in 0..groups.len() {
        if groups[g].len() != 1 {
            continue;
        }
        let a = &town.patches[groups[g][0]];
        let near = (0..n).find(|&b| {
            owner[b].is_some_and(|o| o != g && groups[o].len() > 1)
                && town.patches[b].within_walls == a.within_walls
                && town.patches[b].within_city == a.within_city
                && shared(&a.shape, &town.patches[b].shape, river) >= 2
        });
        if let Some(b) = near {
            let o = owner[b].unwrap_or(g);
            let p = groups[g][0];
            groups[o].push(p);
            owner[p] = Some(o);
            groups[g].clear();
        }
    }
    groups.retain(|g| !g.is_empty());
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut bridge_named = false;
    // At most 2 districts take a name of the sea, and 2 of the river.
    let (mut sea_named, mut river_named) = (0, 0);
    let chain = Chain::new(base);
    let mut out = Vec::new();
    for (gi, members) in groups.iter().enumerate() {
        let area: f64 = members
            .iter()
            .map(|&i| poly::square(&town.patches[i].shape).abs())
            .sum();
        let label = label_point(town, members);
        let first = &town.patches[members[0]];
        let shapes = || members.iter().map(|&i| &town.patches[i].shape);
        let river_side = shapes().any(|s| touches(s, river));
        let sea_side = shapes().any(|s| touches(s, &shore));
        let bridged = town
            .bridges
            .iter()
            .any(|b| shapes().any(|s| s.contains(&b.at)));
        let ctx = Context {
            class: classes[members[0]].unwrap_or(Class::Work),
            first: gi == 0 && first.within_walls && groups.len() >= 5,
            outside: !first.within_walls,
            river_side: river_side && river_named < 2,
            sea_side: sea_side && sea_named < 2,
            bridged: bridged && !bridge_named,
            dir: compass(label),
        };
        bridge_named |= ctx.bridged;
        sea_named += usize::from(ctx.sea_side);
        river_named += usize::from(ctx.river_side && !ctx.sea_side && !ctx.bridged);
        let name = name_of(&ctx, &chain, &mut rng, &mut used);
        out.push(District {
            name,
            patches: members.clone(),
            label,
            area,
        });
    }
    out
}

/// The label point: the centroid of the district if a patch holds it, or
/// else the centroid of the largest patch.
fn label_point(town: &Town, members: &[usize]) -> P {
    let mut sum = [0.0, 0.0];
    let mut total = 0.0;
    for &i in members {
        let s = &town.patches[i].shape;
        let a = poly::square(s).abs();
        let c = poly::centroid(s);
        sum = poly::add(sum, poly::scale(c, a));
        total += a;
    }
    let c = poly::scale(sum, 1.0 / total.max(1e-9));
    if members.iter().any(|&i| contains(&town.patches[i].shape, c)) {
        return c;
    }
    let big = poly::max_by(members, |&i| poly::square(&town.patches[i].shape).abs());
    poly::centroid(&town.patches[big].shape)
}

/// `true` if `p` is inside the polygon `s`.
pub fn contains(s: &[P], p: P) -> bool {
    let mut inside = false;
    let n = s.len();
    for i in 0..n {
        let (a, b) = (s[i], s[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

/// The side of the town of a point: the y axis goes south.
fn compass(p: P) -> &'static str {
    let a = libm::atan2(p[1], p[0]).to_degrees();
    match a {
        a if (-135.0..-45.0).contains(&a) => "North",
        a if (-45.0..45.0).contains(&a) => "East",
        a if (45.0..135.0).contains(&a) => "South",
        _ => "West",
    }
}

struct Context {
    class: Class,
    first: bool,
    outside: bool,
    river_side: bool,
    sea_side: bool,
    bridged: bool,
    dir: &'static str,
}

/// The patterns of the names. `{n}` is a name from the name base and `{d}`
/// is a side of the town.
fn patterns(c: &Context) -> Vec<&'static str> {
    if c.first {
        return vec!["Old Town", "Old {n}", "{n}"];
    }
    if c.sea_side {
        return vec!["Harbour", "Docks", "Quayside", "{n} Wharf", "{d} Quay"];
    }
    if c.bridged {
        return vec!["Bridge End", "{n} Bridge", "Bridgefoot"];
    }
    if c.river_side {
        return vec!["Riverside", "{n} Bank", "Waterside", "{n} Mills"];
    }
    if c.outside {
        return vec!["{d} End", "{n}", "Outer {n}", "{d} Gate", "{n} Fields"];
    }
    match c.class {
        Class::Poor => vec![
            "{d} Slums",
            "{n} Rookery",
            "Lower {n}",
            "The Warrens",
            "{n}",
        ],
        Class::Work => vec![
            "{n} Row",
            "{n}",
            "Smiths' Row",
            "Tanners' Lane",
            "Weavers' Quarter",
            "{n} Quarter",
            "Coopers' Lane",
        ],
        Class::Rich => vec![
            "{n} Hill",
            "High {n}",
            "{n}",
            "{n} Court",
            "Guild Quarter",
            "Upper Town",
        ],
    }
}

fn name_of(c: &Context, chain: &Chain, rng: &mut Rand, used: &mut BTreeSet<String>) -> String {
    let mut list = patterns(c);
    // Each pattern once at most, then a plain name.
    for _ in 0..list.len() {
        let i = rng.int(0, list.len() as i64 - 1) as usize;
        let pat = list.remove(i);
        let name = if pat.contains("{n}") {
            let root = chain.short(rng, used);
            pat.replace("{n}", &root).replace("{d}", c.dir)
        } else {
            pat.replace("{d}", c.dir)
        };
        if used.insert(name.clone()) {
            return name;
        }
    }
    chain.name(rng, used)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_is_inside_a_square() {
        let s = vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
        assert!(contains(&s, [1.0, 1.0]));
        assert!(!contains(&s, [3.0, 1.0]));
    }

    #[test]
    fn the_compass_has_north_up() {
        assert_eq!(compass([0.0, -5.0]), "North");
        assert_eq!(compass([5.0, 0.0]), "East");
        assert_eq!(compass([0.0, 5.0]), "South");
        assert_eq!(compass([-5.0, 0.0]), "West");
    }
}
