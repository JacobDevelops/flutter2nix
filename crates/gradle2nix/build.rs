use std::path::Path;

fn main() {
    let jar_path = Path::new("../../tapi-shim/build/libs/tapi-shim.jar");

    if !jar_path.exists() {
        panic!(
            "tapi-shim JAR not found at {}\n\
             Run: cd tapi-shim && gradle build\n\
             This must be done before `cargo build -p gradle2nix`.",
            jar_path.display()
        );
    }

    // Tell Cargo to re-run build.rs if the JAR changes
    println!("cargo:rerun-if-changed=../../tapi-shim/build/libs/tapi-shim.jar");
    println!("cargo:rerun-if-changed=build.rs");
}
