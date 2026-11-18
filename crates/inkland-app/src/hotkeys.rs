//! The hotkeys of FMG (`app/hotkeys.ts`) that make sense in the app.
//!
//! The app gives the key as text: a typed character, or the name of a
//! special key (`F2`, `Tab`, `Escape`, `Enter`, `Delete`, `Left`, `Right`,
//! `Up`, `Down`). The result is what the key does.

use crate::catalog;

/// What a hotkey does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hotkey {
    Undo,
    Redo,
    Save,
    SaveAs,
    Open,
    /// F2: a new map with a new seed.
    NewMap,
    /// Tab: open or close the menu.
    ToggleMenu,
    /// Escape: stop the drawing, the tool, the dialogs, and the menu.
    Escape,
    /// Enter: finish the drawing.
    Finish,
    /// Delete: remove the selected burg.
    Delete,
    /// Open a dialog or start a tool of the Tools tab.
    Tool(&'static str),
    /// Show or hide the layer with this FMG id.
    Layer(&'static str),
    /// Multiply the zoom.
    Zoom(f64),
    /// Show the full page.
    Fit,
    /// Move the map by screen pixels.
    Pan(f64, f64),
    /// Make the brush larger (`true`) or smaller.
    BrushSize(bool),
}

/// The tool of a Shift hotkey, as the upper-case letter.
const SHIFT_TOOLS: &[(char, &str)] = &[
    ('H', "heightmap"),
    ('S', "states"),
    ('P', "provinces"),
    ('L', "labels"),
    ('C', "cultures"),
    ('Z', "regions"),
    ('R', "religions"),
    ('Q', "units"),
    ('T', "burgs"),
    ('U', "routes"),
    ('V', "rivers"),
    ('E', "cells"),
    ('W', "variants"),
    ('Y', "emblems"),
    ('D', "diplomacy"),
];

/// The tools of the Shift and number hotkeys: `!` is Shift+1 on most
/// keyboards.
const SYMBOL_TOOLS: &[(char, &str)] = &[('!', "add-burg"), ('$', "add-river"), ('%', "add-route")];

/// What the key does. `ctrl` is Control or Command.
pub fn hotkey(key: &str, ctrl: bool, shift: bool, alt: bool) -> Option<Hotkey> {
    let one = {
        let mut chars = key.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Some(c),
            _ => None,
        }
    };
    if ctrl {
        let c = one?.to_ascii_lowercase();
        return match (c, shift) {
            ('z', false) => Some(Hotkey::Undo),
            ('z', true) | ('y', _) => Some(Hotkey::Redo),
            ('s', false) => Some(Hotkey::Save),
            ('s', true) => Some(Hotkey::SaveAs),
            ('o', _) => Some(Hotkey::Open),
            _ => None,
        };
    }
    if alt {
        return None;
    }
    match key {
        "F2" => return Some(Hotkey::NewMap),
        "Tab" => return Some(Hotkey::ToggleMenu),
        "Escape" => return Some(Hotkey::Escape),
        "Enter" => return Some(Hotkey::Finish),
        "Delete" | "Backspace" => return Some(Hotkey::Delete),
        "Left" => return Some(Hotkey::Pan(40.0, 0.0)),
        "Right" => return Some(Hotkey::Pan(-40.0, 0.0)),
        "Up" => return Some(Hotkey::Pan(0.0, 40.0)),
        "Down" => return Some(Hotkey::Pan(0.0, -40.0)),
        _ => {}
    }
    let c = one?;
    if let Some((_, t)) = SYMBOL_TOOLS.iter().find(|(k, _)| *k == c) {
        return Some(Hotkey::Tool(t));
    }
    if shift && let Some((_, t)) = SHIFT_TOOLS.iter().find(|(k, _)| *k == c) {
        return Some(Hotkey::Tool(t));
    }
    match c {
        '+' | '=' => return Some(Hotkey::Zoom(1.2)),
        '-' => return Some(Hotkey::Zoom(0.8)),
        '0' => return Some(Hotkey::Fit),
        ']' => return Some(Hotkey::BrushSize(true)),
        '[' => return Some(Hotkey::BrushSize(false)),
        _ => {}
    }
    if !shift && let Some(def) = catalog::layer_of_key(c) {
        return Some(Hotkey::Layer(def.id));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fmg_hotkeys_map_to_actions() {
        assert_eq!(hotkey("z", true, false, false), Some(Hotkey::Undo));
        assert_eq!(hotkey("Z", true, true, false), Some(Hotkey::Redo));
        assert_eq!(hotkey("y", true, false, false), Some(Hotkey::Redo));
        assert_eq!(hotkey("s", true, false, false), Some(Hotkey::Save));
        assert_eq!(hotkey("S", true, true, false), Some(Hotkey::SaveAs));
        assert_eq!(hotkey("F2", false, false, false), Some(Hotkey::NewMap));
        assert_eq!(hotkey("Tab", false, false, false), Some(Hotkey::ToggleMenu));
        assert_eq!(
            hotkey("H", false, true, false),
            Some(Hotkey::Tool("heightmap"))
        );
        assert_eq!(hotkey("T", false, true, false), Some(Hotkey::Tool("burgs")));
        assert_eq!(
            hotkey("!", false, true, false),
            Some(Hotkey::Tool("add-burg"))
        );
        assert_eq!(
            hotkey("v", false, false, false),
            Some(Hotkey::Layer("rivers"))
        );
        assert_eq!(
            hotkey("i", false, false, false),
            Some(Hotkey::Layer("burgIcons"))
        );
        assert_eq!(
            hotkey("/", false, false, false),
            Some(Hotkey::Layer("scaleBar"))
        );
        assert_eq!(hotkey("0", false, false, false), Some(Hotkey::Fit));
        assert_eq!(hotkey("-", false, false, false), Some(Hotkey::Zoom(0.8)));
        assert_eq!(
            hotkey("Left", false, false, false),
            Some(Hotkey::Pan(40.0, 0.0))
        );
        assert_eq!(
            hotkey("]", false, false, false),
            Some(Hotkey::BrushSize(true))
        );
        assert_eq!(
            hotkey("g", false, false, false),
            Some(Hotkey::Layer("goods"))
        );
        // Alt and unknown keys do nothing.
        assert_eq!(hotkey("v", false, false, true), None);
        assert_eq!(hotkey("Home", false, false, false), None);
    }

    #[test]
    fn each_tool_of_a_hotkey_is_a_tool_of_the_tools_tab() {
        for (_, t) in SHIFT_TOOLS.iter().chain(SYMBOL_TOOLS) {
            assert!(catalog::TOOLS.iter().any(|d| d.id == *t), "{t}");
        }
    }
}
