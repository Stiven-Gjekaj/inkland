//! The parts of the inkland desktop app that do not need a window: the
//! view-model, the map view transform, and the worker threads. The binary
//! in `main.rs` connects them to the Slint user interface.

pub mod catalog;
pub mod model;
pub mod options;
pub mod pins;
pub mod rows;
pub mod snapshot;
pub mod variants;
pub mod view;
pub mod worker;
