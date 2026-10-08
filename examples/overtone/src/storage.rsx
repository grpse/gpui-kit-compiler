use crate::model::*;
use serde::{
    Deserialize, Serialize,
    de::{Error as _, SeqAccess, Visitor},
    ser::SerializeSeq,
};
use std::{
    collections::{HashMap, HashSet},
    fmt,
};

pub const SYNTHESIS_MODEL: &str = "additive-sine-v1";

/// A partial is [multiple, linear amplitude, phase degrees?, detune cents?].
/// Zero trailing values are omitted, with no rounding or quantization.
#[derive(Clone, Debug, PartialEq)]
struct Partial(Harmonic);
impl Serialize for Partial {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let h = &self.0;
        let len = if h.detune != 0. {
            4
        } else if h.phase != 0. {
            3
        } else {
            2
        };
        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&h.multiple)?;
        seq.serialize_element(&h.amplitude)?;
        if len >= 3 {
            seq.serialize_element(&h.phase)?;
        }
        if len == 4 {
            seq.serialize_element(&h.detune)?;
        }
        seq.end()
    }
}
impl<'de> Deserialize<'de> for Partial {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PartialVisitor;
        impl<'de> Visitor<'de> for PartialVisitor {
            type Value = Partial;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("[multiple, amplitude, phase?, detune?]")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Partial, A::Error> {
                let multiple = seq
                    .next_element()?
                    .ok_or_else(|| A::Error::custom("Missing partial multiple"))?;
                let amplitude = seq
                    .next_element()?
                    .ok_or_else(|| A::Error::custom("Missing partial amplitude"))?;
                let phase = seq.next_element()?.unwrap_or(0.);
                let detune = seq.next_element()?.unwrap_or(0.);
                if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(A::Error::custom("Too many partial values"));
                }
                Ok(Partial(Harmonic {
                    multiple,
                    amplitude,
                    phase,
                    detune,
                }))
            }
        }
        deserializer.deserialize_seq(PartialVisitor)
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Patch {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    recorded_sample: bool,
    partials: Vec<Partial>,
    noise: Noise,
    envelope: Envelope,
    brightness: f32,
    gain: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capture: Option<SoundCapture>,
}
impl Patch {
    fn from_sound(s: &Sound) -> Self {
        Self {
            recorded_sample:s.recorded_sample,
            partials: s.harmonics.iter().cloned().map(Partial).collect(),
            noise: s.noise.clone(),
            envelope: s.envelope.clone(),
            brightness: s.brightness,
            gain: s.gain,
            source: s.source,
            capture: s.capture.clone(),
        }
    }
    fn sound(&self, id: Id, name: String) -> Sound {
        Sound {
            recorded_sample:self.recorded_sample,
            id,
            name,
            harmonics: self.partials.iter().map(|h| h.0.clone()).collect(),
            noise: self.noise.clone(),
            envelope: self.envelope.clone(),
            brightness: self.brightness,
            gain: self.gain,
            source: self.source,
            capture: self.capture.clone(),
        }
    }
}
#[derive(Serialize, Deserialize)]
struct SoundRef {
    id: Id,
    name: String,
    patch: usize,
}
#[derive(Serialize, Deserialize)]
struct StoredClip {
    id: Id,
    track: Id,
    name: String,
    start_seconds: f64,
    duration_beats: f64,
    sound: SoundRef,
    original_sound: SoundRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    library_sound: Option<Id>,
    notes: Vec<(u8, f64, f64, f32)>,
    gain: f32,
    muted: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct WorkspaceSettings {
    #[serde(default)]
    pub reconstruction: ReconstructionSettings,
    pub schema_version: u32,
    #[serde(default)]
    pub timeline: TimelineSettings,
    pub profiles: Vec<Profile>,
    pub active_profile: Id,
    pub dark: bool,
    #[serde(default)]
    pub recording: AnalysisSettings,
}
impl WorkspaceSettings {
    pub fn from_project(p: &Project) -> Self {
        Self {
            schema_version: 1,
            reconstruction: p.reconstruction.clone(),
            timeline: p.timeline.clone(),
            profiles: p.profiles.clone(),
            active_profile: p.active_profile,
            dark: p.dark,
            recording: p.recording.clone(),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Unsupported settings version".into());
        }
        let mut probe = Project::default();
        let mut ids = HashSet::new();
        if self.profiles.is_empty()
            || self.profiles.len() > 256
            || !self.profiles.iter().any(|p| p.id == self.active_profile)
        {
            return Err("Invalid settings profiles".into());
        }
        for p in &self.profiles {
            if !ids.insert(p.id) {
                return Err("Duplicate profile ID".into());
            }
        }
        probe.reconstruction = self.reconstruction.clone();
        probe.timeline = self.timeline.clone();
        probe.profiles.clear();
        probe.recording = self.recording.clone();
        for p in &self.profiles {
            let mut p = p.clone();
            let selected = p.id == self.active_profile;
            p.id = probe.allocate();
            if selected {
                probe.active_profile = p.id;
            }
            probe.profiles.push(p);
        }
        probe.upgrade_workspace();
        probe.validate()
    }
    pub fn apply(&self, p: &mut Project) -> Result<(), String> {
        self.validate()?;
        let mut profiles = vec![];
        let mut active = p.active_profile;
        for profile in &self.profiles {
            let mut next = profile.clone();
            next.upgrade_reconstruction_panel();
            next.id = p.allocate();
            if profile.id == self.active_profile {
                active = next.id;
            }
            profiles.push(next);
        }
        p.reconstruction = self.reconstruction.clone();
        p.timeline = self.timeline.clone();
        p.profiles = profiles;
        p.active_profile = active;
        p.dark = self.dark;
        p.recording = self.recording.clone();
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
struct Workspace {
    #[serde(default)]
    reconstruction: ReconstructionSettings,
    #[serde(default)]
    editing_sound: Option<Id>,
    #[serde(default)]
    timeline: TimelineSettings,
    profiles: Vec<Profile>,
    active_profile: Id,
    dark: bool,
    selected_clip: Option<Id>,
    target_track: Id,
    insert_seconds: f64,
}
#[derive(Serialize, Deserialize)]
struct Document {
    format: String,
    schema_version: u32,
    synthesis_model: String,
    title: String,
    next_id: Id,
    session_bpm: f64,
    tuning: Tuning,
    #[serde(default)]
    recording: AnalysisSettings,
    patches: Vec<Patch>,
    draft: SoundRef,
    library: Vec<SoundRef>,
    sources: Vec<VoiceReference>,
    tracks: Vec<Track>,
    clips: Vec<StoredClip>,
    workspace: Workspace,
}
fn intern(
    s: &Sound,
    patches: &mut Vec<Patch>,
    index: &mut HashMap<String, usize>,
) -> Result<SoundRef, String> {
    let patch = Patch::from_sound(s);
    let key = serde_json::to_string(&patch).map_err(|e| e.to_string())?;
    let id = if let Some(id) = index.get(&key) {
        *id
    } else {
        let id = patches.len();
        index.insert(key, id);
        patches.push(patch);
        id
    };
    Ok(SoundRef {
        id: s.id,
        name: s.name.clone(),
        patch: id,
    })
}
pub fn encode(project: &Project) -> Result<Vec<u8>, String> {
    project.validate()?;
    let mut patches = vec![];
    let mut index = HashMap::new();
    let draft = intern(&project.draft, &mut patches, &mut index)?;
    let library = project
        .library
        .iter()
        .map(|s| intern(s, &mut patches, &mut index))
        .collect::<Result<Vec<_>, _>>()?;
    let clips = project
        .clips
        .iter()
        .map(|c| {
            Ok(StoredClip {
                id: c.id,
                track: c.track,
                name: c.name.clone(),
                start_seconds: c.start_seconds,
                duration_beats: c.duration_beats,
                sound: intern(&c.sound, &mut patches, &mut index)?,
                original_sound: intern(&c.original_sound, &mut patches, &mut index)?,
                library_sound: c.library_sound,
                notes: c
                    .notes
                    .iter()
                    .map(|n| (n.midi, n.start_beat, n.duration_beats, n.velocity))
                    .collect(),
                gain: c.gain,
                muted: c.muted,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let document = Document {
        format: "overtone".into(),
        schema_version: SCHEMA_VERSION,
        synthesis_model: SYNTHESIS_MODEL.into(),
        title: project.title.clone(),
        next_id: project.next_id,
        session_bpm: project.session_bpm,
        tuning: project.tuning.clone(),
        recording: project.recording.clone(),
        patches,
        draft,
        library,
        sources: project.sources.clone(),
        tracks: project.tracks.clone(),
        clips,
        workspace: Workspace {
            reconstruction: project.reconstruction.clone(),
            editing_sound: project.editing_sound, timeline: project.timeline.clone(),
            profiles: project.profiles.clone(),
            active_profile: project.active_profile,
            dark: project.dark,
            selected_clip: project.selected_clip,
            target_track: project.target_track,
            insert_seconds: project.insert_seconds,
        },
    };
    serde_json::to_vec(&document).map_err(|e| e.to_string())
}
pub fn decode(bytes: &[u8]) -> Result<Project, String> {
    #[derive(Deserialize)]
    struct Header {
        schema_version: u32,
    }
    let header: Header =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid project JSON: {e}"))?;
    if header.schema_version == 1 {
        let mut legacy: Project =
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid legacy project: {e}"))?;
        legacy.schema_version = SCHEMA_VERSION;
        legacy.upgrade_workspace();
        legacy.validate()?;
        return Ok(legacy);
    }
    if header.schema_version != SCHEMA_VERSION {
        return Err("Unsupported project version".into());
    }
    let doc: Document =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid project JSON: {e}"))?;
    if doc.format != "overtone" || doc.synthesis_model != SYNTHESIS_MODEL {
        return Err("Unsupported project format or synthesis model".into());
    }
    if doc.patches.len() > 210001
        || doc
            .patches
            .iter()
            .any(|p| p.partials.is_empty() || p.partials.len() > MAX_HARMONICS)
    {
        return Err("Invalid patch table".into());
    }
    let expand = |s: SoundRef| -> Result<Sound, String> {
        doc.patches
            .get(s.patch)
            .map(|p| p.sound(s.id, s.name))
            .ok_or_else(|| "Missing sound patch reference".into())
    };
    let mut project = Project {
        reconstruction: doc.workspace.reconstruction,
        schema_version: SCHEMA_VERSION,
        title: doc.title,
        next_id: doc.next_id,
        session_bpm: doc.session_bpm,
        tuning: doc.tuning,
        recording: doc.recording,
        draft: expand(doc.draft)?,
        library: doc
            .library
            .into_iter()
            .map(expand)
            .collect::<Result<Vec<_>, _>>()?,
        sources: doc.sources,
        tracks: doc.tracks,
        clips: doc
            .clips
            .into_iter()
            .map(|c| {
                Ok(Clip {
                    id: c.id,
                    track: c.track,
                    name: c.name,
                    start_seconds: c.start_seconds,
                    duration_beats: c.duration_beats,
                    sound: expand(c.sound)?,
                    original_sound: expand(c.original_sound)?,
                    library_sound: c.library_sound,
                    notes: c
                        .notes
                        .into_iter()
                        .map(|(midi, start_beat, duration_beats, velocity)| Note {
                            midi,
                            start_beat,
                            duration_beats,
                            velocity,
                        })
                        .collect(),
                    gain: c.gain,
                    muted: c.muted,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        editing_sound: doc.workspace.editing_sound, timeline: doc.workspace.timeline,
        profiles: doc.workspace.profiles,
        active_profile: doc.workspace.active_profile,
        dark: doc.workspace.dark,
        selected_clip: doc.workspace.selected_clip,
        target_track: doc.workspace.target_track,
        insert_seconds: doc.workspace.insert_seconds,
    };
    project.upgrade_workspace();
    project.validate()?;
    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patch_table_deduplicates_and_restores_independent_instances() {
        let mut p = Project::default();
        let id = p.save_sound();
        for _ in 0..100 {
            p.add_clip(p.library[0].clone(), Some(id));
        }
        p.clips[99].sound.harmonics[1].phase = 45.;
        p.clips[99].sound.harmonics[1].detune = -3.;
        p.clips[99].sound.noise.seed = 42;
        p.add_note(60);
        p.profile_mut().panel_gap = 18.;
        p.profile_mut().panels[0].height = Some(400.);
        p.profile_mut().panels[0].share = 0.35;
        p.profile_mut().panels[0].gap_after = Some(24.);
        p.profile_mut().panels[0].pair_gap = Some(16.);
        p.profile_mut().zoom = 1.25;
        p.profile_mut().panels[0].zoom = 0.75;
        let bytes = encode(&p).unwrap();
        let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(raw["patches"].as_array().unwrap().len(), 2);
        assert!(bytes.len() * 3 < serde_json::to_vec_pretty(&p).unwrap().len());
        let mut loaded = decode(&bytes).unwrap();
        assert_eq!(loaded, p);
        loaded.clips[0].sound.noise.level = 0.9;
        assert_eq!(loaded.clips[1].sound.noise.level, 0.08);
        assert_eq!(loaded.library[0].noise.level, 0.08);
    }
    #[test]
    fn partials_omit_only_trailing_zero_values() {
        let p = Partial(Harmonic {
            multiple: 1,
            amplitude: 0.8,
            phase: 0.,
            detune: 0.,
        });
        assert_eq!(serde_json::to_string(&p).unwrap(), "[1,0.8]");
        let h: Partial = serde_json::from_str("[2,0.25,0,-3]").unwrap();
        assert_eq!(h.0.detune, -3.);
        assert!(serde_json::from_str::<Partial>("[1,0.8,0,0,1]").is_err());
    }
    #[test]
    fn legacy_projects_migrate_and_invalid_references_fail() {
        let mut p = Project::default();
        p.schema_version = 1;
        let mut legacy = serde_json::to_value(&p).unwrap();
        legacy.as_object_mut().unwrap().remove("tuning");
        legacy["draft"]["noise"]
            .as_object_mut()
            .unwrap()
            .remove("seed");
        for profile in legacy["profiles"].as_array_mut().unwrap() {
            for key in ["panel_gap", "dock_gap", "panel_padding", "zoom"] {
                profile.as_object_mut().unwrap().remove(key);
            }
            for panel in profile["panels"].as_array_mut().unwrap() {
                panel.as_object_mut().unwrap().remove("share");
                panel.as_object_mut().unwrap().remove("height");
                panel.as_object_mut().unwrap().remove("zoom");
            }
        }
        let mut expected = Project::default();
        for profile in &mut expected.profiles {
            for panel in &mut profile.panels { panel.height = None; }
        }
        assert_eq!(decode(&serde_json::to_vec(&legacy).unwrap()).unwrap(), expected);
        let mut raw: serde_json::Value =
            serde_json::from_slice(&encode(&Project::default()).unwrap()).unwrap();
        raw["draft"]["patch"] = 999.into();
        assert!(decode(&serde_json::to_vec(&raw).unwrap()).is_err());
    }
    #[test]
    fn standalone_settings_remap_ids_and_preserve_music() {
        let mut p = Project::default();
        p.add_clip(p.draft.clone(), None);
        let clips = p.clips.clone();
        let mut settings = WorkspaceSettings::from_project(&p);
        settings.profiles[0].left_width = 400.;
        settings.profiles[0].panel_padding = 20.;
        settings.dark = false;
        settings.timeline.zoom = 2.;
        settings.timeline.snap_beats = 1.;
        let decoded: WorkspaceSettings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        decoded.apply(&mut p).unwrap();
        assert_eq!(p.clips, clips);
        assert_eq!(p.timeline, settings.timeline);
        assert_eq!(p.profile().left_width, 400.);
        assert!(!p.dark);
        p.validate().unwrap();
    }
}

#[cfg(test)]
mod editing_timeline_tests {
    use super::*;
    #[test]
    fn editing_removing_and_roundtripping_preserve_clip_snapshots() {
        let mut p=Project::default();let id=p.save_sound();
        p.add_library_clip(p.library[0].clone(),id);
        let clip=p.clips[0].clone();assert!(!clip.notes.is_empty());
        p.editing_sound=Some(id);p.draft=p.library[0].clone();
        p.draft.name="Edited".into();p.draft.harmonics[0].amplitude=0.27;p.draft.noise.level=0.32;
        p.timeline.zoom=4.;p.timeline.snap_beats=0.5;
        assert_eq!(p.save_sound(),id);assert_eq!(p.library.len(),1);assert_eq!(p.library[0],p.draft);
        assert_eq!(p.clips[0],clip);
        let restored=decode(&encode(&p).unwrap()).unwrap();assert_eq!(restored,p);
        p.remove_sound(id);assert!(p.library.is_empty());assert_eq!(p.editing_sound,None);
        assert_eq!(p.clips[0].sound,clip.sound);assert_eq!(p.clips[0].original_sound,clip.original_sound);assert_eq!(p.clips[0].library_sound,None);
        assert_eq!(decode(&encode(&p).unwrap()).unwrap(),p);
    }
    #[test]
    fn grid_snaps_to_destination_tempo_and_old_documents_default_the_view() {
        let mut p=Project::default();p.tracks[1].bpm=90.;p.tracks[1].follow_session=false;p.timeline.snap_beats=1.;
        assert_eq!(p.snap_seconds(0.8,1),1.);assert!((p.snap_seconds(0.8,2)-2./3.).abs()<1e-10);
        let id=p.add_clip(p.draft.clone(),None);p.place_clip(id,2,0.8);assert!((p.clips[0].start_seconds-2./3.).abs()<1e-10);
        p.timeline.snap_beats=0.;assert_eq!(p.snap_seconds(0.817,2),0.817);
        let mut doc:serde_json::Value=serde_json::from_slice(&encode(&p).unwrap()).unwrap();doc["workspace"].as_object_mut().unwrap().remove("timeline");doc["workspace"].as_object_mut().unwrap().remove("editing_sound");
        let restored=decode(&serde_json::to_vec(&doc).unwrap()).unwrap();assert_eq!(restored.timeline,TimelineSettings::default());assert_eq!(restored.editing_sound,None);
        p.timeline.zoom=f32::NAN;assert!(p.validate().is_err());
    }
}

#[cfg(test)]
mod partial_removal_tests {
    use super::*;
    #[test]
    fn deleting_a_partial_preserves_ratios_and_survives_save_and_expansion() {
        let mut p=Project::default();
        let h3=p.draft.harmonics[2].clone();p.draft.remove_harmonic(1);
        assert_eq!(p.draft.harmonics[1],h3);
        p.draft.resize_harmonics(8);assert_eq!(p.draft.harmonics[1].multiple,2);assert_eq!(p.draft.harmonics[1].amplitude,0.);
        assert_eq!(p.draft.harmonics[2],h3);
        p.draft.remove_harmonic(1);let id=p.save_sound();p.add_library_clip(p.library[0].clone(),id);
        assert_eq!(decode(&encode(&p).unwrap()).unwrap(),p);
        p.draft.resize_harmonics(32);assert_eq!(p.draft.harmonics.len(),32);p.validate().unwrap();
        p.draft.harmonics[1].multiple=1;assert!(p.validate().is_err());
    }
    #[test]
    fn library_drop_uses_destination_grid_and_independent_snapshots() {
        let mut p=Project::default();p.tracks[1].bpm=90.;p.tracks[1].follow_session=false;let sound=p.save_sound();
        let clip=p.place_library_sound(sound,2,0.91).unwrap();
        assert_eq!(p.clips[0].track,2);assert_eq!(p.selected_clip,Some(clip));
        assert!((p.clips[0].start_seconds-5./6.).abs()<1e-9);
        let instance=p.clips[0].clone();p.editing_sound=Some(sound);p.draft.remove_harmonic(1);p.save_sound();
        assert_eq!(p.clips[0],instance);assert!(!p.clips[0].notes.is_empty());
        let before=p.clone();assert_eq!(p.place_library_sound(sound,999,0.),None);assert_eq!(p,before);
    }
}

#[cfg(test)] mod reconstruction_migration_tests {
    use super::*;
    #[test] fn old_projects_and_settings_gain_a_hidden_component_without_losing_the_layout() {
        let project=Project::default();let mut raw:serde_json::Value=serde_json::from_slice(&encode(&project).unwrap()).unwrap();
        raw["workspace"].as_object_mut().unwrap().remove("reconstruction");
        for profile in raw["workspace"]["profiles"].as_array_mut().unwrap(){profile["panels"].as_array_mut().unwrap().retain(|p|p["module"]!="Reconstruction");}
        let restored=decode(&serde_json::to_vec(&raw).unwrap()).unwrap();
        assert_eq!(restored.reconstruction,ReconstructionSettings::default());
        for p in &restored.profiles {assert_eq!(p.panels.len(),8);assert!(!p.panels.iter().find(|p|p.module==Module::Reconstruction).unwrap().visible);}
        let mut settings=WorkspaceSettings::from_project(&project);
        for p in &mut settings.profiles {p.panels.retain(|p|p.module!=Module::Reconstruction);}
        settings.validate().unwrap();let mut destination=Project::default();settings.apply(&mut destination).unwrap();destination.validate().unwrap();
        assert!(destination.profile().panels.iter().any(|p|p.module==Module::Reconstruction && !p.visible));
        raw["workspace"]["profiles"][0]["panels"].as_array_mut().unwrap().retain(|p|p["module"]!="Timeline");
        assert!(decode(&serde_json::to_vec(&raw).unwrap()).is_err());
    }
    #[test] fn cleanup_controls_and_sparse_sound_edits_roundtrip_together() {
        let mut p=Project::default();p.reconstruction.weak_threshold=0.12;p.reconstruction.remove_noise=true;p.reconstruction.cycles=8.;
        p.draft.harmonics[1].amplitude=0.001;p.draft.clean_harmonics(0.12,true);
        assert_eq!(decode(&encode(&p).unwrap()).unwrap(),p);
        let settings=WorkspaceSettings::from_project(&p);let mut restored=Project::default();settings.apply(&mut restored).unwrap();assert_eq!(restored.reconstruction,p.reconstruction);
        p.reconstruction.weak_threshold=f32::NAN;assert!(p.validate().is_err());
    }
}
