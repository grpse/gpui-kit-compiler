// Cross-platform audio device wrapper. GPUI never enters the DSP callback.
use crate::{
    engine::*,
    model::{Id, Project, Sound, Tuning},
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
const QUEUE: usize = 128;
struct Status {
    failed: AtomicBool,
    finished: AtomicBool,
    dropped: AtomicU64,
    frame: AtomicU64,
}
pub struct AudioSession {
    _stream: cpal::Stream,
    commands: rtrb::Producer<Command>,
    api: SynthApi,
    status: Arc<Status>,
    next_id: u64,
    audition_sample:Option<PreparedSample>,
}
impl AudioSession {
    /// Device negotiation and plan compilation run on the control thread before stream creation.
    pub fn start(project: Option<&Project>, tuning: Tuning) -> Result<Self, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No audio output device available")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let config: cpal::StreamConfig = supported.config();
        if config.channels == 0 {
            return Err("Audio device has no output channels".into());
        }
        let api = SynthApi::new(
            EngineConfig {
                sample_rate: config.sample_rate,
                voices: 32,
                ..Default::default()
            },
            tuning,
        )?;
        let engine = if let Some(project) = project {
            SynthEngine::from_plan(api.timeline(project)?)?
        } else {
            SynthEngine::new(api.config())?
        };
        let (commands, consumer) = rtrb::RingBuffer::new(QUEUE);
        let status = Arc::new(Status {
            failed: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            dropped: AtomicU64::new(0),
            frame: AtomicU64::new(0),
        });
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                stream::<f32>(&device, &config, engine, consumer, status.clone())
            }
            cpal::SampleFormat::F64 => {
                stream::<f64>(&device, &config, engine, consumer, status.clone())
            }
            cpal::SampleFormat::I16 => {
                stream::<i16>(&device, &config, engine, consumer, status.clone())
            }
            cpal::SampleFormat::U16 => {
                stream::<u16>(&device, &config, engine, consumer, status.clone())
            }
            cpal::SampleFormat::I32 => {
                stream::<i32>(&device, &config, engine, consumer, status.clone())
            }
            other => return Err(format!("Unsupported device sample format: {other:?}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            _stream: stream,
            commands,
            api,
            status,
            next_id: 1,
            audition_sample:None,
        })
    }
    pub fn trigger(
        &mut self,
        sound: &Sound,
        owner: Id,
        midi: u8,
        seconds: f64,
    ) -> Result<(), String> {
        let note = self.api.note(
            sound,
            NoteRequest {
                voice_id: self.next_id,
                owner,
                midi,
                velocity: 0.8,
                gate_seconds: seconds,
                gain: 1.,
                pan: 0.,
            },
        )?;
        self.next_id = self.next_id.wrapping_add(1);
        self.send(Command::Note(note))
    }
    pub fn audition_recording(&mut self, project:&Project,sound:&Sound)->Result<(),String> {
        let sample=self.api.recorded_sound(project,sound)?;
        self.audition_sample=Some(sample.clone());
        self.send(Command::Sample(sample))
    }
    pub fn audition(&mut self, sound: &Sound, owner: Id) -> Result<(), String> {
        let capture = sound.capture.as_ref();
        let hz = capture.and_then(|c| c.fundamental_hz).unwrap_or_else(||
            self.api.tuning().reference_hz * 2_f64.powf((60. - self.api.tuning().reference_midi as f64) / 12.));
        let seconds = capture.map(|c| (c.duration_seconds - sound.envelope.release_ms as f64 / 1000.).max(0.05)).unwrap_or(2.);
        let note = self.api.note_frequency(sound, NoteRequest {
            voice_id: self.next_id, owner, midi: 60, velocity: 0.8, gate_seconds: seconds, gain: 1., pan: 0.,
        }, hz)?;
        self.next_id = self.next_id.wrapping_add(1);
        self.send(Command::Note(note))
    }
    /// Existing voices owned by this draft/clip receive only that sound's edits.
    pub fn update_sound(&mut self, owner: Id, sound: &Sound) -> Result<(), String> {
        let patch = self.api.patch(sound)?;
        self.send(Command::Update { owner, patch })
    }
    pub fn set_paused(&mut self, paused: bool) -> Result<(), String> { self.send(Command::Pause(paused)) }
    pub fn elapsed_seconds(&self) -> f64 { self.status.frame.load(Ordering::Relaxed) as f64 / self.api.config().sample_rate as f64 }
    pub fn stop(&mut self) -> Result<(), String> {
        self.send(Command::Stop)
    }
    pub fn finished(&self) -> bool {
        self.status.finished.load(Ordering::Relaxed)
    }
    pub fn failed(&self) -> bool {
        self.status.failed.load(Ordering::Relaxed)
    }
    pub fn dropped_notes(&self) -> u64 {
        self.status.dropped.load(Ordering::Relaxed)
    }
    fn send(&mut self, command: Command) -> Result<(), String> {
        if self.failed() {
            return Err("Audio device stream failed; restart playback".into());
        }
        self.commands
            .push(command)
            .map_err(|_| "Audio command queue full; retry the edit".into())
    }
}
fn stream<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut engine: SynthEngine,
    mut commands: rtrb::Consumer<Command>,
    status: Arc<Status>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels as usize;
    let failed = status.clone();
    device
        .build_output_stream(
            *config,
            move |output: &mut [T], _| {
                // Bounded work: at most eight commands, then a fixed voice/partial bank per frame.
                for _ in 0..8 {
                    let Ok(command) = commands.pop() else {
                        break;
                    };
                    engine.command(command);
                }
                for frame in output.chunks_exact_mut(channels) {
                    let stereo = engine.next_frame();
                    if channels == 1 {
                        frame[0] = T::from_sample(
                            (stereo[0] + stereo[1]) * std::f32::consts::FRAC_1_SQRT_2,
                        );
                    } else {
                        frame[0] = T::from_sample(stereo[0]);
                        frame[1] = T::from_sample(stereo[1]);
                        for channel in &mut frame[2..] {
                            *channel = T::from_sample(0.);
                        }
                    }
                }
                status.frame.store(engine.frame(), Ordering::Relaxed);
                status.finished.store(engine.finished(), Ordering::Relaxed);
                status
                    .dropped
                    .store(engine.dropped_notes, Ordering::Relaxed);
            },
            move |_| {
                failed.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| e.to_string())
}
