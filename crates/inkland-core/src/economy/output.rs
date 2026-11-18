//! What the land yields (`output.ts`): rural output, deposits, and the
//! multipliers of a good at a cell.

use super::goods::Good;
use crate::cultures::{Culture, CultureType};
use crate::grid::Grid;
use crate::population::Cells;
use crate::society::Place;

/// The share of the rural output that reaches the market.
const RURAL_OUTPUT_FACTOR: f64 = 0.9;
/// Units a day for each population point of the cell of a deposit, on a map
/// of `DEPOSIT_MAP_CELLS` cells.
pub const DEPOSIT_RICHNESS: f64 = 2.2;
/// The deposit counts are for each map, so their yield grows with the cells.
const DEPOSIT_MAP_CELLS: f64 = 5000.0;
/// The yield of a deposit stops to grow past this population.
const DEPOSIT_MAX_POPULATION: f64 = 10.0;
/// Development past this makes more of each good.
const PRODUCTIVITY_START: f64 = 30.0;

/// What the output reads of the world.
pub struct Land<'a> {
    pub grid: &'a Grid,
    pub cells: &'a Cells,
    pub biomes: &'a [u8],
    pub development: &'a [u8],
    pub cell_culture: &'a [u32],
    pub cultures: &'a [Culture],
    pub places: &'a [Place],
    /// The place id on each cell, or 0.
    pub cell_burg: &'a [u32],
    /// The market of each cell, or 0.
    pub cell_market: &'a [u32],
    pub cell_state: &'a [u32],
}

impl Land<'_> {
    /// The development of a cell; a water cell, worked from the shore,
    /// takes the best of the land around (`Development.ofCell`).
    pub fn development(&self, cell: usize) -> f64 {
        if self.cells.h[cell] >= 20.0 {
            return f64::from(self.development[cell]);
        }
        let mut level = 0u8;
        for m in self.grid.neighbors(cell) {
            if self.cells.h[m] >= 20.0 {
                level = level.max(self.development[m]);
            }
        }
        f64::from(level)
    }

    /// The multipliers and the development on the good at the cell; 0
    /// beyond its technology (`getModifiers`).
    pub fn modifiers(&self, good: &Good, cell: usize) -> f64 {
        let development = self.development(cell);
        if good.development != 0 && development < f64::from(good.development) {
            return 0.0;
        }
        let productivity = (development / PRODUCTIVITY_START).max(1.0);
        if good.culture_type.is_empty() {
            return productivity;
        }
        let burg = self.cell_burg[cell];
        let kind = if burg != 0 {
            self.places[burg as usize - 1].ptype
        } else {
            let culture = self.cell_culture[cell];
            (culture as usize)
                .checked_sub(1)
                .and_then(|k| self.cultures.get(k))
                .map_or(CultureType::Generic, |c| c.kind)
        };
        good.culture_factor(kind) * productivity
    }

    /// The people of a land cell, or of the land around a water cell.
    fn population(&self, cell: usize) -> f64 {
        if self.cells.h[cell] >= 20.0 {
            return self.cells.pop[cell];
        }
        self.grid
            .neighbors(cell)
            .fold(0.0, |s, m| s + self.cells.pop[m])
    }

    /// Units a day of a deposit, capped by people; more on bigger maps
    /// (`getDeposit`).
    pub fn deposit(&self, good: &Good, cell: usize) -> f64 {
        let pop = self.population(cell).min(DEPOSIT_MAX_POPULATION);
        let scale = self.grid.len() as f64 / DEPOSIT_MAP_CELLS;
        pop * DEPOSIT_RICHNESS * good.deposit_yield * scale * self.modifiers(good, cell)
    }

    /// The rural output of each cell a day: good id and units, by good id
    /// (`getRural`).
    pub fn rural(&self, goods: &[Good]) -> Vec<Vec<(u32, f64)>> {
        // The goods that each biome yields, in catalogue order.
        let mut by_biome: Vec<Vec<(usize, f64)>> = vec![Vec::new(); 256];
        for (k, g) in goods.iter().enumerate() {
            for &(biome, production) in &g.biome_output {
                if production != 0.0 {
                    by_biome[biome as usize].push((k, production));
                }
            }
        }
        (0..self.grid.len())
            .map(|cell| self.cell(goods, &by_biome, cell))
            .collect()
    }

    fn cell(&self, goods: &[Good], by_biome: &[Vec<(usize, f64)>], cell: usize) -> Vec<(u32, f64)> {
        let mut produced: Vec<(u32, f64)> = Vec::new();
        let mut add =
            |id: u32, amount: f64, factor: f64| match produced.iter_mut().find(|(g, _)| *g == id) {
                Some(entry) => entry.1 = crate::rand::rn(entry.1 + amount * factor, 2),
                None => produced.push((id, crate::rand::rn(amount * factor, 2))),
            };
        let pop = self.population(cell);
        if pop > 0.0 {
            for &(k, production) in &by_biome[self.biomes[cell] as usize] {
                let g = &goods[k];
                add(
                    g.id,
                    pop * production * self.modifiers(g, cell),
                    RURAL_OUTPUT_FACTOR,
                );
            }
            let deposit = self.cells.good[cell] as usize;
            if let Some(g) = deposit.checked_sub(1).and_then(|k| goods.get(k)) {
                add(g.id, self.deposit(g, cell), 1.0);
            }
        }
        // FMG keys the output by good id: the ids go in order.
        produced.sort_by_key(|&(id, _)| id);
        produced
    }

    /// The land cell whose people work the output of a cell: its own, or a
    /// shore cell of the same market (`getWorkingCell`).
    pub fn working_cell(&self, cell: usize) -> usize {
        let h = &self.cells.h;
        if h[cell] >= 20.0 {
            return cell;
        }
        let shore: Vec<usize> = self
            .grid
            .neighbors(cell)
            .filter(|&m| h[m] >= 20.0)
            .collect();
        shore
            .iter()
            .copied()
            .find(|&m| self.cell_market[m] == self.cell_market[cell])
            .or_else(|| shore.first().copied())
            .unwrap_or(cell)
    }
}
