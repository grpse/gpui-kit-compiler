use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    pub appearance: Appearance,
    pub allocated: bool,
}

impl Preferences {
    pub fn path() -> Option<PathBuf> {
        Self::path_for("rsx-disk-explorer")
    }

    pub fn path_for(application: &str) -> Option<PathBuf> {
        let base = if cfg!(target_os = "macos") {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join("Library/Application Support"))
        } else if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
                })
        };
        base.map(|base| base.join(application).join("preferences"))
    }

    pub fn load(path: &Path) -> Self {
        let mut preferences = Self::default();
        if let Ok(text) = fs::read_to_string(path) {
            for line in text.lines() {
                match line.split_once('=') {
                    Some(("appearance", "light")) => preferences.appearance = Appearance::Light,
                    Some(("appearance", "dark")) => preferences.appearance = Appearance::Dark,
                    Some(("appearance", "system")) => preferences.appearance = Appearance::System,
                    Some(("metric", "allocated")) => preferences.allocated = true,
                    Some(("metric", "logical")) => preferences.allocated = false,
                    _ => {}
                }
            }
        }
        preferences
    }

    pub fn save(self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(
            &temporary,
            format!(
                "appearance={}\nmetric={}\n",
                self.appearance.key(),
                if self.allocated {
                    "allocated"
                } else {
                    "logical"
                }
            ),
        )?;
        fs::rename(&temporary, path).inspect_err(|_| {
            let _ = fs::remove_file(&temporary);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip_and_ignore_unknown_values() {
        let path = std::env::temp_dir().join(format!("disk-preferences-{}", std::process::id()));
        assert_eq!(Preferences::load(&path), Preferences::default());
        for appearance in [Appearance::System, Appearance::Light, Appearance::Dark] {
            let preferences = Preferences {
                appearance,
                allocated: true,
            };
            preferences.save(&path).unwrap();
            assert_eq!(Preferences::load(&path), preferences);
        }
        fs::write(&path, "appearance=invalid\nmetric=invalid\nfuture=value\n").unwrap();
        assert_eq!(Preferences::load(&path), Preferences::default());
        fs::remove_file(path).unwrap();
    }
}
