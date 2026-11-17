//! The SVG writer of inkland.
//!
//! Milestone 7 adds print-safe styles and the SVG checks.

pub mod decor;
pub mod extras;
pub mod page;
#[cfg(feature = "png")]
pub mod png;
pub mod svg;
