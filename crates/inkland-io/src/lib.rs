//! File formats of inkland: config, world, report, schema, and the import
//! of Azgaar maps.

// The json! value of world.json has many keys.
#![recursion_limit = "256"]

/// The version of this build: the package version at a release tag, else
/// the package version, the number of commits after the last release tag,
/// and the commit, for example `1.3.1+2-gb12d578`. The tool writes it to
/// `--version`, `report.tool`, and `world.json`, so that a map records
/// which build drew it.
pub const VERSION: &str = env!("INKLAND_VERSION");

pub mod azgaar;
pub mod config;
pub mod geojson;
pub mod heights;
pub mod query;
pub mod report;
pub mod schema;
pub mod story;
pub mod world;

#[cfg(test)]
mod tests {
    #[test]
    fn the_version_is_the_package_version_or_a_build_after_it() {
        let pkg = env!("CARGO_PKG_VERSION");
        if super::VERSION != pkg {
            let rest = super::VERSION
                .strip_prefix(&format!("{pkg}+"))
                .expect("the package version first");
            let (count, hash) = rest.split_once("-g").expect("N-gHASH");
            assert!(count.parse::<u32>().is_ok(), "{rest}");
            assert!(
                hash.len() >= 7 && hash.chars().all(|c| c.is_ascii_hexdigit()),
                "{rest}"
            );
        }
    }
}
