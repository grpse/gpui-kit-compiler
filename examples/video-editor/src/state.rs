//! Deterministic UI fixtures and an in-memory action boundary. No media I/O.
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Edit,
    Tracking,
    Library,
    Overview,
}
impl Screen {
    pub const ALL: [Self; 4] = [Self::Library, Self::Overview, Self::Edit, Self::Tracking];
    pub fn label(self) -> &'static str {
        match self {
            Self::Edit => "Edit",
            Self::Tracking => "Tracking",
            Self::Library => "Media",
            Self::Overview => "Overview",
        }
    }
    pub fn has_timeline(self) -> bool {
        matches!(self, Self::Edit | Self::Tracking)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Video,
    Audio,
    Image,
}
#[derive(Clone, Debug)]
pub struct Asset {
    pub name: &'static str,
    pub kind: Kind,
    pub duration: f32,
    pub size: u32,
    pub added: &'static str,
    pub crop: [f32; 4],
    pub favorite: bool,
}
impl Asset {
    pub fn duration_label(&self) -> String {
        if self.kind == Kind::Image {
            return "Still image".into();
        }
        let s = self.duration as u32;
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}
/// Rectangles reference photographs inside the supplied mocks, without altering those files.
pub fn assets() -> Vec<Asset> {
    let values = [
        (
            "skate_01.mp4",
            Kind::Video,
            12.,
            214,
            "Oct 4, 2026",
            [606., 105., 569., 349.],
        ),
        (
            "mountain.mov",
            Kind::Video,
            28.,
            412,
            "Oct 4, 2026",
            [299., 173., 129., 62.],
        ),
        (
            "person.mov",
            Kind::Video,
            18.,
            180,
            "Oct 4, 2026",
            [443., 173., 129., 62.],
        ),
        (
            "ocean.mp4",
            Kind::Video,
            20.,
            293,
            "Oct 3, 2026",
            [153., 306., 130., 59.],
        ),
        (
            "forest.mov",
            Kind::Video,
            14.,
            320,
            "Oct 3, 2026",
            [299., 306., 129., 59.],
        ),
        (
            "city.mp4",
            Kind::Video,
            32.,
            450,
            "Oct 3, 2026",
            [443., 306., 129., 59.],
        ),
        (
            "interior.mov",
            Kind::Video,
            22.,
            310,
            "Oct 2, 2026",
            [443., 441., 129., 60.],
        ),
        (
            "road.mp4",
            Kind::Video,
            15.,
            210,
            "Oct 2, 2026",
            [299., 441., 129., 60.],
        ),
        ("music.wav", Kind::Audio, 225., 52, "Oct 4, 2026", [0.; 4]),
        (
            "voiceover.wav",
            Kind::Audio,
            138.,
            32,
            "Oct 4, 2026",
            [0.; 4],
        ),
        (
            "sfx_whoosh.wav",
            Kind::Audio,
            15.,
            4,
            "Oct 3, 2026",
            [0.; 4],
        ),
        (
            "plant.mov",
            Kind::Video,
            10.,
            120,
            "Oct 1, 2026",
            [153., 441., 130., 60.],
        ),
        (
            "landscape.jpg",
            Kind::Image,
            0.,
            8,
            "Oct 4, 2026",
            [299., 173., 129., 62.],
        ),
        (
            "botanical.png",
            Kind::Image,
            0.,
            6,
            "Oct 3, 2026",
            [153., 441., 130., 60.],
        ),
        (
            "city-cover.jpg",
            Kind::Image,
            0.,
            12,
            "Oct 3, 2026",
            [443., 306., 129., 59.],
        ),
    ];
    values
        .into_iter()
        .map(|(name, kind, duration, size, added, crop)| Asset {
            name,
            kind,
            duration,
            size,
            added,
            crop,
            favorite: false,
        })
        .collect()
}
#[derive(Clone, Debug)]
pub struct Job {
    pub asset: usize,
    pub progress: u8,
    pub cancelled: bool,
}
#[derive(Clone, Debug)]
pub struct Track {
    pub name: String,
    pub audio: bool,
    pub visible: bool,
    pub locked: bool,
}
#[derive(Clone, Debug)]
pub struct Clip {
    pub asset: usize,
    pub track: usize,
    pub start: f32,
    pub length: f32,
}
#[derive(Clone, Debug)]
pub enum Action {
    Screen(Screen),
    ResetWorkspace,
    Preset(&'static str),
    ClearPresetFilter,
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
    CancelJob(usize),
    CancelAll,
    Play,
    Seek(f32),
    Step(f32),
    Mute,
    Loop,
    Quality,
    Fullscreen,
    Inspector(usize),
    DetailTab(usize),
    TrackingMode(usize),
    Track(bool),
    TrackTarget,
    Attach,
    Toggle(&'static str),
    Section(&'static str),
    Control(usize, f32),
    ResetTransform,
    ResetCrop,
    Tool(&'static str),
    SelectClip(usize),
    TrackVisible(usize),
    TrackLocked(usize),
    AddTrack,
    Zoom(f32),
    Export,
    Share,
    Project,
    Dismiss,
    Mock(&'static str),
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
    pub looping: bool,
    pub quality: usize,
    pub fullscreen: bool,
    pub inspector: usize,
    pub detail_tab: usize,
    pub tracking_mode: usize,
    pub tracking_target: usize,
    pub attach: bool,
    pub toggles: BTreeSet<&'static str>,
    pub collapsed: BTreeSet<&'static str>,
    pub controls: [f32; 10],
    pub jobs: Vec<Job>,
    pub tracks: Vec<Track>,
    pub clips: Vec<Clip>,
    pub selected_clip: usize,
    pub zoom: f32,
    pub selected_preset: Option<&'static str>,
    pub notice: Option<String>,
    pub events: Vec<String>,
}
impl Default for EditorState {
    fn default() -> Self {
        Self {
            screen: Screen::Edit,
            assets: assets(),
            selected: 0,
            checked: BTreeSet::from([0]),
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
            position: 4.4,
            muted: false,
            looping: false,
            quality: 0,
            fullscreen: false,
            inspector: 0,
            detail_tab: 0,
            tracking_mode: 1,
            tracking_target: 0,
            attach: false,
            toggles: BTreeSet::from(["AI Tracking", "Show Track Path"]),
            collapsed: BTreeSet::new(),
            controls: [100., 0., 100., 0., 0., 0., 0., 50., 100., 0.],
            jobs: vec![
                Job {
                    asset: 0,
                    progress: 70,
                    cancelled: false,
                },
                Job {
                    asset: 1,
                    progress: 40,
                    cancelled: false,
                },
                Job {
                    asset: 2,
                    progress: 20,
                    cancelled: false,
                },
                Job {
                    asset: 3,
                    progress: 0,
                    cancelled: false,
                },
            ],
            tracks: [
                ("Video 1", false),
                ("Video 2", false),
                ("Music", true),
                ("Voiceover", true),
            ]
            .into_iter()
            .map(|(name, audio)| Track {
                name: name.into(),
                audio,
                visible: true,
                locked: false,
            })
            .collect(),
            clips: vec![
                Clip {
                    asset: 0,
                    track: 0,
                    start: 0.,
                    length: 7.5,
                },
                Clip {
                    asset: 1,
                    track: 0,
                    start: 7.5,
                    length: 4.,
                },
                Clip {
                    asset: 2,
                    track: 0,
                    start: 14.,
                    length: 3.2,
                },
                Clip {
                    asset: 4,
                    track: 1,
                    start: 7.5,
                    length: 6.2,
                },
                Clip {
                    asset: 8,
                    track: 2,
                    start: 0.,
                    length: 17.2,
                },
                Clip {
                    asset: 9,
                    track: 3,
                    start: 0.,
                    length: 16.9,
                },
            ],
            selected_clip: 0,
            zoom: 1.,
            selected_preset: None,
            notice: None,
            events: vec![],
        }
    }
}
impl EditorState {
    pub fn seek_limit(&self) -> f32 {
        if self.screen.has_timeline() {
            18.
        } else {
            self.assets[self.selected].duration.max(1.)
        }
    }
    pub fn visible_assets(&self) -> Vec<usize> {
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
                    && (!self.resolution_filter || a.kind == Kind::Video)
                    && (!self.duration_filter || a.duration < 20.)
                    && (!self.source_filter || a.name.ends_with(".mp4"))
                    && match self.folder.as_str() {
                        "Music" => a.kind == Kind::Audio,
                        "SFX" => a.name.starts_with("sfx"),
                        "House" => a.name == "interior.mov",
                        "Trash" => false,
                        _ => true,
                    }
            })
            .map(|(i, _)| i)
            .collect();
        if self.sort_ascending {
            ids.sort_by_key(|&i| self.assets[i].name);
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
            Action::ClearPresetFilter => self.query.clear(),
            Action::Preset(name) => {
                self.selected_preset = Some(name);
                self.notice = Some(format!("Mock {} preset: {name}", self.category));
            }
            Action::Screen(v) => {
                self.screen = v;
                self.fullscreen = false;
                self.inspector = if v == Screen::Tracking {
                    2
                } else if self.assets[self.selected].kind == Kind::Audio {
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
                } else if self.screen == Screen::Tracking {
                    2
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
                } else if self.screen == Screen::Tracking {
                    2
                } else {
                    0
                };
            }
            Action::Category(v) => {
                self.category = v;
                self.selected_preset = None;
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
                self.assets[self.selected].favorite = !self.assets[self.selected].favorite
            }
            Action::Import => {
                self.jobs = vec![
                    Job {
                        asset: self.selected,
                        progress: 60,
                        cancelled: false,
                    },
                    Job {
                        asset: 8,
                        progress: 100,
                        cancelled: false,
                    },
                    Job {
                        asset: 9,
                        progress: 25,
                        cancelled: false,
                    },
                ];
                self.notice = Some("Mock import queued 3 fixture files".into());
            }
            Action::CancelJob(i) => {
                if let Some(j) = self.jobs.get_mut(i) {
                    j.cancelled = true;
                }
            }
            Action::CancelAll => {
                for j in &mut self.jobs {
                    j.cancelled = true;
                }
            }
            Action::Play => {
                self.playing = !self.playing;
                self.notice = Some(
                    if self.playing {
                        "Mock playback started · preview is a still frame"
                    } else {
                        "Mock playback paused"
                    }
                    .into(),
                );
            }
            Action::Seek(v) => self.position = v.clamp(0., self.seek_limit()),
            Action::Step(v) => self.position = (self.position + v).clamp(0., self.seek_limit()),
            Action::Mute => self.muted = !self.muted,
            Action::Loop => self.looping = !self.looping,
            Action::Quality => self.quality = (self.quality + 1) % 3,
            Action::Fullscreen => self.fullscreen = !self.fullscreen,
            Action::Inspector(v) => self.inspector = v,
            Action::DetailTab(v) => self.detail_tab = v,
            Action::TrackingMode(v) => self.tracking_mode = v,
            Action::Track(forward) => {
                self.position = if forward { 8. } else { 1. };
                self.notice = Some("Mock tracking path applied to the preview".into());
            }
            Action::TrackTarget => self.tracking_target = (self.tracking_target + 1) % 3,
            Action::Attach => self.attach = !self.attach,
            Action::Toggle(v) => {
                if !self.toggles.remove(v) {
                    self.toggles.insert(v);
                }
            }
            Action::Section(v) => {
                if !self.collapsed.remove(v) {
                    self.collapsed.insert(v);
                }
            }
            Action::Control(i, v) if i < self.controls.len() => self.controls[i] = v,
            Action::ResetTransform => {
                self.controls[0] = 100.;
                self.controls[1] = 0.;
                self.controls[2] = 100.;
            }
            Action::ResetCrop => self.controls[3..7].fill(0.),
            Action::SelectClip(i) if i < self.clips.len() => {
                self.selected_clip = i;
                self.selected = self.clips[i].asset;
                self.inspector = if self.assets[self.selected].kind == Kind::Audio {
                    1
                } else if self.screen == Screen::Tracking {
                    2
                } else {
                    0
                };
                self.checked = BTreeSet::from([self.selected]);
            }
            Action::TrackVisible(i) => {
                if let Some(t) = self.tracks.get_mut(i) {
                    t.visible = !t.visible;
                }
            }
            Action::TrackLocked(i) => {
                if let Some(t) = self.tracks.get_mut(i) {
                    t.locked = !t.locked;
                }
            }
            Action::AddTrack => self.tracks.push(Track {
                name: format!(
                    "Video {}",
                    self.tracks.iter().filter(|t| !t.audio).count() + 1
                ),
                audio: false,
                visible: true,
                locked: false,
            }),
            Action::Zoom(v) => self.zoom = (self.zoom + v).clamp(0.25, 8.),
            Action::Dismiss => self.notice = None,
            Action::Export => {
                self.notice = Some("Mock export queued · My Project.mp4 · 4K / H.264".into())
            }
            Action::Share => {
                self.notice = Some("Mock share link copied · flowcut.example/my-project".into())
            }
            Action::Project => {
                self.notice =
                    Some("Mock project menu · My Project / Travel Film / New Project".into())
            }
            Action::Tool(v) | Action::Mock(v) => self.notice = Some(format!("Mock action: {v}")),
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filtering_combines_search_kind_and_favorites() {
        let mut s = EditorState::default();
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
        let mut s = EditorState::default();
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
        let mut s = EditorState::default();
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
        s.apply(Action::Category("Text".into()));
        s.apply(Action::Preset("Lower third"));
        assert_eq!(s.selected_preset, Some("Lower third"));
        s.apply(Action::Category("Effects".into()));
        assert!(s.selected_preset.is_none());
    }
    #[test]
    fn project_playhead_can_seek_past_the_selected_clip_and_audio_has_full_duration() {
        let mut s = EditorState::default();
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
    fn mock_import_cancel_and_timeline_controls_are_local() {
        let mut s = EditorState::default();
        s.apply(Action::Import);
        assert_eq!(s.jobs.len(), 3);
        s.apply(Action::CancelJob(0));
        assert!(s.jobs[0].cancelled);
        assert!(!s.jobs[1].cancelled);
        s.apply(Action::CancelAll);
        assert!(s.jobs.iter().all(|j| j.cancelled));
        s.apply(Action::TrackLocked(0));
        assert!(s.tracks[0].locked);
        s.apply(Action::AddTrack);
        assert_eq!(s.tracks.len(), 5);
        s.apply(Action::Zoom(100.));
        assert_eq!(s.zoom, 8.);
        s.apply(Action::Zoom(-100.));
        assert_eq!(s.zoom, 0.25);
        s.apply(Action::Screen(Screen::Tracking));
        assert_eq!(s.inspector, 2);
        assert!(s.screen.has_timeline());
    }
}
