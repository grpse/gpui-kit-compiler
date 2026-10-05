//! In-process FFmpeg decoding. All FFmpeg contexts stay on their worker thread.
use crate::state::{Asset, Kind};
use ffmpeg_next as ffmpeg;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub const PREVIEW_WIDTH: u32 = 960;
pub const PREVIEW_HEIGHT: u32 = 540;
pub const FRAME_QUEUE_CAPACITY: usize = 2;

pub(crate) fn initialize() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        ffmpeg::init().map_err(|e| e.to_string())?;
        ffmpeg::log::set_level(ffmpeg::log::Level::Error);
        Ok(())
    })
    .clone()
}

#[derive(Debug)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub timestamp: f64,
}
impl VideoFrame {
    pub fn render_image(self) -> Arc<gpui_kit::RenderImage> {
        let pixels = image::RgbaImage::from_raw(self.width, self.height, self.bgra)
            .expect("packed BGRA frame");
        Arc::new(gpui_kit::RenderImage::new(vec![image::Frame::new(pixels)]))
    }
    fn save_poster(mut self, path: &Path) -> Result<(), String> {
        for pixel in self.bgra.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        image::save_buffer(
            path,
            &self.bgra,
            self.width,
            self.height,
            image::ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())
    }
}

pub(crate) struct Decoder {
    input: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: Option<ffmpeg::software::scaling::context::Context>,
    scale_source: Option<(ffmpeg::format::Pixel, u32, u32)>,
    stream: usize,
    time_base: f64,
    origin: f64,
    duration: f64,
    fps: f64,
    draining: bool,
    fallback_time: f64,
    pub(crate) max_dimensions: [u32; 2],
}
/// Bounded source-frame decoding for the compositing worker.
pub fn composite_frame(path: &Path, stream: usize, position: f64) -> Result<VideoFrame, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    let mut decoder = Decoder::open_stream(path, cancel.clone(), Some(stream))?;
    decoder.seek(position)?;
    decoder
        .next(&cancel, position)?
        .ok_or_else(|| "No video frame at this source time.".into())
}
impl Decoder {
    fn open(path: &Path, cancel: Arc<AtomicBool>) -> Result<Self, String> {
        Self::open_stream(path, cancel, None)
    }
    pub(crate) fn open_stream(
        path: &Path,
        cancel: Arc<AtomicBool>,
        stream_index: Option<usize>,
    ) -> Result<Self, String> {
        initialize()?;
        let input =
            ffmpeg::format::input_with_interrupt(path, move || cancel.load(Ordering::Relaxed))
                .map_err(|e| e.to_string())?;
        let stream = if let Some(index) = stream_index {
            input
                .stream(index)
                .filter(|stream| stream.parameters().medium() == ffmpeg::media::Type::Video)
        } else {
            input.streams().best(ffmpeg::media::Type::Video)
        }
        .ok_or("No video stream")?;
        let time_base = f64::from(stream.time_base());
        let origin = if stream.start_time() == ffmpeg::ffi::AV_NOPTS_VALUE {
            0.
        } else {
            stream.start_time() as f64 * time_base
        };
        let stream_duration = stream.duration() as f64 * time_base;
        let duration = if input.duration() > 0 {
            input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
        } else {
            stream_duration
        };
        let rate = f64::from(stream.avg_frame_rate());
        let fps = if rate.is_finite() && rate > 0. {
            rate
        } else {
            30.
        };
        let index = stream.index();
        let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .map_err(|e| e.to_string())?;
        let mut threading =
            ffmpeg::codec::threading::Config::kind(ffmpeg::codec::threading::Type::Frame);
        threading.count = 2;
        context.set_threading(threading);
        let mut decoder_context = context.decoder();
        decoder_context.set_packet_time_base(stream.time_base());
        let decoder = decoder_context.video().map_err(|e| e.to_string())?;
        Ok(Self {
            input,
            decoder,
            scaler: None,
            scale_source: None,
            stream: index,
            time_base,
            origin,
            duration,
            fps,
            draining: false,
            fallback_time: 0.,
            max_dimensions: [PREVIEW_WIDTH, PREVIEW_HEIGHT],
        })
    }
    pub(crate) fn seek(&mut self, position: f64) -> Result<(), String> {
        let timestamp = ((position + self.origin) * ffmpeg::ffi::AV_TIME_BASE as f64) as i64;
        self.input
            .seek(timestamp, ..timestamp)
            .map_err(|e| e.to_string())?;
        self.decoder.flush();
        self.draining = false;
        self.fallback_time = position;
        Ok(())
    }
    pub(crate) fn next(
        &mut self,
        cancel: &AtomicBool,
        minimum_timestamp: f64,
    ) -> Result<Option<VideoFrame>, String> {
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let mut decoded = ffmpeg::frame::Video::empty();
            match self.decoder.receive_frame(&mut decoded) {
                Ok(()) => {
                    let timestamp = decoded
                        .timestamp()
                        .or(decoded.pts())
                        .map(|t| t as f64 * self.time_base - self.origin)
                        .unwrap_or(self.fallback_time)
                        .max(0.);
                    self.fallback_time = timestamp + 1. / self.fps;
                    // Decode from the preceding keyframe; scale only frames that will be presented.
                    if timestamp + 0.5 / self.fps < minimum_timestamp {
                        continue;
                    }
                    let source = (decoded.format(), decoded.width(), decoded.height());
                    if source.1 == 0 || source.2 == 0 {
                        return Err("Invalid frame dimensions".into());
                    }
                    if self.scale_source != Some(source) {
                        let scale = (self.max_dimensions[0] as f64 / source.1 as f64)
                            .min(self.max_dimensions[1] as f64 / source.2 as f64)
                            .min(1.);
                        let width = (source.1 as f64 * scale).round().max(1.) as u32;
                        let height = (source.2 as f64 * scale).round().max(1.) as u32;
                        self.scaler = Some(
                            ffmpeg::software::scaling::context::Context::get(
                                source.0,
                                source.1,
                                source.2,
                                ffmpeg::format::Pixel::BGRA,
                                width,
                                height,
                                ffmpeg::software::scaling::flag::Flags::LANCZOS
                                    | ffmpeg::software::scaling::flag::Flags::ACCURATE_RND,
                            )
                            .map_err(|e| e.to_string())?,
                        );
                        self.scale_source = Some(source);
                    }
                    let mut output = ffmpeg::frame::Video::empty();
                    self.scaler
                        .as_mut()
                        .unwrap()
                        .run(&decoded, &mut output)
                        .map_err(|e| e.to_string())?;
                    let row_bytes = output.width() as usize * 4;
                    let mut bgra = Vec::with_capacity(row_bytes * output.height() as usize);
                    for row in 0..output.height() as usize {
                        let start = row * output.stride(0);
                        bgra.extend_from_slice(&output.data(0)[start..start + row_bytes]);
                    }
                    return Ok(Some(VideoFrame {
                        width: output.width(),
                        height: output.height(),
                        bgra,
                        timestamp,
                    }));
                }
                Err(ffmpeg::Error::Eof) => return Ok(None),
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {}
                Err(error) => return Err(error.to_string()),
            }
            if self.draining {
                return Ok(None);
            }
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) if packet.stream() == self.stream => self
                    .decoder
                    .send_packet(&packet)
                    .map_err(|e| e.to_string())?,
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => {
                    self.decoder.send_eof().map_err(|e| e.to_string())?;
                    self.draining = true;
                }
                Err(error) => return Err(error.to_string()),
            }
        }
    }
}

pub fn inspect(
    asset: &mut Asset,
    cache: Option<&Path>,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    inspect_with_progress(asset, cache, cancel, &mut |_| {})
}
pub fn inspect_with_progress(
    asset: &mut Asset,
    cache: Option<&Path>,
    cancel: Arc<AtomicBool>,
    progress: &mut dyn FnMut(crate::processing::Update),
) -> Result<(), String> {
    let path = asset.path.as_deref().ok_or("Missing source path")?;
    if asset.kind == Kind::Image {
        progress(crate::processing::Update::new(
            path,
            crate::processing::Stage::Inspecting,
            None,
        ));
        let (width, height) = image::image_dimensions(path).map_err(|e| e.to_string())?;
        asset.resolution = Some([width, height]);
        asset.poster = Some(path.to_owned());
        asset.codec = path
            .extension()
            .map(|ext| ext.to_string_lossy().to_uppercase());
        return Ok(());
    }
    let cache = cache.ok_or("Cannot preprocess media without a writable cache directory")?;
    let prepared = crate::preprocess::prepare_with_progress(path, cache, cancel.clone(), progress)?;
    if let Some(video) = prepared
        .videos
        .iter()
        .find(|video| Some(video.index) == prepared.primary_video)
    {
        asset.kind = Kind::Video;
        asset.duration = video
            .timing
            .duration_pts
            .map(|pts| video.timing.time_base.seconds(pts))
            .unwrap_or(0.)
            .max(0.) as f32;
        asset.resolution = Some([video.width, video.height]);
        asset.codec = Some(video.codec.clone());
        if video.frame_rate[1] > 0 {
            asset.frame_rate = Some(format!(
                "{:.2} fps",
                video.frame_rate[0] as f64 / video.frame_rate[1] as f64
            ));
        }
        progress(crate::processing::Update::new(
            path,
            crate::processing::Stage::Thumbnail,
            None,
        ));
        let mut decoder = Decoder::open(path, cancel.clone())?;
        if asset.duration <= 0. && decoder.duration.is_finite() {
            asset.duration = decoder.duration.max(0.) as f32;
        }
        let poster = prepared.manifest.with_file_name("poster.png");
        if let Some(frame) = decoder.next(&cancel, 0.)? {
            frame.save_poster(&poster)?;
            asset.poster = Some(poster);
        }
    } else if !prepared.audio.is_empty() {
        asset.kind = Kind::Audio;
        asset.duration = prepared
            .audio
            .iter()
            .map(|stream| stream.duration())
            .fold(0., f64::max) as f32;
        asset.codec = Some(prepared.audio[0].codec.clone());
    }
    asset.duration = asset.duration.max(prepared.duration() as f32);
    asset.prepared = Some(prepared);
    Ok(())
}

#[derive(Clone)]
pub struct PreviewRequest {
    pub path: PathBuf,
    pub position: f64,
    pub end: f64,
    pub playing: bool,
    pub generation: u64,
    pub stream: Option<usize>,
    pub keyframes: Option<Arc<crate::preprocess::KeyframeIndex>>,
    pub clock: Option<Arc<crate::playback::PlaybackClock>>,
    pub clock_offset: f64,
}
pub enum PreviewEvent {
    Frame { generation: u64, frame: VideoFrame },
    End(u64),
    Error(u64, String),
}
#[derive(Default)]
struct Mailbox {
    request: Option<PreviewRequest>,
    stop: bool,
}
pub struct PreviewWorker {
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
    cancel: Arc<AtomicBool>,
}
impl PreviewWorker {
    pub fn start() -> (Self, async_channel::Receiver<PreviewEvent>) {
        let mailbox = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = async_channel::bounded(FRAME_QUEUE_CAPACITY);
        let worker_mailbox = mailbox.clone();
        let worker_cancel = cancel.clone();
        thread::Builder::new()
            .name("flowcut-preview".into())
            .spawn(move || {
                preview_loop(worker_mailbox, worker_cancel, sender);
            })
            .expect("start preview decoder");
        (Self { mailbox, cancel }, receiver)
    }
    pub fn request(&self, request: PreviewRequest) {
        let (lock, ready) = &*self.mailbox;
        lock.lock().unwrap().request = Some(request);
        ready.notify_one();
    }
    pub fn pause(&self) {
        // Wake the worker and interrupt the current request without dropping its decoder.
        let (lock, ready) = &*self.mailbox;
        lock.lock().unwrap().request = Some(PreviewRequest {
            path: PathBuf::new(),
            position: 0.,
            end: 0.,
            playing: false,
            generation: 0,
            stream: None,
            keyframes: None,
            clock: None,
            clock_offset: 0.,
        });
        ready.notify_one();
    }
}
impl Drop for PreviewWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        let (lock, ready) = &*self.mailbox;
        lock.lock().unwrap().stop = true;
        ready.notify_one();
    }
}

fn preview_loop(
    mailbox: Arc<(Mutex<Mailbox>, Condvar)>,
    cancel: Arc<AtomicBool>,
    sender: async_channel::Sender<PreviewEvent>,
) {
    let (lock, ready) = &*mailbox;
    let mut decoder: Option<(PathBuf, Option<usize>, Decoder)> = None;
    loop {
        let request = {
            let mut state = lock.lock().unwrap();
            while state.request.is_none() && !state.stop {
                state = ready.wait(state).unwrap();
            }
            if state.stop {
                return;
            }
            state.request.take().unwrap()
        };
        if request.path.as_os_str().is_empty() {
            continue;
        }
        let result = (|| -> Result<(), String> {
            if decoder
                .as_ref()
                .is_none_or(|(path, stream, _)| path != &request.path || *stream != request.stream)
            {
                decoder = Some((
                    request.path.clone(),
                    request.stream,
                    Decoder::open_stream(&request.path, cancel.clone(), request.stream)?,
                ));
            }
            let decoder = &mut decoder.as_mut().unwrap().2;
            let position = request
                .position
                .min((request.end - 1. / decoder.fps).max(0.));
            let keyframe = request.keyframes.as_ref().and_then(|index| {
                index
                    .preceding(position + decoder.origin)
                    .ok()
                    .flatten()
                    .map(|frame| (index.time_base.seconds(frame.pts) - decoder.origin).max(0.))
            });
            decoder.seek(keyframe.unwrap_or(position))?;
            let clock = Instant::now();
            let mut shown = false;
            loop {
                if cancel.load(Ordering::Relaxed) || lock.lock().unwrap().request.is_some() {
                    return Ok(());
                }
                let minimum = request
                    .clock
                    .as_ref()
                    .filter(|_| request.playing)
                    .map(|audio| (audio.position() - request.clock_offset - 0.02).max(position))
                    .unwrap_or(position);
                let Some(frame) = decoder.next(&cancel, minimum)? else {
                    break;
                };
                if frame.timestamp + 0.5 / decoder.fps < position {
                    continue;
                }
                if frame.timestamp >= request.end {
                    break;
                }
                if request.playing {
                    loop {
                        let remaining = if let Some(audio) = &request.clock {
                            if !audio.running() {
                                return Ok(());
                            }
                            frame.timestamp + request.clock_offset - audio.position()
                        } else {
                            frame.timestamp - position - clock.elapsed().as_secs_f64()
                        };
                        if remaining <= 0. {
                            break;
                        }
                        let state = lock.lock().unwrap();
                        let (state, _) = ready
                            .wait_timeout_while(
                                state,
                                Duration::from_secs_f64(remaining.min(0.01)),
                                |s| s.request.is_none() && !s.stop,
                            )
                            .unwrap();
                        if state.stop {
                            return Ok(());
                        }
                        if state.request.is_some() {
                            return Ok(());
                        }
                    }
                    // Discard video that fell behind the shared playback clock during decode.
                    if request.clock.as_ref().is_some_and(|audio| {
                        audio.position() > frame.timestamp + request.clock_offset + 0.08
                    }) {
                        continue;
                    }
                }
                // At most two scaled frames await the UI; a slow UI drops frames.
                match sender.try_send(PreviewEvent::Frame {
                    generation: request.generation,
                    frame,
                }) {
                    Ok(()) => shown = true,
                    Err(async_channel::TrySendError::Closed(_)) => return Ok(()),
                    Err(async_channel::TrySendError::Full(event)) if !request.playing => {
                        send_control(&mailbox, &sender, event);
                        shown = true;
                    }
                    Err(async_channel::TrySendError::Full(_)) => {}
                }
                if !request.playing && shown {
                    return Ok(());
                }
            }
            if !shown && !request.playing {
                return Err("No frame at this position".into());
            }
            send_control(&mailbox, &sender, PreviewEvent::End(request.generation));
            Ok(())
        })();
        if let Err(error) = result {
            send_control(
                &mailbox,
                &sender,
                PreviewEvent::Error(request.generation, error),
            );
        }
    }
}

fn send_control(
    mailbox: &Arc<(Mutex<Mailbox>, Condvar)>,
    sender: &async_channel::Sender<PreviewEvent>,
    mut event: PreviewEvent,
) {
    loop {
        match sender.try_send(event) {
            Ok(()) | Err(async_channel::TrySendError::Closed(_)) => return,
            Err(async_channel::TrySendError::Full(pending)) => event = pending,
        }
        let (lock, ready) = &**mailbox;
        let state = lock.lock().unwrap();
        if state.stop || state.request.is_some() {
            return;
        }
        let (state, _) = ready.wait_timeout(state, Duration::from_millis(5)).unwrap();
        if state.stop || state.request.is_some() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/media/murchison-falls.webm")
    }
    #[test]
    fn in_process_decode_seek_and_packed_buffer_bounds() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut decoder = Decoder::open(&fixture(), cancel.clone()).unwrap();
        assert!(decoder.duration > 1.);
        let first = decoder.next(&cancel, 0.).unwrap().unwrap();
        assert!(first.width <= PREVIEW_WIDTH && first.height <= PREVIEW_HEIGHT);
        assert_eq!(
            first.bgra.len(),
            first.width as usize * first.height as usize * 4
        );
        decoder.seek(1.).unwrap();
        let mut frame = decoder.next(&cancel, 0.).unwrap().unwrap();
        while frame.timestamp < 1. {
            frame = decoder.next(&cancel, 0.).unwrap().unwrap();
        }
        assert!(frame.timestamp < 1.2);
        cancel.store(true, Ordering::Relaxed);
        assert!(decoder.next(&cancel, 0.).unwrap().is_none());
    }
    #[test]
    fn preview_worker_terminates_at_end() {
        let (worker, receiver) = PreviewWorker::start();
        worker.request(PreviewRequest {
            path: fixture(),
            position: 0.,
            end: 0.1,
            playing: true,
            generation: 42,
            stream: None,
            keyframes: None,
            clock: None,
            clock_offset: 0.,
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut frames = 0;
        loop {
            assert!(Instant::now() < deadline, "preview did not finish");
            match receiver.try_recv() {
                Ok(PreviewEvent::Frame { generation, .. }) => {
                    assert_eq!(generation, 42);
                    frames += 1;
                }
                Ok(PreviewEvent::End(generation)) => {
                    assert_eq!(generation, 42);
                    break;
                }
                Ok(PreviewEvent::Error(_, error)) => panic!("{error}"),
                Err(_) => thread::sleep(Duration::from_millis(5)),
            }
        }
        assert!(frames > 0);
        assert!(receiver.len() <= FRAME_QUEUE_CAPACITY);
        drop(worker);
    }
    #[test]
    fn preview_seeks_with_preprocessed_stream_keyframes() {
        let workspace = crate::Workspace::new().unwrap();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/tests/multichannel.mkv");
        let analysis =
            crate::preprocess::prepare(&path, &workspace.path, Arc::new(AtomicBool::new(false)))
                .unwrap();
        let video = &analysis.videos[0];
        let (worker, receiver) = PreviewWorker::start();
        worker.request(PreviewRequest {
            path,
            position: 0.2,
            end: 0.4,
            playing: false,
            generation: 10,
            stream: Some(video.index),
            keyframes: Some(video.keyframes.clone()),
            clock: None,
            clock_offset: 0.,
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                Instant::now() < deadline,
                "indexed seek did not produce a preview"
            );
            match receiver.try_recv() {
                Ok(PreviewEvent::Frame {
                    generation: 10,
                    frame,
                }) => {
                    assert!(
                        frame.timestamp >= 0.18 && frame.timestamp < 0.26,
                        "{}",
                        frame.timestamp
                    );
                    assert_eq!((frame.width, frame.height), (96, 64));
                    break;
                }
                Ok(PreviewEvent::Error(_, error)) => panic!("{error}"),
                _ => thread::sleep(Duration::from_millis(5)),
            }
        }
        drop(worker);
    }
    #[test]
    fn saturated_queue_accepts_a_new_seek_and_reaches_the_requested_frame() {
        let (worker, receiver) = PreviewWorker::start();
        worker.request(PreviewRequest {
            path: fixture(),
            position: 0.,
            end: 2.,
            playing: true,
            generation: 1,
            stream: None,
            keyframes: None,
            clock: None,
            clock_offset: 0.,
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while receiver.len() < FRAME_QUEUE_CAPACITY {
            assert!(Instant::now() < deadline, "preview queue did not fill");
            thread::sleep(Duration::from_millis(5));
        }
        worker.request(PreviewRequest {
            path: fixture(),
            position: 0.2,
            end: 2.,
            playing: false,
            generation: 2,
            stream: None,
            keyframes: None,
            clock: None,
            clock_offset: 0.,
        });
        worker.request(PreviewRequest {
            path: fixture(),
            position: 1.,
            end: 2.,
            playing: false,
            generation: 3,
            stream: None,
            keyframes: None,
            clock: None,
            clock_offset: 0.,
        });
        loop {
            assert!(
                Instant::now() < deadline,
                "latest seek was blocked by queued frames"
            );
            match receiver.try_recv() {
                Ok(PreviewEvent::Frame {
                    generation: 3,
                    frame,
                }) => {
                    assert!(
                        frame.timestamp >= 0.98 && frame.timestamp < 1.2,
                        "{}",
                        frame.timestamp
                    );
                    break;
                }
                Ok(PreviewEvent::Error(generation, error)) => {
                    panic!("generation {generation}: {error}")
                }
                _ => thread::sleep(Duration::from_millis(5)),
            }
        }
        drop(worker);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !receiver.is_closed() {
            assert!(Instant::now() < deadline, "worker did not shut down");
            thread::sleep(Duration::from_millis(5));
        }
    }
}
