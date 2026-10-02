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
    if !source.join("app.rsx").exists() {
        return Err(format!("{} needs app.rsx", source.display()));
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
            println!("Watching .rsx and compiler sources. Press Ctrl+C to stop.");
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

#[derive(Clone, Copy)]
struct AppStartupConfig {
    width: f32,
    height: f32,
    min_width: Option<f32>,
    min_height: Option<f32>,
    state: &'static str,
    decorations: &'static str,
    resizable: bool,
    minimizable: bool,
    movable: bool,
    focus: bool,
    show: bool,
}

impl Default for AppStartupConfig {
    fn default() -> Self {
        Self {
            width: 1240.0,
            height: 870.0,
            min_width: None,
            min_height: None,
            state: "Windowed",
            decorations: "Client",
            resizable: true,
            minimizable: true,
            movable: true,
            focus: true,
            show: true,
        }
    }
}

impl AppStartupConfig {
    fn rust_expression(self) -> String {
        let option = |value: Option<f32>| match value {
            Some(value) => format!("Some({value:?})"),
            None => "None".into(),
        };
        format!(
            "runtime::StartupConfig {{ width: {:?}, height: {:?}, min_width: {}, min_height: {}, state: runtime::StartupWindowState::{}, decorations: runtime::StartupDecorations::{}, resizable: {}, minimizable: {}, movable: {}, focus: {}, show: {} }}",
            self.width,
            self.height,
            option(self.min_width),
            option(self.min_height),
            self.state,
            self.decorations,
            self.resizable,
            self.minimizable,
            self.movable,
            self.focus,
            self.show,
        )
    }
}

fn read_startup_config(source: &Path) -> Result<AppStartupConfig, String> {
    let path = source.join("gpui-rsc.toml");
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppStartupConfig::default());
        }
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let document = contents
        .parse::<toml::Value>()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let table = document
        .as_table()
        .ok_or_else(|| format!("{} must contain a TOML table", path.display()))?;
    for key in table.keys() {
        if key != "window" {
            return Err(format!(
                "{}: unsupported top-level key `{key}` (expected [window])",
                path.display()
            ));
        }
    }
    let Some(window) = table.get("window") else {
        return Ok(AppStartupConfig::default());
    };
    let window = window
        .as_table()
        .ok_or_else(|| format!("{}: [window] must be a table", path.display()))?;
    const SUPPORTED: &[&str] = &[
        "width",
        "height",
        "min_width",
        "min_height",
        "state",
        "decorations",
        "resizable",
        "minimizable",
        "movable",
        "focus",
        "show",
    ];
    for key in window.keys() {
        if !SUPPORTED.contains(&key.as_str()) {
            return Err(format!(
                "{}: unsupported [window] key `{key}`",
                path.display()
            ));
        }
    }

    let mut config = AppStartupConfig::default();
    if let Some(value) = window.get("width") {
        config.width = parse_dimension(value, "width", &path)?;
    }
    if let Some(value) = window.get("height") {
        config.height = parse_dimension(value, "height", &path)?;
    }
    for (key, target) in [
        ("min_width", &mut config.min_width),
        ("min_height", &mut config.min_height),
    ] {
        if let Some(value) = window.get(key) {
            *target = Some(parse_dimension(value, key, &path)?);
        }
    }
    if config.min_width.is_some() != config.min_height.is_some() {
        return Err(format!(
            "{}: `min_width` and `min_height` must be configured together",
            path.display()
        ));
    }
    if let Some(value) = window.get("state") {
        config.state = match value.as_str() {
            Some("windowed") => "Windowed",
            Some("maximized") => "Maximized",
            Some("fullscreen") => "Fullscreen",
            Some(value) => {
                return Err(format!(
                    "{}: invalid [window].state `{value}` (expected windowed, maximized, or fullscreen)",
                    path.display()
                ));
            }
            None => {
                return Err(format!(
                    "{}: [window].state must be a string",
                    path.display()
                ));
            }
        };
    }
    if let Some(value) = window.get("decorations") {
        config.decorations = match value.as_str() {
            Some("client") => "Client",
            Some("server") => "Server",
            Some(value) => {
                return Err(format!(
                    "{}: invalid [window].decorations `{value}` (expected client or server)",
                    path.display()
                ));
            }
            None => {
                return Err(format!(
                    "{}: [window].decorations must be a string",
                    path.display()
                ));
            }
        };
    }
    for (key, target) in [
        ("resizable", &mut config.resizable),
        ("minimizable", &mut config.minimizable),
        ("movable", &mut config.movable),
        ("focus", &mut config.focus),
        ("show", &mut config.show),
    ] {
        if let Some(value) = window.get(key) {
            *target = value.as_bool().ok_or_else(|| {
                format!("{}: [window].{key} must be true or false", path.display())
            })?;
        }
    }
    Ok(config)
}

fn parse_dimension(value: &toml::Value, key: &str, path: &Path) -> Result<f32, String> {
    let dimension = match value {
        toml::Value::Integer(value) => *value as f32,
        toml::Value::Float(value) => *value as f32,
        _ => {
            return Err(format!(
                "{}: [window].{key} must be a positive number",
                path.display()
            ));
        }
    };
    if !dimension.is_finite() || dimension <= 0.0 {
        return Err(format!(
            "{}: [window].{key} must be a positive number",
            path.display()
        ));
    }
    Ok(dimension)
}

fn compile(root: &Path, source: &Path, generated: &Path, package: &str) -> Result<(), String> {
    let startup = read_startup_config(source)?;
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
    let main = format!(
        "extern crate self as gpui_rsc;\nmod template;\npub use template::*;\npub mod runtime;\nmod generated;\nfn main() {{ runtime::run_with_config(generated::app::App(), {}); }}\n",
        startup.rust_expression()
    );
    write_if_changed(&generated.join("src/main.rs"), &main)?;
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
                    Some("rs" | "rsx" | "toml")
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
