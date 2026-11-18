//! The world model and the generation stages of inkland.
//!
//! This crate is pure. It does no I/O, and it reads no clock and no
//! environment.

pub mod biomes;
pub mod borders;
pub mod burgs;
pub mod climate;
pub mod colors;
pub mod config;
pub mod coordinates;
pub mod courses;
pub mod culture_sets;
pub mod cultures;
pub mod development;
pub mod diplomacy;
pub mod economy;
pub mod emblems;
pub mod error;
pub mod features;
pub mod fit;
pub mod geom;
pub mod geonames;
pub mod grid;
pub mod heightmap;
pub mod ice;
pub mod journeys;
pub mod lakes;
pub mod markers;
pub mod meander;
pub mod military;
pub mod names;
pub mod num;
pub mod overlay;
pub mod pack;
pub mod population;
pub mod provinces;
pub mod quadtree;
pub mod rand;
pub mod religions;
pub mod rivers;
pub mod roads;
pub mod routes;
pub mod seed;
pub mod society;
pub mod stability;
pub mod states;
pub mod world;
