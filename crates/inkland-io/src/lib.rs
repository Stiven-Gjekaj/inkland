//! File formats of inkland: config, world, report, schema, and the import
//! of Azgaar maps.

// The json! value of world.json has many keys.
#![recursion_limit = "256"]

pub mod azgaar;
pub mod config;
pub mod geojson;
pub mod query;
pub mod report;
pub mod schema;
pub mod world;
