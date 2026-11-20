//! The parts of the inkland desktop app that do not need a window: the
//! view-model, the map view transform, and the worker threads. The binary
//! in `main.rs` connects them to the Slint user interface.

pub mod catalog;
pub mod data_edits;
pub mod emblems;
pub mod hotkeys;
pub mod model;
pub mod options;
pub mod overlay;
pub mod overviews;
pub mod pins;
pub mod positions;
pub mod rows;
pub mod snapshot;
pub mod town;
pub mod variants;
pub mod view;
pub mod worker;
