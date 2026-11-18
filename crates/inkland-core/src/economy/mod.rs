//! The economy of FMG (`model/economy`): goods and deposits, markets, the
//! production run of one day, taxes, and what the run gives.
//!
//! - `goods`: the catalogue and the deposits (`Goods.generate`). The
//!   deposits are placed before `rankCells`, as in FMG, and live in
//!   `Society::cells.good`.
//! - `demand`: what people need and how they spend.

pub mod demand;
pub mod goods;
pub mod goods_data;

/// The economy of a world.
#[derive(Debug, Clone, PartialEq)]
pub struct Economy {
    /// The goods catalogue. The good with id `k` is at index `k - 1`.
    pub goods: Vec<goods::Good>,
}

impl Economy {
    /// The good with an id (`Goods.get`); `None` for 0 or an unknown id.
    pub fn good(&self, id: u32) -> Option<&goods::Good> {
        (id as usize).checked_sub(1).and_then(|k| self.goods.get(k))
    }
}
