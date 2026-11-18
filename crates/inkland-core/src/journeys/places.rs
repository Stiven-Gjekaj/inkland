//! The place that names a cell (`journey-places.ts`).

use super::map::Map;

/// The burg search goes this many steps from a cell.
const NEARBY_BURG_MAX_CELLS: usize = 3;

/// The burg that names a cell: its name, its position, and `true` if it
/// is only near the cell (FMG `CellPlace`).
#[derive(Debug, Clone, PartialEq)]
pub struct CellPlace {
    pub name: String,
    pub at: [f64; 2],
    pub nearby: bool,
}

/// The name of a burg, or `Burg <id>` for a burg without a name.
pub fn burg_name(name: &str, id: u32) -> String {
    if name.is_empty() {
        format!("Burg {id}")
    } else {
        name.to_string()
    }
}

fn burg_at(map: &Map, cell: usize) -> Option<(String, [f64; 2])> {
    match map.burg[cell] {
        0 => None,
        id => map.burg_by_id(id).map(|b| (burg_name(&b.name, b.id), b.at)),
    }
}

/// The nearest burg to a cell within `max` steps, by a breadth-first
/// search (`findNearbyBurg`).
fn nearby_burg(map: &Map, cell: usize, max: usize) -> Option<(String, [f64; 2])> {
    if map.burgs.is_empty() {
        return None;
    }
    let mut visited = std::collections::BTreeSet::from([cell]);
    let mut frontier = vec![cell];
    for _ in 0..max {
        let mut next = Vec::new();
        for &c in &frontier {
            for &n in &map.c[c] {
                if !visited.insert(n) {
                    continue;
                }
                if let Some(hit) = burg_at(map, n) {
                    return Some(hit);
                }
                next.push(n);
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    None
}

/// The burg that names a cell: the burg on it, else the nearest burg in
/// three steps, else none (`resolveCellPlace`).
pub fn resolve_cell_place(map: &Map, cell: usize) -> Option<CellPlace> {
    if let Some((name, at)) = burg_at(map, cell) {
        return Some(CellPlace {
            name,
            at,
            nearby: false,
        });
    }
    nearby_burg(map, cell, NEARBY_BURG_MAX_CELLS).map(|(name, at)| CellPlace {
        name,
        at,
        nearby: true,
    })
}

/// A short label of an endpoint cell: the burg, `<burg> vicinity`, or
/// `cell <id>` (`cellEndpointLabel`).
pub fn cell_endpoint_label(map: &Map, cell: usize) -> String {
    match resolve_cell_place(map, cell) {
        None => format!("cell {cell}"),
        Some(p) if p.nearby => format!("{} vicinity", p.name),
        Some(p) => p.name,
    }
}

/// Where a halt is, as a phrase for a template: `at Redgate`, `near
/// Redgate`, or none away from all burgs (`cellPlacePhrase`).
pub fn cell_place_phrase(map: &Map, cell: usize) -> Option<String> {
    let p = resolve_cell_place(map, cell)?;
    Some(format!(
        "{} {}",
        if p.nearby { "near" } else { "at" },
        p.name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journeys::map::Burg;
    use crate::journeys::path::tests::blank;

    #[test]
    fn a_cell_takes_the_name_of_its_burg_or_of_a_burg_near_it() {
        // Cells 0-1-2-3-4-5 in a row, with a burg on cell 0.
        let p = (0..6).map(|i| [i as f64 * 10.0, 0.0]).collect();
        let c = (0..6usize)
            .map(|i| {
                let mut n = Vec::new();
                if i > 0 {
                    n.push(i - 1);
                }
                if i < 5 {
                    n.push(i + 1);
                }
                n
            })
            .collect();
        let mut m = blank(p, c);
        m.burg[0] = 1;
        m.burgs.push(Burg {
            id: 1,
            cell: 0,
            at: [1.0, 1.0],
            name: "Redgate".into(),
            state: 0,
            culture: 0,
            capital: false,
            port: false,
            population: 1.0,
        });
        assert_eq!(cell_place_phrase(&m, 0).as_deref(), Some("at Redgate"));
        assert_eq!(cell_place_phrase(&m, 3).as_deref(), Some("near Redgate"));
        assert_eq!(cell_place_phrase(&m, 4), None);
        assert_eq!(cell_endpoint_label(&m, 2), "Redgate vicinity");
        assert_eq!(cell_endpoint_label(&m, 5), "cell 5");
        m.burgs[0].name.clear();
        assert_eq!(cell_endpoint_label(&m, 0), "Burg 1");
    }
}
