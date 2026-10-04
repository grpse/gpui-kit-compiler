use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

mod preferences;
pub use preferences::{Appearance, Preferences};

#[derive(Clone, Debug)]
pub struct Node {
    pub path: PathBuf,
    pub directory: bool,
    pub logical: u64,
    pub allocated: u64,
    pub files: u64,
    pub children: Vec<Node>,
}
impl Node {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_string_lossy()
            .into_owned()
    }
    pub fn at(&self, indices: &[usize]) -> &Node {
        indices
            .iter()
            .fold(self, |node, &index| &node.children[index])
    }
    pub fn weight(&self, allocated: bool) -> u64 {
        if allocated {
            self.allocated
        } else {
            self.logical
        }
    }

    /// Inspector metadata without copying a directory's entire subtree.
    pub fn summary(&self) -> Self {
        Self {
            path: self.path.clone(),
            directory: self.directory,
            logical: self.logical,
            allocated: self.allocated,
            files: self.files,
            children: vec![],
        }
    }
}
#[derive(Clone, Debug)]
pub struct Scan {
    pub root: Node,
    pub warnings: Vec<String>,
    pub skipped: u64,
}
#[derive(Clone, Debug, Default)]
pub struct ScanProgress {
    pub visited: u64,
    pub files: u64,
    pub logical: u64,
    pub allocated: u64,
    pub skipped: u64,
    pub current: PathBuf,
}

struct Scanner<'a> {
    cancelled: Arc<AtomicBool>,
    seen: HashSet<(u64, u64)>,
    device: u64,
    warnings: Vec<String>,
    skipped: u64,
    progress: ScanProgress,
    last_report: Instant,
    report: &'a mut dyn FnMut(ScanProgress),
}
impl Scanner<'_> {
    fn warning(&mut self, text: String) {
        self.skipped += 1;
        if self.warnings.len() < 30 {
            self.warnings.push(text);
        }
    }
    fn visit(&mut self, path: &Path, depth: usize) -> Result<Option<Node>, String> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err("Scan cancelled".into());
        }
        self.progress.visited += 1;
        self.progress.current = path.to_owned();
        if self.last_report.elapsed() >= Duration::from_millis(100) {
            self.progress.skipped = self.skipped;
            (self.report)(self.progress.clone());
            self.last_report = Instant::now();
        }
        let metadata = match fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) => {
                self.warning(format!("{}: {e}", path.display()));
                return Ok(None);
            }
        };
        if metadata.file_type().is_symlink() {
            self.warning(format!("Symbolic link skipped: {}", path.display()));
            return Ok(None);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != self.device {
                self.warning(format!("Other filesystem skipped: {}", path.display()));
                return Ok(None);
            }
        }
        if depth > 256 {
            self.warning(format!("Depth limit: {}", path.display()));
            return Ok(None);
        }
        let mut node = Node {
            path: path.to_owned(),
            directory: metadata.is_dir(),
            logical: 0,
            allocated: 0,
            files: 0,
            children: vec![],
        };
        if metadata.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if !self.seen.insert((metadata.dev(), metadata.ino())) {
                    return Ok(Some(node));
                }
                node.allocated = metadata.blocks().saturating_mul(512);
            }
            #[cfg(not(unix))]
            {
                node.allocated = metadata.len();
            }
            node.logical = metadata.len();
            node.files = 1;
            self.progress.files += 1;
            self.progress.logical = self.progress.logical.saturating_add(node.logical);
            self.progress.allocated = self.progress.allocated.saturating_add(node.allocated);
        } else if metadata.is_dir() {
            let entries = match fs::read_dir(path) {
                Ok(entries) => entries,
                Err(e) => {
                    self.warning(format!("{}: {e}", path.display()));
                    return Ok(Some(node));
                }
            };
            let mut paths = Vec::new();
            for entry in entries {
                if self.cancelled.load(Ordering::Relaxed) {
                    return Err("Scan cancelled".into());
                }
                match entry {
                    Ok(entry) => paths.push(entry.path()),
                    Err(e) => self.warning(format!("{}: {e}", path.display())),
                }
            }
            paths.sort();
            for path in paths {
                if let Some(child) = self.visit(&path, depth + 1)? {
                    node.logical = node.logical.saturating_add(child.logical);
                    node.allocated = node.allocated.saturating_add(child.allocated);
                    node.files += child.files;
                    node.children.push(child);
                }
            }
            node.children
                .sort_by(|a, b| b.logical.cmp(&a.logical).then_with(|| a.path.cmp(&b.path)));
        } else {
            self.warning(format!("Special file skipped: {}", path.display()));
            return Ok(None);
        }
        Ok(Some(node))
    }
}
pub fn scan(path: &Path, cancelled: Arc<AtomicBool>) -> Result<Scan, String> {
    scan_with_progress(path, cancelled, |_| {})
}

pub fn scan_with_progress(
    path: &Path,
    cancelled: Arc<AtomicBool>,
    mut report: impl FnMut(ScanProgress),
) -> Result<Scan, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_dir() {
        return Err("Choose a directory (not a symbolic link).".into());
    }
    #[cfg(unix)]
    let device = {
        use std::os::unix::fs::MetadataExt;
        metadata.dev()
    };
    #[cfg(not(unix))]
    let device = 0;
    let mut scanner = Scanner {
        cancelled,
        device,
        seen: HashSet::new(),
        warnings: vec![],
        skipped: 0,
        progress: ScanProgress::default(),
        last_report: Instant::now(),
        report: &mut report,
    };
    let root = scanner
        .visit(path, 0)?
        .ok_or("Directory could not be scanned")?;
    scanner.progress.skipped = scanner.skipped;
    (scanner.report)(scanner.progress.clone());
    Ok(Scan {
        root,
        warnings: scanner.warnings,
        skipped: scanner.skipped,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
// Balanced slice-and-dice preserves area and avoids a long row of tiny strips.
pub fn treemap(nodes: &[Node], allocated: bool) -> Vec<Rect> {
    let weights = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let weight = n.weight(allocated);
            (weight > 0).then_some((i, weight as f64))
        })
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    split(&weights, 0., 0., 1., 1., &mut output);
    output
}
fn split(weights: &[(usize, f64)], x: f32, y: f32, w: f32, h: f32, out: &mut Vec<Rect>) {
    if weights.is_empty() {
        return;
    }
    if weights.len() == 1 {
        out.push(Rect {
            index: weights[0].0,
            x,
            y,
            w,
            h,
        });
        return;
    }
    let total: f64 = weights.iter().map(|n| n.1).sum();
    let mut first = 0.;
    let mut midpoint = 1;
    for (index, item) in weights.iter().enumerate().take(weights.len() - 1) {
        first += item.1;
        midpoint = index + 1;
        if first >= total / 2. {
            break;
        }
    }
    let ratio = (first / total) as f32;
    if w >= h {
        split(&weights[..midpoint], x, y, w * ratio, h, out);
        split(
            &weights[midpoint..],
            x + w * ratio,
            y,
            w * (1. - ratio),
            h,
            out,
        );
    } else {
        split(&weights[..midpoint], x, y, w, h * ratio, out);
        split(
            &weights[midpoint..],
            x,
            y + h * ratio,
            w,
            h * (1. - ratio),
            out,
        );
    }
}
pub fn largest_files(node: &Node, allocated: bool) -> Vec<Node> {
    fn walk(node: &Node, output: &mut Vec<Node>, allocated: bool) {
        if !node.directory {
            output.push(node.clone());
        } else {
            for child in &node.children {
                walk(child, output, allocated);
            }
        }
        // Keep memory bounded while traversing a large filesystem.
        if output.len() > 400 {
            output.sort_by_key(|n| std::cmp::Reverse(n.weight(allocated)));
            output.truncate(200);
        }
    }
    let mut output = vec![];
    walk(node, &mut output, allocated);
    output.sort_by_key(|n| std::cmp::Reverse(n.weight(allocated)));
    output.truncate(200);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_aggregates_and_does_not_follow_links_or_count_hardlinks_twice() {
        let root = std::env::temp_dir().join(format!("rsx-disk-{}", std::process::id()));
        fs::create_dir_all(root.join("folder")).unwrap();
        fs::write(root.join("first"), vec![1; 20]).unwrap();
        fs::write(root.join("folder/second"), vec![1; 30]).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
            fs::hard_link(root.join("first"), root.join("hardlink")).unwrap();
        }
        let mut updates = vec![];
        let result = scan_with_progress(&root, Arc::new(AtomicBool::new(false)), |progress| {
            updates.push(progress)
        })
        .unwrap();
        assert_eq!(result.root.logical, 50);
        assert_eq!(result.root.files, 2);
        let progress = updates.last().unwrap();
        assert_eq!(progress.logical, result.root.logical);
        assert_eq!(progress.allocated, result.root.allocated);
        assert_eq!(progress.files, result.root.files);
        assert_eq!(progress.skipped, result.skipped);
        #[cfg(unix)]
        assert_eq!(result.skipped, 1);
        let rects = treemap(&result.root.children, false);
        let area: f32 = rects.iter().map(|r| r.w * r.h).sum();
        assert!((area - 1.).abs() < 1e-6);
        for rect in rects {
            let ratio = result.root.children[rect.index].logical as f32 / 50.;
            assert!((rect.w * rect.h - ratio).abs() < 1e-6);
        }
        assert_eq!(largest_files(&result.root, false)[0].logical, 30);
        assert!(
            scan(&root, Arc::new(AtomicBool::new(true)))
                .unwrap_err()
                .contains("cancelled")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
