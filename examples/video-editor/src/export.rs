//! Bounded, in-process timeline rendering and H.264/AAC MP4 encoding.
use crate::{
    media::{Decoder, VideoFrame},
    playback::{Mixer, PlaybackPlan},
    state::{ClipComponent, EditorState, Kind, Screen},
};
use ffmpeg_next as ffmpeg;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
#[derive(Clone, Debug)]
pub struct Progress {
    pub percent: u8,
    pub step: &'static str,
}
struct Visual {
    path: PathBuf,
    stream: Option<usize>,
    track: usize,
    start: f64,
    end: f64,
    source: f64,
}
struct Cursor {
    decoder: Decoder,
    current: Option<VideoFrame>,
    future: Option<VideoFrame>,
}
impl Cursor {
    fn open(
        visual: &Visual,
        dimensions: [u32; 2],
        cancel: Arc<AtomicBool>,
    ) -> Result<Self, String> {
        let mut decoder = Decoder::open_stream(&visual.path, cancel, visual.stream)?;
        decoder.max_dimensions = dimensions;
        decoder.seek(visual.source)?;
        Ok(Self {
            decoder,
            current: None,
            future: None,
        })
    }
    fn frame(&mut self, time: f64, cancel: &AtomicBool) -> Result<&VideoFrame, String> {
        loop {
            if self.future.is_none() {
                self.future = self.decoder.next(cancel, 0.)?;
            }
            if self
                .future
                .as_ref()
                .is_some_and(|frame| frame.timestamp <= time + 1e-6)
                || self.current.is_none()
            {
                self.current = self.future.take();
                if self.current.is_none() {
                    return Err("No decoded source frame".into());
                }
            } else {
                break;
            }
        }
        self.current
            .as_ref()
            .ok_or_else(|| "No decoded source frame".into())
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn drain(
    mut receive: impl FnMut(&mut ffmpeg::Packet) -> Result<(), ffmpeg::Error>,
    output: &mut ffmpeg::format::context::Output,
    stream: usize,
    time_base: ffmpeg::Rational,
) -> Result<(), String> {
    loop {
        let mut packet = ffmpeg::Packet::empty();
        match receive(&mut packet) {
            Ok(()) => {
                packet.set_stream(stream);
                packet.rescale_ts(
                    time_base,
                    output
                        .stream(stream)
                        .ok_or("Missing output stream")?
                        .time_base(),
                );
                packet.set_position(-1);
                packet
                    .write_interleaved(output)
                    .map_err(|e| e.to_string())?;
            }
            Err(ffmpeg::Error::Eof) => break,
            Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => break,
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}
fn still(path: &Path, dimensions: [u32; 2]) -> Result<VideoFrame, String> {
    let image = image::open(path)
        .map_err(|e| e.to_string())?
        .thumbnail(dimensions[0], dimensions[1])
        .to_rgba8();
    let (width, height) = image.dimensions();
    let mut bgra = image.into_raw();
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Ok(VideoFrame {
        width,
        height,
        bgra,
        timestamp: 0.,
    })
}
pub fn run(
    state: &EditorState,
    destination: &Path,
    cancel: Arc<AtomicBool>,
    progress: &mut dyn FnMut(Progress),
) -> Result<(), String> {
    crate::media::initialize()?;
    crate::project::Timeline::capture(state).validate(&state.assets)?;
    if state.clips.is_empty() {
        return Err("Add media to the timeline before exporting.".into());
    }
    if destination.exists() {
        return Err("That file already exists. Choose a new export filename.".into());
    }
    let mut state = state.clone();
    state.screen = Screen::Edit;
    // Preview mute and monitor level never change the finished file; track mute/gain do.
    state.muted = false;
    state.monitor_gain = 1.;
    let plan = PlaybackPlan::from_state(&state);
    let mut visuals = Vec::new();
    let mut fingerprints = std::collections::BTreeMap::new();
    for clip in &state.clips {
        let asset = state.assets.get(clip.asset).ok_or("Missing clip source")?;
        let path = asset.path.as_ref().ok_or("A clip has no source file")?;
        if !path.is_file() {
            return Err(format!("Missing source: {}", path.display()));
        }
        fingerprints.insert(
            path.clone(),
            crate::preprocess::SourceFingerprint::read(path)?,
        );
        if let Some(media) = &asset.prepared {
            if crate::preprocess::SourceFingerprint::read(path)? != media.fingerprint {
                return Err(format!(
                    "Source changed; import it again: {}",
                    path.display()
                ));
            }
        } else if asset.kind != Kind::Image {
            return Err(format!("Source is not ready: {}", asset.name));
        }
        let track = state.tracks.get(clip.track).ok_or("Missing clip track")?;
        if track.visible && !track.audio {
            let stream = match clip.component {
                Some(ClipComponent::Video(index)) => Some(index),
                None if asset.kind == Kind::Image => None,
                _ => continue,
            };
            visuals.push(Visual {
                path: path.clone(),
                stream,
                track: clip.track,
                start: clip.start as f64,
                end: (clip.start + clip.length) as f64,
                source: clip.source_start as f64,
            });
        }
    }
    visuals.sort_by_key(|visual| visual.track);
    let [w, h] = state
        .clips
        .iter()
        .filter_map(|clip| state.assets.get(clip.asset))
        .find_map(|asset| asset.resolution)
        .unwrap_or([1280, 720]);
    let scale = (1920. / w.max(1) as f64)
        .min(1080. / h.max(1) as f64)
        .min(1.);
    let width = ((w as f64 * scale) as u32 / 2 * 2).max(2);
    let height = ((h as f64 * scale) as u32 / 2 * 2).max(2);
    let dimensions = [width, height];
    let duration = plan.end;
    if !duration.is_finite() || duration <= 0. {
        return Err("The timeline duration is invalid".into());
    }
    progress(Progress {
        percent: 0,
        step: "Preparing video and audio",
    });
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = destination.parent().unwrap_or(Path::new("."));
    let temporary = Temporary(parent.join(format!(
        ".flowcut-export-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir(&temporary.0).map_err(|e| e.to_string())?;
    let staged = temporary.0.join("video.mp4");
    let mut output = ffmpeg::format::output_as(&staged, "mp4").map_err(|e| e.to_string())?;
    let flags = ffmpeg::codec::Flags::GLOBAL_HEADER;
    let vcodec = ffmpeg::encoder::find_by_name("libx264")
        .ok_or("The installed FFmpeg library has no H.264 encoder (libx264)")?;
    let mut video = ffmpeg::codec::context::Context::new_with_codec(vcodec)
        .encoder()
        .video()
        .map_err(|e| e.to_string())?;
    video.set_width(width);
    video.set_height(height);
    video.set_format(ffmpeg::format::Pixel::YUV420P);
    video.set_time_base((1, 30));
    video.set_frame_rate(Some((30, 1)));
    video.set_flags(flags);
    video.set_max_b_frames(0);
    let mut options = ffmpeg::Dictionary::new();
    options.set("preset", "fast");
    options.set("crf", "20");
    options.set("threads", "2");
    let mut video = video.open_with(options).map_err(|e| e.to_string())?;
    {
        let mut stream = output.add_stream(vcodec).map_err(|e| e.to_string())?;
        stream.set_time_base((1, 30));
        stream.set_parameters(&video);
    }
    let acodec = ffmpeg::encoder::find(ffmpeg::codec::Id::AAC)
        .ok_or("The installed FFmpeg library has no AAC encoder")?;
    let mut audio = ffmpeg::codec::context::Context::new_with_codec(acodec)
        .encoder()
        .audio()
        .map_err(|e| e.to_string())?;
    audio.set_rate(48000);
    audio.set_channel_layout(ffmpeg::ChannelLayout::STEREO);
    audio.set_format(ffmpeg::format::Sample::F32(
        ffmpeg::format::sample::Type::Planar,
    ));
    audio.set_bit_rate(192000);
    audio.set_time_base((1, 48000));
    audio.set_flags(flags);
    let mut audio = audio.open_as(acodec).map_err(|e| e.to_string())?;
    if audio.frame_size() == 0 {
        return Err("AAC encoder has no fixed frame size".into());
    }
    {
        let mut stream = output.add_stream(acodec).map_err(|e| e.to_string())?;
        stream.set_time_base((1, 48000));
        stream.set_parameters(&audio);
    }
    let mut mux_options = ffmpeg::Dictionary::new();
    mux_options.set("movflags", "+faststart");
    output
        .write_header_with(mux_options)
        .map_err(|e| e.to_string())?;
    let mut scaler = ffmpeg::software::scaling::context::Context::get(
        ffmpeg::format::Pixel::BGRA,
        width,
        height,
        ffmpeg::format::Pixel::YUV420P,
        width,
        height,
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )
    .map_err(|e| e.to_string())?;
    let mut mixer = Mixer::new(&plan, 0., 48000);
    let mut active = None;
    let mut cursor = None;
    let mut image = None;
    let frames = (duration * 30.).ceil() as u64;
    let samples = (duration * 48000.).ceil() as u64;
    let mut audio_position = 0;
    let mut last_percent = 0;
    for index in 0..frames {
        if cancel.load(Ordering::Relaxed) {
            return Err("Export cancelled".into());
        }
        let time = index as f64 / 30.;
        let visual = visuals
            .iter()
            .enumerate()
            .rev()
            .find(|(_, visual)| time >= visual.start && time < visual.end);
        let id = visual.map(|(id, _)| id);
        if id != active {
            cursor = None;
            image = None;
            active = id;
            if let Some((_, visual)) = visual {
                if visual.stream.is_some() {
                    cursor = Some(Cursor::open(visual, dimensions, cancel.clone())?);
                } else {
                    image = Some(still(&visual.path, dimensions)?);
                }
            }
        }
        let mut raw = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::BGRA, width, height);
        raw.data_mut(0).fill(0);
        let source = if let (Some(cursor), Some((_, visual))) = (cursor.as_mut(), visual) {
            Some(cursor.frame(visual.source + time - visual.start, &cancel)?)
        } else {
            image.as_ref()
        };
        if let Some(source) = source {
            let left = (width - source.width) / 2;
            let top = (height - source.height) / 2;
            let stride = raw.stride(0);
            for row in 0..source.height as usize {
                let start = (row + top as usize) * stride + left as usize * 4;
                let count = source.width as usize * 4;
                raw.data_mut(0)[start..start + count]
                    .copy_from_slice(&source.bgra[row * count..(row + 1) * count]);
                // MP4 has no alpha channel; transparent pixels are composited over the black canvas.
                for pixel in raw.data_mut(0)[start..start + count].as_chunks_mut::<4>().0 {
                    let alpha = pixel[3] as u16;
                    if alpha < 255 {
                        for value in &mut pixel[..3] {
                            *value = ((*value as u16 * alpha + 127) / 255) as u8;
                        }
                    }
                    pixel[3] = 255;
                }
            }
        }
        let mut encoded = ffmpeg::frame::Video::empty();
        scaler.run(&raw, &mut encoded).map_err(|e| e.to_string())?;
        encoded.set_pts(Some(index as i64));
        video.send_frame(&encoded).map_err(|e| e.to_string())?;
        drain(
            |packet| video.receive_packet(packet),
            &mut output,
            0,
            (1, 30).into(),
        )?;
        let audio_limit = (((index + 1) as f64 / 30. * 48000.).ceil() as u64).min(samples);
        while audio_position < audio_limit {
            let count = (audio.frame_size() as u64).min(samples - audio_position) as usize;
            let mut block = vec![[0.; 2]; count];
            mixer.render(&mut block)?;
            let mut frame =
                ffmpeg::frame::Audio::new(audio.format(), count, ffmpeg::ChannelLayout::STEREO);
            frame.set_rate(48000);
            frame.set_pts(Some(audio_position as i64));
            for channel in 0..2 {
                for (out, sample) in frame.plane_mut::<f32>(channel).iter_mut().zip(&block) {
                    *out = sample[channel].clamp(-1., 1.);
                }
            }
            audio.send_frame(&frame).map_err(|e| e.to_string())?;
            drain(
                |packet| audio.receive_packet(packet),
                &mut output,
                1,
                (1, 48000).into(),
            )?;
            audio_position += count as u64;
        }
        let percent = ((index + 1) * 99 / frames) as u8;
        if percent != last_percent {
            progress(Progress {
                percent,
                step: "Rendering timeline and mixing audio",
            });
            last_percent = percent;
        }
    }
    video.send_eof().map_err(|e| e.to_string())?;
    drain(
        |packet| video.receive_packet(packet),
        &mut output,
        0,
        (1, 30).into(),
    )?;
    audio.send_eof().map_err(|e| e.to_string())?;
    drain(
        |packet| audio.receive_packet(packet),
        &mut output,
        1,
        (1, 48000).into(),
    )?;
    progress(Progress {
        percent: 99,
        step: "Finalizing MP4",
    });
    output.write_trailer().map_err(|e| e.to_string())?;
    drop(output);
    if cancel.load(Ordering::Relaxed) {
        return Err("Export cancelled".into());
    }
    for (path, fingerprint) in fingerprints {
        if crate::preprocess::SourceFingerprint::read(&path)? != fingerprint {
            return Err(format!("Source changed during export: {}", path.display()));
        }
    }
    fs::File::open(&staged)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::hard_link(&staged, destination).map_err(|e| format!("Cannot publish export: {e}"))?;
    progress(Progress {
        percent: 100,
        step: "Export complete",
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Workspace, catalog, state::Action};
    fn real_project(cache: &Workspace) -> EditorState {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/media/murchison-falls.webm");
        let report = catalog::import(
            vec![source],
            Default::default(),
            Some(&cache.path),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let mut state = EditorState::default();
        state.import_into_project(report.assets);
        let index = state.selected_clip;
        state.edit_clip_timing(index, 0.2, 0.1, 0.4).unwrap();
        state.monitor_gain = 0.;
        state.muted = true;
        state
    }
    #[test]
    fn encodes_trimmed_timeline_with_gaps_and_audio_independent_of_monitor_mute() {
        let cache = Workspace::new().unwrap();
        let out = Workspace::new().unwrap();
        let state = real_project(&cache);
        let path = out.path.join("result.mp4");
        let mut updates = Vec::new();
        run(
            &state,
            &path,
            Arc::new(AtomicBool::new(false)),
            &mut |update| updates.push(update.percent),
        )
        .unwrap();
        assert_eq!(updates.last(), Some(&100));
        assert!(updates.windows(2).all(|pair| pair[0] <= pair[1]));
        let imported = catalog::import(
            vec![path.clone()],
            Default::default(),
            Some(&cache.path),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(imported.errors.is_empty(), "{:?}", imported.errors);
        let media = imported.assets[0].prepared.as_ref().unwrap();
        assert_eq!(media.videos[0].codec, "h264");
        assert_eq!(media.audio[0].codec, "aac");
        assert!((media.duration() - state.project_duration() as f64).abs() < 0.05);
        let dark = crate::media::composite_frame(&path, 0, 0.).unwrap();
        assert!(
            dark.bgra
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[0] < 4 && pixel[1] < 4 && pixel[2] < 4)
        );
        let frame = crate::media::composite_frame(&path, 0, 0.3).unwrap();
        assert!(frame.bgra.iter().filter(|&&v| v > 40).count() > frame.bgra.len() / 4);
        let peaks = media.audio[0].peaks(0, 0.2, 0.6, 100);
        assert!(
            peaks
                .iter()
                .any(|peak| peak.max.abs() > 0.01 || peak.min.abs() > 0.01)
        );
        let bytes = fs::read(&path).unwrap();
        assert!(run(&state, &path, Arc::new(AtomicBool::new(false)), &mut |_| {}).is_err());
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    #[test]
    fn cancellation_leaves_no_output_or_partial_files() {
        let cache = Workspace::new().unwrap();
        let out = Workspace::new().unwrap();
        let state = real_project(&cache);
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = cancelled.clone();
        let result = run(
            &state,
            &out.path.join("cancel.mp4"),
            cancelled,
            &mut |update| {
                if update.percent > 0 {
                    signal.store(true, Ordering::Relaxed);
                }
            },
        );
        assert_eq!(result.unwrap_err(), "Export cancelled");
        assert_eq!(fs::read_dir(&out.path).unwrap().count(), 0);
    }
    #[test]
    fn renders_still_images_and_respects_track_visibility() {
        let cache = Workspace::new().unwrap();
        let out = Workspace::new().unwrap();
        let path = out.path.join("red.png");
        image::RgbaImage::from_pixel(64, 32, image::Rgba([230, 0, 0, 255]))
            .save(&path)
            .unwrap();
        let report = catalog::import(
            vec![path],
            Default::default(),
            Some(&cache.path),
            Arc::new(AtomicBool::new(false)),
        );
        let mut state = EditorState::default();
        state.import_into_project(report.assets);
        state.clips[0].length = 0.2;
        let output = out.path.join("still.mp4");
        run(
            &state,
            &output,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap();
        let frame = crate::media::composite_frame(&output, 0, 0.).unwrap();
        assert!(
            frame
                .bgra
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[2] > 200 && pixel[1] < 10 && pixel[0] < 10)
        );
        let transparent = out.path.join("transparent.png");
        image::RgbaImage::from_pixel(64, 32, image::Rgba([230, 0, 0, 0]))
            .save(&transparent)
            .unwrap();
        state.assets[0].path = Some(transparent);
        let output = out.path.join("transparent.mp4");
        run(
            &state,
            &output,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap();
        let frame = crate::media::composite_frame(&output, 0, 0.).unwrap();
        assert!(
            frame
                .bgra
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[0] < 4 && pixel[1] < 4 && pixel[2] < 4)
        );
        state.apply(Action::TrackVisible(0));
        let output = out.path.join("hidden.mp4");
        run(
            &state,
            &output,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap();
        let frame = crate::media::composite_frame(&output, 0, 0.).unwrap();
        assert!(
            frame
                .bgra
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[0] < 4 && pixel[1] < 4 && pixel[2] < 4)
        );

        // Later clips break same-track ties identically in preview and export.
        state.tracks[0].visible = true;
        let video = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/media/murchison-falls.webm");
        let report = catalog::import(
            vec![video],
            Default::default(),
            Some(&cache.path),
            Arc::new(AtomicBool::new(false)),
        );
        state.import_into_project(report.assets);
        state
            .edit_clip_timing(state.selected_clip, 0., 0., 0.4)
            .unwrap();
        let plan = PlaybackPlan::from_state(&state);
        assert!(plan.video_at(0.).unwrap().1.order > plan.image_at(0.).unwrap().1.order);
        let output = out.path.join("overlapping.mp4");
        run(
            &state,
            &output,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap();
        let frame = crate::media::composite_frame(&output, 0, 0.).unwrap();
        assert!(
            frame
                .bgra
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[1] > 40)
        );
    }
}
