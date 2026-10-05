//! Portable editing metadata. Media caches are regenerated from source references on open.
use crate::{
    catalog,
    composition::Composition,
    state::{Asset, Clip, EditorState, Kind, Track},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub tracks: Vec<Track>,
    pub clips: Vec<Clip>,
    pub sources: BTreeMap<u64, Vec<Clip>>,
    pub next_group: u64,
    pub selected: usize,
    pub minimum: f32,
}
impl Timeline {
    pub fn capture(state: &EditorState) -> Self {
        Self {
            tracks: state.tracks.clone(),
            clips: state.clips.clone(),
            sources: state.clip_sources.clone(),
            next_group: state.next_link_group,
            selected: state.selected_clip,
            minimum: state.project_min_duration,
        }
    }
    pub fn restore(&self, state: &mut EditorState) {
        state.tracks = self.tracks.clone();
        state.clips = self.clips.clone();
        state.clip_sources = self.sources.clone();
        state.next_link_group = self.next_group;
        state.selected_clip = self.selected.min(state.clips.len().saturating_sub(1));
        if let Some(clip) = state.clips.get(state.selected_clip) {
            state.selected = clip.asset;
        }
        state.project_min_duration = self.minimum;
        state.track_revision = state.track_revision.wrapping_add(1);
        state.position = state.position.clamp(0., state.project_duration());
        state.playing = false;
        state.controls[8] = state
            .clips
            .get(state.selected_clip)
            .and_then(|clip| state.tracks.get(clip.track))
            .map_or(100., |track| track.gain * 100.);
    }
    pub(crate) fn validate(&self, assets: &[Asset]) -> Result<(), String> {
        if !self.minimum.is_finite()
            || self.minimum < 0.
            || self.next_group == u64::MAX
            || self.tracks.len() > 10000
            || self.clips.len() > 100000
        {
            return Err("Invalid project timeline".into());
        }
        for track in &self.tracks {
            if !track.gain.is_finite() || !(0. ..=4.).contains(&track.gain) {
                return Err("Invalid track gain".into());
            }
        }
        if self.clips.iter().any(|clip| clip.start < 0.) {
            return Err("Invalid clip start".into());
        }
        // Full recovery spans may start before zero after trimming source-in forward.
        for clip in self.clips.iter().chain(self.sources.values().flatten()) {
            if clip.asset >= assets.len()
                || clip.track >= self.tracks.len()
                || ![clip.start, clip.source_start, clip.length]
                    .iter()
                    .all(|v| v.is_finite())
                || clip.source_start < 0.
                || clip.length <= 0.
            {
                return Err("Invalid clip metadata".into());
            }
            let asset = &assets[clip.asset];
            match clip.component {
                Some(crate::state::ClipComponent::Video(stream))
                    if !asset.prepared.as_ref().is_some_and(|media| {
                        media.videos.iter().any(|video| video.index == stream)
                    }) =>
                {
                    return Err("Missing video stream".into());
                }
                Some(crate::state::ClipComponent::AudioChannel { stream, channel })
                    if !asset.prepared.as_ref().is_some_and(|media| {
                        media
                            .audio
                            .iter()
                            .any(|audio| audio.index == stream && channel < audio.channels.len())
                    }) =>
                {
                    return Err("Missing audio channel".into());
                }
                None if asset.kind != Kind::Image => return Err("Unprepared clip source".into()),
                _ => {}
            }
            if asset.kind != Kind::Image && clip.source_start + clip.length > asset.duration + 0.1 {
                return Err("A saved clip extends beyond its source".into());
            }
        }
        if self
            .sources
            .keys()
            .chain(
                self.clips
                    .iter()
                    .filter_map(|clip| clip.link_group.as_ref()),
            )
            .any(|group| *group >= self.next_group)
        {
            return Err("Invalid linked clip IDs".into());
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct Source {
    path: PathBuf,
    favorite: bool,
}
#[derive(Serialize, Deserialize)]
pub struct Document {
    version: u32,
    sources: Vec<Source>,
    timeline: Timeline,
    composition: Composition,
}
#[derive(Default)]
struct OwnedSources {
    directory: Option<PathBuf>,
    keep: bool,
}
impl Drop for OwnedSources {
    fn drop(&mut self) {
        if !self.keep
            && let Some(path) = &self.directory
        {
            let _ = fs::remove_dir_all(path);
        }
    }
}
pub fn save(
    state: &EditorState,
    composition: &Composition,
    destination: &Path,
) -> Result<(), String> {
    static NEXT_MEDIA: AtomicU64 = AtomicU64::new(0);
    let mut owned = OwnedSources::default();
    let mut sources = Vec::new();
    for (index, asset) in state.assets.iter().enumerate() {
        let mut path = asset
            .path
            .clone()
            .ok_or("Project contains a source without a file")?;
        if path.starts_with(
            fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir()),
        ) && path
            .components()
            .any(|part| part.as_os_str().to_string_lossy().starts_with("rsx-video-"))
        {
            if owned.directory.is_none() {
                let directory = destination.with_file_name(format!(
                    "{}.media-{}-{}",
                    destination
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    std::process::id(),
                    NEXT_MEDIA.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir(&directory).map_err(|e| e.to_string())?;
                owned.directory = Some(directory);
            }
            let directory = owned.directory.as_ref().unwrap();
            let copy = directory.join(format!(
                "source-{index}.{}",
                path.extension().unwrap_or_default().to_string_lossy()
            ));
            let mut input = fs::File::open(&path).map_err(|e| e.to_string())?;
            let mut output = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&copy)
                .map_err(|e| e.to_string())?;
            std::io::copy(&mut input, &mut output)
                .and_then(|_| output.sync_all())
                .map_err(|e| e.to_string())?;
            path = fs::canonicalize(copy).map_err(|e| e.to_string())?;
        }
        sources.push(Source {
            path,
            favorite: asset.favorite,
        });
    }
    let document = Document {
        version: 1,
        sources,
        timeline: Timeline::capture(state),
        composition: composition.clone(),
    };
    document.timeline.validate(&state.assets)?;
    let bytes = serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temp = destination.with_file_name(format!(
        ".flowcut-project-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, destination).map_err(|e| e.to_string())
    })();
    let _ = fs::remove_file(temp);
    owned.keep = result.is_ok();
    result
}
pub fn load(
    path: &Path,
    cache: &Path,
    cancel: Arc<AtomicBool>,
    progress: &mut dyn FnMut(crate::processing::Update),
) -> Result<(EditorState, Composition), String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 {
        return Err("Project file exceeds 8 MiB".into());
    }
    let mut document: Document =
        serde_json::from_reader(file).map_err(|e| format!("Invalid project: {e}"))?;
    if document.version != 1 {
        return Err("Unsupported project version".into());
    }
    let mut paths = Vec::new();
    for source in &document.sources {
        paths.push(
            fs::canonicalize(&source.path)
                .map_err(|e| format!("Missing source {}: {e}", source.path.display()))?,
        );
    }
    let report = catalog::import_with_progress(
        paths.clone(),
        BTreeSet::new(),
        Some(cache),
        cancel,
        progress,
    );
    if report.cancelled {
        return Err("Project open cancelled".into());
    }
    if let Some(error) = report.errors.first() {
        return Err(format!("Cannot open project: {error}"));
    }
    let mut state = EditorState::default();
    for (source, path) in document.sources.iter().zip(paths) {
        let mut asset = report
            .assets
            .iter()
            .find(|asset| asset.path.as_ref() == Some(&path))
            .cloned()
            .ok_or_else(|| format!("Unsupported source: {}", path.display()))?;
        asset.favorite = source.favorite;
        state.assets.push(asset);
    }
    document.timeline.validate(&state.assets)?;
    document.composition.validate_project(state.assets.len())?;
    document.timeline.restore(&mut state);
    Ok((state, document.composition))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Workspace, state::Action};
    #[test]
    fn roundtrip_preserves_edits_linked_sources_and_composition_and_survives_source_cache_cleanup()
    {
        let source = Workspace::new().unwrap();
        let cache = Workspace::new().unwrap();
        let output = Workspace::new().unwrap();
        let media = source.path.join("clip.webm");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/media/murchison-falls.webm"),
            &media,
        )
        .unwrap();
        let report = catalog::import(
            vec![media],
            BTreeSet::new(),
            Some(&cache.path),
            Arc::new(AtomicBool::new(false)),
        );
        let mut state = EditorState::default();
        state.import_into_project(report.assets);
        state.assets[0].favorite = true;
        state.apply(Action::RenameTrack(0, "Main video".into()));
        state
            .edit_clip_timing(state.selected_clip, 0.2, 0.5, 0.5)
            .unwrap();
        let original = Timeline::capture(&state);
        state.apply(Action::DuplicateClip(state.selected_clip));
        let edited = Timeline::capture(&state);
        original.restore(&mut state);
        assert_eq!(Timeline::capture(&state), original);
        edited.restore(&mut state);
        assert_eq!(Timeline::capture(&state), edited);
        let mut composition = Composition::default();
        composition.seed(&state.assets);
        let path = output.path.join("project.flowcut");
        save(&state, &composition, &path).unwrap();
        drop(source);
        drop(state);
        drop(cache);
        let cache = Workspace::new().unwrap();
        let (state, loaded) = load(
            &path,
            &cache.path,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(Timeline::capture(&state), edited);
        assert!(state.assets[0].favorite);
        assert_eq!(loaded.nodes.len(), composition.nodes.len());
        assert!(state.assets[0].path.as_ref().unwrap().is_file());
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        json["timeline"]["clips"][0]["asset"] = serde_json::json!(999);
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(
            load(
                &path,
                &cache.path,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {}
            )
            .unwrap_err()
            .contains("Invalid clip")
        );
    }
    #[test]
    fn rejects_wrong_version_and_missing_sources() {
        let cache = Workspace::new().unwrap();
        let output = Workspace::new().unwrap();
        let path = output.path.join("invalid.flowcut");
        let mut json = serde_json::json!({"version":999,"sources":[],"timeline":Timeline::capture(&EditorState::default()),"composition":Composition::default()});
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(
            load(
                &path,
                &cache.path,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {}
            )
            .unwrap_err()
            .contains("version")
        );
        json["version"] = serde_json::json!(1);
        json["sources"] = serde_json::json!([{"path":"/missing-source.webm","favorite":false}]);
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(
            load(
                &path,
                &cache.path,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {}
            )
            .unwrap_err()
            .contains("Missing source")
        );
    }
}
