use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
}

pub fn read_directory(path: &Path) -> Result<(Vec<Entry>, usize), String> {
    let mut entries = Vec::new();
    let mut unreadable = 0;
    for result in fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))? {
        let entry = match result {
            Ok(entry) => entry,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(m) => m,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        let is_symlink = metadata.file_type().is_symlink();
        // Browsing follows a user-selected directory link; scans do not recurse here.
        let is_dir = metadata.is_dir() || (is_symlink && entry.path().is_dir());
        entries.push(Entry {
            path: entry.path(),
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir,
            is_symlink,
            bytes: metadata.len(),
            modified: metadata.modified().ok(),
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok((entries, unreadable))
}

#[derive(Clone, Debug)]
pub enum Preview {
    Text(String),
    Image(PathBuf),
    Binary(String),
}

pub fn preview(path: &Path) -> Result<Preview, String> {
    if !fs::metadata(path).map_err(|e| e.to_string())?.is_file() {
        return Err("Preview is available for regular files only.".into());
    }
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
    ) {
        return Ok(Preview::Image(path.to_owned()));
    }
    let mut data = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(32769)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    let truncated = data.len() > 32768;
    data.truncate(32768);
    if !data.contains(&0) {
        // A byte limit can cut through the final UTF-8 scalar.
        let text = match std::str::from_utf8(&data) {
            Ok(text) => Some(text),
            Err(error) if truncated && error.error_len().is_none() => {
                std::str::from_utf8(&data[..error.valid_up_to()]).ok()
            }
            Err(_) => None,
        };
        if let Some(text) = text {
            return Ok(Preview::Text(format!(
                "{text}{}",
                if truncated {
                    "\n\n… Preview limited to 32 KiB."
                } else {
                    ""
                }
            )));
        }
    }
    Ok(Preview::Binary(
        data.iter()
            .take(256)
            .collect::<Vec<_>>()
            .chunks(16)
            .map(|row| {
                row.iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n"),
    ))
}

#[derive(Clone, Debug)]
pub struct History {
    pub paths: Vec<PathBuf>,
    pub index: usize,
}
impl History {
    pub fn new(path: PathBuf) -> Self {
        Self {
            paths: vec![path],
            index: 0,
        }
    }
    pub fn current(&self) -> &Path {
        &self.paths[self.index]
    }
    pub fn navigate(&mut self, path: PathBuf) {
        if self.current() == path {
            return;
        }
        self.paths.truncate(self.index + 1);
        self.paths.push(path);
        self.index += 1;
    }
    pub fn back(&mut self) {
        self.index = self.index.saturating_sub(1);
    }
    pub fn forward(&mut self) {
        self.index = (self.index + 1).min(self.paths.len() - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_discards_forward_branch_after_navigation() {
        let mut h = History::new("a".into());
        h.navigate("b".into());
        h.navigate("c".into());
        h.back();
        h.navigate("d".into());
        h.forward();
        assert_eq!(h.current(), Path::new("d"));
        assert_eq!(h.paths, vec![PathBuf::from("a"), "b".into(), "d".into()]);
    }
    #[test]
    fn browsing_orders_folders_and_bounds_binary_and_text_previews() {
        let root = std::env::temp_dir().join(format!("rsx-browser-{}", std::process::id()));
        fs::create_dir_all(root.join("z-folder")).unwrap();
        fs::write(root.join("a.txt"), vec![b'x'; 50000]).unwrap();
        fs::write(root.join("binary"), [0, 255, 16]).unwrap();
        let (entries, failures) = read_directory(&root).unwrap();
        assert_eq!(entries[0].name, "z-folder");
        assert_eq!(failures, 0);
        assert!(
            matches!(preview(&root.join("a.txt")).unwrap(), Preview::Text(text) if text.contains("limited to 32 KiB") && text.len() < 33000)
        );
        assert!(
            matches!(preview(&root.join("binary")).unwrap(), Preview::Binary(text) if text == "00 ff 10")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
