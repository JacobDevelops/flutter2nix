use std::path::Path;
use std::time::SystemTime;

/// Latest modification time of any file under `dir` (recursive).
fn newest_mtime(dir: &Path) -> Option<SystemTime> {
    let mut newest: Option<SystemTime> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let mtime = if path.is_dir() {
            newest_mtime(&path)
        } else {
            entry.metadata().ok().and_then(|m| m.modified().ok())
        };
        if let Some(t) = mtime {
            newest = Some(newest.map_or(t, |n| n.max(t)));
        }
    }
    newest
}

fn main() {
    let jar_path = Path::new("../../tapi-shim/build/libs/tapi-shim.jar");
    let shim_src = Path::new("../../tapi-shim/src");

    if !jar_path.exists() {
        panic!(
            "tapi-shim JAR not found at {}\n\
             Run: cd tapi-shim && gradle build\n\
             This must be done before `cargo build -p gradle2nix`.",
            jar_path.display()
        );
    }

    // Guard against embedding a stale JAR: cargo only sees the JAR file, so an
    // edit to the Kotlin shim or the bundled init script is invisible until the
    // JAR is rebuilt — fail loudly instead of silently shipping old behavior.
    let jar_mtime = std::fs::metadata(jar_path)
        .and_then(|m| m.modified())
        .expect("tapi-shim JAR mtime");
    if let Some(src_mtime) = newest_mtime(shim_src) {
        if src_mtime > jar_mtime {
            panic!(
                "tapi-shim sources are newer than {}\n\
                 Rebuild the shim: cd tapi-shim && gradle jar",
                jar_path.display()
            );
        }
    }

    // Re-run if the JAR changes, or if shim sources change (to re-check staleness)
    println!("cargo:rerun-if-changed=../../tapi-shim/build/libs/tapi-shim.jar");
    println!("cargo:rerun-if-changed=../../tapi-shim/src");
    println!("cargo:rerun-if-changed=build.rs");
}
