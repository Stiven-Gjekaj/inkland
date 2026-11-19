//! Put the build of inkland in `INKLAND_VERSION`.
//!
//! A build at a release tag `v<version>` gets the package version, for
//! example `1.3.1`. Another build gets the number of commits after the last
//! release tag and the commit, for example `1.3.1+2-gb12d578`. Without git,
//! the build gets the package version.

use std::path::PathBuf;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    Some(text.trim().to_owned())
}

/// The version for the package version `pkg` and the output of
/// `git describe --tags --long`, for example `v1.3.1-2-gb12d578`.
fn version(pkg: &str, describe: Option<&str>) -> String {
    let Some(d) = describe else {
        return pkg.to_owned();
    };
    let mut parts = d.rsplitn(3, '-');
    let (Some(hash), Some(count), Some(tag)) = (parts.next(), parts.next(), parts.next()) else {
        return pkg.to_owned();
    };
    if tag == format!("v{pkg}") && count == "0" {
        pkg.to_owned()
    } else {
        format!("{pkg}+{count}-{hash}")
    }
}

fn main() {
    let pkg = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let describe = git(&["describe", "--tags", "--long", "--match", "v*"]);
    println!(
        "cargo:rustc-env=INKLAND_VERSION={}",
        version(&pkg, describe.as_deref())
    );
    // Build again when a commit or a tag changes.
    if let Some(dir) = git(&["rev-parse", "--git-dir"]) {
        println!(
            "cargo:rerun-if-changed={}",
            PathBuf::from(dir).join("HEAD").display()
        );
    }
    if let Some(dir) = git(&["rev-parse", "--git-common-dir"]) {
        let dir = PathBuf::from(dir);
        println!("cargo:rerun-if-changed={}", dir.join("refs").display());
        println!(
            "cargo:rerun-if-changed={}",
            dir.join("packed-refs").display()
        );
    }
    println!("cargo:rerun-if-changed=build.rs");
}
