#![cfg(all(feature = "compiler", unix))]

use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[test]
fn nested_metadata_and_build_use_the_launching_cargo() {
    let directory =
        std::env::temp_dir().join(format!("gpui-rsc-cli-toolchain-{}", std::process::id()));
    fs::create_dir_all(directory.join("src")).unwrap();
    let directory = fs::canonicalize(directory).unwrap();
    let manifest = directory.join("Cargo.toml");
    let main_rs = directory.join("src/main.rs");
    fs::write(
        &manifest,
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(&main_rs, "fn main() {}\n").unwrap();
    fs::write(
        directory.join("src/app.rsx"),
        "pub fn App() -> gpui::AnyElement { <div>Hello</div> }\n",
    )
    .unwrap();
    let metadata = directory.join("metadata.json");
    fs::write(
        &metadata,
        serde_json::json!({
            "packages": [{
                "manifest_path": manifest,
                "targets": [{"kind": ["bin"], "src_path": main_rs, "name": "fixture"}]
            }],
            "target_directory": directory.join("target")
        })
        .to_string(),
    )
    .unwrap();
    let cargo = directory.join("selected-cargo");
    let calls = directory.join("calls.txt");
    fs::write(
        &cargo,
        "#!/bin/sh\nprintf '%s\\n' \"$1\" >> \"$FIXTURE_CALLS\"\ncase \"$1\" in\nmetadata) /bin/cat \"$FIXTURE_METADATA\" ;;\nbuild) exit 41 ;;\n*) exit 42 ;;\nesac\n",
    )
    .unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_gpui-rsc"))
        .arg("build")
        .arg(&directory)
        .env("CARGO", &cargo)
        .env("PATH", directory.join("no-tools"))
        .env("FIXTURE_CALLS", &calls)
        .env("FIXTURE_METADATA", &metadata)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Cargo build failed: exit status: 41"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(calls).unwrap(), "metadata\nbuild\n");
    fs::remove_dir_all(directory).unwrap();
}
