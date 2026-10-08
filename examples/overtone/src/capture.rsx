// Lock-free microphone capture; only a worker touches the recording file.
use crate::analysis::MAX_SECONDS;
use cpal::Sample;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    fs::{self, OpenOptions},
    io::BufWriter,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
static NONCE: AtomicU64 = AtomicU64::new(0);
struct Status {
    failed: AtomicBool,
    stop: AtomicBool,
    frames: AtomicU64,
    dropped: AtomicU64,
    peak: AtomicU32,
}
#[derive(Clone, Copy, Debug)]
pub struct CaptureStatus {
    pub seconds: f64,
    pub peak: f32,
    pub dropped_frames: u64,
    pub failed: bool,
    pub complete: bool,
}
pub struct CaptureSession {
    stream: Option<cpal::Stream>,
    worker: Option<JoinHandle<Result<(), String>>>,
    status: Arc<Status>,
    path: PathBuf,
    sample_rate: u32,
    pub device_name: String,
}
impl CaptureSession {
    /// Use the OS default input device. Call on the background/control thread.
    pub fn start() -> Result<Self, String> {
        let device = cpal::default_host()
            .default_input_device()
            .ok_or("No microphone/input device available")?;
        let supported = device
            .default_input_config()
            .map_err(|e| format!("Microphone configuration: {e}"))?;
        let config = supported.config();
        if !(8000..=192000).contains(&config.sample_rate)
            || config.channels == 0
            || config.channels > 32
        {
            return Err("Microphone must support 8–192 kHz and 1–32 channels".into());
        }
        let directory = recording_directory()?;
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let nonce = NONCE.fetch_add(1, Ordering::Relaxed);
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let path = directory.join(format!("take-{epoch}-{}-{nonce}.wav", std::process::id()));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let writer = hound::WavWriter::new(
            BufWriter::new(file),
            hound::WavSpec {
                channels: 1,
                sample_rate: config.sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|e| e.to_string())?;
        let (producer, consumer) = rtrb::RingBuffer::new(config.sample_rate as usize * 2);
        let status = Arc::new(Status {
            failed: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            peak: AtomicU32::new(0),
        });
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => input::<f32>(&device, &config, producer, status.clone()),
            cpal::SampleFormat::F64 => input::<f64>(&device, &config, producer, status.clone()),
            cpal::SampleFormat::I8 => input::<i8>(&device, &config, producer, status.clone()),
            cpal::SampleFormat::U8 => input::<u8>(&device, &config, producer, status.clone()),
            cpal::SampleFormat::I16 => input::<i16>(&device, &config, producer, status.clone()),
            cpal::SampleFormat::U16 => input::<u16>(&device, &config, producer, status.clone()),
            cpal::SampleFormat::I32 => input::<i32>(&device, &config, producer, status.clone()),
            cpal::SampleFormat::U32 => input::<u32>(&device, &config, producer, status.clone()),
            other => Err(format!("Unsupported microphone sample format: {other:?}")),
        };
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                drop(writer);
                let _ = fs::remove_file(&path);
                return Err(e);
            }
        };
        if let Err(e) = stream.play() {
            drop(stream);
            drop(writer);
            let _ = fs::remove_file(&path);
            return Err(format!("Cannot start microphone: {e}"));
        }
        let worker_status = status.clone();
        let worker = thread::spawn(move || write_take(writer, consumer, worker_status));
        Ok(Self {
            stream: Some(stream),
            worker: Some(worker),
            status,
            path,
            sample_rate: config.sample_rate,
            device_name: device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| "Default microphone".into()),
        })
    }
    pub fn status(&self) -> CaptureStatus {
        let frames = self.status.frames.load(Ordering::Relaxed);
        CaptureStatus {
            seconds: frames as f64 / self.sample_rate as f64,
            peak: f32::from_bits(self.status.peak.load(Ordering::Relaxed)),
            dropped_frames: self.status.dropped.load(Ordering::Relaxed),
            failed: self.status.failed.load(Ordering::Relaxed),
            complete: frames >= self.sample_rate as u64 * MAX_SECONDS as u64,
        }
    }
    /// Stop the stream, drain the queue, and finalize WAV before handing it to analysis.
    pub fn finish(mut self) -> Result<PathBuf, String> {
        self.stream.take();
        self.status.stop.store(true, Ordering::Release);
        let result = self
            .worker
            .take()
            .ok_or("Recording already stopped")?
            .join()
            .map_err(|_| "Recording writer failed".to_string())?;
        result.map_err(|e| format!("{e}; recording: {}", self.path.display()))?;
        let status = self.status();
        if status.failed || status.dropped_frames > 0 {
            return Err(format!(
                "Recording interrupted ({} dropped frames). Partial take retained at {}",
                status.dropped_frames,
                self.path.display()
            ));
        }
        if status.seconds < 0.05 {
            let _ = fs::remove_file(&self.path);
            return Err("Recording is too short".into());
        }
        Ok(self.path.clone())
    }
    pub fn discard(self) -> Result<(), String> {
        let path = self.path.clone();
        let _ = self.finish();
        fs::remove_file(path)
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(e)
                }
            })
            .map_err(|e| e.to_string())
    }
}
impl Drop for CaptureSession {
    fn drop(&mut self) {
        self.stream.take();
        self.status.stop.store(true, Ordering::Release);
        // A worker owns its writer and drains/finalizes independently on unexpected shutdown.
        // Explicit finish joins on a background thread. Dropping the UI never waits for disk I/O.
        self.worker.take();
    }
}
fn recording_directory() -> Result<PathBuf, String> {
    if let Some(base) = std::env::var_os("XDG_DATA_HOME").filter(|v| Path::new(v).is_absolute()) {
        return Ok(PathBuf::from(base).join("overtone/recordings"));
    }
    #[cfg(target_os = "windows")]
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(base).join("overtone/recordings"));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or("No user data directory available")?;
    #[cfg(target_os = "macos")]
    let base = PathBuf::from(home).join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(home).join("AppData/Local");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = PathBuf::from(home).join(".local/share");
    Ok(base.join("overtone/recordings"))
}
fn write_take(
    mut writer: hound::WavWriter<BufWriter<std::fs::File>>,
    mut samples: rtrb::Consumer<f32>,
    status: Arc<Status>,
) -> Result<(), String> {
    loop {
        while let Ok(sample) = samples.pop() {
            if let Err(e) = writer.write_sample((sample.clamp(-1., 1.) * 32767.).round() as i16) {
                status.failed.store(true, Ordering::Relaxed);
                status.stop.store(true, Ordering::Release);
                return Err(format!("Recording write failed: {e}"));
            }
        }
        if status.stop.load(Ordering::Acquire) && samples.is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    writer.finalize().map_err(|e| e.to_string())
}
pub(crate) fn capture_frames<T: cpal::Sample>(
    data: &[T],
    channels: usize,
    mut send: impl FnMut(f32) -> bool,
) -> (usize, usize, f32)
where
    f32: cpal::FromSample<T>,
{
    let mut captured = 0;
    let mut dropped = 0;
    let mut peak: f32 = 0.;
    for frame in data.chunks_exact(channels) {
        let value = frame.iter().map(|v| f32::from_sample(*v)).sum::<f32>() / channels as f32;
        let value = if value.is_finite() {
            value.clamp(-1., 1.)
        } else {
            0.
        };
        peak = peak.max(value.abs());
        if send(value) {
            captured += 1;
        } else {
            dropped += 1;
        }
    }
    (captured, dropped, peak)
}
fn input<T: cpal::SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut queue: rtrb::Producer<f32>,
    status: Arc<Status>,
) -> Result<cpal::Stream, String>
where
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    let max = config.sample_rate as u64 * MAX_SECONDS as u64;
    let failed = status.clone();
    device
        .build_input_stream(
            *config,
            move |data: &[T], _| {
                if status.stop.load(Ordering::Relaxed) {
                    return;
                }
                let remaining = max.saturating_sub(status.frames.load(Ordering::Relaxed)) as usize;
                let length = data.len().min(remaining * channels);
                let (captured, dropped, peak) =
                    capture_frames(&data[..length], channels, |v| queue.push(v).is_ok());
                status
                    .frames
                    .fetch_add((captured + dropped) as u64, Ordering::Relaxed);
                status.dropped.fetch_add(dropped as u64, Ordering::Relaxed);
                status.peak.store(peak.to_bits(), Ordering::Relaxed);
            },
            move |_| {
                failed.failed.store(true, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| format!("Cannot open microphone: {e}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_downmixes_converts_and_counts_overruns() {
        let mut out = vec![];
        let (captured, dropped, peak) =
            capture_frames(&[16384i16, 16384, -32768, -32768, 0, 0], 2, |v| {
                if out.len() < 2 {
                    out.push(v);
                    true
                } else {
                    false
                }
            });
        assert_eq!((captured, dropped), (2, 1));
        assert_eq!(out, vec![0.5, -1.]);
        assert_eq!(peak, 1.);
    }
    #[test]
    fn worker_drains_queue_and_finalizes_a_readable_take() {
        let path =
            std::env::temp_dir().join(format!("overtone-capture-test-{}.wav", std::process::id()));
        let writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        // Writer path uses the same buffered file type as the device worker.
        drop(writer);
        let writer = hound::WavWriter::new(
            BufWriter::new(std::fs::File::create(&path).unwrap()),
            hound::WavSpec {
                channels: 1,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        let (mut tx, rx) = rtrb::RingBuffer::new(32);
        for _ in 0..20 {
            tx.push(0.25).unwrap();
        }
        let status = Arc::new(Status {
            failed: AtomicBool::new(false),
            stop: AtomicBool::new(true),
            frames: AtomicU64::new(20),
            dropped: AtomicU64::new(0),
            peak: AtomicU32::new(0),
        });
        write_take(writer, rx, status).unwrap();
        let mut reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.duration(), 20);
        assert!(reader.samples::<i16>().all(|s| s.unwrap() == 8192));
        fs::remove_file(path).unwrap();
    }
}
