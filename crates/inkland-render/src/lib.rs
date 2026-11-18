//! The SVG writer of inkland.
//!
//! Milestone 7 adds print-safe styles and the SVG checks.

pub mod checks;
pub mod decor;
pub mod emblem;
pub mod extras;
pub mod fmg;
pub mod labels;
pub mod metrics;
mod metrics_data;
pub mod page;
#[cfg(feature = "png")]
pub mod png;
pub mod print_layers;
pub mod svg;
