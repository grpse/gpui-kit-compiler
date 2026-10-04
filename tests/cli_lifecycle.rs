#![cfg(all(feature = "compiler", unix))]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("gpui-rsc-{name}-{}", std::process::id()));
        fs::create_dir_all(directory.join("src")).unwrap();
        fs::create_dir_all(directory.join("target/debug")).unwrap();
        let directory = fs::canonicalize(directory).unwrap();
        let manifest = directory.join("Cargo.toml");
        let main = directory.join("src/main.rs");
        fs::write(
            &manifest,
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(&main, "fn main() {}\n").unwrap();
        fs::write(
            directory.join("src/app.rsx"),
            "pub fn App() -> gpui::AnyElement { <div>Hello</div> }\n",
        )
        .unwrap();
        fs::write(directory.join("metadata.json"), serde_json::json!({
            "packages": [{ "manifest_path": manifest, "targets": [{"kind": ["bin"], "src_path": main, "name": "fixture"}] }],
            "target_directory": directory.join("target")
        }).to_string()).unwrap();
        let cargo = directory.join("selected-cargo");
        fs::write(&cargo, "#!/bin/sh\ncase \"$1\" in\nmetadata) /bin/cat \"$FIXTURE_METADATA\" ;;\nbuild) exit 0 ;;\n*) exit 42 ;;\nesac\n").unwrap();
        let binary = directory.join("target/debug/fixture");
        fs::write(&binary, "#!/bin/sh\nif [ \"$1\" = \"--validate\" ]; then exit \"$FIXTURE_VALIDATION_CODE\"; fi\nexit \"$FIXTURE_APP_CODE\"\n").unwrap();
        for executable in [cargo, binary] {
            fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self(directory)
    }

    fn run(&self, mode: &str, validation_code: i32, app_code: i32) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gpui-rsc"))
            .arg(mode)
            .arg(&self.0)
            .env("CARGO", self.0.join("selected-cargo"))
            .env("FIXTURE_METADATA", self.0.join("metadata.json"))
            .env("FIXTURE_VALIDATION_CODE", validation_code.to_string())
            .env("FIXTURE_APP_CODE", app_code.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if child.try_wait().unwrap().is_some() {
                return child.wait_with_output().unwrap();
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "{mode} did not observe app exit: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn run_and_dev_propagate_app_failures_and_finish_after_normal_quit() {
    let fixture = Fixture::new("cli-app-exit");
    for mode in ["run", "dev"] {
        let normal = fixture.run(mode, 0, 0);
        assert!(
            normal.status.success(),
            "{}",
            String::from_utf8_lossy(&normal.stderr)
        );
        let failure = fixture.run(mode, 0, 17);
        assert!(!failure.status.success());
        let error = String::from_utf8_lossy(&failure.stderr);
        assert!(error.contains("application quit unexpectedly"), "{error}");
        assert!(error.contains("17"), "{error}");
    }
}

#[test]
fn invalid_bindings_stop_launch_and_keep_the_validation_diagnostic() {
    let fixture = Fixture::new("cli-validation-exit");
    let output = fixture.run("run", 9, 17);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("component binding validation failed"),
        "{error}"
    );
    assert!(!error.contains("application quit unexpectedly"), "{error}");
}
