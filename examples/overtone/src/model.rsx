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
    /// Play the stored recording bounds directly; older harmonic sounds keep synthesis.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub recorded_sample: bool,
    pub id: Id,
    pub name: String,
    pub harmonics: Vec<Harmonic>,
    pub noise: Noise,
    pub envelope: Envelope,
    pub brightness: f32,
    pub gain: f32,
    pub source: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<SoundCapture>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoundCapture {
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub fundamental_hz: Option<f64>,
    pub confidence: f32,
}
impl Sound {
    pub fn new(id: Id, name: &str) -> Self {
        Self {
            recorded_sample: false,
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
            capture: None,
        }
    }
    pub fn resize_harmonics(&mut self, count: usize) {
        let count = count.clamp(1, MAX_HARMONICS);
        self.harmonics.truncate(count);
        while self.harmonics.len() < count {
            self.harmonics.push(Harmonic {
                multiple: (1..=MAX_HARMONICS as u8).find(|n|!self.harmonics.iter().any(|h|h.multiple==*n)).expect("available partial"),
                amplitude: 0.,
                phase: 0.,
                detune: 0.,
            });
        }
        self.harmonics.sort_by_key(|h|h.multiple);
    }
    pub fn weak_harmonics(&self, threshold:f32)->Vec<u8> {
        let Some(strongest)=self.harmonics.iter().reduce(|a,b|if b.amplitude>a.amplitude{b}else{a}) else {return vec![];};
        let cutoff=strongest.amplitude*threshold.clamp(0.,0.5);
        self.harmonics.iter().filter(|h|h.multiple!=strongest.multiple && h.amplitude<=cutoff).map(|h|h.multiple).collect()
    }
    /// One undoable edit: preserve the strongest partial and leave noise removal explicit.
    pub fn clean_harmonics(&mut self,threshold:f32,remove_noise:bool)->(usize,bool) {
        let weak=self.weak_harmonics(threshold);
        self.harmonics.retain(|h|!weak.contains(&h.multiple));
        let noise_removed=remove_noise && self.noise.level>0.;if remove_noise{self.noise.level=0.;}
        (weak.len(),noise_removed)
    }
    /// Preserve harmonic frequencies when a partial is removed; keep one editable slot.
    pub fn remove_harmonic(&mut self,index:usize)->bool {
        if self.harmonics.len()>1 && index<self.harmonics.len() {self.harmonics.remove(index);true}else{false}
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
    #[serde(default = "default_silence")]
    pub silence_db: f32,
    #[serde(default = "default_gap_ms")]
    pub split_gap_ms: u32,
    #[serde(default = "default_min_ms")]
    pub min_sound_ms: u32,
}
fn default_silence() -> f32 {
    -40.
}
fn default_gap_ms() -> u32 {
    150
}
fn default_min_ms() -> u32 {
    100
}
impl Default for AnalysisSettings {
    fn default() -> Self {
        Self {
            max_harmonics: 32,
            fundamental_hz: None,
            trim_start_seconds: 0.,
            trim_end_seconds: None,
            silence_db: default_silence(),
            split_gap_ms: default_gap_ms(),
            min_sound_ms: default_min_ms(),
        }
    }
}
impl AnalysisSettings {
    pub fn valid(&self) -> bool {
        (1..=MAX_HARMONICS).contains(&self.max_harmonics)
            && self.trim_start_seconds.is_finite()
            && (0.0..=86400.0).contains(&self.trim_start_seconds)
            && self
                .trim_end_seconds
                .is_none_or(|v| v.is_finite() && (self.trim_start_seconds..=86400.0).contains(&v))
            && self
                .fundamental_hz
                .is_none_or(|v| v.is_finite() && (20.0..=20000.0).contains(&v))
            && self.silence_db.is_finite()
            && (-80.0..=-10.0).contains(&self.silence_db)
            && (50..=1000).contains(&self.split_gap_ms)
            && (50..=2000).contains(&self.min_sound_ms)
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
    Reconstruction,
}
impl Module {
    pub const ALL: [Self; 8] = [
        Self::Source,
        Self::Library,
        Self::Harmonics,
        Self::Controls,
        Self::Keyboard,
        Self::Timeline,
        Self::Inspector,
        Self::Reconstruction,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "Record & split",
            Self::Library => "Sounds",
            Self::Harmonics => "Harmonic builder",
            Self::Controls => "Tone & noise",
            Self::Keyboard => "Keyboard",
            Self::Timeline => "Timeline",
            Self::Inspector => "Selected clip",
            Self::Reconstruction => "Waveform reconstruction",
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
        let _ = index;
        let panels = Module::ALL.into_iter().map(|module| {
            let (dock, visible) = match module {
                Module::Source => (Dock::Main, true),
                Module::Library => (Dock::Left, true),
                Module::Timeline => (Dock::Main, true),
                Module::Inspector => (Dock::Right, true),
                _ => (Dock::Main, false),
            };
            Placement { module, dock, visible, half:false, share:0.5, height:None,
                gap_after:None, pair_gap:None, zoom:1. }
        }).collect();
        Self {
            id,
            name: name.into(),
            panels,
            left_width: 260.,
            right_width: 260.,
            lower_height: 350.,
            panel_gap: 10.,
            dock_gap: 8.,
            panel_padding: 12.,
            zoom: 1.,
        }
    }
    /// Existing seven-panel projects gain the new component hidden, preserving their layout.
    pub fn upgrade_reconstruction_panel(&mut self) {
        if !self.panels.iter().any(|p|p.module==Module::Reconstruction) {
            self.panels.push(Placement {module:Module::Reconstruction,dock:Dock::Main,visible:false,half:false,share:0.5,height:Some(560.),gap_after:None,pair_gap:None,zoom:1.});
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
pub struct TimelineSettings {
    pub zoom: f32,
    /// Zero disables snapping; otherwise subdivisions in beats of the destination track.
    pub snap_beats: f64,
}
impl Default for TimelineSettings {
    fn default() -> Self { Self { zoom: 1., snap_beats: 0.25 } }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReconstructionSettings {
    pub weak_threshold: f32,
    pub remove_noise: bool,
    pub cycles: f32,
}
impl Default for ReconstructionSettings {
    fn default()->Self {Self {weak_threshold:0.05,remove_noise:false,cycles:3.}}
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub reconstruction: ReconstructionSettings,
    #[serde(default)]
    pub editing_sound: Option<Id>,
    #[serde(default)]
    pub timeline: TimelineSettings,
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
    #[serde(default)]
    pub recording: AnalysisSettings,
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
            reconstruction: ReconstructionSettings::default(),
            editing_sound: None, timeline: TimelineSettings::default(),
            title: "Untitled session".into(),
            next_id: 10,
            session_bpm: 120.,
            draft: Sound::new(0, "New sound"),
            library: vec![],
            sources: vec![],
            tracks: vec![
                Track {
                    id: 1,
                    name: "Track 1".into(),
                    bpm: 120.,
                    follow_session: true,
                    gain: 0.8,
                    pan: 0.,
                    muted: false,
                    solo: false,
                },
                Track {
                    id: 2,
                    name: "Track 2".into(),
                    bpm: 120.,
                    follow_session: true,
                    gain: 0.7,
                    pan: 0.,
                    muted: false,
                    solo: false,
                },
            ],
            clips: vec![],
            profiles: vec![
                Profile::preset(3, "Recording desk", 0),
            ],
            active_profile: 3,
            selected_clip: None,
            target_track: 1,
            insert_seconds: 0.,
            dark: true,
            tuning: Tuning::default(),
            recording: AnalysisSettings::default(),
        }
    }
}
impl Project {
    pub fn upgrade_workspace(&mut self){for profile in &mut self.profiles{profile.upgrade_reconstruction_panel();}}
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
        if clip.sound.recorded_sample {
            clip.sound.capture.as_ref().map(|c| c.duration_seconds).unwrap_or(0.)
        } else { clip.duration_beats * 60. / self.tempo(clip.track) }
    }
    pub fn save_sound(&mut self) -> Id {
        if let Some(id) = self.editing_sound {
            if let Some(record) = self.library.iter_mut().find(|s| s.id == id) {
                *record = self.draft.clone(); record.id = id; return id;
            }
        }
        let id = self.allocate();
        let mut sound = self.draft.clone();
        sound.id = id;
        self.library.push(sound);
        id
    }
    pub fn remove_sound(&mut self, id: Id) {
        self.library.retain(|s| s.id != id);
        for clip in &mut self.clips { if clip.library_sound == Some(id) { clip.library_sound = None; } }
        if self.editing_sound == Some(id) { self.editing_sound = None; }
    }
    pub fn snap_seconds(&self, seconds: f64, track: Id) -> f64 {
        let step = self.timeline.snap_beats * 60. / self.tempo(track);
        if step > 0. { ((seconds / step).round() * step).clamp(0., 86400.) } else { seconds.clamp(0., 86400.) }
    }
    pub fn place_clip(&mut self, id: Id, track: Id, seconds: f64) {
        let start = self.snap_seconds(seconds, track);
        if let Some(c) = self.clips.iter_mut().find(|c| c.id == id) { c.track = track; c.start_seconds = start; self.selected_clip = Some(id); }
    }
    pub fn add_clip(&mut self, sound: Sound, library: Option<Id>) -> Id {
        let id = self.allocate();
        self.clips.push(Clip {
            id,
            track: self.target_track,
            name: sound.name.clone(),
            start_seconds: self.snap_seconds(self.insert_seconds, self.target_track),
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
    pub fn place_library_sound(&mut self, library:Id,track:Id,seconds:f64)->Option<Id> {
        let sound=self.library.iter().find(|s|s.id==library)?.clone();
        if !self.tracks.iter().any(|t|t.id==track) {return None;}
        self.target_track=track;self.insert_seconds=self.snap_seconds(seconds,track);
        Some(self.add_library_clip(sound,library))
    }
    /// Library placement is immediately audible; keyboard-created clips still start empty.
    pub fn add_library_clip(&mut self, sound: Sound, library: Id) -> Id {
        let tempo = self.tempo(self.target_track);
        let seconds = sound.capture.as_ref().map(|c| c.duration_seconds).unwrap_or(2.);
        let hz = sound.capture.as_ref().and_then(|c| c.fundamental_hz);
        let midi = hz.map(|hz| (self.tuning.reference_midi as f64 + 12.*(hz/self.tuning.reference_hz).log2()).round().clamp(0.,127.) as u8).unwrap_or(60);
        let beats = (seconds*tempo/60.).max(0.25);
        let id = self.add_clip(sound, Some(library));
        if let Some(c) = self.selected_mut() {
            c.duration_beats = beats;
            if !c.sound.recorded_sample { c.notes.push(Note { midi, start_beat: 0., duration_beats: beats, velocity: 0.8 }); }
        }
        id
    }
    /// Add separate recorded sounds and playable clips in the take's original order.
    pub fn add_recorded_sounds(&mut self, source:Id, name:&str, sounds:Vec<Sound>) -> Vec<Id> {
        let insertion=self.insert_seconds;
        let first_start=sounds.first().and_then(|s|s.capture.as_ref()).map(|c|c.start_seconds).unwrap_or(0.);
        let mut ids=Vec::new();
        for (i,mut sound) in sounds.into_iter().enumerate() {
            sound.id=self.allocate();sound.source=Some(source);
            sound.name=format!("{} · {}",name.chars().take(180).collect::<String>(),i+1);
            let offset=sound.capture.as_ref().map(|c|c.start_seconds-first_start).unwrap_or(0.);
            let sound_id=sound.id;
            self.library.push(sound.clone());
            let clip_id=self.add_library_clip(sound,sound_id);
            if let Some(clip)=self.selected_mut(){clip.start_seconds=(insertion+offset).clamp(0.,86400.);}
            ids.push(clip_id);
        }
        ids
    }
    /// Non-destructive cut of a recorded clip at an absolute timeline position.
    pub fn split_selected_recording(&mut self, seconds:f64)->Result<(),String> {
        let mut right=self.selected().cloned().ok_or("Select a recorded clip first")?;
        if !right.sound.recorded_sample {return Err("Select a recorded clip to split".into());}
        let capture=right.sound.capture.clone().ok_or("Clip has no recording bounds")?;
        let offset=seconds-right.start_seconds;
        if !offset.is_finite() || offset<0.05 || offset>capture.duration_seconds-0.05 {
            return Err("Set Insert inside the selected clip, at least 50 ms from each edge".into());
        }
        let tempo=self.tempo(right.track);
        if let Some(left)=self.selected_mut() {
            left.sound.capture.as_mut().unwrap().duration_seconds=offset;
            left.original_sound=left.sound.clone();left.library_sound=None;
            left.duration_beats=(offset*tempo/60.).max(0.25);
        }
        right.id=self.allocate();right.start_seconds=seconds;right.library_sound=None;
        let bounds=right.sound.capture.as_mut().unwrap();bounds.start_seconds+=offset;bounds.duration_seconds-=offset;
        right.duration_beats=(bounds.duration_seconds*tempo/60.).max(0.25);
        right.original_sound=right.sound.clone();self.selected_clip=Some(right.id);self.clips.push(right);
        Ok(())
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
        check(self.recording.valid(), "Invalid recording settings")?;
        check(range(self.reconstruction.weak_threshold as f64,0.,0.5) && range(self.reconstruction.cycles as f64,1.,16.),"Invalid reconstruction settings")?;
        check(range(self.timeline.zoom as f64, 0.25, 8.) && [0., 0.25, 0.5, 1., 4.].contains(&self.timeline.snap_beats), "Invalid timeline view settings")?;
        check(self.editing_sound.is_none_or(|id| self.library.iter().any(|s| s.id == id)), "Missing library edit record")?;
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
                    (1..=MAX_HARMONICS as u8).contains(&h.multiple) && (i==0 || s.harmonics[i-1].multiple<h.multiple)
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
                && (!s.recorded_sample || s.source.is_some() && s.capture.is_some())
                && s.capture.as_ref().is_none_or(|c| {
                    s.source.is_some()
                        && range(c.start_seconds, 0., 86400.)
                        && range(c.duration_seconds, 0.001, 120.)
                        && c.fundamental_hz.is_none_or(|v| range(v, 20., 20000.))
                        && range(c.confidence as f64, 0., 1.)
                })
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
                    && a.fundamental_hz.is_none_or(|v| range(v, 20., 20000.))
                    && range(a.silence_db as f64, -80., -10.)
                    && (50..=1000).contains(&a.split_gap_ms)
                    && (50..=2000).contains(&a.min_sound_ms),
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
                    && kinds.len() == Module::ALL.len()
                    && profile.panels.len() == Module::ALL.len()
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
