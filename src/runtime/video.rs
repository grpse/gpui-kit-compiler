use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use gpui_kit::{
    Context, Image, ImageFormat, ImageSource, InteractiveElement, IntoElement, ObjectFit,
    ParentElement, Refineable, Render, StatefulInteractiveElement, Styled, StyledImage, Window,
    div, img, px, rgb, rgba,
};

use crate::runtime::binding::InlineStyle;

const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;

pub(crate) struct VideoPlayer {
    id: String,
    source: String,
    poster: Option<String>,
    controls: bool,
    autoplay: bool,
    looping: bool,
    muted: bool,
    fit: VideoFit,
    width: f32,
    height: f32,
    style: InlineStyle,
    frame: Option<Arc<Image>>,
    playing: bool,
    error: Option<String>,
    session: Option<VideoSession>,
    generation: u64,
}

pub(crate) struct VideoOptions {
    pub id: String,
    pub source: String,
    pub poster: Option<String>,
    pub controls: bool,
    pub autoplay: bool,
    pub looping: bool,
    pub muted: bool,
    pub fit: VideoFit,
    pub width: f32,
    pub height: f32,
    pub style: InlineStyle,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum VideoFit {
    Contain,
    Cover,
    Fill,
    ScaleDown,
    None,
}

impl VideoFit {
    fn object_fit(self) -> ObjectFit {
        match self {
            Self::Contain => ObjectFit::Contain,
            Self::Cover => ObjectFit::Cover,
            Self::Fill => ObjectFit::Fill,
            Self::ScaleDown => ObjectFit::ScaleDown,
            Self::None => ObjectFit::None,
        }
    }
}

struct VideoSession {
    child: Arc<Mutex<Child>>,
}

impl Drop for VideoSession {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

enum VideoEvent {
    Frame(Vec<u8>),
    End,
}

impl VideoPlayer {
    pub(crate) fn new(options: VideoOptions, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut player = Self {
            id: options.id,
            source: options.source,
            poster: options.poster,
            controls: options.controls,
            autoplay: options.autoplay,
            looping: options.looping,
            muted: options.muted,
            fit: options.fit,
            width: options.width,
            height: options.height,
            style: options.style,
            frame: None,
            playing: false,
            error: None,
            session: None,
            generation: 0,
        };
        if player.autoplay {
            player.start(window, cx);
        }
        player
    }

    pub(crate) fn update_options(
        &mut self,
        options: VideoOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let source_changed = self.source != options.source;
        let muted_changed = self.muted != options.muted;
        let changed = source_changed
            || muted_changed
            || self.poster != options.poster
            || self.controls != options.controls
            || self.autoplay != options.autoplay
            || self.looping != options.looping
            || self.fit != options.fit
            || self.width != options.width
            || self.height != options.height
            || self.style != options.style;
        self.poster = options.poster;
        self.controls = options.controls;
        self.autoplay = options.autoplay;
        self.looping = options.looping;
        self.muted = options.muted;
        self.fit = options.fit;
        self.width = options.width;
        self.height = options.height;
        self.style = options.style;
        if source_changed {
            self.source = options.source;
            self.stop();
            self.frame = None;
            self.error = None;
            if self.autoplay {
                self.start(window, cx);
            }
        } else if muted_changed && self.playing {
            self.start(window, cx);
        }
        if changed {
            cx.notify();
        }
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn set_source(&mut self, source: String, cx: &mut Context<Self>) {
        self.stop();
        self.source = source;
        self.frame = None;
        self.error = None;
        cx.notify();
    }

    pub(crate) fn playback(&mut self, action: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.source.is_empty() {
            return;
        }
        match action {
            "play" if !self.playing => self.toggle(window, cx),
            "pause" if self.playing => self.toggle(window, cx),
            "stop" => {
                self.stop();
                self.frame = None;
                cx.notify();
            }
            _ => {}
        }
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stop();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        match spawn_pipeline(&self.source, self.muted) {
            Ok((session, receiver)) => {
                self.session = Some(session);
                self.playing = true;
                self.error = None;
                cx.spawn_in(window, async move |this, cx| {
                    while let Ok(event) = receiver.recv().await {
                        let done = matches!(event, VideoEvent::End);
                        if this
                            .update_in(cx, |player, window, cx| {
                                if player.generation != generation {
                                    return;
                                }
                                match event {
                                    VideoEvent::Frame(bytes) => {
                                        player.frame = Some(Arc::new(Image::from_bytes(
                                            ImageFormat::Jpeg,
                                            bytes,
                                        )));
                                        cx.notify();
                                    }
                                    VideoEvent::End => player.finished(window, cx),
                                }
                            })
                            .is_err()
                        {
                            break;
                        }
                        if done {
                            break;
                        }
                    }
                })
                .detach();
            }
            Err(error) => {
                self.error = Some(error);
                self.playing = false;
            }
        }
        cx.notify();
    }

    fn finished(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.session = None;
        self.playing = false;
        if self.frame.is_none() {
            self.error = Some("Unable to decode video".into());
        } else if self.looping {
            self.start(window, cx);
            return;
        }
        cx.notify();
    }

    fn stop(&mut self) {
        self.session = None;
        self.playing = false;
        self.generation = self.generation.wrapping_add(1);
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.is_none() {
            self.start(window, cx);
            return;
        }
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let pid = session.child.lock().map(|child| child.id()).ok();
        #[cfg(unix)]
        if let Some(pid) = pid {
            let signal = if self.playing {
                libc::SIGSTOP
            } else {
                libc::SIGCONT
            };
            if unsafe { libc::kill(pid as libc::pid_t, signal) } == 0 {
                self.playing = !self.playing;
            }
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            self.stop();
            let _ = window;
        }
        cx.notify();
    }
}

impl Render for VideoPlayer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let image = if let Some(frame) = &self.frame {
            img(frame.clone())
                .object_fit(self.fit.object_fit())
                .w_full()
                .h_full()
                .into_any_element()
        } else if let Some(poster) = &self.poster {
            let source = if poster.starts_with("https://") || poster.starts_with("http://") {
                ImageSource::from(poster.clone())
            } else {
                ImageSource::from(std::path::PathBuf::from(poster))
            };
            img(source)
                .object_fit(self.fit.object_fit())
                .w_full()
                .h_full()
                .into_any_element()
        } else {
            div()
                .w_full()
                .h_full()
                .bg(rgb(0x151515))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0xffffff))
                .child(self.error.clone().unwrap_or_else(|| {
                    if self.source.is_empty() {
                        "Choose a video file to begin".into()
                    } else {
                        String::new()
                    }
                }))
                .into_any_element()
        };
        let mut video = div()
            .id(self.id.clone())
            .relative()
            .w(px(self.width))
            .h(px(self.height))
            .overflow_hidden()
            .child(image);
        if self.controls {
            let label = if self.playing { "Pause" } else { "Play" };
            video = video.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .p(px(8.0))
                    .bg(rgba(0x000000bb))
                    .child(
                        div()
                            .id(format!("{}/play-pause", self.id))
                            .cursor_pointer()
                            .text_color(rgb(0xffffff))
                            .child(label)
                            .on_click(
                                cx.listener(|player, _, window, cx| player.toggle(window, cx)),
                            ),
                    ),
            );
        }
        video.style().refine(&self.style);
        video
    }
}

fn spawn_pipeline(
    source: &str,
    muted: bool,
) -> Result<(VideoSession, async_channel::Receiver<VideoEvent>), String> {
    let uri = source_uri(source)?;
    let mut child = Command::new("gst-launch-1.0")
        .arg("-q")
        .arg("playbin")
        .arg(format!("uri={uri}"))
        .arg("video-sink=videoconvert ! jpegenc quality=85 ! fdsink fd=1 sync=true")
        .arg(if muted {
            "audio-sink=fakesink"
        } else {
            "audio-sink=autoaudiosink"
        })
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Cannot start GStreamer: {error}"))?;
    let stdout = child.stdout.take().ok_or("GStreamer has no video output")?;
    let child = Arc::new(Mutex::new(child));
    let (sender, receiver) = async_channel::bounded(3);
    let overflow_receiver = receiver.clone();
    let child_for_reader = child.clone();
    std::thread::spawn(move || {
        let mut input = stdout;
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        while let Ok(count) = input.read(&mut chunk) {
            if count == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..count]);
            for frame in take_jpeg_frames(&mut buffer) {
                match sender.try_send(VideoEvent::Frame(frame)) {
                    Ok(()) => {}
                    Err(async_channel::TrySendError::Full(event)) => {
                        let _ = overflow_receiver.try_recv();
                        if sender.try_send(event).is_err() {
                            return;
                        }
                    }
                    Err(async_channel::TrySendError::Closed(_)) => return,
                }
            }
            if buffer.len() > MAX_FRAME_BYTES {
                buffer.clear();
            }
        }
        loop {
            let exited = child_for_reader
                .lock()
                .map(|mut child| child.try_wait().ok().flatten().is_some())
                .unwrap_or(true);
            if exited {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = sender.send_blocking(VideoEvent::End);
    });
    Ok((VideoSession { child }, receiver))
}

fn source_uri(source: &str) -> Result<String, String> {
    if source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("file://")
    {
        return Ok(source.to_owned());
    }
    if source.trim().is_empty() {
        return Err("Video source is empty".into());
    }
    let path = std::path::Path::new(source);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let path = path.to_string_lossy();
    let mut uri = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(uri)
}

fn take_jpeg_frames(buffer: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    loop {
        let Some(start) = buffer.windows(2).position(|bytes| bytes == [0xff, 0xd8]) else {
            if buffer.last() == Some(&0xff) {
                buffer.clear();
                buffer.push(0xff);
            } else {
                buffer.clear();
            }
            break;
        };
        if start > 0 {
            buffer.drain(..start);
        }
        let Some(end) = buffer[2..]
            .windows(2)
            .position(|bytes| bytes == [0xff, 0xd9])
        else {
            break;
        };
        let end = end + 4;
        frames.push(buffer.drain(..end).collect());
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jpeg_frames_across_read_boundaries() {
        let mut buffer = vec![7, 0xff, 0xd8, 1];
        assert!(take_jpeg_frames(&mut buffer).is_empty());
        buffer.extend([2, 0xff, 0xd9, 0xff, 0xd8, 3, 0xff, 0xd9]);
        let frames = take_jpeg_frames(&mut buffer);
        assert_eq!(
            frames,
            vec![
                vec![0xff, 0xd8, 1, 2, 0xff, 0xd9],
                vec![0xff, 0xd8, 3, 0xff, 0xd9]
            ]
        );
        assert!(buffer.is_empty());
    }

    #[test]
    fn file_source_is_encoded_as_uri() {
        assert!(source_uri("a clip.mp4").unwrap().ends_with("/a%20clip.mp4"));
    }

    #[test]
    fn gstreamer_decodes_mp4_frames() {
        if Command::new("gst-launch-1.0")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            return;
        }
        let source = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/clip.mp4");
        let (_session, receiver) = spawn_pipeline(source, true).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut frames = 0;
        let mut ended = false;
        while std::time::Instant::now() < deadline {
            match receiver.try_recv() {
                Ok(VideoEvent::Frame(bytes)) => {
                    assert!(bytes.starts_with(&[0xff, 0xd8]));
                    assert!(bytes.ends_with(&[0xff, 0xd9]));
                    frames += 1;
                }
                Ok(VideoEvent::End) => {
                    ended = true;
                    break;
                }
                Err(async_channel::TryRecvError::Empty) => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(async_channel::TryRecvError::Closed) => break,
            }
        }
        assert!(ended, "GStreamer did not finish decoding");
        assert!(frames > 1, "GStreamer did not produce multiple frames");
    }
}
