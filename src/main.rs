use std::{
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::Duration,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("rsc: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "run".into());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example = root.join(args.next().unwrap_or_else(|| "examples/coffee".into()));
    if !example.join("app.rsc").exists() {
        return Err(format!("{} needs app.rsc", example.display()));
    }
    let name = example
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("invalid example directory")?
        .replace('_', "-");
    let generated = root.join("target/rsc-build").join(&name);
    let package = format!("{name}-rsc-app");
    match mode.as_str() {
        "compile" => compile(&root, &example, &generated, &package),
        "build" => build(&root, &example, &generated, &package),
        "run" => {
            build(&root, &example, &generated, &package)?;
            let mut child = launch(&root, &package)?;
            child.wait().map_err(|e| e.to_string())?;
            Ok(())
        }
        "dev" => {
            build(&root, &example, &generated, &package)?;
            let mut child = launch(&root, &package)?;
            let mut previous = source_hash(&root, &example);
            println!("Watching .rsc and compiler sources. Press Ctrl+C to stop.");
            loop {
                thread::sleep(Duration::from_millis(500));
                let current = source_hash(&root, &example);
                if current != previous {
                    match build(&root, &example, &generated, &package) {
                        Ok(()) => {
                            let _ = child.kill();
                            let _ = child.wait();
                            child = launch(&root, &package)?;
                        }
                        Err(error) => {
                            eprintln!("Rebuild failed; previous app remains open: {error}")
                        }
                    }
                    previous = source_hash(&root, &example);
                }
            }
        }
        _ => Err("usage: gpui-rsc [compile|build|run|dev] [examples/coffee]".into()),
    }
}
fn write_if_changed(path: &Path, content: &str) -> Result<(), String> {
    if fs::read_to_string(path).ok().as_deref() != Some(content) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, content).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn compile(root: &Path, example: &Path, generated: &Path, package: &str) -> Result<(), String> {
    let files = gpui_rsc::compile_directory(example, &generated.join("generated"))?;
    let modules=files.iter().map(|file| {
        let filename=file.file_name().unwrap().to_string_lossy();
        let stem=filename.trim_end_matches(".inter.rs").replace('-',"_");
        format!("pub mod {stem} {{ include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{filename}\")); }}\n")
    }).collect::<String>();
    write_if_changed(&generated.join("src/generated.rs"), &modules)?;
    let main = "mod generated;\nfn main() { gpui_rsc::runtime::run(generated::app::AppComponent::definition()); }\n";
    write_if_changed(&generated.join("src/main.rs"), main)?;
    let features = if cfg!(feature = "debug-fps") {
        ", features = [\"debug-fps\"]"
    } else {
        ""
    };
    let manifest = format!(
        "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n\n[dependencies]\ngpui-rsc = {{ path = {:?}{features} }}\n",
        root.display().to_string()
    );
    write_if_changed(&generated.join("Cargo.toml"), &manifest)?;
    for file in files {
        println!("generated {}", file.display());
    }
    Ok(())
}
fn build(root: &Path, example: &Path, generated: &Path, package: &str) -> Result<(), String> {
    compile(root, example, generated, package)?;
    let status = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(generated.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(root.join("target"))
        .current_dir(root)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("Cargo build failed: {status}"));
    }
    let status = Command::new(root.join("target/debug").join(package))
        .arg("--validate")
        .current_dir(root)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("component binding validation failed: {status}"))
    }
}
fn launch(root: &Path, package: &str) -> Result<Child, String> {
    Command::new(root.join("target/debug").join(package))
        .current_dir(root)
        .spawn()
        .map_err(|e| e.to_string())
}
fn source_hash(root: &Path, example: &Path) -> u64 {
    fn visit(path: &Path, h: &mut impl Hasher) {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    visit(&p, h);
                } else if matches!(
                    p.extension().and_then(|s| s.to_str()),
                    Some("rs" | "rsc" | "toml")
                ) {
                    p.hash(h);
                    if let Ok(bytes) = fs::read(&p) {
                        bytes.hash(h);
                    }
                }
            }
        }
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    visit(&root.join("src"), &mut h);
    visit(example, &mut h);
    h.finish()
}
