//! The world model and the generation stages of inkland.
//!
//! This crate is pure. It does no I/O, and it reads no clock and no
//! environment.

pub mod config;
pub mod error;
pub mod features;
pub mod grid;
pub mod heightmap;
pub mod rand;
pub mod seed;
