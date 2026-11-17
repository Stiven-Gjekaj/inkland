//! What the window shows for the window image of `INKLAND_APP_SNAPSHOT`.
//!
//! `INKLAND_APP_SNAPSHOT_OPEN=TAB[/TOOL,TOOL...]` opens a tab of the menu
//! and the dialogs or tools of the Tools tab, for example `tools/burgs` or
//! `layers/heightmap,print`. The tab `closed` closes the menu.
//! `INKLAND_APP_SNAPSHOT_POINTER=x,y` puts the pointer at a point of the
//! world (config units).

/// The tabs of the menu, in their order.
pub const TABS: [&str; 5] = ["layers", "style", "options", "tools", "about"];

/// What to open before the window image.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// The index of the tab, or `None` to close the menu.
    pub tab: Option<usize>,
    /// The ids of the tools to open (see `catalog::TOOLS`).
    pub tools: Vec<String>,
}

/// Read the value of `INKLAND_APP_SNAPSHOT_OPEN`.
pub fn plan(text: &str) -> Result<Plan, String> {
    let (tab, tools) = text.split_once('/').unwrap_or((text, ""));
    let tab = tab.trim();
    let tab = if tab == "closed" {
        None
    } else {
        Some(
            TABS.iter()
                .position(|t| *t == tab)
                .ok_or_else(|| format!("{tab:?} is not a tab; use one of {TABS:?} or closed"))?,
        )
    };
    let tools = tools
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            if crate::catalog::TOOLS.iter().any(|d| d.id == t) {
                Ok(t.to_owned())
            } else {
                Err(format!("{t:?} is not a tool"))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Plan { tab, tools })
}

/// A point `x,y` in config units.
pub fn point(text: &str) -> Option<[f64; 2]> {
    let (x, y) = text.split_once(',')?;
    Some([x.trim().parse().ok()?, y.trim().parse().ok()?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_names_a_tab_and_tools() {
        assert_eq!(
            plan("tools/burgs,print").unwrap(),
            Plan {
                tab: Some(3),
                tools: vec!["burgs".into(), "print".into()]
            }
        );
        assert_eq!(plan("closed").unwrap().tab, None);
        assert_eq!(plan("style").unwrap().tools, Vec::<String>::new());
        assert!(plan("menu").is_err());
        assert!(plan("tools/nothing").is_err());
        assert_eq!(point(" 150, 120.5"), Some([150.0, 120.5]));
        assert_eq!(point("150"), None);
    }
}
