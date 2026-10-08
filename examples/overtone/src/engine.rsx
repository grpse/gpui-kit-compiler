// Device-independent additive DSP and a validated adapter for the saved UI model.
use crate::model::{Id, MAX_HARMONICS, NoiseColor, Project, Sound, Tuning};
use std::{collections::HashMap, f64::consts::TAU, io::Write, path::Path, sync::Arc};
pub const MAX_VOICES: usize = 64;
pub const MAX_EVENTS: usize = 65_536;
const TABLE: usize = 4096;
#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub sample_rate: u32,
    pub voices: usize,
    pub master_gain: f32,
}
impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            voices: 32,
            master_gain: 0.25,
        }
    }
}
impl EngineConfig {
    pub fn validate(self) -> Result<Self, String> {
        if !(8000..=192000).contains(&self.sample_rate)
            || !(1..=MAX_VOICES).contains(&self.voices)
            || !self.master_gain.is_finite()
            || !(0.0..=1.0).contains(&self.master_gain)
        {
            Err("Invalid audio configuration".into())
        } else {
            Ok(self)
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Patch {
    amplitude: [f32; MAX_HARMONICS],
    ratio: [f64; MAX_HARMONICS],
    phase: [f64; MAX_HARMONICS],
    noise: f32,
    color: NoiseColor,
    seed: u64,
    attack: u64,
    decay: u64,
    sustain: f32,
    release: u64,
    gain: f32,
    normalization: f32,
    filter: f32,
    sample_rate: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct NoteRequest {
    pub voice_id: u64,
    pub owner: Id,
    pub midi: u8,
    pub velocity: f32,
    pub gate_seconds: f64,
    pub gain: f32,
    pub pan: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct PreparedNote {
    pub(crate) patch: Patch,
    pub(crate) step: [f64; MAX_HARMONICS],
    pub(crate) weight: [f32; MAX_HARMONICS],
    pub(crate) request: NoteRequest,
    pub(crate) gate: u64,
    pub(crate) frequency: f64,
    pub(crate) stereo: [f32; 2],
    rotation: [[f64; 2]; MAX_HARMONICS],
    initial: [[f64; 2]; MAX_HARMONICS],
}
#[derive(Clone, Debug)]
pub struct PreparedSample {
    samples:Arc<[f32]>, start:usize, end:usize, step:f64,
    frames:u64, stereo:[f32;2], gain:f32, sample_rate:u32,
}
struct ScheduledSample {frame:u64, sample:Option<PreparedSample>}
#[derive(Clone)]
struct SampleVoice {sample:PreparedSample, age:u64}
#[derive(Clone)]
pub enum Command {
    Sample(PreparedSample),
    Note(PreparedNote),
    Update { owner: Id, patch: Patch },
    Pause(bool),
    Stop,
}
#[derive(Clone, Copy, Debug)]
pub struct ScheduledNote {
    pub frame: u64,
    pub note: PreparedNote,
}
pub struct RenderPlan {
    events: Vec<ScheduledNote>,
    samples: Vec<ScheduledSample>,
    // Keep decoded buffers alive until the plan is dropped on the control thread.
    _source_buffers:Vec<Arc<[f32]>>,
    pub total_frames: u64,
    config: EngineConfig,
}
/// Public boundary: validation, conversions, and transcendental math happen here, off the audio callback.
#[derive(Clone)]
pub struct SynthApi {
    config: EngineConfig,
    tuning: Tuning,
}
impl SynthApi {
    pub fn config(&self) -> EngineConfig {
        self.config
    }
    pub fn tuning(&self) -> &Tuning {
        &self.tuning
    }
    pub fn new(config: EngineConfig, tuning: Tuning) -> Result<Self, String> {
        let config = config.validate()?;
        if tuning.reference_midi > 127
            || !tuning.reference_hz.is_finite()
            || !(20.0..=20000.0).contains(&tuning.reference_hz)
        {
            return Err("Invalid tuning".into());
        }
        Ok(Self { config, tuning })
    }
    pub fn patch(&self, sound: &Sound) -> Result<Patch, String> {
        fn valid(v: f32, lo: f32, hi: f32) -> bool {
            v.is_finite() && (lo..=hi).contains(&v)
        }
        if sound.harmonics.is_empty()
            || sound.harmonics.len() > MAX_HARMONICS
            || !valid(sound.noise.level, 0., 1.)
            || !valid(sound.gain, 0., 1.)
            || !valid(sound.brightness, 0., 1.)
            || !valid(sound.envelope.attack_ms, 0., 10000.)
            || !valid(sound.envelope.decay_ms, 0., 10000.)
            || !valid(sound.envelope.sustain, 0., 1.)
            || !valid(sound.envelope.release_ms, 0., 10000.)
        {
            return Err("Invalid synthesis parameters".into());
        }
        let fs = self.config.sample_rate;
        let frames = |ms: f32| ((ms as f64 * fs as f64 / 1000.).round() as u64).max(1);
        let mut p = Patch {
            amplitude: [0.; MAX_HARMONICS],
            ratio: [0.; MAX_HARMONICS],
            phase: [0.; MAX_HARMONICS],
            noise: sound.noise.level,
            color: sound.noise.color,
            seed: sound.noise.seed,
            attack: frames(sound.envelope.attack_ms).max(fs as u64 / 1000),
            decay: frames(sound.envelope.decay_ms),
            sustain: sound.envelope.sustain,
            release: frames(sound.envelope.release_ms).max(fs as u64 / 1000),
            gain: sound.gain,
            normalization: 1.,
            filter: 0.,
            sample_rate: fs,
        };
        let mut sum = sound.noise.level;
        for (i, h) in sound.harmonics.iter().enumerate() {
            if !(1..=MAX_HARMONICS as u8).contains(&h.multiple) || (i>0 && sound.harmonics[i-1].multiple>=h.multiple)
                || !valid(h.amplitude, 0., 1.)
                || !valid(h.phase, -180., 180.)
                || !valid(h.detune, -100., 100.)
            {
                return Err("Invalid harmonic".into());
            }
            // Fixed slots follow harmonic numbers, so deleting a row preserves other
            // active oscillators' phase and frequency during live patch updates.
            let slot=(h.multiple-1) as usize;
            p.amplitude[slot] = h.amplitude;
            p.ratio[slot] = h.multiple as f64 * 2_f64.powf(h.detune as f64 / 1200.);
            p.phase[slot] = h.phase as f64 / 360.;
            sum += h.amplitude;
        }
        p.normalization = 1. / sum.max(1.);
        let cutoff = (100. * 180_f64.powf(sound.brightness as f64)).min(fs as f64 * 0.45);
        p.filter = (1. - (-TAU * cutoff / fs as f64).exp()) as f32;
        Ok(p)
    }
    pub fn note(&self, sound: &Sound, request: NoteRequest) -> Result<PreparedNote, String> {
        self.prepare(self.patch(sound)?, request)
    }
    fn prepare(&self, patch: Patch, r: NoteRequest) -> Result<PreparedNote, String> {
        let hz = self.tuning.reference_hz
            * 2_f64.powf((r.midi as f64 - self.tuning.reference_midi as f64) / 12.);
        self.prepare_frequency(patch, r, hz)
    }
    /// Audition a captured timbre at its measured frequency without MIDI rounding.
    pub fn note_frequency(
        &self,
        sound: &Sound,
        request: NoteRequest,
        hz: f64,
    ) -> Result<PreparedNote, String> {
        self.prepare_frequency(self.patch(sound)?, request, hz)
    }
    fn prepare_frequency(
        &self,
        patch: Patch,
        r: NoteRequest,
        hz: f64,
    ) -> Result<PreparedNote, String> {
        if r.midi > 127
            || !hz.is_finite()
            || hz <= 0.0
            || hz > 100_000_000.0
            || !r.velocity.is_finite()
            || !(0.0..=1.0).contains(&r.velocity)
            || !r.gain.is_finite()
            || !(0.0..=1.0).contains(&r.gain)
            || !r.pan.is_finite()
            || !(-1.0..=1.0).contains(&r.pan)
            || !r.gate_seconds.is_finite()
            || !(0.0..=86400.0).contains(&r.gate_seconds)
        {
            return Err("Invalid note parameters".into());
        }
        let (step, weight) = oscillators(patch, hz);
        let angle = (r.pan as f64 + 1.) * std::f64::consts::FRAC_PI_4;
        Ok(PreparedNote {
            patch,
            step,
            weight,
            request: r,
            gate: (r.gate_seconds * self.config.sample_rate as f64).round() as u64,
            frequency: hz,
            stereo: [angle.cos() as f32, angle.sin() as f32],
            initial: patch.phase.map(|p| {
                let (s, c) = (TAU * p).sin_cos();
                [s, c]
            }),
            rotation: step.map(|s| {
                let (sin, cos) = (TAU * s).sin_cos();
                [sin, cos]
            }),
        })
    }
    /// Decode and verify off the device thread, then share the source between its clips.
    fn sample_source(project:&Project,sound:&Sound)->Result<(Arc<[f32]>,u32),String> {
        let source=project.sources.iter().find(|s|Some(s.id)==sound.source).ok_or("Recorded sound has no source")?;
        let current=crate::persistence::import_source(&source.path)?;
        if current.sha256!=source.sha256 || current.bytes!=source.bytes{return Err("Recording changed; import it again".into());}
        let recording=crate::analysis::decode(&source.path)?;
        Ok((recording.samples.into(),recording.sample_rate))
    }
    fn prepare_sample(&self,sound:&Sound,samples:Arc<[f32]>,rate:u32,gain:f32,pan:f32)->Result<PreparedSample,String> {
        let bounds=sound.capture.as_ref().ok_or("Recorded sound has no bounds")?;
        if !sound.recorded_sample || !gain.is_finite() || !(0.0..=1.0).contains(&gain)
            || !pan.is_finite() || !(-1.0..=1.0).contains(&pan) || !bounds.start_seconds.is_finite()
            || !bounds.duration_seconds.is_finite() || bounds.start_seconds<0. || bounds.duration_seconds<=0. {
            return Err("Invalid recorded sound".into());
        }
        let start=(bounds.start_seconds*rate as f64).round() as usize;
        let end=((bounds.start_seconds+bounds.duration_seconds)*rate as f64).round() as usize;
        if start>=end || end>samples.len(){return Err("Sound bounds exceed the recording".into());}
        let angle=(pan as f64+1.)*std::f64::consts::FRAC_PI_4;
        Ok(PreparedSample{samples,start,end,step:rate as f64/self.config.sample_rate as f64,
            frames:((end-start) as f64*self.config.sample_rate as f64/rate as f64).ceil() as u64,
            stereo:[angle.cos() as f32,angle.sin() as f32],gain:gain*sound.gain,sample_rate:self.config.sample_rate})
    }
    pub fn recorded_sound(&self,project:&Project,sound:&Sound)->Result<PreparedSample,String> {
        let (samples,rate)=Self::sample_source(project,sound)?;
        self.prepare_sample(sound,samples,rate,1.,0.)
    }
    pub fn timeline(&self, project: &Project) -> Result<RenderPlan, String> {
        project.validate()?;
        if project.tuning != self.tuning {
            return Err("Engine tuning differs from project".into());
        }
        let mut events = Vec::new();
        let mut samples=Vec::new();
        let mut sources=HashMap::new();
        let mut endpoints = Vec::new();
        let mut end = 0;
        let solo = project.tracks.iter().any(|t| t.solo);
        for clip in &project.clips {
            let track = project
                .tracks
                .iter()
                .find(|t| t.id == clip.track)
                .ok_or("Missing track")?;
            if clip.muted || track.muted || solo && !track.solo {
                continue;
            }
            if clip.sound.recorded_sample {
                if samples.len()+events.len()>=MAX_EVENTS{return Err("Timeline exceeds 65,536 sounds".into());}
                let source_id=clip.sound.source.ok_or("Recorded clip has no source")?;
                if !sources.contains_key(&source_id){sources.insert(source_id,Self::sample_source(project,&clip.sound)?);}
                let (source,rate)=sources.get(&source_id).unwrap();
                let sample=self.prepare_sample(&clip.sound,source.clone(),*rate,clip.gain*track.gain,track.pan)?;
                let frame=(clip.start_seconds*self.config.sample_rate as f64).round() as u64;
                let finish=frame+sample.frames;end=end.max(finish);
                endpoints.push((frame,1_i32));endpoints.push((finish,-1_i32));
                samples.push(ScheduledSample{frame,sample:Some(sample)});continue;
            }
            let patch = self.patch(&clip.sound)?;
            let seconds_per_beat = 60. / project.tempo(track.id);
            for note in &clip.notes {
                if events.len()+samples.len() >= MAX_EVENTS {
                    return Err("Timeline exceeds 65,536 playable notes".into());
                }
                let frame = ((clip.start_seconds + note.start_beat * seconds_per_beat)
                    * self.config.sample_rate as f64)
                    .round() as u64;
                let request = NoteRequest {
                    voice_id: events.len() as u64 + 1,
                    owner: clip.id,
                    midi: note.midi,
                    velocity: note.velocity,
                    gate_seconds: note.duration_beats * seconds_per_beat,
                    gain: clip.gain * track.gain,
                    pan: track.pan,
                };
                let prepared = self.prepare(patch, request)?;
                let finish = frame + prepared.gate + patch.release;
                end = end.max(finish);
                endpoints.push((frame, 1_i32));
                endpoints.push((finish, -1_i32));
                events.push(ScheduledNote {
                    frame,
                    note: prepared,
                });
            }
        }
        // Release tails count towards polyphony; reject rather than silently drop notes.
        endpoints.sort_unstable();
        let mut active = 0;
        for (_, delta) in endpoints {
            active += delta;
            if active > self.config.voices as i32 {
                return Err(format!(
                    "Timeline needs more than {} simultaneous voices (including release tails)",
                    self.config.voices
                ));
            }
        }
        events.sort_by_key(|e| e.frame);
        samples.sort_by_key(|e|e.frame);
        Ok(RenderPlan {
            events, samples, _source_buffers:sources.into_values().map(|(samples,_)|samples).collect(),
            total_frames: end,
            config: self.config,
        })
    }
    /// Streams PCM16 stereo WAV through a bounded block buffer; does not allocate the whole song.
    pub fn export_wav(&self, project: &Project, path: &Path) -> Result<(), String> {
        let plan = self.timeline(project)?;
        let bytes = plan
            .total_frames
            .checked_mul(4)
            .filter(|b| *b <= u32::MAX as u64 - 36)
            .ok_or("WAV exceeds RIFF size limit")? as u32;
        let mut engine = SynthEngine::from_plan(plan)?;
        static EXPORT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let temp = path.with_extension(format!(
            "{}.{}.tmp",
            std::process::id(),
            EXPORT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let write = (|| -> std::io::Result<()> {
            let mut file = std::io::BufWriter::new(file);
            file.write_all(b"RIFF")?;
            file.write_all(&(bytes + 36).to_le_bytes())?;
            file.write_all(b"WAVEfmt ")?;
            file.write_all(&16_u32.to_le_bytes())?;
            file.write_all(&1_u16.to_le_bytes())?;
            file.write_all(&2_u16.to_le_bytes())?;
            file.write_all(&self.config.sample_rate.to_le_bytes())?;
            file.write_all(&(self.config.sample_rate * 4).to_le_bytes())?;
            file.write_all(&4_u16.to_le_bytes())?;
            file.write_all(&16_u16.to_le_bytes())?;
            file.write_all(b"data")?;
            file.write_all(&bytes.to_le_bytes())?;
            let mut audio = [0_f32; 512];
            let mut pcm = [0_u8; 1024];
            let mut remaining = bytes as u64 / 4;
            while remaining > 0 {
                let frames = remaining.min(256) as usize;
                engine.render(&mut audio[..frames * 2]);
                for (i, sample) in audio[..frames * 2].iter().enumerate() {
                    pcm[i * 2..i * 2 + 2].copy_from_slice(
                        &((sample.clamp(-1., 1.) * 32767.).round() as i16).to_le_bytes(),
                    );
                }
                file.write_all(&pcm[..frames * 4])?;
                remaining -= frames as u64;
            }
            file.flush()?;
            file.get_ref().sync_all()?;
            drop(file);
            std::fs::rename(&temp, path)?;
            Ok(())
        })();
        if let Err(error) = write {
            let _ = std::fs::remove_file(&temp);
            return Err(error.to_string());
        }
        Ok(())
    }
}
fn lookup(table: &[f32; TABLE + 1], mut phase: f64) -> f32 {
    if phase < 0. {
        phase += 1.;
    } else if phase >= 1. {
        phase -= 1.;
    }
    let index = phase * TABLE as f64;
    let base = index as usize;
    let fraction = (index - base as f64) as f32;
    table[base] + (table[base + 1] - table[base]) * fraction
}
fn oscillators(p: Patch, hz: f64) -> ([f64; MAX_HARMONICS], [f32; MAX_HARMONICS]) {
    let mut step = [0.; MAX_HARMONICS];
    let mut weight = [0.; MAX_HARMONICS];
    for i in 0..MAX_HARMONICS {
        let f = hz * p.ratio[i] / p.sample_rate as f64;
        if f < 0.5 {
            step[i] = f;
            weight[i] = p.amplitude[i] * ((0.5 - f) / 0.05).clamp(0., 1.) as f32;
        }
    }
    (step, weight)
}
#[derive(Clone, Copy)]
struct Voice {
    note: PreparedNote,
    phase: [f64; MAX_HARMONICS],
    step: [f64; MAX_HARMONICS],
    weight: [f32; MAX_HARMONICS],
    phase_offset: [f64; MAX_HARMONICS],
    age: u64,
    release_age: u64,
    release_start: f32,
    gain: f32,
    noise_level: f32,
    normalization: f32,
    filter: f32,
    filtered: f32,
    rng: u64,
    pink: [f32; 8],
    pink_counter: u32,
    brown: f32,
    releasing: bool,
    smoothing: u32,
    color_mix: [f32; 3],
    sustain: f32,
    sine: [f64; MAX_HARMONICS],
    cosine: [f64; MAX_HARMONICS],
}
impl Voice {
    fn new(note: PreparedNote) -> Self {
        let patch = note.patch;
        Self {
            note,
            phase: [0.; MAX_HARMONICS],
            step: note.step,
            weight: note.weight,
            phase_offset: patch.phase,
            age: 0,
            release_age: 0,
            release_start: 0.,
            gain: patch.gain,
            noise_level: patch.noise,
            normalization: patch.normalization,
            filter: patch.filter,
            filtered: 0.,
            rng: if patch.seed == 0 {
                0x9e3779b97f4a7c15
            } else {
                patch.seed
            },
            pink: [0.; 8],
            pink_counter: 0,
            brown: 0.,
            releasing: false,
            smoothing: 0,
            sustain: patch.sustain,
            sine: note.initial.map(|p| p[0]),
            cosine: note.initial.map(|p| p[1]),
            color_mix: match patch.color {
                NoiseColor::White => [1., 0., 0.],
                NoiseColor::Pink => [0., 1., 0.],
                NoiseColor::Brown => [0., 0., 1.],
            },
        }
    }
    fn envelope(&self) -> f32 {
        let p = self.note.patch;
        if self.age < p.attack {
            self.age as f32 / p.attack as f32
        } else if self.age < p.attack + p.decay {
            1. - (1. - self.sustain) * (self.age - p.attack) as f32 / p.decay as f32
        } else {
            self.sustain
        }
    }
    fn white(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / 8388608. - 1.
    }
    fn sample(&mut self, table: &[f32; TABLE + 1], smooth: f32, brown_decay: f32) -> Option<f32> {
        if !self.releasing && self.age >= self.note.gate {
            self.release_start = self.envelope();
            self.releasing = true;
        }
        if self.releasing && self.release_age >= self.note.patch.release {
            return None;
        }
        let envelope = if self.releasing {
            self.release_start * (1. - self.release_age as f32 / self.note.patch.release as f32)
        } else {
            self.envelope()
        };
        let p = self.note.patch;
        let mut value = 0.;
        for i in 0..MAX_HARMONICS {
            if self.smoothing > 0 {
                self.step[i] += (self.note.step[i] - self.step[i]) * smooth as f64;
                self.weight[i] += (self.note.weight[i] - self.weight[i]) * smooth;
                let diff = p.phase[i] - self.phase_offset[i];
                let shortest = if diff > 0.5 {
                    diff - 1.
                } else if diff < -0.5 {
                    diff + 1.
                } else {
                    diff
                };
                self.phase_offset[i] += shortest * smooth as f64;
                if self.phase_offset[i] < -0.5 {
                    self.phase_offset[i] += 1.;
                } else if self.phase_offset[i] > 0.5 {
                    self.phase_offset[i] -= 1.;
                }
            }
            if self.smoothing == 0 {
                if self.weight[i].abs() > 1e-8 {
                    value += self.sine[i] as f32 * self.weight[i];
                }
                let [sin_step, cos_step] = self.note.rotation[i];
                let sine = self.sine[i];
                let cosine = self.cosine[i];
                self.sine[i] = sine * cos_step + cosine * sin_step;
                self.cosine[i] = cosine * cos_step - sine * sin_step;
            } else if self.weight[i].abs() > 1e-8 {
                value += lookup(table, self.phase[i] + self.phase_offset[i]) * self.weight[i];
            }
            self.phase[i] += self.step[i];
            if self.phase[i] >= 1. {
                self.phase[i] -= 1.;
            }
        }
        if self.smoothing > 0 {
            self.noise_level += (p.noise - self.noise_level) * smooth;
            self.gain += (p.gain - self.gain) * smooth;
            self.normalization += (p.normalization - self.normalization) * smooth;
            self.filter += (p.filter - self.filter) * smooth;
            self.sustain += (p.sustain - self.sustain) * smooth;
            let target = match p.color {
                NoiseColor::White => [1., 0., 0.],
                NoiseColor::Pink => [0., 1., 0.],
                NoiseColor::Brown => [0., 0., 1.],
            };
            for (current, target) in self.color_mix.iter_mut().zip(target) {
                *current += (target - *current) * smooth;
            }
            self.smoothing -= 1;
            if self.smoothing == 0 {
                self.step = self.note.step;
                self.weight = self.note.weight;
                self.phase_offset = p.phase;
                self.gain = p.gain;
                self.noise_level = p.noise;
                self.filter = p.filter;
                self.sustain = p.sustain;
                self.normalization = p.normalization;
                self.color_mix = target;
                for i in 0..MAX_HARMONICS {
                    self.sine[i] = lookup(table, self.phase[i] + p.phase[i]) as f64;
                    self.cosine[i] = lookup(table, self.phase[i] + p.phase[i] + 0.25) as f64;
                }
            }
        }
        if self.noise_level > 1e-8 {
            let white = self.white();
            self.pink_counter = self.pink_counter.wrapping_add(1);
            let index = self.pink_counter.trailing_zeros() as usize;
            if index < 8 {
                self.pink[index] = white;
            }
            let pink = (self.pink.iter().sum::<f32>() + self.white()) / 9.;
            self.brown = (self.brown * brown_decay + white * 0.025).clamp(-1., 1.);
            let noise = white * self.color_mix[0]
                + pink * self.color_mix[1]
                + self.brown * self.color_mix[2];
            value += noise * self.noise_level;
        }
        self.filtered += self.filter * (value - self.filtered);
        self.age += 1;
        if self.releasing {
            self.release_age += 1;
        }
        Some(
            self.filtered
                * self.normalization
                * self.gain
                * envelope
                * self.note.request.velocity
                * self.note.request.gain,
        )
    }
}
/// Own this object on the audio thread. Render and command application never allocate or lock.
pub struct SynthEngine {
    config: EngineConfig,
    voices: Box<[Option<Voice>; MAX_VOICES]>,
    sample_voices: Box<[Option<SampleVoice>; MAX_VOICES]>,
    next_sample:usize,
    table: Box<[f32; TABLE + 1]>,
    smooth: f32,
    brown_decay: f32,
    frame: u64,
    plan: Option<RenderPlan>,
    next_event: usize,
    stopped: bool,
    paused: bool,
    pub dropped_notes: u64,
}
impl SynthEngine {
    pub fn new(config: EngineConfig) -> Result<Self, String> {
        let config = config.validate()?;
        let mut table = Box::new([0.; TABLE + 1]);
        for (i, v) in table.iter_mut().enumerate() {
            *v = (TAU * i as f64 / TABLE as f64).sin() as f32;
        }
        Ok(Self {
            config,
            voices: vec![None; MAX_VOICES]
                .into_boxed_slice()
                .try_into()
                .ok()
                .unwrap(),
            sample_voices:vec![None;MAX_VOICES].into_boxed_slice().try_into().ok().unwrap(),
            next_sample:0,
            table,
            smooth: (1. - (-1. / (config.sample_rate as f64 * 0.005)).exp()) as f32,
            brown_decay: (-TAU * 20. / config.sample_rate as f64).exp() as f32,
            frame: 0,
            plan: None,
            next_event: 0,
            stopped: false,
            paused: false,
            dropped_notes: 0,
        })
    }
    pub fn from_plan(plan: RenderPlan) -> Result<Self, String> {
        let mut engine = Self::new(plan.config)?;
        engine.plan = Some(plan);
        Ok(engine)
    }
    pub fn command(&mut self, command: Command) {
        match command {
            Command::Sample(sample)=>{
                if sample.sample_rate!=self.config.sample_rate || self.active_voices()>=self.config.voices {self.dropped_notes+=1;return;}
                if let Some(slot)=self.sample_voices.iter_mut().find(|v|v.is_none()){*slot=Some(SampleVoice{sample,age:0});}
            }
            Command::Note(note) => {
                if note.patch.sample_rate != self.config.sample_rate || self.active_voices()>=self.config.voices {
                    self.dropped_notes += 1;
                    return;
                }
                if let Some(slot) = self.voices[..self.config.voices]
                    .iter_mut()
                    .find(|v| v.is_none())
                {
                    *slot = Some(Voice::new(note));
                } else {
                    self.dropped_notes += 1;
                }
            }
            Command::Update { owner, patch } => {
                if patch.sample_rate != self.config.sample_rate {
                    return;
                }
                for voice in self
                    .voices
                    .iter_mut()
                    .flatten()
                    .filter(|v| v.note.request.owner == owner)
                {
                    let old = voice.note.patch;
                    voice.note.patch = patch;
                    // Changing an attack/decay already in progress must not jump the envelope.
                    voice.note.patch.attack = old.attack;
                    voice.note.patch.decay = old.decay;
                    if voice.releasing {
                        voice.note.patch.release = old.release;
                    }
                    voice.smoothing = self.config.sample_rate / 20;
                    let (step, weight) = oscillators(patch, voice.note.frequency);
                    voice.note.step = step;
                    voice.note.rotation = step.map(|s| {
                        let sin = lookup(&self.table, s) as f64;
                        let cos = lookup(&self.table, s + 0.25) as f64;
                        let inv = 1. / (sin * sin + cos * cos).sqrt();
                        [sin * inv, cos * inv]
                    });
                    voice.note.weight = weight;
                }
            }
            Command::Pause(paused) => self.paused = paused,
            Command::Stop => {
                self.paused = false;
                self.stopped = true;
                for slot in &mut self.sample_voices { *slot=None; }
                for v in self.voices.iter_mut().flatten() {
                    v.release_start = v.envelope();
                    v.releasing = true;
                    v.release_age = 0;
                    v.note.patch.release = (self.config.sample_rate as u64 / 100).max(1);
                }
            }
        }
    }
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.is_some()).count()+self.sample_voices.iter().filter(|v|v.is_some()).count()
    }
    pub fn frame(&self) -> u64 {
        self.frame
    }
    pub fn finished(&self) -> bool {
        self.active_voices() == 0
            && (self.stopped
                || self
                    .plan
                    .as_ref()
                    .is_none_or(|p| self.next_event == p.events.len() && self.next_sample==p.samples.len()))
    }
    pub fn paused(&self) -> bool { self.paused }
    pub fn next_frame(&mut self) -> [f32; 2] {
        if self.paused { return [0.; 2]; }
        for slot in &mut self.sample_voices {
            if slot.as_ref().is_some_and(|v|v.age>=v.sample.frames){*slot=None;}
        }
        // Free completed releases before starting notes at exactly the same sample.
        for slot in &mut self.voices[..self.config.voices] {
            if slot
                .as_ref()
                .is_some_and(|v| v.releasing && v.release_age >= v.note.patch.release)
            {
                *slot = None;
            }
        }
        if !self.stopped {
            loop {
                let event = self.plan.as_ref().and_then(|p| {
                    p.events
                        .get(self.next_event)
                        .filter(|e| e.frame <= self.frame)
                        .copied()
                });
                let Some(event) = event else {
                    break;
                };
                self.command(Command::Note(event.note));
                self.next_event += 1;
            }
        }
        if !self.stopped {
            loop {
                let sample=self.plan.as_mut().and_then(|p|p.samples.get_mut(self.next_sample))
                    .filter(|e|e.frame<=self.frame).and_then(|e|e.sample.take());
                let Some(sample)=sample else{break;};
                self.command(Command::Sample(sample));self.next_sample+=1;
            }
        }
        let mut stereo = [0.; 2];
        for voice in self.sample_voices.iter_mut().flatten() {
            let sample=&voice.sample;
            let position=sample.start as f64+voice.age as f64*sample.step;
            let at=(position.floor() as usize).min(sample.end-1);
            let next=(at+1).min(sample.end-1);
            let value=sample.samples[at]+(sample.samples[next]-sample.samples[at])*(position-position.floor()) as f32;
            let fade=(self.config.sample_rate as f32*0.003).min(sample.frames as f32/2.).max(1.);
            let envelope=(voice.age as f32/fade).min((sample.frames-voice.age) as f32/fade).min(1.);
            for channel in 0..2{stereo[channel]+=value*envelope*sample.gain*sample.stereo[channel];}
            voice.age+=1;
        }
        for slot in &mut self.voices[..self.config.voices] {
            if let Some(voice) = slot {
                if let Some(sample) = voice.sample(&self.table, self.smooth, self.brown_decay) {
                    stereo[0] += sample * voice.note.stereo[0];
                    stereo[1] += sample * voice.note.stereo[1];
                } else {
                    *slot = None;
                }
            }
        }
        self.frame += 1;
        [
            (stereo[0] * self.config.master_gain).clamp(-1., 1.),
            (stereo[1] * self.config.master_gain).clamp(-1., 1.),
        ]
    }
    /// Interleaved stereo; an odd trailing sample is zeroed. Block boundaries never reset phase.
    pub fn render(&mut self, output: &mut [f32]) {
        for frame in output.chunks_exact_mut(2) {
            frame.copy_from_slice(&self.next_frame());
        }
        if output.len() % 2 != 0 {
            *output.last_mut().unwrap() = 0.;
        }
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("OUT_DIR"), "/engine_tests.rs"));
}
