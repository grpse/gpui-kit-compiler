use crate::model::*;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_PROJECT_BYTES: u64 = 64 * 1024 * 1024;
static NONCE: AtomicU64 = AtomicU64::new(0);
pub struct Loaded {
    pub project: Project,
    pub missing_sources: Vec<String>,
}
pub struct Saved {
    pub sources: Vec<(Id, PathBuf)>,
}

fn fingerprint(path: &Path) -> Result<(String, u64), String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        bytes += n as u64;
    }
    Ok((format!("{:x}", hash.finalize()), bytes))
}
pub fn import_source(path: &Path) -> Result<VoiceReference, String> {
    let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
    if !path.is_file() {
        return Err("Choose an audio file".into());
    }
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ![
        "wav", "wave", "aif", "aiff", "flac", "mp3", "m4a", "ogg", "opus", "webm",
    ]
    .contains(&extension.as_str())
    {
        return Err(
            "Choose a WAV, AIFF, FLAC, MP3, M4A, OGG, Opus, or WebM voice reference".into(),
        );
    }
    let (sha256, bytes) = fingerprint(&path)?;
    Ok(VoiceReference {
        id: 0,
        name: path.file_name().unwrap().to_string_lossy().into(),
        path,
        bytes,
        sha256,
        analysis: AnalysisSettings::default(),
    })
}
fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.into())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|e| e.to_string())
    }
}
pub fn load(path: &Path) -> Result<Loaded, String> {
    let path = absolute(path)?;
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let mut bytes = vec![];
    file.take(MAX_PROJECT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Project file exceeds 64 MiB".into());
    }
    let mut project = crate::storage::decode(&bytes)?;
    let parent = path.parent().unwrap();
    let mut missing_sources = vec![];
    for source in &mut project.sources {
        if source.path.is_relative() {
            source.path = parent.join(&source.path);
        }
        if !source.path.is_file() {
            missing_sources.push(source.name.clone());
        }
    }
    Ok(Loaded {
        project,
        missing_sources,
    })
}
fn temporary(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        ".{}.{}-{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ))
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = temporary(path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
pub fn save(project: &Project, path: &Path) -> Result<Saved, String> {
    project.validate()?;
    if ![Some("overtone"), Some("json")].contains(&path.extension().and_then(|s| s.to_str())) {
        return Err("Use an .overtone or .json project filename".into());
    }
    let path = absolute(path)?;
    let parent = path.parent().ok_or("Project needs a filename")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let folder = format!(
        "{}.assets",
        path.file_stem()
            .ok_or("Project needs a filename")?
            .to_string_lossy()
    );
    let source_dir = parent.join(&folder).join("sources");
    let mut serialized = project.clone();
    let mut saved = vec![];
    for source in &mut serialized.sources {
        let extension = source
            .path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("audio")
            .to_lowercase();
        let relative = PathBuf::from(&folder)
            .join("sources")
            .join(format!("{}.{}", source.sha256, extension));
        let target = parent.join(&relative);
        if source.path.is_file() {
            let (hash, size) = fingerprint(&source.path)?;
            if hash != source.sha256 || size != source.bytes {
                return Err(format!(
                    "Voice reference changed: {}. Import the new file before saving.",
                    source.name
                ));
            }
            fs::create_dir_all(&source_dir).map_err(|e| e.to_string())?;
            if target.is_file() {
                let (hash, size) = fingerprint(&target)?;
                if hash != source.sha256 || size != source.bytes {
                    return Err(format!("Stored source is damaged: {}", target.display()));
                }
            } else {
                let temp = temporary(&target);
                let result: Result<(), String> = (|| {
                    let mut input = File::open(&source.path).map_err(|e| e.to_string())?;
                    let mut output = OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(&temp)
                        .map_err(|e| e.to_string())?;
                    std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
                    output.sync_all().map_err(|e| e.to_string())?;
                    let (hash, size) = fingerprint(&temp)?;
                    if hash != source.sha256 || size != source.bytes {
                        return Err("Source changed while copying; retry saving".into());
                    }
                    fs::rename(&temp, &target).map_err(|e| e.to_string())?;
                    Ok(())
                })();
                if result.is_err() {
                    let _ = fs::remove_file(temp);
                }
                result?;
            }
            source.path = relative;
            saved.push((source.id, target));
        } else if source.path == target {
            // Keep the portable reference so a missing asset can be restored later.
            source.path = relative;
        } else {
            return Err(format!(
                "Missing voice reference: {}. Restore it before Save As.",
                source.name
            ));
        }
    }
    let bytes = crate::storage::encode(&serialized)?;
    if bytes.len() as u64 > MAX_PROJECT_BYTES {
        return Err("Project exceeds 64 MiB".into());
    }
    atomic_write(&path, &bytes)?;
    Ok(Saved { sources: saved })
}
pub fn save_settings(
    settings: &crate::storage::WorkspaceSettings,
    path: &Path,
) -> Result<(), String> {
    settings.validate()?;
    let path = absolute(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec(settings).map_err(|e| e.to_string())?;
    atomic_write(&path, &bytes)
}
pub fn load_settings(path: &Path) -> Result<crate::storage::WorkspaceSettings, String> {
    let mut bytes = vec![];
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Settings exceed 1 MiB".into());
    }
    let settings: crate::storage::WorkspaceSettings =
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    settings.validate()?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "overtone-{}-{}",
                std::process::id(),
                NONCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn saving_roundtrips_all_editable_information_and_portable_assets() {
        let temp = Temp::new();
        let audio = temp.0.join("voice.wav");
        fs::write(&audio, b"opaque reference bytes; no decoding").unwrap();
        let mut p = Project::default();
        let mut source = import_source(&audio).unwrap();
        source.id = p.allocate();
        p.draft.source = Some(source.id);
        p.sources.push(source);
        p.draft.harmonics[0].phase = 45.;
        p.draft.harmonics[1].detune = -3.;
        p.draft.noise.color = NoiseColor::Brown;
        let id = p.save_sound();
        p.add_clip(p.library[0].clone(), Some(id));
        p.add_note(62);
        p.selected_mut().unwrap().sound.noise.level = 0.42;
        p.add_profile("Mixing".into(), false);
        p.profile_mut()
            .move_panel(Module::Library, Dock::Right, None);
        let file = temp.0.join("session.overtone");
        let saved = save(&p, &file).unwrap();
        assert_eq!(saved.sources.len(), 1);
        fs::remove_file(&audio).unwrap();
        let loaded = load(&file).unwrap();
        assert!(loaded.missing_sources.is_empty());
        p.sources[0].path = saved.sources[0].1.clone();
        assert_eq!(loaded.project, p);
        let relocated = temp.0.join("relocated");
        fs::create_dir(&relocated).unwrap();
        fs::rename(&file, relocated.join("session.overtone")).unwrap();
        fs::rename(
            temp.0.join("session.assets"),
            relocated.join("session.assets"),
        )
        .unwrap();
        assert!(
            load(&relocated.join("session.overtone"))
                .unwrap()
                .missing_sources
                .is_empty()
        );
    }
    #[test]
    fn failed_save_keeps_previous_project_and_detects_changed_source() {
        let temp = Temp::new();
        let file = temp.0.join("session.overtone");
        let mut p = Project::default();
        save(&p, &file).unwrap();
        let old = fs::read(&file).unwrap();
        p.session_bpm = 0.;
        assert!(save(&p, &file).is_err());
        assert_eq!(fs::read(&file).unwrap(), old);
        p.session_bpm = 120.;
        let audio = temp.0.join("voice.wav");
        fs::write(&audio, b"old").unwrap();
        let mut source = import_source(&audio).unwrap();
        source.id = p.allocate();
        p.sources.push(source);
        fs::write(&audio, b"new").unwrap();
        assert!(save(&p, &file).is_err());
        assert_eq!(fs::read(&file).unwrap(), old);
    }
    #[test]
    fn corrupt_and_future_projects_are_rejected_missing_assets_are_reported() {
        let temp = Temp::new();
        let file = temp.0.join("session.overtone");
        fs::write(&file, b"{}").unwrap();
        assert!(load(&file).is_err());
        let mut p = Project::default();
        p.schema_version = 999;
        fs::write(&file, serde_json::to_vec(&p).unwrap()).unwrap();
        assert!(load(&file).is_err());
        p.schema_version = SCHEMA_VERSION;
        let audio = temp.0.join("voice.wav");
        fs::write(&audio, b"old").unwrap();
        let mut s = import_source(&audio).unwrap();
        s.id = p.allocate();
        p.sources.push(s);
        let saved = save(&p, &file).unwrap();
        fs::remove_file(&saved.sources[0].1).unwrap();
        let loaded = load(&file).unwrap();
        assert_eq!(loaded.missing_sources.len(), 1);
        save(&loaded.project, &file).unwrap();
    }
}
