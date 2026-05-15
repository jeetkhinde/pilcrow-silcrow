use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));

    if let Err(err) = routekit::compile_to_out_dir(&manifest_dir, &out_dir) {
        eprintln!("Pilcrow template compile error:");
        eprintln!("{err}");
        std::process::exit(1);
    }

    for dir in routekit::watched_source_directories(&manifest_dir) {
        println!("cargo:rerun-if-changed={}", dir.display());
    }
}
