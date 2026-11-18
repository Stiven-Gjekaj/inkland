//! What states charge (`taxes.ts`): the tariff rules and the costs of trade
//! from one state to another (`Trade.getTradeTerms`).

use super::finance::Tariff;
use crate::society::State;

/// `true` for trade between two states; the neutral lands are no state
/// (`isForeign`).
pub fn is_foreign(a: u32, b: u32) -> bool {
    a != 0 && b != 0 && a != b
}

/// The state with an id, or `None` for 0.
fn state(states: &[State], id: u32) -> Option<&State> {
    (id as usize).checked_sub(1).and_then(|k| states.get(k))
}

/// Any matching embargo of the importer, else its most specific tariff:
/// state and good, state, good, all (`findTariff`).
pub fn find_tariff(states: &[State], importer: u32, exporter: u32, good: u32) -> Tariff {
    let Some(s) = state(states, importer) else {
        return Tariff::Rate(0.0);
    };
    if !is_foreign(importer, exporter) {
        return Tariff::Rate(0.0);
    }
    let (mut rate, mut rank) = (0.0, -1i32);
    for rule in &s.economy.tariffs {
        if rule.states.as_ref().is_some_and(|l| !l.contains(&exporter)) {
            continue;
        }
        if rule.goods.as_ref().is_some_and(|l| !l.contains(&good)) {
            continue;
        }
        let r = match rule.rate {
            Tariff::Embargo => return Tariff::Embargo,
            Tariff::Rate(r) => r,
        };
        let rule_rank = if rule.states.is_none() { 0 } else { 2 } + i32::from(rule.goods.is_some());
        if rule_rank >= rank {
            rate = r;
            rank = rule_rank;
        }
    }
    Tariff::Rate(rate)
}

/// The rate of the tariff of the importer on the good from the exporter, 0
/// under an embargo (`getTariffRate`).
pub fn tariff_rate(states: &[State], importer: u32, exporter: u32, good: u32) -> f64 {
    match find_tariff(states, importer, exporter, good) {
        Tariff::Rate(r) => r,
        Tariff::Embargo => 0.0,
    }
}

/// The sales tax rate of a state; the neutral lands charge none.
pub fn sales_tax(states: &[State], id: u32) -> f64 {
    state(states, id).map_or(0.0, |s| s.economy.sales_tax)
}

/// The poll tax that a state charges people at its rate; the neutral lands
/// charge none (`getPollTaxDue`).
pub fn poll_tax_due(states: &[State], id: u32, people: f64) -> f64 {
    state(states, id).map_or(0.0, |s| s.economy.poll_tax * people)
}

/// The license fee a day of a market centered in the state
/// (`Markets.getFee`).
pub fn market_fee(states: &[State], id: u32) -> f64 {
    state(states, id).map_or(0.0, |s| s.economy.market_fee)
}

/// The sales tax rate of each exporter and the tariff of each pair of
/// states and good, resolved once for the run (`getTradeTerms`).
#[derive(Debug, Clone)]
pub struct TermTable {
    /// The number of state ids, the neutral lands included.
    n: usize,
    goods: usize,
    tax_rate: Vec<f64>,
    /// By exporter, importer, and good index: `None` for an embargo.
    tariffs: Vec<Option<f64>>,
}

impl TermTable {
    /// The terms of all pairs of states. `goods` is the number of goods.
    pub fn new(states: &[State], goods: usize) -> TermTable {
        let n = states.len() + 1;
        let tax_rate: Vec<f64> = (0..n as u32).map(|id| sales_tax(states, id)).collect();
        let mut tariffs = vec![Some(0.0); n * n * goods];
        for exporter in 0..n as u32 {
            for importer in 0..n as u32 {
                if !is_foreign(exporter, importer) {
                    continue;
                }
                for g in 0..goods {
                    let id = g as u32 + 1;
                    let import = find_tariff(states, importer, exporter, id);
                    let export = find_tariff(states, exporter, importer, id);
                    let at = (exporter as usize * n + importer as usize) * goods + g;
                    tariffs[at] = match (import, export) {
                        (Tariff::Embargo, _) | (_, Tariff::Embargo) => None,
                        (Tariff::Rate(r), _) => Some(r),
                    };
                }
            }
        }
        TermTable {
            n,
            goods,
            tax_rate,
            tariffs,
        }
    }
}

impl TermTable {
    /// The sales tax and the tariff for each unit on the good from the
    /// exporter to the importer at a price; `None` under an embargo. A state
    /// `exempt` (not 0) pays no sales tax or tariff to itself
    /// (`StateOrders.exempt`).
    pub fn costs(
        &self,
        exempt: u32,
        exporter: u32,
        importer: u32,
        good: u32,
        price: f64,
    ) -> Option<(f64, f64)> {
        let tax_rate = self.tax_rate.get(exporter as usize).copied().unwrap_or(0.0);
        let mut tariff = 0.0;
        if is_foreign(exporter, importer) {
            let at =
                (exporter as usize * self.n + importer as usize) * self.goods + (good as usize - 1);
            tariff = self.tariffs[at]?;
        }
        let tax = tax_rate * price;
        let tariff = tariff * price;
        if exempt == 0 {
            return Some((tax, tariff));
        }
        Some((
            if exporter == exempt { 0.0 } else { tax },
            if importer == exempt { 0.0 } else { tariff },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::finance::TariffRule;

    fn state(id: u32, tariffs: Vec<TariffRule>, sales_tax: f64) -> State {
        let mut s = State {
            id,
            form: "Monarchy".into(),
            ..State::default()
        };
        s.economy.tariffs = tariffs;
        s.economy.sales_tax = sales_tax;
        s
    }

    #[test]
    fn the_most_specific_tariff_wins_and_any_embargo_wins_over_it() {
        let rules = vec![
            TariffRule {
                states: None,
                goods: None,
                rate: Tariff::Rate(0.05),
            },
            TariffRule {
                states: Some(vec![2]),
                goods: None,
                rate: Tariff::Rate(0.2),
            },
            TariffRule {
                states: None,
                goods: Some(vec![7]),
                rate: Tariff::Rate(0.1),
            },
            TariffRule {
                states: Some(vec![3]),
                goods: Some(vec![9]),
                rate: Tariff::Embargo,
            },
        ];
        let states = vec![
            state(1, rules, 0.1),
            state(2, vec![], 0.05),
            state(3, vec![], 0.0),
        ];
        assert_eq!(find_tariff(&states, 1, 2, 1), Tariff::Rate(0.2));
        assert_eq!(find_tariff(&states, 1, 2, 7), Tariff::Rate(0.2));
        assert_eq!(find_tariff(&states, 1, 3, 7), Tariff::Rate(0.1));
        assert_eq!(find_tariff(&states, 1, 3, 9), Tariff::Embargo);
        assert_eq!(find_tariff(&states, 1, 3, 1), Tariff::Rate(0.05));
        // No tariff within a state or with the neutral lands.
        assert_eq!(find_tariff(&states, 1, 1, 1), Tariff::Rate(0.0));
        assert_eq!(find_tariff(&states, 1, 0, 1), Tariff::Rate(0.0));

        let table = TermTable::new(&states, 10);
        // From 2 to 1: the sales tax of 2 and the tariff of 1.
        let (tax, tariff) = table.costs(0, 2, 1, 1, 10.0).unwrap();
        assert!((tax - 0.5).abs() < 1e-12 && (tariff - 2.0).abs() < 1e-12);
        // An embargo of either side stops the trade.
        assert!(table.costs(0, 3, 1, 9, 10.0).is_none());
        assert!(table.costs(0, 1, 3, 9, 10.0).is_none());
        // A state pays itself nothing.
        assert_eq!(table.costs(1, 1, 1, 1, 10.0), Some((0.0, 0.0)));
        assert_eq!(table.costs(1, 2, 1, 1, 10.0).map(|c| c.1), Some(0.0));
    }
}
