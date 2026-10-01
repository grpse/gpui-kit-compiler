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
    let source = args
        .next()
        .ok_or("usage: gpui-rsc [compile|build|run|dev] <component-dir>")?;
    let source = fs::canonicalize(source).map_err(|error| error.to_string())?;
    if !source.join("app.rsc").exists() {
        return Err(format!("{} needs app.rsc", source.display()));
    }
    let name = source
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("invalid source directory")?
        .replace('_', "-");
    let generated = root.join("target/rsc-build").join(&name);
    let package = format!("{name}-rsc-app");
    match mode.as_str() {
        "compile" => compile(&root, &source, &generated, &package),
        "build" => build(&root, &source, &generated, &package),
        "run" => {
            build(&root, &source, &generated, &package)?;
            let mut child = launch(&root, &package)?;
            child.wait().map_err(|e| e.to_string())?;
            Ok(())
        }
        "dev" => {
            build(&root, &source, &generated, &package)?;
            let mut child = launch(&root, &package)?;
            let mut previous = source_hash(&root, &source);
            println!("Watching .rsc and compiler sources. Press Ctrl+C to stop.");
            loop {
                thread::sleep(Duration::from_millis(500));
                let current = source_hash(&root, &source);
                if current != previous {
                    match build(&root, &source, &generated, &package) {
                        Ok(()) => {
                            let _ = child.kill();
                            let _ = child.wait();
                            child = launch(&root, &package)?;
                        }
                        Err(error) => {
                            eprintln!("Rebuild failed; previous app remains open: {error}")
                        }
                    }
                    previous = source_hash(&root, &source);
                }
            }
        }
        _ => Err("usage: gpui-rsc [compile|build|run|dev] <component-dir>".into()),
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
fn compile(root: &Path, source: &Path, generated: &Path, package: &str) -> Result<(), String> {
    let files = gpui_rsc::compile_directory(source, &generated.join("generated"))?;
    let modules=files.iter().map(|file| {
        let filename=file.file_name().unwrap().to_string_lossy();
        let stem=filename.trim_end_matches(".inter.rs").replace('-',"_");
        format!("pub mod {stem} {{ include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/generated/{filename}\")); }}\n")
    }).collect::<String>();
    write_if_changed(&generated.join("src/generated.rs"), &modules)?;
    for relative in [
        "src/template.rs",
        "src/runtime/mod.rs",
        "src/runtime/model.rs",
        "src/runtime/component.rs",
        "src/runtime/binding.rs",
        "src/runtime/view.rs",
        "build.rs",
    ] {
        let source = fs::read_to_string(root.join(relative)).map_err(|error| error.to_string())?;
        write_if_changed(&generated.join(relative), &source)?;
    }
    let main = "extern crate self as gpui_rsc;\nmod template;\npub use template::*;\npub mod runtime;\nmod generated;\nfn main() { runtime::run(generated::app::AppComponent::definition()); }\n";
    write_if_changed(&generated.join("src/main.rs"), main)?;
    let default_features = if cfg!(feature = "debug-fps") {
        "[\"debug-fps\"]"
    } else {
        "[]"
    };
    let manifest = format!(
        "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n\n[features]\ndefault = {default_features}\ndebug-fps = []\n\n[dependencies]\ngpui = {{ package = \"gpui-pre\", version = \"=0.3.7\" }}\ngpui-kit = \"0.7\"\n"
    );
    write_if_changed(&generated.join("Cargo.toml"), &manifest)?;
    for file in files {
        println!("generated {}", file.display());
    }
    Ok(())
}
fn build(root: &Path, source: &Path, generated: &Path, package: &str) -> Result<(), String> {
    compile(root, source, generated, package)?;
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
fn source_hash(root: &Path, source: &Path) -> u64 {
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
    visit(source, &mut h);
    h.finish()
}
