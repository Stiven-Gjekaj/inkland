//! Goods and their deposits (`goods-generator.ts`, `Goods.generate`).
//!
//! - The catalogue is the default table of FMG (`goods_data.rs`).
//! - Each good with a distribution gets a number of deposits in its range.
//!   The goods go in a random order. The first deposit of a good lands on a
//!   random cell where the distribution is true; each next deposit lands
//!   next to the one before with the chance of the clustering of the good.
//! - A cell holds at most one deposit. Glaciers get none.
//! - A distribution is an expression of the FMG spread functions (see
//!   `Goods-spread-functions.md`). `Distribution::parse` reads it, and
//!   `Distribution::fits` evaluates it with the short circuits of
//!   JavaScript, so it draws the random numbers in the FMG order.

use super::demand::Need;
use super::goods_data::{GOODS, RawGood};
use crate::cultures::CultureType;
use crate::features::Features;
use crate::grid::Grid;
use crate::population::Cells;
use crate::rand::Rand;

/// The chance that a deposit lands next to the one before it
/// (`DEFAULT_CLUSTERING`).
pub const DEFAULT_CLUSTERING: f64 = 0.5;

/// One good of the catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct Good {
    /// 1, 2, 3, ... in catalogue order.
    pub id: u32,
    pub name: String,
    pub tags: Vec<String>,
    pub icon: String,
    pub color: String,
    /// The base price.
    pub value: f64,
    /// The price to carry one unit over a decima.
    pub carriage: f64,
    pub unit: String,
    /// The deposits on the map: minimum and maximum.
    pub deposits: Option<[f64; 2]>,
    /// The multiplier on the output of each deposit (`depositYield`).
    pub deposit_yield: f64,
    /// The chance that a deposit lands next to the one before it.
    pub clustering: f64,
    /// The spread expression of the deposits.
    pub distribution: Option<String>,
    /// The least development that a cell needs to make the good; 0 for none.
    pub development: u8,
    /// Units for each rural population point, by biome.
    pub biome_output: Vec<(u8, f64)>,
    /// Alternative input sets: good id and amount, by good id. Each recipe
    /// makes one unit.
    pub recipes: Vec<Vec<(u32, f64)>>,
    /// How much of each need one unit covers, in `Need` order.
    pub coverage: [f64; 5],
    /// The production factors by culture type (`multipliers.cultureType`).
    pub culture_type: Vec<(CultureType, f64)>,
}

impl Good {
    /// `true` if the good covers a need.
    pub fn covers_needs(&self) -> bool {
        self.coverage.iter().any(|&c| c > 0.0)
    }

    /// The multiplier for a culture type; 1 if the good names none.
    pub fn culture_factor(&self, kind: CultureType) -> f64 {
        self.culture_type
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(1.0, |&(_, v)| v)
    }

    /// The coverage of one need.
    pub fn covers(&self, need: Need) -> f64 {
        self.coverage[need.index()]
    }
}

fn resolve(raw: &RawGood, id: u32) -> Good {
    let mut coverage = [0.0; 5];
    for &(need, v) in raw.coverage {
        coverage[need.index()] = v;
    }
    // FMG keys a recipe by good id, so its ingredients go in id order.
    let recipes = raw
        .recipes
        .iter()
        .map(|r| {
            let mut list: Vec<(u32, f64)> = r
                .iter()
                .map(|&(name, amount)| {
                    let k = GOODS
                        .iter()
                        .position(|g| g.name == name)
                        .expect("a recipe names a good of the catalogue");
                    (k as u32 + 1, amount)
                })
                .collect();
            list.sort_by_key(|&(id, _)| id);
            list
        })
        .collect();
    Good {
        id,
        name: raw.name.to_owned(),
        tags: raw.tags.iter().map(|t| (*t).to_owned()).collect(),
        icon: raw.icon.to_owned(),
        color: raw.color.to_owned(),
        value: raw.value,
        carriage: raw.carriage,
        unit: raw.unit.to_owned(),
        deposits: raw.deposits,
        deposit_yield: 1.0,
        clustering: DEFAULT_CLUSTERING,
        distribution: raw.distribution.map(str::to_owned),
        development: raw.development,
        biome_output: raw.biome_output.to_vec(),
        recipes,
        coverage,
        culture_type: raw.culture_type.to_vec(),
    }
}

/// The default catalogue. The good with id `k` is at index `k - 1`.
pub fn catalogue() -> Vec<Good> {
    GOODS
        .iter()
        .enumerate()
        .map(|(k, raw)| resolve(raw, k as u32 + 1))
        .collect()
}

/// The value of each good by id; entry 0 is 0.
pub fn values(goods: &[Good]) -> Vec<f64> {
    std::iter::once(0.0)
        .chain(goods.iter().map(|g| g.value))
        .collect()
}

/// A spread function of a distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Func {
    Random,
    Nth,
    MinPop,
    MaxPop,
    MinHabitability,
    Habitability,
    Elevation,
    Biome,
    MinHeight,
    MaxHeight,
    MinTemp,
    MaxTemp,
    Shore,
    Type,
    River,
}

impl Func {
    fn of(name: &str) -> Option<Func> {
        Some(match name {
            "random" => Func::Random,
            "nth" => Func::Nth,
            "minPop" => Func::MinPop,
            "maxPop" => Func::MaxPop,
            "minHabitability" => Func::MinHabitability,
            "habitability" => Func::Habitability,
            "elevation" => Func::Elevation,
            "biome" => Func::Biome,
            "minHeight" => Func::MinHeight,
            "maxHeight" => Func::MaxHeight,
            "minTemp" => Func::MinTemp,
            "maxTemp" => Func::MaxTemp,
            "shore" => Func::Shore,
            "type" => Func::Type,
            "river" => Func::River,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Arg {
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Call(Func, Vec<Arg>),
}

/// A parsed distribution expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Distribution {
    expr: Expr,
}

/// The data of one cell that the spread functions read.
pub struct CellView<'a> {
    pub id: usize,
    /// The FMG height: 20 is sea level.
    pub h: f64,
    pub biome: u8,
    pub habitability: f64,
    /// The temperature in degrees C.
    pub temp: f64,
    /// The distance to the coast (`cells.t`).
    pub t: i32,
    /// The feature type: `ocean`, `lake`, or `island`.
    pub kind: &'a str,
    /// The feature subtype (the group).
    pub group: &'a str,
    pub river: bool,
    /// Rural population. None yet on a new map.
    pub pop: f64,
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn skip(&mut self) {
        while self.at < self.s.len() && self.s[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }

    fn eat(&mut self, token: &str) -> bool {
        self.skip();
        if self.s[self.at..].starts_with(token.as_bytes()) {
            self.at += token.len();
            true
        } else {
            false
        }
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut left = self.and()?;
        while self.eat("||") {
            left = Expr::Or(Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut left = self.unary()?;
        while self.eat("&&") {
            left = Expr::And(Box::new(left), Box::new(self.unary()?));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat("!") {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if self.eat("(") {
            let e = self.or()?;
            if !self.eat(")") {
                return Err(format!("no ) at {}", self.at));
            }
            return Ok(e);
        }
        self.call()
    }

    fn call(&mut self) -> Result<Expr, String> {
        self.skip();
        let start = self.at;
        while self.at < self.s.len() && self.s[self.at].is_ascii_alphabetic() {
            self.at += 1;
        }
        let name = std::str::from_utf8(&self.s[start..self.at]).unwrap_or("");
        let func = Func::of(name).ok_or_else(|| format!("unknown function {name:?}"))?;
        if !self.eat("(") {
            return Err(format!("no ( after {name}"));
        }
        let mut args = Vec::new();
        if !self.eat(")") {
            loop {
                args.push(self.arg()?);
                if self.eat(")") {
                    break;
                }
                if !self.eat(",") {
                    return Err(format!("no , or ) at {}", self.at));
                }
            }
        }
        Ok(Expr::Call(func, args))
    }

    fn arg(&mut self) -> Result<Arg, String> {
        self.skip();
        let quote = self.s.get(self.at).copied();
        if quote == Some(b'"') || quote == Some(b'\'') {
            let q = quote.unwrap_or(b'"');
            let start = self.at + 1;
            let end = self.s[start..]
                .iter()
                .position(|&c| c == q)
                .ok_or("an open string")?
                + start;
            self.at = end + 1;
            return Ok(Arg::Text(
                String::from_utf8_lossy(&self.s[start..end]).into_owned(),
            ));
        }
        let start = self.at;
        while self.at < self.s.len()
            && (self.s[self.at].is_ascii_digit() || b"-+.".contains(&self.s[self.at]))
        {
            self.at += 1;
        }
        std::str::from_utf8(&self.s[start..self.at])
            .ok()
            .and_then(|t| t.parse::<f64>().ok())
            .map(Arg::Number)
            .ok_or_else(|| format!("a bad argument at {start}"))
    }
}

impl Distribution {
    /// Read a distribution expression. The error tells what is wrong.
    pub fn parse(text: &str) -> Result<Distribution, String> {
        let mut p = Parser {
            s: text.as_bytes(),
            at: 0,
        };
        let expr = p.or()?;
        p.skip();
        if p.at != p.s.len() {
            return Err(format!("unexpected text at {}", p.at));
        }
        Ok(Distribution { expr })
    }

    /// `true` if the good can go on the cell. The random spread functions
    /// draw from `rng` as FMG draws from `Math.random`.
    pub fn fits(&self, cell: &CellView, rng: &mut Rand) -> bool {
        eval(&self.expr, cell, rng)
    }
}

fn number(args: &[Arg], k: usize) -> f64 {
    match args.get(k) {
        Some(Arg::Number(v)) => *v,
        // JavaScript makes NaN of a missing number: each test is false.
        _ => f64::NAN,
    }
}

fn eval(e: &Expr, c: &CellView, rng: &mut Rand) -> bool {
    match e {
        Expr::Or(a, b) => eval(a, c, rng) || eval(b, c, rng),
        Expr::And(a, b) => eval(a, c, rng) && eval(b, c, rng),
        Expr::Not(a) => !eval(a, c, rng),
        Expr::Call(f, args) => {
            let n = number(args, 0);
            let has = |v: f64| args.iter().any(|a| matches!(a, Arg::Number(x) if *x == v));
            match f {
                Func::Random => n >= 100.0 || (n > 0.0 && n / 100.0 > rng.unit()),
                Func::Nth => {
                    // `!(cellId % number)`: true for 0, and for NaN.
                    let r = (c.id as f64) % n;
                    r == 0.0 || r.is_nan()
                }
                Func::MinPop => c.pop >= n,
                Func::MaxPop => c.pop <= n,
                Func::MinHabitability => c.habitability >= n,
                Func::Habitability => c.habitability > rng.unit() * 100.0,
                Func::Elevation => c.h / 100.0 > rng.unit(),
                Func::Biome => has(f64::from(c.biome)),
                Func::MinHeight => c.h >= n,
                Func::MaxHeight => c.h <= n,
                Func::MinTemp => c.temp >= n,
                Func::MaxTemp => c.temp <= n,
                Func::Shore => has(f64::from(c.t)),
                Func::Type => args
                    .iter()
                    .any(|a| matches!(a, Arg::Text(t) if t == c.kind || (!c.group.is_empty() && t == c.group))),
                Func::River => c.river,
            }
        }
    }
}

/// What the deposits read.
pub struct DepositInput<'a> {
    pub grid: &'a Grid,
    /// The cell data before `rankCells`: no population yet.
    pub cells: &'a Cells,
    pub features: &'a Features,
    pub biomes: &'a [u8],
    pub temperature: &'a [f64],
}

/// The deposit count of a good: a whole number in its range, with the ends
/// rounded up (`getDepositCount`). It draws a number only for a good with a
/// distribution and a range.
fn deposit_count(good: &Good, rng: &mut Rand) -> usize {
    match (&good.distribution, good.deposits) {
        (Some(_), Some([lo, hi])) => {
            let (lo, hi) = (lo.ceil(), hi.ceil());
            (lo + (rng.unit() * (hi - lo + 1.0)).floor()) as usize
        }
        _ => 0,
    }
}

/// The good of each cell: a good id, or 0 (`Goods.generate`, `pack.cells.good`).
pub fn deposits(input: &DepositInput, goods: &[Good], rng: &mut Rand) -> Vec<u16> {
    let DepositInput {
        grid,
        cells,
        features,
        biomes,
        temperature,
    } = *input;
    let n = grid.len();
    let mut good = vec![0u16; n];
    let view = |c: usize| CellView {
        id: c,
        h: cells.h[c],
        biome: biomes[c],
        habitability: crate::biomes::HABITABILITY[biomes[c] as usize],
        temp: temperature[c],
        t: cells.t[c],
        kind: features.get(features.of_cell[c]).kind.name(),
        group: cells.group[c],
        river: cells.r[c] != 0,
        pop: 0.0,
    };
    let glacier = crate::biomes::HABITABILITY[crate::biomes::GLACIER as usize] == 0.0;

    // The goods in a random order (d3 `shuffler`).
    let mut order: Vec<usize> = (0..goods.len()).collect();
    shuffle(&mut order, rng);
    for k in order {
        let g = &goods[k];
        let count = deposit_count(g, rng);
        let Some(text) = g.distribution.as_deref() else {
            continue;
        };
        // A rule that does not parse places nothing, as in FMG.
        let Ok(dist) = Distribution::parse(text) else {
            continue;
        };
        let free = |good: &[u16], c: usize| {
            good[c] == 0 && !(glacier && biomes[c] == crate::biomes::GLACIER)
        };
        let fits =
            |good: &[u16], c: usize, rng: &mut Rand| free(good, c) && dist.fits(&view(c), rng);

        // The cells that fit, listed once random draws keep missing.
        let mut eligible: Option<Vec<usize>> = None;
        let mut placed = 0;
        let mut previous: Option<usize> = None;
        while placed < count {
            let mut cell = None;
            if let Some(p) = previous
                && rng.unit() < g.clustering
            {
                let near: Vec<usize> = grid.neighbors(p).filter(|&m| fits(&good, m, rng)).collect();
                if !near.is_empty() {
                    cell = Some(near[(rng.unit() * near.len() as f64) as usize]);
                }
            }
            if cell.is_none() {
                // `pickAnywhere`.
                if eligible.is_none() {
                    for _ in 0..100 {
                        let c = (rng.unit() * n as f64) as usize;
                        if fits(&good, c, rng) {
                            cell = Some(c);
                            break;
                        }
                    }
                    if cell.is_none() {
                        eligible = Some((0..n).filter(|&c| fits(&good, c, rng)).collect());
                    }
                }
                if cell.is_none()
                    && let Some(list) = eligible.as_mut()
                {
                    while !list.is_empty() {
                        let index = (rng.unit() * list.len() as f64) as usize;
                        let c = list.swap_remove(index);
                        if free(&good, c) {
                            cell = Some(c);
                            break;
                        }
                    }
                }
            }
            let Some(c) = cell else {
                break;
            };
            good[c] = g.id as u16;
            placed += 1;
            previous = Some(c);
        }
    }
    good
}

/// Shuffle in place as d3 `shuffler` does: from the end, each item swaps
/// with a random item before it.
pub fn shuffle<T>(items: &mut [T], rng: &mut Rand) {
    let mut m = items.len();
    while m > 0 {
        let i = (rng.unit() * m as f64) as usize;
        m -= 1;
        items.swap(m, i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::{Layer, SeedTree};

    fn rng() -> Rand {
        Rand::new(SeedTree::new(1).part_rng(Layer::Economy, "test"))
    }

    fn cell<'a>(kind: &'a str, group: &'a str) -> CellView<'a> {
        CellView {
            id: 10,
            h: 45.0,
            biome: 6,
            habitability: 100.0,
            temp: 12.0,
            t: 1,
            kind,
            group,
            river: true,
            pop: 0.0,
        }
    }

    #[test]
    fn the_catalogue_has_the_fmg_goods_and_ids() {
        let goods = catalogue();
        assert_eq!(goods.len(), 71);
        assert_eq!(goods[0].name, "Wood");
        assert_eq!(goods[70].name, "Perfume");
        for (k, g) in goods.iter().enumerate() {
            assert_eq!(g.id as usize, k + 1);
            for r in &g.recipes {
                assert!(r.windows(2).all(|w| w[0].0 < w[1].0), "{}", g.name);
            }
            if let Some(d) = &g.distribution {
                assert!(Distribution::parse(d).is_ok(), "{}: {d}", g.name);
            }
        }
        // Books: Paper (49), Ink (50), Tools (58) by id.
        let books = &goods[50];
        assert_eq!(books.name, "Books");
        assert_eq!(books.recipes[0], vec![(49, 1.0), (50, 0.5), (58, 0.2)]);
        assert_eq!(books.recipes[1], vec![(43, 1.0), (50, 0.5), (58, 0.2)]);
    }

    #[test]
    fn a_distribution_follows_javascript_logic() {
        let mut r = rng();
        let c = cell("island", "continent");
        let t = |s: &str, r: &mut Rand| Distribution::parse(s).unwrap().fits(&c, r);
        assert!(t("biome(5, 6, 7)", &mut r));
        assert!(!t("biome(1)", &mut r));
        assert!(t("minHeight(40) && river()", &mut r));
        assert!(!t("!river()", &mut r));
        assert!(t("nth(5)", &mut r) && !t("nth(3)", &mut r));
        assert!(t("shore(1, 2) && minTemp(8) && maxTemp(12)", &mut r));
        assert!(t("biome(1) || minHabitability(100)", &mut r));
        assert!(t("(biome(1) || biome(6)) && !shore(-1)", &mut r));
        assert!(t("random(100)", &mut r) && !t("random(0)", &mut r));
        assert!(Distribution::parse("minHeight(20) && elevation(25)").is_ok());
        assert!(Distribution::parse("bogus(1)").is_err());
        assert!(Distribution::parse("biome(1").is_err());
    }

    #[test]
    fn a_feature_matches_by_its_type_or_its_subtype() {
        let mut r = rng();
        let d = Distribution::parse("type('ocean', \"freshwater\")").unwrap();
        assert!(d.fits(&cell("ocean", "sea"), &mut r));
        assert!(d.fits(&cell("lake", "freshwater"), &mut r));
        assert!(!d.fits(&cell("lake", "salt"), &mut r));
        assert!(!d.fits(&cell("island", "isle"), &mut r));
    }

    #[test]
    fn a_short_circuit_draws_no_number() {
        // `biome(1) && random(50)` is false at the first test, so it draws
        // nothing: the stream stays where it was.
        let mut a = rng();
        let mut b = rng();
        let c = cell("island", "continent");
        assert!(
            !Distribution::parse("biome(1) && random(50)")
                .unwrap()
                .fits(&c, &mut a)
        );
        assert_eq!(a.unit(), b.unit());
    }

    #[test]
    fn a_count_is_whole_and_in_the_range_with_ends_rounded_up() {
        let mut r = rng();
        let mut g = catalogue()[0].clone();
        g.deposits = Some([1.2, 2.5]);
        for _ in 0..200 {
            let n = deposit_count(&g, &mut r);
            assert!((2..=3).contains(&n), "{n}");
        }
        g.distribution = None;
        assert_eq!(deposit_count(&g, &mut r), 0);
    }

    #[test]
    fn deposits_keep_their_counts_and_distributions() {
        let c: crate::config::Config = serde_json::from_value(serde_json::json!({
            "version": 1, "seed": 11, "frame": { "width": 400, "height": 300 },
            "world": { "cells": 4000, "template": "continents" }
        }))
        .unwrap();
        let w = crate::world::World::build(&c).unwrap();
        let cells = &w.society.cells;
        let goods = &w.economy.goods;
        let mut count = vec![0usize; goods.len() + 1];
        for (cell, &g) in cells.good.iter().enumerate() {
            count[g as usize] += 1;
            if g == 0 {
                continue;
            }
            assert_ne!(w.biomes[cell], crate::biomes::GLACIER);
            let good = &goods[g as usize - 1];
            let text = good.distribution.clone().unwrap();
            // A rule with no random function holds on each of its cells.
            if !["random", "elevation", "habitability"]
                .iter()
                .any(|f| text.contains(f))
            {
                let view = CellView {
                    id: cell,
                    h: cells.h[cell],
                    biome: w.biomes[cell],
                    habitability: crate::biomes::HABITABILITY[w.biomes[cell] as usize],
                    temp: w.temperature[cell],
                    t: cells.t[cell],
                    kind: w.features.get(w.features.of_cell[cell]).kind.name(),
                    group: cells.group[cell],
                    river: cells.r[cell] != 0,
                    pop: 0.0,
                };
                assert!(
                    Distribution::parse(&text).unwrap().fits(&view, &mut rng()),
                    "{}",
                    good.name
                );
            }
        }
        for g in goods {
            let max = g.deposits.map_or(0.0, |d| d[1].ceil()) as usize;
            assert!(count[g.id as usize] <= max, "{}", g.name);
        }
        assert!(count[1..].iter().sum::<usize>() > 20);
        // A deposit adds its value and 10 to the suitability of its cell.
        assert!(
            cells
                .good
                .iter()
                .enumerate()
                .any(|(c, &g)| g != 0 && cells.s[c] > 0.0)
        );
    }
}
