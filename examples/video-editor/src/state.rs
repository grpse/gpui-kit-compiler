//! Real project media and non-destructive timeline state.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Edit,
    Library,
    Overview,
    Compositing,
}
impl Screen {
    pub const ALL: [Self; 4] = [Self::Library, Self::Overview, Self::Edit, Self::Compositing];
    pub fn label(self) -> &'static str {
        match self {
            Self::Edit => "Edit",
            Self::Library => "Media",
            Self::Overview => "Overview",
            Self::Compositing => "Compositing",
        }
    }
    pub fn has_timeline(self) -> bool {
        matches!(self, Self::Edit)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    Video,
    Audio,
    Image,
}
#[derive(Clone, Debug)]
pub struct Asset {
    pub name: String,
    pub kind: Kind,
    pub duration: f32,
    pub size: u64,
    pub added: String,
    pub crop: [f32; 4],
    pub favorite: bool,
    pub path: Option<PathBuf>,
    pub import_folder: Option<PathBuf>,
    pub poster: Option<PathBuf>,
    pub resolution: Option<[u32; 2]>,
    pub codec: Option<String>,
    pub frame_rate: Option<String>,
    pub metadata_error: Option<String>,
    pub prepared: Option<Arc<crate::preprocess::PreparedMedia>>,
}
impl Asset {
    pub fn duration_label(&self) -> String {
        if self.kind == Kind::Image {
            return "Still image".into();
        }
        if self.duration <= 0. {
            return "Unknown duration".into();
        }
        if self.duration < 1. {
            return format!("{:.3} s", self.duration);
        }
        let s = self.duration as u32;
        format!("{:02}:{:02}", s / 60, s % 60)
    }
    pub fn size_label(&self) -> String {
        if self.size >= 1_000_000_000 {
            format!("{:.1} GB", self.size as f64 / 1_000_000_000.)
        } else if self.size >= 1_000_000 {
            format!("{:.1} MB", self.size as f64 / 1_000_000.)
        } else if self.size >= 1_000 {
            format!("{:.1} KB", self.size as f64 / 1_000.)
        } else {
            format!("{} B", self.size)
        }
    }
    pub fn resolution_label(&self) -> String {
        self.resolution
            .map(|[w, h]| format!("{w} × {h}"))
            .unwrap_or_else(|| "Unknown".into())
    }
    pub fn location(&self) -> String {
        self.path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Source unavailable".into())
    }
}
/// Historical editing fixtures are compiled only into unit tests.
#[cfg(test)]
fn mock_project() -> serde_json::Value {
    serde_json::from_str(include_str!("../assets/mocks/project.json"))
        .expect("valid bundled mock project")
}
#[cfg(test)]
pub fn assets() -> Vec<Asset> {
    mock_project()["assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            let kind = match value["kind"].as_str().unwrap() {
                "Audio" => Kind::Audio,
                "Image" => Kind::Image,
                _ => Kind::Video,
            };
            Asset {
                name: value["name"].as_str().unwrap().into(),
                kind,
                duration: value["duration"].as_f64().unwrap() as f32,
                size: value["size"].as_u64().unwrap(),
                added: value["added"].as_str().unwrap().into(),
                crop: std::array::from_fn(|i| value["crop"][i].as_f64().unwrap() as f32),
                favorite: false,
                path: None,
                import_folder: None,
                poster: None,
                resolution: value["resolution"]
                    .as_array()
                    .map(|v| [v[0].as_u64().unwrap() as u32, v[1].as_u64().unwrap() as u32]),
                codec: value["codec"].as_str().map(str::to_owned),
                frame_rate: value["frame_rate"].as_str().map(str::to_owned),
                metadata_error: None,
                prepared: None,
            }
        })
        .collect()
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub name: String,
    pub audio: bool,
    pub visible: bool,
    pub locked: bool,
    pub muted: bool,
    pub gain: f32,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Clip {
    pub name: Option<String>,
    pub asset: usize,
    pub track: usize,
    pub start: f32,
    pub length: f32,
    pub source_start: f32,
    pub component: Option<ClipComponent>,
    pub link_group: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ClipComponent {
    Video(usize),
    AudioChannel { stream: usize, channel: usize },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipDragKind {
    Move,
    Start,
    End,
}
#[derive(Clone, Copy, Debug)]
pub struct ClipTiming {
    pub start: f32,
    pub source_start: f32,
    pub length: f32,
}
/// Only editing metadata is copied during a pointer gesture; decoded media is shared.
#[derive(Clone, Debug)]
pub struct TimelineSnapshot {
    clips: Vec<Clip>,
    sources: BTreeMap<u64, Vec<Clip>>,
    selected_clip: usize,
    position: f32,
}
/// Project-local clipboard. Track slots survive reordering and recreate deleted tracks on paste.
#[derive(Clone, Debug)]
struct ClipClipboard {
    clips: Vec<Clip>,
    sources: Vec<Clip>,
    tracks: Vec<(Option<usize>, Track)>,
    selected: usize,
    origin: f32,
}
#[derive(Clone, Debug)]
pub enum Action {
    Screen(Screen),
    ResetWorkspace,
    Select(usize),
    MultiSelect(usize),
    Category(String),
    Folder(String),
    Search(String),
    TypeFilter,
    ResolutionFilter,
    DurationFilter,
    SourceFilter,
    ClearFilters,
    Sort,
    Layout(bool),
    FilterPanel,
    Favorite,
    Import,
    ImportFolder,
    AddToTimeline,
    ShowInFinder,
    CancelAll,
    Play,
    Seek(f32),
    Step(f32),
    Mute,
    Loop,
    Fullscreen,
    Inspector(usize),
    DetailTab(usize),
    Control(usize, f32),
    Tool(&'static str),
    SelectClip(usize),
    TrackVisible(usize),
    TrackLocked(usize),
    TrackMuted(usize),
    RenameTrack(usize, String),
    RenameClip(usize, String),
    TargetClip(usize),
    TargetAsset(usize),
    FavoriteAsset(usize),
    ShowAssetInFinder(usize),
    MoveTrack {
        from: usize,
        to: usize,
    },
    DuplicateClip(usize),
    CopyClips,
    CutClips,
    PasteClips,
    DeleteTrack(usize),
    EditClipTiming {
        index: usize,
        start: f32,
        source_start: f32,
        length: f32,
    },
    AddTrack,
    Zoom(f32),
    Export,
    SaveProject,
    OpenProject,
    CancelExport,
    Undo,
    Redo,
    Dismiss,
}
pub fn timeline_shortcut(key: &str, command: bool, alt: bool, shift: bool) -> Option<Action> {
    if command && !alt && key == "z" {
        return Some(if shift { Action::Redo } else { Action::Undo });
    }
    if alt || shift {
        return None;
    }
    match (command, key) {
        (true, "c") => Some(Action::CopyClips),
        (true, "x") => Some(Action::CutClips),
        (true, "v") => Some(Action::PasteClips),
        (false, "delete" | "backspace") => Some(Action::Tool("Delete")),
        _ => None,
    }
}
#[derive(Clone, Debug)]
pub struct EditorState {
    pub screen: Screen,
    pub assets: Vec<Asset>,
    pub selected: usize,
    pub checked: BTreeSet<usize>,
    pub category: String,
    pub folder: String,
    pub query: String,
    pub type_filter: usize,
    pub resolution_filter: bool,
    pub duration_filter: bool,
    pub source_filter: bool,
    pub sort_ascending: bool,
    pub list: bool,
    pub filters_open: bool,
    pub playing: bool,
    pub position: f32,
    pub muted: bool,
    pub monitor_gain: f32,
    pub looping: bool,
    pub fullscreen: bool,
    pub inspector: usize,
    pub detail_tab: usize,
    pub controls: [f32; 10],
    pub tracks: Vec<Track>,
    pub track_revision: u64,
    pub clips: Vec<Clip>,
    /// Full imported source spans survive trims, so linked channels can be restored.
    pub(crate) clip_sources: BTreeMap<u64, Vec<Clip>>,
    pub selected_clip: usize,
    clipboard: Option<ClipClipboard>,
    pub zoom: f32,
    pub(crate) next_link_group: u64,
    pub project_min_duration: f32,
    pub notice: Option<String>,
    pub events: Vec<String>,
}
impl Default for EditorState {
    fn default() -> Self {
        Self {
            screen: Screen::Edit,
            assets: vec![],
            selected: 0,
            checked: BTreeSet::new(),
            category: "All".into(),
            folder: String::new(),
            query: String::new(),
            type_filter: 0,
            resolution_filter: false,
            duration_filter: false,
            source_filter: false,
            sort_ascending: false,
            list: false,
            filters_open: false,
            playing: false,
            position: 0.,
            muted: false,
            monitor_gain: 1.,
            looping: false,
            fullscreen: false,
            inspector: 0,
            detail_tab: 0,
            controls: [100., 0., 100., 0., 0., 0., 0., 50., 100., 0.],
            tracks: vec![],
            track_revision: 0,
            clips: vec![],
            clip_sources: BTreeMap::new(),
            selected_clip: 0,
            clipboard: None,
            zoom: 1.,
            next_link_group: 1,
            project_min_duration: 1.,
            notice: None,
            events: vec![],
        }
    }
}
impl EditorState {
    #[cfg(test)]
    pub fn mock() -> Self {
        let data = mock_project();
        Self {
            assets: assets(),
            checked: BTreeSet::from([0]),
            position: data["position"].as_f64().unwrap() as f32,
            project_min_duration: data["min_duration"].as_f64().unwrap() as f32,
            tracks: data["tracks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| Track {
                    name: v[0].as_str().unwrap().into(),
                    audio: v[1].as_bool().unwrap(),
                    visible: true,
                    locked: false,
                    muted: false,
                    gain: 1.,
                })
                .collect(),
            clips: data["clips"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| Clip {
                    name: None,
                    asset: v[0].as_u64().unwrap() as usize,
                    track: v[1].as_u64().unwrap() as usize,
                    start: v[2].as_f64().unwrap() as f32,
                    length: v[3].as_f64().unwrap() as f32,
                    source_start: 0.,
                    component: None,
                    link_group: None,
                })
                .collect(),
            ..Self::default()
        }
    }
    pub fn clip_label(&self, index: usize) -> String {
        let Some(clip) = self.clips.get(index) else {
            return String::new();
        };
        if let Some(name) = &clip.name {
            return name.clone();
        }
        let asset = &self.assets[clip.asset];
        if let Some(ClipComponent::AudioChannel { stream, channel }) = clip.component
            && let Some(audio) = asset
                .prepared
                .as_ref()
                .and_then(|media| media.audio.iter().find(|audio| audio.index == stream))
            && let Some(channel) = audio.channels.get(channel)
        {
            return format!("{} · S{} {}", asset.name, stream, channel.name);
        }
        asset.name.clone()
    }
    pub fn linked_clip_indices(&self, index: usize) -> Vec<usize> {
        let Some(selected) = self.clips.get(index) else {
            return vec![];
        };
        self.clips
            .iter()
            .enumerate()
            .filter_map(|(i, clip)| {
                selected
                    .link_group
                    .map_or(i == index, |group| clip.link_group == Some(group))
                    .then_some(i)
            })
            .collect()
    }
    pub fn linked_locked(&self, index: usize) -> bool {
        let visible_locked = self
            .linked_clip_indices(index)
            .iter()
            .any(|&i| self.tracks[self.clips[i].track].locked);
        visible_locked
            || self
                .clips
                .get(index)
                .and_then(|clip| clip.link_group)
                .and_then(|group| self.clip_sources.get(&group))
                .is_some_and(|sources| sources.iter().any(|clip| self.tracks[clip.track].locked))
    }
    pub fn timeline_snapshot(&self) -> TimelineSnapshot {
        TimelineSnapshot {
            clips: self.clips.clone(),
            sources: self.clip_sources.clone(),
            selected_clip: self.selected_clip,
            position: self.position,
        }
    }
    pub fn restore_timeline(&mut self, snapshot: &TimelineSnapshot) {
        self.clips.clone_from(&snapshot.clips);
        self.clip_sources.clone_from(&snapshot.sources);
        self.selected_clip = snapshot.selected_clip;
        self.position = snapshot.position;
    }
    pub fn clip_source_bounds(&self, index: usize) -> Option<(f32, f32)> {
        let clip = self.clips.get(index)?;
        if let Some(sources) = clip
            .link_group
            .and_then(|group| self.clip_sources.get(&group))
        {
            let epsilon = (clip.source_start + clip.length).max(1.) * f32::EPSILON * 8.;
            let source = sources.iter().find(|source| {
                source.track == clip.track
                    && source.component == clip.component
                    && source.source_start <= clip.source_start + epsilon
                    && source.source_start + source.length + epsilon
                        >= clip.source_start + clip.length
            })?;
            Some((source.source_start, source.source_start + source.length))
        } else {
            let duration = self.assets.get(clip.asset)?.duration;
            Some((0., duration.max(clip.source_start + clip.length)))
        }
    }
    pub fn drag_clip_timing(
        &self,
        index: usize,
        kind: ClipDragKind,
        delta: f32,
    ) -> Result<ClipTiming, String> {
        let clip = self
            .clips
            .get(index)
            .ok_or("Clip is no longer available.")?;
        if !delta.is_finite() || self.linked_locked(index) {
            return Err("Unlock the linked tracks before dragging.".into());
        }
        let (first, end) = self
            .clip_source_bounds(index)
            .ok_or("Source range is unavailable.")?;
        let minimum = 0.001_f32.min(clip.length);
        let timing = match kind {
            ClipDragKind::Move => {
                let earliest = self
                    .linked_clip_indices(index)
                    .iter()
                    .map(|&i| self.clips[i].start)
                    .fold(clip.start, f32::min);
                ClipTiming {
                    start: clip.start + delta.max(-earliest),
                    source_start: clip.source_start,
                    length: clip.length,
                }
            }
            ClipDragKind::Start => {
                let delta = delta.clamp(
                    (first - clip.source_start).max(-clip.start),
                    clip.length - minimum,
                );
                ClipTiming {
                    start: clip.start + delta,
                    source_start: clip.source_start + delta,
                    length: clip.length - delta,
                }
            }
            ClipDragKind::End => {
                let delta = delta.clamp(
                    minimum - clip.length,
                    (end - clip.source_start - clip.length).max(0.),
                );
                ClipTiming {
                    start: clip.start,
                    source_start: clip.source_start,
                    length: clip.length + delta,
                }
            }
        };
        self.validate_clip_timing(index, timing.start, timing.source_start, timing.length)?;
        Ok(timing)
    }
    pub fn valid_name(name: String) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().any(char::is_control) || name.chars().count() > 120 {
            return Err("Use a name of 1–120 characters without control characters.".into());
        }
        Ok(name.to_owned())
    }
    fn copy_clips(&mut self) -> Result<(), String> {
        let index = self.selected_clip;
        let selected = self.clips.get(index).ok_or("Select a clip to copy.")?;
        let linked = self.linked_clip_indices(index);
        let mut clips: Vec<_> = linked.iter().map(|&i| self.clips[i].clone()).collect();
        let mut sources = selected
            .link_group
            .and_then(|group| self.clip_sources.get(&group))
            .cloned()
            .unwrap_or_else(|| clips.clone());
        let track_ids: BTreeSet<_> = clips.iter().chain(&sources).map(|c| c.track).collect();
        let tracks: Vec<_> = track_ids
            .iter()
            .map(|&i| (Some(i), self.tracks[i].clone()))
            .collect();
        let origin = clips.iter().map(|c| c.start).fold(f32::INFINITY, f32::min);
        for clip in clips.iter_mut().chain(&mut sources) {
            clip.track = track_ids.iter().position(|&i| i == clip.track).unwrap();
        }
        self.clipboard = Some(ClipClipboard {
            clips,
            sources,
            tracks,
            selected: linked.iter().position(|&i| i == index).unwrap(),
            origin,
        });
        Ok(())
    }
    pub fn can_paste(&self) -> bool {
        self.clipboard.is_some()
    }
    fn paste_clips(&mut self) -> Result<(), String> {
        let copied = self
            .clipboard
            .as_ref()
            .ok_or("Copy or cut a clip first.")?
            .clone();
        let shift = self.position - copied.origin;
        if copied.tracks.iter().any(|(index, _)| {
            index
                .and_then(|i| self.tracks.get(i))
                .is_some_and(|t| t.locked)
        }) {
            return Err("Unlock the destination tracks before pasting.".into());
        }
        if !self.position.is_finite()
            || self.position < 0.
            || copied
                .clips
                .iter()
                .any(|c| !(c.start + shift + c.length).is_finite())
        {
            return Err("Paste would exceed the timeline range.".into());
        }
        let track_ids: Vec<_> = copied
            .tracks
            .iter()
            .map(|(index, template)| {
                if let Some(index) = index.filter(|&i| i < self.tracks.len()) {
                    index
                } else {
                    let index = self.tracks.len();
                    let mut track = template.clone();
                    track.locked = false;
                    self.tracks.push(track);
                    self.track_revision = self.track_revision.wrapping_add(1);
                    index
                }
            })
            .collect();
        let group = copied.clips.first().and_then(|c| c.link_group).map(|_| {
            let group = self.next_link_group;
            self.next_link_group += 1;
            group
        });
        let first = self.clips.len();
        for mut clip in copied.clips {
            clip.start += shift;
            clip.track = track_ids[clip.track];
            clip.link_group = group;
            self.clips.push(clip);
        }
        if let Some(group) = group {
            let sources = copied
                .sources
                .into_iter()
                .map(|mut c| {
                    c.start += shift;
                    c.track = track_ids[c.track];
                    c.link_group = Some(group);
                    c
                })
                .collect();
            self.clip_sources.insert(group, sources);
        }
        // Remember recreated destinations so repeated pastes reuse them.
        if let Some(clipboard) = &mut self.clipboard {
            for ((index, _), &track) in clipboard.tracks.iter_mut().zip(&track_ids) {
                *index = Some(track);
            }
        }
        self.selected_clip = first + copied.selected;
        self.selected = self.clips[self.selected_clip].asset;
        self.playing = false;
        Ok(())
    }
    fn delete_track(&mut self, index: usize) -> Result<(), String> {
        let track = self
            .tracks
            .get(index)
            .ok_or("Track is no longer available.")?;
        if track.locked {
            return Err("Unlock the track before deleting it.".into());
        }
        let selected = self.clips.get(self.selected_clip).cloned();
        let selected_index = self.selected_clip;
        let removed_before = self
            .clips
            .iter()
            .take(selected_index)
            .filter(|c| c.track == index)
            .count();
        self.tracks.remove(index);
        self.clips.retain(|c| c.track != index);
        for clip in &mut self.clips {
            if clip.track > index {
                clip.track -= 1;
            }
        }
        self.clip_sources.retain(|_, sources| {
            sources.retain(|c| c.track != index);
            for clip in sources.iter_mut() {
                if clip.track > index {
                    clip.track -= 1;
                }
            }
            !sources.is_empty()
        });
        if let Some(clipboard) = &mut self.clipboard {
            for (track, _) in &mut clipboard.tracks {
                if let Some(i) = track {
                    if *i == index {
                        *track = None;
                    } else if *i > index {
                        *i -= 1;
                    }
                }
            }
        }
        self.selected_clip = if selected.is_some_and(|c| c.track != index) {
            selected_index - removed_before
        } else {
            selected_index.min(self.clips.len().saturating_sub(1))
        };
        if let Some(clip) = self.clips.get(self.selected_clip) {
            self.selected = clip.asset;
        }
        self.position = self.position.min(self.seek_limit());
        self.playing = false;
        self.track_revision = self.track_revision.wrapping_add(1);
        Ok(())
    }
    pub fn move_track(&mut self, from: usize, to: usize) -> Result<(), String> {
        if from >= self.tracks.len() || to >= self.tracks.len() {
            return Err("Track is no longer available.".into());
        }
        if from == to {
            return Ok(());
        }
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);
        for clip in &mut self.clips {
            if clip.track == from {
                clip.track = to;
            } else if from < to && clip.track > from && clip.track <= to {
                clip.track -= 1;
            } else if from > to && clip.track >= to && clip.track < from {
                clip.track += 1;
            }
        }
        for sources in self.clip_sources.values_mut() {
            for clip in sources {
                if clip.track == from {
                    clip.track = to;
                } else if from < to && clip.track > from && clip.track <= to {
                    clip.track -= 1;
                } else if from > to && clip.track >= to && clip.track < from {
                    clip.track += 1;
                }
            }
        }
        if let Some(clipboard) = &mut self.clipboard {
            for (index, _) in &mut clipboard.tracks {
                if let Some(i) = index {
                    if *i == from {
                        *i = to;
                    } else if from < to && *i > from && *i <= to {
                        *i -= 1;
                    } else if from > to && *i >= to && *i < from {
                        *i += 1;
                    }
                }
            }
        }
        self.track_revision = self.track_revision.wrapping_add(1);
        Ok(())
    }
    pub fn validate_clip_timing(
        &self,
        index: usize,
        start: f32,
        source_start: f32,
        length: f32,
    ) -> Result<(), String> {
        let selected = self
            .clips
            .get(index)
            .ok_or("Clip is no longer available.")?;
        if !start.is_finite()
            || !source_start.is_finite()
            || !length.is_finite()
            || start < 0.
            || source_start < 0.
            || length <= 0.
            || !(start + length).is_finite()
        {
            return Err("Start and source-in must be nonnegative; duration must be positive. Use finite seconds.".into());
        }
        if self.linked_locked(index) {
            return Err("Unlock the linked tracks before editing timing.".into());
        }
        let epsilon = (selected.source_start + selected.length).abs().max(1.) * f32::EPSILON * 4.;
        let (first, end) = self
            .clip_source_bounds(index)
            .ok_or("Source range is unavailable.")?;
        if source_start < first - epsilon || source_start + length > end + epsilon {
            return Err("Keep source-in and duration within the available source media.".into());
        }
        if (source_start - selected.source_start).abs() <= epsilon
            && (length - selected.length).abs() <= epsilon
        {
            let delta = start - selected.start;
            if self
                .linked_clip_indices(index)
                .iter()
                .any(|&i| self.clips[i].start + delta < -epsilon)
            {
                return Err("Linked clips cannot move before the project start.".into());
            }
        }
        Ok(())
    }
    pub fn edit_clip_timing(
        &mut self,
        index: usize,
        start: f32,
        source_start: f32,
        length: f32,
    ) -> Result<(), String> {
        self.validate_clip_timing(index, start, source_start, length)?;
        let selected = self.clips[index].clone();
        let epsilon = (selected.source_start + selected.length).abs().max(1.) * f32::EPSILON * 4.;
        let linked = self.linked_clip_indices(index);
        let move_only = (source_start - selected.source_start).abs() <= epsilon
            && (length - selected.length).abs() <= epsilon;
        if move_only {
            let delta = start - selected.start;
            for &i in &linked {
                self.clips[i].start = (self.clips[i].start + delta).max(0.);
            }
            if let Some(sources) = selected
                .link_group
                .and_then(|group| self.clip_sources.get_mut(&group))
            {
                for clip in sources {
                    clip.start += delta;
                }
            }
        } else {
            let trim_start = selected.start + source_start - selected.source_start;
            let trim_end = trim_start + length;
            let sources = selected
                .link_group
                .and_then(|group| self.clip_sources.get(&group))
                .cloned()
                .unwrap_or_else(|| {
                    let (first, end) = self.clip_source_bounds(index).unwrap();
                    vec![Clip {
                        start: selected.start + first - selected.source_start,
                        source_start: first,
                        length: end - first,
                        ..selected.clone()
                    }]
                });
            let (first, end) = self.clip_source_bounds(index).unwrap();
            let left_changed = (source_start - selected.source_start).abs() > epsilon;
            let right_changed =
                (source_start + length - selected.source_start - selected.length).abs() > epsilon;
            let window_start = if !left_changed {
                linked
                    .iter()
                    .map(|&i| self.clips[i].start)
                    .fold(trim_start, f32::min)
            } else if source_start <= first + epsilon {
                sources
                    .iter()
                    .map(|clip| clip.start)
                    .fold(trim_start, f32::min)
                    .max(trim_start - start)
            } else {
                trim_start
            };
            let window_end = if !right_changed {
                linked
                    .iter()
                    .map(|&i| self.clips[i].start + self.clips[i].length)
                    .fold(trim_end, f32::max)
            } else if source_start + length >= end - epsilon {
                sources
                    .iter()
                    .map(|clip| clip.start + clip.length)
                    .fold(trim_end, f32::max)
            } else {
                trim_end
            };
            let mut edited = Vec::with_capacity(self.clips.len());
            for (i, clip) in self.clips.iter().enumerate() {
                if !linked.contains(&i) {
                    edited.push(clip.clone());
                }
            }
            let mut new_selected = None;
            for clip in &sources {
                let left = clip.start.max(window_start);
                let right = (clip.start + clip.length).min(window_end);
                if right <= left {
                    continue;
                }
                let mut clip = clip.clone();
                if let Some(current) = linked.iter().map(|&i| &self.clips[i]).find(|current| {
                    current.track == clip.track
                        && current.component == clip.component
                        && current.source_start < clip.source_start + clip.length
                        && current.source_start + current.length > clip.source_start
                }) {
                    clip.name = current.name.clone();
                }
                clip.source_start += left - clip.start;
                clip.start = start + left - trim_start;
                clip.length = right - left;
                if clip.track == selected.track
                    && clip.component == selected.component
                    && source_start >= clip.source_start - epsilon
                    && source_start + length <= clip.source_start + clip.length + epsilon
                    && new_selected.is_none()
                {
                    new_selected = Some(edited.len());
                }
                edited.push(clip);
            }
            self.selected_clip = new_selected.ok_or("The requested range contains no media.")?;
            self.clips = edited;
            if let Some(group) = selected.link_group {
                let mut sources = sources;
                for clip in &mut sources {
                    clip.start += start - trim_start;
                }
                self.clip_sources.insert(group, sources);
            }
        }
        self.position = start;
        self.playing = false;
        Ok(())
    }
    fn duplicate_clip(&mut self, index: usize) -> Result<(), String> {
        let selected = self
            .clips
            .get(index)
            .ok_or("Clip is no longer available.")?;
        if self.linked_locked(index) {
            return Err("Unlock the linked tracks before duplicating.".into());
        }
        let linked = self.linked_clip_indices(index);
        let first = linked
            .iter()
            .map(|&i| self.clips[i].start)
            .fold(f32::INFINITY, f32::min);
        let end = linked
            .iter()
            .map(|&i| self.clips[i].start + self.clips[i].length)
            .fold(0., f32::max);
        let shift = end - first;
        if !linked
            .iter()
            .all(|&i| (self.clips[i].start + shift + self.clips[i].length).is_finite())
        {
            return Err("Duplicate would exceed the timeline range.".into());
        }
        let group = selected.link_group.map(|_| self.next_link_group);
        let sources = selected
            .link_group
            .and_then(|group| self.clip_sources.get(&group))
            .cloned();
        if group.is_some() {
            self.next_link_group += 1;
        }
        for i in linked {
            let mut clip = self.clips[i].clone();
            clip.start += shift;
            clip.link_group = group;
            if i == index {
                self.selected_clip = self.clips.len();
            }
            self.clips.push(clip);
        }
        if let (Some(group), Some(mut sources)) = (group, sources) {
            for clip in &mut sources {
                clip.start += shift;
                clip.link_group = Some(group);
            }
            self.clip_sources.insert(group, sources);
        }
        self.selected = self.clips[self.selected_clip].asset;
        self.position = self.clips[self.selected_clip].start;
        self.playing = false;
        Ok(())
    }
    pub fn project_duration(&self) -> f32 {
        self.clips
            .iter()
            .map(|clip| clip.start + clip.length)
            .fold(self.project_min_duration, f32::max)
    }
    /// Import references, preserving stable asset indices and existing edits.
    pub fn import_assets(&mut self, assets: Vec<Asset>) -> (usize, usize) {
        let mut added = Vec::new();
        let mut duplicates = 0;
        let mut identities: BTreeMap<_, _> = self
            .assets
            .iter()
            .enumerate()
            .filter_map(|(index, asset)| asset.path.clone().map(|path| (path, index)))
            .collect();
        for mut asset in assets {
            if let Some(index) = asset
                .path
                .as_ref()
                .and_then(|path| identities.get(path))
                .copied()
            {
                // Reinspection can repair failed metadata without breaking clip references.
                if asset.codec.is_some() || asset.metadata_error.is_some() {
                    asset.favorite = self.assets[index].favorite;
                    asset.import_folder = self.assets[index]
                        .import_folder
                        .clone()
                        .or(asset.import_folder);
                    self.assets[index] = asset;
                }
                duplicates += 1;
                continue;
            }
            if let Some(path) = &asset.path {
                identities.insert(path.clone(), self.assets.len());
            }
            added.push(self.assets.len());
            self.assets.push(asset);
        }
        if let Some(&first) = added.first() {
            self.apply(Action::ClearFilters);
            self.apply(Action::Select(first));
            self.checked = added.iter().copied().collect();
        }
        (added.len(), duplicates)
    }
    /// Only newly imported, successfully prepared sources become timeline clips.
    pub fn import_into_project(&mut self, assets: Vec<Asset>) -> (usize, usize, usize) {
        self.import_project_media(assets, true)
    }
    /// Compositing frames are library previews; only rendered audio is placed on the timeline.
    pub fn import_composition_result(&mut self, assets: Vec<Asset>) -> (usize, usize, usize) {
        self.import_project_media(assets, false)
    }
    fn import_project_media(
        &mut self,
        assets: Vec<Asset>,
        place_images: bool,
    ) -> (usize, usize, usize) {
        let position = self.position;
        let first_clip = self.clips.len();
        let (added, duplicates) = self.import_assets(assets);
        let eligible: BTreeSet<_> = if added > 0 {
            self.checked
                .iter()
                .copied()
                .filter(|&id| {
                    let asset = &self.assets[id];
                    asset.metadata_error.is_none()
                        && ((place_images && asset.kind == Kind::Image)
                            || asset.prepared.as_ref().is_some_and(|media| {
                                !media.videos.is_empty() || !media.audio.is_empty()
                            }))
                })
                .collect()
        } else {
            BTreeSet::new()
        };
        let placed = eligible.len();
        if placed > 0 {
            self.checked = eligible;
            self.position = position;
            self.apply(Action::AddToTimeline);
            if let Some(index) =
                self.clips
                    .iter()
                    .enumerate()
                    .skip(first_clip)
                    .find_map(|(i, clip)| {
                        matches!(clip.component, Some(ClipComponent::Video(_))).then_some(i)
                    })
            {
                self.apply(Action::SelectClip(index));
            }
        }
        (added, duplicates, placed)
    }
    pub fn imported_folders(&self) -> BTreeSet<PathBuf> {
        self.assets
            .iter()
            .filter_map(|asset| asset.import_folder.clone())
            .collect()
    }
    pub fn seek_limit(&self) -> f32 {
        if self.screen.has_timeline() {
            self.project_duration()
        } else {
            self.assets
                .get(self.selected)
                .map_or(1., |asset| asset.duration.max(1.))
        }
    }
    pub fn visible_assets(&self) -> Vec<usize> {
        let imported_folder = self
            .imported_folders()
            .iter()
            .any(|p| p.to_string_lossy() == self.folder);
        let mut ids: Vec<_> = self
            .assets
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                let kind = match self.category.as_str() {
                    "Videos" => Some(Kind::Video),
                    "Audio" => Some(Kind::Audio),
                    "Images" => Some(Kind::Image),
                    _ => None,
                };
                let type_kind = match self.type_filter {
                    1 => Some(Kind::Video),
                    2 => Some(Kind::Audio),
                    3 => Some(Kind::Image),
                    _ => None,
                };
                a.name.to_lowercase().contains(&self.query.to_lowercase())
                    && kind.is_none_or(|k| a.kind == k)
                    && type_kind.is_none_or(|k| a.kind == k)
                    && (self.category != "Favorites" || a.favorite)
                    && (!self.resolution_filter
                        || a.resolution.is_some_and(|[w, h]| w >= 3840 || h >= 2160))
                    && (!self.duration_filter
                        || (a.duration > 0. && a.duration < 20.)
                        || a.kind == Kind::Image)
                    && (!self.source_filter || a.name.to_lowercase().ends_with(".mp4"))
                    && match self.folder.as_str() {
                        "Music" => a.kind == Kind::Audio,
                        "SFX" => a.name.starts_with("sfx"),
                        "House" => a.name == "interior.mov",
                        "Trash" => false,
                        folder if imported_folder => a
                            .import_folder
                            .as_ref()
                            .is_some_and(|p| p.to_string_lossy() == folder),
                        _ => true,
                    }
            })
            .map(|(i, _)| i)
            .collect();
        if self.sort_ascending {
            ids.sort_by(|&a, &b| {
                self.assets[a]
                    .name
                    .to_lowercase()
                    .cmp(&self.assets[b].name.to_lowercase())
            });
        } else {
            // Newly imported media leads Date Added without rearranging the demo fixtures.
            ids.sort_by_key(|&id| {
                let imported = self.assets[id].path.is_some();
                (!imported, if imported { usize::MAX - id } else { id })
            });
        }
        ids
    }
    pub fn apply(&mut self, action: Action) {
        let event = format!("{action:?}");
        self.events.push(event.clone());
        if self.events.len() > 100 {
            self.events.remove(0);
        }
        match action {
            Action::ResetWorkspace => {}
            Action::Screen(v) => {
                self.screen = v;
                self.fullscreen = false;
                self.inspector = if self
                    .assets
                    .get(self.selected)
                    .is_some_and(|asset| asset.kind == Kind::Audio)
                    || (v.has_timeline()
                        && self.clips.get(self.selected_clip).is_some_and(|clip| {
                            clip.asset == self.selected
                                && matches!(
                                    clip.component,
                                    Some(ClipComponent::AudioChannel { .. })
                                )
                        })) {
                    1
                } else {
                    0
                };
                self.position = self.position.min(self.seek_limit());
            }
            Action::Select(i) if i < self.assets.len() => {
                self.selected = i;
                self.inspector = if self.assets[i].kind == Kind::Audio {
                    1
                } else {
                    0
                };
                self.checked = BTreeSet::from([i]);
                self.position = 0.;
                self.playing = false;
            }
            Action::MultiSelect(i) if i < self.assets.len() => {
                if !self.checked.remove(&i) {
                    self.checked.insert(i);
                }
                self.selected = i;
                self.position = self.position.min(self.seek_limit());
                self.inspector = if self.assets[i].kind == Kind::Audio {
                    1
                } else {
                    0
                };
            }
            Action::Category(v) => {
                self.category = v;
                self.folder.clear();
                self.query.clear();
                self.type_filter = 0;
                self.resolution_filter = false;
                self.duration_filter = false;
                self.source_filter = false;
                let kind = match self.category.as_str() {
                    "Audio" => Some(Kind::Audio),
                    "Images" => Some(Kind::Image),
                    "Videos" => Some(Kind::Video),
                    _ => None,
                };
                if let Some(kind) = kind
                    && let Some(index) = self.assets.iter().position(|asset| asset.kind == kind)
                {
                    self.selected = index;
                    self.checked = BTreeSet::from([index]);
                    self.position = 0.;
                    self.inspector = if kind == Kind::Audio { 1 } else { 0 };
                }
            }
            Action::Folder(v) => {
                self.folder = v;
                self.category = "All".into();
            }
            Action::Search(v) => self.query = v,
            Action::TypeFilter => self.type_filter = (self.type_filter + 1) % 4,
            Action::ResolutionFilter => self.resolution_filter = !self.resolution_filter,
            Action::DurationFilter => self.duration_filter = !self.duration_filter,
            Action::SourceFilter => self.source_filter = !self.source_filter,
            Action::ClearFilters => {
                self.query.clear();
                self.type_filter = 0;
                self.resolution_filter = false;
                self.duration_filter = false;
                self.source_filter = false;
                self.category = "All".into();
                self.folder.clear();
            }
            Action::Sort => self.sort_ascending = !self.sort_ascending,
            Action::Layout(v) => self.list = v,
            Action::FilterPanel => self.filters_open = !self.filters_open,
            Action::Favorite => {
                if let Some(asset) = self.assets.get_mut(self.selected) {
                    asset.favorite = !asset.favorite;
                }
            }
            Action::FavoriteAsset(index) => {
                if let Some(asset) = self.assets.get_mut(index) {
                    asset.favorite = !asset.favorite;
                }
            }
            Action::Import
            | Action::ImportFolder
            | Action::ShowInFinder
            | Action::ShowAssetInFinder(_) => {}
            Action::AddToTimeline => {
                let mut ids: Vec<_> = self
                    .checked
                    .iter()
                    .copied()
                    .filter(|&id| id < self.assets.len())
                    .collect();
                self.track_revision = self.track_revision.wrapping_add(1);
                ids.sort_by_key(|&id| match self.assets[id].kind {
                    Kind::Video => 0,
                    Kind::Audio => 1,
                    Kind::Image => 2,
                });
                if ids.is_empty() {
                    self.notice = Some("Select media to add to the timeline.".into());
                    return;
                }
                if ids.iter().any(|&id| {
                    self.assets[id].kind != Kind::Image && self.assets[id].duration <= 0.
                }) {
                    self.notice = Some("Duration is unavailable. Check that the video is readable and reimport the media before adding it to the timeline.".into());
                    return;
                }
                for id in &ids {
                    if let Some(prepared) = self.assets[*id].prepared.clone() {
                        let group = self.next_link_group;
                        self.next_link_group += 1;
                        let primary_track = (!prepared.videos.is_empty()).then(|| {
                            self.tracks
                                .iter()
                                .position(|track| !track.audio && !track.locked)
                                .unwrap_or_else(|| {
                                    self.tracks.push(Track {
                                        name: "Video".into(),
                                        audio: false,
                                        visible: true,
                                        locked: false,
                                        muted: false,
                                        gain: 1.,
                                    });
                                    self.tracks.len() - 1
                                })
                        });
                        let start = if prepared.videos.is_empty() {
                            self.position.max(0.)
                        } else {
                            self.clips
                                .iter()
                                .filter(|clip| Some(clip.track) == primary_track)
                                .map(|clip| clip.start + clip.length)
                                .fold(0., f32::max)
                        };
                        let mut first_video = None;
                        for video in &prepared.videos {
                            let track = if Some(video.index) == prepared.primary_video {
                                primary_track.unwrap()
                            } else {
                                self.tracks.push(Track {
                                    name: format!("{} · V{}", self.assets[*id].name, video.index),
                                    audio: false,
                                    visible: true,
                                    locked: false,
                                    muted: false,
                                    gain: 1.,
                                });
                                self.tracks.len() - 1
                            };
                            let offset = video
                                .timing
                                .start_pts
                                .map(|pts| video.timing.time_base.seconds(pts))
                                .unwrap_or(prepared.origin_seconds)
                                - prepared.origin_seconds;
                            let length = video
                                .timing
                                .duration_pts
                                .map(|pts| video.timing.time_base.seconds(pts))
                                .unwrap_or(self.assets[*id].duration as f64)
                                .max(0.) as f32;
                            if length <= 0. {
                                continue;
                            }
                            self.clips.push(Clip {
                                name: None,
                                asset: *id,
                                track,
                                start: start + offset as f32,
                                length,
                                source_start: 0.,
                                component: Some(ClipComponent::Video(video.index)),
                                link_group: Some(group),
                            });
                            if Some(video.index) == prepared.primary_video {
                                first_video = Some(self.clips.len() - 1);
                            }
                        }
                        let first_channel = self.clips.len();
                        for audio in &prepared.audio {
                            for channel in &audio.channels {
                                self.tracks.push(Track {
                                    name: format!(
                                        "{} · S{} {}",
                                        self.assets[*id].name, audio.index, channel.name
                                    ),
                                    audio: true,
                                    visible: true,
                                    locked: false,
                                    muted: false,
                                    gain: 1.,
                                });
                                let track = self.tracks.len() - 1;
                                for span in &audio.spans {
                                    let time = span
                                        .pts
                                        .map(|pts| audio.timing.time_base.seconds(pts))
                                        .unwrap_or(
                                            audio.start_seconds()
                                                + span.first_sample as f64
                                                    / audio.sample_rate as f64,
                                        );
                                    self.clips.push(Clip {
                                        name: None,
                                        asset: *id,
                                        track,
                                        start: start + (time - prepared.origin_seconds) as f32,
                                        length: span.samples as f32 / audio.sample_rate as f32,
                                        source_start: span.first_sample as f32
                                            / audio.sample_rate as f32,
                                        component: Some(ClipComponent::AudioChannel {
                                            stream: audio.index,
                                            channel: channel.index,
                                        }),
                                        link_group: Some(group),
                                    });
                                }
                            }
                        }
                        if let Some(selected) = first_video
                            .or_else(|| (first_channel < self.clips.len()).then_some(first_channel))
                        {
                            self.selected_clip = selected;
                        }
                        self.clip_sources.insert(
                            group,
                            self.clips
                                .iter()
                                .filter(|clip| clip.link_group == Some(group))
                                .cloned()
                                .collect(),
                        );
                    } else {
                        let audio = self.assets[*id].kind == Kind::Audio;
                        let track = self
                            .tracks
                            .iter()
                            .position(|track| track.audio == audio && !track.locked)
                            .unwrap_or_else(|| {
                                self.tracks.push(Track {
                                    name: if audio {
                                        "Audio".into()
                                    } else {
                                        "Video".into()
                                    },
                                    audio,
                                    visible: true,
                                    locked: false,
                                    muted: false,
                                    gain: 1.,
                                });
                                self.tracks.len() - 1
                            });
                        let start = self
                            .clips
                            .iter()
                            .filter(|clip| clip.track == track)
                            .map(|clip| clip.start + clip.length)
                            .fold(0., f32::max);
                        let length = if self.assets[*id].kind == Kind::Image {
                            5.
                        } else {
                            self.assets[*id].duration
                        };
                        self.clips.push(Clip {
                            name: None,
                            asset: *id,
                            track,
                            start,
                            length,
                            source_start: 0.,
                            component: None,
                            link_group: None,
                        });
                        self.selected_clip = self.clips.len() - 1;
                    }
                }
                self.selected = self.clips[self.selected_clip].asset;
                self.position = self.clips[self.selected_clip].start;
                self.playing = false;
                self.apply(Action::Screen(Screen::Edit));
                self.notice = Some(format!(
                    "Added {} item(s) to the project timeline.",
                    ids.len()
                ));
            }
            Action::CancelAll => self.notice = Some("Stopping media processing…".into()),
            Action::Play => {
                if self.assets.is_empty() {
                    self.notice = Some("Import media to start playback.".into());
                    return;
                }
                self.playing = !self.playing;
                self.notice = if self
                    .assets
                    .get(self.selected)
                    .is_some_and(|asset| asset.path.is_some())
                {
                    None
                } else {
                    Some(
                        if self.playing {
                            "Source unavailable for playback"
                        } else {
                            "Preview paused"
                        }
                        .into(),
                    )
                };
            }
            Action::Seek(v) => self.position = v.clamp(0., self.seek_limit()),
            Action::Step(v) => self.position = (self.position + v).clamp(0., self.seek_limit()),
            Action::Mute => self.muted = !self.muted,
            Action::Loop => self.looping = !self.looping,
            Action::Fullscreen => self.fullscreen = !self.fullscreen,
            Action::Inspector(v) => self.inspector = v.min(1),
            Action::DetailTab(v) => self.detail_tab = v,
            Action::Control(i, v) if i < self.controls.len() => {
                self.controls[i] = v;
                if i == 8 {
                    if self.screen.has_timeline()
                        && self.inspector == 1
                        && let Some(track) = self
                            .clips
                            .get(self.selected_clip)
                            .and_then(|clip| self.tracks.get_mut(clip.track))
                            .filter(|track| track.audio)
                    {
                        track.gain = v.clamp(0., 100.) / 100.;
                    } else {
                        self.monitor_gain = v.clamp(0., 100.) / 100.;
                    }
                }
            }
            Action::SelectClip(i) if i < self.clips.len() => {
                self.selected_clip = i;
                self.selected = self.clips[i].asset;
                self.position = self.clips[i].start;
                self.playing = false;
                self.inspector = if self.assets[self.selected].kind == Kind::Audio
                    || matches!(
                        self.clips[i].component,
                        Some(ClipComponent::AudioChannel { .. })
                    ) {
                    1
                } else {
                    0
                };
                self.controls[8] = self.tracks[self.clips[i].track].gain * 100.;
                self.checked = BTreeSet::from([self.selected]);
            }
            Action::TargetClip(index) => {
                let position = self.position;
                let playing = self.playing;
                self.apply(Action::SelectClip(index));
                self.position = position;
                self.playing = playing;
            }
            Action::TargetAsset(index) => {
                let position = self.position;
                let playing = self.playing;
                self.apply(Action::Select(index));
                self.position = position;
                self.playing = playing;
            }
            Action::RenameTrack(index, name) => {
                self.notice = Some(match Self::valid_name(name) {
                    Ok(name) => {
                        if let Some(track) = self.tracks.get_mut(index) {
                            track.name = name;
                            "Track renamed.".into()
                        } else {
                            "Track is no longer available.".into()
                        }
                    }
                    Err(error) => error,
                });
            }
            Action::RenameClip(index, name) => {
                let locked = self.linked_locked(index);
                self.notice = Some(match Self::valid_name(name) {
                    Ok(name) => {
                        if let Some(clip) = self.clips.get_mut(index) {
                            if locked {
                                "Unlock the linked tracks before renaming the clip.".into()
                            } else {
                                clip.name = Some(name.clone());
                                if let Some(sources) = clip
                                    .link_group
                                    .and_then(|group| self.clip_sources.get_mut(&group))
                                {
                                    for source in sources {
                                        if source.track == clip.track
                                            && source.component == clip.component
                                            && source.source_start <= clip.source_start + 1e-5
                                            && source.source_start + source.length + 1e-5
                                                >= clip.source_start + clip.length
                                        {
                                            source.name = Some(name.clone());
                                        }
                                    }
                                }
                                "Clip renamed; source file unchanged.".into()
                            }
                        } else {
                            "Clip is no longer available.".into()
                        }
                    }
                    Err(error) => error,
                });
            }
            Action::MoveTrack { from, to } => {
                self.notice = Some(self.move_track(from, to).map_or_else(
                    |error| error,
                    |_| "Track reordered; clip ranges and linked channels preserved.".into(),
                ))
            }
            Action::CopyClips => {
                self.notice = Some(
                    self.copy_clips()
                        .map_or_else(|e| e, |_| "Clip and linked channels copied.".into()),
                );
            }
            Action::CutClips => {
                if self.linked_locked(self.selected_clip) {
                    self.notice = Some("Unlock the linked tracks before cutting.".into());
                } else {
                    match self.copy_clips() {
                        Ok(()) => {
                            self.apply(Action::Tool("Delete"));
                            self.playing = false;
                            self.notice =
                                Some("Clip and linked channels cut. Paste at the playhead.".into());
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
            }
            Action::PasteClips => {
                self.notice = Some(
                    self.paste_clips()
                        .map_or_else(|e| e, |_| "Clips pasted at the playhead.".into()),
                );
            }
            Action::DeleteTrack(index) => {
                self.notice = Some(
                    self.delete_track(index)
                        .map_or_else(|e| e, |_| "Track and its clips deleted.".into()),
                );
            }
            Action::DuplicateClip(index) => {
                self.notice = Some(self.duplicate_clip(index).map_or_else(
                    |error| error,
                    |_| "Clip and linked channels duplicated.".into(),
                ))
            }
            Action::EditClipTiming {
                index,
                start,
                source_start,
                length,
            } => {
                self.notice = Some(
                    self.edit_clip_timing(index, start, source_start, length)
                        .map_or_else(
                            |error| error,
                            |_| "Timing updated; linked channels stay aligned.".into(),
                        ),
                )
            }
            Action::TrackVisible(i) => {
                if let Some(t) = self.tracks.get_mut(i) {
                    t.visible = !t.visible;
                }
            }
            Action::TrackMuted(i) => {
                if let Some(track) = self.tracks.get_mut(i) {
                    track.muted = !track.muted;
                }
            }
            Action::TrackLocked(i) => {
                if let Some(t) = self.tracks.get_mut(i) {
                    t.locked = !t.locked;
                }
            }
            Action::AddTrack => {
                self.track_revision = self.track_revision.wrapping_add(1);
                self.tracks.push(Track {
                    name: format!(
                        "Video {}",
                        self.tracks.iter().filter(|t| !t.audio).count() + 1
                    ),
                    audio: false,
                    visible: true,
                    locked: false,
                    muted: false,
                    gain: 1.,
                });
            }
            Action::Zoom(v) => self.zoom = (self.zoom + v).clamp(0.25, 8.),
            Action::Dismiss => self.notice = None,
            Action::Export
            | Action::SaveProject
            | Action::OpenProject
            | Action::CancelExport
            | Action::Undo
            | Action::Redo => {}
            Action::Tool("Split") => {
                let selected_index = self.selected_clip;
                if let Some(selected) = self.clips.get(selected_index).cloned() {
                    let linked: Vec<_> = self
                        .clips
                        .iter()
                        .enumerate()
                        .filter(|(index, clip)| {
                            selected
                                .link_group
                                .map_or(*index == self.selected_clip, |group| {
                                    clip.link_group == Some(group)
                                })
                        })
                        .map(|(index, _)| index)
                        .collect();
                    if self.linked_locked(selected_index) {
                        self.notice = Some("Unlock the linked tracks before splitting.".into());
                    } else if self.position > selected.start
                        && self.position < selected.start + selected.length
                    {
                        let right_group = selected.link_group.map(|_| {
                            let group = self.next_link_group;
                            self.next_link_group += 1;
                            group
                        });
                        if let (Some(left), Some(right)) = (selected.link_group, right_group)
                            && let Some(mut sources) = self.clip_sources.get(&left).cloned()
                        {
                            for clip in &mut sources {
                                clip.link_group = Some(right);
                            }
                            self.clip_sources.insert(right, sources);
                        }
                        for index in linked {
                            let clip = self.clips[index].clone();
                            if clip.start >= self.position {
                                self.clips[index].link_group = right_group;
                            } else if clip.start + clip.length > self.position {
                                let offset = self.position - clip.start;
                                self.clips[index].length = offset;
                                self.clips.push(Clip {
                                    start: self.position,
                                    length: clip.length - offset,
                                    source_start: clip.source_start + offset,
                                    link_group: right_group,
                                    ..clip
                                });
                                if index == selected_index {
                                    self.selected_clip = self.clips.len() - 1;
                                }
                            }
                        }
                        self.notice = Some(
                            "Clip split at the playhead; linked channels stay aligned.".into(),
                        );
                    } else {
                        self.notice =
                            Some("Place the playhead inside the selected clip to split it.".into());
                    }
                }
            }
            Action::Tool("Delete") => {
                if let Some(selected) = self.clips.get(self.selected_clip) {
                    let group = selected.link_group;
                    let linked: BTreeSet<_> = self
                        .clips
                        .iter()
                        .enumerate()
                        .filter(|(index, clip)| {
                            group.map_or(*index == self.selected_clip, |group| {
                                clip.link_group == Some(group)
                            })
                        })
                        .map(|(index, _)| index)
                        .collect();
                    if self.linked_locked(self.selected_clip) {
                        self.notice = Some("Unlock the linked tracks before deleting.".into());
                    } else {
                        let mut index = 0;
                        self.clips.retain(|_| {
                            let keep = !linked.contains(&index);
                            index += 1;
                            keep
                        });
                        if let Some(group) = group {
                            self.clip_sources.remove(&group);
                        }
                        self.selected_clip =
                            self.selected_clip.min(self.clips.len().saturating_sub(1));
                        if let Some(clip) = self.clips.get(self.selected_clip) {
                            self.selected = clip.asset;
                        }
                        self.position = self.position.min(self.seek_limit());
                        self.notice =
                            Some("Clip and linked channels removed from the timeline.".into());
                    }
                }
            }
            Action::Tool(v) => self.notice = Some(format!("Unavailable action: {v}")),
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn native_project() -> (crate::Workspace, EditorState) {
        let workspace = crate::Workspace::new().unwrap();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/tests/multichannel.mkv");
        let report = crate::catalog::import(
            vec![path],
            BTreeSet::new(),
            Some(&workspace.path),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let mut state = EditorState::default();
        state.import_into_project(report.assets);
        (workspace, state)
    }
    fn drag(state: &mut EditorState, kind: ClipDragKind, delta: f32) {
        let index = state.selected_clip;
        let timing = state.drag_clip_timing(index, kind, delta).unwrap();
        state
            .edit_clip_timing(index, timing.start, timing.source_start, timing.length)
            .unwrap();
    }
    #[test]
    fn timeline_standard_shortcuts_map_without_modified_delete() {
        assert!(matches!(
            timeline_shortcut("x", true, false, false),
            Some(Action::CutClips)
        ));
        assert!(matches!(
            timeline_shortcut("c", true, false, false),
            Some(Action::CopyClips)
        ));
        assert!(matches!(
            timeline_shortcut("v", true, false, false),
            Some(Action::PasteClips)
        ));
        for key in ["delete", "backspace"] {
            assert!(matches!(
                timeline_shortcut(key, false, false, false),
                Some(Action::Tool("Delete"))
            ));
        }
        assert!(timeline_shortcut("delete", true, false, false).is_none());
        assert!(timeline_shortcut("v", true, true, false).is_none());
    }
    #[test]
    fn copy_paste_keeps_linked_offsets_names_and_independent_trim_recovery() {
        let (_workspace, mut state) = native_project();
        state.clips[0].name = Some("Picture".into());
        state.selected_clip = 0;
        drag(&mut state, ClipDragKind::Start, 0.1);
        let original = state.clips.clone();
        let count = original.len();
        let old_group = original[0].link_group;
        state.apply(Action::CopyClips);
        state.position = 2.;
        state.apply(Action::PasteClips);
        assert_eq!(state.clips.len(), count * 2);
        let first = original
            .iter()
            .map(|c| c.start)
            .fold(f32::INFINITY, f32::min);
        for (before, pasted) in original.iter().zip(&state.clips[count..]) {
            assert!((pasted.start - (2. + before.start - first)).abs() < 1e-5);
            assert_eq!(
                (pasted.source_start, pasted.length, &pasted.name),
                (before.source_start, before.length, &before.name)
            );
            assert_ne!(pasted.link_group, old_group);
            assert_eq!(pasted.link_group, state.clips[count].link_group);
        }
        drag(&mut state, ClipDragKind::Start, -0.1);
        assert!(state.clips[state.selected_clip].length > original[0].length);
        assert_eq!(state.clips[0].length, original[0].length);
    }
    #[test]
    fn cut_and_paste_respect_locks_and_keep_clipboard_on_rejection() {
        let (_workspace, mut state) = native_project();
        state.selected_clip = 0;
        state.apply(Action::CopyClips);
        let original = state.clips.len();
        state.tracks[state.clips[0].track].locked = true;
        state.apply(Action::CutClips);
        assert_eq!(state.clips.len(), original);
        state.apply(Action::PasteClips);
        assert_eq!(state.clips.len(), original);
        state.tracks[state.clips[0].track].locked = false;
        state.apply(Action::CutClips);
        assert!(state.clips.is_empty());
        state.position = 3.;
        state.apply(Action::PasteClips);
        assert_eq!(state.clips.len(), original);
        assert!((state.clips[0].start - 3.).abs() < 1e-5);
    }
    #[test]
    fn track_deletion_remaps_survivors_and_does_not_restore_deleted_channels() {
        let (_workspace, mut state) = native_project();
        let removed = state.clips.iter().find(|c| c.track == 1).unwrap().component;
        state.selected_clip = 0;
        drag(&mut state, ClipDragKind::Start, 0.1);
        state.apply(Action::CopyClips);
        let tracks = state.tracks.len();
        state.tracks[1].locked = true;
        state.apply(Action::DeleteTrack(1));
        assert_eq!(state.tracks.len(), tracks);
        state.tracks[1].locked = false;
        state.apply(Action::DeleteTrack(1));
        assert_eq!(state.tracks.len(), tracks - 1);
        assert!(state.clips.iter().all(|c| c.track < state.tracks.len()));
        drag(&mut state, ClipDragKind::Start, -0.1);
        assert!(state.clips.iter().all(|c| c.component != removed));
        state.position = 3.;
        state.apply(Action::PasteClips);
        assert_eq!(state.tracks.len(), tracks);
        assert!(state.clips.iter().any(|c| c.component == removed));
        state.position = 5.;
        state.apply(Action::PasteClips);
        assert_eq!(state.tracks.len(), tracks);
    }
    #[test]
    fn clipboard_destinations_follow_track_reordering_and_last_track_can_be_deleted() {
        let mut state = EditorState::mock();
        state.selected_clip = 0;
        state.apply(Action::CopyClips);
        state.apply(Action::MoveTrack { from: 0, to: 2 });
        state.position = 4.;
        state.apply(Action::PasteClips);
        assert_eq!(state.clips[state.selected_clip].track, 2);
        while !state.tracks.is_empty() {
            state.tracks[0].locked = false;
            state.apply(Action::DeleteTrack(0));
        }
        assert!(state.clips.is_empty());
        state.position = 0.;
        state.apply(Action::PasteClips);
        assert_eq!(state.tracks.len(), 1);
        assert_eq!(state.clips.len(), 1);
    }
    #[test]
    fn dragging_moves_linked_ranges_and_restores_both_source_edges() {
        let (_workspace, mut state) = native_project();
        let original = state.clips.clone();
        drag(&mut state, ClipDragKind::Move, 2.);
        for (old, new) in original.iter().zip(&state.clips) {
            assert!((new.start - old.start - 2.).abs() < 1e-5);
            assert_eq!(
                (new.length, new.source_start, new.link_group),
                (old.length, old.source_start, old.link_group)
            );
        }
        drag(&mut state, ClipDragKind::Start, 0.1);
        drag(&mut state, ClipDragKind::End, -0.1);
        let clip = &state.clips[state.selected_clip];
        assert!((clip.start - 2.1).abs() < 1e-5 && (clip.length - 0.2).abs() < 1e-5);
        drag(&mut state, ClipDragKind::Start, -100.);
        drag(&mut state, ClipDragKind::End, 100.);
        let clip = &state.clips[state.selected_clip];
        assert!((clip.start - 2.).abs() < 1e-5 && clip.source_start.abs() < 1e-5);
        assert!((clip.length - original[0].length).abs() < 1e-5);
        for clip in &state.clips {
            let old = &original[clip.track];
            assert!((clip.start - old.start - 2.).abs() < 1e-5);
            assert!((clip.source_start - old.source_start).abs() < 1e-5);
            assert!((clip.length - old.length).abs() < 1e-5);
        }
        drag(&mut state, ClipDragKind::Move, -100.);
        assert!(state.clips.iter().all(|clip| clip.start >= 0.));
        assert!(
            state
                .drag_clip_timing(state.selected_clip, ClipDragKind::End, f32::NAN)
                .is_err()
        );
    }
    #[test]
    fn resizing_audio_preserves_video_lead_in_and_restores_it_after_start_trim() {
        let (_workspace, mut state) = native_project();
        let group = state.clips[0].link_group.unwrap();
        state.clips[1].start = 0.05;
        state.clips[1].length = 0.35;
        state.clip_sources.get_mut(&group).unwrap()[1] = state.clips[1].clone();
        state.apply(Action::SelectClip(1));
        drag(&mut state, ClipDragKind::End, -0.1);
        assert_eq!(
            state
                .clips
                .iter()
                .find(|clip| clip.track == 0)
                .unwrap()
                .start,
            0.
        );
        drag(&mut state, ClipDragKind::End, 0.1);
        drag(&mut state, ClipDragKind::Start, 0.1);
        assert!(
            (state
                .clips
                .iter()
                .find(|clip| clip.track == 0)
                .unwrap()
                .start
                - 0.15)
                .abs()
                < 1e-5
        );
        drag(&mut state, ClipDragKind::Start, -0.1);
        assert!(
            state
                .clips
                .iter()
                .find(|clip| clip.track == 0)
                .unwrap()
                .start
                .abs()
                < 1e-5
        );
    }
    #[test]
    fn extending_restores_hidden_channel_spans_after_reordering_and_respects_locks() {
        let (_workspace, mut state) = native_project();
        let group = state.clips[0].link_group.unwrap();
        // A secondary stream starts later; trimming must keep its recovery metadata.
        state.clips[3].start = 0.25;
        state.clips[3].length = 0.1;
        state.clips[3].source_start = 0.;
        state.clip_sources.get_mut(&group).unwrap()[3] = state.clips[3].clone();
        state.apply(Action::RenameClip(3, "Late narration".into()));
        drag(&mut state, ClipDragKind::End, -0.3);
        assert_eq!(state.clips.len(), 3);
        state.tracks[3].locked = true;
        assert!(
            state
                .drag_clip_timing(state.selected_clip, ClipDragKind::End, 0.3)
                .is_err()
        );
        state.tracks[3].locked = false;
        state.move_track(3, 0).unwrap();
        drag(&mut state, ClipDragKind::End, 0.3);
        let restored = state.clips.iter().find(|clip| clip.track == 0).unwrap();
        assert!((restored.start - 0.25).abs() < 1e-5 && (restored.length - 0.1).abs() < 1e-5);
        assert_eq!(restored.source_start, 0.);
        assert_eq!(restored.link_group, Some(group));
        assert_eq!(restored.name.as_deref(), Some("Late narration"));
    }
    #[test]
    fn duplicate_and_split_keep_independent_recoverable_source_ranges() {
        let (_workspace, mut state) = native_project();
        drag(&mut state, ClipDragKind::End, -0.2);
        state.position = 0.1;
        state.apply(Action::Tool("Split"));
        let right_group = state.clips[state.selected_clip].link_group;
        state.apply(Action::DuplicateClip(state.selected_clip));
        assert_ne!(state.clips[state.selected_clip].link_group, right_group);
        let prior = state
            .clips
            .iter()
            .filter(|clip| clip.link_group == right_group)
            .cloned()
            .collect::<Vec<_>>();
        drag(&mut state, ClipDragKind::Move, 1.);
        drag(&mut state, ClipDragKind::End, 100.);
        let copy = &state.clips[state.selected_clip];
        assert!((copy.source_start + copy.length - 0.4).abs() < 1e-5);
        for (old, new) in prior.iter().zip(
            state
                .clips
                .iter()
                .filter(|clip| clip.link_group == right_group),
        ) {
            assert_eq!(
                (old.start, old.length, old.source_start),
                (new.start, new.length, new.source_start)
            );
        }
    }
    #[test]
    fn audio_span_edge_drag_keeps_the_selected_span_and_its_gap() {
        let (_workspace, mut state) = native_project();
        let group = state.clips[0].link_group.unwrap();
        state.clips[1].start = 0.;
        state.clips[1].source_start = 0.;
        state.clips[1].length = 0.1;
        let later = Clip {
            start: 0.2,
            source_start: 0.1,
            length: 0.1,
            ..state.clips[1].clone()
        };
        let sources = state.clip_sources.get_mut(&group).unwrap();
        sources[1] = state.clips[1].clone();
        sources.push(later.clone());
        state.clips.push(later);
        state.apply(Action::SelectClip(4));
        drag(&mut state, ClipDragKind::End, -0.05);
        assert!((state.clips[state.selected_clip].source_start - 0.1).abs() < 1e-5);
        drag(&mut state, ClipDragKind::End, 0.05);
        let selected = &state.clips[state.selected_clip];
        assert!((selected.start - 0.2).abs() < 1e-5 && (selected.source_start - 0.1).abs() < 1e-5);
        let spans = state
            .clips
            .iter()
            .filter(|clip| clip.track == 1)
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 2);
        assert!((spans[0].start + spans[0].length - 0.1).abs() < 1e-5);
        assert!((spans[1].start - 0.2).abs() < 1e-5);
    }
    #[test]
    fn gesture_snapshot_restores_metadata_and_inspector_has_two_modes() {
        let (_workspace, mut state) = native_project();
        let snapshot = state.timeline_snapshot();
        let prepared = state.assets[0].prepared.as_ref().unwrap().clone();
        let original = state.clips.clone();
        drag(&mut state, ClipDragKind::Move, 1.);
        drag(&mut state, ClipDragKind::End, -0.3);
        state.restore_timeline(&snapshot);
        for (old, new) in original.iter().zip(&state.clips) {
            assert_eq!(
                (old.start, old.length, old.source_start),
                (new.start, new.length, new.source_start)
            );
        }
        assert!(Arc::ptr_eq(
            &prepared,
            state.assets[0].prepared.as_ref().unwrap()
        ));
        state.apply(Action::Inspector(4));
        assert_eq!(state.inspector, 1);
        state.apply(Action::Screen(Screen::Edit));
        assert_eq!(state.inspector, 0);
        state.apply(Action::SelectClip(1));
        assert_eq!(state.inspector, 1);
        state.apply(Action::Screen(Screen::Edit));
        assert_eq!(state.inspector, 1);
    }
    #[test]
    fn reordering_remaps_every_clip_preserves_channel_properties_and_playback() {
        let (_workspace, mut state) = native_project();
        state.tracks[1].muted = true;
        state.tracks[2].locked = true;
        state.tracks[2].gain = 0.3;
        state.apply(Action::TargetClip(2));
        let selected = state.selected_clip;
        let clips = state.clips.clone();
        let audio = crate::playback::PlaybackPlan::from_state(&state).audio;
        let moved_name = state.tracks[2].name.clone();
        state.apply(Action::MoveTrack { from: 2, to: 0 });
        assert_eq!(state.selected_clip, selected);
        assert_eq!(state.clips[selected].track, 0);
        assert_eq!(state.tracks[0].name, moved_name);
        assert!(state.tracks[0].locked);
        assert_eq!(state.tracks[0].gain, 0.3);
        assert!(state.tracks[2].muted);
        for (old, new) in clips.iter().zip(&state.clips) {
            assert_eq!(
                state.tracks[new.track].audio,
                matches!(old.component, Some(ClipComponent::AudioChannel { .. }))
            );
            assert_eq!(
                (
                    old.asset,
                    old.start,
                    old.length,
                    old.source_start,
                    old.component,
                    old.link_group
                ),
                (
                    new.asset,
                    new.start,
                    new.length,
                    new.source_start,
                    new.component,
                    new.link_group
                )
            );
        }
        let after = crate::playback::PlaybackPlan::from_state(&state).audio;
        for (old, new) in audio.iter().zip(after) {
            assert_eq!(
                (
                    old.stream,
                    old.channel,
                    old.start,
                    old.source_start,
                    old.duration,
                    old.gain
                ),
                (
                    new.stream,
                    new.channel,
                    new.start,
                    new.source_start,
                    new.duration,
                    new.gain
                )
            );
        }
        state.apply(Action::MoveTrack { from: 0, to: 3 });
        state.apply(Action::MoveTrack { from: 3, to: 2 });
        for (old, new) in clips.iter().zip(&state.clips) {
            assert_eq!(old.track, new.track);
        }
        let revision = state.track_revision;
        state.apply(Action::MoveTrack { from: 99, to: 0 });
        assert_eq!(state.track_revision, revision);
    }
    #[test]
    fn clip_names_are_local_and_split_duplicate_keep_them_with_new_links() {
        let (_workspace, mut state) = native_project();
        let video = state.selected_clip;
        let asset = state.clips[video].asset;
        let source_name = state.assets[asset].name.clone();
        state.apply(Action::RenameClip(video, "  Opening shot  ".into()));
        assert_eq!(state.clip_label(video), "Opening shot");
        assert_eq!(state.assets[asset].name, source_name);
        state.apply(Action::RenameTrack(0, "Picture".into()));
        assert_eq!(state.tracks[0].name, "Picture");
        state.apply(Action::RenameClip(
            video,
            "
"
            .into(),
        ));
        assert_eq!(state.clip_label(video), "Opening shot");
        state.position = state.clips[video].start + 0.2;
        state.apply(Action::Tool("Split"));
        let right = state.selected_clip;
        assert_eq!(state.clip_label(right), "Opening shot");
        let count = state.linked_clip_indices(right).len();
        let previous = state.clips.len();
        let old_group = state.clips[right].link_group;
        state.apply(Action::DuplicateClip(right));
        assert_eq!(state.clips.len(), previous + count);
        assert_eq!(state.clip_label(state.selected_clip), "Opening shot");
        assert_ne!(state.clips[state.selected_clip].link_group, old_group);
        assert_eq!(
            state.clips[state.selected_clip].source_start,
            state.clips[right].source_start
        );
    }
    #[test]
    fn inline_timing_moves_and_trims_linked_channels_and_rejects_locked_invalid_ranges() {
        let (_workspace, mut state) = native_project();
        let video = state.selected_clip;
        let original = state.clips.clone();
        let selected = state.clips[video].clone();
        state.apply(Action::EditClipTiming {
            index: video,
            start: 2.,
            source_start: selected.source_start,
            length: selected.length,
        });
        let shift = 2. - selected.start;
        for (old, new) in original.iter().zip(&state.clips) {
            assert!((new.start - old.start - shift).abs() < 0.000001);
            assert_eq!(
                (old.source_start, old.length, old.link_group),
                (new.source_start, new.length, new.link_group)
            );
        }
        state.apply(Action::EditClipTiming {
            index: video,
            start: 1.,
            source_start: 0.1,
            length: 0.2,
        });
        assert_eq!(state.clips.len(), 4);
        for clip in &state.clips {
            assert!((clip.start - 1.).abs() < 0.000001);
            assert!((clip.length - 0.2).abs() < 0.000001);
            let old = &original[clip.track];
            assert!(
                (clip.source_start - (old.source_start + selected.start + 0.1 - old.start)).abs()
                    < 0.000001
            );
            assert_eq!(clip.link_group, selected.link_group);
        }
        let before = state.clips.clone();
        state.tracks[2].locked = true;
        state.apply(Action::EditClipTiming {
            index: state.selected_clip,
            start: 3.,
            source_start: 0.1,
            length: 0.1,
        });
        assert!(state.notice.as_deref().unwrap().contains("Unlock"));
        state.tracks[2].locked = false;
        for (start, source_start, length) in [
            (f32::NAN, 0.1, 0.1),
            (-1., 0.1, 0.1),
            (1., 0., 100.),
            (1., 0.1, 0.),
        ] {
            state.apply(Action::EditClipTiming {
                index: state.selected_clip,
                start,
                source_start,
                length,
            });
            for (old, new) in before.iter().zip(&state.clips) {
                assert_eq!(
                    (old.start, old.length, old.source_start),
                    (new.start, new.length, new.source_start)
                );
            }
        }
    }
    #[test]
    fn context_target_preserves_transport_position_for_split_at_playhead() {
        let mut state = EditorState::mock();
        state.position = 2.;
        state.playing = true;
        state.apply(Action::TargetClip(0));
        assert_eq!(state.position, 2.);
        assert!(state.playing);
        state.apply(Action::Tool("Split"));
        assert_eq!(state.clips[state.selected_clip].start, 2.);
    }

    #[test]
    fn library_context_actions_preserve_the_playhead_for_audio_insertion() {
        let (workspace, mut state) = native_project();
        let audio = state.assets.len();
        let report = crate::catalog::import(
            vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/media/cracker-stereo.wav")],
            BTreeSet::new(),
            Some(&workspace.path),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        state.assets.extend(report.assets);
        state.position = 2.;
        state.playing = true;
        let selection = state.selected;
        state.apply(Action::FavoriteAsset(audio));
        assert_eq!(state.selected, selection);
        state.apply(Action::TargetAsset(audio));
        assert_eq!(state.position, 2.);
        assert!(state.playing);
        assert!(state.assets[audio].favorite);
        state.apply(Action::AddToTimeline);
        let inserted = &state.clips[state.selected_clip];
        assert_eq!(inserted.asset, audio);
        assert_eq!(inserted.start, 2.);
    }

    #[test]
    fn empty_project_handles_transport_and_screen_changes_without_fixture_assets() {
        let mut state = EditorState::default();
        assert!(state.assets.is_empty() && state.clips.is_empty());
        for screen in Screen::ALL {
            state.apply(Action::Screen(screen));
            state.apply(Action::Favorite);
            state.apply(Action::Play);
            state.apply(Action::Seek(100.));
        }
        assert!(!state.playing);
        assert_eq!(state.seek_limit(), 1.);
    }

    #[test]
    fn bundled_real_files_import_to_video_and_seven_channel_tracks_without_duplicates() {
        use std::sync::atomic::AtomicBool;
        let workspace = crate::Workspace::new().unwrap();
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/media");
        let (files, errors) =
            crate::catalog::discover(vec![folder.clone()], &AtomicBool::new(false));
        assert!(errors.is_empty());
        assert_eq!(files.len(), 4);
        let originals: Vec<_> = files
            .iter()
            .map(|(path, _)| (path.clone(), std::fs::read(path).unwrap()))
            .collect();
        let report = crate::catalog::import(
            vec![folder.clone()],
            BTreeSet::new(),
            Some(&workspace.path),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let mut state = EditorState::default();
        assert_eq!(state.import_into_project(report.assets), (4, 0, 4));
        assert_eq!(state.tracks.len(), 8);
        assert_eq!(state.tracks.iter().filter(|track| track.audio).count(), 7);
        assert!(
            state
                .assets
                .iter()
                .all(|asset| asset.path.is_some() && asset.prepared.is_some())
        );
        assert!(matches!(
            state.clips[state.selected_clip].component,
            Some(ClipComponent::Video(_))
        ));
        assert!(
            state
                .clips
                .iter()
                .filter(|clip| state.assets[clip.asset].kind == Kind::Audio)
                .all(|clip| clip.start >= 0. && clip.start < 0.1),
            "{:?}",
            state
                .clips
                .iter()
                .map(|clip| (&state.assets[clip.asset].name, clip.start, clip.length))
                .collect::<Vec<_>>()
        );
        let plan = crate::playback::PlaybackPlan::from_state(&state);
        assert_eq!(plan.videos.len(), 1);
        assert_eq!(plan.audio.len(), 7);
        assert!(state.project_duration() > 20.);
        let piano = state
            .assets
            .iter()
            .find(|asset| asset.name == "piano-emo-10-excerpt.ogg")
            .unwrap()
            .prepared
            .as_ref()
            .unwrap();
        assert_eq!(
            piano.audio[0].spans.len(),
            1,
            "Vorbis window transitions must not make false gaps/overlaps"
        );
        let video = state
            .assets
            .iter()
            .find(|asset| asset.name == "murchison-falls.webm")
            .unwrap()
            .prepared
            .as_ref()
            .unwrap();
        assert_eq!(
            video.audio[0].spans.len(),
            1,
            "Opus priming must use native packet time base"
        );
        assert_eq!(video.audio[0].sample_rate, 48_000);
        assert_eq!(piano.audio[0].sample_rate, 44_100);
        let clips = state.clips.len();
        let existing = state
            .assets
            .iter()
            .filter_map(|asset| asset.path.clone())
            .collect();
        let again = crate::catalog::import(
            vec![folder],
            existing,
            Some(&workspace.path),
            Arc::new(AtomicBool::new(false)),
        );
        assert_eq!(state.import_into_project(again.assets), (0, 4, 0));
        assert_eq!(state.clips.len(), clips);
        assert!(state.assets.iter().all(|asset| asset.prepared.is_some()));
        let audio_asset = state
            .assets
            .iter()
            .find(|asset| asset.name == "cracker-stereo.wav")
            .unwrap()
            .clone();
        let mut overlay = EditorState {
            position: 3.,
            ..EditorState::default()
        };
        assert_eq!(overlay.import_into_project(vec![audio_asset]), (1, 0, 1));
        assert_eq!(overlay.tracks.len(), 2);
        assert!(
            overlay
                .clips
                .iter()
                .all(|clip| (clip.start - 3.).abs() < 0.001)
        );
        for (path, bytes) in originals {
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
    }

    #[test]
    fn preprocessed_channels_have_separate_linked_tracks_and_edit_atomically() {
        let workspace = crate::Workspace::new().unwrap();
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/tests/multichannel.mkv");
        let report = crate::catalog::import(
            vec![path],
            BTreeSet::new(),
            Some(&workspace.path),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let mut state = EditorState::mock();
        state.clips.clear();
        state.tracks.clear();
        state.import_assets(report.assets);
        let asset = state.selected;
        state.apply(Action::AddToTimeline);
        assert_eq!(state.tracks.len(), 4); // One video and three independently editable channels.
        assert_eq!(state.tracks.iter().filter(|track| track.audio).count(), 3);
        let video = state.clips[state.selected_clip].clone();
        assert!(matches!(video.component, Some(ClipComponent::Video(0))));
        assert!(state.clips.iter().all(|clip| clip.start >= 0.));
        let group = video.link_group;
        assert!(group.is_some());
        assert!(state.clips.iter().all(|clip| clip.link_group == group));
        let pcm_track = state
            .clips
            .iter()
            .find(|clip| {
                matches!(
                    clip.component,
                    Some(ClipComponent::AudioChannel {
                        stream: 1,
                        channel: 0
                    })
                )
            })
            .unwrap()
            .track;
        let original = state.clips.len();
        let split = video.start + 0.2;
        state.position = split;
        state.tracks[pcm_track].locked = true;
        state.apply(Action::Tool("Split"));
        assert_eq!(state.clips.len(), original);
        state.tracks[pcm_track].locked = false;
        state.apply(Action::Tool("Split"));
        let right_group = state.clips[state.selected_clip].link_group;
        assert_ne!(right_group, group);
        assert!((state.clips[state.selected_clip].source_start - 0.2).abs() < 0.000001);
        let right: Vec<_> = state
            .clips
            .iter()
            .filter(|clip| clip.link_group == right_group)
            .collect();
        assert!(right.iter().any(|clip| clip.component
            == Some(ClipComponent::AudioChannel {
                stream: 1,
                channel: 0
            })));
        assert!(right.iter().any(|clip| clip.component
            == Some(ClipComponent::AudioChannel {
                stream: 1,
                channel: 1
            })));
        assert!(right.iter().all(|clip| clip.start >= split));
        let left: Vec<_> = state
            .clips
            .iter()
            .filter(|clip| clip.link_group == group)
            .collect();
        assert!(
            left.iter()
                .all(|clip| clip.start + clip.length <= split + 0.000001)
        );
        state.tracks[pcm_track].locked = true;
        let before = state.clips.len();
        state.apply(Action::Tool("Delete"));
        assert_eq!(state.clips.len(), before);
        state.tracks[pcm_track].locked = false;
        state.apply(Action::Tool("Delete"));
        assert!(state.clips.iter().all(|clip| clip.link_group == group));
        assert!(
            state.assets[asset]
                .prepared
                .as_ref()
                .unwrap()
                .manifest
                .is_file()
        );
    }

    #[test]
    fn imported_references_keep_stable_ids_filter_by_folder_and_extend_timeline() {
        let mut s = EditorState::mock();
        let old_clips = s.clips.len();
        let mut video = assets().remove(0);
        video.name = "My video.MP4".into();
        video.duration = 30.;
        video.path = Some("/videos/My video.MP4".into());
        video.import_folder = Some("/videos".into());
        let original = video.clone();
        let first = s.assets.len();
        assert_eq!(s.import_assets(vec![video]), (1, 0));
        assert_eq!(s.selected, first);
        assert_eq!(s.visible_assets()[0], first);
        s.apply(Action::Folder("/videos".into()));
        assert_eq!(s.visible_assets(), vec![first]);
        s.apply(Action::Search("my video".into()));
        assert_eq!(s.visible_assets(), vec![first]);
        assert_eq!(s.import_assets(vec![original]), (0, 1));
        s.apply(Action::AddToTimeline);
        assert_eq!(s.clips.len(), old_clips + 1);
        let clip = &s.clips[s.selected_clip];
        assert_eq!(clip.asset, first);
        assert_eq!(clip.length, 30.);
        assert_eq!(clip.source_start, 0.);
        assert!(s.project_duration() > 30.);
        let split = clip.start + 10.;
        s.apply(Action::Seek(split));
        s.apply(Action::Tool("Split"));
        let right = &s.clips[s.selected_clip];
        assert_eq!(right.start, split);
        assert_eq!(right.source_start, 10.);
        assert_eq!(right.length, 20.);
        assert_eq!(s.assets[first].duration, 30.);
        let track = right.track;
        s.apply(Action::TrackLocked(track));
        s.apply(Action::Tool("Delete"));
        assert_eq!(s.clips.len(), old_clips + 2);
        s.apply(Action::TrackLocked(track));
        s.apply(Action::Tool("Delete"));
        assert_eq!(s.clips.len(), old_clips + 1);
        assert_eq!(
            s.assets[first].path.as_deref(),
            Some(std::path::Path::new("/videos/My video.MP4"))
        );
    }
    #[test]
    fn unknown_duration_does_not_create_invalid_clips_and_locked_tracks_get_a_new_track() {
        let mut s = EditorState::mock();
        let mut video = assets().remove(0);
        video.path = Some("/videos/unknown.mp4".into());
        video.duration = 0.;
        assert_eq!(video.duration_label(), "Unknown duration");
        s.import_assets(vec![video]);
        let count = s.clips.len();
        s.apply(Action::AddToTimeline);
        assert_eq!(s.clips.len(), count);
        assert!(s.notice.as_ref().unwrap().contains("Duration"));
        s.assets[s.selected].favorite = true;
        let id = s.selected;
        let mut repaired = s.assets[id].clone();
        repaired.duration = 10.;
        repaired.favorite = false;
        assert_eq!(s.import_assets(vec![repaired]), (0, 1));
        assert_eq!(s.selected, id);
        assert_eq!(s.assets[id].duration, 10.);
        assert!(s.assets[id].favorite);
        for track in &mut s.tracks {
            track.locked = true;
        }
        s.apply(Action::AddToTimeline);
        assert_eq!(s.clips.len(), count + 1);
        assert!(!s.tracks[s.clips[s.selected_clip].track].locked);
    }
    #[test]
    fn filtering_combines_search_kind_and_favorites() {
        let mut s = EditorState::mock();
        s.apply(Action::Category("Audio".into()));
        assert_eq!(s.visible_assets().len(), 3);
        s.apply(Action::Search("voice".into()));
        assert_eq!(s.visible_assets(), vec![9]);
        s.apply(Action::ClearFilters);
        s.apply(Action::Select(0));
        s.apply(Action::Favorite);
        s.apply(Action::Category("Favorites".into()));
        assert_eq!(s.visible_assets(), vec![0]);
    }
    #[test]
    fn selections_and_transport_stay_in_bounds() {
        let mut s = EditorState::mock();
        s.apply(Action::SelectClip(1));
        assert_eq!(s.selected, 1);
        s.apply(Action::Seek(99.));
        assert_eq!(s.position, 18.);
        s.apply(Action::Step(-100.));
        assert_eq!(s.position, 0.);
        s.apply(Action::Select(2));
        assert_eq!(s.checked, BTreeSet::from([2]));
        s.apply(Action::MultiSelect(3));
        assert_eq!(s.checked.len(), 2);
        s.apply(Action::Select(1000));
        assert_eq!(s.selected, 3);
    }
    #[test]
    fn media_categories_reset_stale_filters_and_show_matching_preview() {
        let mut s = EditorState::mock();
        s.apply(Action::Search("skate".into()));
        s.apply(Action::ResolutionFilter);
        s.apply(Action::Category("Audio".into()));
        assert_eq!(s.assets[s.selected].kind, Kind::Audio);
        assert_eq!(s.inspector, 1);
        assert_eq!(s.visible_assets().len(), 3);
        s.apply(Action::Category("Images".into()));
        assert_eq!(s.assets[s.selected].kind, Kind::Image);
        assert_eq!(s.visible_assets().len(), 3);
        assert_eq!(s.assets[s.selected].duration_label(), "Still image");
        s.apply(Action::Select(9));
        assert_eq!(s.inspector, 1);
    }
    #[test]
    fn project_playhead_can_seek_past_the_selected_clip_and_audio_has_full_duration() {
        let mut s = EditorState::mock();
        assert_eq!(s.assets[s.selected].duration, 12.);
        s.apply(Action::Seek(17.));
        assert_eq!(s.position, 17.);
        s.apply(Action::Seek(-2.));
        assert_eq!(s.position, 0.);
        s.apply(Action::Screen(Screen::Library));
        s.apply(Action::Category("Audio".into()));
        s.apply(Action::Seek(200.));
        assert_eq!(s.position, 200.);
        s.apply(Action::Screen(Screen::Edit));
        assert_eq!(s.position, 18.);
    }
    #[test]
    fn cancel_and_timeline_controls_are_local() {
        let mut s = EditorState::mock();
        s.apply(Action::CancelAll);
        s.apply(Action::TrackLocked(0));
        assert!(s.tracks[0].locked);
        s.apply(Action::AddTrack);
        assert_eq!(s.tracks.len(), 5);
        s.apply(Action::Zoom(100.));
        assert_eq!(s.zoom, 8.);
        s.apply(Action::Zoom(-100.));
        assert_eq!(s.zoom, 0.25);
        s.apply(Action::Screen(Screen::Edit));
        assert!(s.inspector <= 1);
        assert!(s.screen.has_timeline());
    }
}
