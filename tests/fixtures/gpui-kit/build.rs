fn main() {
    println!("cargo:rerun-if-changed=src/components.rsx");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    gpui_rsc::compile_file(
        std::path::Path::new("src/components.rsx"),
        &output.join("components.rs"),
    )
    .expect("GPUI Kit component RSX must compile");
}
