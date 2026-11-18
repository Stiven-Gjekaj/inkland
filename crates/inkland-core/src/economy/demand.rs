//! What people need and how they spend (`demand.ts`). The run, the
//! accounts, and prosperity read these constants.

use super::goods::Good;

/// A demand category. The order is the priority of FMG
/// (`DEMAND_PRIORITY`): food first, luxury last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Need {
    Food,
    Utilities,
    Construction,
    Military,
    Luxury,
}

impl Need {
    /// All categories in priority order.
    pub const ALL: [Need; 5] = [
        Need::Food,
        Need::Utilities,
        Need::Construction,
        Need::Military,
        Need::Luxury,
    ];

    /// The index of the category in `ALL`.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The FMG name of the category.
    pub fn name(self) -> &'static str {
        match self {
            Need::Food => "food",
            Need::Utilities => "utilities",
            Need::Construction => "construction",
            Need::Military => "military",
            Need::Luxury => "luxury",
        }
    }
}

/// What a person needs of each category a day (`DEMAND_TARGET_FACTORS`).
pub const TARGET_FACTORS: [f64; 5] = [0.2, 0.15, 0.1, 0.08, 0.07];
/// How spare money is split once needs are met (`WANT_SHARES`).
pub const WANT_SHARES: [f64; 5] = [0.25, 0.2, 0.15, 0.1, 0.3];
/// The most a person buys beyond needs, in days of need (`WANT_LIMITS`).
pub const WANT_LIMITS: [f64; 5] = [1.0, 1.0, 1.0, 1.0, 4.0];
/// `true` for the wants that grow with affluence (`WEALTH_WANTS`): luxury.
pub const WEALTH_WANTS: [bool; 5] = [false, false, false, false, true];
/// The share of a day's income that households keep (`SAVINGS_RATE`).
pub const SAVINGS_RATE: f64 = 0.32;
/// Spending over the goods of a need goes by (price per coverage) to the
/// power of minus this (`VARIETY`).
pub const VARIETY: f64 = 2.8;
/// How much more a buyer pays for each tenfold of affluence for a good that
/// it has had none of yet (`NOVELTY`).
pub const NOVELTY: f64 = 5.3;

/// Spending shares: cheaper coverage gets more, and each option gets some
/// (`getVarietyShares`).
pub fn variety_shares(costs: &[f64], variety: f64) -> Vec<f64> {
    let cheapest = costs
        .iter()
        .copied()
        .filter(|&c| c > 0.0)
        .fold(f64::INFINITY, f64::min);
    let weights: Vec<f64> = costs
        .iter()
        .map(|&c| {
            if c > 0.0 {
                libm::pow(c / cheapest, -variety)
            } else {
                0.0
            }
        })
        .collect();
    let sum: f64 = weights.iter().fold(0.0, |s, w| s + w);
    let total = if sum == 0.0 { 1.0 } else { sum };
    weights.iter().map(|w| w / total).collect()
}

/// The daily needs of a person at base prices, each covered the cheapest
/// way (`getNeedsCost`).
pub fn needs_cost(goods: &[Good]) -> f64 {
    Need::ALL.iter().fold(0.0, |sum, &need| {
        let k = need.index();
        let cheapest = goods
            .iter()
            .filter(|g| g.coverage[k] > 0.0)
            .map(|g| g.value / g.coverage[k])
            .fold(f64::INFINITY, f64::min);
        if cheapest.is_finite() {
            sum + TARGET_FACTORS[k] * cheapest
        } else {
            sum
        }
    })
}

/// The needs of a population a day, by category (`getDemandTargets`).
pub fn demand_targets(population: f64) -> [f64; 5] {
    TARGET_FACTORS.map(|f| population * f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spending_goes_more_to_cheaper_coverage() {
        let shares = variety_shares(&[1.0, 2.0, 0.0], VARIETY);
        assert!(shares[0] > shares[1] && shares[1] > 0.0);
        assert_eq!(shares[2], 0.0);
        assert!((shares.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        // 2 costs twice as much: its weight is 2^-2.8 of the first.
        assert!((shares[1] / shares[0] - libm::pow(2.0, -VARIETY)).abs() < 1e-12);
        assert_eq!(variety_shares(&[0.0, 0.0], VARIETY), vec![0.0, 0.0]);
    }

    #[test]
    fn targets_scale_with_people() {
        assert_eq!(
            demand_targets(10.0),
            [
                10.0 * 0.2,
                10.0 * 0.15,
                10.0 * 0.1,
                10.0 * 0.08,
                10.0 * 0.07
            ]
        );
    }
}
