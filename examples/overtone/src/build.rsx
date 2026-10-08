use std::{env, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for module in [
        "model",
        "waveform",
        "reconstruction",
        "reconstruction_ui",
        "engine",
        "engine_tests",
        "audio",
        "capture",
        "analysis",
        "analysis_tests",
        "storage",
        "persistence",
        "studio",
        "capture_ui",
        "primitives",
        "panels",
        "workspace",
        "settings",
        "ui_tests",
    ] {
        let source = root.join(format!("src/{module}.rsx"));
        println!("cargo:rerun-if-changed={}", source.display());
        gpui_rsc::convert_file(&source, &out.join(format!("{module}.rs")))
            .unwrap_or_else(|error| panic!("{}: {error}", source.display()));
    }
}
