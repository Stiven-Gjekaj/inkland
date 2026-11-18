//! The accounts of the economy (`accounts.ts`): by state, by province, and
//! the world, summed from the run.
//!
//! - The product has three sectors: rural (the earnings of rural people),
//!   manufacturing (the product of the places), and trade (the margins of
//!   the market centers).
//! - Wealth is product for each person. Taxes are what the state collected,
//!   with no adjustments.
//! - A tax counts in the province where the own taxpayer of its state is,
//!   or, for a foreigner, at the market where it was paid.

use super::Economy;
use super::deals::DealKind;
use super::finance::Expense;
use super::prosperity::{Living, RuralCell, level};
use crate::society::Society;

/// The living of a group of people, averaged by their numbers.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GroupLiving {
    pub people: f64,
    pub living: Living,
}

/// What states and provinces both sum (`PeopleAccounts`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct People {
    pub people: f64,
    pub urban_people: f64,
    /// The earnings of rural people from their produce.
    pub rural: f64,
    /// The product of its places.
    pub manufacturing: f64,
    /// The margins that its market centers took.
    pub trade: f64,
    pub product: f64,
    /// Product for each person.
    pub wealth: f64,
    /// The prosperity of all its people, by their numbers.
    pub prosperity: Option<f64>,
    pub urban_living: Option<GroupLiving>,
    pub rural_living: Option<GroupLiving>,
    /// The people at each prosperity level: in places and in cells.
    pub urban_levels: [f64; 10],
    pub rural_levels: [f64; 10],
}

/// The accounts of a state (`StateAccounts`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StateAccounts {
    pub people: People,
    /// The taxes and fees that it collected, adjustments aside.
    pub taxes: f64,
    /// The product of each place for each person.
    pub burg_wealth: Vec<f64>,
}

/// The taxes raised in a province for its state (`ProvinceTaxes`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ProvinceTaxes {
    pub sales_tax: f64,
    pub tariffs: f64,
    pub poll_tax: f64,
    pub market_fees: f64,
}

/// The accounts of a province (`ProvinceAccounts`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProvinceAccounts {
    pub people: People,
    /// The value made at base prices, by good index.
    pub output: Vec<f64>,
    pub taxes: ProvinceTaxes,
    /// The state orders delivered to its places.
    pub spent: f64,
    /// What officials took of the spending meant for it.
    pub pocketed: f64,
}

/// The trade of a state with the others (`StateTrade`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StateTrade {
    pub exports: f64,
    pub imports: f64,
    /// Partner state id, exports, and imports, in the order first met.
    pub partners: Vec<(u32, f64, f64)>,
    /// Value and units by good index.
    pub exported: Vec<f64>,
    pub imported: Vec<f64>,
    pub exported_units: Vec<f64>,
    pub imported_units: Vec<f64>,
}

/// All accounts of the run.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Accounts {
    /// By state id; entry 0 is the neutral lands.
    pub states: Vec<StateAccounts>,
    /// All states, the neutral lands included.
    pub world: StateAccounts,
    /// By province index (id - 1).
    pub provinces: Vec<ProvinceAccounts>,
    /// The units of each good that the people and the orders of each state
    /// consumed, by state id and good index (`getConsumption`).
    pub consumption: Vec<Vec<f64>>,
}

/// People-weighted sums of the living of each group (`LivingSums`).
#[derive(Debug, Clone, Copy, Default)]
struct Sums {
    urban: Option<(f64, Living)>,
    rural: Option<(f64, Living)>,
}

fn add_living(
    sums: &mut Sums,
    acc: &mut People,
    urban: bool,
    people: f64,
    living: Option<&Living>,
) {
    let Some(l) = living else {
        return;
    };
    if people == 0.0 {
        return;
    }
    let slot = if urban {
        &mut sums.urban
    } else {
        &mut sums.rural
    };
    let (p, row) = slot.get_or_insert((0.0, Living::default()));
    *p += people;
    row.needs += people * l.needs;
    row.comfort += people * l.comfort;
    row.variety += people * l.variety;
    row.infrastructure += people * l.infrastructure;
    row.prosperity += people * l.prosperity;
    let levels = if urban {
        &mut acc.urban_levels
    } else {
        &mut acc.rural_levels
    };
    levels[level(l.prosperity)] += people;
}

fn average(row: Option<(f64, Living)>) -> Option<GroupLiving> {
    let (p, l) = row?;
    if p == 0.0 {
        return None;
    }
    Some(GroupLiving {
        people: p,
        living: Living {
            needs: l.needs / p,
            comfort: l.comfort / p,
            variety: l.variety / p,
            infrastructure: l.infrastructure / p,
            prosperity: l.prosperity / p,
        },
    })
}

/// Close the accounts: product, wealth, and the mean living (`close`).
fn close(acc: &mut People, sums: Sums) {
    acc.product = acc.rural + acc.manufacturing + acc.trade;
    acc.wealth = if acc.people != 0.0 {
        acc.product / acc.people
    } else {
        0.0
    };
    acc.urban_living = average(sums.urban);
    acc.rural_living = average(sums.rural);
    let lived: Vec<GroupLiving> = [acc.urban_living, acc.rural_living]
        .into_iter()
        .flatten()
        .collect();
    let people = lived.iter().fold(0.0, |t, g| t + g.people);
    acc.prosperity = if people != 0.0 {
        Some(
            lived
                .iter()
                .fold(0.0, |t, g| t + g.living.prosperity * g.people)
                / people,
        )
    } else {
        None
    };
}

/// What the accounts read beyond the economy.
pub struct Input<'a> {
    pub soc: &'a Society,
    pub economy: &'a Economy,
    /// How the people of each place live.
    pub burg_living: &'a [Option<Living>],
    /// The rural people of each cell.
    pub rural: &'a [Option<RuralCell>],
    /// The rural output of each cell (`Output.getRural`).
    pub output: &'a [Vec<(u32, f64)>],
    /// The land cell that works each cell (`Output.getWorkingCell`).
    pub working: &'a [usize],
}

fn market_state(soc: &Society, e: &Economy, market: u32) -> u32 {
    e.market(market)
        .map_or(0, |m| soc.places[m.center as usize - 1].state)
}

/// The sales tax and tariffs by state from the deals (`getTaxesByState`).
pub fn taxes_by_state(soc: &Society, e: &Economy) -> (Vec<f64>, Vec<f64>) {
    let n = soc.states.len() + 1;
    let (mut sales, mut tariffs) = (vec![0.0; n], vec![0.0; n]);
    for d in &e.deals {
        let burg = soc.places[d.burg as usize - 1].state;
        let market = market_state(soc, e, d.market);
        let (tax_state, tariff_state) = if d.kind == DealKind::Sale {
            (burg, market)
        } else {
            (market, burg)
        };
        if d.tax != 0.0 {
            sales[tax_state as usize] += d.tax;
        }
        if d.tariff != 0.0 {
            tariffs[tariff_state as usize] += d.tariff;
        }
    }
    (sales, tariffs)
}

/// The poll tax owed and paid by state (`getPollTax`).
pub fn poll_tax(soc: &Society, e: &Economy) -> (Vec<f64>, Vec<f64>) {
    let n = soc.states.len() + 1;
    let (mut owed, mut paid) = (vec![0.0; n], vec![0.0; n]);
    for (b, pe) in e.places.iter().enumerate() {
        if pe.poll_tax_paid != 0.0 {
            let s = soc.places[b].state as usize;
            owed[s] += pe.poll_tax_paid;
            paid[s] += pe.poll_tax_paid;
        }
    }
    for m in &e.markets {
        for &(s, o, p) in m.rural.as_ref().map_or(&[][..], |r| &r.poll_tax[..]) {
            owed[s as usize] += o;
            paid[s as usize] += p;
        }
    }
    (owed, paid)
}

/// The market fees paid by the state of each market center
/// (`getMarketFees`).
pub fn market_fees(soc: &Society, e: &Economy) -> Vec<f64> {
    let mut paid = vec![0.0; soc.states.len() + 1];
    for m in &e.markets {
        if m.fee_paid != 0.0 {
            paid[market_state(soc, e, m.id) as usize] += m.fee_paid;
        }
    }
    paid
}

/// The trade of a state with the other states, at market prices without
/// carriage, taxes, and margin (`getTrade`).
pub fn trade(soc: &Society, e: &Economy, state: u32) -> StateTrade {
    let g = e.goods.len();
    let mut t = StateTrade {
        exported: vec![0.0; g],
        imported: vec![0.0; g],
        exported_units: vec![0.0; g],
        imported_units: vec![0.0; g],
        ..StateTrade::default()
    };
    for d in &e.deals {
        if d.kind == DealKind::Kept {
            continue;
        }
        let burg = soc.places[d.burg as usize - 1].state;
        let market = market_state(soc, e, d.market);
        if burg == market || (burg != state && market != state) {
            continue;
        }
        let export = if d.kind == DealKind::Sale {
            burg == state
        } else {
            market == state
        };
        let partner = if burg == state { market } else { burg };
        let fee = e.market(d.market).map_or(0.0, |m| m.fee_paid);
        let value = d.goods_value(fee);
        let k = d.good as usize - 1;
        let row = match t.partners.iter().position(|p| p.0 == partner) {
            Some(i) => i,
            None => {
                t.partners.push((partner, 0.0, 0.0));
                t.partners.len() - 1
            }
        };
        if export {
            t.exports += value;
            t.partners[row].1 += value;
            t.exported[k] += value;
            t.exported_units[k] += d.units;
        } else {
            t.imports += value;
            t.partners[row].2 += value;
            t.imported[k] += value;
            t.imported_units[k] += d.units;
        }
    }
    t
}

/// The value at base prices of each good that the places and the
/// countryside of each state made, by state id and good index, and the
/// world's (`getOutput`, `getWorldOutput`).
pub fn outputs(
    soc: &Society,
    e: &Economy,
    output: &[Vec<(u32, f64)>],
    working: &[usize],
) -> (Vec<Vec<f64>>, Vec<f64>) {
    let g = e.goods.len();
    let mut by_state = vec![vec![0.0; g]; soc.states.len() + 1];
    let mut world = vec![0.0; g];
    let mut add = |s: u32, id: u32, units: f64| {
        let k = id as usize - 1;
        let v = units * e.goods[k].value;
        by_state[s as usize][k] += v;
        world[k] += v;
    };
    for (b, pe) in e.places.iter().enumerate() {
        for &(id, units) in &pe.production {
            add(soc.places[b].state, id, units);
        }
    }
    for (cell, produced) in output.iter().enumerate() {
        if e.cell_market[cell] == 0 {
            continue;
        }
        for &(id, units) in produced {
            add(soc.cell_state[working[cell]], id, units);
        }
    }
    (by_state, world)
}

/// The taxes, fees, and corruption of a state in the run (`getBudget`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StateBudget {
    pub sales_tax: f64,
    pub tariffs: f64,
    pub poll_tax: f64,
    pub poll_tax_owed: f64,
    pub market_fees: f64,
    /// The number of markets centered in the state.
    pub markets: usize,
    pub corruption: f64,
}

/// The budget of a state in the run (`getBudget`).
pub fn budget(soc: &Society, e: &Economy, state: u32) -> StateBudget {
    let s = state as usize;
    let (sales, tariffs) = taxes_by_state(soc, e);
    let (owed, paid) = poll_tax(soc, e);
    StateBudget {
        sales_tax: sales[s],
        tariffs: tariffs[s],
        poll_tax: paid[s],
        poll_tax_owed: owed[s],
        market_fees: market_fees(soc, e)[s],
        markets: e
            .markets
            .iter()
            .filter(|m| market_state(soc, e, m.id) == state)
            .count(),
        corruption: s
            .checked_sub(1)
            .and_then(|k| soc.states.get(k))
            .map_or(0.0, |st| st.economy.pocketed),
    }
}

/// The sector whose share of the product most exceeds its share of the
/// world: `rural`, `manufacturing`, or `trade` (`leaningSector`).
pub fn leaning_sector(acc: &People, world: &People) -> Option<&'static str> {
    if acc.product == 0.0 {
        return None;
    }
    let share = |p: &People, v: f64| if p.product != 0.0 { v / p.product } else { 0.0 };
    let lean = |k: usize| {
        let (a, w) = match k {
            0 => (acc.rural, world.rural),
            1 => (acc.manufacturing, world.manufacturing),
            _ => (acc.trade, world.trade),
        };
        let ws = share(world, w);
        share(acc, a) / if ws != 0.0 { ws } else { 1.0 }
    };
    let mut best = 0;
    for k in 1..3 {
        if lean(k) > lean(best) {
            best = k;
        }
    }
    Some(["rural", "manufacturing", "trade"][best])
}

/// The economy type of a state: `Agrarian`, `Artisan`, or `Mercantile`
/// (`economyType`).
pub fn economy_type(acc: &People, world: &People) -> Option<&'static str> {
    leaning_sector(acc, world).map(|s| match s {
        "rural" => "Agrarian",
        "manufacturing" => "Artisan",
        _ => "Mercantile",
    })
}

/// The good that weighs most in the exports of a state: its share squared
/// over its share of the world (`knownFor`). A good id.
pub fn known_for(soc: &Society, e: &Economy, state: u32) -> Option<u32> {
    let own = trade(soc, e, state);
    if own.exports == 0.0 {
        return None;
    }
    let mut world = vec![0.0; e.goods.len()];
    for id in 0..=soc.states.len() as u32 {
        for (k, v) in trade(soc, e, id).exported.iter().enumerate() {
            world[k] += v;
        }
    }
    let total = world.iter().fold(0.0, |t, v| t + v);
    let (mut best, mut best_lean) = (None, 0.0);
    for (k, &v) in own.exported.iter().enumerate() {
        if v == 0.0 {
            continue;
        }
        let lean = libm::pow(v / own.exports, 2.0) / (world[k] / total);
        if lean > best_lean {
            best = Some(k as u32 + 1);
            best_lean = lean;
        }
    }
    best
}

/// Sum the accounts of the run (`indexStates`, `indexProvinces`, and
/// `indexConsumption`).
pub fn build(input: &Input) -> Accounts {
    let Input {
        soc,
        economy: e,
        burg_living,
        rural,
        ..
    } = *input;
    let n = soc.states.len() + 1;
    let g = e.goods.len();
    let mut out = Accounts {
        consumption: consumption(soc, e, rural),
        ..Accounts::default()
    };
    if e.deals.is_empty() {
        return out;
    }

    // States, the neutral lands included, and the world.
    let mut states = vec![StateAccounts::default(); n];
    let mut world = StateAccounts::default();
    let mut sums = vec![Sums::default(); n];
    let mut world_sums = Sums::default();
    for (b, p) in soc.places.iter().enumerate() {
        if p.population == 0.0 {
            continue;
        }
        let product = e.places[b].product;
        for acc in [&mut states[p.state as usize], &mut world] {
            acc.people.people += p.population;
            acc.people.urban_people += p.population;
            acc.people.manufacturing += product;
            acc.burg_wealth.push(product / p.population);
        }
        let l = burg_living[b].as_ref();
        add_living(
            &mut sums[p.state as usize],
            &mut states[p.state as usize].people,
            true,
            p.population,
            l,
        );
        add_living(&mut world_sums, &mut world.people, true, p.population, l);
    }
    for cell in 0..soc.cell_state.len() {
        let people = soc.cells.pop[cell];
        if people == 0.0 {
            continue;
        }
        let s = soc.cell_state[cell] as usize;
        let r = rural[cell].as_ref();
        let earned = r.map_or(0.0, |r| r.earned);
        for acc in [&mut states[s], &mut world] {
            acc.people.people += people;
            acc.people.rural += earned;
        }
        let l = r.map(|r| &r.living);
        add_living(&mut sums[s], &mut states[s].people, false, people, l);
        add_living(&mut world_sums, &mut world.people, false, people, l);
    }
    for d in &e.deals {
        let m = &e.markets[d.market as usize - 1];
        let margin = d.margin(m.fee_paid);
        if margin == 0.0 {
            continue;
        }
        let s = market_state(soc, e, d.market) as usize;
        states[s].people.trade += margin;
        world.people.trade += margin;
    }
    // Rural purchases make no deals, but their margin goes to the center
    // too.
    for m in &e.markets {
        let margin = m.rural.as_ref().map_or(0.0, |r| r.margin);
        if margin == 0.0 {
            continue;
        }
        let s = market_state(soc, e, m.id) as usize;
        states[s].people.trade += margin;
        world.people.trade += margin;
    }
    let (sales, tariffs) = taxes_by_state(soc, e);
    let (_, paid) = poll_tax(soc, e);
    let fees = market_fees(soc, e);
    let taxes = [&sales, &tariffs, &paid, &fees];
    for (s, acc) in states.iter_mut().enumerate() {
        acc.taxes = taxes.iter().fold(0.0, |t, by| t + by[s]);
        close(&mut acc.people, sums[s]);
    }
    world.taxes = taxes
        .iter()
        .fold(0.0, |t, by| by.iter().fold(t, |u, v| u + v));
    close(&mut world.people, world_sums);
    out.states = states;
    out.world = world;
    out.provinces = provinces(input, g);
    out
}

/// The units for each good that the people and the orders of each state
/// bought a day, kept output in, unsold produce out (`indexConsumption`).
fn consumption(soc: &Society, e: &Economy, rural: &[Option<RuralCell>]) -> Vec<Vec<f64>> {
    let g = e.goods.len();
    let mut out = vec![vec![0.0; g]; soc.states.len() + 1];
    for d in &e.deals {
        if matches!(d.kind, DealKind::Demand | DealKind::Kept | DealKind::Order) {
            let s = soc.places[d.burg as usize - 1].state as usize;
            out[s][d.good as usize - 1] += d.units;
        }
    }
    for (cell, r) in rural.iter().enumerate() {
        if let Some(r) = r {
            let s = soc.cell_state[cell] as usize;
            for &(id, units) in &r.bought {
                out[s][id as usize - 1] += units;
            }
        }
    }
    out
}

/// The accounts of each province (`indexProvinces`).
fn provinces(input: &Input, g: usize) -> Vec<ProvinceAccounts> {
    let Input {
        soc,
        economy: e,
        burg_living,
        rural,
        output,
        working,
    } = *input;
    let np = soc.provinces.len();
    if np == 0 {
        return Vec::new();
    }
    let mut accs: Vec<ProvinceAccounts> = (0..np)
        .map(|_| ProvinceAccounts {
            output: vec![0.0; g],
            ..ProvinceAccounts::default()
        })
        .collect();
    let mut sums = vec![Sums::default(); np];
    let of = |cell: usize| (soc.cell_province[cell] as usize).checked_sub(1);
    for (b, p) in soc.places.iter().enumerate() {
        if p.population == 0.0 {
            continue;
        }
        let Some(k) = of(p.cell) else {
            continue;
        };
        let a = &mut accs[k];
        a.people.people += p.population;
        a.people.urban_people += p.population;
        a.people.manufacturing += e.places[b].product;
        a.taxes.poll_tax += e.places[b].poll_tax_paid;
        add_living(
            &mut sums[k],
            &mut accs[k].people,
            true,
            p.population,
            burg_living[b].as_ref(),
        );
    }
    for (b, p) in soc.places.iter().enumerate() {
        if let Some(k) = of(p.cell) {
            for &(id, units) in &e.places[b].production {
                accs[k].output[id as usize - 1] += units * e.goods[id as usize - 1].value;
            }
        }
    }
    for (cell, produced) in output.iter().enumerate() {
        if e.cell_market[cell] == 0 {
            continue;
        }
        if let Some(k) = of(working[cell]) {
            for &(id, units) in produced {
                accs[k].output[id as usize - 1] += units * e.goods[id as usize - 1].value;
            }
        }
    }
    // Rural people share what their market paid of the poll tax of their
    // state, by head.
    let ns = soc.states.len() + 1;
    let mut heads = vec![vec![0.0; ns]; e.markets.len()];
    for cell in 0..soc.cell_state.len() {
        let m = e.cell_market[cell];
        if m == 0 || soc.cells.pop[cell] == 0.0 {
            continue;
        }
        heads[m as usize - 1][soc.cell_state[cell] as usize] += soc.cells.pop[cell];
    }
    let per_head: Vec<Vec<(u32, f64)>> = e
        .markets
        .iter()
        .enumerate()
        .map(|(k, m)| {
            m.rural.as_ref().map_or_else(Vec::new, |r| {
                r.poll_tax
                    .iter()
                    .map(|&(s, _, pays)| {
                        let h = heads[k][s as usize];
                        (s, pays / if h != 0.0 { h } else { 1.0 })
                    })
                    .collect()
            })
        })
        .collect();
    for cell in 0..soc.cell_state.len() {
        let people = soc.cells.pop[cell];
        if people == 0.0 {
            continue;
        }
        let Some(k) = of(cell) else {
            continue;
        };
        let r = rural[cell].as_ref();
        accs[k].people.people += people;
        accs[k].people.rural += r.map_or(0.0, |r| r.earned);
        let s = soc.cell_state[cell];
        let m = e.cell_market[cell];
        let rate = (m as usize)
            .checked_sub(1)
            .and_then(|i| per_head[i].iter().find(|x| x.0 == s))
            .map_or(0.0, |x| x.1);
        accs[k].taxes.poll_tax += people * rate;
        add_living(
            &mut sums[k],
            &mut accs[k].people,
            false,
            people,
            r.map(|r| &r.living),
        );
    }
    // A tax counts where the own taxpayer of its state is, or at the market
    // where a foreigner paid it.
    let center_cell = |m: u32| e.market(m).map(|x| soc.places[x.center as usize - 1].cell);
    for d in &e.deals {
        let burg = &soc.places[d.burg as usize - 1];
        if d.kind == DealKind::Order
            && let Some(k) = of(burg.cell)
        {
            accs[k].spent += d.units * d.price;
        }
        let fee = e.market(d.market).map_or(0.0, |m| m.fee_paid);
        let margin = d.margin(fee);
        if margin != 0.0
            && let Some(k) = center_cell(d.market).and_then(of)
        {
            accs[k].people.trade += margin;
        }
        if d.tax == 0.0 && d.tariff == 0.0 {
            continue;
        }
        let market = market_state(soc, e, d.market);
        let (tax_state, tariff_state) = if d.kind == DealKind::Sale {
            (burg.state, market)
        } else {
            (market, burg.state)
        };
        let raised_for = |state: u32, at_market: Option<u32>| {
            if burg.state == state {
                of(burg.cell)
            } else {
                at_market.and_then(center_cell).and_then(of)
            }
        };
        if d.tax != 0.0
            && let Some(k) = raised_for(tax_state, Some(d.market))
        {
            accs[k].taxes.sales_tax += d.tax;
        }
        let importer = if d.kind == DealKind::Sale {
            Some(d.market)
        } else {
            Some(e.cell_market[burg.cell]).filter(|&m| m != 0)
        };
        if d.tariff != 0.0
            && let Some(k) = raised_for(tariff_state, importer)
        {
            accs[k].taxes.tariffs += d.tariff;
        }
    }
    for m in &e.markets {
        let Some(k) = center_cell(m.id).and_then(of) else {
            continue;
        };
        accs[k].people.trade += m.rural.as_ref().map_or(0.0, |r| r.margin);
        accs[k].taxes.market_fees += m.fee_paid;
    }
    // What officials pocketed of the spending meant for each province.
    for s in &soc.states {
        let f = &s.economy;
        if f.collected <= 0.0 {
            continue;
        }
        let capital_cell = (s.capital as usize)
            .checked_sub(1)
            .map(|c| soc.places[c].cell);
        let capital = e.corruption.of_capital(capital_cell);
        let in_provinces =
            f.budget.of(Expense::Administration) + f.budget.of(Expense::Infrastructure);
        let in_capital = f.budget.total() - in_provinces;
        for p in e.corruption.provinces(s.id) {
            let Some(k) = (p.id as usize).checked_sub(1) else {
                continue;
            };
            let corruption = e.corruption.of_province(s, p, f.collected);
            let share = in_provinces * p.share + if capital == Some(p) { in_capital } else { 0.0 };
            accs[k].pocketed += f.collected * share * corruption / 10000.0;
        }
    }
    for (k, a) in accs.iter_mut().enumerate() {
        close(&mut a.people, sums[k]);
    }
    accs
}
