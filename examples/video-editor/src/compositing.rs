//! Background graph previews: isolated Blender video and native libavfilter audio.
use crate::{
    composition::{Composition, Operation, Signal},
    media,
    preprocess::SampleEncoding,
    state::{Asset, ClipComponent},
};
use ffmpeg_next as ffmpeg;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
const RATE: u32 = 48_000;
const DOUBLE: ffmpeg::format::Sample =
    ffmpeg::format::Sample::F64(ffmpeg::format::sample::Type::Packed);

pub struct RenderedComposition {
    pub audio_media: Option<std::sync::Arc<crate::preprocess::PreparedMedia>>,
    pub video: Option<media::VideoFrame>,
    pub image: Option<PathBuf>,
    pub audio: Option<PathBuf>,
    pub peak: f64,
    pub duration: f64,
}
enum Value {
    Video(ffmpeg::frame::Video),
    Audio(ffmpeg::frame::Audio),
}
fn error(e: ffmpeg::Error) -> String {
    format!("FFmpeg compositing: {e}")
}
fn video_filter(
    inputs: &[&ffmpeg::frame::Video],
    spec: &str,
) -> Result<ffmpeg::frame::Video, String> {
    let mut graph = ffmpeg::filter::Graph::new();
    for (i, frame) in inputs.iter().enumerate() {
        graph
            .add(
                &ffmpeg::filter::find("buffer").ok_or("FFmpeg buffer filter unavailable")?,
                &format!("src{i}"),
                &format!(
                    "video_size={}x{}:pix_fmt={}:time_base=1/25:pixel_aspect=1/1",
                    frame.width(),
                    frame.height(),
                    ffmpeg::ffi::AVPixelFormat::from(frame.format()) as i32
                ),
            )
            .map_err(error)?;
    }
    graph
        .add(
            &ffmpeg::filter::find("buffersink").ok_or("FFmpeg video sink unavailable")?,
            "sink",
            "",
        )
        .map_err(error)?;
    let mut parser = graph.input("sink", 0).map_err(error)?;
    for i in 0..inputs.len() {
        parser = parser.output(&format!("src{i}"), 0).map_err(error)?;
    }
    parser
        .parse(&format!("{spec},format=bgra[sink]"))
        .map_err(error)?;
    graph.validate().map_err(error)?;
    for (i, input) in inputs.iter().enumerate() {
        let mut frame = (*input).clone();
        frame.set_pts(Some(0));
        graph
            .get(&format!("src{i}"))
            .unwrap()
            .source()
            .add(&frame)
            .map_err(error)?;
        graph
            .get(&format!("src{i}"))
            .unwrap()
            .source()
            .flush()
            .map_err(error)?;
    }
    let mut frame = ffmpeg::frame::Video::empty();
    graph
        .get("sink")
        .unwrap()
        .sink()
        .frame(&mut frame)
        .map_err(error)?;
    Ok(frame)
}
fn audio_filter(
    inputs: &[&ffmpeg::frame::Audio],
    spec: &str,
) -> Result<ffmpeg::frame::Audio, String> {
    let mut graph = ffmpeg::filter::Graph::new();
    for (i, frame) in inputs.iter().enumerate() {
        graph
            .add(
                &ffmpeg::filter::find("abuffer").ok_or("FFmpeg audio buffer unavailable")?,
                &format!("src{i}"),
                &format!(
                    "sample_rate={}:sample_fmt=dbl:channel_layout={}:time_base=1/{}",
                    frame.rate(),
                    if frame.channels() == 1 {
                        "mono"
                    } else {
                        "stereo"
                    },
                    frame.rate()
                ),
            )
            .map_err(error)?;
    }
    graph
        .add(
            &ffmpeg::filter::find("abuffersink").ok_or("FFmpeg audio sink unavailable")?,
            "sink",
            "",
        )
        .map_err(error)?;
    let mut parser = graph.input("sink", 0).map_err(error)?;
    for i in 0..inputs.len() {
        parser = parser.output(&format!("src{i}"), 0).map_err(error)?;
    }
    parser
        .parse(&format!(
            "{spec},aformat=sample_fmts=dbl:sample_rates={RATE}:channel_layouts=stereo[sink]"
        ))
        .map_err(error)?;
    graph.validate().map_err(error)?;
    for (i, input) in inputs.iter().enumerate() {
        let mut frame = (*input).clone();
        frame.set_pts(Some(0));
        graph
            .get(&format!("src{i}"))
            .unwrap()
            .source()
            .add(&frame)
            .map_err(error)?;
        graph
            .get(&format!("src{i}"))
            .unwrap()
            .source()
            .flush()
            .map_err(error)?;
    }
    let mut samples = vec![];
    loop {
        let mut frame = ffmpeg::frame::Audio::empty();
        match graph.get("sink").unwrap().sink().frame(&mut frame) {
            Ok(()) => samples.extend_from_slice(frame.plane::<f64>(0)),
            Err(ffmpeg::Error::Eof) => break,
            Err(e) => return Err(error(e)),
        }
    }
    if samples.is_empty() {
        return Err("Audio filter produced no samples.".into());
    }
    let mut frame =
        ffmpeg::frame::Audio::new(DOUBLE, samples.len() / 2, ffmpeg::ChannelLayout::STEREO);
    frame.set_rate(RATE);
    frame.set_pts(Some(0));
    frame.plane_mut::<f64>(0).copy_from_slice(&samples);
    Ok(frame)
}
fn amplitude(encoding: SampleEncoding, bytes: &[u8]) -> f64 {
    match encoding {
        SampleEncoding::U8 => (bytes[0] as f64 - 128.) / 128.,
        SampleEncoding::I16 => i16::from_ne_bytes(bytes.try_into().unwrap()) as f64 / 32768.,
        SampleEncoding::I32 => i32::from_ne_bytes(bytes.try_into().unwrap()) as f64 / 2147483648.,
        SampleEncoding::I64 => {
            i64::from_ne_bytes(bytes.try_into().unwrap()) as f64 / 9223372036854775808.
        }
        SampleEncoding::F32 => f32::from_ne_bytes(bytes.try_into().unwrap()) as f64,
        SampleEncoding::F64 => f64::from_ne_bytes(bytes.try_into().unwrap()),
    }
}
fn source_audio(
    asset: &Asset,
    stream: usize,
    channel: usize,
    time: f64,
    duration: f64,
) -> Result<ffmpeg::frame::Audio, String> {
    let media = asset
        .prepared
        .as_ref()
        .ok_or("Source has not been preprocessed")?;
    let stream = media
        .audio
        .iter()
        .find(|s| s.index == stream)
        .ok_or("Audio stream is missing")?;
    let channel = stream
        .channels
        .get(channel)
        .ok_or("Audio channel is missing")?;
    if !(1..=384_000).contains(&stream.sample_rate) {
        return Err("Audio source rate exceeds preview budget.".into());
    }
    let count = ((duration * stream.sample_rate as f64).ceil() as usize).max(1);
    let mut frame = ffmpeg::frame::Audio::new(DOUBLE, count, ffmpeg::ChannelLayout::MONO);
    frame.set_rate(stream.sample_rate);
    frame.set_pts(Some(0));
    frame.plane_mut::<f64>(0).fill(0.);
    // Cached samples are contiguous on disk, while PTS spans retain source gaps and lead-ins.
    for span in &stream.spans {
        let span_start = span
            .pts
            .map(|pts| stream.timing.time_base.seconds(pts))
            .unwrap_or(
                stream.start_seconds() + span.first_sample as f64 / stream.sample_rate as f64,
            )
            - media.origin_seconds;
        let lo = time.max(span_start);
        let hi =
            (time + duration).min(span_start + span.samples as f64 / stream.sample_rate as f64);
        if hi <= lo {
            continue;
        }
        let offset = ((lo - time) * stream.sample_rate as f64).round() as usize;
        let relative = ((lo - span_start) * stream.sample_rate as f64).round() as u64;
        let remaining = ((hi - lo) * stream.sample_rate as f64).round() as usize;
        let remaining = remaining
            .min(count.saturating_sub(offset))
            .min(span.samples.saturating_sub(relative) as usize);
        let mut done = 0;
        while done < remaining {
            let block = (remaining - done)
                .min(crate::preprocess::MAX_AUDIO_READ_BYTES / stream.encoding.bytes());
            let bytes = channel.read_samples(
                stream.encoding,
                span.first_sample + relative + done as u64,
                block,
            )?;
            for (i, value) in bytes.chunks_exact(stream.encoding.bytes()).enumerate() {
                let sample = amplitude(stream.encoding, value);
                frame.plane_mut::<f64>(0)[offset + done + i] =
                    if sample.is_finite() { sample } else { 0. };
            }
            done += block;
        }
    }
    let pan = match channel.name.as_str() {
        "FL" | "BL" | "SL" => "pan=stereo|c0=c0|c1=0*c0",
        "FR" | "BR" | "SR" => "pan=stereo|c0=0*c0|c1=c0",
        _ if stream.channels.len() == 2 && channel.index == 0 => "pan=stereo|c0=c0|c1=0*c0",
        _ if stream.channels.len() == 2 && channel.index == 1 => "pan=stereo|c0=0*c0|c1=c0",
        _ => "pan=stereo|c0=c0|c1=c0",
    };
    let normalized = audio_filter(
        &[&frame],
        &format!("[src0]aresample={RATE}:filter_size=64:exact_rational=1,{pan}"),
    )?;
    let samples = (duration * RATE as f64).round() as usize;
    let mut output = ffmpeg::frame::Audio::new(DOUBLE, samples, ffmpeg::ChannelLayout::STEREO);
    output.set_rate(RATE);
    output.set_pts(Some(0));
    output.plane_mut::<f64>(0).fill(0.);
    let count = normalized
        .plane::<f64>(0)
        .len()
        .min(output.plane::<f64>(0).len());
    output.plane_mut::<f64>(0)[..count].copy_from_slice(&normalized.plane::<f64>(0)[..count]);
    Ok(output)
}
fn source_video(asset: &Asset, stream: usize, time: f64) -> Result<ffmpeg::frame::Video, String> {
    let prepared = asset
        .prepared
        .as_ref()
        .ok_or("Source has not been preprocessed")?;
    let video = prepared
        .videos
        .iter()
        .find(|v| v.index == stream)
        .ok_or("Video stream is missing")?;
    let offset = video
        .timing
        .start_pts
        .map(|pts| video.timing.time_base.seconds(pts))
        .unwrap_or(prepared.origin_seconds)
        - prepared.origin_seconds;
    if time < offset {
        return Err("Video source has not started at this source time.".into());
    }
    let decoded = media::composite_frame(
        asset.path.as_deref().ok_or("Missing source path")?,
        stream,
        time - offset,
    )?;
    let mut frame =
        ffmpeg::frame::Video::new(ffmpeg::format::Pixel::BGRA, decoded.width, decoded.height);
    let stride = frame.stride(0);
    let row = decoded.width as usize * 4;
    for y in 0..decoded.height as usize {
        frame.data_mut(0)[y * stride..y * stride + row]
            .copy_from_slice(&decoded.bgra[y * row..(y + 1) * row]);
    }
    frame.set_pts(Some(0));
    Ok(frame)
}
fn packed_video(frame: &ffmpeg::frame::Video, time: f64) -> media::VideoFrame {
    let row = frame.width() as usize * 4;
    let mut bgra = Vec::with_capacity(row * frame.height() as usize);
    for y in 0..frame.height() as usize {
        bgra.extend_from_slice(&frame.data(0)[y * frame.stride(0)..y * frame.stride(0) + row]);
    }
    media::VideoFrame {
        width: frame.width(),
        height: frame.height(),
        bgra,
        timestamp: time,
    }
}
fn save_wav(path: &Path, frame: &ffmpeg::frame::Audio) -> Result<f64, String> {
    let samples = frame.plane::<f64>(0);
    let size = (samples.len() * 4) as u32;
    let mut file = std::io::BufWriter::new(fs::File::create_new(path).map_err(|e| e.to_string())?);
    file.write_all(b"RIFF")
        .and_then(|_| file.write_all(&(size + 36).to_le_bytes()))
        .and_then(|_| file.write_all(b"WAVEfmt "))
        .and_then(|_| file.write_all(&16u32.to_le_bytes()))
        .map_err(|e| e.to_string())?;
    for bytes in [3u16.to_le_bytes(), 2u16.to_le_bytes()] {
        file.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    for value in [RATE, RATE * 8] {
        file.write_all(&value.to_le_bytes())
            .map_err(|e| e.to_string())?;
    }
    for value in [8u16, 32u16] {
        file.write_all(&value.to_le_bytes())
            .map_err(|e| e.to_string())?;
    }
    file.write_all(b"data")
        .and_then(|_| file.write_all(&size.to_le_bytes()))
        .map_err(|e| e.to_string())?;
    let mut peak: f64 = 0.;
    for sample in samples {
        peak = peak.max(sample.abs());
        file.write_all(&(*sample as f32).to_le_bytes())
            .map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(peak)
}

fn video_spec(operation: &Operation) -> String {
    match operation {
        Operation::Scale { width } => {
            format!("[src0]scale={width}:540:force_original_aspect_ratio=decrease:flags=lanczos")
        }
        Operation::Flip => "[src0]hflip".into(),
        Operation::Color { saturation } => format!("[src0]eq=saturation={saturation}"),
        Operation::Blur { sigma } if *sigma == 0. => "[src0]null".into(),
        Operation::Blur { sigma } => format!("[src0]gblur=sigma={sigma}:steps=2"),
        Operation::Exposure { stops } => {
            format!("[src0]format=gbrapf32le,exposure=exposure={stops}")
        }
        Operation::Opacity { factor } => format!("[src0]format=bgra,colorchannelmixer=aa={factor}"),
        Operation::Overlay { x, y } => {
            format!("[src0][src1]overlay=x={x}:y={y}:format=auto:alpha=straight:shortest=1")
        }
        _ => unreachable!("expected a video effect"),
    }
}
fn audio_spec(operation: &Operation) -> String {
    match operation {
        Operation::Gain { db } => format!("[src0]volume={db}dB:precision=double"),
        Operation::Mix => "[src0][src1]amix=inputs=2:duration=longest:normalize=1".into(),
        Operation::LowPass { hz } => format!("[src0]lowpass=f={hz}:precision=f64"),
        Operation::HighPass { hz } => format!("[src0]highpass=f={hz}:precision=f64"),
        _ => unreachable!("expected an audio effect"),
    }
}

/// One frame and up to ten seconds of audio. Per-node intermediates are bounded and worker-owned.
pub fn render(
    graph: &Composition,
    assets: &[Asset],
    time: f64,
    duration: f64,
    directory: &Path,
) -> Result<RenderedComposition, String> {
    if directory.exists() {
        return Err(
            "Choose a new directory for this render; existing results are preserved.".into(),
        );
    }
    if !time.is_finite() || time < 0. || !duration.is_finite() || !(0.1..=10.).contains(&duration) {
        return Err(
            "Source time must be nonnegative; audio duration must be 0.1–10 seconds.".into(),
        );
    }
    if graph.nodes.len() > crate::composition::MAX_NODES {
        return Err("Preview graph exceeds node budget.".into());
    }
    media::initialize()?;
    let order = graph.order()?;
    let blender = order
        .iter()
        .any(|id| graph.node(*id).unwrap().operation.definition().is_some());
    let mut sources = vec![];
    let mut values = BTreeMap::new();
    let mut video = None;
    let mut audio = None;
    for id in order {
        let node = graph.node(id).unwrap();
        node.operation.with_parameters(
            &node
                .operation
                .parameters()
                .into_iter()
                .map(|(_, v)| v)
                .collect::<Vec<_>>(),
        )?;
        if blender && node.operation.signal() == Signal::Video {
            if let Operation::Source {
                asset,
                component: ClipComponent::Video(stream),
            } = node.operation
            {
                let asset = assets.get(asset).ok_or("Source media is missing")?;
                let prepared = asset
                    .prepared
                    .as_ref()
                    .ok_or("Source has not been preprocessed")?;
                if crate::preprocess::SourceFingerprint::read(
                    asset.path.as_deref().ok_or("Missing source path")?,
                )? != prepared.fingerprint
                {
                    return Err("Source media changed. Reimport before rendering.".into());
                }
                sources.push((id, packed_video(&source_video(asset, stream, time)?, time)));
            }
            continue;
        }
        let inputs: Vec<_> = node
            .inputs
            .iter()
            .flatten()
            .map(|id| values.get(id).ok_or("Missing input result"))
            .collect::<Result<_, _>>()?;
        if node.muted && !node.operation.is_output() && !inputs.is_empty() {
            values.insert(
                id,
                match inputs[0] {
                    Value::Video(f) => Value::Video(f.clone()),
                    Value::Audio(f) => Value::Audio(f.clone()),
                },
            );
            continue;
        }
        let value = match &node.operation {
            Operation::Source { asset, component } => {
                let asset = assets.get(*asset).ok_or("Source media is missing")?;
                let prepared = asset
                    .prepared
                    .as_ref()
                    .ok_or("Source has not been preprocessed")?;
                let path = asset.path.as_deref().ok_or("Source path is missing")?;
                if crate::preprocess::SourceFingerprint::read(path)? != prepared.fingerprint {
                    return Err(format!(
                        "{} has changed. Reimport it before rendering.",
                        asset.name
                    ));
                }
                match component {
                    ClipComponent::Video(stream) => {
                        Value::Video(source_video(asset, *stream, time)?)
                    }
                    ClipComponent::AudioChannel { stream, channel } => {
                        Value::Audio(source_audio(asset, *stream, *channel, time, duration)?)
                    }
                }
            }
            Operation::VideoOutput => {
                let Value::Video(frame) = inputs[0] else {
                    return Err("Video output needs video.".into());
                };
                video = Some(packed_video(frame, time));
                continue;
            }
            Operation::AudioOutput => {
                let Value::Audio(frame) = inputs[0] else {
                    return Err("Audio output needs audio.".into());
                };
                audio = Some(frame.clone());
                continue;
            }
            operation if operation.signal() == Signal::Video => {
                let frames: Vec<_> = inputs
                    .iter()
                    .map(|v| match v {
                        Value::Video(f) => Ok(f),
                        _ => Err("Video node needs video inputs."),
                    })
                    .collect::<Result<_, _>>()?;
                let spec = video_spec(operation);
                Value::Video(video_filter(&frames, &spec)?)
            }
            operation => {
                let frames: Vec<_> = inputs
                    .iter()
                    .map(|v| match v {
                        Value::Audio(f) => Ok(f),
                        _ => Err("Audio node needs audio inputs."),
                    })
                    .collect::<Result<_, _>>()?;
                let spec = audio_spec(operation);
                Value::Audio(audio_filter(&frames, &spec)?)
            }
        };
        values.insert(id, value);
    }
    if blender {
        video = Some(crate::blender_backend::render(
            graph, &sources, time, directory,
        )?);
    }
    // Publish only complete results. Each render has its own unique directory.
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let saved = (|| {
        let image = if let Some(frame) = &video {
            let path = directory.join("composite-frame.png");
            let mut rgba = frame.bgra.clone();
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            image::save_buffer(
                &path,
                &rgba,
                frame.width,
                frame.height,
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
            Some(path)
        } else {
            None
        };
        let (audio, peak) = if let Some(frame) = audio {
            let path = directory.join("composite-audio.wav");
            let peak = save_wav(&path, &frame)?;
            (Some(path), peak)
        } else {
            (None, 0.)
        };
        let audio_media = audio
            .as_ref()
            .map(|path| {
                crate::preprocess::prepare(
                    path,
                    directory,
                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                )
            })
            .transpose()?;
        Ok(RenderedComposition {
            audio_media,
            video,
            image,
            audio,
            peak,
            duration,
        })
    })();
    if saved.is_err() {
        let _ = fs::remove_dir_all(directory);
    }
    saved
}

#[cfg(test)]
mod tests {
    use super::*;
    fn solid(bgra: [u8; 4]) -> ffmpeg::frame::Video {
        let mut f = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::BGRA, 4, 2);
        let stride = f.stride(0);
        for y in 0..2 {
            for x in 0..4 {
                f.data_mut(0)[y * stride + x * 4..y * stride + x * 4 + 4].copy_from_slice(&bgra);
            }
        }
        f.set_pts(Some(0));
        f
    }
    fn tone(value: f64, rate: u32) -> ffmpeg::frame::Audio {
        let mut f =
            ffmpeg::frame::Audio::new(DOUBLE, rate as usize / 10, ffmpeg::ChannelLayout::STEREO);
        f.set_rate(rate);
        f.set_pts(Some(0));
        f.plane_mut::<f64>(0).fill(value);
        f
    }
    #[test]
    fn native_video_overlay_scale_and_flip_change_real_pixels() {
        media::initialize().unwrap();
        let mut a = solid([0, 0, 255, 255]);
        let b = solid([255, 0, 0, 255]);
        a.data_mut(0)[0..4].copy_from_slice(&[0, 255, 0, 255]);
        let flip = video_filter(&[&a], "[src0]hflip").unwrap();
        assert_eq!(&flip.data(0)[12..16], &[0, 255, 0, 255]);
        let result = video_filter(
            &[&a, &b],
            "[src0][src1]overlay=x=2:y=0:format=auto:shortest=1",
        )
        .unwrap();
        assert_eq!(&result.data(0)[0..4], &[0, 255, 0, 255]);
        assert!(result.data(0)[8] > 240);
        let scale = video_filter(&[&a], "[src0]scale=2:1:flags=lanczos").unwrap();
        assert_eq!((scale.width(), scale.height()), (2, 1));
    }
    #[test]
    fn opacity_alpha_over_exposure_and_blur_change_pixels() {
        media::initialize().unwrap();
        let background = solid([255, 0, 0, 255]);
        let foreground = solid([0, 0, 255, 255]);
        let half = video_filter(
            &[&foreground],
            &video_spec(&Operation::Opacity { factor: 0.5 }),
        )
        .unwrap();
        assert!((half.data(0)[3] as i32 - 128).abs() <= 1);
        let merged = video_filter(
            &[&background, &half],
            &video_spec(&Operation::Overlay { x: 0, y: 0 }),
        )
        .unwrap();
        assert!((merged.data(0)[0] as i32 - 127).abs() <= 3);
        assert!((merged.data(0)[2] as i32 - 128).abs() <= 3);
        let dim = solid([32, 32, 32, 255]);
        let bright =
            video_filter(&[&dim], &video_spec(&Operation::Exposure { stops: 1. })).unwrap();
        assert!(bright.data(0)[0] > dim.data(0)[0]);
        assert_eq!(bright.data(0)[3], 255);
        let mut edge = solid([0, 0, 0, 255]);
        edge.data_mut(0)[0..4].copy_from_slice(&[255, 255, 255, 255]);
        let blurred = video_filter(&[&edge], &video_spec(&Operation::Blur { sigma: 1. })).unwrap();
        assert!(blurred.data(0)[0] < 255 && blurred.data(0)[4] > 0);
        let unchanged =
            video_filter(&[&edge], &video_spec(&Operation::Blur { sigma: 0. })).unwrap();
        assert_eq!(&unchanged.data(0)[0..16], &edge.data(0)[0..16]);
    }
    #[test]
    fn audio_filters_attenuate_the_expected_frequency_bands() {
        media::initialize().unwrap();
        let signal = |hz: f64| {
            let mut frame = tone(0., RATE);
            for (i, pair) in frame
                .plane_mut::<f64>(0)
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .enumerate()
            {
                pair.fill((i as f64 * hz * std::f64::consts::TAU / RATE as f64).sin() * 0.5);
            }
            frame
        };
        let rms = |frame: &ffmpeg::frame::Audio| {
            let samples = &frame.plane::<f64>(0)[1000..];
            (samples.iter().map(|s| s * s).sum::<f64>() / samples.len() as f64).sqrt()
        };
        let high = signal(8000.);
        let low = signal(80.);
        let lowpass =
            audio_filter(&[&high], &audio_spec(&Operation::LowPass { hz: 500. })).unwrap();
        let highpass =
            audio_filter(&[&low], &audio_spec(&Operation::HighPass { hz: 2000. })).unwrap();
        assert!(rms(&lowpass) < rms(&high) * 0.1);
        assert!(rms(&highpass) < rms(&low) * 0.1);
        assert_eq!((lowpass.samples(), highpass.samples()), (4800, 4800));
        assert_eq!((lowpass.channels(), highpass.channels()), (2, 2));
    }
    #[test]
    fn native_gain_mix_and_resample_preserve_stereo_and_duration() {
        media::initialize().unwrap();
        let a = tone(0.4, 44100);
        let a = audio_filter(&[&a], "[src0]anull").unwrap();
        assert_eq!(a.rate(), RATE);
        assert_eq!(a.samples(), 4800);
        let b = tone(0.2, RATE);
        let gain = audio_filter(&[&b], "[src0]volume=-6.020599913dB:precision=double").unwrap();
        assert!((gain.plane::<f64>(0)[200] - 0.1).abs() < 1e-7);
        let mix = audio_filter(&[&a, &gain], "[src0][src1]amix=inputs=2:normalize=1").unwrap();
        assert_eq!(mix.channels(), 2);
        assert!(
            (mix.plane::<f64>(0)[200] - 0.25).abs() < 1e-5,
            "mix={} a={} gain={}",
            mix.plane::<f64>(0)[200],
            a.plane::<f64>(0)[200],
            gain.plane::<f64>(0)[200]
        );
    }
    #[test]
    fn real_bundled_sources_render_native_video_and_channel_mix() {
        let workspace = crate::Workspace::new().unwrap();
        let report = crate::catalog::import(
            vec![
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/media/murchison-falls.webm"),
            ],
            Default::default(),
            Some(&workspace.path),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let mut graph = Composition::default();
        graph.seed(&report.assets);
        let output = render(
            &graph,
            &report.assets,
            0.,
            0.2,
            &workspace.path.join("composition"),
        )
        .unwrap();
        assert!(output.video.unwrap().width > 0);
        assert!(output.image.unwrap().is_file());
        let wav = fs::read(output.audio.unwrap()).unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 2);
        assert!(output.peak > 0.);
        assert!(
            render(
                &graph,
                &report.assets,
                0.,
                11.,
                &workspace.path.join("invalid")
            )
            .is_err()
        );
    }
    #[test]
    fn unnamed_stereo_channel_sources_keep_their_speaker_position() {
        let workspace = crate::Workspace::new().unwrap();
        let report = crate::catalog::import(
            vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/media/cracker-stereo.wav")],
            Default::default(),
            Some(&workspace.path),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(report.errors.is_empty());
        let asset = &report.assets[0];
        let stream = &asset.prepared.as_ref().unwrap().audio[0];
        let cache_before = fs::read(&stream.channels[0].path).unwrap();
        let left = source_audio(asset, stream.index, 0, 0., 0.2).unwrap();
        assert!(
            left.plane::<f64>(0)
                .as_chunks::<2>()
                .0
                .iter()
                .any(|s| s[0].abs() > 0.01)
        );
        assert!(
            left.plane::<f64>(0)
                .as_chunks::<2>()
                .0
                .iter()
                .all(|s| s[1].abs() < 1e-12)
        );
        let right = source_audio(asset, stream.index, 1, 0., 0.2).unwrap();
        assert!(
            right
                .plane::<f64>(0)
                .as_chunks::<2>()
                .0
                .iter()
                .all(|s| s[0].abs() < 1e-12)
        );
        assert!(
            right
                .plane::<f64>(0)
                .as_chunks::<2>()
                .0
                .iter()
                .any(|s| s[1].abs() > 0.01)
        );
        assert_eq!(fs::read(&stream.channels[0].path).unwrap(), cache_before);
    }
    #[test]
    fn changed_sources_are_rejected_without_publishing_partial_results() {
        let workspace = crate::Workspace::new().unwrap();
        let path = workspace.path.join("source.webm");
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/media/murchison-falls.webm"),
            &path,
        )
        .unwrap();
        let report = crate::catalog::import(
            vec![path.clone()],
            Default::default(),
            Some(&workspace.path),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(report.errors.is_empty());
        let mut graph = Composition::default();
        graph.seed(&report.assets);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&[0])
            .unwrap();
        let destination = workspace.path.join("changed");
        let error = render(&graph, &report.assets, 0., 0.1, &destination)
            .err()
            .unwrap();
        assert!(error.contains("has changed"), "{error}");
        assert!(!destination.exists());
    }
    #[test]
    fn composition_results_add_stereo_audio_tracks_and_keep_frames_in_the_library() {
        let workspace = crate::Workspace::new().unwrap();
        let sources = crate::catalog::import(
            vec![
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/media/murchison-falls.webm"),
            ],
            Default::default(),
            Some(&workspace.path),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        let mut graph = Composition::default();
        graph.seed(&sources.assets);
        let result = render(
            &graph,
            &sources.assets,
            0.,
            0.2,
            &workspace.path.join("result"),
        )
        .unwrap();
        let imports = crate::catalog::import(
            vec![result.image.unwrap(), result.audio.unwrap()],
            Default::default(),
            Some(&workspace.path),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(imports.errors.is_empty());
        let mut state = crate::state::EditorState::default();
        state.position = 2.;
        assert_eq!(state.import_composition_result(imports.assets), (2, 0, 1));
        assert_eq!(state.clips.len(), 2);
        assert!(
            state
                .clips
                .iter()
                .all(|c| matches!(c.component, Some(ClipComponent::AudioChannel { .. })))
        );
        assert!(
            state
                .assets
                .iter()
                .any(|a| a.kind == crate::state::Kind::Image)
        );
        assert!(state.clips.iter().all(|c| (c.start - 2.).abs() < 0.001));
        assert_eq!(state.clips[0].link_group, state.clips[1].link_group);
    }
}
