use super::*;
use crate::{
    engine::{Command, EngineConfig, NoteRequest, SynthApi, SynthEngine},
    persistence, storage,
};
fn tone(rate: u32, hz: f64, seconds: f64, partials: &[(usize, f64, f64)]) -> Vec<f32> {
    (0..(seconds * rate as f64) as usize)
        .map(|i| {
            partials
                .iter()
                .map(|(n, a, p)| {
                    a * (std::f64::consts::TAU * hz * *n as f64 * i as f64 / rate as f64 + p).sin()
                })
                .sum::<f64>() as f32
        })
        .collect()
}
#[test]
fn whole_recording_keeps_pauses_and_all_pitched_sections_in_one_patch() {
    let rate = 48000;
    let mut samples = vec![0.; rate as usize / 5];
    samples.extend(tone(
        rate,
        220.,
        0.7,
        &[(1, 0.5, 0.3), (2, 0.25, -0.2), (3, 0.1, 0.1)],
    ));
    samples.extend(vec![0.; rate as usize / 3]);
    samples.extend(tone(rate, 330., 0.6, &[(2, 0.4, 0.2), (3, 0.25, 0.3)]));
    samples.extend(vec![0.; rate as usize / 5]);
    let result = analyze(
        &Recording {
            samples,
            sample_rate: rate,
        },
        &AnalysisSettings::default(),
    )
    .unwrap();
    assert_eq!(result.sounds.len(), 1);
    let sound=&result.sounds[0];let capture=sound.capture.as_ref().unwrap();
    assert!((capture.fundamental_hz.unwrap()-110.).abs()<2.);
    assert_eq!(capture.start_seconds,0.);
    assert!((capture.duration_seconds-(0.2+0.7+1./3.+0.6+0.2)).abs()<0.001);
    // 220/440 from the first section and 660/990 from the second share one 110 Hz basis.
    for multiple in [2,4,6,9] {assert!(sound.harmonics[multiple-1].amplitude>0.08,"H{multiple} missing");}
    assert!(sound.harmonics[0].amplitude<0.03);
    assert!(sound.noise.level<0.15,"Residual noise {}",sound.noise.level);
}
#[test]
fn sustained_pitch_changes_become_harmonics_of_one_sound() {
    let rate = 48000;
    let mut samples = tone(rate, 220., 0.6, &[(1, 0.5, 0.)]);
    samples.extend(tone(rate, 440., 0.6, &[(1, 0.5, 0.)]));
    let result = analyze(
        &Recording {
            samples,
            sample_rate: rate,
        },
        &AnalysisSettings::default(),
    )
    .unwrap();
    assert_eq!(result.sounds.len(), 1);
    let sound=&result.sounds[0];
    assert!((sound.capture.as_ref().unwrap().fundamental_hz.unwrap()-220.).abs()<3.);
    assert!(sound.harmonics[0].amplitude>0.5);
    assert!(sound.harmonics[1].amplitude>0.5);
    assert!(sound.noise.level<0.15,"Residual noise {}",sound.noise.level);
}
#[test]
fn noise_becomes_editable_noise_and_silence_creates_no_fake_sound() {
    let mut seed = 42u64;
    let samples = (0..24000)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed as u32 as f64 / u32::MAX as f64 * 2. - 1.) as f32 * 0.3
        })
        .collect();
    let result = analyze(
        &Recording {
            samples,
            sample_rate: 48000,
        },
        &AnalysisSettings::default(),
    )
    .unwrap();
    assert_eq!(result.sounds.len(), 1);
    let sound = &result.sounds[0];
    assert!(sound.capture.as_ref().unwrap().fundamental_hz.is_none());
    assert!(sound.noise.level > 0.5);
    assert!(sound.harmonics.iter().all(|h| h.amplitude == 0.));
    assert!(
        analyze(
            &Recording {
                samples: vec![0.; 48000],
                sample_rate: 48000
            },
            &AnalysisSettings::default()
        )
        .unwrap()
        .sounds
        .is_empty()
    );
}
#[test]
fn respects_trim_maximum_and_rejects_invalid_input() {
    let rate = 48000;
    let samples = tone(rate, 440., 1., &[(1, 0.5, 0.)]);
    let settings = AnalysisSettings {
        max_harmonics: 7,
        trim_start_seconds: 0.2,
        trim_end_seconds: Some(0.8),
        ..Default::default()
    };
    let result = analyze(
        &Recording {
            samples: samples.clone(),
            sample_rate: rate,
        },
        &settings,
    )
    .unwrap();
    assert_eq!(result.sounds[0].harmonics.len(), 7);
    assert!((result.sounds[0].capture.as_ref().unwrap().start_seconds - 0.2).abs() < 0.01);
    assert!(
        analyze(
            &Recording {
                samples: vec![f32::NAN; 48000],
                sample_rate: rate
            },
            &settings
        )
        .is_err()
    );
    assert!(
        analyze(
            &Recording {
                samples,
                sample_rate: rate
            },
            &AnalysisSettings {
                max_harmonics: 33,
                ..settings
            }
        )
        .is_err()
    );
}
#[test]
fn decodes_stereo_wav_then_preserves_whole_sound_and_assets_on_save() {
    let dir = std::env::temp_dir().join(format!("overtone-analysis-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("take.wav");
    let rate = 44100;
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for x in tone(rate, 250., 0.5, &[(1, 0.5, 0.), (2, 0.2, 0.)]) {
        let x = (x * 32767.) as i16;
        writer.write_sample(x).unwrap();
        writer.write_sample(x).unwrap();
    }
    writer.finalize().unwrap();
    let decoded = decode(&path).unwrap();
    assert_eq!(decoded.sample_rate, rate);
    assert_eq!(decoded.samples.len(), rate as usize / 2);
    let mut project = Project::default();
    let mut source = persistence::import_source(&path).unwrap();
    source.id = project.allocate();
    let source_id = source.id;
    project.sources.push(source);
    let result = analyze_file(&path, &AnalysisSettings::default()).unwrap();
    for mut sound in result.sounds {
        sound.id = project.allocate();
        sound.source = Some(source_id);
        project.library.push(sound);
    }
    let file = dir.join("session.overtone");
    let saved = persistence::save(&project, &file).unwrap();
    assert_eq!(saved.sources.len(), 1);
    let loaded = persistence::load(&file).unwrap();
    assert_eq!(loaded.project.library, project.library);
    assert_eq!(
        storage::decode(&storage::encode(&project).unwrap()).unwrap(),
        project
    );
    std::fs::remove_file(&path).unwrap();
    assert!(persistence::load(&file).unwrap().missing_sources.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn synthesized_patch_can_be_played_at_measured_frequency_and_tweaked() {
    let rate = 48000;
    let samples = tone(rate, 237., 0.5, &[(1, 0.5, 0.), (2, 0.2, 0.)]);
    let mut sound = analyze(
        &Recording {
            samples,
            sample_rate: rate,
        },
        &AnalysisSettings::default(),
    )
    .unwrap()
    .sounds
    .remove(0);
    let hz = sound.capture.as_ref().unwrap().fundamental_hz.unwrap();
    let api = SynthApi::new(EngineConfig::default(), Tuning::default()).unwrap();
    let mut engine = SynthEngine::new(api.config()).unwrap();
    let request = NoteRequest {
        voice_id: 1,
        owner: 11,
        midi: 60,
        velocity: 1.,
        gate_seconds: 0.4,
        gain: 1.,
        pan: 0.,
    };
    engine.command(Command::Note(
        api.note_frequency(&sound, request, hz).unwrap(),
    ));
    let mut output = vec![0.; 48000];
    engine.render(&mut output);
    assert!(output.iter().any(|v| v.abs() > 0.01));
    sound.harmonics[1].amplitude = 0.8;
    engine.command(Command::Update {
        owner: 11,
        patch: api.patch(&sound).unwrap(),
    });
    assert!(api.note_frequency(&sound, request, f64::NAN).is_err());
}
#[test]
fn legacy_patch_defaults_capture_and_analysis_configuration() {
    let mut raw: serde_json::Value =
        serde_json::from_slice(&storage::encode(&Project::default()).unwrap()).unwrap();
    for patch in raw["patches"].as_array_mut().unwrap() {
        patch.as_object_mut().unwrap().remove("capture");
    }
    assert_eq!(
        storage::decode(&serde_json::to_vec(&raw).unwrap()).unwrap(),
        Project::default()
    );
    let legacy:AnalysisSettings=serde_json::from_str(r#"{"max_harmonics":32,"fundamental_hz":null,"trim_start_seconds":0,"trim_end_seconds":null}"#).unwrap();
    assert_eq!(legacy, AnalysisSettings::default());
}

#[test]
fn repeated_attacks_with_gaps_never_split_the_recording() {
    let rate = 48000;
    let mut samples = tone(rate, 220., 0.4, &[(1, 0.5, 0.)]);
    samples.extend(vec![0.; rate as usize / 20]);
    samples.extend(tone(rate, 220., 0.4, &[(1, 0.5, 0.)]));
    let result = analyze(
        &Recording {
            samples,
            sample_rate: rate,
        },
        &AnalysisSettings::default(),
    )
    .unwrap();
    assert_eq!(result.sounds.len(), 1);
    assert!((result.sounds[0].capture.as_ref().unwrap().duration_seconds-0.85).abs()<0.001);
}

#[test]
fn high_rate_recordings_preserve_low_fundamentals_and_harmonic_balance() {
    let rate = 192000;
    let samples = tone(rate, 55., 0.6, &[(1, 0.5, 0.), (2, 0.2, 0.), (3, 0.1, 0.)]);
    let result = analyze(
        &Recording {
            samples,
            sample_rate: rate,
        },
        &AnalysisSettings::default(),
    )
    .unwrap();
    let sound = &result.sounds[0];
    assert!((sound.capture.as_ref().unwrap().fundamental_hz.unwrap() - 55.).abs() < 1.);
    assert!(sound.harmonics[0].amplitude > 0.7);
    assert!((sound.harmonics[1].amplitude / sound.harmonics[0].amplitude - 0.4).abs() < 0.06);
    assert!(sound.noise.level < 0.1);
}

#[test]
fn split_recordings_keep_original_bounds_and_respect_gap_minimum_and_trim() {
    let rate=48000;
    let mut samples=vec![0.;rate/5];
    samples.extend(tone(rate as u32,220.,0.3,&[(1,0.6,0.)]));
    samples.extend(vec![0.;rate/4]);
    samples.extend(tone(rate as u32,330.,0.4,&[(1,0.4,0.)]));
    samples.extend(vec![0.;rate/5]);
    let recording=Recording{samples,sample_rate:rate as u32};
    let mut settings=AnalysisSettings::default();
    let result=split_recording(&recording,&settings).unwrap();
    assert_eq!(result.sounds.len(),2);
    let first=result.sounds[0].capture.as_ref().unwrap();let second=result.sounds[1].capture.as_ref().unwrap();
    assert_eq!(first.start_seconds,0.2);assert_eq!(first.duration_seconds,0.3);
    assert_eq!(second.start_seconds,0.75);assert_eq!(second.duration_seconds,0.4);
    assert!(result.sounds.iter().all(|s|s.recorded_sample));
    settings.split_gap_ms=300;assert_eq!(split_recording(&recording,&settings).unwrap().sounds.len(),1);
    settings.split_gap_ms=150;settings.min_sound_ms=350;assert_eq!(split_recording(&recording,&settings).unwrap().sounds.len(),1);
    settings.min_sound_ms=100;settings.trim_start_seconds=0.8;settings.trim_end_seconds=Some(1.);
    let trimmed=split_recording(&recording,&settings).unwrap();
    assert_eq!(trimmed.sounds.len(),1);assert_eq!(trimmed.sounds[0].capture.as_ref().unwrap().start_seconds,0.8);
    assert_eq!(split_recording(&Recording{samples:vec![0.;rate],sample_rate:rate as u32},&settings).unwrap().sounds.len(),0);
    settings.split_gap_ms=0;assert!(split_recording(&recording,&settings).is_err());
}
#[test]
fn split_short_clicks_are_filtered_and_short_internal_pauses_stay_in_one_sound() {
    let mut settings=AnalysisSettings::default();settings.min_sound_ms=50;
    let mut samples=vec![0.5;800];samples.extend(vec![0.;400]);samples.extend(vec![0.5;800]);
    let result=split_recording(&Recording{samples,sample_rate:8000},&settings).unwrap();
    assert_eq!(result.sounds.len(),1);assert_eq!(result.sounds[0].capture.as_ref().unwrap().duration_seconds,0.25);
    let result=split_recording(&Recording{samples:vec![0.5;160],sample_rate:8000},&settings).unwrap();assert!(result.sounds.is_empty());
}
