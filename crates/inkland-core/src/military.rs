//! The military of FMG (`model/political/military-generator.ts` and
//! `military-units.ts`): the units, the regiments of each state, and the
//! upkeep and power of the forces.
//!
//! `generate` runs after the taxes, as `Military.generate` does in the FMG
//! pipeline. It sets the war alert of each state again, drafts platoons in
//! the cells and the places, turns the platoons that the state cannot equip
//! into levies, merges the platoons into regiments, names them, and moves
//! the regiments of a state at war to its fronts. The regiments of a state
//! are `State::military`, as in FMG.
//!
//! inkland: the roster is the default roster of FMG; a map has no user
//! units. The population scale is 1000 people a point and the
//! urbanization rate is 1, the FMG defaults. A regiment keeps the glyph of
//! its icon as FMG does. The composition lines of a note start with "- "
//! where FMG writes an em-dash.

use crate::config::Config;
use crate::diplomacy::{Relation, YEAR};
use crate::economy::Economy;
use crate::economy::finance::Expense;
use crate::features::Features;
use crate::grid::Grid;
use crate::quadtree::Quadtree;
use crate::rand::{Rand, rn};
use crate::religions::Religions;
use crate::seed::{Layer, SeedTree};
use crate::society::{NONE, Society, State};
use std::collections::BTreeMap;

/// The military spending a day that keeps troops of power 1, a population
/// point's worth (`UPKEEP_COST`).
pub const UPKEEP_COST: f64 = 20.0;
/// Unpaid troops still fight, if barely.
const MIN_UPKEEP: f64 = 0.05;
/// Population points of troops that a unit a day of consumed goods equips.
const EQUIPMENT: f64 = 0.365;
/// The lines of cells behind the border where a state at war deploys.
const FRONT_DEPTH: usize = 2;
/// The regiment box height at the default style, in map units.
const REGIMENT_SIZE: f64 = 3.0;
/// The people of one population point (`units.population.scale`).
pub const POPULATION_SCALE: f64 = 1000.0;
/// The urban people of one urban point (`urbanization.rate`).
pub const URBANIZATION_RATE: f64 = 1.0;

/// A military unit (`MilitaryUnit`).
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// The glyph of the icon.
    pub icon: String,
    pub name: String,
    /// The percent of the rural people that serve.
    pub rural: f64,
    /// The percent of the urban people that serve.
    pub urban: f64,
    pub crew: f64,
    pub power: f64,
    /// The unit type: `levies`, `melee`, `ranged`, `mounted`, `machinery`,
    /// or `naval`.
    pub kind: String,
    /// `true` if the unit forms regiments of its own (fleets).
    pub separate: bool,
    /// The names of the goods that equip the unit.
    pub goods: Vec<String>,
}

/// One unit of the default roster, as the table gives it.
struct RawUnit {
    icon: &'static str,
    name: &'static str,
    rural: f64,
    urban: f64,
    crew: f64,
    power: f64,
    kind: &'static str,
    separate: bool,
    goods: &'static [&'static str],
}

/// The default roster of FMG (`getDefaultMilitaryUnits`).
pub fn default_units() -> Vec<Unit> {
    RAW_UNITS
        .iter()
        .map(|u| Unit {
            icon: u.icon.to_owned(),
            name: u.name.to_owned(),
            rural: u.rural,
            urban: u.urban,
            crew: u.crew,
            power: u.power,
            kind: u.kind.to_owned(),
            separate: u.separate,
            goods: u.goods.iter().map(|g| (*g).to_owned()).collect(),
        })
        .collect()
}

/// The roster with the changes of the config (`world.military_units`).
pub fn units_from(entries: &[crate::config::UnitEntry]) -> Result<Vec<Unit>, crate::error::Error> {
    use crate::error::{Error, ErrorCode};
    let mut list = default_units();
    for (i, e) in entries.iter().enumerate() {
        let path = format!("/world/military_units/{i}");
        let at = list.iter().position(|u| u.name == e.name);
        if e.remove {
            let Some(k) = at else {
                return Err(Error::new(
                    ErrorCode::RefUnknown,
                    format!("{path}/name"),
                    format!("no military unit has the name \"{}\"", e.name),
                ));
            };
            list.remove(k);
            continue;
        }
        let k = match at {
            Some(k) => k,
            None => {
                let Some(kind) = e.kind else {
                    return Err(Error::new(
                        ErrorCode::TableInvalid,
                        format!("{path}/kind"),
                        format!("the new military unit \"{}\" must give its kind", e.name),
                    ));
                };
                list.push(Unit {
                    icon: DEFAULT_ICON.to_owned(),
                    name: e.name.clone(),
                    rural: 0.0,
                    urban: 0.0,
                    crew: 1.0,
                    power: 1.0,
                    kind: kind.name().to_owned(),
                    separate: kind == crate::config::UnitKind::Naval,
                    goods: Vec::new(),
                });
                list.len() - 1
            }
        };
        let u = &mut list[k];
        let check = |field: &str, v: f64, ok: bool, rule: &str| {
            if ok && v.is_finite() {
                Ok(v)
            } else {
                Err(Error::new(
                    ErrorCode::ValueOutOfRange,
                    format!("{path}/{field}"),
                    format!("{field} must be {rule}, found {v}"),
                ))
            }
        };
        if let Some(v) = &e.icon {
            u.icon = v.clone();
        }
        if let Some(v) = e.rural {
            u.rural = check("rural", v, (0.0..=100.0).contains(&v), "0 to 100")?;
        }
        if let Some(v) = e.urban {
            u.urban = check("urban", v, (0.0..=100.0).contains(&v), "0 to 100")?;
        }
        if let Some(v) = e.crew {
            u.crew = check("crew", v, v > 0.0, "more than 0")?;
        }
        if let Some(v) = e.power {
            u.power = check("power", v, v >= 0.0, "0 or more")?;
        }
        if let Some(v) = e.kind {
            u.kind = v.name().to_owned();
        }
        if let Some(v) = e.separate {
            u.separate = v;
        }
        if let Some(v) = &e.goods {
            u.goods = v.clone();
        }
    }
    Ok(list)
}

const RAW_UNITS: [RawUnit; 6] = [
    RawUnit {
        icon: "\u{1FA93}",
        name: "levies",
        rural: 0.0,
        urban: 0.0,
        crew: 1.0,
        power: 0.5,
        kind: "levies",
        separate: false,
        goods: &[],
    },
    RawUnit {
        icon: "\u{2694}\u{FE0F}",
        name: "infantry",
        rural: 0.25,
        urban: 0.2,
        crew: 1.0,
        power: 1.0,
        kind: "melee",
        separate: false,
        goods: &["Arms"],
    },
    RawUnit {
        icon: "\u{1F3F9}",
        name: "archers",
        rural: 0.12,
        urban: 0.2,
        crew: 1.0,
        power: 1.0,
        kind: "ranged",
        separate: false,
        goods: &["Arms"],
    },
    RawUnit {
        icon: "\u{1F434}",
        name: "cavalry",
        rural: 0.12,
        urban: 0.03,
        crew: 1.0,
        power: 2.0,
        kind: "mounted",
        separate: false,
        goods: &["Horses", "Camels", "Tack"],
    },
    RawUnit {
        icon: "\u{1F4A3}",
        name: "artillery",
        rural: 0.0,
        urban: 0.03,
        crew: 8.0,
        power: 12.0,
        kind: "machinery",
        separate: false,
        goods: &["Artillery", "Gunpowder"],
    },
    RawUnit {
        icon: "\u{1F30A}",
        name: "fleet",
        rural: 0.0,
        urban: 0.015,
        crew: 100.0,
        power: 50.0,
        kind: "naval",
        separate: true,
        goods: &["Boats"],
    },
];

/// The icon of a royal regiment, based in the capital of a monarchy.
pub const ROYAL_ICON: &str = "\u{1F451}";
/// The icon of a regiment without troops.
pub const NEWBIE_ICON: &str = "\u{1F530}";
/// The icon of a regiment whose main unit is not in the roster.
pub const DEFAULT_ICON: &str = "\u{2694}\u{FE0F}";

/// The unit of a name in a roster.
pub fn unit<'a>(units: &'a [Unit], name: &str) -> Option<&'a Unit> {
    units.iter().find(|u| u.name == name)
}

/// A regiment (`Regiment`).
#[derive(Debug, Clone, PartialEq)]
pub struct Regiment {
    /// The id in its state: 0, 1, 2, ... (FMG `i`).
    pub id: u32,
    /// The state id.
    pub state: u32,
    pub name: String,
    /// The cell where the regiment formed.
    pub cell: usize,
    /// The position, in map units (FMG `x`, `y`). A regiment at war stands
    /// at the front.
    pub at: [f64; 2],
    /// The base, where it is stationed (FMG `bx`, `by`).
    pub base: [f64; 2],
    /// `true` for a fleet (FMG `n`).
    pub naval: bool,
    /// The troops of each unit in population points, in the order FMG
    /// adds them (FMG `u`).
    pub units: Vec<(String, f64)>,
    /// All troops in population points (FMG `a`).
    pub total: f64,
    /// The troops of each unit times the power of the unit
    /// (`getTroopsPower`).
    pub power: f64,
    /// The glyph of the icon.
    pub icon: String,
    /// The legend text (FMG `note`).
    pub note: String,
}

impl Regiment {
    /// The points of one unit.
    pub fn of(&self, unit: &str) -> f64 {
        self.units
            .iter()
            .find(|(n, _)| *n == unit)
            .map_or(0.0, |u| u.1)
    }

    /// All troops, in people (`getTroops(a)`).
    pub fn troops(&self) -> f64 {
        troops(self.total)
    }

    /// Troops times unit power (`getTroopsPower`).
    pub fn troops_power(&self) -> f64 {
        self.power
    }
}

/// The troops that population points stand for, rounded up
/// (`getTroops`).
pub fn troops(points: f64) -> f64 {
    rn(points * POPULATION_SCALE, 3).ceil()
}

/// The ordinal of a number: 1st, 2nd, 3rd, 4th, 11th, 21st (`nth`).
pub fn nth(n: i64) -> String {
    let k = ((((n + 90) % 100) - 10) % 10) - 1;
    let suffix = match k {
        0 => "st",
        1 => "nd",
        2 => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// The state modifier of a unit type for a state type (`STATE_MODIFIER`).
fn state_modifier(kind: &str, state: &str) -> Option<f64> {
    let row: [f64; 6] = match kind {
        "melee" => [0.5, 1.2, 1.0, 0.7, 1.2, 1.1],
        "ranged" => [0.9, 1.3, 1.0, 0.8, 2.0, 0.8],
        "mounted" => [2.3, 0.6, 0.7, 0.3, 0.7, 0.8],
        "machinery" => [0.8, 1.4, 1.1, 1.4, 0.4, 1.1],
        "naval" => [0.5, 0.5, 1.2, 1.8, 0.7, 1.2],
        "armored" => [1.0, 0.5, 1.0, 1.0, 0.7, 1.1],
        "aviation" => [0.5, 0.5, 1.2, 1.2, 0.6, 1.2],
        "magical" => [1.0, 2.0, 1.0, 1.0, 1.0, 1.0],
        _ => return None,
    };
    let k = ["Nomadic", "Highland", "Lake", "Naval", "Hunting", "River"]
        .iter()
        .position(|s| *s == state)?;
    Some(row[k])
}

/// The unit types of the cell and place modifiers, in table order.
const CELL_KINDS: [&str; 8] = [
    "melee",
    "ranged",
    "mounted",
    "machinery",
    "naval",
    "armored",
    "aviation",
    "magical",
];

/// The cell modifier of a unit type (`CELL_MODIFIER`), or the place
/// modifier (`BURG_MODIFIER`).
fn terrain_modifier(urban: bool, terrain: &str, kind: &str) -> Option<f64> {
    let row: [f64; 8] = match (urban, terrain) {
        (false, "nomadic") => [0.2, 0.5, 3.0, 0.4, 0.3, 1.6, 1.0, 0.5],
        (false, "wetland") => [0.8, 2.0, 0.3, 1.2, 1.0, 0.2, 0.5, 0.5],
        (false, "highland") => [1.2, 1.6, 0.3, 3.0, 1.0, 0.8, 0.3, 2.0],
        (true, "nomadic") => [0.3, 0.8, 3.0, 0.4, 1.0, 1.6, 1.0, 0.5],
        (true, "wetland") => [1.0, 1.6, 0.2, 1.2, 1.0, 0.2, 0.5, 0.5],
        (true, "highland") => [1.2, 2.0, 0.3, 3.0, 1.0, 0.8, 0.3, 2.0],
        _ => return None,
    };
    CELL_KINDS.iter().position(|k| *k == kind).map(|k| row[k])
}

/// The lines of cells of a front: the border, then the lines behind it.
type Lines = Vec<Vec<usize>>;

/// A platoon: the troops of one unit from one cell or place (`Platoon`).
#[derive(Debug, Clone)]
struct Platoon {
    cell: usize,
    /// Its own troops.
    a: f64,
    /// The troops with the merged platoons.
    t: f64,
    x: f64,
    y: f64,
    /// The unit index.
    u: usize,
    n: bool,
    s: bool,
    /// The merged platoons, by index.
    children: Vec<usize>,
}

/// What the military reads.
pub struct Input<'a> {
    pub config: &'a Config,
    pub grid: &'a Grid,
    pub features: &'a Features,
    pub biomes: &'a [u8],
    pub religions: &'a Religions,
    pub economy: &'a Economy,
    /// The era of the calendar (`lore.calendar.era`).
    pub era: &'a str,
    /// The roster of units.
    pub units: &'a [Unit],
}

/// The share of the conscripts of each unit that the goods the state
/// consumed equip (`getEquipped`): 1 with no goods or no run.
pub fn equipped(e: &Economy, units: &[Unit], state: u32, demand: &[f64]) -> Vec<f64> {
    let consumed = e
        .accounts
        .consumption
        .get(state as usize)
        .map_or(&[][..], Vec::as_slice);
    let goods_of = |u: &Unit| -> Vec<&crate::economy::goods::Good> {
        u.goods
            .iter()
            .filter_map(|name| e.goods.iter().find(|g| g.name == *name))
            .collect()
    };
    let needing = |good: u32| {
        units.iter().enumerate().fold(0.0, |s, (k, u)| {
            let uses = goods_of(u).iter().any(|g| g.id == good);
            s + if uses { demand[k] } else { 0.0 }
        })
    };
    let mut out = vec![1.0; units.len()];
    for (k, u) in units.iter().enumerate() {
        let troops = demand[k];
        let goods = goods_of(u);
        if troops == 0.0 || goods.is_empty() || e.deals.is_empty() {
            continue;
        }
        let kits = goods.iter().fold(0.0, |s, g| {
            let units = consumed.get(g.id as usize - 1).copied().unwrap_or(0.0);
            let coverage = g.coverage[crate::economy::demand::Need::Military as usize];
            let coverage = if coverage == 0.0 { 1.0 } else { coverage };
            let stock = units * EQUIPMENT * coverage;
            s + stock * troops / needing(g.id)
        });
        out[k] = (kits / troops).min(1.0);
    }
    out
}

/// The forces of a state times unit power (`getNominalPower`).
pub fn nominal_power(state: &State) -> f64 {
    state.military.iter().fold(0.0, |s, r| s + r.troops_power())
}

/// The military spending a day that keeps the forces of a state in full
/// (`getUpkeepNeed`).
pub fn upkeep_need(state: &State) -> f64 {
    nominal_power(state) * UPKEEP_COST
}

/// The upkeep of a state, 0 (powerless) to 1 (fully kept); 1 with no run
/// or for a state that levies nothing (`getUpkeep`).
pub fn upkeep(state: &State, e: &Economy) -> f64 {
    if state.id == NONE || !crate::states::levies(state) || e.deals.is_empty() {
        return 1.0;
    }
    let need = upkeep_need(state);
    if need != 0.0 {
        (state.economy.spent_on(Expense::Military) / need).min(1.0)
    } else {
        1.0
    }
}

/// How strongly the forces fight for their upkeep (`getBattleUpkeep`).
pub fn battle_upkeep(state: &State, e: &Economy) -> f64 {
    upkeep(state, e).max(MIN_UPKEEP)
}

/// The battle power of the forces of a state (`getPower`).
pub fn power(state: &State, e: &Economy) -> f64 {
    nominal_power(state) * battle_upkeep(state, e)
}

/// The battle power of a regiment of a state (`getRegimentPower`).
pub fn regiment_power(regiment: &Regiment, state: &State, e: &Economy) -> f64 {
    regiment.troops_power() * battle_upkeep(state, e)
}

/// The cell terrain for the modifiers (`getType`).
fn terrain(biome: u8, h: f64) -> &'static str {
    if [1, 2, 3, 4].contains(&biome) {
        "nomadic"
    } else if [7, 8, 9, 12].contains(&biome) {
        "wetland"
    } else if h >= 70.0 {
        "highland"
    } else {
        "generic"
    }
}

struct Gen<'a> {
    input: &'a Input<'a>,
    /// The place index on each cell.
    burg: Vec<Option<usize>>,
    rng: Rand,
    era_short: String,
}

impl Gen<'_> {
    fn point(&self, c: usize) -> [f64; 2] {
        self.input.grid.points[c]
    }

    /// The platoons of each state (`draft`), by state index.
    fn draft(&self, soc: &Society) -> Vec<Vec<Platoon>> {
        let cells = &soc.cells;
        let religion = &self.input.religions.cell_religion;
        let feature = &self.input.features.of_cell;
        let biomes = self.input.biomes;
        let units = self.input.units;
        let modifiers: Vec<Vec<f64>> = soc
            .states
            .iter()
            .map(|s| {
                let mut m = vec![0.0; units.len()];
                for (k, u) in units.iter().enumerate() {
                    let mut modifier = state_modifier(&u.kind, s.kind.name())
                        .filter(|&v| v != 0.0)
                        .unwrap_or(1.0);
                    if u.kind == "mounted" && s.form_name.contains("Horde") {
                        modifier *= 2.0;
                    } else if u.kind == "naval" && s.form == "Republic" {
                        modifier *= 1.2;
                    }
                    m[k] = modifier * s.alert;
                }
                m
            })
            .collect();
        let mut platoons: Vec<Vec<Platoon>> = vec![Vec::new(); soc.states.len()];
        // The divisors of a people outside the culture, the faith, or the
        // landmass of the state.
        let scale = |s: &State, culture: u32, faith: u32, c: usize, m: f64| {
            let mut m = m;
            if culture != s.culture {
                m = if s.form == "Union" { m / 1.2 } else { m / 2.0 };
            }
            if faith != religion[s.center] {
                m = if s.form == "Theocracy" {
                    m / 2.2
                } else {
                    m / 1.4
                };
            }
            if feature[c] != feature[s.center] {
                m = if s.kind == crate::cultures::CultureType::Naval {
                    m / 1.2
                } else {
                    m / 1.8
                };
            }
            m
        };
        let raise =
            |list: &mut Vec<Platoon>, st: usize, c: usize, m: f64, urban: bool, port: bool| {
                let t = terrain(biomes[c], cells.h[c]);
                for (k, u) in units.iter().enumerate() {
                    let perc = if urban { u.urban } else { u.rural };
                    if perc <= 0.0 || modifiers[st][k] == 0.0 {
                        continue;
                    }
                    let naval = u.kind == "naval";
                    if naval && (!port || cells.haven[c].is_none()) {
                        continue;
                    }
                    let terrain_mod = if t == "generic" {
                        1.0
                    } else {
                        terrain_modifier(urban, t, &u.kind).unwrap_or(1.0)
                    };
                    let army = m * perc * terrain_mod;
                    let total = rn(army * modifiers[st][k], 3);
                    if total == 0.0 {
                        continue;
                    }
                    let [x, y] = match (naval, cells.haven[c]) {
                        (true, Some(h)) => self.point(h),
                        _ => self.point(c),
                    };
                    list.push(Platoon {
                        cell: c,
                        a: total,
                        t: total,
                        x,
                        y,
                        u: k,
                        n: naval,
                        s: u.separate,
                        children: Vec::new(),
                    });
                }
            };
        // Rural cells.
        for c in 0..self.input.grid.len() {
            if cells.pop[c] == 0.0 {
                continue;
            }
            let state = soc.cell_state[c];
            if state == NONE {
                continue;
            }
            let st = state as usize - 1;
            let s = &soc.states[st];
            let m = scale(s, soc.cell_culture[c], religion[c], c, cells.pop[c] / 100.0);
            raise(&mut platoons[st], st, c, m, false, true);
        }
        // Places.
        for p in &soc.places {
            if p.state == NONE || p.population == 0.0 {
                continue;
            }
            let st = p.state as usize - 1;
            let s = &soc.states[st];
            let mut m = p.population * URBANIZATION_RATE / 100.0;
            if p.capital {
                m *= 1.2;
            }
            let m = scale(s, p.culture, religion[p.cell], p.cell, m);
            raise(&mut platoons[st], st, p.cell, m, true, p.port);
        }
        // The conscripts that the state cannot equip serve as levies where
        // they were raised.
        let levy = units.iter().position(|u| u.kind == "levies");
        for (st, list) in platoons.iter_mut().enumerate() {
            let mut demand = vec![0.0; units.len()];
            for p in list.iter() {
                demand[p.u] += p.a;
            }
            let share = equipped(self.input.economy, units, st as u32 + 1, &demand);
            let mut raised: Vec<Platoon> = Vec::new();
            let mut by_cell: BTreeMap<usize, usize> = BTreeMap::new();
            for p in list.iter_mut() {
                let share = share[p.u];
                if share >= 1.0 {
                    continue;
                }
                let unequipped = rn(p.a - p.a * share, 3);
                p.a = rn(p.a - unequipped, 3);
                p.t = p.a;
                let Some(levy) = levy else { continue };
                if unequipped == 0.0 {
                    continue;
                }
                match by_cell.get(&p.cell) {
                    Some(&k) => {
                        let l = &mut raised[k];
                        l.a = rn(l.a + unequipped, 3);
                        l.t = l.a;
                    }
                    None => {
                        let [x, y] = self.input.grid.points[p.cell];
                        by_cell.insert(p.cell, raised.len());
                        raised.push(Platoon {
                            cell: p.cell,
                            a: unequipped,
                            t: unequipped,
                            x,
                            y,
                            u: levy,
                            n: false,
                            s: units[levy].separate,
                            children: Vec::new(),
                        });
                    }
                }
            }
            list.retain(|p| p.a != 0.0);
            list.extend(raised);
        }
        platoons
    }

    /// Merge the platoons of a state into regiments (`form`).
    fn form(&mut self, soc: &Society, st: usize, mut nodes: Vec<Platoon>) -> Vec<Regiment> {
        const EXPECTED: f64 = 3.0;
        if nodes.is_empty() {
            return Vec::new();
        }
        let mergeable = |a: &Platoon, b: &Platoon| (!a.s && !b.s) || a.u == b.u;
        nodes.sort_by(|a, b| a.a.total_cmp(&b.a));
        let points: Vec<[f64; 2]> = nodes.iter().map(|p| [p.x, p.y]).collect();
        let mut tree = Quadtree::from_points(&points);
        let merge = |nodes: &mut Vec<Platoon>, n0: usize, n1: usize| {
            let children = std::mem::take(&mut nodes[n0].children);
            nodes[n1].children.push(n0);
            nodes[n1].children.extend(children.iter().copied());
            nodes[n0].children = children;
            nodes[n1].t = rn(nodes[n1].t + nodes[n0].t, 3);
            nodes[n0].t = 0.0;
        };
        for k in 0..nodes.len() {
            tree.remove(k);
            let (x, y) = (nodes[k].x, nodes[k].y);
            if let Some(o) = tree.find(x, y, 20.0)
                && nodes[o].t != 0.0
                && mergeable(&nodes[k], &nodes[o])
            {
                merge(&mut nodes, k, o);
                continue;
            }
            if nodes[k].t > EXPECTED {
                continue;
            }
            let r = (EXPECTED - nodes[k].t) * if nodes[k].s { 25.0 } else { 50.0 };
            for c in tree.find_all(x, y, r) {
                if nodes[c].t < EXPECTED && mergeable(&nodes[k], &nodes[c]) {
                    merge(&mut nodes, k, c);
                    break;
                }
            }
        }
        let mut roots: Vec<usize> = (0..nodes.len()).filter(|&k| nodes[k].t != 0.0).collect();
        roots.sort_by(|&a, &b| nodes[b].t.total_cmp(&nodes[a].t));
        let state = &soc.states[st];
        let mut regiments: Vec<Regiment> = roots
            .iter()
            .enumerate()
            .map(|(i, &k)| {
                let r = &nodes[k];
                let roster = self.input.units;
                let mut units: Vec<(String, f64)> = vec![(roster[r.u].name.clone(), r.a)];
                for &c in &r.children {
                    let n = &nodes[c];
                    let name = &roster[n.u].name;
                    match units.iter_mut().find(|(u, _)| u == name) {
                        Some(e) => e.1 = rn(e.1 + n.a, 3),
                        None => units.push((name.clone(), rn(n.a, 3))),
                    }
                }
                let power = units.iter().fold(0.0, |s, (name, points)| {
                    s + points * unit(roster, name).map_or(0.0, |u| u.power)
                });
                Regiment {
                    id: i as u32,
                    state: state.id,
                    name: String::new(),
                    cell: r.cell,
                    at: [r.x, r.y],
                    base: [r.x, r.y],
                    naval: r.n,
                    units,
                    total: r.t,
                    power,
                    icon: String::new(),
                    note: String::new(),
                }
            })
            .collect();
        for k in 0..regiments.len() {
            let name = self.name(soc, &regiments[k], &regiments);
            regiments[k].name = name;
            regiments[k].icon = self.emblem(soc, &regiments[k]);
            regiments[k].note = self.note(soc, &regiments[k], state);
        }
        regiments
    }

    /// The name of a regiment (`getName`).
    fn name(&self, soc: &Society, r: &Regiment, all: &[Regiment]) -> String {
        let proper = if r.naval {
            None
        } else {
            let province = soc.cell_province[r.cell];
            if province != NONE {
                soc.provinces
                    .get(province as usize - 1)
                    .map(|p| p.name.clone())
            } else {
                self.burg[r.cell].map(|k| soc.places[k].name.clone())
            }
        };
        let number = nth(all
            .iter()
            .filter(|o| o.naval == r.naval && o.id < r.id)
            .count() as i64
            + 1);
        let form = if r.naval { "Fleet" } else { "Regiment" };
        match proper {
            Some(p) => format!("{number} ({p}) {form}"),
            None => format!("{number} {form}"),
        }
    }

    /// The default icon of a regiment (`getEmblem`).
    fn emblem(&self, soc: &Society, r: &Regiment) -> String {
        if !r.naval && r.units.is_empty() {
            return NEWBIE_ICON.to_owned();
        }
        let state = soc.states.get((r.state as usize).wrapping_sub(1));
        if !r.naval
            && state.is_some_and(|s| s.form == "Monarchy")
            && self.burg[r.cell].is_some_and(|k| soc.places[k].capital)
        {
            return ROYAL_ICON.to_owned();
        }
        // The unit with the most troops; the first of equals.
        let main = r
            .units
            .iter()
            .fold(None::<&(String, f64)>, |best, u| match best {
                Some(b) if b.1 >= u.1 => Some(b),
                _ => Some(u),
            })
            .map(|u| u.0.as_str());
        main.and_then(|name| unit(self.input.units, name))
            .map_or(DEFAULT_ICON, |u| u.icon.as_str())
            .to_owned()
    }

    /// The legend of a regiment (`generateNote`).
    fn note(&mut self, soc: &Society, r: &Regiment, s: &State) -> String {
        let base = match self.burg[r.cell] {
            Some(k) => Some(soc.places[k].name.clone()),
            None => {
                let province = soc.cell_province[r.cell];
                (province != NONE)
                    .then(|| soc.provinces.get(province as usize - 1))
                    .flatten()
                    .map(|p| p.full_name.clone())
            }
        };
        let station = match base {
            Some(b) => format!(
                "{} is {} in {b}. ",
                r.name,
                if r.naval { "based" } else { "stationed" }
            ),
            None => String::new(),
        };
        let troops_text = if r.total != 0.0 {
            let lines: Vec<String> = r
                .units
                .iter()
                .map(|(u, a)| format!("- {u}: {}", troops(*a)))
                .collect();
            format!(
                "\r\n\r\nRegiment composition in {} {}:\r\n{}.",
                YEAR,
                self.era_short,
                lines.join("\r\n")
            )
        } else {
            String::new()
        };
        let campaign = if s.campaigns.is_empty() {
            None
        } else {
            Some(self.rng.pick(&s.campaigns).clone())
        };
        let year = match &campaign {
            Some(c) => self.rng.rand(c.start as i64, c.end.unwrap_or(YEAR) as i64) as f64,
            None => self.rng.gauss(YEAR - 100.0, 150.0, 1.0, YEAR - 6.0, 0),
        };
        let conflict = campaign.map_or(String::new(), |c| format!(" during the {}", c.name));
        format!(
            "Regiment was formed in {year} {}{conflict}. {station}{troops_text}",
            self.input.era
        )
    }

    /// Move the regiments of the states at war to the nearest free cell of
    /// their foremost line, fleets first (`deployToFronts`).
    fn deploy_to_fronts(&self, soc: &mut Society) {
        let grid = self.input.grid;
        let cells = &soc.cells;
        let feature = &self.input.features.of_cell;
        let water = |c: usize| cells.h[c] < 20.0;
        // By state: the land border and the water next to either side.
        let mut borders: BTreeMap<u32, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
        let mut order: Vec<u32> = Vec::new();
        for c in 0..grid.len() {
            let s = soc.cell_state[c];
            if s == NONE || water(c) {
                continue;
            }
            let relations = &soc.states[s as usize - 1].diplomacy;
            if !relations.contains(&Relation::Enemy) {
                continue;
            }
            let enemy = |n: usize| {
                !water(n) && relations.get(soc.cell_state[n] as usize) == Some(&Relation::Enemy)
            };
            let enemies: Vec<usize> = grid.neighbors(c).filter(|&n| enemy(n)).collect();
            if enemies.is_empty() {
                continue;
            }
            let border = borders.entry(s).or_insert_with(|| {
                order.push(s);
                (Vec::new(), Vec::new())
            });
            border.0.push(c);
            for n in std::iter::once(c).chain(enemies) {
                for w in grid.neighbors(n) {
                    if water(w) && !border.1.contains(&w) {
                        border.1.push(w);
                    }
                }
            }
        }
        let deepen = |border: &[usize], fits: &dyn Fn(usize) -> bool| -> Vec<Vec<usize>> {
            let mut lines = vec![border.to_vec()];
            let mut seen: std::collections::BTreeSet<usize> = border.iter().copied().collect();
            for depth in 1..=FRONT_DEPTH {
                let mut line = Vec::new();
                for &i in &lines[depth - 1] {
                    for n in grid.neighbors(i) {
                        if fits(n) && seen.insert(n) {
                            line.push(n);
                        }
                    }
                }
                lines.push(line);
            }
            lines
        };
        let mut fronts: BTreeMap<u32, (Lines, Lines)> = BTreeMap::new();
        for s in &order {
            let (land, sea) = &borders[s];
            let st = *s;
            let land = deepen(land, &|n| soc.cell_state[n] == st && !water(n));
            let sea = deepen(sea, &|n| water(n));
            fronts.insert(st, (land, sea));
        }
        let width = REGIMENT_SIZE * 9.0;
        let mut placed: Vec<[f64; 2]> = Vec::new();
        let blocked = |placed: &[[f64; 2]], x: f64, y: f64| {
            placed.iter().any(|o| {
                let (dx, dy) = (x - o[0], y - o[1]);
                dx * dx + dy * dy < width * width
                    && dx.abs() < width
                    && dy.abs() < REGIMENT_SIZE * 3.0
            })
        };
        let points = &grid.points;
        let deploy = |placed: &mut Vec<[f64; 2]>,
                      r: &mut Regiment,
                      lines: &[Vec<usize>],
                      feature_id: Option<u32>| {
            let Some(feature_id) = feature_id else {
                return;
            };
            let [x, y] = r.at;
            let distance = |i: usize| (points[i][0] - x).powi(2) + (points[i][1] - y).powi(2);
            for line in lines {
                let mut fit: Vec<usize> = line
                    .iter()
                    .copied()
                    .filter(|&i| feature[i] == feature_id)
                    .collect();
                fit.sort_by(|&a, &b| distance(a).total_cmp(&distance(b)));
                if let Some(t) = fit
                    .into_iter()
                    .find(|&i| !blocked(placed, points[i][0], points[i][1]))
                {
                    r.at = points[t];
                    placed.push(r.at);
                    return;
                }
            }
        };
        let havens: Vec<Option<usize>> = cells.haven.clone();
        for naval in [true, false] {
            for state in soc.states.iter_mut() {
                let Some((land, sea)) = fronts.get(&state.id) else {
                    continue;
                };
                for r in state.military.iter_mut() {
                    if r.naval != naval {
                        continue;
                    }
                    if naval {
                        let f = havens[r.cell].map(|h| feature[h]);
                        deploy(&mut placed, r, sea, f);
                    } else {
                        let f = Some(feature[r.cell]);
                        deploy(&mut placed, r, land, f);
                    }
                }
            }
        }
    }
}

/// Raise the forces of every state (`Military.generate`): the war alert
/// again, then the regiments.
pub fn generate(input: &Input, soc: &mut Society) {
    let alerts: Vec<f64> = soc
        .states
        .iter()
        .map(|s| crate::states::alert(s, &soc.states))
        .collect();
    for (s, a) in soc.states.iter_mut().zip(alerts) {
        s.alert = a;
    }
    let mut burg = vec![None; input.grid.len()];
    for (k, p) in soc.places.iter().enumerate() {
        burg[p.cell] = Some(k);
    }
    let mut g = Gen {
        input,
        burg,
        rng: Rand::new(SeedTree::new(input.config.seed).part_rng(Layer::Military, "generate")),
        era_short: crate::markers::era_short(input.era),
    };
    let drafts = g.draft(soc);
    for (st, platoons) in drafts.into_iter().enumerate() {
        let military = g.form(soc, st, platoons);
        soc.states[st].military = military;
    }
    g.deploy_to_fronts(soc);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn ordinals_follow_fmg() {
        let got: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 101, 111, 112]
            .iter()
            .map(|&n| nth(n))
            .collect();
        assert_eq!(
            got,
            [
                "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "101st",
                "111th", "112th"
            ]
        );
    }

    #[test]
    fn troops_round_up_the_people() {
        assert_eq!(troops(0.0), 0.0);
        assert_eq!(troops(1.2345), 1235.0);
        assert_eq!(troops(0.0001), 1.0);
    }

    #[test]
    fn the_modifiers_follow_the_fmg_tables() {
        assert_eq!(state_modifier("mounted", "Nomadic"), Some(2.3));
        assert_eq!(state_modifier("naval", "Naval"), Some(1.8));
        assert_eq!(state_modifier("levies", "Naval"), None);
        assert_eq!(state_modifier("melee", "Generic"), None);
        assert_eq!(terrain_modifier(false, "highland", "machinery"), Some(3.0));
        assert_eq!(terrain_modifier(true, "wetland", "mounted"), Some(0.2));
        assert_eq!(terrain_modifier(true, "generic", "melee"), None);
        assert_eq!(terrain(3, 10.0), "nomadic");
        assert_eq!(terrain(12, 80.0), "wetland");
        assert_eq!(terrain(6, 70.0), "highland");
        assert_eq!(terrain(6, 69.0), "generic");
    }

    #[test]
    fn the_roster_goods_are_in_the_catalogue() {
        let goods = crate::economy::goods::catalogue();
        for u in &default_units() {
            for name in &u.goods {
                assert!(goods.iter().any(|g| g.name == *name), "{name}");
            }
        }
    }

    #[test]
    fn config_entries_change_add_and_remove_units() {
        use crate::config::{UnitEntry, UnitKind};
        let e = |name: &str| UnitEntry {
            name: name.to_owned(),
            ..UnitEntry::default()
        };
        let list = units_from(&[
            UnitEntry {
                power: Some(3.0),
                ..e("archers")
            },
            UnitEntry {
                remove: true,
                ..e("artillery")
            },
            UnitEntry {
                kind: Some(UnitKind::Mounted),
                rural: Some(1.5),
                ..e("camel riders")
            },
        ])
        .unwrap();
        assert_eq!(list.len(), 6);
        assert_eq!(unit(&list, "archers").unwrap().power, 3.0);
        assert!(unit(&list, "artillery").is_none());
        let new = unit(&list, "camel riders").unwrap();
        assert_eq!((new.kind.as_str(), new.rural), ("mounted", 1.5));
        let err = units_from(&[e("pikemen")]).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::TableInvalid);
        let err = units_from(&[UnitEntry {
            crew: Some(0.0),
            ..e("fleet")
        }])
        .unwrap_err();
        assert_eq!(err.path, "/world/military_units/0/crew");
    }

    #[test]
    fn a_world_raises_the_units_of_its_config() {
        use crate::config::UnitEntry;
        let mut c: Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 7, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "continents", "states": 6 }
        }))
        .unwrap();
        // Only levies and a new ranged unit.
        let mut entries: Vec<UnitEntry> = default_units()
            .iter()
            .filter(|u| u.kind != "levies")
            .map(|u| UnitEntry {
                name: u.name.clone(),
                remove: true,
                ..UnitEntry::default()
            })
            .collect();
        entries.push(UnitEntry {
            name: "slingers".to_owned(),
            kind: Some(crate::config::UnitKind::Ranged),
            rural: Some(0.3),
            urban: Some(0.3),
            ..UnitEntry::default()
        });
        c.world.military_units = entries;
        let w = World::build(&c).unwrap();
        let names: Vec<&str> = w.military_units.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["levies", "slingers"]);
        let mut raised = std::collections::BTreeSet::new();
        for s in &w.society.states {
            for r in &s.military {
                for (u, _) in &r.units {
                    raised.insert(u.clone());
                }
            }
        }
        assert!(raised.contains("slingers"), "{raised:?}");
        assert!(
            raised.iter().all(|u| u == "levies" || u == "slingers"),
            "{raised:?}"
        );
    }

    fn world(seed: u64) -> World {
        let c: Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": seed, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "continents", "states": 6 }
        }))
        .unwrap();
        World::build(&c).unwrap()
    }

    #[test]
    fn each_state_raises_named_regiments_with_a_note() {
        let w = world(7);
        let soc = &w.society;
        let mut any = false;
        for s in &soc.states {
            let mut seen = std::collections::BTreeSet::new();
            for (k, r) in s.military.iter().enumerate() {
                any = true;
                assert_eq!(r.id, k as u32);
                assert_eq!(r.state, s.id);
                assert!(r.total > 0.0);
                let sum = r.units.iter().fold(0.0, |t, u| t + u.1);
                assert!((sum - r.total).abs() < 0.01, "{sum} {}", r.total);
                assert!(seen.insert(r.name.clone()), "{}", r.name);
                assert!(r.name.ends_with(if r.naval { "Fleet" } else { "Regiment" }));
                assert!(r.note.starts_with("Regiment was formed in "));
                assert!(!r.icon.is_empty());
                assert!(!r.note.contains('\u{2014}'));
                for (u, _) in &r.units {
                    assert!(unit(&w.military_units, u).is_some());
                }
                if r.naval {
                    assert!(r.units.iter().all(|(u, _)| *u == "fleet"));
                }
            }
            // The biggest regiments come first.
            for w in s.military.windows(2) {
                assert!(w[0].total >= w[1].total - 1e-9);
            }
        }
        assert!(any);
    }

    #[test]
    fn only_states_at_war_move_their_regiments_to_their_fronts() {
        // A war comes on some seeds only, so the test looks at several.
        let mut moved = 0;
        for seed in 0..8 {
            let w = world(seed);
            for s in &w.society.states {
                let war = s.diplomacy.contains(&Relation::Enemy);
                for r in &s.military {
                    if r.at == r.base {
                        continue;
                    }
                    assert!(war, "{}", r.name);
                    moved += 1;
                    let cell = (0..w.grid.len())
                        .find(|&c| w.grid.points[c] == r.at)
                        .expect("a deployed regiment stands on a cell site");
                    let land = w.society.cells.h[cell] >= 20.0;
                    assert_eq!(land, !r.naval, "{}", r.name);
                    if land {
                        assert_eq!(w.society.cell_state[cell], s.id);
                    }
                }
            }
        }
        assert!(moved > 0);
    }

    #[test]
    fn the_upkeep_is_a_share() {
        let w = world(11);
        for s in &w.society.states {
            let u = upkeep(s, &w.economy);
            assert!((0.0..=1.0).contains(&u), "{u}");
            assert!(battle_upkeep(s, &w.economy) >= MIN_UPKEEP);
            assert!(power(s, &w.economy) <= nominal_power(s) + 1e-9);
        }
    }
}
