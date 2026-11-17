//! The parts of the inkland desktop app that do not need a window: the
//! view-model, the map view transform, and the worker threads. The binary
//! in `main.rs` connects them to the Slint user interface.

pub mod catalog;
pub mod model;
pub mod view;
pub mod worker;
