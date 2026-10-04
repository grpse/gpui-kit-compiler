pub mod state;

use std::{
    ffi::OsString,
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub struct Media {
    pub path: PathBuf,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub video: String,
    pub audio: Option<String>,
}
pub fn probe(path: &Path) -> Result<Media, String> {
    if !fs::metadata(path).map_err(|e| e.to_string())?.is_file() {
        return Err("Choose a regular video file.".into());
    }
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_type,codec_name,width,height,duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("ffprobe: {e}. Install FFmpeg and put ffmpeg/ffprobe on PATH."))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let streams = json["streams"].as_array().ok_or("No streams found")?;
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video")
        .ok_or("No video stream found")?;
    let duration = json["format"]["duration"]
        .as_str()
        .or_else(|| video["duration"].as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n > 0.)
        .ok_or("Video has no finite duration")?;
    Ok(Media {
        path: path.to_owned(),
        duration,
        width: video["width"].as_u64().unwrap_or(0) as u32,
        height: video["height"].as_u64().unwrap_or(0) as u32,
        video: video["codec_name"].as_str().unwrap_or("unknown").into(),
        audio: streams
            .iter()
            .find(|s| s["codec_type"] == "audio")
            .and_then(|s| s["codec_name"].as_str())
            .map(str::to_owned),
    })
}

pub struct Workspace {
    pub path: PathBuf,
}
impl Workspace {
    pub fn new() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rsx-video-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        Ok(Self { path })
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Debug)]
pub struct Edit {
    pub media: Media,
    pub output: PathBuf,
    pub start: f64,
    pub end: f64,
    pub rotation: u16,
    pub width: Option<u32>,
    pub mute: bool,
}
impl Edit {
    pub fn validate(&self) -> Result<(), String> {
        if !self.start.is_finite()
            || !self.end.is_finite()
            || self.start < 0.
            || self.end <= self.start
            || self.end > self.media.duration + 0.001
        {
            return Err("Trim must satisfy 0 ≤ start < end ≤ duration.".into());
        }
        if !matches!(self.rotation, 0 | 90 | 180 | 270) {
            return Err("Rotation must be 0, 90, 180 or 270 degrees.".into());
        }
        if self.width.is_some_and(|w| w < 2 || w > 7680 || w % 2 != 0) {
            return Err("Output width must be an even number between 2 and 7680.".into());
        }
        if self
            .output
            .extension()
            .and_then(|s| s.to_str())
            .is_none_or(|s| !s.eq_ignore_ascii_case("mp4"))
        {
            return Err("Choose an .mp4 output filename.".into());
        }
        if self.output.exists() {
            return Err("The output already exists. Choose a new filename.".into());
        }
        if self.output == self.media.path {
            return Err("Choose a different output filename.".into());
        }
        if !self.output.parent().unwrap_or(Path::new(".")).is_dir() {
            return Err("The output folder does not exist.".into());
        }
        Ok(())
    }
    pub fn filter(&self) -> String {
        let mut filters = vec![];
        match self.rotation {
            90 => filters.push("transpose=1".into()),
            180 => {
                filters.push("hflip".into());
                filters.push("vflip".into());
            }
            270 => filters.push("transpose=2".into()),
            _ => {}
        }
        // Avoid upscaling; round both dimensions for H.264's yuv420p format.
        filters.push(match self.width {
            Some(width) => format!("scale=w='min({width},trunc(iw/2)*2)':h=-2"),
            None => "scale=trunc(iw/2)*2:trunc(ih/2)*2".into(),
        });
        filters.join(",")
    }
    pub fn args(&self, staging: &Path) -> Vec<OsString> {
        let mut args = [
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-n",
            "-ss",
        ]
        .map(OsString::from)
        .to_vec();
        args.push(format!("{:.6}", self.start).into());
        args.push("-i".into());
        args.push(self.media.path.as_os_str().to_owned());
        args.extend([
            "-t".into(),
            format!("{:.6}", self.end - self.start).into(),
            "-map".into(),
            "0:v:0".into(),
        ]);
        if self.mute {
            args.push("-an".into());
        } else {
            args.extend(["-map", "0:a:0?", "-c:a", "aac", "-b:a", "160k"].map(OsString::from));
        }
        args.extend(["-vf".into(), self.filter().into()]);
        args.extend(
            [
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "23",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
                "-progress",
                "pipe:1",
                "-nostats",
            ]
            .map(OsString::from),
        );
        args.push(staging.as_os_str().to_owned());
        args
    }
}
#[derive(Debug)]
pub enum ExportEvent {
    Progress(f32),
    Finished(Result<PathBuf, String>),
}
fn stderr_tail(mut reader: impl Read) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                if bytes.len() > 65536 {
                    bytes.drain(..bytes.len() - 65536);
                }
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
fn run(
    args: &[OsString],
    cancel: Arc<AtomicBool>,
    progress: Option<(async_channel::Sender<ExportEvent>, f64)>,
) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    let mut child = Command::new("ffmpeg")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}. Install FFmpeg and put it on PATH."))?;
    let stderr = child.stderr.take().unwrap();
    let errors = thread::spawn(move || stderr_tail(stderr));
    let stdout = child.stdout.take().unwrap();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some((sender, duration)) = &progress {
                if let Some(value) = line
                    .strip_prefix("out_time_us=")
                    .and_then(|s| s.parse::<f64>().ok())
                {
                    let _ = sender.try_send(ExportEvent::Progress(
                        (value / 1_000_000. / duration).clamp(0., 0.99) as f32,
                    ));
                }
            }
        }
    });
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            break Err("Cancelled".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e.to_string());
            }
        }
    };
    let _ = reader.join();
    let stderr = errors
        .join()
        .unwrap_or_else(|_| "Could not read FFmpeg log".into());
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(if stderr.trim().is_empty() {
            "FFmpeg exited with an error.".into()
        } else {
            stderr
        }),
        Err(error) => Err(error),
    }
}
pub fn frame(
    media: &Media,
    time: f64,
    rotation: u16,
    width: Option<u32>,
    output: &Path,
    cancel: Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    if output.exists() {
        return Err("Preview output already exists.".into());
    }
    if !time.is_finite() || time < 0. || time >= media.duration {
        return Err("Preview time must be within the clip.".into());
    }
    let edit = Edit {
        media: media.clone(),
        output: output.with_extension("mp4"),
        start: 0.,
        end: media.duration,
        rotation,
        width,
        mute: true,
    };
    let mut args = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-n",
        "-ss",
    ]
    .map(OsString::from)
    .to_vec();
    args.extend([
        format!("{time:.6}").into(),
        "-i".into(),
        media.path.as_os_str().to_owned(),
        "-vf".into(),
        format!("{},scale=640:-2", edit.filter()).into(),
        "-frames:v".into(),
        "1".into(),
        output.as_os_str().to_owned(),
    ]);
    if let Err(error) = run(&args, cancel, None) {
        let _ = fs::remove_file(output);
        return Err(error);
    }
    if !output.is_file() {
        return Err("FFmpeg produced no preview frame. Try an earlier time.".into());
    }
    Ok(output.to_owned())
}
pub fn export(
    edit: &Edit,
    cancel: Arc<AtomicBool>,
    events: async_channel::Sender<ExportEvent>,
) -> Result<PathBuf, String> {
    edit.validate()?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let staging = edit
        .output
        .with_file_name(format!(".rsx-export-{}-{nonce}.mp4", std::process::id()));
    let result = run(
        &edit.args(&staging),
        cancel.clone(),
        Some((events, edit.end - edit.start)),
    )
    .and_then(|_| {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        // Publishing with a hard link atomically refuses an existing destination.
        fs::hard_link(&staging, &edit.output)
            .map_err(|e| format!("Could not publish export: {e}"))?;
        Ok(edit.output.clone())
    });
    let _ = fs::remove_file(staging);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn media(path: PathBuf) -> Media {
        Media {
            path,
            duration: 4.,
            width: 320,
            height: 240,
            video: "h264".into(),
            audio: Some("aac".into()),
        }
    }
    #[test]
    fn validates_ranges_and_preserves_paths_as_separate_arguments() {
        let workspace = Workspace::new().unwrap();
        let edit = Edit {
            media: media(workspace.path.join("source ; $file.mp4")),
            output: workspace.path.join("new.mp4"),
            start: 0.5,
            end: 2.,
            rotation: 90,
            width: Some(720),
            mute: true,
        };
        edit.validate().unwrap();
        let args = edit.args(&workspace.path.join("partial.mp4"));
        assert!(args.contains(&edit.media.path.as_os_str().to_owned()));
        assert!(args.contains(&OsString::from("-an")));
        assert!(edit.filter().contains("transpose=1"));
        let mut invalid = edit.clone();
        invalid.start = f64::NAN;
        assert!(invalid.validate().is_err());
        invalid = edit.clone();
        invalid.end = 5.;
        assert!(invalid.validate().is_err());
        fs::write(&edit.output, "existing").unwrap();
        assert!(edit.validate().unwrap_err().contains("already exists"));
    }
    #[test]
    #[ignore = "requires ffmpeg and ffprobe on PATH"]
    fn ffmpeg_end_to_end_trim_rotate_mute_preview_and_cancel() {
        let workspace = Workspace::new().unwrap();
        let source = workspace.path.join("source ; spaced.mp4");
        let status = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-nostdin",
                "-n",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x240:rate=24",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440",
                "-t",
                "4",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&source)
            .status()
            .unwrap();
        assert!(status.success());
        let media = probe(&source).unwrap();
        assert_eq!(media.width, 320);
        assert!(media.audio.is_some());
        let image = workspace.path.join("frame.png");
        frame(
            &media,
            1.,
            90,
            None,
            &image,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert!(fs::metadata(&image).unwrap().len() > 0);
        let edit = Edit {
            media,
            output: workspace.path.join("edited.mp4"),
            start: 0.5,
            end: 2.,
            rotation: 90,
            width: None,
            mute: true,
        };
        let (tx, _rx) = async_channel::bounded(32);
        export(&edit, Arc::new(AtomicBool::new(false)), tx).unwrap();
        let edited = probe(&edit.output).unwrap();
        assert!((edited.duration - 1.5).abs() < 0.15);
        assert_eq!((edited.width, edited.height), (240, 320));
        assert!(edited.audio.is_none());
        let mut cancelled = edit.clone();
        cancelled.output = workspace.path.join("cancelled.mp4");
        let (tx, _rx) = async_channel::bounded(32);
        assert!(
            export(&cancelled, Arc::new(AtomicBool::new(true)), tx)
                .unwrap_err()
                .contains("Cancelled")
        );
        assert!(!cancelled.output.exists());
        let mut running = edit.clone();
        running.output = workspace.path.join("cancel-running.mp4");
        running.start = 0.;
        running.end = running.media.duration;
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let (tx, rx) = async_channel::bounded(32);
        let watcher = thread::spawn(move || {
            if let Ok(ExportEvent::Progress(_)) = rx.recv_blocking() {
                signal.store(true, Ordering::Relaxed);
            }
        });
        assert!(
            export(&running, cancel, tx)
                .unwrap_err()
                .contains("Cancelled")
        );
        watcher.join().unwrap();
        assert!(!running.output.exists());
        assert!(!fs::read_dir(&workspace.path).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".rsx-export-")
        }));
        let original = fs::read(&edit.output).unwrap();
        let (tx, _rx) = async_channel::bounded(32);
        assert!(export(&edit, Arc::new(AtomicBool::new(false)), tx).is_err());
        assert_eq!(fs::read(&edit.output).unwrap(), original);
    }
}
