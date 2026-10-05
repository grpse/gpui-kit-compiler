//! Disk-backed channel mixing and device-clock transport. No file I/O, locks, or
//! allocation in the normal audio data callback. Conversion is preview-only.
use crate::{
    preprocess::{PreparedMedia, SampleEncoding},
    state::{ClipComponent, EditorState},
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ffmpeg_next as ffmpeg;
use std::{
    collections::VecDeque,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub const AUDIO_QUEUE_FRAMES: usize = 8192;
pub const MIX_BLOCK_FRAMES: usize = 512;

#[derive(Clone)]
pub struct AudioRegion {
    pub media: Arc<PreparedMedia>,
    pub stream: usize,
    pub channel: usize,
    pub start: f64,
    pub duration: f64,
    pub source_start: f64,
    pub gain: f32,
    pub pan: [f32; 2],
}
#[derive(Clone)]
pub struct VideoRegion {
    pub asset: usize,
    pub track: usize,
    pub path: PathBuf,
    pub stream: usize,
    pub start: f64,
    pub duration: f64,
    pub source_start: f64,
    pub keyframes: Arc<crate::preprocess::KeyframeIndex>,
}
#[derive(Clone, Default)]
pub struct PlaybackPlan {
    pub audio: Vec<AudioRegion>,
    pub videos: Vec<VideoRegion>,
    pub end: f64,
}
fn pan(name: &str, index: usize, count: usize) -> [f32; 2] {
    match name {
        "FL" => [1., 0.],
        "FR" => [0., 1.],
        "BL" | "SL" | "FLC" => [std::f32::consts::FRAC_1_SQRT_2, 0.],
        "BR" | "SR" | "FRC" => [0., std::f32::consts::FRAC_1_SQRT_2],
        "LFE" => [0.5, 0.5],
        _ if count == 1 => [1., 1.],
        _ if count == 2 => {
            if index == 0 {
                [1., 0.]
            } else {
                [0., 1.]
            }
        }
        _ => [std::f32::consts::FRAC_1_SQRT_2; 2],
    }
}
impl PlaybackPlan {
    pub fn from_state(state: &EditorState) -> Self {
        let mut plan = Self::default();
        if state.screen.has_timeline() {
            plan.end = state.project_duration() as f64;
            for clip in &state.clips {
                let Some(track) = state.tracks.get(clip.track) else {
                    continue;
                };
                let Some(asset) = state.assets.get(clip.asset) else {
                    continue;
                };
                let Some(media) = &asset.prepared else {
                    continue;
                };
                match clip.component {
                    Some(ClipComponent::AudioChannel { stream, channel }) if !track.muted => {
                        if let Some(audio) = media.audio.iter().find(|audio| audio.index == stream)
                            && let Some(source) = audio.channels.get(channel)
                        {
                            plan.audio.push(AudioRegion {
                                media: media.clone(),
                                stream,
                                channel,
                                start: clip.start as f64,
                                duration: clip.length as f64,
                                source_start: clip.source_start as f64,
                                gain: track.gain * state.monitor_gain,
                                pan: pan(&source.name, channel, audio.channels.len()),
                            });
                        }
                    }
                    Some(ClipComponent::Video(stream)) if track.visible => {
                        if let Some(video) = media.videos.iter().find(|video| video.index == stream)
                            && let Some(path) = &asset.path
                        {
                            plan.videos.push(VideoRegion {
                                asset: clip.asset,
                                track: clip.track,
                                path: path.clone(),
                                stream,
                                start: clip.start as f64,
                                duration: clip.length as f64,
                                source_start: clip.source_start as f64,
                                keyframes: video.keyframes.clone(),
                            });
                        }
                    }
                    _ => {}
                }
            }
            // Highest visible video track wins; deterministic clip order breaks ties.
            plan.videos.sort_by_key(|video| video.track);
        } else if let Some(asset) = state.assets.get(state.selected) {
            plan.end = asset.duration as f64;
            if let Some(media) = &asset.prepared {
                // Source preview monitors the first audio stream. Timeline tracks can mix all streams.
                if let Some(audio) = media.audio.first() {
                    for channel in &audio.channels {
                        for span in &audio.spans {
                            let start = span
                                .pts
                                .map(|pts| audio.timing.time_base.seconds(pts))
                                .unwrap_or(
                                    audio.start_seconds()
                                        + span.first_sample as f64 / audio.sample_rate as f64,
                                )
                                - media.origin_seconds;
                            let duration = span.samples as f64 / audio.sample_rate as f64;
                            plan.end = plan.end.max(start + duration);
                            plan.audio.push(AudioRegion {
                                media: media.clone(),
                                stream: audio.index,
                                channel: channel.index,
                                start,
                                duration,
                                source_start: span.first_sample as f64 / audio.sample_rate as f64,
                                gain: state.monitor_gain,
                                pan: pan(&channel.name, channel.index, audio.channels.len()),
                            });
                        }
                    }
                }
                if let Some(video) = media
                    .videos
                    .iter()
                    .find(|video| Some(video.index) == media.primary_video)
                    && let Some(path) = &asset.path
                {
                    let start = video
                        .timing
                        .start_pts
                        .map(|pts| video.timing.time_base.seconds(pts))
                        .unwrap_or(media.origin_seconds)
                        - media.origin_seconds;
                    let duration = video
                        .timing
                        .duration_pts
                        .map(|pts| video.timing.time_base.seconds(pts))
                        .unwrap_or(asset.duration as f64);
                    plan.end = plan.end.max(start + duration);
                    plan.videos.push(VideoRegion {
                        asset: state.selected,
                        track: 0,
                        path: path.clone(),
                        stream: video.index,
                        start,
                        duration,
                        source_start: 0.,
                        keyframes: video.keyframes.clone(),
                    });
                }
            }
        }
        if state.muted {
            for region in &mut plan.audio {
                region.gain = 0.;
            }
        }
        plan
    }
    pub fn video_at(&self, time: f64) -> Option<(usize, &VideoRegion)> {
        self.videos
            .iter()
            .enumerate()
            .rev()
            .find(|(_, region)| time >= region.start && time < region.start + region.duration)
    }
}
fn format(encoding: SampleEncoding) -> ffmpeg::format::Sample {
    use ffmpeg::format::{Sample, sample::Type::Packed};
    match encoding {
        SampleEncoding::U8 => Sample::U8(Packed),
        SampleEncoding::I16 => Sample::I16(Packed),
        SampleEncoding::I32 => Sample::I32(Packed),
        SampleEncoding::I64 => Sample::I64(Packed),
        SampleEncoding::F32 => Sample::F32(Packed),
        SampleEncoding::F64 => Sample::F64(Packed),
    }
}
struct ChannelReader {
    file: File,
    encoding: SampleEncoding,
    rate: u32,
    output_rate: u32,
    remaining: u64,
    converter: ffmpeg::software::resampling::Context,
    ready: VecDeque<f32>,
    finished: bool,
}
impl ChannelReader {
    fn new(region: &AudioRegion, position: f64, output_rate: u32) -> Result<Self, String> {
        let audio = region
            .media
            .audio
            .iter()
            .find(|audio| audio.index == region.stream)
            .ok_or("Missing audio stream")?;
        let channel = audio
            .channels
            .get(region.channel)
            .ok_or("Missing audio channel")?;
        if !(1000..=768000).contains(&audio.sample_rate) {
            return Err("Unsupported playback sample rate (cache remains intact)".into());
        }
        let target = ((region.source_start + (position - region.start).max(0.))
            * audio.sample_rate as f64)
            .round() as u64;
        let span = audio
            .spans
            .iter()
            .find(|span| target >= span.first_sample && target < span.first_sample + span.samples);
        let lower = span.map(|span| span.first_sample).unwrap_or(target);
        let upper = span
            .map(|span| span.first_sample + span.samples)
            .unwrap_or(audio.samples);
        let first = target.saturating_sub(128).max(lower);
        let mut file = File::open(&channel.path).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(first * audio.encoding.bytes() as u64))
            .map_err(|e| e.to_string())?;
        let mut options = ffmpeg::Dictionary::new();
        options.set("filter_size", "64");
        options.set("phase_shift", "10");
        options.set("cutoff", "0.97");
        let converter = ffmpeg::software::resampling::Context::get_with(
            format(audio.encoding),
            ffmpeg::ChannelLayout::MONO,
            audio.sample_rate,
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            ffmpeg::ChannelLayout::MONO,
            output_rate,
            options,
        )
        .map_err(|e| e.to_string())?;
        let mut result = Self {
            file,
            encoding: audio.encoding,
            rate: audio.sample_rate,
            output_rate,
            remaining: upper.saturating_sub(first),
            converter,
            ready: VecDeque::with_capacity(4096),
            finished: false,
        };
        let discard = ((target - first) as f64 * output_rate as f64 / audio.sample_rate as f64)
            .round() as usize;
        for _ in 0..discard {
            result.next()?;
        }
        Ok(result)
    }
    fn next(&mut self) -> Result<f32, String> {
        loop {
            if let Some(sample) = self.ready.pop_front() {
                return Ok(if sample.is_finite() { sample } else { 0. });
            }
            if self.finished {
                return Ok(0.);
            }
            let count = self.remaining.min((self.rate as u64 / 100).clamp(32, 4096)) as usize;
            let capacity = ((count.max(32) as f64 * self.output_rate as f64 / self.rate as f64)
                .ceil() as usize
                + 256)
                .min(16384);
            let mut output = ffmpeg::frame::Audio::new(
                ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                capacity,
                ffmpeg::ChannelLayout::MONO,
            );
            output.set_rate(self.output_rate);
            if count > 0 {
                let mut input = ffmpeg::frame::Audio::new(
                    format(self.encoding),
                    count,
                    ffmpeg::ChannelLayout::MONO,
                );
                input.set_rate(self.rate);
                self.file
                    .read_exact(&mut input.data_mut(0)[..count * self.encoding.bytes()])
                    .map_err(|e| e.to_string())?;
                self.remaining -= count as u64;
                self.converter
                    .run(&input, &mut output)
                    .map_err(|e| e.to_string())?;
            } else {
                self.converter
                    .flush(&mut output)
                    .map_err(|e| e.to_string())?;
                if output.samples() == 0 {
                    self.finished = true;
                }
            }
            self.ready.extend(output.plane::<f32>(0).iter().copied());
        }
    }
}
/// Worker-only mixer; filter state persists across consecutive blocks and is reset on seek.
pub struct Mixer {
    regions: Vec<AudioRegion>,
    readers: Vec<Option<ChannelReader>>,
    start: f64,
    rate: u32,
    frame: u64,
}
impl Mixer {
    pub fn new(plan: &PlaybackPlan, start: f64, rate: u32) -> Self {
        Self {
            regions: plan.audio.clone(),
            readers: (0..plan.audio.len()).map(|_| None).collect(),
            start,
            rate,
            frame: 0,
        }
    }
    pub fn render(&mut self, out: &mut [[f32; 2]]) -> Result<(), String> {
        out.fill([0.; 2]);
        let time = self.start + self.frame as f64 / self.rate as f64;
        for (region, reader) in self.regions.iter().zip(&mut self.readers) {
            if time >= region.start + region.duration {
                *reader = None;
                continue;
            }
            if region.gain == 0. {
                continue;
            }
            let lo = (((region.start - time) * self.rate as f64 - 1e-6)
                .ceil()
                .max(0.) as usize)
                .min(out.len());
            let hi = (((region.start + region.duration - time) * self.rate as f64 - 1e-6)
                .ceil()
                .max(0.) as usize)
                .min(out.len());
            if lo >= hi {
                continue;
            }
            if reader.is_none() {
                *reader = Some(ChannelReader::new(
                    region,
                    time + lo as f64 / self.rate as f64,
                    self.rate,
                )?)
            }
            let reader = reader.as_mut().unwrap();
            for sample in &mut out[lo..hi] {
                let value = reader.next()? * region.gain;
                sample[0] += value * region.pan[0];
                sample[1] += value * region.pan[1];
            }
        }
        self.frame += out.len() as u64;
        Ok(())
    }
}

/// Time published by the device callback, adjusted using CPAL's predicted DAC latency.
pub struct PlaybackClock {
    origin: Instant,
    generation: AtomicU64,
    running: AtomicBool,
    ended: AtomicBool,
    failed: AtomicBool,
    base: AtomicU64,
    nanos: AtomicU64,
    ceiling: AtomicU64,
    pub rate: AtomicU32,
    pub consumed: AtomicU64,
    pub nonzero: AtomicU64,
    pub underruns: AtomicU64,
    pub clipped: AtomicU64,
}
impl PlaybackClock {
    fn new() -> Self {
        Self {
            origin: Instant::now(),
            generation: AtomicU64::new(0),
            running: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            base: AtomicU64::new(0),
            nanos: AtomicU64::new(0),
            ceiling: AtomicU64::new(0),
            rate: AtomicU32::new(0),
            consumed: AtomicU64::new(0),
            nonzero: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            clipped: AtomicU64::new(0),
        }
    }
    pub fn position(&self) -> f64 {
        let base = f64::from_bits(self.base.load(Ordering::Acquire));
        let elapsed =
            self.origin
                .elapsed()
                .as_nanos()
                .saturating_sub(self.nanos.load(Ordering::Acquire) as u128) as f64
                / 1e9;
        if !self.running.load(Ordering::Acquire) {
            return base;
        }
        (base + elapsed).min(f64::from_bits(self.ceiling.load(Ordering::Acquire)))
    }
    pub fn running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
    pub fn ended(&self) -> bool {
        self.ended.load(Ordering::Acquire)
            && self.position() + 0.001 >= f64::from_bits(self.ceiling.load(Ordering::Acquire))
    }
    fn reset(&self, position: f64, generation: u64) {
        self.running.store(false, Ordering::Release);
        self.base.store(position.to_bits(), Ordering::Release);
        self.ceiling.store(position.to_bits(), Ordering::Release);
        self.nanos
            .store(self.origin.elapsed().as_nanos() as u64, Ordering::Release);
        self.ended.store(false, Ordering::Release);
        self.generation.store(generation, Ordering::Release);
        self.running.store(true, Ordering::Release);
    }
    pub fn pause(&self) {
        let position = self.position();
        self.running.store(false, Ordering::Release);
        self.base.store(position.to_bits(), Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}
#[derive(Clone, Copy, Debug)]
struct OutputFrame {
    generation: u64,
    position: f64,
    sample: [f32; 2],
    end: bool,
}
struct DeviceSink {
    queue: rtrb::Consumer<OutputFrame>,
    clock: Arc<PlaybackClock>,
    channels: usize,
    rate: u32,
}
impl DeviceSink {
    fn fill<T: cpal::SizedSample + cpal::FromSample<f32>>(&mut self, data: &mut [T], latency: f64) {
        data.fill(T::from_sample(0.));
        let generation = self.clock.generation.load(Ordering::Acquire);
        let running = self.clock.running();
        let (mut first, mut last) = (None, None);
        let mut count = 0;
        let mut nonzero = 0;
        let mut clipped = 0;
        let mut end = false;
        for (index, output) in data.chunks_mut(self.channels).enumerate() {
            let frame = loop {
                match self.queue.pop() {
                    Ok(frame) if running && frame.generation == generation => break Some(frame),
                    Ok(_) => continue,
                    Err(_) => break None,
                }
            };
            let Some(frame) = frame else { continue };
            if first.is_none() {
                first = Some((index, frame.position));
            }
            last = Some(frame.position + 1. / self.rate as f64);
            count += 1;
            if frame.sample.iter().any(|sample| sample.abs() > 1e-7) {
                nonzero += 1;
            }
            if frame.sample.iter().any(|sample| sample.abs() > 1.) {
                clipped += 1;
            }
            for (channel, sample) in output.iter_mut().enumerate() {
                let value = if self.channels == 1 {
                    (frame.sample[0] + frame.sample[1]) * 0.5
                } else if channel < 2 {
                    frame.sample[channel]
                } else {
                    0.
                };
                *sample = T::from_sample(if value.is_finite() {
                    value.clamp(-1., 1.)
                } else {
                    0.
                });
            }
            end |= frame.end;
        }
        if running && self.clock.generation.load(Ordering::Acquire) == generation {
            if let (Some((index, position)), Some(last)) = (first, last) {
                self.clock.base.store(
                    (position - latency - index as f64 / self.rate as f64).to_bits(),
                    Ordering::Release,
                );
                self.clock.ceiling.store(last.to_bits(), Ordering::Release);
                self.clock.nanos.store(
                    self.clock.origin.elapsed().as_nanos() as u64,
                    Ordering::Release,
                );
                self.clock.consumed.fetch_add(count, Ordering::Relaxed);
                self.clock.nonzero.fetch_add(nonzero, Ordering::Relaxed);
                self.clock.clipped.fetch_add(clipped, Ordering::Relaxed);
                if end {
                    self.clock.ended.store(true, Ordering::Release);
                }
            }
            if count < (data.len() / self.channels) as u64
                && !self.clock.ended.load(Ordering::Acquire)
            {
                self.clock.underruns.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
pub enum OutputEvent {
    Error(u64, String),
}
struct Command {
    plan: PlaybackPlan,
    start: f64,
    generation: u64,
}
#[derive(Default)]
struct Mailbox {
    command: Option<Command>,
    stop: bool,
}
pub struct OutputWorker {
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
    pub clock: Arc<PlaybackClock>,
}
impl OutputWorker {
    pub fn start() -> (Self, async_channel::Receiver<OutputEvent>) {
        let clock = Arc::new(PlaybackClock::new());
        let mailbox = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let (events, receiver) = async_channel::bounded(8);
        let worker_clock = clock.clone();
        let worker_mailbox = mailbox.clone();
        thread::Builder::new()
            .name("flowcut-audio".into())
            .spawn(move || output_loop(worker_mailbox, worker_clock, events))
            .expect("start audio worker");
        (Self { mailbox, clock }, receiver)
    }
    pub fn play(&self, plan: PlaybackPlan, start: f64, generation: u64) {
        self.clock.reset(start, generation);
        let (lock, ready) = &*self.mailbox;
        lock.lock().unwrap().command = Some(Command {
            plan,
            start,
            generation,
        });
        ready.notify_one();
    }
    pub fn pause(&self) {
        self.clock.pause();
        self.mailbox.1.notify_one();
    }
}
impl Drop for OutputWorker {
    fn drop(&mut self) {
        self.clock.pause();
        self.mailbox.0.lock().unwrap().stop = true;
        self.mailbox.1.notify_one();
    }
}
fn build_stream<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    queue: rtrb::Consumer<OutputFrame>,
    clock: Arc<PlaybackClock>,
    events: async_channel::Sender<OutputEvent>,
) -> Result<cpal::Stream, String> {
    let error_clock = clock.clone();
    let mut sink = DeviceSink {
        queue,
        clock,
        channels: config.channels as usize,
        rate: config.sample_rate,
    };
    device
        .build_output_stream(
            config,
            move |data: &mut [T], info| {
                let timestamp = info.timestamp();
                sink.fill(
                    data,
                    timestamp
                        .playback
                        .duration_since(timestamp.callback)
                        .as_secs_f64(),
                );
            },
            move |error| {
                let generation = error_clock.generation.load(Ordering::Acquire);
                error_clock.failed.store(true, Ordering::Release);
                error_clock.pause();
                let _ = events.try_send(OutputEvent::Error(generation, error.to_string()));
            },
            Some(Duration::from_secs(3)),
        )
        .map_err(|e| e.to_string())
}
fn open_device(
    clock: Arc<PlaybackClock>,
    events: async_channel::Sender<OutputEvent>,
) -> Result<(cpal::Stream, rtrb::Producer<OutputFrame>, u32), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("No audio output device")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    let rate = config.sample_rate;
    let (producer, consumer) = rtrb::RingBuffer::new(AUDIO_QUEUE_FRAMES);
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            build_stream::<f32>(&device, config, consumer, clock.clone(), events)
        }
        cpal::SampleFormat::F64 => {
            build_stream::<f64>(&device, config, consumer, clock.clone(), events)
        }
        cpal::SampleFormat::I16 => {
            build_stream::<i16>(&device, config, consumer, clock.clone(), events)
        }
        cpal::SampleFormat::I32 => {
            build_stream::<i32>(&device, config, consumer, clock.clone(), events)
        }
        cpal::SampleFormat::U16 => {
            build_stream::<u16>(&device, config, consumer, clock.clone(), events)
        }
        _ => return Err("Unsupported output device sample encoding".into()),
    }?;
    clock.rate.store(rate, Ordering::Release);
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, producer, rate))
}
fn output_loop(
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
    clock: Arc<PlaybackClock>,
    events: async_channel::Sender<OutputEvent>,
) {
    let (lock, ready) = &*mailbox;
    let mut device = None;
    loop {
        let command = {
            let mut state = lock.lock().unwrap();
            while state.command.is_none() && !state.stop {
                state = ready.wait(state).unwrap();
            }
            if state.stop {
                return;
            }
            state.command.take().unwrap()
        };
        let result = (|| -> Result<(), String> {
            if clock.failed.swap(false, Ordering::AcqRel) {
                device = None;
            }
            if device.is_none() {
                device = Some(open_device(clock.clone(), events.clone())?);
            }
            let (_stream, producer, rate) = device.as_mut().unwrap();
            let total = ((command.plan.end - command.start).max(0.) * *rate as f64).ceil() as u64;
            if total == 0 {
                clock.ended.store(true, Ordering::Release);
                return Ok(());
            }
            let mut mixer = Mixer::new(&command.plan, command.start, *rate);
            let mut buffer = [[0.; 2]; MIX_BLOCK_FRAMES];
            let mut frame = 0;
            while frame < total {
                if !clock.running()
                    || clock.generation.load(Ordering::Acquire) != command.generation
                    || lock.lock().unwrap().stop
                {
                    return Ok(());
                }
                if producer.slots() < MIX_BLOCK_FRAMES {
                    let state = lock.lock().unwrap();
                    let (state, _) = ready.wait_timeout(state, Duration::from_millis(2)).unwrap();
                    if state.stop {
                        return Ok(());
                    }
                    continue;
                }
                let count = (total - frame).min(MIX_BLOCK_FRAMES as u64) as usize;
                mixer.render(&mut buffer[..count])?;
                for sample in &buffer[..count] {
                    let position = command.start + frame as f64 / *rate as f64;
                    producer
                        .push(OutputFrame {
                            generation: command.generation,
                            position,
                            sample: *sample,
                            end: frame + 1 == total,
                        })
                        .map_err(|_| "Audio queue overrun")?;
                    frame += 1;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            if clock.generation.load(Ordering::Acquire) == command.generation {
                clock.pause();
                let _ = events.try_send(OutputEvent::Error(command.generation, error));
            }
            device = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Workspace, catalog,
        state::{Action, Screen},
    };
    fn source() -> (Workspace, EditorState) {
        let workspace = Workspace::new().unwrap();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/tests/multichannel.mkv");
        let report = catalog::import(
            vec![path],
            Default::default(),
            Some(&workspace.path),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let mut state = EditorState::default();
        state.import_assets(report.assets);
        state.screen = Screen::Library;
        (workspace, state)
    }
    #[test]
    fn native_mixer_reads_seek_offsets_and_keeps_stereo_samples_separate() {
        let (_workspace, state) = source();
        let plan = PlaybackPlan::from_state(&state);
        let pcm = &plan.audio[0];
        let audio = &pcm.media.audio[0];
        let mut mixer = Mixer::new(&plan, pcm.start + 0.02, 48000);
        let mut samples = [[0.; 2]; 512];
        mixer.render(&mut samples).unwrap();
        for channel in 0..2 {
            let bytes = audio.channels[channel]
                .read_samples(audio.encoding, 960, 512)
                .unwrap();
            for (output, input) in samples.iter().zip(bytes.as_chunks::<4>().0) {
                let expected = i32::from_ne_bytes(*input) as f64 / 2147483648.;
                assert!(
                    (output[channel] as f64 - expected).abs() < 1e-6,
                    "{} / {}",
                    output[channel],
                    expected
                );
            }
        }
    }
    #[test]
    fn mixer_sums_overlap_and_keeps_gaps_silent() {
        let (_workspace, state) = source();
        let mut plan = PlaybackPlan::from_state(&state);
        let mut reference = Mixer::new(&plan, plan.audio[0].start + 0.02, 48000);
        let mut original = [[0.; 2]; 512];
        reference.render(&mut original).unwrap();
        let mut duplicate = plan.audio[0].clone();
        duplicate.gain = 0.5;
        plan.audio.push(duplicate);
        let mut mixer = Mixer::new(&plan, plan.audio[0].start + 0.02, 48000);
        let mut summed = [[0.; 2]; 512];
        mixer.render(&mut summed).unwrap();
        for (mixed, source) in summed.iter().zip(original) {
            assert!((mixed[0] - source[0] * 1.5).abs() < 1e-6);
            assert_eq!(mixed[1], source[1]);
        }
        for region in &mut plan.audio {
            region.start = 1.;
        }
        let mut mixer = Mixer::new(&plan, 0.99, 48000);
        let mut gap = [[0.; 2]; 1024];
        mixer.render(&mut gap).unwrap();
        assert!(gap[..480].iter().all(|sample| *sample == [0.; 2]));
        assert!(gap[480..].iter().any(|sample| sample[0].abs() > 0.01));
    }
    #[test]
    fn native_resampling_preserves_pitch_and_leaves_the_channel_cache_unchanged() {
        let (_workspace, mut state) = source();
        state.clips.clear();
        state.tracks.clear();
        state.project_min_duration = 1.;
        state.apply(Action::AddToTimeline);
        let mut plan = PlaybackPlan::from_state(&state);
        plan.audio.retain(|region| region.stream == 2);
        let region = &plan.audio[0];
        let audio = region
            .media
            .audio
            .iter()
            .find(|stream| stream.index == 2)
            .unwrap();
        assert_eq!(audio.sample_rate, 44100);
        let before = std::fs::read(&audio.channels[0].path).unwrap();
        let mut mixer = Mixer::new(&plan, 0.06, 48000);
        let mut output = vec![[0.; 2]; 12000];
        mixer.render(&mut output).unwrap();
        let crossings = output
            .windows(2)
            .filter(|samples| samples[0][0] <= 0. && samples[1][0] > 0.)
            .count();
        assert!(
            (52..=58).contains(&crossings),
            "{crossings} crossings: expected 220 Hz over 0.25 seconds"
        );
        assert!(output.iter().all(|samples| samples[0] == samples[1]));
        assert_eq!(std::fs::read(&audio.channels[0].path).unwrap(), before);
    }
    #[test]
    fn channel_mute_gain_and_video_visibility_are_applied_to_the_plan() {
        let (_workspace, mut state) = source();
        state.clips.clear();
        state.tracks.clear();
        state.apply(Action::AddToTimeline);
        let initial = PlaybackPlan::from_state(&state);
        assert_eq!(initial.videos.len(), 1);
        let index = state
            .clips
            .iter()
            .position(|clip| {
                clip.component
                    == Some(ClipComponent::AudioChannel {
                        stream: 1,
                        channel: 0,
                    })
            })
            .unwrap();
        state.apply(Action::SelectClip(index));
        state.apply(Action::Control(8, 25.));
        let plan = PlaybackPlan::from_state(&state);
        assert!(
            plan.audio
                .iter()
                .filter(|region| region.stream == 1 && region.channel == 0)
                .all(|region| region.gain == 0.25)
        );
        assert!(
            plan.audio
                .iter()
                .filter(|region| region.stream == 1 && region.channel == 1)
                .all(|region| region.gain == 1.)
        );
        state.apply(Action::TrackMuted(state.clips[index].track));
        assert!(
            PlaybackPlan::from_state(&state)
                .audio
                .iter()
                .all(|region| region.stream != 1 || region.channel != 0)
        );
        state.apply(Action::TrackVisible(0));
        assert!(PlaybackPlan::from_state(&state).videos.is_empty());
        state.apply(Action::Mute);
        assert!(
            PlaybackPlan::from_state(&state)
                .audio
                .iter()
                .all(|region| region.gain == 0.)
        );
    }
    #[test]
    fn device_callback_rejects_stale_seeks_limits_monitor_output_and_pauses_to_silence() {
        let clock = Arc::new(PlaybackClock::new());
        clock.reset(4., 2);
        let (mut producer, consumer) = rtrb::RingBuffer::new(16);
        producer
            .push(OutputFrame {
                generation: 1,
                position: 0.,
                sample: [0.9; 2],
                end: false,
            })
            .unwrap();
        producer
            .push(OutputFrame {
                generation: 2,
                position: 4.,
                sample: [1.5, -2.],
                end: false,
            })
            .unwrap();
        let mut sink = DeviceSink {
            queue: consumer,
            clock: clock.clone(),
            channels: 2,
            rate: 48000,
        };
        let mut output = [9f32; 4];
        sink.fill(&mut output, 0.);
        assert_eq!(output, [1., -1., 0., 0.]);
        assert_eq!(clock.consumed.load(Ordering::Relaxed), 1);
        assert_eq!(clock.clipped.load(Ordering::Relaxed), 1);
        clock.pause();
        producer
            .push(OutputFrame {
                generation: 2,
                position: 4.,
                sample: [0.8; 2],
                end: false,
            })
            .unwrap();
        sink.fill(&mut output, 0.);
        assert_eq!(output, [0.; 4]);
    }
}
