use std::{
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::Duration,
};

struct RustProject {
    root: PathBuf,
    manifest: PathBuf,
    target_directory: PathBuf,
    binary: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("rsc: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "run".into());
    let source = args.next().ok_or(
        "usage: gpui-rsc [compile|build|run|dev] <cargo-project-dir> [cargo build options...]",
    )?;
    let root = fs::canonicalize(source).map_err(|error| error.to_string())?;
    let project = load_project(&root)?;
    let cargo_args = args.collect::<Vec<_>>();

    match mode.as_str() {
        "compile" => {
            if !cargo_args.is_empty() {
                return Err("cargo build options are only accepted by build, run, and dev".into());
            }
            compile_project(&project)
        }
        "build" => {
            build(&project, &cargo_args)?;
            Ok(())
        }
        "run" => {
            let binary = build(&project, &cargo_args)?;
            let mut child = launch(&root, &binary)?;
            child.wait().map_err(|error| error.to_string())?;
            Ok(())
        }
        "dev" => {
            let binary = build(&project, &cargo_args)?;
            let mut child = launch(&root, &binary)?;
            let mut previous = source_hash(&root, &project.root);
            println!("Watching .rsx, Rust, and Cargo sources. Press Ctrl+C to stop.");
            loop {
                thread::sleep(Duration::from_millis(500));
                let current = source_hash(&root, &project.root);
                if current != previous {
                    match build(&project, &cargo_args) {
                        Ok(binary) => {
                            let _ = child.kill();
                            let _ = child.wait();
                            child = launch(&root, &binary)?;
                        }
                        Err(error) => {
                            eprintln!("Rebuild failed; previous app remains open: {error}")
                        }
                    }
                    previous = source_hash(&root, &project.root);
                }
            }
        }
        _ => Err(
            "usage: gpui-rsc [compile|build|run|dev] <cargo-project-dir> [cargo build options...]"
                .into(),
        ),
    }
}

fn load_project(root: &Path) -> Result<RustProject, String> {
    let manifest = root.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!("{} needs Cargo.toml", root.display()));
    }
    let main_rs = root.join("src/main.rs");
    if !main_rs.is_file() {
        return Err(format!("{} needs src/main.rs", root.display()));
    }

    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(&manifest)
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("could not read cargo metadata: {error}"))?;
    let manifest = fs::canonicalize(&manifest).map_err(|error| error.to_string())?;
    let manifest_text = manifest.to_string_lossy();
    let package = metadata["packages"]
        .as_array()
        .and_then(|packages| {
            packages
                .iter()
                .find(|package| package["manifest_path"].as_str() == Some(manifest_text.as_ref()))
        })
        .ok_or("Cargo.toml does not declare a package in this project")?;
    let main_rs = fs::canonicalize(main_rs).map_err(|error| error.to_string())?;
    let main_rs_text = main_rs.to_string_lossy();
    let binary = package["targets"]
        .as_array()
        .and_then(|targets| {
            targets.iter().find(|target| {
                let is_binary = target["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")));
                is_binary && target["src_path"].as_str() == Some(main_rs_text.as_ref())
            })
        })
        .and_then(|target| target["name"].as_str())
        .ok_or("Cargo.toml must define a binary target at src/main.rs")?
        .to_owned();
    let target_directory = metadata["target_directory"]
        .as_str()
        .ok_or("cargo metadata did not return a target directory")?;

    Ok(RustProject {
        root: root.to_path_buf(),
        manifest,
        target_directory: PathBuf::from(target_directory),
        binary,
    })
}

fn build(project: &RustProject, cargo_args: &[String]) -> Result<PathBuf, String> {
    compile_project(project)?;
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(&project.manifest)
        .arg("--bin")
        .arg(&project.binary)
        .args(cargo_args)
        .current_dir(&project.root);
    let status = command.status().map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!("Cargo build failed: {status}"));
    }

    let binary = binary_path(project, cargo_args);
    let status = Command::new(&binary)
        .arg("--validate")
        .current_dir(&project.root)
        .status()
        .map_err(|error| format!("could not validate {}: {error}", binary.display()))?;
    if status.success() {
        Ok(binary)
    } else {
        Err(format!("component binding validation failed: {status}"))
    }
}

fn compile_project(project: &RustProject) -> Result<(), String> {
    let source = project.root.join("src");
    let output = project.root.join("target/rsc-build/generated");
    let files = gpui_rsc::compile_directory(&source, &output)?;
    let mut modules = String::new();
    let mut module_names = std::collections::HashSet::new();
    for file in &files {
        let filename = file
            .file_name()
            .expect("generated component file name")
            .to_string_lossy();
        let stem = filename.trim_end_matches(".inter.rs").replace('-', "_");
        if !module_names.insert(stem.clone()) {
            return Err(format!(
                "multiple .rsx files in {} map to generated module `{stem}`",
                source.display()
            ));
        }
        let relative = file
            .strip_prefix(&output)
            .map_err(|error| {
                format!(
                    "could not map {} into generated tree: {error}",
                    file.display()
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let include_path = format!("/target/rsc-build/generated/{relative}");
        modules.push_str(&format!(
            "pub mod {stem} {{ include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {include_path:?})); }}\n"
        ));
    }
    let generated_module = project.root.join("target/rsc-build/generated.rs");
    if let Some(parent) = generated_module.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    if fs::read_to_string(&generated_module).ok().as_deref() != Some(&modules) {
        fs::write(&generated_module, modules)
            .map_err(|error| format!("could not write {}: {error}", generated_module.display()))?;
    }
    for file in files {
        println!("generated {}", file.display());
    }
    println!("generated {}", generated_module.display());
    Ok(())
}

fn binary_path(project: &RustProject, cargo_args: &[String]) -> PathBuf {
    let profile = if cargo_args.iter().any(|arg| arg == "--release") {
        "release"
    } else if let Some(index) = cargo_args.iter().position(|arg| arg == "--profile") {
        cargo_args
            .get(index + 1)
            .map(String::as_str)
            .unwrap_or("debug")
    } else {
        "debug"
    };
    let executable = if cfg!(windows) {
        format!("{}.exe", project.binary)
    } else {
        project.binary.clone()
    };
    project.target_directory.join(profile).join(executable)
}

fn launch(root: &Path, binary: &Path) -> Result<Child, String> {
    Command::new(binary)
        .current_dir(root)
        .spawn()
        .map_err(|error| format!("could not start {}: {error}", binary.display()))
}

fn source_hash(compiler_root: &Path, project_root: &Path) -> u64 {
    fn visit(path: &Path, hasher: &mut impl Hasher) {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("target" | ".git")
                ) {
                    visit(&path, hasher);
                }
            } else if matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("rs" | "rsx" | "toml")
            ) || path.file_name().and_then(|name| name.to_str()) == Some("Cargo.lock")
            {
                path.hash(hasher);
                if let Ok(bytes) = fs::read(&path) {
                    bytes.hash(hasher);
                }
            }
        }
    }

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    visit(&compiler_root.join("src"), &mut hasher);
    visit(project_root, &mut hasher);
    hasher.finish()
}
