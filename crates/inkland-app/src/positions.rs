//! The places of the dialogs for the session. A dialog that closes and
//! opens again comes back where the user put it, as the jQuery dialogs of
//! FMG do.

use std::collections::BTreeMap;

/// The last place of each dialog, by dialog id, in logical pixels.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Positions(BTreeMap<String, [f32; 2]>);

impl Positions {
    /// The place of a dialog: its last place, or `default` for a dialog
    /// that did not move yet.
    pub fn get(&self, id: &str, default: [f32; 2]) -> [f32; 2] {
        self.0.get(id).copied().unwrap_or(default)
    }

    /// Keep the place of a dialog. A dialog without an id has no place.
    pub fn set(&mut self, id: &str, at: [f32; 2]) {
        if !id.is_empty() && at.iter().all(|v| v.is_finite()) {
            self.0.insert(id.to_owned(), at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dialog_comes_back_where_it_was() {
        let mut p = Positions::default();
        assert_eq!(p.get("zones", [440.0, 80.0]), [440.0, 80.0]);
        p.set("zones", [120.0, 300.0]);
        assert_eq!(p.get("zones", [440.0, 80.0]), [120.0, 300.0]);
        // Other dialogs keep their own default.
        assert_eq!(p.get("notes", [460.0, 70.0]), [460.0, 70.0]);
        p.set("", [1.0, 1.0]);
        p.set("notes", [f32::NAN, 1.0]);
        assert_eq!(p.get("notes", [460.0, 70.0]), [460.0, 70.0]);
    }
}
