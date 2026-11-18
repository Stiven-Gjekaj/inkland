//! The deal log (`deals.ts`): one deal for each place, market, good, and
//! kind sums the trades of the run.

use super::markets::margin_rate;

/// The kind of a deal (`DEAL_KINDS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DealKind {
    /// The place sold its output.
    Sale = 0,
    /// Bought as a production input.
    Input = 1,
    /// Bought for the needs or wants of its people.
    Demand = 2,
    /// Its own output, taken back for its people, free.
    Kept = 3,
    /// A purchase of a state for an expense, delivered to the place.
    Order = 4,
}

impl DealKind {
    /// The FMG name of the kind.
    pub fn name(self) -> &'static str {
        match self {
            DealKind::Sale => "sale",
            DealKind::Input => "input",
            DealKind::Demand => "demand",
            DealKind::Kept => "kept",
            DealKind::Order => "order",
        }
    }
}

/// The trade of a place with a market in one good and kind, summed over
/// the run (`Deal`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Deal {
    /// The place id.
    pub burg: u32,
    /// The market id.
    pub market: u32,
    /// The good id.
    pub good: u32,
    pub kind: DealKind,
    pub units: f64,
    /// The average price of a unit at the door of the place.
    pub price: f64,
    /// The sales tax: of the place on a sale, of the market on a purchase.
    pub tax: f64,
    /// The tariff of the importer, on an import.
    pub tariff: f64,
    /// The average carriage of a unit between the market center and the
    /// place, in the price.
    pub carriage: f64,
}

/// The amounts of one trade: price and carriage for each unit, tax and
/// tariff in total (`Transaction`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Transaction {
    pub units: f64,
    pub price: f64,
    pub tax: f64,
    pub tariff: f64,
    pub carriage: f64,
}

/// The spread of a market on a trade, before taxes and carriage
/// (`marginOf`). `fee_paid` is the license fee of the market.
pub fn margin_of(fee_paid: f64, kind: DealKind, t: &Transaction) -> f64 {
    if kind == DealKind::Kept || t.units == 0.0 {
        return 0.0;
    }
    let rate = margin_rate(fee_paid);
    if kind == DealKind::Sale {
        return t.units * (t.price + t.carriage) * rate / (1.0 - rate);
    }
    let market_price = t.price - (t.tax + t.tariff) / t.units - t.carriage;
    t.units * market_price * rate / (1.0 + rate)
}

impl Deal {
    /// The amounts of the deal.
    pub fn transaction(&self) -> Transaction {
        Transaction {
            units: self.units,
            price: self.price,
            tax: self.tax,
            tariff: self.tariff,
            carriage: self.carriage,
        }
    }

    /// The margin of the market center on the deal (`getMargin`).
    pub fn margin(&self, fee_paid: f64) -> f64 {
        margin_of(fee_paid, self.kind, &self.transaction())
    }

    /// The money to the market: the payment of a place with carriage, less
    /// taxes, minus what it pays a place (`getMarketIncome`).
    pub fn market_income(&self, fee_paid: f64) -> f64 {
        if self.kind == DealKind::Kept {
            return 0.0;
        }
        let margin = self.margin(fee_paid);
        if self.kind == DealKind::Sale {
            return -self.units * self.price - margin;
        }
        self.units * self.price - self.tax - self.tariff - margin
    }

    /// The goods of the deal at the price of the market: without carriage,
    /// taxes, and the margin (`getGoodsValue`).
    pub fn goods_value(&self, fee_paid: f64) -> f64 {
        let margin = self.margin(fee_paid);
        if self.kind == DealKind::Sale {
            self.units * (self.price + self.carriage) + margin
        } else {
            self.units * self.price - self.tax - self.tariff - self.units * self.carriage - margin
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_margin_gives_back_the_price_of_the_market() {
        // A purchase at a market price of 10 with a 4% margin, 1 of tax, and
        // 2 of carriage: the place paid 10.4 + 1 + 2 a unit.
        let t = Transaction {
            units: 2.0,
            price: 13.4,
            tax: 2.0,
            tariff: 0.0,
            carriage: 2.0,
        };
        let m = margin_of(0.0, DealKind::Demand, &t);
        assert!((m - 2.0 * 10.4 * 0.04 / 1.04).abs() < 1e-12);
        // A sale at a market price of 10: 9.6 less 1 of carriage reaches
        // the place.
        let s = Transaction {
            units: 1.0,
            price: 8.6,
            tax: 0.0,
            tariff: 0.0,
            carriage: 1.0,
        };
        assert!((margin_of(0.0, DealKind::Sale, &s) - 9.6 * 0.04 / 0.96).abs() < 1e-12);
        assert_eq!(margin_of(0.0, DealKind::Kept, &s), 0.0);
    }
}
