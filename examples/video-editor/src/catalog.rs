//! Local media discovery. Sources are referenced, never copied or rewritten.
use crate::{
    media,
    state::{Asset, Kind},
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default)]
pub struct ImportReport {
    pub assets: Vec<Asset>,
    pub errors: Vec<String>,
    pub cancelled: bool,
}

pub fn kind(path: &Path) -> Option<Kind> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "mp4" | "mov" | "m4v" | "mkv" | "webm" | "avi" | "mpeg" | "mpg" | "mts" | "m2ts" => {
            Some(Kind::Video)
        }
        "wav" | "mp3" | "m4a" | "aac" | "flac" | "ogg" | "aiff" => Some(Kind::Audio),
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "gif" => Some(Kind::Image),
        _ => None,
    }
}

/// Stable ordering, canonical identities, and no directory symlink traversal.
pub fn discover(
    paths: Vec<PathBuf>,
    cancel: &AtomicBool,
) -> (Vec<(PathBuf, Option<PathBuf>)>, Vec<String>) {
    let mut pending: Vec<_> = paths
        .into_iter()
        .map(|path| {
            let folder = path
                .is_dir()
                .then(|| fs::canonicalize(&path).unwrap_or_else(|_| path.clone()));
            (path, folder)
        })
        .collect();
    let mut files = Vec::new();
    let mut seen = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let mut errors = Vec::new();
    while let Some((path, folder)) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let result = (|| -> Result<(), String> {
            let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if metadata.is_dir() {
                if !directories.insert(fs::canonicalize(&path).map_err(|e| e.to_string())?) {
                    return Ok(());
                }
                for entry in fs::read_dir(&path).map_err(|e| e.to_string())? {
                    match entry {
                        Ok(entry) => pending.push((entry.path(), folder.clone())),
                        Err(e) => errors.push(e.to_string()),
                    }
                }
            } else if path.is_file() && kind(&path).is_some() {
                let canonical = fs::canonicalize(&path).map_err(|e| e.to_string())?;
                if seen.insert(canonical.clone()) {
                    files.push((canonical, folder));
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            errors.push(format!("{}: {error}", path.display()));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    (files, errors)
}

pub fn import(
    paths: Vec<PathBuf>,
    existing: BTreeSet<PathBuf>,
    cache: Option<&Path>,
    cancel: Arc<AtomicBool>,
) -> ImportReport {
    import_with_progress(paths, existing, cache, cancel, &mut |_| {})
}
pub fn import_with_progress(
    paths: Vec<PathBuf>,
    existing: BTreeSet<PathBuf>,
    cache: Option<&Path>,
    cancel: Arc<AtomicBool>,
    progress: &mut dyn FnMut(crate::processing::Update),
) -> ImportReport {
    use crate::processing::{Stage, Update};
    let (files, errors) = discover(paths, &cancel);
    for (path, _) in &files {
        progress(Update::new(path, Stage::Queued, Some(0)));
    }
    let mut report = ImportReport {
        errors,
        ..Default::default()
    };
    for (path, folder) in files {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        let Some(kind) = kind(&path) else {
            continue;
        };
        let Ok(metadata) = fs::metadata(&path) else {
            report
                .errors
                .push(format!("{}: file is no longer available", path.display()));
            continue;
        };
        let mut asset = Asset {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            kind,
            duration: 0.,
            size: metadata.len(),
            added: "Just imported".into(),
            crop: [0.; 4],
            favorite: false,
            path: Some(path.clone()),
            import_folder: folder,
            poster: None,
            resolution: None,
            codec: None,
            frame_rate: None,
            metadata_error: None,
            prepared: None,
        };
        // Reimported references are reported as duplicates without decoding them again.
        if !existing.contains(&path)
            && let Err(error) =
                media::inspect_with_progress(&mut asset, cache, cancel.clone(), progress)
        {
            asset.metadata_error = Some(error.clone());
            report.errors.push(format!("{}: {error}", path.display()));
        }
        let mut update = Update::new(
            &path,
            if cancel.load(Ordering::Relaxed) {
                Stage::Cancelled
            } else if asset.metadata_error.is_some() {
                Stage::Failed
            } else {
                Stage::Ready
            },
            Some(100),
        );
        update.error = asset.metadata_error.clone();
        progress(update);
        report.assets.push(asset);
    }
    report.cancelled |= cancel.load(Ordering::Relaxed);
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Workspace;
    #[test]
    fn discovers_nested_media_deduplicates_and_ignores_directory_symlink_loops() {
        let workspace = Workspace::new().unwrap();
        fs::create_dir(workspace.path.join("nested")).unwrap();
        let video = workspace.path.join("nested/clip.MP4");
        fs::write(&video, b"test").unwrap();
        fs::write(workspace.path.join("notes.txt"), b"test").unwrap();
        fs::write(workspace.path.join("photo.png"), b"test").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&workspace.path, workspace.path.join("nested/loop")).unwrap();
        let (files, errors) =
            discover(vec![video, workspace.path.clone()], &AtomicBool::new(false));
        assert!(errors.is_empty());
        assert_eq!(files.len(), 2);
        let root = fs::canonicalize(&workspace.path).unwrap();
        assert!(
            files
                .iter()
                .all(|(_, folder)| folder.as_ref() == Some(&root))
        );
        assert_eq!(kind(&files[0].0), Some(Kind::Video));
        assert!(
            discover(vec![workspace.path.clone()], &AtomicBool::new(true))
                .0
                .is_empty()
        );
    }
    #[test]
    fn missing_folder_is_reported() {
        let workspace = Workspace::new().unwrap();
        let (_, errors) = discover(
            vec![workspace.path.join("missing")],
            &AtomicBool::new(false),
        );
        assert_eq!(errors.len(), 1);
    }
    #[test]
    fn imports_real_video_metadata_and_poster_then_skips_existing_references() {
        let workspace = Workspace::new().unwrap();
        let cache = Workspace::new().unwrap();
        let video = workspace.path.join("clip.webm");
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/media/murchison-falls.webm"),
            &video,
        )
        .unwrap();
        let report = import(
            vec![workspace.path.clone()],
            BTreeSet::new(),
            Some(&cache.path),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.assets.len(), 1);
        let asset = &report.assets[0];
        assert!(asset.duration > 1.);
        assert!(asset.size > 0);
        assert!(asset.resolution.is_some());
        assert!(asset.poster.as_ref().unwrap().is_file());
        let existing = BTreeSet::from([fs::canonicalize(&video).unwrap()]);
        let repeated = import(
            vec![video],
            existing,
            None,
            Arc::new(AtomicBool::new(false)),
        );
        assert_eq!(repeated.assets.len(), 1);
        assert!(repeated.assets[0].poster.is_none());
        let cancelled = import(
            vec![workspace.path.clone()],
            BTreeSet::new(),
            None,
            Arc::new(AtomicBool::new(true)),
        );
        assert!(cancelled.cancelled);
        assert!(cancelled.assets.is_empty());
    }
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::{Workspace, processing::Stage};
    #[test]
    fn reports_actual_pipeline_steps_and_terminal_failures() {
        let cache = Workspace::new().unwrap();
        let source = Workspace::new().unwrap();
        let path = source.path.join("source.webm");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/media/murchison-falls.webm"),
            &path,
        )
        .unwrap();
        let broken = source.path.join("broken.webm");
        fs::write(&broken, b"invalid media").unwrap();
        let mut updates = Vec::new();
        let report = import_with_progress(
            vec![path.clone(), broken],
            Default::default(),
            Some(&cache.path),
            Arc::new(AtomicBool::new(false)),
            &mut |update| updates.push(update),
        );
        assert_eq!(report.errors.len(), 1);
        let source = fs::canonicalize(path).unwrap();
        let steps: Vec<_> = updates
            .iter()
            .filter(|update| update.path == source)
            .map(|update| update.stage)
            .collect();
        for stage in [
            Stage::Queued,
            Stage::Inspecting,
            Stage::Decoding,
            Stage::Finalizing,
            Stage::Thumbnail,
            Stage::Ready,
        ] {
            assert!(steps.contains(&stage), "{steps:?}");
        }
        assert_eq!(steps.last(), Some(&Stage::Ready));
        assert!(
            updates
                .iter()
                .any(|update| update.stage == Stage::Failed && update.error.is_some())
        );
        let percentages: Vec<_> = updates
            .iter()
            .filter(|update| update.path == source && update.stage == Stage::Decoding)
            .filter_map(|update| update.progress)
            .collect();
        assert!(percentages.len() > 1);
        assert!(percentages.windows(2).all(|pair| pair[0] <= pair[1]));
    }
}
