use std::{
    ffi::OsStr,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

mod search;
pub use rsx_disk_explorer::{
    Appearance, Node, Preferences, Scan, ScanProgress, largest_files, scan_with_progress,
};
pub use search::{SearchUpdate, fuzzy_score, search, search_tree};

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub bytes: u64,
    pub allocated: u64,
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
            allocated: allocated_bytes(&metadata),
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

pub fn unique_path(directory: &Path, name: &OsStr) -> PathBuf {
    let mut candidate = directory.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let stem = Path::new(name)
        .file_stem()
        .unwrap_or(name)
        .to_string_lossy();
    let extension = Path::new(name)
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    for index in 2..10_000 {
        candidate = directory.join(format!("{stem} {index}{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{stem} copy{extension}"))
}

fn validate_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a name.".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err("A name cannot contain a path separator.".into());
    }
    if name == "." || name == ".." {
        return Err("That name is reserved.".into());
    }
    Ok(())
}

pub fn rename_entry(path: &Path, new_name: &str) -> Result<PathBuf, String> {
    validate_name(new_name)?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("Cannot rename {}", path.display()))?;
    let destination = parent.join(new_name.trim());
    if destination != path && destination.exists() {
        return Err(format!(
            "{} already exists.",
            destination
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        ));
    }
    fs::rename(path, &destination).map_err(|error| format!("{error}"))?;
    Ok(destination)
}

pub fn create_folder(parent: &Path, name: &str) -> Result<PathBuf, String> {
    validate_name(name)?;
    let destination = unique_path(parent, OsStr::new(name.trim()));
    fs::create_dir(&destination).map_err(|error| format!("{error}"))?;
    Ok(destination)
}

pub fn duplicate_entry(path: &Path) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Cannot duplicate {}", path.display()))?;
    let name = path.file_name().unwrap_or_else(|| OsStr::new("copy"));
    let stem = Path::new(name)
        .file_stem()
        .unwrap_or(name)
        .to_string_lossy();
    let extension = Path::new(name)
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    let destination = unique_path(parent, OsStr::new(&format!("{stem} copy{extension}")));
    copy_item(path, &destination)?;
    Ok(destination)
}

fn copy_item(from: &Path, to: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(from).map_err(|error| format!("{error}"))?;
    if metadata.is_dir() {
        fs::create_dir(to).map_err(|error| format!("{error}"))?;
        for entry in fs::read_dir(from).map_err(|error| format!("{error}"))? {
            let entry = entry.map_err(|error| format!("{error}"))?;
            copy_item(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to)
            .map(|_| ())
            .map_err(|error| format!("{error}"))
    }
}

pub fn delete_permanently(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| format!("{error}"))?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path).map_err(|error| format!("{error}"))
    } else {
        fs::remove_file(path).map_err(|error| format!("{error}"))
    }
}

pub fn move_to_trash(path: &Path) -> Result<(), String> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("Cannot move {} to the Trash", path.display()))?;
    if let Some(trash) = trash_directory() {
        if fs::create_dir_all(&trash).is_ok() {
            let destination = unique_path(&trash, name);
            if fs::rename(path, &destination).is_ok() {
                return Ok(());
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let escaped = path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
        let status = std::process::Command::new("osascript")
            .args([
                "-e",
                &format!("tell application \"Finder\" to delete POSIX file \"{escaped}\""),
            ])
            .status()
            .map_err(|error| format!("{error}"))?;
        if status.success() {
            return Ok(());
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if std::process::Command::new("gio")
            .args(["trash", "--", &path.to_string_lossy()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
        {
            return Ok(());
        }
    }
    Err(format!("Could not move {} to the Trash.", path.display()))
}

fn trash_directory() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        #[cfg(target_os = "macos")]
        {
            return Some(PathBuf::from(home).join(".Trash"));
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Some(PathBuf::from(home).join(".local/share/Trash/files"));
        }
    }
    None
}

pub fn node_at<'a>(node: &'a Node, path: &Path) -> Option<&'a Node> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .iter()
        .find(|child| path.starts_with(&child.path))
        .and_then(|child| node_at(child, path))
}

pub fn allocated_bytes(metadata: &fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.blocks().saturating_mul(512)
    }
    #[cfg(not(unix))]
    {
        metadata.len()
    }
}

impl Entry {
    pub fn kind(&self) -> String {
        if self.is_symlink {
            return "Alias".into();
        }
        if self.is_dir {
            return "Folder".into();
        }
        match self
            .path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "pdf" => "PDF document".into(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => "Image".into(),
            "mp4" | "mov" | "mkv" => "Video".into(),
            "mp3" | "wav" | "flac" => "Audio".into(),
            "zip" | "gz" | "tar" | "7z" => "Archive".into(),
            "rs" | "js" | "ts" | "tsx" | "py" | "rsx" => "Source code".into(),
            "txt" | "md" | "log" => "Text document".into(),
            "" => "File".into(),
            extension => format!("{} file", extension.to_uppercase()),
        }
    }
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

    #[test]
    fn rename_duplicate_and_delete_round_trip() {
        let root = std::env::temp_dir().join(format!("rsx-ops-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let file = root.join("notes.txt");
        fs::write(&file, "hello").unwrap();
        let renamed = rename_entry(&file, "memo.txt").unwrap();
        assert_eq!(renamed.file_name().unwrap(), "memo.txt");
        let copy = duplicate_entry(&renamed).unwrap();
        assert!(copy.file_name().unwrap().to_string_lossy().contains("copy"));
        let folder = create_folder(&root, "Album").unwrap();
        assert!(folder.is_dir());
        move_to_trash(&copy).unwrap();
        assert!(!copy.exists());
        delete_permanently(&renamed).unwrap();
        assert!(!renamed.exists());
        assert!(rename_entry(&folder, "bad/name").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
