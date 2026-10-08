use super::*;
use crate::model::{NoiseColor, Note};
fn api() -> SynthApi {
    SynthApi::new(
        EngineConfig {
            master_gain: 1.,
            ..Default::default()
        },
        Tuning::default(),
    )
    .unwrap()
}
fn sine() -> Sound {
    let mut s = Sound::new(0, "Sine");
    s.resize_harmonics(1);
    s.harmonics[0].amplitude = 1.;
    s.noise.level = 0.;
    s.gain = 1.;
    s.brightness = 1.;
    s.envelope.attack_ms = 0.;
    s.envelope.decay_ms = 0.;
    s.envelope.sustain = 1.;
    s.envelope.release_ms = 20.;
    s
}
fn request(id: u64, owner: u64) -> NoteRequest {
    NoteRequest {
        voice_id: id,
        owner,
        midi: 69,
        velocity: 1.,
        gate_seconds: 1.,
        gain: 1.,
        pan: -1.,
    }
}
fn samples(sound: &Sound) -> Vec<f32> {
    let mut e = SynthEngine::new(api().config()).unwrap();
    e.command(Command::Note(api().note(sound, request(1, 0)).unwrap()));
    let mut data = vec![0.; 48000];
    e.render(&mut data);
    data.chunks_exact(2).map(|f| f[0]).collect()
}
fn amplitude(data: &[f32], hz: f64) -> f64 {
    let mut re = 0.;
    let mut im = 0.;
    for (n, &v) in data.iter().enumerate() {
        let angle = TAU * hz * n as f64 / 48000.;
        re += v as f64 * angle.cos();
        im += v as f64 * angle.sin();
    }
    2. * (re * re + im * im).sqrt() / data.len() as f64
}
#[test]
fn pitch_phase_detune_and_nyquist_are_correct() {
    let data = samples(&sine());
    assert!(amplitude(&data, 440.) > 0.97);
    assert!(amplitude(&data, 880.) < 0.001);
    let mut sound = sine();
    sound.harmonics[0].detune = 50.;
    let shifted = samples(&sound);
    let hz = 440. * 2_f64.powf(50. / 1200.);
    assert!(amplitude(&shifted, hz) > 0.97);
    assert!(amplitude(&shifted, 440.) < 0.06);
    sound.harmonics[0].phase = 90.;
    sound.harmonics[0].detune = 0.;
    let phase = samples(&sound);
    let covariance = data[200..]
        .iter()
        .zip(&phase[200..])
        .map(|(a, b)| a * b)
        .sum::<f32>()
        / 23800.;
    assert!(covariance.abs() < 0.005);
    let high = api()
        .note(
            &sound,
            NoteRequest {
                midi: 127,
                ..request(1, 0)
            },
        )
        .unwrap();
    assert!(high.weight[0] > 0.);
    sound.resize_harmonics(32);
    sound.harmonics[31].amplitude = 1.;
    let high = api()
        .note(
            &sound,
            NoteRequest {
                midi: 127,
                ..request(1, 0)
            },
        )
        .unwrap();
    assert_eq!(high.weight[31], 0.);
}
#[test]
fn seeded_noise_and_block_sizes_are_reproducible() {
    let mut sound = sine();
    sound.noise.level = 0.5;
    sound.noise.seed = 42;
    sound.noise.color = NoiseColor::Pink;
    let note = api().note(&sound, request(1, 0)).unwrap();
    let mut a = SynthEngine::new(api().config()).unwrap();
    let mut b = SynthEngine::new(api().config()).unwrap();
    a.command(Command::Note(note));
    b.command(Command::Note(note));
    let mut all = [0.; 4096];
    let mut chunks = [0.; 4096];
    a.render(&mut all);
    for chunk in chunks.chunks_mut(14) {
        b.render(chunk);
    }
    assert_eq!(all, chunks);
    sound.noise.seed = 43;
    assert_ne!(
        samples(&sound),
        samples(&{
            let mut s = sound.clone();
            s.noise.seed = 42;
            s
        })
    );
}
#[test]
fn envelope_releases_and_stop_never_leave_stuck_voices() {
    let mut e = SynthEngine::new(api().config()).unwrap();
    let note = api()
        .note(
            &sine(),
            NoteRequest {
                gate_seconds: 0.01,
                ..request(1, 0)
            },
        )
        .unwrap();
    e.command(Command::Note(note));
    let mut data = [0.; 4096];
    e.render(&mut data);
    assert_eq!(data[0], 0.);
    assert_eq!(e.active_voices(), 0);
    assert!(data[3000..].iter().all(|v| *v == 0.));
    e.command(Command::Note(api().note(&sine(), request(2, 0)).unwrap()));
    e.render(&mut data[..256]);
    e.command(Command::Stop);
    e.render(&mut data);
    assert!(e.finished());
    assert_eq!(e.dropped_notes, 0);
}
#[test]
fn noise_colors_have_different_spectral_balance() {
    let mut roughness = Vec::new();
    for color in [NoiseColor::White, NoiseColor::Pink, NoiseColor::Brown] {
        let mut s = sine();
        s.harmonics[0].amplitude = 0.;
        s.noise.level = 1.;
        s.noise.seed = 42;
        s.noise.color = color;
        let data = samples(&s);
        let energy = data.iter().map(|x| x * x).sum::<f32>();
        let diff = data.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f32>();
        roughness.push(diff / energy);
    }
    assert!(roughness[0] > roughness[1] * 1.5);
    assert!(roughness[1] > roughness[2] * 1.5);
}
#[test]
fn voice_updates_are_smoothed_and_isolated_to_the_owner() {
    let mut s = sine();
    let mut e = SynthEngine::new(api().config()).unwrap();
    e.command(Command::Note(api().note(&s, request(1, 12)).unwrap()));
    e.command(Command::Note(api().note(&s, request(2, 13)).unwrap()));
    let mut data = [0.; 4096];
    e.render(&mut data);
    s.gain = 0.;
    e.command(Command::Update {
        owner: 12,
        patch: api().patch(&s).unwrap(),
    });
    let first = e
        .voices
        .iter()
        .flatten()
        .find(|v| v.note.request.owner == 12)
        .unwrap();
    assert_eq!(first.gain, 1.);
    assert_eq!(first.note.patch.gain, 0.);
    e.render(&mut data);
    let first = e
        .voices
        .iter()
        .flatten()
        .find(|v| v.note.request.owner == 12)
        .unwrap();
    assert!(first.gain < 0.001);
    let other = e
        .voices
        .iter()
        .flatten()
        .find(|v| v.note.request.owner == 13)
        .unwrap();
    assert_eq!(other.gain, 1.);
}
fn timeline_project() -> Project {
    let mut p = Project::default();
    p.draft = sine();
    p.add_clip(p.draft.clone(), None);
    let clip = &mut p.clips[0];
    clip.start_seconds = 0.5;
    clip.duration_beats = 2.;
    clip.notes = vec![Note {
        midi: 69,
        start_beat: 0.,
        duration_beats: 1.,
        velocity: 1.,
    }];
    p.tracks[0].follow_session = false;
    p.tracks[0].bpm = 60.;
    p
}
#[test]
fn timeline_uses_track_tempo_mute_solo_and_release_tails() {
    let mut p = timeline_project();
    let a = SynthApi::new(api().config(), p.tuning.clone()).unwrap();
    let plan = a.timeline(&p).unwrap();
    assert_eq!(plan.events[0].frame, 24000);
    assert_eq!(plan.events[0].note.gate, 48000);
    assert_eq!(plan.total_frames, 72960);
    p.tracks[0].bpm = 120.;
    let faster = a.timeline(&p).unwrap();
    assert_eq!(faster.events[0].frame, 24000);
    assert_eq!(faster.events[0].note.gate, 24000);
    p.tracks[0].muted = true;
    assert_eq!(a.timeline(&p).unwrap().total_frames, 0);
    p.tracks[0].muted = false;
    p.tracks[1].solo = true;
    assert_eq!(a.timeline(&p).unwrap().total_frames, 0);
}
#[test]
fn simultaneous_voice_limit_is_explicit_and_releases_reuse_slots() {
    let mut p = timeline_project();
    p.clips[0].notes = vec![p.clips[0].notes[0].clone(); 2];
    let a = SynthApi::new(
        EngineConfig {
            voices: 1,
            ..api().config()
        },
        p.tuning.clone(),
    )
    .unwrap();
    assert!(a.timeline(&p).is_err());
    let mut e = SynthEngine::new(a.config()).unwrap();
    let n = a
        .note(
            &sine(),
            NoteRequest {
                gate_seconds: 0.01,
                ..request(1, 0)
            },
        )
        .unwrap();
    e.command(Command::Note(n));
    let mut buffer = vec![0.; (n.gate + n.patch.release) as usize * 2];
    e.render(&mut buffer);
    e.next_frame();
    e.command(Command::Note(n));
    assert_eq!(e.dropped_notes, 0);
}
#[test]
fn invalid_parameters_reject_nan_and_wav_roundtrips_pcm() {
    let mut s = sine();
    s.gain = f32::NAN;
    assert!(api().patch(&s).is_err());
    assert!(
        SynthApi::new(
            EngineConfig {
                sample_rate: 0,
                ..Default::default()
            },
            Tuning::default()
        )
        .is_err()
    );
    let p = timeline_project();
    let path = std::env::temp_dir().join(format!(
        "overtone-dsp-{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    api().export_wav(&p, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 48000);
    assert_eq!(bytes.len(), 44 + 72960 * 4);
    assert!(bytes[44..44 + 24000 * 4].iter().all(|x| *x == 0));
    assert!(bytes[44 + 24000 * 4..].iter().any(|x| *x != 0));
    std::fs::remove_file(path).unwrap();
}

thread_local! {static COUNTING:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};static ALLOCATIONS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
struct CheckedAllocator;
#[global_allocator]
static ALLOCATOR: CheckedAllocator = CheckedAllocator;
unsafe impl std::alloc::GlobalAlloc for CheckedAllocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        COUNTING.with(|c| {
            if c.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        COUNTING.with(|c| {
            if c.get() {
                ALLOCATIONS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}
#[test]
fn rendering_scheduled_notes_and_applying_controls_allocate_nothing() {
    let a = api();
    let p = timeline_project();
    let mut engine = SynthEngine::from_plan(a.timeline(&p).unwrap()).unwrap();
    let update = Command::Update {
        owner: p.clips[0].id,
        patch: a.patch(&sine()).unwrap(),
    };
    let mut output = [0.; 512];
    ALLOCATIONS.with(|n| n.set(0));
    COUNTING.with(|c| c.set(true));
    for _ in 0..300 {
        engine.render(&mut output);
        engine.command(update.clone());
    }
    engine.command(Command::Stop);
    engine.render(&mut output);
    COUNTING.with(|c| c.set(false));
    assert_eq!(ALLOCATIONS.with(|n| n.get()), 0);
}

#[test]
fn edited_phase_detune_and_envelope_remain_finite_and_preserve_attack() {
    let mut sound = sine();
    let mut engine = SynthEngine::new(api().config()).unwrap();
    engine.command(Command::Note(api().note(&sound, request(1, 0)).unwrap()));
    let mut buffer = [0.; 1024];
    engine.render(&mut buffer);
    let attack = engine
        .voices
        .iter()
        .flatten()
        .next()
        .unwrap()
        .note
        .patch
        .attack;
    sound.envelope.attack_ms = 500.;
    sound.envelope.sustain = 0.3;
    sound.harmonics[0].phase = -170.;
    sound.harmonics[0].detune = 100.;
    engine.command(Command::Update {
        owner: 0,
        patch: api().patch(&sound).unwrap(),
    });
    for _ in 0..8 {
        engine.render(&mut buffer);
        assert!(buffer.iter().all(|s| s.is_finite() && s.abs() <= 1.));
    }
    let voice = engine.voices.iter().flatten().next().unwrap();
    assert_eq!(voice.note.patch.attack, attack);
    assert_eq!(voice.sustain, 0.3);
    assert_eq!(voice.smoothing, 0);
}

#[cfg(feature="audio-output")]
#[test]
fn capturing_microphone_frames_allocates_and_deallocates_nothing() {
    let (mut tx,mut rx)=rtrb::RingBuffer::new(512);
    let input=[0.25f32;512];
    ALLOCATIONS.with(|n|n.set(0));COUNTING.with(|c|c.set(true));
    for _ in 0..50 {
        let stats=crate::capture::capture_frames(&input,2,|v|tx.push(v).is_ok());
        assert_eq!((stats.0,stats.1),(256,0));
        while rx.pop().is_ok(){}
    }
    COUNTING.with(|c|c.set(false));assert_eq!(ALLOCATIONS.with(|n|n.get()),0);
}

#[test]
fn measured_frequency_adapter_preserves_the_existing_midi_tuning_range() {
    for tuning in [Tuning{reference_midi:127,reference_hz:20.},Tuning{reference_midi:0,reference_hz:20000.}] {
        let api=SynthApi::new(EngineConfig::default(),tuning).unwrap();
        for midi in [0,127] {let mut r=request(1,0);r.midi=midi;assert!(api.note(&sine(),r).is_ok());}
    }
    assert!(api().note_frequency(&sine(),request(1,0),0.).is_err());
}

#[test]
fn pause_freezes_scheduled_notes_and_voice_phase_then_resumes_exactly() {
    let p=timeline_project();
    let mut playing=SynthEngine::from_plan(api().timeline(&p).unwrap()).unwrap();
    let mut reference=SynthEngine::from_plan(api().timeline(&p).unwrap()).unwrap();
    for _ in 0..12000 {assert_eq!(playing.next_frame(),reference.next_frame());}
    let frame=playing.frame();let voices=playing.active_voices();
    playing.command(Command::Pause(true));
    for _ in 0..48000 {assert_eq!(playing.next_frame(),[0.;2]);}
    assert_eq!(playing.frame(),frame);assert_eq!(playing.active_voices(),voices);
    playing.command(Command::Pause(false));
    for _ in 0..96000 {assert_eq!(playing.next_frame(),reference.next_frame());}
}

#[test]
fn removing_h2_does_not_retune_h3_and_the_engine_rejects_duplicate_ratios() {
    let mut sound=sine();sound.resize_harmonics(3);sound.harmonics[1].amplitude=0.4;sound.harmonics[2].amplitude=0.3;
    sound.remove_harmonic(1);
    let data=samples(&sound);
    assert!(amplitude(&data,880.)<0.005);assert!(amplitude(&data,1320.)>0.2);
    sound.harmonics[1].multiple=1;assert!(api().patch(&sound).is_err());
}

#[test]
fn live_partial_removal_preserves_other_oscillator_phase_and_pitch() {
    let mut sound=sine();sound.resize_harmonics(3);sound.harmonics[1].amplitude=0.4;sound.harmonics[2].amplitude=0.3;
    let mut engine=SynthEngine::new(api().config()).unwrap();engine.command(Command::Note(api().note(&sound,request(1,0)).unwrap()));
    for _ in 0..1000 {engine.next_frame();}
    let voice=engine.voices[0].as_ref().unwrap();let before=(voice.sine[2],voice.cosine[2],voice.note.step[2]);
    sound.remove_harmonic(1);engine.command(Command::Update{owner:0,patch:api().patch(&sound).unwrap()});
    let voice=engine.voices[0].as_ref().unwrap();assert_eq!((voice.sine[2],voice.cosine[2],voice.note.step[2]),before);
    assert_eq!(voice.note.patch.amplitude[1],0.);assert_eq!(voice.note.patch.ratio[2],3.);
}

struct RecordedFixture {project:Project,dir:std::path::PathBuf,original:Vec<f32>}
impl RecordedFixture {
    fn new()->Self {
        let dir=std::env::temp_dir().join(format!("overtone-sample-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();let path=dir.join("take.wav");
        let rate=8000;let mut original=Vec::new();
        let mut writer=hound::WavWriter::create(&path,hound::WavSpec{channels:1,sample_rate:rate,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();
        for i in 0..6400 {
            let value=if i<1600 || (3200..5600).contains(&i){((i*37%601) as f32/600.-0.5)*0.8}else{0.};
            let pcm=(value*32767.).round() as i16;writer.write_sample(pcm).unwrap();original.push(pcm as f32/32768.);
        }
        writer.finalize().unwrap();
        let mut project=Project::default();project.tracks[0].gain=1.;
        let mut source=crate::persistence::import_source(&path).unwrap();source.id=project.allocate();let source_id=source.id;project.sources.push(source);
        let result=crate::analysis::split_file(&path,&project.recording).unwrap();
        project.add_recorded_sounds(source_id,"Take",result.sounds);
        project.validate().unwrap();Self{project,dir,original}
    }
    fn api(&self)->SynthApi {SynthApi::new(EngineConfig{sample_rate:8000,master_gain:1.,..Default::default()},self.project.tuning.clone()).unwrap()}
}
impl Drop for RecordedFixture {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.dir);}}
#[test]
fn recorded_timeline_plays_original_samples_pauses_and_preserves_duration_at_other_tempos() {
    let mut fixture=RecordedFixture::new();assert_eq!(fixture.project.clips.len(),2);
    let api=fixture.api();let plan=api.timeline(&fixture.project).unwrap();assert_eq!(plan.total_frames,5600);
    let mut engine=SynthEngine::from_plan(plan).unwrap();
    for i in 0..5600 {
        let value=engine.next_frame();
        if (30..1570).contains(&i) || (3230..5570).contains(&i){assert!((value[0]-fixture.original[i]*std::f32::consts::FRAC_1_SQRT_2).abs()<1e-6);}
        if (1600..3200).contains(&i){assert_eq!(value,[0.,0.]);}
        if i==500 {
            engine.command(Command::Pause(true));assert_eq!(engine.next_frame(),[0.,0.]);assert_eq!(engine.frame(),501);
            engine.command(Command::Pause(false));
        }
    }
    engine.next_frame();assert!(engine.finished());
    fixture.project.session_bpm=60.;assert_eq!(fixture.project.clip_seconds(&fixture.project.clips[0]),0.2);
    assert_eq!(api.timeline(&fixture.project).unwrap().total_frames,5600);
    fixture.project.clips[1].muted=true;assert_eq!(api.timeline(&fixture.project).unwrap().total_frames,1600);
    fixture.project.tracks[0].muted=true;assert_eq!(api.timeline(&fixture.project).unwrap().total_frames,0);
}
#[test]
fn recorded_clips_split_independently_roundtrip_with_assets_and_export_real_audio() {
    let mut fixture=RecordedFixture::new();fixture.project.selected_clip=Some(fixture.project.clips[0].id);
    let before=fixture.project.clone();assert!(fixture.project.split_selected_recording(0.01).is_err());assert_eq!(fixture.project,before);
    fixture.project.split_selected_recording(0.1).unwrap();assert_eq!(fixture.project.clips.len(),3);
    assert_eq!(fixture.project.library[0].capture.as_ref().unwrap().duration_seconds,0.2);
    assert_eq!(fixture.project.clips[2].sound.capture.as_ref().unwrap().start_seconds,0.1);
    fixture.project.validate().unwrap();
    let bytes=crate::storage::encode(&fixture.project).unwrap();assert_eq!(crate::storage::decode(&bytes).unwrap(),fixture.project);
    let file=fixture.dir.join("music.overtone");crate::persistence::save(&fixture.project,&file).unwrap();
    std::fs::remove_file(&fixture.project.sources[0].path).unwrap();
    let loaded=crate::persistence::load(&file).unwrap();assert!(loaded.missing_sources.is_empty());
    let output=fixture.dir.join("music.wav");fixture.api().export_wav(&loaded.project,&output).unwrap();
    let mut wav=hound::WavReader::open(output).unwrap();assert_eq!(wav.duration(),5600);
    let samples=wav.samples::<i16>().map(Result::unwrap).collect::<Vec<_>>();
    assert!((samples[500*2] as f32/32767.-fixture.original[500]*std::f32::consts::FRAC_1_SQRT_2).abs()<0.00005);
    let source=&loaded.project.sources[0].path;std::fs::write(source,b"changed").unwrap();
    assert!(fixture.api().timeline(&loaded.project).err().unwrap().contains("changed"));
}
#[test]
fn recorded_samples_resample_and_render_without_allocating_or_freeing_buffers() {
    let fixture=RecordedFixture::new();
    let api=SynthApi::new(EngineConfig{sample_rate:48000,master_gain:1.,..Default::default()},fixture.project.tuning.clone()).unwrap();
    let mut engine=SynthEngine::from_plan(api.timeline(&fixture.project).unwrap()).unwrap();
    let mut output=[0.;512];ALLOCATIONS.with(|n|n.set(0));COUNTING.with(|c|c.set(true));
    for _ in 0..140{engine.render(&mut output);}
    COUNTING.with(|c|c.set(false));assert_eq!(ALLOCATIONS.with(|n|n.get()),0);assert!(engine.finished());
    let sample=api.recorded_sound(&fixture.project,&fixture.project.library[1]).unwrap();
    let mut engine=SynthEngine::new(api.config()).unwrap();engine.command(Command::Sample(sample));
    for _ in 0..1800{engine.next_frame();}
    let at=3200+1800/6;let value=engine.next_frame()[0];
    assert!((value-fixture.original[at]*std::f32::consts::FRAC_1_SQRT_2).abs()<1e-6);
    engine.command(Command::Stop);assert!(engine.finished());
}
