use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};

pub const SCHEMA_VERSION: u32 = 2;
pub const MAX_HARMONICS: usize = 32;
pub type Id = u64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Harmonic {
    pub multiple: u8,
    pub amplitude: f32,
    pub phase: f32,
    pub detune: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoiseColor {
    White,
    Pink,
    Brown,
}
impl NoiseColor {
    pub fn label(self) -> &'static str {
        match self {
            Self::White => "White",
            Self::Pink => "Pink",
            Self::Brown => "Brown",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::White => Self::Pink,
            Self::Pink => Self::Brown,
            Self::Brown => Self::White,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Noise {
    pub level: f32,
    pub color: NoiseColor,
    #[serde(default)]
    pub seed: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub attack_ms: f32,
    pub decay_ms: f32,
    pub sustain: f32,
    pub release_ms: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sound {
    pub id: Id,
    pub name: String,
    pub harmonics: Vec<Harmonic>,
    pub noise: Noise,
    pub envelope: Envelope,
    pub brightness: f32,
    pub gain: f32,
    pub source: Option<Id>,
}
impl Sound {
    pub fn new(id: Id, name: &str) -> Self {
        Self {
            id,
            name: name.into(),
            harmonics: (1..=8)
                .map(|n| Harmonic {
                    multiple: n,
                    amplitude: if n == 1 { 0.8 } else { 0.4 / n as f32 },
                    phase: 0.,
                    detune: 0.,
                })
                .collect(),
            noise: Noise {
                level: 0.08,
                color: NoiseColor::White,
                seed: 0,
            },
            envelope: Envelope {
                attack_ms: 20.,
                decay_ms: 180.,
                sustain: 0.7,
                release_ms: 400.,
            },
            brightness: 0.5,
            gain: 0.8,
            source: None,
        }
    }
    pub fn resize_harmonics(&mut self, count: usize) {
        let count = count.clamp(1, MAX_HARMONICS);
        self.harmonics.truncate(count);
        while self.harmonics.len() < count {
            self.harmonics.push(Harmonic {
                multiple: (self.harmonics.len() + 1) as u8,
                amplitude: 0.,
                phase: 0.,
                detune: 0.,
            });
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoiceReference {
    pub id: Id,
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub analysis: AnalysisSettings,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnalysisSettings {
    pub max_harmonics: usize,
    pub fundamental_hz: Option<f64>,
    pub trim_start_seconds: f64,
    pub trim_end_seconds: Option<f64>,
}
impl Default for AnalysisSettings {
    fn default() -> Self {
        Self {
            max_harmonics: 32,
            fundamental_hz: None,
            trim_start_seconds: 0.,
            trim_end_seconds: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: Id,
    pub name: String,
    pub bpm: f64,
    pub follow_session: bool,
    pub gain: f32,
    pub pan: f32,
    pub muted: bool,
    pub solo: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub midi: u8,
    pub start_beat: f64,
    pub duration_beats: f64,
    pub velocity: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: Id,
    pub track: Id,
    pub name: String,
    pub start_seconds: f64,
    pub duration_beats: f64,
    pub sound: Sound,
    pub original_sound: Sound,
    pub library_sound: Option<Id>,
    pub notes: Vec<Note>,
    pub gain: f32,
    pub muted: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Module {
    Source,
    Library,
    Harmonics,
    Controls,
    Keyboard,
    Timeline,
    Inspector,
}
impl Module {
    pub const ALL: [Self; 7] = [
        Self::Source,
        Self::Library,
        Self::Harmonics,
        Self::Controls,
        Self::Keyboard,
        Self::Timeline,
        Self::Inspector,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "Voice reference",
            Self::Library => "Sound library",
            Self::Harmonics => "Harmonic builder",
            Self::Controls => "Tone & noise",
            Self::Keyboard => "Keyboard",
            Self::Timeline => "Timeline",
            Self::Inspector => "Sound instance",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dock {
    Left,
    Main,
    Right,
    Lower,
}
impl Dock {
    pub const ALL: [Self; 4] = [Self::Left, Self::Main, Self::Right, Self::Lower];
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Main => "Center",
            Self::Right => "Right",
            Self::Lower => "Lower",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Left => Self::Main,
            Self::Main => Self::Right,
            Self::Right => Self::Lower,
            Self::Lower => Self::Left,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub module: Module,
    pub dock: Dock,
    pub visible: bool,
    pub half: bool,
    #[serde(default = "default_share")]
    pub share: f32,
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_after: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair_gap: Option<f32>,
    #[serde(default = "default_zoom")]
    pub zoom: f32,
}
fn default_share() -> f32 {
    0.5
}
fn default_zoom() -> f32 {
    1.
}
fn default_gap() -> f32 {
    10.
}
fn default_dock_gap() -> f32 {
    8.
}
fn default_padding() -> f32 {
    12.
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: Id,
    pub name: String,
    pub panels: Vec<Placement>,
    pub left_width: f32,
    pub right_width: f32,
    pub lower_height: f32,
    #[serde(default = "default_gap")]
    pub panel_gap: f32,
    #[serde(default = "default_dock_gap")]
    pub dock_gap: f32,
    #[serde(default = "default_padding")]
    pub panel_padding: f32,
    #[serde(default = "default_zoom")]
    pub zoom: f32,
}
impl Profile {
    fn preset(id: Id, name: &str, index: usize) -> Self {
        let mut panels = Module::ALL
            .into_iter()
            .map(|module| {
                let (dock, visible) = match (index, module) {
                    (0, Module::Library) => (Dock::Left, true),
                    (0, Module::Source) => (Dock::Right, true),
                    (0, Module::Timeline | Module::Keyboard) => (Dock::Lower, true),
                    (0, Module::Inspector) => (Dock::Right, false),
                    (1, Module::Source) => (Dock::Left, true),
                    (1, Module::Library) => (Dock::Right, true),
                    (1, Module::Keyboard | Module::Timeline) => (Dock::Lower, true),
                    (1, Module::Inspector) => (Dock::Right, false),
                    (2, Module::Library) => (Dock::Left, true),
                    (2, Module::Inspector) => (Dock::Right, true),
                    (2, Module::Timeline | Module::Keyboard) => (Dock::Main, true),
                    (2, _) => (Dock::Main, false),
                    _ => (Dock::Main, true),
                };
                Placement {
                    module,
                    dock,
                    visible,
                    half: dock == Dock::Lower
                        || index < 2 && matches!(module, Module::Harmonics | Module::Controls),
                    share: 0.5,
                    height: None,
                    gap_after: None,
                    pair_gap: None,
                    zoom: 1.,
                }
            })
            .collect::<Vec<_>>();
        if index == 2 {
            panels.sort_by_key(|p| {
                if p.module == Module::Timeline {
                    0
                } else if p.module == Module::Keyboard {
                    1
                } else {
                    2
                }
            });
        }
        Self {
            id,
            name: name.into(),
            panels,
            left_width: 258.,
            right_width: 288.,
            lower_height: 350.,
            panel_gap: 10.,
            dock_gap: 8.,
            panel_padding: 12.,
            zoom: 1.,
        }
    }
    pub fn move_panel(&mut self, module: Module, dock: Dock, before: Option<Module>) {
        if before == Some(module) {
            return;
        }
        let Some(index) = self.panels.iter().position(|p| p.module == module) else {
            return;
        };
        let mut panel = self.panels.remove(index);
        panel.dock = dock;
        panel.visible = true;
        let index = before
            .and_then(|target| self.panels.iter().position(|p| p.module == target))
            .unwrap_or(self.panels.len());
        self.panels.insert(index, panel);
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    pub title: String,
    pub next_id: Id,
    pub session_bpm: f64,
    pub draft: Sound,
    pub library: Vec<Sound>,
    pub sources: Vec<VoiceReference>,
    pub tracks: Vec<Track>,
    pub clips: Vec<Clip>,
    pub profiles: Vec<Profile>,
    pub active_profile: Id,
    pub selected_clip: Option<Id>,
    pub target_track: Id,
    pub insert_seconds: f64,
    pub dark: bool,
    #[serde(default)]
    pub tuning: Tuning,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    pub reference_midi: u8,
    pub reference_hz: f64,
}
impl Default for Tuning {
    fn default() -> Self {
        Self {
            reference_midi: 69,
            reference_hz: 440.,
        }
    }
}
impl Default for Project {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            title: "Untitled session".into(),
            next_id: 10,
            session_bpm: 120.,
            draft: Sound::new(0, "New sound"),
            library: vec![],
            sources: vec![],
            tracks: vec![
                Track {
                    id: 1,
                    name: "Voice / Lead".into(),
                    bpm: 120.,
                    follow_session: true,
                    gain: 0.8,
                    pan: 0.,
                    muted: false,
                    solo: false,
                },
                Track {
                    id: 2,
                    name: "Texture".into(),
                    bpm: 90.,
                    follow_session: false,
                    gain: 0.7,
                    pan: 0.,
                    muted: false,
                    solo: false,
                },
            ],
            clips: vec![],
            profiles: vec![
                Profile::preset(3, "Studio Desk", 0),
                Profile::preset(4, "Voice Lab", 1),
                Profile::preset(5, "Clip Composer", 2),
            ],
            active_profile: 3,
            selected_clip: None,
            target_track: 1,
            insert_seconds: 0.,
            dark: true,
            tuning: Tuning::default(),
        }
    }
}
impl Project {
    pub fn allocate(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    pub fn profile(&self) -> &Profile {
        self.profiles
            .iter()
            .find(|p| p.id == self.active_profile)
            .expect("validated profile")
    }
    pub fn profile_mut(&mut self) -> &mut Profile {
        self.profiles
            .iter_mut()
            .find(|p| p.id == self.active_profile)
            .expect("validated profile")
    }
    pub fn selected(&self) -> Option<&Clip> {
        self.clips.iter().find(|c| Some(c.id) == self.selected_clip)
    }
    pub fn selected_mut(&mut self) -> Option<&mut Clip> {
        self.clips
            .iter_mut()
            .find(|c| Some(c.id) == self.selected_clip)
    }
    pub fn tempo(&self, track: Id) -> f64 {
        self.tracks
            .iter()
            .find(|t| t.id == track)
            .map(|t| {
                if t.follow_session {
                    self.session_bpm
                } else {
                    t.bpm
                }
            })
            .unwrap_or(self.session_bpm)
    }
    pub fn clip_seconds(&self, clip: &Clip) -> f64 {
        clip.duration_beats * 60. / self.tempo(clip.track)
    }
    pub fn save_sound(&mut self) -> Id {
        let id = self.allocate();
        let mut sound = self.draft.clone();
        sound.id = id;
        self.library.push(sound);
        id
    }
    pub fn add_clip(&mut self, sound: Sound, library: Option<Id>) -> Id {
        let id = self.allocate();
        self.clips.push(Clip {
            id,
            track: self.target_track,
            name: sound.name.clone(),
            start_seconds: self.insert_seconds,
            duration_beats: 4.,
            original_sound: sound.clone(),
            sound,
            library_sound: library,
            notes: vec![],
            gain: 1.,
            muted: false,
        });
        self.selected_clip = Some(id);
        id
    }
    pub fn add_note(&mut self, midi: u8) {
        if self.selected().is_none() {
            self.add_clip(self.draft.clone(), None);
        }
        if let Some(clip) = self.selected_mut() {
            let beat = clip
                .notes
                .last()
                .map(|n| n.start_beat + n.duration_beats)
                .unwrap_or(0.);
            clip.notes.push(Note {
                midi,
                start_beat: beat,
                duration_beats: 1.,
                velocity: 0.8,
            });
            clip.duration_beats = clip.duration_beats.max(beat + 1.);
        }
    }
    pub fn add_profile(&mut self, name: String, empty: bool) {
        let mut profile = self.profile().clone();
        profile.id = self.allocate();
        profile.name = name;
        if empty {
            for panel in &mut profile.panels {
                panel.visible = false;
            }
        }
        self.active_profile = profile.id;
        self.profiles.push(profile);
    }
    pub fn add_track(&mut self) {
        let id = self.allocate();
        self.tracks.push(Track {
            id,
            name: format!("Track {}", self.tracks.len() + 1),
            bpm: self.session_bpm,
            follow_session: true,
            gain: 0.8,
            pan: 0.,
            muted: false,
            solo: false,
        });
        self.target_track = id;
    }
    pub fn validate(&self) -> Result<(), String> {
        fn check(ok: bool, error: &str) -> Result<(), String> {
            if ok { Ok(()) } else { Err(error.into()) }
        }
        fn range(v: f64, lo: f64, hi: f64) -> bool {
            v.is_finite() && (lo..=hi).contains(&v)
        }
        check(
            self.schema_version == SCHEMA_VERSION,
            "Unsupported project version",
        )?;
        check(
            !self.title.trim().is_empty() && self.title.len() <= 512,
            "Invalid project title",
        )?;
        check(
            range(self.session_bpm, 20., 300.) && range(self.insert_seconds, 0., 86400.),
            "Invalid session tempo or insertion time",
        )?;
        check(
            !self.tracks.is_empty() && !self.profiles.is_empty(),
            "Project needs a track and workspace profile",
        )?;
        check(
            self.clips.len() <= 100_000
                && self.library.len() <= 10_000
                && self.sources.len() <= 10_000
                && self.tracks.len() <= 1024
                && self.profiles.len() <= 256,
            "Project exceeds UI limits",
        )?;
        let mut ids = HashSet::new();
        for id in self
            .library
            .iter()
            .map(|v| v.id)
            .chain(self.sources.iter().map(|v| v.id))
            .chain(self.tracks.iter().map(|v| v.id))
            .chain(self.clips.iter().map(|v| v.id))
            .chain(self.profiles.iter().map(|v| v.id))
        {
            check(
                id > 0 && ids.insert(id) && id < self.next_id && self.next_id < u64::MAX,
                "Invalid or duplicate object ID",
            )?;
        }
        let sound_valid = |s: &Sound| -> bool {
            !s.name.trim().is_empty()
                && s.name.len() <= 512
                && (1..=MAX_HARMONICS).contains(&s.harmonics.len())
                && s.harmonics.iter().enumerate().all(|(i, h)| {
                    h.multiple as usize == i + 1
                        && range(h.amplitude as f64, 0., 1.)
                        && range(h.phase as f64, -180., 180.)
                        && range(h.detune as f64, -100., 100.)
                })
                && range(s.noise.level as f64, 0., 1.)
                && range(s.brightness as f64, 0., 1.)
                && range(s.gain as f64, 0., 1.)
                && range(s.envelope.attack_ms as f64, 0., 10000.)
                && range(s.envelope.decay_ms as f64, 0., 10000.)
                && range(s.envelope.sustain as f64, 0., 1.)
                && range(s.envelope.release_ms as f64, 0., 10000.)
                && s.source
                    .is_none_or(|id| self.sources.iter().any(|v| v.id == id))
        };
        check(
            sound_valid(&self.draft) && self.library.iter().all(sound_valid),
            "Invalid sound parameters or voice reference",
        )?;
        for source in &self.sources {
            check(
                !source.name.is_empty()
                    && !source.path.as_os_str().is_empty()
                    && source.sha256.len() == 64
                    && source.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid source reference",
            )?;
            let a = &source.analysis;
            check(
                (1..=MAX_HARMONICS).contains(&a.max_harmonics)
                    && range(a.trim_start_seconds, 0., 86400.)
                    && a.trim_end_seconds
                        .is_none_or(|v| range(v, a.trim_start_seconds, 86400.))
                    && a.fundamental_hz.is_none_or(|v| range(v, 20., 20000.)),
                "Invalid analysis settings",
            )?;
        }
        for track in &self.tracks {
            check(
                !track.name.trim().is_empty()
                    && range(track.bpm, 20., 300.)
                    && range(track.gain as f64, 0., 1.)
                    && range(track.pan as f64, -1., 1.),
                "Invalid track settings",
            )?;
        }
        for clip in &self.clips {
            check(
                self.tracks.iter().any(|t| t.id == clip.track)
                    && !clip.name.trim().is_empty()
                    && sound_valid(&clip.sound)
                    && sound_valid(&clip.original_sound)
                    && clip
                        .library_sound
                        .is_none_or(|id| self.library.iter().any(|s| s.id == id)),
                "Invalid clip reference or sound",
            )?;
            check(
                range(clip.start_seconds, 0., 86400.)
                    && range(clip.duration_beats, 0.25, 100_000.)
                    && range(clip.gain as f64, 0., 1.)
                    && clip.notes.len() <= 100_000,
                "Invalid clip timing or gain",
            )?;
            check(
                clip.notes.iter().all(|n| {
                    n.midi <= 127
                        && range(n.start_beat, 0., clip.duration_beats)
                        && range(n.duration_beats, 0.01, clip.duration_beats)
                        && n.start_beat + n.duration_beats <= clip.duration_beats
                        && range(n.velocity as f64, 0., 1.)
                }),
                "Invalid note event",
            )?;
        }
        for profile in &self.profiles {
            let kinds: HashSet<_> = profile.panels.iter().map(|p| p.module).collect();
            check(
                !profile.name.trim().is_empty()
                    && kinds.len() == 7
                    && profile.panels.len() == 7
                    && range(profile.left_width as f64, 200., 420.)
                    && range(profile.right_width as f64, 200., 420.)
                    && range(profile.lower_height as f64, 180., 700.)
                    && range(profile.panel_gap as f64, 4., 40.)
                    && range(profile.dock_gap as f64, 6., 32.)
                    && range(profile.panel_padding as f64, 4., 32.)
                    && range(profile.zoom as f64, 0.5, 2.)
                    && profile.panels.iter().all(|p| {
                        range(p.share as f64, 0.15, 0.85)
                            && p.height.is_none_or(|v| range(v as f64, 100., 1800.))
                            && p.gap_after.is_none_or(|v| range(v as f64, 4., 40.))
                            && p.pair_gap.is_none_or(|v| range(v as f64, 4., 40.))
                            && range(p.zoom as f64, 0.5, 2.)
                    }),
                "Invalid workspace profile",
            )?;
        }
        check(
            self.tuning.reference_midi <= 127 && range(self.tuning.reference_hz, 20., 20000.),
            "Invalid tuning reference",
        )?;
        check(
            self.profiles.iter().any(|p| p.id == self.active_profile)
                && self.tracks.iter().any(|t| t.id == self.target_track)
                && self
                    .selected_clip
                    .is_none_or(|id| self.clips.iter().any(|c| c.id == id)),
            "Invalid active selection",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clips_are_independent_snapshots() {
        let mut p = Project::default();
        let library = p.save_sound();
        p.add_clip(p.library[0].clone(), Some(library));
        p.add_clip(p.library[0].clone(), Some(library));
        p.selected_mut().unwrap().sound.noise.level = 0.91;
        p.selected_mut().unwrap().sound.harmonics[0].amplitude = 0.1;
        assert_eq!(p.library[0].noise.level, 0.08);
        assert_eq!(p.clips[0].sound.noise.level, 0.08);
        assert_eq!(p.clips[1].original_sound.harmonics[0].amplitude, 0.8);
        p.validate().unwrap();
    }
    #[test]
    fn harmonic_limit_and_expansion_preserve_parameters() {
        let mut s = Sound::new(0, "Test");
        s.resize_harmonics(1000);
        assert_eq!(s.harmonics.len(), 32);
        assert_eq!(s.harmonics[0].amplitude, 0.8);
        assert_eq!(s.harmonics[31].amplitude, 0.);
        s.resize_harmonics(0);
        assert_eq!(s.harmonics.len(), 1);
    }
    #[test]
    fn tempos_change_lengths_without_moving_clip_starts() {
        let mut p = Project::default();
        p.insert_seconds = 3.;
        p.add_clip(p.draft.clone(), None);
        assert_eq!(p.clip_seconds(&p.clips[0]), 2.);
        p.tracks[0].follow_session = false;
        p.tracks[0].bpm = 60.;
        assert_eq!(p.clip_seconds(&p.clips[0]), 4.);
        assert_eq!(p.clips[0].start_seconds, 3.);
    }
    #[test]
    fn profiles_and_note_sequences_are_connected_and_independent() {
        let mut p = Project::default();
        p.add_profile("Mixing".into(), false);
        p.profile_mut()
            .move_panel(Module::Library, Dock::Right, None);
        assert_eq!(
            p.profiles[0]
                .panels
                .iter()
                .find(|v| v.module == Module::Library)
                .unwrap()
                .dock,
            Dock::Left
        );
        for _ in 0..6 {
            p.add_note(60);
        }
        assert_eq!(p.clips[0].duration_beats, 6.);
        assert_eq!(p.clips[0].notes[5].start_beat, 5.);
        p.validate().unwrap();
    }
    #[test]
    fn validation_rejects_broken_references_and_nan() {
        let mut p = Project::default();
        p.validate().unwrap();
        p.add_clip(p.draft.clone(), None);
        p.clips[0].track = 999;
        assert!(p.validate().is_err());
        p.clips[0].track = 1;
        p.draft.noise.level = f32::NAN;
        assert!(p.validate().is_err());
    }
}
