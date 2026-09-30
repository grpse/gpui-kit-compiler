#[cfg(target_os = "linux")]
fn main() {
    use std::{env, fs, os::unix::fs::symlink, path::PathBuf, process::Command};

    let out =
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR")).join("native-libs");
    fs::create_dir_all(&out).expect("create native link directory");
    let cache = Command::new("ldconfig")
        .arg("-p")
        .output()
        .ok()
        .filter(|result| result.status.success())
        .map(|result| String::from_utf8_lossy(&result.stdout).into_owned())
        .unwrap_or_default();
    for (link_name, soname) in [
        ("libxcb.so", "libxcb.so.1"),
        ("libxkbcommon.so", "libxkbcommon.so.0"),
        ("libxkbcommon-x11.so", "libxkbcommon-x11.so.0"),
    ] {
        let installed = cache.lines().find_map(|line| {
            let (name, path) = line.trim().split_once(" => ")?;
            name.split_whitespace()
                .next()
                .filter(|name| *name == soname)?;
            Some(PathBuf::from(path))
        });
        let installed = installed.or_else(|| {
            [
                "/usr/lib/x86_64-linux-gnu",
                "/lib/x86_64-linux-gnu",
                "/usr/lib64",
                "/usr/lib",
            ]
            .into_iter()
            .map(|dir| PathBuf::from(dir).join(soname))
            .find(|path| path.exists())
        });
        if let Some(installed) = installed {
            let link = out.join(link_name);
            if link.symlink_metadata().is_ok() {
                fs::remove_file(&link).expect("replace native link");
            }
            symlink(installed, link).expect("create native link");
        } else {
            println!(
                "cargo:warning=Could not find {soname}; install the platform development package if linking fails"
            );
        }
    }
    println!("cargo:rustc-link-search=native={}", out.display());
}

#[cfg(not(target_os = "linux"))]
fn main() {}
