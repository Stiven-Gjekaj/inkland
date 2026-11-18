//! The finance of a state (`StateFinance`, `TariffRule`, and `StateBudget`
//! of `states-generator.ts`, and `data/expenses.ts`).
//!
//! `states::forms` sets the rates, the fee, the tariffs, and the budget.
//! The production run sets what the state collected, spent, and pocketed.

/// What a state spends its taxes on (`EXPENSES`), in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Expense {
    Court,
    Administration,
    Infrastructure,
    Military,
}

impl Expense {
    /// All expenses in FMG order.
    pub const ALL: [Expense; 4] = [
        Expense::Court,
        Expense::Administration,
        Expense::Infrastructure,
        Expense::Military,
    ];

    /// The index of the expense in `ALL`.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The FMG name of the expense.
    pub fn name(self) -> &'static str {
        match self {
            Expense::Court => "court",
            Expense::Administration => "administration",
            Expense::Infrastructure => "infrastructure",
            Expense::Military => "military",
        }
    }
}

/// The rate of a tariff rule: a share of the price, or an embargo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tariff {
    Rate(f64),
    Embargo,
}

/// An import rule of a state (`TariffRule`). `None` means all states or all
/// goods.
#[derive(Debug, Clone, PartialEq)]
pub struct TariffRule {
    pub states: Option<Vec<u32>>,
    pub goods: Option<Vec<u32>>,
    pub rate: Tariff,
}

/// The budget of a state (`StateBudget`): each expense in whole percents of
/// the taxes collected, and the percent of each expense that officials
/// pocket. The expenses may sum past 100.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Budget {
    /// The expenses in `Expense` order.
    pub expenses: [f64; 4],
    pub corruption: f64,
}

impl Budget {
    /// The percent of one expense.
    pub fn of(&self, e: Expense) -> f64 {
        self.expenses[e.index()]
    }

    /// The sum of the expenses, in FMG order.
    pub fn total(&self) -> f64 {
        self.expenses.iter().fold(0.0, |s, v| s + v)
    }
}

/// The taxes, fee, tariffs, budget, and treasury of a state
/// (`StateFinance`). All zero for a state before its form and for the
/// neutral lands. inkland has no user adjustments, so they are not here:
/// FMG adds them as zero lines on a new map.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StateFinance {
    /// The sales tax rate, a share of the price.
    pub sales_tax: f64,
    /// The poll tax a day for each population point.
    pub poll_tax: f64,
    /// The license fee a day from each market centered in the state.
    pub market_fee: f64,
    pub budget: Budget,
    /// All taxes collected in the run.
    pub collected: f64,
    /// What each expense bought in the run, in `Expense` order.
    pub spent: [f64; 4],
    /// What officials pocketed in the run.
    pub pocketed: f64,
    pub treasury: f64,
    pub tariffs: Vec<TariffRule>,
}

impl StateFinance {
    /// What the state spent on one expense.
    pub fn spent_on(&self, e: Expense) -> f64 {
        self.spent[e.index()]
    }
}
