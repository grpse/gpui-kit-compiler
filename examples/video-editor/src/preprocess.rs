//! Immutable source analysis and lossless, disk-backed audio channel caches.
use ffmpeg_next as ffmpeg;
use std::{
    fs::{self, File},
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::UNIX_EPOCH,
};

pub const MAX_PEAKS: usize = 131_072;
pub const MAX_AUDIO_READ_BYTES: usize = 1024 * 1024;
const INDEX_RECORD_BYTES: u64 = 24;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SourceFingerprint {
    pub bytes: u64,
    pub modified_ns: u128,
}
impl SourceFingerprint {
    pub fn read(path: &Path) -> Result<Self, String> {
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
        Ok(Self {
            bytes: metadata.len(),
            modified_ns: metadata
                .modified()
                .map_err(|e| e.to_string())?
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TimeBase {
    pub numerator: i32,
    pub denominator: i32,
}
impl TimeBase {
    fn from_rational(value: ffmpeg::Rational) -> Self {
        Self {
            numerator: value.numerator(),
            denominator: value.denominator(),
        }
    }
    pub fn seconds(self, pts: i64) -> f64 {
        pts as f64 * self.numerator as f64 / self.denominator as f64
    }
}
#[derive(Clone, Copy, Debug)]
pub struct StreamTiming {
    pub time_base: TimeBase,
    pub start_pts: Option<i64>,
    pub duration_pts: Option<i64>,
}
impl StreamTiming {
    fn read(stream: &ffmpeg::Stream<'_>) -> Self {
        Self {
            time_base: TimeBase::from_rational(stream.time_base()),
            start_pts: (stream.start_time() != ffmpeg::ffi::AV_NOPTS_VALUE)
                .then_some(stream.start_time()),
            duration_pts: (stream.duration() > 0).then_some(stream.duration()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Keyframe {
    pub pts: i64,
    pub dts: i64,
    pub byte_position: i64,
}
#[derive(Debug)]
pub struct KeyframeIndex {
    pub path: PathBuf,
    pub entries: u64,
    pub time_base: TimeBase,
    pub monotonic: bool,
}
impl KeyframeIndex {
    fn read_record(file: &mut File, index: u64) -> Result<Keyframe, String> {
        file.seek(SeekFrom::Start(
            index
                .checked_mul(INDEX_RECORD_BYTES)
                .ok_or("Index offset overflow")?,
        ))
        .map_err(|e| e.to_string())?;
        let mut bytes = [0; 24];
        file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        Ok(Keyframe {
            pts: i64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            dts: i64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            byte_position: i64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        })
    }
    /// O(log n) disk reads; no packet payloads or complete index loaded into RAM.
    pub fn preceding(&self, seconds: f64) -> Result<Option<Keyframe>, String> {
        if self.entries == 0 || !self.monotonic {
            return Ok(None);
        }
        let mut file = File::open(&self.path).map_err(|e| e.to_string())?;
        let (mut lo, mut hi) = (0, self.entries);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self
                .time_base
                .seconds(Self::read_record(&mut file, mid)?.pts)
                <= seconds
            {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            Ok(None)
        } else {
            Self::read_record(&mut file, lo - 1).map(Some)
        }
    }
}
#[derive(Debug)]
pub struct VideoStream {
    pub index: usize,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: String,
    pub color_space: String,
    pub color_range: String,
    pub color_primaries: String,
    pub color_transfer: String,
    pub aspect_ratio: [i32; 2],
    pub frame_rate: [i32; 2],
    pub timing: StreamTiming,
    pub packet_count: u64,
    pub keyframes: Arc<KeyframeIndex>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleEncoding {
    U8,
    I16,
    I32,
    I64,
    F32,
    F64,
}
impl SampleEncoding {
    fn from_format(format: ffmpeg::format::Sample) -> Result<Self, String> {
        use ffmpeg::format::Sample;
        match format {
            Sample::U8(_) => Ok(Self::U8),
            Sample::I16(_) => Ok(Self::I16),
            Sample::I32(_) => Ok(Self::I32),
            Sample::I64(_) => Ok(Self::I64),
            Sample::F32(_) => Ok(Self::F32),
            Sample::F64(_) => Ok(Self::F64),
            Sample::None => Err("Unknown decoded sample format".into()),
        }
    }
    pub fn bytes(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::I16 => 2,
            Self::I32 | Self::F32 => 4,
            Self::I64 | Self::F64 => 8,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::U8 => "8-bit unsigned PCM",
            Self::I16 => "16-bit PCM",
            Self::I32 => "32-bit PCM",
            Self::I64 => "64-bit PCM",
            Self::F32 => "32-bit float PCM",
            Self::F64 => "64-bit float PCM",
        }
    }
    fn amplitude(self, bytes: &[u8]) -> f32 {
        let value = match self {
            Self::U8 => (bytes[0] as f64 - 128.) / 128.,
            Self::I16 => i16::from_ne_bytes(bytes.try_into().unwrap()) as f64 / 32768.,
            Self::I32 => i32::from_ne_bytes(bytes.try_into().unwrap()) as f64 / 2147483648.,
            Self::I64 => {
                i64::from_ne_bytes(bytes.try_into().unwrap()) as f64 / 9223372036854775808.
            }
            Self::F32 => f32::from_ne_bytes(bytes.try_into().unwrap()) as f64,
            Self::F64 => f64::from_ne_bytes(bytes.try_into().unwrap()),
        };
        if value.is_finite() {
            value.clamp(-(f32::MAX as f64), f32::MAX as f64) as f32
        } else {
            0.
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Peak {
    pub min: f32,
    pub max: f32,
}
impl Peak {
    fn merge(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}
#[derive(Debug)]
pub struct Waveform {
    pub samples_per_peak: u64,
    pub samples: u64,
    pub levels: Box<[Box<[Peak]>]>,
}
impl Waveform {
    /// Query a source sample range at a resolution bounded by the visible pixel count.
    pub fn query(&self, start: u64, end: u64, columns: usize) -> Vec<Peak> {
        let columns = columns.clamp(1, 1024);
        if start >= end || start >= self.samples || self.levels.is_empty() {
            return vec![Peak::default(); columns];
        }
        let end = end.min(self.samples);
        let samples_per_column = (end - start).div_ceil(columns as u64);
        let mut level = 0;
        let mut stride = self.samples_per_peak;
        while level + 1 < self.levels.len() && stride * 2 <= samples_per_column {
            level += 1;
            stride *= 2;
        }
        let peaks = &self.levels[level];
        (0..columns)
            .map(|column| {
                let lo = start + (end - start) * column as u64 / columns as u64;
                let hi = start + ((end - start) * (column + 1) as u64).div_ceil(columns as u64);
                let first = (lo / stride) as usize;
                let last = hi.div_ceil(stride) as usize;
                peaks
                    .get(first..last.min(peaks.len()))
                    .and_then(|peaks| peaks.iter().copied().reduce(Peak::merge))
                    .unwrap_or_default()
            })
            .collect()
    }
}
struct WaveformBuilder {
    stride: u64,
    samples: u64,
    in_peak: u64,
    pending: Peak,
    peaks: Vec<Peak>,
}
impl WaveformBuilder {
    fn new(rate: u32) -> Self {
        Self {
            stride: (rate as u64 / 100).max(1),
            samples: 0,
            in_peak: 0,
            pending: Peak::default(),
            peaks: Vec::new(),
        }
    }
    fn push(&mut self, sample: f32) {
        if self.in_peak == 0 {
            self.pending = Peak {
                min: sample,
                max: sample,
            };
        } else {
            self.pending = self.pending.merge(Peak {
                min: sample,
                max: sample,
            });
        }
        self.in_peak += 1;
        self.samples += 1;
        if self.in_peak == self.stride {
            self.peaks.push(self.pending);
            self.in_peak = 0;
            self.compact();
        }
    }
    fn compact(&mut self) {
        if self.peaks.len() >= MAX_PEAKS {
            let length = self.peaks.len();
            for index in 0..length.div_ceil(2) {
                self.peaks[index] = if index * 2 + 1 < length {
                    self.peaks[index * 2].merge(self.peaks[index * 2 + 1])
                } else {
                    self.peaks[index * 2]
                };
            }
            self.peaks.truncate(length.div_ceil(2));
            self.stride *= 2;
        }
    }
    fn finish(mut self) -> Waveform {
        if self.in_peak > 0 {
            self.peaks.push(self.pending);
            self.compact();
        }
        let mut levels = vec![self.peaks.into_boxed_slice()];
        while levels.last().unwrap().len() > 1 {
            let next = levels
                .last()
                .unwrap()
                .chunks(2)
                .map(|pair| {
                    if pair.len() == 2 {
                        pair[0].merge(pair[1])
                    } else {
                        pair[0]
                    }
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            levels.push(next);
        }
        Waveform {
            samples_per_peak: self.stride,
            samples: self.samples,
            levels: levels.into_boxed_slice(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AudioSpan {
    pub first_sample: u64,
    pub samples: u64,
    pub pts: Option<i64>,
}
#[derive(Debug)]
pub struct AudioChannel {
    pub index: usize,
    pub name: String,
    pub path: PathBuf,
    pub waveform: Waveform,
}
impl AudioChannel {
    /// Bounded random access; never loads an entire channel into memory.
    pub fn read_samples(
        &self,
        encoding: SampleEncoding,
        start: u64,
        count: usize,
    ) -> Result<Vec<u8>, String> {
        let bytes = count
            .checked_mul(encoding.bytes())
            .filter(|bytes| *bytes <= MAX_AUDIO_READ_BYTES)
            .ok_or("Audio read exceeds 1 MiB buffer limit")?;
        if start
            .checked_add(count as u64)
            .is_none_or(|end| end > self.waveform.samples)
        {
            return Err("Audio sample range is out of bounds".into());
        }
        let mut file = File::open(&self.path).map_err(|e| e.to_string())?;
        file.seek(SeekFrom::Start(
            start
                .checked_mul(encoding.bytes() as u64)
                .ok_or("Audio offset overflow")?,
        ))
        .map_err(|e| e.to_string())?;
        let mut result = vec![0; bytes];
        file.read_exact(&mut result).map_err(|e| e.to_string())?;
        Ok(result)
    }
}
#[derive(Debug)]
pub struct AudioStream {
    pub index: usize,
    pub codec: String,
    pub sample_rate: u32,
    pub encoding: SampleEncoding,
    pub layout: String,
    pub timing: StreamTiming,
    pub spans: Box<[AudioSpan]>,
    pub samples: u64,
    pub channels: Box<[AudioChannel]>,
}
impl AudioStream {
    pub fn start_seconds(&self) -> f64 {
        self.spans
            .first()
            .and_then(|span| span.pts)
            .or(self.timing.start_pts)
            .map(|pts| self.timing.time_base.seconds(pts))
            .unwrap_or(0.)
    }
    pub fn duration(&self) -> f64 {
        self.samples as f64 / self.sample_rate as f64
    }
    pub fn peaks(&self, channel: usize, start: f64, end: f64, columns: usize) -> Vec<Peak> {
        self.channels
            .get(channel)
            .map(|channel| {
                channel.waveform.query(
                    (start.max(0.) * self.sample_rate as f64) as u64,
                    (end.max(0.) * self.sample_rate as f64).ceil() as u64,
                    columns,
                )
            })
            .unwrap_or_default()
    }
}

#[derive(Debug)]
struct CacheDirectory(PathBuf);
impl Drop for CacheDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[derive(Debug)]
pub struct PreparedMedia {
    pub fingerprint: SourceFingerprint,
    pub origin_seconds: f64,
    pub primary_video: Option<usize>,
    pub videos: Box<[VideoStream]>,
    pub audio: Box<[AudioStream]>,
    pub cache_bytes: u64,
    pub manifest: PathBuf,
    _cache: Arc<CacheDirectory>,
}

impl PreparedMedia {
    pub fn duration(&self) -> f64 {
        self.videos
            .iter()
            .filter_map(|video| {
                video.timing.duration_pts.map(|duration| {
                    video
                        .timing
                        .time_base
                        .seconds(video.timing.start_pts.unwrap_or(0).saturating_add(duration))
                        - self.origin_seconds
                })
            })
            .chain(self.audio.iter().flat_map(|audio| {
                audio.spans.iter().map(move |span| {
                    span.pts
                        .map(|pts| audio.timing.time_base.seconds(pts))
                        .unwrap_or(
                            audio.start_seconds()
                                + span.first_sample as f64 / audio.sample_rate as f64,
                        )
                        + span.samples as f64 / audio.sample_rate as f64
                        - self.origin_seconds
                })
            }))
            .filter(|seconds| seconds.is_finite())
            .fold(0., f64::max)
    }
}

struct IndexWriter {
    writer: BufWriter<File>,
    index: KeyframeIndex,
    last_pts: Option<i64>,
    packets: u64,
    first_pts: Option<i64>,
    end_pts: Option<i64>,
}
impl IndexWriter {
    fn packet(&mut self, packet: &ffmpeg::Packet) -> Result<(), String> {
        self.packets += 1;
        if let Some(pts) = packet.pts() {
            self.first_pts = Some(self.first_pts.map_or(pts, |first| first.min(pts)));
            let end = pts.saturating_add(packet.duration().max(0));
            self.end_pts = Some(self.end_pts.map_or(end, |last| last.max(end)));
        }
        if packet.is_key()
            && let Some(pts) = packet.pts().or(packet.dts())
        {
            self.index.monotonic &= self.last_pts.is_none_or(|last| pts >= last);
            self.last_pts = Some(pts);
            for value in [
                pts,
                packet.dts().unwrap_or(ffmpeg::ffi::AV_NOPTS_VALUE),
                packet.position() as i64,
            ] {
                self.writer
                    .write_all(&value.to_le_bytes())
                    .map_err(|e| e.to_string())?;
            }
            self.index.entries += 1;
        }
        Ok(())
    }
}

struct ChannelWriter {
    writer: BufWriter<File>,
    path: PathBuf,
    name: String,
    waveform: WaveformBuilder,
    scratch: Vec<u8>,
}
struct AudioProcessor {
    decoder: ffmpeg::decoder::Audio,
    index: usize,
    codec: String,
    timing: StreamTiming,
    encoding: Option<SampleEncoding>,
    rate: u32,
    layout: String,
    channels: Vec<ChannelWriter>,
    spans: Vec<AudioSpan>,
    samples: u64,
    directory: PathBuf,
}

fn channel_names(layout: ffmpeg::ChannelLayout) -> (String, Vec<String>) {
    let mut description = [0i8; 256];
    // FFmpeg owns this borrowed layout for the duration of this call, including custom maps.
    unsafe {
        ffmpeg::ffi::av_channel_layout_describe(
            &layout.0,
            description.as_mut_ptr(),
            description.len(),
        );
    }
    let description = unsafe { std::ffi::CStr::from_ptr(description.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    let names = (0..layout.channels() as u32)
        .map(|index| {
            let mut name = [0i8; 128];
            let result = unsafe {
                let channel = ffmpeg::ffi::av_channel_layout_channel_from_index(&layout.0, index);
                ffmpeg::ffi::av_channel_name(name.as_mut_ptr(), name.len(), channel)
            };
            let label = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }.to_string_lossy();
            if result < 0 || label == "NONE" || label == "UNK" {
                format!("Channel {}", index + 1)
            } else {
                label.into_owned()
            }
        })
        .collect();
    (description, names)
}
impl AudioProcessor {
    fn receive(&mut self, cancel: &AtomicBool) -> Result<(), String> {
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("Preprocessing cancelled".into());
            }
            let mut frame = ffmpeg::frame::Audio::empty();
            match self.decoder.receive_frame(&mut frame) {
                Ok(()) => self.frame(&frame)?,
                Err(ffmpeg::Error::Eof) => return Ok(()),
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {
                    return Ok(());
                }
                Err(error) => return Err(error.to_string()),
            }
        }
    }
    fn frame(&mut self, frame: &ffmpeg::frame::Audio) -> Result<(), String> {
        let encoding = SampleEncoding::from_format(frame.format())?;
        if frame.rate() == 0 || frame.channels() == 0 {
            return Err("Invalid audio sample rate or channel count".into());
        }
        if let Some(previous) = self.encoding {
            if previous != encoding
                || self.rate != frame.rate()
                || self.channels.len() != frame.channels() as usize
            {
                return Err(
                    "Audio format changes within a stream; split the source at the format change"
                        .into(),
                );
            }
        } else {
            self.encoding = Some(encoding);
            self.rate = frame.rate();
            let (layout, names) = channel_names(frame.channel_layout());
            self.layout = layout;
            for (index, name) in names.into_iter().enumerate() {
                let path = self
                    .directory
                    .join(format!("audio-{}-channel-{index}.pcm", self.index));
                self.channels.push(ChannelWriter {
                    writer: BufWriter::with_capacity(
                        64 * 1024,
                        File::create(&path).map_err(|e| e.to_string())?,
                    ),
                    path,
                    name,
                    waveform: WaveformBuilder::new(self.rate),
                    scratch: Vec::new(),
                });
            }
        }
        let mut pts = frame.pts().or(frame.timestamp());
        if self.codec == "vorbis" {
            // Variable Vorbis windows may expose packet duration shorter than the PCM
            // frame (e.g. 128 ticks / 576 samples). Anchor those samples to the
            // packet end to avoid manufacturing tiny gaps and overlapping clips.
            let duration = frame.packet().duration;
            let sample_ticks = unsafe {
                ffmpeg::ffi::av_rescale_q(
                    frame.samples() as i64,
                    ffmpeg::Rational(1, self.rate as i32).into(),
                    ffmpeg::Rational(
                        self.timing.time_base.numerator,
                        self.timing.time_base.denominator,
                    )
                    .into(),
                )
            };
            if duration > 0 && duration < sample_ticks {
                pts = pts.map(|pts| pts.saturating_add(duration - sample_ticks));
            }
        }
        let tolerance = self.timing.time_base.seconds(1).abs() * 0.51 + 0.51 / self.rate as f64;
        if let Some(span) = self.spans.last_mut().filter(|span| match (span.pts, pts) {
            (Some(first), Some(current)) => {
                (self.timing.time_base.seconds(first) + span.samples as f64 / self.rate as f64
                    - self.timing.time_base.seconds(current))
                .abs()
                    <= tolerance
            }
            (None, None) => true,
            _ => false,
        }) {
            span.samples += frame.samples() as u64;
        } else {
            self.spans.push(AudioSpan {
                first_sample: self.samples,
                samples: frame.samples() as u64,
                pts,
            });
        }
        let bytes = encoding.bytes();
        let channel_count = self.channels.len();
        for (index, channel) in self.channels.iter_mut().enumerate() {
            let data = if frame.is_planar() {
                frame
                    .data(index)
                    .get(
                        ..frame
                            .samples()
                            .checked_mul(bytes)
                            .ok_or("Audio frame size overflow")?,
                    )
                    .ok_or("Truncated audio plane")?
            } else {
                let data = frame.data(0);
                let expected = frame
                    .samples()
                    .checked_mul(bytes * channel_count)
                    .ok_or("Audio frame size overflow")?;
                if data.len() < expected {
                    return Err("Truncated interleaved audio frame".into());
                }
                channel.scratch.clear();
                for sample in data[..expected].chunks_exact(bytes * channel_count) {
                    channel
                        .scratch
                        .extend_from_slice(&sample[index * bytes..(index + 1) * bytes]);
                }
                &channel.scratch
            };
            // Copy original native-endian sample bytes, excluding AVFrame alignment padding.
            channel.writer.write_all(data).map_err(|e| e.to_string())?;
            for sample in data.chunks_exact(bytes) {
                channel.waveform.push(encoding.amplitude(sample));
            }
        }
        self.samples += frame.samples() as u64;
        Ok(())
    }
    fn finish(mut self) -> Result<AudioStream, String> {
        let encoding = self
            .encoding
            .ok_or("Audio stream contains no decoded samples")?;
        let channels = self
            .channels
            .into_iter()
            .enumerate()
            .map(|(index, mut channel)| {
                channel.writer.flush().map_err(|e| e.to_string())?;
                Ok(AudioChannel {
                    index,
                    name: channel.name,
                    path: channel.path,
                    waveform: channel.waveform.finish(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.spans.shrink_to_fit();
        Ok(AudioStream {
            index: self.index,
            codec: self.codec,
            sample_rate: self.rate,
            encoding,
            layout: self.layout,
            timing: self.timing,
            spans: self.spans.into_boxed_slice(),
            samples: self.samples,
            channels: channels.into_boxed_slice(),
        })
    }
}

/// One demux pass indexes video packets and decodes all audio streams, without transcoding video.
pub fn prepare(
    path: &Path,
    root: &Path,
    cancel: Arc<AtomicBool>,
) -> Result<Arc<PreparedMedia>, String> {
    prepare_with_progress(path, root, cancel, &mut |_| {})
}
pub fn prepare_with_progress(
    path: &Path,
    root: &Path,
    cancel: Arc<AtomicBool>,
    progress: &mut dyn FnMut(crate::processing::Update),
) -> Result<Arc<PreparedMedia>, String> {
    use crate::processing::{Stage, Update};
    progress(Update::new(path, Stage::Inspecting, None));
    crate::media::initialize()?;
    let fingerprint = SourceFingerprint::read(path)?;
    static NEXT_CACHE: AtomicU64 = AtomicU64::new(0);
    let directory = root.join(format!(
        "media-{}",
        NEXT_CACHE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let cache = Arc::new(CacheDirectory(directory.clone()));
    let interrupt = cancel.clone();
    let mut input =
        ffmpeg::format::input_with_interrupt(path, move || interrupt.load(Ordering::Relaxed))
            .map_err(|e| e.to_string())?;
    let primary_video = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .map(|stream| stream.index());
    let mut videos = Vec::new();
    let mut indexes = Vec::new();
    let mut audio = Vec::new();
    let mut routing = vec![None; input.nb_streams() as usize];
    for stream in input.streams() {
        let parameters = stream.parameters();
        let timing = StreamTiming::read(&stream);
        let context = ffmpeg::codec::context::Context::from_parameters(parameters.clone())
            .map_err(|e| e.to_string())?;
        let mut decoder_context = context.decoder();
        // Native timestamps and codec priming/discard durations use the demux stream units.
        decoder_context.set_packet_time_base(stream.time_base());
        match parameters.medium() {
            ffmpeg::media::Type::Video => {
                let decoder = decoder_context.video().map_err(|e| e.to_string())?;
                let rate = stream.avg_frame_rate();
                let aspect = decoder.aspect_ratio();
                let index_path = directory.join(format!("video-{}-keyframes.bin", stream.index()));
                let index = KeyframeIndex {
                    path: index_path.clone(),
                    entries: 0,
                    time_base: timing.time_base,
                    monotonic: true,
                };
                videos.push(VideoStream {
                    index: stream.index(),
                    codec: decoder
                        .codec()
                        .map(|codec| codec.name().into())
                        .unwrap_or_default(),
                    width: decoder.width(),
                    height: decoder.height(),
                    pixel_format: format!("{:?}", decoder.format()),
                    color_space: format!("{:?}", decoder.color_space()),
                    color_range: format!("{:?}", decoder.color_range()),
                    color_primaries: format!("{:?}", decoder.color_primaries()),
                    color_transfer: format!("{:?}", decoder.color_transfer_characteristic()),
                    aspect_ratio: [aspect.numerator(), aspect.denominator()],
                    frame_rate: [rate.numerator(), rate.denominator()],
                    timing,
                    packet_count: 0,
                    keyframes: Arc::new(KeyframeIndex {
                        path: index_path.clone(),
                        entries: 0,
                        time_base: timing.time_base,
                        monotonic: true,
                    }),
                });
                routing[stream.index()] = Some((false, indexes.len()));
                indexes.push(IndexWriter {
                    writer: BufWriter::with_capacity(
                        64 * 1024,
                        File::create(index_path).map_err(|e| e.to_string())?,
                    ),
                    index,
                    last_pts: None,
                    packets: 0,
                    first_pts: None,
                    end_pts: None,
                });
            }
            ffmpeg::media::Type::Audio => {
                let decoder = decoder_context.audio().map_err(|e| e.to_string())?;
                let codec = decoder
                    .codec()
                    .map(|codec| codec.name().into())
                    .unwrap_or_default();
                routing[stream.index()] = Some((true, audio.len()));
                audio.push(AudioProcessor {
                    decoder,
                    index: stream.index(),
                    codec,
                    timing,
                    encoding: None,
                    rate: 0,
                    layout: String::new(),
                    channels: Vec::new(),
                    spans: Vec::new(),
                    samples: 0,
                    directory: directory.clone(),
                });
            }
            _ => {}
        }
    }
    if videos.is_empty() && audio.is_empty() {
        return Err("No video or audio streams".into());
    }
    let mut last_percent = None;
    progress(Update::new(path, Stage::Decoding, Some(0)));
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Preprocessing cancelled".into());
        }
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut input) {
            Ok(()) => {}
            Err(ffmpeg::Error::Eof) => break,
            Err(error) => return Err(error.to_string()),
        }
        let percent = (packet.position() >= 0 && fingerprint.bytes > 0).then(|| {
            ((packet.position() as u64).saturating_mul(100) / fingerprint.bytes).min(99) as u8
        });
        if percent != last_percent {
            progress(Update::new(path, Stage::Decoding, percent));
            last_percent = percent;
        }
        if let Some((is_audio, index)) = routing[packet.stream()] {
            if is_audio {
                match audio[index].decoder.send_packet(&packet) {
                    Ok(()) => {}
                    Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {
                        audio[index].receive(&cancel)?;
                        audio[index]
                            .decoder
                            .send_packet(&packet)
                            .map_err(|e| e.to_string())?;
                    }
                    Err(error) => return Err(error.to_string()),
                }
                audio[index].receive(&cancel)?;
            } else {
                indexes[index].packet(&packet)?;
            }
        }
    }
    progress(Update::new(path, Stage::Finalizing, None));
    for processor in &mut audio {
        processor.decoder.send_eof().map_err(|e| e.to_string())?;
        processor.receive(&cancel)?;
    }
    let audio = audio
        .into_iter()
        .map(AudioProcessor::finish)
        .collect::<Result<Vec<_>, _>>()?;
    for (video, mut writer) in videos.iter_mut().zip(indexes) {
        writer.writer.flush().map_err(|e| e.to_string())?;
        video.packet_count = writer.packets;
        if video.timing.start_pts.is_none() {
            video.timing.start_pts = writer.first_pts;
        }
        if video.timing.duration_pts.is_none() {
            video.timing.duration_pts = writer
                .end_pts
                .zip(video.timing.start_pts)
                .and_then(|(end, first)| (end > first).then_some(end - first));
        }
        video.keyframes = Arc::new(writer.index);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Preprocessing cancelled".into());
    }
    if SourceFingerprint::read(path)? != fingerprint {
        return Err("Source changed during preprocessing; import it again".into());
    }
    // Include encoder priming and any earlier audio stream in the project origin.
    // Each preview decoder still uses its own stream's start time.
    let origin_seconds = videos
        .iter()
        .filter_map(|video| {
            video
                .timing
                .start_pts
                .map(|pts| video.timing.time_base.seconds(pts))
        })
        .chain(audio.iter().map(AudioStream::start_seconds))
        .filter(|seconds| seconds.is_finite())
        .reduce(f64::min)
        .unwrap_or(0.);
    let cache_bytes = fs::read_dir(&directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();
    let manifest = directory.join("analysis.json");
    let metadata = serde_json::json!({"version":1,"source":path,"source_bytes":fingerprint.bytes,"source_modified_ns":fingerprint.modified_ns.to_string(),"native_endian":if cfg!(target_endian="little"){"little"}else{"big"},"origin_seconds":origin_seconds,
        "video":videos.iter().map(|video|serde_json::json!({"stream":video.index,"codec":video.codec,"resolution":[video.width,video.height],"pixel_format":video.pixel_format,"color_space":video.color_space,"color_range":video.color_range,"primaries":video.color_primaries,"transfer":video.color_transfer,"aspect_ratio":video.aspect_ratio,"frame_rate":video.frame_rate,"time_base":[video.timing.time_base.numerator,video.timing.time_base.denominator],"start_pts":video.timing.start_pts,"duration_pts":video.timing.duration_pts,"keyframes":video.keyframes.entries,"index":video.keyframes.path})).collect::<Vec<_>>(),
        "audio":audio.iter().map(|stream|serde_json::json!({"stream":stream.index,"codec":stream.codec,"sample_rate":stream.sample_rate,"sample_encoding":stream.encoding.label(),"layout":stream.layout,"time_base":[stream.timing.time_base.numerator,stream.timing.time_base.denominator],"samples":stream.samples,"spans":stream.spans.iter().map(|span|serde_json::json!({"first_sample":span.first_sample,"samples":span.samples,"pts":span.pts})).collect::<Vec<_>>(),"channels":stream.channels.iter().map(|channel|serde_json::json!({"index":channel.index,"name":channel.name,"pcm":channel.path,"peak_stride":channel.waveform.samples_per_peak})).collect::<Vec<_>>()})).collect::<Vec<_>>()});
    fs::write(
        &manifest,
        serde_json::to_vec(&metadata).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(Arc::new(PreparedMedia {
        fingerprint,
        origin_seconds,
        primary_video,
        videos: videos.into_boxed_slice(),
        audio: audio.into_boxed_slice(),
        cache_bytes,
        manifest,
        _cache: cache,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Workspace;

    fn active() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }
    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/tests/multichannel.mkv")
    }
    fn wav(path: &Path, samples: &[[i16; 2]]) {
        let mut data = Vec::new();
        for sample in samples {
            for channel in sample {
                data.extend_from_slice(&channel.to_le_bytes());
            }
        }
        let mut file = File::create(path).unwrap();
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36 + data.len() as u32).to_le_bytes())
            .unwrap();
        file.write_all(b"WAVEfmt ").unwrap();
        file.write_all(&16u32.to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap();
        file.write_all(&2u16.to_le_bytes()).unwrap();
        file.write_all(&48000u32.to_le_bytes()).unwrap();
        file.write_all(&192000u32.to_le_bytes()).unwrap();
        file.write_all(&4u16.to_le_bytes()).unwrap();
        file.write_all(&16u16.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&(data.len() as u32).to_le_bytes()).unwrap();
        file.write_all(&data).unwrap();
    }

    #[test]
    fn packed_pcm_is_lossless_separated_and_random_reads_are_bounded() {
        let workspace = Workspace::new().unwrap();
        let source = workspace.path.join("stereo.wav");
        let samples: Vec<_> = (0..960)
            .map(|i| {
                if i % 2 == 0 {
                    [i16::MIN, 8192]
                } else {
                    [i16::MAX, -16384]
                }
            })
            .collect();
        wav(&source, &samples);
        let original = fs::read(&source).unwrap();
        let media = prepare(&source, &workspace.path, active()).unwrap();
        assert!(media.videos.is_empty());
        let audio = &media.audio[0];
        assert_eq!(audio.sample_rate, 48000);
        assert_eq!(audio.encoding, SampleEncoding::I16);
        assert_eq!(audio.samples, 960);
        assert_eq!(audio.channels.len(), 2);
        assert_eq!(audio.channels[0].name, "Channel 1");
        assert_eq!(audio.channels[1].name, "Channel 2");
        for channel in 0..2 {
            let expected: Vec<_> = samples
                .iter()
                .flat_map(|sample| sample[channel].to_ne_bytes())
                .collect();
            assert_eq!(fs::read(&audio.channels[channel].path).unwrap(), expected);
            let partial: Vec<_> = samples[777..779]
                .iter()
                .flat_map(|sample| sample[channel].to_ne_bytes())
                .collect();
            assert_eq!(
                audio.channels[channel]
                    .read_samples(audio.encoding, 777, 2)
                    .unwrap(),
                partial
            );
            assert!(
                audio.channels[channel]
                    .read_samples(audio.encoding, 960, 1)
                    .is_err()
            );
            assert!(
                audio.channels[channel]
                    .read_samples(audio.encoding, 0, MAX_AUDIO_READ_BYTES)
                    .is_err()
            );
        }
        let left = audio.peaks(0, 0., audio.duration(), 1)[0];
        let right = audio.peaks(1, 0., audio.duration(), 1)[0];
        assert_eq!(left.min, -1.);
        assert!(left.max > 0.999);
        assert_eq!((right.min, right.max), (-0.5, 0.25));
        assert_eq!(fs::read(&source).unwrap(), original);
        let directory = media.manifest.parent().unwrap().to_owned();
        let shared = media.clone();
        drop(media);
        assert!(directory.is_dir());
        drop(shared);
        assert!(!directory.exists());
    }

    #[test]
    fn planar_float_preserves_bits_headroom_and_excludes_padding() {
        crate::media::initialize().unwrap();
        let workspace = Workspace::new().unwrap();
        let input = ffmpeg::format::input(&fixture()).unwrap();
        let stream = input.streams().best(ffmpeg::media::Type::Audio).unwrap();
        let decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .unwrap()
            .decoder()
            .audio()
            .unwrap();
        let mut processor = AudioProcessor {
            decoder,
            index: 5,
            codec: "test".into(),
            timing: StreamTiming {
                time_base: TimeBase {
                    numerator: 1,
                    denominator: 48000,
                },
                start_pts: Some(0),
                duration_pts: None,
            },
            encoding: None,
            rate: 0,
            layout: String::new(),
            channels: Vec::new(),
            spans: Vec::new(),
            samples: 0,
            directory: workspace.path.clone(),
        };
        let values = [[1.5f32, -2., -0.0], [0.125f32, -0.25, 0.5]];
        let mut frame = ffmpeg::frame::Audio::new(
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar),
            3,
            ffmpeg::ChannelLayout::STEREO,
        );
        frame.set_rate(48000);
        frame.set_pts(Some(0));
        for (channel, samples) in values.iter().enumerate() {
            frame.data_mut(channel).fill(0x5a);
            for (index, sample) in samples.iter().enumerate() {
                frame.data_mut(channel)[index * 4..(index + 1) * 4]
                    .copy_from_slice(&sample.to_ne_bytes());
            }
        }
        processor.frame(&frame).unwrap();
        frame.set_pts(Some(10)); // A genuine seven-sample gap: keep source timing, don't insert silence.
        processor.frame(&frame).unwrap();
        let audio = processor.finish().unwrap();
        assert_eq!(audio.encoding, SampleEncoding::F32);
        assert_eq!(audio.channels[0].name, "FL");
        assert_eq!(audio.channels[1].name, "FR");
        assert_eq!(audio.spans.len(), 2);
        assert_eq!(audio.spans[1].pts, Some(10));
        for (channel, samples) in values.iter().enumerate() {
            let expected: Vec<_> = samples
                .iter()
                .chain(samples)
                .flat_map(|value| value.to_ne_bytes())
                .collect();
            assert_eq!(fs::read(&audio.channels[channel].path).unwrap(), expected);
        }
        let peak = audio.peaks(0, 0., audio.duration(), 1)[0];
        assert_eq!((peak.min, peak.max), (-2., 1.5));
    }

    #[test]
    fn native_multistream_analysis_keeps_video_precision_and_audio_rates() {
        let workspace = Workspace::new().unwrap();
        let media = prepare(&fixture(), &workspace.path, active()).unwrap();
        assert_eq!(media.videos.len(), 1);
        let video = &media.videos[0];
        assert_eq!((video.width, video.height), (96, 64));
        assert_eq!(video.pixel_format, "YUV420P10LE");
        assert_eq!(video.packet_count, 10);
        assert!(video.keyframes.entries > 0);
        let key = video.keyframes.preceding(0.3).unwrap().unwrap();
        assert!(video.timing.time_base.seconds(key.pts) <= 0.3);
        assert_eq!(
            fs::metadata(&video.keyframes.path).unwrap().len(),
            video.keyframes.entries * INDEX_RECORD_BYTES
        );
        assert_eq!(media.audio.len(), 2);
        let pcm = media
            .audio
            .iter()
            .find(|audio| audio.codec == "pcm_s24le")
            .unwrap();
        assert_eq!(
            (pcm.sample_rate, pcm.encoding, pcm.channels.len()),
            (48000, SampleEncoding::I32, 2)
        );
        assert_eq!(pcm.samples, 19200);
        assert_eq!(pcm.channels[0].name, "Channel 1");
        assert_eq!(pcm.channels[1].name, "Channel 2");
        let aac = media
            .audio
            .iter()
            .find(|audio| audio.codec == "aac")
            .unwrap();
        assert_eq!(
            (aac.sample_rate, aac.encoding, aac.channels.len()),
            (44100, SampleEncoding::F32, 1)
        );
        assert!(aac.samples >= 17640);
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&media.manifest).unwrap()).unwrap();
        assert_eq!(manifest["version"], 1);
        assert_eq!(manifest["audio"].as_array().unwrap().len(), 2);
        assert!(media.cache_bytes > 19200 * 2 * 4);
    }

    #[test]
    fn waveform_pyramid_compacts_without_losing_extrema() {
        let mut builder = WaveformBuilder::new(100);
        for i in 0..MAX_PEAKS * 3 + 1 {
            builder.push(if i == 12345 {
                -0.9
            } else if i == MAX_PEAKS * 2 {
                0.8
            } else {
                0.1
            });
        }
        let waveform = builder.finish();
        assert!(waveform.levels[0].len() < MAX_PEAKS);
        assert!(
            waveform
                .levels
                .iter()
                .map(|level| level.len())
                .sum::<usize>()
                < MAX_PEAKS * 2 + 32
        );
        assert!(waveform.samples_per_peak > 1);
        let peak = waveform.query(0, waveform.samples, 1)[0];
        assert_eq!((peak.min, peak.max), (-0.9, 0.8));
        assert_eq!(waveform.query(0, waveform.samples, usize::MAX).len(), 1024);
        assert!(
            waveform
                .query(waveform.samples, waveform.samples, 10)
                .iter()
                .all(|peak| peak.max == 0.)
        );
    }

    #[test]
    fn cancellation_and_decode_failure_leave_no_partial_cache() {
        let workspace = Workspace::new().unwrap();
        assert!(prepare(&fixture(), &workspace.path, Arc::new(AtomicBool::new(true))).is_err());
        assert_eq!(fs::read_dir(&workspace.path).unwrap().count(), 0);
        let invalid = workspace.path.join("invalid.mkv");
        fs::write(&invalid, "invalid media").unwrap();
        assert!(prepare(&invalid, &workspace.path, active()).is_err());
        assert_eq!(fs::read_dir(&workspace.path).unwrap().count(), 1);
    }
}
