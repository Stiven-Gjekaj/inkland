//! Corruption by province (`corruption.ts`): the percent of the spending of
//! a state that officials pocket, varied by province and curbed by the
//! administration.
//!
//! - A province is more corrupt than its state where its capital is
//!   remote and where more of its people are strangers.
//! - The cells of a state outside any province count as one more province,
//!   governed from the capital. inkland gives each land cell a province,
//!   so this list is empty there.
//! - Administration curbs corruption as far as it is paid of what the
//!   people of the state need.

use super::finance::Expense;
use crate::society::{NONE, State};

/// Tenths of the map diagonal from the capital at which a province is
/// fully remote.
const REMOTE_DECIMAS: f64 = 2.0;
/// How much more corrupt a fully remote province is than its state.
const REMOTE_CORRUPTION: f64 = 0.5;
/// How much more corrupt a province of strangers only is than its state.
const DIVERSITY_CORRUPTION: f64 = 1.0;
/// The money of the administration a person a day that curbs corruption
/// fully.
const ADMINISTRATION_NEED: f64 = 0.1;
/// The share of corruption that a fully paid administration curbs.
const ADMINISTRATION_CURB: f64 = 0.5;

/// What sets the corruption of a province apart from its state
/// (`ProvinceCorruption`).
#[derive(Debug, Clone, PartialEq)]
pub struct ProvinceCorruption {
    /// The province id; 0 for the cells of the state outside any province.
    pub id: u32,
    /// The place id of the capital of the province, or of the state for the
    /// cells outside any province; 0 for none.
    pub burg: u32,
    /// Population points; urban people count whole.
    pub people: f64,
    /// The share of the people of the state.
    pub share: f64,
    /// The share outside the culture of the state and the faith of its
    /// capital, each counting half.
    pub diversity: f64,
    /// The distance of its capital from the capital of the state: 1 at
    /// `REMOTE_DECIMAS` or overseas.
    pub remoteness: f64,
    /// Its corruption against the state: above 1 where more remote or
    /// diverse than the mean.
    pub factor: f64,
}

/// The corruption data of all states.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Corruption {
    /// The provinces with people of each state, by state id, in the order of
    /// their first cell.
    pub by_state: Vec<Vec<ProvinceCorruption>>,
    /// The state id and the index in its list of each cell, if any.
    pub of_cell: Vec<Option<(u32, usize)>>,
}

/// What the corruption index reads.
pub struct Input<'a> {
    pub states: &'a [State],
    pub provinces: &'a [crate::society::Province],
    pub places: &'a [crate::society::Place],
    pub pop: &'a [f64],
    pub cell_state: &'a [u32],
    pub cell_province: &'a [u32],
    pub cell_culture: &'a [u32],
    pub cell_religion: &'a [u32],
    /// The place id on each cell, or 0.
    pub cell_burg: &'a [u32],
    /// The feature id of each cell.
    pub feature: &'a [u32],
    /// A tenth of the map diagonal.
    pub decima: f64,
}

impl Corruption {
    /// Index the provinces of each state (`index`).
    pub fn new(input: &Input) -> Corruption {
        let Input {
            states,
            provinces,
            places,
            pop,
            cell_state,
            cell_province,
            cell_culture,
            cell_religion,
            cell_burg,
            feature,
            decima,
        } = *input;
        let n = cell_state.len();
        let place = |id: u32| (id as usize).checked_sub(1).and_then(|k| places.get(k));
        let mut by_state: Vec<Vec<ProvinceCorruption>> = vec![Vec::new(); states.len() + 1];
        let mut positions: Vec<Vec<(u32, usize)>> = vec![Vec::new(); states.len() + 1];
        let mut strangers: Vec<Vec<f64>> = vec![Vec::new(); states.len() + 1];
        let mut faiths: Vec<Option<u32>> = vec![None; states.len() + 1];
        let mut of_cell = vec![None; n];
        for c in 0..n {
            let sid = cell_state[c];
            let Some(state) = (sid as usize).checked_sub(1).and_then(|k| states.get(k)) else {
                continue;
            };
            let s = sid as usize;
            let province = (cell_province[c] as usize)
                .checked_sub(1)
                .and_then(|k| provinces.get(k));
            let id = province.filter(|p| p.state == sid).map_or(NONE, |p| p.id);
            let index = match positions[s].iter().find(|(p, _)| *p == id) {
                Some(&(_, k)) => k,
                None => {
                    let k = by_state[s].len();
                    positions[s].push((id, k));
                    let seat = if id != NONE {
                        province.map_or(NONE, |p| p.seat)
                    } else {
                        NONE
                    };
                    let own = place(seat).is_some_and(|b| b.state == sid);
                    let capital = if own { seat } else { state.capital };
                    by_state[s].push(ProvinceCorruption {
                        id,
                        burg: capital,
                        people: 0.0,
                        share: 0.0,
                        diversity: 0.0,
                        remoteness: 0.0,
                        factor: 1.0,
                    });
                    strangers[s].push(0.0);
                    k
                }
            };
            of_cell[c] = Some((sid, index));
            let burg = cell_burg[c];
            let people = pop[c] + place(burg).map_or(0.0, |b| b.population);
            by_state[s][index].people += people;
            let faith = *faiths[s].get_or_insert_with(|| {
                place(state.capital).map_or(NONE, |b| cell_religion[b.cell])
            });
            if cell_culture[c] != state.culture {
                strangers[s][index] += people / 2.0;
            }
            if cell_religion[c] != faith {
                strangers[s][index] += people / 2.0;
            }
        }
        let reach = REMOTE_DECIMAS * decima;
        for (s, list) in by_state.iter_mut().enumerate().skip(1) {
            if list.is_empty() {
                continue;
            }
            let capital = place(states[s - 1].capital);
            let total = list.iter().fold(0.0, |t, d| t + d.people);
            for (k, d) in list.iter_mut().enumerate() {
                d.share = if total != 0.0 { d.people / total } else { 0.0 };
                d.diversity = if d.people != 0.0 {
                    strangers[s][k] / d.people
                } else {
                    0.0
                };
                let (Some(b), Some(cap)) = (place(d.burg), capital) else {
                    continue;
                };
                if b.id == cap.id {
                    continue;
                }
                let overseas = feature[b.cell] != feature[cap.cell];
                d.remoteness = if overseas {
                    1.0
                } else {
                    (libm::hypot(b.at[0] - cap.at[0], b.at[1] - cap.at[1]) / reach).min(1.0)
                };
            }
            let remoteness = list.iter().fold(0.0, |t, d| t + d.share * d.remoteness);
            let diversity = list.iter().fold(0.0, |t, d| t + d.share * d.diversity);
            for d in list.iter_mut() {
                d.factor = (1.0
                    + REMOTE_CORRUPTION * (d.remoteness - remoteness)
                    + DIVERSITY_CORRUPTION * (d.diversity - diversity))
                    .max(0.0);
            }
        }
        Corruption { by_state, of_cell }
    }

    /// The provinces with people of a state (`getProvinces`).
    pub fn provinces(&self, state: u32) -> &[ProvinceCorruption] {
        self.by_state.get(state as usize).map_or(&[], Vec::as_slice)
    }

    /// The province of a cell (`ofCell`).
    pub fn of_cell(&self, cell: usize) -> Option<&ProvinceCorruption> {
        let (s, k) = self.of_cell.get(cell).copied().flatten()?;
        self.by_state.get(s as usize).and_then(|l| l.get(k))
    }

    /// The province of the capital: where the court and the army spend
    /// (`ofCapital`). `capital_cell` is the cell of the capital.
    pub fn of_capital(&self, capital_cell: Option<usize>) -> Option<&ProvinceCorruption> {
        capital_cell.and_then(|c| self.of_cell(c))
    }
}

/// The corruption of a province before administration: the state's, higher
/// where more remote or diverse than its mean (`getTendency`).
pub fn tendency(state: &State, province: &ProvinceCorruption) -> f64 {
    (state.economy.budget.corruption * province.factor).min(100.0)
}

/// The percent that officials pocket once this much is collected, curbed
/// as administration meets its need (`curb`).
pub fn curb(state: &State, collected: f64, corruption: f64) -> f64 {
    let administration = state.economy.budget.of(Expense::Administration);
    let need = ADMINISTRATION_NEED * (state.urban + state.rural);
    if corruption == 0.0 || need == 0.0 || collected <= 0.0 {
        return corruption;
    }
    // pay = allotted x (1 - graft), graft = corruption x (1 - curb x pay /
    // need): solved for pay.
    let allotted = collected * administration / 100.0;
    let graft = corruption / 100.0;
    let rest = 1.0 - allotted * graft * ADMINISTRATION_CURB / need;
    let pay = if rest > 0.0 {
        allotted * (1.0 - graft) / rest
    } else {
        f64::INFINITY
    };
    corruption * (1.0 - ADMINISTRATION_CURB * (pay / need).min(1.0))
}

impl Corruption {
    /// The corruption in a province, curbed by administration
    /// (`ofProvince`).
    pub fn of_province(&self, state: &State, province: &ProvinceCorruption, collected: f64) -> f64 {
        curb(state, collected, tendency(state, province))
    }

    /// The rate of the provinces for the state by expense, else by people;
    /// `None` without provinces (`byExpenses`).
    fn by_expenses(
        &self,
        state: &State,
        capital_cell: Option<usize>,
        rate: &dyn Fn(&ProvinceCorruption) -> f64,
    ) -> Option<f64> {
        let provinces = self.provinces(state.id);
        let capital = self.of_capital(capital_cell)?;
        if provinces.is_empty() {
            return None;
        }
        let in_provinces = provinces.iter().fold(0.0, |s, d| s + d.share * rate(d));
        let b = &state.economy.budget;
        let (court, military) = (b.of(Expense::Court), b.of(Expense::Military));
        let (administration, infrastructure) =
            (b.of(Expense::Administration), b.of(Expense::Infrastructure));
        let total = court + military + administration + infrastructure;
        if total == 0.0 {
            return Some(in_provinces);
        }
        Some(
            ((court + military) * rate(capital) + (administration + infrastructure) * in_provinces)
                / total,
        )
    }

    /// The corruption of a state by where each expense is spent, or by
    /// people when it spends nothing (`ofState`).
    pub fn of_state(&self, state: &State, capital_cell: Option<usize>, collected: f64) -> f64 {
        self.by_expenses(state, capital_cell, &|d| {
            self.of_province(state, d, collected)
        })
        .unwrap_or_else(|| curb(state, collected, state.economy.budget.corruption))
    }

    /// The corruption of a state before administration, weighed as
    /// `of_state` (`getStateTendency`).
    pub fn state_tendency(&self, state: &State, capital_cell: Option<usize>) -> f64 {
        self.by_expenses(state, capital_cell, &|d| tendency(state, d))
            .unwrap_or(state.economy.budget.corruption)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        let mut s = State {
            id: 1,
            urban: 0.0,
            rural: 100.0,
            ..State::default()
        };
        s.economy.budget.corruption = 20.0;
        s
    }

    #[test]
    fn corruption_stays_when_administration_gets_nothing() {
        let s = state();
        assert_eq!(curb(&s, 100.0, 20.0), 20.0);
    }

    #[test]
    fn corruption_halves_once_administration_is_paid_what_the_people_need() {
        let mut s = state();
        // The need is 0.1 x 100 = 10; 50% of 1000 pays far more than that.
        s.economy.budget.expenses[Expense::Administration.index()] = 50.0;
        assert_eq!(curb(&s, 1000.0, 20.0), 10.0);
        // Less pay curbs less: the pay is net of what officials pocket.
        let some = curb(&s, 10.0, 20.0);
        assert!(some > 10.0 && some < 20.0, "{some}");
        // 5 allotted, graft 0.2: rest = 1 - 5 x 0.2 x 0.5 / 10 = 0.95, pay =
        // 4 / 0.95.
        let pay = 5.0 * 0.8 / 0.95;
        assert!((some - 20.0 * (1.0 - 0.5 * pay / 10.0)).abs() < 1e-12);
    }
}
